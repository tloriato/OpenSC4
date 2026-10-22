"""One nonblocking build/check lease per real worktree, shared by qbuild and fc4verify.

The guardian owns the OS lock (Windows locks are NOT inherited). It also parents
and waits for commands, so a wrapper crash cannot release a running build's lease.
Only an explicitly spawned delegate, verified by OS pipe/socket peer PID, may join.
The parent RPC is synchronous: there is exactly one active command/delegate branch.
This is lifetime supervision, not a job queue: contenders are always rejected.
"""
from __future__ import annotations

import base64
import contextvars
import errno
import json
import os
from pathlib import Path
import secrets
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
from multiprocessing.connection import Client, Listener
from multiprocessing import AuthenticationError

INVITE = "FC4_BUILD_DELEGATION"
LOCK = ".build-check.lock"
OWNER = ".build-check-owner.json"
_current = contextvars.ContextVar("fc4_build_lease", default=None)


class LeaseError(RuntimeError):
    """Busy, invalid delegation, or uncertain ownership; no check was started."""


class _LockBusy(OSError):
    pass


def canonical_worktree(wt):
    wt = Path(os.path.normcase(str(Path(wt).resolve(strict=True))))
    if not wt.is_dir() or not (wt / "engine").is_dir():
        raise LeaseError(f"build/check: not a worktree with an engine directory: {wt}")
    return wt


def _wire(value):
    if isinstance(value, bytes):
        return {"bytes": base64.b64encode(value).decode("ascii")}
    if isinstance(value, os.PathLike):
        return os.fspath(value)
    if isinstance(value, dict):
        return {k: _wire(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [_wire(v) for v in value]
    return value


def _unwire(value):
    if isinstance(value, dict):
        if set(value) == {"bytes"}:
            return base64.b64decode(value["bytes"])
        return {k: _unwire(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_unwire(v) for v in value]
    return value


def _send(stream, value):
    stream.write(json.dumps(_wire(value), ensure_ascii=True) + "\n")
    stream.flush()


def _recv(stream):
    line = stream.readline()
    if not line:
        raise LeaseError("build/check guardian channel closed; ownership uncertain; inspect owner metadata")
    return _unwire(json.loads(line))


def _conn_send(conn, value):
    conn.send_bytes(json.dumps(_wire(value), ensure_ascii=True).encode("utf-8"))


def _conn_recv(conn):
    # A copied authentication key is not peer authorization. Never unpickle
    # anything from a connection, including a not-yet-verified hello.
    return _unwire(json.loads(conn.recv_bytes().decode("utf-8")))


def _peer_pid(conn, server=False):
    if os.name == "nt":
        import ctypes
        from ctypes import wintypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        get_pid = kernel.GetNamedPipeServerProcessId if server else kernel.GetNamedPipeClientProcessId
        get_pid.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.ULONG)]
        get_pid.restype = wintypes.BOOL
        pid = wintypes.ULONG()
        if not get_pid(conn.fileno(), ctypes.byref(pid)):
            raise LeaseError("cannot verify delegated pipe peer PID")
        return pid.value
    if hasattr(socket, "SO_PEERCRED"):
        with socket.socket(fileno=os.dup(conn.fileno())) as sock:
            return struct.unpack("3i", sock.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))[0]
    raise LeaseError("delegation requires OS peer credentials (Windows named pipes or Linux SO_PEERCRED)")


class _KernelLock:
    def __init__(self, path):
        # Never unlink, replace or truncate this inode, including on release.
        self.fd = os.open(path, os.O_RDWR | os.O_CREAT, 0o600)
        try:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(self.fd, msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BaseException as exc:
            os.close(self.fd)
            if isinstance(exc, OSError) and exc.errno in (errno.EACCES, errno.EAGAIN):
                raise _LockBusy(str(exc)) from exc
            raise

    def close(self):
        os.close(self.fd)  # closing releases the process-owned lock on both platforms


def _owner_text(path):
    try:
        d = json.loads(path.read_text(encoding="utf-8"))
        if d.get("status") != "active":
            return f"owner publication/recovery in progress; metadata={path} (do not break the live OS lock)"
        return (f"worktree={d['worktree']} owner PID={d['pid']} guardian PID={d['guardian_pid']} "
                f"invocation={d['invocation']} started={d['started']} source={d['source']} command={d['command']}")
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return f"owner identity unavailable; metadata={path} (do not break the live OS lock)"


def _publish(path, info):
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    os.replace(tmp, path)  # metadata only; never the lock file


class _Delegation:
    """A one-shot, PID-bound child session; even a copied valid token is insufficient."""
    def __init__(self, wt, info):
        self.wt, self.info = wt, info
        self.key = secrets.token_bytes(32)
        if os.name == "nt":
            self.address = r"\\.\pipe\fc4-build-" + secrets.token_hex(16)
            family = "AF_PIPE"
        else:
            self.address = os.path.join(tempfile.gettempdir(), "fc4-build-" + secrets.token_hex(16))
            family = "AF_UNIX"
        self.listener = Listener(self.address, family=family, authkey=self.key)
        self.invite = json.dumps({"address": self.address, "key": self.key.hex(), "worktree": str(wt),
                                  "invocation": info["invocation"], "guardian_pid": os.getpid()})
        self.thread = None
        self.done = threading.Event()
        self.claimed = False
        self.handlers = []

    def start(self, child):
        self.child = child  # Windows process handle / POSIX waitpid identity, not PID existence
        self.pid = child.pid
        self.thread = threading.Thread(target=self.serve, daemon=True)
        self.thread.start()

    def serve(self):
        try:
            while True:
                try:
                    conn = self.listener.accept()
                except (AuthenticationError, EOFError):
                    continue  # a forged/aborted handshake must not disable the real child's session
                except OSError:
                    return
                try:
                    peer = _peer_pid(conn)
                    if (peer not in (self.pid, os.getpid())
                            or (peer == self.pid and (self.claimed or self.child.poll() is not None))):
                        if conn.poll(5):
                            conn.recv_bytes()  # drain hello without trusting/deserializing it
                        _conn_send(conn, {"error": "build/check: delegation rejected (wrong live child/worktree/invocation)"})
                        conn.close()
                        continue
                    if not conn.poll(5):
                        conn.close()
                        continue
                    hello = _conn_recv(conn)
                    if peer == os.getpid() and hello == {"stop": True}:
                        conn.close()
                        return
                    if (peer != self.pid or hello.get("worktree") != str(self.wt)
                            or hello.get("invocation") != self.info["invocation"] or self.claimed
                            or self.child.poll() is not None):
                        _conn_send(conn, {"error": "build/check: delegation rejected (wrong live child/worktree/invocation)"})
                        conn.close()
                        continue
                    self.claimed = True
                    _conn_send(conn, {"owner": self.info})
                    handler = threading.Thread(target=self.handle, args=(conn,), daemon=True)
                    self.handlers.append(handler)
                    handler.start()
                except (EOFError, OSError, LeaseError, ValueError, TypeError, AttributeError):
                    conn.close()
        finally:
            self.done.set()

    def handle(self, conn):
        # One connection, one child. The acceptor remains available to reject
        # copied invitations immediately even while the real child is working.
        with conn:
            while True:
                try:
                    request = _conn_recv(conn)
                    if request.get("delegate"):
                        _conn_send(conn, {"error": "build/check: a delegated child cannot create another branch"})
                    else:
                        _conn_send(conn, _execute(request, self.wt, self.info))
                except (EOFError, OSError):
                    return

    def close(self):
        if self.thread:
            if not self.done.is_set():
                def stop():
                    try:
                        with Client(self.address, authkey=self.key) as conn:
                            _conn_send(conn, {"stop": True})
                    except (OSError, EOFError):
                        pass
                threading.Thread(target=stop, daemon=True).start()
            # If the child crashed while its command is live, keep the lease until
            # that command finishes. Never kill it or infer its death from a PID.
            self.thread.join()
            for handler in self.handlers:
                handler.join()
        self.listener.close()


def _execute(request, wt, info):
    """Guardian alone creates/waits commands. No inherited Windows lock assumptions."""
    delegation = None
    try:
        cmd, kw = request["cmd"], request["kwargs"]
        env = dict(kw.get("env") or os.environ)
        env.pop(INVITE, None)
        kw["env"] = env
        if request.get("delegate"):
            delegation = _Delegation(wt, info)
            env[INVITE] = delegation.invite
        capture = kw.pop("capture_output", False)
        log = request.get("log")
        # Guardian stdout is the private RPC channel. Console output is returned
        # to the caller; fixed step logs are opened only under this live lease.
        with open(log, "ab" if request.get("append") else "wb") if log else _NullFile() as output:
            kw["stdout"] = output if log else subprocess.PIPE
            kw["stderr"] = subprocess.STDOUT if log else subprocess.PIPE
            timeout = kw.pop("timeout", None)
            with subprocess.Popen(cmd, **kw) as child:
                if delegation:
                    delegation.start(child)
                try:
                    stdout, stderr = child.communicate(timeout=timeout)
                except subprocess.TimeoutExpired:
                    # Match subprocess.run's established command timeout behavior.
                    child.kill()
                    child.communicate()
                    return {"timeout": timeout, "cmd": cmd}
                return {"returncode": child.returncode, "stdout": stdout, "stderr": stderr,
                        "console": not capture and not log}
    except OSError as exc:
        return {"oserror": [exc.errno, exc.strerror, exc.filename]}
    except Exception as exc:
        return {"error": f"build/check command supervision failed: {exc}"}
    finally:
        if delegation:
            delegation.close()


class _NullFile:
    def __enter__(self):
        return None

    def __exit__(self, *exc):
        pass


def _guardian():
    lock = None
    owner = None
    info = None
    safe = True
    try:
        request = _recv(sys.stdin)
        wt = canonical_worktree(request["worktree"])
        runs = wt / "build-runs"
        runs.mkdir(exist_ok=True)
        owner = runs / OWNER
        try:
            lock = _KernelLock(runs / LOCK)
        except _LockBusy:
            raise LeaseError("build/check busy (OS lease held; published owner): " + _owner_text(owner)
                             + "; let that invocation finish, then retry")
        if owner.exists():
            try:
                old = json.loads(owner.read_text(encoding="utf-8"))
                if not isinstance(old, dict) or old.get("status") != "released":
                    raise ValueError("unclean guardian")
            except (ValueError, OSError):
                raise LeaseError("build/check ownership uncertain after guardian failure: " + _owner_text(owner)
                                 + "; inspect surviving commands before manually archiving " + str(owner)
                                 + " and retrying; never remove the lock file")
        info = {**request["owner"], "worktree": str(wt), "guardian_pid": os.getpid(),
                "invocation": secrets.token_hex(16), "started": time.strftime("%Y-%m-%d %H:%M:%S"),
                "status": "active"}
        _publish(owner, info)
        _send(sys.stdout, {"owner": info})
        while True:
            line = sys.stdin.readline()
            if not line:
                break  # wrapper crashed; all previously requested commands have been waited
            request = _unwire(json.loads(line))
            if request.get("close"):
                break
            safe = False
            response = _execute(request, wt, info)
            safe = True  # every command and delegated handler has been drained
            _send(sys.stdout, response)
    except (LeaseError, OSError, ValueError) as exc:
        try:
            _send(sys.stdout, {"error": str(exc)})
        except (OSError, ValueError):
            pass
    finally:
        if info is not None and safe:
            _publish(owner, {**info, "status": "released", "finished": time.strftime("%Y-%m-%d %H:%M:%S")})
        if lock:
            lock.close()


class Lease:
    def __init__(self, wt):
        self.wt = canonical_worktree(wt)
        self.process = self.conn = None
        self.mutex = threading.Lock()

    def __enter__(self):
        invitation = os.environ.pop(INVITE, None)
        try:
            if invitation is not None:
                invite = json.loads(invitation)
                if not isinstance(invite, dict):
                    raise LeaseError("build/check: invalid delegation invitation (expected an object)")
                if invite["worktree"] != str(self.wt):
                    raise LeaseError("build/check: delegation rejected for a different canonical worktree")
                owner_path = self.wt / "build-runs" / OWNER
                owner = json.loads(owner_path.read_text(encoding="utf-8"))
                if (owner.get("status") != "active" or owner.get("invocation") != invite["invocation"]
                        or owner.get("guardian_pid") != invite["guardian_pid"]):
                    raise LeaseError("build/check: stale/forged delegation does not match the live owner metadata")
                try:
                    probe = _KernelLock(self.wt / "build-runs" / LOCK)
                except _LockBusy:
                    pass
                else:
                    probe.close()
                    raise LeaseError("build/check: delegation has no live OS lease; ownership uncertain")
                self.conn = _connect(invite)
                if _peer_pid(self.conn, server=True) != owner["guardian_pid"]:
                    raise LeaseError("build/check: delegation server is not the live guardian")
                _conn_send(self.conn, {"worktree": str(self.wt), "invocation": invite["invocation"]})
                if not self.conn.poll(5):
                    raise LeaseError("build/check: delegation handshake timed out; ownership uncertain")
                response = _conn_recv(self.conn)
            else:
                env = dict(os.environ)
                env.pop(INVITE, None)
                self.process = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "--guardian"],
                                                stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
                                                encoding="utf-8", env=env, cwd=self.wt,
                                                start_new_session=os.name != "nt",
                                                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)
                _send(self.process.stdin, {"worktree": str(self.wt), "owner": {
                    "pid": os.getpid(), "command": subprocess.list2cmdline(sys.argv),
                    "source": str(Path(sys.argv[0]).resolve())}})
                response = _recv(self.process.stdout)
            if "error" in response:
                raise LeaseError(response["error"])
            self.info = response["owner"]
            self.token = _current.set(self)
            return self
        except BaseException as exc:
            self._close()
            if isinstance(exc, (OSError, ValueError, KeyError, TypeError, EOFError)):
                raise LeaseError(f"build/check: invalid/stale delegation or failed guardian: {exc}") from exc
            raise

    def _close(self):
        if self.conn:
            self.conn.close()
        if self.process:
            try:
                self.process.stdin.close()
            except OSError:
                pass
            try:
                self.process.wait()  # guardian drains any live command before releasing ownership
            finally:
                self.process.stdout.close()

    def __exit__(self, *exc):
        _current.reset(self.token)
        self._close()

    def run(self, cmd, *, log=None, append=False, delegate=False, **kwargs):
        if not self.mutex.acquire(blocking=False):
            raise LeaseError("build/check: concurrent branches within one invocation are forbidden")
        try:
            request = {"cmd": cmd, "kwargs": kwargs, "log": log, "append": append, "delegate": delegate}
            if self.conn:
                _conn_send(self.conn, request)
                response = _conn_recv(self.conn)
            else:
                _send(self.process.stdin, request)
                response = _recv(self.process.stdout)
            if "error" in response:
                raise LeaseError(response["error"])
            if "oserror" in response:
                raise OSError(*response["oserror"])
            if "timeout" in response:
                raise subprocess.TimeoutExpired(cmd, response["timeout"])
            out, err = response["stdout"], response["stderr"]
            if response["console"]:
                for stream, value in ((sys.stdout, out), (sys.stderr, err)):
                    if value:
                        stream.write(value.decode("utf-8", "replace") if isinstance(value, bytes) else value)
                        stream.flush()
                out = err = None
            return subprocess.CompletedProcess(cmd, response["returncode"], out, err)
        finally:
            self.mutex.release()


def run_process(cmd, **kwargs):
    lease = _current.get()
    if lease is None:
        return subprocess.run(cmd, **kwargs)  # non-check helpers and independent rustfmt --check
    return lease.run(cmd, **kwargs)


def ownership():
    return dict(_current.get().info)


def _connect(invite):
    # Client's authentication handshake can otherwise wait indefinitely on a
    # broken/stale endpoint. Timing out never grants ownership or alters files.
    done = threading.Event()
    abandoned = threading.Event()
    result = []

    def connect():
        try:
            conn = Client(invite["address"], authkey=bytes.fromhex(invite["key"]))
            if abandoned.is_set():
                conn.close()
            else:
                result.append(conn)
        except Exception as exc:
            result.append(exc)
        finally:
            done.set()

    threading.Thread(target=connect, daemon=True).start()
    if not done.wait(5):
        abandoned.set()
        raise LeaseError("build/check: delegation endpoint unavailable; ownership uncertain")
    if isinstance(result[0], Exception):
        raise LeaseError(f"build/check: failed delegation authentication: {result[0]}")
    return result[0]


if __name__ == "__main__":
    _guardian()

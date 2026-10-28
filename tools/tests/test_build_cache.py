"""Cache correctness, including train278 with the real Windows toolchain."""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".claude/lane-bin"))
import cache_compile
from cache_server import foreground_server
import fc4verify


@pytest.mark.skipif(os.name != "nt" or not fc4verify.VCVARS.exists() or not shutil.which("sccache"),
                    reason="requires Windows build tools")
def test_registry_crate_hits_across_worktree_cwds():
    import rust_cache
    manifests = sorted((Path.home() / ".cargo/registry/src").glob("*/adler2-*/Cargo.toml"))
    if not manifests:
        pytest.skip("adler2 registry source is not installed")
    (ROOT / "build-runs").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="rust-cache-", dir=ROOT / "build-runs") as tmp:
        tmp = Path(tmp)
        a, b = tmp / "a", tmp / "b"
        a.mkdir()
        b.mkdir()
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        env = fc4verify.build_env(ROOT, "registry-cache-test")
        env.update(SCCACHE_SERVER_PORT=str(port), SCCACHE_DIR=str(tmp / "cache"),
                   CARGO_MANIFEST_DIR=str(manifests[0].parent))
        env.pop("OUT_DIR", None)
        env.pop("CARGO_PRIMARY_PACKAGE", None)
        with foreground_server(env["FC4_SCCACHE"], env):
            for directory in (a, b):
                args = [sys.executable, rust_cache.__file__, shutil.which("rustc"),
                        "--crate-name", "adler2", "--crate-type=rlib", "--edition=2021",
                        "--emit=dep-info,metadata,link", "-Cdebuginfo=0", "--out-dir", str(directory),
                        str(manifests[0].parent / "src/lib.rs")]
                result = subprocess.run(args, cwd=directory, env=env, capture_output=True, text=True, timeout=60)
                assert result.returncode == 0, result.stdout + result.stderr
                assert (directory / "libadler2.rlib").is_file()
            stats = subprocess.check_output([env["FC4_SCCACHE"], "--show-stats", "--stats-format", "json"],
                                             env=env, text=True, timeout=15)
            counts = json.loads(stats)["stats"]
            assert counts["cache_hits"]["counts"].get("Rust") == 1
            assert counts["cache_misses"]["counts"].get("Rust") == 1


@pytest.mark.parametrize("path,expected", [
    (b"C:\\repo\\.claude\\worktrees\\removed\\engine\\header.h", b"D:/lane/engine/header.h"),
    (b"C:/repo/engine/header.h", b"D:/lane/engine/header.h"),
    (b"C:/repository/header.h", b"C:/repository/header.h"),
    (b"C:/Program Files/SDK/header.h", b"C:/Program Files/SDK/header.h"),
])
def test_include_mapping(path, expected):
    line = b"Note: including file:   " + path + b"\r\n"
    assert cache_compile.rewrite_include(line, "D:/lane", ["C:/repo"]) == (
        b"Note: including file:   " + expected + b"\r\n")
    assert cache_compile.rewrite_include(b"error: " + path, "D:/lane", ["C:/repo"]) == b"error: " + path


@pytest.mark.skipif(os.name != "nt" or not fc4verify.VCVARS.exists() or not shutil.which("sccache"),
                    reason="requires Windows build tools")
def test_real_msvc_replayed_include_prefix():
    """Exercise the mapper with real cached output, independently of Ninja IPC."""
    (ROOT / "build-runs").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="msvc-cache-", dir=ROOT / "build-runs") as tmp:
        tmp = Path(tmp)
        a, b = tmp / "a", tmp / "b"
        for directory in (a, b):
            directory.mkdir()
            (directory / "value.h").write_text("#define VALUE 1\n")
            (directory / "main.cpp").write_text('#include "value.h"\nint main(){return VALUE == 1 ? 0 : 1;}\n')
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        env = fc4verify.build_env(ROOT, "msvc-cache-test")
        env.update(SCCACHE_SERVER_PORT=str(port), SCCACHE_DIR=str(tmp / "cache"),
                   SCCACHE_BASEDIRS=os.pathsep.join(map(str, (a, b))))
        compiler = shutil.which("cl", path=env["PATH"])
        command = [env["FC4_SCCACHE"], compiler, "/nologo", "/showIncludes", "/c", "main.cpp", "/Fomain.obj"]
        with foreground_server(env["FC4_SCCACHE"], env):
            subprocess.run(command, cwd=a, env=env, capture_output=True, check=True, timeout=30)
            mapped = [sys.executable, cache_compile.__file__, "--root", str(b), "--from-root", str(a), "--",
                      *command]
            output = subprocess.check_output(mapped, cwd=b, env=env, timeout=30).replace(b"\\", b"/")
            assert (b / "value.h").as_posix().encode() in output
            assert (a / "value.h").as_posix().encode() not in output
            stats = json.loads(subprocess.check_output(
                [env["FC4_SCCACHE"], "--show-stats", "--stats-format", "json"], env=env, timeout=15))["stats"]
            assert stats["cache_hits"]["counts"].get("C/C++") == 1
            original = (b / "main.obj").read_bytes()
            (b / "value.h").write_text("#define VALUE 2\n")
            subprocess.run(mapped, cwd=b, env=env, capture_output=True, check=True, timeout=30)
            assert (b / "main.obj").read_bytes() != original


@pytest.mark.skipif(os.name != "nt" or not fc4verify.VCVARS.exists() or not shutil.which("sccache"),
                    reason="real MSVC/Ninja/sccache integration requires Windows build tools")
def test_train278_header_change_after_cross_worktree_hit():
    """First demonstrate stale success without mapping; then require a real rebuild.

    No fake compiler or cache: Ninja's deps database and the linked executable
    are inspected. This fails if the launcher is removed from the fixed case.
    """
    from cache_probe import queued_probe
    result = queued_probe(ROOT)
    assert result["unsafe_stale_success"] and result["mapped_header_rebuilt_object_and_test"]


@pytest.mark.parametrize("exit_code", [1, 2])
def test_rust_cache_falls_back_to_the_compiler_when_sccache_is_unreachable(tmp_path, exit_code):
    """A cache server fault (sccache exit 2) must never fail a gate: the wrapper runs the compiler directly."""
    import rust_cache
    fake_sccache = tmp_path / "fake_sccache.py"
    fake_sccache.write_text("import sys\nsys.stderr.write('sccache: error: server unreachable')\nsys.exit(2)\n")
    runner = tmp_path / "sccache.cmd"
    runner.write_text(f'@"{sys.executable}" "{fake_sccache}" %*\n')
    env = dict(os.environ, FC4_SCCACHE=str(runner))
    done = subprocess.run([sys.executable, rust_cache.__file__, sys.executable, "-c", "print('compiled directly')"],
                          env=env, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    assert "compiled directly" in done.stdout
    assert "running the compiler directly" in done.stderr
    # a genuine compiler failure still fails
    bad = subprocess.run([sys.executable, rust_cache.__file__, sys.executable, "-c", "raise SystemExit(1)"],
                         env=dict(os.environ, FC4_SCCACHE=sys.executable), capture_output=True, text=True)
    assert bad.returncode != 0


@pytest.mark.parametrize("gate_passed", [True, False])
def test_queued_cache_proof_is_scoped_to_its_own_passing_ctest(tmp_path, monkeypatch, gate_passed):
    import cache_probe
    import qbuild

    alive = tmp_path / "build-runs/build-queue-worker.alive"
    alive.parent.mkdir()
    alive.touch()
    monkeypatch.setattr(fc4verify, "main_root", lambda root: tmp_path)
    proof = {"cache": {}, "unsafe_stale_success": True, "mapped_header_rebuilt_object_and_test": True}

    def queued(root, steps):
        assert steps == ["build", "ctest"]
        log = tmp_path / "engine/build-x86/Testing/Temporary/LastTest.log"
        log.parent.mkdir(parents=True)
        log.write_text('1/2 Testing: compiler_cache_dependency_regression\n' + json.dumps(proof) +
                       '\nTest Passed.\n2/2 Testing: cargo_test\nTest Failed.\n', encoding="utf-8")
        return gate_passed

    monkeypatch.setattr(qbuild, "queued_fc4verify", queued)
    assert cache_probe.queued_probe(tmp_path) == proof


@pytest.mark.parametrize("name,status", [
    ("compiler_cache_dependency_regression", "Failed"), ("unrelated_test", "Passed")])
def test_queued_cache_proof_rejects_failed_or_unrelated_tests(tmp_path, monkeypatch, name, status):
    import cache_probe
    import qbuild

    alive = tmp_path / "build-runs/build-queue-worker.alive"
    alive.parent.mkdir()
    alive.touch()
    monkeypatch.setattr(fc4verify, "main_root", lambda root: tmp_path)

    def queued(root, steps):
        log = tmp_path / "engine/build-x86/Testing/Temporary/LastTest.log"
        log.parent.mkdir(parents=True)
        log.write_text(f'1/2 Testing: {name}\n' + json.dumps({"cache": {},
                       "mapped_header_rebuilt_object_and_test": True}) + f'\nTest {status}.\n', encoding="utf-8")
        return True

    monkeypatch.setattr(qbuild, "queued_fc4verify", queued)
    with pytest.raises(AssertionError, match="no passing compiler cache regression"):
        cache_probe.queued_probe(tmp_path)

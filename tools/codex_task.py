"""Start and watch Codex tasks for the native port.

    python tools/codex_task.py start <name> --prompt <file> [--dir <checkout>] [--worktree] [--effort high]
    python tools/codex_task.py resume <name> --prompt <file>
    python tools/codex_task.py status

A task is one `codex exec` run, detached, at below-normal priority. Its files live in build-runs/codex-tasks/<name>/:
prompt.txt, events.jsonl (Codex's JSON event stream), final.txt (its last message), pid, cmd.json.

--worktree makes a git worktree at C:/Users/tlori/fc4-wt/<name> on a new branch native/<name> from main and runs
there; without it the task runs in --dir (default: the main checkout). `resume` sends a finished task a follow-up in
the same Codex session and directory; the previous final message is kept as final-<n>.txt.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TASKS = REPO / 'build-runs' / 'codex-tasks'
WORKTREES = Path('C:/Users/tlori/fc4-wt')
NOWIN = getattr(subprocess, 'CREATE_NO_WINDOW', 0)


def alive(pid):
    if os.name != 'nt':
        try:
            os.kill(pid, 0)
            return True
        except OSError:
            return False
    # Windows reuses pids quickly, so the process must still be codex.exe.
    out = subprocess.run(['tasklist', '/FI', f'PID eq {pid}', '/FI', 'IMAGENAME eq codex.exe', '/NH'],
                         capture_output=True, text=True, creationflags=NOWIN).stdout
    return str(pid) in out


def command(cwd, final, effort, model, thread=None):
    cli = shutil.which('codex')
    if not cli:
        sys.exit('codex is not installed')
    # No sandbox: tasks start the game in test slots and attach a debugger to it, which the Windows sandboxes
    # block. The elevated sandbox is never used (it leaks logon sessions).
    head = [cli, '--no-daemon', '-c', 'forced_login_method="chatgpt"', '-c', 'model_provider="openai"',
            '-c', 'approval_policy="never"', '-c', 'web_search="disabled"', '-c', 'features.multi_agent=false',
            '-c', 'shell_environment_policy.set={CARGO_BUILD_JOBS="3"}',
            '-c', f'model_reasoning_effort="{effort}"', '-m', model]
    if thread:
        return head + ['-c', 'sandbox_mode="danger-full-access"', 'exec', 'resume', '--ignore-user-config', '--json',
                       '-o', str(final), thread, '-']
    return head + ['exec', '--ignore-user-config', '--json', '-s', 'danger-full-access', '-C', str(cwd),
                   '-o', str(final), '-']


def launch(d, cmd, cwd, prompt):
    (d / 'cmd.json').write_text(json.dumps({'cmd': cmd, 'cwd': str(cwd), 'started': time.time()}, indent=1))
    flags = NOWIN | getattr(subprocess, 'BELOW_NORMAL_PRIORITY_CLASS', 0) | \
        getattr(subprocess, 'CREATE_NEW_PROCESS_GROUP', 0)
    with open(d / prompt, 'rb') as inp, open(d / 'events.jsonl', 'ab') as out:
        p = subprocess.Popen(cmd, cwd=cwd, stdin=inp, stdout=out, stderr=subprocess.STDOUT, creationflags=flags)
    (d / 'pid').write_text(str(p.pid))
    print(f'{d.name}: pid {p.pid} in {cwd}')


def start(a):
    d = TASKS / a.name
    if (d / 'pid').exists() and alive(int((d / 'pid').read_text())):
        sys.exit(f'{a.name} is already running')
    d.mkdir(parents=True, exist_ok=True)
    cwd = Path(a.dir) if a.dir else REPO
    if a.worktree:
        cwd = WORKTREES / a.name
        if not cwd.exists():
            subprocess.run(['git', '-C', str(REPO), 'worktree', 'add', '-b', f'native/{a.name}', str(cwd), 'main'],
                           check=True)
    shutil.copyfile(a.prompt, d / 'prompt.txt')
    launch(d, command(cwd, d / 'final.txt', a.effort, a.model), cwd, 'prompt.txt')


def resume(a):
    d = TASKS / a.name
    if not (d / 'pid').exists():
        sys.exit(f'{a.name} was never started')
    if alive(int((d / 'pid').read_text())):
        sys.exit(f'{a.name} is still running')
    thread = None
    for line in open(d / 'events.jsonl', 'rb'):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get('type') == 'thread.started':
            thread = e['thread_id']
            break
    if not thread:
        sys.exit(f'{a.name} has no Codex session id in events.jsonl')
    old = json.loads((d / 'cmd.json').read_text())
    n = 1 + len(list(d.glob('followup-*.txt')))
    shutil.copyfile(a.prompt, d / f'followup-{n}.txt')
    if (d / 'final.txt').exists():
        (d / 'final.txt').replace(d / f'final-{n}.txt')
    effort = next((c.split('"')[1] for c in old['cmd'] if c.startswith('model_reasoning_effort=')), a.effort)
    model = old['cmd'][old['cmd'].index('-m') + 1]
    cwd = Path(old['cwd'])
    launch(d, command(cwd, d / 'final.txt', effort, model, thread), cwd, f'followup-{n}.txt')


def last_message(d):
    text = ''
    try:
        for line in open(d / 'events.jsonl', 'rb'):
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if not isinstance(e, dict):
                continue  # a bare JSON value (a lane's tool output can land on its own line)
            item = e.get('item') or {}
            if item.get('type') == 'agent_message':
                text = item.get('text', '')
    except OSError:
        pass
    return text


def status(_):
    for d in sorted(TASKS.glob('*')):
        if not (d / 'pid').exists():
            continue
        pid = int((d / 'pid').read_text())
        state = 'running' if alive(pid) else 'done' if (d / 'final.txt').exists() else 'stopped'
        started = json.loads((d / 'cmd.json').read_text()).get('started', 0)
        mins = (time.time() - started) / 60
        msg = last_message(d).replace('\n', ' ')[:160]
        print(f'{d.name:28} {state:8} {mins:6.0f} min  {msg}')


def main():
    # Task messages carry any Unicode; the Windows console code page cannot print all of it.
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='cmd', required=True)
    s = sub.add_parser('start')
    s.add_argument('name')
    s.add_argument('--prompt', required=True)
    s.add_argument('--dir')
    s.add_argument('--worktree', action='store_true')
    s.add_argument('--effort', default='high')
    s.add_argument('--model', default='gpt-6.1-sol')
    s.set_defaults(fn=start)
    r = sub.add_parser('resume')
    r.add_argument('name')
    r.add_argument('--prompt', required=True)
    r.add_argument('--effort', default='high')
    r.set_defaults(fn=resume)
    sub.add_parser('status').set_defaults(fn=status)
    a = ap.parse_args()
    a.fn(a)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Replace-mode check for engine lanes: your owners switched on in the real game against the original's references
under the fixed schedule (docs/specs/simulator.md 5.8). It runs the coordinator's check from the main checkout on
<WT>/engine/build-x86/FreeCity4.dll; build first (qbuild.py all).

Usage: python qcheck.py --wt <worktree> --replace <keys> [--scenarios cover|<name,name>] [--jobs 4]
  cover (the default): the scenarios that reach your owners' code. One line per scenario: exact, or where it first
  differs: the first delivery whose rand() state differs (day, delivery number, the reference's target vtable), the
  first DIGEST date and fields, and the save records that differ (how many only shift object IDs, from which ID and
  class, and the first record whose content differs, with its owner key and class), then the run directory (its
  game.log has your plugin's lines; end.sc4 is the save).
The full output goes to <WT>/build-runs/qcheck-<n>.log. Exit 0 all exact, 1 a difference, 2 an error. One qcheck at
a time per lane; the game queue runs it beside the other lanes' games.
"""
import argparse
import itertools
import os
import subprocess
import sys
import time

MAIN = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--wt', required=True)
    ap.add_argument('--replace', required=True)
    ap.add_argument('--scenarios', default='cover')
    ap.add_argument('--jobs', type=int, default=4)
    ap.add_argument('--from', dest='from_date', nargs='?', const='auto', default='auto',
                    help='reference snapshot date; default last matched month of the previous failure')
    ap.add_argument('--months', type=int, default=1)
    ap.add_argument('--full', action='store_true', help='run from the original start save')
    ap.add_argument('--no-early-stop', action='store_true')
    ap.add_argument('--legacy', action='store_true', help='old end-save-only references')
    ap.add_argument('--dll', help='explicit candidate DLL (default: worktree build)')
    ap.add_argument('--backend', choices=('queue', 'direct'), default='queue')
    ap.add_argument('--slot', type=int, default=6)
    ap.add_argument('--harness-root')
    ap.add_argument('--store', help='reference store (including the coordinator pin after a repin)')
    a = ap.parse_args()
    dll = a.dll or os.path.join(a.wt, 'engine', 'build-x86', 'FreeCity4.dll')
    if not os.path.isfile(dll):
        print(f'qcheck: no {dll}; build first (qbuild.py --wt <WT> all)')
        return 2
    logs = os.path.join(a.wt, 'build-runs')
    os.makedirs(logs, exist_ok=True)
    log = next(p for p in (os.path.join(logs, f'qcheck-{n}.log') for n in itertools.count(1)) if not os.path.exists(p))
    command = [sys.executable, os.path.join(MAIN, 'tools', 'check.py'), 'run', '--dll', dll, '--replace', a.replace,
               '--scenarios', a.scenarios, '--jobs', str(max(1, min(a.jobs, 6))),
               '--backend', a.backend, '--slot', str(a.slot)]
    if not a.full and not a.legacy:
        command += ['--from', a.from_date, '--months', str(a.months)]
    if a.no_early_stop:
        command += ['--no-early-stop']
    if a.legacy:
        command += ['--legacy']
    if a.harness_root:
        command += ['--harness-root', a.harness_root]
    if a.store:
        command += ['--store', a.store]
    started = time.perf_counter()
    p = subprocess.run(command, cwd=MAIN, capture_output=True, text=True, encoding='utf-8', errors='replace')
    out = p.stdout + p.stderr + f'\nqcheck wall_seconds={time.perf_counter() - started:.3f}\n'
    with open(log, 'w', encoding='utf-8') as f:
        f.write(out)
    lines = out.splitlines()
    for line in lines[:60]:
        print(line[:600])
    if len(lines) > 60:
        print(f'... {len(lines) - 60} more lines in {log}')
    note = os.path.join(MAIN, 'build-runs', 'golden', 'RECORDING')
    if 'no golden baseline' in out:
        # the coordinator writes RECORDING while it re-records the references (a repin): not your code
        print('NOTE: some scenarios have no reference yet ("no golden baseline"), which is not a difference of yours.'
              + (' ' + open(note, encoding='utf-8').read().strip() if os.path.isfile(note) else '')
              + ' Rerun those scenarios later; the others compared normally.')
    print(f'exit {p.returncode}; full output {log}')
    return p.returncode


if __name__ == '__main__':
    sys.exit(main())

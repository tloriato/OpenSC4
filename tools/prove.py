"""Capture and replay original game functions against import-free native ports."""
import argparse
import sys
from pathlib import Path

from prove.common import resolve, name_of, owner_of


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest='command', required=True)
    cap = sub.add_parser('capture')
    cap.add_argument('--functions', required=True)
    cap.add_argument('--scenario', required=True)
    cap.add_argument('--hits', type=int, default=8)
    cap.add_argument('--slot', type=int, choices=(9, 10), default=10)
    cap.add_argument('--timeout', type=int, default=600)
    cap.add_argument('--replay-timeout', type=float, default=30)
    chk = sub.add_parser('check')
    chk.add_argument('functions')
    chk.add_argument('--dll', type=Path, default=Path('engine/target/i686-pc-windows-msvc/release/fc4native.dll'))
    chk.add_argument('--variants', type=int, default=1000)
    chk.add_argument('--seed', type=int, default=0xFC4)
    chk.add_argument('--report', type=Path)
    chk.add_argument('--no-proven', action='store_true', help='for deliberately broken self-test DLLs')
    chk.add_argument('--keep-going', action='store_true', help='test all captures and variants, retaining the first difference')
    sub.add_parser('list')
    selftest = sub.add_parser('selftest', help='build and check the Rust baseline and seventeen mutations')
    selftest.add_argument('--variants', type=int, default=1000)
    selftest.add_argument('--seed', type=int, default=0xFC4)
    args = ap.parse_args()
    if args.command == 'selftest':
        from prove.selftest import run
        return run(args)
    if args.command == 'capture':
        from prove.capture import capture
        if args.hits < 1 or args.timeout < 1:
            ap.error('hits and timeout must be positive')
        return capture(args, resolve(args.functions))
    if args.command == 'check':
        from prove.check import check
        if args.variants < 0:
            ap.error('variants must be nonnegative')
        return check(args, resolve(args.functions))
    from prove.store import all_functions, captures_of, read_capture
    for a in all_functions():
        captures = [read_capture(p) for p in captures_of(a)]
        print(f'{a:08x}\t{name_of(a)}\t{owner_of(a)}\t'
              f'{sum(c["validation"]["valid"] for c in captures)}/{len(captures)} valid\t'
              f'{",".join(sorted(set(c["scenario"] for c in captures)))}')
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError) as exc:
        print(f'prove: {exc}', file=sys.stderr)
        sys.exit(2)

"""Monthly save checkpoints for golden.py (stdlib, no plugin changes).

The script pauses before SaveCity and leaves a short tick-driven capture gap.
The watcher verifies the unique script, suspends ONLY that game's PID while
copying/hashing, and fails closed on missed dates. No live file is a baseline.
"""
import calendar
import argparse
import ctypes
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import time

try:
    from .golden_dbpf import read_records, tgi_text
    from . import golden_relabel
except ImportError:
    from golden_dbpf import read_records, tgi_text
    import golden_relabel

SCHEMA = 1
NORMALIZER = 'class-reference-traversal-v1'
ENTRY_BUDGET = 96 * 1024 * 1024
STORE_BUDGET = 18 * 1024 ** 3


def save_date(records):
    values = [v for k, v in records.items() if k[0] == 0x2990C1E5]
    if len(values) != 1 or len(values[0]) != 91 or values[0][12:14] != b'\x04\x00':
        raise ValueError('save has no unique supported simulator v4 record')
    day = struct.unpack_from('<I', values[0], 87)[0]
    return str(dt.date(2000, 1, 1) + dt.timedelta(days=day - 2451545))


def dates(start, until):
    start, until = dt.date.fromisoformat(start), dt.date.fromisoformat(until)
    if until <= start:
        raise ValueError('--until must follow the checkpoint start date')
    found = []
    cursor = start.replace(day=1)
    while cursor <= until:
        end = cursor.replace(day=calendar.monthrange(cursor.year, cursor.month)[1])
        if start < end <= until:
            found.append(str(end))
        cursor = (end + dt.timedelta(days=1))
    if str(until) not in found:
        found.append(str(until))  # retain the terminal partial month as well
    return found


def instrument(text, start, until):
    """Keep same-day original actions in order, then checkpoint. Never replay old actions."""
    actions = {}
    for line in text.splitlines():
        words = line.split('#', 1)[0].split()
        if not words:
            continue
        if start < words[0] <= until:
            actions.setdefault(words[0], []).append(line)
    checkpoints = dates(start, until)
    out = []
    for day in sorted(set(actions) | set(checkpoints)):
        out.extend(actions.get(day, []))
        if day in checkpoints:
            out.extend(f'{day} {op}' for op in (
                'pause', 'digest', 'save', f'mark GOLDEN_MONTH {day}',
                # The watcher suspends the process inside this gap. A slow
                # copy/hash cannot let simulation run past a differing month.
                'uiwait 2000', 'resume'))
    out.append(f'{until} mark GOLDEN_FULL_SAVE_COMPLETE')
    return '\n'.join(out) + '\n'


def normalized_records(records):
    """Class-aware object-ID relabelling, shared with terminal save comparison.

    Assign IDs by deterministic root/reference traversal, not numeric ID. Keep
    sharing, class, counts and payload bytes. As in golden_relabel.compare,
    class-entry framing CRCs are container metadata, not object content.
    """
    classes = golden_relabel.classes_of(records)
    parsed = {k: golden_relabel.entries(v) for k, v in records.items()
              if golden_relabel.store(k[0], k[1])}
    labels, queue = {}, []

    def label(cls, oid):
        if (cls, oid) not in labels:
            labels[cls, oid] = len(labels) + 1
            queue.append((cls, oid))
        return labels[cls, oid]

    def visit(data):
        for _, oid, cls in golden_relabel.refs(data, classes):
            label(cls, oid)

    objects = {(k[0], oid): payload for k, entries in parsed.items() if entries is not None
               for oid, payload in entries}
    for k in sorted(records):
        if k not in parsed:
            visit(records[k])
    at = 0
    while at < len(queue):
        obj = queue[at]
        at += 1
        if obj in objects:
            visit(objects[obj])
    for k in sorted(parsed):
        for oid, payload in parsed[k] or []:
            label(k[0], oid)
            visit(payload)

    def substitute(data):
        out = bytearray(data)
        for offset, oid, cls in golden_relabel.refs(data, classes):
            struct.pack_into('<I', out, offset, labels[cls, oid])
        return bytes(out)

    out = {}
    for k, value in records.items():
        if k in parsed and parsed[k] is not None:
            parts = sorted((label(k[0], oid), substitute(p)) for oid, p in parsed[k])
            out[k] = b''.join(struct.pack('<II', oid, len(p)) + p for oid, p in parts)
        else:
            out[k] = substitute(value)
    return out


def record_hashes(path):
    records = read_records(path)
    return {'date': save_date(records), 'records': {
        tgi_text(k): hashlib.sha256(v).hexdigest()
        for k, v in sorted(normalized_records(records).items())}}


def difference(expected, actual):
    a, b = expected['records'], actual['records']
    return [{'tgi': k, 'status': 'added' if k not in a else 'removed' if k not in b else 'different'}
            for k in sorted(a.keys() | b.keys()) if a.get(k) != b.get(k)]


def compare(expected, actual):
    if expected.get('normalizer') != NORMALIZER or actual.get('normalizer') != NORMALIZER:
        raise ValueError('monthly normalizer mismatch; re-record the reference once')
    changes, first = [], None
    last_match = expected['start']
    for day, month in sorted(expected['months'].items()):
        other = actual['months'].get(day)
        if other is None:
            if actual.get('stopped_early') and first:
                break
            raise ValueError(f'missing monthly checkpoint {day}')
        records = difference(month, other)
        if records:
            row = {'date': day, 'records': records}
            changes.append(row)
            if first is None:
                first = row
        elif first is None:
            last_match = day
    return {'first_difference': first, 'last_matching_month': last_match,
            'differences': changes, 'complete': not actual.get('stopped_early', False)}


def snapshot_dates(months, budget=ENTRY_BUDGET):
    """Evenly thin snapshots by actual byte size; hashes always retain every month."""
    days = sorted(months)
    for stride in range(1, len(days) + 1):
        chosen = days[::stride]
        if sum(months[d] for d in chosen) <= budget:
            return chosen
    return []


def publish_snapshots(run, stage, available=STORE_BUDGET):
    source = Path(run) / 'months'
    sizes = {p.stem: p.stat().st_size for p in source.glob('*.sc4')}
    chosen = snapshot_dates(sizes, min(ENTRY_BUDGET, available))
    if chosen:
        (Path(stage) / 'months').mkdir()
        for day in chosen:
            shutil.copyfile(source / (day + '.sc4'), Path(stage) / 'months' / (day + '.sc4'))
    return chosen


class GameProcess:
    """A handle pins PID identity. Never run a process-name-wide stop."""
    def __init__(self, pid):
        if os.name != 'nt':
            raise OSError('monthly live capture requires Windows')
        self.kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        self.nt = ctypes.WinDLL('ntdll')
        self.kernel.OpenProcess.restype = ctypes.c_void_p
        self.kernel.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
        for name in ('NtSuspendProcess', 'NtResumeProcess'):
            getattr(self.nt, name).argtypes = [ctypes.c_void_p]
        self.kernel.TerminateProcess.argtypes = [ctypes.c_void_p, ctypes.c_uint]
        self.kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        self.handle = self.kernel.OpenProcess(0x0800 | 0x0001, False, pid)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())

    def suspend(self):
        if self.nt.NtSuspendProcess(self.handle) != 0:
            raise OSError('cannot suspend checkpoint game')

    def resume(self):
        if self.nt.NtResumeProcess(self.handle) != 0:
            raise OSError('cannot resume checkpoint game')

    def stop(self):
        if not self.kernel.TerminateProcess(self.handle, 0):
            raise ctypes.WinError(ctypes.get_last_error())

    def close(self):
        self.kernel.CloseHandle(self.handle)


class Watcher:
    def __init__(self, slot, script, result, start, until, reference=None, early_stop=True):
        self.slot, self.script, self.result = Path(slot), Path(script), Path(result)
        self.until, self.expected_dates = until, dates(start, until)
        self.reference, self.early_stop = reference, early_stop
        self.data = {'schema': SCHEMA, 'normalizer': NORMALIZER, 'start': start, 'until': until, 'months': {}}
        self.process = None

    def tick(self):
        log = self.slot / 'Plugins/FreeCity4.log'
        try:
            text = log.read_text(encoding='utf-8-sig', errors='replace')
        except (FileNotFoundError, PermissionError):
            return
        if str(self.script.resolve()).replace('/', '\\').lower() not in text.replace('/', '\\').lower():
            return
        if self.process is None:
            pid = re.search(r'\[pid (\d+) tid \d+\] DLL_PROCESS_ATTACH', text)
            if not pid:
                return
            if int(pid[1]) != getattr(self, 'expected_pid', int(pid[1])):
                raise ValueError('monthly watcher PID does not match the unique script log')
            self.process = GameProcess(int(pid[1]))
        for day in self.expected_dates:
            if day in self.data['months'] or f'GOLDEN_MONTH {day}' not in text:
                continue
            self.process.suspend()
            stopped = False
            try:
                renamed = re.findall(r'\(SetEstablished\(true\); city name=(.+)\)', text)
                city = renamed[-1] if renamed else self.city
                if not re.fullmatch(r"[A-Za-z0-9 ._()'-]+", city):
                    raise ValueError('invalid checkpoint city name')
                source = self.slot / 'Regions/FC4 Test' / f'City - {city}.sc4'
                dest = self.result / 'months' / (day + '.sc4')
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, dest)
                month = record_hashes(dest)
                if month['date'] != day:
                    raise ValueError(f'missed monthly save: {day}, captured {month["date"]}')
                self.data['months'][day] = month
                differs = self.reference and difference(self.reference['months'][day], month)
                if differs and self.early_stop and day != self.until:
                    self.data['stopped_early'] = True
                    self.data['stop_date'] = day
                    # Publish evidence before terminating; failures never count as a comparison.
                    shutil.copyfile(dest, self.result / 'end.sc4')
                    (self.result / 'game.log').write_text(text, encoding='utf-8')
                    self.write()
                    self.process.stop()
                    stopped = True
            finally:
                if not stopped:
                    self.process.resume()
            self.write()

    def write(self):
        path = self.result / 'monthly.json'
        path.write_text(json.dumps(self.data, sort_keys=True, separators=(',', ':')), encoding='utf-8')

    def close(self):
        if self.process:
            self.process.close()


def watch_main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--slot', type=Path, required=True)
    parser.add_argument('--pid', type=int, required=True)
    args = parser.parse_args()
    config = json.loads(args.config.read_text(encoding='utf-8'))
    if (Path(config['script']).resolve() != args.config.parent.resolve() / 'actions.script'
            or Path(config['result']).resolve() != args.config.parent.resolve() / 'result'):
        raise ValueError('monthly configuration must use its own run directory')
    watcher = Watcher(args.slot, config['script'], config['result'], config['start'], config['until'],
                      reference=config.get('reference'), early_stop=config['early_stop'])
    watcher.city = config['city']
    watcher.expected_pid = args.pid
    deadline = time.monotonic() + config['seconds'] + 150
    try:
        while time.monotonic() < deadline:
            watcher.tick()
            if watcher.data.get('stopped_early') or len(watcher.data['months']) == len(watcher.expected_dates):
                return 0
            time.sleep(0.025)
        raise ValueError('monthly capture timed out')
    except (OSError, ValueError, KeyError) as exc:
        watcher.result.mkdir(parents=True, exist_ok=True)
        (watcher.result / 'monthly-error.txt').write_text(str(exc), encoding='utf-8')
        return 2
    finally:
        watcher.close()


if __name__ == '__main__':
    raise SystemExit(watch_main())


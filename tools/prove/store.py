"""Where captures live: a content-addressed page store plus one small record per captured hit.

build-runs/prove/captures/
  pages.db                         4 KB pages, zlib-compressed, keyed by their 16-byte BLAKE2b digest. A page is
                                   stored once however many captures use it (the exe image is shared by all).
  runs/<run>.json                  per game run: scenario, slot, DLL, module ranges, clock instruction sites
  <address>/<scenario>/<run>-<n>.json.gz
                                   one hit: entry registers and x87/SSE state, the page map (address -> digest) of
                                   every page the original's replay touched plus the stack around ESP, the exit
                                   registers and the exit pages that changed, and the validation verdict.
"""
import gzip
import hashlib
import json
import os
import sqlite3
import threading
import zlib
from pathlib import Path

from .common import CAPTURES, STORE_DB, PAGE

ZERO = bytes(PAGE)


def digest(data):
    return hashlib.blake2b(data, digest_size=16).hexdigest()


class PageStore:
    def __init__(self, path=STORE_DB):
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        self.path = str(path)
        self.db = sqlite3.connect(self.path, timeout=120, check_same_thread=False)
        self.db.execute('PRAGMA journal_mode=WAL')
        self.db.execute('PRAGMA synchronous=NORMAL')
        self.db.execute('CREATE TABLE IF NOT EXISTS pages (h TEXT PRIMARY KEY, z BLOB NOT NULL)')
        self.db.commit()
        self.cache = {}
        self.lock = threading.Lock()

    def put_many(self, pages):
        """pages: iterable of raw 4 KB pages; returns their digests. New pages are compressed and inserted."""
        out, rows = [], []
        for data in pages:
            if len(data) != PAGE:
                raise ValueError('pages are 4096 bytes')
            h = digest(data)
            out.append(h)
            if h not in self.cache:
                rows.append((h, zlib.compress(data, 6)))
                self.cache[h] = data
        if rows:
            with self.lock:
                self.db.executemany('INSERT OR IGNORE INTO pages (h, z) VALUES (?, ?)', rows)
                self.db.commit()
        return out

    def get(self, h):
        data = self.cache.get(h)
        if data is None:
            with self.lock:
                row = self.db.execute('SELECT z FROM pages WHERE h = ?', (h,)).fetchone()
            if row is None:
                raise KeyError(f'page {h} missing from {self.path}')
            data = zlib.decompress(row[0])
            if len(self.cache) > 200_000:
                self.cache.clear()
            self.cache[h] = data
        return data

    def stats(self):
        with self.lock:
            n, z = self.db.execute('SELECT COUNT(*), COALESCE(SUM(LENGTH(z)), 0) FROM pages').fetchone()
        return n, z

    def compressed_size(self, hashes):
        hs = list(set(hashes))
        total = 0
        with self.lock:
            for i in range(0, len(hs), 500):
                chunk = hs[i:i + 500]
                q = 'SELECT COALESCE(SUM(LENGTH(z)), 0) FROM pages WHERE h IN (%s)' % ','.join('?' * len(chunk))
                total += self.db.execute(q, chunk).fetchone()[0]
        return total

    def close(self):
        self.db.close()


def capture_path(func, scen, run, n):
    return CAPTURES / f'{func:08x}' / scen / f'{run}-{n:03d}.json.gz'


def write_capture(rec):
    p = capture_path(rec['function'], rec['scenario'], rec['run'], rec['hit'])
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_suffix('.tmp')
    with gzip.open(tmp, 'wt', encoding='utf-8') as f:
        json.dump(rec, f, separators=(',', ':'))
    os.replace(tmp, p)
    return p


def read_capture(path):
    with gzip.open(path, 'rt', encoding='utf-8') as f:
        rec = json.load(f)
    rec['_path'] = str(path)
    return rec


def captures_of(func):
    d = CAPTURES / f'{func:08x}'
    if not d.is_dir():
        return []
    return sorted(d.glob('*/*.json.gz'))


def all_functions():
    return sorted(int(p.name, 16) for p in CAPTURES.glob('[0-9a-f]' * 8) if p.is_dir())


def write_run(run, meta):
    d = CAPTURES / 'runs'
    d.mkdir(parents=True, exist_ok=True)
    (d / f'{run}.json').write_text(json.dumps(meta, indent=1))


_runs = {}


def read_run(run):
    if run not in _runs:
        _runs[run] = json.loads((CAPTURES / 'runs' / f'{run}.json').read_text())
    return _runs[run]

#!/usr/bin/env python3
"""Generate the save-record fallback labels; registry answers still take precedence.

Run python tools/gen_record_owners.py [--check]. Class identities come from
explicit Class / Class ID tables in the behaviour specs and the cited bindings
below. Coverage patterns are applied in file order. A '-' replace cell may use
the subsystem's sole replace key; ambiguous/no-key subsystems retain their name.
Every inferred replacement has a '?' suffix (it is a lead, not causal proof).
"""
import argparse
import csv
import fnmatch
import io
from pathlib import Path
import re

from check import REPO, registry_record_owners

OUTPUT = REPO / 'tools/record-owners.tsv'
# Non-class records: save-records section 2 / foundations sections 3 and 11.
RECORDS = {
    0x499B23FE: ('class-store index', 'class-store index', 'foundations.md §3.3'),
    0x8A2482B9: ('region thumbnails', 'region-view PNG', 'save-records.md §2'),
    0x2026960B: ('region/city names', 'thumbnail/city/mayor names', 'save-records.md §2'),
    0xEA3FF4F5: ('app version', 'app load/save version', 'foundations.md §3.7'),
    0x0B7D45E0: ('module version', 'module version', 'foundations.md §3.7'),
    0xCA027EDB: ('region data', 'regional-city record', 'foundations.md §11.2'),
    0xA9C8647C: ('3D view', 'cSC4View3DWin', 'save-records.md §2'),
    0x49DD6F4C: ('save root', 'root city reference', 'foundations.md §3.7'),
    0x89D9B9EE: ('city cheat state', 'city auxiliary cheat state', 'save-records.md §2.1'),
    0x8A91E7E0: ('terrain data', 'terrain vertex properties', 'save-records.md §2'),
    0x8A91E7E3: ('terrain data', 'terrain misc data', 'save-records.md §2'),
    0xA9DD6FF4: ('terrain data', 'terrain altitudes/edge strips', 'save-records.md §2'),
    0xE86B1EEF: ('DBPF directory', 'compressed-record directory', 'foundations.md §3.1'),
    0x6534284A: ('exemplar data', 'exemplar resource', 'foundations.md §9.2'),
}
# Identities specified in prose or in differently shaped tables, rather than a
# Class / Class ID table. These are class names, never hand-assigned replace keys.
IDENTITIES = {
    0x8990C372: ('cSC4City', 'save-records.md §3.2'),
    0x0A134F24: ('cSC4NeighborConnection', 'save-records.md §3.1; neighbors.md §4.3'),
    0x2A134F42: ('cSC4NeighborDeal', 'save-records.md §3.1; neighbors.md §5.6'),
    0x8A22BEBE: ('cSC4LandfillOccupant', 'save-records.md §3.1; landfill.md §6.4'),
    0x29D25F1D: ('cSC4FireOccupant', 'save-records.md §3.1; disasters.md §5.11.2'),
    0x6A0F82B2: ('cSC4TrafficNetworkMap', 'save-records.md §3.1; traffic-network.md §6.2'),
    0x6A9335DE: ('cSC4MySimAdvice', 'advisors.md §1.1'),
    0x6B70EFAD: ('cSC4MySimBalloonAdvice', 'advisors.md §1.1'),
    0x8BE3753B: ('cSC4CitySituationAdvice', 'advisors.md §1.1'),
    0x4C0AE114: ('cSC4FirePlaneDispatchSource', 'save-records.md §7.4; aircraft.md §5'),
    0x0C0B06E8: ('cSC4PoliceHelicopterDispatchSource', 'save-records.md §7.4; aircraft.md §5'),
    0xE98F9525: ('cSTETerrain', 'save-records.md §3.2; terrain.md §13.2'),
    0xC9C628EC: ('cSC4CameraControl', 'save-records.md §3.2; camera.md §2'),
    0xE9C622D8: ('cSC43DRender', 'save-records.md §3.2; view3d-window.md §6'),
    0xA9C741B3: ('cSC4ModelMaker', 'save-records.md §3.2; model-maker.md'),
    0xC9B84E10: ('cSTETerrainView3D', 'view3d.md §0; save-records.md §3.2'),
}
for offset in range(2, 12):
    IDENTITIES[0x49B9E600 + offset] = ('cSC4SimGrid', 'simgrid.md §2')


def coverage_rows():
    with (REPO / 'tools/coverage-map.tsv').open(encoding='utf-8') as stream:
        return list(csv.DictReader((line for line in stream if not line.startswith('#')), delimiter='\t'))


def inferred_owner(name, rows):
    for row in rows:
        if fnmatch.fnmatchcase(name, row['pattern']):
            key = row['replace']
            if key == '-':
                keys = {r['replace'] for r in rows
                        if r['subsystem'] == row['subsystem'] and r['replace'] != '-'}
                key = next(iter(keys)) if len(keys) == 1 else '-'
            return (key if key != '-' else row['subsystem']) + '?'
    return None


def class_identities():
    # Existing registration is authoritative for class identities too.
    identities = {t: (name, 'engine/plugin/FC4Subsystems.cpp')
                  for t, (_, name) in registry_record_owners().items()}
    for clsid, identity in IDENTITIES.items():
        identities.setdefault(clsid, identity)
    for path in sorted((REPO / 'docs/specs').glob('*.md')):
        header, section = [], ''
        for line in path.read_text(encoding='utf-8').splitlines():
            if line.startswith('#'):
                section = line.lstrip('# ').split(' ', 1)[0].rstrip('.')
            if not line.startswith('|'):
                header = []
                continue
            cells = [c.strip(' `*') for c in line.strip('|').split('|')]
            if len(cells) >= 2 and cells[0] == 'Class' and (
                    cells[1].startswith(('Class ID', 'ID ', 'ID (')) or cells[1] == 'ID'):
                header = cells
                continue
            if not header:
                continue
            names = re.findall(r'\bc(?:SC4|STE)\w+', cells[0])
            clsid = re.search(r'0x([0-9A-Fa-f]{8})\b', cells[1])
            if len(names) == 1 and clsid:
                identities.setdefault(int(clsid[1], 16), (names[0], f'{path.name} §{section}'))
    return identities


def generate():
    rows, records = coverage_rows(), dict(RECORDS)
    for clsid, (name, source) in class_identities().items():
        owner = inferred_owner(name, rows)
        if owner is not None:
            records.setdefault(clsid, (owner, name, source))
    stream = io.StringIO(newline='')
    writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
    writer.writerow(('type', 'owner', 'class', 'source'))
    for clsid, record in sorted(records.items()):
        writer.writerow((f'{clsid:08X}', *record))
    return stream.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = generate()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_text(encoding='utf-8') != text:
            print('record-owners.tsv is stale; run python tools/gen_record_owners.py')
            return 1
    else:
        with OUTPUT.open('w', encoding='utf-8', newline='') as stream:
            stream.write(text)
    print(f'record-owners.tsv: {len(text.splitlines()) - 1} record types')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

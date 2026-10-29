"""Build the independent Rust fixture and verify all seventeen seeded defects."""
import contextlib
import json
import subprocess
from argparse import Namespace
from pathlib import Path

from .common import REPO
from .check import check

ROOT = REPO / 'build-runs/prove-selftest'
FUNCTIONS = [0x6bd540, 0x6bd810]


def mutations(source):
    start, end = source.index('// Truncate'), source.index('\ntype VCall0')
    conversion = {
        'bare-rust-as': 'fn trunc<T:Into<Wide>>(f:T)->i32 { f.into().scalar() as i32 }\n',
        'cvttss2si': 'fn trunc<T:Into<Wide>>(f:T)->i32 { let f=f.into().scalar();let r:i32;unsafe{asm!("cvttss2si {0}, {1}",out(reg) r,in(xmm_reg) f)};r }\n',
    }
    result = {name: (0x6bd810, source[:start]+code+source[end:]) for name, code in conversion.items()}
    replacements = {
        'off-by-one': ('x0.wrapping_add(size)', 'x0.wrapping_add(size).wrapping_sub(1)'),
        'missing-call': ('core::mem::transmute::<usize, VCall0>(vslot(grid, 0x2C))(grid)', '1'),
        'ecx-plus-four': ('vslot(grid, 0x2C))(grid)', 'vslot(grid, 0x2C))(grid.add(4))'),
        'different-argument': ('(zones, x, z)', '(zones, x.wrapping_add(1), z)'),
        'spike-TractIsZonedBug': ('x0.wrapping_add(size)', 'x0.wrapping_add(size).wrapping_sub(1)'),
        'spike-LookupAltitudeEffectBug': ('trunc(position) as f32', 'trunc(position.plus(0.5)) as f32'),
    }
    for name, (before, after) in replacements.items():
        if before not in source:
            raise ValueError(f'self-test fixture no longer has mutation site: {name}')
        result[name] = (0x6bd810 if 'Lookup' in name else 0x6bd540, source.replace(before, after))
    insert = source.index('    let grid:', source.index('fn fc4p_006bd540'))
    snippets = {
        'extra-call': 'let _:usize=core::mem::transmute::<usize,unsafe extern "thiscall" fn(*const u8)->usize>(0x006bd4b0)(this);',
        'conditional-field-write': 'if tx & 1 == 0 { let p=this.add(0x70) as *mut u8; core::ptr::write_volatile(p,core::ptr::read_volatile(p)^1); }',
        'x87-extra-value': 'asm!("fld1");',
        'x87-control-word': 'let mut cw:u16=0;asm!("fnstcw word ptr [{p}]",p=in(reg) &mut cw);cw^=0x400;asm!("fldcw word ptr [{p}]",p=in(reg) &cw);',
        'mxcsr-rounding': 'let mut mx:u32=0;asm!("stmxcsr dword ptr [{p}]",p=in(reg) &mut mx);mx^=0x2000;asm!("ldmxcsr dword ptr [{p}]",p=in(reg) &mx);',
        'direction-flag': 'asm!("std");',
        'hidden-state': 'core::ptr::write_volatile(&raw mut HIDDEN,core::ptr::read_volatile(&raw const HIDDEN).wrapping_add(1));',
    }
    for name, code in snippets.items():
        text = source[:insert]+'    '+code+'\n'+source[insert:]
        if name == 'hidden-state':
            text += '\nstatic mut HIDDEN:u32=0;\n'
        result[name] = (0x6bd540, text)
    inner = source.replace('fn fc4p_006bd540(', 'fn inner_zoned(')
    result['edi-clobbered'] = (0x6bd540, inner+'''
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "thiscall" fn fc4p_006bd540(_: *const u8,_:i32,_:i32)->u8 {
    core::arch::naked_asm!("push dword ptr [esp+8]","push dword ptr [esp+8]","call {inner}","xor edi,edi","ret 8",inner=sym inner_zoned);
}
''')
    result['original-entry'] = (0x6bd540, source.replace('    let grid: *const u8 = at(this, 0x24);',
        '    return core::mem::transmute::<usize,unsafe extern "thiscall" fn(*const u8,i32,i32)->u8>(0x006bd540)(this,tx,tz);\n'
        '    let grid: *const u8 = at(this, 0x24);', 1))
    assert len(result) == 17
    return result


def build(source, folder):
    folder.mkdir(parents=True, exist_ok=True)
    src, dll = folder/'lib.rs', folder/'port.dll'
    src.write_text(source)
    command = ['rustc', '--edition', '2021', '--crate-type', 'cdylib', '--target', 'i686-pc-windows-msvc',
               '-C', 'opt-level=2', '-C', 'panic=abort', '-C', 'overflow-checks=off',
               '-C', 'link-arg=/NOENTRY', '-C', 'link-arg=/NODEFAULTLIB', '-C', 'link-arg=/BASE:0x5B000000',
               str(src), '-o', str(dll)]
    with (folder/'build.log').open('w') as log:
        code = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, cwd=REPO).returncode
    if code:
        raise ValueError(f'self-test build failed; see {folder / "build.log"}')
    return dll


def run(args):
    source = Path(__file__).with_name('fixtures').joinpath('selftest.rs').read_text()
    cases = {'baseline': (None, source), **mutations(source)}
    summary, failed = [], False
    for name, (changed, text) in cases.items():
        folder = ROOT/'observable'/name
        dll = build(text, folder)
        opts = Namespace(dll=dll, variants=args.variants, seed=args.seed, no_proven=True,
                         report=folder/'report.json', keep_going=False)
        with (folder/'check.log').open('w') as log, contextlib.redirect_stdout(log):
            check(opts, FUNCTIONS)
        report = json.loads(opts.report.read_text())
        expected = {f'{a:08x}': 'DIFFERENT' if a == changed else 'EXACT' for a in FUNCTIONS}
        passed = all(r['verdict'] == expected[r['address']] for r in report['results'])
        failed |= not passed
        summary.append(dict(case=name, changed_function=f'{changed:08x}' if changed else None,
                            passed=passed, report=str(opts.report), results=report['results']))
        print(f'{name}: {"PASS" if passed else "FAIL"}; '+', '.join(r['address']+' '+r['verdict'] for r in report['results']), flush=True)
    destination = ROOT/'observable/summary.json'
    destination.write_text(json.dumps(dict(passed=not failed, cases=summary), indent=2))
    print(f'report: {destination}', flush=True)
    return int(failed)

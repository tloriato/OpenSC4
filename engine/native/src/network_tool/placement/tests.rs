// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{Fake, GameFpu, Object, fake, fake_data};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Scale,
    Allocate(u32, u32),
    Construct(u32, usize, u32),
    AddRef(usize),
    Release(usize, u8),
    Set(usize, u32),
    Clear(usize, u32),
    Points(usize),
    Query(usize, u32),
    Reset(usize),
    Connection(u32, u32),
    Status(u32),
    Support(u32, u32),
    Cell,
    Altitude(u32),
    Transform(u32),
    Insert,
    Usable(usize, u32),
    Free(usize, u8),
}
struct State {
    entry_location: usize,
    calls: Vec<Call>,
    tool: usize,
    allocated: usize,
    interface: usize,
    query_result: u32,
    scale: f64,
    flip_real: Option<u8>,
    points: Vec<u32>,
    sentinel: usize,
    nodes: Vec<usize>,
    occupants: Vec<usize>,
    retained: usize,
    captured_cell: Vec<u8>,
    descriptor: [u32; 3],
    altitudes: Vec<f64>,
    transform_bias: f64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            entry_location: 0,
            calls: vec![],
            tool: 0,
            allocated: 0,
            interface: 0,
            query_result: 0,
            scale: 16.,
            flip_real: None,
            points: vec![],
            sentinel: 0,
            nodes: vec![],
            occupants: vec![],
            retained: 0,
            captured_cell: vec![],
            descriptor: [0; 3],
            altitudes: vec![],
            transform_bias: 0.,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn scale(_this: *mut u8) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Scale);
        s.scale
    })
}
fn allocated(address: u32, size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(address, size));
        s.allocated as *mut u8
    })
}
extern "C" fn alloc_plain(size: u32) -> *mut u8 {
    allocated(0x0061_9E90, size)
}
extern "C" fn alloc_model(size: u32) -> *mut u8 {
    allocated(0x0061_D200, size)
}
extern "C" fn alloc_extended(size: u32) -> *mut u8 {
    allocated(0x0060_5040, size)
}
extern "C" fn alloc_tunnel(size: u32) -> *mut u8 {
    allocated(0x0064_7230, size)
}
fn constructed(address: u32, this: *mut u8, initialize: u32) -> *mut u8 {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Construct(address, this as usize, initialize))
    });
    this
}
extern "thiscall" fn construct_plain(this: *mut u8, initialize: u32) -> *mut u8 {
    constructed(0x0061_BDD0, this, initialize)
}
extern "thiscall" fn construct_model(this: *mut u8, initialize: u32) -> *mut u8 {
    constructed(0x0061_E1A0, this, initialize)
}
extern "thiscall" fn construct_extended(this: *mut u8, initialize: u32) -> *mut u8 {
    constructed(0x0060_5160, this, initialize)
}
extern "thiscall" fn construct_tunnel(this: *mut u8, initialize: u32) -> *mut u8 {
    constructed(0x0064_76F0, this, initialize)
}
extern "thiscall" fn addref(this: *mut u8) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::AddRef(this as usize)));
    5
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let real = unsafe { read(s.tool as *mut u8, 0x50) };
        s.calls.push(Call::Release(this as usize, real));
    });
    registers(0x99AA_BBCC, 0xDDEE_FF00)
}
extern "thiscall" fn set(this: *mut u8, flag: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Set(this as usize, flag)));
    1
}
extern "thiscall" fn clear(this: *mut u8, flag: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Clear(this as usize, flag)));
    1
}
extern "thiscall" fn points(this: *mut u8, points: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Points(this as usize));
        s.points = unsafe { core::slice::from_raw_parts(points, 12) }.to_vec();
        if let Some(real) = s.flip_real.take() {
            unsafe {
                write(s.tool as *mut u8, 0x50, real);
            }
        }
    });
    1
}
extern "thiscall" fn query(this: *mut u8, iid: u32, output: *mut *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.entry_location != 0 {
            assert_eq!(output as usize, unsafe { *(s.entry_location as *const usize) } - 4);
        }
        s.calls.push(Call::Query(this as usize, iid));
        assert!(unsafe { *output }.is_null());
        unsafe {
            *output = s.interface as *mut u8;
        }
        s.query_result
    })
}
extern "thiscall" fn reset(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Reset(this as usize)));
}
extern "thiscall" fn connection(_this: *mut u8, kind: u32, packed: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Connection(kind, packed)));
    1
}
extern "thiscall" fn status(_this: *mut u8, flags: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Status(flags)));
    1
}
extern "thiscall" fn support(_this: *mut u8, id: u32, value: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Support(id, value)));
    registers(0x1122_3344, 0x5566_7788)
}
extern "thiscall" fn construct_cell(this: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Cell);
        unsafe {
            core::ptr::write_bytes(this, 0xA5, 0xB8);
            write(this, 0xC, s.retained as u32);
        }
    });
}
extern "thiscall" fn altitude(_this: *mut u8, vertex: u32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Altitude(vertex));
        s.altitudes.remove(0)
    })
}
extern "thiscall" fn transform(_this: *mut u8, value: f32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Transform(value.to_bits()));
        f64::from(value) + s.transform_bias
    })
}
extern "C" fn allocate_list(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(0x0090_CF54, size));
        assert_eq!(unsafe { read::<u8>(s.tool as *mut u8, 0x50) }, 1);
        s.sentinel as *mut u8
    })
}
extern "thiscall" fn insert(_this: *mut u8, cell: *mut u8, descriptor: *const u32, head: *mut *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Insert);
        assert_eq!(unsafe { read::<u8>(s.tool as *mut u8, 0x50) }, 1);
        s.captured_cell = unsafe { core::slice::from_raw_parts(cell, 0xB8) }.to_vec();
        s.descriptor = unsafe { [*descriptor, *descriptor.add(1), *descriptor.add(2)] };
        let h = unsafe { *head };
        assert_eq!(unsafe { read::<usize>(h, 0) }, h as usize);
        assert_eq!(unsafe { read::<usize>(h, 4) }, h as usize);
        for i in 0..s.nodes.len() {
            let p = s.nodes[i] as *mut u8;
            unsafe {
                write(
                    p,
                    0,
                    if i + 1 == s.nodes.len() {
                        h as u32
                    } else {
                        s.nodes[i + 1] as u32
                    },
                );
                write(p, 8, s.occupants[i] as u32);
            }
        }
        if !s.nodes.is_empty() {
            unsafe {
                write(h, 0, s.nodes[0] as u32);
                write(h, 4, *s.nodes.last().unwrap() as u32);
            }
        }
    });
    1
}
extern "thiscall" fn usable(this: *mut u8, value: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Usable(this as usize, value)));
    1
}
extern "C" fn free(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let real = unsafe { read(s.tool as *mut u8, 0x50) };
        s.calls.push(Call::Free(this as usize, real));
        if this as usize == s.sentinel {
            assert_eq!(unsafe { read::<usize>(this, 0) }, this as usize);
            assert_eq!(unsafe { read::<usize>(this, 4) }, this as usize);
        }
    });
    registers(0x1234_5678, 0x8765_4321)
}
fn fakes() -> Vec<Fake> {
    [
        (0x0061_9E90, alloc_plain as *const ()),
        (0x0061_D200, alloc_model as *const ()),
        (0x0060_5040, alloc_extended as *const ()),
        (0x0064_7230, alloc_tunnel as *const ()),
        (0x0061_BDD0, construct_plain as *const ()),
        (0x0061_E1A0, construct_model as *const ()),
        (0x0060_5160, construct_extended as *const ()),
        (0x0064_76F0, construct_tunnel as *const ()),
        (0x0061_D230, reset as *const ()),
        (0x0064_7850, construct_cell as *const ()),
        (0x0090_CF54, allocate_list as *const ()),
        (0x0090_CF63, free as *const ()),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f as usize))
    .collect()
}
fn table() -> [usize; 75] {
    let mut t = [0usize; 75];
    for (offset, f) in [
        (0, query as *const ()),
        (4, addref as *const ()),
        (8, release as *const ()),
        (0xC, points as *const ()),
        (0x18, scale as *const ()),
        (0x28, set as *const ()),
        (0x2C, clear as *const ()),
        (0x30, altitude as *const ()),
        (0x50, usable as *const ()),
        (0x94, connection as *const ()),
        (0x100, support as *const ()),
        (0x118, status as *const ()),
        (0x120, insert as *const ()),
        (0x128, transform as *const ()),
    ] {
        t[offset / 4] = f as usize;
    }
    t
}
#[test]
fn create_occupant_class_selection_reference_order_flags_connections_and_quad_points() {
    let _fpu = GameFpu::new();
    let _f = fakes();
    let mut correction = 4_294_967_296.0f32;
    let _c = fake_data(0x00A8_0AA8, (&mut correction as *mut f32).cast());
    let mut flags = [0u32; 13];
    for (i, f) in flags.iter_mut().enumerate() {
        *f = 1 << i;
    }
    let _g = fake_data(0x00AA_835C, flags.as_mut_ptr().cast());
    let table = table();
    let mut occupant = [table.as_ptr() as usize];
    let mut old = occupant;
    let mut interface = occupant;
    let mut terrain = occupant;
    let o = occupant.as_mut_ptr().cast::<u8>();
    let oldp = old.as_mut_ptr().cast::<u8>();
    let ip = interface.as_mut_ptr().cast::<u8>();
    for class in [0u32, 0x49C1_A034, 0x49CC_1BCD, 0x8A4B_D52B, o as u32] {
        for real in [0u8, 0x80] {
            for query_result in [0xABCD_0000u32, 0xABCD_0001] {
                let mut tool = Object::new(0x360);
                let t = tool.ptr();
                unsafe {
                    NetworkTool::from_ptr(t).set_terrain(terrain.as_mut_ptr().cast());
                    write(t, 0x50, real);
                    write(t, 0x32C, 0x9876_5432u32);
                }
                let mut cell = [0u8; 0xB8];
                unsafe {
                    write(cell.as_mut_ptr(), 0, 7u32);
                    write(cell.as_mut_ptr(), 4, 9u32);
                    write(cell.as_mut_ptr(), 8, 0x000C_1005u32);
                    write(cell.as_mut_ptr(), 0xC, oldp);
                    write(cell.as_mut_ptr(), 0x50, 0x81u8);
                    for i in 0..4 {
                        write(cell.as_mut_ptr(), 0x58 + i * 4, 100.0 + i as f32);
                    }
                    for i in 0..13 {
                        write(cell.as_mut_ptr(), 0x18 + i * 4, 0x0102_0300u32 + i as u32);
                    }
                }
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        tool: t as usize,
                        allocated: o as usize,
                        interface: ip as usize,
                        query_result,
                        ..State::default()
                    }
                });
                let mut output = core::ptr::null_mut();
                let mut points = [0u32; 14];
                assert_eq!(
                    unsafe { CREATE_OCCUPANT(t, cell.as_mut_ptr(), &mut output, points.as_mut_ptr().cast(), class) },
                    registers(0x99AA_BBCC, 0xDDEE_FF00)
                );
                assert_eq!(output, o);
                assert_eq!(
                    points[..12],
                    [112., 100., 144., 112., 101., 160., 128., 102., 160., 128., 103., 144.].map(f32::to_bits)
                );
                assert_eq!(points[12..], [0, 0]);
                assert_eq!(
                    unsafe { read::<*mut u8>(cell.as_ptr(), 0xC) },
                    if real != 0 { o } else { oldp }
                );
                let mut expected = vec![Call::Scale];
                if class != o as u32 {
                    let (a, c, size) = match class {
                        0 => (0x619E90, 0x61BDD0, 0x180),
                        0x49C1_A034 => (0x61D200, 0x61E1A0, 0x1D8),
                        0x49CC_1BCD => (0x605040, 0x605160, 0x1F0),
                        _ => (0x647230, 0x6476F0, 0x1E0),
                    };
                    expected.push(Call::Allocate(a, size));
                    expected.push(Call::Construct(c, o as usize, 1));
                }
                expected.push(Call::AddRef(o as usize));
                if real != 0 {
                    expected.push(Call::AddRef(o as usize));
                    expected.push(Call::Release(oldp as usize, real));
                }
                expected.push(Call::Set(o as usize, if real != 0 { 0xC1005 } else { 0xC0000 }));
                expected.push(Call::Points(o as usize));
                expected.push(if real != 0 {
                    Call::Clear(o as usize, 0x80000000)
                } else {
                    Call::Set(o as usize, 0x80000000)
                });
                expected.push(Call::Query(o as usize, 0x49C1_A034));
                if query_result as u8 != 0 {
                    expected.push(Call::Reset(ip as usize));
                }
                if class == 0x8A4B_D52B {
                    expected.push(Call::Set(o as usize, 0x10000000));
                    expected.push(Call::Clear(o as usize, 0x4000));
                } else {
                    expected.push(Call::Set(o as usize, 0x4000));
                }
                if real != 0 {
                    for i in [0u32, 2, 12] {
                        expected.push(Call::Connection(i, 0x0102_0300 + i));
                    }
                    expected.push(Call::Status(0x81));
                }
                expected.push(Call::Support(0x9876_5432, 0));
                expected.push(Call::Release(ip as usize, real));
                assert_eq!(calls(), expected);
            }
        }
    }
}
#[test]
fn create_reads_real_mode_again_after_points_and_does_not_addref_an_unchanged_cell_reference() {
    let _fpu = GameFpu::new();
    let _f = fakes();
    let table = table();
    let mut occupant = [table.as_ptr() as usize];
    let o = occupant.as_mut_ptr().cast::<u8>();
    let mut flags = [0u32; 13];
    let _g = fake_data(0x00AA_835C, flags.as_mut_ptr().cast());
    let mut tool = Object::new(0x360);
    let t = tool.ptr();
    unsafe {
        NetworkTool::from_ptr(t).set_terrain(o);
        write(t, 0x50, 1u8);
    }
    let mut cell = [0u8; 0xB8];
    unsafe {
        write(cell.as_mut_ptr(), 0xC, o);
    }
    let mut points = [0u32; 12];
    let mut out = core::ptr::null_mut();
    STATE.with(|s| {
        *s.borrow_mut() = State {
            tool: t as usize,
            flip_real: Some(0),
            ..State::default()
        }
    });
    assert_eq!(
        unsafe { CREATE_OCCUPANT(t, cell.as_mut_ptr(), &mut out, points.as_mut_ptr().cast(), o as u32) },
        registers(0x1122_3344, 0x5566_7788)
    );
    let calls = calls();
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::AddRef(_))).count(), 1);
    assert!(calls.contains(&Call::Set(o as usize, 0x80000000)));
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, Call::Connection(..) | Call::Status(..) | Call::Release(..)))
    );
}
#[test]
fn place_by_id_preserves_core_cell_and_usability_cases_restores_real_before_list_cleanup() {
    let _fpu = GameFpu::new();
    let _f = fakes();
    let table = table();
    let mut flags = [0u32; 13];
    for (i, f) in flags.iter_mut().enumerate() {
        *f = 1 << i;
    }
    let _g = fake_data(0x00AA_835C, flags.as_mut_ptr().cast());
    let mut objects = [[table.as_ptr() as usize]; 3];
    let ops: Vec<_> = objects.iter_mut().map(|o| o.as_mut_ptr() as usize).collect();
    for usable in [0u32, 1, 0x100, 0x80] {
        for saved_real in [0u8, 1, 0x80] {
            for count in [0usize, 2] {
                let mut tool = Object::new(0x360);
                let t = tool.ptr();
                unsafe {
                    write(t, 0, table.as_ptr());
                    NetworkTool::from_ptr(t).set_terrain(ops[0] as *mut u8);
                    write(t, 0x50, saved_real);
                    write(t, 0x248, 65u32);
                    write(t, 0x2C4, 0.25f32);
                    write(t, 0x208, 1u32);
                }
                let mut sentinel = [0xA5A5_A5A5u32; 3];
                let mut nodes = [[0u32; 3]; 2];
                let ns: Vec<_> = nodes.iter_mut().take(count).map(|n| n.as_mut_ptr() as usize).collect();
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        tool: t as usize,
                        sentinel: sentinel.as_mut_ptr() as usize,
                        nodes: ns.clone(),
                        occupants: ops[..count].to_vec(),
                        retained: ops[2],
                        altitudes: vec![100., 101., 102., 103.],
                        ..State::default()
                    }
                });
                assert_eq!(
                    unsafe { fc4p_00624fd0(t, 7, 9, 0x100, usable, 0) },
                    registers(0x99AA_BBCC, 0xDDEE_FF00)
                );
                assert_eq!(unsafe { read::<u8>(t, 0x50) }, saved_real);
                assert_eq!(sentinel[2], 0xA5A5_A5A5);
                STATE.with(|s| {
                    let s = s.borrow();
                    assert_eq!(s.descriptor, [0x100, 2, 0]);
                    let c = s.captured_cell.as_ptr();
                    unsafe {
                        assert_eq!([read::<u32>(c, 0), read(c, 4), read(c, 8)], [7, 9, 1]);
                        assert_eq!(read::<u8>(c, 0x10), 0);
                        for (i, vertex) in [592u32, 657, 658, 593].into_iter().enumerate() {
                            assert_eq!(read::<u32>(c, 0xA8 + i * 4), vertex);
                            assert_eq!(read::<f32>(c, 0x58 + i * 4), 100. + i as f32);
                            assert_eq!(read::<f32>(c, 0x68 + i * 8), 99.75 + i as f32);
                            assert_eq!(read::<f32>(c, 0x6C + i * 8), 100.25 + i as f32);
                        }
                        assert_eq!(read::<u32>(t, 0x208), 1);
                    }
                });
                let mut expected = vec![Call::Cell];
                for (i, vertex) in [592u32, 657, 658, 593].into_iter().enumerate() {
                    expected.push(Call::Altitude(vertex));
                    expected.push(Call::Transform((100. + i as f32).to_bits()));
                }
                expected.push(Call::Allocate(0x0090_CF54, 12));
                expected.push(Call::Insert);
                for &o in &ops[..count] {
                    if usable as u8 != 0 {
                        expected.push(Call::Usable(o, 1));
                        expected.push(Call::Clear(o, 0x4000));
                    }
                    expected.push(Call::Release(o, 1));
                }
                for n in ns {
                    expected.push(Call::Free(n, saved_real));
                }
                expected.push(Call::Free(sentinel.as_ptr() as usize, saved_real));
                expected.push(Call::Release(ops[2], saved_real));
                assert_eq!(calls(), expected);
            }
        }
    }
}

#[test]
fn creation_unsigned_coordinates_round_cell_size_before_multiplication_and_keep_height_bits() {
    let _fpu = GameFpu::new();
    let _f = fakes();
    let table = table();
    let mut occupant = [table.as_ptr() as usize];
    let o = occupant.as_mut_ptr().cast::<u8>();
    let mut correction = 4_294_967_296.0f32;
    let _c = fake_data(0x00A8_0AA8, (&mut correction as *mut f32).cast());
    let mut tool = Object::new(0x360);
    let t = tool.ptr();
    unsafe {
        NetworkTool::from_ptr(t).set_terrain(o);
    }
    for (x, z, expected_x, expected_z) in [
        (0x8000_0001u32, u32::MAX, 2_147_483_648.0f32, 4_294_967_296.0f32),
        (16_777_217, 0, 16_777_216., 0.),
    ] {
        let mut cell = [0u8; 0xB8];
        unsafe {
            write(cell.as_mut_ptr(), 0, x);
            write(cell.as_mut_ptr(), 4, z);
            write(cell.as_mut_ptr(), 0x58, 0x7F80_0011u32);
        }
        let before = cell;
        let mut points = [0u32; 12];
        let mut out = core::ptr::null_mut();
        STATE.with(|s| {
            *s.borrow_mut() = State {
                tool: t as usize,
                scale: 1.000_000_01,
                ..State::default()
            }
        });
        unsafe {
            CREATE_OCCUPANT(t, cell.as_mut_ptr(), &mut out, points.as_mut_ptr().cast(), o as u32);
        }
        assert_eq!(points[0], expected_x.to_bits());
        assert_eq!(points[2], expected_z.to_bits());
        assert_eq!(points[1], 0x7F80_0011);
        assert_eq!(cell, before);
        calls();
    }
}

#[test]
fn placement_uses_unrounded_transform_for_lower_limit_and_rounds_before_upper_limit() {
    let _fpu = GameFpu::new();
    let _f = fakes();
    let table = table();
    let mut flags = [0u32; 13];
    let _g = fake_data(0x00AA_835C, flags.as_mut_ptr().cast());
    let mut terrain = [table.as_ptr() as usize];
    let mut tool = Object::new(0x360);
    let t = tool.ptr();
    unsafe {
        write(t, 0, table.as_ptr());
        NetworkTool::from_ptr(t).set_terrain(terrain.as_mut_ptr().cast());
        write(t, 0x248, u32::MAX);
        write(t, 0x2C4, 1.0f32);
    }
    let mut sentinel = [0u32; 3];
    STATE.with(|s| {
        *s.borrow_mut() = State {
            tool: t as usize,
            sentinel: sentinel.as_mut_ptr() as usize,
            altitudes: vec![1.000_000_01; 4],
            transform_bias: -0.000_000_01,
            ..State::default()
        }
    });
    assert_eq!(
        unsafe { fc4p_00624fd0(t, u32::MAX, u32::MAX, 0x1234, 0, 0) },
        registers(0x1234_5678, 0x8765_4321)
    );
    STATE.with(|s| {
        let s = s.borrow();
        let c = s.captured_cell.as_ptr();
        unsafe {
            assert_eq!(read::<u32>(c, 0x58), 1.0f32.to_bits());
            assert_ne!(read::<u32>(c, 0x68), 0.0f32.to_bits());
            assert_eq!(read::<u32>(c, 0x6C), 2.0f32.to_bits());
            assert_eq!(
                [read::<u32>(c, 0xA8), read(c, 0xAC), read(c, 0xB0), read(c, 0xB4)],
                [0, u32::MAX, 0, 1]
            );
        }
    });
}

fn float_environment() -> (u16, u16, u32, u32) {
    let mut cw = 0u16;
    let mut mxcsr = 0u32;
    let sw: u16;
    let flags: u32;
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
            cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr, out("ax") sw, flags = lateout(reg) flags);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}
#[test]
fn creation_and_placement_preserve_float_environment_and_an_existing_x87_value() {
    let _fpu = GameFpu::new();
    let _f = fakes();
    let table = table();
    let mut terrain = [table.as_ptr() as usize];
    let t = terrain.as_mut_ptr().cast::<u8>();
    let mut flags = [0u32; 13];
    let _g = fake_data(0x00AA_835C, flags.as_mut_ptr().cast());
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    unsafe {
        write(p, 0, table.as_ptr());
        NetworkTool::from_ptr(p).set_terrain(t);
        write(p, 0x248, 65u32);
        write(p, 0x2C4, 0.25f32);
    }
    let mut cell = [0u8; 0xB8];
    let mut points = [0u32; 12];
    let mut output = core::ptr::null_mut();
    let mut sentinel = [0u32; 3];
    STATE.with(|s| {
        *s.borrow_mut() = State {
            tool: p as usize,
            sentinel: sentinel.as_mut_ptr() as usize,
            altitudes: vec![100., 101., 102., 103.],
            ..State::default()
        }
    });
    let value = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &value, out("st(0)") _);
    }
    let before = float_environment();
    unsafe {
        CREATE_OCCUPANT(p, cell.as_mut_ptr(), &mut output, points.as_mut_ptr().cast(), t as u32);
    }
    let after_creation = float_environment();
    unsafe {
        fc4p_00624fd0(p, 7, 9, 0x100, 0, 0);
    }
    let after_placement = float_environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(before, after_creation);
    assert_eq!(before, after_placement);
    assert_eq!(restored.to_bits(), value.to_bits());
}

const CREATE_OCCUPANT: unsafe extern "thiscall" fn(*mut u8, *mut u8, *mut *mut u8, *mut u8, u32) -> u64 =
    unsafe { core::mem::transmute(fc4p_00624210 as unsafe extern "C" fn()) };

#[test]
fn model_query_keeps_the_interface_output_at_the_original_entry_offset() {
    let _fpu = GameFpu::new();
    let _fakes = fakes();
    let table = table();
    let mut occupant = [table.as_ptr() as usize];
    let mut terrain = occupant;
    let mut tool = Object::new(0x360);
    let t = tool.ptr();
    unsafe {
        NetworkTool::from_ptr(t).set_terrain(terrain.as_mut_ptr().cast());
    }
    let mut cell = [0u8; 0xB8];
    let mut output = core::ptr::null_mut::<u8>();
    let mut points = [0u32; 12];
    let o = occupant.as_mut_ptr().cast::<u8>();
    let args = [
        cell.as_mut_ptr() as u32,
        &mut output as *mut *mut u8 as u32,
        points.as_mut_ptr() as u32,
        o as u32,
    ];
    let mut call = super::super::stack_tests::StackCall::new(fc4p_00624210 as *const () as usize, t, &args);
    STATE.with(|s| {
        *s.borrow_mut() = State {
            tool: t as usize,
            entry_location: &mut call.entry as *mut usize as usize,
            ..State::default()
        }
    });
    assert_eq!(unsafe { call.run() }, (0x1122_3344, 0x5566_7788));
    assert_eq!(output, o);
}

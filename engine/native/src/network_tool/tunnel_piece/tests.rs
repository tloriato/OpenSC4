// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Create,
    ClearFlag(u32),
    SetFlag(u32),
    Exemplar(u32, u32),
    ClearCell(u32, u32, u32),
    Data(u32),
    Insert(usize, u32),
    Set(usize, u32, u32),
    Height(usize, u32),
    Begin(usize),
    Update(usize, [u32; 4]),
    End(usize),
    Release,
}

#[derive(Default)]
struct State {
    tool: usize,
    cell: usize,
    occupant: usize,
    heights: [f64; 2],
    height_index: usize,
    calls: Vec<Call>,
    swap_manager: usize,
    swap_terrain: usize,
    set_count: usize,
    mutate_cell: bool,
    occupant_output: usize,
    clear_output_on_end: bool,
    clear_output_on_release: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn create(this: *mut u8, cell: *mut u8, occupant: *mut *mut u8, points: *mut u32, class: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        assert_eq!(cell as usize, s.cell);
        assert_eq!(class, 0x8a4bd52b);
        s.occupant_output = occupant as usize;
        unsafe {
            assert!((*occupant).is_null());
            *occupant = s.occupant as *mut u8;
            for i in 0..12 {
                *points.add(i) = 0xcafe0000 + i as u32;
            }
        }
        s.calls.push(Call::Create);
    });
}

extern "thiscall" fn clear_flag(this: *mut u8, flag: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.occupant);
        s.calls.push(Call::ClearFlag(flag));
    });
}
extern "thiscall" fn set_flag(this: *mut u8, flag: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.occupant);
        s.calls.push(Call::SetFlag(flag));
    });
}
extern "thiscall" fn exemplar(
    this: *mut u8,
    occupant: *mut u8,
    id: u32,
    position: *const u32,
    rotation: u32,
    flags: u32,
) {
    assert_eq!(flags, 0);
    assert_eq!(unsafe { core::slice::from_raw_parts(position, 3) }, [0; 3]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        assert_eq!(occupant as usize, s.occupant);
        s.calls.push(Call::Exemplar(id, rotation));
    });
}
extern "thiscall" fn clear(this: *mut u8, x: u32, z: u32, network: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        s.calls.push(Call::ClearCell(x, z, network));
    });
}
extern "thiscall" fn data(this: *mut u8, value: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.occupant);
        s.calls.push(Call::Data(value));
        if s.swap_manager != 0 {
            unsafe {
                write(s.tool as *mut u8, 0x48, s.swap_manager as u32);
            }
        }
    });
    0xfacecafe
}
extern "thiscall" fn insert_manager(this: *mut u8, value: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Insert(this as usize, value)));
    0xabcdef80
}
extern "thiscall" fn wrong_manager(_: *mut u8, _: u32) -> u32 {
    panic!("method address must be saved before Data")
}

extern "thiscall" fn set_height(this: *mut u8, vertex: u32, height: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Set(this as usize, vertex, height));
        s.set_count += 1;
        if s.set_count == 1 && s.swap_terrain != 0 {
            unsafe {
                write(s.tool as *mut u8, 0x44, s.swap_terrain as u32);
            }
        }
        if s.set_count == 1 && s.mutate_cell {
            // Subsequent corners are read only as their callbacks are reached.
            unsafe {
                write(s.cell as *mut u8, 0xA8 + 4, 0x9000_0001u32);
                write(s.cell as *mut u8, 0x58 + 4, 123.0f32);
            }
        }
    });
}
extern "thiscall" fn height(this: *mut u8, vertex: u32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Height(this as usize, vertex));
        let value = s.heights[s.height_index];
        s.height_index += 1;
        value
    })
}
extern "thiscall" fn begin(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Begin(this as usize)));
}
extern "thiscall" fn update(this: *mut u8, a: u32, b: u32, rect: *const u32, flags: u32) {
    assert_eq!((a, b, flags), (1, 1, 6));
    let rect = unsafe { [*rect, *rect.add(1), *rect.add(2), *rect.add(3)] };
    STATE.with(|s| s.borrow_mut().calls.push(Call::Update(this as usize, rect)));
}
extern "thiscall" fn end(this: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::End(this as usize));
        if s.clear_output_on_end {
            unsafe {
                *(s.occupant_output as *mut u32) = 0;
            }
        }
    });
}
extern "thiscall" fn release(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.occupant);
        s.calls.push(Call::Release);
        if s.clear_output_on_release {
            unsafe {
                *(s.occupant_output as *mut u32) = 0;
            }
        }
    });
    0xdeadbeef
}

// Preserve the three stack slots after the callee pops them, to inspect its caller-owned writes.
#[unsafe(naked)]
unsafe extern "fastcall" fn invoke(_this: *mut u8, _args: *mut u32) -> u32 {
    core::arch::naked_asm!(
        "push esi", "mov esi, edx",
        "push dword ptr [esi + 8]", "push dword ptr [esi + 4]", "push dword ptr [esi]",
        "call {entry}", "sub esp, 12",
        "pop edx", "mov [esi], edx", "pop edx", "mov [esi + 4], edx",
        "pop edx", "mov [esi + 8], edx", "pop esi", "ret",
        entry = sym fc4p_00628390,
    );
}

struct World {
    tool: Object,
    cell: [u8; 0xB8],
    occupant: [usize; 1],
    terrain: [[usize; 1]; 2],
    managers: [[usize; 1]; 2],
    exemplars: [u32; 3],
    rotations: [u32; 3],
    raises: [f32; 3],
    vtables: [Box<[usize; 89]>; 5],
    _fakes: [crate::testing::Fake; 2],
    _fpu: GameFpu,
}

impl World {
    fn new() -> Self {
        let mut w = Self {
            tool: Object::new(0x380),
            cell: [0xA5; 0xB8],
            occupant: [0],
            terrain: [[0]; 2],
            managers: [[0]; 2],
            exemplars: [0x12345678, 0xabcdef00, 0x98765432],
            rotations: [0, 0x123456ff, 2],
            raises: [2.0, 3.0, 5.0],
            vtables: core::array::from_fn(|_| Box::new([0; 89])),
            _fakes: [
                fake(0x624860, exemplar as *const () as usize),
                fake(0x6272f0, clear as *const () as usize),
            ],
            _fpu: GameFpu::new(),
        };
        w.vtables[0][0xd8 / 4] = create as *const () as usize;
        for (offset, f) in [
            (0x2c, clear_flag as *const () as usize),
            (0x28, set_flag as *const () as usize),
            (0xc0, data as *const () as usize),
            (8, release as *const () as usize),
        ] {
            w.vtables[1][offset / 4] = f;
        }
        for (offset, f) in [
            (0xc8, set_height as *const () as usize),
            (0x30, height as *const () as usize),
            (0x15c, begin as *const () as usize),
            (0x110, update as *const () as usize),
            (0x160, end as *const () as usize),
        ] {
            w.vtables[2][offset / 4] = f;
        }
        w.vtables[3][0x50 / 4] = insert_manager as *const () as usize;
        w.vtables[4][0x50 / 4] = wrong_manager as *const () as usize;
        w
    }

    fn prepare(&mut self, heights: [f64; 2], manager: bool) {
        unsafe {
            write(self.tool.ptr(), 0, self.vtables[0].as_ptr());
            self.occupant[0] = self.vtables[1].as_ptr() as usize;
            self.terrain[0][0] = self.vtables[2].as_ptr() as usize;
            self.terrain[1][0] = self.vtables[2].as_ptr() as usize;
            self.managers[0][0] = self.vtables[3].as_ptr() as usize;
            self.managers[1][0] = self.vtables[4].as_ptr() as usize;
            write(self.tool.ptr(), 0x44, self.terrain[0].as_mut_ptr());
            write(
                self.tool.ptr(),
                0x48,
                if manager {
                    self.managers[0].as_mut_ptr()
                } else {
                    core::ptr::null_mut()
                },
            );
            write(self.tool.ptr(), 0x208, 0x87654321u32);
            let tool = TunnelTool::from_ptr(self.tool.ptr());
            tool.set_x_stride(10);
            tool.set_z_stride(10);
            tool.set_exemplars(self.exemplars.as_mut_ptr().cast());
            tool.set_exemplars_end(self.exemplars.as_mut_ptr().add(3).cast());
            tool.set_rotations(self.rotations.as_mut_ptr().cast());
            tool.set_raises(self.raises.as_mut_ptr().cast());
            tool.set_clear_network(0x10203040);
            write(self.cell.as_mut_ptr(), 0, 3u32);
            write(self.cell.as_mut_ptr(), 4, 4u32);
            for (i, v) in [43u32, 53, 54, 44].into_iter().enumerate() {
                write(self.cell.as_mut_ptr(), 0xA8 + 4 * i, v);
            }
            for (i, v) in [10.0f32, 20.0, 30.0, 40.0].into_iter().enumerate() {
                write(self.cell.as_mut_ptr(), 0x58 + 4 * i, v);
            }
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                tool: self.tool.ptr() as usize,
                cell: self.cell.as_mut_ptr() as usize,
                occupant: self.occupant.as_mut_ptr() as usize,
                heights,
                ..State::default()
            }
        });
    }

    fn run(&mut self, direction: u32, kind: u32) -> [u32; 3] {
        let mut args = [direction, kind, self.cell.as_mut_ptr() as u32];
        assert_eq!(
            unsafe { invoke(self.tool.ptr(), args.as_mut_ptr()) },
            self.occupant.as_mut_ptr() as u32
        );
        args
    }
}

#[test]
fn empty_exemplar_range_returns_null_without_calls_or_argument_writes() {
    let mut tool = Object::new(0x380);
    let mut args = [0xf1234567, 0xabcdef12, 0x11223344];
    STATE.with(|s| *s.borrow_mut() = State::default());
    let before = args;
    assert_eq!(unsafe { invoke(tool.ptr(), args.as_mut_ptr()) }, 0);
    assert_eq!(args, before);
    STATE.with(|s| assert!(s.borrow().calls.is_empty()));
}

#[test]
fn all_directions_types_low_byte_terrain_branches_and_caller_rectangle() {
    for direction in [0u32, 1, 2, 3, 0xabcdef00, 0xffffffff] {
        for kind in [0u32, 1, 0xabcdef02] {
            for mask in 0..4 {
                let mut w = World::new();
                w.prepare(
                    [
                        if mask & 1 != 0 { -1.0 } else { 100.0 },
                        if mask & 2 != 0 { -1.0 } else { 100.0 },
                    ],
                    mask & 1 != 0,
                );
                let expected_tool = unsafe { core::slice::from_raw_parts(w.tool.ptr(), 0x380).to_vec() };
                let mut expected_cell = w.cell;
                let vertices = [43u32, 53, 54, 44];
                let first = (!direction & 3) as usize;
                let second = (first + 1) & 3;
                let mut heights = [10.0f32, 20.0, 30.0, 40.0];
                let index = (kind as u8) as usize;
                heights[first] += w.raises[index];
                heights[second] += w.raises[index];
                for (i, h) in heights.iter().enumerate() {
                    expected_cell[0x58 + 4 * i..0x5c + 4 * i].copy_from_slice(&h.to_bits().to_le_bytes());
                }
                if mask & 1 != 0 {
                    expected_cell[0x54] = 0x80;
                }
                let terrain = w.terrain[0].as_ptr() as usize;
                let manager = w.managers[0].as_ptr() as usize;
                let mut calls = vec![
                    Call::Create,
                    Call::ClearFlag(0x2000000),
                    Call::ClearFlag(0x4000000),
                    Call::ClearFlag(0x4000),
                    Call::SetFlag(0x10000000),
                    Call::Exemplar(
                        w.exemplars[index],
                        ((w.rotations[index] as u8).wrapping_add(direction as u8) & 3) as u32,
                    ),
                    Call::ClearCell(3, 4, 0x10203040),
                ];
                if mask & 1 != 0 {
                    calls.extend([Call::Data(0x87654321), Call::Insert(manager, 0xfacecafe)]);
                }
                for (vertex, height) in vertices.into_iter().zip(heights) {
                    calls.push(Call::Set(terrain, vertex, height.to_bits()));
                }
                let mut rect = [
                    vertices[first].min(vertices[second]) % 10,
                    vertices[first].min(vertices[second]) / 10,
                    vertices[first].max(vertices[second]) % 10,
                    vertices[first].max(vertices[second]) / 10,
                ];
                // Choose min/max separately per coordinate, as vertical edges may have different vertex rows.
                rect[0] = (vertices[first] % 10).min(vertices[second] % 10);
                rect[2] = (vertices[first] % 10).max(vertices[second] % 10);
                for (bit, corner, neighbour) in [(1, first, (first + 3) & 3), (2, second, (second + 1) & 3)] {
                    let vertex = vertices[corner] * 2 - vertices[neighbour];
                    calls.push(Call::Height(terrain, vertex));
                    if mask & bit != 0 {
                        calls.push(Call::Set(terrain, vertex, heights[corner].to_bits()));
                        rect[0] = rect[0].min(vertex % 10);
                        rect[2] = rect[2].max(vertex % 10);
                        rect[1] = rect[1].min(vertex / 10);
                        rect[3] = rect[3].max(vertex / 10);
                    }
                }
                calls.extend([
                    Call::Begin(terrain),
                    Call::Update(terrain, rect),
                    Call::End(terrain),
                    Call::Release,
                ]);
                assert_eq!(w.run(direction, kind), [rect[0], rect[1], rect[2]]);
                assert_eq!(w.cell, expected_cell);
                assert_eq!(
                    unsafe { core::slice::from_raw_parts(w.tool.ptr(), 0x380) },
                    expected_tool
                );
                STATE.with(|s| assert_eq!(s.borrow().calls, calls));
            }
        }
    }
}

#[test]
fn final_occupant_pointer_is_loaded_after_terrain_end_and_saved_across_release() {
    for clear_at_end in [false, true] {
        let mut w = World::new();
        w.prepare([1000.0; 2], false);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.clear_output_on_end = clear_at_end;
            s.clear_output_on_release = !clear_at_end;
        });
        let mut args = [0, 1, w.cell.as_mut_ptr() as u32];
        let result = unsafe { invoke(w.tool.ptr(), args.as_mut_ptr()) };
        assert_eq!(
            result,
            if clear_at_end {
                0
            } else {
                w.occupant.as_mut_ptr() as u32
            }
        );
        STATE.with(|s| assert_eq!(s.borrow().calls.contains(&Call::Release), !clear_at_end));
    }
}

#[test]
fn st0_comparison_keeps_extended_precision_and_rejects_equal_or_unordered() {
    for (values, extra) in [
        ([43.0 - 0.00000001, f64::NAN], true),
        ([43.0, 13.0], false),
        ([f64::NAN, f64::INFINITY], false),
    ] {
        let mut w = World::new();
        w.prepare(values, false);
        w.run(0, 1); // first corner 3 -> 43, second corner 0 -> 13.
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(
                s.calls.iter().filter(|c| matches!(c, Call::Set(..))).count(),
                4 + usize::from(extra)
            );
        });
    }
}

#[test]
fn callbacks_reread_terrain_cell_fields_and_manager_pointer_but_keep_saved_vtable() {
    let mut w = World::new();
    w.prepare([1000.0; 2], true);
    let first_terrain = w.terrain[0].as_ptr() as usize;
    let second_terrain = w.terrain[1].as_ptr() as usize;
    let second_manager = w.managers[1].as_ptr() as usize;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.swap_manager = second_manager;
        s.swap_terrain = second_terrain;
        s.mutate_cell = true;
    });
    w.run(0, 1);
    STATE.with(|s| {
        let s = s.borrow();
        assert!(s.calls.contains(&Call::Insert(second_manager, 0xfacecafe)));
        let sets: Vec<_> = s.calls.iter().filter(|c| matches!(c, Call::Set(..))).collect();
        assert_eq!(sets[0], &Call::Set(first_terrain, 43, 13.0f32.to_bits()));
        assert_eq!(sets[1], &Call::Set(second_terrain, 0x90000001, 123.0f32.to_bits()));
        assert!(s.calls.contains(&Call::End(second_terrain)));
    });
}

#[test]
fn unsigned_division_signed_rectangle_bounds_and_wrapped_vertex_extension() {
    let mut w = World::new();
    w.prepare([-100.0; 2], false);
    unsafe {
        TunnelTool::from_ptr(w.tool.ptr()).set_x_stride(u32::MAX);
        TunnelTool::from_ptr(w.tool.ptr()).set_z_stride(1);
        for (i, vertex) in [0x80000001u32, 0x80000002, 3, 4].into_iter().enumerate() {
            write(w.cell.as_mut_ptr(), 0xA8 + i * 4, vertex);
        }
    }
    assert_eq!(w.run(0, 0), [0x80000000, 0x80000000, 5]);
    STATE.with(|s| {
        let s = s.borrow();
        let terrain = w.terrain[0].as_ptr() as usize;
        assert!(s.calls.contains(&Call::Height(terrain, 5)));
        assert!(s.calls.contains(&Call::Height(terrain, 0x80000000)));
        assert!(s.calls.contains(&Call::Update(terrain, [0x80000000, 0x80000000, 5, 5])));
    });
}

#[test]
fn raising_edge_preserves_stored_nan_payloads_signed_zero_and_x87_rounding() {
    let _fpu = GameFpu::new();
    let cases: [(u32, u32, u32, [u32; 2]); 4] = [
        (0x80000000u32, 0x80000000, 0, [0x80000000, 0]),
        (0x3f800000, 0x7fc12345, 0x7f812345, [0x7fc12345, 0x7fc12345]),
        (0x7fc00123, 0x7fc12345, 0x7fc23456, [0x7fc12345, 0x7fc23456]),
        (0x3f800000, 0x4b800000, 0xcb800000, [0x4b800000, 0xcb7fffff]),
    ];
    for (raise, first, second, expected) in cases {
        let mut words = [first, second];
        unsafe {
            raise_edge(
                F80::from_f32(f32::from_bits(raise)),
                words.as_mut_ptr().cast(),
                words.as_mut_ptr().add(1).cast(),
            );
        }
        assert_eq!(words, expected);
    }
}

fn environment() -> (u16, u16, u32, u32) {
    let mut cw = 0u16;
    let mut mxcsr = 0u32;
    let sw: u16;
    let flags: u32;
    unsafe {
        core::arch::asm!(
            "fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
            cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
            out("ax") sw, flags = lateout(reg) flags,
        );
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn insertion_preserves_control_word_x87_stack_value_mxcsr_and_direction_flag() {
    let mut w = World::new();
    w.prepare([1000.0; 2], true);
    let value = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{value}]", value = in(reg) &value, out("st(0)") _);
    }
    let before = environment();
    w.run(0, 1);
    let after = environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{value}]", value = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(before, after);
    assert_eq!(restored.to_bits(), value.to_bits());
}

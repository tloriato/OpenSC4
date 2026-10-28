// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Native-layout versions of the core's scalar, grid and supply tests, with original-call fakes.

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

const SIZE: usize = 0x250;
const EAX: u32 = 0xA1B2_C3D4;
const EDX: u32 = 0xE5F6_0718;

unsafe fn invoke(address: u32, this: *mut u8, args: &[u32]) -> u64 {
    // SAFETY: each test supplies the original stack words and a live native-layout object.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, crate::testing::resolve(address), this as u32, args);
        call.eax = EAX;
        call.edx = EDX;
        call_raw(&mut call);
        registers(call.eax, call.edx)
    }
}

unsafe fn bytes(this: *mut u8) -> Vec<u8> {
    // SAFETY: test simulator objects have SIZE bytes.
    unsafe { std::slice::from_raw_parts(this, SIZE).to_vec() }
}

fn unchanged_except(before: &[u8], after: &[u8], ranges: &[(usize, usize)]) {
    for i in 0..before.len() {
        if !ranges.iter().any(|&(start, len)| (start..start + len).contains(&i)) {
            assert_eq!(after[i], before[i], "unexpected write at +{i:#x}");
        }
    }
}

#[test]
fn scalar_accessors_read_original_widths_preserve_registers_and_write_nothing() {
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    // SAFETY: every tested field lies inside the object.
    unsafe {
        let sim = PollutionSim::from_ptr(this);
        sim.set_uncollected(0xFEDC_BA98);
        sim.set_average_water(-500);
        sim.set_average_garbage(0xFEDC);
        sim.set_capacity(i32::MIN);
        sim.set_produced_previous(-0x1234_5678_9ABC_DEF0);
        sim.set_energy_previous(i64::MIN);
        sim.set_landfilled_previous(i64::MAX);
        sim.set_abducted_previous(u32::MAX);
        let before = bytes(this);
        for (address, expected) in [
            (0x006E_2CC0, registers(0x8990_C065, EDX)),
            (0x006E_3000, registers(0xFEDC_BA98, EDX)),
            (0x006E_3010, registers((-500i32) as u32, EDX)),
            (0x006E_3020, registers(0xFEDC, EDX)),
            (0x006E_3150, registers(this.add(0x134) as u32, EDX)),
            (0x006E_4F20, u64::from(u32::MAX)),
            (0x006E_4F30, (-0x1234_5678_9ABC_DEF0i64) as u64),
            (0x006E_4F80, i64::MIN as u64),
            (0x006E_4F90, i64::MAX as u64),
            (0x006E_5030, registers(0xE9A4_300B, EDX)),
            (0x006E_5040, registers(i32::MIN as u32, EDX)),
        ] {
            assert_eq!(invoke(address, this, &[]), expected, "{address:08x}");
        }
        assert_eq!(bytes(this), before);
        let effect = invoke(0x006E_3150, this, &[]) as u32 as *mut f32;
        write_unaligned(effect, 0.3125);
        assert_eq!(sim.recycling_effect(), 0.3125);
    }
}

#[test]
fn landfill_capacity_reads_the_live_global_and_capacity_setter_writes_one_word() {
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    let mut capacity = 0x8765_4321u32;
    let _global = fake_data(0x00B4_9BA4, (&mut capacity as *mut u32).cast());
    // SAFETY: the test object and installed global.
    unsafe {
        assert_eq!(invoke(0x006E_4F10, this, &[]), registers(0x8765_4321, EDX));
        capacity = 42;
        assert_eq!(invoke(0x006E_4F10, this, &[]), registers(capacity, EDX));
        let before = bytes(this);
        assert_eq!(invoke(0x006E_5050, this, &[0xDEAD_BEEF]), registers(0xDEAD_BEEF, EDX));
        assert_eq!(PollutionSim::from_ptr(this).capacity() as u32, 0xDEAD_BEEF);
        unchanged_except(&before, &bytes(this), &[(0x230, 4)]);
    }
}

#[test]
fn saved_recycling_counter_uses_full_int64_truncation_and_keeps_float_environment() {
    let _fpu = GameFpu::new();
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    // SAFETY: the saved counter is in the test object.
    unsafe {
        let sim = PollutionSim::from_ptr(this);
        for (value, expected) in [
            (0.0, 0),
            (-0.0, 0),
            (123.875, 123),
            (-123.875, -123),
            (3_000_000_000.0, 3_000_000_000),
            (1.0e19, i64::MIN),
            (f64::INFINITY, i64::MIN),
            (f64::NEG_INFINITY, i64::MIN),
            (f64::NAN, i64::MIN),
            (9_223_372_036_854_774_784.0, 9_223_372_036_854_774_784),
        ] {
            sim.set_recycled_previous(value);
            let before = bytes(this);
            assert_eq!(invoke(0x006E_4F60, this, &[]) as i64, expected);
            assert_eq!(bytes(this), before);
            assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
        }
        let input = 37.25f32;
        let mut output = 0f32;
        core::arch::asm!("fld dword ptr [{input}]", input = in(reg) &input);
        invoke(0x006E_4F60, this, &[]);
        core::arch::asm!("fstp dword ptr [{output}]", output = in(reg) &mut output, out("st(0)") _);
        assert_eq!(output, input);
    }
}

#[test]
fn worst_tract_outputs_zero_extend_coordinates_and_allow_aliasing() {
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    // SAFETY: the field offsets and output pointers are within allocated storage.
    unsafe {
        for (address, offset) in [(0x006E_4FA0, 0x210), (0x006E_4FD0, 0x218), (0x006E_5000, 0x220)] {
            write_unaligned(this.add(offset).cast::<u16>(), 0xFEDC);
            write_unaligned(this.add(offset + 2).cast::<u16>(), 0x8765);
            write_unaligned(this.add(offset + 4).cast::<u32>(), 0xDEAD_BEEF);
            let before = bytes(this);
            let mut out = [0u32; 3];
            let args = [
                out.as_mut_ptr() as u32,
                out.as_mut_ptr().add(1) as u32,
                out.as_mut_ptr().add(2) as u32,
            ];
            assert_eq!(invoke(address, this, &args), registers(0xDEAD_BEEF, args[1]));
            assert_eq!(out, [0xFEDC, 0x8765, 0xDEAD_BEEF]);
            assert_eq!(bytes(this), before);
            // Writing x over the source coordinates replaces z before z is read.
            let mut z = 99u32;
            let mut value = 0u32;
            let args = [
                this.add(offset) as u32,
                &mut z as *mut u32 as u32,
                &mut value as *mut u32 as u32,
            ];
            invoke(address, this, &args);
            assert_eq!(z, 0);
            assert_eq!(value, 0xDEAD_BEEF);
        }
    }
}

#[test]
fn list_counts_walk_nodes_including_duplicate_occupants_and_keep_the_sentinel_in_edx() {
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    // SAFETY: the list storage stays live for each count call.
    unsafe {
        for (address, offset) in [
            (0x006E_50D0, 0x6C),
            (0x006E_50F0, 0x70),
            (0x006E_5110, 0x74),
            (0x006E_5130, 0x68),
        ] {
            for n in [0usize, 1, 2, 17] {
                let mut nodes = vec![[0u32; 3]; n + 1];
                let sentinel = nodes.as_mut_ptr().cast::<u8>();
                for i in 0..=n {
                    nodes[i][0] = nodes.as_mut_ptr().add((i + 1) % (n + 1)) as u32;
                    nodes[i][1] = nodes.as_mut_ptr().add((i + n) % (n + 1)) as u32;
                    nodes[i][2] = 0xABCD_1234;
                }
                write_unaligned(this.add(offset).cast(), sentinel);
                let before = bytes(this);
                let list_before = nodes.clone();
                assert_eq!(invoke(address, this, &[]), registers(n as u32, sentinel as u32));
                assert_eq!(nodes, list_before);
                assert_eq!(bytes(this), before);
            }
        }
    }
}

#[test]
fn supply_returns_delta_before_clamping_and_uses_wrapped_signed_comparisons() {
    // Core supply vectors, plus negative overflow where the wrapped sum is positive.
    let cases = [
        (1000, 256, 256, 1256),
        (2_147_483_632, 100, 15, i32::MAX),
        (50, -80, -50, 0),
        (50, -30, -30, 20),
        (7, 0, 0, 7),
        (-40, 0, 0, 0),
        (-40, 10, 10, 0),
        (-40, 100, 100, 60),
        (-40, -5, 40, 0),
        (i32::MIN, 0, 0, 0),
        (i32::MIN, -1, -1, i32::MAX),
        (i32::MIN, i32::MAX, i32::MAX, 0),
        (-1000, 256, 256, 0),
        (0x7FFF_FF00, 0x200, 0xFF, i32::MAX),
        (i32::MAX, 1, 0, i32::MAX),
        (i32::MIN + 5, -10, -10, i32::MAX - 4),
    ];
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    // SAFETY: the simulator's scalar fields.
    unsafe {
        let sim = PollutionSim::from_ptr(this);
        for (old, delta, accepted, stored) in cases {
            sim.set_supply(old);
            let before = bytes(this);
            let result = invoke(0x006E_32E0, this, &[delta as u32]);
            assert_eq!(
                result,
                registers(accepted as u32, old.wrapping_add(accepted) as u32),
                "{old} + {delta}"
            );
            assert_eq!(sim.supply(), stored, "{old} + {delta}");
            unchanged_except(&before, &bytes(this), &[(0x22C, 4)]);
        }
        for old in [i32::MIN, -40, -1, 0, 1, i32::MAX] {
            sim.set_supply(old);
            let before = bytes(this);
            assert_eq!(invoke(0x006E_3340, this, &[]), registers(old.max(0) as u32, EDX));
            assert_eq!(sim.supply(), old.max(0));
            unchanged_except(&before, &bytes(this), &[(0x22C, 4)]);
        }
    }
}

#[test]
fn monthly_landfill_addition_wraps_at_signed_int64_bounds_with_low_word_adjustments() {
    let cases = [
        (0, 100, 100),
        (100, -200, -100),
        (123, 0, 123),
        (0x1234_FFFF_FFFF, 1, 0x1235_0000_0000),
        (i64::MAX, 1, i64::MAX),
        (i64::MAX - 5, 10, i64::MAX),
        (i64::MIN, -1, i64::MIN),
        (i64::MIN + 5, -10, i64::MIN),
        (0, i32::MIN, i64::from(i32::MIN)),
        (0, i32::MAX, i64::from(i32::MAX)),
    ];
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    // SAFETY: the simulator's counter field.
    unsafe {
        let sim = PollutionSim::from_ptr(this);
        for (old, delta, expected) in cases {
            sim.set_landfilled(old);
            let before = bytes(this);
            let result = invoke(0x006E_3250, this, &[delta as u32]);
            assert_eq!(sim.landfilled(), expected, "{old} + {delta}");
            let adjusted = expected.wrapping_sub(old) as i32;
            assert_eq!(result, registers((expected >> 32) as u32, (adjusted >> 31) as u32));
            unchanged_except(&before, &bytes(this), &[(0x170, 8)]);
        }
    }
}

#[derive(Debug, PartialEq)]
enum Call {
    Shift(u32, u32),
    Get(u32, u32, u32, u32),
    Set(u32, u32, u32, u32, u8, u32),
    Zone(u32, u32, u32),
    Active(u32, u32, u32),
    AddRef(u32, i32),
    Shutdown(u32),
    Destroy(u32),
    Free(u32),
    Query(u32, u32, u32),
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
fn log(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}
fn take_calls() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

#[repr(C)]
struct Grid {
    vtable: *const usize,
    sim: *mut u8,
    offset: usize,
    dirty: usize,
    shift: u64,
    value: u64,
    set_result: u64,
    after_shift: *mut u8,
    after_get: *mut u8,
}
extern "fastcall" fn shift(this: *mut Grid, edx: u32) -> u64 {
    log(Call::Shift(this as u32, edx));
    // SAFETY: test grid and simulator storage.
    unsafe {
        let grid = &*this;
        if !grid.after_shift.is_null() {
            write_unaligned(grid.sim.add(grid.offset).cast(), grid.after_shift);
        }
        grid.shift
    }
}
extern "fastcall" fn get(this: *mut Grid, edx: u32, x: u32, z: u32) -> u64 {
    log(Call::Get(this as u32, x, z, edx));
    // SAFETY: test grid and simulator storage.
    unsafe {
        let grid = &*this;
        if !grid.after_get.is_null() {
            write_unaligned(grid.sim.add(grid.offset).cast(), grid.after_get);
        }
        grid.value
    }
}
extern "fastcall" fn set(this: *mut Grid, _edx: u32, x: u32, z: u32, value: u32) -> u64 {
    // SAFETY: test grid and simulator storage.
    unsafe {
        let grid = &*this;
        log(Call::Set(
            this as u32,
            x,
            z,
            value,
            *grid.sim.add(grid.dirty),
            PollutionSim::from_ptr(grid.sim).active_landfill_cells(),
        ));
        grid.set_result
    }
}
extern "fastcall" fn zone(this: *mut Grid, _edx: u32, x: u32, z: u32) -> u64 {
    log(Call::Zone(this as u32, x, z));
    // SAFETY: zone fake uses the Grid storage.
    unsafe { (*this).value }
}
extern "fastcall" fn active(this: *mut u8, _edx: u32, x: u32, z: u32) -> u64 {
    log(Call::Active(this as u32, x, z));
    // SAFETY: result word in the test simulator's spare area.
    unsafe { read_unaligned(this.add(0x240).cast()) }
}

struct World {
    object: Object,
    grids: Vec<Grid>,
    _grid_vtable: Box<[usize; 32]>,
    _sim_vtable: Box<[usize; 80]>,
}
impl World {
    fn new(offset: usize) -> Self {
        let mut object = Object::new(SIZE);
        let this = object.ptr();
        let mut grid_vtable = Box::new([0usize; 32]);
        grid_vtable[0x1C / 4] = get as *const () as usize;
        grid_vtable[0x2C / 4] = shift as *const () as usize;
        grid_vtable[0x68 / 4] = set as *const () as usize;
        grid_vtable[0x30 / 4] = zone as *const () as usize;
        let mut sim_vtable = Box::new([0usize; 80]);
        sim_vtable[0xC4 / 4] = active as *const () as usize;
        let mut grids: Vec<_> = (0..3)
            .map(|_| Grid {
                vtable: grid_vtable.as_ptr(),
                sim: this,
                offset,
                dirty: 0x209 + (offset - 0x50) / 4,
                shift: registers(1, 0x1234_5678),
                value: registers(0xABCD_8064, 0xFEED_BEEF),
                set_result: registers(0x1234_5678, 0x8765_4321),
                after_shift: core::ptr::null_mut(),
                after_get: core::ptr::null_mut(),
            })
            .collect();
        // SAFETY: simulator storage and stable vector elements.
        unsafe {
            write_unaligned(this.add(offset).cast(), (&mut grids[0] as *mut Grid).cast::<u8>());
            write_unaligned(this.cast(), sim_vtable.as_ptr());
            write_unaligned(this.add(0x24).cast(), (&mut grids[2] as *mut Grid).cast::<u8>());
        }
        take_calls();
        Self {
            object,
            grids,
            _grid_vtable: grid_vtable,
            _sim_vtable: sim_vtable,
        }
    }
    fn this(&mut self) -> *mut u8 {
        self.object.ptr()
    }
    fn grid(&mut self, i: usize) -> *mut u8 {
        (&mut self.grids[i] as *mut Grid).cast()
    }
}

#[test]
fn cell_getters_sign_extend_then_garbage_clears_only_bit_fifteen() {
    for (address, offset, mask) in [
        (0x006E_2EF0, 0x50, u32::MAX),
        (0x006E_2F10, 0x54, u32::MAX),
        (0x006E_2F30, 0x58, 0xFFFF_7FFF),
    ] {
        let mut world = World::new(offset);
        let this = world.this();
        let grid = world.grid(0) as u32;
        for raw in [0, 1, 0x7FFF, 0x8000, 0x8064, 0xFFFF, 0x1234_8000] {
            world.grids[0].value = registers(raw, 0xCAFE_1234);
            let mut output = 0i32;
            // SAFETY: the test world and scalar output.
            unsafe {
                let before = bytes(this);
                let expected = (raw as u16 as i16 as i32 as u32) & mask;
                assert_eq!(
                    invoke(address, this, &[u32::MAX, 30, &mut output as *mut i32 as u32]),
                    registers(expected, 0xCAFE_1234)
                );
                assert_eq!(output as u32, expected);
                assert_eq!(bytes(this), before);
            }
            assert_eq!(take_calls(), [Call::Get(grid, u32::MAX, 30, u32::MAX)]);
        }
    }
}

#[test]
fn air_and_water_setters_shift_logically_reload_grid_and_set_only_the_dirty_byte() {
    for (address, offset, dirty) in [(0x006E_3070, 0x50, 0x209), (0x006E_30B0, 0x54, 0x20A)] {
        let mut world = World::new(offset);
        let this = world.this();
        let first = world.grid(0);
        let second = world.grid(1);
        world.grids[0].after_shift = second;
        world.grids[0].shift = registers(33, 0x7777);
        // SAFETY: the test world.
        unsafe {
            let before = bytes(this);
            assert_eq!(
                invoke(address, this, &[0x8000_0002, u32::MAX, 0xDEAD_1234]),
                registers(0x1234_5678, 0x8765_4321)
            );
            assert_eq!(*this.add(dirty), 1);
            unchanged_except(&before, &bytes(this), &[(offset, 4), (dirty, 1)]);
        }
        assert_eq!(
            take_calls(),
            [
                Call::Shift(first as u32, EDX),
                Call::Set(second as u32, 0x4000_0001, 0x7FFF_FFFF, 0xDEAD_1234, 0, 0)
            ]
        );
    }
}

#[test]
fn garbage_setter_reloads_after_both_calls_and_keeps_signed_high_bits() {
    let mut world = World::new(0x58);
    let this = world.this();
    let (first, second, third) = (world.grid(0), world.grid(1), world.grid(2));
    world.grids[0].after_shift = second;
    world.grids[1].after_get = third;
    world.grids[0].shift = registers(34, 0);
    world.grids[1].value = registers(0x1234_8064, 0x8877_6655);
    // SAFETY: the test world.
    unsafe {
        let before = bytes(this);
        assert_eq!(
            invoke(0x006E_30F0, this, &[u32::MAX, 12, 100]),
            world.grids[2].set_result
        );
        assert_eq!(*this.add(0x20B), 1);
        unchanged_except(&before, &bytes(this), &[(0x58, 4), (0x20B, 1)]);
    }
    assert_eq!(
        take_calls(),
        [
            Call::Shift(first as u32, EDX),
            Call::Get(second as u32, u32::MAX, 12, world._grid_vtable.as_ptr() as u32),
            Call::Set(third as u32, 0x3FFF_FFFF, 3, 0xFFFF_8064, 0, 0),
        ]
    );
    // A supplied flag or high word is passed through even when the old cell was positive.
    world.grids[0].after_shift = core::ptr::null_mut();
    world.grids[2].after_get = core::ptr::null_mut();
    world.grids[2].value = registers(7, 0);
    // SAFETY: the test world.
    unsafe {
        invoke(0x006E_30F0, this, &[0, 0, 0xABCD_8000]);
    }
    assert!(matches!(
        take_calls().last(),
        Some(Call::Set(_, 0, 0, 0xABCD_8000, 1, 0))
    ));
}

#[test]
fn active_landfill_query_preserves_shifted_high_result_bits() {
    let mut world = World::new(0x58);
    let this = world.this();
    for raw in [0, 0x8000, 0x8064, 0xFFFF_7FFF, 0xFFFF_FFFF] {
        world.grids[0].value = registers(raw, 0xAABB_CCDD);
        // SAFETY: the test world.
        unsafe {
            let expected = ((raw >> 15) & !0xFF) | ((raw >> 15) & 1);
            assert_eq!(invoke(0x006E_31A0, this, &[30, 31]), registers(expected, 0xAABB_CCDD));
        }
        assert_eq!(take_calls(), [Call::Get(world.grid(0) as u32, 30, 31, 30)]);
    }
}

#[test]
fn landfill_flag_changes_require_the_zone_and_actual_transition() {
    for (zone_value, requested, old_active, count, expected_value, expected_count) in [
        (13, 1, 0, 0, None, 0),
        (14, 1, 1, 4, None, 4),
        (13, 0, 0, 4, None, 4),
        (14, 1, 0, u32::MAX, Some(0xABCD_8064), 0),
        (13, 0, 1, 4, Some(100), 3),
        (13, 0, 1, 0, Some(100), 0),
        (14, 0x100, 1, 2, Some(100), 1),
    ] {
        let mut world = World::new(0x58);
        let this = world.this();
        world.grids[2].value = registers(zone_value, 0x1111_2222);
        // SAFETY: test simulator and fake active-result field.
        unsafe {
            let sim = PollutionSim::from_ptr(this);
            sim.set_active_landfill_cells(count);
            write_unaligned(this.add(0x240).cast(), registers(old_active, 0x3333_4444));
            let before = bytes(this);
            let result = invoke(0x006E_31C0, this, &[30, 31, requested]);
            let mut expected = vec![Call::Zone(world.grid(2) as u32, 30, 31)];
            if requested as u8 != 0 && zone_value != 14 {
                assert_eq!(result, registers(zone_value, 0x1111_2222));
            } else {
                expected.push(Call::Active(this as u32, 30, 31));
                if let Some(value) = expected_value {
                    expected.push(Call::Get(world.grid(0) as u32, 30, 31, 0x3333_4444));
                    expected.push(Call::Set(world.grid(0) as u32, 30, 31, value, 0, expected_count));
                    assert_eq!(result, world.grids[0].set_result);
                } else {
                    assert_eq!(result, registers(old_active, 0x3333_4444));
                }
            }
            assert_eq!(take_calls(), expected);
            assert_eq!(sim.active_landfill_cells(), expected_count);
            assert_eq!(sim.dirty_garbage(), u8::from(expected_value.is_some()));
            unchanged_except(&before, &bytes(this), &[(0x1BC, 4), (0x20B, 1)]);
        }
    }
}

#[test]
fn pollution_grid_uses_unchecked_wrapping_index_and_special_radiation_source() {
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    // SAFETY: selected unchecked indexes all address allocated fields.
    unsafe {
        for offset in (0x40..0x70).step_by(4) {
            write_unaligned(this.add(offset).cast::<u32>(), 0x1234_0000 + offset as u32);
        }
        let before = bytes(this);
        for kind in [0u32, 1, 2, 3, 4, 7, u32::MAX, 0x4000_0000] {
            let offset = if kind == 3 {
                0x4C
            } else {
                kind.wrapping_mul(4).wrapping_add(0x50)
            };
            assert_eq!(invoke(0x006E_3360, this, &[kind]), registers(0x1234_0000 + offset, EDX));
        }
        assert_eq!(bytes(this), before);
    }
}

extern "fastcall" fn add_ref(this: *mut u8, _edx: u32) -> u64 {
    // SAFETY: test simulator's refcount field.
    unsafe {
        let sim = PollutionSim::from_ptr(this);
        log(Call::AddRef(this as u32, sim.references()));
        sim.set_references(sim.references() + 1);
    }
    registers(0xAABB_CCDD, 0x1234_5678)
}
extern "fastcall" fn shutdown(this: *mut u8, _edx: u32) -> u64 {
    log(Call::Shutdown(this as u32));
    registers(0xAABB_CCDD, 0x7654_3210)
}
extern "fastcall" fn destroy(this: *mut u8, edx: u32) -> u64 {
    assert_eq!(edx, 0x7654_3210);
    log(Call::Destroy(this as u32));
    registers(0xBBBB_CCCC, 0x8888_9999)
}
extern "C" fn free(this: *mut u8) -> u64 {
    log(Call::Free(this as u32));
    registers(0xCCCC_DDDD, 0x9999_AAAA)
}
extern "fastcall" fn query(this: *mut u8, edx: u32, iid: u32, output: *mut *mut u8) -> u64 {
    log(Call::Query(this as u32, edx, iid));
    // SAFETY: scalar output passed by the test.
    unsafe {
        *output = this;
    }
    registers(0xAA55_CC01, 0x1122_3344)
}

#[test]
fn query_interface_recognizes_five_ids_retain_primary_and_keeps_failure_upper_bytes() {
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    let mut vt = [0usize; 8];
    vt[1] = add_ref as *const () as usize;
    // SAFETY: test object and stable table.
    unsafe {
        write_unaligned(this.cast(), vt.as_ptr());
        for (iid, offset) in [
            (1, 0),
            (0x8771_81EC, 0),
            (0x4522_94AA, 4),
            (0x89C4_98BE, 8),
            (0xE4FD_A3D4, 12),
        ] {
            let sim = PollutionSim::from_ptr(this);
            sim.set_references(7);
            let mut output = core::ptr::null_mut::<u8>();
            let before = bytes(this);
            assert_eq!(
                invoke(0x006E_2CD0, this, &[iid, &mut output as *mut *mut u8 as u32]),
                registers(0xAABB_CC01, 0x1234_5678)
            );
            assert_eq!(output, this.add(offset));
            assert_eq!(sim.references(), 8);
            assert_eq!(take_calls(), [Call::AddRef(this as u32, 7)]);
            unchanged_except(&before, &bytes(this), &[(0x14, 4)]);
        }
        for iid in [0u32, 2, 0x4522_94A9, 0x4522_94AB, 0x8771_81EB, 0x8771_81ED, u32::MAX] {
            let mut output = 0x1234_5678usize as *mut u8;
            let expected = if iid < 0x8771_81EC {
                iid.wrapping_sub(0x4522_94AA)
            } else {
                iid
            } & !0xFF;
            assert_eq!(
                invoke(0x006E_2CD0, this, &[iid, &mut output as *mut *mut u8 as u32]),
                registers(expected, EDX)
            );
            assert_eq!(output as u32, 0x1234_5678);
            assert_eq!(take_calls(), []);
        }
    }
}

#[test]
fn interface_thunks_call_the_original_query_address_with_adjusted_this() {
    let _query = fake(0x006E_2CD0, query as *const () as usize);
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    for (address, offset) in [(0x006E_50B0, 8), (0x006E_50C0, 12)] {
        let mut output = core::ptr::null_mut::<u8>();
        // SAFETY: test interfaces at their proper offsets.
        unsafe {
            assert_eq!(
                invoke(
                    address,
                    this.add(offset),
                    &[0x89C4_98BE, &mut output as *mut *mut u8 as u32]
                ),
                registers(0xAA55_CC01, 0x1122_3344)
            );
        }
        assert_eq!(output, this);
        assert_eq!(take_calls(), [Call::Query(this as u32, EDX, 0x89C4_98BE)]);
    }
}

#[test]
fn release_decrements_only_above_one_and_calls_shutdown_destroy_free_in_order() {
    let _destroy = fake(0x006E_4E20, destroy as *const () as usize);
    let _free = fake(0x005E_5620, free as *const () as usize);
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    let mut vt = [0usize; 8];
    vt[4] = shutdown as *const () as usize;
    // SAFETY: test simulator and original lifecycle fakes.
    unsafe {
        write_unaligned(this.cast(), vt.as_ptr());
        let sim = PollutionSim::from_ptr(this);
        for count in [2, 3, i32::MAX] {
            sim.set_references(count);
            let before = bytes(this);
            assert_eq!(invoke(0x006E_5F20, this, &[]), registers((count - 1) as u32, EDX));
            assert_eq!(sim.references(), count - 1);
            assert_eq!(take_calls(), []);
            unchanged_except(&before, &bytes(this), &[(0x14, 4)]);
        }
        for count in [1, 0, -1, i32::MIN] {
            sim.set_references(count);
            let before = bytes(this);
            assert_eq!(invoke(0x006E_5F20, this, &[]), registers(0, 0x9999_AAAA));
            assert_eq!(
                take_calls(),
                [
                    Call::Shutdown(this as u32),
                    Call::Destroy(this as u32),
                    Call::Free(this as u32)
                ]
            );
            assert_eq!(bytes(this), before);
        }
    }
}

#[test]
fn declared_fields_have_the_expected_widths_and_do_not_overlap() {
    assert!(PollutionSim::FIELDS.contains(&("garbage", 0x58, 4)));
    assert!(PollutionSim::FIELDS.contains(&("average_garbage", 0x128, 2)));
    assert!(PollutionSim::FIELDS.contains(&("recycled_previous", 0x190, 8)));
    let mut fields: Vec<_> = PollutionSim::FIELDS
        .iter()
        .map(|&(_, offset, len)| (offset, offset + len))
        .collect();
    fields.sort();
    assert!(fields.windows(2).all(|pair| pair[0].1 <= pair[1].0));
}

#[test]
fn source_value_clamps_signed_words_and_retains_original_scratch_address_bits() {
    let mut world = World::new(0x50);
    let this = world.this();
    // SAFETY: the source and value-grid fields refer to the same fake grid in this test.
    unsafe {
        PollutionSim::from_ptr(this).set_air_source(world.grid(0));
        for (raw, stack, expected) in [
            (0xAABB_1234, 0x1234_5678, 0x1234_1234),
            (0xAABB_8000, 0x1234_5678, 0x1234_0000),
            (0xFFFF_7FFF, 0x1234_FFFC, 0x1235_7FFF),
            (0x1234_FFFF, 0x1234_0000, 0x1233_0000),
        ] {
            world.grids[0].value = registers(raw, 0x7788_99AA);
            assert_eq!(
                grids::pollution_value_body(this, stack, 0, 9, 10),
                registers(expected, 0x7788_99AA)
            );
            assert_eq!(
                take_calls(),
                [Call::Get(
                    world.grid(0) as u32,
                    9,
                    10,
                    world._grid_vtable.as_ptr() as u32
                )]
            );
            let before = bytes(this);
            let result = invoke(0x006E_37E0, this, &[0, 9, 10]);
            assert_eq!(result as u16, expected as u16);
            assert_eq!((result >> 32) as u32, 0x7788_99AA);
            assert_eq!(bytes(this), before);
            take_calls();
        }
    }
}

thread_local! { static CAPACITY: RefCell<(u32, u32)> = const { RefCell::new((0, 0)) }; }
extern "thiscall" fn current_recycling(this: *mut u8, id: u32, occupant: *mut u8, out: *mut u32) -> u32 {
    assert_eq!(id, 0x88ED_1059);
    assert_eq!(occupant as u32, 0x1234_5678);
    // The original query destination initially holds the occupant argument.
    assert_eq!(unsafe { *out }, occupant as u32);
    log(Call::Query(this as u32, id, occupant as u32));
    CAPACITY.with(|c| {
        let (result, value) = *c.borrow();
        // SAFETY: native port's query destination.
        unsafe {
            *out = value;
        }
        result
    })
}

#[test]
fn recycling_capacity_uses_unsigned_x87_scaling_and_preserves_query_result_bytes() {
    let _fpu = GameFpu::new();
    let mut object = Object::new(SIZE);
    let this = object.ptr();
    let mut vt = [0usize; 64];
    vt[0xD4 / 4] = current_recycling as *const () as usize;
    let mut default = 0x7F80_1234u32;
    let _global = fake_data(0x00B4_9BC4, (&mut default as *mut u32).cast());
    // SAFETY: live fake simulator table and output scalar.
    unsafe {
        write_unaligned(this.cast(), vt.as_ptr());
        for (capacity, expected) in [
            (0, 0.0f32),
            (500, 5.0),
            (12345, 123.45),
            (0x8000_0001, 21_474_836.0),
            (u32::MAX, 42_949_672.0),
        ] {
            CAPACITY.with(|c| *c.borrow_mut() = (0xAABB_CC01, capacity));
            let mut output = 99f32;
            let before = bytes(this);
            assert_eq!(
                invoke(0x006E_3720, this, &[0x1234_5678, &mut output as *mut f32 as u32]),
                registers(0xAABB_CC01, &mut output as *mut f32 as u32)
            );
            assert_eq!(output.to_bits(), expected.to_bits(), "capacity {capacity}");
            assert_eq!(bytes(this), before);
            assert_eq!(take_calls(), [Call::Query(this as u32, 0x88ED_1059, 0x1234_5678)]);
        }
        CAPACITY.with(|c| *c.borrow_mut() = (0xAABB_CC00, u32::MAX));
        let mut output = 99f32;
        assert_eq!(
            invoke(0x006E_3720, this, &[0x1234_5678, &mut output as *mut f32 as u32]),
            registers(0xAABB_CC00, default)
        );
        assert_eq!(output.to_bits(), default);
        take_calls();
    }
}

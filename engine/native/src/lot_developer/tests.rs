// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native conformance cases, including the habitation and terrain cases from the core lot developer.

use super::{foundation::*, lots::*, operations::*, small::*, terrain::*, *};
use crate::testing::{call_thiscall_with_eax, fake, fake_data, GameFpu, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

const RETURN: u64 = 0x9876_5432_A1B2_C3D4;


enum Arg {
    Word(u32),
    Out(Vec<u32>),
    In(Vec<u32>),
}

struct Step {
    object: u32,
    method: &'static str,
    args: Vec<Arg>,
    returned: u64,
    edits: Vec<(*mut u32, u32)>,
}

thread_local! {
    static STEPS: RefCell<VecDeque<Step>> = const { RefCell::new(VecDeque::new()) };
    static FLOATS: RefCell<VecDeque<F80>> = const { RefCell::new(VecDeque::new()) };
}

fn w(value: u32) -> Arg {
    Arg::Word(value)
}
fn p(value: *mut u8) -> Arg {
    w(value as u32)
}
fn out(values: &[u32]) -> Arg {
    Arg::Out(values.to_vec())
}

fn expect(object: *mut u8, method: &'static str, args: Vec<Arg>, returned: u64) {
    STEPS.with(|s| {
        s.borrow_mut().push_back(Step {
            object: object as u32,
            method,
            args,
            returned,
            edits: Vec::new(),
        })
    });
}

fn edit_last(address: *mut u32, value: u32) {
    STEPS.with(|s| s.borrow_mut().back_mut().unwrap().edits.push((address, value)));
}

fn finished() {
    STEPS.with(|s| assert!(s.borrow().is_empty(), "uncalled methods remain"));
}

fn run(object: *mut u8, method: &'static str, args: &[u32]) -> u64 {
    let step = STEPS.with(|s| s.borrow_mut().pop_front()).expect("unexpected call");
    assert_eq!((object as u32, method), (step.object, step.method));
    assert_eq!(args.len(), step.args.len(), "{method}");
    for (actual, expected) in args.iter().zip(step.args) {
        match expected {
            Arg::Word(value) => assert_eq!(*actual, value, "{method}"),
            Arg::Out(values) => unsafe {
                assert_ne!(*actual, 0);
                for (index, value) in values.iter().enumerate() {
                    (*actual as *mut u32).add(index).write_unaligned(*value);
                }
            },
            Arg::In(values) => unsafe {
                assert_eq!(std::slice::from_raw_parts(*actual as *const u32, values.len()), values);
            },
        }
    }
    for (address, value) in step.edits {
        unsafe { address.write_unaligned(value) };
    }
    step.returned
}

macro_rules! slot {
    ($name:ident $(, $arg:ident)*) => {
        #[allow(clippy::too_many_arguments)]
        unsafe extern "thiscall" fn $name(this: *mut u8 $(, $arg: u32)*) -> u64 {
            run(this, stringify!($name), &[$($arg),*])
        }
    };
}

slot!(add_ref);
slot!(release);
slot!(destroy, flags);
slot!(shutdown);
slot!(get_zone);
slot!(get_building, lot);
slot!(set_state, state, wealth, reason);
slot!(get_occupant);
slot!(remove_building_props, occupant);
slot!(remove_building_foundation, occupant);
slot!(remove_occupant, occupant, immediately, flags);
slot!(remove_props, lot);
slot!(get_config);
slot!(get_primary);
slot!(get_position, lot, cfg, object, position, facing);
slot!(get_building_type, growth);
slot!(get_profile, kind);
slot!(get_wealth, developer);
slot!(get_holder);
slot!(get_property, id);
slot!(get_variant);
slot!(get_data);
slot!(set_condition, condition);
slot!(get_population, developer, total);
slot!(set_population, developer, population);
slot!(get_world_position, position);
slot!(get_type);
slot!(foundation_position, kind, x, z);
slot!(get_location, x, z);
slot!(foundation_cell, kind, x, z);
slot!(get_lot, x, z, create);
slot!(get_flags);
slot!(set_flags, flags);
slot!(remove_foundation_cell, x, z);
slot!(get_foundation, lot);
slot!(get_foundation_type);
slot!(add_lot_type, lot, kind, altitude, level, walls);
slot!(add_lot_config, lot, cfg, altitude, level, walls);
slot!(remove_foundation_position, kind, x, z);
slot!(get_texture, lot, output, create);
slot!(update_texture, refresh, foundation, unused);
slot!(set_uint, value);
slot!(resource_exists, key);
slot!(lot_wealth);
slot!(surrounding_lots, lot, list, flags);
slot!(refresh_foundation, lot);
slot!(set_business_unit, unit);
slot!(set_growth_type, kind);
slot!(set_configuration, config);
slot!(set_empty_texture, lot);
slot!(area_lots, list, x0, z0, x1, z1, flags);
slot!(neighbor_lots, rect, list, flags);
slot!(business_unit);
slot!(unit_flags);
slot!(test_unit_flags, mask);
slot!(habitation_state);
slot!(building_occupant);

unsafe extern "C" fn operation_allocate(bytes: u32) -> u32 {
    run(core::ptr::null_mut(), "operation_allocate", &[bytes]) as u32
}
unsafe extern "C" fn operation_free(pointer: u32) -> u64 {
    run(core::ptr::null_mut(), "operation_free", &[pointer])
}
unsafe extern "C" fn clear_contents(lot: u32, flags: u32) -> u64 {
    run(core::ptr::null_mut(), "clear_contents", &[lot, flags])
}
unsafe extern "C" fn clear_traffic(lot: u32) -> u64 {
    run(core::ptr::null_mut(), "clear_traffic", &[lot])
}
unsafe extern "thiscall" fn surrounding_direct(this: *mut u8, lot: u32) -> u64 {
    run(this, "surrounding_direct", &[lot])
}

unsafe extern "thiscall" fn clear_list(list: *mut *mut u8) -> u64 {
    run(core::ptr::null_mut(), "clear_list", &[unsafe { list.read_unaligned() } as u32])
}

struct Mock {
    memory: Object,
    _vtable: Vec<usize>,
}

impl Mock {
    fn new(slots: &[(usize, usize)]) -> Self {
        let mut table = vec![0; slots.iter().map(|s| s.0).max().unwrap_or(0).max(0x124) / 4 + 1];
        for &(offset, function) in slots {
            table[offset / 4] = function;
        }
        let mut memory = Object::new(0x120);
        unsafe {
            memory.ptr().cast::<usize>().write(table.as_ptr() as usize);
        }
        Self { memory, _vtable: table }
    }
    fn ptr(&mut self) -> *mut u8 {
        self.memory.ptr()
    }
}

macro_rules! mock {
    ($($offset:expr => $function:ident),* $(,)?) => {
        Mock::new(&[$(($offset, $function as *const () as usize)),*])
    };
}

mod operations;
mod anchor;
mod objects;
mod updates;
mod textures;
mod network;
mod altitudes;
mod planting;
mod daily;
mod position;
mod connections;
mod lot_update;
mod lighting_transition;

#[test]
fn record_initializers_retain_every_padding_byte() {
    unsafe {
        let mut record = [0xA5u8; 72];
        let address = record.as_mut_ptr().add(4);
        assert_eq!(fc4p_004591e0(address), address);
        let zero = [0..11, 12..25, 28..44, 48..64];
        for (offset, &value) in record.iter().enumerate() {
            let cleared = offset >= 4 && zero.iter().any(|range| range.contains(&(offset - 4)));
            assert_eq!(value, if cleared { 0 } else { 0xA5 }, "byte {offset}");
        }
        record.fill(0xA5);
        let mut ecx = address as u32;
        let eax: u32;
        core::arch::asm!("call {function}", function = sym fc4p_005ca770,
            inlateout("ecx") ecx, lateout("eax") eax, clobber_abi("C"));
        assert_eq!(eax, address as u32);
        assert_eq!(ecx, 255);
        assert_eq!(&record[4..12], &[0; 8]);
        assert_eq!(&record[12..18], &[255; 6]);
        assert!(record[..4].iter().chain(&record[18..]).all(|v| *v == 0xA5));
    }
}

unsafe extern "C" fn destroy_elements(begin: u32, end: u32, allocator: u32) -> u32 {
    assert_ne!(allocator, 0);
    run(core::ptr::null_mut(), "destroy_elements", &[begin, end]) as u32
}
unsafe extern "C" fn free_elements(storage: u32) -> u32 {
    run(core::ptr::null_mut(), "free_elements", &[storage]) as u32
}

#[test]
fn vector_destruction_reads_storage_again_after_destroying_elements() {
    let _destroy = fake(0x0045_9170, destroy_elements as *const () as usize);
    let _free = fake(0x0090_CF63, free_elements as *const () as usize);
    for (begin, replacement) in [(0, 0), (0x1000, 0), (0x1000, 0x3000), (0, 0x3000)] {
        let mut object = [0xCCCC_CCCCu32; 8];
        object[3] = begin;
        object[4] = 0x2000;
        expect(
            core::ptr::null_mut(),
            "destroy_elements",
            vec![w(begin), w(0x2000)],
            0x1234_ABCD,
        );
        edit_last(unsafe { object.as_mut_ptr().add(3) }, replacement);
        if replacement != 0 {
            expect(
                core::ptr::null_mut(),
                "free_elements",
                vec![w(replacement)],
                0x8765_4321,
            );
        }
        let eax = unsafe { fc4p_00459220(object.as_mut_ptr().cast()) };
        assert_eq!(eax, if replacement == 0 { 0x1234_ABCD } else { 0x8765_4321 });
        assert_eq!(&object[..3], &[0xCCCC_CCCC; 3]);
        assert_eq!(object[3], replacement);
        assert_eq!(object[4], 0x2000);
        assert_eq!(&object[5..], &[0xCCCC_CCCC; 3]);
        finished();
    }
}

#[test]
fn identity_and_force_empty_keep_unused_register_bytes() {
    let mut memory = Object::new(0x108);
    let this = memory.ptr();
    for value in [0, 1, 0x80, 0xFF] {
        unsafe {
            LotDeveloper::from_ptr(this).set_force_empty(value);
            let (eax, edx) = call_thiscall_with_eax(fc4p_006aaf40 as *const () as usize, this, &[], 0xAABB_CCDD);
            assert_eq!(eax, 0xAABB_CC00 | u32::from(value));
            assert_eq!(edx, 0);
            let (eax, _) = call_thiscall_with_eax(fc4p_006c3b00 as *const () as usize, this, &[], 0);
            assert_eq!(eax, 0xA990_BFE0);
        }
    }
}

#[test]
fn interface_queries_keep_failure_outputs_and_addref_successes() {
    let mut object = Mock::new(&[(4, add_ref as *const () as usize)]);
    let this = object.ptr();
    for (iid, offset) in [(1, 0), (0x4818_E5D2, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
        let mut output = core::ptr::null_mut();
        expect(this, "add_ref", vec![], RETURN);
        assert_eq!(
            unsafe { fc4p_006c3b80(this, 0xDDDD_DDDD, iid, &mut output) },
            RETURN & !255 | 1
        );
        assert_eq!(output, this.wrapping_add(offset));
        finished();
    }
    for iid in [0u32, 2, 0x4522_94A9, 0x4818_E5D1, 0x4818_E5D3, u32::MAX] {
        let mut output = 0x1234usize as *mut u8;
        let eax = if iid < 0x4818_E5D2 {
            iid.wrapping_sub(0x4522_94AA)
        } else {
            iid
        } & !255;
        assert_eq!(
            unsafe { fc4p_006c3b80(this, 0xDDDD_DDDD, iid, &mut output) },
            0xDDDD_DDDD_0000_0000 | u64::from(eax)
        );
        assert_eq!(output as usize, 0x1234);
        finished();
    }
    let mut output = core::ptr::null_mut::<u8>();
    expect(this, "add_ref", vec![], RETURN);
    let (eax, edx) = unsafe {
        call_thiscall_with_eax(
            fc4p_006c3b80 as *const () as usize,
            this,
            &[1, &mut output as *mut *mut u8 as u32],
            0,
        )
    };
    assert_eq!((eax, edx), (0xA1B2_C301, 0x9876_5432));
    finished();
}

#[test]
fn zot_patterns_cover_zone_boundaries_negative_coordinates_and_wrapping_sums() {
    let mut lot_object = Mock::new(&[(0x7C, get_zone as *const () as usize)]);
    let lot = lot_object.ptr();
    let coords: [i32; 9] = [i32::MIN, -5, -4, -1, 0, 1, 4, 5, i32::MAX];
    for zone in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, u32::MAX] {
        for x in coords {
            for z in coords {
                expect(lot, "get_zone", vec![], u64::from(zone));
                let mut call = RawCall::new(Conv::Thiscall, fc4p_006c3b10 as *const () as usize, lot as u32, &[]);
                call.eax = z as u32;
                call.edx = x as u32;
                unsafe {
                    call_raw(&mut call);
                }
                let answer = match zone {
                    1..=6 => (x.wrapping_add(z) as u32 & 3) == 0,
                    7..=9 => (x as u32 & 3) == 0 && (z as u32 & 3) == 0,
                    _ => false,
                };
                assert_eq!(call.eax as u8, u8::from(answer), "zone={zone}, x={x}, z={z}");
                finished();
            }
        }
    }
}

#[test]
fn release_handles_signed_counts_and_uses_the_updated_destructor_table() {
    let mut object = Mock::new(&[
        (0x10, shutdown as *const () as usize),
        (0x104, destroy as *const () as usize),
    ]);
    let this = object.ptr();
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    for count in [i32::MIN, -1, 0, 1, 2, 8, i32::MAX] {
        developer.set_references(count);
        if count <= 1 {
            expect(this, "shutdown", vec![], RETURN);
            expect(this, "destroy", vec![w(1)], RETURN);
        }
        assert_eq!(
            unsafe { fc4p_006c49d0(this) },
            if count > 1 { count as u32 - 1 } else { 0 }
        );
        assert_eq!(developer.references(), if count > 1 { count - 1 } else { count });
        finished();
    }
    let mut replacement = mock!(0x104 => destroy);
    developer.set_references(1);
    expect(this, "shutdown", vec![], RETURN);
    edit_last(this.cast(), unsafe { table(replacement.ptr()) });
    expect(this, "destroy", vec![w(1)], RETURN);
    assert_eq!(unsafe { fc4p_006c49d0(this) }, 0);
    finished();
}

#[test]
fn clear_buildings_repeats_lookup_and_preserves_removal_order() {
    let mut dev_object = mock!(0x98 => get_building, 0xF0 => remove_building_foundation, 0x2C => remove_props);
    let mut lot_object = mock!(0x88 => set_state);
    let mut building_a = mock!(4 => add_ref, 0x0C => get_occupant, 8 => release);
    let mut building_b = mock!(4 => add_ref, 0x0C => get_occupant, 8 => release);
    let mut prop_object = mock!(0x98 => remove_building_props);
    let mut manager_object = mock!(0x54 => remove_occupant);
    let (this, lot, a, b, props, manager) = (
        dev_object.ptr(),
        lot_object.ptr(),
        building_a.ptr(),
        building_b.ptr(),
        prop_object.ptr(),
        manager_object.ptr(),
    );
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_prop_manager(props);
    developer.set_occupant_manager(manager);
    for count in [0, 1, 2] {
        for remove in [0u32, 1, 0x100] {
            let buildings = [a, b];
            expect(
                this,
                "get_building",
                vec![p(lot)],
                if count == 0 { 0 } else { a as u64 },
            );
            expect(lot, "set_state", vec![w(6), w(0), w(0)], RETURN);
            for (index, &building) in buildings.iter().take(count).enumerate() {
                let occupant = (0x1000usize + index * 16) as *mut u8;
                expect(building, "add_ref", vec![], RETURN);
                expect(building, "get_occupant", vec![], occupant as u64);
                expect(props, "remove_building_props", vec![p(occupant)], RETURN);
                if remove as u8 != 0 {
                    expect(this, "remove_building_foundation", vec![p(occupant)], RETURN);
                }
                expect(
                    manager,
                    "remove_occupant",
                    vec![p(occupant), w(1), w(0x7654_3210)],
                    RETURN,
                );
                expect(building, "release", vec![], RETURN);
                expect(
                    this,
                    "get_building",
                    vec![p(lot)],
                    if index + 1 < count {
                        buildings[index + 1] as u64
                    } else {
                        0
                    },
                );
            }
            expect(this, "remove_props", vec![p(lot)], RETURN);
            assert_eq!(unsafe { fc4p_006c3c50(this, lot, 6, remove, 0x7654_3210) }, RETURN);
            finished();
        }
    }
}

#[test]
fn building_position_leaves_outputs_untouched_without_a_primary_object() {
    let mut dev_object = mock!(0x9C => get_position);
    let mut lot_object = mock!(0x24 => get_config);
    let mut cfg_object = mock!(0x60 => get_primary);
    let (this, lot, cfg) = (dev_object.ptr(), lot_object.ptr(), cfg_object.ptr());
    let mut position = [0xCCCC_CCCCu32; 3];
    let mut facing = 0xDDDD_DDDDu32;
    for branch in 0..3 {
        expect(
            lot,
            "get_config",
            vec![],
            if branch == 0 { RETURN & !0xFFFF_FFFF } else { cfg as u64 },
        );
        if branch != 0 {
            expect(
                cfg,
                "get_primary",
                vec![],
                if branch == 1 { RETURN & !0xFFFF_FFFF } else { 0x8888 },
            );
        }
        if branch == 2 {
            expect(
                this,
                "get_position",
                vec![p(lot), p(cfg), w(0x8888), out(&[1, 2, 3]), out(&[2])],
                RETURN,
            );
        }
        let returned = unsafe {
            fc4p_006c3cf0(
                this,
                0,
                lot,
                position.as_mut_ptr().cast(),
                (&mut facing as *mut u32).cast(),
            )
        };
        assert_eq!(returned, if branch == 2 { RETURN } else { RETURN & !0xFFFF_FFFF });
        if branch < 2 {
            assert_eq!(position, [0xCCCC_CCCC; 3]);
            assert_eq!(facing, 0xDDDD_DDDD);
        } else {
            assert_eq!(position, [1, 2, 3]);
            assert_eq!(facing, 2);
        }
        finished();
    }
}

#[test]
fn abandon_and_reoccupy_transfer_state_condition_and_population() {
    let mut dev_object = mock!();
    let mut lot_object = mock!(0x88 => set_state, 0xB8 => get_building_type, 0x0C => get_holder,
        0x90 => set_condition, 0xD0 => get_population, 0xDC => set_population);
    let mut bds_object = mock!(0x9C => get_profile, 0x7C => get_wealth);
    let mut holder_object = mock!(0x24 => get_property);
    let mut property_object = mock!(0x14 => get_variant);
    let mut variant_object = mock!(0x124 => get_data);
    let (this, lot, bds, holder, property, variant) = (
        dev_object.ptr(),
        lot_object.ptr(),
        bds_object.ptr(),
        holder_object.ptr(),
        property_object.ptr(),
        variant_object.ptr(),
    );
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_building_development(bds);
    expect(lot, "set_state", vec![w(4), w(0), w(0xABCD_0003)], RETURN);
    assert_eq!(unsafe { fc4p_006c3d30(this, lot, 0xABCD_0003) }, RETURN);
    finished();
    let mut profile = [0u32; 4];
    for (wealth, target, loss) in [
        (2i32, 0i32, 2),
        (2, 1, 1),
        (2, 2, 0),
        (0, 3, -3),
        (i32::MIN, i32::MAX, 1),
    ] {
        for has_property in [false, true] {
            for no_power in [0, 7] {
                profile[1] = wealth as u32;
                let mut power: u32 = no_power;
                let expected_condition = match loss {
                    1 => 2,
                    2 => 3,
                    _ => u32::from(has_property && no_power != 0),
                };
                expect(lot, "get_building_type", vec![w(0)], 0x3000);
                expect(bds, "get_profile", vec![w(0x3000)], profile.as_mut_ptr() as u64);
                expect(bds, "get_wealth", vec![w(0xDEAD_BEEF)], u64::from(target as u32));
                expect(lot, "get_holder", vec![], holder as u64);
                expect(
                    holder,
                    "get_property",
                    vec![w(0xE999_B11F)],
                    if has_property { property as u64 } else { 0 },
                );
                if has_property {
                    expect(property, "get_variant", vec![], variant as u64);
                    expect(variant, "get_data", vec![], &mut power as *mut u32 as u64);
                }
                expect(lot, "set_state", vec![w(3), w(target as u32), w(0)], RETURN);
                expect(lot, "set_condition", vec![w(expected_condition)], RETURN);
                expect(lot, "get_population", vec![w(0xDEAD_BEEF), w(0)], 123);
                expect(lot, "set_population", vec![w(0xDEAD_BEEF), w(123)], RETURN);
                assert_eq!(unsafe { fc4p_006c3d50(this, lot, 0xDEAD_BEEF) }, RETURN);
                finished();
            }
        }
    }
}

#[test]
fn foundation_lookup_uses_type_and_exact_position_words() {
    let mut dev_object =
        mock!(0xC8 => foundation_position, 0xD0 => remove_foundation_position, 0xC4 => foundation_cell);
    let mut occupant_object = mock!(0x20 => get_world_position, 0x1C => get_type);
    let mut lot_object = mock!(0x54 => get_location);
    let (this, occupant, lot) = (dev_object.ptr(), occupant_object.ptr(), lot_object.ptr());
    for (kind, foundation) in [(0x2781_28A0, 1), (0xA823_821E, 3), (0, 4), (0x2781_28A1, 4)] {
        expect(
            occupant,
            "get_world_position",
            vec![out(&[0x8000_0000, 0x1234, 0x7FC0_1234])],
            RETURN,
        );
        expect(occupant, "get_type", vec![], kind);
        expect(
            this,
            "foundation_position",
            vec![w(foundation), w(0x8000_0000), w(0x7FC0_1234)],
            RETURN,
        );
        assert_eq!(unsafe { fc4p_006c3e10(this, occupant) }, RETURN);
        finished();
    }
    expect(
        occupant,
        "get_world_position",
        vec![out(&[0x8000_0000, 0x1234, 0x7FC0_1234])],
        RETURN,
    );
    expect(
        this,
        "remove_foundation_position",
        vec![w(1), w(0x8000_0000), w(0x7FC0_1234)],
        RETURN,
    );
    assert_eq!(unsafe { fc4p_006c4150(this, occupant) }, RETURN);
    expect(
        lot,
        "get_location",
        vec![out(&[u32::MAX]), out(&[i32::MIN as u32])],
        RETURN,
    );
    expect(
        this,
        "foundation_cell",
        vec![w(2), w(u32::MAX), w(i32::MIN as u32)],
        RETURN,
    );
    assert_eq!(unsafe { fc4p_006c3e70(this, lot) }, RETURN);
    finished();
}

#[test]
fn remove_cell_foundation_clears_flags_even_when_no_occupant_exists() {
    let mut dev_object = mock!(0xC4 => foundation_cell);
    let mut foundation_object = mock!(0x0C => get_occupant);
    let mut occupant_object = mock!(4 => add_ref, 8 => release);
    let mut manager_object = mock!(0x54 => remove_occupant);
    let mut lots_object = mock!(0x18 => get_lot);
    let mut lot_object = mock!(0x30 => get_flags, 0x34 => set_flags);
    let (this, foundation, occupant, manager, lots, lot) = (
        dev_object.ptr(),
        foundation_object.ptr(),
        occupant_object.ptr(),
        manager_object.ptr(),
        lots_object.ptr(),
        lot_object.ptr(),
    );
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_occupant_manager(manager);
    developer.set_lot_manager(lots);
    for found in [false, true] {
        for has_lot in [false, true] {
            expect(
                this,
                "foundation_cell",
                vec![w(2), w(u32::MAX), w(4)],
                if found { foundation as u64 } else { 0 },
            );
            if found {
                expect(foundation, "get_occupant", vec![], occupant as u64);
                expect(occupant, "add_ref", vec![], RETURN);
                expect(manager, "remove_occupant", vec![p(occupant), w(1), w(0)], RETURN);
                expect(occupant, "release", vec![], RETURN);
            }
            expect(
                lots,
                "get_lot",
                vec![w(u32::MAX), w(4), w(0)],
                RETURN & !0xFFFF_FFFF | if has_lot { lot as u64 } else { 0 },
            );
            if has_lot {
                expect(lot, "get_flags", vec![], 0xFFFF_FFFB);
                expect(lot, "set_flags", vec![w(0xFFFF_FFF8)], RETURN);
            }
            let returned = unsafe { fc4p_006c3eb0(this, u32::MAX, 4) };
            let expected_eax = if has_lot { 0xA1B2_C300 } else { 0 } | u32::from(found);
            assert_eq!(returned, RETURN & !0xFFFF_FFFF | u64::from(expected_eax));
            finished();
        }
    }
}

#[test]
fn remove_lot_foundation_keeps_the_cell_result_and_clears_flags_again() {
    let mut dev_object = mock!(0xD4 => remove_foundation_cell);
    let mut lot_object = mock!(0x54 => get_location, 0x30 => get_flags, 0x34 => set_flags);
    let (this, lot) = (dev_object.ptr(), lot_object.ptr());
    for answer in [0, 1, 0xFE] {
        expect(lot, "get_location", vec![out(&[2]), out(&[3])], RETURN);
        expect(this, "remove_foundation_cell", vec![w(2), w(3)], 0xFFFF_0000 | answer);
        expect(lot, "get_flags", vec![], 0xCAFE_FFFF);
        expect(lot, "set_flags", vec![w(0xCAFE_FFFC)], RETURN);
        assert_eq!(unsafe { fc4p_006c3f30(this, lot) }, RETURN & !255 | answer);
        finished();
    }
}

unsafe extern "thiscall" fn remove_lot_direct(this: *mut u8, lot: u32) -> u64 {
    run(this, "remove_lot_direct", &[lot])
}

#[test]
fn update_foundation_handles_each_flag_and_preserves_existing_types() {
    let mut dev_object = mock!(0xCC => get_foundation, 0xDC => add_lot_type, 0xD8 => add_lot_config);
    let mut lot_object = mock!(0x30 => get_flags, 0x24 => get_config);
    let mut foundation_object = mock!(0x10 => get_foundation_type);
    let (this, lot, foundation) = (dev_object.ptr(), lot_object.ptr(), foundation_object.ptr());
    let _direct = fake(0x006C_3F30, remove_lot_direct as *const () as usize);
    for flags in [0u32, 4, 0x8000_0000, 1, 2, 3] {
        for existing in [false, true] {
            expect(lot, "get_flags", vec![], RETURN & !0xFFFF_FFFF | u64::from(flags));
            if flags & 3 != 0 {
                expect(
                    this,
                    "get_foundation",
                    vec![p(lot)],
                    if existing { foundation as u64 } else { 0 },
                );
                if existing {
                    expect(foundation, "get_foundation_type", vec![], 0x00C0_FFEE);
                    expect(this, "remove_lot_direct", vec![p(lot)], RETURN);
                    expect(
                        this,
                        "add_lot_type",
                        vec![p(lot), w(0x00C0_FFEE), w(1), w(0), w(0)],
                        RETURN,
                    );
                } else {
                    expect(lot, "get_config", vec![], 0x1234);
                    expect(
                        this,
                        "add_lot_config",
                        vec![p(lot), w(0x1234), w(1), w(0), w(0)],
                        RETURN,
                    );
                }
            }
            assert_eq!(
                unsafe { fc4p_006c3f80(this, 0x1111, lot) },
                if flags & 3 == 0 {
                    RETURN & !0xFFFF_FFFF | u64::from(flags)
                } else {
                    RETURN
                }
            );
            finished();
        }
    }
}

unsafe extern "thiscall" fn slope_value(this: *mut u8, occupant: u32, water: u32) -> f32 {
    f32::from_bits(run(this, "slope_value", &[occupant, water]) as u32)
}

#[unsafe(naked)]
unsafe extern "thiscall" fn slope_with_edx(_this: *mut u8, _occupant: u32, _water: u32) -> f32 {
    core::arch::naked_asm!("push dword ptr [esp + 8]", "push dword ptr [esp + 8]", "call {body}",
        "mov edx, 0x98765432", "ret 8", body = sym slope_value)
}

#[test]
fn foundation_threshold_is_strict_and_rejects_unordered_values() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x7C => slope_with_edx);
    let this = dev_object.ptr();
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    for (slope, threshold, required) in [
        (1.0, 2.0, false),
        (2.0, 2.0, false),
        (3.0, 2.0, true),
        (f32::NAN, 2.0, false),
        (2.0, f32::NAN, false),
        (f32::INFINITY, 2.0, true),
        (-0.0, 0.0, false),
    ] {
        developer.set_foundation_slope(threshold);
        expect(this, "slope_value", vec![w(0x1000), w(1)], u64::from(slope.to_bits()));
        assert_eq!(
            unsafe { fc4p_006c3ff0(this, 0, 0x1000usize as *mut u8) },
            0x9876_5432_0000_0000 | u64::from(required)
        );
        finished();
    }
}

unsafe extern "C" fn retaining(lot: u32) -> u64 {
    run(core::ptr::null_mut(), "retaining", &[lot])
}

#[test]
fn retaining_wall_refresh_calls_the_shared_helper() {
    let _helper = fake(0x005E_37F0, retaining as *const () as usize);
    expect(core::ptr::null_mut(), "retaining", vec![w(0x1234)], RETURN);
    assert_eq!(
        unsafe { fc4p_006c4190(core::ptr::null_mut(), 0x1234usize as *mut u8) },
        RETURN
    );
    finished();
}

#[test]
fn texture_update_uses_low_byte_success_and_releases_after_updating() {
    let mut dev_object = mock!(0xA4 => get_texture);
    let mut texture_object = mock!(0x24 => update_texture, 8 => release);
    let (this, texture) = (dev_object.ptr(), texture_object.ptr());
    for answer in [0u32, 0x1234_5600, 1, 0x1234_5680] {
        expect(
            this,
            "get_texture",
            vec![w(0x1000), out(&[texture as u32]), w(0)],
            RETURN & !0xFFFF_FFFF | u64::from(answer),
        );
        if answer as u8 != 0 {
            expect(texture, "update_texture", vec![w(1), w(0xFEDC_BA80), w(0)], RETURN);
            expect(texture, "release", vec![], RETURN);
        }
        assert_eq!(
            unsafe { fc4p_006c4290(this, 0x1000usize as *mut u8, 0xFEDC_BA80) },
            if answer as u8 != 0 {
                RETURN
            } else {
                RETURN & !0xFFFF_FFFF | u64::from(answer)
            }
        );
        finished();
    }
}

#[test]
fn prop_removal_clears_only_an_existing_zot_property() {
    let mut dev_object = mock!();
    let mut props_object = mock!(0x70 => remove_props);
    let mut lot_object = mock!(0x0C => get_holder);
    let mut holder_object = mock!(0x24 => get_property);
    let mut property_object = mock!(0x14 => get_variant);
    let mut variant_object = mock!(0x68 => set_uint);
    let (this, props, lot, holder, property, variant) = (
        dev_object.ptr(),
        props_object.ptr(),
        lot_object.ptr(),
        holder_object.ptr(),
        property_object.ptr(),
        variant_object.ptr(),
    );
    unsafe { LotDeveloper::from_ptr(this) }.set_prop_manager(props);
    for exists in [false, true] {
        expect(props, "remove_props", vec![p(lot)], RETURN);
        expect(lot, "get_holder", vec![], holder as u64);
        expect(
            holder,
            "get_property",
            vec![w(0xC999_C45E)],
            if exists { property as u64 } else { 0 },
        );
        if exists {
            expect(property, "get_variant", vec![], variant as u64);
            expect(variant, "set_uint", vec![w(0)], RETURN);
        }
        assert_eq!(
            unsafe { fc4p_006c42d0(this, lot) },
            if exists { RETURN as u32 } else { 0 }
        );
        finished();
    }
}

fn expect_float(object: *mut u8, method: &'static str, args: Vec<Arg>, returned: u64, value: F80) {
    expect(object, method, args, returned);
    FLOATS.with(|f| f.borrow_mut().push_back(value));
}

macro_rules! float_slot {
    ($entry:ident, $body:ident, $count:expr $(, $arg:ident)*) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $entry() {
            core::arch::naked_asm!(
                "sub esp, 12", "lea edx, [esp]", ".rept {count}",
                "push dword ptr [esp + {offset}]", ".endr", "call {body}",
                "fld tbyte ptr [esp]", "add esp, 12", "ret {pop}",
                count = const $count, offset = const 12 + 4 * $count, pop = const 4 * $count,
                body = sym $body,
            )
        }
        unsafe extern "fastcall" fn $body(this: *mut u8, output: *mut F80 $(, $arg: u32)*) -> u64 {
            let returned = run(this, stringify!($body), &[$($arg),*]);
            let value = FLOATS.with(|f| f.borrow_mut().pop_front()).unwrap();
            unsafe { output.write(value); }
            returned
        }
    };
}

float_slot!(altitude_float, altitude_body, 1, index);
float_slot!(water_float, water_body, 1, index);
float_slot!(vertex_float, vertex_body, 3, x, z, water);
float_slot!(area_slope_float, area_slope_body, 2, rect, water);
float_slot!(curve_float, curve_body, 1, input);
float_slot!(lowest_area_float, lowest_area_body, 3, rect, lowest, water);
float_slot!(lot_altitude_float, lot_altitude_body, 0);
mod area_altitude;
mod rubble;
mod construction;
mod business;
mod power_changes;
mod explosion;

mod services;
mod callbacks;
mod area_slope;
mod foundation_bottom;
mod pick_foundation;
mod add_foundation;
slot!(bounding_rect, rect);
slot!(coverage, occupant, rect);

unsafe fn call_float(entry: usize, this: *mut u8, args: &[u32]) -> RawCall {
    let mut call = RawCall::new(Conv::Thiscall, entry, this as u32, args);
    call.eax = 0xABCD_1234;
    call.edx = 0x1234_5678;
    call.take_st0 = 1;
    unsafe {
        call_raw(&mut call);
    }
    call
}

#[test]
fn vertex_altitude_uses_wrapping_stride_and_the_low_water_byte() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!();
    let mut terrain_object = mock!(0x30 => altitude_float, 0x7C => water_float);
    let (this, terrain) = (dev_object.ptr(), terrain_object.ptr());
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_terrain(terrain);
    developer.set_vertices_x(65);
    for water in [0u32, 0x100, 1, 0xFE] {
        let index = i32::MAX.wrapping_mul(65).wrapping_add(-2) as u32;
        expect_float(terrain, "altitude_body", vec![w(index)], RETURN, F80::from_f32(3.5));
        if water as u8 != 0 {
            expect_float(terrain, "water_body", vec![w(index)], RETURN, F80::from_f32(5.0));
        }
        let call = unsafe {
            call_float(
                fc4p_006c4310 as *const () as usize,
                this,
                &[(-2i32) as u32, i32::MAX as u32, water],
            )
        };
        assert_eq!(call.st0.to_f32(), if water as u8 != 0 { 5.0 } else { 3.5 });
        assert_eq!(call.edx, (RETURN >> 32) as u32);
        if water as u8 == 0 {
            assert_eq!(call.eax, 0xA1B2_C300);
        } else {
            assert_eq!(call.eax & 0xFFFF_7D00, 0xA1B2_3800);
        }
        finished();
    }
}

#[test]
fn vertex_altitude_keeps_unrounded_water_and_reloads_the_terrain_service() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!();
    let mut terrain_a = mock!(0x30 => altitude_float);
    let mut terrain_b = mock!(0x7C => water_float);
    let (this, a, b) = (dev_object.ptr(), terrain_a.ptr(), terrain_b.ptr());
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    for (altitude, water, expected) in [
        (2.0, F80::from_f32(1.0), F80::from_f32(2.0)),
        (2.0, F80::from_f32(2.0), F80::from_f32(2.0)),
        (2.0, F80::from_f32(f32::NAN), F80::from_f32(2.0)),
        (2.0, F80::from_f64(2f64.powi(200)), F80::from_f64(2f64.powi(200))),
    ] {
        developer.set_vertices_x(65);
        developer.set_terrain(a);
        expect_float(a, "altitude_body", vec![w(67)], RETURN, F80::from_f32(altitude));
        edit_last(unsafe { this.add(0x3C).cast() }, b as u32);
        edit_last(unsafe { this.add(0x40).cast() }, 999);
        expect_float(b, "water_body", vec![w(67)], RETURN, water);
        let call = unsafe { call_float(fc4p_006c4310 as *const () as usize, this, &[2, 1, 1]) };
        assert_eq!(call.st0.to_bytes(), expected.to_bytes());
        finished();
    }
}

#[test]
fn corners_follow_clockwise_order_and_observe_aliased_output_writes() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x3C => vertex_float);
    let this = dev_object.ptr();
    let rect = [0i32, 0, 1, 1];
    let mut heights = [0.0; 4];
    for ((x, z), height) in [(0, 0), (2, 0), (2, 2), (0, 2)].into_iter().zip([0.0, 7.25, 30.0, 9.5]) {
        expect_float(
            this,
            "vertex_body",
            vec![w(x), w(z), w(0xABCD_0001)],
            RETURN,
            F80::from_f32(height),
        );
    }
    assert_eq!(
        unsafe { fc4p_006c4360(this, 0, rect.as_ptr(), heights.as_mut_ptr(), 0xABCD_0001) },
        RETURN
    );
    assert_eq!(heights, [0.0, 7.25, 30.0, 9.5]);
    finished();
    let mut alias = [0i32, 0, 1, 1];
    for ((x, z), height) in [(0, 0), (2, 0), (2, 2), (0x3F80_0000, 2)]
        .into_iter()
        .zip([1.0, 2.0, 3.0, 4.0])
    {
        expect_float(
            this,
            "vertex_body",
            vec![w(x), w(z), w(0)],
            RETURN,
            F80::from_f32(height),
        );
    }
    unsafe {
        fc4p_006c4360(this, 0, alias.as_ptr(), alias.as_mut_ptr().cast(), 0);
    }
    assert_eq!(alias.map(|v| f32::from_bits(v as u32)), [1.0, 2.0, 3.0, 4.0]);
    finished();
}

#[test]
fn extrema_and_average_include_far_vertices_in_x_then_z_order() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x3C => vertex_float);
    let this = dev_object.ptr();
    let rect = [0i32, 0, 1, 1];
    let data = [0.0f32, 3.5, 7.25, 1.0, 12.0, 2.0, 9.5, 4.0, 30.0];
    for (water, lowest, expected) in [(0, 1, 0.0), (0, 0, 30.0), (1, 1, 5.0), (1, 0, 30.0)] {
        for x in 0..3 {
            for z in 0..3 {
                let height = data[(x + z * 3) as usize];
                let height = if water != 0 { height.max(5.0) } else { height };
                expect_float(
                    this,
                    "vertex_body",
                    vec![w(x), w(z), w(water)],
                    RETURN,
                    F80::from_f32(height),
                );
            }
        }
        let call = unsafe {
            call_float(
                fc4p_006c5620 as *const () as usize,
                this,
                &[rect.as_ptr() as u32, lowest, water],
            )
        };
        assert_eq!(call.st0.to_f32(), expected);
        assert_eq!(call.edx, 0x9876_5432);
        finished();
    }
    for water in [0u32, 1] {
        for x in 0..3 {
            for z in 0..3 {
                let height = data[(x + z * 3) as usize];
                expect_float(
                    this,
                    "vertex_body",
                    vec![w(x), w(z), w(water)],
                    RETURN,
                    F80::from_f32(if water != 0 { height.max(5.0) } else { height }),
                );
            }
        }
        let call = unsafe {
            call_float(
                fc4p_006c5b70 as *const () as usize,
                this,
                &[rect.as_ptr() as u32, water],
            )
        };
        let expected = F80::from_f32(if water == 0 { 69.25 } else { 83.75 }) / F80::from_i32(9);
        assert_eq!(call.st0.to_bytes(), expected.to_bytes());
        assert_eq!((call.eax, call.edx), (RETURN as u32, (RETURN >> 32) as u32));
        finished();
    }
}

#[test]
fn empty_rectangles_and_unordered_samples_keep_the_search_sentinels() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x3C => vertex_float);
    let this = dev_object.ptr();
    for rect in [[3i32, 0, 1, 0], [0, 3, 0, 1], [0, 0, i32::MAX, 0], [0, 0, 0, i32::MAX]] {
        for lowest in [0u32, 1] {
            let call = unsafe {
                call_float(
                    fc4p_006c5620 as *const () as usize,
                    this,
                    &[rect.as_ptr() as u32, lowest, 0],
                )
            };
            assert_eq!(call.st0.to_f32(), if lowest != 0 { f32::MAX } else { -f32::MAX });
            assert_eq!(call.edx, 0x1234_5678);
        }
        let call = unsafe { call_float(fc4p_006c5b70 as *const () as usize, this, &[rect.as_ptr() as u32, 0]) };
        assert!(call.st0.to_f32().is_nan());
        finished();
    }
    let rect = [0i32, 0, -1, -1];
    for lowest in [0u32, 1] {
        expect_float(
            this,
            "vertex_body",
            vec![w(0), w(0), w(0)],
            RETURN,
            F80::from_f32(f32::NAN),
        );
        let call = unsafe {
            call_float(
                fc4p_006c5620 as *const () as usize,
                this,
                &[rect.as_ptr() as u32, lowest, 0],
            )
        };
        assert_eq!(call.st0.to_f32(), if lowest != 0 { f32::MAX } else { -f32::MAX });
        finished();
    }
}

#[test]
fn slope_wrappers_forward_rectangles_and_do_not_round_extended_results() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x74 => area_slope_float, 0x58 => coverage);
    let mut lot_object = mock!(0x38 => bounding_rect);
    let (this, lot) = (dev_object.ptr(), lot_object.ptr());
    let extended = F80::from_f64(2f64.powi(200));
    let rect = [1u32, 2, 3, 4];
    expect(lot, "bounding_rect", vec![out(&rect)], RETURN);
    expect_float(
        this,
        "area_slope_body",
        vec![Arg::In(rect.to_vec()), w(0xABCD_0100)],
        RETURN,
        extended,
    );
    let call = unsafe { call_float(fc4p_006c5f20 as *const () as usize, this, &[lot as u32, 0xABCD_0100]) };
    assert_eq!(call.st0.to_bytes(), extended.to_bytes());
    assert_eq!((call.eax, call.edx), (RETURN as u32, (RETURN >> 32) as u32));
    finished();
    expect(this, "coverage", vec![w(0x1234), out(&rect)], RETURN);
    expect_float(
        this,
        "area_slope_body",
        vec![Arg::In(rect.to_vec()), w(0xFEDC_0001)],
        RETURN,
        extended,
    );
    let call = unsafe { call_float(fc4p_006c5f50 as *const () as usize, this, &[0x1234, 0xFEDC_0001]) };
    assert_eq!(call.st0.to_bytes(), extended.to_bytes());
    assert_eq!((call.eax, call.edx), (RETURN as u32, (RETURN >> 32) as u32));
    finished();
}

#[test]
fn development_cost_captures_dimensions_before_calls_and_uses_int64_truncation() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x74 => area_slope_float);
    let this = dev_object.ptr();
    let mut scale = 0.0625f32;
    let _scale = fake_data(0x00AB_1B04, (&mut scale as *mut f32).cast());
    let _curve = fake(0x005F_4250, curve_float as *const () as usize);
    for (rect, value, expected) in [
        ([0i32, 0, 1, 2], F80::from_f32(2.75), 16i64),
        ([0, 0, 1, 2], F80::from_f32(-2.75), -16),
        ([0, 0, 1, 2], F80::from_f32(f32::NAN), i64::MIN),
        ([0, 0, 1, 2], F80::from_f64(2f64.powi(70)), i64::MIN),
        ([i32::MIN, 0, i32::MAX, 0], F80::from_f32(2.0), 0),
    ] {
        let mut rect = rect;
        expect_float(
            this,
            "area_slope_body",
            vec![Arg::In(rect.map(|v| v as u32).to_vec()), w(1)],
            RETURN,
            F80::from_f32(32.0),
        );
        edit_last(rect.as_mut_ptr().cast(), 0);
        edit_last(unsafe { rect.as_mut_ptr().add(2).cast() }, 0);
        expect_float(
            unsafe { this.add(0xEC) },
            "curve_body",
            vec![w(2.0f32.to_bits())],
            RETURN,
            value,
        );
        assert_eq!(unsafe { fc4p_006c5f80(this, rect.as_ptr()) }, expected);
        finished();
    }
}

slot!(world_bounds, min, max);
slot!(get_facing);
slot!(get_exemplar);
slot!(
    create_foundation,
    kind,
    category,
    position,
    width,
    depth,
    facing,
    bottom
);
slot!(add_building_type, building, kind);
slot!(get_primary_foundation);
slot!(to_cell, x, z, cx, cz);
slot!(
    modify_height,
    do_it,
    vertices,
    x0,
    z0,
    x1,
    z1,
    altitude,
    lot,
    reason,
    cost
);

unsafe extern "C" fn read_uint_property(holder: *mut u8, id: u32, output: u32) -> u64 {
    run(holder, "read_uint_property", &[id, output])
}

unsafe extern "thiscall" fn draw_uniform(random: *mut u8, count: u32) -> u64 {
    run(random, "draw_uniform", &[count])
}

#[test]
fn explicit_building_foundation_type_falls_back_to_property_then_default() {
    let _fpu = GameFpu::new();
    let _property = fake(0x005F_D480, read_uint_property as *const () as usize);
    let mut dev_object = mock!();
    let mut building_object = mock!(0x0C => get_occupant, 0x38 => get_facing);
    let mut occupant_object = mock!(0x20 => get_world_position, 0x28 => world_bounds, 0x18 => get_exemplar);
    let mut bds_object = mock!(0x5C => create_foundation);
    let (this, building, occupant, bds) = (
        dev_object.ptr(),
        building_object.ptr(),
        occupant_object.ptr(),
        bds_object.ptr(),
    );
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_building_development(bds);
    developer.set_building_foundation_type(77);
    let position = [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()];
    for (explicit, property, expected) in [(12, 0, 12), (0, 18, 18), (0, 0, 77)] {
        expect(building, "get_occupant", vec![], occupant as u64);
        expect(occupant, "get_world_position", vec![out(&position)], RETURN);
        expect(building, "get_occupant", vec![], occupant as u64);
        expect(
            occupant,
            "world_bounds",
            vec![
                out(&[1.0f32.to_bits(), 0, 4.0f32.to_bits()]),
                out(&[9.0f32.to_bits(), 0, 16.0f32.to_bits()]),
            ],
            RETURN,
        );
        expect(building, "get_facing", vec![], 2);
        if explicit == 0 {
            expect(building, "get_occupant", vec![], occupant as u64);
            expect(occupant, "get_exemplar", vec![], 0x3333);
            expect(
                0x3333usize as *mut u8,
                "read_uint_property",
                vec![w(0x88FC_D877), out(&[property])],
                RETURN,
            );
        }
        expect(
            bds,
            "create_foundation",
            vec![
                w(expected),
                w(1),
                Arg::In(position.to_vec()),
                w(8.0f32.to_bits()),
                w(12.0f32.to_bits()),
                w(2),
                w((-1.0f32).to_bits()),
            ],
            RETURN,
        );
        assert_eq!(unsafe { fc4p_006c4020(this, building, explicit) }, RETURN);
        finished();
    }
}

#[test]
fn configuration_building_foundation_prefers_primary_and_preserves_missing_property_default() {
    let _property = fake(0x005F_D480, read_uint_property as *const () as usize);
    let mut dev_object = mock!(0xEC => add_building_type);
    let mut building_object = mock!(0x0C => get_occupant);
    let mut occupant_object = mock!(0x18 => get_exemplar);
    let mut cfg_object = mock!(0x60 => get_primary);
    let mut primary_object = Object::new(0x40);
    let (this, building, occupant, cfg, primary) = (
        dev_object.ptr(),
        building_object.ptr(),
        occupant_object.ptr(),
        cfg_object.ptr(),
        primary_object.ptr(),
    );
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    for (has_primary, primary_type, property, expected) in [
        (true, 12, None, 12),
        (true, 0, Some(18), 18),
        (false, 0, None, 77),
        (false, 0, Some(0), 0),
    ] {
        developer.set_building_foundation_type(77);
        unsafe {
            primary.add(0x20).cast::<u32>().write_unaligned(primary_type);
        }
        expect(cfg, "get_primary", vec![], if has_primary { primary as u64 } else { 0 });
        if !has_primary || primary_type == 0 {
            expect(building, "get_occupant", vec![], occupant as u64);
            expect(occupant, "get_exemplar", vec![], 0x3333);
            expect(
                0x3333usize as *mut u8,
                "read_uint_property",
                vec![w(0x88FC_D877), out(&property.map(|v| vec![v]).unwrap_or_default())],
                RETURN,
            );
        }
        if expected != 0 {
            expect(this, "add_building_type", vec![p(building), w(expected)], RETURN);
        }
        assert_eq!(
            unsafe { fc4p_006c40e0(this, building, cfg) },
            if expected != 0 { RETURN } else { RETURN & !0xFFFF_FFFF }
        );
        finished();
    }
}

#[test]
fn lot_foundation_selection_handles_null_inputs_default_primary_and_id_draws() {
    let _draw = fake(0x0091_36B1, draw_uniform as *const () as usize);
    let mut random = 0u8;
    let random_ptr = &mut random as *mut u8;
    let _random = fake_data(0x00B4_9AF0, random_ptr);
    let mut dev_object = mock!(0xDC => add_lot_type);
    let mut cfg_object = mock!(0x68 => get_primary_foundation);
    let mut primary_object = Object::new(0x40);
    let (this, cfg, primary) = (dev_object.ptr(), cfg_object.ptr(), primary_object.ptr());
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    let mut ids = vec![0u32; 258];
    ids[1] = 99;
    unsafe {
        primary.add(0x20).cast::<u32>().write_unaligned(88);
        primary.add(0x28).cast::<*const u32>().write_unaligned(ids.as_ptr());
    }
    for (lot, config) in [(0u32, 0u32), (1, 0), (0, cfg as u32)] {
        let args = [lot, config, 0xABCD_0100, 0x1234, 0x5678];
        let (eax, edx) =
            unsafe { call_thiscall_with_eax(fc4p_006c5460 as *const () as usize, this, &args, 0xAABB_CCDD) };
        assert_eq!((eax, edx), (0xAABB_CC00, 0));
        finished();
    }
    for (has_primary, count, draw, expected) in [
        (false, 0, 0, 77),
        (true, 0, 0, 88),
        (true, 2, 1, 99),
        (true, 258, 257, 0),
    ] {
        developer.set_lot_foundation_type(77);
        unsafe {
            primary
                .add(0x2C)
                .cast::<*const u32>()
                .write_unaligned(ids.as_ptr().add(count));
        }
        expect(
            cfg,
            "get_primary_foundation",
            vec![],
            RETURN & !0xFFFF_FFFF | if has_primary { primary as u64 } else { 0 },
        );
        edit_last(unsafe { this.add(0xD8).cast() }, 123);
        if has_primary && count != 0 {
            expect(
                random_ptr,
                "draw_uniform",
                vec![w(count as u32)],
                RETURN & !0xFFFF_FFFF | draw,
            );
        }
        if expected != 0 {
            expect(
                this,
                "add_lot_type",
                vec![w(1), w(expected), w(0xABCD_0100), w(0x1234), w(0x5678)],
                RETURN,
            );
        }
        let args = [1, cfg as u32, 0xABCD_0100, 0x1234, 0x5678];
        let mut call = RawCall::new(Conv::Thiscall, fc4p_006c5460 as *const () as usize, this as u32, &args);
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!(
            result(call),
            if expected != 0 {
                RETURN
            } else {
                RETURN & !0xFFFF_FFFF | 0x100
            }
        );
        finished();
    }
}

#[test]
fn occupant_coverage_insets_bounds_and_reloads_city_for_the_maximum() {
    let _fpu = GameFpu::new();
    let mut inset = 0.05f32;
    let _inset = fake_data(0x00A8_80D8, (&mut inset as *mut f32).cast());
    let mut dev_object = mock!();
    let mut occupant_object = mock!(0x28 => world_bounds);
    let mut city_a = mock!(0x17C => to_cell);
    let mut city_b = mock!(0x17C => to_cell);
    let (this, occupant, a, b) = (dev_object.ptr(), occupant_object.ptr(), city_a.ptr(), city_b.ptr());
    unsafe { LotDeveloper::from_ptr(this) }.set_city(a);
    let mut rect = [0xCCCC_CCCCu32; 4];
    expect(
        occupant,
        "world_bounds",
        vec![out(&[0, 0, 0]), out(&[16.0f32.to_bits(), 0, 32.0f32.to_bits()])],
        RETURN,
    );
    expect(
        a,
        "to_cell",
        vec![w(inset.to_bits()), w(inset.to_bits()), out(&[0]), out(&[0])],
        RETURN,
    );
    edit_last(unsafe { this.add(0x1C).cast() }, b as u32);
    expect(
        b,
        "to_cell",
        vec![
            w((16.0f32 - inset).to_bits()),
            w((32.0f32 - inset).to_bits()),
            out(&[0]),
            out(&[1]),
        ],
        RETURN,
    );
    assert_eq!(unsafe { fc4p_006c4590(this, occupant, rect.as_mut_ptr()) }, RETURN);
    assert_eq!(rect, [0, 0, 0, 1]);
    finished();
}

#[test]
fn bordering_vertices_pass_full_target_and_lot_words_and_wrap_upper_bounds() {
    let mut dev_object = mock!();
    let mut demo_object = mock!(0x24 => modify_height);
    let (this, demo) = (dev_object.ptr(), demo_object.ptr());
    unsafe { LotDeveloper::from_ptr(this) }.set_demolition(demo);
    let rect = [-1i32, 3, i32::MAX, 6];
    expect(
        demo,
        "modify_height",
        vec![
            w(1),
            w(1),
            w(u32::MAX),
            w(3),
            w(i32::MIN as u32),
            w(7),
            w(0x7FC0_1234),
            w(0x1234_5678),
            w(5),
            w(0),
        ],
        RETURN,
    );
    assert_eq!(
        unsafe { fc4p_006c44b0(this, rect.as_ptr(), 0x7FC0_1234, 0xFFFF_FFFF, 0x1234_5678) },
        RETURN
    );
    finished();
}

#[test]
fn floating_adapter_keeps_existing_x87_values_control_word_mxcsr_and_direction_flag() {
    #[repr(C, align(16))]
    struct State([u8; 512]);
    let _fpu = GameFpu::new();
    let mut dev_object = mock!();
    let mut terrain_object = mock!(0x30 => altitude_float);
    let (this, terrain) = (dev_object.ptr(), terrain_object.ptr());
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_terrain(terrain);
    developer.set_vertices_x(65);
    expect_float(terrain, "altitude_body", vec![w(67)], RETURN, F80::from_f32(3.5));
    let mut before = State([0; 512]);
    let mut after = State([0; 512]);
    let before_flags: u32;
    let after_flags: u32;
    unsafe {
        core::arch::asm!("fld1", "fxsave [{state}]", "pushfd", "pop {flags}",
            state = in(reg) &mut before, flags = out(reg) before_flags, out("st(0)") _,);
        let call = call_float(fc4p_006c4310 as *const () as usize, this, &[2, 1, 0]);
        core::arch::asm!("fxsave [{state}]", "fstp st(0)", "pushfd", "pop {flags}",
            state = in(reg) &mut after, flags = out(reg) after_flags, out("st(0)") _,);
        assert_eq!(call.st0.to_f32(), 3.5);
    }
    assert_eq!(before.0[..2], after.0[..2]);
    assert_eq!(before.0[3] & 0x38, after.0[3] & 0x38);
    assert_eq!(before.0[4], after.0[4]);
    assert_eq!(before.0[24..28], after.0[24..28]);
    assert_eq!(before.0[32..42], after.0[32..42]);
    assert_eq!(before_flags & 0x400, after_flags & 0x400);
    finished();
}

slot!(find_occupant, output, x, z, kind);
slot!(query_texture, iid, output);
slot!(texture_kind, kind);
slot!(create_texture, clsid, iid, output);
slot!(initialize_occupant);

unsafe extern "C" fn framework_fake() -> *mut u8 {
    run(core::ptr::null_mut(), "framework", &[]) as u32 as *mut u8
}
unsafe extern "C" fn allocate_texture_fake(size: u32) -> *mut u8 {
    run(core::ptr::null_mut(), "allocate_texture", &[size]) as u32 as *mut u8
}
unsafe extern "thiscall" fn construct_texture_fake(this: *mut u8) -> *mut u8 {
    run(this, "construct_texture", &[]) as u32 as *mut u8
}

#[test]
fn base_texture_lookup_observes_query_results_and_retains_failed_queries() {
    let mut dev_object = mock!();
    let mut lot_object = mock!(0x54 => get_location);
    let mut manager_object = mock!(0x84 => find_occupant);
    let mut occupant_object = mock!(0 => query_texture, 8 => release);
    let mut texture_object = mock!(0x14 => texture_kind);
    let (this, lot, manager, occupant, texture) = (
        dev_object.ptr(),
        lot_object.ptr(),
        manager_object.ptr(),
        occupant_object.ptr(),
        texture_object.ptr(),
    );
    unsafe { LotDeveloper::from_ptr(this) }.set_occupant_manager(manager);
    for (found, query, output_present, create) in [
        (false, 0, false, 0u32),
        (false, 0, false, 0x100),
        (true, 0, false, 1),
        (true, 0x100, true, 1),
        (true, 0xFF, true, 0),
    ] {
        let mut output = 0x1234usize as *mut u8;
        expect(lot, "get_location", vec![out(&[7]), out(&[9])], RETURN);
        expect(
            manager,
            "find_occupant",
            vec![out(&[occupant as u32]), w(7), w(9), w(0x497F_6D9D)],
            RETURN & !255 | u64::from(found),
        );
        if found {
            expect(
                occupant,
                "query_texture",
                vec![w(0x097F_6D98), out(&[if output_present { texture as u32 } else { 0 }])],
                RETURN & !0xFFFF_FFFF | query,
            );
            if query as u8 != 0 {
                expect(texture, "texture_kind", vec![w(2)], RETURN);
                expect(occupant, "release", vec![], RETURN);
            }
        }
        assert_eq!(
            unsafe { fc4p_006c41a0(this, lot, &mut output, create) },
            RETURN & !0xFFFF_FFFF | u64::from(output_present)
        );
        assert_eq!(output, if output_present { texture } else { core::ptr::null_mut() });
        finished();
    }
}

#[test]
fn missing_base_texture_uses_framework_then_allocator_fallback_and_initializes_in_order() {
    let _framework = fake(0x0090_DDF1, framework_fake as *const () as usize);
    let _allocate = fake(0x005D_ECF0, allocate_texture_fake as *const () as usize);
    let _construct = fake(0x005E_05B0, construct_texture_fake as *const () as usize);
    let mut dev_object = mock!();
    let mut lot_object = mock!(0x54 => get_location);
    let mut manager_object = mock!(0x84 => find_occupant);
    let mut framework_object = mock!(0x0C => create_texture);
    let mut texture_object = mock!(4 => add_ref, 0x0C => get_occupant, 0x14 => texture_kind);
    let mut occupant_object = mock!(0x0C => initialize_occupant);
    let (this, lot, manager, framework, texture, occupant) = (
        dev_object.ptr(),
        lot_object.ptr(),
        manager_object.ptr(),
        framework_object.ptr(),
        texture_object.ptr(),
        occupant_object.ptr(),
    );
    unsafe { LotDeveloper::from_ptr(this) }.set_occupant_manager(manager);
    for factory_success in [false, true] {
        let mut output = core::ptr::null_mut();
        expect(lot, "get_location", vec![out(&[7]), out(&[9])], RETURN);
        expect(manager, "find_occupant", vec![out(&[]), w(7), w(9), w(0x497F_6D9D)], 0);
        expect(core::ptr::null_mut(), "framework", vec![], framework as u64);
        expect(
            framework,
            "create_texture",
            vec![
                w(0xC97F_987C),
                w(0x097F_6D98),
                out(&if factory_success { vec![texture as u32] } else { vec![] }),
            ],
            u64::from(factory_success),
        );
        if !factory_success {
            expect(core::ptr::null_mut(), "allocate_texture", vec![w(0x60)], texture as u64);
            expect(texture, "construct_texture", vec![], texture as u64);
            expect(texture, "add_ref", vec![], RETURN);
        }
        expect(texture, "get_occupant", vec![], occupant as u64);
        expect(occupant, "initialize_occupant", vec![], RETURN);
        expect(texture, "texture_kind", vec![w(2)], RETURN);
        assert_eq!(
            unsafe { fc4p_006c41a0(this, lot, &mut output, 0xABCD_0001) },
            RETURN & !0xFFFF_FFFF | 1
        );
        assert_eq!(output, texture);
        finished();
    }
}

#[test]
fn positional_foundation_removal_accepts_null_occupants_and_only_clears_lot_kind_flags() {
    let _fpu = GameFpu::new();
    let mut scale = 0.0625f32;
    let _scale = fake_data(0x00AB_1B04, (&mut scale as *mut f32).cast());
    let mut dev_object = mock!(0xC8 => foundation_position);
    let mut foundation_object = mock!(0x0C => get_occupant);
    let mut occupant_object = mock!(4 => add_ref, 8 => release);
    let mut manager_object = mock!(0x54 => remove_occupant);
    let mut lots_object = mock!(0x18 => get_lot);
    let mut lot_object = mock!(0x30 => get_flags, 0x34 => set_flags);
    let (this, foundation, occupant, manager, lots, lot) = (
        dev_object.ptr(),
        foundation_object.ptr(),
        occupant_object.ptr(),
        manager_object.ptr(),
        lots_object.ptr(),
        lot_object.ptr(),
    );
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_occupant_manager(manager);
    developer.set_lot_manager(lots);
    for kind in [1u32, 2, 3] {
        for found in [false, true] {
            for has_occupant in [false, true] {
                for has_lot in [false, true] {
                    let occupant = if has_occupant { occupant } else { core::ptr::null_mut() };
                    expect(
                        this,
                        "foundation_position",
                        vec![w(kind), w((-17.0f32).to_bits()), w(31.0f32.to_bits())],
                        RETURN & !0xFFFF_FFFF | if found { foundation as u64 } else { 0 },
                    );
                    if found {
                        expect(foundation, "get_occupant", vec![], occupant as u64);
                        if has_occupant {
                            expect(occupant, "add_ref", vec![], RETURN);
                        }
                        expect(manager, "remove_occupant", vec![p(occupant), w(1), w(0)], RETURN);
                        if has_occupant {
                            expect(occupant, "release", vec![], RETURN);
                        }
                    }
                    if kind == 2 {
                        expect(
                            lots,
                            "get_lot",
                            vec![w(u32::MAX), w(1), w(0)],
                            RETURN & !0xFFFF_FFFF | if has_lot { lot as u64 } else { 0 },
                        );
                        if has_lot {
                            expect(lot, "get_flags", vec![], u64::from(u32::MAX));
                            expect(lot, "set_flags", vec![w(0xFFFF_FFFC)], RETURN);
                        }
                    }
                    let call = unsafe { fc4p_006c54e0(this, kind, (-17.0f32).to_bits(), 31.0f32.to_bits()) };
                    let last_full_result = (kind != 2 && found) || (kind == 2 && has_lot);
                    assert_eq!(
                        call,
                        RETURN & !0xFFFF_FFFF | if last_full_result { 0xA1B2_C300 } else { 0 } | u64::from(found)
                    );
                    finished();
                }
            }
        }
    }
}

float_slot!(average_float, average_body, 2, rect, water);
slot!(is_water, x, z);

fn expect_modify(demo: *mut u8, x: u32, z: u32, altitude: f32) {
    expect(
        demo,
        "modify_height",
        vec![
            w(1),
            w(1),
            w(x),
            w(z),
            w(x),
            w(z),
            w(altitude.to_bits()),
            w(0x1234),
            w(5),
            w(0),
        ],
        RETURN,
    );
}

#[test]
fn internal_levelling_observes_strict_delta_water_skips_and_force() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x3C => vertex_float);
    let mut terrain_object = mock!(0x94 => is_water);
    let mut demo_object = mock!(0x24 => modify_height);
    let (this, terrain, demo) = (dev_object.ptr(), terrain_object.ptr(), demo_object.ptr());
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_terrain(terrain);
    developer.set_demolition(demo);
    developer.set_level_delta(2.0);
    let rect = [0i32, 0, 2, 2];
    for (skip, force) in [(0u32, 0u32), (1, 1), (0x100, 0x100)] {
        let mut count = 0;
        for (x, z, altitude) in [(1, 1, 11.0), (1, 2, 12.0), (2, 1, 13.0), (2, 2, f32::NAN)] {
            let wet = x == 1 && z == 2;
            if skip as u8 != 0 {
                expect(terrain, "is_water", vec![w(x), w(z)], 0xFFFF_FF00 | u64::from(wet));
            }
            if skip as u8 != 0 && wet {
                continue;
            }
            expect_float(
                this,
                "vertex_body",
                vec![w(x), w(z), w(1)],
                RETURN,
                F80::from_f32(altitude),
            );
            if force as u8 != 0 || altitude > 12.0 {
                expect_modify(demo, x, z, 10.0);
                count += 1;
            }
        }
        assert_eq!(
            unsafe { fc4p_006c43d0(this, 0, rect.as_ptr(), 10.0, skip, force, 0x1234) },
            ((rect.as_ptr() as u64) << 32) | count
        );
        finished();
    }
}

#[test]
fn internal_levelling_uses_average_for_negative_targets_and_rereads_mutated_bounds() {
    let _fpu = GameFpu::new();
    let mut dev_object = mock!(0x3C => vertex_float, 0x48 => average_float);
    let mut demo_object = mock!(0x24 => modify_height);
    let (this, demo) = (dev_object.ptr(), demo_object.ptr());
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_demolition(demo);
    let mut rect = [0i32, 0, 2, 2];
    expect_float(
        this,
        "average_body",
        vec![Arg::In(rect.map(|v| v as u32).to_vec()), w(1)],
        RETURN,
        F80::from_f32(10.5),
    );
    expect_float(this, "vertex_body", vec![w(1), w(1), w(1)], RETURN, F80::from_f32(15.0));
    expect_modify(demo, 1, 1, 10.5);
    edit_last(unsafe { rect.as_mut_ptr().add(2).cast() }, 0);
    edit_last(unsafe { rect.as_mut_ptr().add(3).cast() }, 0);
    assert_eq!(
        unsafe { fc4p_006c43d0(this, 0, rect.as_ptr(), -1.0, 0, 1, 0x1234) },
        ((rect.as_ptr() as u64) << 32) | 1
    );
    finished();
    let empty = [3i32, 0, 1, 1];
    let returned = unsafe { fc4p_006c43d0(this, 0xAABB_CCDD, empty.as_ptr(), f32::NAN, 1, 1, 0) };
    assert_eq!(returned, 0xAABB_CCDD_0000_0000);
    finished();
}

#[test]
fn attempt_levelling_tests_slope_only_below_the_limit_and_handles_sentinel_or_nan_maxima() {
    let _fpu = GameFpu::new();
    let mut sentinel = f32::MAX;
    let mut limit = 1000.0f32;
    let _sentinel = fake_data(0x00AB_1B00, (&mut sentinel as *mut f32).cast());
    let _limit = fake_data(0x00A8_FAF0, (&mut limit as *mut f32).cast());
    let mut dev_object = mock!(0x48 => average_float, 0x74 => area_slope_float);
    let mut demo_object = mock!(0x24 => modify_height);
    let (this, demo) = (dev_object.ptr(), demo_object.ptr());
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_demolition(demo);
    developer.set_level_max_slope(5.0);
    let rect = [1i32, 2, 3, 4];
    for (maximum, slope, check, reject) in [
        (5.0, 6.0, true, true),
        (5.0, 5.0, true, false),
        (1000.0, 0.0, false, false),
        (f32::INFINITY, 0.0, false, false),
        (f32::MAX, 6.0, true, true),
        (f32::NAN, 3.0, true, false),
        (5.0, f32::NAN, true, false),
    ] {
        expect_float(
            this,
            "average_body",
            vec![Arg::In(rect.map(|v| v as u32).to_vec()), w(0x1234_0100)],
            RETURN,
            F80::from_f32(12.5),
        );
        if check {
            expect_float(
                this,
                "area_slope_body",
                vec![Arg::In(rect.map(|v| v as u32).to_vec()), w(0x1234_0100)],
                RETURN,
                F80::from_f32(slope),
            );
        }
        if !reject {
            expect(
                demo,
                "modify_height",
                vec![
                    w(1),
                    w(0xABCD_0080),
                    w(1),
                    w(2),
                    w(4),
                    w(5),
                    w(12.5f32.to_bits()),
                    w(0x1234),
                    w(5),
                    w(0),
                ],
                RETURN,
            );
        }
        assert_eq!(
            unsafe { fc4p_006c44f0(this, rect.as_ptr(), u32::MAX, 0x1234_0100, 0xABCD_0080, maximum, 0x1234) },
            if reject { RETURN & !0xFFFF_FFFF } else { RETURN }
        );
        finished();
    }
}

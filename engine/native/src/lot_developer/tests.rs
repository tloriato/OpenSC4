// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native conformance cases, including the habitation and terrain cases from the core lot developer.

use super::{foundation::*, lots::*, small::*, *};
use crate::testing::{call_thiscall_with_eax, fake, GameFpu, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

const RETURN: u64 = 0x9876_5432_A1B2_C3D4;

enum Arg {
    Word(u32),
    Out(Vec<u32>),
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
        }
    }
    for (address, value) in step.edits {
        unsafe { address.write_unaligned(value) };
    }
    step.returned
}

macro_rules! slot {
    ($name:ident $(, $arg:ident)*) => {
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

struct Mock {
    memory: Object,
    _vtable: Vec<usize>,
}

impl Mock {
    fn new(slots: &[(usize, usize)]) -> Self {
        let mut table = vec![0; 0x128 / 4];
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

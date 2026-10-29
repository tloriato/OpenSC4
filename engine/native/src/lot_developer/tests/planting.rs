// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::planting::*;

slot!(plant_valid, kind);
slot!(plant_family, set, kind);
slot!(plant_objects);
slot!(plant_choices);
unsafe extern "thiscall" fn plant_position(
    this: *mut u8,
    lot: u32,
    config: u32,
    record: u32,
    position: u32,
    facing: u32,
) -> u64 {
    let initial = unsafe { (crate::abi::data_target(0x00B4_9AEC) as *const u32).read() };
    assert_eq!(
        unsafe { std::slice::from_raw_parts(position as *const u32, 3) },
        [initial; 3]
    );
    run(this, "plant_position", &[lot, config, record, position, facing])
}
slot!(plant_add, kind, position, flags);
slot!(plant_tag, tag);
slot!(plant_construct, lot, config, kind, flags);

unsafe extern "thiscall" fn set_make(this: *mut u32, capacity: u32) {
    assert_eq!(unsafe { std::slice::from_raw_parts(this.add(1), 4) }, [0; 4]);
    run(core::ptr::null_mut(), "set_make", &[this as u32, capacity]);
}
unsafe extern "thiscall" fn set_insert(this: *mut u32, output: *mut u32, value: *const u32) {
    run(
        core::ptr::null_mut(),
        "set_insert",
        &[this as u32, output as u32, unsafe { value.read() }],
    );
}
unsafe extern "thiscall" fn set_begin(this: *mut u32, output: *mut u32) {
    run(core::ptr::null_mut(), "set_begin", &[this as u32, output as u32]);
}
unsafe extern "thiscall" fn set_next(this: *mut u32) -> u32 {
    run(core::ptr::null_mut(), "set_next", &[this as u32]) as u32
}
unsafe extern "thiscall" fn set_destroy(this: *mut u32) {
    run(core::ptr::null_mut(), "set_destroy", &[this as u32]);
}
unsafe extern "C" fn set_free(storage: u32) {
    run(core::ptr::null_mut(), "set_free", &[storage]);
}
unsafe extern "thiscall" fn plant_random(this: *mut u8, count: u32) -> u32 {
    run(this, "plant_random", &[count]) as u32
}
unsafe extern "thiscall" fn plant_pick(this: *mut u8, ids: *mut u32, position: *mut u32) -> u32 {
    run(this, "plant_pick", &[ids as u32, position as u32]) as u32
}

fn set_fakes() -> Vec<crate::testing::Fake> {
    [
        (0x0082_9BC0, set_make as *const () as usize),
        (0x0045_D650, set_insert as *const () as usize),
        (0x004B_0390, set_begin as *const () as usize),
        (0x0053_DCF0, set_next as *const () as usize),
        (0x008E_BA60, set_destroy as *const () as usize),
        (0x0090_CF63, set_free as *const () as usize),
        (0x0091_36B1, plant_random as *const () as usize),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f))
    .collect()
}

fn input(values: &[u32]) -> Arg {
    Arg::In(values.to_vec())
}

#[test]
fn flora_empty_explicit_and_family_selection_preserve_call_order() {
    let _fakes = set_fakes();
    let mut random = 0u8;
    let _random = fake_data(0x00B4_9AF0, &mut random);
    let mut developer = Object::new(0x108);
    let mut flora = mock!(0x2C => plant_valid, 0x38 => plant_family);
    unsafe {
        crate::lot_developer::updates::UpdateState::from_ptr(developer.ptr()).set_flora(flora.ptr());
    }
    let mut ids = [0u32; 3];
    assert_eq!(
        unsafe { fc4p_006cd010(developer.ptr(), ids.as_mut_ptr(), core::ptr::null_mut()) },
        0
    );
    finished();
    let mut types = [0x1100, 0x2200, 0x3300];
    ids[0] = types.as_mut_ptr() as u32;
    ids[1] = ids[0] + 12;
    expect(&mut random, "plant_random", vec![w(3)], 1);
    expect(flora.ptr(), "plant_valid", vec![w(0x2200)], 0xAABB_CC01);
    assert_eq!(
        unsafe { fc4p_006cd010(developer.ptr(), ids.as_mut_ptr(), core::ptr::null_mut()) },
        0x2200
    );
    finished();

    // Follow a chain once, then use the helper to cross an empty bucket.
    let mut second = [0u32, 0xB2];
    let mut first = [second.as_mut_ptr() as u32, 0xA1];
    let mut third = [0u32, 0xC3];
    let set = [0x11, 0x1234, 0x1244, 0x1254, 3];
    expect(&mut random, "plant_random", vec![w(3)], 0);
    expect(flora.ptr(), "plant_valid", vec![w(0x1100)], 0xABCD_EF00);
    expect(core::ptr::null_mut(), "set_make", vec![out(&set), w(100)], 0);
    expect(flora.ptr(), "plant_family", vec![input(&set), w(0x1100)], 0x100);
    expect(&mut random, "plant_random", vec![w(3)], 2);
    expect(
        core::ptr::null_mut(),
        "set_begin",
        vec![input(&set), out(&[first.as_mut_ptr() as u32, 0x9876])],
        0,
    );
    expect(
        core::ptr::null_mut(),
        "set_next",
        vec![input(&[second.as_mut_ptr() as u32, 0x9876])],
        third.as_mut_ptr() as u64,
    );
    expect(core::ptr::null_mut(), "set_destroy", vec![input(&set)], 0);
    expect(core::ptr::null_mut(), "set_free", vec![w(0x1234)], 0);
    assert_eq!(
        unsafe { fc4p_006cd010(developer.ptr(), ids.as_mut_ptr(), core::ptr::null_mut()) },
        0xC3
    );
    finished();
}

#[test]
fn flora_failed_and_empty_families_are_destroyed_and_freed() {
    let _fakes = set_fakes();
    let mut random = 0u8;
    let _random = fake_data(0x00B4_9AF0, &mut random);
    for (found, count, storage) in [(0, 1, 0), (0, 1, 0x1234), (1, 0, 0x1234)] {
        let mut developer = Object::new(0x108);
        let mut flora = mock!(0x2C => plant_valid, 0x38 => plant_family);
        unsafe {
            crate::lot_developer::updates::UpdateState::from_ptr(developer.ptr()).set_flora(flora.ptr());
        }
        let mut kind = 0x1111u32;
        let begin = &mut kind as *mut u32 as u32;
        let mut ids = [begin, begin + 4, begin + 4];
        let set = [0x11, storage, 0, 0, count];
        expect(&mut random, "plant_random", vec![w(1)], 0);
        expect(flora.ptr(), "plant_valid", vec![w(kind)], 0);
        expect(core::ptr::null_mut(), "set_make", vec![out(&set), w(100)], 0);
        expect(flora.ptr(), "plant_family", vec![input(&set), w(kind)], found);
        expect(core::ptr::null_mut(), "set_destroy", vec![input(&set)], 0);
        if storage != 0 {
            expect(core::ptr::null_mut(), "set_free", vec![w(storage)], 0);
        }
        assert_eq!(
            unsafe { fc4p_006cd010(developer.ptr(), ids.as_mut_ptr(), core::ptr::null_mut()) },
            0
        );
        finished();
    }
}

#[test]
fn flora_selection_reloads_the_id_vector_after_random_and_the_service_after_validation() {
    let _fakes = set_fakes();
    let mut random = 0u8;
    let _random = fake_data(0x00B4_9AF0, &mut random);
    let mut developer = Object::new(0x108);
    let mut first = mock!(0x2C => plant_valid);
    let mut second = mock!(0x38 => plant_family);
    let mut original = [0x1111u32];
    let mut replacement = [0x2222u32];
    let mut ids = [original.as_mut_ptr() as u32, original.as_mut_ptr() as u32 + 4, 0];
    unsafe {
        crate::lot_developer::updates::UpdateState::from_ptr(developer.ptr()).set_flora(first.ptr());
    }
    expect(&mut random, "plant_random", vec![w(1)], 0);
    edit_last(ids.as_mut_ptr(), replacement.as_mut_ptr() as u32);
    expect(first.ptr(), "plant_valid", vec![w(0x2222)], 0);
    edit_last(unsafe { developer.ptr().add(0x38).cast() }, second.ptr() as u32);
    expect(core::ptr::null_mut(), "set_make", vec![out(&[0; 5]), w(100)], 0);
    expect(second.ptr(), "plant_family", vec![input(&[0; 5]), w(0x2222)], 0);
    expect(core::ptr::null_mut(), "set_destroy", vec![input(&[0; 5])], 0);
    assert_eq!(
        unsafe { fc4p_006cd010(developer.ptr(), ids.as_mut_ptr(), core::ptr::null_mut()) },
        0
    );
    finished();
}

unsafe fn construct(this: *mut u8, args: [u32; 4]) -> (u64, u32) {
    let mut frame = [
        fc4p_006ccef0 as *const () as u32,
        this as u32,
        args[0],
        args[1],
        args[2],
        args[3],
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push [edi + 20]", "push [edi + 16]", "push [edi + 12]", "push [edi + 8]",
            "mov ecx, [edi + 4]", "call dword ptr [edi]",
            "mov [edi + 24], eax", "mov [edi + 28], edx", "mov eax, [esp - 8]", "mov [edi + 32], eax",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"),
        );
    }
    ((u64::from(frame[7]) << 32) | u64::from(frame[6]), frame[8])
}

#[test]
fn default_construction_short_paths_forward_all_arguments_and_register_results() {
    let mut developer = mock!(0x18 => plant_construct);
    let mut config = mock!(0x60 => plant_choices);
    let mut choices = Object::new(0x30);
    for (cfg, kind, query) in [
        (0, 0, false),
        (config.ptr() as u32, 7, false),
        (config.ptr() as u32, 0, true),
    ] {
        if query {
            expect(config.ptr(), "plant_choices", vec![], choices.ptr() as u64);
        }
        expect(
            developer.ptr(),
            "plant_construct",
            vec![w(0x123), w(cfg), w(kind), w(0xABCD_0101)],
            RETURN,
        );
        assert_eq!(
            unsafe { construct(developer.ptr(), [0x123, cfg, kind, 0xABCD_0101]) },
            (RETURN, kind)
        );
        finished();
    }
    expect(config.ptr(), "plant_choices", vec![], 0);
    expect(
        developer.ptr(),
        "plant_construct",
        vec![w(0x123), p(config.ptr()), w(0), w(9)],
        RETURN,
    );
    assert_eq!(
        unsafe { construct(developer.ptr(), [0x123, config.ptr() as u32, 0, 9]) },
        (RETURN, 0)
    );
    finished();
}

#[test]
fn default_construction_collects_types_and_families_then_writes_the_first_set_value() {
    let _fakes = set_fakes();
    let mut developer = mock!(0x18 => plant_construct);
    let mut manager = mock!(0x3C => plant_valid, 0x4C => plant_family);
    let mut config = mock!(0x60 => plant_choices);
    let mut choices = Object::new(0x30);
    let mut kinds = [0x11, 0x22];
    let mut node = [0u32, 0x3344];
    let set = [0x10, 0x1234, 0, 0, 1];
    unsafe {
        LotDeveloper::from_ptr(developer.ptr()).set_building_development(manager.ptr());
        choices.ptr().add(0x28).cast::<u32>().write(kinds.as_mut_ptr() as u32);
        choices
            .ptr()
            .add(0x2C)
            .cast::<u32>()
            .write(kinds.as_mut_ptr().add(2) as u32);
    }
    expect(config.ptr(), "plant_choices", vec![], choices.ptr() as u64);
    expect(core::ptr::null_mut(), "set_make", vec![out(&set), w(100)], 0);
    expect(manager.ptr(), "plant_valid", vec![w(0x11)], 1);
    expect(
        core::ptr::null_mut(),
        "set_insert",
        vec![input(&set), out(&[0; 3]), w(0x11)],
        0,
    );
    expect(manager.ptr(), "plant_valid", vec![w(0x22)], 0x100);
    expect(manager.ptr(), "plant_family", vec![input(&set), w(0x22)], 0);
    expect(
        core::ptr::null_mut(),
        "set_begin",
        vec![input(&set), out(&[node.as_mut_ptr() as u32, 0])],
        0,
    );
    expect(core::ptr::null_mut(), "set_destroy", vec![input(&set)], 0);
    expect(core::ptr::null_mut(), "set_free", vec![w(0x1234)], 0);
    expect(
        developer.ptr(),
        "plant_construct",
        vec![w(0x123), p(config.ptr()), w(0x3344), w(9)],
        RETURN,
    );
    assert_eq!(
        unsafe { construct(developer.ptr(), [0x123, config.ptr() as u32, 0, 9]) },
        (RETURN, 0x3344)
    );
    finished();
}

#[test]
fn default_construction_empty_result_and_reversed_bounds_still_destroy_the_set() {
    let _fakes = set_fakes();
    for reversed in [false, true] {
        let mut developer = mock!(0x18 => plant_construct);
        let mut manager = mock!(0x3C => plant_valid, 0x4C => plant_family);
        let mut config = mock!(0x60 => plant_choices);
        let mut choices = Object::new(0x30);
        let mut kinds = [0x11u32];
        let begin = kinds.as_mut_ptr() as u32;
        unsafe {
            LotDeveloper::from_ptr(developer.ptr()).set_building_development(manager.ptr());
            choices.ptr().add(0x28).cast::<u32>().write(begin);
            choices
                .ptr()
                .add(0x2C)
                .cast::<u32>()
                .write(if reversed { begin - 4 } else { begin + 4 });
        }
        expect(config.ptr(), "plant_choices", vec![], choices.ptr() as u64);
        expect(core::ptr::null_mut(), "set_make", vec![out(&[0; 5]), w(100)], 0);
        if !reversed {
            expect(manager.ptr(), "plant_valid", vec![w(0x11)], 0);
            expect(manager.ptr(), "plant_family", vec![input(&[0; 5]), w(0x11)], 0);
        }
        expect(core::ptr::null_mut(), "set_destroy", vec![input(&[0; 5])], 0);
        expect(
            developer.ptr(),
            "plant_construct",
            vec![w(0x123), p(config.ptr()), w(0), w(9)],
            RETURN,
        );
        assert_eq!(
            unsafe { construct(developer.ptr(), [0x123, config.ptr() as u32, 0, 9]) },
            (RETURN, 0)
        );
        finished();
    }
}

#[test]
fn flora_placement_skips_other_records_missing_types_and_failed_creations() {
    let _pick = fake(0x006C_D010, plant_pick as *const () as usize);
    let mut initial = 0xCAFE_1234u32;
    let _initial = fake_data(0x00B4_9AEC, (&mut initial as *mut u32).cast());
    let mut developer = mock!(0x9C => plant_position);
    let mut flora = mock!(0x44 => plant_add);
    let mut config = mock!(0x5C => plant_objects);
    let mut occupant = mock!(0x70 => plant_tag);
    let mut records = [0u32; 52];
    for index in 1..4 {
        records[index * 13] = 4;
        records[index * 13 + 9] = 0x8000 + index as u32;
    }
    let begin = records.as_mut_ptr() as u32;
    let mut objects = [begin, begin + 208, begin + 208];
    unsafe {
        crate::lot_developer::updates::UpdateState::from_ptr(developer.ptr()).set_flora(flora.ptr());
    }
    expect(config.ptr(), "plant_objects", vec![], objects.as_mut_ptr() as u64);
    for index in 1..4 {
        let record = begin + index * 52;
        let pos = [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()];
        expect(
            developer.ptr(),
            "plant_position",
            vec![w(0x123), p(config.ptr()), w(record), out(&pos), out(&[2])],
            0,
        );
        expect(
            developer.ptr(),
            "plant_pick",
            vec![w(record + 0x28), input(&pos)],
            if index == 1 { 0 } else { 0x44 },
        );
        if index != 1 {
            expect(
                flora.ptr(),
                "plant_add",
                vec![w(0x44), input(&pos), w(0)],
                if index == 2 { 0 } else { occupant.ptr() as u64 },
            );
        }
        if index == 3 {
            expect(occupant.ptr(), "plant_tag", vec![w(0x8003)], RETURN);
        }
    }
    unsafe {
        fc4p_006cd2b0(developer.ptr(), 0x123 as *mut u8, config.ptr(), 0xFF00_0101);
    }
    finished();
    for end in [0, 1, u32::MAX - 51] {
        objects[1] = begin.wrapping_add(end);
        expect(config.ptr(), "plant_objects", vec![], objects.as_mut_ptr() as u64);
        unsafe {
            fc4p_006cd2b0(developer.ptr(), 0x123 as *mut u8, config.ptr(), 0);
        }
        finished();
    }
    expect(config.ptr(), "plant_objects", vec![], 0);
    unsafe {
        fc4p_006cd2b0(developer.ptr(), 0x123 as *mut u8, config.ptr(), 0);
    }
    finished();
}

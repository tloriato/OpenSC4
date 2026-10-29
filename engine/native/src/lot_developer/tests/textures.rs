// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::textures::*;

slot!(texture_rect, output);
slot!(texture_objects);
slot!(texture_position, lot, config, object, position, facing);
slot!(texture_cells, cells, count, flags);
slot!(texture_inserted, flags);
slot!(texture_notify, message, flags);
slot!(texture_refresh, occupant, flags);
slot!(texture_insert, occupant, flags);

thread_local! {
    static CELL_STORAGE: RefCell<(usize, usize)> = const { RefCell::new((0, 0)) };
    static FREE_REPLACEMENT: RefCell<(usize, u32)> = const { RefCell::new((0, 0)) };
}

unsafe extern "thiscall" fn initialize_texture_cell(this: *mut u8) -> u64 {
    run(core::ptr::null_mut(), "initialize_texture_cell", &[]);
    let (eax, edx) = unsafe { call_thiscall_with_eax(fc4p_005ca770 as *const () as usize, this, &[], 0) };
    (u64::from(edx) << 32) | u64::from(eax)
}

unsafe extern "thiscall" fn append_texture_cell(this: *mut u32, cell: *mut u32) -> u64 {
    unsafe {
        FREE_REPLACEMENT.with(|replacement| replacement.borrow_mut().0 = this as usize);
        let returned = run(
            core::ptr::null_mut(),
            "append_texture_cell",
            &[
                cell.read_unaligned(),
                cell.add(1).read_unaligned(),
                cell.add(2).read_unaligned(),
                cell.add(3).read_unaligned() & 0xFFFF,
            ],
        );
        CELL_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            let pointer = (storage.0 as *mut u32).add(storage.1 * 4);
            core::ptr::copy_nonoverlapping(cell, pointer, 4);
            storage.1 += 1;
            this.write_unaligned(storage.0 as u32);
            this.add(1).write_unaligned(storage.0 as u32 + storage.1 as u32 * 16);
            this.add(2).write_unaligned(storage.0 as u32 + 256);
        });
        returned
    }
}

unsafe extern "thiscall" fn texture_release(this: *mut u8) -> u64 {
    let returned = run(this, "texture_release", &[]);
    FREE_REPLACEMENT.with(|replacement| {
        let (vector, pointer) = *replacement.borrow();
        if pointer != 0 {
            unsafe {
                (vector as *mut u32).write_unaligned(pointer);
            }
        }
    });
    returned
}

unsafe extern "thiscall" fn pick_texture_for_lot(this: *mut u8, lot: u32, candidates: u32) -> u64 {
    run(this, "pick_texture_for_lot", &[lot, candidates])
}

unsafe extern "C" fn free_texture_cells(pointer: u32) -> u64 {
    run(core::ptr::null_mut(), "free_texture_cells", &[pointer])
}

unsafe fn invoke(this: *mut u8, lot: *mut u8, config: *mut u8) -> [u32; 5] {
    let mut frame = [
        this as u32,
        fc4p_006ca8a0 as *const () as u32,
        lot as u32,
        config as u32,
        0xDEAD_BEEF,
        0xAABB_CCDD,
        0x9876_5432,
        0,
        0,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 16]", "push dword ptr [edi + 12]", "push dword ptr [edi + 8]",
            "mov word ptr [esp - 22], 0xAA55",
            "mov ecx, dword ptr [edi]", "mov eax, dword ptr [edi + 20]", "mov edx, dword ptr [edi + 24]",
            "call dword ptr [edi + 4]", "mov dword ptr [edi + 28], eax", "mov dword ptr [edi + 32], edx",
            "mov eax, dword ptr [esp - 12]", "mov dword ptr [edi + 36], eax",
            "mov eax, dword ptr [esp - 8]", "mov dword ptr [edi + 40], eax",
            "mov eax, dword ptr [esp - 4]", "mov dword ptr [edi + 44], eax",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"),
        );
    }
    frame[7..12].try_into().unwrap()
}

#[test]
fn normal_texture_null_inputs_preserve_entry_words_and_all_three_stack_arguments() {
    let mut developer = Object::new(0x108);
    let mut object = Object::new(4);
    for (lot, config) in [
        (core::ptr::null_mut(), object.ptr()),
        (object.ptr(), core::ptr::null_mut()),
    ] {
        let returned = unsafe { invoke(developer.ptr(), lot, config) };
        assert_eq!(
            returned,
            [0xAABB_CCDD, 0x9876_5432, lot as u32, config as u32, 0xDEAD_BEEF]
        );
        finished();
    }
}

#[test]
fn force_empty_texture_delegates_except_for_the_four_special_zones() {
    for zone in [0u32, 1, 10, 11, 12, 13, 15, 16, u32::MAX] {
        let mut developer = mock!(0xAC => set_empty_texture);
        let mut lot = mock!(0x7C => get_zone, 0x38 => texture_rect);
        let mut config = mock!(0x5C => texture_objects);
        unsafe {
            LotDeveloper::from_ptr(developer.ptr()).set_force_empty(0xFD);
        }
        expect(lot.ptr(), "get_zone", vec![], u64::from(zone));
        if matches!(zone, 0 | 11 | 12 | 15) {
            expect(lot.ptr(), "texture_rect", vec![out(&[0, 0, 0, 0])], RETURN);
            expect(config.ptr(), "texture_objects", vec![], RETURN & !0xFFFF_FFFF);
        } else {
            expect(developer.ptr(), "set_empty_texture", vec![p(lot.ptr())], RETURN);
        }
        let returned = unsafe { invoke(developer.ptr(), lot.ptr(), config.ptr()) };
        assert_eq!(
            returned,
            [
                if matches!(zone, 0 | 11 | 12 | 15) {
                    0
                } else {
                    RETURN as u32
                },
                (RETURN >> 32) as u32,
                lot.ptr() as u32,
                config.ptr() as u32,
                0xDEAD_BEEF
            ]
        );
        finished();
    }
}

#[test]
fn normal_texture_empty_and_negative_object_ranges_keep_signed_count_registers() {
    for difference in [0i32, -1, -52, i32::MIN] {
        let mut developer = Object::new(0x108);
        let mut lot = mock!(0x38 => texture_rect);
        let mut config = mock!(0x5C => texture_objects);
        let mut vector = [0x8000_0000u32, 0x8000_0000u32.wrapping_add(difference as u32), 0];
        expect(lot.ptr(), "texture_rect", vec![out(&[0, 0, 0, 0])], RETURN);
        expect(config.ptr(), "texture_objects", vec![], vector.as_mut_ptr() as u64);
        let returned = unsafe { invoke(developer.ptr(), lot.ptr(), config.ptr()) };
        let high = ((i64::from(difference) * 0x4EC4_EC4F) >> 32) as i32 >> 4;
        assert_eq!(
            returned,
            [
                high.wrapping_add((high as u32 >> 31) as i32) as u32,
                high as u32,
                lot.ptr() as u32,
                config.ptr() as u32,
                0xDEAD_BEEF
            ]
        );
        finished();
    }
}

#[test]
fn normal_texture_nontexture_records_leave_empty_cells_and_do_not_request_an_occupant() {
    let mut developer = Object::new(0x108);
    let mut lot = mock!(0x38 => texture_rect);
    let mut config = mock!(0x5C => texture_objects);
    let mut record = Object::new(52);
    unsafe {
        TextureObject::from_ptr(record.ptr()).set_kind(7);
    }
    let mut vector = [record.ptr() as u32, record.ptr() as u32 + 52, 0];
    expect(lot.ptr(), "texture_rect", vec![out(&[0, 0, 0, 0])], RETURN);
    expect(config.ptr(), "texture_objects", vec![], vector.as_mut_ptr() as u64);
    assert_eq!(
        unsafe { invoke(developer.ptr(), lot.ptr(), config.ptr()) },
        [0, 52, 52, config.ptr() as u32, 0xDEAD_BEEF]
    );
    finished();
}

#[test]
fn normal_texture_filters_records_and_candidates_wraps_cell_bytes_and_updates_or_inserts() {
    let _fpu = GameFpu::new();
    let _pick = fake(0x006C_60C0, pick_texture_for_lot as *const () as usize);
    let _initialize = fake(0x005C_A770, initialize_texture_cell as *const () as usize);
    let _append = fake(0x004B_2400, append_texture_cell as *const () as usize);
    let _free = fake(0x0090_CF63, free_texture_cells as *const () as usize);
    let mut initial = 0x1234_5678u32;
    let _initial = fake_data(0x00B4_9AEC, (&mut initial as *mut u32).cast());
    let mut scale = 0.0625f32;
    let _scale = fake_data(0x00AB_1B04, (&mut scale as *mut f32).cast());
    for inserted in [0x100u32, 0x101] {
        for query_ok in [false, true] {
            let mut developer = mock!(0x9C => texture_position, 0xA4 => get_texture);
            let mut lot = mock!(0x38 => texture_rect);
            let mut config = mock!(0x5C => texture_objects);
            let mut texture = mock!(0x20 => texture_cells, 0x0C => get_occupant, 8 => texture_release);
            let mut occupant = mock!(0x80 => texture_inserted, 0x3C => texture_notify);
            let mut manager = mock!(0x50 => texture_insert, 0x60 => texture_refresh);
            unsafe {
                TextureBounds::from_ptr(developer.ptr()).set_cells_x(5);
                TextureBounds::from_ptr(developer.ptr()).set_cells_z(5);
                LotDeveloper::from_ptr(developer.ptr()).set_occupant_manager(manager.ptr());
            }
            let mut records = Object::new(6 * 52);
            let mut candidates = [0u32, 0xDEAD, 0xCAFE];
            for i in 0..6 {
                let object = unsafe { TextureObject::from_ptr(records.ptr().add(i * 52)) };
                object.set_kind(if i == 0 { 1 } else { 2 });
                object.set_flip(0xA0 + i as u8);
                object.set_candidates(candidates.as_mut_ptr());
                object.set_candidates_end(if i == 2 {
                    candidates.as_mut_ptr()
                } else {
                    unsafe { candidates.as_mut_ptr().add(1) }
                });
            }
            // Family selection fails for the last object. The preceding objects use explicit zero.
            unsafe {
                TextureObject::from_ptr(records.ptr().add(4 * 52)).set_candidates(candidates.as_mut_ptr().add(2));
                TextureObject::from_ptr(records.ptr().add(4 * 52)).set_candidates_end(candidates.as_mut_ptr().add(3));
                TextureObject::from_ptr(records.ptr().add(5 * 52)).set_candidates(candidates.as_mut_ptr().add(1));
                TextureObject::from_ptr(records.ptr().add(5 * 52)).set_candidates_end(candidates.as_mut_ptr().add(2));
            }
            let mut vector = [records.ptr() as u32, records.ptr() as u32 + 6 * 52, 0];
            let mut storage = [0u32; 64];
            CELL_STORAGE.with(|s| *s.borrow_mut() = (storage.as_mut_ptr() as usize, 0));
            let freed = if inserted == 0x101 && query_ok {
                0xBEEF_0000
            } else {
                storage.as_mut_ptr() as u32
            };
            FREE_REPLACEMENT.with(|r| *r.borrow_mut() = (0, if freed == 0xBEEF_0000 { freed } else { 0 }));
            expect(lot.ptr(), "texture_rect", vec![out(&[0, 0, 0, 0])], RETURN);
            expect(config.ptr(), "texture_objects", vec![], vector.as_mut_ptr() as u64);
            for i in 1..6 {
                let position = match i {
                    1 => [16.0f32, 0.0, 32.0],
                    3 => [80.0, 0.0, 0.0],
                    4 => [-4080.0, 0.0, f32::NAN],
                    _ => [0.0, 0.0, 0.0],
                };
                expect(
                    developer.ptr(),
                    "texture_position",
                    vec![
                        p(lot.ptr()),
                        p(config.ptr()),
                        p(unsafe { records.ptr().add(i * 52) }),
                        out(&position.map(f32::to_bits)),
                        out(&[0x1234_5603]),
                    ],
                    RETURN,
                );
                if i == 5 {
                    expect(
                        developer.ptr(),
                        "pick_texture_for_lot",
                        vec![p(lot.ptr()), p(unsafe { records.ptr().add(i * 52 + 0x28) })],
                        RETURN & !0xFFFF_FFFF,
                    );
                } else if i != 2 {
                    if i == 4 {
                        expect(
                            developer.ptr(),
                            "pick_texture_for_lot",
                            vec![p(lot.ptr()), p(unsafe { records.ptr().add(i * 52 + 0x28) })],
                            0xBEEF,
                        );
                    }
                    expect(core::ptr::null_mut(), "initialize_texture_cell", vec![], RETURN);
                    if i != 3 {
                        expect(
                            core::ptr::null_mut(),
                            "append_texture_cell",
                            vec![
                                w(if i == 4 { 0xBEEF } else { 0 }),
                                w(if i == 1 { 0xA103_0201 } else { 0xA403_0001 }),
                                w(u32::MAX),
                                w(0xFFFF),
                            ],
                            RETURN,
                        );
                    }
                }
            }
            expect(
                developer.ptr(),
                "get_texture",
                vec![p(lot.ptr()), out(&[texture.ptr() as u32]), w(1)],
                if query_ok { 0x101 } else { 0x100 },
            );
            if query_ok {
                expect(
                    texture.ptr(),
                    "texture_cells",
                    vec![w(storage.as_mut_ptr() as u32), w(2), w(0)],
                    RETURN,
                );
                expect(texture.ptr(), "get_occupant", vec![], occupant.ptr() as u64);
                expect(occupant.ptr(), "texture_inserted", vec![w(1)], u64::from(inserted));
                expect(
                    manager.ptr(),
                    if inserted as u8 != 0 {
                        "texture_refresh"
                    } else {
                        "texture_insert"
                    },
                    vec![p(occupant.ptr()), w(0)],
                    RETURN,
                );
                if inserted as u8 != 0 {
                    expect(occupant.ptr(), "texture_notify", vec![w(0x99EF_1145), w(0)], RETURN);
                }
                expect(texture.ptr(), "texture_release", vec![], RETURN);
            }
            expect(core::ptr::null_mut(), "free_texture_cells", vec![w(freed)], RETURN);
            let returned = unsafe { invoke(developer.ptr(), lot.ptr(), config.ptr()) };
            assert_eq!(
                returned,
                [
                    RETURN as u32,
                    (RETURN >> 32) as u32,
                    6 * 52,
                    texture.ptr() as u32,
                    0xDEAD_BEEF
                ]
            );
            assert_eq!(CELL_STORAGE.with(|s| s.borrow().1), 2);
            assert_eq!(storage[1], 0xA103_0201);
            assert_eq!(storage[5], 0xA403_0001);
            assert_eq!((storage[3], storage[7]), (0xAA55_FFFF, 0xAA55_FFFF));
            finished();
        }
    }
}

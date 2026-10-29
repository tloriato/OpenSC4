// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::power_changes::*;

slot!(ignore_power);
slot!(cell_power, x, z);
slot!(lot_power);
slot!(set_lot_power, powered);
slot!(condition);
slot!(needs_power);
slot!(power_bounds, min, max);
float_slot!(cell_width_float, cell_width_body, 0);
float_slot!(cell_depth_float, cell_depth_body, 0);

unsafe extern "thiscall" fn add_power_zot(this: *mut u8, lot: u32, kind: u32, number: u32) -> u64 {
    run(this, "add_power_zot", &[lot, kind, number])
}
unsafe extern "thiscall" fn remove_power_zot(this: *mut u8, lot: u32, number: u32) -> u64 {
    run(this, "remove_power_zot", &[lot, number])
}
#[unsafe(naked)]
unsafe extern "C" fn suitable() {
    core::arch::naked_asm!("push eax", "call {}", "ret", sym suitable_body);
}
unsafe extern "fastcall" fn suitable_body(lot: *mut u8, x: u32, z: u32) -> u64 {
    run(lot, "suitable", &[x, z])
}

unsafe fn changed(this: *mut u8, rect: *const u32) -> [u32; 3] {
    let mut frame = [rect as u32, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi]", "call {function}", "mov [edi + 4], eax", "mov [edi + 8], edx",
            "mov eax, [esp - 4]", "mov [edi + 12], eax",
            function = sym fc4p_006cbf40, in("edi") frame.as_mut_ptr(), in("ecx") this, clobber_abi("C"),
        );
    }
    frame[1..].try_into().unwrap()
}

#[test]
fn power_query_uses_full_result_and_always_releases_and_frees_queried_nodes() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    unsafe {
        for populated in [false, true] {
            let mut developer = Object::new(0x108);
            let mut manager = mock!(0x1C => area_lots);
            let mut lot = mock!(8 => release);
            let (this, manager, lot) = (developer.ptr(), manager.ptr(), lot.ptr());
            LotDeveloper::from_ptr(this).set_lot_manager(manager);
            let mut head = [0xA5A5_A5A5u32; 3];
            let sentinel = head.as_mut_ptr() as u32;
            let mut last = [sentinel, 0, 0u32];
            let mut node = [last.as_mut_ptr() as u32, 0, lot as u32];
            let rect = [u32::MAX, 3, 4, 5];
            expect(
                core::ptr::null_mut(),
                "operation_allocate",
                vec![w(12)],
                u64::from(sentinel),
            );
            expect(
                manager,
                "area_lots",
                vec![
                    Arg::In(vec![sentinel]),
                    w(rect[0]),
                    w(rect[1]),
                    w(rect[2]),
                    w(rect[3]),
                    w(1),
                ],
                0,
            );
            if populated {
                edit_last(head.as_mut_ptr(), node.as_mut_ptr() as u32);
                expect(lot, "release", vec![], RETURN);
                expect(
                    core::ptr::null_mut(),
                    "operation_free",
                    vec![w(node.as_mut_ptr() as u32)],
                    RETURN,
                );
                expect(
                    core::ptr::null_mut(),
                    "operation_free",
                    vec![w(last.as_mut_ptr() as u32)],
                    RETURN,
                );
            }
            expect(core::ptr::null_mut(), "operation_free", vec![w(sentinel)], RETURN);
            assert_eq!(
                changed(this, rect.as_ptr()),
                [RETURN as u32, (RETURN >> 32) as u32, rect.as_ptr() as u32]
            );
            assert_eq!(head, [sentinel, sentinel, 0xA5A5_A5A5]);
            finished();
        }
    }
}

#[derive(Clone, Copy)]
struct Center {
    min: [f32; 3],
    max: [f32; 3],
    widths: [f32; 2],
    cell: [u32; 2],
}

#[derive(Clone, Copy)]
struct Case {
    building: bool,
    ignore: u32,
    power: u32,
    old: u32,
    condition: u32,
    needs: u32,
    zot: Option<u32>,
    suitable: u32,
    center: Center,
}

#[test]
fn power_changes_cover_conditions_cheats_empty_lots_and_zot_priority() {
    let _fpu = GameFpu::new();
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _add = fake(0x006C_98C0, add_power_zot as *const () as usize);
    let _remove = fake(0x006C_9780, remove_power_zot as *const () as usize);
    let _suitable = fake(0x006C_3B10, suitable as *const () as usize);
    // Includes the core's loss/restoration vectors, then noncanonical Boolean bytes and condition words.
    let base = Case {
        building: true,
        ignore: 0,
        power: 0,
        old: 1,
        condition: 0,
        needs: 1,
        zot: Some(4),
        suitable: 1,
        center: Center {
            min: [3.0, 4.0, 5.0],
            max: [29.0, 10.0, 59.0],
            widths: [16.0; 2],
            cell: [1, 2],
        },
    };
    for case in [
        base,
        Case {
            power: 0x101,
            old: 0,
            condition: 1,
            ..base
        },
        Case {
            ignore: 0x101,
            old: 0,
            condition: 1,
            ..base
        },
        Case {
            power: 0x100,
            old: 0,
            condition: 0,
            ..base
        },
        Case {
            power: 1,
            old: 0x80,
            condition: 0x101,
            zot: Some(6),
            ..base
        },
        Case {
            power: 1,
            old: 1,
            zot: None,
            ..base
        },
        Case {
            needs: 0x100,
            zot: Some(4),
            ..base
        },
        Case {
            building: false,
            ..base
        },
        Case {
            building: false,
            suitable: 0x100,
            ..base
        },
        Case {
            building: false,
            power: 1,
            old: 1,
            zot: Some(4),
            ..base
        },
        Case {
            building: false,
            power: 1,
            old: 1,
            zot: Some(6),
            ..base
        },
        Case {
            center: Center {
                min: [-17.0, 0.0, -15.0],
                max: [-17.0, 0.0, -15.0],
                widths: [16.0; 2],
                cell: [u32::MAX, 0],
            },
            ..base
        },
        Case {
            center: Center {
                min: [3e9, 0.0, 1e19],
                max: [3e9, 0.0, 1e19],
                widths: [1.0; 2],
                cell: [3_000_000_000, 0],
            },
            ..base
        },
        Case {
            center: Center {
                min: [f32::NAN, 0.0, 0.0],
                max: [0.0; 3],
                widths: [1.0, 0.0],
                cell: [0; 2],
            },
            ..base
        },
    ] {
        unsafe {
            let mut developer = Object::new(0x108);
            let mut manager = mock!(0x1C => area_lots);
            let mut bds = mock!(0xDC => ignore_power);
            let mut power = mock!(0x94 => cell_power);
            let mut city = mock!(0x16C => cell_width_float, 0x170 => cell_depth_float);
            let mut lot = mock!(0xC0 => building_occupant, 0x38 => bounding_rect, 0x54 => get_location,
                0x108 => lot_power, 0x10C => set_lot_power, 0x8C => condition, 0x90 => set_condition,
                0x104 => needs_power, 0x0C => get_holder, 8 => release);
            let mut building = mock!(0x0C => get_occupant);
            let mut occupant = mock!(0x28 => power_bounds);
            let mut holder = mock!(0x24 => get_property);
            let mut property = mock!(0x14 => get_variant);
            let mut variant = mock!(0x124 => get_data, 0x68 => set_uint);
            let (this, manager, bds, power, city, lot, building, occupant, holder, property, variant) = (
                developer.ptr(),
                manager.ptr(),
                bds.ptr(),
                power.ptr(),
                city.ptr(),
                lot.ptr(),
                building.ptr(),
                occupant.ptr(),
                holder.ptr(),
                property.ptr(),
                variant.ptr(),
            );
            let dev = LotDeveloper::from_ptr(this);
            dev.set_lot_manager(manager);
            dev.set_building_development(bds);
            dev.set_city(city);
            this.add(0x34).cast::<*mut u8>().write(power);
            let mut head = [0xA5A5_A5A5u32; 3];
            let sentinel = head.as_mut_ptr() as u32;
            let mut node = [sentinel, sentinel, lot as u32];
            let rect = [0u32, 0, 0, 0];
            let powered = case.ignore as u8 != 0 || case.power as u8 != 0;
            expect(
                core::ptr::null_mut(),
                "operation_allocate",
                vec![w(12)],
                u64::from(sentinel),
            );
            // A nonzero full word with AL zero still enters the lot traversal.
            expect(
                manager,
                "area_lots",
                vec![Arg::In(vec![sentinel]), w(0), w(0), w(0), w(0), w(1)],
                0x100,
            );
            edit_last(head.as_mut_ptr(), node.as_mut_ptr() as u32);
            expect(
                lot,
                "building_occupant",
                vec![],
                if case.building { building as u64 } else { 0 },
            );
            expect(lot, "bounding_rect", vec![out(&[4, 5, 6, 7])], RETURN);
            let (x, z) = if case.building {
                expect(building, "get_occupant", vec![], occupant as u64);
                expect(
                    occupant,
                    "power_bounds",
                    vec![
                        out(&case.center.min.map(f32::to_bits)),
                        out(&case.center.max.map(f32::to_bits)),
                    ],
                    RETURN,
                );
                expect_float(
                    city,
                    "cell_width_body",
                    vec![],
                    RETURN,
                    F80::from_f32(case.center.widths[0]),
                );
                expect_float(
                    city,
                    "cell_depth_body",
                    vec![],
                    RETURN,
                    F80::from_f32(case.center.widths[1]),
                );
                (case.center.cell[0], case.center.cell[1])
            } else {
                expect(lot, "get_location", vec![out(&[u32::MAX]), out(&[17])], RETURN);
                (u32::MAX, 17)
            };
            expect(bds, "ignore_power", vec![], u64::from(case.ignore));
            if case.ignore as u8 == 0 {
                expect(power, "cell_power", vec![w(x), w(z)], u64::from(case.power));
            }
            expect(lot, "lot_power", vec![], u64::from(case.old));
            let flag = (rect.as_ptr() as u32 & !255) | u32::from(powered);
            if case.old as u8 != u8::from(powered) {
                expect(lot, "set_lot_power", vec![w(flag)], RETURN);
                expect(lot, "condition", vec![], u64::from(case.condition));
                if (!powered && case.condition == 0) || (powered && case.condition == 1) {
                    expect(lot, "set_condition", vec![w(u32::from(!powered))], RETURN);
                }
            }
            if case.building {
                expect(lot, "needs_power", vec![], u64::from(case.needs));
            }
            let mut zot = case.zot.unwrap_or(0);
            if !powered && (!case.building || case.needs as u8 != 0) {
                if !case.building {
                    expect(lot, "suitable", vec![w(x), w(z)], u64::from(case.suitable));
                }
                if case.building || case.suitable as u8 != 0 {
                    expect(this, "add_power_zot", vec![p(lot), w(0x0FD1_0000), w(4)], RETURN);
                }
            } else {
                expect(lot, "get_holder", vec![], holder as u64);
                expect(
                    holder,
                    "get_property",
                    vec![w(0xC999_C45E)],
                    if case.zot.is_some() { property as u64 } else { 0 },
                );
                if case.zot.is_some() {
                    expect(property, "get_variant", vec![], variant as u64);
                    expect(variant, "get_data", vec![], &mut zot as *mut u32 as u64);
                    if zot == 4 {
                        expect(this, "remove_power_zot", vec![p(lot), w(4)], RETURN);
                        expect(variant, "set_uint", vec![w(0)], RETURN);
                    }
                }
            }
            expect(lot, "release", vec![], RETURN);
            expect(
                core::ptr::null_mut(),
                "operation_free",
                vec![w(node.as_mut_ptr() as u32)],
                RETURN,
            );
            expect(core::ptr::null_mut(), "operation_free", vec![w(sentinel)], RETURN);
            assert_eq!(
                changed(this, rect.as_ptr()),
                [RETURN as u32, (RETURN >> 32) as u32, flag]
            );
            assert_eq!(head, [sentinel, sentinel, 0xA5A5_A5A5]);
            finished();
        }
    }
}

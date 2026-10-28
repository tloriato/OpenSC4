// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

unsafe fn invoke(this: *mut u8, rect: *const u32, purpose: i32) -> (u32, u32, u32) {
    let mut ecx = this as u32;
    let eax: u32;
    let mut edx = 0xDEAD_BEEFu32;
    unsafe {
        core::arch::asm!("push {purpose}", "push {rect}", "call {function}",
            purpose = in(reg) purpose, rect = in(reg) rect, function = sym fc4p_006c7a70,
            inlateout("ecx") ecx, lateout("eax") eax, inlateout("edx") edx, clobber_abi("C"));
    }
    (eax, edx, ecx)
}

#[test]
fn anchor_rejects_other_purposes_without_reading_area_and_keeps_ecx_and_edx() {
    for purpose in [i32::MIN, -1, 0, 1, 5, 9, 0x1234, i32::MAX] {
        let this = 0x1234_5678usize as *mut u8;
        assert_eq!(
            unsafe { invoke(this, core::ptr::null(), purpose) },
            (purpose as u32 & !255, 0xDEAD_BEEF, this as u32)
        );
    }
    finished();
}

#[test]
fn flagged_unit_in_area_requires_anchor_and_clears_the_temporary_list() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _clear = fake(0x0047_3EC0, clear_list as *const () as usize);
    unsafe {
        for purpose in 6..=8 {
            let mut dev = Object::new(0x108);
            let this = dev.ptr();
            let mut manager = mock!(0x1C => area_lots);
            let manager = manager.ptr();
            LotDeveloper::from_ptr(this).set_lot_manager(manager);
            let mut members: Vec<_> = (0..3).map(|_| mock!(0xC0 => business_unit)).collect();
            let members: Vec<_> = members.iter_mut().map(Mock::ptr).collect();
            let mut unit = mock!(0x0C => unit_flags);
            let unit = unit.ptr();
            let mut flags = mock!(0x58 => test_unit_flags);
            let flags = flags.ptr();
            let rect = [u32::MAX, 2, 3, 4];
            let mut sentinel = [0xA5A5_A5A5u32; 3];
            let head = sentinel.as_mut_ptr().cast::<u8>();
            let mut nodes: Vec<_> = members.iter().map(|&p| [0u32, 0, p as u32]).collect();
            let links: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for (i, node) in nodes.iter_mut().enumerate() {
                node[0] = links.get(i + 1).copied().unwrap_or(head as u32);
            }
            expect(core::ptr::null_mut(), "operation_allocate", vec![w(12)], head as u64);
            expect(
                manager,
                "area_lots",
                vec![
                    Arg::In(vec![head as u32]),
                    w(rect[0]),
                    w(rect[1]),
                    w(rect[2]),
                    w(rect[3]),
                    w(0),
                ],
                0,
            );
            edit_last(sentinel.as_mut_ptr(), links[0]);
            expect(members[0], "business_unit", vec![], 0);
            expect(members[1], "business_unit", vec![], unit as u64);
            expect(unit, "unit_flags", vec![], flags as u64);
            expect(flags, "test_unit_flags", vec![w(0x3000)], 0xABCD_0100);
            expect(members[2], "business_unit", vec![], unit as u64);
            expect(unit, "unit_flags", vec![], flags as u64);
            expect(flags, "test_unit_flags", vec![w(0x3000)], 0xABCD_0001);
            expect(core::ptr::null_mut(), "clear_list", vec![p(head)], RETURN);
            edit_last(sentinel.as_mut_ptr(), head as u32);
            edit_last(sentinel.as_mut_ptr().add(1), head as u32);
            expect(core::ptr::null_mut(), "operation_free", vec![p(head)], RETURN);
            let (eax, edx, ecx) = invoke(this, rect.as_ptr(), purpose);
            assert_eq!(eax, (RETURN as u32 & !255) | 1);
            assert_eq!(edx, (RETURN >> 32) as u32);
            assert_eq!(ecx, this as u32);
            assert_eq!(sentinel[2], 0xA5A5_A5A5);
            finished();
        }
    }
}

#[test]
fn anchor_search_visits_four_sides_then_checks_building_eligibility_and_purpose() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _clear = fake(0x0047_3EC0, clear_list as *const () as usize);
    unsafe {
        for matching in [false, true] {
            let mut dev = Object::new(0x108);
            let this = dev.ptr();
            let mut manager = mock!(0x1C => area_lots, 0x28 => neighbor_lots);
            let manager = manager.ptr();
            let mut replacement = mock!(0x1C => area_lots, 0x28 => neighbor_lots);
            let replacement = replacement.ptr();
            let mut bds = mock!(0x9C => get_profile);
            let bds = bds.ptr();
            let developer = LotDeveloper::from_ptr(this);
            developer.set_lot_manager(manager);
            developer.set_building_development(bds);
            let mut members: Vec<_> = (0..5).map(|_| mock!(0xB8 => get_building_type, 0x84 => habitation_state, 0x64 => building_occupant, 8 => release)).collect();
            let members: Vec<_> = members.iter_mut().map(Mock::ptr).collect();
            let rect = [0u32, 2, 3, 4];
            let mut sentinel = [0xA5A5_A5A5u32; 3];
            let head = sentinel.as_mut_ptr().cast::<u8>();
            let mut nodes: Vec<_> = members.iter().map(|&p| [0u32, 0, p as u32]).collect();
            let links: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for (i, node) in nodes.iter_mut().enumerate() {
                node[0] = links.get(i + 1).copied().unwrap_or(head as u32);
            }
            expect(core::ptr::null_mut(), "operation_allocate", vec![w(12)], head as u64);
            expect(
                manager,
                "area_lots",
                vec![Arg::In(vec![head as u32]), w(0), w(2), w(3), w(4), w(0)],
                1,
            );
            expect(core::ptr::null_mut(), "clear_list", vec![p(head)], RETURN);
            for flag in [1, 2, 4, 8] {
                expect(
                    if flag == 1 { manager } else { replacement },
                    "neighbor_lots",
                    vec![p(rect.as_ptr() as *mut u8), Arg::In(vec![head as u32]), w(flag)],
                    0,
                );
                edit_last(this.add(0x24).cast(), replacement as u32);
                if flag == 8 {
                    edit_last(sentinel.as_mut_ptr(), links[0]);
                }
            }
            expect(members[0], "get_building_type", vec![w(1)], 0);
            expect(members[1], "get_building_type", vec![w(1)], 11);
            expect(members[1], "habitation_state", vec![], 1);
            expect(members[1], "building_occupant", vec![], 0);
            expect(members[2], "get_building_type", vec![w(1)], 12);
            expect(members[2], "habitation_state", vec![], 0x8000_0000);
            expect(members[2], "building_occupant", vec![], 1);
            let mut wrong = [7i32];
            expect(bds, "get_profile", vec![w(12)], wrong.as_mut_ptr() as u64);
            expect(members[3], "get_building_type", vec![w(1)], 13);
            expect(members[3], "habitation_state", vec![], 2);
            let mut other = [if matching { 6i32 } else { 8 }];
            expect(bds, "get_profile", vec![w(13)], other.as_mut_ptr() as u64);
            if matching {
                expect(core::ptr::null_mut(), "clear_list", vec![p(head)], RETURN);
                edit_last(sentinel.as_mut_ptr(), head as u32);
                edit_last(sentinel.as_mut_ptr().add(1), head as u32);
            } else {
                expect(members[4], "get_building_type", vec![w(1)], 0);
                for (&member, &node) in members.iter().zip(&links) {
                    expect(member, "release", vec![], RETURN);
                    expect(core::ptr::null_mut(), "operation_free", vec![w(node)], RETURN);
                }
            }
            expect(core::ptr::null_mut(), "operation_free", vec![p(head)], RETURN);
            let (eax, edx, ecx) = invoke(this, rect.as_ptr(), 6);
            assert_eq!(eax, (RETURN as u32 & !255) | u32::from(!matching));
            assert_eq!(edx, (RETURN >> 32) as u32);
            assert_eq!(ecx, this as u32);
            assert_eq!(sentinel[0], head as u32);
            assert_eq!(sentinel[1], head as u32);
            assert_eq!(sentinel[2], 0xA5A5_A5A5);
            finished();
        }
    }
}

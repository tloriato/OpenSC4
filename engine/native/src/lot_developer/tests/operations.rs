// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

#[test]
fn texture_selection_draws_once_and_tries_variants_in_order() {
    unsafe {
        for condition in [0u32, 1, 2, 3, 4, 5, 6, 7, u32::MAX] {
            for success in 0..=3 {
                let mut dev = Object::new(0x108);
                let this = dev.ptr();
                let mut lot = mock!(0xA4 => lot_wealth, 0x8C => get_type);
                let lot = lot.ptr();
                let mut resources = mock!(0x38 => resource_exists);
                let mut resource = resources.ptr();
                let _resources = fake_data(0x00B4_3CA8, (&mut resource as *mut *mut u8).cast());
                let mut random = [0u32; 2];
                let random_ptr = random.as_mut_ptr().cast();
                let _random = fake_data(0x00B4_9AF0, random_ptr);
                let _draw = fake(0x0091_36B1, draw_uniform as *const () as usize);
                let mut original = [0xBAD0_0000u32; 3];
                let mut ids = [0xAABB_CCDD, 0x89AB_CDEF, 0x1234_5678];
                let mut vector = [original.as_mut_ptr(), original.as_mut_ptr().add(3), core::ptr::null_mut()];
                expect(random_ptr, "draw_uniform", vec![w(3)], 1);
                edit_last(vector.as_mut_ptr().cast(), ids.as_mut_ptr() as u32);
                expect(lot, "lot_wealth", vec![], 2);
                expect(lot, "get_type", vec![], u64::from(condition));
                let appearance = match condition { 2 | 3 => 1, 4..=6 => 2, _ => 0 };
                let variants = [0x89AB_2000 | appearance << 8, 0x89AB_2000, 0x89AB_0000 | appearance << 8];
                for (i, &candidate) in variants.iter().enumerate().take((success + 1).min(3)) {
                    expect(resource, "resource_exists", vec![Arg::In(vec![0x7AB5_0E44, 0x0986_135E, candidate])],
                        if i == success { 0xABCD_0001 } else { 0xABCD_0100 });
                }
                let actual = fc4p_006c60c0(this, lot, vector.as_mut_ptr().cast());
                assert_eq!(actual, if success < 3 { variants[success] } else { ids[1] });
                assert_eq!(ids, [0xAABB_CCDD, 0x89AB_CDEF, 0x1234_5678]);
                finished();
            }
        }
    }
}

#[test]
fn texture_selection_handles_empty_ids_and_wealth_fallbacks() {
    unsafe {
        let mut dev = Object::new(0x108);
        let this = dev.ptr();
        let mut vector = [core::ptr::null_mut::<u32>(); 3];
        assert_eq!(fc4p_006c60c0(this, core::ptr::null_mut(), vector.as_mut_ptr().cast()), 0);
        finished();
        for has_building in [false, true] {
            let mut lot = mock!(0xA4 => lot_wealth, 0x8C => get_type, 0xB8 => get_building_type);
            let lot = lot.ptr();
            let mut resources = mock!(0x38 => resource_exists);
            let mut resource = resources.ptr();
            let _resources = fake_data(0x00B4_3CA8, (&mut resource as *mut *mut u8).cast());
            let mut random = [0u32; 2];
            let random_ptr = random.as_mut_ptr().cast();
            let _random = fake_data(0x00B4_9AF0, random_ptr);
            let _draw = fake(0x0091_36B1, draw_uniform as *const () as usize);
            let mut building_dev = mock!(0x9C => get_profile);
            let bds = building_dev.ptr();
            let mut replacement = mock!(0x9C => get_profile);
            let replacement = replacement.ptr();
            LotDeveloper::from_ptr(this).set_building_development(bds);
            let mut profile = [0u32, 0x1234_5678];
            let mut ids = [0xDBCA_1234u32];
            vector[0] = ids.as_mut_ptr();
            vector[1] = ids.as_mut_ptr().add(1);
            expect(random_ptr, "draw_uniform", vec![w(1)], 0);
            expect(lot, "lot_wealth", vec![], 0);
            expect(lot, "get_type", vec![], 5);
            expect(lot, "get_building_type", vec![w(0)], if has_building { 7 } else { 0 });
            edit_last(this.add(0x18).cast(), replacement as u32);
            if has_building {
                expect(replacement, "get_profile", vec![w(7)], profile.as_mut_ptr() as u64);
            }
            let wealth = if has_building { profile[1] } else { 1 };
            let expected = 0xDBCA_0000 | ((wealth << 4 | 2) << 8);
            expect(resource, "resource_exists", vec![Arg::In(vec![0x7AB5_0E44, 0x0986_135E, expected])], 1);
            assert_eq!(fc4p_006c60c0(this, lot, vector.as_mut_ptr().cast()), expected);
            finished();
        }
    }
}

#[test]
fn surrounding_foundations_filter_flags_release_every_member_and_preserve_ecx() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    unsafe {
        for queried in [0, 0x100, 1] {
            let mut dev = mock!(0xF4 => refresh_foundation);
            let this = dev.ptr();
            let mut manager = mock!(0x24 => surrounding_lots);
            let manager = manager.ptr();
            LotDeveloper::from_ptr(this).set_lot_manager(manager);
            let mut lot = Object::new(0x120);
            let lot = lot.ptr();
            let mut sentinel = [0xA5A5_A5A5u32; 3];
            let head = sentinel.as_mut_ptr().cast::<u8>();
            let mut members: Vec<_> = (0..6).map(|_| mock!(0x30 => get_flags, 8 => release)).collect();
            let pointers: Vec<_> = members.iter_mut().map(Mock::ptr).collect();
            let mut nodes: Vec<_> = pointers.iter().map(|&p| [0u32, 0, p as u32]).collect();
            let links: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for i in 0..nodes.len() {
                nodes[i][0] = links.get(i + 1).copied().unwrap_or(head as u32);
                nodes[i][1] = if i == 0 { head as u32 } else { links[i - 1] };
            }
            expect(core::ptr::null_mut(), "operation_allocate", vec![w(12)], head as u64);
            expect(manager, "surrounding_lots", vec![p(lot), Arg::In(vec![head as u32]), w(15)], queried);
            edit_last(sentinel.as_mut_ptr(), links[0]);
            edit_last(sentinel.as_mut_ptr().add(1), *links.last().unwrap());
            if queried != 0 {
                for (&member, flag) in pointers.iter().zip([0, 1, 2, 3, 0x100, 0x102]) {
                    expect(member, "get_flags", vec![], flag);
                    if flag & 3 != 0 { expect(this, "refresh_foundation", vec![p(member)], RETURN); }
                }
            }
            for (&member, &node) in pointers.iter().zip(&links) {
                expect(member, "release", vec![], RETURN);
                expect(core::ptr::null_mut(), "operation_free", vec![w(node)], RETURN);
            }
            expect(core::ptr::null_mut(), "operation_free", vec![p(head)], RETURN);
            let mut ecx = this as u32;
            let eax: u32;
            let edx: u32;
            core::arch::asm!("push {lot}", "call {function}", lot = in(reg) lot,
                function = sym fc4p_006c7720, inlateout("ecx") ecx, lateout("eax") eax,
                lateout("edx") edx, clobber_abi("C"));
            assert_eq!(ecx, this as u32);
            assert_eq!(u64::from(edx) << 32 | u64::from(eax), RETURN);
            assert_eq!(sentinel, [head as u32, head as u32, 0xA5A5_A5A5]);
            finished();
        }
    }
}

#[test]
fn surrounding_foundations_empty_list_keeps_payload_and_still_frees_head() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    unsafe {
        let mut dev = Object::new(0x108);
        let this = dev.ptr();
        let mut manager = mock!(0x24 => surrounding_lots);
        let manager = manager.ptr();
        LotDeveloper::from_ptr(this).set_lot_manager(manager);
        let mut sentinel = [0xA5A5_A5A5u32; 3];
        let head = sentinel.as_mut_ptr().cast::<u8>();
        expect(core::ptr::null_mut(), "operation_allocate", vec![w(12)], head as u64);
        expect(manager, "surrounding_lots", vec![w(77), Arg::In(vec![head as u32]), w(15)], 1);
        expect(core::ptr::null_mut(), "operation_free", vec![p(head)], RETURN);
        assert_eq!(call_thiscall_with_eax(fc4p_006c7720 as *const () as usize, this, &[77], 0).0, RETURN as u32);
        assert_eq!(sentinel, [head as u32, head as u32, 0xA5A5_A5A5]);
        finished();
    }
}

#[test]
fn clear_lot_retains_location_across_callbacks_and_resets_in_order() {
    let _contents = fake(0x005E_3070, clear_contents as *const () as usize);
    let _surrounding = fake(0x006C_7720, surrounding_direct as *const () as usize);
    let _traffic = fake(0x005E_3960, clear_traffic as *const () as usize);
    unsafe {
        let mut dev = mock!(0xD4 => remove_foundation_cell, 0xAC => set_empty_texture);
        let this = dev.ptr();
        let mut replacement = mock!(0xD4 => remove_foundation_cell, 0xAC => set_empty_texture);
        let replacement = replacement.ptr();
        let mut lot = mock!(0x54 => get_location, 0x114 => set_business_unit, 0x88 => set_state,
            0xBC => set_growth_type, 0x28 => set_configuration);
        let lot = lot.ptr();
        expect(lot, "get_location", vec![out(&[u32::MAX]), out(&[0x8000_0000])], RETURN);
        expect(core::ptr::null_mut(), "clear_contents", vec![p(lot), w(0)], RETURN);
        edit_last(this.cast(), replacement.cast::<u32>().read());
        expect(this, "remove_foundation_cell", vec![w(u32::MAX), w(0x8000_0000)], RETURN);
        expect(this, "surrounding_direct", vec![p(lot)], RETURN);
        expect(core::ptr::null_mut(), "clear_traffic", vec![p(lot)], RETURN);
        expect(lot, "set_business_unit", vec![w(0)], RETURN);
        expect(lot, "set_state", vec![w(1), w(0), w(0)], RETURN);
        expect(lot, "set_growth_type", vec![w(0)], RETURN);
        expect(lot, "set_configuration", vec![w(0)], RETURN);
        expect(this, "set_empty_texture", vec![p(lot)], RETURN);
        assert_eq!(fc4p_006c8a20(this, lot), RETURN);
        finished();
    }
}

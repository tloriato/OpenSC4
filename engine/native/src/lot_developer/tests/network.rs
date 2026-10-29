// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::maintenance::OccupantNode;
use crate::lot_developer::network::*;

slot!(connection_rect, output);
slot!(connection_allocate, bytes);
slot!(connection_query, list, min, max, kind, limit);
slot!(connection_iid, iid, output);
slot!(connection_flags, mask);
slot!(connection_remove, list);
slot!(connection_release_list, list);
slot!(connection_service, class, iid, output);

unsafe extern "thiscall" fn connection_free(this: *mut u8, pointer: *mut u8) -> u64 {
    unsafe {
        let node = OccupantNode::from_ptr(pointer);
        assert_eq!(OccupantNode::from_ptr(node.previous()).next(), node.next());
        assert_eq!(OccupantNode::from_ptr(node.next()).previous(), node.previous());
    }
    run(this, "connection_free", &[pointer as u32])
}

unsafe extern "C" fn get_connection_service() -> u64 {
    run(core::ptr::null_mut(), "get_connection_service", &[])
}

unsafe extern "C" fn connection_framework() -> u64 {
    run(core::ptr::null_mut(), "connection_framework", &[])
}

unsafe extern "thiscall" fn connection_service_release(this: *mut *mut u8) -> u64 {
    run(
        core::ptr::null_mut(),
        "connection_service_release",
        &[unsafe { this.read_unaligned() } as u32],
    )
}

unsafe extern "C" fn connection_destroy_body(list: *mut *mut u8) -> u64 {
    run(
        core::ptr::null_mut(),
        "connection_destroy",
        &[unsafe { list.read_unaligned() } as u32],
    )
}

#[unsafe(naked)]
unsafe extern "C" fn connection_destroy() {
    core::arch::naked_asm!("push ecx", "call {}", "add esp, 4", "mov ecx, 0xCAFE1234", "ret", sym connection_destroy_body);
}

unsafe fn invoke(this: *mut u8, lot: *mut u8) -> [u32; 4] {
    let mut frame = [this as u32, fc4p_006c9fb0 as *const () as u32, lot as u32, 0, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 8]", "mov ecx, dword ptr [edi]", "call dword ptr [edi + 4]",
            "mov dword ptr [edi + 12], eax", "mov dword ptr [edi + 16], edx", "mov dword ptr [edi + 20], ecx",
            "mov eax, dword ptr [esp - 4]", "mov dword ptr [edi + 24], eax",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"),
        );
    }
    frame[3..7].try_into().unwrap()
}

#[test]
fn network_removal_always_destroys_list_and_empty_success_calls_both_services() {
    let _framework = fake(0x0087_93EC, connection_framework as *const () as usize);
    let _release = fake(0x0041_1870, connection_service_release as *const () as usize);
    let _service = fake(0x0060_2680, get_connection_service as *const () as usize);
    let _destroy = fake(0x0041_3FE0, connection_destroy as *const () as usize);
    for success in [false, true] {
        let mut developer = Object::new(0x108);
        let mut lot = mock!(0x38 => connection_rect);
        let mut manager = mock!(0xA4 => connection_query, 0xC0 => connection_release_list);
        let mut root = mock!(0x14 => connection_service);
        let mut first_allocator = mock!(0x0C => connection_allocate);
        let mut allocator = mock!(0x0C => connection_allocate);
        let mut service = mock!(0x18 => connection_remove);
        let mut head = Object::new(12);
        let mut allocator_pointer = if success {
            core::ptr::null_mut()
        } else {
            allocator.ptr()
        };
        let _allocator = fake_data(0x00B2_07E4, (&mut allocator_pointer as *mut *mut u8).cast());
        unsafe {
            LotDeveloper::from_ptr(developer.ptr()).set_occupant_manager(manager.ptr());
        }
        expect(
            lot.ptr(),
            "connection_rect",
            vec![out(&[0xFFFF_FFFE, 4, 8, 12])],
            RETURN,
        );
        if success {
            expect(core::ptr::null_mut(), "connection_framework", vec![], root.ptr() as u64);
            expect(
                root.ptr(),
                "connection_service",
                vec![w(0x3AE4_BEA3), w(0x3AE4_BEAB), out(&[first_allocator.ptr() as u32])],
                RETURN,
            );
            expect(
                core::ptr::null_mut(),
                "connection_service_release",
                vec![p(first_allocator.ptr())],
                RETURN,
            );
            edit_last((&mut allocator_pointer as *mut *mut u8).cast(), allocator.ptr() as u32);
        }
        expect(allocator.ptr(), "connection_allocate", vec![w(12)], head.ptr() as u64);
        expect(
            manager.ptr(),
            "connection_query",
            vec![
                Arg::In(vec![head.ptr() as u32]),
                Arg::In(vec![0xFFFF_FFFE, 4]),
                Arg::In(vec![8, 12]),
                w(0xC772_BF98),
                w(u32::MAX),
            ],
            if success { 0x101 } else { 0x100 },
        );
        if success {
            expect(
                core::ptr::null_mut(),
                "get_connection_service",
                vec![],
                service.ptr() as u64,
            );
            expect(
                service.ptr(),
                "connection_remove",
                vec![Arg::In(vec![head.ptr() as u32])],
                RETURN,
            );
            expect(
                manager.ptr(),
                "connection_release_list",
                vec![Arg::In(vec![head.ptr() as u32])],
                RETURN,
            );
        }
        expect(core::ptr::null_mut(), "connection_destroy", vec![p(head.ptr())], RETURN);
        assert_eq!(
            unsafe { invoke(developer.ptr(), lot.ptr()) },
            [RETURN as u32, (RETURN >> 32) as u32, 0xCAFE_1234, head.ptr() as u32]
        );
        unsafe {
            let sentinel = OccupantNode::from_ptr(head.ptr());
            assert_eq!((sentinel.next(), sentinel.previous()), (head.ptr(), head.ptr()));
        }
        finished();
    }
}

#[test]
fn network_removal_filters_flag_byte_unlinks_after_release_and_reloads_allocator_and_manager() {
    let _framework = fake(0x0087_93EC, connection_framework as *const () as usize);
    let _release = fake(0x0041_1870, connection_service_release as *const () as usize);
    let _service = fake(0x0060_2680, get_connection_service as *const () as usize);
    let _destroy = fake(0x0041_3FE0, connection_destroy as *const () as usize);
    let mut developer = Object::new(0x108);
    let mut lot = mock!(0x38 => connection_rect);
    let mut manager = mock!(0xA4 => connection_query, 0xC0 => connection_release_list);
    let mut replacement = mock!(0xC0 => connection_release_list);
    let mut root = mock!(0x14 => connection_service);
    let mut original_allocator = mock!(0x0C => connection_allocate);
    let mut returned_allocator = mock!(0x10 => connection_free);
    let mut allocator = mock!(0x10 => connection_free);
    let mut service = mock!(0x18 => connection_remove);
    let mut occupants = (0..3)
        .map(|_| mock!(0 => connection_iid, 8 => release))
        .collect::<Vec<_>>();
    let mut interfaces = (0..3)
        .map(|_| mock!(0x20 => connection_flags, 8 => release))
        .collect::<Vec<_>>();
    let mut head = Object::new(12);
    let mut nodes = (0..3).map(|_| Object::new(12)).collect::<Vec<_>>();
    let pointers = nodes.iter_mut().map(Object::ptr).collect::<Vec<_>>();
    unsafe {
        for i in 0..3 {
            let node = OccupantNode::from_ptr(pointers[i]);
            node.set_next(if i == 2 { head.ptr() } else { pointers[i + 1] });
            node.set_previous(if i == 0 { head.ptr() } else { pointers[i - 1] });
            node.set_occupant(occupants[i].ptr());
        }
        LotDeveloper::from_ptr(developer.ptr()).set_occupant_manager(manager.ptr());
    }
    let mut allocator_pointer = original_allocator.ptr();
    let _allocator = fake_data(0x00B2_07E4, (&mut allocator_pointer as *mut *mut u8).cast());
    expect(lot.ptr(), "connection_rect", vec![out(&[1, 2, 3, 4])], RETURN);
    expect(
        original_allocator.ptr(),
        "connection_allocate",
        vec![w(12)],
        head.ptr() as u64,
    );
    expect(
        manager.ptr(),
        "connection_query",
        vec![
            Arg::In(vec![head.ptr() as u32]),
            Arg::In(vec![1, 2]),
            Arg::In(vec![3, 4]),
            w(0xC772_BF98),
            w(u32::MAX),
        ],
        0x101,
    );
    edit_last(head.ptr().cast(), pointers[0] as u32);
    edit_last(unsafe { head.ptr().add(4).cast() }, pointers[2] as u32);
    for i in 0..3 {
        expect(
            occupants[i].ptr(),
            "connection_iid",
            vec![w(0xA821_EF94), out(&[interfaces[i].ptr() as u32])],
            0x100,
        );
        expect(
            interfaces[i].ptr(),
            "connection_flags",
            vec![w(0x0020_0000)],
            if i == 1 { 0x101 } else { 0x100 },
        );
        if i != 1 {
            expect(occupants[i].ptr(), "release", vec![], RETURN);
            if i == 0 {
                edit_last((&mut allocator_pointer as *mut *mut u8).cast(), 0);
                expect(core::ptr::null_mut(), "connection_framework", vec![], root.ptr() as u64);
                expect(
                    root.ptr(),
                    "connection_service",
                    vec![w(0x3AE4_BEA3), w(0x3AE4_BEAB), out(&[returned_allocator.ptr() as u32])],
                    RETURN,
                );
                expect(
                    core::ptr::null_mut(),
                    "connection_service_release",
                    vec![p(returned_allocator.ptr())],
                    RETURN,
                );
                edit_last((&mut allocator_pointer as *mut *mut u8).cast(), allocator.ptr() as u32);
            }
            expect(allocator.ptr(), "connection_free", vec![p(pointers[i])], RETURN);
        }
        expect(interfaces[i].ptr(), "release", vec![], RETURN);
        if i == 1 {
            edit_last(unsafe { developer.ptr().add(0x28).cast() }, replacement.ptr() as u32);
        }
    }
    expect(
        core::ptr::null_mut(),
        "get_connection_service",
        vec![],
        service.ptr() as u64,
    );
    expect(
        service.ptr(),
        "connection_remove",
        vec![Arg::In(vec![head.ptr() as u32])],
        RETURN,
    );
    expect(
        replacement.ptr(),
        "connection_release_list",
        vec![Arg::In(vec![head.ptr() as u32])],
        RETURN,
    );
    expect(core::ptr::null_mut(), "connection_destroy", vec![p(head.ptr())], RETURN);
    assert_eq!(
        unsafe { invoke(developer.ptr(), lot.ptr()) },
        [RETURN as u32, (RETURN >> 32) as u32, 0xCAFE_1234, head.ptr() as u32]
    );
    unsafe {
        let sentinel = OccupantNode::from_ptr(head.ptr());
        assert_eq!((sentinel.next(), sentinel.previous()), (pointers[1], pointers[1]));
        let kept = OccupantNode::from_ptr(pointers[1]);
        assert_eq!((kept.next(), kept.previous()), (head.ptr(), head.ptr()));
    }
    assert_eq!(allocator_pointer, allocator.ptr());
    finished();
}

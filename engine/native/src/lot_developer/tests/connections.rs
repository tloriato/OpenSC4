// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::connections::*;
use crate::lot_developer::maintenance::OccupantNode;

slot!(records);
slot!(allocate, bytes);
slot!(free, pointer);
slot!(position, lot, config, record, output, facing);
slot!(create, output, kind, coords, b, c, d, facing);
slot!(submit, lot, list);
slot!(release_nodes, list);
slot!(service_interface, class, iid, output);

unsafe extern "C" fn service() -> u64 {
    run(core::ptr::null_mut(), "service", &[])
}
unsafe extern "C" fn root() -> u64 {
    run(core::ptr::null_mut(), "root", &[])
}
unsafe extern "thiscall" fn release_interface(this: *mut *mut u8) -> u64 {
    run(
        core::ptr::null_mut(),
        "release_interface",
        &[unsafe { this.read_unaligned() } as u32],
    )
}
unsafe extern "thiscall" fn clear(this: *mut *mut u8) -> u64 {
    unsafe {
        let head = this.read_unaligned();
        let mut node = OccupantNode::from_ptr(head).next();
        let mut values = Vec::new();
        let mut previous = head;
        while node != head {
            let view = OccupantNode::from_ptr(node);
            assert_eq!(view.previous(), previous);
            values.push(view.occupant() as u32);
            previous = node;
            node = view.next();
        }
        assert_eq!(OccupantNode::from_ptr(head).previous(), previous);
        let returned = run(head, "clear", &values);
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        returned
    }
}

struct Globals {
    _values: Box<[u32; 5]>,
    _guards: Vec<crate::testing::Fake>,
}
impl Globals {
    fn new(allocator: *mut u8) -> Self {
        let mut values = Box::new([
            allocator as u32,
            0,
            0.0625f32.to_bits(),
            0.5f32.to_bits(),
            12_582_912f32.to_bits(),
        ]);
        let guards = [0x00B2_07E4, 0x00B4_9AEC, 0x00AB_1B04, 0x00A8_4D2C, 0x00AB_1B08]
            .iter()
            .enumerate()
            .map(|(i, &a)| fake_data(a, unsafe { values.as_mut_ptr().add(i).cast() }))
            .collect();
        Self {
            _values: values,
            _guards: guards,
        }
    }
}

fn fakes() -> Vec<crate::testing::Fake> {
    vec![
        fake(0x0060_2680, service as *const () as usize),
        fake(0x0041_3E60, clear as *const () as usize),
        fake(0x0087_93EC, root as *const () as usize),
        fake(0x0041_1870, release_interface as *const () as usize),
    ]
}

#[test]
fn connection_records_filter_rotate_and_append_in_configuration_order() {
    let _fpu = GameFpu::new();
    let _fakes = fakes();
    let mut developer = mock!(0x9C => position);
    let mut config = mock!(0x5C => records);
    let mut allocator = mock!(0x0C => allocate, 0x10 => free);
    let mut service = mock!(0x50 => create, 0x54 => submit);
    let mut manager = mock!(0xC0 => release_nodes);
    let _globals = Globals::new(allocator.ptr());
    let mut head = Object::new(12);
    let mut nodes = (0..5).map(|_| Object::new(12)).collect::<Vec<_>>();
    let mut storage = [[0u32; 13]; 7];
    let mut ids = [
        vec![0xA, 0x1122_3344, 0x5566_7788, 0x99],
        vec![0xB, 1],
        vec![0xC, 1, 2],
        vec![0xD, 0xAABB_CCDD, 0x0102_0304],
        vec![0xE, 0x8000_0001, 1, 2, 3],
        vec![0xF, 0x8000_0001, 0x8000_0001],
        vec![0x10, 0x1122_3344, 0x0102_0304],
    ];
    let facings = [1u32, 0, 0, 0, 3, 4, 0x2000_0001];
    let coordinates = [
        [40., 0., 60.],
        [40., 0., 56.],
        [40., 0., 56.],
        [33., 0., 63.],
        [47., 0., 49.],
        [-16., 0., 16.],
        [32., 0., 48.],
    ];
    for i in 0..7 {
        storage[i][0] = if i == 2 { 1 } else { 7 };
        storage[i][10] = ids[i].as_mut_ptr() as u32;
        storage[i][11] = storage[i][10] + ids[i].len() as u32 * 4;
    }
    let mut vector = [storage.as_mut_ptr() as u32, storage.as_mut_ptr() as u32 + 7 * 52, 0];
    let lot = 0x1234usize as *mut u8;
    unsafe { LotDeveloper::from_ptr(developer.ptr()).set_occupant_manager(manager.ptr()) };
    expect(config.ptr(), "records", vec![], vector.as_mut_ptr() as u64);
    expect(allocator.ptr(), "allocate", vec![w(12)], head.ptr() as u64);
    let mut n = 0usize;
    for i in 0..7 {
        if i == 2 {
            continue;
        }
        expect(
            developer.ptr(),
            "position",
            vec![
                p(lot),
                p(config.ptr()),
                w(storage[i].as_mut_ptr() as u32),
                out(&coordinates[i].map(f32::to_bits)),
                out(&[facings[i]]),
            ],
            RETURN,
        );
        if i == 1 {
            continue;
        }
        expect(core::ptr::null_mut(), "service", vec![], service.ptr() as u64);
        let (x, z) = match i {
            5 => (-1i32, 1i32),
            _ => (2, 3),
        };
        let rotate = |v: u32| {
            if facings[i] == 0 {
                v
            } else {
                let shift = facings[i].wrapping_mul(8);
                v.wrapping_shl(shift)
                    .wrapping_add(v.wrapping_shr(32u32.wrapping_sub(shift)))
            }
        };
        expect(
            service.ptr(),
            "create",
            vec![
                out(&[0x1000 + n as u32]),
                w(ids[i][0]),
                Arg::In(vec![x as u32, z as u32]),
                w(rotate(ids[i][1])),
                w(rotate(ids[i][2])),
                w(*ids[i].get(3).unwrap_or(&0)),
                w(facings[i]),
            ],
            RETURN,
        );
        expect(allocator.ptr(), "allocate", vec![w(12)], nodes[n].ptr() as u64);
        n += 1;
    }
    expect(core::ptr::null_mut(), "service", vec![], service.ptr() as u64);
    expect(
        service.ptr(),
        "submit",
        vec![p(lot), Arg::In(vec![head.ptr() as u32])],
        RETURN,
    );
    expect(
        manager.ptr(),
        "release_nodes",
        vec![Arg::In(vec![head.ptr() as u32])],
        RETURN,
    );
    expect(head.ptr(), "clear", (0..5).map(|i| w(0x1000 + i)).collect(), RETURN);
    expect(allocator.ptr(), "free", vec![p(head.ptr())], RETURN);
    let returned = unsafe {
        call_thiscall_with_eax(
            fc4p_006c9c80 as *const () as usize,
            developer.ptr(),
            &[lot as u32, config.ptr() as u32, 0xABCD_1200],
            0xCCCC_CCCC,
        )
    };
    assert_eq!(returned, (RETURN as u32, (RETURN >> 32) as u32));
    finished();
}

#[test]
fn missing_and_empty_record_vectors_still_submit_and_release_an_empty_list() {
    let _fakes = fakes();
    for missing in [false, true] {
        let mut developer = Object::new(0x108);
        let mut config = mock!(0x5C => records);
        let mut allocator = mock!(0x0C => allocate, 0x10 => free);
        let mut service = mock!(0x54 => submit);
        let mut manager = mock!(0xC0 => release_nodes);
        let mut root = mock!(0x14 => service_interface);
        let mut globals = Globals::new(core::ptr::null_mut());
        let mut head = Object::new(12);
        let mut vector = [0u32; 3];
        unsafe { LotDeveloper::from_ptr(developer.ptr()).set_occupant_manager(manager.ptr()) };
        expect(
            config.ptr(),
            "records",
            vec![],
            if missing { 0 } else { vector.as_mut_ptr() as u64 },
        );
        expect(core::ptr::null_mut(), "root", vec![], root.ptr() as u64);
        expect(
            root.ptr(),
            "service_interface",
            vec![w(0x3AE4_BEA3), w(0x3AE4_BEAB), out(&[allocator.ptr() as u32])],
            RETURN,
        );
        expect(
            core::ptr::null_mut(),
            "release_interface",
            vec![p(allocator.ptr())],
            RETURN,
        );
        expect(allocator.ptr(), "allocate", vec![w(12)], head.ptr() as u64);
        expect(core::ptr::null_mut(), "service", vec![], service.ptr() as u64);
        expect(
            service.ptr(),
            "submit",
            vec![w(0x1234), Arg::In(vec![head.ptr() as u32])],
            RETURN,
        );
        expect(
            manager.ptr(),
            "release_nodes",
            vec![Arg::In(vec![head.ptr() as u32])],
            RETURN,
        );
        // The last allocator lookup uses the caller's configuration word as its service temporary.
        edit_last(globals._values.as_mut_ptr(), 0);
        expect(head.ptr(), "clear", vec![], RETURN);
        expect(core::ptr::null_mut(), "root", vec![], root.ptr() as u64);
        expect(
            root.ptr(),
            "service_interface",
            vec![w(0x3AE4_BEA3), w(0x3AE4_BEAB), out(&[allocator.ptr() as u32])],
            RETURN,
        );
        expect(
            core::ptr::null_mut(),
            "release_interface",
            vec![p(allocator.ptr())],
            RETURN,
        );
        expect(allocator.ptr(), "free", vec![p(head.ptr())], RETURN);
        let returned = unsafe {
            call_thiscall_with_eax(
                fc4p_006c9c80 as *const () as usize,
                developer.ptr(),
                &[0x1234, config.ptr() as u32, 0],
                0xCCCC_CCCC,
            )
        };
        assert_eq!(returned, (RETURN as u32, (RETURN >> 32) as u32));
        assert_eq!(globals._values[0], allocator.ptr() as u32);
        finished();
    }
}

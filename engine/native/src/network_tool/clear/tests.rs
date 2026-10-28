// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data};
use std::cell::RefCell;

#[derive(Default)]
struct State {
    allocator: usize,
    head: usize,
    node: usize,
    enumerate: u32,
    calls: Vec<(u32, usize)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(tag: u32, p: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push((tag, p as usize)));
}
extern "C" fn factory() -> *mut u8 {
    log(1, core::ptr::null_mut());
    STATE.with(|s| s.borrow().allocator as *mut u8)
}
extern "thiscall" fn allocator_query(this: *mut u8, a: u32, b: u32, output: *mut *mut u8) {
    assert_eq!((a, b), (0x3AE4_BEA3, 0x3AE4_BEAB));
    log(2, this);
    unsafe {
        assert!((*output).is_null());
        *output = this;
    }
}
extern "thiscall" fn release_holder(this: *mut *mut u8) {
    log(3, unsafe { *this });
    unsafe {
        assert_eq!(*list_allocator(), *this);
        *this = core::ptr::null_mut();
    }
}
extern "thiscall" fn allocate(this: *mut u8, size: u32) -> *mut u8 {
    assert_eq!(size, 12);
    log(4, this);
    STATE.with(|s| s.borrow().head as *mut u8)
}
extern "thiscall" fn enumerate(
    this: *mut u8,
    list: *mut *mut u8,
    first: *const u32,
    last: *const u32,
    zero: u32,
    mask: u32,
) -> u32 {
    assert_eq!((first, zero, mask), (last, 0, u32::MAX));
    unsafe {
        assert_eq!(core::slice::from_raw_parts(first, 2), [17, 23]);
    }
    log(5, this);
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            let head = *list;
            assert_eq!(head as usize, s.head);
            assert_eq!(
                (read::<*mut u8>(head, 0), read::<*mut u8>(head, 4), read::<u32>(head, 8)),
                (head, head, 0xA5A5_A5A5)
            );
            if s.node != 0 {
                write(head, 0, s.node as u32);
            }
        }
        s.enumerate
    })
}
extern "thiscall" fn filter(this: *mut u8, occupant: *mut u8) -> u32 {
    log(6, this);
    unsafe { read(occupant, 8) }
}
extern "thiscall" fn remove(this: *mut u8, occupant: *mut u8, detach: u32, flags: u32) {
    assert_eq!((detach, flags), (1, 0));
    log(7, this);
    log(70, occupant);
}
extern "thiscall" fn class(this: *mut u8) -> u32 {
    log(8, this);
    unsafe { read(this, 12) }
}
extern "thiscall" fn usable(this: *mut u8, mode: u32) -> u32 {
    assert_eq!(mode, 1);
    log(9, this);
    unsafe { read(this, 16) }
}
extern "thiscall" fn output(this: *mut *mut u8) -> *mut *mut u8 {
    unsafe {
        assert!((*this).is_null());
    }
    log(10, core::ptr::null_mut());
    this
}
extern "thiscall" fn qi(this: *mut u8, iid: u32, output: *mut *mut u8) -> u32 {
    assert_eq!(iid, 0xA8C8_6142);
    log(11, this);
    unsafe {
        let result: u32 = read(this, 28);
        if result != 0 {
            *output = this;
        }
        result
    }
}
extern "thiscall" fn position(this: *mut u8, x: *mut u32, z: *mut u32) {
    log(12, this);
    unsafe {
        *x = read(this, 20);
        *z = read(this, 24);
    }
}
extern "thiscall" fn demolish(this: *mut u8, a: u32, occupant: *mut u8, b: u32, flags: u32, c: u32, d: u32, e: u32) {
    assert_eq!((a, b, flags, c, d, e), (1, 1, 0x2001, 0, 0, 0));
    log(13, this);
    log(130, occupant);
}
extern "thiscall" fn release(this: *mut u8) {
    log(14, this);
}
extern "thiscall" fn finish(this: *mut u8, list: *mut *mut u8) {
    STATE.with(|s| assert_eq!(unsafe { *list } as usize, s.borrow().head));
    log(15, this);
}
extern "thiscall" fn destroy(this: *mut *mut u8) -> u64 {
    log(16, unsafe { *this });
    registers(0xAABB_CCDD, 0x1234_5678)
}

#[test]
fn null_filter_preserves_registers_and_does_not_touch_services() {
    let args = [17, 23, 0];
    let mut c = RawCall::new(Conv::Thiscall, fc4p_006272f0 as *const () as usize, 1, &args);
    c.eax = 0x1234_5678;
    c.edx = 0xDEAD_BEEF;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!((c.eax, c.edx), (0x1234_5678, 0xDEAD_BEEF));
}

#[test]
fn clear_occupants_covers_filter_type_class_usable_interface_and_cell_branches_in_order() {
    let _f = [
        fake(0x0087_93EC, factory as *const () as usize),
        fake(0x0041_1870, release_holder as *const () as usize),
        fake(0x004F_20A0, output as *const () as usize),
        fake(0x0041_3FE0, destroy as *const () as usize),
    ];
    // filter result, network type, class, usability, QI result, position x/z, enumeration success, empty list.
    for (matches, kind, class_id, usable_result, qi_result, x, z, enumerated, empty) in [
        (1u32, 0u32, 0u32, 0u32, 0u32, 17u32, 23u32, 1u32, false),
        (0, 0, 0x2890_D4DE, 1, 1, 17, 23, 1, false),
        (0, 5, 0x2890_D4DE, 1, 1, 17, 23, 1, false),
        (0, 7, 0x2890_D4DE, 1, 1, 17, 23, 1, false),
        (0, 4, 0x2890_D4DE, 1, 1, 17, 23, 1, false),
        (0, 0, 0, 1, 1, 17, 23, 1, false),
        (0, 0, 0x2890_D4DE, 0x100, 1, 17, 23, 1, false),
        (0, 0, 0x2890_D4DE, 1, 0, 17, 23, 1, false),
        (0, 0, 0x2890_D4DE, 1, 0x100, 17, 23, 1, false),
        (0, 0, 0x2890_D4DE, 1, 1, 18, 23, 1, false),
        (0, 0, 0x2890_D4DE, 1, 1, 17, 24, 1, false),
        (0x100, 0, 0x2890_D4DE, 0x101, 0x101, 17, 23, 0x101, false),
        (1, 0, 0, 0, 0, 17, 23, 0x100, false),
        (1, 0, 0, 0, 0, 17, 23, 1, true),
    ] {
        for lazy in [false, true] {
            let mut table = [0usize; 49];
            table[0] = qi as *const () as usize;
            table[2] = release as *const () as usize;
            table[3] = filter as *const () as usize;
            table[5] = allocator_query as *const () as usize;
            table[7] = class as *const () as usize;
            table[8] = position as *const () as usize;
            table[0x54 / 4] = remove as *const () as usize;
            table[0x80 / 4] = usable as *const () as usize;
            table[0xA0 / 4] = enumerate as *const () as usize;
            table[0xC0 / 4] = finish as *const () as usize;
            let mut alloc_table = table;
            alloc_table[3] = allocate as *const () as usize;
            let mut demo_table = table;
            demo_table[8] = demolish as *const () as usize;
            let mut allocator = [alloc_table.as_ptr() as u32];
            let ap = allocator.as_mut_ptr().cast::<u8>();
            let mut manager = [table.as_ptr() as u32];
            let mp = manager.as_mut_ptr().cast::<u8>();
            let mut demo = [demo_table.as_ptr() as u32];
            let mut demo_ptr = demo.as_mut_ptr().cast::<u8>();
            let mut occupant = [
                table.as_ptr() as u32,
                99,
                matches,
                class_id,
                usable_result,
                x,
                z,
                qi_result,
            ];
            let op = occupant.as_mut_ptr().cast::<u8>();
            let mut head = [0xA5A5_A5A5u32; 3];
            let hp = head.as_mut_ptr().cast::<u8>();
            let mut node = [hp as u32, hp as u32, op as u32];
            let np = node.as_mut_ptr().cast::<u8>();
            let mut tool = [0u32; 0x368 / 4];
            tool[0x48 / 4] = mp as u32;
            tool[0x35C / 4] = kind;
            let t = tool.as_mut_ptr().cast::<u8>();
            let before = tool;
            let mut allocator_global = if lazy { core::ptr::null_mut() } else { ap };
            let _g = [
                fake_data(0x00B2_07E4, (&mut allocator_global as *mut *mut u8).cast()),
                fake_data(0x00B4_3D00, (&mut demo_ptr as *mut *mut u8).cast()),
            ];
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    allocator: ap as usize,
                    head: hp as usize,
                    node: if empty { 0 } else { np as usize },
                    enumerate: enumerated,
                    ..State::default()
                }
            });
            let args = [17, 23, mp as u32];
            let mut c = RawCall::new(Conv::Thiscall, fc4p_006272f0 as *const () as usize, t as u32, &args);
            unsafe {
                call_raw(&mut c);
            }
            assert_eq!((c.eax, c.edx), (0xAABB_CCDD, 0x1234_5678));
            assert_eq!(tool, before);
            assert_eq!(allocator_global, ap);
            let mut expected = vec![];
            if lazy {
                expected.extend([(1, 0), (2, ap as usize), (3, ap as usize)]);
            }
            expected.extend([(4, ap as usize), (5, mp as usize)]);
            if enumerated as u8 != 0 {
                if !empty {
                    expected.push((6, mp as usize));
                    if matches as u8 != 0 {
                        expected.extend([(7, mp as usize), (70, op as usize)]);
                    } else if ![5, 7, 4].contains(&kind) {
                        expected.push((8, op as usize));
                        if class_id == 0x2890_D4DE {
                            expected.push((9, op as usize));
                            if usable_result as u8 != 0 {
                                expected.extend([(10, 0), (11, op as usize)]);
                                if qi_result as u8 != 0 {
                                    expected.push((12, op as usize));
                                    if x == 17 && z == 23 {
                                        expected.extend([(13, demo_ptr as usize), (130, op as usize)]);
                                    }
                                }
                                if qi_result != 0 {
                                    expected.push((14, op as usize));
                                }
                            }
                        }
                    }
                }
                expected.push((15, mp as usize));
            }
            expected.push((16, hp as usize));
            STATE.with(|s| assert_eq!(s.borrow().calls, expected));
        }
    }
}

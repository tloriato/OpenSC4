// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data};
use std::cell::RefCell;

#[derive(Default)]
struct State {
    mask: u32,
    lookup: u32,
    files: [usize; 4],
    ini: usize,
    tree: usize,
    section: usize,
    highways: usize,
    calls: Vec<(u32, usize)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
extern "C" fn lookup(key: *const u32, file: *mut *mut u8, flags: u32) -> u64 {
    assert_eq!(flags, 0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let n = s.lookup;
        let expected = [[0, 0x8A59_71C5, 0x8A59_93B9], [10, 11, 12], [20, 21, 22], [30, 31, 32]][n as usize];
        unsafe {
            assert_eq!(core::slice::from_raw_parts(key, 3), expected);
        }
        s.calls.push((0, n as usize));
        s.lookup += 1;
        let success = s.mask & (1 << n) != 0;
        if success {
            unsafe {
                *file = s.files[n as usize] as *mut u8;
            }
        }
        registers(0x1234_5600 | u32::from(success), 0x7777_8888)
    })
}
extern "thiscall" fn new_ini(this: *mut u8, file: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.ini == 0 {
            s.ini = this as usize;
        }
        assert_eq!(s.ini, this as usize);
        assert_eq!(file as usize, s.files[s.lookup as usize - 1]);
        s.calls.push((1, file as usize));
    });
    unsafe {
        write(this, 0, 0xAA55_AA55u32);
    }
}
extern "thiscall" fn destroy_ini(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.ini);
        s.calls.push((2, 0));
    });
    registers(0xAAAA_BBBB, 0xCCCC_DDDD)
}
fn reader(this: *mut u8, id: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.ini);
        unsafe {
            assert_eq!(read::<u32>(this, 0), 0xAA55_AA55);
        }
        s.calls.push((id, 0));
    });
}
extern "C" fn types(this: *mut u8) {
    reader(this, 3);
}
extern "C" fn intersections(this: *mut u8) {
    reader(this, 4);
}
extern "C" fn matrix(this: *mut u8, values: *mut u32) {
    reader(this, 5);
    unsafe {
        assert_eq!(core::slice::from_raw_parts(values, 13), [0; 13]);
        *values.add(2) = 0xDEAD_BEEF;
        *values.add(12) = 0xFF00_AA55;
    }
}
extern "thiscall" fn clear(this: *mut u8, root: *mut u8) {
    assert_eq!(root as usize, 0x1234);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tree);
        s.calls.push((6, root as usize));
    });
}
extern "C" fn conflicts(this: *mut u8, section: *const u8) {
    reader(this, 7);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(section as usize, s.section);
        unsafe {
            let tree = s.tree as *mut u8;
            let head: *mut u8 = read(tree, 0);
            assert_eq!(read::<u32>(tree, 4), 0);
            assert_eq!(
                (
                    read::<u32>(head, 4),
                    read::<*mut u8>(head, 8),
                    read::<*mut u8>(head, 12)
                ),
                (0, head, head)
            );
        }
    });
}
extern "C" fn highways(this: *mut u8, output: *mut u8) {
    reader(this, 8);
    STATE.with(|s| assert_eq!(output as usize, s.borrow().highways));
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push((9, this as usize)));
    registers(0x1357_2468, 0xFACE_CAFE)
}

#[test]
fn reload_reads_each_available_resource_in_order_retains_missing_tables_and_releases_final_file() {
    let _f = [
        fake(0x005F_9670, lookup as *const () as usize),
        fake(0x0091_CF46, new_ini as *const () as usize),
        fake(0x0091_CDE3, destroy_ini as *const () as usize),
        fake(0x0062_3310, types as *const () as usize),
        fake(0x0064_1D30, intersections as *const () as usize),
        fake(0x0063_D630, matrix as *const () as usize),
        fake(0x0062_4CE0, clear as *const () as usize),
        fake(0x0063_D3E0, conflicts as *const () as usize),
        fake(0x0064_1B60, highways as *const () as usize),
    ];
    for mask in 0u32..16 {
        let mut key1 = [10u32, 11, 12];
        let mut key2 = [20u32, 21, 22];
        let mut key3 = [30u32, 31, 32];
        let mut section = 0u8;
        let mut highway_output = 0u8;
        let mut matrix_output = [0xA5u8; 0xE04];
        let mut head = [0x5566u32, 0x1234, 0xABCD, 0x5678];
        let mut tree = [head.as_mut_ptr() as u32, 7];
        let _g = [
            fake_data(0x00B0_B544, key1.as_mut_ptr().cast()),
            fake_data(0x00B0_B550, key2.as_mut_ptr().cast()),
            fake_data(0x00B0_B538, key3.as_mut_ptr().cast()),
            fake_data(0x00AA_86F0, &mut section),
            fake_data(0x00B4_52B0, &mut highway_output),
            fake_data(0x00B4_53C4, matrix_output.as_mut_ptr()),
            fake_data(0x00B4_66D0, tree.as_mut_ptr().cast()),
        ];
        let mut vtable = [0usize; 3];
        vtable[2] = release as *const () as usize;
        let mut files = [[0usize; 1]; 4];
        for file in &mut files {
            file[0] = vtable.as_ptr() as usize;
        }
        let file_ptrs = files.each_mut().map(|f| f.as_mut_ptr() as usize);
        STATE.with(|s| {
            *s.borrow_mut() = State {
                mask,
                files: file_ptrs,
                tree: tree.as_mut_ptr() as usize,
                section: &mut section as *mut u8 as usize,
                highways: &mut highway_output as *mut u8 as usize,
                ..State::default()
            }
        });
        let result = unsafe { fc4p_006253d0() };
        assert_eq!(
            result,
            if mask == 0 {
                registers(0x1234_5600, 0x7777_8888)
            } else {
                registers(0x1357_2468, 0xFACE_CAFE)
            }
        );
        let mut expected = vec![];
        let mut last = 0;
        for (n, &file_pointer) in file_ptrs.iter().enumerate() {
            expected.push((0, n));
            if mask & (1 << n) != 0 {
                last = file_pointer;
                expected.push((1, last));
                match n {
                    0 => expected.push((3, 0)),
                    1 => expected.extend([(4, 0), (5, 0)]),
                    2 => expected.extend([(6, 0x1234), (7, 0)]),
                    3 => expected.push((8, 0)),
                    _ => unreachable!(),
                }
                expected.push((2, 0));
            }
        }
        if last != 0 {
            expected.push((9, last));
        }
        STATE.with(|s| assert_eq!(s.borrow().calls, expected));
        let mut expected_matrix = [0xA5u8; 0xE04];
        if mask & 2 != 0 {
            for i in 0..13 {
                let value = match i {
                    2 => 0xDEAD_BEEFu32,
                    12 => 0xFF00_AA55,
                    _ => 0,
                };
                expected_matrix[i * 0x114..i * 0x114 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        assert_eq!(matrix_output, expected_matrix);
        if mask & 4 == 0 {
            assert_eq!((tree[1], head), (7, [0x5566, 0x1234, 0xABCD, 0x5678]));
        }
    }
}

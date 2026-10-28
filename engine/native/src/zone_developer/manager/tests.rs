// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

thread_local! {
    static CALLS: RefCell<Vec<(u32,usize,Vec<u32>)>> = const { RefCell::new(Vec::new()) };
    static DRAWS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static CHANGE: RefCell<Option<(usize,usize,u32)>> = const { RefCell::new(None) };
}
fn record(code: u32, this: *mut u8, args: &[u32]) {
    CALLS.with(|c| c.borrow_mut().push((code, this as usize, args.to_vec())));
}
fn calls() -> Vec<(u32, usize, Vec<u32>)> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}
fn change() {
    if let Some((address, offset, value)) = CHANGE.with(|c| c.borrow_mut().take()) {
        unsafe { write_unaligned((address as *mut u8).add(offset).cast::<u32>(), value) };
    }
}
extern "thiscall" fn version(this: *mut u8, value: u32) -> u32 {
    record(0x1C, this, &[value]);
    0
}
extern "thiscall" fn integer(this: *mut u8, value: u32) -> u32 {
    record(0x24, this, &[value]);
    0
}
extern "fastcall" fn error(this: *mut u8, entry_edx: u32) -> u64 {
    record(0x48, this, &[]);
    (u64::from(entry_edx) << 32) | u64::from(unsafe { read_unaligned(this.add(4).cast::<u32>()) })
}
extern "C" fn reference(this: *mut u8, value: u32) -> u32 {
    record(0x005D_99A0, this, &[value]);
    change();
    0
}
extern "thiscall" fn draw(this: *mut u8, bound: u32) -> u32 {
    record(0x0091_36B1, this, &[bound]);
    change();
    DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_empty() {
            0
        } else {
            d.remove(0)
        }
    })
}
extern "C" fn free(this: *mut u8) -> u32 {
    record(0x0090_CF63, this, &[]);
    change();
    0xFFFF_FFFF
}

#[test]
fn lot_map_version_dimensions_references_and_low_error_byte() {
    let _ref = fake(0x005D_99A0, reference as *const () as usize);
    let mut table = [0usize; 19];
    table[7] = version as *const () as usize;
    table[9] = integer as *const () as usize;
    table[18] = error as *const () as usize;
    let mut stream = [table.as_ptr() as usize, 0usize];
    let stream_ptr = stream.as_mut_ptr().cast::<u8>();
    let mut object = Object::new(0x60);
    let ptr = object.ptr();
    let mut a = [0u32, 0x1234, 0xFF00_0001];
    let mut b = [7u32, 7, 0x8765_4321];
    let mut rows = [
        a.as_mut_ptr() as usize,
        a.as_mut_ptr().wrapping_add(3) as usize,
        0xABCD,
        b.as_mut_ptr() as usize,
        b.as_mut_ptr().wrapping_add(3) as usize,
        0xDCBA,
    ];
    unsafe {
        write_unaligned(ptr.add(0x48).cast::<usize>(), rows.as_mut_ptr() as usize);
        write_unaligned(
            ptr.add(0x4C).cast::<usize>(),
            rows.as_mut_ptr().wrapping_add(6) as usize,
        );
    }
    for error in [0u32, 1, 0x1234_5600, 0xFFFF_FFFF] {
        unsafe { write_unaligned(stream_ptr.add(4).cast::<u32>(), error) };
        let before = unsafe { core::slice::from_raw_parts(ptr, 0x60) }.to_vec();
        assert_eq!(
            unsafe { fc4p_0072ab40(ptr, stream_ptr) },
            (u64::from(table.as_ptr() as u32) << 32) | u64::from((error & !255) | u32::from(error as u8 == 0))
        );
        assert_eq!(unsafe { core::slice::from_raw_parts(ptr, 0x60) }, before);
        let mut expected = vec![
            (0x1C, stream_ptr as usize, vec![1]),
            (0x24, stream_ptr as usize, vec![2]),
            (0x24, stream_ptr as usize, vec![3]),
        ];
        expected
            .extend([0, 0x1234, 0xFF00_0001, 7, 7, 0x8765_4321].map(|v| (0x005D_99A0, stream_ptr as usize, vec![v])));
        expected.push((0x48, stream_ptr as usize, vec![]));
        assert_eq!(calls(), expected);
    }
    // A reference writer may shorten a row; iteration reads that row's end again after each call.
    CHANGE.with(|c| *c.borrow_mut() = Some((rows.as_mut_ptr() as usize, 4, a.as_mut_ptr().wrapping_add(1) as u32)));
    unsafe { fc4p_0072ab40(ptr, stream_ptr) };
    let actual = calls();
    assert_eq!(
        actual
            .iter()
            .filter(|c| c.0 == 0x005D_99A0)
            .map(|c| c.2[0])
            .collect::<Vec<_>>(),
        [0, 7, 7, 0x8765_4321]
    );
    // Empty maps do not read a row and still emit both zero dimensions.
    unsafe { write_unaligned(ptr.add(0x4C).cast::<usize>(), rows.as_mut_ptr() as usize) };
    unsafe { fc4p_0072ab40(ptr, stream_ptr) };
    assert_eq!(
        calls(),
        [
            (0x1C, stream_ptr as usize, vec![1]),
            (0x24, stream_ptr as usize, vec![0]),
            (0x24, stream_ptr as usize, vec![0]),
            (0x48, stream_ptr as usize, vec![])
        ]
    );
}

#[test]
fn random_lot_order_retries_limit_and_live_dimensions() {
    let _draw = fake(0x0091_36B1, draw as *const () as usize);
    let mut rng = [0xA5u8; 16];
    let _rng = fake_data(0x00B4_B398, rng.as_mut_ptr());
    let mut object = Object::new(0x60);
    let ptr = object.ptr();
    let manager = unsafe { LotManager::from_ptr(ptr) };
    let mut a = [0u32, 0, 0];
    let mut b = [0u32, 0xABCD_1234, 0];
    let mut rows = [a.as_mut_ptr() as usize, 0, 0, b.as_mut_ptr() as usize, 0, 0];
    manager.set_cells_x(2);
    manager.set_cells_z(3);
    manager.set_rows(rows.as_mut_ptr().cast());
    DRAWS.with(|d| *d.borrow_mut() = vec![0, 2, 1, 1]);
    assert_eq!(
        unsafe { fc4p_0072b3c0(ptr) },
        (u64::from(b.as_ptr() as u32) << 32) | 0xABCD_1234
    );
    assert_eq!(
        calls(),
        [
            (0x0091_36B1, rng.as_mut_ptr() as usize, vec![2]),
            (0x0091_36B1, rng.as_mut_ptr() as usize, vec![3]),
            (0x0091_36B1, rng.as_mut_ptr() as usize, vec![2]),
            (0x0091_36B1, rng.as_mut_ptr() as usize, vec![3])
        ]
    );
    assert_eq!(unsafe { fc4p_0072b3c0(ptr) }, u64::from(a.as_ptr() as u32) << 32);
    let actual = calls();
    assert_eq!(actual.len(), 1998);
    assert!(actual.as_chunks::<2>().0.iter().all(|c| c[0].2 == [2] && c[1].2 == [3]));
    CHANGE.with(|c| *c.borrow_mut() = Some((ptr as usize, 0x28, 1)));
    DRAWS.with(|d| *d.borrow_mut() = vec![1, 1]);
    assert_eq!(unsafe { fc4p_0072b3c0(ptr) } as u32, 0xABCD_1234);
    assert_eq!(calls()[1].2, [1]);
    assert_eq!(rng, [0xA5; 16]);
}

#[test]
fn remove_list_entry_updates_only_matching_safe_iterators_and_unlinks_before_free() {
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let mut head = [0usize; 3];
    let mut a = [0usize, 0, 11];
    let mut b = [0usize, 0, 22];
    let mut c = [0usize, 0, 22];
    let (hp, ap, bp, cp) = (
        head.as_mut_ptr() as usize,
        a.as_mut_ptr() as usize,
        b.as_mut_ptr() as usize,
        c.as_mut_ptr() as usize,
    );
    head[..2].copy_from_slice(&[ap, cp]);
    a[..2].copy_from_slice(&[bp, hp]);
    b[..2].copy_from_slice(&[cp, ap]);
    c[..2].copy_from_slice(&[hp, bp]);
    let mut positions = [bp, bp, cp, hp];
    let mut iter_head = [0usize; 3];
    let mut iters = [[0usize; 3]; 4];
    let ih = iter_head.as_mut_ptr() as usize;
    let ip = iters.as_mut_ptr();
    for (i, iter) in iters.iter_mut().enumerate() {
        iter[0] = if i == 3 { ih } else { ip.wrapping_add(i + 1) as usize };
        iter[2] = positions.as_mut_ptr().wrapping_add(i) as usize;
    }
    unsafe { write_unaligned(ih as *mut usize, ip as usize) };
    let mut list = [hp, ih, 3usize];
    let ptr = list.as_mut_ptr().cast::<u8>();
    assert_eq!(unsafe { fc4p_0072b990(ptr, &99) }, hp as u32);
    assert!(calls().is_empty());
    // The allocator's result does not become the return; the count is reloaded after freeing the node.
    CHANGE.with(|v| *v.borrow_mut() = Some((ptr as usize, 8, 10)));
    assert_eq!(unsafe { fc4p_0072b990(ptr, &22) }, 9);
    assert_eq!(calls(), [(0x0090_CF63, bp, vec![])]);
    assert_eq!(positions, [cp, cp, cp, hp]);
    assert_eq!((a[0], c[1]), (cp, ap));
    assert_eq!(b, [cp, ap, 22]);
    assert_eq!(head, [ap, cp, 0]);
    assert_eq!(list, [hp, ih, 9]);
    // Empty lists do not dereference the search value.
    unsafe { write_unaligned(hp as *mut usize, hp) };
    assert_eq!(unsafe { fc4p_0072b990(ptr, core::ptr::null()) }, hp as u32);
    assert!(calls().is_empty());
}

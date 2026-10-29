// SPDX-License-Identifier: GPL-3.0-or-later
//! Two-byte cell deque append helpers.

use crate::port::Port;

layout! {
    /// The finish iterator and block map of the cell deque.
    pub struct CellDeque {
        0x10 => cursor / set_cursor: *mut u8,
        0x14 => block / set_block: *mut u8,
        0x18 => end / set_end: *mut u8,
        0x1C => map_cursor / set_map_cursor: *mut *mut u8,
        0x20 => map / set_map: *mut *mut u8,
        0x24 => map_capacity / set_map_capacity: u32,
    }
}
game_fn! { fn grow_map(this:*mut u8, count:u32, front:u32) -> u32 = "thiscall" 0x006D_B4A0; }
game_fn! { fn allocate_block(size:u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn append_boundary(this:*mut u8, value:*const u16) -> u64 = "thiscall" 0x006D_C360; }

port! {
    /// Copies the cell before any allocation, grows the map when fewer than two slots remain, allocates
    /// the next 128-byte block, writes the old finish cell, and advances the finish iterator.
    #[port(address = 0x006D_C360, owner = "plumbing", name = "CellDequeAppendBoundary")]
    pub unsafe extern "thiscall" fn fc4p_006dc360(this:*mut u8, value:*const u16) -> u64 {
        // SAFETY: the caller supplies a live deque at the last cell of its block and readable input.
        unsafe {
            let deque=CellDeque::from_ptr(this);
            let cell=value.read_unaligned();
            let used=(deque.map_cursor() as u32).wrapping_sub(deque.map() as u32) as i32 >> 2;
            if deque.map_capacity().wrapping_sub(used as u32) < 2 { grow_map(this,1,0); }
            let block=allocate_block(128);
            deque.map_cursor().wrapping_add(1).write_unaligned(block);
            let old=deque.cursor();
            if !old.is_null() { old.cast::<u16>().write_unaligned(cell); }
            let next=deque.map_cursor().wrapping_add(1);
            deque.set_map_cursor(next);
            let first=next.read_unaligned();
            deque.set_block(first);
            let end=first.wrapping_add(128);
            deque.set_end(end);
            deque.set_cursor(deque.block());
            (u64::from(deque.block() as u32)<<32)|u64::from(end as u32)
        }
    }
}

port! {
    /// Appends directly except at the last cell of a block, where the boundary helper is called.
    /// Null finish pointers still advance by two; EDX retains its partial-word copy on the ordinary path.
    #[port(address = 0x006D_C970, owner = "plumbing", name = "CellDequeAppend")]
    pub unsafe extern "thiscall" fn fc4p_006dc970(this:*mut u8, value:*const u16) -> u64 {
        // SAFETY: the caller supplies the live deque and input cell when it is read.
        unsafe {
            let deque=CellDeque::from_ptr(this);
            let limit=(deque.end() as u32).wrapping_sub(2);
            let cursor=deque.cursor();
            if cursor as u32 == limit { return append_boundary(this,value); }
            let mut edx=limit;
            if !cursor.is_null() {
                let cell=value.read_unaligned();
                edx=(value as u32 & 0xFFFF_0000)|u32::from(cell);
                cursor.cast::<u16>().write_unaligned(cell);
            }
            deque.set_cursor(deque.cursor().wrapping_add(2));
            (u64::from(edx)<<32)|u64::from(cursor as u32)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006dc360::PORT, fc4p_006dc970::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, Object};
    use std::cell::RefCell;
    #[derive(Default)]
    struct State {
        calls: Vec<(usize, u32, u32)>,
        allocation: usize,
        replacement: usize,
        input: usize,
    }
    thread_local! { static STATE:RefCell<State> = RefCell::new(State::default()); }
    extern "thiscall" fn grow(this: *mut u8, count: u32, front: u32) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push((this as usize, count, front));
            let deque = unsafe { CellDeque::from_ptr(this) };
            deque.set_map_cursor(s.replacement as *mut *mut u8);
            unsafe { (s.input as *mut u16).write(0xEEEE) };
        });
        0
    }
    extern "C" fn allocate(size: u32) -> *mut u8 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push((0, size, 0));
            s.allocation as *mut u8
        })
    }
    #[test]
    fn boundary_append_copies_before_growth_and_rereads_the_map() {
        let _grow = fake(0x006D_B4A0, grow as *const () as usize);
        let _allocate = fake(0x0090_CF54, allocate as *const () as usize);
        for remaining in [0u32, 1, 2, 3, u32::MAX] {
            let mut object = Object::new(0x28);
            let deque = unsafe { CellDeque::from_ptr(object.ptr()) };
            let mut old = [0xCCu8; 128];
            let mut new = [0xDDu8; 128];
            let mut map = [core::ptr::null_mut::<u8>(); 4];
            let mut replacement = [core::ptr::null_mut::<u8>(); 4];
            let mut value = 0x1234u16;
            deque.set_cursor(old.as_mut_ptr().wrapping_add(126));
            deque.set_block(old.as_mut_ptr());
            deque.set_end(old.as_mut_ptr().wrapping_add(128));
            deque.set_map(map.as_mut_ptr());
            deque.set_map_cursor(map.as_mut_ptr().wrapping_add(1));
            deque.set_map_capacity(remaining.wrapping_add(1));
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    calls: vec![],
                    allocation: new.as_mut_ptr() as usize,
                    replacement: replacement.as_mut_ptr() as usize,
                    input: &mut value as *mut u16 as usize,
                }
            });
            let result = unsafe { fc4p_006dc360(object.ptr(), &value) };
            assert_eq!(
                result,
                (u64::from(new.as_ptr() as u32) << 32) | u64::from(new.as_ptr().wrapping_add(128) as u32)
            );
            assert_eq!(&old[126..], &0x1234u16.to_le_bytes());
            assert!(old[..126].iter().all(|&x| x == 0xCC));
            assert_eq!(new, [0xDD; 128]);
            let grew = remaining < 2;
            assert_eq!(value, if grew { 0xEEEE } else { 0x1234 });
            assert_eq!(deque.cursor(), new.as_mut_ptr());
            assert_eq!(deque.block(), new.as_mut_ptr());
            if grew {
                assert_eq!(replacement[1], new.as_mut_ptr());
                assert!(map.iter().all(|p| p.is_null()));
            } else {
                assert_eq!(map[2], new.as_mut_ptr());
            }
            STATE.with(|s| {
                assert_eq!(
                    s.borrow().calls,
                    if grew {
                        vec![(object.ptr() as usize, 1, 0), (0, 128, 0)]
                    } else {
                        vec![(0, 128, 0)]
                    }
                )
            });
        }
    }
    #[test]
    fn boundary_append_with_null_finish_and_null_allocation_still_updates_the_iterator() {
        let _allocate = fake(0x0090_CF54, allocate as *const () as usize);
        let mut object = Object::new(0x28);
        let deque = unsafe { CellDeque::from_ptr(object.ptr()) };
        let mut map = [0x1234usize as *mut u8; 4];
        deque.set_map(map.as_mut_ptr());
        deque.set_map_cursor(map.as_mut_ptr());
        deque.set_map_capacity(4);
        STATE.with(|s| *s.borrow_mut() = State::default());
        let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x28) }.to_vec();
        assert_eq!(unsafe { fc4p_006dc360(object.ptr(), &0x4321) }, 128);
        let mut expected = before;
        expected[0x18..0x1C].copy_from_slice(&128u32.to_le_bytes());
        expected[0x1C..0x20].copy_from_slice(&(map.as_mut_ptr().wrapping_add(1) as u32).to_le_bytes());
        assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x28) }, expected);
        assert_eq!(
            map,
            [
                0x1234usize as *mut u8,
                core::ptr::null_mut(),
                0x1234usize as *mut u8,
                0x1234usize as *mut u8
            ]
        );
        STATE.with(|s| assert_eq!(s.borrow().calls, [(0, 128, 0)]));
    }

    extern "thiscall" fn boundary(this: *mut u8, value: *const u16) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push((this as usize, value as u32, 0)));
        0x1234_5678_AABB_CCDD
    }
    #[test]
    fn append_handles_ordinary_null_and_boundary_finish_pointers() {
        let _boundary = fake(0x006D_C360, boundary as *const () as usize);
        let mut object = Object::new(0x28);
        let deque = unsafe { CellDeque::from_ptr(object.ptr()) };
        let mut block = [0xCCu8; 128];
        let value = 0x4321u16;
        let p = &value as *const u16;
        deque.set_cursor(block.as_mut_ptr());
        deque.set_end(block.as_mut_ptr().wrapping_add(128));
        let result = unsafe { fc4p_006dc970(object.ptr(), p) };
        assert_eq!(
            result,
            (u64::from((p as u32 & 0xFFFF_0000) | 0x4321) << 32) | u64::from(block.as_ptr() as u32)
        );
        assert_eq!(&block[..2], &value.to_le_bytes());
        assert_eq!(deque.cursor(), block.as_mut_ptr().wrapping_add(2));
        deque.set_cursor(core::ptr::null_mut());
        let limit = (deque.end() as u32).wrapping_sub(2);
        assert_eq!(
            unsafe { fc4p_006dc970(object.ptr(), core::ptr::null()) },
            u64::from(limit) << 32
        );
        assert_eq!(deque.cursor() as usize, 2);
        deque.set_cursor(block.as_mut_ptr().wrapping_add(126));
        assert_eq!(unsafe { fc4p_006dc970(object.ptr(), p) }, 0x1234_5678_AABB_CCDD);
        STATE.with(|s| {
            assert_eq!(
                core::mem::take(&mut s.borrow_mut().calls),
                [(object.ptr() as usize, p as u32, 0)]
            )
        });
    }
}

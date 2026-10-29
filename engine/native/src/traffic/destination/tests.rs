// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Object};
use crate::traffic::test_support::{invoke, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Lot(i32, i32, u32),
    Holder(u32),
    Rect(u32),
    Pool(u32, i32, i32),
    Connection(u32, i32, i32),
    Get(u32, u32),
    Resize(u32, u32),
    Set(u32, u32, [u32; 9]),
    Free(u32),
    Max(u32, u32),
    Available(u32, u32),
    Capacity(u32, u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    lot: u32,
    connection: u32,
    holder: u32,
    connection_holder: u32,
    maximum: u32,
    arrived: u32,
    get_success: bool,
    values: [u32; 9],
    rect: [i32; 4],
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn lot_fake(_this: *mut u8, x: i32, z: i32, zero: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Lot(x, z, zero));
        s.lot
    })
}
extern "thiscall" fn holder_fake(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Holder(this as u32));
        if this as u32 == s.lot {
            s.holder
        } else {
            s.connection_holder
        }
    })
}
extern "thiscall" fn rect_fake(this: *mut u8, rect: *mut [i32; 4]) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Rect(this as u32));
        unsafe { rect.write_unaligned(s.rect) };
        0
    })
}
extern "thiscall" fn pool_fake(_this: *mut u8, travel: u32, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Pool(travel, x, z));
        s.holder
    })
}
extern "thiscall" fn connection_fake(_this: *mut u8, travel: u32, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Connection(travel, x, z));
        s.connection
    })
}
extern "C" fn get_fake(holder: u32, id: u32, vector: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Get(holder, id));
        if s.get_success {
            unsafe {
                vector.write_unaligned(s.values.as_mut_ptr() as u32);
                vector.add(1).write_unaligned(s.values.as_mut_ptr().add(9) as u32);
                vector.add(2).write_unaligned(s.values.as_mut_ptr().add(9) as u32);
            }
        }
        0xAABB_CC00 | u32::from(s.get_success)
    })
}
extern "thiscall" fn resize_fake(this: *mut u8, count: u32, zero: *const u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Resize(count, unsafe { zero.read_unaligned() }));
        s.values.fill(0);
        unsafe {
            store(this, 0, s.values.as_mut_ptr() as u32);
            store(this, 4, s.values.as_mut_ptr().add(9) as u32);
            store(this, 8, s.values.as_mut_ptr().add(9) as u32);
        }
        0
    })
}
extern "C" fn set_fake(holder: u32, id: u32, vector: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let value = unsafe { read_unaligned((vector.read_unaligned() as *const u32).cast::<[u32; 9]>()) };
        s.calls.push(Call::Set(holder, id, value));
        0
    })
}
extern "C" fn free_fake(ptr: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(ptr)));
    0
}
extern "thiscall" fn max_fake(_this: *mut u8, holder: u32, kind: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Max(holder, kind));
        s.maximum
    })
}
extern "thiscall" fn available_fake(_this: *mut u8, holder: u32, kind: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Available(holder, kind));
        s.arrived
    })
}
extern "C" fn capacity_body(holder: u32, value: u32, index: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Capacity(holder, value, index)));
    0
}
#[unsafe(naked)]
unsafe extern "C" fn capacity_fake() {
    core::arch::naked_asm!("push dword ptr [esp + 4]","push ebx","push edi","call {body}","add esp, 12","ret",body=sym capacity_body);
}

/// The core's t13 lot-capacity example and repeated full-destination calls, with wrapping arrivals,
/// inclusive lot rectangles and the original signed low-word coordinates.
#[test]
fn lot_arrivals_update_capacity_then_clear_the_whole_rectangle_when_full() {
    unsafe {
        let _available = fake(0x0070_DFF0, available_fake as *const () as usize);
        let _capacity = fake(0x0070_DE70, capacity_fake as *const () as usize);
        let sv = table(&[(0x50, max_fake as *const () as usize)]);
        let mv = table(&[(0x18, lot_fake as *const () as usize)]);
        let lv = table(&[
            (0x0C, holder_fake as *const () as usize),
            (0x38, rect_fake as *const () as usize),
        ]);
        let mut manager = [mv.as_ptr() as u32];
        let mut manager_ptr = manager.as_mut_ptr().cast::<u8>();
        let _data = fake_data(0x00B4_3D08, (&mut manager_ptr as *mut *mut u8).cast());
        let mut lot = [lv.as_ptr() as u32];
        let lp = lot.as_mut_ptr() as u32;
        for (count, arrived, maximum, full) in [
            (5u32, 25u32, 30u32, true),
            (4, 25, 30, false),
            (10, u32::MAX - 5, 30, false),
            (0, 30, 30, true),
        ] {
            let mut sim = Object::new(0xD60);
            let p = sim.ptr();
            store(p, 0, sv.as_ptr() as u32);
            let ts = TrafficSim::from_ptr(p);
            ts.set_max_cell_x(9);
            ts.set_max_cell_z(9);
            let mut rows = [0u32; 33];
            let mut cells = [[3u8; 12]; 10];
            for (i, row) in cells.iter_mut().enumerate() {
                rows[(i + 1) * 3] = row.as_mut_ptr() as u32;
            }
            store(p, 0x4C, rows.as_mut_ptr() as u32);
            store(p, 0x1B4, 0);
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    lot: lp,
                    holder: 0x1234,
                    maximum,
                    arrived,
                    rect: [4, 4, 5, 5],
                    ..State::default()
                }
            });
            invoke(
                fc4p_00714740 as *const () as usize,
                p,
                &[count, 0xABCD_0005, 0xFACE_0005, 0x1111_0001, 0x2222_0001],
                0,
                0,
                false,
            );
            assert_eq!(
                take(),
                [
                    Call::Lot(5, 5, 0),
                    Call::Holder(lp),
                    Call::Rect(lp),
                    Call::Max(0x1234, 1),
                    Call::Available(0x1234, 1),
                    Call::Capacity(0x1234, arrived.wrapping_add(count), 5)
                ]
            );
            assert_eq!(word(p, 0x1B4), if full { u32::MAX } else { 0 });
            for (x, row) in cells.iter().enumerate() {
                for (z, &value) in row.iter().enumerate() {
                    assert_eq!(
                        value,
                        if full && (4..=5).contains(&x) && (5..=6).contains(&z) {
                            1
                        } else {
                            3
                        }
                    );
                }
            }
        }
        let mut sim = Object::new(0xD60);
        let p = sim.ptr();
        let ts = TrafficSim::from_ptr(p);
        ts.set_max_cell_x(9);
        ts.set_max_cell_z(9);
        STATE.with(|s| *s.borrow_mut() = State::default());
        invoke(fc4p_00714740 as *const () as usize, p, &[10, 1, 1, 0, 0], 0, 0, false);
        assert_eq!(take(), [Call::Lot(1, 1, 0)]);
    }
}

/// The core's neighbor-destination example: the pool total is mirrored onto the connection holder,
/// with a per-travel array for kinds below three. A missing pool still permits connection accounting.
#[test]
fn neighbor_arrivals_read_or_create_property_then_mirror_capacity_and_clear_boundary_byte() {
    unsafe {
        let _pool = fake(0x0071_14E0, pool_fake as *const () as usize);
        let _connection = fake(0x0071_04A0, connection_fake as *const () as usize);
        let _get = fake(0x005F_F510, get_fake as *const () as usize);
        let _set = fake(0x005F_E6B0, set_fake as *const () as usize);
        let _resize = fake(0x006B_DF10, resize_fake as *const () as usize);
        let _free = fake(0x0090_CF63, free_fake as *const () as usize);
        let _available = fake(0x0070_DFF0, available_fake as *const () as usize);
        let _capacity = fake(0x0070_DE70, capacity_fake as *const () as usize);
        let sv = table(&[(0x50, max_fake as *const () as usize)]);
        let cv = table(&[(0x0C, holder_fake as *const () as usize)]);
        let mut connection = [cv.as_ptr() as u32];
        let cp = connection.as_mut_ptr() as u32;
        for kind in [0u32, 1, 2, 3, 8] {
            for get_success in [true, false] {
                for (holder, conn) in [(0x1234, cp), (0, cp), (0x1234, 0)] {
                    let mut sim = Object::new(0xD60);
                    let p = sim.ptr();
                    store(p, 0, sv.as_ptr() as u32);
                    let ts = TrafficSim::from_ptr(p);
                    ts.set_max_cell_x(9);
                    ts.set_max_cell_z(9);
                    let mut rows = [0u32; 3];
                    let mut cells = [0xFFu8; 12];
                    rows[0] = cells.as_mut_ptr() as u32;
                    store(p, 0x4C, rows.as_mut_ptr() as u32);
                    store(p, 0x1B0 + kind as usize * 4, 7);
                    let values = [0u32, u32::MAX - 3, 0, 0, 0, 0, 0, 0, 0];
                    STATE.with(|s| {
                        *s.borrow_mut() = State {
                            holder,
                            connection: conn,
                            connection_holder: 0x5678,
                            maximum: 50,
                            arrived: 30,
                            get_success,
                            values,
                            ..State::default()
                        }
                    });
                    invoke(
                        fc4p_00714740 as *const () as usize,
                        p,
                        &[25, 0x1234_FFFF, 0x1111_0005, 1, kind],
                        0,
                        0,
                        false,
                    );
                    let mut want = vec![Call::Pool(1, -1, 5), Call::Connection(1, -1, 5)];
                    if conn != 0 {
                        want.push(Call::Holder(cp));
                        if kind < 3 {
                            let id = 0xAA9E_A81A + kind;
                            want.push(Call::Get(0x5678, id));
                            if !get_success {
                                want.push(Call::Resize(9, 0));
                            }
                            let mut values = if get_success { values } else { [0; 9] };
                            values[1] = values[1].wrapping_add(25);
                            want.push(Call::Set(0x5678, id, values));
                            want.push(Call::Free(STATE.with(|s| s.borrow().values.as_ptr() as u32)));
                        }
                    }
                    if holder != 0 {
                        want.extend([
                            Call::Max(holder, kind),
                            Call::Available(holder, kind),
                            Call::Capacity(holder, 55, kind + 4),
                        ]);
                        if conn != 0 {
                            want.push(Call::Capacity(0x5678, 55, kind + 4));
                        }
                    }
                    assert_eq!(take(), want);
                    assert_eq!(word(p, 0x1B0 + kind as usize * 4), if holder != 0 { 6 } else { 7 });
                    assert_eq!(
                        cells[6],
                        if holder != 0 {
                            0xFF & !(1u32.wrapping_shl(kind) as u8)
                        } else {
                            0xFF
                        }
                    );
                }
            }
        }
    }
}

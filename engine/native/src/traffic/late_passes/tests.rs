// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use crate::traffic::test_support::{bytes, invoke, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Lot(u32, i32, i32, u32),
    Holder(u32),
    Pool(u32, u32, i32, i32),
    Max(u32, u32, u32),
    Available(u32, u32, u32),
    Begin(u32),
    Resize(u32, i32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    lot: u32,
    holder: u32,
    maximum: u32,
    arrived: u32,
    first: u32,
    table: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn lot_fake(this: *mut u8, x: i32, z: i32, zero: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Lot(this as u32, x, z, zero));
        s.lot
    })
}
extern "thiscall" fn holder_fake(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Holder(this as u32));
        s.holder
    })
}
extern "thiscall" fn pool_fake(this: *mut u8, kind: u32, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Pool(this as u32, kind, x, z));
        s.holder
    })
}
extern "thiscall" fn max_fake(this: *mut u8, holder: u32, kind: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Max(this as u32, holder, kind));
        s.maximum
    })
}
extern "thiscall" fn available_fake(this: *mut u8, holder: u32, kind: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Available(this as u32, holder, kind));
        s.arrived
    })
}
extern "thiscall" fn begin_fake(this: *mut u8, pair: *mut u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Begin(this as u32));
        unsafe {
            pair.write_unaligned(s.first);
            pair.add(1).write_unaligned(s.table);
        }
    });
    0x9988_7766_AABB_CCDD
}
extern "thiscall" fn resize_fake(this: *mut u8, count: i32, initial: *const u32) -> u64 {
    let zero = unsafe { initial.read_unaligned() };
    STATE.with(|s| s.borrow_mut().calls.push(Call::Resize(this as u32, count, zero)));
    unsafe {
        store(this, 4, word(this, 0).wrapping_add((count as u32).wrapping_mul(4)));
    }
    0x9988_7766_AABB_CCDD
}

/// The core's t13 destination-capacity vectors, expanded at each inclusive map boundary and for
/// unsigned wraparound, a missing lot and a missing holder.
#[test]
fn destination_capacity_observes_bounds_signed_coordinates_and_wrapping_subtraction() {
    unsafe {
        let _pool = fake(0x0071_14E0, pool_fake as *const () as usize);
        let _available = fake(0x0070_DFF0, available_fake as *const () as usize);
        let sv = table(&[(0x50, max_fake as *const () as usize)]);
        let mv = table(&[(0x18, lot_fake as *const () as usize)]);
        let lv = table(&[(0x0C, holder_fake as *const () as usize)]);
        let mut sim = Object::new(0xD60);
        let p = sim.ptr();
        store(p, 0, sv.as_ptr() as u32);
        let s = TrafficSim::from_ptr(p);
        s.set_min_cell_x(2);
        s.set_min_cell_z(3);
        s.set_max_cell_x(8);
        s.set_max_cell_z(9);
        let mut manager = [mv.as_ptr() as u32];
        let mut manager_ptr = manager.as_mut_ptr().cast::<u8>();
        let _data = fake_data(0x00B4_3D08, (&mut manager_ptr as *mut *mut u8).cast());
        let mut lot = [lv.as_ptr() as u32];
        let lp = lot.as_mut_ptr().cast::<u8>();
        let mut trip = [0u8; 36];
        let t = trip.as_mut_ptr();
        t.add(28).write(1);
        for (x, z, inside) in [
            (2i16, 3i16, true),
            (8, 9, true),
            (1, 3, false),
            (2, 2, false),
            (9, 3, false),
            (2, 10, false),
            (-1, 3, false),
        ] {
            write_unaligned(t.add(32).cast::<i16>(), x);
            write_unaligned(t.add(34).cast::<i16>(), z);
            for (maximum, arrived, count, holder, has_lot, want) in [
                (30, 25, 5, 0x1234, true, 1),
                (30, 25, 6, 0x1234, true, 0),
                (0, 1, u32::MAX, 0x1234, true, 1),
                (30, 25, 0, 0, true, 1),
                (30, 25, 1, 0, true, 0),
                (30, 25, 1, 0x1234, false, 0),
            ] {
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.lot = if has_lot { lp as u32 } else { 0 };
                    s.holder = holder;
                    s.maximum = maximum;
                    s.arrived = arrived;
                });
                store(t, 24, count);
                let before = bytes(p, 0xD60);
                let r = invoke(fc4p_007136b0 as *const () as usize, p, &[t as u32], 0, 0, false);
                let resolved = holder != 0 && (!inside || has_lot);
                assert_eq!(r.eax, if !inside && !has_lot { 1 } else { want });
                assert_eq!(r.edx, count);
                assert_eq!(bytes(p, 0xD60), before);
                let mut calls = if inside {
                    vec![Call::Lot(manager_ptr as u32, i32::from(x), i32::from(z), 0)]
                } else {
                    vec![Call::Pool(p as u32, u32::MAX, i32::from(x), i32::from(z))]
                };
                if inside && has_lot {
                    calls.push(Call::Holder(lp as u32));
                }
                if resolved {
                    calls.extend([Call::Max(p as u32, holder, 1), Call::Available(p as u32, holder, 1)]);
                }
                assert_eq!(take(), calls);
            }
        }
    }
}

/// The core's switch-reduction and edge-density-dump vectors, with real bucket/chain layouts.
#[test]
fn maintenance_passes_visit_chains_then_nonempty_buckets_and_keep_untouched_bytes() {
    unsafe {
        let _begin = fake(0x004B_0390, begin_fake as *const () as usize);
        let _fpu = GameFpu::new();
        let mut sim = Object::new(0xD60);
        let p = sim.ptr();
        let mut a = [0xA5u8; 80];
        let a = a.as_mut_ptr();
        let mut b = [0xA5u8; 80];
        let b = b.as_mut_ptr();
        let mut c = [0xA5u8; 80];
        let c = c.as_mut_ptr();
        store(a, 0, b as u32);
        store(a, 4, 0);
        store(b, 0, 0);
        store(b, 4, 0);
        store(c, 0, 0);
        store(c, 4, 3);
        let mut buckets = [a as u32, 0, 0, c as u32];
        let bp = buckets.as_mut_ptr();
        let mut hash = [0, bp as u32, bp.add(4) as u32];
        let h = hash.as_mut_ptr().cast::<u8>();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.first = a as u32;
            s.table = h as u32;
        });
        for factor in [0.0f32, 0.5, 0.1] {
            PassSim::from_ptr(p).set_density_reduction(factor);
            for (n, value) in [(a, 8.0f32), (b, -5.0), (c, 17.25)] {
                store(n, 12, value.to_bits());
            }
            let before = [bytes(a, 80), bytes(b, 80), bytes(c, 80)];
            let r = invoke(fc4p_00713750 as *const () as usize, p, &[0xDEAD], 0, 0, false);
            assert_eq!((r.eax, r.ecx), (1, h as u32));
            assert_eq!(take(), [Call::Begin(p.add(0x70C) as u32)]);
            for ((n, value), mut expected) in [(a, 8.0f32), (b, -5.0), (c, 17.25)].into_iter().zip(before) {
                expected[12..16].copy_from_slice(
                    &multiply_float(F80::from_f32(factor), value)
                        .to_f32()
                        .to_bits()
                        .to_le_bytes(),
                );
                assert_eq!(bytes(n, 80), expected);
            }
        }
        for (n, value) in [(a, 40u16), (b, 0xFFFF), (c, 4095)] {
            for i in 0..16 {
                write_unaligned(n.add(42 + i * 2).cast::<u16>(), if i == 0 { value } else { value / 2 });
            }
        }
        let before = [bytes(a, 80), bytes(b, 80), bytes(c, 80)];
        let r = invoke(fc4p_007137d0 as *const () as usize, p, &[0], 0, 0, false);
        assert_eq!((r.eax, r.ecx), (1, 0));
        assert_eq!(take(), [Call::Begin(p.add(0x178) as u32)]);
        for (n, mut expected) in [a, b, c].into_iter().zip(before) {
            let current: Vec<_> = (0..16)
                .map(|i| read_unaligned(n.add(42 + i * 2).cast::<u16>()))
                .collect();
            let sum = current.iter().map(|&v| u32::from(v)).sum::<u32>().min(0xFFFF) as u16;
            expected.copy_within(42..74, 8);
            expected[40..42].copy_from_slice(&sum.to_le_bytes());
            assert_eq!(bytes(n, 80), expected);
        }
        STATE.with(|s| s.borrow_mut().first = 0);
        for function in [fc4p_00713750 as *const () as usize, fc4p_007137d0 as *const () as usize] {
            let r = invoke(function, p, &[0], 0, 0, false);
            assert_eq!(r.eax, 1);
            assert_eq!(take().len(), 1);
        }
    }
}

/// The core's per-subnetwork totals: subnetwork zero is skipped. Missing keys retain the resized
/// output's existing values; matching keys overwrite just their own element.
#[test]
fn occupant_counts_resize_then_search_unsigned_keys_in_each_subnetwork() {
    unsafe {
        let _resize = fake(0x006D_6920, resize_fake as *const () as usize);
        let mut sim = Object::new(0xD60);
        let p = sim.ptr();
        let s = PassSim::from_ptr(p);
        let mut records = [0u32; 28];
        let rp = records.as_mut_ptr().cast::<u8>();
        let mut headers = [[0u32; 6]; 4];
        let mut nodes = [[0u32; 6]; 3];
        for (i, key) in [0x8000_0000u32, 7, u32::MAX].into_iter().enumerate() {
            nodes[i][4] = key;
            nodes[i][5] = i as u32 + 11;
        }
        headers[1][1] = nodes[0].as_mut_ptr() as u32;
        headers[2][1] = nodes[1].as_mut_ptr() as u32;
        headers[3][1] = nodes[2].as_mut_ptr() as u32;
        for i in 0..4 {
            store(rp, i * 28, headers[i].as_mut_ptr() as u32);
        }
        let mut output = [0xCAFEu32; 4];
        let mut vector = [output.as_mut_ptr() as u32, 0, 0];
        let v = vector.as_mut_ptr().cast::<u8>();
        s.set_subnetworks(rp);
        for count in [0, 1, 4] {
            s.set_subnetworks_end(rp.add(count * 28));
            for kind in [0u32, 7, 0x8000_0000, u32::MAX] {
                output.fill(0xCAFE);
                let before = bytes(p, 0xD60);
                let r = invoke(
                    fc4p_007143b0 as *const () as usize,
                    p,
                    &[kind, v as u32],
                    0x11,
                    0x22,
                    false,
                );
                assert_eq!(r.ecx, p as u32);
                assert_eq!(take(), [Call::Resize(v as u32, count as i32, 0)]);
                assert_eq!(bytes(p, 0xD60), before);
                assert_eq!(output[0], 0xCAFE);
                for (i, entry) in output.iter().enumerate().skip(1) {
                    assert_eq!(
                        *entry,
                        if i < count && nodes[i - 1][4] == kind {
                            nodes[i - 1][5]
                        } else {
                            0xCAFE
                        }
                    );
                }
                if count <= 1 {
                    assert_eq!((r.eax, r.edx), (0xAABB_CCDD, 0x9988_7766));
                }
            }
        }
    }
}

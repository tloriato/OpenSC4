// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Regional push cases from the core, with the original reference-list ownership.

use super::*;
use crate::abi::Conv;
use crate::neighbors::tests::checked_call;
use crate::testing::{fake, fake_data, Fake};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Framework,
    Service,
    ReleaseLocal,
    Allocate(u32),
    Adjacent(u32),
    Current,
    Deals(u32, u32),
    Stats(u32),
    Release(u32),
    Free(usize),
    Destroy(u8, u8, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocator: usize,
    framework: usize,
    current: usize,
    sim: usize,
    cities: Vec<usize>,
    invalidate: bool,
    nodes: Vec<[u32; 3]>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

extern "C" fn framework() -> u64 {
    record(Call::Framework);
    pair(STATE.with(|s| s.borrow().framework) as u32, 1)
}
extern "thiscall" fn service(_this: *mut u8, service: u32, iid: u32, out: *mut usize) -> u64 {
    assert_eq!((service, iid), (0x3AE4_BEA3, 0x3AE4_BEAB));
    record(Call::Service);
    unsafe {
        out.write(STATE.with(|s| s.borrow().allocator));
    }
    pair(1, 2)
}
extern "thiscall" fn release_local(local: *mut usize) -> u64 {
    assert_eq!(unsafe { city_allocator().read() } as usize, unsafe { local.read() });
    record(Call::ReleaseLocal);
    unsafe {
        local.write(0);
    }
    pair(3, 4)
}
extern "thiscall" fn allocate(_this: *mut u8, bytes: u32) -> u64 {
    record(Call::Allocate(bytes));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        pair(s.nodes[0].as_mut_ptr() as u32, 5)
    })
}
extern "thiscall" fn adjacent(this: *mut u8, list: *mut *mut u8, include: u32) -> u64 {
    assert_eq!(this as usize, STATE.with(|s| s.borrow().sim));
    record(Call::Adjacent(include));
    unsafe {
        let header = list.read();
        assert_eq!(pointer(header, 0), header);
        assert_eq!(pointer(header, 4), header);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for i in 0..s.cities.len() {
            let city = s.cities[i];
            let n = s.nodes[i + 1].as_mut_ptr();
            unsafe {
                let h = list.read();
                let prev = pointer(h, 4);
                n.write(h as u32);
                n.add(1).write(prev as u32);
                n.add(2).write(city as u32);
                prev.cast::<u32>().write(n as u32);
                h.add(4).cast::<u32>().write(n as u32);
            }
        }
    });
    pair(6, 7)
}
extern "thiscall" fn current(_this: *mut u8) -> u64 {
    record(Call::Current);
    pair(STATE.with(|s| s.borrow().current) as u32, 8)
}
#[repr(C)]
struct City {
    table: *const usize,
    id: u32,
}
extern "thiscall" fn deals(this: *mut City, list: *mut u32) -> u64 {
    let sim = STATE.with(|s| s.borrow().sim) as *mut u8;
    assert_eq!(list, unsafe { sim.add(0x20).cast() });
    assert_eq!(unsafe { *sim.add(0x3C) }, 0x81);
    assert_eq!(unsafe { *sim.add(0x3D) }, 0x82);
    record(Call::Deals(unsafe { (*this).id }, unsafe { list.read() }));
    unsafe {
        list.write(list.read().wrapping_add(1));
    }
    pair(9, 10)
}
extern "thiscall" fn stats(this: *mut City) -> u64 {
    let sim = STATE.with(|s| s.borrow().sim) as *mut u8;
    assert_eq!(unsafe { *sim.add(0x3D) }, 0x82);
    record(Call::Stats(unsafe { (*this).id }));
    pair(11, 12)
}
extern "thiscall" fn release(this: *mut City) -> u64 {
    record(Call::Release(unsafe { (*this).id }));
    STATE.with(|s| {
        if s.borrow().invalidate {
            unsafe {
                city_allocator().write(core::ptr::null_mut());
            }
        }
    });
    pair(13, 14)
}
extern "thiscall" fn free(_this: *mut u8, node: *mut u8) -> u64 {
    record(Call::Free(node as usize));
    unsafe {
        let prev = pointer(node, 4);
        let next = pointer(node, 0);
        assert_eq!(pointer(prev, 0), next);
        assert_eq!(pointer(next, 4), prev);
    }
    pair(15, 16)
}
extern "fastcall" fn destroy(list: *mut *mut u8, edx: u32) -> u64 {
    unsafe {
        let h = list.read();
        assert_eq!(pointer(h, 0), h);
        assert_eq!(pointer(h, 4), h);
    }
    let sim = STATE.with(|s| s.borrow().sim) as *mut u8;
    record(Call::Destroy(unsafe { *sim.add(0x3C) }, unsafe { *sim.add(0x3D) }, edx));
    pair(0xAABB_CCDD, 0x1122_3344)
}

struct World {
    sim: Box<[u32; 18]>,
    _tables: [Box<[usize; 73]>; 4],
    cities: [Box<City>; 3],
    _allocator: Box<usize>,
    _manager: Box<usize>,
    _framework: Box<usize>,
    _cache: Box<*mut u8>,
    _global: Box<*mut u8>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(cached: bool, cities: bool, invalidate: bool) -> Self {
        let mut tables = [
            Box::new([0usize; 73]),
            Box::new([0; 73]),
            Box::new([0; 73]),
            Box::new([0; 73]),
        ];
        tables[0][0x50 / 4] = adjacent as *const () as usize;
        tables[1][0x114 / 4] = deals as *const () as usize;
        tables[1][0x11C / 4] = stats as *const () as usize;
        tables[1][2] = release as *const () as usize;
        tables[2][3] = allocate as *const () as usize;
        tables[2][4] = free as *const () as usize;
        tables[3][0x14 / 4] = service as *const () as usize;
        tables[3][0x90 / 4] = current as *const () as usize;
        let mut sim = Box::new([0xA5A5_A5A5; 18]);
        sim[0] = tables[0].as_ptr() as u32;
        sim[8] = 100;
        unsafe {
            *sim.as_mut_ptr().cast::<u8>().add(0x3C) = 0x81;
            *sim.as_mut_ptr().cast::<u8>().add(0x3D) = 0x82;
        }
        let mut city_objects = core::array::from_fn(|i| {
            Box::new(City {
                table: tables[1].as_ptr(),
                id: i as u32,
            })
        });
        let mut allocator = Box::new(tables[2].as_ptr() as usize);
        let ap = (&mut *allocator as *mut usize).cast::<u8>();
        let mut manager = Box::new(tables[3].as_ptr() as usize);
        let mut framework_object = Box::new(tables[3].as_ptr() as usize);
        let mut cache = Box::new(if cached { ap } else { core::ptr::null_mut() });
        let mut global = Box::new((&mut *manager as *mut usize).cast::<u8>());
        STATE.with(|s| {
            *s.borrow_mut() = State {
                allocator: ap as usize,
                framework: &mut *framework_object as *mut usize as usize,
                current: &mut *city_objects[0] as *mut City as usize,
                sim: sim.as_mut_ptr() as usize,
                cities: if cities {
                    vec![
                        &mut *city_objects[1] as *mut City as usize,
                        0,
                        &mut *city_objects[2] as *mut City as usize,
                    ]
                } else {
                    vec![]
                },
                invalidate,
                nodes: vec![[0xA5A5_A5A5; 3]; 4],
                ..State::default()
            }
        });
        let fakes = vec![
            fake_data(0x00B2_07E0, (&mut *cache as *mut *mut u8).cast()),
            fake_data(0x00B4_3C94, (&mut *global as *mut *mut u8).cast()),
            fake(0x0087_93EC, framework as *const () as usize),
            fake(0x0041_1870, release_local as *const () as usize),
            fake(0x0041_1FE0, destroy as *const () as usize),
        ];
        Self {
            sim,
            cities: city_objects,
            _tables: tables,
            _allocator: allocator,
            _manager: manager,
            _framework: framework_object,
            _cache: cache,
            _global: global,
            _fakes: fakes,
        }
    }
    fn run(&mut self, deals: u32, stats: u32) -> u64 {
        let mut args = [deals, stats];
        let before = *self.sim;
        let result = unsafe {
            checked_call(
                fc4p_006d0a40 as *const () as usize,
                self.sim.as_mut_ptr().cast(),
                &mut args,
                0xDEAD_BEEF,
                0xCAFE_BABE,
                Conv::Thiscall,
            )
        };
        assert_eq!(args, [deals, stats]);
        let mut expected = before;
        if deals as u8 != 0 {
            expected[8] += if STATE.with(|s| s.borrow().cities.is_empty()) {
                1
            } else {
                3
            };
            expected[15] &= !0xFF;
        }
        if stats as u8 != 0 {
            expected[15] &= !0xFF00;
        }
        assert_eq!(*self.sim, expected);
        result
    }
}

#[test]
fn core_region_push_reverses_adjacent_cities_then_updates_current_and_clears_only_requested_flags() {
    for (deals, stats) in [(1, 0), (0, 0x80), (0xFFFF_FF80, 0xAABB_CC01)] {
        for populated in [false, true] {
            for cached in [false, true] {
                let mut w = World::new(cached, populated, false);
                assert_eq!(w.run(deals, stats), pair(0xAABB_CCDD, 0x1122_3344));
                let mut calls = Vec::new();
                if !cached {
                    calls.extend([Call::Framework, Call::Service, Call::ReleaseLocal]);
                }
                calls.extend([Call::Allocate(12), Call::Adjacent(0), Call::Current]);
                let mut value = 100;
                if populated {
                    for (id, node) in [(2, 3), (u32::MAX, 2), (1, 1)] {
                        if id != u32::MAX {
                            if deals as u8 != 0 {
                                calls.push(Call::Deals(id, value));
                                value += 1;
                            }
                            if stats as u8 != 0 {
                                calls.push(Call::Stats(id));
                            }
                            calls.push(Call::Release(id));
                        }
                        calls.push(Call::Free(STATE.with(|s| s.borrow().nodes[node].as_ptr() as usize)));
                    }
                }
                if deals as u8 != 0 {
                    calls.push(Call::Deals(w.cities[0].id, value));
                }
                if stats as u8 != 0 {
                    calls.push(Call::Stats(0));
                }
                calls.push(Call::Destroy(
                    if deals as u8 != 0 { 0 } else { 0x81 },
                    if stats as u8 != 0 { 0 } else { 0x82 },
                    if stats as u8 != 0 { 12 } else { 10 },
                ));
                assert_eq!(take(), calls);
            }
        }
    }
}

#[test]
fn high_flag_bytes_alone_do_nothing_and_preserve_return_registers() {
    let mut w = World::new(false, true, false);
    assert_eq!(w.run(0x1234_5600, 0xFFFF_FF00), pair(0xDEAD_BE00, 0xCAFE_BABE));
    assert_eq!(take(), []);
}

#[test]
fn allocator_cache_is_reloaded_after_each_city_release() {
    let mut w = World::new(true, true, true);
    assert_eq!(w.run(0, 1), pair(0xAABB_CCDD, 0x1122_3344));
    let calls = take();
    assert_eq!(calls.iter().filter(|c| **c == Call::Framework).count(), 2);
    assert!(calls
        .windows(5)
        .filter(|v| matches!(v[0], Call::Release(_)))
        .all(|v| matches!(
            v,
            [
                Call::Release(_),
                Call::Framework,
                Call::Service,
                Call::ReleaseLocal,
                Call::Free(_)
            ]
        )));
}

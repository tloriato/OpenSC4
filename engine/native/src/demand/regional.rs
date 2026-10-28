// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Replacing the connected-city list and applying its population differences.

use super::queries::{add_float, subtract_from_float};
use super::*;
use crate::x87::F80;

game_data! { static allocator: *mut u8 = 0x00B2_07E0; }
game_data! { static application: *mut u8 = 0x00B4_3C94; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_reference(reference: *mut u32) = "thiscall" 0x0041_1870; }
game_fn! { fn clear_city_list(list: *mut *mut u8) = "thiscall" 0x0041_1DF0; }
game_fn! { fn find_node(map: *mut u8, output: *mut *mut u8, key: *const u32) = "thiscall" 0x0049_7520; }

layout! {
    /// Circular connected-city list, including its header.
    struct CityLink {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => city / set_city: *mut u8,
    }
}

unsafe fn load_allocator(temporary: *mut u32) -> *mut u8 {
    // SAFETY: the game global and framework expose the allocator's normal interface.
    unsafe {
        if read_unaligned(allocator()).is_null() {
            write_unaligned(temporary, 0);
            let framework = framework();
            if !framework.is_null() {
                vcall!(framework, 0x14 => fn(u32, u32, *mut u32), 0x3AE4_BEA3, 0x3AE4_BEAB, temporary);
            }
            write_unaligned(allocator(), read_unaligned(temporary) as *mut u8);
            release_reference(temporary);
        }
        read_unaligned(allocator())
    }
}

unsafe fn sum_previous(sim: DemandSim, me: *mut u8, key: u32) -> f32 {
    // SAFETY: the simulator owns a circular list of live regional cities.
    unsafe {
        let mut link = CityLink::from_ptr(sim.previous_cities()).next();
        let mut sum = 0.0f32;
        while link != sim.previous_cities() {
            let city = CityLink::from_ptr(link).city();
            if city != me {
                let population = vcall!(city, 0x44 => fn(u32) -> i32, key);
                sum = add_float(F80::from_i32(population), sum).to_f32();
            }
            link = CityLink::from_ptr(link).next();
        }
        sum
    }
}

port! {
    /// Gather the new connected-city and export lists. For each region-use-1/2 node with a regional
    /// counterpart, sum the previous cities before the new cities, excluding this city's own entry.
    /// Initial updates replace committed plus pending totals; later updates apply the list difference.
    /// Swap in the new list, clear and release the old list, retaining the allocator's return registers.
    #[port(address = 0x006A_5CB0, owner = "demand", name = "cSC4DemandSimulator::ResetRegionalDemands", naked)]
    pub unsafe extern "C" fn fc4p_006a5cb0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 4",
            body = sym reset,
        )
    }
}

unsafe extern "fastcall" fn reset(this: *mut u8, entry: *mut Entry) -> u64 {
    // SAFETY: the original caller supplies the simulator, argument word, lists and service objects.
    unsafe {
        let entry = &mut *entry;
        let sim = DemandSim::from_ptr(this);
        let mut temporary = 0u32;
        let alloc = load_allocator(&mut temporary);
        let mut new_list = vcall!(alloc, 0x0C => fn(u32) -> *mut u8, 12);
        CityLink::from_ptr(new_list).set_next(new_list);
        CityLink::from_ptr(new_list).set_previous(new_list);
        vcall!(sim.neighbors(), 0x58 => fn(*mut *mut u8, u32), &mut new_list, 0);
        vcall!(sim.neighbors(), 0x54 => fn(*mut *mut u8, u32), this.add(0x9C).cast(), 0);
        let me = vcall!(read_unaligned(application()), 0x90 => fn() -> *mut u8);
        let mut node = TreeNode::from_ptr(sim.nodes()).left();
        while node != sim.nodes() {
            let tree = TreeNode::from_ptr(node);
            let object = tree.value() as *mut u8;
            let key = tree.key();
            let kind = vcall!(object, 0x70 => fn() -> i32);
            if kind == 1 || kind == 2 {
                let regional_key = vcall!(object, 0x18 => fn() -> u32).wrapping_add(0x10000);
                let mut regional = core::ptr::null_mut();
                find_node(this.add(0x50), &mut regional, &regional_key);
                if regional != sim.nodes() {
                    let regional = TreeNode::from_ptr(regional).value() as *mut u8;
                    let previous = sum_previous(sim, me, key);
                    let mut link = CityLink::from_ptr(new_list).next();
                    let mut sum = 0.0f32;
                    while link != new_list {
                        let city = CityLink::from_ptr(link).city();
                        if city != me {
                            let population = vcall!(city, 0x44 => fn(u32) -> i32, key);
                            sum = add_float(F80::from_i32(population), sum).to_f32();
                        }
                        link = CityLink::from_ptr(link).next();
                    }
                    let mut delta = (F80::from_f32(sum) - F80::from_f32(previous)).to_f32();
                    if entry.arg(0) as u8 != 0 {
                        let pending = vcall_st0!(regional, if kind == 1 { 0x28 } else { 0x2C }).to_f32();
                        let committed = vcall_st0!(regional, if kind == 1 { 0x20 } else { 0x24 });
                        delta = subtract_from_float(add_float(committed, pending), sum).to_f32();
                    }
                    vcall!(regional, if kind == 1 { 0x40 } else { 0x44 } => fn(u32), delta.to_bits());
                }
            }
            node = successor(node);
        }
        let mut old_list = sim.previous_cities();
        sim.set_previous_cities(new_list);
        clear_city_list(&mut old_list);
        let to_free = old_list;
        let alloc = load_allocator(entry.args.as_mut_ptr());
        vcall!(alloc, 0x10 => fn(*mut u8) -> u64, to_free)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a5cb0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map, test_support::Probe};
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Framework,
        Query(u32, u32),
        Release(u32),
        Allocate(u32),
        AllCities,
        Exports,
        OwnCity,
        Kind(u32),
        Id(u32),
        Find(u32),
        Population(u32, u32),
        Pending(u32),
        Committed(u32),
        Add(u32, u32),
        Clear(u32),
        Free(u32),
    }
    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        sim: u32,
        neighbors: u32,
        app: u32,
        allocator: u32,
        factory: u32,
        allocator_global: usize,
        clear_allocator: bool,
        new_head: u32,
        new_first: u32,
        new_last: u32,
        export_head: u32,
        me: u32,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    extern "C" fn get_framework() -> *mut u8 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Framework);
            s.factory as *mut u8
        })
    }
    extern "thiscall" fn query(this: *mut u8, class: u32, iid: u32, output: *mut u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.factory);
            assert_eq!(unsafe { *output }, 0);
            s.calls.push(Call::Query(class, iid));
            unsafe {
                *output = s.allocator;
            }
        });
    }
    extern "thiscall" fn release(reference: *mut u32) {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Release(unsafe { *reference })));
    }
    extern "thiscall" fn allocate(this: *mut u8, size: u32) -> *mut u8 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.allocator);
            s.calls.push(Call::Allocate(size));
            s.new_head as *mut u8
        })
    }
    extern "thiscall" fn all_cities(this: *mut u8, output: *mut u32, flags: u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.neighbors);
            assert_eq!(flags, 0);
            s.calls.push(Call::AllCities);
            let header = unsafe { *output };
            assert_eq!(header, s.new_head);
            let link = unsafe { CityLink::from_ptr(header as *mut u8) };
            assert_eq!(link.next() as u32, header);
            assert_eq!(link.previous() as u32, header);
            assert_eq!(link.city() as u32, 0xA5A5_A5A5);
            link.set_next(s.new_first as *mut u8);
            link.set_previous(s.new_last as *mut u8);
        });
    }
    extern "thiscall" fn exports(this: *mut u8, output: *mut u32, flags: u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.neighbors);
            assert_eq!(flags, 0);
            assert_eq!(output as u32, s.sim + 0x9C);
            s.calls.push(Call::Exports);
            unsafe {
                *output = s.export_head;
            }
        });
    }
    extern "thiscall" fn own(this: *mut u8) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.app);
            s.calls.push(Call::OwnCity);
            s.me
        })
    }
    extern "thiscall" fn kind(p: *mut u8) -> i32 {
        let p = p.cast::<u32>();
        let (key, kind) = unsafe { (*p.add(1), *p.add(2)) };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Kind(key)));
        kind as i32
    }
    extern "thiscall" fn id(p: *mut u8) -> u32 {
        let p = p.cast::<u32>();
        let (key, id) = unsafe { (*p.add(1), *p.add(3)) };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Id(key)));
        id
    }
    extern "thiscall" fn find(map: *mut u8, output: *mut *mut u8, key: *const u32) {
        unsafe {
            let key = *key;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                assert_eq!(map as u32, s.sim + 0x50);
                s.calls.push(Call::Find(key));
            });
            let head = read_unaligned(map.cast::<*mut u8>());
            let mut node = TreeNode::from_ptr(head).left();
            while node != head && TreeNode::from_ptr(node).key() != key {
                node = successor(node);
            }
            *output = node;
        }
    }
    extern "thiscall" fn population(p: *mut u8, key: u32) -> i32 {
        let p = p.cast::<u32>();
        let id = unsafe { *p.add(1) };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Population(id, key)));
        unsafe { *p.add(if key == 2 { 2 } else { 3 }) as i32 }
    }
    extern "thiscall" fn pending_supply(p: *mut u8) -> f32 {
        pending(p, 0)
    }
    extern "thiscall" fn pending_demand(p: *mut u8) -> f32 {
        pending(p, 1)
    }
    fn pending(p: *mut u8, which: usize) -> f32 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Pending(which as u32)));
        f32::from_bits(unsafe { *p.cast::<u32>().add(4 + which) })
    }
    extern "thiscall" fn supply(p: *mut u8) -> f32 {
        committed(p, 0)
    }
    extern "thiscall" fn demand(p: *mut u8) -> f32 {
        committed(p, 1)
    }
    fn committed(p: *mut u8, which: usize) -> f32 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Committed(which as u32)));
        f32::from_bits(unsafe { *p.cast::<u32>().add(6 + which) })
    }
    extern "thiscall" fn add_supply(p: *mut u8, bits: u32) {
        add(p, bits, 0);
    }
    extern "thiscall" fn add_demand(p: *mut u8, bits: u32) {
        add(p, bits, 1);
    }
    fn add(_p: *mut u8, bits: u32, which: u32) {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Add(which, bits)));
    }
    extern "thiscall" fn clear(output: *mut *mut u8) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(
                unsafe { DemandSim::from_ptr(s.sim as *mut u8) }.previous_cities() as u32,
                s.new_head
            );
            s.calls.push(Call::Clear(unsafe { *output } as u32));
            if s.clear_allocator {
                unsafe {
                    *(s.allocator_global as *mut u32) = 0;
                }
            }
        });
        let header = unsafe { CityLink::from_ptr(*output) };
        header.set_next(header.as_ptr());
        header.set_previous(header.as_ptr());
    }
    extern "thiscall" fn free(this: *mut u8, pointer: *mut u8) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.allocator);
            s.calls.push(Call::Free(pointer as u32));
        });
        0x7654_3210_AABB_CCDD
    }
    struct List {
        links: Vec<[u32; 3]>,
    }
    impl List {
        fn new(cities: &[u32]) -> Self {
            let mut links = vec![[0xA5A5_A5A5; 3]; cities.len() + 1];
            let pointers: Vec<_> = links.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for i in 0..links.len() {
                links[i][0] = pointers[(i + 1) % links.len()];
                links[i][1] = pointers[(i + links.len() - 1) % links.len()];
                if i != 0 {
                    links[i][2] = cities[i - 1];
                }
            }
            Self { links }
        }
        fn head(&mut self) -> u32 {
            self.links[0].as_mut_ptr() as u32
        }
    }
    struct Fixture {
        memory: Object,
        old: List,
        new: List,
        _tables: Vec<[usize; 40]>,
        _nodes: Vec<[u32; 8]>,
        _cities: Vec<[u32; 4]>,
        _services: Vec<[u32; 1]>,
        _map: Map,
        _allocator: Box<u32>,
        _app: Box<u32>,
        _guards: Vec<Fake>,
    }
    impl Fixture {
        fn new(empty: bool, initial_allocator: bool, clear_allocator: bool) -> Self {
            let table = |entries: &[(usize, usize)]| {
                let mut t = [0usize; 40];
                for &(slot, f) in entries {
                    t[slot / 4] = f;
                }
                t
            };
            let mut tables = vec![
                table(&[
                    (0x0C, allocate as *const () as usize),
                    (0x10, free as *const () as usize),
                ]),
                table(&[
                    (0x58, all_cities as *const () as usize),
                    (0x54, exports as *const () as usize),
                ]),
                table(&[(0x90, own as *const () as usize)]),
                table(&[(0x14, query as *const () as usize)]),
                table(&[
                    (0x70, kind as *const () as usize),
                    (0x18, id as *const () as usize),
                    (0x20, supply as *const () as usize),
                    (0x24, demand as *const () as usize),
                    (0x28, pending_supply as *const () as usize),
                    (0x2C, pending_demand as *const () as usize),
                    (0x40, add_supply as *const () as usize),
                    (0x44, add_demand as *const () as usize),
                ]),
                table(&[(0x44, population as *const () as usize)]),
            ];
            let mut services: Vec<_> = (0..4).map(|i| [tables[i].as_mut_ptr() as u32]).collect();
            let pointers: Vec<_> = services.iter_mut().map(|o| o.as_mut_ptr() as u32).collect();
            let mut cities: Vec<_> = [(1, 10i32, 15), (2, -2, 7), (3, 30, 50), (9, 999, 999)]
                .into_iter()
                .map(|(id, r, j)| [tables[5].as_mut_ptr() as u32, id, r as u32, j as u32])
                .collect();
            let cp: Vec<_> = cities.iter_mut().map(|c| c.as_mut_ptr() as u32).collect();
            let mut old = List::new(&[cp[0], cp[3], cp[1]]);
            let mut new = List::new(&[cp[3], cp[1], cp[2]]);
            let specs = if empty {
                vec![]
            } else {
                vec![
                    (1, 0, 1),
                    (2, 1, 20),
                    (3, 2, 3),
                    (4, 3, 4),
                    (5, 1, 5),
                    (0x10003, 0, 0x10003),
                    (0x10014, 0, 0x10014),
                ]
            };
            let mut nodes: Vec<_> = specs
                .iter()
                .map(|&(key, kind, id)| {
                    [
                        tables[4].as_mut_ptr() as u32,
                        key,
                        kind,
                        id,
                        3.0f32.to_bits(),
                        4.0f32.to_bits(),
                        5.0f32.to_bits(),
                        6.0f32.to_bits(),
                    ]
                })
                .collect();
            let entries: Vec<_> = specs
                .iter()
                .zip(nodes.iter_mut())
                .map(|(&(key, _, _), n)| (key, n.as_mut_ptr() as u32))
                .collect();
            let mut map = Map::new(&entries);
            let mut memory = Object::new(0xBC);
            let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
            sim.set_neighbors(pointers[1] as *mut u8);
            sim.set_nodes(map.header());
            sim.set_previous_cities(old.head() as *mut u8);
            sim.set_export_cities(0x5555usize as *mut u8);
            let mut allocator = Box::new(if initial_allocator { pointers[0] } else { 0 });
            let mut app = Box::new(pointers[2]);
            let guards = vec![
                fake(0x0087_93EC, get_framework as *const () as usize),
                fake(0x0041_1870, release as *const () as usize),
                fake(0x0041_1DF0, clear as *const () as usize),
                fake(0x0049_7520, find as *const () as usize),
                fake_data(0x00B2_07E0, (&mut *allocator as *mut u32).cast()),
                fake_data(0x00B4_3C94, (&mut *app as *mut u32).cast()),
            ];
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    calls: vec![],
                    sim: sim.as_ptr() as u32,
                    neighbors: pointers[1],
                    app: pointers[2],
                    allocator: pointers[0],
                    factory: pointers[3],
                    allocator_global: &mut *allocator as *mut u32 as usize,
                    clear_allocator,
                    new_head: new.head(),
                    new_first: new.links[0][0],
                    new_last: new.links[0][1],
                    export_head: 0x1234_5678,
                    me: cp[3],
                }
            });
            Self {
                memory,
                old,
                new,
                _tables: tables,
                _nodes: nodes,
                _cities: cities,
                _services: services,
                _map: map,
                _allocator: allocator,
                _app: app,
                _guards: guards,
            }
        }
        fn run(&mut self, initial: u32) -> Probe {
            let mut before = unsafe { core::slice::from_raw_parts(self.memory.ptr(), 0xBC).to_vec() };
            let mut call = Probe::new(fc4p_006a5cb0 as *const (), self.memory.ptr(), [initial, 77, 88]);
            unsafe {
                call.run();
            }
            assert_eq!((call.eax, call.edx), (0xAABB_CCDD, 0x7654_3210));
            before[0x98..0x9C].copy_from_slice(&self.new.head().to_le_bytes());
            before[0x9C..0xA0].copy_from_slice(&0x1234_5678u32.to_le_bytes());
            assert_eq!(unsafe { core::slice::from_raw_parts(self.memory.ptr(), 0xBC) }, before);
            call
        }
    }

    #[test]
    fn regional_reset_sums_old_before_new_skips_self_and_selects_supply_or_demand() {
        let _fpu = GameFpu::new();
        for initial in [0, 1, 0x100, 0xFF] {
            let mut f = Fixture::new(false, true, false);
            let call = f.run(initial);
            assert_eq!(call.after, [initial, 77, 88]);
            let mut expected = vec![
                Call::Allocate(12),
                Call::AllCities,
                Call::Exports,
                Call::OwnCity,
                Call::Kind(1),
                Call::Kind(2),
                Call::Id(2),
                Call::Find(0x10014),
                Call::Population(1, 2),
                Call::Population(2, 2),
                Call::Population(2, 2),
                Call::Population(3, 2),
            ];
            if initial as u8 != 0 {
                expected.extend([Call::Pending(0), Call::Committed(0)]);
            }
            expected.extend([
                Call::Add(0, 20.0f32.to_bits()),
                Call::Kind(3),
                Call::Id(3),
                Call::Find(0x10003),
                Call::Population(1, 3),
                Call::Population(2, 3),
                Call::Population(2, 3),
                Call::Population(3, 3),
            ]);
            if initial as u8 != 0 {
                expected.extend([Call::Pending(1), Call::Committed(1)]);
            }
            expected.extend([
                Call::Add(1, (if initial as u8 != 0 { 47.0f32 } else { 35.0 }).to_bits()),
                Call::Kind(4),
                Call::Kind(5),
                Call::Id(5),
                Call::Find(0x10005),
                Call::Kind(0x10003),
                Call::Kind(0x10014),
                Call::Clear(f.old.head()),
                Call::Free(f.old.head()),
            ]);
            STATE.with(|s| assert_eq!(s.borrow().calls, expected));
        }
    }

    #[test]
    fn empty_node_maps_still_replace_lists_and_both_lazy_allocator_paths_use_the_game_factory() {
        for initial_allocator in [false, true] {
            for clear_allocator in [false, true] {
                let mut f = Fixture::new(true, initial_allocator, clear_allocator);
                let call = f.run(1);
                let alloc = *f._allocator;
                assert_eq!(call.after, [if clear_allocator { alloc } else { 1 }, 77, 88]);
                let initialization = || {
                    vec![
                        Call::Framework,
                        Call::Query(0x3AE4_BEA3, 0x3AE4_BEAB),
                        Call::Release(alloc),
                    ]
                };
                let mut expected = if initial_allocator { vec![] } else { initialization() };
                expected.extend([
                    Call::Allocate(12),
                    Call::AllCities,
                    Call::Exports,
                    Call::OwnCity,
                    Call::Clear(f.old.head()),
                ]);
                if clear_allocator {
                    expected.extend(initialization());
                }
                expected.push(Call::Free(f.old.head()));
                STATE.with(|s| assert_eq!(s.borrow().calls, expected));
            }
        }
    }

    #[test]
    fn integer_city_counts_are_loaded_exactly_before_each_stored_float_sum() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(false, true, false);
        f._cities[0][2] = 16_777_216;
        f._cities[1][2] = (-16_777_216i32) as u32;
        f._cities[2][2] = 16_777_217;
        f.run(0);
        STATE.with(|s| assert!(s.borrow().calls.contains(&Call::Add(0, 1.0f32.to_bits()))));
    }
}

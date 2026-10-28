// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Connection, airport and seaport demand-cap relief.

use super::queries::{add_float, multiply_float};
use super::*;
use crate::x87::F80;

game_data! { static connection_allocator: *mut u8 = 0x00B2_07DC; }
game_data! { static city_allocator: *mut u8 = 0x00B2_07E0; }
game_data! { static traffic: *mut u8 = 0x00B4_3DA8; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static logarithm_scale: f32 = 0x00AB_0E80; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_reference(reference: *mut u32) = "thiscall" 0x0041_1870; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(pointer: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn insert_city(set: *mut u32, result: *mut u32, key: *const u32) = "thiscall" 0x0049_A050; }
game_fn! { fn destroy_subtree(set: *mut u32, root: *mut u8) = "thiscall" 0x004A_4660; }
game_fn! { fn clear_connection_list(list: *mut *mut u8) = "thiscall" 0x0041_1D70; }
game_fn! { fn drop_connection_list(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_1F80; }
game_fn! { fn drop_city_list(list: *mut *mut u8) = "thiscall" 0x0041_1FE0; }
game_fn! { fn map_value(map: *mut u8, key: *const u32) -> *mut *mut u8 = "thiscall" 0x0049_7A00; }

layout! {
    /// Circular list links with either a connection or a regional city as payload.
    struct Link {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => object / set_object: *mut u8,
    }
}

unsafe fn get_allocator(global: *mut *mut u8) -> *mut u8 {
    // SAFETY: the framework supplies the allocator interface cached at the specified game global.
    unsafe {
        if read_unaligned(global).is_null() {
            let mut temporary = 0u32;
            let factory = framework();
            if !factory.is_null() {
                vcall!(factory, 0x14 => fn(u32, u32, *mut u32), 0x3AE4_BEA3, 0x3AE4_BEAB, &mut temporary);
            }
            write_unaligned(global, temporary as *mut u8);
            release_reference(&mut temporary);
        }
        read_unaligned(global)
    }
}

unsafe fn new_list(global: *mut *mut u8) -> *mut u8 {
    unsafe {
        let list = vcall!(get_allocator(global), 0x0C => fn(u32) -> *mut u8, 12);
        let link = Link::from_ptr(list);
        link.set_next(list);
        link.set_previous(list);
        list
    }
}

fn log_count(count: i32) -> F80 {
    let mut output = F80::ZERO;
    // SAFETY: two x87 values are pushed, combined with FYL2X, then popped to the output.
    unsafe {
        core::arch::asm!(
            "fild dword ptr [{count}]", "fldln2", "fxch st(1)", "fyl2x", "fstp tbyte ptr [{output}]",
            count = in(reg) &count, output = in(reg) &mut output, out("st(0)") _, out("st(1)") _,
        );
    }
    output
}

fn add_term(sum: F80, term: F80) -> F80 {
    let mut output = F80::ZERO;
    // SAFETY: the original operand order is retained, including when both operands are NaNs.
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{sum}]", "fld tbyte ptr [{term}]", "faddp st(1), st(0)", "fstp tbyte ptr [{output}]",
            sum = in(reg) &sum, term = in(reg) &term, output = in(reg) &mut output, out("st(0)") _, out("st(1)") _,
        );
    }
    output
}

fn subtract_float(value: F80, operand: f32) -> F80 {
    let mut output = F80::ZERO;
    // SAFETY: the old relief is a stored operand, preserving signaling-NaN handling.
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fsub dword ptr [{operand}]", "fstp tbyte ptr [{output}]",
            value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output, out("st(0)") _,
        );
    }
    output
}

unsafe fn unsigned(value: u32) -> F80 {
    let integer = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        integer
    } else {
        unsafe { add_float(integer, read_unaligned(unsigned_bias())) }
    }
}

unsafe fn list_count(list: *mut u8) -> i32 {
    unsafe {
        let mut link = Link::from_ptr(list).next();
        let mut count = 0i32;
        while link != list {
            count = count.wrapping_add(1);
            link = Link::from_ptr(link).next();
        }
        count
    }
}

struct CitySet([u32; 3]);
impl CitySet {
    unsafe fn new() -> Self {
        unsafe {
            let header = allocate(20);
            let node = TreeNode::from_ptr(header);
            node.set_color(0);
            node.set_parent(core::ptr::null_mut());
            node.set_left(header);
            node.set_right(header);
            Self([header as u32, 0, 0])
        }
    }
    unsafe fn contains(&self, key: u32) -> bool {
        unsafe {
            let header = self.0[0] as *mut u8;
            let mut found = header;
            let mut node = TreeNode::from_ptr(header).parent();
            while !node.is_null() {
                let view = TreeNode::from_ptr(node);
                if view.key() >= key {
                    found = node;
                    node = view.left();
                } else {
                    node = view.right();
                }
            }
            found != header && key >= TreeNode::from_ptr(found).key()
        }
    }
    unsafe fn insert(&mut self, key: &u32) {
        let mut result = [0u32; 2];
        unsafe {
            insert_city(self.0.as_mut_ptr(), result.as_mut_ptr(), key);
        }
    }
    unsafe fn fill(&mut self, list: *mut u8, excluded: &[&CitySet]) {
        unsafe {
            let mut link = Link::from_ptr(list).next();
            while link != list {
                let city = vcall!(Link::from_ptr(link).object(), 0x24 => fn() -> u32);
                // Exclusion requires membership in every preceding set.
                if excluded.is_empty() || !excluded.iter().all(|set| set.contains(city)) {
                    self.insert(&city);
                }
                link = Link::from_ptr(link).next();
            }
        }
    }
    unsafe fn clear(&mut self, whole_subtree: bool) {
        unsafe {
            if self.0[1] != 0 {
                let header = self.0[0] as *mut u8;
                let root = TreeNode::from_ptr(header).parent();
                if whole_subtree {
                    destroy_subtree(self.0.as_mut_ptr(), root);
                } else {
                    let mut node = root;
                    while !node.is_null() {
                        destroy_subtree(self.0.as_mut_ptr(), TreeNode::from_ptr(node).right());
                        let left = TreeNode::from_ptr(node).left();
                        free(node);
                        node = left;
                    }
                }
                TreeNode::from_ptr(self.0[0] as *mut u8).set_left(self.0[0] as *mut u8);
                TreeNode::from_ptr(self.0[0] as *mut u8).set_parent(core::ptr::null_mut());
                TreeNode::from_ptr(self.0[0] as *mut u8).set_right(self.0[0] as *mut u8);
                self.0[1] = 0;
            }
            if self.0[0] != 0 {
                free(self.0[0] as *mut u8);
            }
        }
    }
    fn count(&self) -> i32 {
        self.0[1] as i32
    }
}

unsafe fn list_by_type(sim: DemandSim, list: &mut *mut u8, kind: u32) {
    unsafe {
        vcall!(sim.neighbors(), 0x44 => fn(u32, *mut *mut u8), kind, list);
    }
}

port! {
    /// Recompute industrial relief from arrived freight trips and both relief totals from distinct-city
    /// and connection counts. Group weights multiply natural logarithms before a shared scale factor.
    /// Airports and seaports add their trip contributions. Apply industrial deltas to 0x4800 and 0x4900,
    /// then the commercial delta to 0x3800. All temporary lists and sets use the game's allocators.
    #[port(address = 0x006A_3ED0, owner = "demand", name = "cSC4DemandSimulator::EvaluateConnectionUtilization")]
    pub unsafe extern "thiscall" fn fc4p_006a3ed0(this: *mut u8) -> u64 {
        // SAFETY: the simulator supplies the live services, circular lists, demand map and tuning array.
        unsafe {
            let sim = DemandSim::from_ptr(this);
            let old_i = f32::from_bits(sim.industrial_relief());
            let old_c = f32::from_bits(sim.commercial_relief());
            sim.set_industrial_relief(0);
            sim.set_commercial_relief(0);
            let mut all = new_list(connection_allocator());
            vcall!(sim.neighbors(), 0x40 => fn(*mut *mut u8), &mut all);
            let mut link = Link::from_ptr(all).next();
            while link != all {
                let connection = Link::from_ptr(link).object();
                let kind = vcall!(connection, 0x14 => fn() -> u32);
                if matches!(kind, 1 | 2 | 3 | 8 | 10 | 12) {
                    let traffic = read_unaligned(traffic());
                    let target = vslot(traffic, 0x54);
                    let connector = vcall!(connection, 0x0C => fn(u32) -> u32, 3);
                    let arrived = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32) -> u32>(target)(traffic, connector);
                    let relief = add_float(unsigned(arrived), f32::from_bits(sim.industrial_relief())).to_f32();
                    sim.set_industrial_relief(relief.to_bits());
                }
                link = Link::from_ptr(link).next();
            }
            sim.set_industrial_relief(multiply_float(F80::from_f32(sim.relief_per_trip()), f32::from_bits(sim.industrial_relief())).to_f32().to_bits());
            let mut cities = new_list(city_allocator());
            vcall!(sim.neighbors(), 0x54 => fn(*mut *mut u8, u32), &mut cities, 0);
            list_count(cities);

            let mut highway = new_list(connection_allocator());
            let mut highway_cities = CitySet::new();
            list_by_type(sim, &mut highway, 3);
            highway_cities.fill(highway, &[]);
            let mut ground = new_list(connection_allocator());
            list_by_type(sim, &mut ground, 12);
            highway_cities.fill(ground, &[]);
            let n_highway = highway_cities.count().wrapping_add(list_count(highway)).max(1);

            let mut avenue = new_list(connection_allocator());
            let mut avenue_cities = CitySet::new();
            list_by_type(sim, &mut avenue, 8);
            avenue_cities.fill(avenue, &[&highway_cities]);
            let n_avenue = avenue_cities.count().wrapping_add(list_count(avenue)).max(1);

            let mut road = new_list(connection_allocator());
            let mut road_cities = CitySet::new();
            list_by_type(sim, &mut road, 2);
            road_cities.fill(road, &[&highway_cities, &avenue_cities]);
            let n_road = road_cities.count().wrapping_add(list_count(road)).max(1);

            let mut dirt = new_list(connection_allocator());
            let mut dirt_cities = CitySet::new();
            list_by_type(sim, &mut dirt, 10);
            dirt_cities.fill(dirt, &[&highway_cities, &avenue_cities, &road_cities]);
            let n_dirt = dirt_cities.count().wrapping_add(list_count(dirt)).max(1);

            let mut rail = new_list(connection_allocator());
            let mut rail_cities = CitySet::new();
            list_by_type(sim, &mut rail, 1);
            rail_cities.fill(rail, &[]);
            let n_rail = rail_cities.count().wrapping_add(list_count(rail)).max(1);

            let mut light = new_list(connection_allocator());
            let mut transit_cities = CitySet::new();
            list_by_type(sim, &mut light, 7);
            transit_cities.fill(light, &[]);
            let mut mono = new_list(connection_allocator());
            list_by_type(sim, &mut mono, 11);
            transit_cities.fill(mono, &[]);
            let mut subway = new_list(connection_allocator());
            list_by_type(sim, &mut subway, 6);
            transit_cities.fill(subway, &[]);
            let n_transit = transit_cities.count().wrapping_add(list_count(subway)).wrapping_add(list_count(light)).max(1);

            let weights = sim.network_relief();
            let city = sim.city();
            let mut commercial = multiply_float(log_count(n_transit), read_unaligned(weights.add(7)));
            commercial = add_term(commercial, multiply_float(log_count(n_rail), read_unaligned(weights.add(1))));
            let log = log_count(n_dirt); let _stored = log.to_f32();
            commercial = add_term(commercial, multiply_float(log, read_unaligned(weights.add(11))));
            let log = log_count(n_avenue); let _stored = log.to_f32();
            commercial = add_term(commercial, multiply_float(log, read_unaligned(weights.add(6))));
            let log = log_count(n_highway); let _stored = log.to_f32();
            commercial = add_term(commercial, multiply_float(log, read_unaligned(weights.add(2))));
            let log = log_count(n_road); let _stored = log.to_f32();
            commercial = add_term(commercial, multiply_float(log, read_unaligned(weights)));
            commercial = add_float(multiply_float(commercial, read_unaligned(logarithm_scale())), f32::from_bits(sim.commercial_relief()));
            sim.set_commercial_relief(commercial.to_f32().to_bits());

            let airport = vcall!(city, 0x134 => fn() -> *mut u8);
            let air = unsigned(vcall!(airport, 0x34 => fn() -> u32));
            let city = sim.city();
            sim.set_commercial_relief(add_float(multiply_float(air, sim.airport_commercial()), f32::from_bits(sim.commercial_relief())).to_f32().to_bits());
            sim.set_industrial_relief(add_float(multiply_float(air, sim.airport_industrial()), f32::from_bits(sim.industrial_relief())).to_f32().to_bits());
            let seaport = vcall!(city, 0x130 => fn() -> *mut u8);
            let sea = unsigned(vcall!(seaport, 0x34 => fn() -> u32));
            let industrial = add_float(multiply_float(sea, sim.seaport_industrial()), f32::from_bits(sim.industrial_relief()));
            sim.set_industrial_relief(industrial.to_f32().to_bits());
            sim.set_commercial_relief(add_float(multiply_float(sea, sim.seaport_commercial()), f32::from_bits(sim.commercial_relief())).to_f32().to_bits());
            let delta_i = subtract_float(industrial, old_i).to_f32().to_bits();
            let object = read_unaligned(map_value(this.add(0x50), &0x4800));
            vcall!(object, 0x40 => fn(u32), delta_i);
            let object = read_unaligned(map_value(this.add(0x50), &0x4900));
            vcall!(object, 0x40 => fn(u32), delta_i);
            let delta_c = subtract_float(F80::from_f32(f32::from_bits(sim.commercial_relief())), old_c).to_f32().to_bits();
            let object = read_unaligned(map_value(this.add(0x50), &0x3800));
            vcall!(object, 0x40 => fn(u32), delta_c);

            clear_connection_list(&mut subway);
            let to_free = subway;
            vcall!(get_allocator(connection_allocator()), 0x10 => fn(*mut u8), to_free);
            clear_connection_list(&mut mono);
            let to_free = mono;
            vcall!(get_allocator(connection_allocator()), 0x10 => fn(*mut u8), to_free);
            transit_cities.clear(false);
            drop_connection_list(&mut light);
            rail_cities.clear(false);
            drop_connection_list(&mut rail);
            dirt_cities.clear(false);
            drop_connection_list(&mut dirt);
            road_cities.clear(false);
            drop_connection_list(&mut road);
            avenue_cities.clear(true);
            drop_connection_list(&mut avenue);
            drop_connection_list(&mut ground);
            highway_cities.clear(true);
            drop_connection_list(&mut highway);
            drop_city_list(&mut cities);
            drop_connection_list(&mut all)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a3ed0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Framework,
        Query,
        Release(u32),
        ListAlloc(u32),
        HeaderAlloc(usize),
        All,
        Cities,
        ByType(u32),
        Kind(u32),
        Connector(u32, u32),
        Arrived(u32),
        City(u32),
        Insert(usize, u32),
        Airport,
        Seaport,
        Trips(u32),
        Node(u32),
        Supply(u32, u32),
        ClearList(i32),
        ListFree(u32, i32),
        DropList(i32),
        Destroy(usize, Option<u32>),
        FreeNode(u32),
        FreeHeader(usize),
        DropCities,
    }
    struct Allocation(Box<[u128; 2]>);
    impl Allocation {
        fn ptr(&mut self) -> *mut u8 {
            self.0.as_mut_ptr().cast()
        }
    }
    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        heap: Vec<Allocation>,
        headers: Vec<u32>,
        lists: Vec<(u32, i32)>,
        connections: Vec<u32>,
        allocator_requests: Vec<u32>,
        allocator_index: usize,
        factory: u32,
        allocator_global: usize,
        reset_allocator: bool,
        airport: u32,
        seaport: u32,
        air: u32,
        sea: u32,
        demand: u32,
        demand_slot: u32,
        commercial_change: Option<u32>,
        sim: u32,
    }
    impl State {
        fn allocate(&mut self) -> *mut u8 {
            self.heap
                .push(Allocation(Box::new([u128::from_le_bytes([0xA5; 16]); 2])));
            self.heap.last_mut().unwrap().ptr()
        }
        fn label(&self, p: u32) -> i32 {
            self.lists.iter().find(|(v, _)| *v == p).unwrap().1
        }
        fn set_index(&self, p: u32) -> usize {
            self.headers.iter().position(|&v| v == p).unwrap() % 6
        }
        unsafe fn populate(&mut self, head: *mut u8, objects: &[u32], label: i32) {
            unsafe {
                let header = Link::from_ptr(head);
                assert_eq!((header.next(), header.previous()), (head, head));
                assert_eq!(header.object() as u32, 0xA5A5_A5A5);
                self.lists.push((head as u32, label));
                let mut last = head;
                for &object in objects {
                    let p = self.allocate();
                    let link = Link::from_ptr(p);
                    link.set_next(head);
                    link.set_previous(last);
                    link.set_object(object as *mut u8);
                    Link::from_ptr(last).set_next(p);
                    header.set_previous(p);
                    last = p;
                }
            }
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    extern "C" fn framework_fake() -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Framework);
            s.factory
        })
    }
    extern "thiscall" fn query(_p: *mut u8, class: u32, iid: u32, output: *mut u32) {
        assert_eq!((class, iid), (0x3AE4_BEA3, 0x3AE4_BEAB));
        assert_eq!(unsafe { *output }, 0);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Query);
            unsafe {
                *output = s.allocator_requests[s.allocator_index];
            }
            s.allocator_index += 1;
        });
    }
    extern "thiscall" fn release(output: *mut u32) {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Release(unsafe { *output })));
    }
    extern "thiscall" fn list_alloc(p: *mut u8, size: u32) -> *mut u8 {
        assert_eq!(size, 12);
        let id = unsafe { *p.add(4).cast::<u32>() };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::ListAlloc(id));
            s.allocate()
        })
    }
    extern "thiscall" fn list_free(p: *mut u8, head: *mut u8) {
        let id = unsafe { *p.add(4).cast::<u32>() };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let label = s.label(head as u32);
            s.calls.push(Call::ListFree(id, label));
        });
    }
    extern "C" fn crt_alloc(size: u32) -> *mut u8 {
        assert_eq!(size, 20);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let index = s.headers.len() % 6;
            s.calls.push(Call::HeaderAlloc(index));
            let p = s.allocate();
            s.headers.push(p as u32);
            p
        })
    }
    extern "C" fn crt_free(p: *mut u8) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.headers.contains(&(p as u32)) {
                let index = s.set_index(p as u32);
                let node = unsafe { TreeNode::from_ptr(p) };
                assert_eq!(
                    (node.parent(), node.left(), node.right()),
                    (core::ptr::null_mut(), p, p)
                );
                assert_eq!(unsafe { core::slice::from_raw_parts(p.add(1), 3) }, &[0xA5; 3]);
                assert_eq!(node.key(), 0xA5A5_A5A5);
                s.calls.push(Call::FreeHeader(index));
            } else {
                s.calls.push(Call::FreeNode(unsafe { TreeNode::from_ptr(p) }.key()));
            }
        });
    }
    extern "thiscall" fn all(_p: *mut u8, output: *mut *mut u8) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::All);
            let objects = s.connections.clone();
            unsafe {
                s.populate(*output, &objects, -1);
            }
        });
    }
    extern "thiscall" fn cities(_p: *mut u8, output: *mut *mut u8, flags: u32) {
        assert_eq!(flags, 0);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Cities);
            unsafe {
                s.populate(*output, &[0x1234], -2);
            }
        });
    }
    extern "thiscall" fn by_type(_p: *mut u8, kind: u32, output: *mut *mut u8) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::ByType(kind));
            let objects: Vec<_> = s
                .connections
                .iter()
                .copied()
                .filter(|&p| unsafe { *((p as *const u32).add(2)) } == kind)
                .collect();
            unsafe {
                s.populate(*output, &objects, kind as i32);
            }
        });
    }
    extern "thiscall" fn kind(p: *mut u8) -> u32 {
        let words = unsafe { &*p.cast::<[u32; 5]>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Kind(words[1])));
        words[2]
    }
    extern "thiscall" fn connector(p: *mut u8, mode: u32) -> u32 {
        let id = unsafe { *p.add(4).cast::<u32>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Connector(id, mode)));
        id
    }
    extern "thiscall" fn arrived(_p: *mut u8, id: u32) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Arrived(id));
            let p = s
                .connections
                .iter()
                .find(|&&p| unsafe { *((p as *const u32).add(1)) } == id)
                .unwrap();
            unsafe { *((*p as *const u32).add(4)) }
        })
    }
    extern "thiscall" fn city(p: *mut u8) -> u32 {
        let words = unsafe { &*p.cast::<[u32; 5]>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::City(words[1])));
        words[3]
    }
    extern "thiscall" fn insert(set: *mut u32, output: *mut u32, key: *const u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            unsafe {
                let header = TreeNode::from_ptr(*set as *mut u8);
                let index = s.set_index(*set);
                let key = *key;
                assert_eq!(header.color(), 0);
                assert_eq!(header.key(), 0xA5A5_A5A5);
                s.calls.push(Call::Insert(index, key));
                let mut parent = header.as_ptr();
                let mut node = header.parent();
                while !node.is_null() {
                    let view = TreeNode::from_ptr(node);
                    parent = node;
                    if key == view.key() {
                        *output = node as u32;
                        return;
                    }
                    node = if key < view.key() { view.left() } else { view.right() };
                }
                let p = s.allocate();
                let node = TreeNode::from_ptr(p);
                node.set_color(1);
                node.set_parent(parent);
                node.set_left(core::ptr::null_mut());
                node.set_right(core::ptr::null_mut());
                node.set_key(key);
                if parent == header.as_ptr() {
                    header.set_parent(p);
                    header.set_left(p);
                    header.set_right(p);
                } else if key < TreeNode::from_ptr(parent).key() {
                    TreeNode::from_ptr(parent).set_left(p);
                    if key < TreeNode::from_ptr(header.left()).key() {
                        header.set_left(p);
                    }
                } else {
                    TreeNode::from_ptr(parent).set_right(p);
                    if key > TreeNode::from_ptr(header.right()).key() {
                        header.set_right(p);
                    }
                }
                *set.add(1) += 1;
                *output = p as u32;
            }
        });
    }
    extern "thiscall" fn destroy(set: *mut u32, root: *mut u8) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let index = s.set_index(unsafe { *set });
            let key = if root.is_null() {
                None
            } else {
                Some(unsafe { TreeNode::from_ptr(root) }.key())
            };
            s.calls.push(Call::Destroy(index, key));
        });
    }
    extern "thiscall" fn clear(output: *mut *mut u8) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let head = unsafe { *output };
            let label = s.label(head as u32);
            s.calls.push(Call::ClearList(label));
            if s.reset_allocator {
                unsafe {
                    *(s.allocator_global as *mut u32) = 0;
                }
                s.reset_allocator = false;
            }
            let head = unsafe { Link::from_ptr(head) };
            head.set_next(head.as_ptr());
            head.set_previous(head.as_ptr());
        });
    }
    extern "thiscall" fn drop_list(output: *mut *mut u8) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let label = s.label(unsafe { *output } as u32);
            s.calls.push(Call::DropList(label));
        });
        0x1122_3344_5566_7788
    }
    extern "thiscall" fn drop_cities(_output: *mut *mut u8) {
        STATE.with(|s| s.borrow_mut().calls.push(Call::DropCities));
    }
    extern "thiscall" fn airport(_p: *mut u8) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Airport);
            s.airport
        })
    }
    extern "thiscall" fn seaport(_p: *mut u8) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Seaport);
            s.seaport
        })
    }
    extern "thiscall" fn trips(p: *mut u8) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let air = p as u32 == s.airport;
            s.calls.push(Call::Trips(u32::from(air)));
            if air {
                s.air
            } else {
                s.sea
            }
        })
    }
    extern "thiscall" fn node(_map: *mut u8, key: *const u32) -> *mut u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let key = unsafe { *key };
            s.calls.push(Call::Node(key));
            unsafe {
                *((s.demand as *mut u32).add(1)) = key;
            }
            s.demand_slot = s.demand;
            &mut s.demand_slot as *mut u32
        })
    }
    extern "thiscall" fn supply(p: *mut u8, bits: u32) {
        let key = unsafe { *p.add(4).cast::<u32>() };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Supply(key, bits));
            if key == 0x4900 {
                if let Some(bits) = s.commercial_change {
                    unsafe { DemandSim::from_ptr(s.sim as *mut u8) }.set_commercial_relief(bits);
                }
            }
        });
    }
    struct Fixture {
        sim: Object,
        weights: Box<[f32; 13]>,
        _tables: Vec<[usize; 80]>,
        _services: Vec<[u32; 2]>,
        _connections: Vec<[u32; 5]>,
        _globals: Box<[u32; 3]>,
        _constants: Box<[f32; 2]>,
        _guards: Vec<Fake>,
    }
    impl Fixture {
        fn new(connections: &[(u32, u32, u32)], lazy: bool, reset_allocator: bool) -> Self {
            let table = |entries: &[(usize, usize)]| {
                let mut t = [0usize; 80];
                for &(slot, f) in entries {
                    t[slot / 4] = f;
                }
                t
            };
            let mut tables = vec![
                table(&[
                    (0x0C, list_alloc as *const () as usize),
                    (0x10, list_free as *const () as usize),
                ]),
                table(&[
                    (0x40, all as *const () as usize),
                    (0x44, by_type as *const () as usize),
                    (0x54, cities as *const () as usize),
                ]),
                table(&[
                    (0x14, kind as *const () as usize),
                    (0x0C, connector as *const () as usize),
                    (0x24, city as *const () as usize),
                ]),
                table(&[(0x54, arrived as *const () as usize)]),
                table(&[
                    (0x134, airport as *const () as usize),
                    (0x130, seaport as *const () as usize),
                ]),
                table(&[(0x34, trips as *const () as usize)]),
                table(&[(0x40, supply as *const () as usize)]),
                table(&[(0x14, query as *const () as usize)]),
            ];
            let mut services: Vec<_> = [(0, 0), (0, 1), (1, 0), (3, 0), (4, 0), (5, 0), (5, 1), (6, 0), (7, 0)]
                .into_iter()
                .map(|(t, id)| [tables[t].as_mut_ptr() as u32, id])
                .collect();
            let p: Vec<_> = services.iter_mut().map(|o| o.as_mut_ptr() as u32).collect();
            let mut connections: Vec<_> = connections
                .iter()
                .enumerate()
                .map(|(i, &(kind, city, trips))| [tables[2].as_mut_ptr() as u32, i as u32, kind, city, trips])
                .collect();
            let mut globals = Box::new([if lazy { 0 } else { p[0] }, if lazy { 0 } else { p[1] }, p[3]]);
            let mut constants = Box::new([4_294_967_296.0f32, core::f32::consts::LOG2_E]);
            let mut weights = Box::new([0.0f32; 13]);
            let mut sim = Object::new(0xDC);
            let view = unsafe { DemandSim::from_ptr(sim.ptr()) };
            view.set_neighbors(p[2] as *mut u8);
            view.set_city(p[4] as *mut u8);
            view.set_network_relief(weights.as_mut_ptr());
            view.set_relief_per_trip(20.0);
            view.set_airport_commercial(1.0);
            view.set_airport_industrial(0.1);
            view.set_seaport_industrial(2.0);
            view.set_seaport_commercial(20.0);
            let guards = vec![
                fake_data(0x00B2_07DC, globals.as_mut_ptr().cast()),
                fake_data(0x00B2_07E0, unsafe { globals.as_mut_ptr().add(1).cast() }),
                fake_data(0x00B4_3DA8, unsafe { globals.as_mut_ptr().add(2).cast() }),
                fake_data(0x00A8_0AA8, constants.as_mut_ptr().cast()),
                fake_data(0x00AB_0E80, unsafe { constants.as_mut_ptr().add(1).cast() }),
                fake(0x0087_93EC, framework_fake as *const () as usize),
                fake(0x0041_1870, release as *const () as usize),
                fake(0x0090_CF54, crt_alloc as *const () as usize),
                fake(0x0090_CF63, crt_free as *const () as usize),
                fake(0x0049_A050, insert as *const () as usize),
                fake(0x004A_4660, destroy as *const () as usize),
                fake(0x0041_1D70, clear as *const () as usize),
                fake(0x0041_1F80, drop_list as *const () as usize),
                fake(0x0041_1FE0, drop_cities as *const () as usize),
                fake(0x0049_7A00, node as *const () as usize),
            ];
            let mut requests = if lazy { vec![p[0], p[1]] } else { vec![] };
            if reset_allocator {
                requests.push(p[0]);
            }
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    connections: connections.iter_mut().map(|o| o.as_mut_ptr() as u32).collect(),
                    allocator_requests: requests,
                    factory: p[8],
                    allocator_global: globals.as_mut_ptr() as usize,
                    reset_allocator,
                    airport: p[5],
                    seaport: p[6],
                    demand: p[7],
                    sim: sim.ptr() as u32,
                    ..State::default()
                }
            });
            Self {
                sim,
                weights,
                _tables: tables,
                _services: services,
                _connections: connections,
                _globals: globals,
                _constants: constants,
                _guards: guards,
            }
        }
        fn view(&mut self) -> DemandSim {
            unsafe { DemandSim::from_ptr(self.sim.ptr()) }
        }
        fn run(&mut self) -> Vec<Call> {
            let mut before = unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0xDC).to_vec() };
            let mut call = crate::demand::test_support::Probe::new(fc4p_006a3ed0 as *const (), self.sim.ptr(), [0; 3]);
            unsafe {
                call.checked_run(0);
            }
            assert_eq!((call.eax, call.edx), (0x5566_7788, 0x1122_3344));
            let view = self.view();
            before[0xA8..0xAC].copy_from_slice(&view.commercial_relief().to_le_bytes());
            before[0xAC..0xB0].copy_from_slice(&view.industrial_relief().to_le_bytes());
            assert_eq!(unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0xDC) }, before);
            STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
        }
    }

    #[test]
    fn empty_connections_clamp_counts_and_release_every_temporary_in_the_original_order() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(&[], false, false);
        f.view().set_industrial_relief(40.0f32.to_bits());
        f.view().set_commercial_relief(100.0f32.to_bits());
        let calls = f.run();
        let mut expected = vec![
            Call::ListAlloc(0),
            Call::All,
            Call::ListAlloc(1),
            Call::Cities,
            Call::ListAlloc(0),
            Call::HeaderAlloc(0),
            Call::ByType(3),
            Call::ListAlloc(0),
            Call::ByType(12),
        ];
        for (index, kind) in [(1, 8), (2, 2), (3, 10), (4, 1), (5, 7)] {
            expected.extend([Call::ListAlloc(0), Call::HeaderAlloc(index), Call::ByType(kind)]);
        }
        expected.extend([
            Call::ListAlloc(0),
            Call::ByType(11),
            Call::ListAlloc(0),
            Call::ByType(6),
            Call::Airport,
            Call::Trips(1),
            Call::Seaport,
            Call::Trips(0),
            Call::Node(0x4800),
            Call::Supply(0x4800, (-40.0f32).to_bits()),
            Call::Node(0x4900),
            Call::Supply(0x4900, (-40.0f32).to_bits()),
            Call::Node(0x3800),
            Call::Supply(0x3800, (-100.0f32).to_bits()),
            Call::ClearList(6),
            Call::ListFree(0, 6),
            Call::ClearList(11),
            Call::ListFree(0, 11),
        ]);
        for (index, kind) in [(5, 7), (4, 1), (3, 10), (2, 2), (1, 8)] {
            expected.extend([Call::FreeHeader(index), Call::DropList(kind)]);
        }
        expected.extend([
            Call::DropList(12),
            Call::FreeHeader(0),
            Call::DropList(3),
            Call::DropCities,
            Call::DropList(-1),
        ]);
        assert_eq!(calls, expected);
        assert_eq!((f.view().commercial_relief(), f.view().industrial_relief()), (0, 0));
    }

    #[test]
    fn core_connection_relief_case_applies_differences_when_a_highway_disappears() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(&[(3, 9, 2)], false, false);
        f.weights[2] = 100_000.0;
        let calls = f.run();
        assert_eq!(f32::from_bits(f.view().commercial_relief()), 100_000.0);
        assert_eq!(f32::from_bits(f.view().industrial_relief()), 40.0);
        assert!(calls.contains(&Call::Supply(0x4800, 40.0f32.to_bits())));
        assert!(calls.contains(&Call::Supply(0x4900, 40.0f32.to_bits())));
        assert!(calls.contains(&Call::Supply(0x3800, 100_000.0f32.to_bits())));
        assert!(calls.contains(&Call::Destroy(0, Some(9))));
        STATE.with(|s| s.borrow_mut().connections.clear());
        let calls = f.run();
        assert!(calls.contains(&Call::Supply(0x4800, (-40.0f32).to_bits())));
        assert!(calls.contains(&Call::Supply(0x4900, (-40.0f32).to_bits())));
        assert!(calls.contains(&Call::Supply(0x3800, (-100_000.0f32).to_bits())));
    }

    #[test]
    fn network_sets_keep_duplicates_at_the_call_boundary_and_require_all_exclusions_to_match() {
        let _fpu = GameFpu::new();
        let descriptors = [
            (3, 9, 1),
            (3, 9, 2),
            (12, 10, 3),
            (8, 9, 4),
            (8, 11, 5),
            (2, 9, 6),
            (2, 11, 7),
            (10, 9, 8),
            (1, 5, 9),
            (1, 5, 10),
            (7, 20, 11),
            (11, 20, 12),
            (11, 21, 13),
            (6, 22, 14),
            (0, 30, 100),
            (13, 40, 100),
        ];
        let mut f = Fixture::new(&descriptors, false, false);
        let calls = f.run();
        let insertions: Vec<_> = calls
            .iter()
            .filter_map(|c| {
                if let Call::Insert(index, key) = c {
                    Some((*index, *key))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            insertions,
            [
                (0, 9),
                (0, 9),
                (0, 10),
                (1, 11),
                (2, 9),
                (2, 11),
                (3, 9),
                (4, 5),
                (4, 5),
                (5, 20),
                (5, 20),
                (5, 21),
                (5, 22)
            ]
        );
        let arrived: Vec<_> = calls
            .iter()
            .filter_map(|c| if let Call::Arrived(id) = c { Some(*id) } else { None })
            .collect();
        assert_eq!(arrived, (0..10).collect::<Vec<_>>());
        assert_eq!(f32::from_bits(f.view().industrial_relief()), 1100.0);
        assert!(calls.contains(&Call::Destroy(5, Some(21))));
        assert!(calls.contains(&Call::FreeNode(20)));
        assert!(calls.contains(&Call::Destroy(1, Some(11))));
    }

    #[test]
    fn airport_and_seaport_coefficients_feed_the_original_totals_and_commercial_delta_is_reloaded() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(&[], false, false);
        f.view().set_industrial_relief(40.0f32.to_bits());
        f.view().set_commercial_relief(100.0f32.to_bits());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.air = 3;
            s.sea = 5;
            s.commercial_change = Some(250.0f32.to_bits());
        });
        let calls = f.run();
        assert_eq!(f32::from_bits(f.view().industrial_relief()), 10.3);
        assert!(calls.contains(&Call::Supply(0x4800, (10.3f32 - 40.0).to_bits())));
        assert!(calls.contains(&Call::Supply(0x4900, (10.3f32 - 40.0).to_bits())));
        assert!(calls.contains(&Call::Supply(0x3800, 150.0f32.to_bits())));
    }

    #[test]
    fn unsigned_trip_counts_and_signaling_old_relief_use_x87_conversion_and_stored_subtraction() {
        let _fpu = GameFpu::new();
        for (trips, bits) in [
            (0x7FFF_FFFF, 0x4F00_0000),
            (0x8000_0001, 0x4F00_0000),
            (u32::MAX, 0x4F80_0000),
        ] {
            let mut f = Fixture::new(&[(1, 1, trips)], false, false);
            f.view().set_relief_per_trip(1.0);
            f.view().set_industrial_relief(0x7F80_0123);
            let calls = f.run();
            assert_eq!(f.view().industrial_relief(), bits);
            assert!(calls.contains(&Call::Supply(0x4800, 0x7FC0_0123)));
            assert!(calls.contains(&Call::Supply(0x4900, 0x7FC0_0123)));
        }
    }

    #[test]
    fn network_relief_sums_transit_rail_dirt_avenue_highway_then_road() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(
            &[
                (3, 10, 0),
                (8, 20, 0),
                (2, 10, 0),
                (10, 30, 0),
                (1, 40, 0),
                (7, 50, 0),
                (11, 51, 0),
                (6, 50, 0),
            ],
            false,
            false,
        );
        f.weights[7] = 11.0;
        f.weights[1] = -100_000_000.0;
        f.weights[11] = 3.0;
        f.weights[6] = 5.0;
        f.weights[2] = 7.0;
        f.weights[0] = 100_000_000.0;
        let calls = f.run();
        // The six weighted terms round to a sum of 24 before multiplying by log2(e).
        assert_eq!(f.view().commercial_relief(), 0x420A_7FAC);
        assert!(calls.contains(&Call::Supply(0x3800, 0x420A_7FAC)));
    }

    #[test]
    fn ground_highways_and_monorails_contribute_distinct_cities_without_their_connection_counts() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(&[(12, 9, 0), (11, 10, 0)], false, false);
        f.weights[2] = 100_000.0;
        f.weights[7] = 50_000.0;
        f.run();
        assert_eq!(f.view().commercial_relief(), 0);
    }

    #[test]
    fn the_seaport_industrial_delta_uses_the_value_retained_before_its_store() {
        let _fpu = GameFpu::new();
        crate::x87::set_control_word(0x037F);
        let mut f = Fixture::new(&[], false, false);
        f.view().set_industrial_relief(10.3f32.to_bits());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.air = 3;
            s.sea = 5;
        });
        let calls = f.run();
        assert_eq!(f.view().industrial_relief(), 10.3f32.to_bits());
        assert!(calls.contains(&Call::Supply(0x4800, 0xB440_0000)));
        assert!(calls.contains(&Call::Supply(0x4900, 0xB440_0000)));
    }

    #[test]
    fn lazy_allocators_are_resolved_for_both_list_classes_and_again_after_cleanup() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(&[], true, true);
        let calls = f.run();
        let initialization: Vec<_> = calls
            .iter()
            .filter(|c| matches!(c, Call::Framework | Call::Query | Call::Release(_)))
            .collect();
        assert_eq!(
            initialization,
            [
                &Call::Framework,
                &Call::Query,
                &Call::Release(f._globals[0]),
                &Call::Framework,
                &Call::Query,
                &Call::Release(f._globals[1]),
                &Call::Framework,
                &Call::Query,
                &Call::Release(f._globals[0])
            ]
        );
    }
}

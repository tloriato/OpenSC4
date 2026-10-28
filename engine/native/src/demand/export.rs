// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Export unmet demand to connected cities in ascending weighted-tax order.

use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_data! {static city:*mut u8=0x00B4_3C94;}
game_data! {static simulation:*mut u8=0x00B4_3D5C;}
game_data! {static history:*mut u8=0x00B4_3DC0;}
game_data! {static minimum:f32=0x00AB_0E10;}
game_data! {static maximum:f32=0x00AB_0E14;}
game_data! {static tax_scale:f32=0x00AB_0E18;}
game_fn! {fn allocate(bytes:u32)->*mut u8="C" 0x0090_CF54;}
game_fn! {fn free(p:*mut u8)->u64="C" 0x0090_CF63;}
game_fn! {fn grow(vector:*mut u8,position:*mut u32,value:*const u32,allocator:*mut u8,a:u32,b:u32)->u64="thiscall" 0x0051_CA60;}
game_fn! {fn score(map:*mut u8,key:*const u32)->*mut f32="thiscall" 0x006A_3170;}
game_fn! {fn copy_map(out:*mut u8,map:*mut u8)->u64="thiscall" 0x006A_2DB0;}
game_fn! {fn sort(first:*mut u32,last:*mut u32,header:u32,count:u32)->u64="C" 0x006A_8F40;}
game_fn! {fn node_value(map:*mut u8,key:*const u32)->*mut *mut u8="thiscall" 0x0049_7A00;}
game_fn! {fn residential(sim:*mut u8,city:*mut u8,developer:u32,amount:f32)->u64="thiscall" 0x006A_68E0;}
game_fn! {fn business(sim:*mut u8,city:*mut u8,developer:u32,amount:f32)->u64="thiscall" 0x006A_6AF0;}
game_fn! {fn destroy_subtree(map:*mut u8,root:*mut u8)->u64="thiscall" 0x006A_25C0;}

layout! {
    /// Circular list of connected city interfaces.
    struct CityLink {0x00=>next/set_next:*mut u8,0x08=>city/set_city:*mut u8,}
}
#[repr(C)]
struct Vector {
    first: *mut u32,
    last: *mut u32,
    end: *mut u32,
}
#[repr(C)]
struct Map {
    header: *mut u8,
    count: u32,
}

unsafe fn bounds(city: *mut u8) {
    unsafe {
        let mut words = [MaybeUninit::<u32>::uninit(); 4];
        let p = words.as_mut_ptr().cast::<u32>();
        vcall!(city,0x14=>fn(*mut u32,*mut u32)->u32,p.add(1),p);
        vcall!(city,0x1C=>fn(*mut u32,*mut u32)->u32,p.add(3),p.add(2));
    }
}

port! {
    /// Export developers whose last tax change is over 65 days old. Rank cities by three times their tax,
    /// limit each transfer to 1000, publish regional supply, and extrapolate both census groups.
    #[port(address=0x006A_8FF0,owner="demand",name="cSC4DemandSimulator::ExportDemand")]
    pub unsafe extern "thiscall" fn fc4p_006a8ff0(this:*mut u8)->u64 {
        unsafe {
            let sim=DemandSim::from_ptr(this);
            let local=vcall!(*city(),0x90=>fn()->*mut u8);
            if !local.is_null(){vcall!(local,4=>fn()->u32);}
            bounds(local);
            let mut vector=Vector {first:core::ptr::null_mut(),last:core::ptr::null_mut(),end:core::ptr::null_mut()};
            let mut allocator=MaybeUninit::<u8>::uninit();
            let mut link=CityLink::from_ptr(sim.export_cities()).next();
            while link!=sim.export_cities() {
                let key=CityLink::from_ptr(link).city() as u32;
                if vector.last==vector.end {
                    grow((&mut vector as *mut Vector).cast(),vector.last,&key,allocator.as_mut_ptr(),1,1);
                }else {
                    if !vector.last.is_null(){write_unaligned(vector.last,key);}
                    vector.last=vector.last.wrapping_add(1);
                }
                link=CityLink::from_ptr(link).next();
            }
            let mut map=Map {header:core::ptr::null_mut(),count:0};
            map.header=allocate(24);
            let h=TreeNode::from_ptr(map.header);h.set_color(0);h.set_parent(core::ptr::null_mut());h.set_left(map.header);h.set_right(map.header);
            let mut totals=[0u32;3];
            let developers=vcall!(sim.building_development(),0x88=>fn()->*mut *mut u8);
            let mut current=TreeNode::from_ptr(*developers).left();
            while current!=*developers {
                let developer=TreeNode::from_ptr(current).value() as *mut u8;
                let kind=vcall!(developer,0x18=>fn()->u32);
                let changed=vcall!(sim.building_development(),0xD0=>fn(u32)->u32,kind);
                let clock=vcall!(*simulation(),0x1C=>fn()->*mut u8);
                let today=vcall!(clock,0x68=>fn()->u32);
                if today>changed&&today.wrapping_sub(changed)>65 {
                    let demand=vcall!(developer,0x48=>fn()->*mut u8);
                    let requested=vcall_st0!(demand,0x24).to_f32();
                    let supplied=vcall_st0!(demand,0x20);
                    let remaining=F80::from_f32(requested)-supplied;
                    let mut remaining_stored=remaining.to_f32();
                    let cmp=remaining.compare(F80::from_f32(read_unaligned(minimum())));
                    if !cmp.less()&&!cmp.unordered() {
                        let mut link=CityLink::from_ptr(sim.export_cities()).next();
                        while link!=sim.export_cities() {
                            let destination=CityLink::from_ptr(link).city();bounds(destination);
                            let tax=vcall_st0!(destination,0x40,kind)*F80::from_f32(read_unaligned(tax_scale()));
                            let stored=tax.to_f32();let key=destination as u32;
                            let slot=score((&mut map as *mut Map).cast(),&key);write_unaligned(slot,stored);
                            link=CityLink::from_ptr(link).next();
                        }
                        let mut copied=MaybeUninit::<[u32;2]>::uninit();
                        copy_map(copied.as_mut_ptr().cast(),(&mut map as *mut Map).cast());
                        let copied=copied.assume_init();sort(vector.first,vector.last,copied[0],copied[1]);
                        let mut position=vector.first;
                        while position!=vector.last {
                            let destination=read_unaligned(position) as *mut u8;
                            let mut amount=vcall!(destination,0x48=>fn(u32)->i32,kind);
                            let cmp=F80::from_i32(amount).compare(F80::from_f32(read_unaligned(maximum())));
                            if !cmp.less()&&!cmp.equal()&&!cmp.unordered(){amount=1000;}
                            let cmp=F80::from_i32(amount).compare(F80::from_f32(remaining_stored));
                            if !cmp.less()&&!cmp.equal()&&!cmp.unordered(){amount=F80::from_f32(remaining_stored).ftol() as i32;}
                            let loaded=F80::from_i32(amount);let stored=loaded.to_f32();
                            let cmp=loaded.compare(F80::from_f32(read_unaligned(minimum())));
                            if !cmp.less()&&!cmp.unordered() {
                                let regional=kind.wrapping_add(0x10000);
                                vcall!(destination,0x4C=>fn(u32,f32)->u32,kind,stored);
                                let node=*node_value(this.add(0x50),&regional);
                                vcall!(node,0x40=>fn(f32)->u32,stored);
                                remaining_stored=(F80::from_f32(remaining_stored)-F80::from_f32(stored)).to_f32();
                                let category=vcall!(developer,0x14=>fn()->u32);
                                if (1..=3).contains(&category){totals[category as usize-1]=totals[category as usize-1].wrapping_add(amount as u32);}
                                residential(this,destination,kind,stored);business(this,destination,kind,stored);
                            }
                            if F80::from_f32(remaining_stored).compare(F80::from_f32(read_unaligned(minimum()))).less(){break;}
                            position=position.wrapping_add(1);
                        }
                    }
                }
                current=successor(current);
            }
            let service=*history();
            for (id,total) in [0xAA77_2F1D,0x6A77_4F64,0xCA77_4F69].into_iter().zip(totals){vcall!(service,0x30=>fn(u32,u32)->u32,id,total);}
            if map.count!=0 {
                let mut root=TreeNode::from_ptr(map.header).parent();
                while !root.is_null() {
                    destroy_subtree((&mut map as *mut Map).cast(),TreeNode::from_ptr(root).right());
                    let next=TreeNode::from_ptr(root).left();free(root);root=next;
                }
                let h=TreeNode::from_ptr(map.header);h.set_left(map.header);h.set_parent(core::ptr::null_mut());h.set_right(map.header);map.count=0;
            }
            if !map.header.is_null(){free(map.header);}
            if !vector.first.is_null(){free(vector.first.cast());}
            vcall!(local,8=>fn()->u64)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a8ff0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map as Tree, test_support::Probe};
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Local,
        Ref,
        Bounds(u32, u32),
        Grow(usize, u32, u32, u32),
        Allocate(u32),
        Developers,
        Kind(u32),
        Changed(u32),
        Clock,
        Today,
        Node(u32),
        Demand(u32),
        Supply(u32),
        Tax(u32, u32),
        Score(u32, u32),
        Copy(u32),
        Sort(Vec<u32>),
        Capacity(u32, u32),
        Export(u32, u32, u32),
        Lookup(u32),
        Add(u32),
        Category(u32),
        Residential(u32, u32, u32),
        Business(u32, u32, u32),
        History(u32, u32),
        Destroy(u32),
        Free(u32),
        Release,
    }
    struct Developer {
        kind: u32,
        category: u32,
        changed: u32,
        node: u32,
        supply: f64,
        demand: f64,
    }
    #[derive(Default)]
    #[allow(clippy::vec_box)]
    struct State {
        calls: Vec<Call>,
        local: u32,
        clock: u32,
        developers: u32,
        devs: Vec<Developer>,
        today: u32,
        taxes: Vec<f64>,
        capacities: Vec<i32>,
        node: u32,
        allocations: Vec<Box<[u32; 6]>>,
        vectors: Vec<Box<[u32]>>,
        scores: Vec<(u32, *mut u32)>,
        score_nodes: Vec<Box<[u32; 6]>>,
        freed: Vec<(u32, [u32; 6])>,
    }
    thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
    fn log(c: Call) {
        STATE.with(|s| s.borrow_mut().calls.push(c));
    }
    unsafe fn id(p: *mut u8) -> u32 {
        unsafe { read_unaligned(p.add(4).cast()) }
    }
    extern "thiscall" fn local(_p: *mut u8) -> u32 {
        log(Call::Local);
        STATE.with(|s| s.borrow().local)
    }
    extern "thiscall" fn addref(_p: *mut u8) -> u32 {
        log(Call::Ref);
        1
    }
    extern "thiscall" fn release(_p: *mut u8) -> u64 {
        log(Call::Release);
        0x1234_5678_90AB_CDEF
    }
    extern "thiscall" fn min_bounds(p: *mut u8, x: *mut u32, y: *mut u32) -> u32 {
        log(Call::Bounds(unsafe { id(p) }, 0x14));
        unsafe {
            *x = 10;
            *y = 20
        };
        1
    }
    extern "thiscall" fn max_bounds(p: *mut u8, x: *mut u32, y: *mut u32) -> u32 {
        log(Call::Bounds(unsafe { id(p) }, 0x1C));
        unsafe {
            *x = 30;
            *y = 40
        };
        1
    }
    extern "thiscall" fn clock(_p: *mut u8) -> u32 {
        log(Call::Clock);
        STATE.with(|s| s.borrow().clock)
    }
    extern "thiscall" fn today(_p: *mut u8) -> u32 {
        log(Call::Today);
        STATE.with(|s| s.borrow().today)
    }
    extern "thiscall" fn developers(_p: *mut u8) -> u32 {
        log(Call::Developers);
        STATE.with(|s| s.borrow().developers)
    }
    extern "thiscall" fn kind(p: *mut u8) -> u32 {
        let i = unsafe { id(p) };
        log(Call::Kind(i));
        STATE.with(|s| s.borrow().devs[i as usize].kind)
    }
    extern "thiscall" fn category(p: *mut u8) -> u32 {
        let i = unsafe { id(p) };
        log(Call::Category(i));
        STATE.with(|s| s.borrow().devs[i as usize].category)
    }
    extern "thiscall" fn changed(_p: *mut u8, kind: u32) -> u32 {
        log(Call::Changed(kind));
        STATE.with(|s| s.borrow().devs.iter().find(|d| d.kind == kind).unwrap().changed)
    }
    extern "thiscall" fn node(p: *mut u8) -> u32 {
        let i = unsafe { id(p) };
        log(Call::Node(i));
        STATE.with(|s| s.borrow().devs[i as usize].node)
    }
    extern "thiscall" fn supply(p: *mut u8) -> f64 {
        let i = unsafe { id(p) };
        log(Call::Supply(i));
        STATE.with(|s| s.borrow().devs[i as usize].supply)
    }
    extern "thiscall" fn demand(p: *mut u8) -> f64 {
        let i = unsafe { id(p) };
        log(Call::Demand(i));
        STATE.with(|s| s.borrow().devs[i as usize].demand)
    }
    extern "thiscall" fn tax(p: *mut u8, kind: u32) -> f64 {
        let i = unsafe { id(p) };
        log(Call::Tax(i, kind));
        STATE.with(|s| s.borrow().taxes[i as usize])
    }
    extern "thiscall" fn capacity(p: *mut u8, kind: u32) -> i32 {
        let i = unsafe { id(p) };
        log(Call::Capacity(i, kind));
        STATE.with(|s| s.borrow().capacities[i as usize])
    }
    extern "thiscall" fn exporting(p: *mut u8, kind: u32, n: u32) -> u32 {
        log(Call::Export(unsafe { id(p) }, kind, n));
        1
    }
    extern "thiscall" fn add(_p: *mut u8, n: u32) -> u32 {
        log(Call::Add(n));
        1
    }
    extern "thiscall" fn lookup(_p: *mut u8, key: *const u32) -> *mut u32 {
        log(Call::Lookup(unsafe { *key }));
        STATE.with(|s| &mut s.borrow_mut().node as *mut u32)
    }
    extern "thiscall" fn residential(_p: *mut u8, city: *mut u8, kind: u32, n: u32) -> u64 {
        log(Call::Residential(unsafe { id(city) }, kind, n));
        0
    }
    extern "thiscall" fn business(_p: *mut u8, city: *mut u8, kind: u32, n: u32) -> u64 {
        log(Call::Business(unsafe { id(city) }, kind, n));
        0
    }
    extern "thiscall" fn history(_p: *mut u8, key: u32, n: u32) -> u32 {
        log(Call::History(key, n));
        0
    }
    extern "C" fn allocate(bytes: u32) -> *mut u8 {
        assert_eq!(bytes, 24);
        log(Call::Allocate(bytes));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let mut b = Box::new([0xA5A5_A5A5u32; 6]);
            let p = b.as_mut_ptr().cast();
            s.allocations.push(b);
            p
        })
    }
    extern "C" fn free(p: *mut u8) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Free(p as u32));
            if let Some(b) = s.allocations.iter().find(|b| b.as_ptr() as u32 == p as u32) {
                let snapshot = **b;
                s.freed.push((p as u32, snapshot));
            }
        });
        0
    }
    extern "thiscall" fn grow(p: *mut u8, position: *mut u32, value: *const u32, _a: *mut u8, a: u32, b: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let vector = unsafe { &mut *p.cast::<Vector>() };
            assert_eq!(position, vector.last);
            let len = (vector.last as usize).wrapping_sub(vector.first as usize) / 4;
            s.calls.push(Call::Grow(len, unsafe { *value }, a, b));
            let mut allocation = vec![0u32; if len == 0 { 2 } else { len * 2 }].into_boxed_slice();
            if len != 0 {
                unsafe { core::ptr::copy_nonoverlapping(vector.first, allocation.as_mut_ptr(), len) };
            }
            allocation[len] = unsafe { *value };
            vector.first = allocation.as_mut_ptr();
            vector.last = unsafe { vector.first.add(len + 1) };
            vector.end = unsafe { vector.first.add(allocation.len()) };
            s.vectors.push(allocation);
        });
        0
    }
    extern "thiscall" fn score(p: *mut u8, key: *const u32) -> *mut f32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let key = unsafe { *key };
            let city = unsafe { id(key as *mut u8) };
            s.calls.push(Call::Score(city, 0));
            if let Some(&(_, slot)) = s.scores.iter().find(|&&(k, _)| k == key) {
                return slot.cast();
            }
            let map = unsafe { &mut *p.cast::<Map>() };
            let mut node = Box::new([0xA5A5_A5A5u32; 6]);
            node[4] = key;
            node[5] = 0;
            node[2] = 0;
            node[3] = 0;
            node[1] = if map.count == 0 {
                map.header as u32
            } else {
                (unsafe { TreeNode::from_ptr(map.header).left() }) as u32
            };
            let ptr = node.as_mut_ptr().cast::<u8>();
            unsafe {
                if map.count == 0 {
                    TreeNode::from_ptr(map.header).set_parent(ptr);
                    TreeNode::from_ptr(map.header).set_right(ptr);
                } else {
                    TreeNode::from_ptr(TreeNode::from_ptr(map.header).left()).set_left(ptr);
                }
                TreeNode::from_ptr(map.header).set_left(ptr);
            }
            map.count += 1;
            let slot = unsafe { ptr.add(20).cast::<u32>() };
            s.scores.push((key, slot));
            s.score_nodes.push(node);
            slot.cast()
        })
    }
    extern "thiscall" fn copying(out: *mut u8, map: *mut u8) -> u64 {
        let count = unsafe { (*map.cast::<Map>()).count };
        log(Call::Copy(count));
        unsafe {
            write_unaligned(out.cast::<u32>(), 0);
            write_unaligned(out.add(4).cast::<u32>(), 0)
        };
        0
    }
    extern "C" fn sorting(first: *mut u32, last: *mut u32, _h: u32, _n: u32) -> u64 {
        let n = (last as usize).wrapping_sub(first as usize) / 4;
        let mut keys = if n == 0 {
            vec![]
        } else {
            unsafe { core::slice::from_raw_parts(first, n) }.to_vec()
        };
        log(Call::Sort(keys.iter().map(|&p| unsafe { id(p as *mut u8) }).collect()));
        let scores = STATE.with(|s| {
            s.borrow()
                .scores
                .iter()
                .map(|&(k, p)| (k, unsafe { *p }))
                .collect::<Vec<_>>()
        });
        keys.sort_by(|a, b| {
            f32::from_bits(scores.iter().find(|&&(k, _)| k == *a).unwrap().1)
                .partial_cmp(&f32::from_bits(scores.iter().find(|&&(k, _)| k == *b).unwrap().1))
                .unwrap()
        });
        if n != 0 {
            unsafe { core::ptr::copy_nonoverlapping(keys.as_ptr(), first, n) }
        };
        0
    }
    extern "thiscall" fn destroy(_p: *mut u8, root: *mut u8) -> u64 {
        log(Call::Destroy(root as u32));
        0
    }
    type Pair = (Box<[usize]>, Box<[u32; 2]>);
    fn object(slots: &[(usize, *const ())], id: u32) -> Pair {
        let mut table = vec![0usize; slots.iter().map(|&(s, _)| s / 4 + 1).max().unwrap()].into_boxed_slice();
        for &(slot, f) in slots {
            table[slot / 4] = f as usize;
        }
        let obj = Box::new([table.as_ptr() as u32, id]);
        (table, obj)
    }
    // Every list node retains its address while the test world is constructed.
    #[allow(clippy::vec_box)]
    struct World {
        sim: Object,
        _objects: Vec<Pair>,
        _developers: Tree,
        header: Box<[u32; 1]>,
        _links: Vec<Box<[u32; 3]>>,
        city: *mut u8,
        simulation: *mut u8,
        history: *mut u8,
        cities: Vec<u32>,
    }
    impl World {
        fn new(count: usize, cities: usize) -> Self {
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    today: 100,
                    ..State::default()
                }
            });
            let mut objects = vec![];
            let city_slots = [
                (4, addref as *const ()),
                (8, release as *const ()),
                (0x14, min_bounds as *const ()),
                (0x1C, max_bounds as *const ()),
                (0x40, tax as *const ()),
                (0x48, capacity as *const ()),
                (0x4C, exporting as *const ()),
            ];
            let mut local = object(&city_slots, 99);
            let local_ptr = local.1.as_mut_ptr() as u32;
            objects.push(local);
            let mut city = object(&[(0x90, super::tests::local as *const ())], 0);
            let city_ptr = city.1.as_mut_ptr().cast();
            objects.push(city);
            let mut clock = object(&[(0x68, today as *const ())], 0);
            let clock_ptr = clock.1.as_mut_ptr() as u32;
            objects.push(clock);
            let mut simulation = object(&[(0x1C, super::tests::clock as *const ())], 0);
            let sim_ptr = simulation.1.as_mut_ptr().cast();
            objects.push(simulation);
            let mut history = object(&[(0x30, super::tests::history as *const ())], 0);
            let history_ptr = history.1.as_mut_ptr().cast();
            objects.push(history);
            let mut building = object(&[(0x88, developers as *const ()), (0xD0, changed as *const ())], 0);
            let building_ptr = building.1.as_mut_ptr().cast();
            objects.push(building);
            let mut regional = object(&[(0x40, add as *const ())], 0);
            let regional_ptr = regional.1.as_mut_ptr() as u32;
            objects.push(regional);
            let mut entries = vec![];
            let mut devs = vec![];
            for i in 0..count {
                let mut node = object(&[(0x20, supply as *const ()), (0x24, demand as *const ())], i as u32);
                let node_ptr = node.1.as_mut_ptr() as u32;
                objects.push(node);
                let mut dev = object(
                    &[
                        (0x14, category as *const ()),
                        (0x18, kind as *const ()),
                        (0x48, super::tests::node as *const ()),
                    ],
                    i as u32,
                );
                entries.push((i as u32, dev.1.as_mut_ptr() as u32));
                objects.push(dev);
                devs.push(Developer {
                    kind: 0x1010 + i as u32,
                    category: i as u32 + 1,
                    changed: 0,
                    node: node_ptr,
                    supply: 0.0,
                    demand: 2500.7,
                });
            }
            let mut developers = Tree::new(&entries);
            let mut header = Box::new([developers.header() as u32]);
            let mut city_words = vec![];
            let mut links = vec![Box::new([0u32; 3])];
            for i in 0..cities {
                let mut c = object(&city_slots, i as u32);
                let p = c.1.as_mut_ptr() as u32;
                city_words.push(p);
                objects.push(c);
                links.push(Box::new([0, 0, p]));
            }
            let addresses: Vec<_> = links.iter_mut().map(|p| p.as_mut_ptr() as u32).collect();
            for (i, l) in links.iter_mut().enumerate() {
                l[0] = addresses[(i + 1) % addresses.len()];
                l[1] = addresses[(i + addresses.len() - 1) % addresses.len()];
            }
            let mut sim = Object::new(0x100);
            unsafe {
                let s = DemandSim::from_ptr(sim.ptr());
                s.set_building_development(building_ptr);
                s.set_export_cities(addresses[0] as *mut u8);
            }
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.local = local_ptr;
                s.clock = clock_ptr;
                s.developers = header.as_mut_ptr() as u32;
                s.devs = devs;
                s.node = regional_ptr;
                s.taxes = (0..cities).map(|i| [3.0, 1.0, 2.0][i % 3]).collect();
                s.capacities = (0..cities).map(|i| [700, 1500, 2000][i % 3]).collect();
            });
            Self {
                sim,
                _objects: objects,
                _developers: developers,
                header,
                _links: links,
                city: city_ptr,
                simulation: sim_ptr,
                history: history_ptr,
                cities: city_words,
            }
        }
        fn run(&mut self) -> Probe {
            let before = unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0x100) }.to_vec();
            let mut p = Probe::new(fc4p_006a8ff0 as *const (), self.sim.ptr(), [0x1122, 0x3344, 0x5566]);
            unsafe { p.checked_run(0) };
            assert_eq!(p.after, p.args);
            assert_eq!((p.eax, p.edx), (0x90ABCDEF, 0x12345678));
            assert_eq!(unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0x100) }, before);
            p
        }
        fn globals(&mut self) -> Vec<crate::testing::Fake> {
            vec![
                fake_data(0x00B4_3C94, (&mut self.city as *mut *mut u8).cast()),
                fake_data(0x00B4_3D5C, (&mut self.simulation as *mut *mut u8).cast()),
                fake_data(0x00B4_3DC0, (&mut self.history as *mut *mut u8).cast()),
            ]
        }
    }
    fn fakes() -> Vec<crate::testing::Fake> {
        [
            (0x0090_CF54, allocate as *const ()),
            (0x0090_CF63, free as *const ()),
            (0x0051_CA60, grow as *const ()),
            (0x006A_3170, score as *const ()),
            (0x006A_2DB0, copying as *const ()),
            (0x006A_8F40, sorting as *const ()),
            (0x0049_7A00, lookup as *const ()),
            (0x006A_68E0, residential as *const ()),
            (0x006A_6AF0, business as *const ()),
            (0x006A_25C0, destroy as *const ()),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f as usize))
        .collect()
    }
    fn constants(values: &mut [f32; 3]) -> Vec<crate::testing::Fake> {
        [0x00AB_0E10, 0x00AB_0E14, 0x00AB_0E18]
            .into_iter()
            .enumerate()
            .map(|(i, a)| fake_data(a, (&mut values[i] as *mut f32).cast()))
            .collect()
    }
    fn calls() -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }

    #[test]
    fn export_ranks_cities_caps_transfers_and_updates_regional_supply_before_both_census_groups() {
        let _f = fakes();
        let _fpu = GameFpu::new();
        let mut constants_values = [10.0, 1000.0, 3.0];
        let _c = constants(&mut constants_values);
        let mut w = World::new(1, 3);
        let _g = w.globals();
        w.run();
        let c = calls();
        let header = STATE.with(|s| s.borrow().allocations[0].as_ptr() as u32);
        let score_nodes = STATE.with(|s| {
            s.borrow()
                .score_nodes
                .iter()
                .map(|p| p.as_ptr() as u32)
                .collect::<Vec<_>>()
        });
        let vector = STATE.with(|s| s.borrow().vectors.last().unwrap().as_ptr() as u32);
        let mut expected = vec![
            Call::Local,
            Call::Ref,
            Call::Bounds(99, 0x14),
            Call::Bounds(99, 0x1C),
            Call::Grow(0, w.cities[0], 1, 1),
            Call::Grow(2, w.cities[2], 1, 1),
            Call::Allocate(24),
            Call::Developers,
            Call::Kind(0),
            Call::Changed(0x1010),
            Call::Clock,
            Call::Today,
            Call::Node(0),
            Call::Demand(0),
            Call::Supply(0),
        ];
        for city in 0..3 {
            expected.extend([
                Call::Bounds(city, 0x14),
                Call::Bounds(city, 0x1C),
                Call::Tax(city, 0x1010),
                Call::Score(city, 0),
            ]);
        }
        expected.extend([Call::Copy(3), Call::Sort(vec![0, 1, 2])]);
        for (city, n) in [(1, 1000f32), (2, 1000f32), (0, 500f32)] {
            expected.extend([
                Call::Capacity(city, 0x1010),
                Call::Export(city, 0x1010, n.to_bits()),
                Call::Lookup(0x11010),
                Call::Add(n.to_bits()),
                Call::Category(0),
                Call::Residential(city, 0x1010, n.to_bits()),
                Call::Business(city, 0x1010, n.to_bits()),
            ]);
        }
        expected.extend([
            Call::History(0xAA772F1D, 2500),
            Call::History(0x6A774F64, 0),
            Call::History(0xCA774F69, 0),
        ]);
        for node in score_nodes {
            expected.extend([Call::Destroy(0), Call::Free(node)]);
        }
        expected.extend([Call::Free(header), Call::Free(vector), Call::Release]);
        assert_eq!(c, expected);
        STATE.with(|s| {
            let s = s.borrow();
            let (p, words) = s.freed[0];
            assert_eq!(p, header);
            assert_eq!(words, [0xA5A5A500, 0, header, header, 0xA5A5A5A5, 0xA5A5A5A5]);
            assert_eq!(
                s.scores.iter().map(|&(_, p)| unsafe { *p }).collect::<Vec<_>>(),
                [9f32.to_bits(), 3f32.to_bits(), 6f32.to_bits()]
            );
        });
        assert_eq!(w.header.len(), 1);
    }

    #[test]
    fn export_handles_empty_developers_and_empty_city_lists_and_records_each_category() {
        let _f = fakes();
        let _fpu = GameFpu::new();
        let mut cv = [10.0, 1000.0, 3.0];
        let _c = constants(&mut cv);
        for (devs, cities) in [(0, 0), (0, 3), (3, 0), (4, 1)] {
            let mut w = World::new(devs, cities);
            let _g = w.globals();
            w.run();
            let c = calls();
            let records = c
                .iter()
                .filter_map(|c| {
                    if let Call::History(k, n) = c {
                        Some((*k, *n))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let n = if cities != 0 && devs != 0 { 700 } else { 0 };
            assert_eq!(records, [(0xAA772F1D, n), (0x6A774F64, n), (0xCA774F69, n)]);
            assert_eq!(c.iter().filter(|c| matches!(c, Call::Sort(..))).count(), devs);
            assert_eq!(c.last(), Some(&Call::Release));
            if devs == 0 || cities == 0 {
                assert!(!c.iter().any(|c| matches!(c, Call::Export(..))));
            }
        }
    }

    #[test]
    fn eligibility_uses_unsigned_days_and_compares_surplus_before_rounding_to_a_float() {
        let _f = fakes();
        let _fpu = GameFpu::new();
        let mut cv = [10.0, 1000.0, 3.0];
        let _c = constants(&mut cv);
        for (today, changed, supply, demand, eligible, transfers) in [
            (100, 35, 0.0, 20.0, false, 0),
            (100, 34, 0.0, 20.0, true, 1),
            (100, 100, 0.0, 20.0, false, 0),
            (100, 101, 0.0, 20.0, false, 0),
            (u32::MAX, 0, 0.0, 20.0, true, 1),
            (100, 0, 0.0, 9.999, false, 0),
            (100, 0, 0.0, 10.0, true, 1),
            (100, 0, 0.0, f64::NAN, false, 0),
            (100, 0, f64::INFINITY, 100.0, false, 0),
            (100, 0, -f64::INFINITY, 100.0, true, 1),
            (100, 0, 0.000001, 10.0, false, 0),
        ] {
            let mut w = World::new(1, 1);
            let _g = w.globals();
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.today = today;
                s.devs[0].changed = changed;
                s.devs[0].supply = supply;
                s.devs[0].demand = demand;
            });
            w.run();
            let c = calls();
            assert_eq!(
                c.iter().any(|c| matches!(c, Call::Sort(..))),
                eligible,
                "{today} {changed} {supply} {demand}"
            );
            assert_eq!(c.iter().filter(|c| matches!(c, Call::Export(..))).count(), transfers);
        }
    }

    #[test]
    fn capacity_is_signed_small_transfers_are_skipped_and_fractional_surplus_is_truncated() {
        let _f = fakes();
        let _fpu = GameFpu::new();
        let mut cv = [10.0, 1000.0, 3.0];
        let _c = constants(&mut cv);
        for (capacity, surplus, amount) in [
            (i32::MIN, 100.0, None),
            (-1, 100.0, None),
            (0, 100.0, None),
            (9, 100.0, None),
            (10, 100.0, Some(10f32)),
            (1000, 100.0, Some(100f32)),
            (1001, 10000.0, Some(1000f32)),
            (i32::MAX, 10000.0, Some(1000f32)),
            (100, 10.9, Some(10f32)),
        ] {
            let mut w = World::new(1, 1);
            let _g = w.globals();
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.capacities[0] = capacity;
                s.devs[0].demand = surplus;
            });
            w.run();
            let c = calls();
            assert_eq!(
                c.iter()
                    .filter_map(|c| if let Call::Export(_, _, n) = c { Some(*n) } else { None })
                    .collect::<Vec<_>>(),
                amount.into_iter().map(f32::to_bits).collect::<Vec<_>>()
            );
        }
    }
}

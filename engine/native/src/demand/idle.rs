// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Time-budgeted census scanning, delta publication and census history.

use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_fn! {fn timer_new(p:*mut u8,kind:u32)->u64="thiscall" 0x0088_FEDF;}
game_fn! {fn timer_start(p:*mut u8)->u64="thiscall" 0x0089_05C4;}
game_fn! {#[allow(dead_code)] fn timer_finish(p:*mut u8)->u64="thiscall" 0x00A6_D837;}
game_fn! {fn budget_new(p:*mut u8,ms:u32)->u64="thiscall" 0x0068_BF50;}
game_fn! {fn expired(p:*mut u8)->u64="thiscall" 0x0068_BF80;}
game_fn! {fn budget_finish(p:*mut u8)->u64="thiscall" 0x0068_B960;}
game_fn! {fn modifiers(p:*mut u8)->u64="thiscall" 0x006A_6070;}
game_fn! {fn zero_census(p:*mut u8,map:*mut u8)->u64="thiscall" 0x006A_37A0;}
game_fn! {fn census(p:*mut u8,x:i32,z:i32)->u64="thiscall" 0x006A_5A00;}
game_fn! {fn find(p:*mut u8,out:*mut *mut u8,key:*const u32)->u64="thiscall" 0x0049_7520;}
game_fn! {fn value(p:*mut u8,key:*const u32)->*mut f32="thiscall" 0x006A_3170;}
game_fn! {fn node_value(p:*mut u8,key:*const u32)->*mut *mut u8="thiscall" 0x0049_7A00;}
game_fn! {fn divide(nlo:u32,nhi:u32,dlo:u32,dhi:u32)->u64="stdcall" 0x009F_0710;}
game_data! {static pacing:[u32;2]=0x00B4_9024;}
game_data! {static pacing_flags:u32=0x00B4_902C;}
game_data! {static history:*mut u8=0x00B4_3DC0;}
game_data! {static salary_keys:[u32;12]=0x00B0_D248;}
game_data! {static zero:f32=0x00A8_1054;}
game_data! {static unsigned_bias:f32=0x00A8_0AA8;}

layout! {
    /// The iterator visits X first, including both end coordinates.
    struct Scan {
        0x00=>state/set_state:u8,
        0x04=>x/set_x:i32,
        0x08=>z/set_z:i32,
        0x0C=>step/set_step:i32,
        0x10=>min_x/set_min_x:i32,
        0x14=>min_z/set_min_z:i32,
        0x18=>max_x/set_max_x:i32,
        0x1C=>max_z/set_max_z:i32,
    }
}

fn unequal(a: f32, b: f32) -> bool {
    let status: u16;
    unsafe {
        core::arch::asm!("fld dword ptr [{b}]","fld dword ptr [{a}]","fucompp","fnstsw ax",
        a=in(reg) &a,b=in(reg) &b,out("ax") status,out("st(0)") _,out("st(1)") _);
    }
    status & 0x4400 != 0x4000
}

unsafe fn unsigned(v: u32) -> F80 {
    let x = F80::from_i32(v as i32);
    if v as i32 >= 0 {
        x
    } else {
        x + F80::from_f32(unsafe { read_unaligned(unsigned_bias()) })
    }
}

unsafe fn history_record(service: *mut u8, id: u32, value: u32) -> u64 {
    unsafe { vcall!(service,0x30=>fn(u32,u32)->u64,id,value) }
}

unsafe fn record_history(sim: DemandSim, e: &mut Entry) -> u64 {
    unsafe {
        let service = *history();
        let map = sim.as_ptr().add(0x50);
        for (key, id) in [
            (0x1001, 0x2A4E_2003),
            (0x1011, 0x4A5E_9485),
            (0x1021, 0xEA5E_948A),
            (0x1031, 0x2A5E_948E),
        ] {
            e.put(0, key);
            let node = *node_value(map, e.args.as_ptr());
            let slot = vslot(service, 0x30);
            let supply = vcall_st0!(node, 0x20).ftol() as u32;
            let record = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, u32) -> u64>(slot);
            record(service, id, supply);
        }
        let mut commercial = 0u32;
        for (i, (key, id)) in [
            (0x3111, 0x8A5E_950E),
            (0x3121, 0x0A5E_9515),
            (0x3131, 0x8A5E_954A),
            (0x3321, 0x8A5E_9552),
            (0x3331, 0xEA5E_9557),
        ]
        .into_iter()
        .enumerate()
        {
            let n = vcall!(sim.as_ptr(),0x24=>fn(u32)->u32,key);
            if i == 4 {
                e.put(0, n);
            }
            history_record(service, id, n);
            commercial = commercial.wrapping_add(n);
        }
        history_record(service, 0x0A4E_2056, commercial);
        let mut industrial = 0u32;
        for (i, (key, id)) in [
            (0x4101, 0x2A5E_95A1),
            (0x4201, 0x6A5E_95A7),
            (0x4301, 0x0A5E_95AC),
            (0x4401, 0xAA5E_95B1),
        ]
        .into_iter()
        .enumerate()
        {
            let n = vcall!(sim.as_ptr(),0x24=>fn(u32)->u32,key);
            if i == 3 {
                e.put(0, n);
            }
            history_record(service, id, n);
            industrial = industrial.wrapping_add(n);
        }
        history_record(service, 0x4A4E_206B, industrial);
        e.put(0, 0x2000);
        let node = *node_value(map, e.args.as_ptr());
        e.put(0, vcall_st0!(node, 0x24).to_f32().to_bits());
        let slot = vslot(service, 0x30);
        let jobs = F80::from_f32(f32::from_bits(e.arg(0))).ftol() as u32;
        let record = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, u32) -> u64>(slot);
        record(service, 0x6A93_1184, jobs);
        let residual = F80::from_f32(f32::from_bits(e.arg(0))) - unsigned(commercial);
        let residual = residual - unsigned(industrial);
        let cmp = residual.compare(F80::from_f32(read_unaligned(zero())));
        let residual = if cmp.less() || cmp.unordered() {
            F80::from_f32(read_unaligned(zero()))
        } else {
            residual
        };
        let slot = vslot(service, 0x30);
        let residual = residual.ftol() as u32;
        let record = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, u32) -> u64>(slot);
        record(service, 0x8A93_1172, residual);
        let mut denominator = 0u64;
        let mut numerator = 0u64;
        e.put(0, sim.as_ptr().add(0x68) as u32);
        for i in 0..12 {
            let node = *node_value(map, salary_keys().cast::<u32>().add(i));
            let n = vcall_st0!(node, 0x20).ftol() as i32;
            denominator = denominator.wrapping_add(n as i64 as u64);
            let weight = read_unaligned(e.arg(0) as *const u32);
            numerator = numerator.wrapping_add(u64::from(weight.wrapping_mul(n as u32)));
            e.put(0, e.arg(0).wrapping_add(4));
        }
        if denominator == 0 {
            history_record(service, 0x8A63_F30C, 0)
        } else {
            let slot = vslot(service, 0x30);
            let quotient = divide(
                numerator as u32,
                (numerator >> 32) as u32,
                denominator as u32,
                (denominator >> 32) as u32,
            );
            let record = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, u32) -> u64>(slot);
            record(service, 0x8A63_F30C, quotient as u32)
        }
    }
}

unsafe fn apply_census(sim: DemandSim, e: &mut Entry) {
    unsafe {
        let mut current = TreeNode::from_ptr(sim.current_census()).left();
        let mut previous = TreeNode::from_ptr(sim.previous_census()).left();
        while current != sim.current_census() {
            let c = TreeNode::from_ptr(current);
            let current_value = f32::from_bits(c.value());
            let delta = (F80::from_f32(current_value)
                - F80::from_f32(f32::from_bits(TreeNode::from_ptr(previous).value())))
            .to_f32();
            e.put(0, delta.to_bits());
            if unequal(delta, read_unaligned(zero())) {
                let key = c.key();
                let mut found = core::ptr::null_mut();
                find(sim.as_ptr().add(0x50), &mut found, &key);
                if found != sim.nodes() {
                    let node = TreeNode::from_ptr(found).value() as *mut u8;
                    let kind = vcall!(node,0x70=>fn()->u32);
                    if kind == 1 {
                        vcall!(node,0x40=>fn(f32)->u32,delta);
                    } else if kind == 2 {
                        vcall!(node,0x44=>fn(f32)->u32,delta);
                    }
                    let slot = value(sim.as_ptr().add(0x28), &key);
                    write_unaligned(slot, current_value);
                }
            }
            current = successor(current);
            previous = successor(previous);
        }
        sim.set_census_state(2);
    }
}

port! {
    /// Advance one census stage. Check the deadline before scanning each cell, and publish a completed scan's
    /// deltas before the next idle call records history. Keep the argument word used as census scratch.
    #[port(address=0x006A_7020,owner="demand",name="cSC4DemandSimulator::DoIdle",naked)]
    pub unsafe extern "C" fn fc4p_006a7020() {
        core::arch::naked_asm!("push edx","push eax","mov edx, esp","call {body}","add esp, 8","ret 4",body=sym idle)
    }
}
unsafe extern "fastcall" fn idle(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let e = &mut *entry;
        let sim = DemandSim::from_ptr(this);
        let mut timer = MaybeUninit::<[u32; 6]>::uninit();
        let timer = timer.as_mut_ptr().cast::<u8>();
        timer_new(timer, 4);
        let started = timer_start(timer);
        let state = sim.census_state();
        let result = if state == 2 {
            let result = record_history(sim, e);
            sim.set_census_state(3);
            result
        } else if state == 0 || state == 1 {
            if state == 0 {
                modifiers(this);
                zero_census(this, this.add(0x1C));
                let scan = Scan::from_ptr(sim.census_iterator());
                scan.set_x(0);
                scan.set_z(0);
                scan.set_state(0);
                if scan.min_x() > 0 || scan.min_z() > 0 || scan.max_x() < 0 || scan.max_z() < 0 {
                    scan.set_state(2);
                }
                sim.set_census_state(1);
            }
            if read_unaligned(pacing_flags()) & 1 == 0 {
                write_unaligned(pacing_flags(), read_unaligned(pacing_flags()) | 1);
                write_unaligned(pacing(), [1024, 0]);
            }
            let mut budget = MaybeUninit::<[u32; 6]>::uninit();
            let budget = budget.as_mut_ptr().cast::<u8>();
            budget_new(budget, e.arg(0));
            let stride = read_unaligned(pacing().cast::<u32>());
            write_unaligned(budget.add(12).cast::<u32>(), stride);
            write_unaligned(budget.add(16), 0u8);
            write_unaligned(budget.add(20).cast::<u32>(), pacing() as u32);
            let mut counter = 0u32;
            loop {
                counter = counter.wrapping_add(stride);
                write_unaligned(budget.add(8).cast::<u32>(), counter);
                if counter & 1023 == 0 && expired(budget) as u8 != 0 {
                    write_unaligned(budget.add(16), 1u8);
                    break;
                }
                let scan = Scan::from_ptr(sim.census_iterator());
                if scan.state() == 2 {
                    apply_census(sim, e);
                    break;
                }
                if scan.state() == 0 {
                    scan.set_state(1);
                    census(this, scan.x(), scan.z());
                    continue;
                }
                scan.set_x(scan.x().wrapping_add(scan.step()));
                if scan.x() > scan.max_x() {
                    scan.set_x(scan.min_x());
                    scan.set_z(scan.z().wrapping_add(scan.step()));
                    if scan.z() > scan.max_z() {
                        scan.set_state(2);
                        apply_census(sim, e);
                        break;
                    }
                }
                census(this, scan.x(), scan.z());
            }
            budget_finish(budget)
        } else {
            registers(state.wrapping_sub(2), (started >> 32) as u32)
        };
        let mut finish = RawCall::new(Conv::Thiscall, crate::abi::target(0x00A6_D837), timer as u32, &[]);
        finish.eax = result as u32;
        finish.edx = (result >> 32) as u32;
        call_raw(&mut finish);
        registers(finish.eax, finish.edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a7020::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map, test_support::Probe};
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Timer(u32),
        Start,
        Finish,
        Modifiers(u32),
        Zero(u32, u32),
        Budget(u32),
        Expired(u32),
        BudgetFinish(u32, u32, u8),
        Census(i32, i32),
        Find(u32),
        Region(u32),
        Add(u32, u32),
        Previous(u32),
        Lookup(u32),
        Supply(u32),
        Demand(u32),
        Query(u32),
        History(u32, u32),
        Divide(u64, u64),
    }
    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        expire_after: usize,
        checks: usize,
        node: u32,
        key: u32,
        region: u32,
        supplies: Vec<(u32, f64)>,
        demand: f64,
        queries: Vec<(u32, u32)>,
        timer: u32,
        budget: u32,
    }
    thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
    fn log(c: Call) {
        STATE.with(|s| s.borrow_mut().calls.push(c));
    }
    extern "thiscall" fn timer(p: *mut u8, kind: u32) -> u64 {
        log(Call::Timer(kind));
        STATE.with(|s| s.borrow_mut().timer = p as u32);
        0
    }
    extern "thiscall" fn start(p: *mut u8) -> u64 {
        assert_eq!(p as u32, STATE.with(|s| s.borrow().timer));
        log(Call::Start);
        u64::MAX
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn finish() {
        core::arch::naked_asm!("push edx","push eax","call {body}","pop eax","pop edx","ret",body=sym finish_log)
    }
    extern "C" fn finish_log() {
        log(Call::Finish);
    }
    extern "thiscall" fn mods(p: *mut u8) -> u64 {
        log(Call::Modifiers(p as u32));
        0
    }
    extern "thiscall" fn zeroed(p: *mut u8, map: *mut u8) -> u64 {
        log(Call::Zero(p as u32, map as u32));
        0
    }
    extern "thiscall" fn budget(p: *mut u8, ms: u32) -> u64 {
        log(Call::Budget(ms));
        STATE.with(|s| s.borrow_mut().budget = p as u32);
        unsafe {
            write_unaligned(p.cast::<u32>(), 111);
            write_unaligned(p.add(4).cast::<u32>(), 222)
        };
        0
    }
    extern "thiscall" fn deadline(p: *mut u8) -> u64 {
        let count = unsafe { read_unaligned(p.add(8).cast::<u32>()) };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Expired(count));
            let expired = s.checks == s.expire_after;
            s.checks += 1;
            u64::from(expired)
        })
    }
    extern "thiscall" fn budget_done(p: *mut u8) -> u64 {
        unsafe {
            assert_eq!(p as u32, STATE.with(|s| s.borrow().budget));
            assert_eq!(read_unaligned(p.cast::<u32>()), 111);
            assert_eq!(read_unaligned(p.add(4).cast::<u32>()), 222);
            assert_eq!(read_unaligned(p.add(20).cast::<u32>()), pacing() as u32);
            log(Call::BudgetFinish(
                read_unaligned(p.add(8).cast()),
                read_unaligned(p.add(12).cast()),
                *p.add(16),
            ));
        }
        0x9988_7766_1234_5678
    }
    extern "thiscall" fn scanning(_p: *mut u8, x: i32, z: i32) -> u64 {
        log(Call::Census(x, z));
        0
    }
    extern "thiscall" fn lookup(_map: *mut u8, key: *const u32) -> *mut u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.key = unsafe { *key };
            let key = s.key;
            s.calls.push(Call::Lookup(key));
            &mut s.node as *mut u32
        })
    }
    extern "thiscall" fn supply(_p: *mut u8) -> f64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let k = s.key;
            s.calls.push(Call::Supply(k));
            s.supplies.iter().find(|&&(key, _)| key == k).map_or(0.0, |&(_, n)| n)
        })
    }
    extern "thiscall" fn demand(_p: *mut u8) -> f64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let k = s.key;
            s.calls.push(Call::Demand(k));
            s.demand
        })
    }
    extern "thiscall" fn query(_p: *mut u8, key: u32) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Query(key));
            s.queries.iter().find(|&&(k, _)| k == key).map_or(0, |&(_, n)| n)
        })
    }
    extern "thiscall" fn record(_p: *mut u8, id: u32, n: u32) -> u64 {
        log(Call::History(id, n));
        0xAABB_CCDD_1234_5678
    }
    extern "stdcall" fn division(a: u32, b: u32, c: u32, d: u32) -> u64 {
        let n = (u64::from(b) << 32) | u64::from(a);
        let den = (u64::from(d) << 32) | u64::from(c);
        log(Call::Divide(n, den));
        n / den
    }
    extern "thiscall" fn finding(map: *mut u8, out: *mut *mut u8, key: *const u32) -> u64 {
        unsafe {
            let key = *key;
            log(Call::Find(key));
            STATE.with(|s| s.borrow_mut().key = key);
            let header = read_unaligned(map.cast::<*mut u8>());
            let mut p = TreeNode::from_ptr(header).left();
            while p != header && TreeNode::from_ptr(p).key() != key {
                p = successor(p);
            }
            write_unaligned(out, p);
            0
        }
    }
    extern "thiscall" fn previous(map: *mut u8, key: *const u32) -> *mut f32 {
        unsafe {
            log(Call::Previous(*key));
            let h = read_unaligned(map.cast::<*mut u8>());
            let mut p = TreeNode::from_ptr(h).left();
            while TreeNode::from_ptr(p).key() != *key {
                p = successor(p);
            }
            p.add(20).cast()
        }
    }
    extern "thiscall" fn region(_p: *mut u8) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let k = s.key;
            s.calls.push(Call::Region(k));
            s.region
        })
    }
    extern "thiscall" fn add_supply(_p: *mut u8, n: u32) -> u32 {
        log(Call::Add(0x40, n));
        0
    }
    extern "thiscall" fn add_demand(_p: *mut u8, n: u32) -> u32 {
        log(Call::Add(0x44, n));
        0
    }
    struct World {
        sim: Object,
        scan: Object,
        current: Map,
        previous: Map,
        nodes: Map,
        _table: Box<[usize; 29]>,
        _sim_table: Box<[usize; 10]>,
        _history_table: Box<[usize; 13]>,
        node: Box<[u32; 1]>,
        history: Box<[u32; 1]>,
    }
    impl World {
        fn new(current: &[(u32, u32)], previous: &[(u32, u32)], keys: &[u32]) -> Self {
            let mut table = Box::new([0usize; 29]);
            table[8] = supply as *const () as usize;
            table[9] = demand as *const () as usize;
            table[28] = region as *const () as usize;
            table[16] = add_supply as *const () as usize;
            table[17] = add_demand as *const () as usize;
            let mut node = Box::new([table.as_ptr() as u32]);
            let mut sim_table = Box::new([0usize; 10]);
            sim_table[9] = query as *const () as usize;
            let mut history_table = Box::new([0usize; 13]);
            history_table[12] = record as *const () as usize;
            let history = Box::new([history_table.as_ptr() as u32]);
            let entries: Vec<_> = keys.iter().map(|&k| (k, node.as_mut_ptr() as u32)).collect();
            let mut w = Self {
                sim: Object::new(0x100),
                scan: Object::new(0x20),
                current: Map::new(current),
                previous: Map::new(previous),
                nodes: Map::new(&entries),
                _table: table,
                _sim_table: sim_table,
                _history_table: history_table,
                node,
                history,
            };
            unsafe {
                let sim = DemandSim::from_ptr(w.sim.ptr());
                sim.set_vtable(w._sim_table.as_ptr() as u32);
                sim.set_census_iterator(w.scan.ptr());
                sim.set_current_census(w.current.header());
                sim.set_previous_census(w.previous.header());
                sim.set_nodes(w.nodes.header());
                let scan = Scan::from_ptr(w.scan.ptr());
                scan.set_step(1);
                scan.set_max_x(1);
                scan.set_max_z(1);
            }
            w
        }
        fn run(&mut self, ms: u32) -> Probe {
            let mut p = Probe::new(fc4p_006a7020 as *const (), self.sim.ptr(), [ms, 0xABCD, 0xEF12]);
            unsafe { p.checked_run(4) };
            assert_eq!(&p.after[1..], &p.args[1..]);
            p
        }
    }
    fn fakes() -> Vec<crate::testing::Fake> {
        [
            (0x0088_FEDF, timer as *const ()),
            (0x0089_05C4, start as *const ()),
            (0x00A6_D837, finish as *const ()),
            (0x0068_BF50, budget as *const ()),
            (0x0068_BF80, deadline as *const ()),
            (0x0068_B960, budget_done as *const ()),
            (0x006A_6070, mods as *const ()),
            (0x006A_37A0, zeroed as *const ()),
            (0x006A_5A00, scanning as *const ()),
            (0x0049_7A00, lookup as *const ()),
            (0x009F_0710, division as *const ()),
            (0x0049_7520, finding as *const ()),
            (0x006A_3170, previous as *const ()),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f as usize))
        .collect()
    }
    fn reset(expire_after: usize) {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                expire_after,
                ..State::default()
            }
        });
    }
    fn calls() -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }

    #[test]
    fn idle_resets_then_resumes_an_inclusive_scan_and_checks_deadlines_before_each_cell() {
        let _f = fakes();
        let mut shared = [7u32, 99];
        let mut flags = 0x80u32;
        let _a = fake_data(0x00B4_9024, shared.as_mut_ptr().cast());
        let _b = fake_data(0x00B4_902C, (&mut flags as *mut u32).cast());
        let mut w = World::new(&[], &[], &[]);
        reset(0);
        let p = w.run(0);
        assert_eq!(shared, [1024, 0]);
        assert_eq!(flags, 0x81);
        assert_eq!(p.after, p.args);
        assert_eq!((p.eax, p.edx), (0x12345678, 0x99887766));
        assert_eq!(
            calls(),
            vec![
                Call::Timer(4),
                Call::Start,
                Call::Modifiers(w.sim.ptr() as u32),
                Call::Zero(w.sim.ptr() as u32, unsafe { w.sim.ptr().add(0x1C) } as u32),
                Call::Budget(0),
                Call::Expired(1024),
                Call::BudgetFinish(1024, 1024, 1),
                Call::Finish
            ]
        );
        assert_eq!(unsafe { DemandSim::from_ptr(w.sim.ptr()).census_state() }, 1);
        assert_eq!(unsafe { Scan::from_ptr(w.scan.ptr()).state() }, 0);
        reset(usize::MAX);
        w.run(100);
        assert_eq!(
            calls(),
            vec![
                Call::Timer(4),
                Call::Start,
                Call::Budget(100),
                Call::Expired(1024),
                Call::Census(0, 0),
                Call::Expired(2048),
                Call::Census(1, 0),
                Call::Expired(3072),
                Call::Census(0, 1),
                Call::Expired(4096),
                Call::Census(1, 1),
                Call::Expired(5120),
                Call::BudgetFinish(5120, 1024, 0),
                Call::Finish
            ]
        );
        assert_eq!(unsafe { DemandSim::from_ptr(w.sim.ptr()).census_state() }, 2);
        assert_eq!(unsafe { Scan::from_ptr(w.scan.ptr()).state() }, 2);
        for state in [3, 4, 256, u32::MAX] {
            unsafe { DemandSim::from_ptr(w.sim.ptr()).set_census_state(state) };
            let p = w.run(99);
            assert_eq!((p.eax, p.edx), (state.wrapping_sub(2), u32::MAX));
            assert_eq!(calls(), vec![Call::Timer(4), Call::Start, Call::Finish]);
        }
    }

    #[test]
    fn iterator_reset_rejects_bounds_excluding_zero_and_existing_pacing_is_retained() {
        let _f = fakes();
        let mut shared = [1u32, 77];
        let mut flags = 3u32;
        let _a = fake_data(0x00B4_9024, shared.as_mut_ptr().cast());
        let _b = fake_data(0x00B4_902C, (&mut flags as *mut u32).cast());
        for bounds in [(1, 0, 3, 3), (0, 1, 3, 3), (0, 0, -1, 3), (0, 0, 3, -1)] {
            reset(usize::MAX);
            let mut w = World::new(&[], &[], &[]);
            let scan = unsafe { Scan::from_ptr(w.scan.ptr()) };
            scan.set_min_x(bounds.0);
            scan.set_min_z(bounds.1);
            scan.set_max_x(bounds.2);
            scan.set_max_z(bounds.3);
            w.run(5);
            assert_eq!(shared, [1, 77]);
            assert_eq!(flags, 3);
            assert!(!calls()
                .iter()
                .any(|c| matches!(c, Call::Census(..) | Call::Expired(..))));
            assert_eq!(unsafe { DemandSim::from_ptr(w.sim.ptr()).census_state() }, 2);
        }
    }

    #[test]
    fn census_deltas_follow_region_use_skip_missing_nodes_and_keep_previous_values_for_zero_deltas() {
        let _f = fakes();
        let _fpu = GameFpu::new();
        let mut shared = [1024u32, 0];
        let mut flags = 1u32;
        let mut zero = 0f32;
        let _a = fake_data(0x00B4_9024, shared.as_mut_ptr().cast());
        let _b = fake_data(0x00B4_902C, (&mut flags as *mut u32).cast());
        let _c = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        for kind in [0, 1, 2, 3, 0x101] {
            reset(usize::MAX);
            STATE.with(|s| s.borrow_mut().region = kind);
            let current = [
                (1, 20f32.to_bits()),
                (2, 5f32.to_bits()),
                (3, 9f32.to_bits()),
                (4, f32::NAN.to_bits()),
            ];
            let previous = [
                (1, 10f32.to_bits()),
                (2, 5f32.to_bits()),
                (3, 1f32.to_bits()),
                (4, 0f32.to_bits()),
            ];
            let mut w = World::new(&current, &previous, &[1, 2, 4]);
            unsafe {
                DemandSim::from_ptr(w.sim.ptr()).set_census_state(1);
                Scan::from_ptr(w.scan.ptr()).set_state(2)
            };
            let p = w.run(88);
            assert!(f32::from_bits(p.after[0]).is_nan());
            let c = calls();
            let relevant: Vec<_> = c
                .into_iter()
                .filter(|c| {
                    matches!(
                        c,
                        Call::Find(..) | Call::Region(..) | Call::Add(..) | Call::Previous(..)
                    )
                })
                .collect();
            let mut expected = vec![Call::Find(1), Call::Region(1)];
            if kind == 1 || kind == 2 {
                expected.push(Call::Add(if kind == 1 { 0x40 } else { 0x44 }, 10f32.to_bits()));
            }
            expected.extend([Call::Previous(1), Call::Find(3), Call::Find(4), Call::Region(4)]);
            if kind == 1 || kind == 2 {
                let last = relevant
                    .iter()
                    .find(|c| matches!(c,Call::Add(_,n) if f32::from_bits(*n).is_nan()))
                    .unwrap();
                expected.push(match last {
                    Call::Add(s, n) => Call::Add(*s, *n),
                    _ => unreachable!(),
                });
            }
            expected.push(Call::Previous(4));
            assert_eq!(relevant, expected);
            let mut p = unsafe { TreeNode::from_ptr(w.previous.header()).left() };
            for (i, expected) in [20.0, 5.0, 1.0, f32::NAN].into_iter().enumerate() {
                let n = unsafe { TreeNode::from_ptr(p) };
                assert_eq!(n.key(), i as u32 + 1);
                if expected.is_nan() {
                    assert!(f32::from_bits(n.value()).is_nan())
                } else {
                    assert_eq!(n.value(), expected.to_bits())
                }
                p = unsafe { successor(p) };
            }
        }
    }

    #[test]
    fn census_history_records_all_series_and_uses_signed_counts_with_wrapping_unsigned_products() {
        let _f = fakes();
        let _fpu = GameFpu::new();
        let mut zero = 0f32;
        let mut bias = 4294967296f32;
        let _z = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        let _b = fake_data(0x00A8_0AA8, (&mut bias as *mut f32).cast());
        let mut keys = [0u32; 12];
        for (i, k) in keys.iter_mut().enumerate() {
            *k = 0x2100 + i as u32;
        }
        let _k = fake_data(0x00B0_D248, keys.as_mut_ptr().cast());
        for (counts, expected_divide) in [
            (vec![0i32; 12], None),
            (vec![2i32; 12], Some((240u64, 24u64))),
            (vec![-1i32; 12], Some((12u64 * 0xFFFF_FFF6, (-12i64) as u64))),
            (vec![i32::MAX; 12], Some((12u64 * 0xFFFF_FFF6, 12u64 * 2_147_483_647))),
        ] {
            reset(usize::MAX);
            let mut w = World::new(&[], &[], &[]);
            let mut service = w.history.as_mut_ptr().cast::<u8>();
            let _h = fake_data(0x00B4_3DC0, (&mut service as *mut *mut u8).cast());
            unsafe {
                let sim = DemandSim::from_ptr(w.sim.ptr());
                sim.set_census_state(2);
                sim.set_salary_weights([10; 12]);
            }
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.node = w.node.as_mut_ptr() as u32;
                s.supplies = keys.iter().zip(counts).map(|(&k, n)| (k, f64::from(n))).collect();
                s.supplies.extend([
                    (0x1001, 16777217.0),
                    (0x1011, -56.9),
                    (0x1021, 3.0e9),
                    (0x1031, f64::INFINITY),
                ]);
                s.demand = 100.9;
                s.queries = vec![
                    (0x3111, 1),
                    (0x3121, 2),
                    (0x3131, 3),
                    (0x3321, 4),
                    (0x3331, 5),
                    (0x4101, 6),
                    (0x4201, 7),
                    (0x4301, 8),
                    (0x4401, 9),
                ];
            });
            let p = w.run(0);
            assert_eq!(p.after[0], unsafe { w.sim.ptr().add(0x98) } as u32);
            assert_eq!(p.eax, 0x12345678);
            assert_eq!(unsafe { DemandSim::from_ptr(w.sim.ptr()).census_state() }, 3);
            let c = calls();
            let records: Vec<_> = c
                .iter()
                .filter_map(|c| {
                    if let Call::History(id, n) = c {
                        Some((*id, *n))
                    } else {
                        None
                    }
                })
                .collect();
            let salary = expected_divide.map_or(0, |(n, d)| (n / d) as u32);
            assert_eq!(
                records,
                vec![
                    (0x2A4E2003, 16777217),
                    (0x4A5E9485, (-56i32) as u32),
                    (0xEA5E948A, 3000000000),
                    (0x2A5E948E, 0),
                    (0x8A5E950E, 1),
                    (0x0A5E9515, 2),
                    (0x8A5E954A, 3),
                    (0x8A5E9552, 4),
                    (0xEA5E9557, 5),
                    (0x0A4E2056, 15),
                    (0x2A5E95A1, 6),
                    (0x6A5E95A7, 7),
                    (0x0A5E95AC, 8),
                    (0xAA5E95B1, 9),
                    (0x4A4E206B, 30),
                    (0x6A931184, 100),
                    (0x8A931172, 55),
                    (0x8A63F30C, salary)
                ],
                "calls: {c:?}"
            );
            assert_eq!(
                c.iter()
                    .filter_map(|c| if let Call::Divide(n, d) = c {
                        Some((*n, *d))
                    } else {
                        None
                    })
                    .collect::<Vec<_>>(),
                expected_divide.into_iter().collect::<Vec<_>>()
            );
            assert_eq!(c.first(), Some(&Call::Timer(4)));
            assert_eq!(c.last(), Some(&Call::Finish));
            assert!(!c.iter().any(|c| matches!(c, Call::Budget(..))));
        }
    }
}

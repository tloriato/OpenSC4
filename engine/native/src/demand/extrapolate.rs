// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Regional residential and business census extrapolation.

use super::queries::multiply_float;
use super::*;
use crate::x87::F80;

game_fn! { fn map_value(map: *mut u8, key: *const u32) -> *mut *mut u8 = "thiscall" 0x0049_7A00; }
game_data! { static default_ratio: f32 = 0x00A9_D190; }

port! {
    /// Extrapolate the four residential workforce groups. The ratio is stored before the workforce
    /// percentage is applied. All four shares are formed before changing any demand node.
    /// A zero sum increments the first count and uses a divisor of one. The third city update uses
    /// the fourth regional key left in the amount argument; the original argument words are scratch.
    #[port(address = 0x006A_68E0, owner = "demand", name = "cSC4DemandSimulator::ExtrapolateResidentialCensus", naked)]
    pub unsafe extern "C" fn fc4p_006a68e0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 12",
            body = sym residential,
        )
    }
}

unsafe extern "fastcall" fn residential(this: *mut u8, entry: *mut Entry) -> u64 {
    // SAFETY: the entry supplies the original three stack words and live city and demand objects.
    unsafe {
        let entry = &mut *entry;
        let dev = entry.arg(1);
        if !matches!(dev, 0x1010 | 0x1020 | 0x1030) {
            return registers(dev.wrapping_sub(0x1030), entry.edx);
        }
        let city = entry.arg(0) as *mut u8;
        let capacity = vcall!(city, 0x24 => fn(u32) -> i32, dev);
        entry.put(0, capacity as u32);
        let census = vcall!(city, 0x24 => fn(u32) -> i32, dev + 1);
        entry.put(1, census as u32);
        let ratio = if capacity > 0 {
            (F80::from_i32(census) / F80::from_i32(capacity)).to_f32()
        } else {
            f32::from_bits(0x3F4C_CCCD)
        };
        entry.put(1, ratio.to_bits());
        let workforce = vcall!(city, 0x34 => fn() -> *const f32);
        let workers = multiply_float(
            multiply_float(F80::from_f32(f32::from_bits(entry.arg(1))), read_unaligned(workforce)),
            f32::from_bits(entry.arg(2)),
        )
        .to_f32();
        let keys = [dev + 0x1101, dev + 0x1102, dev + 0x1103, dev + 0x1104];
        entry.put(2, workers.to_bits());
        entry.put(1, keys[2]);
        entry.put(0, keys[3]);
        let mut counts = [0i32; 4];
        counts[0] = vcall!(city, 0x24 => fn(u32) -> i32, keys[0]);
        counts[1] = vcall!(city, 0x24 => fn(u32) -> i32, keys[1]);
        counts[2] = vcall!(city, 0x24 => fn(u32) -> i32, entry.arg(1));
        counts[3] = vcall!(city, 0x24 => fn(u32) -> i32, entry.arg(0));
        let mut total = counts[3]
            .wrapping_add(counts[2])
            .wrapping_add(counts[1])
            .wrapping_add(counts[0]);
        if total == 0 {
            counts[0] = counts[0].wrapping_add(1);
            total = 1;
        }
        let mut shares = [0u32; 4];
        for (share, count) in shares.iter_mut().zip(counts) {
            *share = (multiply_float(F80::from_i32(count), f32::from_bits(entry.arg(2))) / F80::from_i32(total))
                .to_f32()
                .to_bits();
        }
        entry.put(2, keys[0] + 0x10000);
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr().add(2)));
        vcall!(object, 0x40 => fn(u32) -> u64, shares[0]);
        entry.put(2, keys[1] + 0x10000);
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr().add(2)));
        vcall!(object, 0x40 => fn(u32) -> u64, shares[1]);
        entry.put(2, entry.arg(1).wrapping_add(0x10000));
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr().add(2)));
        vcall!(object, 0x40 => fn(u32) -> u64, shares[2]);
        entry.put(2, entry.arg(0).wrapping_add(0x10000));
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr().add(2)));
        vcall!(object, 0x40 => fn(u32) -> u64, shares[3]);
        vcall!(city, 0x4C => fn(u32, u32) -> u64, keys[0], shares[0]);
        vcall!(city, 0x4C => fn(u32, u32) -> u64, keys[1], shares[1]);
        vcall!(city, 0x4C => fn(u32, u32) -> u64, entry.arg(2), shares[2]);
        vcall!(city, 0x4C => fn(u32, u32) -> u64, entry.arg(0), shares[3])
    }
}

port! {
    /// Extrapolate occupied business capacity and the three wealth-specific job counts. Keep the
    /// population ratio in x87 precision until multiplying the amount. Form all three job products
    /// before the first job-node update, using jobs[1], jobs[2] and jobs[3].
    #[port(address = 0x006A_6AF0, owner = "demand", name = "cSC4DemandSimulator::ExtrapolateBusinessCensus", naked)]
    pub unsafe extern "C" fn fc4p_006a6af0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 12",
            body = sym business,
        )
    }
}

unsafe extern "fastcall" fn business(this: *mut u8, entry: *mut Entry) -> u64 {
    // SAFETY: the entry supplies the original three stack words and live city and demand objects.
    unsafe {
        let entry = &mut *entry;
        let dev = entry.arg(1);
        if !matches!(
            dev,
            0x3110 | 0x3120 | 0x3130 | 0x3320 | 0x3330 | 0x4100 | 0x4200 | 0x4300 | 0x4400
        ) {
            return registers(dev, entry.edx);
        }
        let city = entry.arg(0) as *mut u8;
        let capacity = vcall!(city, 0x24 => fn(u32) -> i32, dev);
        entry.put(0, capacity as u32);
        let census = vcall!(city, 0x24 => fn(u32) -> i32, dev + 1);
        entry.put(1, census as u32);
        let ratio = if capacity > 0 {
            F80::from_i32(census) / F80::from_i32(capacity)
        } else {
            F80::from_f32(read_unaligned(default_ratio()))
        };
        let occupied = multiply_float(ratio, f32::from_bits(entry.arg(2))).to_f32().to_bits();
        entry.put(0, dev + 0x10001);
        entry.put(1, occupied);
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr()));
        vcall!(object, 0x40 => fn(u32) -> u64, entry.arg(1));
        vcall!(city, 0x4C => fn(u32, u32) -> u64, dev + 1, entry.arg(1));
        let mut jobs = [0u32; 4];
        vcall!(this, 0x20 => fn(*mut u32, u32, u32) -> u64, jobs.as_mut_ptr(), dev + 1, 0);
        let first = multiply_float(F80::from_f32(f32::from_bits(jobs[1])), f32::from_bits(entry.arg(1)))
            .to_f32()
            .to_bits();
        entry.put(0, first);
        let second = multiply_float(F80::from_f32(f32::from_bits(jobs[2])), f32::from_bits(entry.arg(1)))
            .to_f32()
            .to_bits();
        entry.put(2, second);
        let third = multiply_float(F80::from_f32(f32::from_bits(jobs[3])), f32::from_bits(entry.arg(1)))
            .to_f32()
            .to_bits();
        entry.put(1, 0x12010);
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr().add(1)));
        let first = entry.arg(0);
        vcall!(object, 0x44 => fn(u32) -> u64, first);
        vcall!(city, 0x4C => fn(u32, u32) -> u64, 0x2010, first);
        entry.put(1, 0x12020);
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr().add(1)));
        let second = entry.arg(2);
        vcall!(object, 0x44 => fn(u32) -> u64, second);
        vcall!(city, 0x4C => fn(u32, u32) -> u64, 0x2020, second);
        entry.put(1, 0x12030);
        let object = read_unaligned(map_value(this.add(0x50), entry.args.as_ptr().add(1)));
        vcall!(object, 0x44 => fn(u32) -> u64, third);
        vcall!(city, 0x4C => fn(u32, u32) -> u64, 0x2030, third)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a68e0::PORT, fc4p_006a6af0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::test_support::Probe;
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Population(u32),
        Workforce,
        Node(u32),
        Supply(u32, u32),
        Demand(u32, u32),
        Grow(u32, u32),
        Jobs(u32, u32),
    }
    struct State {
        city: u32,
        sim: u32,
        populations: Vec<i32>,
        workforce: f32,
        jobs: [u32; 4],
        nodes: [u32; 4],
        next: usize,
        calls: Vec<Call>,
    }
    impl Default for State {
        fn default() -> Self {
            Self {
                city: 0,
                sim: 0,
                populations: vec![],
                workforce: 1.0,
                jobs: [0; 4],
                nodes: [0; 4],
                next: 0,
                calls: vec![],
            }
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    extern "thiscall" fn population(city: *mut u8, key: u32) -> i32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(city as u32, s.city);
            s.calls.push(Call::Population(key));
            s.populations.remove(0)
        })
    }
    extern "thiscall" fn workforce(city: *mut u8) -> *const f32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(city as u32, s.city);
            s.calls.push(Call::Workforce);
            &s.workforce as *const f32
        })
    }
    extern "thiscall" fn node(map: *mut u8, key: *const u32) -> *mut u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(map as u32, s.sim + 0x50);
            let key = unsafe { *key };
            s.calls.push(Call::Node(key));
            let i = s.next;
            s.next += 1;
            unsafe {
                *((s.nodes[i] as *mut u32).add(1)) = key;
            }
            &mut s.nodes[i] as *mut u32
        })
    }
    extern "thiscall" fn supply(object: *mut u8, bits: u32) -> u64 {
        let key = unsafe { *object.add(4).cast::<u32>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Supply(key, bits)));
        0x1234_5678_AABB_CCDD
    }
    extern "thiscall" fn demand(object: *mut u8, bits: u32) -> u64 {
        let key = unsafe { *object.add(4).cast::<u32>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Demand(key, bits)));
        0x1234_5678_AABB_CCDD
    }
    extern "thiscall" fn grow(city: *mut u8, key: u32, bits: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(city as u32, s.city);
            s.calls.push(Call::Grow(key, bits));
        });
        0xDEAD_BEEF_7654_3210
    }
    extern "thiscall" fn jobs(sim: *mut u8, output: *mut u32, key: u32, index: u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(sim as u32, s.sim);
            s.calls.push(Call::Jobs(key, index));
            // Slot zero is deliberately left alone.
            unsafe {
                for i in 1..4 {
                    *output.add(i) = s.jobs[i];
                }
            }
        });
    }

    struct Fixture {
        sim: Object,
        city: Box<[u32; 1]>,
        _tables: [Box<[usize; 20]>; 3],
        _nodes: [Box<[u32; 2]>; 4],
        _ratio: Box<f32>,
        _guards: Vec<Fake>,
    }
    impl Fixture {
        fn new(populations: &[i32], workforce_pct: f32, job_factors: [f32; 3]) -> Self {
            let mut city_table = Box::new([0usize; 20]);
            city_table[0x24 / 4] = population as *const () as usize;
            city_table[0x34 / 4] = workforce as *const () as usize;
            city_table[0x4C / 4] = grow as *const () as usize;
            let mut sim_table = Box::new([0usize; 20]);
            sim_table[0x20 / 4] = jobs as *const () as usize;
            let mut node_table = Box::new([0usize; 20]);
            node_table[0x40 / 4] = supply as *const () as usize;
            node_table[0x44 / 4] = demand as *const () as usize;
            let mut sim = Object::new(0xB4);
            unsafe {
                DemandSim::from_ptr(sim.ptr()).set_vtable(sim_table.as_ptr() as u32);
            }
            let mut city = Box::new([city_table.as_ptr() as u32]);
            let mut nodes: [Box<[u32; 2]>; 4] = core::array::from_fn(|_| Box::new([node_table.as_ptr() as u32, 0]));
            let mut ratio = Box::new(0.8f32);
            let guards = vec![
                fake(0x0049_7A00, node as *const () as usize),
                fake_data(0x00A9_D190, (&mut *ratio as *mut f32).cast()),
            ];
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    city: city.as_mut_ptr() as u32,
                    sim: sim.ptr() as u32,
                    populations: populations.to_vec(),
                    workforce: workforce_pct,
                    jobs: [
                        0xDEAD_BEEF,
                        job_factors[0].to_bits(),
                        job_factors[1].to_bits(),
                        job_factors[2].to_bits(),
                    ],
                    nodes: core::array::from_fn(|i| nodes[i].as_mut_ptr() as u32),
                    next: 0,
                    calls: vec![],
                }
            });
            Self {
                sim,
                city,
                _tables: [city_table, sim_table, node_table],
                _nodes: nodes,
                _ratio: ratio,
                _guards: guards,
            }
        }
        fn run(&mut self, target: *const (), dev: u32, amount: f32) -> Probe {
            let before = unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0xB4).to_vec() };
            let mut call = Probe::new(
                target,
                self.sim.ptr(),
                [self.city.as_mut_ptr() as u32, dev, amount.to_bits()],
            );
            unsafe {
                call.checked_run(12);
            }
            assert_eq!(unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0xB4) }, before);
            assert_eq!((call.eax, call.edx), (0x7654_3210, 0xDEAD_BEEF));
            call
        }
    }

    fn residential_calls(dev: u32, shares: [f32; 4]) -> Vec<Call> {
        let keys = [dev + 0x1101, dev + 0x1102, dev + 0x1103, dev + 0x1104];
        let mut calls = vec![Call::Population(dev), Call::Population(dev + 1), Call::Workforce];
        calls.extend(keys.map(Call::Population));
        for (key, share) in keys.into_iter().zip(shares) {
            calls.push(Call::Node(key + 0x10000));
            calls.push(Call::Supply(key + 0x10000, share.to_bits()));
        }
        for (key, share) in [keys[0], keys[1], keys[3] + 0x10000, keys[3]].into_iter().zip(shares) {
            calls.push(Call::Grow(key, share.to_bits()));
        }
        calls
    }
    fn business_calls(dev: u32, occupied: f32, products: [f32; 3]) -> Vec<Call> {
        let mut calls = vec![
            Call::Population(dev),
            Call::Population(dev + 1),
            Call::Node(dev + 0x10001),
            Call::Supply(dev + 0x10001, occupied.to_bits()),
            Call::Grow(dev + 1, occupied.to_bits()),
            Call::Jobs(dev + 1, 0),
        ];
        for (key, product) in [0x2010, 0x2020, 0x2030].into_iter().zip(products) {
            calls.extend([
                Call::Node(key + 0x10000),
                Call::Demand(key + 0x10000, product.to_bits()),
                Call::Grow(key, product.to_bits()),
            ]);
        }
        calls
    }

    #[test]
    fn residential_groups_use_cached_shares_and_the_fourth_regional_key_for_the_third_city_update() {
        let _fpu = GameFpu::new();
        for dev in [0x1010, 0x1020, 0x1030] {
            let mut f = Fixture::new(&[100, 60, 10, 20, 30, 40], 0.5, [0.0; 3]);
            let call = f.run(fc4p_006a68e0 as *const (), dev, 100.0);
            assert_eq!(call.after, [dev + 0x1104, dev + 0x1103, dev + 0x11104]);
            // The stored 0.6 ratio makes 30.0000019 workers; every resulting share is one ULP above an integer.
            let shares = [0x4040_0001, 0x40C0_0001, 0x4110_0001, 0x4140_0001].map(f32::from_bits);
            STATE.with(|s| assert_eq!(s.borrow().calls, residential_calls(dev, shares)));
        }
    }

    #[test]
    fn residential_zero_sums_increment_the_existing_first_count_and_integer_sums_wrap() {
        let _fpu = GameFpu::new();
        for (counts, shares) in [
            ([0, 0, 0, 0], [8.0, 0.0, 0.0, 0.0]),
            ([5, -3, -2, 0], [48.0, -24.0, -16.0, 0.0]),
            (
                [i32::MIN, i32::MIN, 0, 0],
                [-17_179_869_184.0, -17_179_869_184.0, 0.0, 0.0],
            ),
        ] {
            let mut f = Fixture::new(&[0, 99, counts[0], counts[1], counts[2], counts[3]], 1.0, [0.0; 3]);
            f.run(fc4p_006a68e0 as *const (), 0x1010, 10.0);
            STATE.with(|s| assert_eq!(s.borrow().calls, residential_calls(0x1010, shares)));
        }
    }

    #[test]
    fn business_types_extrapolate_occupied_capacity_and_each_distinct_job_output() {
        let _fpu = GameFpu::new();
        for dev in [0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400] {
            let mut f = Fixture::new(&[100, 80], 0.0, [1.0, 2.0, 3.0]);
            let call = f.run(fc4p_006a6af0 as *const (), dev, 100.0);
            assert_eq!(call.after, [80.0f32.to_bits(), 0x12030, 160.0f32.to_bits()]);
            STATE.with(|s| assert_eq!(s.borrow().calls, business_calls(dev, 80.0, [80.0, 160.0, 240.0])));
        }
        for capacity in [0, -1, i32::MIN] {
            let mut f = Fixture::new(&[capacity, 123], 0.0, [1.0, 2.0, 3.0]);
            f.run(fc4p_006a6af0 as *const (), 0x3110, -10.0);
            STATE.with(|s| assert_eq!(s.borrow().calls, business_calls(0x3110, -8.0, [-8.0, -16.0, -24.0])));
        }
    }

    #[test]
    fn ratio_storage_distinguishes_residential_and_business_under_extended_precision() {
        let _fpu = GameFpu::new();
        crate::x87::set_control_word(0x037F);
        let mut f = Fixture::new(&[3, 1, 1, 0, 0, 0], 1.0, [0.0; 3]);
        f.run(fc4p_006a68e0 as *const (), 0x1010, 7.0);
        STATE.with(|s| {
            assert_eq!(
                s.borrow().calls,
                residential_calls(0x1010, [f32::from_bits(0x4015_5556), 0.0, 0.0, 0.0])
            )
        });
        drop(f);
        let mut f = Fixture::new(&[3, 1], 0.0, [1.0, 2.0, 3.0]);
        f.run(fc4p_006a6af0 as *const (), 0x3110, 7.0);
        STATE.with(|s| {
            assert_eq!(
                s.borrow().calls,
                business_calls(
                    0x3110,
                    f32::from_bits(0x4015_5555),
                    [f32::from_bits(0x4015_5555), f32::from_bits(0x4095_5555), 7.0]
                )
            )
        });
        assert_eq!(crate::x87::control_word(), 0x037F);
    }

    #[test]
    fn unsupported_types_do_not_dereference_objects_or_change_argument_words() {
        for dev in [
            0,
            0x100F,
            0x1011,
            0x102F,
            0x1031,
            0x3111,
            0x312F,
            0x3131,
            0x3321,
            0x3331,
            0x4101,
            0x4301,
            0x4401,
            u32::MAX,
        ] {
            for target in [fc4p_006a68e0 as *const (), fc4p_006a6af0 as *const ()] {
                let args = [0xDEAD_BEEF, dev, 0x7F80_0001];
                let mut call = Probe::new(target, core::ptr::null_mut(), args);
                unsafe {
                    call.checked_run(12);
                }
                assert_eq!(call.after, args);
                assert_eq!(call.edx, 0x5566_7788);
                assert_eq!(
                    call.eax,
                    if target == fc4p_006a68e0 as *const () {
                        dev.wrapping_sub(0x1030)
                    } else {
                        dev
                    }
                );
            }
        }
    }
}

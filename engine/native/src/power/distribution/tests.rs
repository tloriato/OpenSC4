// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Width,
    Height,
    Clear(u32),
    Grow(u32, u32, u32, u32),
    Bounds(u32),
    Position(u32, u32),
    Any(Rect),
    Produced(u32),
    Allocate(u32),
    Deals(u32),
    City(u32),
    Used(u32),
    Amount(u32),
    Connection(u32, u32, u32),
    SetUsed(u32, i32),
    Flags(u32, u32, u32),
    Reset(u32, [u32; 4]),
    Next,
    Or(Rect),
    Mark(u32, u32),
    FreeIterator,
    ClearDeals,
    FreeHead,
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    // List links point into these separate allocations while the vector grows.
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 3]>>,
    network: Vec<(u32, u32)>,
    grow_bounds: Option<(u32, u32)>,
    blocks: Vec<(u32, u32)>,
    block_index: usize,
    buy: Vec<usize>,
    sell: Vec<usize>,
    connected: Vec<u32>,
    iterator_storage: usize,
    sim: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn node() -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut n = Box::new([0u32; 3]);
        let p = n.as_mut_ptr().cast();
        s.nodes.push(n);
        p
    })
}
#[repr(C)]
struct Service {
    table: *const usize,
    width: u32,
    height: u32,
}
#[repr(C)]
struct Plant {
    table: *const usize,
    id: u32,
    low: (u32, u32),
    high: (u32, u32),
    output: u32,
}
#[repr(C)]
struct Deal {
    table: *const usize,
    city: u32,
    used: i32,
    amount: i32,
}

unsafe extern "thiscall" fn width(this: *mut u8) -> u32 {
    log(Call::Width);
    unsafe { (*this.cast::<Service>()).width }
}
unsafe extern "thiscall" fn height(this: *mut u8) -> u32 {
    log(Call::Height);
    unsafe { (*this.cast::<Service>()).height }
}
extern "thiscall" fn alloc(_this: *mut u8, size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    node()
}
extern "thiscall" fn free_head(_this: *mut u8, _head: u32) -> u64 {
    log(Call::FreeHead);
    0x0123_4567_89AB_CDEF
}
unsafe extern "C" fn clear(bits: *mut u32, width: u32) {
    log(Call::Clear(width));
    for i in 0..(width / 32 * width) as usize {
        unsafe {
            bits.add(i).write(0);
        }
    }
}
unsafe extern "C" fn grow_fake(
    out: *mut u32,
    _source: *mut u32,
    width: u32,
    limit: u32,
    low: *mut u32,
    high: *mut u32,
) {
    log(Call::Grow(width, limit, unsafe { low.read() }, unsafe { high.read() }));
    STATE.with(|s| {
        let s = s.borrow();
        for &(x, z) in &s.network {
            unsafe {
                let p = out.add((width / 32 * z + x / 32) as usize);
                p.write(p.read() | 1 << (x & 31));
            }
        }
        if let Some((a, b)) = s.grow_bounds {
            unsafe {
                low.write(a);
                high.write(b);
            }
        }
    });
}
unsafe fn bit(bits: *mut u32, width: u32, x: u32, z: u32) -> bool {
    unsafe { bits.add((width / 32 * z + x / 32) as usize).read() & (1 << (x & 31)) != 0 }
}
unsafe extern "C" fn any(bits: *mut u32, width: u32, rect: *mut Rect) -> u32 {
    let r = unsafe { rect.read() };
    log(Call::Any(r));
    for x in r.x0..=r.x1 {
        for z in r.z0..=r.z1 {
            if unsafe { bit(bits, width, x, z) } {
                return 0x101;
            }
        }
    }
    0x100
}
unsafe extern "C" fn or_rect(out: *mut u32, source: *mut u32, width: u32, rect: *mut Rect) {
    let r = unsafe { rect.read() };
    log(Call::Or(r));
    for x in r.x0..=r.x1 {
        for z in r.z0..=r.z1 {
            if unsafe { bit(source, width, x, z) } {
                unsafe {
                    let p = out.add((width / 32 * z + x / 32) as usize);
                    p.write(p.read() | 1 << (x & 31));
                }
            }
        }
    }
}
unsafe extern "thiscall" fn bounds(this: *mut u8, low: *mut f32, high: *mut f32) {
    let p = unsafe { &*this.cast::<Plant>() };
    log(Call::Bounds(p.id));
    unsafe {
        low.write(p.low.0 as f32);
        low.add(1).write(0.0);
        low.add(2).write(p.low.1 as f32);
        high.write(p.high.0 as f32);
        high.add(1).write(0.0);
        high.add(2).write(p.high.1 as f32);
    }
}
unsafe extern "thiscall" fn position(_this: *mut u8, x: u32, z: u32, ox: *mut u32, oz: *mut u32) {
    let x = F80::from_f32(f32::from_bits(x)).ftol() as u32;
    let z = F80::from_f32(f32::from_bits(z)).ftol() as u32;
    log(Call::Position(x, z));
    unsafe {
        ox.write(x);
        oz.write(z);
    }
}
unsafe extern "thiscall" fn produced_fake(this: *mut u8, occupant: *mut u8) -> u32 {
    assert_eq!(this as usize, STATE.with(|s| s.borrow().sim));
    let p = unsafe { &*occupant.cast::<Plant>() };
    log(Call::Produced(p.id));
    p.output
}
unsafe extern "thiscall" fn deals(_this: *mut u8, kind: u32, list: *mut u32) {
    log(Call::Deals(kind));
    let values = STATE.with(|s| {
        if kind == 0x4773_FFE0 {
            s.borrow().buy.clone()
        } else {
            s.borrow().sell.clone()
        }
    });
    unsafe {
        let head = list.read() as *mut u8;
        head.cast::<u32>().write(head as u32);
        head.add(4).cast::<u32>().write(head as u32);
        for value in values {
            let n = node();
            let last = pointer(head, 4);
            n.cast::<u32>().write(head as u32);
            n.add(4).cast::<u32>().write(last as u32);
            n.add(8).cast::<u32>().write(value as u32);
            last.cast::<u32>().write(n as u32);
            head.add(4).cast::<u32>().write(n as u32);
        }
    }
}
unsafe extern "thiscall" fn city(this: *mut u8) -> u32 {
    let d = unsafe { &*this.cast::<Deal>() };
    log(Call::City(d.city));
    d.city
}
unsafe extern "thiscall" fn used(this: *mut u8) -> i32 {
    let d = unsafe { &*this.cast::<Deal>() };
    log(Call::Used(d.city));
    d.used
}
unsafe extern "thiscall" fn amount(this: *mut u8) -> i32 {
    let d = unsafe { &*this.cast::<Deal>() };
    log(Call::Amount(d.city));
    d.amount
}
unsafe extern "thiscall" fn set_used(this: *mut u8, amount: i32) {
    let d = unsafe { &mut *this.cast::<Deal>() };
    log(Call::SetUsed(d.city, amount));
    d.used = amount;
}
unsafe extern "thiscall" fn flags(this: *mut u8, flags: u32, on: u32) {
    log(Call::Flags(unsafe { (*this.cast::<Deal>()).city }, flags, on));
}
extern "thiscall" fn connection(_this: *mut u8, city: u32, visit: u32, power: u32) -> u32 {
    log(Call::Connection(city, visit, power));
    0x100 | u32::from(STATE.with(|s| s.borrow().connected.contains(&city)))
}
unsafe extern "thiscall" fn reset(this: *mut u8, mode: u32) {
    let bounds = core::array::from_fn(|i| unsafe { this.add(4 + i * 4).cast::<u32>().read() });
    assert_eq!(unsafe { this.read() }, 1);
    assert_eq!(unsafe { this.add(1).read() }, 0);
    assert_eq!(unsafe { this.add(2).read() }, 1);
    log(Call::Reset(mode, bounds));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.block_index = 0;
        unsafe {
            this.add(0x18).cast::<u32>().write(s.iterator_storage as u32);
        }
    });
}
unsafe extern "thiscall" fn next(_this: *mut u8, x: *mut u32, z: *mut u32) -> u32 {
    log(Call::Next);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.block_index;
        s.block_index += 1;
        if let Some(&(a, b)) = s.blocks.get(i) {
            unsafe {
                x.write(a);
                z.write(b);
            }
            0x101
        } else {
            0x100
        }
    })
}
extern "thiscall" fn mark(_this: *mut u8, used: u32, local: u32) {
    log(Call::Mark(used, local));
}
extern "C" fn free_iterator(_p: *mut u8) {
    log(Call::FreeIterator);
}
extern "thiscall" fn clear_deals_fake(_this: *mut u32) {
    log(Call::ClearDeals);
}

struct Fixture {
    sim: Object,
    _city: Box<Service>,
    _allocator: Box<Service>,
    _table: Box<[usize; 96]>,
    plant_table: Box<[usize; 16]>,
    deal_table: Box<[usize; 40]>,
    _cache: Box<*mut u8>,
    _ratio: Box<f32>,
    _bias: Box<f32>,
    _fakes: Vec<Fake>,
    selected: Vec<u32>,
    next: Vec<u32>,
    _wire: Vec<u32>,
    _rows: Vec<[u32; 3]>,
    cells: Vec<Vec<i16>>,
    plants: Vec<Plant>,
    // The sentinel and every plant link need stable addresses.
    #[allow(clippy::vec_box)]
    plant_nodes: Vec<Box<[u32; 4]>>,
    buy: Vec<Deal>,
    sell: Vec<Deal>,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut table = Box::new([0usize; 96]);
        table[3] = alloc as *const () as usize;
        table[4] = free_head as *const () as usize;
        table[0x174 / 4] = width as *const () as usize;
        table[0x178 / 4] = height as *const () as usize;
        table[0x78 / 4] = deals as *const () as usize;
        table[0x14 / 4] = position as *const () as usize;
        let mut pt = Box::new([0usize; 16]);
        pt[0x28 / 4] = bounds as *const () as usize;
        let mut dt = Box::new([0usize; 40]);
        for (slot, f) in [
            (0x1C, city as *const ()),
            (0x50, used as *const ()),
            (0x28, amount as *const ()),
            (0x54, set_used as *const ()),
            (0x94, flags as *const ()),
        ] {
            dt[slot / 4] = f as usize;
        }
        let mut city = Box::new(Service {
            table: table.as_ptr(),
            width: 32,
            height: 32,
        });
        let mut alloc = Box::new(Service {
            table: table.as_ptr(),
            width: 0,
            height: 0,
        });
        let mut cache = Box::new((&mut *alloc as *mut Service).cast::<u8>());
        let mut bias = Box::new(4_294_967_296.0f32);
        let mut ratio = Box::new(-1.1f32);
        let fakes = vec![
            fake_data(0x00B2_0D58, (&mut *cache as *mut *mut u8).cast()),
            fake_data(0x00A8_0AA8, (&mut *bias as *mut f32).cast()),
            fake_data(0x00AB_2888, (&mut *ratio as *mut f32).cast()),
            fake(0x0060_3E00, clear as *const () as usize),
            fake(0x0060_4570, grow_fake as *const () as usize),
            fake(0x0060_4350, any as *const () as usize),
            fake(0x0060_4440, or_rect as *const () as usize),
            fake(0x006E_B1C0, produced_fake as *const () as usize),
            fake(0x006E_DB40, connection as *const () as usize),
            fake(0x006E_BC10, mark as *const () as usize),
            fake(0x0049_D1C0, reset as *const () as usize),
            fake(0x0049_CB10, next as *const () as usize),
            fake(0x0090_CF63, free_iterator as *const () as usize),
            fake(0x0042_1660, clear_deals_fake as *const () as usize),
        ];
        let mut sim = Object::new(0x400);
        let view = unsafe { DistributionSim::from_ptr(sim.ptr()) };
        view.set_city((&mut *city as *mut Service).cast());
        view.set_neighbors((&mut *alloc as *mut Service).cast());
        unsafe {
            sim.ptr().cast::<*const usize>().write(table.as_ptr());
        }
        let mut selected = vec![0; 32];
        let mut next = vec![0; 32];
        let mut wire = vec![0; 32];
        view.set_selected(selected.as_mut_ptr());
        view.set_next_powered(next.as_mut_ptr());
        view.set_wire(wire.as_mut_ptr());
        view.set_factor(1.0);
        let mut rows = vec![[0u32; 3]; 32];
        let mut cells = vec![vec![0i16; 32]; 32];
        for (r, c) in rows.iter_mut().zip(&mut cells) {
            r[0] = c.as_mut_ptr() as u32;
        }
        view.set_rows(rows.as_mut_ptr().cast());
        Fixture {
            sim,
            _city: city,
            _allocator: alloc,
            _table: table,
            plant_table: pt,
            deal_table: dt,
            _cache: cache,
            _bias: bias,
            _ratio: ratio,
            _fakes: fakes,
            selected,
            next,
            _wire: wire,
            _rows: rows,
            cells,
            plants: Vec::new(),
            plant_nodes: Vec::new(),
            buy: Vec::new(),
            sell: Vec::new(),
        }
    }
    fn view(&mut self) -> DistributionSim {
        unsafe { DistributionSim::from_ptr(self.sim.ptr()) }
    }
    fn plant(&mut self, x: u32, z: u32, output: u32) {
        self.plants.push(Plant {
            table: self.plant_table.as_ptr(),
            id: self.plants.len() as u32,
            low: (x, z),
            high: (x, z),
            output,
        });
    }
    fn deal(&mut self, buying: bool, city: u32, amount: i32, used: i32, connected: bool) {
        let list = if buying { &mut self.buy } else { &mut self.sell };
        list.push(Deal {
            table: self.deal_table.as_ptr(),
            city,
            used,
            amount,
        });
        if connected {
            STATE.with(|s| s.borrow_mut().connected.push(city));
        }
    }
    fn run(&mut self, x: u32, z: u32) -> (u32, u32) {
        if self.plant_nodes.is_empty() {
            for _ in 0..=self.plants.len() {
                self.plant_nodes.push(Box::new([0; 4]));
            }
            let ptrs: Vec<_> = self.plant_nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            let len = ptrs.len();
            for (i, n) in self.plant_nodes.iter_mut().enumerate() {
                n[0] = ptrs[(i + 1) % len];
                n[1] = ptrs[(i + len - 1) % len];
                if i != 0 {
                    n[3] = &mut self.plants[i - 1] as *mut Plant as u32;
                }
            }
            self.view().set_plants(ptrs[0] as *mut u8);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.sim = self.sim.ptr() as usize;
            s.buy = self.buy.iter_mut().map(|d| d as *mut Deal as usize).collect();
            s.sell = self.sell.iter_mut().map(|d| d as *mut Deal as usize).collect();
        });
        unsafe {
            call_thiscall_with_eax(
                fc4p_006ee110 as *const () as usize,
                self.sim.ptr(),
                &[x, z],
                0xDEAD_BEEF,
            )
        }
    }
}

#[test]
fn plant_usage_divisor_covers_both_production_and_consumption_with_signed_words() {
    let _fpu = GameFpu::new();
    for (production, demand, used, divisor) in [
        (1000, 0, 0, 1000),
        (1000, 500, 500, 1000),
        (1000, 1000, 1000, 1000),
        (1000, 1200, 1100, 1100),
        (0x8000_0000, 0, 0, 0),
    ] {
        let mut f = Fixture::new();
        f.plant(1, 1, production);
        f.cells[2][2] = demand;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.network = vec![(1, 1), (2, 2)];
            s.blocks = vec![(0, 0)];
        });
        f.run(1, 1);
        assert_eq!(f.view().consumption(), used);
        let marks: Vec<_> = calls().into_iter().filter(|c| matches!(c, Call::Mark(..))).collect();
        assert_eq!(
            marks,
            [Call::Mark(used, divisor)],
            "production {production}, demand {demand}"
        );
    }
}

#[test]
fn one_plant_one_block_accounts_supply_consumption_and_cleanup_in_order() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.plant(1, 1, 1000);
    f.cells[2][2] = 500;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.network = vec![(1, 1), (2, 2)];
        s.blocks = vec![(0, 0)];
        s.iterator_storage = 0x1122_3344;
    });
    assert_eq!(f.run(1, 1), (0x89AB_CDEF, 0x0123_4567));
    let v = f.view();
    assert_eq!(
        (v.production(), v.consumption(), v.demand(), v.powered_count()),
        (1000, 500, 500, 1)
    );
    assert_eq!(f.next[1], 2);
    assert_eq!(f.next[2], 4);
    assert_eq!(f.plant_nodes[1][2], 1);
    let all = calls();
    assert_eq!(all.iter().filter(|c| **c == Call::Width).count(), 71);
    let significant: Vec<_> = all.into_iter().filter(|c| *c != Call::Width).collect();
    assert_eq!(
        significant,
        [
            Call::Clear(32),
            Call::Grow(32, 600, 1, 2),
            Call::Height,
            Call::Bounds(0),
            Call::Position(1, 1),
            Call::Position(1, 1),
            Call::Any(Rect {
                x0: 1,
                z0: 1,
                x1: 1,
                z1: 1
            }),
            Call::Produced(0),
            Call::Allocate(12),
            Call::Deals(0x4773_FFE0),
            Call::Allocate(12),
            Call::Deals(0x4773_FFE1),
            Call::Reset(1, [0, 0, 3, 0]),
            Call::Next,
            Call::Any(Rect {
                x0: 0,
                z0: 0,
                x1: 7,
                z1: 7
            }),
            Call::Or(Rect {
                x0: 0,
                z0: 0,
                x1: 7,
                z1: 7
            }),
            Call::Next,
            Call::Deals(0x4773_FFE0),
            Call::Mark(500, 1000),
            Call::FreeIterator,
            Call::ClearDeals,
            Call::FreeHead,
            Call::ClearDeals,
            Call::FreeHead
        ]
    );
}

#[test]
fn shortfall_uses_iterator_order_and_consumes_all_remaining_supply() {
    let _fpu = GameFpu::new();
    for order in [vec![(0, 0), (1, 0)], vec![(1, 0), (0, 0)]] {
        let mut f = Fixture::new();
        f.plant(1, 1, 1000);
        f.cells[2][2] = 700;
        f.cells[10][2] = 500;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.network = vec![(1, 1), (2, 2), (10, 2)];
            s.blocks = order.clone();
        });
        f.run(1, 1);
        let v = f.view();
        assert_eq!((v.consumption(), v.demand(), v.powered_count()), (1100, 1200, 1));
        assert_eq!(f.next[2] & (1 << 2) != 0, order[0].0 == 0);
        assert_eq!(f.next[2] & (1 << 10) != 0, order[0].0 == 1);
        assert!(calls().contains(&Call::Mark(1100, 1100)));
    }
}

#[test]
fn conservation_keeps_x87_rounding_and_ignores_nonpositive_consumption_cells() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.view().set_factor(0.9);
    f.plant(1, 1, 1000);
    f.cells[2][2] = 555;
    f.cells[3][3] = 40000u32 as i16;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.network = vec![(1, 1), (2, 2), (3, 3)];
        s.blocks = vec![(0, 0)];
    });
    f.run(1, 1);
    let v = f.view();
    assert_eq!((v.consumption(), v.demand(), v.powered_count()), (500, 555, 1));
    assert!(calls().contains(&Call::Mark(500, 1000)));
}

#[test]
fn imports_visit_connections_and_only_charge_the_amount_consumed() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.deal(true, 7, 300, 0, true);
    f.deal(true, 8, 100, 100, true);
    f.deal(true, 9, 1000, 0, false);
    f.cells[2][2] = 200;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.network = vec![(2, 2)];
        s.blocks = vec![(0, 0)];
    });
    f.run(2, 2);
    let v = f.view();
    assert_eq!(
        (v.production(), v.consumption(), v.imports(), v.exports()),
        (0, 200, 200, 0)
    );
    assert_eq!(f.buy[0].used, 200);
    let all = calls();
    let connections: Vec<_> = all.iter().filter(|c| matches!(c, Call::Connection(..))).collect();
    assert_eq!(
        connections,
        [
            &Call::Connection(7, 1, 0),
            &Call::Connection(9, 1, 0),
            &Call::Connection(7, 0, 0),
            &Call::Connection(9, 0, 0)
        ]
    );
    assert!(all.contains(&Call::SetUsed(7, 200)));
    assert!(all.contains(&Call::Mark(0, 0)));
}

#[test]
fn exports_precede_local_blocks_and_preserve_the_flag_argument_upper_bytes() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.plant(1, 1, 1000);
    f.deal(false, 7, 400, 0, true);
    f.cells[2][2] = 100;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.network = vec![(1, 1), (2, 2)];
        s.blocks = vec![(0, 0)];
    });
    f.run(1, 1);
    let v = f.view();
    assert_eq!(
        (v.consumption(), v.exports(), v.imports(), v.demand()),
        (500, 400, 0, 100)
    );
    assert_eq!(f.sell[0].used, 400);
    let all = calls();
    assert!(all.contains(&Call::Connection(7, 0, 0x101)));
    assert!(
        all.iter().position(|c| *c == Call::SetUsed(7, 400)).unwrap()
            < all.iter().position(|c| matches!(c, Call::Reset(..))).unwrap()
    );
    assert!(all.contains(&Call::Mark(500, 1000)));
}

#[test]
fn zero_availability_flags_connected_exports_and_leaves_even_zero_need_blocks_unpowered() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.deal(false, 7, 400, 0, true);
    STATE.with(|s| s.borrow_mut().blocks = vec![(0, 0)]);
    f.run(1, 1);
    let all = calls();
    assert!(all.contains(&Call::Connection(7, 0, 0x100)));
    assert!(all.contains(&Call::Flags(7, 0x20, 1)));
    assert!(!all.iter().any(|c| matches!(c, Call::Or(..))));
    assert!(all.contains(&Call::Mark(0, 0)));
    assert!(unsafe { bit(f.selected.as_mut_ptr(), 32, 1, 1) });
}

#[test]
fn growth_bounds_fallback_height_clamp_and_visited_plants_skip_output_calls() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.plant(1, 31, 1000);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.grow_bounds = Some((100, 1));
        s.network = vec![(1, 31)];
    });
    f.run(1, 31);
    let all = calls();
    assert_eq!(all.iter().filter(|c| **c == Call::Height).count(), 2);
    assert!(all.contains(&Call::Reset(1, [0, 3, 3, 3])));
    assert_eq!(f.plant_nodes[1][2], 1);
    f.run(1, 31);
    let all = calls();
    assert!(!all.iter().any(|c| matches!(c, Call::Bounds(..) | Call::Produced(..))));
}

#[test]
fn unsigned_production_and_signed_final_maximum_keep_their_original_word_semantics() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.plant(1, 1, 0x8000_0000);
    f.run(1, 1);
    let v = f.view();
    assert_eq!((v.production(), v.consumption()), (0x8000_0000, 0));
    assert!(calls().contains(&Call::Mark(0, 0)));
}

#[test]
fn negative_zero_and_nan_factors_follow_unsigned_loads_and_the_games_integer_helper() {
    let _fpu = GameFpu::new();
    for (factor, consumption) in [(-1.0, 76), (0.0, 1100), (f32::NAN, 1100)] {
        let mut f = Fixture::new();
        f.plant(1, 1, 1000);
        f.view().set_factor(factor);
        f.run(1, 1);
        assert_eq!(f.view().consumption(), consumption, "factor {factor}");
        assert!(calls().contains(&Call::Mark(consumption, consumption.max(1000))));
    }
}

#[test]
fn off_network_plants_and_blocks_do_not_contribute_to_totals() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.plant(1, 1, 1000);
    f.plant(9, 9, 3000);
    f.cells[9][9] = 100;
    STATE.with(|s| s.borrow_mut().blocks = vec![(1, 1)]);
    f.run(1, 1);
    let v = f.view();
    assert_eq!(
        (v.production(), v.consumption(), v.demand(), v.powered_count()),
        (1000, 0, 0, 0)
    );
    assert_eq!((f.plant_nodes[1][2], f.plant_nodes[2][2]), (1, 0));
    assert!(!calls().contains(&Call::Produced(1)));
}

fn environment() -> (u16, u16, u32, u32) {
    let mut mxcsr = 0u32;
    let flags: u32;
    unsafe {
        core::arch::asm!("stmxcsr [{mx}]", "pushfd", "pop eax",
            mx = in(reg) &mut mxcsr, lateout("eax") flags);
    }
    (
        crate::x87::control_word(),
        crate::x87::status_word() & 0x3800,
        mxcsr,
        flags & 0x400,
    )
}

#[test]
fn distribution_preserves_the_float_environment_and_an_existing_x87_value() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.plant(1, 1, 1000);
    f.cells[2][2] = 555;
    f.view().set_factor(0.9);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.network = vec![(1, 1), (2, 2)];
        s.blocks = vec![(0, 0)];
    });
    let sentinel = 1.234_567_890_123_456_7f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
    }
    let before = environment();
    f.run(1, 1);
    let after = environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(after, before);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
}

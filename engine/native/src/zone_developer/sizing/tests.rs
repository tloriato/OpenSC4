// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Developer(u32),
    Estimate(u32, i32, i32),
    Source(u32),
    Demand(u32),
    HasStages(u32),
    Stages(u32),
    Grid(u32),
    GridValue(u32, u32),
    Shift,
    Wealth(u32),
    Purpose(u32),
    Initialize(u32),
    FilterNew,
    Configurations([u32; 3]),
    FilterDrop,
    Begin(u32),
    Property(u32),
    Size(u32),
    Family(u32),
    Profile(u32),
    Release(u32),
    Clear(u32),
    Free(u32),
}

#[repr(C)]
struct Developer {
    table: *const usize,
    id: u32,
    desirability: i16,
    demand: f32,
    purpose: u32,
    wealth: u32,
    has_stages: bool,
    stage_vector: [u32; 3],
    stages: Vec<[u32; 3]>,
    grid_value: i16,
    shift: u32,
}

#[repr(C)]
struct Configuration {
    table: *const usize,
    id: u32,
    width: u32,
    depth: u32,
    property: [u32; 12],
    ids: Vec<u32>,
    has_property: bool,
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    developers: Vec<usize>,
    configs: Vec<usize>,
    families: Vec<(u32, Vec<u32>)>,
    profiles: Vec<(u32, [u32; 3])>,
    tables: Vec<u32>,
    null_buckets: bool,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn words(s: &mut State, values: &[u32]) -> *mut u32 {
    let mut memory = values.to_vec().into_boxed_slice();
    let pointer = memory.as_mut_ptr();
    s.allocations.push(memory);
    pointer
}

extern "thiscall" fn developer(_this: *mut u8, id: u32) -> *mut u8 {
    log(Call::Developer(id));
    STATE.with(|s| {
        let s = s.borrow();
        *s.developers
            .iter()
            .find(|&&p| unsafe { (*(p as *const Developer)).id == id })
            .unwrap() as *mut u8
    })
}
extern "thiscall" fn estimate(this: *mut Developer, x: i32, z: i32) -> u32 {
    unsafe {
        log(Call::Estimate((*this).id, x, z));
        0xFACE_0000 | u32::from((*this).desirability as u16)
    }
}
extern "thiscall" fn source(this: *mut Developer) -> *mut u8 {
    unsafe {
        log(Call::Source((*this).id));
    }
    this.cast()
}
extern "thiscall" fn demand(this: *mut Developer) -> f32 {
    unsafe {
        log(Call::Demand((*this).id));
        (*this).demand
    }
}
extern "thiscall" fn has_stages(this: *mut Developer) -> u32 {
    unsafe {
        log(Call::HasStages((*this).id));
        0xABCD_0000 | u32::from((*this).has_stages)
    }
}
extern "thiscall" fn stages(this: *mut Developer) -> *mut u8 {
    unsafe {
        log(Call::Stages((*this).id));
        (*this).stage_vector[0] = (*this).stages.as_ptr() as u32;
        (*this).stage_vector[1] = (*this).stages.as_ptr().add((*this).stages.len()) as u32;
        (*this).stage_vector.as_mut_ptr().cast()
    }
}
extern "thiscall" fn grid(this: *mut Developer) -> *mut u8 {
    unsafe {
        log(Call::Grid((*this).id));
    }
    this.cast()
}
extern "thiscall" fn grid_value(this: *mut Developer, x: u32, z: u32) -> u32 {
    log(Call::GridValue(x, z));
    unsafe { 0xAABB_0000 | u32::from((*this).grid_value as u16) }
}
extern "thiscall" fn shift(this: *mut Developer) -> u32 {
    log(Call::Shift);
    unsafe { (*this).shift }
}
extern "thiscall" fn purpose(this: *mut Developer) -> u32 {
    unsafe {
        log(Call::Purpose((*this).id));
        (*this).purpose
    }
}
extern "thiscall" fn wealth(this: *mut Developer) -> u32 {
    unsafe {
        log(Call::Wealth((*this).id));
        (*this).wealth
    }
}

extern "thiscall" fn initialize(this: *mut u8, size: u32) {
    assert_eq!(size, 100);
    unsafe {
        let table = Table::from_ptr(this);
        assert!(table.buckets().is_null());
        assert!(table.end().is_null());
        assert!(table.limit().is_null());
        assert_eq!(table.count(), 0);
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.calls.push(Call::Initialize(this as u32));
            s.tables.push(this as u32);
            if !s.null_buckets {
                let buckets = words(&mut s, &[0; 8]).cast::<*mut u8>();
                table.set_buckets(buckets);
                table.set_end(buckets.add(8));
                table.set_limit(buckets.add(8));
            }
        });
    }
}

unsafe fn insert(s: &mut State, table: Table, key: u32, pointer: bool) {
    unsafe {
        let bucket = if pointer { key >> 2 } else { key } % 8;
        let slot = table.buckets().add(bucket as usize);
        let node = words(s, &[slot.read() as u32, key]).cast::<u8>();
        slot.write(node);
        table.set_count(table.count() + 1);
    }
}

extern "thiscall" fn filter_new(this: *mut u8) {
    log(Call::FilterNew);
    unsafe {
        this.cast::<u32>().write(0x00A8_EC1C);
        this.add(4).cast::<u32>().write(0);
    }
}
extern "thiscall" fn filter_drop(this: *mut u8) {
    assert_eq!(unsafe { this.cast::<u32>().read() }, 0x00AB_36FC);
    log(Call::FilterDrop);
}
extern "thiscall" fn configurations(_this: *mut u8, map: *mut u8, filter: *mut u8) {
    unsafe {
        assert_eq!(filter.cast::<u32>().read(), 0x00AB_36FC);
        assert_eq!(filter.add(4).cast::<u32>().read(), 0);
        log(Call::Configurations([
            filter.add(8).cast::<u32>().read(),
            filter.add(12).cast::<u32>().read(),
            filter.add(16).cast::<u32>().read(),
        ]));
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            let configs = s.configs.clone();
            for p in configs {
                insert(&mut s, Table::from_ptr(map), p as u32, true);
            }
        });
    }
}
extern "thiscall" fn begin(this: *mut u8, out: *mut u32) {
    log(Call::Begin(this as u32));
    unsafe {
        out.write(first_node(Table::from_ptr(this)) as u32);
        out.add(1).write(this as u32);
    }
}
extern "thiscall" fn property(this: *mut Configuration) -> *mut u8 {
    unsafe {
        log(Call::Property((*this).id));
        (*this).property[10] = (*this).ids.as_ptr() as u32;
        (*this).property[11] = (*this).ids.as_ptr().add((*this).ids.len()) as u32;
        if (*this).has_property {
            (*this).property.as_mut_ptr().cast()
        } else {
            null_mut()
        }
    }
}
extern "thiscall" fn size(this: *mut Configuration, width: *mut u32, depth: *mut u32) {
    unsafe {
        log(Call::Size((*this).id));
        width.write((*this).width);
        depth.write((*this).depth);
    }
}
extern "thiscall" fn release(this: *mut Configuration) {
    unsafe {
        log(Call::Release((*this).id));
    }
}
extern "thiscall" fn family(_this: *mut u8, map: *mut u8, id: u32) {
    log(Call::Family(id));
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        let ids = s
            .families
            .iter()
            .find(|(family, _)| *family == id)
            .map_or(vec![], |(_, ids)| ids.clone());
        for id in ids {
            unsafe {
                insert(&mut s, Table::from_ptr(map), id, false);
            }
        }
    });
}
extern "thiscall" fn profile(_this: *mut u8, id: u32) -> *mut u8 {
    log(Call::Profile(id));
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        let value = s.profiles.iter().find(|(key, _)| *key == id).unwrap().1;
        words(&mut s, &value).cast()
    })
}
extern "thiscall" fn clear(this: *mut u8) -> u64 {
    log(Call::Clear(this as u32));
    unsafe {
        let table = Table::from_ptr(this);
        if !table.buckets().is_null() {
            for i in 0..8 {
                table.buckets().add(i).write(null_mut());
            }
        }
    }
    0xFACE_CAFE_1234_5678
}
extern "C" fn free(pointer: *mut u8) -> u64 {
    log(Call::Free(pointer as u32));
    0xAABB_CCDD_5566_7788
}

struct World {
    memory: Object,
    region: Object,
    developers: Vec<Developer>,
    configs: Vec<Configuration>,
    // The objects retain these addresses while the owning vector is assembled and moved.
    #[allow(clippy::vec_box)]
    _tables: Vec<Box<[usize; 48]>>,
    _growth: Box<u32>,
    _manager: Box<u32>,
    _globals: Box<[u32; 8]>,
    _fakes: Vec<Fake>,
    _fpu: GameFpu,
}

impl World {
    fn new(zone: u32) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut developer_table = Box::new([0usize; 48]);
        for (offset, function) in [
            (0x74, estimate as *const () as usize),
            (0x48, source as *const () as usize),
            (0x30, demand as *const () as usize),
            (0x28, has_stages as *const () as usize),
            (0x2C, stages as *const () as usize),
            (0x34, grid as *const () as usize),
            (0x58, grid_value as *const () as usize),
            (0x1C, purpose as *const () as usize),
            (0x20, wealth as *const () as usize),
        ] {
            developer_table[offset / 4] = function;
        }
        // The grid and developer share slot 0x2C, so return a separate grid object below.
        let mut grid_table = Box::new([0usize; 48]);
        grid_table[0x58 / 4] = grid_value as *const () as usize;
        grid_table[0x2C / 4] = shift as *const () as usize;
        let mut config_table = Box::new([0usize; 48]);
        config_table[0x60 / 4] = property as *const () as usize;
        config_table[0x1C / 4] = size as *const () as usize;
        config_table[2] = release as *const () as usize;
        let mut growth_table = Box::new([0usize; 48]);
        growth_table[0x80 / 4] = developer as *const () as usize;
        growth_table[0x4C / 4] = family as *const () as usize;
        growth_table[0x9C / 4] = profile as *const () as usize;
        let mut manager_table = Box::new([0usize; 48]);
        manager_table[0x24 / 4] = configurations as *const () as usize;
        let mut growth = Box::new(growth_table.as_ptr() as u32);
        let mut manager = Box::new(manager_table.as_ptr() as u32);
        let mut globals = Box::new([0u32; 8]);
        globals[0] = &mut *manager as *mut u32 as u32;
        globals[1] = 4294967296.0f32.to_bits();
        globals[2..4].copy_from_slice(&[0, 0x41F0_0000]);
        globals[4..6].copy_from_slice(&[0, 0x4004_0000]);
        let mut fakes = vec![];
        for (address, function) in [
            (0x0082_9BC0, initialize as *const () as usize),
            (0x0048_9A50, filter_new as *const () as usize),
            (0x0048_9660, filter_drop as *const () as usize),
            (0x004B_0390, begin as *const () as usize),
            (0x008E_BA60, clear as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
        ] {
            fakes.push(fake(address, function));
        }
        for (address, index) in [(0x00B4_3D18, 0), (0x00A8_0AA8, 1), (0x00A8_0AB8, 2), (0x00A9_56F8, 4)] {
            fakes.push(fake_data(address, unsafe { globals.as_mut_ptr().add(index).cast() }));
        }
        let mut developers = Vec::with_capacity(16);
        for id in [0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330] {
            developers.push(Developer {
                table: developer_table.as_ptr(),
                id,
                desirability: 100,
                demand: 200.0,
                purpose: if zone <= 3 { 1 } else { 2 },
                wealth: 1,
                has_stages: false,
                stage_vector: [0; 3],
                stages: vec![],
                grid_value: 100,
                shift: 2,
            });
        }
        STATE.with(|s| {
            s.borrow_mut().developers = developers
                .iter_mut()
                .map(|dev| dev as *mut Developer as usize)
                .collect()
        });
        let mut memory = Object::new(0x100);
        let mut region = Object::new(0x30);
        unsafe {
            let settings = Settings::from_ptr(memory.ptr());
            settings.set_zone(zone);
            settings.set_growth((&mut *growth as *mut u32).cast());
            settings.set_block(0x1234_5678);
            let r = Region::from_ptr(region.ptr());
            r.set_x1(10);
            r.set_z1(20);
            r.set_x2(20);
            r.set_z2(30);
        }
        Self {
            memory,
            region,
            developers,
            configs: Vec::with_capacity(16),
            _tables: vec![developer_table, grid_table, config_table, growth_table, manager_table],
            _growth: growth,
            _manager: manager,
            _globals: globals,
            _fakes: fakes,
            _fpu: GameFpu::new(),
        }
    }
    fn settings(&mut self) -> Settings {
        unsafe { Settings::from_ptr(self.memory.ptr()) }
    }
    fn config(&mut self, width: u32, depth: u32, ids: &[u32], has_property: bool) -> usize {
        let id = self.configs.len();
        assert!(id < 16);
        self.configs.push(Configuration {
            table: self._tables[2].as_ptr(),
            id: id as u32,
            width,
            depth,
            property: [0; 12],
            ids: ids.to_vec(),
            has_property,
        });
        STATE.with(|s| {
            s.borrow_mut()
                .configs
                .push(&mut self.configs[id] as *mut Configuration as usize)
        });
        id
    }
    fn profile(&mut self, family: u32, id: u32, purpose: u32, wealth: u32, capacity: u16) {
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.profiles.push((id, [purpose, wealth, u32::from(capacity)]));
            if let Some((_, ids)) = s.families.iter_mut().find(|(key, _)| *key == family) {
                ids.push(id);
            } else {
                s.families.push((family, vec![id]));
            }
        });
    }
    fn call(&mut self) -> u64 {
        unsafe {
            let pointer = self.memory.ptr();
            let before = core::slice::from_raw_parts(pointer, 0x100).to_vec();
            let result = fc4p_00732bf0(pointer, self.region.ptr());
            let after = core::slice::from_raw_parts(pointer, 0x100);
            assert_eq!(&after[..0x84], &before[..0x84]);
            assert_eq!(&after[0x98..], &before[0x98..]);
            result
        }
    }
    fn sizes(&mut self) -> [u32; 5] {
        let s = self.settings();
        [s.width(), s.depth(), s.minimum(), s.maximum(), s.block()]
    }
}

// Return a grid copy with its own vtable, while preserving the developer's desirability-grid fields.
extern "thiscall" fn separate_grid(this: *mut Developer) -> *mut u8 {
    unsafe {
        log(Call::Grid((*this).id));
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            // Filled by the test wrapper before entering the port.
            let table = GRID_TABLE.with(|table| *table.borrow());
            let p = words(&mut s, &[0; 32]).cast::<Developer>();
            p.write(Developer {
                table: table as *const usize,
                id: (*this).id,
                desirability: 0,
                demand: 0.0,
                purpose: 0,
                wealth: 0,
                has_stages: false,
                stage_vector: [0; 3],
                stages: Vec::new(),
                grid_value: (*this).grid_value,
                shift: (*this).shift,
            });
            p.cast()
        })
    }
}
thread_local! { static GRID_TABLE: RefCell<usize> = const { RefCell::new(0) }; }

fn prepare_grid(world: &mut World) {
    world._tables[0][0x34 / 4] = separate_grid as *const () as usize;
    GRID_TABLE.with(|table| *table.borrow_mut() = world._tables[1].as_ptr() as usize);
}

#[test]
fn core_capacity_and_depth_range_vectors_use_game_arithmetic() {
    let _fpu = GameFpu::new();
    for (tract, area, average, expected) in [
        (16, 4, 50, 200),
        (16, 6, 150, 400),
        (16, 9, 335, 595),
        (256, 3, 49154, 4_194_475),
        (4096, 3, 3074, 4_197_035),
        (4096, 6, 6145, 4_194_987),
        (4096, 9, 4616, 2_100_793),
    ] {
        assert_eq!(unsafe { capacity(tract, area, average) }, expected);
    }
    for (lo, hi, expected) in [
        (2, 2, (1, 3)),
        (3, 3, (1, 3)),
        (1, 2, (1, 3)),
        (2, 3, (1, 3)),
        (1, 1, (1, 3)),
        (3, 4, (2, 4)),
        (4, 4, (2, 4)),
        (5, 6, (4, 6)),
        (1, 3, (1, 3)),
        (1, 4, (1, 4)),
    ] {
        let mut world = World::new(1);
        let settings = world.settings();
        settings.set_minimum(lo);
        settings.set_maximum(hi);
        adjust_range(settings);
        assert_eq!((settings.minimum(), settings.maximum()), expected);
    }
}

#[test]
fn unsigned_remainders_keep_all_thirty_two_bits() {
    for dividend in [0, 1, 2, 3, 4, 31, 32, 0x7FFF_FFFF, 0x8000_0000, u32::MAX] {
        for divisor in [1, 2, 3, 8, 100, 0x8000_0000, u32::MAX] {
            assert_eq!(unsigned_remainder(dividend, divisor), dividend % divisor);
        }
    }
}

#[test]
fn developer_selection_ties_signed_desirability_truncated_demand_and_wrapping_centers() {
    for zone in [1, 2, 3, 4, 5, 6] {
        let mut world = World::new(zone);
        prepare_grid(&mut world);
        for dev in &mut world.developers {
            dev.desirability = -100;
            dev.demand = 500.75;
        }
        let candidates = if zone <= 3 {
            vec![0x1010, 0x1020, 0x1030]
        } else {
            vec![0x3110, 0x3120, 0x3130, 0x3320, 0x3330]
        };
        let last = *candidates.last().unwrap();
        world.developers.iter_mut().find(|dev| dev.id == last).unwrap().demand = 501.25;
        unsafe {
            let r = Region::from_ptr(world.region.ptr());
            r.set_x1(i32::MAX);
            r.set_x2(i32::MAX);
            r.set_z1(-5);
            r.set_z2(0);
        }
        assert_eq!(world.call(), 0xAABB_CCDD_5566_7788);
        let calls = take();
        let mut expected = vec![];
        for id in candidates {
            expected.extend([
                Call::Developer(id),
                Call::Estimate(id, -1, -2),
                Call::Source(id),
                Call::Demand(id),
            ]);
        }
        assert_eq!(&calls[..expected.len()], expected);
        assert!(calls.contains(&Call::HasStages(last)));
        assert!(calls.contains(&Call::Configurations([if zone <= 3 { 1 } else { 2 }, 1, 1])));
        assert_eq!(
            world.sizes(),
            [
                2,
                2,
                1,
                3,
                match zone {
                    1 => 6,
                    2 => 5,
                    3 => 4,
                    4 => 7,
                    5 => 6,
                    _ => 5,
                }
            ]
        );
    }
}

#[test]
fn no_developer_keeps_default_dimensions_and_density_zero_keeps_the_block_field() {
    for zone in [0, 7, 8, 9, 10, u32::MAX] {
        let mut world = World::new(zone);
        assert_eq!(world.call(), 0xAABB_CCDD_5566_7788);
        assert_eq!(
            world.sizes(),
            [2, 2, 1, 3, if (7..=9).contains(&zone) { 5 } else { 0x1234_5678 }]
        );
        let calls = take();
        assert!(!calls
            .iter()
            .any(|call| matches!(call, Call::Developer(..) | Call::Configurations(..))));
    }
    let mut world = World::new(0);
    STATE.with(|s| s.borrow_mut().null_buckets = true);
    assert_eq!(world.call(), 0xFACE_CAFE_0000_0000);
    assert!(!take().iter().any(|call| matches!(call, Call::Free(..))));
}

#[test]
fn stages_stop_at_the_first_value_no_greater_than_desirability() {
    for value in [99, 100, 101] {
        let mut world = World::new(1);
        prepare_grid(&mut world);
        world.developers[0].has_stages = true;
        world.developers[0].stages = vec![[3, 4, 7], [5, 6, 9]];
        world.developers[0].grid_value = value;
        world.call();
        let calls = take();
        assert!(calls.contains(&Call::Configurations([1, 1, if value > 100 { 9 } else { 1 }])));
        let values: Vec<_> = calls
            .iter()
            .filter(|call| matches!(call, Call::GridValue(..)))
            .cloned()
            .collect();
        assert_eq!(
            values,
            if value > 100 {
                vec![Call::GridValue(3, 4), Call::GridValue(5, 6)]
            } else {
                vec![Call::GridValue(3, 4)]
            }
        );
    }
}

#[test]
fn pointer_bucket_order_breaks_capacity_ties_and_every_configuration_is_released_in_that_order() {
    let mut world = World::new(1);
    prepare_grid(&mut world);
    world.config(2, 2, &[10], true);
    world.config(1, 3, &[20], true);
    world.profile(10, 17, 1, 1, 50);
    world.profile(20, 25, 1, 1, 38);
    // Values 200 and 202 are both one away from demand 201; the first pointer-table entry wins.
    let pointers: Vec<_> = STATE.with(|s| s.borrow().configs.clone());
    let first = if ((pointers[0] >> 2) % 8) < ((pointers[1] >> 2) % 8) {
        0
    } else {
        1
    };
    for dev in &mut world.developers {
        dev.demand = 201.0;
    }
    world.call();
    let calls = take();
    let order: Vec<_> = calls
        .iter()
        .filter_map(|call| if let Call::Property(id) = call { Some(*id) } else { None })
        .collect();
    assert_eq!(order.len(), 2);
    assert_eq!(
        calls
            .iter()
            .filter_map(|call| if let Call::Release(id) = call { Some(*id) } else { None })
            .collect::<Vec<_>>(),
        order
    );
    let expected = [world.configs[first].width, world.configs[first].depth];
    assert_eq!(&world.sizes()[..2], expected);
}

#[test]
fn shallow_size_filter_profiles_and_two_commercial_block_depths() {
    for zone in [4, 5, 6] {
        let mut world = World::new(zone);
        prepare_grid(&mut world);
        world.config(2, 3, &[10], true);
        world.config(2, 2, &[20], true);
        world.profile(10, 17, 2, 1, 75);
        world.profile(10, 18, 1, 1, 5000);
        world.profile(10, 19, 2, 2, 5000);
        world.profile(20, 25, 2, 1, 60);
        world.call();
        let sizes = world.sizes();
        let calls = take();
        assert_eq!(&sizes[..2], if zone == 4 { &[2, 3] } else { &[2, 2] });
        assert_eq!(calls.contains(&Call::Family(10)), zone == 4);
        assert_eq!(
            sizes[4],
            match zone {
                4 => 7,
                5 => 6,
                _ => 5,
            }
        );
    }
}

#[test]
fn wrapped_absolute_minimum_wins_and_demand_uses_the_game_float_to_integer_helper() {
    for demand in [f32::NAN, f32::INFINITY, 1e19, -2147483648.0] {
        let mut world = World::new(1);
        prepare_grid(&mut world);
        for developer in &mut world.developers {
            developer.demand = demand;
        }
        world.config(1, 3, &[10], true);
        world.profile(10, 33, 1, 1, 0);
        world.call();
        assert_eq!(&world.sizes()[..2], &[1, 3]);
        take();
    }
}

#[test]
fn medium_residential_depth_three_truncates_the_x87_block_product() {
    let mut world = World::new(2);
    prepare_grid(&mut world);
    world.config(2, 3, &[10], true);
    world.profile(10, 17, 1, 1, 75);
    world.call();
    assert_eq!(world.sizes(), [2, 3, 1, 3, 7]);
}

#[test]
fn missing_primary_buildings_and_excluded_sizes_are_still_released_and_widen_depth_range() {
    let mut world = World::new(1);
    prepare_grid(&mut world);
    world.config(1, 1, &[10], true);
    world.config(2, 4, &[20], true);
    world.config(4, 2, &[30], true);
    world.config(1, 6, &[], false);
    world.call();
    assert_eq!(world.sizes(), [2, 2, 1, 6, 6]);
    let calls = take();
    assert!(!calls.iter().any(|call| matches!(call, Call::Family(..))));
    let mut released: Vec<_> = calls
        .iter()
        .filter_map(|call| if let Call::Release(id) = call { Some(*id) } else { None })
        .collect();
    released.sort();
    assert_eq!(released, [0, 1, 2, 3]);
}

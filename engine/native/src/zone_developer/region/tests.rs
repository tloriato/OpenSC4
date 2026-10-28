// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Clone)]
enum Call {
    Allocate(u32),
    Construct([i32; 4], i32, i32, u8),
    Free(usize),
    Network(i32, i32, u32),
    Water(i32, i32),
    Occupied(i32, i32),
    GetLot(i32, i32, u32),
    Configuration(u32),
    Compatible(u32, u32),
    Zone(u32),
    Delete(u32),
    Force,
    Update(u32),
    LotState(u32),
    Texture(u32),
    Dezone([i32; 4], Vec<(i32, i32)>, [u32; 9]),
    Destroy(u32),
    AllocateQueue(u32),
    FreeQueue(u32, u32),
    DestroyQueue,
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    networks: Vec<(i32, i32)>,
    water: Vec<(i32, i32)>,
    occupied: Vec<(i32, i32)>,
    lots: BTreeMap<(i32, i32, u32), usize>,
    configs: BTreeMap<u32, usize>,
    compatible: bool,
    zones: BTreeMap<u32, i32>,
    new_zones: BTreeMap<u32, i32>,
    lot_states: BTreeMap<u32, i32>,
    force: u32,
    developer: usize,
    configuration: usize,
    zone_manager: usize,
    change_zone_after_water: Option<u32>,
    source: usize,
    mutate_source_on_get: bool,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn allocate_words(s: &mut State, count: usize, fill: u32) -> *mut u32 {
    let mut words = vec![fill; count].into_boxed_slice();
    let p = words.as_mut_ptr();
    s.allocations.push(words);
    p
}

extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        allocate_words(&mut s, size.div_ceil(4) as usize, 0xA5A5_A5A5).cast()
    })
}

extern "thiscall" fn construct_fake(this: *mut u8, width: i32, height: i32, fill: *const u8) -> u32 {
    let r = unsafe { Region::from_ptr(this.sub(16)) };
    let fill = unsafe { fill.read() };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls
            .push(Call::Construct([r.x1(), r.z1(), r.x2(), r.z2()], width, height, fill));
        let words = (height as u32).div_ceil(32);
        let columns = allocate_words(&mut s, width.max(0) as usize, 0).cast::<*mut u32>();
        for x in 0..width.max(0) as usize {
            unsafe {
                columns.add(x).write(allocate_words(
                    &mut s,
                    words as usize,
                    if fill == 0 { 0 } else { u32::MAX },
                ))
            };
        }
        r.set_vtable(0x00A8_6364);
        r.set_width(width);
        r.set_height(height);
        r.set_words(words);
        r.set_columns(columns);
    });
    this as u32
}

extern "C" fn free_fake(p: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.source != 0 {
            assert_eq!(unsafe { Region::from_ptr(s.source as *mut u8) }.vtable(), 0x00A8_6364);
        }
        s.calls.push(Call::Free(p as usize));
    });
    0xFACE_CAFE_1234_5678
}

extern "C" fn allocate_queue_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::AllocateQueue(size));
        allocate_words(&mut s, size.div_ceil(4) as usize, 0xA5A5_A5A5).cast()
    })
}

extern "C" fn free_queue_fake(node: *mut u8) {
    unsafe {
        let next = read_unaligned(node.cast::<*mut u8>());
        let previous = read_unaligned(node.add(4).cast::<*mut u8>());
        assert_eq!(read_unaligned(previous.cast::<*mut u8>()), next);
        assert_eq!(read_unaligned(next.add(4).cast::<*mut u8>()), previous);
        STATE.with(|s| {
            s.borrow_mut().calls.push(Call::FreeQueue(
                read_unaligned(node.add(8).cast()),
                read_unaligned(node.add(12).cast()),
            ))
        });
    }
}

extern "thiscall" fn destroy_queue_fake(list: *mut *mut u8) -> u64 {
    let header = unsafe { read_unaligned(list) };
    assert_eq!(unsafe { read_unaligned(header.cast::<*mut u8>()) }, header);
    assert_eq!(unsafe { read_unaligned(header.add(4).cast::<*mut u8>()) }, header);
    assert_eq!(unsafe { read_unaligned(header.add(8).cast::<u32>()) }, 0xA5A5_A5A5);
    assert_eq!(unsafe { read_unaligned(header.add(12).cast::<u32>()) }, 0xA5A5_A5A5);
    STATE.with(|s| s.borrow_mut().calls.push(Call::DestroyQueue));
    0xFEED_FACE_1234_5678
}

extern "thiscall" fn destroy_fake(this: *mut u8) -> u64 {
    let vtable = unsafe { read_unaligned(this.cast()) };
    STATE.with(|s| s.borrow_mut().calls.push(Call::Destroy(vtable)));
    0x89AB_CDEF_7654_3210
}

extern "thiscall" fn network_fake(this: *mut u8, x: i32, z: i32, mask: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.developer);
        s.calls.push(Call::Network(x, z, mask));
        0xF0F0_0000 | u32::from(s.networks.contains(&(x, z)))
    })
}

extern "thiscall" fn water_fake(this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.developer);
        s.calls.push(Call::Water(x, z));
        if let Some(zone) = s.change_zone_after_water.take() {
            unsafe { ZoneDeveloper::from_ptr(this) }.set_current_zone(zone);
        }
        0xF0F0_0000 | (u32::from(s.water.contains(&(x, z))) * 0x80)
    })
}

extern "thiscall" fn occupied_fake(this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.developer);
        s.calls.push(Call::Occupied(x, z));
        0xF0F0_0000 | u32::from(s.occupied.contains(&(x, z)))
    })
}

extern "thiscall" fn get_lot_fake(this: *mut u8, x: i32, z: i32, origin: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.developer + 4);
        s.calls.push(Call::GetLot(x, z, origin));
        if s.mutate_source_on_get {
            s.mutate_source_on_get = false;
            // A callback clears the next source cell. The following membership check must see it.
            unsafe { Region::from_ptr(s.source as *mut u8).set(1, 0, false) };
        }
        s.lots.get(&(x, z, origin)).copied().unwrap_or(0) as *mut u8
    })
}

unsafe fn object_id(p: *mut u8) -> u32 {
    unsafe { p.add(4).cast::<u32>().read() }
}

extern "thiscall" fn configuration_fake(this: *mut u8) -> *mut u8 {
    let id = unsafe { object_id(this) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Configuration(id));
        s.configs.get(&id).copied().unwrap_or(0) as *mut u8
    })
}

extern "thiscall" fn compatible_fake(this: *mut u8, zone: u32) -> u32 {
    let id = unsafe { object_id(this) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Compatible(id, zone));
        0xCAFE_0000 | (u32::from(s.compatible) * 0x80)
    })
}

extern "thiscall" fn zone_fake(this: *mut u8) -> i32 {
    let id = unsafe { object_id(this) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Zone(id));
        s.zones.get(&id).copied().unwrap_or(0)
    })
}

extern "thiscall" fn delete_fake(this: *mut u8, lot: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.developer + 4);
        s.calls.push(Call::Delete(unsafe { object_id(lot) }));
    });
}

extern "thiscall" fn force_fake(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.configuration);
        s.calls.push(Call::Force);
        s.force
    })
}

extern "thiscall" fn update_fake(this: *mut u8) {
    let id = unsafe { object_id(this) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Update(id));
        if let Some(zone) = s.new_zones.get(&id).copied() {
            s.zones.insert(id, zone);
        }
    });
}

extern "thiscall" fn lot_state_fake(this: *mut u8) -> i32 {
    let id = unsafe { object_id(this) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::LotState(id));
        s.lot_states.get(&id).copied().unwrap_or(0)
    })
}

extern "thiscall" fn texture_fake(this: *mut u8, lot: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.configuration);
        s.calls.push(Call::Texture(unsafe { object_id(lot) }));
    });
}

extern "thiscall" fn dezone_fake(
    this: *mut u8,
    region: *mut u8,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
    f: u32,
    g: u32,
    h: u32,
    i: u32,
) {
    let r = unsafe { Region::from_ptr(region) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.zone_manager);
        s.calls.push(Call::Dezone(
            [r.x1(), r.z1(), r.x2(), r.z2()],
            cells(r),
            [a, b, c, d, e, f, g, h, i],
        ));
    });
}

fn cells(r: Region) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for x in 0..r.width() {
        for z in 0..r.height() {
            if unsafe { r.get(x as u32, z as u32) } {
                cells.push((r.x1().wrapping_add(x), r.z1().wrapping_add(z)));
            }
        }
    }
    cells
}

struct NativeRegion {
    object: Box<[u32; 9]>,
    columns: Box<[*mut u32]>,
    rows: Vec<Box<[u32]>>,
}

impl NativeRegion {
    fn new(bounds: [i32; 4], set: &[(i32, i32)]) -> Self {
        let [x1, z1, x2, z2] = bounds;
        let (w, h) = (x2.wrapping_sub(x1).wrapping_add(1), z2.wrapping_sub(z1).wrapping_add(1));
        let mut rows: Vec<_> = (0..w.max(0))
            .map(|_| vec![0; (h.max(0) as usize).div_ceil(32)].into_boxed_slice())
            .collect();
        let columns = rows
            .iter_mut()
            .map(|r| r.as_mut_ptr())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut result = Self {
            object: Box::new([0; 9]),
            columns,
            rows,
        };
        let r = result.view();
        r.set_x1(x1);
        r.set_z1(z1);
        r.set_x2(x2);
        r.set_z2(z2);
        r.set_vtable(0x1234_5678);
        r.set_width(w);
        r.set_height(h);
        r.set_words((h.max(0) as u32).div_ceil(32));
        r.set_columns(result.columns.as_mut_ptr());
        for &(x, z) in set {
            unsafe { r.set(x.wrapping_sub(x1) as u32, z.wrapping_sub(z1) as u32, true) };
        }
        result
    }
    fn view(&mut self) -> Region {
        unsafe { Region::from_ptr(self.object.as_mut_ptr().cast()) }
    }
    fn bytes(&self) -> Vec<u32> {
        self.object
            .iter()
            .copied()
            .chain(self.rows.iter().flat_map(|r| r.iter().copied()))
            .collect()
    }
}

struct World {
    dev: Object,
    _primary: Box<[usize; 10]>,
    _manager: Box<[usize; 25]>,
    _configuration_table: Box<[usize; 47]>,
    _zone_table: Box<[usize; 12]>,
    _lot_table: Box<[usize; 34]>,
    _cfg_table: Box<[usize; 16]>,
    _configuration: Box<[usize; 1]>,
    _zone: Box<[usize; 1]>,
    objects: Vec<Box<[usize]>>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut primary = Box::new([0usize; 10]);
        primary[8] = network_fake as *const () as usize;
        primary[9] = water_fake as *const () as usize;
        let mut manager = Box::new([0usize; 25]);
        manager[6] = get_lot_fake as *const () as usize;
        manager[24] = delete_fake as *const () as usize;
        let mut config = Box::new([0usize; 47]);
        config[0xB8 / 4] = force_fake as *const () as usize;
        config[0xAC / 4] = texture_fake as *const () as usize;
        let mut zone_table = Box::new([0usize; 12]);
        zone_table[11] = dezone_fake as *const () as usize;
        let mut lot_table = Box::new([0usize; 34]);
        lot_table[9] = configuration_fake as *const () as usize;
        lot_table[31] = zone_fake as *const () as usize;
        lot_table[32] = update_fake as *const () as usize;
        lot_table[33] = lot_state_fake as *const () as usize;
        let mut cfg_table = Box::new([0usize; 16]);
        cfg_table[15] = compatible_fake as *const () as usize;
        let configuration = Box::new([config.as_ptr() as usize]);
        let zone = Box::new([zone_table.as_ptr() as usize]);
        let mut dev = Object::new(0xEC);
        let d = unsafe { ZoneDeveloper::from_ptr(dev.ptr()) };
        d.set_primary_vtable(primary.as_ptr() as u32);
        d.set_lot_manager_vtable(manager.as_ptr() as u32);
        d.set_lot_configuration(configuration.as_ptr() as *mut u8);
        d.set_zone_manager(zone.as_ptr() as *mut u8);
        d.set_current_zone(1);
        d.set_real_mode(1);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.developer = dev.ptr() as usize;
            s.configuration = configuration.as_ptr() as usize;
            s.zone_manager = zone.as_ptr() as usize;
        });
        Self {
            dev,
            _primary: primary,
            _manager: manager,
            _configuration_table: config,
            _zone_table: zone_table,
            _lot_table: lot_table,
            _cfg_table: cfg_table,
            _configuration: configuration,
            _zone: zone,
            objects: Vec::new(),
            _fakes: vec![
                fake(0x005E_55E0, allocate_fake as *const () as usize),
                fake(0x005E_5620, free_fake as *const () as usize),
                fake(0x0044_22A0, construct_fake as *const () as usize),
                fake(0x0044_1A60, destroy_fake as *const () as usize),
                fake(0x0072_C050, occupied_fake as *const () as usize),
                fake(0x0090_CF54, allocate_queue_fake as *const () as usize),
                fake(0x0090_CF63, free_queue_fake as *const () as usize),
                fake(0x0056_50A0, destroy_queue_fake as *const () as usize),
            ],
        }
    }
    fn lot(&mut self, id: u32, x: i32, z: i32, origin: u32, cfg: bool) {
        let mut object = vec![self._lot_table.as_ptr() as usize, id as usize].into_boxed_slice();
        let p = object.as_mut_ptr() as usize;
        self.objects.push(object);
        STATE.with(|s| {
            s.borrow_mut().lots.insert((x, z, origin), p);
        });
        if cfg {
            let mut object = vec![self._cfg_table.as_ptr() as usize, id as usize].into_boxed_slice();
            let p = object.as_mut_ptr() as usize;
            self.objects.push(object);
            STATE.with(|s| {
                s.borrow_mut().configs.insert(id, p);
            });
        }
    }
    fn dev(&mut self) -> ZoneDeveloper {
        unsafe { ZoneDeveloper::from_ptr(self.dev.ptr()) }
    }
}

fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

#[test]
fn compress_corner_region_preserves_holes_and_frees_columns_before_shell() {
    let _w = World::new();
    let mut source = NativeRegion::new([10, 20, 14, 54], &[(12, 51), (13, 51), (12, 54)]);
    let old = source.view();
    let mut slot = old.as_ptr();
    STATE.with(|s| s.borrow_mut().source = slot as usize);
    let result = unsafe { fc4p_0072af30(&mut slot) };
    assert_eq!(result, (0xFACE_CAFEu64 << 32) | &mut slot as *mut *mut u8 as u32 as u64);
    let r = unsafe { Region::from_ptr(slot) };
    assert_eq!([r.x1(), r.z1(), r.x2(), r.z2()], [12, 51, 13, 54]);
    assert_eq!(cells(r), [(12, 51), (12, 54), (13, 51)]);
    assert_eq!(unsafe { r.bit(0, 0).read() }, 0xFFFF_FFF9);
    let mut expected = vec![Call::Allocate(36), Call::Construct([12, 51, 13, 54], 2, 4, 1)];
    expected.extend(source.rows.iter().map(|r| Call::Free(r.as_ptr() as usize)));
    expected.push(Call::Free(source.columns.as_ptr() as usize));
    expected.push(Call::Free(old.as_ptr() as usize));
    assert_eq!(calls(), expected);
}

#[test]
fn compress_full_extent_and_single_or_zero_cell_empty_maps_keep_allocation() {
    let _w = World::new();
    for (bounds, set) in [
        ([0, 0, 4, 4], vec![(0, 4), (4, 0)]),
        ([3, 7, 3, 7], vec![]),
        ([3, 7, 2, 7], vec![]),
    ] {
        let mut source = NativeRegion::new(bounds, &set);
        let before = source.bytes();
        let p = source.view().as_ptr();
        let mut slot = p;
        unsafe { fc4p_0072af30(&mut slot) };
        assert_eq!(slot, p);
        assert_eq!(source.bytes(), before);
        assert!(calls().is_empty());
    }
}

#[test]
fn compress_empty_map_crops_to_one_clear_origin_cell() {
    let _w = World::new();
    let mut source = NativeRegion::new([-3, -5, 2, 40], &[]);
    let mut slot = source.view().as_ptr();
    unsafe { fc4p_0072af30(&mut slot) };
    let r = unsafe { Region::from_ptr(slot) };
    assert_eq!([r.x1(), r.z1(), r.x2(), r.z2()], [-3, -5, -3, -5]);
    assert!(cells(r).is_empty());
    assert_eq!(unsafe { r.bit(0, 0).read() }, 0xFFFF_FFFE);
}

#[test]
fn parcellizable_region_skips_clear_network_and_water_cells_in_x_major_order() {
    let mut w = World::new();
    let set = [(10, 20), (10, 21), (11, 20), (11, 21)];
    let mut source = NativeRegion::new([10, 20, 12, 21], &set);
    let before = source.bytes();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.networks.push((10, 21));
        s.water.push((11, 20));
    });
    let p = unsafe { fc4p_0072cb90(w.dev.ptr(), source.view().as_ptr()) };
    assert_eq!(cells(unsafe { Region::from_ptr(p) }), [(10, 20), (11, 21)]);
    assert_eq!(source.bytes(), before);
    assert_eq!(
        calls(),
        [
            Call::Allocate(36),
            Call::Construct([10, 20, 12, 21], 3, 2, 0),
            Call::Network(10, 20, 0x1F4F),
            Call::Water(10, 20),
            Call::GetLot(10, 20, 0),
            Call::Network(10, 21, 0x1F4F),
            Call::Network(11, 20, 0x1F4F),
            Call::Water(11, 20),
            Call::Network(11, 21, 0x1F4F),
            Call::Water(11, 21),
            Call::GetLot(11, 21, 0)
        ]
    );
}

#[test]
fn compatible_configurations_survive_and_incompatible_ones_are_deleted() {
    for compatible in [false, true] {
        let mut w = World::new();
        w.lot(42, 10, 20, 0, true);
        STATE.with(|s| s.borrow_mut().compatible = compatible);
        let mut source = NativeRegion::new([10, 20, 10, 20], &[(10, 20)]);
        let p = unsafe { fc4p_0072cb90(w.dev.ptr(), source.view().as_ptr()) };
        assert_eq!(cells(unsafe { Region::from_ptr(p) }).is_empty(), compatible);
        let mut expected = vec![
            Call::Allocate(36),
            Call::Construct([10, 20, 10, 20], 1, 1, 0),
            Call::Network(10, 20, 0x1F4F),
            Call::Water(10, 20),
            Call::GetLot(10, 20, 0),
            Call::Configuration(42),
            Call::Compatible(42, 1),
        ];
        if !compatible {
            expected.push(Call::Delete(42));
        }
        assert_eq!(calls(), expected);
    }
}

#[test]
fn unconfigured_lots_survive_only_when_both_zone_types_are_one_through_six() {
    for zone in [0, 1, 6, 7, u32::MAX] {
        for lot_zone in [-1, 0, 1, 6, 7] {
            let mut w = World::new();
            w.dev().set_current_zone(zone);
            w.lot(5, 0, 0, 0, false);
            STATE.with(|s| {
                s.borrow_mut().zones.insert(5, lot_zone);
            });
            let mut source = NativeRegion::new([0, 0, 0, 0], &[(0, 0)]);
            let p = unsafe { fc4p_0072cb90(w.dev.ptr(), source.view().as_ptr()) };
            let keep_lot = (1..=6).contains(&zone) && (1..=6).contains(&lot_zone);
            assert_eq!(cells(unsafe { Region::from_ptr(p) }).is_empty(), keep_lot);
            let calls = calls();
            assert_eq!(calls.contains(&Call::Zone(5)), (1..=6).contains(&zone));
            assert_eq!(calls.contains(&Call::Delete(5)), !keep_lot);
            assert_eq!(calls.iter().any(|c| matches!(c, Call::Network(..))), zone != 0);
        }
    }
}

#[test]
fn preview_mode_checks_occupancy_before_consulting_real_lots_and_reloads_current_zone() {
    let mut w = World::new();
    w.dev().set_real_mode(0);
    w.lot(1, 0, 0, 0, true);
    w.lot(2, 0, 1, 0, true);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.occupied.push((0, 1));
        s.compatible = true;
        s.change_zone_after_water = Some(7);
    });
    let mut source = NativeRegion::new([0, 0, 0, 1], &[(0, 0), (0, 1)]);
    let p = unsafe { fc4p_0072cb90(w.dev.ptr(), source.view().as_ptr()) };
    assert_eq!(cells(unsafe { Region::from_ptr(p) }), [(0, 0)]);
    assert_eq!(
        calls(),
        [
            Call::Allocate(36),
            Call::Construct([0, 0, 0, 1], 1, 2, 0),
            Call::Network(0, 0, 0x1F4F),
            Call::Water(0, 0),
            Call::Occupied(0, 0),
            Call::Network(0, 1, 0x1F4F),
            Call::Water(0, 1),
            Call::Occupied(0, 1),
            Call::GetLot(0, 1, 0),
            Call::Configuration(2),
            Call::Compatible(2, 7)
        ]
    );
}

#[test]
fn update_zone_type_updates_origins_and_dezones_only_uncovered_set_cells() {
    let mut w = World::new();
    w.lot(1, 10, 20, 1, false);
    w.lot(2, 11, 20, 0, false);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.zones.insert(1, 0);
        s.new_zones.insert(1, 1);
    });
    let mut source = NativeRegion::new([10, 20, 12, 21], &[(10, 20), (11, 20), (12, 20), (10, 21)]);
    let before = source.bytes();
    assert_eq!(
        unsafe { fc4p_0072b180(w.dev.ptr(), source.view().as_ptr()) },
        0x89AB_CDEF_7654_3210
    );
    assert_eq!(source.bytes(), before);
    assert_eq!(
        calls(),
        [
            Call::Construct([10, 20, 12, 21], 3, 2, 0),
            Call::Force,
            Call::GetLot(10, 20, 1),
            Call::Zone(1),
            Call::Update(1),
            Call::Zone(1),
            Call::LotState(1),
            Call::Texture(1),
            Call::GetLot(11, 20, 1),
            Call::GetLot(11, 20, 0),
            Call::GetLot(12, 20, 1),
            Call::GetLot(12, 20, 0),
            Call::GetLot(10, 21, 1),
            Call::GetLot(10, 21, 0),
            Call::Dezone([10, 20, 12, 21], vec![(10, 21), (12, 20)], [0, 1, 1, 0, 1, 0, 0, 0, 0]),
            Call::Destroy(0x00A8_6364)
        ]
    );
}

#[test]
fn texture_refresh_uses_low_force_byte_signed_lot_state_and_zone_change() {
    for (force, lot_state, changed, refresh) in [
        (0, 0, true, true),
        (0, 1, true, true),
        (0, 2, true, false),
        (0, -1, true, true),
        (0x100, 2, true, false),
        (0x80, 2, true, true),
        (1, 2, false, false),
    ] {
        let mut w = World::new();
        w.lot(1, 0, 0, 1, false);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.force = force;
            s.lot_states.insert(1, lot_state);
            s.new_zones.insert(1, i32::from(changed));
        });
        let mut source = NativeRegion::new([0, 0, 0, 0], &[(0, 0)]);
        unsafe { fc4p_0072b180(w.dev.ptr(), source.view().as_ptr()) };
        let c = calls();
        assert_eq!(c.contains(&Call::Texture(1)), refresh);
        assert_eq!(c.contains(&Call::LotState(1)), changed && force as u8 == 0);
        assert!(!c.iter().any(|c| matches!(c, Call::Dezone(..))));
        assert_eq!(c.last(), Some(&Call::Destroy(0x00A8_6364)));
    }
}

#[test]
fn update_observes_source_bit_changes_made_by_lot_callbacks() {
    let mut w = World::new();
    let mut source = NativeRegion::new([0, 0, 1, 0], &[(0, 0), (1, 0)]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.source = source.view().as_ptr() as usize;
        s.mutate_source_on_get = true;
    });
    unsafe { fc4p_0072b180(w.dev.ptr(), source.view().as_ptr()) };
    assert_eq!(
        calls(),
        [
            Call::Construct([0, 0, 1, 0], 2, 1, 0),
            Call::Force,
            Call::GetLot(0, 0, 1),
            Call::GetLot(0, 0, 0),
            Call::Dezone([0, 0, 1, 0], vec![(0, 0)], [0, 1, 1, 0, 1, 0, 0, 0, 0]),
            Call::Destroy(0x00A8_6364)
        ]
    );
}

#[test]
fn extract_connected_cells_uses_fifo_neighbour_order_and_leaves_diagonal_cells() {
    let _w = World::new();
    let set = [(1, 1), (2, 1), (1, 2), (0, 1), (1, 0), (0, 0), (3, 3)];
    let mut source = NativeRegion::new([10, 20, 13, 23], &set.map(|(x, z)| (x + 10, z + 20)));
    let result = unsafe { fc4p_0072cda0(source.view().as_ptr(), 1, 1) };
    assert_eq!(result >> 32, 0xFEED_FACE);
    let r = unsafe { Region::from_ptr(result as u32 as *mut u8) };
    assert_eq!(cells(source.view()), [(13, 23)]);
    assert_eq!(cells(r), [(10, 20), (10, 21), (11, 20), (11, 21), (11, 22), (12, 21)]);
    assert_eq!(
        calls(),
        [
            Call::Allocate(36),
            Call::Construct([10, 20, 13, 23], 4, 4, 0),
            Call::AllocateQueue(16),
            Call::AllocateQueue(16),
            Call::FreeQueue(1, 1),
            Call::AllocateQueue(16),
            Call::AllocateQueue(16),
            Call::AllocateQueue(16),
            Call::AllocateQueue(16),
            Call::FreeQueue(2, 1),
            Call::FreeQueue(1, 2),
            Call::FreeQueue(0, 1),
            Call::AllocateQueue(16),
            Call::FreeQueue(1, 0),
            Call::FreeQueue(0, 0),
            Call::DestroyQueue
        ]
    );
}

#[test]
fn clear_seed_extracts_clear_component_and_preserves_true_tail_bits() {
    let _w = World::new();
    let mut source = NativeRegion::new([0, 0, 2, 1], &[(1, 0), (1, 1)]);
    let result = unsafe { fc4p_0072cda0(source.view().as_ptr(), 0, 0) };
    let r = unsafe { Region::from_ptr(result as u32 as *mut u8) };
    assert_eq!(cells(source.view()), [(0, 0), (0, 1), (1, 0), (1, 1)]);
    assert_eq!(cells(r), [(1, 0), (1, 1), (2, 0), (2, 1)]);
    assert_eq!(unsafe { r.bit(0, 0).read() }, 0xFFFF_FFFC);
    assert_eq!(
        calls(),
        [
            Call::Allocate(36),
            Call::Construct([0, 0, 2, 1], 3, 2, 1),
            Call::AllocateQueue(16),
            Call::AllocateQueue(16),
            Call::FreeQueue(0, 0),
            Call::AllocateQueue(16),
            Call::FreeQueue(0, 1),
            Call::DestroyQueue
        ]
    );
}

#[test]
fn region_components_and_compression_match_corner_and_isolated_cell_cases() {
    let _w = World::new();
    let mut source = NativeRegion::new([0, 0, 4, 4], &[(0, 0), (0, 1), (4, 4)]);
    for (x, z, bounds, expected) in [
        (0, 0, [0, 0, 0, 1], vec![(0, 0), (0, 1)]),
        (4, 4, [4, 4, 4, 4], vec![(4, 4)]),
    ] {
        let mut slot = unsafe { fc4p_0072cda0(source.view().as_ptr(), x, z) } as u32 as *mut u8;
        unsafe { fc4p_0072af30(&mut slot) };
        let r = unsafe { Region::from_ptr(slot) };
        assert_eq!([r.x1(), r.z1(), r.x2(), r.z2()], bounds);
        assert_eq!(cells(r), expected);
    }
    assert!(cells(source.view()).is_empty());
}

#[test]
fn connected_component_crosses_packed_word_boundary_without_touching_other_bits() {
    let _w = World::new();
    let mut source = NativeRegion::new([-2, -10, -1, 24], &[(-2, 20), (-2, 21), (-2, 22), (-1, 22), (-1, -10)]);
    let result = unsafe { fc4p_0072cda0(source.view().as_ptr(), 0, 31) };
    let r = unsafe { Region::from_ptr(result as u32 as *mut u8) };
    assert_eq!(cells(r), [(-2, 20), (-2, 21), (-2, 22), (-1, 22)]);
    assert_eq!(cells(source.view()), [(-1, -10)]);
}

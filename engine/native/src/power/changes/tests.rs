// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Framework,
    QueryAllocator(u32, u32),
    ReleaseReference,
    Allocate(u32),
    Connections(u32, u32),
    Width,
    Height,
    Cell(u32),
    Visited(u32, u32),
    Pixel(u32, u32, u32),
    Clear,
    Free,
    QueryOccupants([i32; 2], [i32; 2], u32, u32),
    Kind(u32),
    Query(u32, u32),
    Release(u32),
    Bounds(u32),
    Flags(u32),
    ObjectCell(u32),
    Expand(Rect),
    SetWire(Rect),
    Neighborhood(u32, u32),
    Zone(u32, u32),
    RectSet(Rect),
    PixelTest(u32, u32, bool),
    Grid(u32, u32, u32),
    ManagerRef(u32),
    Break(u32, u32),
    Remove(u32, u32, u32),
    HeapAllocate(u32),
    HeapFree,
    Endpoints(u32),
    RemoveConnection(u32, u32, u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    // Linked nodes keep their addresses when the allocation log grows.
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 3]>>,
    connections: Vec<usize>,
    allocator: usize,
    occupants: Vec<usize>,
    query_ok: bool,
    zone: Option<(u32, u32)>,
    powered: usize,
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
        let mut node = Box::new([0xABCD_1234; 3]);
        let p = node.as_mut_ptr().cast();
        s.nodes.push(node);
        p
    })
}

#[repr(C)]
struct Service {
    vtable: *const usize,
    width: i32,
    height: i32,
}
#[repr(C)]
struct Connection {
    vtable: *const usize,
    x: i32,
    z: i32,
    id: u32,
}
extern "C" fn get_framework() -> *mut u8 {
    log(Call::Framework);
    STATE.with(|s| s.borrow().allocator as *mut u8)
}
unsafe extern "thiscall" fn query_allocator(_this: *mut u8, cls: u32, iid: u32, out: *mut u32) {
    log(Call::QueryAllocator(cls, iid));
    unsafe {
        out.write(STATE.with(|s| s.borrow().allocator as u32));
    }
}
extern "thiscall" fn release_reference_fake(_this: *mut u32) {
    log(Call::ReleaseReference);
}
extern "thiscall" fn allocate(_this: *mut u8, size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    node()
}
extern "C" fn free_head_result() -> u64 {
    log(Call::Free);
    0x4455_6677_AABB_CCDD
}
#[unsafe(naked)]
unsafe extern "thiscall" fn free_head(_this: *mut u8, _head: u32) -> u64 {
    core::arch::naked_asm!(
        "call {body}", "mov ecx, 0x13579bdf", "ret 4", body = sym free_head_result,
    );
}
unsafe extern "thiscall" fn width(this: *mut u8) -> i32 {
    log(Call::Width);
    unsafe { (*this.cast::<Service>()).width }
}
unsafe extern "thiscall" fn height(this: *mut u8) -> i32 {
    log(Call::Height);
    unsafe { (*this.cast::<Service>()).height }
}
unsafe extern "thiscall" fn get_connections(_this: *mut u8, kind: u32, city: u32, list: *mut u32) {
    log(Call::Connections(kind, city));
    let connections = STATE.with(|s| s.borrow().connections.clone());
    unsafe {
        let head = list.read() as *mut u8;
        for c in connections {
            let n = node();
            let prev = pointer(head, 4);
            n.cast::<u32>().write(head as u32);
            n.add(4).cast::<u32>().write(prev as u32);
            n.add(8).cast::<u32>().write(c as u32);
            prev.cast::<u32>().write(n as u32);
            head.add(4).cast::<u32>().write(n as u32);
        }
    }
}
unsafe extern "thiscall" fn connection_cell(this: *mut u8, x: *mut i32, z: *mut i32) {
    unsafe {
        let c = &*this.cast::<Connection>();
        log(Call::Cell(c.id));
        x.write(c.x);
        z.write(c.z);
    }
}
unsafe extern "thiscall" fn visited(this: *mut u8, on: u32) {
    unsafe {
        log(Call::Visited((*this.cast::<Connection>()).id, on));
    }
}
extern "C" fn set_pixel(_bits: *mut u32, width: u32, x: u32, z: u32) {
    log(Call::Pixel(width, x, z));
}
extern "thiscall" fn clear(_this: *mut u32) {
    log(Call::Clear);
}

struct Fixture {
    sim: Object,
    city: Box<Service>,
    allocator: Box<Service>,
    // Tables and globals must stay at fixed addresses while the fakes are installed.
    _service_table: Box<[usize; 96]>,
    connection_table: Box<[usize; 16]>,
    _allocator_cache: Box<*mut u8>,
    _global_city: Box<*mut u8>,
    _fakes: Vec<Fake>,
    bits: Vec<u32>,
    connections: Vec<Connection>,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut table = Box::new([0usize; 96]);
        table[3] = allocate as *const () as usize;
        table[1] = manager_addref as *const () as usize;
        table[2] = manager_release as *const () as usize;
        table[4] = free_head as *const () as usize;
        table[5] = query_allocator as *const () as usize;
        table[0x48 / 4] = get_connections as *const () as usize;
        table[0x174 / 4] = width as *const () as usize;
        table[0x178 / 4] = height as *const () as usize;
        table[0xA0 / 4] = query_occupants as *const () as usize;
        table[0x30 / 4] = zone as *const () as usize;
        table[0x68 / 4] = grid as *const () as usize;
        table[0x54 / 4] = remove_occupant as *const () as usize;
        let mut ct = Box::new([0usize; 16]);
        ct[0x18 / 4] = connection_cell as *const () as usize;
        ct[0x30 / 4] = visited as *const () as usize;
        let mut city = Box::new(Service {
            vtable: table.as_ptr(),
            width: 64,
            height: 64,
        });
        let mut alloc = Box::new(Service {
            vtable: table.as_ptr(),
            width: 0,
            height: 0,
        });
        let alloc_ptr = (&mut *alloc as *mut Service).cast::<u8>();
        STATE.with(|s| s.borrow_mut().allocator = alloc_ptr as usize);
        let mut cache = Box::new(alloc_ptr);
        let mut global = Box::new((&mut *city as *mut Service).cast::<u8>());
        let fakes = vec![
            fake_data(0x00B2_07DC, (&mut *cache as *mut *mut u8).cast()),
            fake_data(0x00B4_3CEC, (&mut *global as *mut *mut u8).cast()),
            fake(0x0087_93EC, get_framework as *const () as usize),
            fake(0x0041_1870, release_reference_fake as *const () as usize),
            fake(0x0041_1D70, clear as *const () as usize),
            fake(0x0060_3DA0, set_pixel as *const () as usize),
            fake_data(0x00B2_07E4, (&mut *cache as *mut *mut u8).cast()),
            fake_data(0x00B4_3D0C, (&mut *global as *mut *mut u8).cast()),
            fake(0x0041_3E60, clear as *const () as usize),
            fake(0x006E_AE20, expand_fake as *const () as usize),
            fake(0x006E_AFD0, wire_fake as *const () as usize),
            fake(0x006E_AD70, neighborhood_fake as *const () as usize),
            fake(0x0060_4180, rect_set_fake as *const () as usize),
            fake(0x0060_3DD0, pixel_fake as *const () as usize),
            fake(0x0090_CF54, heap_allocate as *const () as usize),
            fake(0x0090_CF63, heap_free as *const () as usize),
            fake(0x0064_DA00, remove_connection_fake as *const () as usize),
        ];
        let mut sim = Object::new(0x400);
        let view = unsafe { ChangeSim::from_ptr(sim.ptr()) };
        view.set_city((&mut *city as *mut Service).cast());
        view.set_neighbors(alloc_ptr);
        view.set_grid(alloc_ptr);
        unsafe {
            PowerSim::from_ptr(sim.ptr()).set_zones(alloc_ptr);
        }
        let mut bits = vec![0; 128];
        view.set_selected(bits.as_mut_ptr());
        Fixture {
            sim,
            city,
            allocator: alloc,
            _service_table: table,
            connection_table: ct,
            _allocator_cache: cache,
            _global_city: global,
            _fakes: fakes,
            bits,
            connections: Vec::new(),
        }
    }
    fn add(&mut self, x: i32, z: i32) {
        let id = self.connections.len() as u32;
        self.connections.push(Connection {
            vtable: self.connection_table.as_ptr(),
            x,
            z,
            id,
        });
    }
    fn select(&mut self, x: usize, z: usize) {
        self.bits[z * 2 + x / 32] |= 1 << (x % 32);
    }
    fn run(&mut self, city: u32, mark: u32, power: u32) -> (u32, u32, u32) {
        STATE.with(|s| {
            s.borrow_mut().connections = self
                .connections
                .iter_mut()
                .map(|c| c as *mut Connection as usize)
                .collect()
        });
        let mut record = [
            fc4p_006edb40 as *const () as u32,
            self.sim.ptr() as u32,
            city,
            mark,
            power,
            0,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!(
                "push [edi + 16]", "push [edi + 12]", "push [edi + 8]",
                "mov ecx, [edi + 4]", "call dword ptr [edi]",
                "mov [edi + 20], eax", "mov [edi + 24], edx",
                "mov eax, [esp - 12]", "mov [edi + 28], eax",
                in("edi") record.as_mut_ptr(), clobber_abi("C"),
            );
        }
        (record[5], record[6], record[7])
    }
}

#[test]
fn connection_search_empty_and_lazy_allocator() {
    let mut f = Fixture::new();
    *f._allocator_cache = core::ptr::null_mut();
    assert_eq!(f.run(25, 0, 0), (0xAABB_CC00, 0x4455_6677, 25));
    assert_eq!(
        calls(),
        [
            Call::Framework,
            Call::QueryAllocator(0x3AE4_BEA3, 0x3AE4_BEAB),
            Call::ReleaseReference,
            Call::Allocate(12),
            Call::Connections(4, 25),
            Call::Clear,
            Call::Free
        ]
    );
    assert_eq!(*f._allocator_cache, (&mut *f.allocator as *mut Service).cast());
}

#[test]
fn connection_search_clamps_and_stops_at_first_selected_cell() {
    let mut f = Fixture::new();
    f.add(3, 4);
    f.add(-5, 1000);
    f.add(2, 3);
    f.select(0, 63);
    assert_eq!(
        f.run(0x1122_3344, 0xABCD_0001, 0xEEFF_0001),
        (0xAABB_CC01, 0x4455_6677, 63)
    );
    assert_eq!(
        calls(),
        [
            Call::Allocate(12),
            Call::Connections(4, 0x1122_3344),
            Call::Width,
            Call::Height,
            Call::Cell(0),
            Call::Width,
            Call::Width,
            Call::Height,
            Call::Cell(1),
            Call::Width,
            Call::Visited(1, 1),
            Call::Width,
            Call::Pixel(64, 0, 63),
            Call::Clear,
            Call::Free
        ]
    );
}

#[test]
fn connection_search_uses_flag_low_bytes_and_checks_every_miss() {
    let mut f = Fixture::new();
    f.add(90, -20);
    f.add(8, 9);
    assert_eq!(f.run(7, 0x100, 0x100).0 as u8, 0);
    assert_eq!(
        calls(),
        [
            Call::Allocate(12),
            Call::Connections(4, 7),
            Call::Width,
            Call::Height,
            Call::Cell(0),
            Call::Width,
            Call::Width,
            Call::Height,
            Call::Cell(1),
            Call::Width,
            Call::Clear,
            Call::Free
        ]
    );
    f.select(8, 9);
    assert_eq!(f.run(7, 0x100, 0x100), (0xAABB_CC01, 0x4455_6677, 9));
    assert!(!calls().iter().any(|c| matches!(c, Call::Visited(..) | Call::Pixel(..))));
}

#[test]
fn connection_search_signed_width_quotient() {
    let mut f = Fixture::new();
    f.city.width = -1;
    f.bits[0] = 1 << 30;
    // A width of -1 gives a row stride of zero through signed division.
    let view = unsafe { ChangeSim::from_ptr(f.sim.ptr()) };
    assert!(unsafe { selected(view, 30, u32::MAX) });
    assert_eq!(calls(), [Call::Width]);
}

#[repr(C)]
struct Occupant {
    table: *const usize,
    id: u32,
    kind: u32,
    x: u32,
    z: u32,
    rect: Rect,
    query_ok: bool,
    flags: u32,
    endpoints: [*mut u8; 2],
}
fn occupant_table() -> Box<[usize; 96]> {
    let mut t = Box::new([0; 96]);
    t[0] = object_query as *const () as usize;
    t[2] = object_release as *const () as usize;
    t[0x1C / 4] = object_kind as *const () as usize;
    t[0x2C / 4] = object_bounds as *const () as usize;
    t[0x30 / 4] = object_flags as *const () as usize;
    t[0x18 / 4] = break_pole as *const () as usize;
    t[0x0C / 4] = endpoints as *const () as usize;
    for slot in [0x14, 0x20, 0xEC] {
        t[slot / 4] = object_cell as *const () as usize;
    }
    t
}
unsafe extern "thiscall" fn object_kind(this: *mut u8) -> u32 {
    let o = unsafe { &*this.cast::<Occupant>() };
    log(Call::Kind(o.id));
    o.kind
}
unsafe extern "thiscall" fn object_query(this: *mut u8, iid: u32, out: *mut u32) -> u32 {
    let o = unsafe { &*this.cast::<Occupant>() };
    log(Call::Query(o.id, iid));
    let ok = o.query_ok
        && matches!(
            (o.kind, iid),
            (0x2890_D4DE, 0xA8C8_6142) | (0x0990_B2EA, 0xC98F_3B24) | (0xC772_BF98, 0xA821_EF94)
        );
    if ok {
        unsafe {
            out.write(this as u32);
        }
    }
    0x100 | u32::from(ok)
}
unsafe extern "thiscall" fn object_release(this: *mut u8) -> u32 {
    let o = unsafe { &*this.cast::<Occupant>() };
    log(Call::Release(o.id));
    0x1234_5678
}
unsafe extern "thiscall" fn object_bounds(this: *mut u8, rect: *mut Rect) {
    let o = unsafe { &*this.cast::<Occupant>() };
    log(Call::Bounds(o.id));
    unsafe {
        rect.write(o.rect);
    }
}
unsafe extern "thiscall" fn object_flags(this: *mut u8) -> u32 {
    let o = unsafe { &*this.cast::<Occupant>() };
    log(Call::Flags(o.id));
    o.flags
}
unsafe extern "thiscall" fn object_cell(this: *mut u8, x: *mut u32, z: *mut u32) {
    let o = unsafe { &*this.cast::<Occupant>() };
    log(Call::ObjectCell(o.id));
    unsafe {
        x.write(o.x);
        z.write(o.z);
    }
}
unsafe extern "thiscall" fn query_occupants(
    _this: *mut u8,
    list: *mut u32,
    low: *mut i32,
    high: *mut i32,
    flag: u32,
    mask: u32,
) -> u32 {
    unsafe {
        log(Call::QueryOccupants(
            [low.read(), low.add(1).read()],
            [high.read(), high.add(1).read()],
            flag,
            mask,
        ));
        let occupants = STATE.with(|s| s.borrow().occupants.clone());
        let head = list.read() as *mut u8;
        for occupant in occupants {
            let n = node();
            let prev = pointer(head, 4);
            n.cast::<u32>().write(head as u32);
            n.add(4).cast::<u32>().write(prev as u32);
            n.add(8).cast::<u32>().write(occupant as u32);
            prev.cast::<u32>().write(n as u32);
            head.add(4).cast::<u32>().write(n as u32);
        }
        0x100 | u32::from(STATE.with(|s| s.borrow().query_ok))
    }
}
unsafe extern "thiscall" fn expand_fake(_this: *mut u8, rect: *mut Rect) {
    unsafe {
        log(Call::Expand(rect.read()));
        (*rect).x0 = (*rect).x0.wrapping_sub(2);
        (*rect).z0 = (*rect).z0.wrapping_sub(2);
        (*rect).x1 = (*rect).x1.wrapping_add(2);
        (*rect).z1 = (*rect).z1.wrapping_add(2);
    }
}
unsafe extern "thiscall" fn wire_fake(_this: *mut u8, rect: *mut Rect) {
    log(Call::SetWire(unsafe { rect.read() }));
}
unsafe extern "thiscall" fn neighborhood_fake(_this: *mut u8, x: u32, z: u32, rect: *mut Rect) {
    log(Call::Neighborhood(x, z));
    unsafe {
        rect.write(Rect {
            x0: x,
            z0: z,
            x1: x + 1,
            z1: z + 1,
        });
    }
}
unsafe extern "C" fn rect_set_fake(_bits: *mut u32, _width: u32, rect: *mut Rect) {
    log(Call::RectSet(unsafe { rect.read() }));
}
unsafe extern "C" fn pixel_fake(bits: *mut u32, width: u32, x: u32, z: u32) -> u32 {
    let powered = STATE.with(|s| s.borrow().powered == bits as usize);
    log(Call::PixelTest(x, z, powered));
    0xAABB_CC00 | u32::from(unsafe { bits.add((width / 32 * z + x / 32) as usize).read() } & (1 << (x & 31)) != 0)
}
extern "thiscall" fn zone(_this: *mut u8, x: u32, z: u32) -> u32 {
    log(Call::Zone(x, z));
    0xABCD_EF00 | u32::from(STATE.with(|s| s.borrow().zone == Some((x, z))))
}
extern "thiscall" fn grid(_this: *mut u8, x: u32, z: u32, value: u32) {
    log(Call::Grid(x, z, value));
}
impl Fixture {
    fn wire(&mut self, rect: &mut Rect, exclude: *mut u8, seed: u32) -> u64 {
        let mut record = [
            fc4p_006ed6e0 as *const () as u32,
            self.sim.ptr() as u32,
            rect as *mut Rect as u32,
            exclude as u32,
            seed,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!(
                "push [edi + 12]", "push [edi + 8]", "mov eax, [edi + 16]",
                "mov [esp - 0x78], eax", "mov ecx, [edi + 4]", "call dword ptr [edi]",
                "mov [edi + 20], eax", "mov [edi + 24], edx",
                in("edi") record.as_mut_ptr(), clobber_abi("C"),
            );
        }
        (u64::from(record[6]) << 32) | u64::from(record[5])
    }
}

#[test]
fn wire_rebuild_publishes_0_1_2_with_the_original_argument_upper_bytes() {
    let mut f = Fixture::new();
    let mut powered = vec![0u32; 128];
    powered[0] = 0b101;
    f.bits[0] = 0b110;
    unsafe {
        let power = PowerSim::from_ptr(f.sim.ptr());
        power.set_radius(2);
        power.set_wire(f.bits.as_mut_ptr());
        power.set_powered(powered.as_mut_ptr());
    }
    STATE.with(|s| s.borrow_mut().powered = powered.as_ptr() as usize);
    let mut r = Rect {
        x0: 0,
        z0: 0,
        x1: 2,
        z1: 0,
    };
    assert_eq!(
        f.wire(&mut r, core::ptr::null_mut(), 0x1234_5678),
        0x4455_6677_AABB_CCDD
    );
    assert_eq!(
        calls(),
        [
            Call::Allocate(12),
            Call::QueryOccupants([-4, -4], [6, 4], 0, u32::MAX),
            Call::Width,
            Call::Zone(0, 0),
            Call::Zone(1, 0),
            Call::Zone(2, 0),
            Call::PixelTest(0, 0, false),
            Call::PixelTest(0, 0, true),
            Call::Grid(0, 0, 0x1234_5600),
            Call::PixelTest(1, 0, false),
            Call::PixelTest(1, 0, true),
            Call::Grid(1, 0, 0x1234_5601),
            Call::PixelTest(2, 0, false),
            Call::PixelTest(2, 0, true),
            Call::Grid(2, 0, 0x1234_5602),
            Call::Clear,
            Call::Free
        ]
    );
}

#[test]
fn wire_rebuild_visits_kinds_in_order_releases_exclusions_and_keeps_network_cells_unexpanded() {
    let mut f = Fixture::new();
    let t = occupant_table();
    let r = Rect {
        x0: 10,
        z0: 11,
        x1: 12,
        z1: 13,
    };
    let mut objects: Vec<_> = [0x2781_28A0, 0x2890_D4DE, 0x0990_B2EA, 0xC772_BF98, 1, 0x2781_28A0]
        .into_iter()
        .enumerate()
        .map(|(id, kind)| Occupant {
            table: t.as_ptr(),
            id: id as u32,
            kind,
            x: 20,
            z: 21,
            rect: r,
            query_ok: true,
            flags: 0x40000,
            endpoints: [core::ptr::null_mut(); 2],
        })
        .collect();
    let exclude = (&mut objects[5] as *mut Occupant).cast();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.occupants = objects.iter_mut().map(|o| o as *mut Occupant as usize).collect();
        s.query_ok = true;
    });
    // Reversed bounds keep this test focused on occupant effects.
    let mut reversed = Rect {
        x0: 2,
        z0: 2,
        x1: 1,
        z1: 1,
    };
    f.wire(&mut reversed, exclude, 0xDEAD_BEEF);
    let actual = calls();
    let point = Rect {
        x0: 20,
        z0: 21,
        x1: 20,
        z1: 21,
    };
    let expanded = Rect {
        x0: 18,
        z0: 19,
        x1: 22,
        z1: 23,
    };
    assert_eq!(
        &actual[2..actual.len() - 3],
        [
            Call::Kind(0),
            Call::Kind(0),
            Call::Bounds(0),
            Call::Expand(r),
            Call::SetWire(Rect {
                x0: 8,
                z0: 9,
                x1: 14,
                z1: 15
            }),
            Call::Release(0),
            Call::Kind(1),
            Call::Kind(1),
            Call::Kind(1),
            Call::Query(1, 0xA8C8_6142),
            Call::ObjectCell(1),
            Call::Expand(point),
            Call::SetWire(expanded),
            Call::Release(1),
            Call::Release(1),
            Call::Kind(2),
            Call::Kind(2),
            Call::Kind(2),
            Call::Kind(2),
            Call::Query(2, 0xC98F_3B24),
            Call::ObjectCell(2),
            Call::Expand(point),
            Call::SetWire(expanded),
            Call::Release(2),
            Call::Release(2),
            Call::Kind(3),
            Call::Kind(3),
            Call::Kind(3),
            Call::Kind(3),
            Call::Kind(3),
            Call::Query(3, 0xA821_EF94),
            Call::Flags(3),
            Call::ObjectCell(3),
            Call::SetWire(point),
            Call::Release(3),
            Call::Release(3),
            Call::Kind(4),
            Call::Kind(4),
            Call::Kind(4),
            Call::Kind(4),
            Call::Release(4),
            Call::Release(5),
        ]
    );
}

#[test]
fn wire_rebuild_zones_use_the_changed_rectangle_and_call_the_bitmap_setter_directly() {
    let mut f = Fixture::new();
    let mut powered = vec![0; 128];
    unsafe {
        let p = PowerSim::from_ptr(f.sim.ptr());
        p.set_wire(f.bits.as_mut_ptr());
        p.set_powered(powered.as_mut_ptr());
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.zone = Some((11, 15));
        s.powered = powered.as_ptr() as usize;
    });
    let mut r = Rect {
        x0: 10,
        z0: 14,
        x1: 12,
        z1: 16,
    };
    f.wire(&mut r, core::ptr::null_mut(), 0);
    let actual = calls();
    let i = actual.iter().position(|c| *c == Call::Neighborhood(11, 15)).unwrap();
    assert_eq!(
        &actual[i - 1..i + 4],
        [
            Call::Zone(11, 15),
            Call::Neighborhood(11, 15),
            Call::Width,
            Call::RectSet(Rect {
                x0: 11,
                z0: 15,
                x1: 12,
                z1: 16
            }),
            Call::Zone(11, 16)
        ]
    );
    let cells: Vec<_> = actual
        .iter()
        .filter_map(|c| {
            if let Call::Grid(x, z, _) = c {
                Some((*x, *z))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        cells,
        [
            (10, 14),
            (10, 15),
            (10, 16),
            (11, 14),
            (11, 15),
            (11, 16),
            (12, 14),
            (12, 15),
            (12, 16)
        ]
    );
}

extern "thiscall" fn manager_addref(_this: *mut u8) {
    log(Call::ManagerRef(4));
}
extern "thiscall" fn manager_release(_this: *mut u8) -> u32 {
    log(Call::ManagerRef(8));
    0x89AB_CDEF
}
unsafe extern "thiscall" fn break_pole(this: *mut u8, flag: u32) {
    log(Call::Break(unsafe { (*this.cast::<Occupant>()).id }, flag));
}
unsafe extern "thiscall" fn remove_occupant(_this: *mut u8, occupant: *mut u8, a: u32, b: u32) {
    log(Call::Remove(unsafe { (*occupant.cast::<Occupant>()).id }, a, b));
}
extern "C" fn heap_allocate(size: u32) -> *mut u8 {
    log(Call::HeapAllocate(size));
    node()
}
extern "C" fn heap_free(_node: *mut u8) {
    log(Call::HeapFree);
}
unsafe extern "thiscall" fn endpoints(this: *mut u8, a: *mut u32, b: *mut u32, flag: *mut u8) {
    let o = unsafe { &*this.cast::<Occupant>() };
    log(Call::Endpoints(o.id));
    unsafe {
        a.write(o.endpoints[0] as u32);
        b.write(o.endpoints[1] as u32);
        flag.write(0xAA);
    }
}
unsafe extern "thiscall" fn remove_connection_fake(this: *mut u8, other: *mut u8, flag: u32) {
    log(Call::RemoveConnection(
        unsafe { (*this.cast::<Occupant>()).id },
        unsafe { (*other.cast::<Occupant>()).id },
        flag,
    ));
}
impl Fixture {
    fn terrain(&mut self, low: &mut [i32; 2], high: &mut [i32; 2]) -> [u32; 3] {
        let mut record = [
            fc4p_006edda0 as *const () as u32,
            self.sim.ptr() as u32,
            low.as_mut_ptr() as u32,
            high.as_mut_ptr() as u32,
            0,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!(
                "push [edi + 12]", "push [edi + 8]", "mov ecx, [edi + 4]", "call dword ptr [edi]",
                "mov [edi + 16], eax", "mov eax, [esp - 8]", "mov [edi + 20], eax",
                "mov eax, [esp - 4]", "mov [edi + 24], eax",
                in("edi") record.as_mut_ptr(), clobber_abi("C"),
            );
        }
        [record[4], record[5], record[6]]
    }
}

#[test]
fn terrain_removes_only_poles_inside_inclusive_bounds_and_deduplicates_ordered_pairs() {
    let mut f = Fixture::new();
    let table = occupant_table();
    let mut objects: Vec<_> = [0x2890_D4DE, 0x2890_D4DE, 0x0990_B2EA, 0x0990_B2EA]
        .into_iter()
        .enumerate()
        .map(|(id, kind)| Occupant {
            table: table.as_ptr(),
            id: id as u32,
            kind,
            x: if id == 0 { 10 } else { 30 },
            z: 20,
            rect: Rect::default(),
            query_ok: true,
            flags: 0,
            endpoints: [core::ptr::null_mut(); 2],
        })
        .collect();
    let pair = [
        (&mut objects[0] as *mut Occupant).cast(),
        (&mut objects[1] as *mut Occupant).cast(),
    ];
    objects[2].endpoints = pair;
    objects[3].endpoints = pair;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.query_ok = true;
        s.occupants = objects.iter_mut().map(|o| o as *mut Occupant as usize).collect();
    });
    let result = f.terrain(&mut [10, 20], &mut [10, 20]);
    assert_eq!(result[0], 0x89AB_CDEF);
    assert_eq!(result[1], pair[0] as u32);
    assert_eq!(result[2] as u8, 0xAA);
    assert_eq!(
        calls(),
        [
            Call::ManagerRef(4),
            Call::Allocate(12),
            Call::QueryOccupants([10, 20], [10, 20], 0, u32::MAX),
            Call::Query(0, 0xA8C8_6142),
            Call::ObjectCell(0),
            Call::Break(0, 1),
            Call::Release(0),
            Call::Remove(0, 1, 0),
            Call::Query(1, 0xA8C8_6142),
            Call::ObjectCell(1),
            Call::Release(1),
            Call::Query(2, 0xA8C8_6142),
            Call::Query(3, 0xA8C8_6142),
            Call::HeapAllocate(12),
            Call::Query(0, 0xC98F_3B24),
            Call::Release(0),
            Call::Query(1, 0xC98F_3B24),
            Call::Release(1),
            Call::Query(2, 0xC98F_3B24),
            Call::Endpoints(2),
            Call::HeapAllocate(12),
            Call::Release(0),
            Call::Release(1),
            Call::Release(2),
            Call::Query(3, 0xC98F_3B24),
            Call::Endpoints(3),
            Call::Endpoints(2),
            Call::Release(0),
            Call::Release(1),
            Call::Release(0),
            Call::Release(1),
            Call::Release(3),
            Call::Release(3),
            Call::Endpoints(2),
            Call::Release(2),
            Call::RemoveConnection(0, 1, 0),
            Call::Release(0),
            Call::Release(1),
            Call::HeapFree,
            Call::HeapFree,
            Call::Clear,
            Call::Free,
            Call::ManagerRef(8)
        ]
    );
}

#[test]
fn terrain_failed_query_still_processes_returned_lines_and_keeps_unpaired_interface_reference() {
    let mut f = Fixture::new();
    let table = occupant_table();
    let mut pole = Occupant {
        table: table.as_ptr(),
        id: 0,
        kind: 0x2890_D4DE,
        x: 5,
        z: 5,
        rect: Rect::default(),
        query_ok: true,
        flags: 0,
        endpoints: [core::ptr::null_mut(); 2],
    };
    let mut line = Occupant {
        table: table.as_ptr(),
        id: 1,
        kind: 0x0990_B2EA,
        x: 5,
        z: 5,
        rect: Rect::default(),
        query_ok: true,
        flags: 0,
        endpoints: [(&mut pole as *mut Occupant).cast(), core::ptr::null_mut()],
    };
    STATE.with(|s| s.borrow_mut().occupants = vec![&mut line as *mut Occupant as usize]);
    f.terrain(&mut [0, 0], &mut [9, 9]);
    assert_eq!(
        calls(),
        [
            Call::ManagerRef(4),
            Call::Allocate(12),
            Call::QueryOccupants([0, 0], [9, 9], 0, u32::MAX),
            Call::HeapAllocate(12),
            Call::Query(1, 0xC98F_3B24),
            Call::Endpoints(1),
            Call::Release(0),
            Call::Release(1),
            Call::HeapFree,
            Call::Clear,
            Call::Free,
            Call::ManagerRef(8)
        ]
    );
}

#[test]
fn terrain_empty_reversed_bounds_keeps_the_caller_words() {
    let mut f = Fixture::new();
    let mut low = [9, 10];
    let mut high = [3, 4];
    let a = low.as_mut_ptr() as u32;
    let b = high.as_mut_ptr() as u32;
    assert_eq!(f.terrain(&mut low, &mut high), [0x89AB_CDEF, a, b]);
    assert_eq!(
        calls(),
        [
            Call::ManagerRef(4),
            Call::Allocate(12),
            Call::QueryOccupants([9, 10], [3, 4], 0, u32::MAX),
            Call::HeapAllocate(12),
            Call::HeapFree,
            Call::Clear,
            Call::Free,
            Call::ManagerRef(8)
        ]
    );
}

#[test]
fn connection_null_entry_reuses_the_existing_coordinate_words() {
    let mut f = Fixture::new();
    f.select(8, 9);
    STATE.with(|s| s.borrow_mut().connections = vec![0]);
    let mut frame = [0u32; 12];
    frame[0x14 / 4] = 8;
    frame[0x20 / 4] = 9;
    let result = unsafe { find_connection(f.sim.ptr(), frame.as_mut_ptr().cast()) };
    assert_eq!(result as u8, 1);
    assert_eq!(
        calls(),
        [
            Call::Allocate(12),
            Call::Connections(4, 9),
            Call::Width,
            Call::Clear,
            Call::Free
        ]
    );
}

#[test]
fn wire_rebuild_returns_the_final_allocators_ecx() {
    let mut f = Fixture::new();
    let mut rect = Rect {
        x0: 2,
        z0: 2,
        x1: 1,
        z1: 1,
    };
    let mut record = [
        fc4p_006ed6e0 as *const () as u32,
        f.sim.ptr() as u32,
        &mut rect as *mut Rect as u32,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push 0", "push dword ptr [edi + 8]", "mov ecx, [edi + 4]",
            "call dword ptr [edi]", "mov [edi + 12], ecx",
            in("edi") record.as_mut_ptr(), clobber_abi("C"),
        );
    }
    assert_eq!(record[3], 0x1357_9BDF);
}

#[test]
fn wire_rebuild_failed_queries_and_nonconducting_networks_do_not_set_bits() {
    let mut f = Fixture::new();
    let table = occupant_table();
    let mut objects: Vec<_> = [0x2890_D4DE, 0x0990_B2EA, 0xC772_BF98, 0xC772_BF98]
        .into_iter()
        .enumerate()
        .map(|(id, kind)| Occupant {
            table: table.as_ptr(),
            id: id as u32,
            kind,
            x: 5,
            z: 5,
            rect: Rect::default(),
            query_ok: id == 3,
            flags: 0x8000_0000,
            endpoints: [core::ptr::null_mut(); 2],
        })
        .collect();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.query_ok = true;
        s.occupants = objects.iter_mut().map(|o| o as *mut Occupant as usize).collect();
    });
    let mut r = Rect {
        x0: 2,
        z0: 2,
        x1: 1,
        z1: 1,
    };
    f.wire(&mut r, core::ptr::null_mut(), 0);
    let all = calls();
    assert!(!all
        .iter()
        .any(|c| matches!(c, Call::Expand(..) | Call::SetWire(..) | Call::ObjectCell(..))));
    assert_eq!(all.iter().filter(|c| **c == Call::Release(3)).count(), 2);
    for id in 0..3 {
        assert_eq!(all.iter().filter(|c| **c == Call::Release(id)).count(), 1);
    }
}

#[test]
fn terrain_reversed_pairs_are_distinct_connections() {
    let mut f = Fixture::new();
    let table = occupant_table();
    let mut objects: Vec<_> = [0x2890_D4DE, 0x2890_D4DE, 0x0990_B2EA, 0x0990_B2EA]
        .into_iter()
        .enumerate()
        .map(|(id, kind)| Occupant {
            table: table.as_ptr(),
            id: id as u32,
            kind,
            x: 30,
            z: 30,
            rect: Rect::default(),
            query_ok: true,
            flags: 0,
            endpoints: [core::ptr::null_mut(); 2],
        })
        .collect();
    let a = (&mut objects[0] as *mut Occupant).cast();
    let b = (&mut objects[1] as *mut Occupant).cast();
    objects[2].endpoints = [a, b];
    objects[3].endpoints = [b, a];
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.query_ok = true;
        s.occupants = objects.iter_mut().map(|o| o as *mut Occupant as usize).collect();
    });
    f.terrain(&mut [10, 10], &mut [20, 20]);
    let all = calls();
    let removed: Vec<_> = all.iter().filter(|c| matches!(c, Call::RemoveConnection(..))).collect();
    assert_eq!(
        removed,
        [&Call::RemoveConnection(0, 1, 0), &Call::RemoveConnection(1, 0, 0)]
    );
    assert!(!all.iter().any(|c| matches!(c, Call::Break(..) | Call::Remove(..))));
}

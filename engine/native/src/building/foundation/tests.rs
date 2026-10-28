// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Event(&'static str, usize, Vec<u32>);

struct State {
    events: Vec<Event>,
    occupant: usize,
    properties: usize,
    holder: usize,
    updated_holder: usize,
    target: usize,
    resource: usize,
    property: usize,
    variant: usize,
    size: usize,
    load_result: u32,
    insert_result: u32,
    property_present: bool,
    top: f64,
    bottom: f64,
    allowed: bool,
    suitable: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            events: vec![],
            occupant: 0,
            properties: 0,
            holder: 0,
            updated_holder: 0,
            target: 0,
            resource: 0,
            property: 0,
            variant: 0,
            size: 0,
            load_result: 0x7788_9901,
            insert_result: 0x4455_6680,
            property_present: true,
            top: 310.0,
            bottom: 304.0,
            allowed: true,
            suitable: true,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn event(name: &'static str, this: *mut u8, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().events.push(Event(name, this as usize, args.to_vec())));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
const RELEASE_RESULT: u64 = 0x1234_5678_AABB_CCDD;

extern "C" fn allocate(size: u32) -> *mut u8 {
    event("allocate", null_mut(), &[size]);
    STATE.with(|s| s.borrow().occupant as *mut u8)
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
    event("construct", this, &[]);
    this
}
extern "thiscall" fn addref(this: *mut u8) -> u64 {
    event("addref", this, &[]);
    1
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    event("release", this, &[]);
    RELEASE_RESULT
}
extern "thiscall" fn set_type(this: *mut u8, value: u32) -> u64 {
    event("type", this, &[value]);
    0
}
extern "thiscall" fn properties(this: *mut u8) -> *mut u8 {
    event("properties", this, &[]);
    STATE.with(|s| s.borrow().properties as *mut u8)
}
extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    event("holder", this, &[]);
    STATE.with(|s| {
        let s = s.borrow();
        if s.events.iter().any(|e| e.0 == "default") {
            s.updated_holder as *mut u8
        } else {
            s.holder as *mut u8
        }
    })
}
extern "thiscall" fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 {
    event("query", this, &[iid]);
    // SAFETY: the loader supplies its writable first argument word.
    unsafe { out.write(STATE.with(|s| s.borrow().target as *mut u8)) };
    0x1122_3344_5566_7701
}
extern "thiscall" fn current_type(this: *mut u8) -> u32 {
    event("current type", this, &[]);
    0x50
}
extern "thiscall" fn load_properties(this: *mut u8, occupant: *mut u8, kind: u32) -> u64 {
    event("load", this, &[occupant as u32, kind]);
    STATE.with(|s| u64::from(s.borrow().load_result))
}
extern "thiscall" fn cells(this: *mut u8, rect: *mut f32, out: *mut i32) {
    // SAFETY: the four-element rectangle and cell output belong to the port.
    unsafe {
        event("cells", this, core::slice::from_raw_parts(rect.cast(), 4));
        core::ptr::copy_nonoverlapping([5, 10, 6, 12].as_ptr(), out, 4);
    }
}
extern "thiscall" fn altitude(this: *mut u8, rect: *mut i32, low: u32, water: u32) -> f64 {
    // SAFETY: cells initialized all four elements.
    let mut words = unsafe { core::slice::from_raw_parts(rect.cast::<u32>(), 4) }.to_vec();
    words.extend([low, water]);
    event("altitude", this, &words);
    STATE.with(|s| if low == 0 { s.borrow().top } else { s.borrow().bottom })
}
extern "thiscall" fn position(this: *mut u8, position: *mut f32) -> u64 {
    event("position", this, unsafe {
        core::slice::from_raw_parts(position.cast(), 3)
    });
    0x0102_0304_0506_0708
}
extern "thiscall" fn size(this: *mut u8, size: *mut f32) -> u64 {
    event("size", this, unsafe { core::slice::from_raw_parts(size.cast(), 3) });
    0
}
extern "thiscall" fn usage(this: *mut u8, value: u32) -> u64 {
    event("usage", this, &[value]);
    0
}
extern "thiscall" fn orientation(this: *mut u8, value: u32) -> u64 {
    event("orientation", this, &[value]);
    0
}
extern "thiscall" fn init(this: *mut u8) -> u64 {
    event("init", this, &[]);
    0
}
extern "thiscall" fn insert(this: *mut u8, occupant: *mut u8) -> u64 {
    event("insert", this, &[occupant as u32]);
    STATE.with(|s| u64::from(s.borrow().insert_result))
}
extern "thiscall" fn resource(this: *mut u8, key: *const u32, iid: u32, out: *mut *mut u8, a: u32, b: u32) -> u64 {
    let mut args = unsafe { core::slice::from_raw_parts(key, 3) }.to_vec();
    args.extend([iid, a, b]);
    event("resource", this, &args);
    unsafe { out.write(STATE.with(|s| s.borrow().resource as *mut u8)) };
    STATE.with(|s| u64::from(s.borrow().load_result))
}
extern "thiscall" fn default_exemplar(this: *mut u8, resource: *mut u8) -> u64 {
    event("default", this, &[resource as u32]);
    0x8877_6655_4433_2211
}
extern "C" fn float_property(holder: *mut u8, id: u32, out: *mut f32) -> u64 {
    event("float property", holder, &[id, unsafe { out.read().to_bits() }]);
    let present = STATE.with(|s| s.borrow().property_present);
    if present {
        unsafe { out.write(12.5) };
    }
    0x1234_5678_AABB_CC00 | u64::from(present)
}
extern "thiscall" fn maximum(this: *mut u8, maximum: f32) -> u64 {
    event("maximum", this, &[maximum.to_bits()]);
    0
}
extern "thiscall" fn property(this: *mut u8, id: u32) -> *mut u8 {
    event("property", this, &[id]);
    STATE.with(|s| {
        if s.borrow().property_present {
            s.borrow().property as *mut u8
        } else {
            null_mut()
        }
    })
}
extern "thiscall" fn variant(this: *mut u8) -> *mut u8 {
    event("variant", this, &[]);
    STATE.with(|s| s.borrow().variant as *mut u8)
}
extern "thiscall" fn float_array(this: *mut u8) -> *mut u8 {
    event("array", this, &[]);
    STATE.with(|s| s.borrow().size as *mut u8)
}
extern "thiscall" fn bounding_box(this: *mut u8, origin: *mut f32, size: *mut f32) -> u64 {
    let mut args = unsafe { core::slice::from_raw_parts(origin.cast::<u32>(), 3) }.to_vec();
    args.extend_from_slice(unsafe { core::slice::from_raw_parts(size.cast(), 3) });
    event("box", this, &args);
    0
}

struct World {
    sim: Object,
    objects: Vec<Object>,
    _tables: Vec<Box<[usize]>>,
    _constants: Box<(f64, [f32; 3])>,
    _guards: Vec<Fake>,
}

impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            sim: Object::new(0xE4),
            objects: vec![],
            _tables: vec![],
            _constants: Box::new((0.5, [-0.5, 0.5, 0.0])),
            _guards: vec![],
        };
        let sim_table = w.table(&[(0x60, insert as *const ()), (0x68, load_properties as *const ())]);
        unsafe { BuildingSim::from_ptr(w.sim.ptr()) }.set_vtable(sim_table);
        let foundation_table = w.table(&[
            (4, addref as *const ()),
            (8, release as *const ()),
            (0x0C, properties as *const ()),
            (0x10, current_type as *const ()),
            (0x14, set_type as *const ()),
            (0x1C, maximum as *const ()),
            (0x20, size as *const ()),
            (0x28, usage as *const ()),
            (0x2C, current_type as *const ()),
            (0x30, orientation as *const ()),
        ]);
        let primary_table = w.table(&[(0x0C, init as *const ()), (0x24, position as *const ())]);
        let occupant = w.object(foundation_table);
        unsafe { occupant.add(4).cast::<*const usize>().write(primary_table) };
        let property_table = w.table(&[(0, query as *const ()), (0x18, holder as *const ())]);
        let properties = w.object(property_table);
        let holder_table = w.table(&[(0x24, property as *const ())]);
        let holder = w.object(holder_table);
        let updated = w.object(holder_table);
        let target_table = w.table(&[(8, release as *const ()), (0x0C, default_exemplar as *const ())]);
        let target = w.object(target_table);
        let exemplar = w.object(target_table);
        let resource_table = w.table(&[(0x0C, resource as *const ())]);
        let manager = w.object(resource_table);
        let lot_table = w.table(&[(0x44, altitude as *const ())]);
        let lot = w.object(lot_table);
        let property_table = w.table(&[(0x14, variant as *const ())]);
        let property = w.object(property_table);
        let variant_table = w.table(&[(0x1BC, float_array as *const ())]);
        let variant = w.object(variant_table);
        let size = w.object(core::ptr::null());
        unsafe { core::ptr::copy_nonoverlapping([10.0f32, 20.0, 30.0].as_ptr(), size.cast(), 3) };
        let sim = unsafe { BuildingSim::from_ptr(w.sim.ptr()) };
        sim.set_resources(manager);
        sim.set_lot_developer(lot);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.occupant = occupant as usize;
            s.properties = properties as usize;
            s.holder = holder as usize;
            s.updated_holder = updated as usize;
            s.target = target as usize;
            s.resource = exemplar as usize;
            s.property = property as usize;
            s.variant = variant as usize;
            s.size = size as usize;
        });
        w._guards.extend([
            fake(0x005D_6220, allocate as *const () as usize),
            fake(0x005D_6440, construct as *const () as usize),
            fake(0x0069_5610, cells as *const () as usize),
            fake(0x005F_D4F0, float_property as *const () as usize),
            fake_data(0x00A9_2D28, (&mut w._constants.0 as *mut f64).cast()),
            fake_data(0x00A9_49FC, (&mut w._constants.1[0] as *mut f32).cast()),
            fake_data(0x00AB_0A04, (&mut w._constants.1[1] as *mut f32).cast()),
            fake_data(0x00A8_1054, (&mut w._constants.1[2] as *mut f32).cast()),
        ]);
        w
    }

    fn table(&mut self, entries: &[(usize, *const ())]) -> *const usize {
        let mut table = vec![0usize; 120].into_boxed_slice();
        for &(slot, function) in entries {
            table[slot / 4] = function as usize;
        }
        let pointer = table.as_ptr();
        self._tables.push(table);
        pointer
    }
    fn object(&mut self, table: *const usize) -> *mut u8 {
        let mut object = Object::new(0x60);
        let pointer = object.ptr();
        unsafe { pointer.cast::<*const usize>().write(table) };
        self.objects.push(object);
        pointer
    }
    fn occupant(&self) -> u32 {
        STATE.with(|s| s.borrow().occupant as u32)
    }

    fn add(&mut self, position: &mut [f32; 3], base: f32) -> (u64, Vec<Event>, Vec<u32>) {
        let mut args = [
            0x50,
            2,
            position.as_mut_ptr() as u32,
            32.0f32.to_bits(),
            48.0f32.to_bits(),
            1,
            base.to_bits(),
        ];
        let result = unsafe { add_foundation(self.sim.ptr(), args.as_mut_ptr()) };
        (result, events(), args.to_vec())
    }
}

#[test]
fn foundation_creation_preserves_order_heights_arguments_and_release_registers() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let p = w.sim.ptr();
    let o = w.occupant() as usize;
    let lot = unsafe { BuildingSim::from_ptr(p) }.lot_developer() as usize;
    let mut position = [96.0, -1.0, 184.0];
    let before = unsafe { core::slice::from_raw_parts(p, 0xE4) }.to_vec();
    let (result, calls, args) = w.add(&mut position, -1.0);
    assert_eq!(result, (RELEASE_RESULT & !255) | 0x80);
    assert_eq!(position, [96.0, 310.0, 184.0]);
    assert_eq!((args[0], args[6]), (6.0f32.to_bits(), 0.5f32.to_bits()));
    let bits = |values: &[f32]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(
        calls,
        [
            Event("allocate", 0, vec![0x5C]),
            Event("construct", o, vec![]),
            Event("addref", o, vec![]),
            Event("type", o, vec![0x50]),
            Event("load", p as usize, vec![o as u32, 0x50]),
            Event("cells", p as usize, bits(&[80.5, 160.5, 111.5, 207.5])),
            Event("altitude", lot, vec![5, 10, 6, 12, 0, 1]),
            Event("altitude", lot, vec![5, 10, 6, 12, 1, 1]),
            Event("position", o + 4, bits(&[96.0, 310.0, 184.0])),
            Event("size", o, bits(&[32.0, 6.0, 48.0])),
            Event("usage", o, vec![2]),
            Event("orientation", o, vec![1]),
            Event("init", o + 4, vec![]),
            Event("insert", p as usize, vec![o as u32]),
            Event("release", o, vec![]),
        ]
    );
    assert_eq!(unsafe { core::slice::from_raw_parts(p, 0xE4) }, before);
}

#[test]
fn foundation_failure_releases_without_altering_position_or_argument_words() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().load_result = 0xFFFF_FF00);
    let mut position = [96.0, -1.0, 184.0];
    let (result, calls, args) = w.add(&mut position, -1.0);
    assert_eq!(result, RELEASE_RESULT & !255);
    assert_eq!(position, [96.0, -1.0, 184.0]);
    assert_eq!((args[0], args[6]), (0x50, (-1.0f32).to_bits()));
    assert_eq!(
        calls.iter().map(|e| e.0).collect::<Vec<_>>(),
        ["allocate", "construct", "addref", "type", "load", "release"]
    );
}

#[test]
fn foundation_zero_negative_nan_and_unrounded_altitudes() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    for (top, base, queries, expected) in [
        (310.0, 304.0, 0, 6.0),
        (304.0, 304.0, 0, 0.5),
        (303.0, 304.0, 0, 0.5),
        (-0.0, -0.0, 0, 0.5),
        (310.0, f32::NAN, 1, 6.0),
        (f32::NAN, 304.0, 0, f32::NAN),
    ] {
        let mut position = [96.0, top, 184.0];
        let (_, calls, args) = w.add(&mut position, base);
        assert_eq!(calls.iter().filter(|e| e.0 == "altitude").count(), queries);
        let height = f32::from_bits(calls.iter().find(|e| e.0 == "size").unwrap().2[1]);
        if expected.is_nan() {
            assert!(height.is_nan());
            assert!(f32::from_bits(args[0]).is_nan());
        } else {
            assert_eq!(height, expected);
        }
    }
    STATE.with(|s| s.borrow_mut().bottom = 304.000_01);
    let (_, calls, _) = w.add(&mut [96.0, 305.0, 184.0], -1.0);
    assert_eq!(
        calls.iter().find(|e| e.0 == "size").unwrap().2[1],
        0.999_99f32.to_bits()
    );
    // Rounding the ST0 bottom to f32 first would give exactly 1 metre.
    STATE.with(|s| s.borrow_mut().insert_result = 0xFFFF_FF00);
    assert_eq!(w.add(&mut [96.0, 310.0, 184.0], 304.0).0, RELEASE_RESULT & !255);
}

fn bind_buckets(kind: Loader, begin: &mut *mut *mut u8, end: &mut *mut *mut u8) -> [Fake; 2] {
    let (a, b) = match kind {
        Loader::Foundation => (0x00B4_85BC, 0x00B4_85C0),
        Loader::Wall => (0x00B4_85D0, 0x00B4_85D4),
        Loader::Building => (0x00B4_854C, 0x00B4_8550),
    };
    [
        fake_data(a, (begin as *mut *mut *mut u8).cast()),
        fake_data(b, (end as *mut *mut *mut u8).cast()),
    ]
}

#[test]
fn exemplar_loaders_find_collisions_fall_back_types_and_release_in_order() {
    let mut w = World::new();
    let mut node = [0u32, 0x50, 0x6534_284A, 3, 0x500];
    let mut collision = [node.as_mut_ptr() as u32, 0x80, 0, 0, 0];
    let mut buckets = [collision.as_mut_ptr().cast::<u8>(), null_mut(), null_mut()];
    // Both 0x50 and 0x80 have bucket index 2.
    buckets.swap(0, 2);
    let mut begin = buckets.as_mut_ptr();
    let mut end = begin.wrapping_add(3);
    for loader in [Loader::Foundation, Loader::Wall, Loader::Building] {
        w._tables[1][0x20 / 4] = if matches!(loader, Loader::Building) {
            bounding_box as *const () as usize
        } else {
            size as *const () as usize
        };
        let _data = bind_buckets(loader, &mut begin, &mut end);
        let mut args = [w.occupant(), 0];
        let result = unsafe { load(w.sim.ptr(), args.as_mut_ptr(), loader) };
        assert_eq!(result, (RELEASE_RESULT & !255) | 1);
        assert_eq!(args[0], STATE.with(|s| s.borrow().target as u32));
        assert_eq!(
            args[1],
            if matches!(loader, Loader::Foundation) {
                12.5f32.to_bits()
            } else {
                STATE.with(|s| s.borrow().resource as u32)
            }
        );
        let calls = events();
        let mut expected = vec!["properties", "holder", "query", "current type", "resource", "default"];
        match loader {
            Loader::Foundation => expected.extend(["float property", "maximum"]),
            Loader::Building => expected.extend(["holder", "property", "variant", "array", "box"]),
            Loader::Wall => {}
        }
        expected.extend(["release", "release"]);
        assert_eq!(calls.iter().map(|e| e.0).collect::<Vec<_>>(), expected);
        let resource_call = calls.iter().find(|e| e.0 == "resource").unwrap();
        assert_eq!(resource_call.2, [0x6534_284A, 3, 0x500, 0xA521_60F5, 0, 0]);
        let releases: Vec<_> = calls.iter().filter(|e| e.0 == "release").map(|e| e.1).collect();
        assert_eq!(releases, STATE.with(|s| vec![s.borrow().resource, s.borrow().target]));
        if matches!(loader, Loader::Building) {
            assert_eq!(
                calls.iter().find(|e| e.0 == "box").unwrap().2,
                [0, 0, 0, 10.0f32.to_bits(), 20.0f32.to_bits(), 30.0f32.to_bits()]
            );
            assert_eq!(
                calls.iter().find(|e| e.0 == "property").unwrap().1,
                STATE.with(|s| s.borrow().updated_holder)
            );
        }
    }
}

#[test]
fn exemplar_missing_keys_resources_properties_and_query_interface() {
    let mut w = World::new();
    let mut node = [0u32, 0x50, 0x6534_284A, 3, 0x500];
    let mut buckets = [node.as_mut_ptr().cast::<u8>()];
    let mut begin = buckets.as_mut_ptr();
    let mut end = begin.wrapping_add(1);
    for loader in [Loader::Foundation, Loader::Wall, Loader::Building] {
        let _data = bind_buckets(loader, &mut begin, &mut end);
        let mut args = [w.occupant(), 0x51];
        let result = unsafe { load(w.sim.ptr(), args.as_mut_ptr(), loader) };
        assert_eq!(
            result,
            (RELEASE_RESULT & !255) | u64::from(matches!(loader, Loader::Building))
        );
        assert_eq!(
            events().iter().map(|e| e.0).collect::<Vec<_>>(),
            ["properties", "holder", "query", "release"]
        );
        STATE.with(|s| s.borrow_mut().load_result = 0xFFFF_FF00);
        let mut args = [w.occupant(), 0x50];
        let result = unsafe { load(w.sim.ptr(), args.as_mut_ptr(), loader) };
        assert_eq!(
            result,
            (RELEASE_RESULT & !255) | u64::from(matches!(loader, Loader::Building))
        );
        assert_eq!(
            events().iter().map(|e| e.0).collect::<Vec<_>>(),
            ["properties", "holder", "query", "resource", "release"]
        );
        STATE.with(|s| {
            s.borrow_mut().load_result = 1;
            s.borrow_mut().property_present = false;
        });
        let mut args = [w.occupant(), 0x50];
        assert_eq!(
            unsafe { load(w.sim.ptr(), args.as_mut_ptr(), loader) },
            (RELEASE_RESULT & !255) | 1
        );
        let calls = events();
        assert!(!calls.iter().any(|e| matches!(e.0, "maximum" | "variant" | "box")));
        STATE.with(|s| s.borrow_mut().property_present = true);
    }
    STATE.with(|s| s.borrow_mut().target = 0);
    for loader in [Loader::Foundation, Loader::Wall, Loader::Building] {
        let _data = bind_buckets(loader, &mut begin, &mut end);
        let mut args = [w.occupant(), 0x1234_5678];
        let result = unsafe { load(w.sim.ptr(), args.as_mut_ptr(), loader) };
        assert_eq!(result, 0x1234_5600 | u64::from(matches!(loader, Loader::Building)));
        assert_eq!(args, [0, 0x1234_5678]);
        assert_eq!(
            events().iter().map(|e| e.0).collect::<Vec<_>>(),
            ["properties", "holder", "query"]
        );
    }
}

#[test]
fn exports_use_the_original_stack_cleanup() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().load_result = 0);
    let mut position = [0.0f32; 3];
    let args = [
        0x50,
        2,
        position.as_mut_ptr() as u32,
        32.0f32.to_bits(),
        48.0f32.to_bits(),
        1,
        0,
    ];
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_00695af0 as *const () as usize,
        w.sim.ptr() as u32,
        &args,
    );
    unsafe { call_raw(&mut call) };
    assert_eq!((call.eax, call.edx), (0xAABB_CC00, 0x1234_5678));
    events();
    let mut buckets = [null_mut::<u8>()];
    let mut begin = buckets.as_mut_ptr();
    let mut end = begin.wrapping_add(1);
    for (loader, entry) in [
        (Loader::Foundation, fc4p_006982b0 as *const ()),
        (Loader::Wall, fc4p_006983c0 as *const ()),
        (Loader::Building, fc4p_00698490 as *const ()),
    ] {
        let _data = bind_buckets(loader, &mut begin, &mut end);
        let args = [w.occupant(), 0x50];
        let mut call = RawCall::new(Conv::Thiscall, entry as usize, w.sim.ptr() as u32, &args);
        unsafe { call_raw(&mut call) };
        assert_eq!(call.edx, 0x1234_5678);
        assert_eq!(call.eax as u8, u8::from(matches!(loader, Loader::Building)));
        events();
    }
}

extern "thiscall" fn allowed(this: *mut u8, kind: u32) -> u64 {
    event("allowed", this, &[kind]);
    0x1122_3344_AABB_CC00 | u64::from(STATE.with(|s| s.borrow().allowed))
}
extern "thiscall" fn suitable(this: *mut u8, kind: u32, x: u32, z: u32) -> u64 {
    event("suitable", this, &[kind, x, z]);
    0x5566_7788_AABB_CC00 | u64::from(STATE.with(|s| s.borrow().suitable))
}
extern "thiscall" fn get_box(this: *mut u8, minimum: *mut f32, maximum: *mut f32) -> u64 {
    event("get box", this, &[]);
    unsafe {
        core::ptr::copy_nonoverlapping([-1.0f32, 0.0, -2.0].as_ptr(), minimum, 3);
        core::ptr::copy_nonoverlapping([10.0f32, 20.0, 30.0].as_ptr(), maximum, 3);
    }
    0
}

fn building_world() -> World {
    let mut w = World::new();
    w._tables[0][0x3C / 4] = allowed as *const () as usize;
    w._tables[0][0x14 / 4] = suitable as *const () as usize;
    w._tables[0][0x20 / 4] = insert as *const () as usize;
    w._tables[1][0x20 / 4] = bounding_box as *const () as usize;
    w._tables[1][0x3C / 4] = orientation as *const () as usize;
    w._tables[2][0x28 / 4] = get_box as *const () as usize;
    // The lot-developer table follows the resource-manager table in this fixture.
    let lot = unsafe { BuildingSim::from_ptr(w.sim.ptr()) }.lot_developer();
    unsafe { *(table(lot) as *mut usize).add(0x40 / 4) = altitude as *const () as usize };
    w._guards.extend([
        fake(0x0045_8A60, allocate as *const () as usize),
        fake(0x0045_8E90, construct as *const () as usize),
        fake(0x0069_8490, load_properties as *const () as usize),
    ]);
    w
}

#[test]
fn building_factory_rotates_only_odd_cardinal_boxes_and_keeps_raw_orientation() {
    let _fpu = GameFpu::new();
    let mut w = building_world();
    let p = w.sim.ptr();
    let o = w.occupant();
    let before = unsafe { core::slice::from_raw_parts(p, 0xE4) }.to_vec();
    for orientation in [0, 1, 2, 3, 4, u32::MAX] {
        let mut position = [96.0, 100.0, 184.0];
        assert_eq!(
            unsafe { fc4p_00698de0(p, 0xDEAD_BEEF, 0x100, position.as_mut_ptr(), orientation) },
            0x1234_5678_0000_0000 | u64::from(o)
        );
        let calls = events();
        assert_eq!(
            calls.iter().map(|e| e.0).collect::<Vec<_>>(),
            [
                "allowed",
                "suitable",
                "allocate",
                "construct",
                "addref",
                "type",
                "load",
                "get box",
                "box",
                "position",
                "orientation",
                "init",
                "insert",
                "release"
            ]
        );
        assert_eq!(calls[1].2, [0x100, 96.0f32.to_bits(), 184.0f32.to_bits()]);
        assert_eq!(calls[6], Event("load", p as usize, vec![o, 0x100]));
        let values: [f32; 6] = if orientation == 1 || orientation == 3 {
            [-2.0, 0.0, -1.0, 30.0, 20.0, 10.0]
        } else {
            [-1.0, 0.0, -2.0, 10.0, 20.0, 30.0]
        };
        assert_eq!(calls[8].2, values.map(f32::to_bits));
        assert_eq!(calls[10].2, [orientation]);
        assert_eq!(position, [96.0, 100.0, 184.0]);
        assert_eq!(unsafe { core::slice::from_raw_parts(p, 0xE4) }, before);
    }
}

#[test]
fn building_factory_short_circuits_and_always_releases_created_occupants() {
    let _fpu = GameFpu::new();
    let mut w = building_world();
    for failure in 0..4 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.allowed = failure != 0;
            s.suitable = failure != 1;
            s.load_result = if failure == 2 { 0xFFFF_FF00 } else { 1 };
            s.insert_result = if failure == 3 { 0xFFFF_FF00 } else { 1 };
        });
        let mut position = [96.0, 100.0, 184.0];
        let result = unsafe { fc4p_00698de0(w.sim.ptr(), 0xDEAD_BEEF, 0x100, position.as_mut_ptr(), 1) };
        assert_eq!(result as u32, 0);
        assert_eq!(
            result >> 32,
            match failure {
                0 => 0x1122_3344,
                1 => 0x5566_7788,
                _ => 0x1234_5678,
            }
        );
        let calls = events();
        let expected: &[&str] = match failure {
            0 => &["allowed"],
            1 => &["allowed", "suitable"],
            2 => &[
                "allowed",
                "suitable",
                "allocate",
                "construct",
                "addref",
                "type",
                "load",
                "release",
            ],
            _ => &[
                "allowed",
                "suitable",
                "allocate",
                "construct",
                "addref",
                "type",
                "load",
                "get box",
                "box",
                "position",
                "orientation",
                "init",
                "insert",
                "release",
            ],
        };
        assert_eq!(calls.iter().map(|e| e.0).collect::<Vec<_>>(), expected);
    }
}

#[test]
fn building_factory_queries_only_strictly_negative_heights_and_rounds_st0_at_the_store() {
    let _fpu = GameFpu::new();
    let mut w = building_world();
    STATE.with(|s| s.borrow_mut().top = 310.000_01);
    for y in [-1.0, -0.0, f32::NAN, f32::INFINITY] {
        let mut position = [96.0, y, 184.0];
        unsafe { fc4p_00698de0(w.sim.ptr(), 0, 0x100, position.as_mut_ptr(), 1) };
        let calls = events();
        let altitude = calls.iter().find(|e| e.0 == "altitude");
        if y < 0.0 {
            assert_eq!(position[1].to_bits(), 310.000_01f32.to_bits());
            assert_eq!(
                altitude.unwrap().2,
                [
                    (-2.0f32).to_bits(),
                    (-1.0f32).to_bits(),
                    30.0f32.to_bits(),
                    10.0f32.to_bits(),
                    0,
                    1
                ]
            );
        } else {
            assert!(altitude.is_none());
            assert_eq!(position[1].to_bits(), y.to_bits());
        }
    }
}

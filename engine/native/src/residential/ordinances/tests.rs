// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu};
use std::cell::RefCell;
use std::collections::BTreeMap;

const SCHOOL: [u32; 6] = [
    0xA91B_3AF4,
    0xA91B_3AFA,
    0xA92D_9D7A,
    0x692E_F65A,
    0x892D_9D02,
    0xC91B_3B02,
];
const HEALTH: [u32; 6] = [
    0x491B_3AD5,
    0x891B_3AE6,
    0xE91B_3AEE,
    0xC92D_9C7A,
    0x092D_909B,
    0xE92D_9DB4,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    factors: [u32; 4],
    curves: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Count,
    ArrayAllocate(u32),
    NewCurve(usize),
    Allocate(u32),
    Insert([u32; 2]),
    Array,
    Ordinance(u32),
    Holder(u32),
    Float(u32, u32),
    Property(u32, u32),
    Points(usize),
    CurveProperty(usize, u32),
    Copy(Snapshot),
    Free(String),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    ids: Vec<u32>,
    objects: BTreeMap<u32, usize>,
    floats: BTreeMap<(u32, u32), u32>,
    properties: BTreeMap<(u32, u32), u32>,
    buffers: Vec<(String, Box<[u32]>)>,
    curves: Vec<usize>,
    reserved: Option<u32>,
    spare_capacity: bool,
    kind_heads: usize,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}

fn buffer(label: String, words: usize) -> *mut u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut storage = vec![0xCCCC_CCCC; words].into_boxed_slice();
        let pointer = storage.as_mut_ptr();
        s.buffers.push((label, storage));
        pointer
    })
}

fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    let label = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if size == 12 {
            let index = s.kind_heads;
            s.kind_heads += 1;
            format!("head{index}")
        } else {
            format!("record{}", s.buffers.len())
        }
    });
    buffer(label, (size / 4) as usize).cast()
}

extern "C" fn array_allocate_fake(size: u32) -> *mut u32 {
    log(Call::ArrayAllocate(size));
    buffer("ids".to_owned(), (size / 4).max(1) as usize)
}

extern "C" fn free_fake(pointer: *mut u8) -> u32 {
    let label = STATE.with(|s| {
        s.borrow()
            .buffers
            .iter()
            .find(|(_, data)| data.as_ptr().cast::<u8>() == pointer)
            .expect("free must use an owned game allocation")
            .0
            .clone()
    });
    log(Call::Free(label));
    0xABCD_9876
}

extern "thiscall" fn curve_new_fake(this: *mut u8) -> u32 {
    let index = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let index = s.curves.len();
        s.curves.push(this as usize);
        index
    });
    log(Call::NewCurve(index));
    unsafe { this.cast::<[u32; 4]>().write([0; 4]) };
    this as u32
}

fn curve_index(this: *mut u8) -> usize {
    STATE.with(|s| s.borrow().curves.iter().position(|&p| p == this as usize).unwrap())
}

extern "thiscall" fn curve_points_fake(this: *mut u8, points: *mut u8) -> u32 {
    let index = curve_index(this);
    log(Call::Points(index));
    unsafe {
        let points = Points::from_ptr(points);
        assert_eq!(points.end() as usize - points.begin() as usize, 16);
        assert_eq!(
            std::slice::from_raw_parts(points.begin(), 4),
            [0, 100f32.to_bits(), 100f32.to_bits(), 100f32.to_bits()]
        );
        let curve = Curve::from_ptr(this);
        if curve.storage().is_null() {
            curve.set_storage(buffer(format!("curve{index}"), 8).cast());
        }
        this.add(12).cast::<u32>().write(0);
    }
    0
}

extern "thiscall" fn curve_property_fake(this: *mut u8, property: *mut u8) -> u32 {
    log(Call::CurveProperty(curve_index(this), property as u32));
    unsafe { this.add(12).cast::<u32>().write(property as u32) };
    // The property exists even when its conversion reports failure.
    0
}

extern "thiscall" fn insert_fake(
    this: *mut u8,
    position: *mut u32,
    point: *const u32,
    _tag: *const u8,
    count: u32,
    stride: u32,
) -> u32 {
    assert_eq!((count, stride), (1, 1));
    let point = unsafe { point.cast::<[u32; 2]>().read() };
    log(Call::Insert(point));
    unsafe {
        let points = Points::from_ptr(this);
        assert_eq!(position, points.end());
        if points.begin().is_null() {
            let capacity = STATE.with(|s| if s.borrow().spare_capacity { 4 } else { 2 });
            let begin = buffer("points".to_owned(), 4);
            begin.cast::<[u32; 2]>().write(point);
            points.set_begin(begin);
            points.set_end(begin.add(2));
            points.set_capacity(begin.add(capacity));
        } else {
            points.end().cast::<[u32; 2]>().write(point);
            points.set_end(points.end().add(2));
            points.set_capacity(points.end());
        }
    }
    0
}

#[repr(C)]
struct Interface {
    vtable: *const usize,
    id: u32,
    holder: *mut u8,
}

extern "thiscall" fn count_fake(_this: *mut u8) -> u32 {
    log(Call::Count);
    STATE.with(|s| {
        let s = s.borrow();
        s.reserved.unwrap_or(s.ids.len() as u32)
    })
}

extern "thiscall" fn array_fake(_this: *mut u8, ids: *mut u32, count: *mut u32) -> u32 {
    log(Call::Array);
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            assert!(*count >= s.ids.len() as u32);
            for (i, &id) in s.ids.iter().enumerate() {
                ids.add(i).write(id);
            }
            *count = s.ids.len() as u32;
        }
    });
    0
}

extern "thiscall" fn ordinance_fake(_this: *mut u8, id: u32) -> *mut u8 {
    log(Call::Ordinance(id));
    STATE.with(|s| s.borrow().objects.get(&id).copied().unwrap_or(0) as *mut u8)
}

extern "thiscall" fn holder_fake(this: *mut Interface) -> *mut u8 {
    unsafe {
        log(Call::Holder((*this).id));
        (*this).holder
    }
}

extern "thiscall" fn property_fake(this: *mut Interface, id: u32) -> *mut u8 {
    let owner = unsafe { (*this).id };
    log(Call::Property(owner, id));
    STATE.with(|s| s.borrow().properties.get(&(owner, id)).copied().unwrap_or(0) as *mut u8)
}

extern "C" fn read_float_fake(this: *mut Interface, id: u32, out: *mut f32) -> u32 {
    let owner = unsafe { (*this).id };
    log(Call::Float(owner, id));
    STATE.with(|s| {
        if let Some(&bits) = s.borrow().floats.get(&(owner, id)) {
            unsafe { out.cast::<u32>().write(bits) };
            0x1234_5601
        } else {
            0x1234_5600
        }
    })
}

extern "thiscall" fn copy_fake(this: *mut u8, source: *mut u8) -> u32 {
    unsafe {
        let effect = Effect::from_ptr(source);
        let snapshot = Snapshot {
            factors: [
                effect.radius().to_bits(),
                effect.capacity().to_bits(),
                effect.strength().to_bits(),
                effect.decay().to_bits(),
            ],
            curves: [
                source.add(0x10).cast::<u32>().read(),
                source.add(0x28).cast::<u32>().read(),
            ],
        };
        let head = effect.kinds();
        assert_eq!(Link::from_ptr(head).next(), head);
        assert_eq!(Link::from_ptr(head).previous(), head);
        log(Call::Copy(snapshot.clone()));
        // Leave the unused payload bytes as canaries. The port itself only writes the node's links.
        for (i, word) in snapshot.factors.into_iter().enumerate() {
            this.add(i * 4).cast::<u32>().write(word);
        }
    }
    this as u32
}

struct World {
    _guards: Vec<Fake>,
    _manager_vt: Box<[usize; 24]>,
    _ordinance_vt: Box<[usize; 24]>,
    _holder_vt: Box<[usize; 24]>,
    _manager: Box<Interface>,
    _global: Box<*mut u8>,
    _percent: Box<f32>,
    _objects: Vec<Interface>,
    _holders: Vec<Interface>,
    health: Box<[u32; 3]>,
    school: Box<[u32; 3]>,
}

impl World {
    fn new(ids: &[u32]) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                ids: ids.to_vec(),
                ..State::default()
            }
        });
        let mut manager_vt = Box::new([0; 24]);
        manager_vt[0x38 / 4] = count_fake as *const () as usize;
        manager_vt[0x3C / 4] = array_fake as *const () as usize;
        manager_vt[0x50 / 4] = ordinance_fake as *const () as usize;
        let mut ordinance_vt = Box::new([0; 24]);
        ordinance_vt[0x40 / 4] = holder_fake as *const () as usize;
        let mut holder_vt = Box::new([0; 24]);
        holder_vt[0x24 / 4] = property_fake as *const () as usize;
        let mut manager = Box::new(Interface {
            vtable: manager_vt.as_ptr(),
            id: 0,
            holder: null_mut(),
        });
        let mut global = Box::new((&mut *manager as *mut Interface).cast::<u8>());
        let mut percent = Box::new(0.01f32);
        let mut holders: Vec<Interface> = ids
            .iter()
            .map(|&id| Interface {
                vtable: holder_vt.as_ptr(),
                id,
                holder: null_mut(),
            })
            .collect();
        let mut objects: Vec<Interface> = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| Interface {
                vtable: ordinance_vt.as_ptr(),
                id,
                holder: (&mut holders[i] as *mut Interface).cast(),
            })
            .collect();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            for object in &mut objects {
                s.objects.insert(object.id, (object as *mut Interface) as usize);
            }
        });
        let guards = vec![
            fake(0x0090_CF54, allocate_fake as *const () as usize),
            fake(0x0090_CF63, free_fake as *const () as usize),
            fake(0x005E_55E0, array_allocate_fake as *const () as usize),
            fake(0x007D_0410, curve_new_fake as *const () as usize),
            fake(0x005F_4480, curve_points_fake as *const () as usize),
            fake(0x005F_4620, curve_property_fake as *const () as usize),
            fake(0x005F_D4F0, read_float_fake as *const () as usize),
            fake(0x006F_B9F0, copy_fake as *const () as usize),
            fake(0x007C_61A0, insert_fake as *const () as usize),
            fake_data(0x00B4_3D90, (&mut *global as *mut *mut u8).cast()),
            fake_data(0x00A8_23A4, (&mut *percent as *mut f32).cast()),
        ];
        let mut world = Self {
            _guards: guards,
            _manager_vt: manager_vt,
            _ordinance_vt: ordinance_vt,
            _holder_vt: holder_vt,
            _manager: manager,
            _global: global,
            _percent: percent,
            _objects: objects,
            _holders: holders,
            health: Box::new([0; 3]),
            school: Box::new([0; 3]),
        };
        for head in [&mut world.health, &mut world.school] {
            let pointer = head.as_mut_ptr() as u32;
            **head = [pointer, pointer, 0x1234_5678];
        }
        world
    }

    fn run(&mut self) -> u32 {
        let mut health = self.health.as_mut_ptr().cast();
        let mut school = self.school.as_mut_ptr().cast();
        unsafe { fc4p_006fe0f0(null_mut(), &mut health, &mut school) }
    }

    fn nodes(&mut self, school: bool) -> Vec<[u32; 15]> {
        let head = if school {
            self.school.as_mut_ptr()
        } else {
            self.health.as_mut_ptr()
        }
        .cast::<u8>();
        let mut out = Vec::new();
        unsafe {
            let mut node = Link::from_ptr(head).next();
            let mut previous = head;
            while node != head {
                assert_eq!(Link::from_ptr(node).previous(), previous);
                out.push(node.cast::<[u32; 15]>().read());
                previous = node;
                node = Link::from_ptr(node).next();
            }
            assert_eq!(Link::from_ptr(head).previous(), previous);
        }
        assert_eq!(self.health[2], 0x1234_5678);
        assert_eq!(self.school[2], 0x1234_5678);
        out
    }
}

fn expected_properties(owner: u32, ids: [u32; 6], curve_start: usize) -> Vec<Call> {
    let mut out = vec![Call::Points(curve_start), Call::Points(curve_start + 1)];
    for (i, id) in ids.into_iter().enumerate() {
        out.push(Call::Holder(owner));
        out.push(if i == 1 || i == 5 {
            Call::Property(owner, id)
        } else {
            Call::Float(owner, id)
        });
    }
    out
}

#[test]
fn empty_lists_construct_and_destroy_temporaries_in_order() {
    let _fpu = GameFpu::new();
    let mut world = World::new(&[]);
    assert_eq!(world.run(), 0);
    assert_eq!(
        calls(),
        [
            Call::Count,
            Call::NewCurve(0),
            Call::NewCurve(1),
            Call::Allocate(12),
            Call::NewCurve(2),
            Call::NewCurve(3),
            Call::Allocate(12),
            Call::Insert([0, 100f32.to_bits()]),
            Call::Insert([100f32.to_bits(); 2]),
            Call::Array,
            Call::Free("points".to_owned()),
            Call::Free("head1".to_owned()),
            Call::Free("head0".to_owned())
        ]
    );
    assert!(world.nodes(true).is_empty());
    assert!(world.nodes(false).is_empty());
}

#[test]
fn missing_objects_holders_and_properties_do_not_append() {
    let _fpu = GameFpu::new();
    let mut world = World::new(&[1, 2, 3]);
    STATE.with(|s| {
        s.borrow_mut().objects.remove(&1);
    });
    world._objects[1].holder = null_mut();
    assert_eq!(world.run(), 0xABCD_9876);
    let all = calls();
    let start = all.iter().position(|c| *c == Call::Array).unwrap() + 1;
    let mut expected = vec![
        Call::Ordinance(1),
        Call::Ordinance(2),
        Call::Holder(2),
        Call::Ordinance(3),
        Call::Holder(3),
    ];
    expected.extend(expected_properties(3, SCHOOL, 2));
    expected.extend(expected_properties(3, HEALTH, 0));
    expected.extend([
        Call::Free("points".to_owned()),
        Call::Free("head1".to_owned()),
        Call::Free("curve3".to_owned()),
        Call::Free("curve2".to_owned()),
        Call::Free("head0".to_owned()),
        Call::Free("curve1".to_owned()),
        Call::Free("curve0".to_owned()),
    ]);
    assert_eq!(&all[start..], expected);
    assert!(world.nodes(true).is_empty());
    assert!(world.nodes(false).is_empty());
}

/// The core ordinance-effect list case: radius 50%, capacity 200%, strength 110%, decay 50%/200%.
#[test]
fn ordinance_effect_lists_keep_defaults_scale_factors_and_append_in_order() {
    let _fpu = GameFpu::new();
    let mut world = World::new(&[0x10, 0x14]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for (id, value) in [
            (SCHOOL[0], 50f32),
            (SCHOOL[2], 200.0),
            (SCHOOL[3], 50.0),
            (SCHOOL[4], 110.0),
            (HEALTH[3], 200.0),
        ] {
            s.floats.insert((0x10, id), value.to_bits());
        }
        s.properties.insert((0x14, SCHOOL[1]), 0x100);
        s.properties.insert((0x14, HEALTH[5]), 0x200);
    });
    assert_eq!(world.run(), 0xABCD_9876);
    let all = calls();
    let snapshots: Vec<_> = all
        .iter()
        .filter_map(|c| if let Call::Copy(s) = c { Some(s.clone()) } else { None })
        .collect();
    let pct = |f: f32| scaled(f, 0.01).to_bits();
    let one = 1f32.to_bits();
    assert_eq!(
        snapshots,
        [
            Snapshot {
                factors: [pct(50.0), pct(200.0), pct(110.0), pct(50.0)],
                curves: [0, 0]
            },
            Snapshot {
                factors: [one, one, one, pct(200.0)],
                curves: [0, 0]
            },
            Snapshot {
                factors: [one; 4],
                curves: [0x100, 0]
            },
            Snapshot {
                factors: [one; 4],
                curves: [0, 0x200]
            },
        ]
    );
    for school in [true, false] {
        let nodes = world.nodes(school);
        assert_eq!(nodes.len(), 2);
        for node in nodes {
            assert!(node[6..].iter().all(|&v| v == 0xCCCC_CCCC));
        }
    }
    // A second read appends; it does not clear the owning destination lists.
    world.run();
    assert_eq!(world.nodes(true).len(), 4);
    assert_eq!(world.nodes(false).len(), 4);
}

#[test]
fn every_effect_property_alone_is_enough_to_append() {
    let _fpu = GameFpu::new();
    for ids in [SCHOOL, HEALTH] {
        for (index, id) in ids.into_iter().enumerate() {
            let mut world = World::new(&[7]);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                if index == 1 || index == 5 {
                    s.properties.insert((7, id), 0xABC);
                } else {
                    s.floats.insert((7, id), 100f32.to_bits());
                }
            });
            world.run();
            assert_eq!(world.nodes(ids == SCHOOL).len(), 1);
            assert!(world.nodes(ids != SCHOOL).is_empty());
            assert_eq!(
                calls()
                    .iter()
                    .filter(|c| matches!(c, Call::Float(..) | Call::Property(..)))
                    .count(),
                12
            );
        }
    }
}

#[test]
fn id_array_threshold_and_spare_point_capacity_preserve_allocation_paths() {
    let _fpu = GameFpu::new();
    for reserved in [0, 1, 63, 64, 65] {
        let mut world = World::new(&[]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.reserved = Some(reserved);
            s.spare_capacity = true;
        });
        world.run();
        let all = calls();
        assert_eq!(all.contains(&Call::ArrayAllocate(reserved * 4)), reserved >= 64);
        assert_eq!(all.iter().filter(|c| matches!(c, Call::Insert(..))).count(), 1);
        assert!(!all.contains(&Call::Free("ids".to_owned())));
    }
}

#[test]
fn scalar_percent_uses_x87_and_keeps_nonfinite_results() {
    let _fpu = GameFpu::new();
    for bits in [
        0,
        0x8000_0000,
        1,
        0x7F80_0000,
        0xFF80_0000,
        0x7FC1_2345,
        0x7F81_2345,
        0x7F7F_FFFF,
        0xBF80_0000,
    ] {
        let mut world = World::new(&[1]);
        STATE.with(|s| {
            s.borrow_mut().floats.insert((1, SCHOOL[0]), bits);
        });
        world.run();
        let expected = scaled(f32::from_bits(bits), 0.01).to_bits();
        let snapshots: Vec<_> = calls()
            .into_iter()
            .filter_map(|c| if let Call::Copy(s) = c { Some(s) } else { None })
            .collect();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].factors[0], expected);
    }
}

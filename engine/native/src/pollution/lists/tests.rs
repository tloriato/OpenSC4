// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! List tests build circular sentinels and the original property and allocator interfaces.

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Current(u32, u32),
    Holder(u32),
    Find(u32),
    Variant(u32),
    Buffer(u32),
    Copy(u32),
    Reset(u32),
    Read(u32),
    Store(u32, u32, u32),
    AddRef(u32),
    Release(u32),
    Allocate(u32, u32, u32),
    Erase(u32, u32),
    Free(u32),
    Application,
    QueryService,
    ReleaseService(u32),
    Range(u32, u32, u32),
    Append(u32, u32, u32, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static ALLOCATIONS: RefCell<Vec<*mut u8>> = const { RefCell::new(Vec::new()) };
    static SERVICES: RefCell<(*mut u8, *mut u8, *mut u8)> = const { RefCell::new((core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut())) };
}
fn log(call: Call) {
    CALLS.with(|c| c.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

struct Ring {
    nodes: Vec<[u32; 3]>,
    header: Box<*mut u8>,
}
impl Ring {
    fn new(occupants: &[*mut u8]) -> Self {
        let n = occupants.len();
        let mut nodes = vec![[0u32; 3]; n + 1];
        for i in 0..=n {
            nodes[i][0] = unsafe { nodes.as_mut_ptr().add((i + 1) % (n + 1)) } as u32;
            nodes[i][1] = unsafe { nodes.as_mut_ptr().add((i + n) % (n + 1)) } as u32;
            if i > 0 {
                nodes[i][2] = occupants[i - 1] as u32;
            }
        }
        let header = Box::new(nodes.as_mut_ptr().cast());
        Self { nodes, header }
    }
    fn list(&mut self) -> *mut u8 {
        (&mut *self.header as *mut *mut u8).cast()
    }
    fn node(&mut self, i: usize) -> *mut u8 {
        (&mut self.nodes[i] as *mut [u32; 3]).cast()
    }
}

#[repr(C)]
struct Variant {
    vt: *const usize,
    value: u32,
    tag: u32,
}
#[repr(C)]
struct Property {
    vt: *const usize,
    variant: *mut Variant,
    tag: u32,
}
#[repr(C)]
struct Holder {
    vt: *const usize,
    properties: Vec<(u32, *mut Property)>,
    numbers: Vec<(u32, u32)>,
}
#[repr(C)]
struct Occupant {
    vt: *const usize,
    holder: *mut Holder,
    references: i32,
}

extern "thiscall" fn holder(this: *mut Occupant) -> *mut Holder {
    log(Call::Holder(this as u32));
    unsafe { (*this).holder }
}
extern "thiscall" fn find(this: *mut Holder, id: u32) -> *mut Property {
    log(Call::Find(id));
    unsafe {
        (*this)
            .properties
            .iter()
            .find(|&&(key, _)| key == id)
            .map_or(core::ptr::null_mut(), |&(_, p)| p)
    }
}
extern "thiscall" fn variant(this: *mut Property) -> *mut Variant {
    unsafe {
        log(Call::Variant((*this).tag));
        (*this).variant
    }
}
extern "thiscall" fn buffer(this: *mut Variant) -> *const u32 {
    unsafe {
        log(Call::Buffer((*this).tag));
        &(*this).value
    }
}
extern "thiscall" fn copy(this: *mut Variant, value: u32) {
    log(Call::Copy(value));
    unsafe {
        (*this).value = value;
    }
}
extern "thiscall" fn reset(this: *mut Variant, value: u32) {
    log(Call::Reset(value));
    unsafe {
        (*this).value = value;
    }
}
extern "C" fn read(this: *mut Holder, id: u32, out: *mut u32) -> u32 {
    log(Call::Read(id));
    unsafe {
        if let Some(&(_, value)) = (*this).numbers.iter().find(|&&(key, _)| key == id) {
            *out = value;
            0xABCD_1201
        } else {
            0xABCD_1200
        }
    }
}
extern "thiscall" fn store(_this: *mut Holder, id: u32, value: u32, flags: u32) {
    log(Call::Store(id, value, flags));
}
extern "thiscall" fn add_ref(this: *mut Occupant) -> u32 {
    log(Call::AddRef(this as u32));
    unsafe {
        (*this).references += 1;
    }
    0xABCD_1234
}
extern "thiscall" fn release(this: *mut Occupant) -> u32 {
    log(Call::Release(this as u32));
    unsafe {
        (*this).references -= 1;
    }
    0x1234_5678
}
extern "thiscall" fn allocate(list: *mut u8, count: u32, hint: u32) -> *mut u8 {
    log(Call::Allocate(list as u32, count, hint));
    ALLOCATIONS.with(|a| a.borrow_mut().remove(0))
}
extern "thiscall" fn erase(list: *mut u8, output: *mut *mut u8, node: *mut u8) -> u32 {
    log(Call::Erase(list as u32, node as u32));
    unsafe {
        let view = ListNode::from_ptr(node);
        ListNode::from_ptr(view.previous()).set_next(view.next());
        ListNode::from_ptr(view.next()).set_previous(view.previous());
        *output = view.next();
    }
    output as u32
}

struct Tables {
    occupant: Box<[usize; 8]>,
    holder: Box<[usize; 16]>,
    property: Box<[usize; 8]>,
    variant: Box<[usize; 80]>,
}
impl Tables {
    fn new() -> Self {
        let mut t = Self {
            occupant: Box::new([0; 8]),
            holder: Box::new([0; 16]),
            property: Box::new([0; 8]),
            variant: Box::new([0; 80]),
        };
        t.occupant[1] = add_ref as *const () as usize;
        t.occupant[2] = release as *const () as usize;
        t.occupant[6] = holder as *const () as usize;
        t.holder[0x24 / 4] = find as *const () as usize;
        t.holder[0x34 / 4] = store as *const () as usize;
        t.property[0x14 / 4] = variant as *const () as usize;
        t.variant[0x12C / 4] = buffer as *const () as usize;
        t.variant[0x68 / 4] = copy as *const () as usize;
        t.variant[0x74 / 4] = reset as *const () as usize;
        take();
        t
    }
}

#[test]
fn reset_capacity_copies_before_zeroing_and_skips_absent_properties() {
    let tables = Tables::new();
    let mut used = Variant {
        vt: tables.variant.as_ptr(),
        value: 0xFEDC_BA98,
        tag: 1,
    };
    let mut previous = Variant {
        vt: tables.variant.as_ptr(),
        value: 77,
        tag: 2,
    };
    let mut used_property = Property {
        vt: tables.property.as_ptr(),
        variant: &mut used,
        tag: 1,
    };
    let mut previous_property = Property {
        vt: tables.property.as_ptr(),
        variant: &mut previous,
        tag: 2,
    };
    let mut properties = Holder {
        vt: tables.holder.as_ptr(),
        properties: vec![],
        numbers: vec![],
    };
    let mut occupant = Occupant {
        vt: tables.occupant.as_ptr(),
        holder: &mut properties,
        references: 1,
    };
    let pointer = &mut occupant as *mut Occupant as u32;
    let mut ring = Ring::new(&[(&mut occupant as *mut Occupant).cast()]);
    for mode in 0..3 {
        used.value = 0xFEDC_BA98;
        previous.value = 77;
        properties.properties.clear();
        if mode > 0 {
            properties.properties.push((10, &mut used_property));
        }
        if mode > 1 {
            properties.properties.push((20, &mut previous_property));
        }
        unsafe {
            fc4p_006e3c80(core::ptr::null_mut(), ring.list().cast(), 10, 20);
        }
        let mut expected = vec![Call::Holder(pointer), Call::Find(10)];
        if mode > 0 {
            expected.extend([Call::Variant(1), Call::Find(20)]);
            if mode > 1 {
                expected.extend([Call::Variant(2), Call::Buffer(1), Call::Copy(0xFEDC_BA98)]);
            }
            expected.push(Call::Reset(0));
        }
        assert_eq!(take(), expected);
        assert_eq!(used.value, if mode == 0 { 0xFEDC_BA98 } else { 0 });
        assert_eq!(previous.value, if mode == 2 { 0xFEDC_BA98 } else { 77 });
    }
    let mut empty = Ring::new(&[]);
    unsafe {
        fc4p_006e3c80(core::ptr::null_mut(), empty.list().cast(), 10, 20);
    }
    assert_eq!(take(), []);
}

#[test]
fn waste_to_energy_queries_short_circuit_and_products_wrap_before_unsigned_division() {
    let _read = fake(0x005F_D480, read as *const () as usize);
    let tables = Tables::new();
    let ids = [0x08FD_1613, 0x48EB_D0B1, 0x88EC_F76C];
    let mut properties = Holder {
        vt: tables.holder.as_ptr(),
        properties: vec![],
        numbers: vec![],
    };
    let mut occupant = Occupant {
        vt: tables.occupant.as_ptr(),
        holder: &mut properties,
        references: 1,
    };
    let pointer = &mut occupant as *mut Occupant as u32;
    let mut ring = Ring::new(&[(&mut occupant as *mut Occupant).cast()]);
    let mut object = Object::new(0x250);
    let this = object.ptr();
    unsafe {
        PollutionSim::from_ptr(this).set_energy_list(ring.node(0));
    }
    for present in 0..=3 {
        properties.numbers = ids
            .iter()
            .take(present)
            .enumerate()
            .map(|(i, &id)| (id, [u32::MAX, 7, 3][i]))
            .collect();
        unsafe {
            fc4p_006e3f50(this);
        }
        let mut expected = vec![Call::Holder(pointer)];
        expected.extend(ids.iter().take((present + 1).min(3)).map(|&id| Call::Read(id)));
        if present == 3 {
            expected.push(Call::Store(0x2781_2852, 0xFFFF_FFFD / 7, 0));
        }
        assert_eq!(take(), expected);
    }
    let mut empty = Ring::new(&[]);
    unsafe {
        PollutionSim::from_ptr(this).set_energy_list(empty.node(0));
        fc4p_006e3f50(this);
    }
    assert_eq!(take(), []);
    assert_eq!(unsigned_divide(u32::MAX, 0x8000_0000), 1);
}

#[test]
fn occupant_insertions_allow_duplicates_and_removal_releases_only_the_first_match() {
    let _allocate = fake(0x0041_3DD0, allocate as *const () as usize);
    let _erase = fake(0x0041_3F70, erase as *const () as usize);
    let tables = Tables::new();
    let mut occupant = Occupant {
        vt: tables.occupant.as_ptr(),
        holder: core::ptr::null_mut(),
        references: 1,
    };
    let pointer = (&mut occupant as *mut Occupant).cast::<u8>();
    let mut ring = Ring::new(&[0x1122usize as *mut u8]);
    let mut nodes = [[0u32; 3]; 2];
    ALLOCATIONS.with(|a| *a.borrow_mut() = nodes.iter_mut().map(|n| (n as *mut [u32; 3]).cast()).collect());
    let list = ring.list();
    for node in &mut nodes {
        let node = (node as *mut [u32; 3]).cast::<u8>();
        let old_first = ring.nodes[0][0];
        assert_eq!(
            unsafe { fc4p_006e45f0(core::ptr::null_mut(), list.cast(), pointer, 0x101) },
            (node as u32 & !0xFF) | 1
        );
        assert_eq!(
            take(),
            [Call::AddRef(pointer as u32), Call::Allocate(list as u32, 1, 0)]
        );
        unsafe {
            assert_eq!(ListNode::from_ptr(node).next() as u32, old_first);
            assert_eq!(ListNode::from_ptr(node).previous(), ring.node(0));
            assert_eq!(ListNode::from_ptr(node).occupant(), pointer);
        }
        assert_eq!(ring.nodes[0][0], node as u32);
    }
    assert_eq!(occupant.references, 3);
    for node in nodes.iter().rev() {
        let node = node.as_ptr() as u32;
        assert_eq!(
            unsafe { fc4p_006e45f0(core::ptr::null_mut(), list.cast(), pointer, 0x100) },
            0x1234_5601
        );
        assert_eq!(take(), [Call::Erase(list as u32, node), Call::Release(pointer as u32)]);
    }
    assert_eq!(occupant.references, 1);
    assert_eq!(
        unsafe { fc4p_006e45f0(core::ptr::null_mut(), list.cast(), pointer, 0) },
        ring.node(0) as u32 & !0xFF
    );
    assert_eq!(take(), []);
}

extern "C" fn application_fake() -> *mut u8 {
    log(Call::Application);
    SERVICES.with(|s| s.borrow().0)
}
extern "thiscall" fn query_service(_this: *mut u8, class: u32, iid: u32, output: *mut *mut u8) -> u32 {
    assert_eq!((class, iid), (0x3AE4_BEA3, 0x3AE4_BEAB));
    log(Call::QueryService);
    unsafe {
        *output = SERVICES.with(|s| s.borrow().1);
    }
    1
}
extern "thiscall" fn release_service_fake(reference: *mut *mut u8) {
    unsafe {
        let service = *reference;
        let global = SERVICES.with(|s| s.borrow().2.cast::<*mut u8>());
        assert_eq!(*global, service);
        log(Call::ReleaseService(service as u32));
        *reference = core::ptr::null_mut();
    }
}
extern "thiscall" fn free_node(_this: *mut u8, node: *mut u8) {
    log(Call::Free(node as u32));
    unsafe {
        let node = ListNode::from_ptr(node);
        assert_eq!(ListNode::from_ptr(node.previous()).next(), node.next());
        assert_eq!(ListNode::from_ptr(node.next()).previous(), node.previous());
        node.set_next(core::ptr::null_mut());
    }
}

#[test]
fn erase_range_unlinks_before_freeing_caches_next_and_initializes_the_shared_service_once() {
    let _app = fake(0x0087_93EC, application_fake as *const () as usize);
    let _release = fake(0x0041_1870, release_service_fake as *const () as usize);
    let mut service_vt = [0usize; 8];
    service_vt[4] = free_node as *const () as usize;
    let mut app_vt = [0usize; 8];
    app_vt[5] = query_service as *const () as usize;
    let mut service = [service_vt.as_ptr() as usize];
    let mut app = [app_vt.as_ptr() as usize];
    let service_ptr = service.as_mut_ptr().cast::<u8>();
    for cached in [false, true] {
        let mut global = if cached { service_ptr } else { core::ptr::null_mut() };
        let _data = fake_data(0x00B2_07E4, (&mut global as *mut *mut u8).cast());
        SERVICES.with(|s| {
            *s.borrow_mut() = (
                app.as_mut_ptr().cast(),
                service_ptr,
                (&mut global as *mut *mut u8).cast(),
            )
        });
        let mut ring = Ring::new(&[0x1001usize as *mut u8, 0x1002usize as *mut u8, 0x1003usize as *mut u8]);
        let list = ring.list();
        let (first, second, last) = (ring.node(1), ring.node(2), ring.node(3));
        let mut output = core::ptr::null_mut();
        assert_eq!(
            unsafe { fc4p_006e41f0(list, &mut output, first, last) },
            &mut output as *mut *mut u8
        );
        let mut expected = vec![];
        if !cached {
            expected.extend([
                Call::Application,
                Call::QueryService,
                Call::ReleaseService(service_ptr as u32),
            ]);
        }
        expected.extend([Call::Free(first as u32), Call::Free(second as u32)]);
        assert_eq!(take(), expected);
        assert_eq!(output, last);
        assert_eq!(global, service_ptr);
        assert_eq!(ring.nodes[0][0], last as u32);
        let sentinel = ring.node(0) as u32;
        assert_eq!(ring.nodes[3][1], sentinel);
        assert_eq!(
            unsafe { fc4p_006e41f0(list, &mut output, last, last) },
            &mut output as *mut *mut u8
        );
        assert_eq!(output, last);
        assert_eq!(take(), []);
    }
}

extern "thiscall" fn range_fake(this: *mut u8, output: *mut *mut u8, first: *mut u8, last: *mut u8) -> u32 {
    log(Call::Range(this as u32, first as u32, last as u32));
    unsafe {
        *output = last;
    }
    output as u32
}
extern "thiscall" fn append_fake(
    this: *mut u8,
    position: *mut u8,
    first: *mut u8,
    last: *mut u8,
    _context: *mut *mut u8,
) {
    log(Call::Append(this as u32, position as u32, first as u32, last as u32));
}

#[test]
fn assignment_reuses_nodes_without_retain_calls_and_delegates_only_the_remaining_range() {
    let _erase = fake(0x006E_41F0, range_fake as *const () as usize);
    let _append = fake(0x006E_4270, append_fake as *const () as usize);
    for (destination_count, source_count) in [(0, 0), (0, 2), (2, 0), (2, 2), (1, 3), (3, 1)] {
        let mut destination = Ring::new(&vec![0xAAAAusize as *mut u8; destination_count]);
        let values: Vec<_> = (0..source_count).map(|i| (0x1000usize + i) as *mut u8).collect();
        let mut source = Ring::new(&values);
        let source_before = source.nodes.clone();
        let destination_before = destination.nodes.clone();
        let this = destination.list();
        assert_eq!(unsafe { fc4p_006e5660(this, source.list().cast()) }, this);
        let reused = destination_count.min(source_count);
        for (i, value) in values.iter().take(reused).enumerate() {
            assert_eq!(destination.nodes[i + 1][2], *value as u32);
            assert_eq!(destination.nodes[i + 1][..2], destination_before[i + 1][..2]);
        }
        assert_eq!(source.nodes, source_before);
        let expected = if source_count <= destination_count {
            Call::Range(
                this as u32,
                destination.node(if source_count == destination_count {
                    0
                } else {
                    source_count + 1
                }) as u32,
                destination.node(0) as u32,
            )
        } else {
            Call::Append(
                this as u32,
                destination.node(0) as u32,
                source.node(destination_count + 1) as u32,
                source.node(0) as u32,
            )
        };
        assert_eq!(take(), [expected]);
        assert_eq!(unsafe { fc4p_006e5660(this, this.cast()) }, this);
        assert_eq!(take(), []);
    }
}

extern "thiscall" fn current_capacity(_this: *mut u8, id: u32, occupant: *mut Occupant, output: *mut u32) -> u32 {
    log(Call::Current(id, occupant as u32));
    unsafe {
        let holder = &*(*occupant).holder;
        if let Some(&(_, capacity)) = holder.numbers.iter().find(|&&(key, _)| key == id) {
            *output = capacity;
            0xABCD_1201
        } else {
            0xABCD_1200
        }
    }
}

#[test]
fn used_capacity_redistributes_before_subtraction_and_preserves_signed_comparison_quirks() {
    let tables = Tables::new();
    for (capacities, initial_used, total, initial_remaining, count, final_used, final_remaining) in [
        (
            vec![Some(20), Some(100), Some(100)],
            vec![0, 10, 0],
            300,
            180,
            3,
            vec![20, 100, 90],
            u32::MAX - 19,
        ),
        (
            vec![Some(20), Some(100), Some(30)],
            vec![0, 10, 0],
            300,
            180,
            3,
            vec![20, 100, 30],
            40,
        ),
        (
            vec![None, Some(0), Some(100)],
            vec![0, 0, 10],
            300,
            90,
            3,
            vec![0, 0, 100],
            0,
        ),
        (
            vec![Some(0x8000_0000)],
            vec![0],
            100,
            100,
            1,
            vec![0x8000_0000],
            0x8000_0064,
        ),
        (vec![Some(50), Some(50)], vec![0, 0], 100, 0, 2, vec![0, 0], 0),
        (vec![Some(50), Some(50)], vec![0, 0], 0, 100, 2, vec![0, 0], 100),
        (vec![Some(50), Some(50)], vec![0, 0], 100, 100, 0, vec![0, 0], 100),
    ] {
        let mut variants: Vec<_> = initial_used
            .iter()
            .map(|&value| Variant {
                vt: tables.variant.as_ptr(),
                value,
                tag: 1,
            })
            .collect();
        let mut properties: Vec<_> = variants
            .iter_mut()
            .map(|v| Property {
                vt: tables.property.as_ptr(),
                variant: v,
                tag: 1,
            })
            .collect();
        let mut holders: Vec<_> = properties
            .iter_mut()
            .zip(&capacities)
            .map(|(p, &c)| Holder {
                vt: tables.holder.as_ptr(),
                properties: vec![(20, p)],
                numbers: c.map_or_else(Vec::new, |c| vec![(10, c)]),
            })
            .collect();
        let mut occupants: Vec<_> = holders
            .iter_mut()
            .map(|h| Occupant {
                vt: tables.occupant.as_ptr(),
                holder: h,
                references: 1,
            })
            .collect();
        let occupant_pointers: Vec<_> = occupants.iter_mut().map(|o| (o as *mut Occupant).cast()).collect();
        let mut ring = Ring::new(&occupant_pointers);
        let mut sim_vtable = [0usize; 64];
        sim_vtable[0xD4 / 4] = current_capacity as *const () as usize;
        let mut object = Object::new(0x250);
        let this = object.ptr();
        unsafe {
            write_unaligned(this.cast(), sim_vtable.as_ptr());
        }
        let mut remaining = initial_remaining;
        unsafe {
            fc4p_006e3fe0(this, ring.list().cast(), 10, 20, total, &mut remaining, count);
        }
        assert_eq!(remaining, final_remaining);
        assert_eq!(variants.iter().map(|v| v.value).collect::<Vec<_>>(), final_used);
        let calls = take();
        if count == 0 || total == 0 || initial_remaining == 0 {
            assert_eq!(calls, []);
            continue;
        }
        let mut expected = vec![];
        for (i, capacity) in capacities.iter().enumerate() {
            expected.extend([
                Call::Holder(occupant_pointers[i] as u32),
                Call::Current(10, occupant_pointers[i] as u32),
            ]);
            if capacity.is_some() {
                expected.extend([
                    Call::Find(20),
                    Call::Variant(1),
                    Call::Buffer(1),
                    Call::Reset(final_used[i]),
                ]);
            }
        }
        assert_eq!(calls, expected);
    }
}

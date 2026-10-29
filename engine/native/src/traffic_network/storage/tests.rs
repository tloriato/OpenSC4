// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;

unsafe fn fc4p_004b0000(this: *mut u8, network: u32, lane: u32) -> u32 {
    unsafe { super::fc4p_004b0000(this, network, lane) as u32 }
}

#[derive(Debug, PartialEq)]
enum Call {
    Virtual(usize, usize, Vec<u32>),
    Free(usize),
    Tree(usize, usize),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    tile: usize,
    objects: Vec<(usize, u32, u32)>,
    lane_counter: usize,
    edge_counter: usize,
    switch_vtable: usize,
    mutate_tree: Option<(usize, usize)>,
    inspect_unlinked: Option<(usize, usize)>,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn record(this: *mut u8, slot: usize, args: &[u32]) {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Virtual(this as usize, slot, args.to_vec()))
    });
}

extern "thiscall" fn shutdown(this: *mut u8) {
    record(this, 0x10, &[]);
    STATE.with(|s| {
        let vt = s.borrow().switch_vtable;
        if vt != 0 {
            unsafe { TrafficNetworkMap::from_ptr(this) }.set_vtable(vt as *const usize);
        }
    });
}

extern "thiscall" fn delete(this: *mut u8, flags: u32) {
    record(this, 0xB4, &[flags]);
}

extern "thiscall" fn release(this: *mut u8) -> u64 {
    record(this, 8, &[]);
    words(0x1234_5678, 0xC008_0000)
}

extern "C" fn free(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some((previous, next)) = s.inspect_unlinked {
            assert_eq!(
                unsafe { AutomatonLink::from_ptr(previous as *mut u8) }.next() as usize,
                next
            );
            assert_eq!(
                unsafe { AutomatonLink::from_ptr(next as *mut u8) }.previous() as usize,
                previous
            );
        }
        s.calls.push(Call::Free(this as usize));
    });
    0xDEAD_BEEF_1234_5678
}

extern "thiscall" fn subtree(this: *mut u8, node: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Tree(this as usize, node as usize));
        if let Some((parent, left)) = s.mutate_tree.take() {
            unsafe { TreeNode::from_ptr(parent as *mut u8) }.set_left(left as *mut u8);
        }
    });
    0xAABB_CCDD_1122_3344
}

extern "thiscall" fn lookup(this: *mut u8, key: u32) -> u64 {
    record(this, 0x40, &[key]);
    STATE.with(|s| words(s.borrow().tile as u32, 0xC040_0000))
}

extern "thiscall" fn inactive(this: *mut u8) -> u64 {
    record(this, 0x154, &[]);
    STATE.with(|s| {
        words(
            s.borrow().objects.iter().find(|o| o.0 == this as usize).unwrap().1,
            0xC154_0000,
        )
    })
}

extern "thiscall" fn lane(this: *mut u8) -> u64 {
    record(this, 0x220, &[]);
    STATE.with(|s| {
        words(
            s.borrow().objects.iter().find(|o| o.0 == this as usize).unwrap().2,
            0xC220_0000,
        )
    })
}

extern "thiscall" fn lane_counter(this: *mut u8, network: u32, category: u32, lane: u32) -> *mut u16 {
    record(this, 0x5C, &[network, category, lane]);
    STATE.with(|s| s.borrow().lane_counter as *mut u16)
}

extern "thiscall" fn edge_counter(this: *mut u8, network: u32, edge: u32) -> *mut u16 {
    record(this, 0x58, &[network, edge]);
    STATE.with(|s| s.borrow().edge_counter as *mut u16)
}

fn object(vt: &[usize], size: usize) -> Object {
    let mut mem = Object::new(size);
    unsafe {
        mem.ptr().cast::<*const usize>().write(vt.as_ptr());
    }
    mem
}

fn list(objects: &[*mut u8]) -> (Object, Vec<Object>, Object) {
    let mut header = Object::new(12);
    let mut nodes = objects.iter().map(|_| Object::new(12)).collect::<Vec<_>>();
    let mut links = vec![header.ptr()];
    links.extend(nodes.iter_mut().map(Object::ptr));
    for i in 0..links.len() {
        let link = unsafe { AutomatonLink::from_ptr(links[i]) };
        link.set_next(links[(i + 1) % links.len()]);
        link.set_previous(links[(i + links.len() - 1) % links.len()]);
        if i > 0 {
            link.set_object(objects[i - 1]);
        }
    }
    let mut tile = Object::new(0x58);
    unsafe { TileInfo::from_ptr(tile.ptr()) }.set_automata(header.ptr());
    (header, nodes, tile)
}

#[test]
fn release_unsigned_counts_and_shutdown_reloads_vtable() {
    reset();
    let mut vt = [0usize; 0xB8 / 4];
    let mut next_vt = vt;
    vt[0x10 / 4] = shutdown as *const () as usize;
    // Destruction is available only in the table installed by Shutdown.
    next_vt[0xB4 / 4] = delete as *const () as usize;
    let mut mem = object(&vt, 0x80);
    let p = mem.ptr();
    let map = unsafe { TrafficNetworkMap::from_ptr(p) };
    for count in [2, 3, 0x8000_0000, u32::MAX] {
        map.set_references(count);
        let mut expected = unsafe { core::slice::from_raw_parts(p, 0x80) }.to_vec();
        expected[0x14..0x18].copy_from_slice(&count.wrapping_sub(1).to_le_bytes());
        assert_eq!(unsafe { fc4p_004afaa0(p) }, count - 1);
        assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x80) }, expected);
        assert!(calls().is_empty());
    }
    STATE.with(|s| s.borrow_mut().switch_vtable = next_vt.as_ptr() as usize);
    for count in [0, 1] {
        map.set_vtable(vt.as_ptr());
        map.set_references(count);
        assert_eq!(unsafe { fc4p_004afaa0(p) }, 0);
        assert_eq!(map.references(), count);
        assert_eq!(
            calls(),
            [
                Call::Virtual(p as usize, 0x10, vec![]),
                Call::Virtual(p as usize, 0xB4, vec![1])
            ]
        );
    }
}

#[test]
fn tree_destruction_visits_right_before_parent_and_left_and_retains_free_edx() {
    reset();
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let mut root = Object::new(16);
    let mut left = Object::new(16);
    let mut right = Object::new(16);
    let parent = unsafe { TreeNode::from_ptr(root.ptr()) };
    parent.set_left(left.ptr());
    parent.set_right(right.ptr());
    let this = 0x1234usize as *mut u8;
    let args = [root.ptr() as u32];
    let mut call = RawCall::new(Conv::Thiscall, fc4p_004afad0 as *const () as usize, this as u32, &args);
    call.edx = 0x9988_7766;
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!(call.edx, 0xDEAD_BEEF);
    assert_eq!(
        calls(),
        [
            Call::Free(right.ptr() as usize),
            Call::Free(root.ptr() as usize),
            Call::Free(left.ptr() as usize)
        ]
    );
    let null = [0];
    call.args = null.as_ptr();
    call.edx = 0xABCD_1234;
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!(call.edx, 0xABCD_1234);
    assert!(calls().is_empty());
}

#[test]
fn tree_recursion_uses_game_entry_and_reads_left_after_recursing() {
    reset();
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let _tree = fake(0x004A_FAD0, subtree as *const () as usize);
    let mut root = Object::new(16);
    let mut old_left = Object::new(16);
    let mut new_left = Object::new(16);
    let mut right = Object::new(16);
    let parent = unsafe { TreeNode::from_ptr(root.ptr()) };
    parent.set_left(old_left.ptr());
    parent.set_right(right.ptr());
    STATE.with(|s| s.borrow_mut().mutate_tree = Some((root.ptr() as usize, new_left.ptr() as usize)));
    let this = 0x1234usize as *mut u8;
    unsafe {
        call_thiscall_with_eax(fc4p_004afad0 as *const () as usize, this, &[root.ptr() as u32], 0);
    }
    assert_eq!(
        calls(),
        [
            Call::Tree(this as usize, right.ptr() as usize),
            Call::Free(root.ptr() as usize),
            Call::Tree(this as usize, 0),
            Call::Free(new_left.ptr() as usize)
        ]
    );
}

#[test]
fn clear_releases_every_object_before_freeing_nodes_and_only_resets_header() {
    reset();
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let mut vt = [0usize; 3];
    vt[2] = release as *const () as usize;
    let mut network = object(&vt, 4);
    let mut first = object(&vt, 4);
    let mut last = object(&vt, 4);
    let (mut header, mut nodes, mut tile) = list(&[first.ptr(), core::ptr::null_mut(), last.ptr()]);
    unsafe { TileInfo::from_ptr(tile.ptr()) }.set_network(network.ptr());
    let before_tile = unsafe { core::slice::from_raw_parts(tile.ptr(), 0x58) }.to_vec();
    let before_nodes = nodes
        .iter_mut()
        .map(|n| unsafe { core::slice::from_raw_parts(n.ptr(), 12) }.to_vec())
        .collect::<Vec<_>>();
    unsafe {
        fc4p_004afb10(core::ptr::null_mut(), tile.ptr());
    }
    assert_eq!(
        calls(),
        [
            Call::Virtual(network.ptr() as usize, 8, vec![]),
            Call::Virtual(first.ptr() as usize, 8, vec![]),
            Call::Virtual(last.ptr() as usize, 8, vec![]),
            Call::Free(nodes[0].ptr() as usize),
            Call::Free(nodes[1].ptr() as usize),
            Call::Free(nodes[2].ptr() as usize)
        ]
    );
    let sentinel = unsafe { AutomatonLink::from_ptr(header.ptr()) };
    assert_eq!((sentinel.next(), sentinel.previous()), (header.ptr(), header.ptr()));
    assert_eq!(unsafe { core::slice::from_raw_parts(tile.ptr(), 0x58) }, before_tile);
    for (node, before) in nodes.iter_mut().zip(before_nodes) {
        assert_eq!(unsafe { core::slice::from_raw_parts(node.ptr(), 12) }, before);
    }
    unsafe {
        fc4p_004afb10(core::ptr::null_mut(), core::ptr::null_mut());
    }
    assert!(calls().is_empty());
    // An empty list and a null network need neither Release nor free.
    unsafe { TileInfo::from_ptr(tile.ptr()) }.set_network(core::ptr::null_mut());
    unsafe {
        fc4p_004afb10(core::ptr::null_mut(), tile.ptr());
    }
    assert!(calls().is_empty());
}

fn map_vtable() -> [usize; 0x60 / 4] {
    let mut vt = [0; 0x60 / 4];
    vt[0x40 / 4] = lookup as *const () as usize;
    vt[0x58 / 4] = edge_counter as *const () as usize;
    vt[0x5C / 4] = lane_counter as *const () as usize;
    vt
}

#[test]
fn count_lane_filters_low_byte_and_preserves_upper_half_of_last_result() {
    reset();
    let vt = map_vtable();
    let mut map = object(&vt, 0x80);
    let p = map.ptr();
    let mut avt = [0usize; 0x224 / 4];
    avt[0x154 / 4] = inactive as *const () as usize;
    avt[0x220 / 4] = lane as *const () as usize;
    let mut a = object(&avt, 4);
    let mut b = object(&avt, 4);
    let mut c = object(&avt, 4);
    let (_header, _nodes, mut tile) = list(&[a.ptr(), b.ptr(), c.ptr()]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tile = tile.ptr() as usize;
        s.objects = vec![
            (a.ptr() as usize, 0xAABB_CC00, 0x1234_5678),
            (b.ptr() as usize, 0x9988_7701, 0x1234_5678),
            (c.ptr() as usize, 0, 0x1234_5678),
        ];
    });
    assert_eq!(unsafe { fc4p_004b0000(p, 77, 0x1234_5678) }, 0x1234_0002);
    assert_eq!(
        calls(),
        [
            Call::Virtual(p as usize, 0x40, vec![77]),
            Call::Virtual(a.ptr() as usize, 0x154, vec![]),
            Call::Virtual(a.ptr() as usize, 0x220, vec![]),
            Call::Virtual(b.ptr() as usize, 0x154, vec![]),
            Call::Virtual(c.ptr() as usize, 0x154, vec![]),
            Call::Virtual(c.ptr() as usize, 0x220, vec![])
        ]
    );
    STATE.with(|s| s.borrow_mut().objects[2].1 = 0xDEAD_BEEF);
    assert_eq!(unsafe { fc4p_004b0000(p, 77, 0x1234_5678) }, 0xDEAD_0001);
    calls();
    let (mut header, _nodes, mut tile) = list(&[]);
    STATE.with(|s| s.borrow_mut().tile = tile.ptr() as usize);
    assert_eq!(unsafe { fc4p_004b0000(p, 77, 0) }, header.ptr() as u32 & 0xFFFF_0000);
    STATE.with(|s| s.borrow_mut().tile = 0);
    assert_eq!(unsafe { fc4p_004b0000(p, 77, 0) }, 0);
}

fn exit(map: *mut u8, object: *mut u8, network: u32, lane: u32) -> u32 {
    unsafe {
        call_thiscall_with_eax(
            fc4p_004b02a0 as *const () as usize,
            map,
            &[object as u32, network, lane],
            0x7654_32FF,
        )
    }
    .0
}

#[test]
fn exit_unlinks_all_duplicates_decrements_without_underflow_and_releases_after_counters() {
    reset();
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let vt = map_vtable();
    let mut map = object(&vt, 0x80);
    let p = map.ptr();
    let mut avt = [0usize; 3];
    avt[2] = release as *const () as usize;
    let mut a = object(&avt, 0x1E8);
    let mut other = object(&avt, 0x1E8);
    let ap = a.ptr();
    unsafe { Automaton::from_ptr(ap) }.set_category(0x9988_7766);
    let (mut header, mut nodes, mut tile) = list(&[ap, other.ptr(), ap]);
    let mut counts = [1u16, 0xFFFF, 0xABCD];
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tile = tile.ptr() as usize;
        s.lane_counter = counts.as_mut_ptr() as usize;
        s.edge_counter = counts.as_mut_ptr().wrapping_add(1) as usize;
    });
    assert_eq!(exit(p, ap, 99, 0xFA12_3456), (tile.ptr() as u32 & !0xFF) | 1);
    assert_eq!(counts, [0, 0xFFFD, 0xABCD]);
    let sentinel = unsafe { AutomatonLink::from_ptr(header.ptr()) };
    let middle = unsafe { AutomatonLink::from_ptr(nodes[1].ptr()) };
    assert_eq!((sentinel.next(), sentinel.previous()), (nodes[1].ptr(), nodes[1].ptr()));
    assert_eq!((middle.next(), middle.previous()), (header.ptr(), header.ptr()));
    assert_eq!(
        calls(),
        [
            Call::Virtual(p as usize, 0x40, vec![99]),
            Call::Free(nodes[0].ptr() as usize),
            Call::Virtual(p as usize, 0x5C, vec![99, 0x9988_7766, 0xFA12_3456]),
            Call::Virtual(p as usize, 0x58, vec![99, 10]),
            Call::Virtual(ap as usize, 8, vec![]),
            Call::Free(nodes[2].ptr() as usize),
            Call::Virtual(p as usize, 0x5C, vec![99, 0x9988_7766, 0xFA12_3456]),
            Call::Virtual(p as usize, 0x58, vec![99, 10]),
            Call::Virtual(ap as usize, 8, vec![]),
        ]
    );
    assert_eq!(exit(p, ap, 99, 0), tile.ptr() as u32 & !0xFF);
    assert_eq!(calls(), [Call::Virtual(p as usize, 0x40, vec![99])]);
}

#[test]
fn exit_null_missing_empty_and_unlink_is_visible_to_free() {
    reset();
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let vt = map_vtable();
    let mut map = object(&vt, 0x80);
    let p = map.ptr();
    assert_eq!(exit(p, core::ptr::null_mut(), 99, 0), 0x7654_3200);
    assert_eq!(exit(p, 0x1234usize as *mut u8, 0, 0), 0x7654_3200);
    assert!(calls().is_empty());
    assert_eq!(exit(p, 0x1234usize as *mut u8, 99, 0), 0);
    calls();
    let (mut header, _nodes, mut tile) = list(&[]);
    STATE.with(|s| s.borrow_mut().tile = tile.ptr() as usize);
    assert_eq!(exit(p, 0x1234usize as *mut u8, 99, 0), tile.ptr() as u32 & !0xFF);
    calls();
    let mut avt = [0usize; 3];
    avt[2] = release as *const () as usize;
    let mut a = object(&avt, 0x1E8);
    let (_old_header, _old_nodes, mut next_tile) = list(&[a.ptr()]);
    let mut zero_count = 0u16;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tile = next_tile.ptr() as usize;
        s.lane_counter = &mut zero_count as *mut u16 as usize;
        s.edge_counter = s.lane_counter;
        let head = unsafe { TileInfo::from_ptr(next_tile.ptr()) }.automata() as usize;
        s.inspect_unlinked = Some((head, head));
    });
    assert_eq!(exit(p, a.ptr(), 99, 0) as u8, 1);
    assert_eq!(zero_count, 0);
    // Keep the earlier empty sentinel alive through the checks as well.
    assert_eq!(unsafe { AutomatonLink::from_ptr(header.ptr()) }.next(), header.ptr());
}

mod vectors {
    use super::*;

    unsafe fn fc4p_004b0110(destination: *mut u8, count: u32, source: *mut u8) -> *mut u8 {
        let f: unsafe extern "C" fn(*mut u8, u32, *mut u8) -> *mut u8 =
            unsafe { core::mem::transmute(super::super::fc4p_004b0110 as *const ()) };
        unsafe { f(destination, count, source) }
    }

    unsafe fn fc4p_004b0440(begin: *mut u8, end: *mut u8, destination: *mut u8) -> *mut u8 {
        let f: unsafe extern "C" fn(*mut u8, *mut u8, *mut u8) -> *mut u8 =
            unsafe { core::mem::transmute(super::super::fc4p_004b0440 as *const ()) };
        unsafe { f(begin, end, destination) }
    }

    #[derive(Default)]
    struct Allocations {
        blocks: Vec<usize>,
        requests: Vec<u32>,
        destination: usize,
        mutate: Option<(usize, usize, usize)>,
    }
    thread_local! { static ALLOCATIONS: RefCell<Allocations> = RefCell::new(Allocations::default()); }
    extern "C" fn allocate(bytes: u32) -> *mut u8 {
        ALLOCATIONS.with(|a| {
            let mut a = a.borrow_mut();
            let dest = unsafe { RecordVector::from_ptr(a.destination as *mut u8) };
            assert!(dest.begin().is_null() && dest.end().is_null() && dest.capacity().is_null());
            a.requests.push(bytes);
            a.destination += 12;
            if let Some((source, begin, end)) = a.mutate.take() {
                let view = unsafe { RecordVector::from_ptr(source as *mut u8) };
                view.set_begin(begin as *mut u8);
                view.set_end(end as *mut u8);
            }
            a.blocks.remove(0) as *mut u8
        })
    }

    fn header(data: &mut [[u32; 4]]) -> Object {
        let mut memory = Object::new(12);
        let view = unsafe { RecordVector::from_ptr(memory.ptr()) };
        let begin = data.as_mut_ptr().cast::<u8>();
        let end = begin.wrapping_add(data.len() * 16);
        view.set_begin(begin);
        view.set_end(end);
        view.set_capacity(end);
        memory
    }

    #[test]
    fn fill_allocates_each_vector_after_zeroing_header_and_copies_all_record_words() {
        let _allocation = fake(0x0090_CF54, allocate as *const () as usize);
        let mut data = [[1u32, 2, 3, 4], [5, 6, 7, 8]];
        let mut source = header(&mut data);
        let mut destination = Object::new(48);
        let p = destination.ptr();
        let mut blocks = [Object::new(64), Object::new(64)];
        for block in &mut blocks {
            unsafe {
                core::ptr::write_bytes(block.ptr(), 0xA5, 64);
            }
        }
        unsafe {
            core::ptr::write_bytes(p, 0x5A, 48);
        }
        ALLOCATIONS.with(|a| {
            *a.borrow_mut() = Allocations {
                blocks: blocks.iter_mut().map(|o| o.ptr() as usize).collect(),
                destination: p as usize,
                ..Allocations::default()
            }
        });
        let (eax, _, arguments) = unsafe {
            crate::traffic_network::stack_arguments(
                super::super::fc4p_004b0110 as *const () as usize,
                core::ptr::null_mut(),
                [p as u32, 2, source.ptr() as u32],
                true,
            )
        };
        assert_eq!(eax, p.wrapping_add(24) as u32);
        assert_eq!(arguments, [0, 2, source.ptr() as u32]);
        assert_eq!(ALLOCATIONS.with(|a| a.borrow().requests.clone()), [32, 32]);
        for (i, block) in blocks.iter_mut().enumerate() {
            let vector = unsafe { RecordVector::from_ptr(p.wrapping_add(i * 12)) };
            assert_eq!(
                (vector.begin(), vector.end(), vector.capacity()),
                (block.ptr(), block.ptr().wrapping_add(32), block.ptr().wrapping_add(32))
            );
            assert_eq!(
                unsafe { core::slice::from_raw_parts(block.ptr().cast::<[u32; 4]>(), 2) },
                data
            );
            assert_eq!(
                unsafe { core::slice::from_raw_parts(block.ptr().wrapping_add(32), 32) },
                [0xA5; 32]
            );
        }
        assert_eq!(
            unsafe { core::slice::from_raw_parts(p.wrapping_add(24), 24) },
            [0x5A; 24]
        );
    }

    #[test]
    fn range_copy_handles_empty_source_vectors_and_empty_or_null_destination_ranges() {
        let _allocation = fake(0x0090_CF54, allocate as *const () as usize);
        let mut data = [[10u32, 20, 30, 40]];
        let mut populated = header(&mut data);
        let mut source = Object::new(24);
        unsafe {
            core::ptr::copy_nonoverlapping(populated.ptr(), source.ptr().wrapping_add(12), 12);
        }
        let mut destination = Object::new(24);
        let mut block = Object::new(16);
        ALLOCATIONS.with(|a| {
            *a.borrow_mut() = Allocations {
                blocks: vec![block.ptr() as usize],
                destination: destination.ptr() as usize + 12,
                ..Allocations::default()
            }
        });
        let (eax, _, arguments) = unsafe {
            crate::traffic_network::stack_arguments(
                super::super::fc4p_004b0440 as *const () as usize,
                core::ptr::null_mut(),
                [
                    source.ptr() as u32,
                    source.ptr().wrapping_add(24) as u32,
                    destination.ptr() as u32,
                ],
                true,
            )
        };
        assert_eq!(eax, destination.ptr().wrapping_add(24) as u32);
        assert_eq!(
            arguments,
            [
                source.ptr() as u32,
                source.ptr().wrapping_add(24) as u32,
                data.as_mut_ptr().wrapping_add(1) as u32
            ]
        );
        let empty = unsafe { RecordVector::from_ptr(destination.ptr()) };
        assert!(empty.begin().is_null() && empty.end().is_null() && empty.capacity().is_null());
        assert_eq!(
            unsafe { core::slice::from_raw_parts(block.ptr().cast::<u32>(), 4) },
            data[0]
        );
        assert_eq!(ALLOCATIONS.with(|a| a.borrow().requests.clone()), [16]);
        assert_eq!(
            unsafe { fc4p_004b0440(core::ptr::null_mut(), core::ptr::null_mut(), destination.ptr()) },
            destination.ptr()
        );
        assert_eq!(
            unsafe { fc4p_004b0110(destination.ptr(), 0, core::ptr::null_mut()) },
            destination.ptr()
        );
        assert_eq!(
            unsafe { fc4p_004b0110(core::ptr::null_mut(), 1, core::ptr::null_mut()) } as usize,
            12
        );
        assert_eq!(
            unsafe { fc4p_004b0440(source.ptr(), source.ptr().wrapping_add(12), core::ptr::null_mut()) } as usize,
            12
        );
        assert_eq!(ALLOCATIONS.with(|a| a.borrow().requests.clone()), [16]);
    }

    #[test]
    fn copy_reloads_source_after_allocator_and_allows_null_single_record_storage() {
        let _allocation = fake(0x0090_CF54, allocate as *const () as usize);
        let mut data = [[1u32, 2, 3, 4], [5, 6, 7, 8]];
        let mut replacement = [[99u32, 98, 97, 96]];
        let mut source = header(&mut data);
        let mut destination = Object::new(12);
        let mut block = Object::new(32);
        let p = destination.ptr();
        let input = replacement.as_mut_ptr().cast::<u8>();
        ALLOCATIONS.with(|a| {
            *a.borrow_mut() = Allocations {
                blocks: vec![block.ptr() as usize],
                destination: p as usize,
                mutate: Some((source.ptr() as usize, input as usize, input.wrapping_add(16) as usize)),
                ..Allocations::default()
            }
        });
        unsafe {
            fc4p_004b0110(p, 1, source.ptr());
        }
        let dest = unsafe { RecordVector::from_ptr(p) };
        assert_eq!(
            (dest.begin(), dest.end(), dest.capacity()),
            (block.ptr(), block.ptr().wrapping_add(16), block.ptr().wrapping_add(32))
        );
        assert_eq!(
            unsafe { core::slice::from_raw_parts(block.ptr().cast::<u32>(), 4) },
            replacement[0]
        );
        assert_eq!(ALLOCATIONS.with(|a| a.borrow().requests.clone()), [32]);
        // The first record construction is skipped when the game allocator yields null, but the end advances.
        ALLOCATIONS.with(|a| {
            *a.borrow_mut() = Allocations {
                blocks: vec![0],
                destination: p as usize,
                ..Allocations::default()
            }
        });
        unsafe {
            fc4p_004b0440(source.ptr(), source.ptr().wrapping_add(12), p);
        }
        assert!(dest.begin().is_null());
        assert_eq!((dest.end() as usize, dest.capacity() as usize), (16, 16));
    }
}

#[test]
fn count_and_exit_keep_secondary_registers_for_null_empty_and_populated_lists() {
    reset();
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let vt = map_vtable();
    let mut map = object(&vt, 0x80);
    let p = map.ptr();
    assert_eq!(unsafe { super::fc4p_004b0000(p, 99, 0) }, words(0, 0xC040_0000));
    let mut avt = [0usize; 0x224 / 4];
    avt[2] = release as *const () as usize;
    avt[0x154 / 4] = inactive as *const () as usize;
    avt[0x220 / 4] = lane as *const () as usize;
    let mut a = object(&avt, 0x1E8);
    let (mut header, _nodes, mut tile) = list(&[a.ptr()]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tile = tile.ptr() as usize;
        s.objects = vec![(a.ptr() as usize, 0, 0x1234_5678)];
    });
    assert_eq!(
        unsafe { super::fc4p_004b0000(p, 99, 0x1234_5678) },
        words(0x1234_0001, 0xC220_0000)
    );
    STATE.with(|s| s.borrow_mut().objects[0].1 = 0xAABB_CC01);
    assert_eq!(
        unsafe { super::fc4p_004b0000(p, 99, 0x1234_5678) },
        words(0xAABB_0000, 0xC154_0000)
    );
    let null = [0, 99, 0];
    let mut call = RawCall::new(Conv::Thiscall, fc4p_004b02a0 as *const () as usize, p as u32, &null);
    call.eax = 0xAABB_CCFF;
    call.edx = 0x1122_3344;
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0xAABB_CC00, 0x1122_3344));
    let args = [a.ptr() as u32, 99, 0];
    call.args = args.as_ptr();
    let mut counter = 0u16;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.lane_counter = &mut counter as *mut u16 as usize;
        s.edge_counter = s.lane_counter;
    });
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), ((tile.ptr() as u32 & !0xFF) | 1, 0xC008_0000));
    assert_eq!(
        unsafe { super::fc4p_004b0000(p, 99, 0) },
        words(header.ptr() as u32 & 0xFFFF_0000, 0xC040_0000)
    );
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (tile.ptr() as u32 & !0xFF, 0xC040_0000));
    STATE.with(|s| s.borrow_mut().tile = 0);
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0, 0xC040_0000));
    let (_, _, arguments) = unsafe {
        crate::traffic_network::stack_arguments(fc4p_004b02a0 as *const () as usize, p, [a.ptr() as u32, 99, 0], false)
    };
    assert_eq!(arguments, [a.ptr() as u32, 0, 0]);
}

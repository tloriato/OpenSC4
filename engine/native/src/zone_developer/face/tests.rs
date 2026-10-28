// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Random(usize, u32),
    Allocate(u32),
    Network(usize, i32, i32, u32),
    Create(usize, u32, u32, [u32; 9]),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    #[allow(clippy::vec_box)] // Linked nodes need stable addresses while the vector grows.
    allocations: Vec<Box<[u32; 3]>>,
    draw: i32,
    roads: Vec<(i32, i32)>,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn random_fake(this: *mut u8, limit: u32) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Random(this as usize, limit));
        s.draw
    })
}

extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        let mut node = Box::new([0xA5A5_A5A5; 3]);
        let p = node.as_mut_ptr().cast();
        s.allocations.push(node);
        p
    })
}

extern "thiscall" fn network_fake(this: *mut u8, x: i32, z: i32, mask: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Network(this as usize, x, z, mask));
        0xABCD_EF00 | (u32::from(s.roads.contains(&(x, z))) * 0x80)
    })
}

#[unsafe(naked)]
unsafe extern "C" fn create_fake() {
    core::arch::naked_asm!("push eax", "push edx", "push ecx", "call {body}",
        "add esp, 12", "ret 36", body = sym create_body);
}

unsafe extern "C" fn create_body(
    this: *mut u8,
    edx: u32,
    eax: u32,
    _return: u32,
    x: u32,
    z: u32,
    w: u32,
    h: u32,
    facing: u32,
    out: *mut *mut u8,
    a: u32,
    b: u32,
    c: u32,
) -> u64 {
    STATE.with(|s| {
        s.borrow_mut().calls.push(Call::Create(
            this as usize,
            eax,
            edx,
            [x, z, w, h, facing, out as u32, a, b, c],
        ))
    });
    unsafe { out.write(0x1234usize as *mut u8) };
    0xFEDC_BA98_7654_3201
}

struct World {
    object: Object,
    primary: Box<[usize; 9]>,
    manager: Box<[usize; 23]>,
    header: Box<[u32; 3]>,
    #[allow(clippy::vec_box)] // The circular links retain each allocation's address.
    nodes: Vec<Box<[u32; 3]>>,
    random: Box<u8>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new(orientations: &[i32]) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            object: Object::new(0xEC),
            primary: Box::new([0; 9]),
            manager: Box::new([0; 23]),
            header: Box::new([0xA5A5_A5A5; 3]),
            nodes: Vec::new(),
            random: Box::new(0),
            _fakes: Vec::new(),
        };
        w.primary[8] = network_fake as *const () as usize;
        w.manager[22] = create_fake as *const () as usize;
        let d = unsafe { ZoneDeveloper::from_ptr(w.object.ptr()) };
        d.set_primary_vtable(w.primary.as_ptr() as u32);
        d.set_lot_manager_vtable(w.manager.as_ptr() as u32);
        d.set_cells_x(32);
        d.set_cells_z(32);
        let header = w.header.as_mut_ptr() as u32;
        w.header[0] = header;
        w.header[1] = header;
        for &f in orientations {
            let mut node = Box::new([header, w.header[1], f as u32]);
            let p = node.as_mut_ptr() as u32;
            unsafe { (w.header[1] as *mut u32).write(p) };
            w.header[1] = p;
            w.nodes.push(node);
        }
        w._fakes = vec![
            fake(0x0091_36B1, random_fake as *const () as usize),
            fake(0x0090_CF54, allocate_fake as *const () as usize),
            fake_data(0x00B4_B398, &mut *w.random),
        ];
        w
    }

    fn run(&mut self, rect: [i32; 4]) -> Vec<Call> {
        let mut header = self.header.as_mut_ptr().cast();
        let mut out = core::ptr::null_mut();
        let [x, z, w, h] = rect;
        let before = unsafe { core::slice::from_raw_parts(self.object.ptr(), 0xEC) }.to_vec();
        assert_eq!(
            unsafe { fc4p_0072d200(self.object.ptr(), x, z, w, h, &mut header, &mut out) },
            0xFEDC_BA98_7654_3201
        );
        assert_eq!(out as usize, 0x1234);
        assert_eq!(header, self.header.as_mut_ptr().cast());
        assert_eq!(unsafe { core::slice::from_raw_parts(self.object.ptr(), 0xEC) }, before);
        let calls = STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls));
        match calls.last().unwrap() {
            Call::Create(this, eax, edx, args) => {
                assert_eq!(*this, self.object.ptr().wrapping_add(4) as usize);
                assert_eq!(&args[..4], &[x as u32, z as u32, w as u32, h as u32]);
                assert_eq!(&args[6..], &[0, 0, 0]);
                let found = calls.iter().any(|c| {
                    matches!(c, Call::Network(_, cx, cz, _)
                    if STATE.with(|s| s.borrow().roads.contains(&(*cx, *cz))))
                });
                assert_eq!(*eax, if found { z as u32 } else { w as u32 });
                assert_eq!(*edx, if found { self.manager.as_ptr() as u32 } else { z as u32 });
            }
            _ => panic!("last call must create the lot"),
        }
        calls
    }

    fn networks(calls: &[Call]) -> Vec<(i32, i32)> {
        calls
            .iter()
            .filter_map(|c| match c {
                Call::Network(_, x, z, mask) => {
                    assert_eq!(*mask, 0xC49);
                    Some((*x, *z))
                }
                _ => None,
            })
            .collect()
    }
}

#[test]
fn empty_list_draws_once_and_keeps_four_linked_orientations() {
    for draw in 0..4 {
        let mut w = World::new(&[]);
        STATE.with(|s| s.borrow_mut().draw = draw);
        let calls = w.run([10, 11, 2, 3]);
        assert_eq!(calls[0], Call::Random(&*w.random as *const u8 as usize, 4));
        assert_eq!(
            &calls[1..5],
            &[
                Call::Allocate(12),
                Call::Allocate(12),
                Call::Allocate(12),
                Call::Allocate(12)
            ]
        );
        let header = w.header.as_ptr() as u32;
        let mut previous = header;
        let mut node = w.header[0];
        for i in 0..4 {
            let words = unsafe { &*(node as *const [u32; 3]) };
            assert_eq!(words[1], previous);
            assert_eq!(words[2], ((draw + i) % 4) as u32);
            previous = node;
            node = words[0];
        }
        assert_eq!(node, header);
        assert_eq!(w.header[1], previous);
        assert_eq!(w.header[2], 0xA5A5_A5A5);
        assert!(matches!(calls.last(), Some(Call::Create(_, _, _, a)) if a[4] == draw as u32));
        assert_eq!(World::networks(&calls).len(), 10);
    }
}

#[test]
fn each_side_scans_ascending_and_stops_at_the_first_road() {
    for (facing, road, expected) in [
        (0, (11, 10), vec![(10, 10), (11, 10)]),
        (1, (12, 12), vec![(12, 11), (12, 12)]),
        (2, (11, 14), vec![(10, 14), (11, 14)]),
        (3, (9, 12), vec![(9, 11), (9, 12)]),
    ] {
        let mut w = World::new(&[99, facing, (facing + 1) % 4]);
        STATE.with(|s| s.borrow_mut().roads.push(road));
        let calls = w.run([10, 11, 2, 3]);
        assert_eq!(World::networks(&calls), expected);
        assert!(matches!(calls.last(), Some(Call::Create(_, _, _, a)) if a[4] == facing as u32));
        assert_eq!(calls.len(), expected.len() + 1);
    }
}

#[test]
fn no_roads_falls_back_to_the_list_front_including_invalid_choices() {
    let mut w = World::new(&[-1, 2, 0]);
    let calls = w.run([10, 11, 2, 3]);
    assert_eq!(World::networks(&calls), [(10, 14), (11, 14), (10, 10), (11, 10)]);
    assert!(matches!(calls.last(), Some(Call::Create(_, _, _, a)) if a[4] == u32::MAX));
}

#[test]
fn city_edges_and_empty_or_wrapped_spans_skip_network_queries() {
    for (facing, rect) in [
        (0, [2, 0, 1, 1]),
        (3, [0, 2, 1, 1]),
        (1, [31, 2, 1, 1]),
        (2, [2, 31, 1, 1]),
        (0, [2, 2, 0, 1]),
        (1, [2, 2, 1, 0]),
        (2, [i32::MAX, 2, 2, 1]),
        (3, [2, i32::MAX, 1, 2]),
    ] {
        let mut w = World::new(&[facing]);
        let calls = w.run(rect);
        assert!(
            World::networks(&calls).is_empty(),
            "facing {facing}, rectangle {rect:?}"
        );
        assert_eq!(calls.len(), 1);
    }
}

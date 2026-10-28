// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native-layout versions of the core's connection deletion and marker cleanup cases.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    City,
    Symmetric(u32, u32),
    Cell,
    Width,
    Framework,
    Service,
    ReleaseLocal,
    AllocateNode(u32),
    Props([i32; 4]),
    Holder(u32),
    Marker(u32),
    Remove(u32),
    Networks,
    AllocateMessage(u32),
    Construct,
    MessageRef,
    Id(u32),
    Kind,
    Data(u32),
    X(i32),
    Z(i32),
    Post(u32),
    MessageRelease,
    ConnectionRef,
    Erase,
    ConnectionRelease,
    Clear,
    Free,
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    connection: usize,
    city: usize,
    allocator: usize,
    message: usize,
    cache: usize,
    sim: usize,
    props: Vec<(usize, u8)>,
    invalidate: bool,
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 3]>>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn city(_: *mut u8) -> u32 {
    record(Call::City);
    STATE.with(|s| s.borrow().city as u32)
}
extern "thiscall" fn symmetric(_: *mut u8, connection: u32, add: u32) {
    record(Call::Symmetric(connection, add));
}
extern "thiscall" fn cell(this: *mut u8, x: *mut i32, z: *mut i32) {
    record(Call::Cell);
    unsafe {
        x.write_unaligned(word(this, 4) as i32);
        z.write_unaligned(word(this, 8) as i32);
    }
}
extern "thiscall" fn width(_: *mut u8) -> u32 {
    record(Call::Width);
    64
}
extern "C" fn framework() -> u32 {
    record(Call::Framework);
    STATE.with(|s| s.borrow().allocator as u32)
}
extern "thiscall" fn service(_: *mut u8, a: u32, b: u32, out: *mut u32) {
    assert_eq!((a, b), (0x3AE4_BEA3, 0x3AE4_BEAB));
    record(Call::Service);
    unsafe {
        assert_eq!(out.read_unaligned(), 0);
        out.write_unaligned(STATE.with(|s| s.borrow().allocator as u32));
    }
}
extern "fastcall" fn release_local(local: *mut u32, edx: u32) {
    record(Call::ReleaseLocal);
    unsafe {
        assert_eq!(edx, local.read_unaligned());
        assert_eq!((STATE.with(|s| s.borrow().cache) as *const u32).read(), edx);
        local.write_unaligned(0);
    }
}
extern "thiscall" fn allocate_node(_: *mut u8, size: u32) -> u32 {
    record(Call::AllocateNode(size));
    assert_eq!(size, 12);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.nodes.push(Box::new([0xA5A5_A5A5; 3]));
        s.nodes.last_mut().unwrap().as_mut_ptr() as u32
    })
}
extern "thiscall" fn props(_: *mut u8, list: *mut *mut u8, rectangle: *const [i32; 4]) {
    record(Call::Props(unsafe { rectangle.read_unaligned() }));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let header = unsafe { list.read_unaligned() }.cast::<u32>();
        assert_eq!(unsafe { header.read() }, header as u32);
        assert_eq!(unsafe { header.add(1).read() }, header as u32);
        assert_eq!(unsafe { header.add(2).read() }, 0xA5A5_A5A5);
        let occupants = s.props.clone();
        for (occupant, _) in occupants {
            let mut node = Box::new([header as u32, unsafe { header.add(1).read() }, occupant as u32]);
            let p = node.as_mut_ptr() as u32;
            unsafe {
                (node[1] as *mut u32).write(p);
                header.add(1).write(p);
            }
            s.nodes.push(node);
        }
    });
}
extern "thiscall" fn holder(this: *mut u8) -> u32 {
    record(Call::Holder(this as u32));
    this as u32
}
extern "C" fn marker(holder: *mut u8, id: u32, out: *mut u8) {
    assert_eq!(id, 0xCAA4_6595);
    unsafe {
        assert_eq!(out.read(), 0);
    }
    record(Call::Marker(holder as u32));
    let value = STATE.with(|s| s.borrow().props.iter().find(|p| p.0 == holder as usize).unwrap().1);
    unsafe {
        out.write(value);
    }
}
extern "thiscall" fn remove(_: *mut u8, occupant: u32) {
    record(Call::Remove(occupant));
}
extern "thiscall" fn networks(_: *mut u8, _: *mut *mut u8) {
    record(Call::Networks);
}
extern "C" fn allocate_message(size: u32) -> u32 {
    record(Call::AllocateMessage(size));
    STATE.with(|s| s.borrow().message as u32)
}
extern "fastcall" fn construct_message(this: *mut u8, _: u32) -> u32 {
    record(Call::Construct);
    assert_eq!(this as usize, STATE.with(|s| s.borrow().message));
    this as u32
}
extern "thiscall" fn message_ref(_: *mut u8) {
    record(Call::MessageRef);
}
extern "thiscall" fn id(_: *mut u8, id: u32) {
    record(Call::Id(id));
}
extern "thiscall" fn kind(_: *mut u8) -> u32 {
    record(Call::Kind);
    11
}
extern "thiscall" fn data(_: *mut u8, value: u32) {
    record(Call::Data(value));
}
extern "thiscall" fn x(_: *mut u8, value: i32) {
    record(Call::X(value));
}
extern "thiscall" fn z(_: *mut u8, value: i32) {
    record(Call::Z(value));
}
extern "thiscall" fn post(_: *mut u8, message: u32, immediate: u32) {
    assert_eq!(message as usize, STATE.with(|s| s.borrow().message));
    record(Call::Post(immediate));
}
extern "thiscall" fn message_release(_: *mut u8) {
    record(Call::MessageRelease);
}
extern "thiscall" fn connection_ref(_: *mut u8) {
    record(Call::ConnectionRef);
}
extern "fastcall" fn erase(list: *mut u8, _: u32, reference: *mut u32) {
    record(Call::Erase);
    assert_eq!(list as usize, STATE.with(|s| s.borrow().sim) + 0x1C);
    assert_eq!(
        unsafe { reference.read_unaligned() } as usize,
        STATE.with(|s| s.borrow().connection)
    );
}
extern "thiscall" fn connection_release(_: *mut u8) {
    record(Call::ConnectionRelease);
}
extern "fastcall" fn clear(list: *mut *mut u8, _: u32) {
    record(Call::Clear);
    unsafe {
        let h = list.read_unaligned().cast::<u32>();
        h.write(h as u32);
        h.add(1).write(h as u32);
    }
    STATE.with(|s| {
        let s = s.borrow();
        if s.invalidate {
            unsafe {
                (s.cache as *mut u32).write(0);
            }
        }
    });
}
extern "thiscall" fn free_body(_: *mut u8, header: *mut u32) -> u64 {
    record(Call::Free);
    assert_eq!(unsafe { header.read() }, header as u32);
    0x1234_5678_ABC0_DEF0
}
#[unsafe(naked)]
extern "thiscall" fn free_node() {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {}", "mov ecx, 0x98765432", "ret 4", sym free_body);
}

// Tables and global stand-ins keep their addresses when their ownership vectors grow.
#[allow(clippy::vec_box)]
struct World {
    sim: Object,
    connection: Object,
    _objects: Vec<Object>,
    _tables: Vec<Box<[usize; 100]>>,
    _globals: Vec<Box<usize>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(cell_value: [i32; 2], symmetric_city: bool, cached: bool, invalidate: bool) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            sim: Object::new(0x48),
            connection: Object::new(12),
            _objects: vec![],
            _tables: vec![],
            _globals: vec![],
            _fakes: vec![],
        };
        let mut table = Box::new([0; 100]);
        table[0x14 / 4] = kind as *const () as usize;
        table[0x18 / 4] = cell as *const () as usize;
        table[0x24 / 4] = city as *const () as usize;
        table[1] = connection_ref as *const () as usize;
        table[2] = connection_release as *const () as usize;
        unsafe {
            w.connection.ptr().cast::<usize>().write(table.as_ptr() as usize);
            w.connection.ptr().add(4).cast::<i32>().write(cell_value[0]);
            w.connection.ptr().add(8).cast::<i32>().write(cell_value[1]);
        }
        w._tables.push(table);
        let city = w.object(&[
            (0x58, symmetric as *const () as usize),
            (0x174, width as *const () as usize),
        ]);
        unsafe { NeighborsSim::from_ptr(w.sim.ptr()) }.set_city(city);
        let allocator = w.object(&[
            (0x0C, allocate_node as *const () as usize),
            (0x10, free_node as *const () as usize),
            (0x14, service as *const () as usize),
        ]);
        let props = w.object(&[
            (0x48, props as *const () as usize),
            (0x74, remove as *const () as usize),
        ]);
        let networks = w.object(&[(0xC0, networks as *const () as usize)]);
        let message = w.object(&[
            (4, message_ref as *const () as usize),
            (8, message_release as *const () as usize),
            (0x14, id as *const () as usize),
            (0x2C, data as *const () as usize),
            (0x40, x as *const () as usize),
            (0x54, z as *const () as usize),
        ]);
        let server = w.object(&[(0x10, post as *const () as usize)]);
        for (address, value) in [
            (0x00B2_07E4, if cached { allocator as usize } else { 0 }),
            (0x00B4_3D10, props as usize),
            (0x00B4_3D0C, networks as usize),
            (0x00B4_3CCC, server as usize),
        ] {
            let mut global = Box::new(value);
            w._fakes.push(fake_data(address, (&mut *global as *mut usize).cast()));
            w._globals.push(global);
        }
        for (address, function) in [
            (0x005F_D390, marker as *const () as usize),
            (0x0091_33DA, allocate_message as *const () as usize),
            (0x0091_34D6, construct_message as *const () as usize),
            (0x006C_FD30, erase as *const () as usize),
            (0x0041_3E60, clear as *const () as usize),
            (0x0087_93EC, framework as *const () as usize),
            (0x0041_1870, release_local as *const () as usize),
        ] {
            w._fakes.push(fake(address, function));
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.connection = w.connection.ptr() as usize;
            s.sim = w.sim.ptr() as usize;
            s.city = if symmetric_city { city as usize } else { 0 };
            s.allocator = allocator as usize;
            s.message = message as usize;
            s.cache = &mut *w._globals[0] as *mut usize as usize;
            s.invalidate = invalidate;
        });
        w
    }
    fn object(&mut self, slots: &[(usize, usize)]) -> *mut u8 {
        let mut table = Box::new([0; 100]);
        for &(slot, function) in slots {
            table[slot / 4] = function;
        }
        let mut object = Object::new(4);
        let p = object.ptr();
        unsafe {
            p.cast::<usize>().write(table.as_ptr() as usize);
        }
        self._tables.push(table);
        self._objects.push(object);
        p
    }
    fn run(&mut self, remove: u32) -> u64 {
        let mut args = [self.connection.ptr() as u32, remove];
        unsafe {
            crate::neighbors::tests::checked_call(
                fc4p_006d17b0 as *const () as usize,
                self.sim.ptr(),
                &mut args,
                0xFACE_CAFE,
                0x1122_3344,
                crate::abi::Conv::Thiscall,
            )
        }
    }
}

#[test]
fn deletion_cleans_the_inward_cell_but_posts_the_original_coordinates() {
    for (cell, inward, width_call) in [
        ([-1, 5], [0, 5], false),
        ([64, 5], [63, 5], true),
        ([5, -1], [5, 0], false),
        ([5, 64], [5, 63], true),
        ([-1, -1], [0, -1], false),
        ([4, 4], [4, 3], true),
        ([i32::MIN, 0], [i32::MIN + 1, 0], false),
        ([i32::MAX, 0], [i32::MAX - 1, 0], true),
    ] {
        let mut w = World::new(cell, true, true, false);
        let before = unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0x48) }.to_vec();
        assert_eq!(w.run(0), 0x1234_5678_ABC0_DEF0);
        let mut expected = vec![Call::City, Call::Symmetric(w.connection.ptr() as u32, 0), Call::Cell];
        if width_call {
            expected.push(Call::Width);
        }
        expected.extend([
            Call::AllocateNode(12),
            Call::Props([inward[0], inward[1], inward[0], inward[1]]),
            Call::Networks,
            Call::AllocateMessage(0x2C),
            Call::Construct,
            Call::MessageRef,
            Call::Id(0xEA0D_163E),
            Call::Kind,
            Call::Data(11),
            Call::X(cell[0]),
            Call::Z(cell[1]),
            Call::Post(0),
            Call::MessageRelease,
            Call::Clear,
            Call::Free,
        ]);
        assert_eq!(take(), expected);
        assert_eq!(unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0x48) }, before);
    }
}

#[test]
fn only_marker_props_are_removed_and_erasing_uses_the_low_flag_byte() {
    for remove_word in [0, 0x100, 1, 0x80, u32::MAX] {
        let mut w = World::new([-1, 5], false, true, false);
        let occupants: Vec<_> = (0..3)
            .map(|_| w.object(&[(0x18, holder as *const () as usize)]) as usize)
            .collect();
        STATE.with(|s| s.borrow_mut().props = vec![(occupants[0], 0), (occupants[1], 0x80), (occupants[2], 1)]);
        assert_eq!(w.run(remove_word), 0x1234_5678_ABC0_DEF0);
        let calls = take();
        assert_eq!(
            &calls[4..12],
            &[
                Call::Holder(occupants[0] as u32),
                Call::Marker(occupants[0] as u32),
                Call::Holder(occupants[1] as u32),
                Call::Marker(occupants[1] as u32),
                Call::Remove(occupants[1] as u32),
                Call::Holder(occupants[2] as u32),
                Call::Marker(occupants[2] as u32),
                Call::Remove(occupants[2] as u32)
            ]
        );
        let erases = calls
            .iter()
            .filter(|c| matches!(c, Call::ConnectionRef | Call::Erase | Call::ConnectionRelease))
            .collect::<Vec<_>>();
        assert_eq!(
            erases,
            if remove_word as u8 == 0 {
                vec![]
            } else {
                vec![&Call::ConnectionRef, &Call::Erase, &Call::ConnectionRelease]
            }
        );
    }
}

#[test]
fn allocator_is_looked_up_on_first_use_and_again_if_cleanup_invalidates_it() {
    let mut w = World::new([-1, 5], false, false, true);
    assert_eq!(w.run(1), 0x1234_5678_ABC0_DEF0);
    let calls = take();
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Framework)).count(), 2);
    assert_eq!(&calls[2..5], &[Call::Framework, Call::Service, Call::ReleaseLocal]);
    assert_eq!(
        &calls[calls.len() - 4..],
        &[Call::Framework, Call::Service, Call::ReleaseLocal, Call::Free]
    );
}

#[test]
fn final_allocator_ecx_is_returned_to_the_caller() {
    let mut w = World::new([-1, 5], false, true, false);
    let args = [w.connection.ptr() as u32, 0];
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Thiscall,
        fc4p_006d17b0 as *const () as usize,
        w.sim.ptr() as u32,
        &args,
    );
    unsafe {
        core::arch::asm!(
            "mov eax, [edi + 16]", "push dword ptr [eax + 4]", "push dword ptr [eax]",
            "mov ecx, [edi + 8]", "call dword ptr [edi]", "mov [edi + 8], ecx",
            in("edi") &mut call, clobber_abi("C"),
        );
    }
    assert_eq!(call.ecx, 0x9876_5432);
}

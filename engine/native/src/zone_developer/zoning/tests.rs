// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Core zoning workflow cases with packed regions and observed game calls.

use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Node(u32),
    FreeNode(u32),
    AllocateRegion,
    FreeRegion(u32),
    Construct(u32, u32),
    DestroyBits(u32),
    Examine(u32),
    Face([i32; 4], Vec<u32>),
    DestroyList(Vec<u32>),
    AllocateMessage,
    ConstructMessage,
    AddRef,
    MessageType(u32),
    MessageValue(u32),
    Post,
    Release,
    Parcellizable(u32),
    Compress(u32),
    LotSize(u32),
    Extend(u32, u32),
    Jog(u32, u32),
    Subdivide(u32),
    Parcellize,
    Simple(u32),
    CellsX,
    CellsZ,
    Extract(u32, u32, u32),
    Update(u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    developer: u32,
    message: u32,
    cropped: u32,
    components: Vec<u32>,
    face_result: u32,
    fail_region: bool,
    extracted: u32,
    compress_to: u32,
    examined_zone: Option<u32>,
    point: [i32; 2],
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn words(s: &mut State, count: usize, fill: u32) -> *mut u32 {
    let mut allocation = vec![fill; count.max(1)].into_boxed_slice();
    let pointer = allocation.as_mut_ptr();
    s.allocations.push(allocation);
    pointer
}

unsafe fn initialize(s: &mut State, pointer: *mut u8, bounds: [i32; 4], set: &[(i32, i32)]) {
    unsafe {
        let region = Region::from_ptr(pointer);
        region.set_x1(bounds[0]);
        region.set_z1(bounds[1]);
        region.set_x2(bounds[2]);
        region.set_z2(bounds[3]);
        let (width, height) = (
            bounds[2].wrapping_sub(bounds[0]).wrapping_add(1),
            bounds[3].wrapping_sub(bounds[1]).wrapping_add(1),
        );
        let count = (height.max(0) as u32).div_ceil(32);
        let columns = words(s, width.max(0) as usize, 0).cast::<*mut u32>();
        for x in 0..width.max(0) as usize {
            columns.add(x).write(words(s, count as usize, 0));
        }
        region.set_vtable(0xAABB_CCDD);
        region.set_width(width);
        region.set_height(height);
        region.set_words(count);
        region.set_columns(columns);
        for &(x, z) in set {
            region.set(x.wrapping_sub(bounds[0]) as u32, z.wrapping_sub(bounds[1]) as u32, true);
        }
    }
}

fn region(bounds: [i32; 4], set: &[(i32, i32)]) -> *mut u8 {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        let pointer = words(&mut s, 9, 0xA5A5_A5A5).cast();
        unsafe {
            initialize(&mut s, pointer, bounds, set);
        }
        pointer
    })
}

fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn verify_this(this: *mut u8) {
    STATE.with(|s| assert_eq!(this as u32, s.borrow().developer));
}

extern "C" fn allocate_node_fake(size: u32) -> *mut u8 {
    assert_eq!(size, 12);
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        let p = words(&mut s, 3, 0xA5A5_A5A5).cast();
        s.calls.push(Call::Node(p as u32));
        p
    })
}

extern "C" fn free_node_fake(node: *mut u8) -> u64 {
    log(Call::FreeNode(node as u32));
    0xABCD_1234_5678_9ABC
}

extern "C" fn allocate_region_fake(size: u32) -> *mut u8 {
    assert_eq!(size, 0x24);
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.calls.push(Call::AllocateRegion);
        if s.fail_region {
            null_mut()
        } else {
            words(&mut s, 9, 0xA5A5_A5A5).cast()
        }
    })
}

extern "C" fn free_region_fake(pointer: *mut u8) {
    log(Call::FreeRegion(pointer as u32));
}

extern "thiscall" fn construct_region_fake(this: *mut u8, source: *mut u8, fill: u32) -> *mut u8 {
    assert_eq!(fill, 0);
    log(Call::Construct(this as u32, source as u32));
    unsafe {
        let source = Region::from_ptr(source);
        STATE.with(|state| {
            initialize(
                &mut state.borrow_mut(),
                this,
                [source.x1(), source.z1(), source.x2(), source.z2()],
                &[],
            )
        });
    }
    this
}

extern "thiscall" fn destroy_bits_fake(this: *mut u8) {
    unsafe {
        assert_eq!(Region::from_ptr(this.sub(16)).vtable(), 0x00A8_6364);
    }
    log(Call::DestroyBits(unsafe { this.sub(16) } as u32));
}

unsafe fn list_values(header: *mut u8) -> Vec<u32> {
    unsafe {
        let mut result = vec![];
        let mut p = read_unaligned(header.cast::<*mut u8>());
        let mut previous = header;
        while p != header {
            assert_eq!(read_unaligned(p.add(4).cast::<*mut u8>()), previous);
            result.push(read_unaligned(p.add(8).cast()));
            previous = p;
            p = read_unaligned(p.cast());
            assert!(result.len() < 20);
        }
        assert_eq!(read_unaligned(header.add(4).cast::<*mut u8>()), previous);
        result
    }
}

extern "thiscall" fn destroy_list_fake(list: *mut *mut u8) -> u64 {
    log(Call::DestroyList(unsafe { list_values(read_unaligned(list)) }));
    0x1020_3040_5060_7080
}

extern "thiscall" fn examine_fake(this: *mut u8) {
    verify_this(this);
    unsafe {
        let dev = ZoneDeveloper::from_ptr(this);
        log(Call::Examine(dev.restriction() as u32));
        STATE.with(|s| {
            if let Some(zone) = s.borrow().examined_zone {
                dev.set_current_zone(zone);
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
extern "thiscall" fn face_fake(
    this: *mut u8,
    x: i32,
    z: i32,
    width: i32,
    height: i32,
    list: *mut *mut u8,
    lot: *mut *mut u8,
) -> u32 {
    verify_this(this);
    unsafe {
        assert!(ZoneDeveloper::from_ptr(this).restriction().is_null());
        assert!(lot.read().is_null());
        // The lot destination is scratch; OnZone does not release its value.
        lot.write(0x8765_4321usize as *mut u8);
        log(Call::Face([x, z, width, height], list_values(list.read())));
    }
    STATE.with(|s| s.borrow().face_result)
}

extern "C" fn allocate_message_fake(size: u32) -> *mut u8 {
    assert_eq!(size, 0x2C);
    log(Call::AllocateMessage);
    STATE.with(|s| s.borrow().message as *mut u8)
}
extern "thiscall" fn construct_message_fake(this: *mut u8) -> *mut u8 {
    log(Call::ConstructMessage);
    this
}
extern "thiscall" fn add_ref(_this: *mut u8) {
    log(Call::AddRef);
}
extern "thiscall" fn message_type(_this: *mut u8, value: u32) {
    log(Call::MessageType(value));
}
extern "thiscall" fn message_value(_this: *mut u8, value: u32) {
    log(Call::MessageValue(value));
}
extern "thiscall" fn post(_this: *mut u8, message: *mut u8, flag: u32) {
    assert_eq!(flag, 0);
    STATE.with(|s| assert_eq!(message as u32, s.borrow().message));
    log(Call::Post);
}
extern "thiscall" fn release(_this: *mut u8) {
    log(Call::Release);
}
extern "thiscall" fn parcellizable_fake(this: *mut u8, source: *mut u8) -> *mut u8 {
    verify_this(this);
    log(Call::Parcellizable(source as u32));
    STATE.with(|s| s.borrow().cropped as *mut u8)
}
extern "stdcall" fn compress_fake(slot: *mut *mut u8) {
    log(Call::Compress(unsafe { slot.read() } as u32));
    STATE.with(|s| {
        if s.borrow().compress_to != 0 {
            unsafe {
                slot.write(s.borrow().compress_to as *mut u8);
            }
        }
    });
}
extern "thiscall" fn lot_size_fake(this: *mut u8, region: *mut u8) {
    verify_this(this);
    log(Call::LotSize(region as u32));
}
extern "thiscall" fn extend_fake(this: *mut u8, region: *mut u8, kind: u32) {
    verify_this(this);
    log(Call::Extend(region as u32, kind));
}
extern "thiscall" fn jog_fake(this: *mut u8, region: *mut u8, kind: u32) {
    verify_this(this);
    log(Call::Jog(region as u32, kind));
}
extern "thiscall" fn subdivide_fake(this: *mut u8, region: *mut u8, list: *mut *mut u8) {
    verify_this(this);
    log(Call::Subdivide(region as u32));
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        unsafe {
            let header = list.read();
            assert!(list_values(header).is_empty());
            for component in s.components.clone() {
                let node = words(&mut s, 3, 0xA5A5_A5A5).cast::<u8>();
                let previous = read_unaligned(header.add(4).cast::<*mut u8>());
                write_unaligned(node.cast::<*mut u8>(), header);
                write_unaligned(node.add(4).cast::<*mut u8>(), previous);
                write_unaligned(node.add(8).cast::<u32>(), component);
                write_unaligned(previous.cast::<*mut u8>(), node);
                write_unaligned(header.add(4).cast::<*mut u8>(), node);
            }
        }
    });
}
extern "thiscall" fn parcellize_fake(this: *mut u8) {
    verify_this(this);
    log(Call::Parcellize);
}
extern "thiscall" fn simple_fake(this: *mut u8, depth: u32) {
    verify_this(this);
    log(Call::Simple(depth));
}
extern "thiscall" fn cells_x(_this: *mut u8) -> i32 {
    log(Call::CellsX);
    64
}
extern "thiscall" fn cells_z(_this: *mut u8) -> i32 {
    log(Call::CellsZ);
    64
}
extern "stdcall" fn extract_fake(region: *mut u8, x: u32, z: u32) -> *mut u8 {
    log(Call::Extract(region as u32, x, z));
    STATE.with(|s| s.borrow().extracted as *mut u8)
}
extern "thiscall" fn update_fake(this: *mut u8, source: *mut u8) {
    verify_this(this);
    log(Call::Update(source as u32));
}

struct World {
    memory: Object,
    source: *mut u8,
    _table: Box<[usize; 96]>,
    _objects: Box<[u32; 3]>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                face_result: 1,
                ..State::default()
            }
        });
        let mut table = Box::new([0usize; 96]);
        for (offset, function) in [
            (4, add_ref as *const () as usize),
            (8, release as *const () as usize),
            (0x14, message_type as *const () as usize),
            (0x2C, message_value as *const () as usize),
            (0x10, post as *const () as usize),
            (0x174, cells_x as *const () as usize),
            (0x178, cells_z as *const () as usize),
        ] {
            table[offset / 4] = function;
        }
        let mut objects = Box::new([table.as_ptr() as u32; 3]);
        let mut memory = Object::new(0x100);
        let primary = memory.ptr();
        let message = objects.as_mut_ptr().cast::<u8>();
        unsafe {
            let dev = ZoneDeveloper::from_ptr(primary);
            dev.set_message_server(message.add(4));
            dev.set_city(message.add(8));
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.developer = primary as u32;
            s.message = message as u32;
        });
        let source = region([10, 10, 12, 12], &[(10, 10), (10, 11), (11, 10), (12, 12)]);
        let cropped = region([10, 10, 12, 12], &[(10, 10), (10, 11), (11, 10)]);
        STATE.with(|s| s.borrow_mut().cropped = cropped as u32);
        let mut fakes = vec![];
        for (address, function) in [
            (0x0090_CF54, allocate_node_fake as *const () as usize),
            (0x0090_CF63, free_node_fake as *const () as usize),
            (0x005E_55E0, allocate_region_fake as *const () as usize),
            (0x005E_5620, free_region_fake as *const () as usize),
            (0x0044_2640, construct_region_fake as *const () as usize),
            (0x0044_1A60, destroy_bits_fake as *const () as usize),
            (0x0056_50A0, destroy_list_fake as *const () as usize),
            (0x0072_D200, face_fake as *const () as usize),
            (0x0073_1C80, examine_fake as *const () as usize),
            (0x0091_33DA, allocate_message_fake as *const () as usize),
            (0x0091_34D6, construct_message_fake as *const () as usize),
            (0x0072_CB90, parcellizable_fake as *const () as usize),
            (0x0072_AF30, compress_fake as *const () as usize),
            (0x0073_2BF0, lot_size_fake as *const () as usize),
            (0x0073_0BA0, extend_fake as *const () as usize),
            (0x0073_0D30, jog_fake as *const () as usize),
            (0x0072_EC60, subdivide_fake as *const () as usize),
            (0x0072_EDD0, parcellize_fake as *const () as usize),
            (0x0072_E9B0, simple_fake as *const () as usize),
            (0x0072_CDA0, extract_fake as *const () as usize),
            (0x0072_B180, update_fake as *const () as usize),
        ] {
            fakes.push(fake(address, function));
        }
        Self {
            memory,
            source,
            _table: table,
            _objects: objects,
            _fakes: fakes,
        }
    }
    fn dev(&mut self) -> ZoneDeveloper {
        unsafe { ZoneDeveloper::from_ptr(self.memory.ptr()) }
    }
    fn selection(&mut self) -> Selection {
        unsafe { Selection::from_ptr(self.memory.ptr()) }
    }
    fn point(&mut self, x: i32, z: i32) {
        let pointer = STATE.with(|s| {
            s.borrow_mut().point = [x, z];
            s.as_ptr().cast::<State>()
        });
        self.selection().set_point(unsafe { (*pointer).point.as_ptr() });
    }
    fn components(&mut self, regions: &[*mut u8]) {
        STATE.with(|s| s.borrow_mut().components = regions.iter().map(|&p| p as u32).collect());
    }
    fn call(&mut self, output: *mut *mut u8) -> u64 {
        let args = [self.source as u32, output as u32];
        let mut c = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            fc4p_00733770 as *const () as usize,
            self.memory.ptr() as u32,
            &args,
        );
        unsafe {
            crate::abi::call_raw(&mut c);
        }
        assert!(self.dev().restriction().is_null());
        (u64::from(c.edx) << 32) | u64::from(c.eax)
    }
}

fn message(value: u32) -> Vec<Call> {
    vec![
        Call::AllocateMessage,
        Call::ConstructMessage,
        Call::AddRef,
        Call::MessageType(0x49D9_68C4),
        Call::MessageValue(value),
        Call::Post,
        Call::Release,
    ]
}

#[test]
fn custom_size_attempt_uses_four_orientations_and_only_writes_output_on_failure() {
    for success in [false, true] {
        for provide_out in [false, true] {
            for allocation_failure in [false, true] {
                let mut world = World::new();
                world.selection().set_custom_size(0x80);
                world.dev().set_orientation_d(0xFFFF_FFFF);
                world.dev().set_orientation_q(2);
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.face_result = if success { 0xFACE_0080 } else { 0xFACE_0000 };
                    s.fail_region = allocation_failure;
                });
                let mut output = 0x1234_5678usize as *mut u8;
                assert_eq!(
                    world.call(if provide_out { &mut output } else { null_mut() }),
                    0x1020_3040_5060_7080
                );
                let calls = take();
                assert_eq!(calls[0], Call::Examine(world.source as u32));
                assert_eq!(calls.iter().filter(|call| matches!(call, Call::Node(..))).count(), 5);
                assert!(calls.contains(&Call::Face([10, 10, 3, 3], vec![u32::MAX, 2, 1, 0])));
                assert_eq!(calls.last(), Some(&Call::DestroyList(vec![u32::MAX, 2, 1, 0])));
                if provide_out && !success {
                    if allocation_failure {
                        assert!(output.is_null());
                    } else {
                        unsafe {
                            let r = Region::from_ptr(output);
                            for x in 0..3 {
                                for z in 0..3 {
                                    assert!(!r.get(x, z));
                                }
                            }
                        }
                    }
                } else {
                    assert_eq!(output as u32, 0x1234_5678);
                }
            }
        }
    }
}

#[test]
fn custom_size_keeps_the_callers_orientation_list_argument_word() {
    let mut world = World::new();
    world.selection().set_custom_size(1);
    let mut frame = [0u32, world.source as u32, 0];
    assert_eq!(
        unsafe { on_zone(world.memory.ptr(), frame.as_mut_ptr()) },
        0x1020_3040_5060_7080
    );
    let calls = take();
    let Some(Call::Node(header)) = calls.get(1) else {
        panic!("{calls:?}");
    };
    assert_eq!(frame[1], *header);
}

#[test]
fn normal_empty_component_list_has_exact_street_pass_and_cleanup_order() {
    for (width, height) in [(3, 3), (2, 3), (3, 2)] {
        for streets in [false, true] {
            for real in [false, true] {
                let mut world = World::new();
                world.dev().set_place_streets(u8::from(streets));
                world.dev().set_real_mode(u8::from(real));
                let cropped = region([1, 2, width, height + 1], &[]);
                STATE.with(|s| s.borrow_mut().cropped = cropped as u32);
                assert_eq!(world.call(null_mut()), 0xABCD_1234_5678_9ABC);
                let calls = take();
                let mut expected = message(1);
                let header = match calls[7] {
                    Call::Node(p) => p,
                    _ => panic!("{calls:?}"),
                };
                expected.extend([
                    Call::Node(header),
                    Call::Parcellizable(world.source as u32),
                    Call::Compress(cropped as u32),
                    Call::LotSize(cropped as u32),
                ]);
                if streets {
                    expected.extend([11, 3, 10, 0, 6].map(|kind| Call::Extend(cropped as u32, kind)));
                    if width > 2 && height > 2 {
                        expected.extend([11, 3, 10, 0, 6].map(|kind| Call::Jog(cropped as u32, kind)));
                    }
                }
                expected.extend([
                    Call::Subdivide(cropped as u32),
                    Call::DestroyBits(cropped as u32),
                    Call::FreeRegion(cropped as u32),
                ]);
                if real {
                    expected.push(Call::Update(world.source as u32));
                }
                expected.extend(message(0));
                expected.push(Call::FreeNode(header));
                assert_eq!(calls, expected);
            }
        }
    }
}

#[test]
fn components_choose_parcellizer_after_examination_and_free_columns_without_a_destructor_call() {
    for zone in [0, 1, 6, 7, 8, u32::MAX] {
        let mut world = World::new();
        let component = region([10, 10, 11, 11], &[(10, 10)]);
        world.components(&[component]);
        world.dev().set_current_zone(zone);
        STATE.with(|s| s.borrow_mut().examined_zone = Some(zone));
        assert_eq!(world.call(null_mut()), 0xABCD_1234_5678_9ABC);
        let calls = take();
        let index = calls
            .iter()
            .position(|call| *call == Call::Examine(component as u32))
            .unwrap();
        assert_eq!(
            calls[index + 1],
            match zone {
                1..=6 => Call::Parcellize,
                7 => Call::Simple(16),
                _ => Call::Simple(6),
            }
        );
        unsafe {
            let r = Region::from_ptr(component);
            assert_eq!(r.vtable(), 0x00A8_6364);
            let cleanup = [
                Call::FreeRegion(r.columns().read() as u32),
                Call::FreeRegion(r.columns().add(1).read() as u32),
                Call::FreeRegion(r.columns() as u32),
                Call::FreeRegion(component as u32),
            ];
            assert_eq!(&calls[index + 2..index + 6], cleanup);
        }
        assert!(!calls.contains(&Call::DestroyBits(component as u32)));
    }
}

#[test]
fn preview_selects_center_or_neighbors_queries_dimensions_and_extracts_the_remaining_output() {
    for (point, expected_dimensions) in [
        ((10, 10), vec![]),
        ((9, 10), vec![Call::CellsX, Call::CellsZ]),
        ((11, 10), vec![Call::CellsX, Call::CellsZ]),
        ((10, 9), vec![Call::CellsX, Call::CellsZ]),
        ((10, 11), vec![Call::CellsX, Call::CellsZ]),
    ] {
        let mut world = World::new();
        world.point(point.0, point.1);
        world.source = region([9, 9, 12, 12], &[(10, 10), (10, 11), (11, 10)]);
        let component = region([10, 10, 10, 10], &[(10, 10)]);
        world.components(&[component]);
        let extracted = region([10, 10, 10, 10], &[(10, 10)]);
        STATE.with(|s| s.borrow_mut().extracted = extracted as u32);
        world.dev().set_current_zone(1);
        let mut output = null_mut();
        assert_eq!(world.call(&mut output), 0xABCD_1234_5678_9ABC);
        let calls = take();
        assert!(calls.contains(&Call::Parcellize));
        assert_eq!(
            calls
                .iter()
                .filter(|call| matches!(call, Call::CellsX | Call::CellsZ))
                .cloned()
                .collect::<Vec<_>>(),
            expected_dimensions
        );
        if point == (10, 10) || point == (10, 11) || point == (11, 10) {
            assert_eq!(output, extracted);
            assert!(calls.iter().any(|call| matches!(call, Call::Extract(..))));
            assert!(calls.contains(&Call::Compress(extracted as u32)));
        } else {
            assert!(output.is_null());
        }
    }
}

#[test]
fn unselected_components_clear_only_their_set_cells_from_a_copied_preview() {
    let mut world = World::new();
    world.point(12, 12);
    let component = region([10, 10, 11, 11], &[(10, 10)]);
    world.components(&[component]);
    // Extract returns the same output to allow inspecting the mask copied before selection.
    STATE.with(|s| s.borrow_mut().extracted = 0);
    let mut output = null_mut();
    world.call(&mut output);
    let calls = take();
    assert!(!calls.contains(&Call::Parcellize));
    assert!(!calls.iter().any(|call| matches!(call, Call::Examine(..))));
    let allocated = calls
        .iter()
        .find_map(|call| {
            if let Call::Construct(p, _) = call {
                Some(*p as *mut u8)
            } else {
                None
            }
        })
        .unwrap();
    unsafe {
        let r = Region::from_ptr(allocated);
        assert!(!r.get(0, 0));
        assert!(r.get(0, 1));
        assert!(r.get(1, 0));
        assert!(r.get(2, 2));
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::super::test_support::{bytes, float_environment, object, table};
use super::*;
use crate::testing::{fake, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Byte(u32),
    Short(u32),
    SignedShort(u32),
    Int(u32),
    UInt(u32),
    Float(u32),
    Long(u32, u32),
    Double(u64),
    Reference(u32),
    Bool(u32),
    PointerRange(u32),
    WordRange(u32),
    Bytes(u32),
    Pool(u32),
    Error,
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    error: u32,
    replace: Option<(u32, u32, u32)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn log(c: Call) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if c == Call::UInt(731) {
            if let Some((this, offset, value)) = s.replace {
                unsafe {
                    store(this as *mut u8, offset as usize, value);
                }
            }
        }
        s.calls.push(c);
    });
    0xFEED_1234
}
macro_rules! writer {
    ($fn:ident, $variant:ident) => {
        extern "thiscall" fn $fn(_this: *mut u8, value: u32) -> u32 {
            log(Call::$variant(value))
        }
    };
}
writer!(byte, Byte);
writer!(short, Short);
writer!(signed_short, SignedShort);
writer!(int, Int);
writer!(uint, UInt);
writer!(float, Float);
extern "thiscall" fn long(_this: *mut u8, low: u32, high: u32) -> u32 {
    log(Call::Long(low, high))
}
extern "thiscall" fn double(_this: *mut u8, value: f64) -> u32 {
    log(Call::Double(value.to_bits()))
}
extern "thiscall" fn error(_this: *mut u8) -> u64 {
    log(Call::Error);
    (0x1234_ABCDu64 << 32) | STATE.with(|s| u64::from(s.borrow().error))
}
extern "C" fn reference(_stream: *mut u8, object: u32) -> u32 {
    log(Call::Reference(object))
}
extern "C" fn boolean(_stream: *mut u8, value: u32) -> u32 {
    log(Call::Bool(value))
}
extern "C" fn pointer_range(_stream: *mut u8, value: u32) -> u32 {
    log(Call::PointerRange(value))
}
extern "C" fn word_range(_stream: *mut u8, value: u32) -> u32 {
    log(Call::WordRange(value))
}
extern "C" fn byte_range(_stream: *mut u8, value: *mut u8) -> u32 {
    log(Call::Bytes(value as u32))
}
extern "thiscall" fn pool(this: *mut u8, _stream: *mut u8) -> u32 {
    log(Call::Pool(this as u32))
}

struct World {
    primary: Object,
    stream: Object,
    empty_rows: Box<[u32; 3]>,
    pool_head: Object,
    ferry_head: Object,
    counts: Box<[[u32; 9]; 2]>,
    _vt: Vec<usize>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let vt = table(&[
            (0x14, byte as *const () as usize),
            (0x18, signed_short as *const () as usize),
            (0x1C, short as *const () as usize),
            (0x20, int as *const () as usize),
            (0x24, uint as *const () as usize),
            (0x28, long as *const () as usize),
            (0x30, float as *const () as usize),
            (0x34, double as *const () as usize),
            (0x48, error as *const () as usize),
        ]);
        let mut w = Self {
            primary: Object::new(0xD68),
            stream: object(4, &vt),
            empty_rows: Box::new([0; 3]),
            pool_head: Object::new(0x18),
            ferry_head: Object::new(0x34),
            counts: Box::new([[0; 9]; 2]),
            _vt: vt,
            _fakes: [
                (0x005D_99A0, reference as *const () as usize),
                (0x005D_9FD0, boolean as *const () as usize),
                (0x005D_A480, pointer_range as *const () as usize),
                (0x005D_A390, word_range as *const () as usize),
                (0x005D_ADA0, byte_range as *const () as usize),
            ]
            .into_iter()
            .map(|(a, f)| fake(a, f))
            .collect(),
        };
        let sim = unsafe { TrafficArchive::from_ptr(w.this()) };
        let rows = w.empty_rows.as_mut_ptr().cast();
        sim.set_byte_rows(rows);
        sim.set_byte_rows_end(rows);
        sim.set_pools(w.pool_head.ptr());
        sim.set_ferries(w.ferry_head.ptr());
        unsafe {
            store(w.pool_head.ptr(), 8, w.pool_head.ptr() as u32);
            store(w.ferry_head.ptr(), 0, w.ferry_head.ptr() as u32);
            let p = w.counts[0].as_mut_ptr() as u32;
            store(w.this(), 0x41C, p);
            let p = w.counts[1].as_mut_ptr() as u32;
            store(w.this(), 0x428, p);
        }
        w
    }
    fn this(&mut self) -> *mut u8 {
        self.primary.ptr().wrapping_add(8)
    }
    fn run(&mut self) -> (u32, u32, u32) {
        let mut frame = [
            fc4p_00712400 as *const () as u32,
            self.this() as u32,
            self.stream.ptr() as u32,
            0,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!("push dword ptr [edi + 8]", "mov ecx, [edi + 4]", "call dword ptr [edi]",
                "mov [edi + 12], eax", "mov [edi + 16], edx", "mov eax, [esp - 4]", "mov [edi + 20], eax",
                in("edi") frame.as_mut_ptr(), clobber_abi("C"));
        }
        (frame[3], frame[4], frame[5])
    }
}

// The version-41 empty wire schema from the archive tests. Keeping every primitive tag here checks
// that equal zero values cannot conceal a wrong stream method or a misplaced field.
fn empty_schema() -> Vec<Call> {
    use Call::*;
    let mut expected = vec![Short(41), Int(13), Int(1), Int(13), Int(9)];
    expected.extend(vec![Reference(0); 6]);
    expected.extend(vec![UInt(0); 3]);
    expected.extend([Reference(0), Reference(0), UInt(0)]);
    expected.extend(vec![UInt(0); 9]);
    expected.extend([
        Reference(0),
        Reference(0),
        UInt(0),
        UInt(0),
        Reference(0),
        Reference(0),
        Long(0, 0),
        Float(0),
        Float(0),
        Float(0),
        Double(0),
        Double(0),
        UInt(0),
        Byte(0),
        Int(0),
        UInt(0),
        UInt(0),
        Float(0),
        UInt(0),
        UInt(0),
        UInt(0),
        UInt(0),
        Float(0),
        UInt(0),
        UInt(0),
        Byte(0),
        PointerRange(0),
        Bool(0),
        Int(0),
        Int(0),
        Int(0),
        Int(0),
        Bool(0),
        UInt(0),
        UInt(0),
        SignedShort(0),
        SignedShort(0),
        WordRange(0),
        Bool(0),
        Bool(0),
        Float(0),
        Int(0),
        Int(0),
        UInt(0),
    ]);
    expected.extend(vec![UInt(0); 4]);
    expected.extend([Byte(0), UInt(0)]);
    expected.extend(vec![Int(0); 4]);
    expected.extend(vec![UInt(0); 27]);
    expected.extend([Byte(0), Byte(0)]);
    expected.extend(vec![Int(0); 3]);
    expected.extend(vec![UInt(0); 4]);
    expected.extend(vec![Int(0); 3]);
    expected.extend(vec![UInt(0); 10]);
    for _ in 0..2 {
        for _ in 0..14 {
            expected.extend([Float(0), UInt(0), Int(0), Int(0), Float(0)]);
        }
        expected.extend(vec![Short(0); 15]);
        expected.push(Int(12));
        for _ in 0..12 {
            expected.extend([Short(0), Float(0), Int(0), Int(0), Float(0)]);
        }
        expected.extend([Float(0), Float(0), Bool(0), Float(0), Byte(0)]);
    }
    expected.extend([UInt(0), Float(0), Float(0), UInt(0), Float(0), UInt(0)]);
    expected.extend(vec![UInt(0); 13]);
    expected.push(UInt(4));
    expected.extend(vec![UInt(0); 4]);
    expected.push(UInt(0));
    expected.extend(vec![UInt(0); 19]);
    expected.push(UInt(0));
    expected.push(Error);
    expected
}

#[test]
fn archive_version_41_empty_containers_follow_every_stream_width_and_final_error_only() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let before = unsafe { bytes(w.primary.ptr(), 0xD68) };
    for (error, success) in [(0, 1), (0xAABB_CC00, 1), (1, 0), (0xAABB_CC80, 0)] {
        STATE.with(|s| s.borrow_mut().error = error);
        assert_eq!(w.run(), (success, 0x1234_ABCD, w.ferry_head.ptr() as u32));
        let actual = calls();
        let expected = empty_schema();
        assert_eq!(actual.len(), expected.len());
        for (i, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "archive item {i}");
        }
        assert_eq!(unsafe { bytes(w.primary.ptr(), 0xD68) }, before);
    }
}

#[test]
fn archive_keeps_scalar_float_bits_and_quiets_only_the_two_loaded_double_values() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    unsafe {
        store(w.this(), 0x778, 0x8000_0000);
        store(w.this(), 0x77C, 0x7F80_1234);
        store(w.this(), 0x780, 0x7FC0_4321);
        w.this().add(0x788).cast::<u64>().write_unaligned(0x7FF0_0000_0000_4321);
        w.this().add(0x790).cast::<u64>().write_unaligned(0x8000_0000_0000_0000);
        w.this().add(0x814).write(0x80);
        w.this().add(0x861).write(0xFF);
        w.this().add(0x860).write(0xFE);
        store(w.this(), 0x770, 0x7654_3210);
        store(w.this(), 0x774, 0xFEDC_BA98);
    }
    let before = unsafe { bytes(w.primary.ptr(), 0xD68) };
    w.run();
    let log = calls();
    let i = log.iter().position(|c| matches!(c, Call::Long(..))).unwrap();
    assert_eq!(
        &log[i..i + 6],
        &[
            Call::Long(0x7654_3210, 0xFEDC_BA98),
            Call::Float(0x8000_0000),
            Call::Float(0x7F80_1234),
            Call::Float(0x7FC0_4321),
            Call::Double(0x7FF8_0000_0000_4321),
            Call::Double(0x8000_0000_0000_0000)
        ]
    );
    assert!(log.contains(&Call::Bool(0x80)));
    assert!(log.contains(&Call::Bool(0xFF)));
    assert!(log.contains(&Call::Byte(0xFE)));
    assert_eq!(unsafe { bytes(w.primary.ptr(), 0xD68) }, before);
}

#[test]
fn archive_destination_rows_and_reference_vector_reload_storage_after_stream_calls() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let mut rows = [[0u32; 3]; 2];
    let mut row_a = [255u8, 4, 3];
    let mut row_b = [2u8, 1, 0];
    rows[0] = [row_a.as_mut_ptr() as u32, row_a.as_mut_ptr().wrapping_add(3) as u32, 0];
    rows[1] = [row_b.as_mut_ptr() as u32, row_b.as_mut_ptr().wrapping_add(3) as u32, 0];
    let sim = unsafe { TrafficArchive::from_ptr(w.this()) };
    sim.set_byte_rows(rows.as_mut_ptr().cast());
    sim.set_byte_rows_end(rows.as_mut_ptr().wrapping_add(2).cast());
    let mut refs = [997u32, 0, 998];
    sim.set_references(refs.as_mut_ptr());
    sim.set_references_end(refs.as_mut_ptr().wrapping_add(3));
    w.run();
    let log = calls();
    assert_eq!(
        &log[11..23],
        &[
            Call::UInt(2),
            Call::UInt(3),
            Call::Byte(255),
            Call::Byte(4),
            Call::Byte(3),
            Call::Byte(2),
            Call::Byte(1),
            Call::Byte(0),
            Call::UInt(3),
            Call::Reference(997),
            Call::Reference(0),
            Call::Reference(998)
        ]
    );
    // A density-key write can redirect the following record data while preserving the cached node.
    let mut buckets = [0u32; 1];
    let mut density = Object::new(0x4C);
    unsafe {
        store(density.ptr(), 4, 731);
        buckets[0] = density.ptr() as u32;
        store(w.this(), 0x174, buckets.as_mut_ptr() as u32);
        store(w.this(), 0x178, buckets.as_mut_ptr().add(1) as u32);
        store(w.this(), 0x180, 1);
    }
    STATE.with(|s| s.borrow_mut().replace = Some((density.ptr() as u32, 8, 0x8000_FFFF)));
    w.run();
    let log = calls();
    let i = log.iter().position(|c| *c == Call::UInt(731)).unwrap();
    assert_eq!(&log[i + 1..i + 3], &[Call::Short(65535), Call::Short(32768)]);
}

unsafe fn hash(this: *mut u8, begin: usize, end: usize, count: usize, buckets: &mut [u32]) {
    unsafe {
        store(this, begin, buckets.as_mut_ptr() as u32);
        store(this, end, buckets.as_mut_ptr().add(buckets.len()) as u32);
        store(this, count, buckets.iter().filter(|&&p| p != 0).count() as u32);
    }
}

#[test]
fn archive_density_switch_rules_sets_and_tunnels_keep_nested_order_and_primitive_sizes() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let mut density = Object::new(0x4C);
    let mut density_buckets = [0, density.ptr() as u32, 0];
    let mut switch = Object::new(0x3C);
    let mut switch_buckets = [switch.ptr() as u32];
    let mut rules = [1u8, 1, 0, 1, 0, 0xA5, 0, 1, 0xFF, 0xFF];
    let mut tunnel = Object::new(0x34);
    let mut tunnel_buckets = [tunnel.ptr() as u32];
    let mut set_nodes = [Object::new(8), Object::new(8), Object::new(8)];
    let sp = set_nodes.each_mut().map(Object::ptr);
    let mut set_buckets = [0u32, sp[0] as u32, sp[2] as u32];
    unsafe {
        store(density.ptr(), 4, 731);
        for i in 0..33 {
            density.ptr().add(8 + i * 2).cast::<u16>().write_unaligned(if i == 16 {
                33553
            } else if i < 16 {
                65535
            } else {
                32768
            });
        }
        hash(w.this(), 0x174, 0x178, 0x180, &mut density_buckets);
        store(switch.ptr(), 4, 423);
        store(switch.ptr(), 8, 997);
        store(switch.ptr(), 12, 0x8000_0000);
        store(switch.ptr(), 16, 0x7FC0_1234);
        store(switch.ptr(), 20, 1.0f32.to_bits());
        switch.ptr().add(0x19).write(0x80);
        switch.ptr().add(0x1A).write(0xFF);
        switch.ptr().add(0x1B).write(0xFE);
        store(switch.ptr(), 0x24, rules.as_mut_ptr() as u32);
        store(switch.ptr(), 0x28, rules.as_mut_ptr().add(10) as u32);
        store(switch.ptr(), 0x34, 0x7F80_4321);
        switch.ptr().add(0x38).write(1);
        hash(w.this(), 0x708, 0x70C, 0x714, &mut switch_buckets);
        store(sp[0], 4, 55);
        store(sp[0], 0, sp[1] as u32);
        store(sp[1], 4, 1);
        store(sp[2], 4, 32768);
        hash(w.this(), 0x7B4, 0x7B8, 0x7C0, &mut set_buckets);
        store(w.this(), 0x7C0, 3);
        tunnel.ptr().add(4).cast::<u16>().write_unaligned(65535);
        tunnel.ptr().add(8).write(255);
        tunnel.ptr().add(9).write(0);
        tunnel.ptr().add(10).cast::<u16>().write_unaligned(0x8001);
        tunnel.ptr().add(0x30).cast::<u16>().write_unaligned(0xFF80);
        for i in 0..9 {
            store(tunnel.ptr(), 12 + i * 4, 0x8000_0000);
        }
        hash(w.this(), 0xC4, 0xC8, 0xD0, &mut tunnel_buckets);
    }
    w.run();
    let log = calls();
    let i = log.iter().position(|c| *c == Call::UInt(731)).unwrap();
    assert_eq!(&log[i + 1..i + 17], vec![Call::Short(65535); 16]);
    assert_eq!(log[i + 17], Call::Short(33553));
    assert_eq!(&log[i + 18..i + 34], vec![Call::Short(32768); 16]);
    let i = log.iter().position(|c| *c == Call::UInt(423)).unwrap();
    assert_eq!(
        &log[i..i + 21],
        &[
            Call::UInt(423),
            Call::Reference(997),
            Call::Float(0x8000_0000),
            Call::Float(0x7FC0_1234),
            Call::Float(1.0f32.to_bits()),
            Call::Bool(128),
            Call::Byte(255),
            Call::Byte(254),
            Call::Float(0),
            Call::Float(0),
            Call::UInt(1),
            Call::Bool(1),
            Call::Bool(1),
            Call::Bool(0),
            Call::Bool(1),
            Call::Bool(0),
            Call::Short(256),
            Call::Short(65535),
            Call::Float(0x7F80_4321),
            Call::Bool(1),
            Call::Reference(0)
        ]
    );
    let i = log.iter().position(|c| *c == Call::Short(55)).unwrap();
    assert_eq!(&log[i..i + 3], &[Call::Short(55), Call::Short(1), Call::Short(32768)]);
    let i = log.iter().position(|c| *c == Call::Short(0xFF80)).unwrap();
    assert_eq!(
        &log[i - 4..i + 10],
        &[
            Call::Short(65535),
            Call::Byte(255),
            Call::Byte(0),
            Call::Short(0x8001),
            Call::Short(0xFF80),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000),
            Call::Float(0x8000_0000)
        ]
    );
}

unsafe fn ring(p: &[*mut u8]) {
    for i in 0..p.len() {
        unsafe {
            store(p[i], 0, p[(i + 1) % p.len()] as u32);
            store(p[i], 4, p[(i + p.len() - 1) % p.len()] as u32);
        }
    }
}

#[test]
fn archive_pool_tree_and_ferry_links_keep_signed_words_route_addresses_and_counter_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let pv = table(&[(0x5C, pool as *const () as usize)]);
    let mut pool_object = object(4, &pv);
    let mut tree_node = Object::new(0x18);
    let mut terminal = Object::new(0x28);
    let mut link_head = Object::new(8);
    let mut neighbor_head = Object::new(8);
    let mut link = Object::new(0x28);
    let mut neighbor = Object::new(0x34);
    unsafe {
        store(w.pool_head.ptr(), 8, tree_node.ptr() as u32);
        store(tree_node.ptr(), 4, w.pool_head.ptr() as u32);
        tree_node.ptr().add(0x10).cast::<u16>().write_unaligned(37);
        store(tree_node.ptr(), 0x14, pool_object.ptr() as u32);
        ring(&[w.ferry_head.ptr(), terminal.ptr()]);
        ring(&[link_head.ptr(), link.ptr()]);
        ring(&[neighbor_head.ptr(), neighbor.ptr()]);
        for i in 0..6 {
            terminal.ptr().add(8 + i).write(i as u8 + 1);
        }
        terminal.ptr().add(0x10).write(1);
        terminal.ptr().add(0x14).cast::<u16>().write_unaligned(0xF0F0);
        terminal.ptr().add(0x20).cast::<u16>().write_unaligned(17);
        terminal.ptr().add(0x22).cast::<u16>().write_unaligned(65535);
        terminal.ptr().add(0x24).write(13);
        terminal.ptr().add(0x25).write(14);
        store(terminal.ptr(), 0x18, link_head.ptr() as u32);
        store(terminal.ptr(), 0x1C, neighbor_head.ptr() as u32);
        store(link.ptr(), 0x10, 0x7F80_1234);
        link.ptr().add(0xA).cast::<u16>().write_unaligned((-2i16) as u16);
        link.ptr().add(0xC).cast::<u16>().write_unaligned(7);
        link.ptr().add(8).cast::<u16>().write_unaligned(0xF0F0);
        for (i, value) in [1u16, 3, 2, 4].into_iter().enumerate() {
            link.ptr().add(0x20 + i * 2).cast::<u16>().write_unaligned(value);
        }
        store(neighbor.ptr(), 0x10, 0x8000_0000);
        for (i, value) in [-1i16, 0, 1, 2].into_iter().enumerate() {
            neighbor.ptr().add(8 + i * 2).cast::<i16>().write_unaligned(value);
        }
        for (i, value) in [5u16, 7, 6, 8].into_iter().enumerate() {
            neighbor.ptr().add(0x2C + i * 2).cast::<u16>().write_unaligned(value);
        }
    }
    w.run();
    let log = calls();
    let i = log.iter().position(|c| matches!(c, Call::Pool(_))).unwrap();
    assert_eq!(
        &log[i - 1..i + 1],
        &[Call::Short(37), Call::Pool(pool_object.ptr() as u32)]
    );
    let i = log.iter().position(|c| *c == Call::Byte(1)).unwrap();
    assert_eq!(
        &log[i..],
        &[
            Call::Byte(1),
            Call::Byte(2),
            Call::Byte(3),
            Call::Byte(4),
            Call::Byte(5),
            Call::Byte(6),
            Call::Byte(1),
            Call::Short(0xF0F0),
            Call::Short(17),
            Call::Short(65535),
            Call::Byte(13),
            Call::Byte(14),
            Call::UInt(1),
            Call::Float(0x7F80_1234),
            Call::SignedShort(65534),
            Call::SignedShort(7),
            Call::Short(0xF0F0),
            Call::Bytes(link.ptr().wrapping_add(0x14) as u32),
            Call::Short(1),
            Call::Short(2),
            Call::Short(3),
            Call::Short(4),
            Call::UInt(1),
            Call::Float(0x8000_0000),
            Call::SignedShort(65535),
            Call::SignedShort(0),
            Call::SignedShort(1),
            Call::SignedShort(2),
            Call::Bytes(neighbor.ptr().wrapping_add(0x14) as u32),
            Call::Bytes(neighbor.ptr().wrapping_add(0x20) as u32),
            Call::Short(5),
            Call::Short(6),
            Call::Short(7),
            Call::Short(8),
            Call::Error
        ]
    );
}

#[test]
fn archive_preserves_float_environment_and_existing_x87_value() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
    }
    let before = float_environment();
    w.run();
    let after = float_environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(after, before);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
}

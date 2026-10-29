// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Height and flag cases moved from the core terrain tests, plus entry-register, aliasing and callback
//! cases for the native object layout. Every entry is called using its original stack convention.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

const ENTRY_EAX: u32 = 0xA1B2_C3D4;
const ENTRY_EDX: u32 = 0xE5F6_8798;
const CALLEE_RESULT: u64 = 0xDEAD_BEEF_1234_5678;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Flip(usize, u32, u32, u32, u16),
    View(usize, u32, u32),
    AddRef(usize, usize),
    Destroy(usize, u32, u32),
}

#[derive(Default)]
struct Trace {
    calls: Vec<Call>,
    redirect: Option<usize>,
    output: usize,
}
thread_local! {
    static TRACE: RefCell<Trace> = RefCell::new(Trace::default());
}

extern "thiscall" fn view(this: *mut u8, z: u32, x: u32) -> u64 {
    TRACE.with(|t| t.borrow_mut().calls.push(Call::View(this as usize, z, x)));
    CALLEE_RESULT
}

extern "thiscall" fn flip_slot(this: *mut u8, x: u32, z: u32, value: u32) -> u64 {
    // SAFETY: installed on the test terrain and called on a valid cell.
    unsafe {
        let terrain = Terrain::from_ptr(this);
        let p = flags(terrain, vertex_offset(terrain, x, z));
        let old = p.read_unaligned();
        TRACE.with(|t| {
            let mut t = t.borrow_mut();
            t.calls.push(Call::Flip(this as usize, x, z, value, old));
            if let Some(base) = t.redirect.take() {
                terrain.set_vertices(base as *mut u8);
            }
        });
        let new = if value as u8 != 0 { old | 0x2000 } else { old & !0x2000 };
        p.write_unaligned(new);
    }
    CALLEE_RESULT
}

extern "thiscall" fn add_ref(this: *mut u8) -> u64 {
    TRACE.with(|t| {
        let mut t = t.borrow_mut();
        // SAFETY: each successful query supplies this output word before calling AddRef.
        let output = unsafe { (t.output as *const usize).read_unaligned() };
        t.calls.push(Call::AddRef(this as usize, output));
    });
    CALLEE_RESULT
}

extern "thiscall" fn destroy(this: *mut u8, value: u32) -> u64 {
    // SAFETY: the test's terrain stays allocated for the assertion.
    let refs = unsafe { Terrain::from_ptr(this) }.references();
    TRACE.with(|t| t.borrow_mut().calls.push(Call::Destroy(this as usize, value, refs)));
    CALLEE_RESULT
}

struct World {
    object: Object,
    vertices: Vec<u8>,
    _table: Box<[usize; 95]>,
    _fake: Fake,
    _zero_fake: Fake,
    _zero: Box<f32>,
}

impl World {
    fn new() -> Self {
        TRACE.with(|t| *t.borrow_mut() = Trace::default());
        let mut object = Object::new(0x110);
        let mut vertices = vec![0xA5; 9 * 0x24];
        let mut table = Box::new([0usize; 95]);
        table[1] = add_ref as *const () as usize;
        table[0x16C / 4] = destroy as *const () as usize;
        table[0x174 / 4] = flip_slot as *const () as usize;
        table[0x178 / 4] = flip_slot as *const () as usize;
        let mut zero = Box::new(0.0f32);
        Self {
            // SAFETY: fake_data retains only the address and the box stays allocated.
            _zero_fake: fake_data(0x00A8_1054, (&mut *zero as *mut f32).cast()),
            _zero: zero,
            _fake: fake(0x0075_2AF0, view as *const () as usize),
            // Set fields while these allocations are still local; moving the containers keeps their buffers.
            object: {
                let t = unsafe { Terrain::from_ptr(object.ptr()) };
                t.set_vtable(table.as_ptr() as u32);
                t.set_cells_x(2);
                t.set_cells_z(2);
                t.set_verts_x(3);
                t.set_verts_z(3);
                t.set_vertex_count(9);
                t.set_vertices(vertices.as_mut_ptr());
                t.set_sea_level(250.0);
                object
            },
            vertices,
            _table: table,
        }
    }

    fn terrain(&mut self) -> Terrain {
        unsafe { Terrain::from_ptr(self.object.ptr()) }
    }

    fn run(&mut self, entry: usize, args: &[u32]) -> [u32; 3] {
        let mut r = Registers {
            eax: ENTRY_EAX,
            ecx: self.object.ptr() as u32,
            edx: ENTRY_EDX,
        };
        // SAFETY: the supplied entry has exactly these original thiscall arguments.
        unsafe { invoke(&mut r, entry, self.object.ptr() as u32, args) };
        [r.eax, r.ecx, r.edx]
    }

    fn altitude(&mut self, entry: usize, args: &[u32]) -> (F80, u32, u32) {
        let mut call = RawCall::new(Conv::Thiscall, entry, self.object.ptr() as u32, args);
        call.eax = ENTRY_EAX;
        call.edx = ENTRY_EDX;
        call.take_st0 = 1;
        unsafe { call_raw(&mut call) };
        (call.st0, call.eax, call.edx)
    }

    fn set_flag(&mut self, index: usize, value: u16) {
        self.vertices[index * 0x24 + 0x22..index * 0x24 + 0x24].copy_from_slice(&value.to_le_bytes());
    }
    fn flag(&self, index: usize) -> u16 {
        u16::from_le_bytes(
            self.vertices[index * 0x24 + 0x22..index * 0x24 + 0x24]
                .try_into()
                .unwrap(),
        )
    }
    fn height_bits(&mut self, index: usize, value: u32) {
        self.vertices[index * 0x24..index * 0x24 + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn calls(&self) -> Vec<Call> {
        TRACE.with(|t| core::mem::take(&mut t.borrow_mut().calls))
    }
}

fn pointer<T>(value: &mut T) -> u32 {
    value as *mut T as u32
}

#[test]
fn vertex_bounds_and_class_id_preserve_untouched_registers() {
    let mut w = World::new();
    let this = w.object.ptr() as u32;
    let before = w.vertices.clone();
    for (x, z, answer) in [
        (0, 0, 1),
        (2, 2, 1),
        (3, 0, 0),
        (0, 3, 0),
        (u32::MAX, 0, 0),
        (0, u32::MAX, 0),
    ] {
        let result = w.run(fc4p_007404e0 as *const () as usize, &[x, z]);
        assert_eq!(result, [answer, this, if x < 3 { z } else { ENTRY_EDX }]);
    }
    assert_eq!(
        w.run(fc4p_007405d0 as *const () as usize, &[]),
        [0xE98F_9525, this, ENTRY_EDX]
    );
    assert_eq!(w.vertices, before);
}

#[test]
fn height_reads_and_corner_order_from_core() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    for (i, value) in [100.0f32, 104.0, 100.0, 108.0, 120.0, 100.0, 100.0, 100.0, 140.0]
        .into_iter()
        .enumerate()
    {
        w.height_bits(i, value.to_bits());
    }
    for (index, expected) in [(4, 120.0), (9, 0.0), (u32::MAX, 0.0)] {
        let (value, eax, edx) = w.altitude(fc4p_007413c0 as *const () as usize, &[index]);
        assert_eq!(value.to_f32(), expected);
        assert_eq!(eax, if index < 9 { index * 9 } else { index });
        assert_eq!(edx, ENTRY_EDX);
    }
    for (x, z, expected) in [
        (1, 1, 120.0),
        (2, 2, 140.0),
        (u32::MAX, 0, 0.0),
        (3, 0, 0.0),
        (0, 3, 0.0),
    ] {
        let (value, eax, edx) = w.altitude(fc4p_007413e0 as *const () as usize, &[x, z]);
        assert_eq!(value.to_f32(), expected);
        assert_eq!(eax, if x < 3 && z < 3 { (z * 3 + x) * 9 } else { 3 });
        assert_eq!(edx, if x < 3 { z } else { ENTRY_EDX });
    }
    let mut out = [0u32; 4];
    w.run(fc4p_007415f0 as *const () as usize, &[0, 0, pointer(&mut out)]);
    assert_eq!(out.map(f32::from_bits), [100.0, 108.0, 120.0, 104.0]);
    for (x, z) in [(2, 0), (0, 2), (u32::MAX, 0), (0, u32::MAX)] {
        let before = out;
        w.run(fc4p_007415f0 as *const () as usize, &[x, z, pointer(&mut out)]);
        assert_eq!(out, before);
    }
}

#[test]
fn altitude_loads_preserve_signed_zero_nan_and_the_callers_float_environment() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let cw = crate::x87::control_word();
    let mut mxcsr = 0u32;
    unsafe { core::arch::asm!("stmxcsr [{}]", in(reg) &mut mxcsr) };
    let sentinel = F80::from_i32(12345);
    unsafe { core::arch::asm!("fld tbyte ptr [{}]", in(reg) &sentinel, out("st(0)") _) };
    for bits in [0x8000_0000, 0x7FC1_2345, 0x7F80_0000, 0xFF80_0000, 1] {
        w.height_bits(0, bits);
        let (got, _, _) = w.altitude(fc4p_007413c0 as *const () as usize, &[0]);
        assert_eq!(got.to_f32().to_bits(), bits);
    }
    let (sea, eax, edx) = w.altitude(fc4p_00741760 as *const () as usize, &[]);
    assert_eq!(sea.to_f32(), 250.0);
    assert_eq!((eax, edx), (ENTRY_EAX, ENTRY_EDX));
    let mut restored = F80::ZERO;
    unsafe { core::arch::asm!("fstp tbyte ptr [{}]", in(reg) &mut restored, out("st(0)") _) };
    assert_eq!(restored, sentinel);
    let mut after = 0u32;
    unsafe { core::arch::asm!("stmxcsr [{}]", in(reg) &mut after) };
    assert_eq!(after, mxcsr);
    assert_eq!(crate::x87::control_word(), cw);
}

#[test]
fn corner_and_normal_outputs_use_sequential_word_copies_when_they_overlap() {
    let mut w = World::new();
    for i in 0..9 {
        w.height_bits(i, i as u32 + 10);
    }
    let out = w.vertices.as_mut_ptr().wrapping_add(3 * 0x24) as u32;
    w.run(fc4p_007415f0 as *const () as usize, &[0, 0, out]);
    let mut got = [0; 4];
    for (i, value) in got.iter_mut().enumerate() {
        *value = unsafe { word(out as *const u8, i * 4) };
    }
    assert_eq!(got, [10, 10, 14, 11]);
    let p = w.vertices.as_mut_ptr().wrapping_add(4);
    for (i, bits) in [0x8000_0000, 0x7FA1_2345, 0xBF80_0000].into_iter().enumerate() {
        unsafe { put(p, i * 4, bits) };
    }
    let result = w.run(fc4p_007417d0 as *const () as usize, &[0, 0, p.wrapping_add(4) as u32]);
    assert_eq!(result, [0x8000_0000, p.wrapping_add(4) as u32, 0x8000_0000]);
    for i in 0..4 {
        assert_eq!(unsafe { word(p, i * 4) }, 0x8000_0000);
    }
    let before = w.vertices.clone();
    for (x, z) in [(3, 0), (0, 3), (u32::MAX, 0)] {
        w.run(fc4p_007417d0 as *const () as usize, &[x, z, p as u32]);
        assert_eq!(w.vertices, before);
    }
}

#[test]
fn whole_city_rectangle_and_signed_padding_include_wrapping_inputs() {
    let mut w = World::new();
    let mut out = [-99i32; 4];
    let out_ptr = pointer(&mut out);
    assert_eq!(w.run(fc4p_00741660 as *const () as usize, &[out_ptr]), [out_ptr, 2, 2]);
    assert_eq!(out, [0, 0, 2, 2]);
    for (input, padding, expected) in [
        ([1, 1, 1, 1], 1, [0, 0, 2, 2]),
        ([0, 0, 2, 2], -1, [1, 1, 1, 1]),
        (
            [i32::MIN, i32::MAX, i32::MAX, i32::MIN],
            1,
            [i32::MAX, i32::MAX - 1, i32::MIN, i32::MIN + 1],
        ),
        ([8, 9, -2, -1], 0, [8, 9, -2, -1]),
    ] {
        out = input;
        assert_eq!(
            w.run(fc4p_00741e30 as *const () as usize, &[out_ptr, padding as u32]),
            [out_ptr, 2, 2]
        );
        assert_eq!(out, expected);
    }
}

#[test]
fn neighbor_masks_cover_edges_corners_and_mixed_signed_unsigned_bounds() {
    let mut w = World::new();
    let this = w.object.ptr() as u32;
    for (x, z, mask) in [
        (1, 1, 0xFF),
        (0, 1, 0x7C),
        (2, 1, 0xC7),
        (1, 0, 0xF1),
        (1, 2, 0x1F),
        (0, 0, 0x70),
        (2, 2, 0x07),
        (-1, -1, 0),
        (i32::MIN, 1, 0x44),
    ] {
        assert_eq!(
            w.run(fc4p_00741710 as *const () as usize, &[x as u32, z as u32]),
            [(ENTRY_EAX & !255) | mask, this, z as u32]
        );
    }
}

#[test]
fn public_cell_flag_mapping_covers_every_stored_word() {
    let mut w = World::new();
    // Sweep every stored bit combination, including all unrelated flags.
    for f in 0..=u16::MAX {
        w.set_flag(4, f);
        let r = w.run(fc4p_00741770 as *const () as usize, &[1, 1]);
        let expected = u32::from(f & 0x20 != 0)
            | (u32::from(f & 0x40 != 0) << 1)
            | (u32::from(f & 0x200 != 0) << 2)
            | (u32::from(f & 0x2000 != 0) << 3);
        assert_eq!(r, [expected, u32::from(f), 1]);
    }
    for args in [[2, 0], [0, 2], [u32::MAX, 0], [0, u32::MAX]] {
        assert_eq!(w.run(fc4p_00741770 as *const () as usize, &args)[0], 0);
    }
}

#[test]
fn triangulation_bits_have_independent_idempotence_and_view_callbacks() {
    let mut w = World::new();
    let view_pointer = 0x2345_6780;
    for (entry, bit) in [
        (fc4p_00741180 as *const () as usize, 0x1000u16),
        (fc4p_007411f0 as *const () as usize, 0x2000),
    ] {
        for ready in [0, 2] {
            w.terrain().set_view_ready(ready);
            w.terrain().set_view(view_pointer as *mut u8);
            w.set_flag(4, 0xAF67 & !bit);
            let r = w.run(entry, &[1, 1, 0xFFFF_0102]);
            assert_eq!(w.flag(4), 0xAF67 | bit);
            if ready == 0 {
                assert_eq!(r[0], u32::from((0xAF67 | bit) & !255));
            } else {
                assert_eq!((r[0], r[2]), (CALLEE_RESULT as u32, (CALLEE_RESULT >> 32) as u32));
            }
            assert_eq!(
                w.calls(),
                if ready == 0 {
                    vec![]
                } else {
                    vec![Call::View(view_pointer, 1, 1)]
                }
            );
            let unchanged = w.run(entry, &[1, 1, 1]);
            assert_eq!(unchanged[0], u32::from(0xAF67 | bit));
            assert!(w.calls().is_empty());
            w.run(entry, &[1, 1, 0x100]);
            assert_eq!(w.flag(4), 0xAF67 & !bit);
            w.calls();
            let before = w.vertices.clone();
            for args in [[2, 0, 1], [0, 2, 1], [u32::MAX, 0, 1]] {
                let r = w.run(entry, &args);
                assert_eq!(r[0], 2);
                assert_eq!(r[2], ENTRY_EDX);
                assert_eq!(w.vertices, before);
                assert!(w.calls().is_empty());
            }
        }
    }
}

#[test]
fn flipped_query_keeps_invalid_upper_eax_and_tests_both_bits() {
    let mut w = World::new();
    for (flags, expected) in [(0, 0), (0x1000, 1), (0x2000, 1), (0xCFFF, 0), (0xFFFF, 1)] {
        w.set_flag(0, flags);
        assert_eq!(w.run(fc4p_00741c20 as *const () as usize, &[0, 0])[0], expected);
    }
    w.terrain().set_cells_x(0x1234);
    assert_eq!(w.run(fc4p_00741c20 as *const () as usize, &[0x1234, 0])[0], 0x1200);
    assert_eq!(w.run(fc4p_00741c20 as *const () as usize, &[0, 2])[0], 0x1200);
}

#[test]
fn cell_flag_setter_retains_other_bits_and_calls_flip_before_view_even_when_unchanged() {
    let mut w = World::new();
    let this = w.object.ptr() as usize;
    w.terrain().set_view(0x1234 as *mut u8);
    w.terrain().set_view_ready(1);
    w.set_flag(4, 0x0A60);
    let r = w.run(fc4p_00742000 as *const () as usize, &[1, 1, 8]);
    assert_eq!(w.flag(4), 0x2800);
    assert_eq!(r, [(w.vertices.as_ptr() as u32 & !255) | 1, 1, 0xDEAD_0A60]);
    assert_eq!(w.calls(), [Call::Flip(this, 1, 1, 1, 0x0800), Call::View(0x1234, 1, 1)]);
    w.run(fc4p_00742000 as *const () as usize, &[1, 1, 8]);
    assert_eq!(w.calls(), [Call::Flip(this, 1, 1, 1, 0x2800), Call::View(0x1234, 1, 1)]);
    w.terrain().set_view_ready(0);
    let r = w.run(fc4p_00742000 as *const () as usize, &[1, 1, 0xFFFF_FFF7]);
    assert_eq!(w.calls(), [Call::Flip(this, 1, 1, 0x1FFF_FF00, 0x2A60)]);
    assert_eq!(w.flag(4), 0x0A60);
    assert_eq!(r[0] & 255, 1);
    let before = w.vertices.clone();
    for args in [[2, 0, 15], [0, 2, 15], [u32::MAX, 0, 15]] {
        assert_eq!(w.run(fc4p_00742000 as *const () as usize, &args)[0], ENTRY_EAX & !255);
        assert!(w.calls().is_empty());
        assert_eq!(w.vertices, before);
    }
}

#[test]
fn rectangle_clears_are_inclusive_and_leave_weights_and_unrelated_bytes() {
    let mut w = World::new();
    let mut rect = [1i32, 0, 2, 1];
    let mut expected = w.vertices.clone();
    for index in [1, 2, 4, 5] {
        for offset in [0x16, 0x1A, 0x1E] {
            expected[index * 0x24 + offset..index * 0x24 + offset + 2].fill(0);
        }
    }
    w.run(fc4p_00740890 as *const () as usize, &[pointer(&mut rect)]);
    assert_eq!(w.vertices, expected);
    for index in [1, 2, 4, 5] {
        let new = w.flag(index) & !0x8421;
        expected[index * 0x24 + 0x22..index * 0x24 + 0x24].copy_from_slice(&new.to_le_bytes());
    }
    let this = w.object.ptr() as u32;
    assert_eq!(
        w.run(fc4p_007409b0 as *const () as usize, &[pointer(&mut rect), 0xFFFF_8421]),
        [1, this, 3]
    );
    assert_eq!(w.vertices, expected);
    for mut rect in [[1i32, 2, 0, 1], [2, 0, 1, 2]] {
        w.run(fc4p_00740890 as *const () as usize, &[pointer(&mut rect)]);
        w.run(fc4p_007409b0 as *const () as usize, &[pointer(&mut rect), 0xFFFF]);
        assert_eq!(w.vertices, expected);
    }
}

#[test]
fn cliff_flip_clear_visits_z_then_x_and_reloads_the_vertex_array_after_a_callback() {
    let mut w = World::new();
    let this = w.object.ptr() as usize;
    for i in 0..9 {
        w.set_flag(i, 0xA807);
    }
    w.set_flag(1, 0xA007);
    let mut rect = [0i32, 0, 1, 1];
    w.run(fc4p_00740900 as *const () as usize, &[pointer(&mut rect)]);
    assert_eq!(
        w.calls(),
        [
            Call::Flip(this, 0, 0, 0, 0xA807),
            Call::Flip(this, 0, 1, 0, 0xA807),
            Call::Flip(this, 1, 1, 0, 0xA807)
        ]
    );
    for i in [0, 3, 4] {
        assert_eq!(w.flag(i), 0x8007);
    }
    let mut other = w.vertices.clone();
    other[0x23] = 0xFF;
    w.set_flag(0, 0x0800);
    TRACE.with(|t| t.borrow_mut().redirect = Some(other.as_mut_ptr() as usize));
    rect = [0, 0, 0, 0];
    w.run(fc4p_00740900 as *const () as usize, &[pointer(&mut rect)]);
    assert_eq!(other[0x23], 0xF7);
    assert_eq!(w.flag(0), 0x0800);
    for mut empty in [[1i32, 0, 0, 0], [0, 1, 0, 0]] {
        w.calls();
        w.run(fc4p_00740900 as *const () as usize, &[pointer(&mut empty)]);
        assert!(w.calls().is_empty());
    }
}

#[test]
fn interface_queries_write_before_addref_and_reject_without_writing() {
    let mut w = World::new();
    let this = w.object.ptr() as usize;
    for (iid, offset) in [
        (1, 0),
        (0xAB95_3253, 0),
        (0xC70B_D37E, 0),
        (0x4522_94AA, 4),
        (0xE4FD_A3D4, 8),
    ] {
        let mut out = usize::MAX;
        TRACE.with(|t| t.borrow_mut().output = &mut out as *mut usize as usize);
        let result = w.run(fc4p_00741e90 as *const () as usize, &[iid, pointer(&mut out)]);
        assert_eq!(out, this + offset);
        assert_eq!(result[0], 0x1234_5601);
        assert_eq!(result[2], 0xDEAD_BEEF);
        assert_eq!(w.calls(), [Call::AddRef(this, this + offset)]);
    }
    for iid in [0u32, 2, 0x4522_94A9, 0xAB95_3252, 0xAB95_3254, 0xFFFF_FFFF] {
        let mut out = usize::MAX;
        let result = w.run(fc4p_00741e90 as *const () as usize, &[iid, pointer(&mut out)]);
        let upper = if iid <= 0xAB95_3253 {
            iid.wrapping_sub(0x4522_94AA)
        } else {
            iid
        } & !255;
        assert_eq!(result, [upper, this as u32, ENTRY_EDX]);
        assert_eq!(out, usize::MAX);
        assert!(w.calls().is_empty());
    }
}

#[test]
fn unsigned_release_decrements_or_destroys_without_a_shutdown_call() {
    let mut w = World::new();
    let this = w.object.ptr() as usize;
    for refs in [2, 3, 0x8000_0000, u32::MAX] {
        w.terrain().set_references(refs);
        assert_eq!(
            w.run(fc4p_00741f70 as *const () as usize, &[]),
            [refs - 1, this as u32, ENTRY_EDX]
        );
        assert_eq!(w.terrain().references(), refs - 1);
        assert!(w.calls().is_empty());
    }
    for refs in [0, 1] {
        w.terrain().set_references(refs);
        let result = w.run(fc4p_00741f70 as *const () as usize, &[]);
        assert_eq!((result[0], result[2]), (0, 0xDEAD_BEEF));
        assert_eq!(w.calls(), [Call::Destroy(this, 1, refs)]);
    }
}

#[test]
fn view_and_plumbing_accessors_handle_null_and_byte_only_state() {
    let mut w = World::new();
    let this = w.object.ptr() as u32;
    for (base, expected) in [(0u32, 0), (0x1234_5678, 0x1234_5684), (0xFFFF_FFFC, 8)] {
        w.terrain().set_view(base as *mut u8);
        assert_eq!(
            w.run(fc4p_00741810 as *const () as usize, &[]),
            [expected, this, ENTRY_EDX]
        );
    }
    let mut display = [0xA5u8; 3];
    let _fake = fake_data(0x00B4_BC5C, display.as_mut_ptr().wrapping_add(1));
    assert_eq!(
        w.run(fc4p_00741ff0 as *const () as usize, &[0xFFFF_0102]),
        [0xA1B2_C302, this, ENTRY_EDX]
    );
    assert_eq!(display, [0xA5, 2, 0xA5]);
}

#[test]
fn texture_accumulation_initializes_only_once_and_preserves_all_other_object_bytes() {
    let mut w = World::new();
    let this = w.object.ptr() as u32;
    let before = unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x110) }.to_vec();
    assert_eq!(
        w.run(fc4p_007420b0 as *const () as usize, &[]),
        [0x8000_0000, this, ENTRY_EDX]
    );
    let mut expected = before;
    expected[0xC0] = 1;
    for offset in [0xC4, 0xC8] {
        expected[offset..offset + 4].copy_from_slice(&i32::MAX.to_le_bytes());
    }
    for offset in [0xCC, 0xD0] {
        expected[offset..offset + 4].copy_from_slice(&i32::MIN.to_le_bytes());
    }
    assert_eq!(unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x110) }, expected);
    w.terrain().set_accumulating(7);
    w.terrain().set_update_left(4);
    assert_eq!(
        w.run(fc4p_007420b0 as *const () as usize, &[]),
        [0xA1B2_C307, this, ENTRY_EDX]
    );
    assert_eq!(w.terrain().update_left(), 4);
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

game_fn! { #[allow(clippy::too_many_arguments)] fn invoke(configuration:*mut u8,record:*mut u8,bounds:*const i32,facing:i32,
x:*mut i32,z:*mut i32,mask1:*mut u32,mask2:*mut u32,kind:*mut u32)->u64 = "C" 0x0072_AD00; }

#[derive(Debug, PartialEq)]
enum Call {
    Size(usize),
    Cell(usize, u32, u32),
    Rotate(i32, i32, i32, [i32; 4]),
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }

extern "thiscall" fn size(this: *mut u8, x: *mut i32, z: *mut i32) {
    CALLS.with(|c| c.borrow_mut().push(Call::Size(this as usize)));
    unsafe {
        x.write(3);
        z.write(2);
    }
}

extern "thiscall" fn cell(this: *mut u8, x: f32, z: f32, out_x: *mut i32, out_z: *mut i32) {
    CALLS.with(|c| c.borrow_mut().push(Call::Cell(this as usize, x.to_bits(), z.to_bits())));
    unsafe {
        out_x.write(0);
        out_z.write(1);
    }
}

extern "C" fn rotate(f: i32, w: i32, h: i32, input: *const i32, output: *mut i32) {
    let r = unsafe { input.cast::<[i32; 4]>().read() };
    CALLS.with(|c| c.borrow_mut().push(Call::Rotate(f, w, h, r)));
    let [x, z, _, _] = r;
    let (x, z) = match f {
        0 => (w - 1 - x, h - 1 - z),
        1 => (z, w - 1 - x),
        2 => (x, z),
        3 => (h - 1 - z, x),
        _ => (7, 9),
    };
    unsafe {
        output.cast::<[i32; 4]>().write([x, z, x, z]);
    }
}

struct World {
    configuration: Box<[usize; 1]>,
    _config_table: Box<[usize; 8]>,
    city: Box<[usize; 1]>,
    _city_table: Box<[usize; 96]>,
    _city_slot: Box<*mut u8>,
    _half: Box<f32>,
    record: Object,
    payload: [u32; 3],
    _fakes: Vec<Fake>,
}

impl World {
    fn new() -> Self {
        CALLS.with(|c| c.borrow_mut().clear());
        let mut config_table = Box::new([0usize; 8]);
        config_table[7] = size as *const () as usize;
        let mut city_table = Box::new([0usize; 96]);
        city_table[0x17C / 4] = cell as *const () as usize;
        let mut city = Box::new([city_table.as_ptr() as usize]);
        let mut city_slot = Box::new(city.as_mut_ptr().cast());
        let mut half = Box::new(0.5f32);
        let mut record = Object::new(0x34);
        let r = unsafe { Placement::from_ptr(record.ptr()) };
        r.set_x1(0.0);
        r.set_z1(16.0);
        r.set_x2(16.0);
        r.set_z2(32.0);
        let fakes = vec![
            fake(0x004B_DFD0, rotate as *const () as usize),
            fake_data(0x00B4_3CEC, (&mut *city_slot as *mut *mut u8).cast()),
            fake_data(0x00A8_4D2C, (&mut *half as *mut f32).cast()),
        ];
        Self {
            configuration: Box::new([config_table.as_ptr() as usize]),
            _config_table: config_table,
            city,
            _city_table: city_table,
            _city_slot: city_slot,
            _half: half,
            record,
            payload: [0, 0x0001_0001, 0x0102_0408],
            _fakes: fakes,
        }
    }
    fn run(&mut self, facing: i32, record_facing: u8, outputs: &mut [u32; 5]) -> u64 {
        let r = unsafe { Placement::from_ptr(self.record.ptr()) };
        r.set_facing(record_facing);
        r.set_payload(self.payload.as_ptr());
        let origin = [40, 40, 42, 41];
        unsafe {
            invoke(
                self.configuration.as_mut_ptr().cast(),
                self.record.ptr(),
                origin.as_ptr(),
                facing,
                outputs.as_mut_ptr().cast(),
                outputs.as_mut_ptr().add(1).cast(),
                outputs.as_mut_ptr().add(2),
                outputs.as_mut_ptr().add(3),
                outputs.as_mut_ptr().add(4),
            )
        }
    }
}

#[test]
fn network_placement_rotates_the_configuration_centre_and_masks() {
    let _fpu = GameFpu::new();
    for (facing, expected) in [(0, [42, 40]), (1, [41, 42]), (2, [40, 41]), (3, [40, 40])] {
        let mut w = World::new();
        let mut out = [0; 5];
        let ret = w.run(facing, 0, &mut out);
        let shift = facing as u32 * 8;
        assert_eq!(&out[..2], &expected);
        assert_eq!(out[2], 0x0001_0001u32.rotate_left(shift));
        assert_eq!(out[3], 0x0102_0408u32.rotate_left(shift));
        assert_eq!(out[4], 0);
        assert_eq!(
            ret,
            (u64::from(shift) << 32) | u64::from(0x0102_0408u32.wrapping_shl(shift))
        );
        assert_eq!(
            CALLS.with(|c| core::mem::take(&mut *c.borrow_mut())),
            [
                Call::Size(w.configuration.as_ptr() as usize),
                Call::Cell(w.city.as_ptr() as usize, 8f32.to_bits(), 24f32.to_bits()),
                Call::Rotate(facing, 3, 2, [0, 1, 0, 1])
            ]
        );
    }
}

#[test]
fn mask_rotation_uses_signed_wrapping_orientation_and_modulo_32_shifts() {
    let _fpu = GameFpu::new();
    for facing in [-9, -4, -3, -1, 0, 1, 4, 7, i32::MIN, i32::MAX] {
        for record_facing in [0, 1, 3, 4, 255] {
            let mut w = World::new();
            let mut out = [0; 5];
            w.payload = [0x1234_5678, 0x1122_3344, 0x5566_7788];
            let result = w.run(facing, record_facing, &mut out);
            let shift = (i32::from(record_facing).wrapping_add(facing) % 4).wrapping_mul(8) as u32;
            assert_eq!(out[2], w.payload[1].rotate_left(shift));
            assert_eq!(out[3], w.payload[2].rotate_left(shift));
            assert_eq!(out[4], w.payload[0]);
            assert_eq!((result >> 32) as u32, shift);
        }
    }
}

#[test]
fn aliased_masks_are_written_then_rotated_in_sequence() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let r = unsafe { Placement::from_ptr(w.record.ptr()) };
    r.set_payload(w.payload.as_ptr());
    r.set_facing(0);
    let mut output = [0u32; 3];
    let p = output.as_mut_ptr();
    let bounds = [40, 40];
    unsafe {
        invoke(
            w.configuration.as_mut_ptr().cast(),
            w.record.ptr(),
            bounds.as_ptr(),
            1,
            p.cast(),
            p.add(1).cast(),
            p.add(2),
            p.add(2),
            p.add(2),
        )
    };
    assert_eq!(output, [41, 42, w.payload[2].rotate_left(16)]);
}

#[test]
fn midpoint_uses_x87_extended_exponent_and_stores_only_after_multiplication() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let r = unsafe { Placement::from_ptr(w.record.ptr()) };
    r.set_x1(f32::MAX);
    r.set_x2(f32::MAX);
    r.set_z1(-0.0);
    r.set_z2(-0.0);
    let mut out = [0; 5];
    w.run(0, 0, &mut out);
    let c = CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()));
    assert_eq!(
        c[1],
        Call::Cell(w.city.as_ptr() as usize, f32::MAX.to_bits(), (-0.0f32).to_bits())
    );
    assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
    assert_eq!(crate::x87::status_word() & 0x3800, 0);
}

extern "C" fn leave_rotation_unwritten(_f: i32, _w: i32, _h: i32, _input: *const i32, _output: *mut i32) {}

#[test]
fn original_cdecl_argument_is_overwritten_and_unwritten_rotation_keeps_original_scratch() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let _rotate = fake(0x004B_DFD0, leave_rotation_unwritten as *const () as usize);
    let mut frame = [0xA5A5_A5A5u32; 32];
    let stack = unsafe { frame.as_mut_ptr().add(16) };
    let bounds = [40i32, 50];
    let mut out = [0u32; 5];
    let r = unsafe { Placement::from_ptr(w.record.ptr()) };
    r.set_payload(w.payload.as_ptr());
    let args = [
        w.configuration.as_mut_ptr() as u32,
        w.record.ptr() as u32,
        bounds.as_ptr() as u32,
        99,
        out.as_mut_ptr() as u32,
        unsafe { out.as_mut_ptr().add(1) } as u32,
        unsafe { out.as_mut_ptr().add(2) } as u32,
        unsafe { out.as_mut_ptr().add(3) } as u32,
        unsafe { out.as_mut_ptr().add(4) } as u32,
    ];
    unsafe {
        core::ptr::copy_nonoverlapping(args.as_ptr(), stack.add(1), 9);
        stack.sub(4).cast::<[i32; 4]>().write([7, 9, 11, 13]);
        network_cell_and_masks(stack);
    }
    assert_eq!(
        unsafe { stack.add(1).read() },
        0,
        "the configuration argument becomes the converted X cell"
    );
    assert_eq!(&out[..2], &[47, 59]);
    assert_eq!(unsafe { stack.sub(4).cast::<[i32; 4]>().read() }, [7, 9, 11, 13]);
    assert_eq!(unsafe { core::slice::from_raw_parts(stack.add(2), 8) }, &args[1..]);
}

#[test]
fn placement_keeps_an_existing_x87_value_control_word_and_mxcsr() {
    #[repr(C, align(16))]
    struct FpuState([u8; 512]);
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let mut out = [0; 5];
    let mut before = FpuState([0; 512]);
    let mut after = FpuState([0; 512]);
    unsafe {
        core::arch::asm!("fld1","fxsave [{p}]",p=in(reg)&mut before,out("st(0)") _,options(nostack));
        w.run(1, 0, &mut out);
        core::arch::asm!("fxsave [{p}]","fstp st(0)",p=in(reg)&mut after,out("st(0)") _,options(nostack));
    }
    assert_eq!(&before.0[..2], &after.0[..2]);
    assert_eq!(
        u16::from_le_bytes([before.0[2], before.0[3]]) & 0x3800,
        u16::from_le_bytes([after.0[2], after.0[3]]) & 0x3800
    );
    assert_eq!(before.0[4], after.0[4]);
    assert_eq!(&before.0[24..28], &after.0[24..28]);
    assert_eq!(&before.0[32..42], &after.0[32..42]);
}

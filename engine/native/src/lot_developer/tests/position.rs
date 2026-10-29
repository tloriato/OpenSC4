// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::position::*;

slot!(position_rect, output);
slot!(position_corner, x, z, out_x, out_z);
slot!(position_facing);
slot!(position_flags);
slot!(position_foundation, lot);
slot!(position_occupant);
slot!(position_get, output);
slot!(position_water);

unsafe extern "thiscall" fn position_height(this: *mut u8) -> f64 {
    run(this, "position_height", &[]);
    100.000_000_953_674_32
}
unsafe extern "thiscall" fn position_terrain(this: *mut u8, x: f32, z: f32) -> f64 {
    run(this, "position_terrain", &[x.to_bits(), z.to_bits()]);
    100.000_000_953_674_32
}
unsafe extern "thiscall" fn position_area(this: *mut u8, rect: *mut u32, lowest: u32, water: u32) -> f64 {
    run(this, "position_area", &[rect as u32, lowest, water]);
    100.000_000_953_674_32
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

unsafe fn position_call(this: *mut u8, args: [u32; 5]) -> (u64, [u32; 3]) {
    let mut frame = [
        fc4p_006c4ef0 as *const () as u32,
        this as u32,
        args[0],
        args[1],
        args[2],
        args[3],
        args[4],
        0,
        0,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push [edi + 24]", "push [edi + 20]", "push [edi + 16]", "push [edi + 12]", "push [edi + 8]",
            "mov ecx, [edi + 4]", "call dword ptr [edi]", "mov [edi + 28], eax", "mov [edi + 32], edx",
            "mov eax, [esp - 20]", "mov [edi + 36], eax", "mov eax, [esp - 12]", "mov [edi + 40], eax",
            "mov eax, [esp - 8]", "mov [edi + 44], eax",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"),
        );
    }
    (
        (u64::from(frame[8]) << 32) | u64::from(frame[7]),
        [frame[9], frame[10], frame[11]],
    )
}

struct World {
    developer: Mock,
    lot: Mock,
    city: Mock,
    terrain: Mock,
    config: Mock,
    record: Object,
    output: [u32; 5],
    orientation: [u32; 3],
    constants: Box<[f32; 3]>,
    _fakes: Vec<crate::testing::Fake>,
}

impl World {
    fn new(kind: u8) -> Self {
        let mut world = Self {
            developer: mock!(0xCC => position_foundation, 0x40 => position_area),
            lot: mock!(0x38 => position_rect, 0x5C => position_facing, 0x30 => position_flags, 0x40 => position_height),
            city: mock!(0x180 => position_corner),
            terrain: mock!(0x34 => position_terrain),
            config: mock!(0x38 => position_water),
            record: Object::new(52),
            output: [0xCAFE_BABE; 5],
            orientation: [0xAABB_CCDD; 3],
            constants: Box::new([0.0625, 0.5, 16.0]),
            _fakes: Vec::new(),
        };
        world._fakes.extend([
            fake_data(0x00AB_1B04, (&mut world.constants[0] as *mut f32).cast()),
            fake_data(0x00A8_4D2C, (&mut world.constants[1] as *mut f32).cast()),
            fake_data(0x00A8_D45C, (&mut world.constants[2] as *mut f32).cast()),
        ]);
        unsafe {
            let developer = LotDeveloper::from_ptr(world.developer.ptr());
            developer.set_city(world.city.ptr());
            developer.set_terrain(world.terrain.ptr());
            let record = PositionRecord::from_ptr(world.record.ptr());
            record.set_kind(kind);
            record.set_orientation(3);
            record.set_x(5.0);
            record.set_y(1.5);
            record.set_z(7.0);
            record.set_x0(2.0);
            record.set_z0(3.0);
            record.set_x1(8.0);
            record.set_z1(11.0);
        }
        world
    }
    fn prepare(&mut self, facing: i32, flags: u32) {
        expect(self.lot.ptr(), "position_rect", vec![out(&[1, 2, 3, 4])], RETURN);
        expect(
            self.city.ptr(),
            "position_corner",
            vec![w(1), w(2), out(&bits(&[16.0])), out(&bits(&[32.0]))],
            RETURN,
        );
        expect(
            self.city.ptr(),
            "position_corner",
            vec![w(4), w(5), out(&bits(&[64.0])), out(&bits(&[80.0]))],
            RETURN,
        );
        expect(self.lot.ptr(), "position_facing", vec![], facing as u32 as u64);
        expect(self.lot.ptr(), "position_flags", vec![], u64::from(flags));
    }
    fn position_pointer(&mut self) -> u32 {
        self.output[1..].as_mut_ptr() as u32
    }
    fn invoke(&mut self, config: bool) -> (u64, [u32; 3]) {
        unsafe {
            position_call(
                self.developer.ptr(),
                [
                    self.lot.ptr() as u32,
                    if config { self.config.ptr() as u32 } else { 0 },
                    self.record.ptr() as u32,
                    self.position_pointer(),
                    self.orientation[1..].as_mut_ptr() as u32,
                ],
            )
        }
    }
    fn check(&mut self, facing: i32, x: f32, z: f32, returned: (u64, [u32; 3])) {
        let orientation = facing.wrapping_add(3) % 4;
        assert_eq!(
            returned.0,
            (u64::from(orientation as u32) << 32) | u64::from(self.orientation[1..].as_mut_ptr() as u32)
        );
        assert_eq!(
            self.output,
            [0xCAFE_BABE, x.to_bits(), 101.5f32.to_bits(), z.to_bits(), 0xCAFE_BABE]
        );
        assert_eq!(self.orientation, [0xAABB_CCDD, orientation as u32, 0xAABB_CCDD]);
        assert_eq!(returned.1[0], 80.0f32.to_bits());
        finished();
    }
}

#[test]
fn object_position_rotates_offsets_and_footprints_and_forwards_the_water_argument_word() {
    let _fpu = GameFpu::new();
    for (facing, x, z, footprint) in [
        (0, 59.0, 73.0, [56.0, 69.0, 62.0, 77.0]),
        (1, 23.0, 75.0, [19.0, 72.0, 27.0, 78.0]),
        (2, 21.0, 39.0, [18.0, 35.0, 24.0, 43.0]),
        (3, 57.0, 37.0, [53.0, 34.0, 61.0, 40.0]),
        (4, 0.0, 0.0, [0.0, 1.0, 0.0, 1.0]),
        (-1, 0.0, 0.0, [0.0, 1.0, 0.0, 1.0]),
        (i32::MIN, 0.0, 0.0, [0.0, 1.0, 0.0, 1.0]),
        (i32::MAX, 0.0, 0.0, [0.0, 1.0, 0.0, 1.0]),
    ] {
        let mut world = World::new(3);
        world.prepare(facing, 0xAABB_CC00);
        expect(
            world.developer.ptr(),
            "position_foundation",
            vec![p(world.lot.ptr())],
            0,
        );
        expect(world.config.ptr(), "position_water", vec![], 0xDEAD_BEAB);
        let water = (world.position_pointer() & !255) | 0xAB;
        expect(
            world.developer.ptr(),
            "position_area",
            vec![Arg::In(bits(&footprint)), w(0), w(water)],
            0,
        );
        let returned = world.invoke(true);
        assert_eq!(returned.1[1], world.record.ptr() as u32);
        assert_eq!(returned.1[2], water);
        world.check(facing, x, z, returned);
    }
}

#[test]
fn object_position_selects_lot_height_foundation_height_or_terrain_in_order() {
    let _fpu = GameFpu::new();
    for kind in [0, 1, 2, 4, 7] {
        let mut world = World::new(kind);
        world.prepare(2, 1);
        expect(world.lot.ptr(), "position_height", vec![], 0);
        let returned = world.invoke(false);
        let (x, z): (f32, f32) = if kind == 2 { (24.0, 40.0) } else { (21.0, 39.0) };
        assert_eq!(returned.1[2], world.position_pointer());
        world.check(2, x, z, returned);
    }
    let mut world = World::new(1);
    let mut foundation = mock!(0x0C => position_occupant);
    let mut occupant = mock!(0x20 => position_get);
    world.prepare(2, 0);
    expect(
        world.developer.ptr(),
        "position_foundation",
        vec![p(world.lot.ptr())],
        foundation.ptr() as u64,
    );
    expect(foundation.ptr(), "position_occupant", vec![], occupant.ptr() as u64);
    expect(
        occupant.ptr(),
        "position_get",
        vec![out(&bits(&[9.0, 100.0, 11.0]))],
        RETURN,
    );
    let returned = world.invoke(true);
    world.check(2, 21.0, 39.0, returned);
    for kind in [1, 2, 4] {
        let mut world = World::new(kind);
        world.prepare(2, 0);
        expect(
            world.developer.ptr(),
            "position_foundation",
            vec![p(world.lot.ptr())],
            0,
        );
        let (x, z): (f32, f32) = if kind == 2 { (24.0, 40.0) } else { (21.0, 39.0) };
        expect(
            world.terrain.ptr(),
            "position_terrain",
            vec![w(x.to_bits()), w(z.to_bits())],
            0,
        );
        let returned = world.invoke(false);
        assert_eq!(returned.1[2], world.position_pointer() & !255);
        world.check(2, x, z, returned);
    }
}

#[test]
fn texture_position_snaps_through_the_integer_helper_and_writes_the_last_cell_to_the_argument() {
    let _fpu = GameFpu::new();
    for (x, z, want_x, want_z, last_cell) in [
        (-17.0, -1.0, 8.0, 40.0, 0),
        (16.0, 32.0, 40.0, 72.0, 2),
        (f32::INFINITY, f32::NAN, 24.0, 40.0, 0),
        (f32::MAX, -f32::MAX, 24.0, 40.0, 0),
    ] {
        let mut world = World::new(2);
        unsafe {
            let record = PositionRecord::from_ptr(world.record.ptr());
            record.set_x(x);
            record.set_z(z);
        }
        world.prepare(2, 1);
        expect(world.lot.ptr(), "position_height", vec![], 0);
        let returned = world.invoke(false);
        assert_eq!(returned.1[1], last_cell);
        world.check(2, want_x, want_z, returned);
    }
}

#[test]
fn object_position_keeps_height_addition_operand_order_for_nan_payloads() {
    let _fpu = GameFpu::new();
    let mut world = World::new(1);
    let mut foundation = mock!(0x0C => position_occupant);
    let mut occupant = mock!(0x20 => position_get);
    unsafe {
        PositionRecord::from_ptr(world.record.ptr()).set_y(f32::from_bits(0x7FC0_0100));
    }
    world.prepare(2, 0);
    expect(
        world.developer.ptr(),
        "position_foundation",
        vec![p(world.lot.ptr())],
        foundation.ptr() as u64,
    );
    expect(foundation.ptr(), "position_occupant", vec![], occupant.ptr() as u64);
    expect(occupant.ptr(), "position_get", vec![out(&[0, 0x7F80_0200, 0])], RETURN);
    world.invoke(false);
    assert_eq!(world.output[2], 0x7FC0_0100);
    finished();
}

#[test]
fn object_position_reads_orientation_after_writing_the_aliased_facing_output() {
    let _fpu = GameFpu::new();
    let mut world = World::new(1);
    world.prepare(2, 1);
    expect(world.lot.ptr(), "position_height", vec![], 0);
    let record = world.record.ptr();
    let returned = unsafe { position_call(world.developer.ptr(), [
        world.lot.ptr() as u32, 0, record as u32, world.position_pointer(), record as u32,
    ]) };
    assert_eq!(returned.0, (2u64 << 32) | u64::from(record as u32));
    assert_eq!(unsafe { record.cast::<u32>().read() }, 2);
    assert_eq!(world.output[2], 101.5f32.to_bits());
    finished();
}

#[test]
fn texture_position_keeps_an_existing_x87_value_and_the_float_environment() {
    #[repr(C, align(16))]
    struct State([u8; 512]);
    let _fpu = GameFpu::new();
    let mut world = World::new(2);
    world.prepare(2, 1);
    expect(world.lot.ptr(), "position_height", vec![], 0);
    let mut before = State([0; 512]);
    let mut after = State([0; 512]);
    let (before_flags, after_flags): (u32, u32);
    unsafe {
        core::arch::asm!("fld1", "fxsave [{state}]", "pushfd", "pop {flags}",
            state = in(reg) &mut before, flags = out(reg) before_flags, out("st(0)") _,);
        world.invoke(false);
        core::arch::asm!("fxsave [{state}]", "fstp st(0)", "pushfd", "pop {flags}",
            state = in(reg) &mut after, flags = out(reg) after_flags, out("st(0)") _,);
    }
    assert_eq!(before.0[..2], after.0[..2]);
    assert_eq!(before.0[3] & 0x38, after.0[3] & 0x38);
    assert_eq!(before.0[4], after.0[4]);
    assert_eq!(before.0[24..28], after.0[24..28]);
    assert_eq!(before.0[32..42], after.0[32..42]);
    assert_eq!(before_flags & 0x400, after_flags & 0x400);
    finished();
}

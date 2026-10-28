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

//! Coordinate, bitmap delegation, zone callback, and lot-query contracts.
use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
struct Call(u32, usize, Vec<u32>);
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
fn log(slot: u32, this: *mut u8, args: &[u32]) {
    CALLS.with(|c| c.borrow_mut().push(Call(slot, this as usize, args.to_vec())));
}
fn calls() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
#[repr(C)]
struct City {
    vt: *const usize,
    width: u32,
    height: u32,
    wx: f64,
    wz: f64,
    sim: *mut u8,
    swap: *mut u8,
    radius: Option<u32>,
    bitmap: *mut u32,
    source_rect: *mut Rect,
}
extern "thiscall" fn width(p: *mut City) -> u32 {
    log(0x174, p.cast(), &[]);
    let c = unsafe { &mut *p };
    if !c.sim.is_null() {
        let s = unsafe { PowerSim::from_ptr(c.sim) };
        if !c.swap.is_null() {
            s.set_city(c.swap);
        }
        if let Some(r) = c.radius {
            s.set_radius(r);
        }
        if !c.bitmap.is_null() {
            s.set_wire(c.bitmap);
            s.set_powered(c.bitmap);
        }
    }
    if !c.source_rect.is_null() {
        unsafe {
            (*c.source_rect).x0 = 500;
        }
    }
    c.width
}
extern "thiscall" fn height(p: *mut City) -> u32 {
    log(0x178, p.cast(), &[]);
    unsafe { (*p).height }
}
extern "thiscall" fn width_x(p: *mut City) -> f64 {
    log(0x16C, p.cast(), &[]);
    let c = unsafe { &*p };
    if !c.swap.is_null() {
        unsafe { PowerSim::from_ptr(c.sim) }.set_city(c.swap);
    }
    c.wx
}
extern "thiscall" fn width_z(p: *mut City) -> f64 {
    log(0x170, p.cast(), &[]);
    unsafe { (*p).wz }
}
struct World {
    object: Object,
    city: Box<City>,
    vt: Box<[usize; 0x17C / 4]>,
}
impl World {
    fn new() -> Self {
        let mut vt = Box::new([0usize; 0x17C / 4]);
        vt[0x174 / 4] = width as *const () as usize;
        vt[0x178 / 4] = height as *const () as usize;
        vt[0x16C / 4] = width_x as *const () as usize;
        vt[0x170 / 4] = width_z as *const () as usize;
        let mut w = Self {
            object: Object::new(0x400),
            city: Box::new(City {
                vt: vt.as_ptr(),
                width: 64,
                height: 32,
                wx: 16.0,
                wz: 16.0,
                sim: core::ptr::null_mut(),
                swap: core::ptr::null_mut(),
                radius: None,
                bitmap: core::ptr::null_mut(),
                source_rect: core::ptr::null_mut(),
            }),
            vt,
        };
        w.city.sim = w.object.ptr();
        let city = w.city_ptr();
        w.sim().set_city(city);
        w.sim().set_radius(2);
        calls();
        w
    }
    fn sim(&mut self) -> PowerSim {
        unsafe { PowerSim::from_ptr(self.object.ptr()) }
    }
    fn city_ptr(&mut self) -> *mut u8 {
        (&mut *self.city as *mut City).cast()
    }
    fn point(&mut self, x: u32, z: u32) -> Rect {
        let mut r = Rect::default();
        unsafe { fc4p_006ead70(self.object.ptr(), x, z, &mut r) };
        r
    }
}
#[test]
fn neighborhoods_clip_inclusive_edges_and_call_size_twice_only_when_clamped() {
    let mut w = World::new();
    let city = w.city_ptr() as usize;
    for (x, z, expected, slots) in [
        (10, 20, [8, 18, 12, 22], vec![0x174, 0x178]),
        (1, 0, [0, 0, 3, 2], vec![0x174, 0x178]),
        (63, 31, [61, 29, 63, 31], vec![0x174, 0x174, 0x178, 0x178]),
        (62, 30, [60, 28, 63, 31], vec![0x174, 0x174, 0x178, 0x178]),
        (
            u32::MAX,
            u32::MAX,
            [u32::MAX - 2, u32::MAX - 2, 1, 1],
            vec![0x174, 0x178],
        ),
    ] {
        let rect = Rect {
            x0: expected[0],
            z0: expected[1],
            x1: expected[2],
            z1: expected[3],
        };
        assert_eq!(w.point(x, z), rect);
        assert_eq!(
            calls(),
            slots.into_iter().map(|s| Call(s, city, vec![])).collect::<Vec<_>>()
        );
        let mut r = Rect {
            x0: x,
            z0: z,
            x1: x,
            z1: z,
        };
        unsafe { fc4p_006eae20(w.object.ptr(), &mut r) };
        assert_eq!(r, rect);
        calls();
    }
    w.city.width = 0;
    w.city.height = 0;
    assert_eq!(
        w.point(0, 0),
        Rect {
            x0: 0,
            z0: 0,
            x1: u32::MAX,
            z1: u32::MAX
        }
    );
    assert_eq!(calls().len(), 4);
    w.sim().set_radius(0);
    w.city.width = 64;
    w.city.height = 32;
    assert_eq!(
        w.point(2, 3),
        Rect {
            x0: 2,
            z0: 3,
            x1: 2,
            z1: 3
        }
    );
}
#[test]
fn neighborhood_reloads_city_and_radius_after_each_virtual_call() {
    let mut w = World::new();
    let mut next = Box::new(City {
        vt: w.vt.as_ptr(),
        width: 70,
        height: 40,
        wx: 16.0,
        wz: 16.0,
        sim: core::ptr::null_mut(),
        swap: core::ptr::null_mut(),
        radius: None,
        bitmap: core::ptr::null_mut(),
        source_rect: core::ptr::null_mut(),
    });
    let n = (&mut *next as *mut City).cast();
    w.city.swap = n;
    w.city.radius = Some(5);
    assert_eq!(
        w.point(63, 31),
        Rect {
            x0: 61,
            z0: 29,
            x1: 69,
            z1: 36
        }
    );
    assert_eq!(
        calls(),
        [
            Call(0x174, w.city_ptr() as usize, vec![]),
            Call(0x174, n as usize, vec![]),
            Call(0x178, n as usize, vec![])
        ]
    );
}
#[test]
fn expansion_reloads_the_later_rectangle_fields_after_city_callbacks() {
    let mut w = World::new();
    let mut r = Rect {
        x0: 10,
        z0: 20,
        x1: 12,
        z1: 22,
    };
    w.city.source_rect = &mut r;
    unsafe { fc4p_006eae20(w.object.ptr(), &mut r) };
    assert_eq!(
        r,
        Rect {
            x0: 500,
            z0: 18,
            x1: 14,
            z1: 24
        }
    );
    assert_eq!(calls().len(), 2);
}
#[test]
fn position_conversion_uses_reciprocal_then_multiply_and_full_truncation_result() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let c = w.city_ptr() as usize;
    for (x, z, ex, ez, full) in [
        (31.999, -31.999, 1, -1, -1i64),
        (-0.0, 0.0, 0, 0, 0),
        (f32::INFINITY, f32::NAN, 0, 0, i64::MIN),
        (0.0, 68_719_476_736.0, 0, 0, 4_294_967_296),
    ] {
        let mut out = [0x1234i32, 0, 0, 0x5678];
        let result = unsafe { fc4p_006eac40(w.object.ptr(), x, z, &mut out[1], &mut out[2]) };
        assert_eq!((out, result), ([0x1234, ex, ez, 0x5678], full));
        assert_eq!(calls(), [Call(0x16C, c, vec![]), Call(0x170, c, vec![])]);
    }
    w.city.wx = 3.0;
    let mut x = 0;
    let mut z = 0;
    unsafe { fc4p_006eac40(w.object.ptr(), 50_331_648.0, 0.0, &mut x, &mut z) };
    assert_eq!(x, 16_777_216);
}
#[test]
fn position_conversion_reloads_city_and_keeps_unrounded_virtual_float_result() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let mut next = Box::new(City {
        vt: w.vt.as_ptr(),
        width: 64,
        height: 32,
        wx: 16.0,
        wz: 1.000_000_06,
        sim: core::ptr::null_mut(),
        swap: core::ptr::null_mut(),
        radius: None,
        bitmap: core::ptr::null_mut(),
        source_rect: core::ptr::null_mut(),
    });
    let n = (&mut *next as *mut City).cast();
    w.city.swap = n;
    let (mut x, mut z) = (0, 0);
    unsafe { fc4p_006eac40(w.object.ptr(), 32.0, 16_777_216.0, &mut x, &mut z) };
    assert_eq!((x, z), (2, 16_777_215));
    assert_eq!(
        calls(),
        [
            Call(0x16C, w.city_ptr() as usize, vec![]),
            Call(0x170, n as usize, vec![])
        ]
    );
}
extern "C" fn set(bits: *mut u32, width: u32, rect: *mut Rect) {
    let r = unsafe { rect.read_unaligned() };
    log(0x0060_4180, bits.cast(), &[width, r.x0, r.z0, r.x1, r.z1]);
    unsafe {
        (*rect).x0 = 999;
    }
}
extern "C" fn clear(bits: *mut u32, width: u32, rect: *mut Rect) {
    let r = unsafe { rect.read_unaligned() };
    log(0x0060_4260, bits.cast(), &[width, r.x0, r.z0, r.x1, r.z1]);
    unsafe {
        (*rect).z1 = 999;
    }
}
extern "C" fn pixel(bits: *mut u32, width: u32, x: i32, z: i32) -> u64 {
    log(0x0060_3DD0, bits.cast(), &[width, x as u32, z as u32]);
    0x1234_5678_ABCD_EF01
}
#[test]
fn wire_edits_copy_bounds_before_width_call_and_reload_the_bitmap_after_it() {
    let _set = fake(0x0060_4180, set as *const () as usize);
    let _clear = fake(0x0060_4260, clear as *const () as usize);
    let mut w = World::new();
    let mut bits = [0u32; 8];
    w.city.bitmap = bits.as_mut_ptr();
    for entry in [fc4p_006eafd0, fc4p_006eb020] {
        let mut r = Rect {
            x0: 1,
            z0: 2,
            x1: 3,
            z1: 4,
        };
        w.city.source_rect = &mut r;
        unsafe { entry(w.object.ptr(), &r) };
        let addr = if entry as usize == fc4p_006eafd0 as *const () as usize {
            0x0060_4180
        } else {
            0x0060_4260
        };
        assert_eq!(
            r,
            Rect {
                x0: 500,
                z0: 2,
                x1: 3,
                z1: 4
            }
        );
        assert_eq!(
            calls(),
            [
                Call(0x174, w.city_ptr() as usize, vec![]),
                Call(addr, bits.as_ptr() as usize, vec![64, 1, 2, 3, 4])
            ]
        );
    }
}
#[test]
fn powered_query_delegates_without_clipping_and_preserves_both_result_registers() {
    let _pixel = fake(0x0060_3DD0, pixel as *const () as usize);
    let mut w = World::new();
    let mut bits = [0u32; 8];
    w.city.bitmap = bits.as_mut_ptr();
    assert_eq!(
        unsafe { fc4p_006eb070(w.object.ptr(), -9, i32::MAX) },
        0x1234_5678_ABCD_EF01
    );
    assert_eq!(
        calls(),
        [
            Call(0x174, w.city_ptr() as usize, vec![]),
            Call(
                0x0060_3DD0,
                bits.as_ptr() as usize,
                vec![64, (-9i32) as u32, i32::MAX as u32]
            )
        ]
    );
}
#[repr(C)]
struct Positioned {
    vt: *const usize,
    x: i32,
    z: i32,
}
extern "thiscall" fn position(p: *mut Positioned, x: *mut i32, z: *mut i32) {
    log(0x18, p.cast(), &[]);
    unsafe {
        x.write_unaligned((*p).x);
        z.write_unaligned((*p).z);
    }
}
#[test]
fn clamp_object_cell_handles_null_signed_edges_empty_city_and_aliased_outputs() {
    let mut w = World::new();
    let mut city = w.city_ptr();
    let _global = fake_data(0x00B4_3CEC, (&mut city as *mut *mut u8).cast());
    let mut vt = [0usize; 7];
    vt[6] = position as *const () as usize;
    let mut p = Positioned {
        vt: vt.as_ptr(),
        x: 0,
        z: 0,
    };
    let (mut x, mut z) = (123, 456);
    unsafe { fc4p_006eb280(core::ptr::null_mut(), &mut x, &mut z) };
    assert_eq!((x, z), (123, 456));
    assert!(calls().is_empty());
    for (px, pz, ex, ez) in [
        (-1, -2, 0, 0),
        (10, 20, 10, 20),
        (64, 32, 63, 31),
        (i32::MAX, i32::MIN, 63, 0),
    ] {
        p.x = px;
        p.z = pz;
        unsafe { fc4p_006eb280((&mut p as *mut Positioned).cast(), &mut x, &mut z) };
        assert_eq!((x, z), (ex, ez));
        assert_eq!(
            calls(),
            [
                Call(0x174, city as usize, vec![]),
                Call(0x178, city as usize, vec![]),
                Call(0x18, &p as *const Positioned as usize, vec![])
            ]
        );
    }
    w.city.width = 0;
    w.city.height = 0;
    unsafe { fc4p_006eb280((&mut p as *mut Positioned).cast(), &mut x, &mut z) };
    assert_eq!((x, z), (-1, -1));
    calls();
    w.city.width = 64;
    w.city.height = 32;
    p.x = 63;
    p.z = 99;
    let out = &mut x as *mut i32;
    unsafe { fc4p_006eb280((&mut p as *mut Positioned).cast(), out, out) };
    assert_eq!(x, 31);
}
extern "thiscall" fn expand_fake(this: *mut u8, r: *mut Rect) {
    let rect = unsafe { r.read_unaligned() };
    log(0x006E_AE20, this, &[rect.x0, rect.z0, rect.x1, rect.z1]);
    unsafe {
        r.write_unaligned(Rect {
            x0: 2,
            z0: 3,
            x1: 8,
            z1: 9,
        })
    };
}
extern "thiscall" fn wire_fake(this: *mut u8, r: *mut Rect) {
    let rect = unsafe { r.read_unaligned() };
    log(0x006E_AFD0, this, &[rect.x0, rect.z0, rect.x1, rect.z1]);
}
extern "thiscall" fn dirty_fake(this: *mut u8, r: *mut Rect) -> u32 {
    let rect = unsafe { r.read_unaligned() };
    log(0x006E_B230, this, &[rect.x0, rect.z0, rect.x1, rect.z1]);
    0xABCD_EF89
}
#[repr(C)]
struct Zones {
    vt: *const usize,
    result: u32,
}
extern "thiscall" fn zone(this: *mut Zones, x: u32, z: u32) -> u32 {
    log(0x30, this.cast(), &[x, z]);
    unsafe { (*this).result }
}
#[test]
fn changed_cell_uses_low_zone_byte_and_original_origin_in_call_order() {
    let _expand = fake(0x006E_AE20, expand_fake as *const () as usize);
    let _wire = fake(0x006E_AFD0, wire_fake as *const () as usize);
    let _dirty = fake(0x006E_B230, dirty_fake as *const () as usize);
    let mut w = World::new();
    let mut vt = [0usize; 13];
    vt[12] = zone as *const () as usize;
    let mut zones = Zones {
        vt: vt.as_ptr(),
        result: 0,
    };
    w.sim().set_zones((&mut zones as *mut Zones).cast());
    for result in [0, 0x100, 1, 0xABCD_EF80] {
        zones.result = result;
        assert_eq!(unsafe { fc4p_006eb350(w.object.ptr(), 4, 5, 6, 7) }, 0xABCD_EF01);
        let mut expected = vec![
            Call(0x006E_AE20, w.object.ptr() as usize, vec![4, 5, 6, 7]),
            Call(0x30, &zones as *const Zones as usize, vec![4, 5]),
        ];
        if result as u8 != 0 {
            expected.push(Call(0x006E_AFD0, w.object.ptr() as usize, vec![2, 3, 8, 9]));
        }
        expected.push(Call(0x006E_B230, w.object.ptr() as usize, vec![2, 3, 8, 9]));
        assert_eq!(calls(), expected);
    }
}
#[repr(C)]
struct Lots {
    vt: *const usize,
    lot: *mut u8,
    yes: u32,
    needs: u32,
}
extern "thiscall" fn lookup(this: *mut Lots, occupant: *mut u8) -> u64 {
    log(0x38, this.cast(), &[occupant as u32]);
    (0x1234_5678u64 << 32) | unsafe { (*this).lot as u32 as u64 }
}
extern "thiscall" fn yes(this: *mut Lots) -> u64 {
    log(0x108, this.cast(), &[]);
    (0x2345_6789u64 << 32) | u64::from(unsafe { (*this).yes })
}
extern "thiscall" fn needs(this: *mut Lots) -> u64 {
    log(0x104, this.cast(), &[]);
    (0x3456_789Au64 << 32) | u64::from(unsafe { (*this).needs })
}
#[test]
fn occupant_power_uses_global_lots_low_flag_bytes_and_short_circuits() {
    let mut vt = [0usize; 0x10C / 4];
    vt[0x38 / 4] = lookup as *const () as usize;
    vt[0x108 / 4] = yes as *const () as usize;
    vt[0x104 / 4] = needs as *const () as usize;
    let mut lots = Lots {
        vt: vt.as_ptr(),
        lot: core::ptr::null_mut(),
        yes: 0,
        needs: 0,
    };
    let p = (&mut lots as *mut Lots).cast::<u8>();
    let mut global = p;
    let _global = fake_data(0x00B4_3D08, (&mut global as *mut *mut u8).cast());
    let occupant = 0x6789usize as *mut u8;
    assert_eq!(
        unsafe { fc4p_006eb3d0(core::ptr::null_mut(), occupant) },
        0x1234_5678_0000_0000
    );
    assert_eq!(calls(), [Call(0x38, p as usize, vec![occupant as u32])]);
    unsafe {
        (*p.cast::<Lots>()).lot = p;
    }
    for (powered, need, answer, edx, slots) in [
        (1, 1, 1, 0x2345_6789, vec![0x108]),
        (0x100, 1, 0, 0x3456_789A, vec![0x108, 0x104]),
        (0, 0x100, 1, 0x3456_789A, vec![0x108, 0x104]),
        (0, 0, 1, 0x3456_789A, vec![0x108, 0x104]),
    ] {
        unsafe {
            (*p.cast::<Lots>()).yes = powered;
            (*p.cast::<Lots>()).needs = need;
        }
        assert_eq!(
            unsafe { fc4p_006eb3d0(core::ptr::null_mut(), occupant) },
            (u64::from(edx as u32) << 32) | answer
        );
        let mut expected = vec![Call(0x38, p as usize, vec![occupant as u32])];
        expected.extend(slots.into_iter().map(|s| Call(s, p as usize, vec![])));
        assert_eq!(calls(), expected);
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn lookup_keeps_edx(_this: *mut Lots, _occupant: *mut u8) -> u64 {
    core::arch::naked_asm!("mov eax, [ecx + 4]", "ret 4");
}
#[unsafe(naked)]
unsafe extern "thiscall" fn low_power_flag(_this: *mut Lots) -> u64 {
    core::arch::naked_asm!("mov al, [ecx + 8]", "ret");
}
#[unsafe(naked)]
unsafe extern "thiscall" fn low_needs_flag(_this: *mut Lots) -> u64 {
    core::arch::naked_asm!("mov al, [ecx + 12]", "ret");
}
#[test]
fn occupant_query_supplies_edx_when_virtual_methods_leave_it_alone() {
    let mut vt = [0usize; 0x10C / 4];
    vt[0x38 / 4] = lookup_keeps_edx as *const () as usize;
    vt[0x108 / 4] = low_power_flag as *const () as usize;
    vt[0x104 / 4] = low_needs_flag as *const () as usize;
    let mut lots = Lots {
        vt: vt.as_ptr(),
        lot: core::ptr::null_mut(),
        yes: 1,
        needs: 0,
    };
    let p = (&mut lots as *mut Lots).cast::<u8>();
    let mut global = p;
    let _global = fake_data(0x00B4_3D08, (&mut global as *mut *mut u8).cast());
    let occupant = 0x6789usize as *mut u8;
    assert_eq!(
        unsafe { fc4p_006eb3d0(core::ptr::null_mut(), occupant) },
        0x0000_6789_0000_0000
    );
    unsafe {
        (*p.cast::<Lots>()).lot = p;
    }
    assert_eq!(
        unsafe { fc4p_006eb3d0(core::ptr::null_mut(), occupant) },
        0x0000_6789_0000_0001
    );
    unsafe {
        (*p.cast::<Lots>()).yes = 0;
    }
    assert_eq!(
        unsafe { fc4p_006eb3d0(core::ptr::null_mut(), occupant) },
        ((vt.as_ptr() as u64) << 32) | 1
    );
}

#[test]
fn clamp_cell_uses_stdcall_and_pops_all_three_argument_words() {
    use crate::abi::{call_raw, Conv, RawCall};
    let (mut x, mut z) = (123i32, 456i32);
    let args = [0, &mut x as *mut i32 as u32, &mut z as *mut i32 as u32];
    let mut call = RawCall::new(Conv::Stdcall, fc4p_006eb280 as *const () as usize, 0x1234, &args);
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((x, z), (123, 456));
    assert!(calls().is_empty());
}

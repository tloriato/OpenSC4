// SPDX-License-Identifier: GPL-3.0-or-later
//! Published grid queries, producer information and counters.

use super::PlumbingSim;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;

layout! {
    /// A producer-list node includes two links followed by the producer record.
    pub struct ProducerNode {
        0x00 => next / set_next: *mut u8,
        0x24 => maximum_capacity / set_maximum_capacity: u32,
        0x28 => production / set_production: u32,
        0x2C => efficiency_bits / set_efficiency_bits: u32,
        0x30 => polluted / set_polluted: u8,
        0x34 => occupant / set_occupant: *mut u8,
    }
}

game_fn! {
    fn property_u32(holder: *mut u8, id: u32, out: *mut u32) -> u64 = "C" 0x005F_D480;
}

port! {
    /// Calls the published grid in cell coordinates, and retains only bit zero of its low result byte.
    #[port(address = 0x006D_A460, owner = "plumbing", name = "cSC4PlumbingSimulator::IsCellWatered")]
    pub unsafe extern "thiscall" fn fc4p_006da460(this: *mut u8, x: i32, z: i32) -> u64 {
        // SAFETY: the game supplies the live simulator, its grid and the grid's cell getter.
        unsafe {
            let grid = PlumbingSim::from_ptr(this).watered_grid();
            let args = [x as u32, z as u32];
            let mut c = RawCall::new(Conv::Thiscall, vslot(grid, 0x1C), grid as u32, &args);
            c.eax = grid.cast::<u32>().read_unaligned();
            c.edx = x as u32;
            call_raw(&mut c);
            (u64::from(c.edx) << 32) | u64::from(c.eax & 0xFFFF_FF01)
        }
    }
}

port! {
    /// Reads a connection's cell and clamps x then z to zero, then to the city's last cell.
    /// A null connection makes no calls or writes. City dimensions are queried before the connection.
    #[port(address = 0x006D_A480, owner = "plumbing", name = "ClampConnectionCell", entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006da480(this: *mut u8, [entry_eax: u32], connection: *mut u8, x: *mut i32, z: *mut i32) -> u32 {
        if connection.is_null() { return entry_eax; }
        // SAFETY: the caller supplies city/connection interfaces and writable coordinate words.
        unsafe {
            let sim = PlumbingSim::from_ptr(this);
            let max_x = vcall!(sim.city(), 0x174 => fn() -> i32).wrapping_sub(1);
            let max_z = vcall!(sim.city(), 0x178 => fn() -> i32).wrapping_sub(1);
            vcall!(connection, 0x18 => fn(*mut i32, *mut i32), x, z);
            if x.read_unaligned() < 0 { x.write_unaligned(0); }
            if z.read_unaligned() < 0 { z.write_unaligned(0); }
            if x.read_unaligned() > max_x { x.write_unaligned(max_x); }
            if z.read_unaligned() > max_z { z.write_unaligned(max_z); }
            z as u32
        }
    }
}

port! {
    /// Finds the first matching occupant. Optional outputs are written in production, capacity,
    /// efficiency, pollution and consumption order. Missing occupants leave every output untouched.
    #[port(address = 0x006D_ADB0, owner = "plumbing", name = "cSC4PlumbingSimulator::GetWaterProducerInfo")]
    pub unsafe extern "thiscall" fn fc4p_006dadb0(this: *mut u8, occupant: *mut u8, production: *mut u32,
        capacity: *mut u32, efficiency: *mut u32, polluted: *mut u8, consumption: *mut u32) -> u64 {
        // SAFETY: the game supplies the circular list and any nonnull output destinations.
        unsafe {
            let header = PlumbingSim::from_ptr(this).producers();
            let mut p = header.cast::<*mut u8>().read_unaligned();
            while p != header {
                let node = ProducerNode::from_ptr(p);
                if node.occupant() == occupant {
                    if !production.is_null() { production.write_unaligned(node.production()); }
                    if !capacity.is_null() { capacity.write_unaligned(node.maximum_capacity()); }
                    if !efficiency.is_null() { efficiency.write_unaligned(node.efficiency_bits()); }
                    if !polluted.is_null() { polluted.write(node.polluted()); }
                    if consumption.is_null() {
                        return (u64::from(polluted as u32) << 32) | 1;
                    }
                    let mut c = RawCall::new(Conv::Thiscall, vslot(occupant, 0x18), occupant as u32, &[]);
                    c.eax = consumption as u32;
                    c.edx = occupant.cast::<u32>().read_unaligned();
                    call_raw(&mut c);
                    let result = property_u32(c.eax as *mut u8, 0x08D9_763D, consumption);
                    return (result & !0xFF) | 1;
                }
                p = node.next();
            }
            (u64::from(header as u32) << 32) | u64::from(header as u32 & !0xFF)
        }
    }
}

port! {
    /// Returns the low word of the published exported-water total, preserving EDX.
    #[port(address = 0x006D_C680, owner = "plumbing", name = "cSC4PlumbingSimulator::GetWaterExported", naked)]
    pub unsafe extern "C" fn fc4p_006dc680() {
        core::arch::naked_asm!("mov eax, [ecx + 0x128]", "ret");
    }
}
port! {
    /// Returns the pipe count, preserving EDX.
    #[port(address = 0x006D_C690, owner = "plumbing", name = "cSC4PlumbingSimulator::GetNumPipes", naked)]
    pub unsafe extern "C" fn fc4p_006dc690() {
        core::arch::naked_asm!("mov eax, [ecx + 0x174]", "ret");
    }
}
port! {
    /// Returns the distressed-pipe count, preserving EDX.
    #[port(address = 0x006D_C6A0, owner = "plumbing", name = "cSC4PlumbingSimulator::GetNumDistressedPipes", naked)]
    pub unsafe extern "C" fn fc4p_006dc6a0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x178]", "ret");
    }
}
port! {
    /// Returns the published watered-building count, preserving EDX.
    #[port(address = 0x006D_C6B0, owner = "plumbing", name = "cSC4PlumbingSimulator::GetNumWateredBuildings", naked)]
    pub unsafe extern "C" fn fc4p_006dc6b0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x18c]", "ret");
    }
}
port! {
    /// Returns the wrapping difference of total and watered building counts, preserving EDX.
    #[port(address = 0x006D_C6C0, owner = "plumbing", name = "cSC4PlumbingSimulator::GetNumUnwateredBuildings")]
    pub unsafe extern "fastcall" fn fc4p_006dc6c0(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the game supplies the simulator.
        let sim = unsafe { PlumbingSim::from_ptr(this) };
        (u64::from(entry_edx) << 32) | u64::from(sim.buildings().wrapping_sub(sim.watered_buildings()))
    }
}
port! {
    /// Counts every node of the circular producer list, leaving its sentinel in EDX.
    #[port(address = 0x006D_C710, owner = "plumbing", name = "cSC4PlumbingSimulator::GetNumWaterSources")]
    pub unsafe extern "thiscall" fn fc4p_006dc710(this: *mut u8) -> u64 {
        // SAFETY: the game's circular producer list ends at its live sentinel.
        unsafe {
            let header = PlumbingSim::from_ptr(this).producers();
            let mut node = header.cast::<*mut u8>().read_unaligned();
            let mut count = 0u32;
            while node != header {
                node = node.cast::<*mut u8>().read_unaligned();
                count = count.wrapping_add(1);
            }
            (u64::from(header as u32) << 32) | u64::from(count)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006da460::PORT,
    fc4p_006da480::PORT,
    fc4p_006dadb0::PORT,
    fc4p_006dc680::PORT,
    fc4p_006dc690::PORT,
    fc4p_006dc6a0::PORT,
    fc4p_006dc6b0::PORT,
    fc4p_006dc6c0::PORT,
    fc4p_006dc710::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, Object};
    use std::cell::RefCell;
    thread_local! { static CALLS: RefCell<Vec<(u32,u32,u32)>> = const { RefCell::new(Vec::new()) }; }
    fn take() -> Vec<(u32, u32, u32)> {
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
    }
    fn note(a: u32, b: u32, c: u32) {
        CALLS.with(|v| v.borrow_mut().push((a, b, c)));
    }

    #[repr(C)]
    struct Grid {
        vt: *const usize,
        value: u32,
    }
    extern "fastcall" fn cell(this: *mut Grid, edx: u32, x: i32, z: i32) -> u64 {
        assert_eq!(edx, x as u32);
        note(1, x as u32, z as u32);
        (0x7654_3210u64 << 32) | u64::from(unsafe { (*this).value })
    }
    #[test]
    fn published_water_bit_uses_cell_coordinates_and_preserves_upper_bytes() {
        // Published-grid query vectors from basic_flood, shortage, and grid_values_and_swap.
        let mut vt = [0usize; 8];
        vt[7] = cell as *const () as usize;
        let mut a = Grid {
            vt: vt.as_ptr(),
            value: 0,
        };
        let mut b = Grid {
            vt: vt.as_ptr(),
            value: 0xFFFF_FFFF,
        };
        let mut object = Object::new(0x1C8);
        let sim = unsafe { PlumbingSim::from_ptr(object.ptr()) };
        for value in [0, 1, 2, 3, 4, 8, 15, 255, 0xABCD_EFFE] {
            a.value = value;
            sim.set_watered_grid((&mut a as *mut Grid).cast());
            let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x1C8) }.to_vec();
            assert_eq!(
                unsafe { fc4p_006da460(object.ptr(), -1, i32::MAX) },
                0x7654_3210_0000_0000 | u64::from(value & 0xFFFF_FF01)
            );
            assert_eq!(take(), [(1, u32::MAX, i32::MAX as u32)]);
            assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x1C8) }, before);
        }
        sim.set_watered_grid((&mut b as *mut Grid).cast());
        assert_eq!(unsafe { fc4p_006da460(object.ptr(), 12, 5) }, 0x7654_3210_FFFF_FF01);
        assert_eq!(take(), [(1, 12, 5)]);
    }

    #[repr(C)]
    struct City {
        vt: *const usize,
        x: i32,
        z: i32,
        simulator: *mut u8,
        replacement: *mut u8,
    }
    extern "thiscall" fn width(this: *mut City) -> i32 {
        note(2, 0, 0);
        unsafe {
            if !(*this).replacement.is_null() {
                PlumbingSim::from_ptr((*this).simulator).set_city((*this).replacement);
            }
            (*this).x
        }
    }
    extern "thiscall" fn depth(this: *mut City) -> i32 {
        note(3, 0, 0);
        unsafe { (*this).z }
    }
    #[repr(C)]
    struct Connection {
        vt: *const usize,
        x: i32,
        z: i32,
    }
    extern "thiscall" fn position(this: *mut Connection, x: *mut i32, z: *mut i32) {
        note(4, 0, 0);
        unsafe {
            x.write_unaligned((*this).x);
            z.write_unaligned((*this).z);
        }
    }
    #[test]
    fn connection_clamping_queries_dimensions_first_and_handles_aliases() {
        let mut object = Object::new(0x1C8);
        let mut vt = [0usize; 95];
        vt[93] = width as *const () as usize;
        vt[94] = depth as *const () as usize;
        let mut cvt = [0usize; 7];
        cvt[6] = position as *const () as usize;
        let mut city = City {
            vt: vt.as_ptr(),
            x: 64,
            z: 32,
            simulator: object.ptr(),
            replacement: core::ptr::null_mut(),
        };
        let sim = unsafe { PlumbingSim::from_ptr(object.ptr()) };
        sim.set_city((&mut city as *mut City).cast());
        for (cx, cz, w, d, want) in [
            (-1, -2, 64, 32, (0, 0)),
            (64, 32, 64, 32, (63, 31)),
            (5, 9, 64, 32, (5, 9)),
            (-5, -9, 0, 0, (-1, -1)),
            (i32::MAX, 1, i32::MIN, 2, (i32::MAX, 1)),
        ] {
            city.x = w;
            city.z = d;
            sim.set_city((&mut city as *mut City).cast());
            let mut conn = Connection {
                vt: cvt.as_ptr(),
                x: cx,
                z: cz,
            };
            let (mut x, mut z) = (999, 999);
            let args = [
                &mut conn as *mut Connection as u32,
                &mut x as *mut i32 as u32,
                &mut z as *mut i32 as u32,
            ];
            assert_eq!(
                unsafe { call_thiscall_with_eax(fc4p_006da480 as *const () as usize, object.ptr(), &args, 0) }.0,
                &mut z as *mut i32 as u32
            );
            assert_eq!((x, z), want);
            assert_eq!(take(), [(2, 0, 0), (3, 0, 0), (4, 0, 0)]);
        }
        let mut conn = Connection {
            vt: cvt.as_ptr(),
            x: 99,
            z: -3,
        };
        let mut xy = 99i32;
        city.x = 64;
        city.z = 32;
        sim.set_city((&mut city as *mut City).cast());
        let p = &mut xy as *mut i32;
        unsafe { fc4p_006da480::body(object.ptr(), 0, (&mut conn as *mut Connection).cast(), p, p) };
        assert_eq!(xy, 0);
        assert_eq!(take(), [(2, 0, 0), (3, 0, 0), (4, 0, 0)]);
        let mut next = City {
            vt: vt.as_ptr(),
            x: 1,
            z: 7,
            simulator: object.ptr(),
            replacement: core::ptr::null_mut(),
        };
        city.replacement = (&mut next as *mut City).cast();
        sim.set_city((&mut city as *mut City).cast());
        conn.z = 40;
        let (mut x, mut z) = (0, 0);
        unsafe { fc4p_006da480::body(object.ptr(), 0, (&mut conn as *mut Connection).cast(), &mut x, &mut z) };
        assert_eq!((x, z), (63, 6));
        assert_eq!(take(), [(2, 0, 0), (3, 0, 0), (4, 0, 0)]);
        assert_eq!(
            unsafe {
                fc4p_006da480::body(
                    core::ptr::null_mut(),
                    0xABCD_EF12,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                )
            },
            0xABCD_EF12
        );
        assert!(take().is_empty());
    }

    extern "thiscall" fn holder(_this: *mut u8) -> u32 {
        note(5, 0, 0);
        0x1122_3344
    }
    extern "C" fn consumption(holder: *mut u8, id: u32, out: *mut u32) -> u64 {
        note(6, holder as u32, id);
        unsafe { out.write(300) };
        0x7654_3210_AABB_CCFE
    }
    #[test]
    fn producer_information_outputs_are_optional_and_use_the_first_match() {
        // Production, maximum capacity, efficiency and pollution from pump_capacity and pollution_power_and_condition.
        let mut object = Object::new(0x1C8);
        let sim = unsafe { PlumbingSim::from_ptr(object.ptr()) };
        let mut header = [0u32; 2];
        let h = header.as_mut_ptr().cast::<u8>();
        let mut a = Object::new(0x38);
        let mut b = Object::new(0x38);
        let na = unsafe { ProducerNode::from_ptr(a.ptr()) };
        let nb = unsafe { ProducerNode::from_ptr(b.ptr()) };
        let mut vt = [0usize; 7];
        vt[6] = holder as *const () as usize;
        let mut occupant = [vt.as_ptr() as usize];
        let o = occupant.as_mut_ptr().cast::<u8>();
        sim.set_producers(h);
        unsafe { h.cast::<u32>().write(a.ptr() as u32) };
        na.set_next(b.ptr());
        nb.set_next(h);
        na.set_occupant(o);
        nb.set_occupant(o);
        na.set_production(300);
        na.set_maximum_capacity(1000);
        na.set_efficiency_bits(0.29999998f32.to_bits());
        na.set_polluted(0x80);
        nb.set_production(999);
        let _fake = fake(0x005F_D480, consumption as *const () as usize);
        for mask in 0..32 {
            let mut outputs = [0xDEAD_BEEFu32; 5];
            let mut pollution = 0xEEu8;
            let p = outputs.as_mut_ptr();
            let arg = |i: usize| {
                if mask & (1 << i) != 0 {
                    p.wrapping_add(i)
                } else {
                    core::ptr::null_mut()
                }
            };
            let polluted = if mask & 8 != 0 {
                &mut pollution as *mut u8
            } else {
                core::ptr::null_mut()
            };
            let r = unsafe { fc4p_006dadb0(object.ptr(), o, arg(0), arg(1), arg(2), polluted, arg(4)) };
            for (i, want) in [(0, 300), (1, 1000), (2, 0.29999998f32.to_bits()), (4, 300)] {
                assert_eq!(outputs[i], if mask & (1 << i) != 0 { want } else { 0xDEAD_BEEF });
            }
            assert_eq!(pollution, if mask & 8 != 0 { 0x80 } else { 0xEE });
            if mask & 16 != 0 {
                assert_eq!(r, 0x7654_3210_AABB_CC01);
                assert_eq!(take(), [(5, 0, 0), (6, 0x1122_3344, 0x08D9_763D)]);
            } else {
                assert_eq!(r, (u64::from(polluted as u32) << 32) | 1);
                assert!(take().is_empty());
            }
        }
        let mut out = 0xCAFE_BABE;
        let r = unsafe {
            fc4p_006dadb0(
                object.ptr(),
                0x1234usize as *mut u8,
                &mut out,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        assert_eq!(r, (u64::from(h as u32) << 32) | u64::from(h as u32 & !0xFF));
        assert_eq!(out, 0xCAFE_BABE);
        assert!(take().is_empty());
        // Earlier outputs can change the record words read by later outputs.
        na.set_maximum_capacity(1000);
        na.set_efficiency_bits(0.75f32.to_bits());
        let mut efficiency = 0u32;
        let result = unsafe {
            fc4p_006dadb0(
                object.ptr(),
                o,
                a.ptr().add(0x24).cast(),
                a.ptr().add(0x2C).cast(),
                &mut efficiency,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        assert_eq!(result, 1);
        assert_eq!(na.maximum_capacity(), 300);
        assert_eq!(efficiency, 300);
        assert!(take().is_empty());
    }

    #[test]
    fn counters_keep_edx_and_wrapping_building_difference() {
        let mut object = Object::new(0x1C8);
        let sim = unsafe { PlumbingSim::from_ptr(object.ptr()) };
        for value in [0, 1, 0x8000_0000, u32::MAX] {
            sim.set_exported(value);
            sim.set_pipes(value);
            sim.set_distressed(value);
            sim.set_watered_buildings(value);
            sim.set_buildings(value.wrapping_sub(1));
            let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x1C8) }.to_vec();
            for (f, want) in [
                (fc4p_006dc680 as *const (), value),
                (fc4p_006dc690 as *const (), value),
                (fc4p_006dc6a0 as *const (), value),
                (fc4p_006dc6b0 as *const (), value),
                (fc4p_006dc6c0 as *const (), u32::MAX),
            ] {
                let mut c = RawCall::new(Conv::Thiscall, f as usize, object.ptr() as u32, &[]);
                c.edx = 0x1234_5678;
                unsafe { call_raw(&mut c) };
                assert_eq!((c.eax, c.edx), (want, 0x1234_5678));
            }
            assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x1C8) }, before);
        }
    }

    #[test]
    fn counts_sources_by_links_including_empty_and_duplicate_occupants() {
        // Source-count vectors from consumers_and_producers: insertion gives one, removal gives zero.
        let mut object = Object::new(0x1C8);
        let sim = unsafe { PlumbingSim::from_ptr(object.ptr()) };
        let mut links = [0u32; 5];
        let base = links.as_mut_ptr();
        sim.set_producers(base.cast());
        for count in 0..5 {
            for (i, link) in links.iter_mut().enumerate().take(count + 1) {
                *link = base.wrapping_add(if i == count { 0 } else { i + 1 }) as u32;
            }
            assert_eq!(
                unsafe { fc4p_006dc710(object.ptr()) },
                (u64::from(base as u32) << 32) | count as u64
            );
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Foundation creation and the foundation, wall, and building exemplar loaders.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::x87::F80;
use core::mem::MaybeUninit;
use core::ptr::{null_mut, read_unaligned};

game_fn! { fn allocate_foundation(size: u32) -> *mut u8 = "C" 0x005D_6220; }
game_fn! { fn construct_foundation(this: *mut u8) -> *mut u8 = "thiscall" 0x005D_6440; }
game_fn! { fn foundation_cells(this: *mut u8, rect: *mut f32, cells: *mut i32) = "thiscall" 0x0069_5610; }
game_fn! { #[allow(dead_code)] fn property_float(holder: *mut u8, id: u32, out: *mut f32) -> u64 = "C" 0x005F_D4F0; }
game_data! { static half_double: f64 = 0x00A9_2D28; }
game_data! { static negative_half: f32 = 0x00A9_49FC; }
game_data! { static half_float: f32 = 0x00AB_0A04; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static building_buckets: *mut *mut u8 = 0x00B4_854C; }
game_data! { static building_buckets_end: *mut *mut u8 = 0x00B4_8550; }
game_data! { static foundation_buckets: *mut *mut u8 = 0x00B4_85BC; }
game_data! { static foundation_buckets_end: *mut *mut u8 = 0x00B4_85C0; }
game_data! { static wall_buckets: *mut *mut u8 = 0x00B4_85D0; }
game_data! { static wall_buckets_end: *mut *mut u8 = 0x00B4_85D4; }

#[derive(Clone, Copy)]
struct Registers {
    eax: u32,
    edx: u32,
}

impl Registers {
    fn bits(self) -> u64 {
        (u64::from(self.edx) << 32) | u64::from(self.eax)
    }
    fn from_bits(bits: u64) -> Self {
        Self {
            eax: bits as u32,
            edx: (bits >> 32) as u32,
        }
    }
    fn low_byte(self, byte: u8) -> u64 {
        (self.bits() & !255) | u64::from(byte)
    }
}

unsafe fn table(this: *mut u8) -> u32 {
    // SAFETY: all callers supply a live interface.
    unsafe { read_unaligned(this.cast()) }
}

/// Supplies the scratch registers too: several interface methods replace only AL or preserve EDX.
unsafe fn invoke(this: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> Registers {
    // SAFETY: the caller supplies the matching interface, slot and stack arguments.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(this, slot), this as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        Registers {
            eax: call.eax,
            edx: call.edx,
        }
    }
}

port! {
    /// Creates a foundation, loads its properties, queries missing altitudes, configures it and inserts it.
    /// The position's height is written back. A NaN base requests the area's lowest altitude.
    /// The final Release supplies EAX's upper bytes and EDX; the insertion result supplies AL.
    #[port(address = 0x0069_5AF0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::AddNewFoundationOccupant", naked)]
    pub unsafe extern "C" fn fc4p_00695af0() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {body}", "ret 28", body = sym add_foundation);
    }
}

/// # Safety
/// The entry supplies the simulator and its seven writable argument words.
unsafe extern "fastcall" fn add_foundation(this: *mut u8, args: *mut u32) -> u64 {
    // SAFETY: the game's simulator, caller-owned position, and allocated foundation interfaces.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        let allocation = allocate_foundation(0x5C);
        let occupant = if allocation.is_null() {
            null_mut()
        } else {
            construct_foundation(allocation)
        };
        let mut r = invoke(occupant, 4, &[], table(occupant), 0);
        let kind = *args;
        r = invoke(occupant, 0x14, &[kind], r.eax, table(occupant));
        r = invoke(this, 0x68, &[occupant as u32, kind], table(this), r.edx);
        let mut ok = 0;
        if r.eax as u8 != 0 {
            let position = *args.add(2) as *mut f32;
            let size_x = f32::from_bits(*args.add(3));
            let size_z = f32::from_bits(*args.add(4));
            let hx = F80::from_f32(size_x) * F80::from_f64(*half_double());
            let x0 = F80::from_f32(*position) - hx;
            let hz = F80::from_f32(size_z) * F80::from_f64(*half_double());
            let z0 = (F80::from_f32(*position.add(2)) - hz).to_f32();
            let x1 = (F80::from_f32(*position) + hx).to_f32();
            let z1 = (F80::from_f32(*position.add(2)) + hz).to_f32();
            let mut rect = [
                (x0 - F80::from_f32(*negative_half())).to_f32(),
                (F80::from_f32(z0) - F80::from_f32(*negative_half())).to_f32(),
                (F80::from_f32(x1) - F80::from_f32(*half_float())).to_f32(),
                (F80::from_f32(z1) - F80::from_f32(*half_float())).to_f32(),
            ];
            let mut cells = MaybeUninit::<[i32; 4]>::uninit();
            foundation_cells(this, rect.as_mut_ptr(), cells.as_mut_ptr().cast());
            if F80::from_f32(*position.add(1)).compare(F80::from_f32(*zero())).less() {
                *position.add(1) = vcall_st0!(sim.lot_developer(), 0x44, cells.as_mut_ptr(), 0u32, 1u32).to_f32();
            }
            let base = f32::from_bits(*args.add(6));
            let comparison = F80::from_f32(base).compare(F80::from_f32(*zero()));
            let bottom = if comparison.less() || comparison.unordered() {
                vcall_st0!(sim.lot_developer(), 0x44, cells.as_mut_ptr(), 1u32, 1u32)
            } else {
                F80::from_f32(base)
            };
            let height = F80::from_f32(*position.add(1)) - bottom;
            *args.add(6) = 0.5f32.to_bits();
            let stored_height = height.to_f32();
            *args = stored_height.to_bits();
            let height = if height.compare(F80::from_f32(*half_float())).less() {
                0.5
            } else {
                stored_height
            };
            let mut size = [f32::from_bits(*args.add(3)), height, f32::from_bits(*args.add(4))];
            let primary = occupant.add(4);
            r = invoke(primary, 0x24, &[position as u32], size_z.to_bits(), table(primary));
            invoke(occupant, 0x20, &[size.as_mut_ptr() as u32], table(occupant), r.edx);
            let usage = *args.add(1);
            invoke(occupant, 0x28, &[usage], usage, table(occupant));
            let orientation = *args.add(5);
            r = invoke(occupant, 0x30, &[orientation], orientation, table(occupant));
            r = invoke(primary, 0x0C, &[], r.eax, table(primary));
            r = invoke(this, 0x60, &[occupant as u32], table(this), r.edx);
            ok = r.eax as u8;
        }
        invoke(occupant, 8, &[], r.eax, table(occupant)).low_byte(ok)
    }
}

/// Finds the resource key, retaining the quotient and the final link in the scratch registers.
unsafe fn find_key(begin: *mut *mut u8, end: *mut *mut u8, key: u32) -> (*mut u8, Registers) {
    // SAFETY: the game's nonempty bucket vector and readable chains.
    unsafe {
        let count = ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32;
        let (quotient, bucket): (u32, u32);
        core::arch::asm!("div {count}", count = in(reg) count, inout("eax") key => quotient,
            inout("edx") 0u32 => bucket, options(nomem, nostack));
        let mut node = read_unaligned(begin.wrapping_add(bucket as usize));
        while !node.is_null() {
            if read_unaligned(node.add(4).cast::<u32>()) == key {
                break;
            }
            node = read_unaligned(node.cast());
        }
        (
            node,
            Registers {
                eax: quotient,
                edx: node as u32,
            },
        )
    }
}

#[derive(Clone, Copy)]
enum Loader {
    Foundation,
    Wall,
    Building,
}

unsafe fn load(this: *mut u8, args: *mut u32, loader: Loader) -> u64 {
    // SAFETY: the three original interfaces share the property and resource contracts used here.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        let occupant = *args as *mut u8;
        let mut r = invoke(occupant, 0x0C, &[], table(occupant), 0);
        let properties = r.eax as *mut u8;
        r = invoke(properties, 0x18, &[], r.eax, table(properties));
        let holder = r.eax as *mut u8;
        *args = 0;
        r = invoke(properties, 0, &[0x0AC2_B5F7, args as u32], table(properties), r.edx);
        let mut kind = *args.add(1);
        if kind == 0 {
            r = invoke(
                occupant,
                if matches!(loader, Loader::Wall) { 0x2C } else { 0x10 },
                &[],
                r.eax,
                table(occupant),
            );
            kind = r.eax;
        }
        let (begin, end) = match loader {
            Loader::Foundation => (*foundation_buckets(), *foundation_buckets_end()),
            Loader::Wall => (*wall_buckets(), *wall_buckets_end()),
            Loader::Building => (*building_buckets(), *building_buckets_end()),
        };
        let (node, found) = find_key(begin, end, kind);
        r = found;
        let mut ok = u8::from(matches!(loader, Loader::Building));
        if !node.is_null() {
            let mut resource = MaybeUninit::<*mut u8>::uninit();
            let resource_out = if matches!(loader, Loader::Foundation) {
                resource.as_mut_ptr()
            } else {
                args.add(1).cast()
            };
            r = invoke(
                sim.resources(),
                0x0C,
                &[node.add(8) as u32, 0xA521_60F5, resource_out as u32, 0, 0],
                table(sim.resources()),
                node.add(8) as u32,
            );
            if r.eax as u8 != 0 {
                let resource = resource_out.read();
                let target = *args as *mut u8;
                r = invoke(target, 0x0C, &[resource as u32], resource as u32, table(target));
                match loader {
                    Loader::Foundation => {
                        *args.add(1) = 0;
                        let words = [holder as u32, 0x88FC_C3E3, args.add(1) as u32];
                        let mut call =
                            RawCall::new(Conv::Cdecl, crate::abi::target(0x005F_D4F0), args.add(1) as u32, &words);
                        call.eax = r.eax;
                        call.edx = holder as u32;
                        call_raw(&mut call);
                        r = Registers::from_bits((u64::from(call.edx) << 32) | u64::from(call.eax));
                        if r.eax as u8 != 0 {
                            let maximum = f32::from_bits(*args.add(1));
                            r = invoke(occupant, 0x1C, &[maximum.to_bits()], table(occupant), r.edx);
                        }
                    }
                    Loader::Wall => {}
                    Loader::Building => {
                        r = invoke(properties, 0x18, &[], r.eax, table(properties));
                        let updated_holder = r.eax as *mut u8;
                        r = invoke(updated_holder, 0x24, &[0x2781_2810], r.eax, table(updated_holder));
                        if r.eax != 0 {
                            let property = r.eax as *mut u8;
                            r = invoke(property, 0x14, &[], r.eax, table(property));
                            let variant = r.eax as *mut u8;
                            r = invoke(variant, 0x1BC, &[], r.eax, table(variant));
                            let mut origin = [0.0f32; 3];
                            r = invoke(
                                occupant,
                                0x20,
                                &[origin.as_mut_ptr() as u32, r.eax],
                                origin.as_mut_ptr() as u32,
                                table(occupant),
                            );
                        }
                    }
                }
                let released = if matches!(loader, Loader::Foundation) {
                    resource
                } else {
                    *args.add(1) as *mut u8
                };
                r = invoke(released, 8, &[], r.eax, table(released));
                ok = 1;
            }
        }
        let target = *args as *mut u8;
        if !target.is_null() {
            r = invoke(target, 8, &[], table(target), r.edx);
        }
        r.low_byte(ok)
    }
}

port! {
    /// Installs the foundation's default exemplar and its optional maximum-height property.
    #[port(address = 0x0069_82B0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::LoadFoundationProperties", naked)]
    pub unsafe extern "C" fn fc4p_006982b0() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {body}", "ret 8", body = sym load_foundation);
    }
}
port! {
    /// Installs a retaining wall's default exemplar, using its current type when the supplied type is zero.
    #[port(address = 0x0069_83C0, owner = "building", name = "LoadLotRetainingWallProperties", naked)]
    pub unsafe extern "C" fn fc4p_006983c0() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {body}", "ret 8", body = sym load_wall);
    }
}
port! {
    /// Installs a building's default exemplar and optional zero-origin size box. Always returns true in AL.
    #[port(address = 0x0069_8490, owner = "building", name = "LoadBuildingProperties", naked)]
    pub unsafe extern "C" fn fc4p_00698490() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {body}", "ret 8", body = sym load_building);
    }
}

unsafe extern "fastcall" fn load_foundation(this: *mut u8, args: *mut u32) -> u64 {
    // SAFETY: the entry supplies the original writable arguments.
    unsafe { load(this, args, Loader::Foundation) }
}
unsafe extern "fastcall" fn load_wall(this: *mut u8, args: *mut u32) -> u64 {
    // SAFETY: the entry supplies the original writable arguments.
    unsafe { load(this, args, Loader::Wall) }
}
unsafe extern "fastcall" fn load_building(this: *mut u8, args: *mut u32) -> u64 {
    // SAFETY: the entry supplies the original writable arguments.
    unsafe { load(this, args, Loader::Building) }
}

pub const PORTS: &[Port] = &[
    fc4p_00695af0::PORT,
    fc4p_006982b0::PORT,
    fc4p_006983c0::PORT,
    fc4p_00698490::PORT,
];

#[cfg(test)]
mod tests;

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Simulator tuning, exemplar registration, messages and container construction.

use super::BuildingSim;
use crate::abi::{call_raw, target, vslot, Conv, RawCall};
use crate::port::Port;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(p: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn allocate_object(size: u32) -> u64 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn allocate_message(size: u32) -> u64 = "C" 0x0091_33DA; }
game_fn! { #[allow(dead_code)] fn construct_message(this: *mut u8) -> u64 = "thiscall" 0x0091_34D6; }
game_fn! { #[allow(dead_code)] fn property_float(holder: *mut u8, id: u32, out: *mut u8) -> u64 = "C" 0x005F_D4F0; }
game_fn! { #[allow(dead_code)] fn property_word(holder: *mut u8, id: u32, out: *mut u8) -> u64 = "C" 0x005F_D480; }
game_fn! { #[allow(dead_code)] fn property_floats(holder: *mut u8, id: u32, out: *mut u8) -> u64 = "C" 0x005F_FCC0; }
game_fn! { #[allow(dead_code)] fn property_words(holder: *mut u8, id: u32, out: *mut u8) -> u64 = "C" 0x005F_F510; }
game_fn! { #[allow(dead_code)] fn resource_entry(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x004A_3470; }
game_fn! { #[allow(dead_code)] fn copy_vectors(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x004C_9470; }
game_fn! { #[allow(dead_code)] fn find_developer(this: *mut u8, out: *mut u32, key: *mut u32) -> u64 = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn developer_entry(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0049_7A00; }
game_fn! { #[allow(dead_code)] fn date_entry(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0042_9760; }
game_fn! { #[allow(dead_code)] fn construct_developer(this: *mut u8) -> u64 = "thiscall" 0x006B_7310; }
game_fn! { #[allow(dead_code)] fn register_developer(this: *mut u8, holder: *mut u8) -> u64 = "thiscall" 0x0069_91A0; }
game_fn! { #[allow(dead_code)] fn register_building(key: *mut u8) -> u64 = "C" 0x0069_D0A0; }
game_fn! { #[allow(dead_code)] fn register_foundation(key: *mut u8) -> u64 = "C" 0x0069_8F50; }
game_fn! { #[allow(dead_code)] fn register_wall(key: *mut u8) -> u64 = "C" 0x0069_8FC0; }
game_fn! { #[allow(dead_code)] fn construct_simulator(this: *mut u8) -> u64 = "thiscall" 0x0069_A000; }
game_fn! { #[allow(dead_code)] fn reserve_buckets(this: *mut u8, count: u32) -> u64 = "thiscall" 0x0069_6540; }
game_fn! { #[allow(dead_code)] fn copy_growth(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x004D_1230; }
game_fn! { #[allow(dead_code)] fn rebalance(node: *mut u8, root: *mut u8) -> u64 = "C" 0x0040_D5E0; }
game_fn! { #[allow(dead_code)] fn previous(this: *mut u32) -> u64 = "thiscall" 0x0068_09E0; }
game_fn! { #[allow(dead_code)] fn insert_stage(this: *mut u8, out: *mut u32, side: u32, parent: *mut u8, value: *mut u8, force: u32) -> u64 = "thiscall" 0x0069_6630; }

game_data! { static resource_manager: *mut u8 = 0x00B4_3CA8; }
game_data! { static message_server: *mut u8 = 0x00B4_3CCC; }
game_data! { static building_count: u32 = 0x00B4_8558; }
game_data! { static foundation_map: u8 = 0x00B4_85B8; }
game_data! { static wall_map: u8 = 0x00B4_85CC; }
game_data! { static category_ids: [u32; 31] = 0x00AB_0A40; }
game_data! { static city: *mut u8 = 0x00B4_3D5C; }

layout! {
    /// Building catalogue record and its accumulated category mask.
    struct BuildingRecord { 0x1C => categories / set_categories: u32 }
}
layout! {
    /// Tree links and the signed developer ID, byte stage and value payload.
    struct StageNode {
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => key / set_key: i32,
        0x14 => stage / set_stage: u8,
        0x18 => value / set_value: u32,
    }
}

#[derive(Clone, Copy)]
struct Registers {
    eax: u32,
    edx: u32,
}
impl Registers {
    fn bits(self) -> u64 {
        (u64::from(self.edx) << 32) | u64::from(self.eax)
    }
    // SAFETY: callers supply the live interface and the signature for each selected address or slot.
    unsafe fn call(&mut self, conv: Conv, address: usize, this: u32, args: &[u32]) {
        let mut c = RawCall::new(conv, address, this, args);
        c.eax = self.eax;
        c.edx = self.edx;
        unsafe { call_raw(&mut c) };
        self.eax = c.eax;
        self.edx = c.edx;
    }
    unsafe fn game(&mut self, address: u32, this: u32, args: &[u32]) {
        unsafe { self.call(Conv::Thiscall, target(address), this, args) };
    }
    unsafe fn cdecl(&mut self, address: u32, ecx: u32, args: &[u32]) {
        unsafe { self.call(Conv::Cdecl, target(address), ecx, args) };
    }
    unsafe fn virtual_call(&mut self, object: *mut u8, slot: usize, args: &[u32], table_eax: bool) {
        let table = unsafe { read(object.cast::<u32>()) };
        if table_eax {
            self.eax = table;
        } else {
            self.edx = table;
        }
        unsafe { self.call(Conv::Thiscall, vslot(object, slot), object as u32, args) };
    }
}

// The wrappers pass the caller's argument words and both incoming scratch registers to Rust.
macro_rules! entry {
    ($export:ident, $address:literal, $name:literal, $body:ident, $pop:literal) => {
        port! {
            /// Preserves the original argument slots and return-register bytes.
            #[port(address = $address, owner = "building", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("push edx", "push eax", "lea edx, [esp + 12]",
                    "call {body}", concat!("ret ", $pop), body = sym $body);
            }
        }
    };
}

entry!(
    fc4p_006959e0,
    0x0069_59E0,
    "cSC4BuildingDevelopmentSimulator::ReadSimulatorTunableValues",
    read_tuning,
    "0"
);
unsafe extern "fastcall" fn read_tuning(this: *mut u8, _args: *mut u32, eax: u32, edx: u32) -> u64 {
    // SAFETY: the simulator and its resource interfaces are supplied by the game.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        sim.set_resource_min(100);
        sim.set_resource_max(2000);
        sim.set_resource_fraction(0.5);
        let mut bias = 10u32;
        let key: [u32; 3] = [0x6534_284A, 0xE7E2_C2DB, 0xE8DA_7677];
        let mut resource = MaybeUninit::<*mut u8>::uninit();
        let mut r = Registers {
            eax,
            edx: resource.as_mut_ptr() as u32,
        };
        r.virtual_call(
            *resource_manager(),
            0x0C,
            &[key.as_ptr() as u32, 0xA521_60F5, resource.as_mut_ptr() as u32, 0, 0],
            true,
        );
        if r.eax as u8 != 0 {
            let resource = resource.assume_init();
            r.virtual_call(resource, 0x1C, &[], true);
            let holder = r.eax as *mut u8;
            r.virtual_call(holder, 0x24, &[0x47E2_C380], false);
            if r.eax != 0 {
                r.virtual_call(r.eax as *mut u8, 0x14, &[], false);
                r.virtual_call(r.eax as *mut u8, 0x19C, &[], false);
                let values = r.eax as *mut u32;
                sim.set_resource_min(read(values));
                sim.set_resource_max(read(values.add(1)));
                r.edx = sim.resource_max();
            }
            r.cdecl(
                0x005F_D4F0,
                if r.eax != 0 { sim.resource_min() } else { holder as u32 },
                &[holder as u32, 0x47E2_C381, this.add(0xC8) as u32],
            );
            r.eax = &mut bias as *mut u32 as u32;
            r.cdecl(0x005F_D480, 0, &[holder as u32, 0x47E2_C382, r.eax]);
            r.cdecl(
                0x005F_FCC0,
                this.add(0xD0) as u32,
                &[holder as u32, 0x47E2_C383, this.add(0xD0) as u32],
            );
            r.virtual_call(resource, 8, &[], false);
            r.eax = bias;
        }
        let _ = edx;
        sim.set_demand_bias(bias as i32);
        r.bits()
    }
}

entry!(
    fc4p_00695fd0,
    0x0069_5FD0,
    "cSC4BuildingDevelopmentSimulator::PostBuildingTypesUpdatedMessage",
    post_types,
    "0"
);
unsafe extern "fastcall" fn post_types(this: *mut u8, _args: *mut u32, eax: u32, edx: u32) -> u64 {
    // SAFETY: the global server, allocation and message interfaces have the game's signatures.
    unsafe {
        let mut r = Registers { eax, edx };
        let server = *message_server();
        if !server.is_null() {
            r.cdecl(0x0091_33DA, this as u32, &[0x2C]);
            if r.eax != 0 {
                r.game(0x0091_34D6, r.eax, &[]);
            }
            let message = r.eax as *mut u8;
            r.virtual_call(message, 4, &[], true);
            r.virtual_call(message, 0x14, &[0xA783_EBC8], false);
            r.virtual_call(message, 0x30, &[0], true);
            r.eax = *building_count();
            r.virtual_call(message, 0x40, &[r.eax], false);
            r.virtual_call(server, 0x10, &[message as u32, 0], false);
            r.virtual_call(message, 8, &[], true);
        }
        r.bits()
    }
}

entry!(
    fc4p_00696b80,
    0x0069_6B80,
    "cSC4BuildingDevelopmentSimulator::SetBuildingCategoryFlags",
    set_categories,
    "0"
);
unsafe extern "fastcall" fn set_categories(_this: *mut u8, args: *mut u32, _eax: u32, edx: u32) -> u64 {
    // SAFETY: caller supplies the record, holder and the property vector's storage.
    unsafe {
        let record = BuildingRecord::from_ptr(*args as *mut u8);
        record.set_categories(0);
        let mut vector = [0u32; 3];
        let mut r = Registers {
            eax: vector.as_mut_ptr() as u32,
            edx,
        };
        r.cdecl(0x005F_F510, *args.add(1), &[*args.add(1), 0xAA1D_D396, r.eax]);
        if r.eax as u8 != 0 {
            let mut p = vector[0] as *mut u32;
            r.eax = p as u32;
            while p as u32 != vector[1] {
                r.edx = read(p);
                for i in 0..31 {
                    if read(category_ids().cast::<u32>().add(i)) == r.edx {
                        record.set_categories(record.categories() | (1 << i));
                    }
                }
                p = p.wrapping_add(1);
                r.eax = p as u32;
            }
        } else {
            r.eax = vector[0];
        }
        if vector[0] != 0 {
            r.cdecl(0x0090_CF63, 31, &[vector[0]]);
        }
        r.bits()
    }
}

game_fn! { #[allow(dead_code)] fn query_interface(this: *mut u8, iid: u32, out: *mut u32) -> u64 = "thiscall" 0x0069_5290; }
port! {
    /// Adjusts the message-target interface to the primary interface before querying it.
    #[port(address = 0x0069_8D40, owner = "building", name = "BuildingQueryInterfaceThunk", naked)]
    pub unsafe extern "C" fn fc4p_00698d40() {
        core::arch::naked_asm!("sub ecx, 8", "jmp {forward}", forward = sym query_forward);
    }
}
#[unsafe(naked)]
unsafe extern "C" fn query_forward() {
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "call {resolve}", "mov [esp + 8], eax",
        "pop edx", "pop ecx", "ret", resolve = sym query_target);
}
extern "C" fn query_target() -> usize {
    target(0x0069_5290)
}

entry!(
    fc4p_00698f50,
    0x0069_8F50,
    "RegisterFoundationExemplar",
    register_foundation_body,
    "0"
);
entry!(
    fc4p_00698fc0,
    0x0069_8FC0,
    "RegisterWallExemplar",
    register_wall_body,
    "0"
);
unsafe extern "fastcall" fn register_foundation_body(_this: *mut u8, args: *mut u32, eax: u32, edx: u32) -> u64 {
    unsafe { register_key(args, eax, edx, foundation_map()) }
}
unsafe extern "fastcall" fn register_wall_body(_this: *mut u8, args: *mut u32, eax: u32, edx: u32) -> u64 {
    unsafe { register_key(args, eax, edx, wall_map()) }
}
unsafe fn register_key(args: *mut u32, eax: u32, _edx: u32, map: *mut u8) -> u64 {
    // SAFETY: the key is three words; resource and map interfaces use the original contracts.
    unsafe {
        let key = *args as *mut u32;
        let mut resource = MaybeUninit::<*mut u8>::uninit();
        let mut r = Registers {
            eax,
            edx: resource.as_mut_ptr() as u32,
        };
        r.virtual_call(
            *resource_manager(),
            0x0C,
            &[key as u32, 0xA521_60F5, resource.as_mut_ptr() as u32, 0, 0],
            true,
        );
        if r.eax as u8 != 0 {
            let resource = resource.assume_init();
            r.virtual_call(resource, 0x1C, &[], true);
            let mut id = read(key.add(2));
            r.edx = &mut id as *mut u32 as u32;
            r.game(0x004A_3470, map as u32, &[r.edx]);
            let destination = r.eax as *mut u32;
            write(destination, read(key));
            write(destination.add(1), read(key.add(1)));
            r.edx = read(key.add(1));
            write(destination.add(2), read(key.add(2)));
            r.virtual_call(resource, 8, &[], false);
        }
        r.eax &= !255;
        r.bits()
    }
}

entry!(fc4p_00699150, 0x0069_9150, "CopyVectorRange", copy_range, "0");
unsafe extern "fastcall" fn copy_range(this: *mut u8, args: *mut u32, eax: u32, edx: u32) -> u64 {
    // SAFETY: the source range and destination have twelve-byte elements.
    unsafe {
        let mut source = *args;
        let end = *args.add(1);
        let mut destination = *args.add(2);
        let mut r = Registers { eax, edx };
        let mut ecx = this as u32;
        while source != end {
            if destination != 0 {
                r.game(0x004C_9470, destination, &[source]);
                ecx = destination;
            }
            source = source.wrapping_add(12);
            destination = destination.wrapping_add(12);
        }
        let _ = ecx;
        r.eax = destination;
        r.bits()
    }
}

entry!(
    fc4p_006991a0,
    0x0069_91A0,
    "RegisterDeveloperExemplar",
    register_developer_body,
    "4"
);
unsafe extern "fastcall" fn register_developer_body(this: *mut u8, args: *mut u32, _eax: u32, edx: u32) -> u64 {
    // SAFETY: simulator containers, holder and city services are live game objects.
    unsafe {
        let holder = *args;
        let mut key = 0u32;
        let mut r = Registers {
            eax: &mut key as *mut u32 as u32,
            edx,
        };
        r.cdecl(0x005F_D480, this as u32, &[holder, 0x21, r.eax]);
        r.eax = key;
        if key != 0 {
            r.edx = args as u32;
            r.game(
                0x0049_7520,
                this.add(0x5C) as u32,
                &[args as u32, &mut key as *mut u32 as u32],
            );
            let node = *args as *mut u8;
            if node != read(this.add(0x5C).cast::<*mut u8>()) {
                let developer = read(node.add(0x14).cast::<*mut u8>());
                r.virtual_call(developer, 0x0C, &[holder], true);
            } else {
                r.eax = node as u32;
                r.cdecl(0x005E_55E0, this.add(0x5C) as u32, &[0x13C]);
                let mut developer = 0;
                if r.eax != 0 {
                    r.game(0x006B_7310, r.eax, &[]);
                    developer = r.eax;
                }
                r.virtual_call(developer as *mut u8, 4, &[], false);
                r.virtual_call(developer as *mut u8, 0x0C, &[holder], true);
                r.game(0x0049_7A00, this.add(0x5C) as u32, &[&mut key as *mut u32 as u32]);
                write(r.eax as *mut u32, developer);
            }
            *args = key;
            r.virtual_call(*city(), 0x1C, &[], true);
            let date = r.eax as *mut u8;
            r.game(0x0042_9760, this.add(0xB0) as u32, &[args as u32]);
            let day = r.eax as *mut u32;
            r.virtual_call(date, 0x68, &[], false);
            write(day, r.eax);
        }
        r.bits()
    }
}

entry!(fc4p_00699e20, 0x0069_9E20, "LoadDeveloperResource", load_developer, "0");
unsafe extern "fastcall" fn load_developer(_this: *mut u8, args: *mut u32, eax: u32, _edx: u32) -> u64 {
    // SAFETY: key, callback context, resource interfaces and property holder follow the game contracts.
    unsafe {
        let mut resource = core::ptr::null_mut::<u8>();
        let mut r = Registers { eax, edx: *args };
        r.virtual_call(
            *resource_manager(),
            0x0C,
            &[*args, 0xA521_60F5, &mut resource as *mut *mut u8 as u32, 0, 0],
            true,
        );
        if r.eax as u8 != 0 {
            r.virtual_call(resource, 0x1C, &[], true);
            let holder = r.eax;
            let mut kind = 0u32;
            r.cdecl(
                0x005F_D480,
                &mut kind as *mut u32 as u32,
                &[holder, 0x10, &mut kind as *mut u32 as u32],
            );
            if r.eax as u8 != 0 && kind == 4 {
                r.game(0x0069_91A0, *args.add(1), &[holder]);
            }
        }
        if !resource.is_null() {
            r.virtual_call(resource, 8, &[], false);
        }
        r.bits()
    }
}

entry!(
    fc4p_0069a7a0,
    0x0069_A7A0,
    "AllocateBuildingSimulator",
    allocate_simulator,
    "0"
);
unsafe extern "fastcall" fn allocate_simulator(this: *mut u8, _args: *mut u32, eax: u32, edx: u32) -> u64 {
    // SAFETY: game allocator and simulator constructor have these fixed signatures.
    unsafe {
        let mut r = Registers { eax, edx };
        r.cdecl(0x005E_55E0, this as u32, &[0xE4]);
        if r.eax != 0 {
            r.game(0x0069_A000, r.eax, &[]);
        }
        r.bits()
    }
}

port! {
    /// Inserts a growth record and returns the previous bucket head in ECX.
    #[port(address = 0x0069_B650, owner = "building", name = "InsertGrowthBucket", naked)]
    pub unsafe extern "C" fn fc4p_0069b650() {
        core::arch::naked_asm!("push ecx", "push edx", "push eax", "lea edx, [esp + 16]",
            "call {body}", "pop ecx", "ret 4", body = sym insert_growth);
    }
}
unsafe extern "fastcall" fn insert_growth(this: *mut u8, args: *mut u32, _eax: u32, edx: u32) -> u64 {
    // SAFETY: the bucket vector and incoming key/value pair are supplied by the game's container.
    unsafe {
        let count = read(this.add(0x10).cast::<u32>()).wrapping_add(1);
        let mut r = Registers { eax: count, edx };
        r.game(0x0069_6540, this as u32, &[count]);
        let begin = read(this.add(4).cast::<*mut u32>());
        let end = read(this.add(8).cast::<u32>());
        let buckets = (end.wrapping_sub(begin as u32) as i32 >> 2) as u32;
        let pair = *args as *mut u8;
        let bucket: u32;
        core::arch::asm!("div {count}", count = in(reg) buckets,
            inout("eax") read(pair.cast::<u32>()) => _, inout("edx") 0u32 => bucket,
            options(nomem, nostack));
        let old = read(begin.wrapping_add(bucket as usize));
        write(args.sub(2), old);
        r.eax = begin as u32;
        r.edx = bucket;
        r.cdecl(0x0090_CF54, old, &[0x50]);
        let node = r.eax as *mut u8;
        write(node.cast::<u32>(), 0);
        if node.wrapping_add(8) as u32 != 0 {
            r.edx = read(pair.cast());
            r.eax = pair.wrapping_add(8) as u32;
            write(node.add(8).cast::<u32>(), r.edx);
            r.game(0x004D_1230, node.add(16) as u32, &[r.eax]);
        }
        write(node.cast(), old);
        let begin = read(this.add(4).cast::<*mut u32>());
        write(begin.wrapping_add(bucket as usize), node as u32);
        write(
            this.add(0x10).cast::<u32>(),
            read(this.add(0x10).cast::<u32>()).wrapping_add(1),
        );
        r.eax = node.wrapping_add(8) as u32;
        r.bits()
    }
}

entry!(
    fc4p_0069d130,
    0x0069_D130,
    "cSC4BuildingDevelopmentSimulator::InitStaticDataFromExemplar",
    init_exemplar,
    "0"
);
unsafe extern "fastcall" fn init_exemplar(_this: *mut u8, args: *mut u32, eax: u32, _edx: u32) -> u64 {
    // SAFETY: the resource key and global resource service use the original game interfaces.
    unsafe {
        let key = *args;
        let mut resource = core::ptr::null_mut::<u8>();
        let mut r = Registers {
            eax,
            edx: &mut resource as *mut *mut u8 as u32,
        };
        r.virtual_call(
            *resource_manager(),
            0x0C,
            &[key, 0xA521_60F5, &mut resource as *mut *mut u8 as u32, 0, 0],
            true,
        );
        if r.eax as u8 != 0 {
            r.virtual_call(resource, 0x1C, &[], true);
            let mut kind = 0u32;
            r.cdecl(
                0x005F_D480,
                &mut kind as *mut u32 as u32,
                &[r.eax, 0x10, &mut kind as *mut u32 as u32],
            );
            if r.eax as u8 != 0 {
                r.eax = kind.wrapping_sub(2);
                let address = if r.eax == 0 {
                    Some(0x0069_D0A0)
                } else {
                    r.eax = r.eax.wrapping_sub(15);
                    if r.eax == 0 {
                        Some(0x0069_8F50)
                    } else {
                        r.eax = r.eax.wrapping_sub(4);
                        if r.eax == 0 {
                            Some(0x0069_8FC0)
                        } else {
                            None
                        }
                    }
                };
                if let Some(address) = address {
                    r.cdecl(address, 0, &[key]);
                }
            }
        }
        if !resource.is_null() {
            r.virtual_call(resource, 8, &[], false);
        }
        r.bits()
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006959e0::PORT,
    fc4p_00695fd0::PORT,
    fc4p_00696b80::PORT,
    fc4p_00698d40::PORT,
    fc4p_00698f50::PORT,
    fc4p_00698fc0::PORT,
    fc4p_00699150::PORT,
    fc4p_00699e20::PORT,
    fc4p_0069a7a0::PORT,
    fc4p_0069b650::PORT,
    fc4p_0069d130::PORT,
    fc4p_00696630::PORT,
    fc4p_00696710::PORT,
];

entry!(
    fc4p_00696630,
    0x0069_6630,
    "InsertDeveloperStageNode",
    insert_stage_body,
    "20"
);
unsafe extern "fastcall" fn insert_stage_body(this: *mut u8, args: *mut u32, _eax: u32, edx: u32) -> u64 {
    // SAFETY: the tree header, parent, value and output iterator are the caller's live storage.
    unsafe {
        let parent = *args.add(2) as *mut u8;
        let value = *args.add(3) as *mut u8;
        let header = read(this.cast::<*mut u8>());
        let left = parent == header
            || (*args.add(4) == 0
                && (*args.add(1) != 0 || {
                    let node = StageNode::from_ptr(parent);
                    node.key() > read(value.cast::<i32>())
                        || (node.key() == read(value.cast::<i32>()) && *value.add(4) < node.stage())
                }));
        let mut r = Registers { eax: 0, edx };
        r.cdecl(0x0090_CF54, 0, &[0x1C]);
        let node = r.eax as *mut u8;
        if node.wrapping_add(0x10) as u32 != 0 {
            write(node.add(0x10).cast::<u32>(), read(value.cast()));
            write(node.add(0x14), *value.add(4));
            write(node.add(0x18).cast::<u32>(), read(value.add(8).cast()));
            if left {
                r.edx = read(value.add(8).cast());
            } else {
                r.edx = (r.edx & !255) | u32::from(*value.add(4));
            }
        }
        let p = StageNode::from_ptr(parent);
        let h = StageNode::from_ptr(read(this.cast()));
        if left {
            p.set_left(node);
            if parent == h.as_ptr() {
                h.set_parent(node);
                StageNode::from_ptr(read(this.cast())).set_right(node);
            } else if parent == h.left() {
                h.set_left(node);
            }
        } else {
            p.set_right(node);
            if parent == h.right() {
                h.set_right(node);
            }
        }
        let n = StageNode::from_ptr(node);
        n.set_parent(parent);
        n.set_left(core::ptr::null_mut());
        n.set_right(core::ptr::null_mut());
        r.eax = 0;
        r.edx = read(this.cast::<u32>()).wrapping_add(4);
        r.cdecl(0x0040_D5E0, read(this.cast()), &[node as u32, r.edx]);
        let count = read(this.add(4).cast::<u32>()).wrapping_add(1);
        write(this.add(4).cast(), count);
        r.eax = *args;
        write(r.eax as *mut u32, node as u32);
        r.bits()
    }
}

entry!(
    fc4p_00696710,
    0x0069_6710,
    "FindOrInsertDeveloperStage",
    stage_entry,
    "8"
);
unsafe extern "fastcall" fn stage_entry(this: *mut u8, args: *mut u32, eax: u32, _edx: u32) -> u64 {
    // SAFETY: the caller supplies a tree, writable iterator result and stage/value record.
    unsafe {
        let value = *args.add(1) as *mut u8;
        let header = read(this.cast::<*mut u8>());
        let mut node = StageNode::from_ptr(header).parent();
        let mut parent = header;
        let mut r = Registers {
            eax: (eax & !255) | 1,
            edx: header as u32,
        };
        let mut left = true;
        while !node.is_null() {
            let n = StageNode::from_ptr(node);
            parent = node;
            r.eax = n.key() as u32;
            left = n.key() > read(value.cast::<i32>());
            if n.key() == read(value.cast::<i32>()) {
                r.eax = (r.eax & !255) | u32::from(*value.add(4));
                left = *value.add(4) < n.stage();
            }
            if left {
                node = n.left();
                r.eax = (r.eax & !255) | 1;
            } else {
                node = n.right();
                r.eax &= !255;
            }
        }
        *args.add(1) = parent as u32;
        let mut insert = false;
        if left {
            if parent == StageNode::from_ptr(header).left() {
                insert = true;
            } else {
                r.game(0x0068_09E0, args.add(1) as u32, &[]);
            }
        }
        if !insert {
            let n = StageNode::from_ptr(*args.add(1) as *mut u8);
            r.eax = n.key() as u32;
            r.edx = read(value.cast());
            insert = (r.edx as i32) > n.key();
            if r.edx as i32 == n.key() {
                r.eax = (r.eax & !255) | u32::from(n.stage());
                insert = n.stage() < *value.add(4);
            }
        }
        let output = *args as *mut u8;
        if insert {
            r.game(
                0x0069_6630,
                this as u32,
                &[args.add(1) as u32, node as u32, parent as u32, value as u32, 0],
            );
            r.edx = read(r.eax as *mut u32);
            write(output.cast(), r.edx);
            *output.add(4) = 1;
        } else {
            write(output.cast(), *args.add(1));
            *output.add(4) = 0;
        }
        r.eax = output as u32;
        r.bits()
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Initialization of tool resources, terrain geometry, and preview services.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::x87::F80;

game_data! { static type_infos: [u8; 0xE04] = 0x00B4_52C8; }
game_data! { static rules_initialized: u8 = 0x00B4_52A8; }
game_data! { static resource_manager: *mut u8 = 0x00B4_3C94; }
game_data! { static type_resources: [u32; 13] = 0x00AA_8428; }
game_data! { static type_masks: [u32; 13] = 0x00AA_835C; }
game_data! { static preview_manager: *mut u8 = 0x00B4_3DD0; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_data! { static terrain: *mut u8 = 0x00B4_3CF4; }
game_data! { static occupants: *mut u8 = 0x00B4_3D0C; }
game_data! { static messages: *mut u8 = 0x00B4_3CCC; }
game_fn! { fn reload_rules() = "C" 0x0062_53D0; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn construct_preview(this: *mut u8) -> *mut u8 = "thiscall" 0x0062_0AD0; }
game_fn! { fn init_preview(this: *mut u8) = "thiscall" 0x0061_FD10; }
game_fn! { fn construct_filter(this: *mut u8) -> *mut u8 = "thiscall" 0x0061_F510; }
game_fn! { fn assign_parameters(this: *mut u8, source: *mut u8) = "thiscall" 0x0062_6860; }
game_fn! { fn collect_bridges(source: *mut u8, first: *mut u8, second: *mut u8) = "C" 0x0062_F9D0; }
game_fn! { fn allocate_occupant(size: u32) -> *mut u8 = "C" 0x0061_9E90; }
game_fn! { fn construct_occupant(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0061_BDD0; }

fn multiply_stored(value: F80, operand: f32) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fmul dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value = in(reg) &value, operand = in(reg) &operand, result = in(reg) &mut result,
            out("st(0)") _,
        );
    }
    result
}

unsafe fn bridge_count(info: *mut u8) -> u32 {
    unsafe { ((read::<u32>(info, 0x34).wrapping_sub(read(info, 0x30)) as i32) / 0x178) as u32 }
}

register_port!(0x0062_69C0, fc4p_006269c0, "cSC4NetworkTool::Init", initialize, "ret");

// The guard returns success in AL alone. The first initialization returns the final service call's
// other EAX bytes. No defaults are supplied for missing terrain or network-specific properties.
unsafe extern "fastcall" fn initialize(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        if tool.initialized() != 0 {
            return registers((*frame.add(1) & !0xFF) | 1, *frame);
        }
        tool.set_type_info(
            type_infos()
                .cast::<u8>()
                .wrapping_add(tool.network_type().wrapping_mul(0x114) as usize),
        );
        if *rules_initialized() == 0 {
            let manager = *resource_manager();
            *rules_initialized() = 1;
            vcall!(manager, 0x114 => fn(u32, u32), 0x0062_32F0, 0);
            reload_rules();
        }
        let id = type_resources()
            .cast::<u32>()
            .wrapping_add(tool.network_type() as usize)
            .read_unaligned();
        vcall!(this, 0x13C => fn(u32), id);
        let allocation = allocate(0x58);
        let preview = if allocation.is_null() {
            core::ptr::null_mut()
        } else {
            construct_preview(allocation)
        };
        let old = tool.preview();
        if preview != old {
            if !preview.is_null() {
                vcall!(preview, 4 => fn());
            }
            tool.set_preview(preview);
            if !old.is_null() {
                vcall!(old, 8 => fn());
            }
        }
        init_preview(tool.preview());
        vcall!(*preview_manager(), 0x80 => fn(*mut u8, u32, u32), tool.preview(), 5, 1000);
        write(this, 0x360, 0x1F4Fu32);
        let mask = type_masks()
            .cast::<u32>()
            .wrapping_add(tool.network_type() as usize)
            .read_unaligned();
        if mask & 0x1F4F == 0 {
            write(this, 0x360, mask);
        }
        let allocation = allocate(0x10);
        let filter = if allocation.is_null() {
            core::ptr::null_mut()
        } else {
            construct_filter(allocation)
        };
        write(this, 0x364, filter);
        vcall!(filter, 4 => fn());
        write(read(this, 0x364), 12, read::<u32>(this, 0x360));
        assign_parameters(this.add(0x290), tool.type_info().add(0x50));
        collect_bridges(tool.type_info().add(0x30), this.add(0x33C), this.add(0x348));
        let info = tool.type_info();
        write(this, 0x354, 0u8);
        if bridge_count(info) != 0 {
            let mut index = 0u32;
            let mut offset = 0u32;
            loop {
                let begin: *mut u8 = read(info, 0x30);
                if !begin.wrapping_add(offset.wrapping_add(0x98) as usize).is_null() {
                    write(this, 0x354, 1u8);
                }
                let count = bridge_count(tool.type_info());
                index = index.wrapping_add(1);
                offset = offset.wrapping_add(0x178);
                if index >= count {
                    break;
                }
            }
        }
        let city = *city();
        let x = vcall!(city, 0x174 => fn() -> u32);
        tool.set_width(x);
        tool.set_vertex_stride(x.wrapping_add(1));
        let z = vcall!(city, 0x178 => fn() -> u32);
        tool.set_height(z);
        write(this, 0x24C, z.wrapping_add(1));
        let stride = tool.vertex_stride();
        let diagonal = stride.wrapping_add(1);
        for (offset, value) in [
            (0x250, diagonal),
            (0x254, stride),
            (0x258, stride),
            (0x25C, 0),
            (0x260, 0),
            (0x264, 1),
            (0x268, 1),
            (0x26C, diagonal),
            (0x270, 1),
            (0x274, 1),
            (0x278, diagonal),
            (0x27C, diagonal),
            (0x280, stride),
            (0x284, stride),
            (0x288, 0),
            (0x28C, 0),
        ] {
            write(this, offset, value);
        }
        let p = *terrain();
        tool.set_terrain(p);
        if !p.is_null() {
            vcall!(p, 4 => fn());
            let scale = vcall_st0!(tool.terrain(), 0x18);
            write(this, 0x238, scale.to_f32());
        }
        let p = *occupants();
        tool.set_occupant_manager(p);
        if !p.is_null() {
            vcall!(p, 4 => fn());
        }
        let p = vcall!(city, 0x84 => fn() -> *mut u8);
        write(this, 0x4C, p);
        if !p.is_null() {
            vcall!(p, 4 => fn());
        }
        let slope = F80::from_f32(read(this, 0x2A8)).tan();
        write(this, 0x23C, multiply_stored(slope, read(this, 0x238)).to_f32());
        let manager = *preview_manager();
        let controller = if manager.is_null() {
            core::ptr::null_mut()
        } else {
            vcall!(manager, 0x28 => fn() -> *mut u8)
        };
        write(
            this,
            0xFC,
            if controller.is_null() {
                0u32
            } else {
                read(controller, 0x14)
            },
        );
        let allocation = allocate_occupant(0x180);
        let occupant = if allocation.is_null() {
            core::ptr::null_mut()
        } else {
            construct_occupant(allocation, 1)
        };
        let interface = if occupant.is_null() {
            core::ptr::null_mut()
        } else {
            occupant.add(4)
        };
        let mut call = RawCall::new(Conv::Thiscall, vslot(interface, 4), interface as u32, &[]);
        call.eax = occupant as u32;
        call.edx = read(interface, 0);
        call_raw(&mut call);
        call.target = vslot(interface, 0xC);
        call.eax = read(interface, 0);
        call_raw(&mut call);
        call.target = vslot(interface, 0x10);
        call.edx = read(interface, 0);
        call_raw(&mut call);
        call.target = vslot(interface, 8);
        call.eax = read(interface, 0);
        call_raw(&mut call);
        let server = *messages();
        if !server.is_null() {
            let args = [this.add(4) as u32, 0xA6B7_9602];
            call = RawCall::new(Conv::Thiscall, vslot(server, 0x14), server as u32, &args);
            call.eax = this.add(4) as u32;
            call.edx = read(server, 0);
            call_raw(&mut call);
        }
        tool.set_initialized(1);
        registers((call.eax & !0xFF) | 1, call.edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006269c0::PORT];

#[cfg(test)]
mod tests;

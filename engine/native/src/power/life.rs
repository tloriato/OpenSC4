// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction, tuning and initialization of the power simulator.

use super::*;
use core::ptr::{read_unaligned, write_volatile};

game_data! { static zero_percent: u32 = 0x00B4_A5B0; }
game_data! { static resource_manager: *mut u8 = 0x00B4_3CA8; }
game_data! { static ordinances: *mut u8 = 0x00B4_3D90; }
game_data! { static percent_scale: f32 = 0x00A8_23A4; }
game_data! { static power_started: u8 = 0x00B4_A5AC; }
game_data! { static current_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static lot_manager: *mut u8 = 0x00B4_3D00; }
game_data! { static simulator: *mut u8 = 0x00B4_3D5C; }
game_data! { static history: *mut u8 = 0x00B4_3DC0; }
game_data! { static message_server: *mut u8 = 0x00B4_3CCC; }
game_data! { static power_name: u8 = 0x00A8_945C; }
game_fn! { fn allocate(bytes: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn container_allocate(bytes: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! {
    #[allow(dead_code)]
    fn random_construct(this: *mut u8, value: i32) -> u32 = "thiscall" 0x0091_3A5C;
}
game_fn! { fn curve_construct(this: *mut u8) -> u32 = "thiscall" 0x007D_0410; }
game_fn! {
    #[allow(dead_code)]
    fn power_construct(this: *mut u8) -> *mut u8 = "thiscall" 0x006E_F7B0;
}
game_fn! { fn container_free(pointer: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! {
    fn pair_insert(this: *mut u32, position: u32, pair: *const u32, allocator: *mut u8,
                   count: u32, width: u32) -> u32 = "thiscall" 0x007C_61A0;
}
game_fn! { fn curve_set(this: *mut u8, points: *const u32) -> u32 = "thiscall" 0x005F_4480; }
game_fn! { fn curve_read(this: *mut u8, key: *const u32, id: u32) -> u32 = "thiscall" 0x005F_4700; }
game_fn! { fn property_u32(properties: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn property_i32(properties: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D450; }
game_fn! { fn property_float(properties: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D4F0; }
game_fn! { fn read_tuning(this: *mut u8) -> u32 = "thiscall" 0x006E_D1C0; }
game_fn! { fn string_construct(this: *mut u32, text: *const u8) -> u32 = "thiscall" 0x0040_8480; }
game_fn! { fn rows_resize(this: *mut u8, count: u32, value: *const u32) -> u64 = "thiscall" 0x0046_23F0; }
game_fn! { fn row_insert(this: *mut u8, end: u32, count: u32, value: *const u16) -> u64 = "thiscall" 0x0046_2170; }
game_fn! { fn init_wire(this: *mut u8) -> u32 = "thiscall" 0x006E_AED0; }

layout! {
    /// Initialization flags, relative to the simulator's primary interface.
    struct InitView {
        0x0C => initialized / set_initialized: u8,
        0x0D => ready / set_ready: u8,
    }
}
game_fn! {
    #[allow(dead_code)]
    fn query_interface(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 = "thiscall" 0x006E_ABC0;
}

unsafe fn store(this: *mut u8, offset: usize, value: u32) {
    // SAFETY: the caller supplies an accessible word of the live object.
    unsafe { write_volatile(this.add(offset).cast(), value) }
}

unsafe fn word(this: *mut u8, offset: usize) -> u32 {
    // SAFETY: the caller supplies an accessible word of the live object.
    unsafe { read_unaligned(this.add(offset).cast()) }
}

unsafe fn append_pair(vector: &mut [u32; 3], pair: &[u32; 2], allocator: *mut u8) {
    // SAFETY: vector is the game's three-word container, with room described by its end pointers.
    unsafe {
        if vector[1] == vector[2] {
            pair_insert(vector.as_mut_ptr(), vector[1], pair.as_ptr(), allocator, 1, 1);
        } else {
            let end = vector[1] as *mut u32;
            if !end.is_null() {
                end.write_unaligned(pair[0]);
                end.add(1).write_unaligned(pair[1]);
            }
            vector[1] = vector[1].wrapping_add(8);
        }
    }
}

port! {
    /// Install the default response curves, then apply the power exemplar's properties and the
    /// current ordinance multiplier. The temporary curve points use the game's container allocator.
    #[port(address = 0x006E_D1C0, owner = "power", name = "cSC4PowerSimulator::ReadSimulatorTunableValues")]
    pub unsafe extern "thiscall" fn fc4p_006ed1c0(this: *mut u8) -> u32 {
        // SAFETY: the simulator and the global services are live game objects.
        unsafe {
            let mut scratch = resource_manager().read() as u32;
            let key = [0x6534_284A, 0xE7E2_C2DB, 0xC911_E35B];
            store(this, 0x2B0, 2);
            store(this, 0x2B4, 0x3F8C_CCCD);
            store(this, 0x368, 0x3F99_999A);
            let mut vector = [0u32; 3];
            let mut allocator = core::mem::MaybeUninit::<u8>::uninit();
            let mut pair = [0u32; 2];
            pair_insert(vector.as_mut_ptr(), 0, pair.as_ptr(), allocator.as_mut_ptr(), 1, 1);
            pair = [0x42C8_0000, 0x42C8_0000];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            pair = [0x42F0_0000, 0x42DC_0000];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            curve_set(this.add(0x378), vector.as_ptr());
            vector[1] = vector[0];
            pair = [0, 0x3F80_0000];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            pair = [0x3F80_0000, 0x3F80_0000];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            curve_set(this.add(0x398), vector.as_ptr());
            curve_set(this.add(0x3A8), vector.as_ptr());
            vector[1] = vector[0];
            pair = [0, 0x41A0_0000];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            pair = [0x42C8_0000, 0x42C8_0000];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            curve_set(this.add(0x388), vector.as_ptr());
            vector[1] = vector[0];
            pair = [0, 0xBCA3_D70A];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            pair = [0x42F0_0000, 0x3C23_D70A];
            append_pair(&mut vector, &pair, allocator.as_mut_ptr());
            curve_set(this.add(0x3B8), vector.as_ptr());
            store(this, 0x36C, 0x3F00_0000);
            store(this, 0x370, 10);
            store(this, 0x374, 0x3F66_6666);
            store(this, 0x364, 0x3F19_999A);
            let mut resource = core::mem::MaybeUninit::<*mut u8>::uninit();
            if vcall!(scratch as *mut u8, 0x0C => fn(*const u32, u32, *mut *mut u8, u32, u32) -> u32,
                      key.as_ptr(), 0xA521_60F5, resource.as_mut_ptr(), 0, 0) as u8 != 0 {
                let resource = resource.assume_init();
                let properties = vcall!(resource, 0x1C => fn() -> *mut u8);
                property_u32(properties, 0xA911_E2F1, this.add(0x2B0).cast());
                if property_float(properties, 0xE911_E2FC, this.add(0x2B4).cast()) as u8 != 0 {
                    let value = multiply_float(F80::from_f32(f32::from_bits(word(this, 0x2B4))), percent_scale().read());
                    store(this, 0x2B4, value.to_f32().to_bits());
                }
                if property_float(properties, 0x2912_1E8B, &mut scratch) as u8 != 0 {
                    store(this, 0x364, multiply_float(F80::from_f32(f32::from_bits(scratch)), percent_scale().read()).to_f32().to_bits());
                }
                curve_read(this.add(0x378), key.as_ptr(), 0x6911_E2DF);
                for (id, offset) in [(0xE911_E2D1, 0x368), (0x0912_2027, 0x36C)] {
                    if property_float(properties, id, &mut scratch) as u8 != 0 {
                        store(this, offset, multiply_float(F80::from_f32(f32::from_bits(scratch)), percent_scale().read()).to_f32().to_bits());
                    }
                }
                property_i32(properties, 0x2912_202E, this.add(0x370).cast());
                if property_float(properties, 0x4912_2036, &mut scratch) as u8 != 0 {
                    store(this, 0x374, multiply_float(F80::from_f32(f32::from_bits(scratch)), percent_scale().read()).to_f32().to_bits());
                }
                for (offset, id) in [(0x388, 0x0911_E2E6), (0x398, 0x0911_E2E8),
                                     (0x3A8, 0x0911_E2E9), (0x3B8, 0xAA71_3782)] {
                    curve_read(this.add(offset), key.as_ptr(), id);
                }
                vcall!(resource, 8 => fn() -> u32);
            }
            let multiplier = vcall_st0!(ordinances().read(), 0x64, 0x0911_E117u32, 0u32);
            store(this, 0x2AC, multiplier.to_f32().to_bits());
            if vector[0] == 0 { 0 } else { container_free(vector[0] as *mut u8) }
        }
    }
}

// Keep the caller's EDI at constructor calls, including the saved-register word in nested frames.
unsafe fn construct_with_edi(address: u32, this: *mut u8, entry_edi: u32, seed: bool) -> *mut u8 {
    let result: *mut u8;
    // SAFETY: both constructors take ECX and return EAX. The random constructor additionally
    // consumes one seed argument. Both preserve the usual callee-saved registers.
    unsafe {
        if seed {
            core::arch::asm!(
                "push -1", "call {target}", target = in(reg) crate::abi::target(address),
                in("ecx") this, in("edi") entry_edi, lateout("eax") result, clobber_abi("C"),
            );
        } else {
            core::arch::asm!(
                "call {target}", target = in(reg) crate::abi::target(address),
                in("ecx") this, in("edi") entry_edi, lateout("eax") result, clobber_abi("C"),
            );
        }
    }
    result
}

unsafe fn register_agent(scheduler: *mut u8, this: *mut u8, phase: u32, name: *mut u32) {
    // SAFETY: slot 0x4c takes the message target, phase, name and flags. Keep the simulator,
    // message target and scheduler in the same preserved registers throughout registration.
    unsafe {
        core::arch::asm!(
            "push esi", "push ebx", "push 2", "push {name}", "push {phase}",
            "mov esi, {this}", "lea ebx, [esi + 4]",
            "push ebx", "mov ecx, edi", "mov eax, [edi]", "call dword ptr [eax + 0x4c]", "pop ebx", "pop esi",
            name = in(reg) name, phase = in(reg) phase,
            this = in(reg) this, in("edi") scheduler, clobber_abi("C"),
        );
    }
}

port! {
    /// Construct the original interfaces, containers, random generator and five curves. History and padding
    /// bytes retain their incoming contents until initialization. Return the primary interface.
    #[port(address = 0x006E_F7B0, owner = "power", name = "cSC4PowerSimulator::cSC4PowerSimulator", naked)]
    pub unsafe extern "C" fn fc4p_006ef7b0() {
        core::arch::naked_asm!("mov edx, edi", "jmp {body}", body = sym construct);
    }
}

unsafe extern "fastcall" fn construct(this: *mut u8, entry_edi: u32) -> *mut u8 {
    // SAFETY: the game provides storage for the complete 0x3c8-byte object.
    unsafe {
        store(this, 4, 0x00A8_1174);
        store(this, 8, 0x00A8_0784);
        store(this, 0, 0x00AB_28B8);
        store(this, 4, 0x00AB_28A4);
        store(this, 8, 0x00AB_288C);
        this.add(0x0C).write(0);
        this.add(0x0D).write(0);
        store(this, 0x10, 0);
        this.add(0x14).write(0);
        for offset in (0x18..=0x60).step_by(4) {
            store(this, offset, 0);
        }
        store(this, 0x64, zero_percent().read());
        for offset in (0x68..=0x8C).step_by(4) {
            store(this, offset, 0);
        }
        store(this, 0x298, 0);
        let header = container_allocate(0x10);
        store(header, 0, header as u32);
        store(header, 4, header as u32);
        store(this, 0x298, header as u32);
        for offset in [0x29C, 0x2A0, 0x2A4] {
            store(this, offset, 0);
        }
        store(this, 0x2AC, 0x3F80_0000);
        store(this, 0x2B0, 0);
        store(this, 0x2B4, 0x3F80_0000);
        for offset in [
            0x2B8, 0x2BC, 0x2C0, 0x2C4, 0x2C8, 0x2CC, 0x2D0, 0x2D4, 0x2DC, 0x308, 0x30C, 0x310,
        ] {
            store(this, offset, 0);
        }
        construct_with_edi(0x0091_3A5C, this.add(0x318), entry_edi, true);
        this.add(0x330).write(0);
        store(this, 0x334, 0);
        this.add(0x33A).write(0);
        for offset in [0x33C, 0x340, 0x344, 0x348, 0x354, 0x358, 0x35C] {
            store(this, offset, 0);
        }
        let text = container_allocate(8);
        store(this, 0x354, text as u32);
        store(this, 0x358, text as u32);
        store(this, 0x35C, text.wrapping_add(8) as u32);
        text.write(0);
        store(this, 0x350, 0x00A8_0810);
        for offset in [0x360, 0x364, 0x368, 0x36C, 0x374] {
            store(this, offset, 0);
        }
        for offset in [0x378, 0x388, 0x398, 0x3A8, 0x3B8] {
            curve_construct(this.add(offset));
        }
        for offset in (0x2E0..=0x304).step_by(4) {
            store(this, offset, 0);
        }
        this
    }
}

port! {
    /// Query an interface through the message-target base, four bytes after the primary interface.
    #[port(address = 0x006E_FA70, owner = "power", name = "cSC4PowerSimulator::QueryInterface message thunk", naked)]
    pub unsafe extern "C" fn fc4p_006efa70() {
        core::arch::naked_asm!("sub ecx, 4", "push 0x006eabc0", "ret");
    }
}

port! {
    /// Allocate the simulator through the game's allocator and call its constructor when successful.
    #[port(address = 0x006F_00D0, owner = "power", name = "CreatePowerSimulator", naked)]
    pub unsafe extern "C" fn fc4p_006f00d0() {
        core::arch::naked_asm!("mov edx, edi", "jmp {body}", body = sym create);
    }
}

unsafe extern "fastcall" fn create(_entry_ecx: u32, entry_edi: u32) -> *mut u8 {
    // SAFETY: the game's constructor consumes exactly this allocation.
    unsafe {
        let object = allocate(0x3C8);
        if object.is_null() {
            object
        } else {
            construct_with_edi(0x006E_F7B0, object, entry_edi, false)
        }
    }
}

port! {
    /// Retain city services, subscribe the simulator, size its consumption rows, and initialize the
    /// power grid and department. Restored interfaces and completed initialization stages are reused.
    #[port(address = 0x006F_03B0, owner = "power", name = "cSC4PowerSimulator::Init")]
    pub unsafe extern "fastcall" fn fc4p_006f03b0(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the game supplies the simulator, city services and container storage.
        unsafe {
            if power_started().read() == 0 { power_started().write(1); }
            let sim = PowerSim::from_ptr(this);
            let flags = InitView::from_ptr(this);
            let mut result = (u64::from(entry_edx) << 32) | u64::from(sim.city() as u32);
            if sim.city().is_null() {
                sim.set_city(current_city().read());
                result = vcall!(sim.city(), 4 => fn() -> u64);
            }
            for (offset, slot) in [(0x28, 0x88), (0x2C, 0x90)] {
                if word(this, offset) == 0 {
                    let service = vcall!(sim.city(), slot => fn() -> *mut u8);
                    store(this, offset, service as u32);
                    result = vcall!(service, 4 => fn() -> u64);
                }
            }
            if word(this, 0x34) == 0 {
                let service = lot_manager().read();
                store(this, 0x34, service as u32);
                result = vcall!(service, 4 => fn() -> u64);
            }
            for (offset, slot) in [(0x30, 0xF8), (0x1C, 0xD0)] {
                if word(this, offset) == 0 {
                    let service = vcall!(sim.city(), slot => fn() -> *mut u8);
                    store(this, offset, service as u32);
                    result = vcall!(service, 4 => fn() -> u64);
                }
            }
            if !sim.budget().is_null() && sim.department().is_null() {
                let department = vcall!(sim.budget(), 0x70 => fn(u32) -> *mut u8, 0x8910_BC8A);
                sim.set_department(department);
                result = department as u64;
                if !department.is_null() { result = vcall!(department, 4 => fn() -> u64); }
            }
            result = (result & !255) | u64::from(flags.initialized());
            if flags.initialized() == 0 {
                flags.set_initialized(1);
                read_tuning(this);
                let scheduler = simulator().read();
                if !scheduler.is_null() {
                    let mut name = core::mem::MaybeUninit::<[u32; 6]>::uninit();
                    string_construct(name.as_mut_ptr().cast(), power_name());
                    for phase in [6, 4, 10] {
                        register_agent(scheduler, this, phase, name.as_mut_ptr().cast());
                    }
                    let name = name.as_mut_ptr().cast::<u32>();
                    let allocation = name.add(1).read();
                    name.write_volatile(0x00A8_0810);
                    if allocation != 0 { container_free(allocation as *mut u8); }
                }
                let history = history().read();
                if !history.is_null() {
                    for id in [0xAA12_0D5E, 0xAA12_0D61, 0xAA12_0D63, 0xAA12_0D65,
                               0xAA12_0D66, 0xAA12_0D68, 0xAA12_0D6A, 0xAA12_0D6B, 0xAA12_0D6D] {
                        vcall!(history, 0x34 => fn(u32, u32) -> u32, id, 2);
                    }
                }
                let server = message_server().read();
                if !server.is_null() {
                    for id in [0x99EF_1142, 0x99EF_1143, 0x0769_9455, 0xCA1B_151C, 0x278E_6003,
                               0xC2C0_9554, 0xE8BD_A2E7, 0xEA41_8D8F, 0x6AC2_84F3, 0x26D3_1EC1] {
                        vcall!(server, 0x14 => fn(*mut u8, u32) -> u32, this.add(4), id);
                    }
                }
                let rows = vcall!(sim.city(), 0x174 => fn() -> u32);
                let columns = vcall!(sim.city(), 0x178 => fn() -> u32);
                let bytes = word(this, 0x88).wrapping_sub(word(this, 0x84)) as i32;
                let high = ((i64::from(bytes) * 0x2AAA_AAAB) >> 33) as i32;
                let size = high.wrapping_add((high as u32 >> 31) as i32) as u32;
                result = (u64::from(high as u32) << 32) | u64::from(size);
                if size != rows {
                    let empty_row = [0u32; 3];
                    result = rows_resize(this.add(0x84), rows, empty_row.as_ptr());
                    let zero = 0u16;
                    for x in 0..rows {
                        let offset = x.wrapping_mul(12);
                        let row = (word(this, 0x84) as *mut u8).wrapping_add(offset as usize);
                        let begin = word(row, 0);
                        let end = word(row, 4);
                        let old = (end.wrapping_sub(begin) as i32 >> 1) as u32;
                        let mut edx = begin;
                        if columns < old {
                            store(row, 4, begin.wrapping_add(columns.wrapping_mul(2)));
                        } else {
                            result = row_insert(row, end, columns.wrapping_sub(old), &zero);
                            edx = (result >> 32) as u32;
                        }
                        for z in 0..columns {
                            let row = (word(this, 0x84) as *mut u8).wrapping_add(offset as usize);
                            let data = word(row, 0) as *mut u16;
                            data.wrapping_add(z as usize).write_volatile(0);
                            edx = offset;
                        }
                        result = (u64::from(edx) << 32) | u64::from(rows.wrapping_sub(x).wrapping_sub(1));
                    }
                }
            }
            result = (result & !255) | u64::from(flags.ready());
            if flags.ready() == 0 {
                flags.set_ready(1);
                for offset in [0x2D8, 0x338, 0x339, 0x33A] { this.add(offset).write(0); }
                init_wire(this);
                if sim.powered_grid().is_null() {
                    let grid = allocate(0x44);
                    if !grid.is_null() {
                        store(grid, 4, 0x00A8_0784);
                        store(grid, 0, 0x00A8_78C0);
                        store(grid, 4, 0x00A8_78A4);
                        grid.add(8).write(0);
                        store(grid, 0x0C, 0);
                        store(grid, 0x10, 0x49B9_E602);
                        for offset in [0x18, 0x38, 0x3C, 0x40] { store(grid, offset, 0); }
                    }
                    sim.set_powered_grid(grid);
                    vcall!(grid, 4 => fn() -> u32);
                    vcall!(sim.powered_grid(), 0x0C => fn() -> u32);
                    vcall!(sim.powered_grid(), 0x24 => fn(u32) -> u32, 1);
                    vcall!(sim.powered_grid(), 0x18 => fn(u32) -> u32, 0x49D5_BC86);
                    vcall!(sim.powered_grid(), 0x6C => fn(u32) -> u32, 0);
                }
                for kind in 0..10 {
                    for month in 0..12 { store(this, 0x90 + kind * 4 + month * 40, 0); }
                    store(this, 0x270 + kind * 4, 0);
                }
                result = (u64::from(this.add(0x298) as u32) << 32) | u64::from(this.add(0x294) as u32);
                store(this, 0x34C, 0);
                store(this, 0x34C, 4);
                if sim.department().is_null() {
                    let department = vcall!(sim.budget(), 0x64 => fn(u32, u32) -> *mut u8,
                                            0x8910_BC8A, 0x4A35_7EAF);
                    sim.set_department(department);
                    vcall!(department, 4 => fn() -> u32);
                    vcall!(sim.department(), 0x14 => fn(u32, u32) -> u32, 0x6A3F_F01C, 0x4A49_3980);
                    vcall!(sim.department(), 0x34 => fn(*mut u8) -> u32, this.add(0x368));
                    let item = vcall!(sim.department(), 0x44 => fn(u32, u32) -> *mut u8, 1, 0);
                    result = vcall!(item, 0x1C => fn(u32, u32) -> u64, 0x6A3F_F01C, 0x2A75_F757);
                }
            }
            (result & !255) | 1
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006ed1c0::PORT,
    fc4p_006ef7b0::PORT,
    fc4p_006efa70::PORT,
    fc4p_006f00d0::PORT,
];

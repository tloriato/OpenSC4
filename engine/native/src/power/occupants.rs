// SPDX-License-Identifier: GPL-3.0-or-later
// FreeCity4 - Copyright (C) 2026 FreeCity4 contributors

//! Occupant notifications, accounting, and wire invalidation.

use super::*;

layout! {
    /// Fields changed when an occupant leaves the city.
    struct OccupantSim {
        0x0C => initialized / set_initialized: u8,
        0x18 => city / set_city: *mut u8,
        0x68 => consumers / set_consumers: i32,
        0x74 => requirement / set_requirement: i32,
        0x84 => consumption_rows / set_consumption_rows: *mut u8,
        0x2DC => plant_count / set_plant_count: u32,
    }
}
layout! {
    /// Circular plant list links and its occupant reference.
    struct PlantLink {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x0C => occupant / set_occupant: *mut u8,
    }
}

game_fn! { fn expand(this: *mut u8, rect: *mut Rect) -> u32 = "thiscall" 0x006E_AE20; }
game_fn! { fn invalidate(this: *mut u8, rect: *mut Rect) -> u32 = "thiscall" 0x006E_B230; }
game_fn! { fn get_uint(holder: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn get_int(holder: *mut u8, id: u32, out: *mut i32) -> u32 = "C" 0x005F_D450; }
game_fn! { fn get_int64(holder: *mut u8, id: u32, out: *mut u64) -> u32 = "C" 0x005F_D4B0; }
game_fn! { fn move_bytes(dst: *mut u8, src: *mut u8, count: u32) -> u32 = "C" 0x009E_EB70; }
game_fn! { fn free(node: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn set_wire(this: *mut u8, rect: *mut Rect) -> u32 = "thiscall" 0x006E_AFD0; }
game_fn! { fn push_pole(vector: *mut u8, pole: *mut u32) -> u32 = "thiscall" 0x007E_BD30; }
game_fn! { fn push_plant(list: *mut u8, value: *mut u32) -> u32 = "thiscall" 0x006E_C530; }
game_fn! { fn set_int64(holder: *mut u8, id: u32, value: u64) -> u32 = "C" 0x005F_E450; }
game_data! { static budget: *mut u8 = 0x00B4_3D64; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static lots: *mut u8 = 0x00B4_3D08; }

/// Read the centre after storing each half-sum to float32, then use the city's two cell widths.
unsafe fn centre(sim: OccupantSim, occupant: *mut u8) -> (i32, i32) {
    unsafe {
        let mut low = [1.0f32; 3];
        let mut high = [0.0f32; 3];
        vcall!(occupant, 0x28 => fn(*mut f32, *mut f32) -> u32, low.as_mut_ptr(), high.as_mut_ptr());
        let z = multiply_float(add_float(F80::from_f32(high[2]), low[2]), *half()).to_f32();
        let x = multiply_float(add_float(F80::from_f32(high[0]), low[0]), *half()).to_f32();
        let width = vcall_st0!(sim.city(), 0x16C);
        let x = divide_into_float(width, x).ftol() as i32;
        let depth = vcall_st0!(sim.city(), 0x170);
        let z = divide_into_float(depth, z).ftol() as i32;
        (x, z)
    }
}

/// The one-cell rectangle used by poles, flora, and network occupants.
unsafe fn cell_rect(interface: *mut u8, slot: usize) -> Rect {
    unsafe {
        let mut x = core::mem::MaybeUninit::<u32>::uninit();
        let mut z = core::mem::MaybeUninit::<u32>::uninit();
        vcall!(interface, slot => fn(*mut u32, *mut u32) -> u32, x.as_mut_ptr(), z.as_mut_ptr());
        let (x, z) = (x.assume_init(), z.assume_init());
        Rect {
            x0: x,
            z0: z,
            x1: x,
            z1: z,
        }
    }
}

port! {
    /// Remove an occupant's consumption, wire footprint, pole cost or plant record. The entry forwards
    /// the caller's argument word because interface queries and the requirement lookup overwrite it.
    /// EAX retains the last call or explicit word loaded on every exit.
    #[port(address = 0x006E_BF60, owner = "power", name = "cSC4PowerSimulator::DoOccupantRemoved", naked)]
    pub unsafe extern "C" fn fc4p_006ebf60() {
        core::arch::naked_asm!(
            "mov edx, eax",
            "lea eax, [esp + 4]",
            "push eax",
            "call {body}",
            "ret 4",
            body = sym removed,
        )
    }
}

unsafe extern "fastcall" fn removed(this: *mut u8, entry_eax: u32, arg: *mut u32) -> u32 {
    unsafe {
        let sim = OccupantSim::from_ptr(this);
        let power = PowerSim::from_ptr(this);
        if sim.initialized() == 0 {
            return entry_eax & !255;
        }
        let occupant = arg.read() as *mut u8;
        let kind = vcall!(occupant, 0x1C => fn() -> u32);
        match kind {
            0x2890_D4DE | 0x0990_B2EA | 0xC772_BF98 => {
                let iid = match kind {
                    0x2890_D4DE => 0xA8C8_6142,
                    0x0990_B2EA => 0xC98F_3B24,
                    _ => {
                        arg.write(0);
                        0xA821_EF94
                    }
                };
                let eax = vcall!(occupant, 0 => fn(u32, *mut u32) -> u32, iid, arg);
                if eax as u8 == 0 {
                    return eax;
                }
                let interface = arg.read() as *mut u8;
                let relevant = match kind {
                    0x2890_D4DE => vcall!(interface, 0x34 => fn() -> u32) as u8 == 0,
                    0x0990_B2EA => vcall!(interface, 0x18 => fn() -> u32) as u8 == 0,
                    _ => {
                        vcall!(interface, 0x20 => fn(u32) -> u32, 0x8000_0000) as u8 == 0
                            && vcall!(arg.read() as *mut u8, 0x20 => fn(u32) -> u32, 0x40000) as u8 != 0
                    }
                };
                if relevant {
                    let slot = match kind {
                        0x2890_D4DE => 0x20,
                        0x0990_B2EA => 0x14,
                        _ => 0xEC,
                    };
                    let mut rect = cell_rect(arg.read() as *mut u8, slot);
                    // Network removal invalidates the cell alone; flora and poles expand it first.
                    if kind != 0xC772_BF98 {
                        expand(this, &mut rect);
                    }
                    invalidate(this, &mut rect);
                    if kind == 0x2890_D4DE {
                        let (mut cursor, end) = (power.poles_begin(), power.poles_end());
                        let pole = arg.read() as *mut u8;
                        while cursor != end {
                            if cursor.read() == pole {
                                vcall!(pole, 8 => fn() -> u32);
                                let end = power.poles_end();
                                let next = cursor.wrapping_add(1);
                                if end != next {
                                    move_bytes(cursor.cast(), next.cast(), (end as u32).wrapping_sub(next as u32));
                                }
                                power.set_poles_end(power.poles_end().wrapping_sub(1));
                                power.set_pole_count(power.pole_count().wrapping_sub(1));
                                break;
                            }
                            cursor = cursor.wrapping_add(1);
                        }
                        let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                        let mut cost = 0u64;
                        get_int64(holder, 0x0913_2DE8, &mut cost);
                        power.set_infrastructure_cost(power.infrastructure_cost().wrapping_sub(cost));
                    }
                    power.set_recompute(1);
                }
                vcall!(arg.read() as *mut u8, 8 => fn() -> u32)
            }
            0x2781_28A0 => {
                let (x, z) = centre(sim, occupant);
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                arg.write(0);
                let mut capacity = 0u32;
                get_uint(holder, 0x2781_2854, arg);
                get_uint(holder, 0x2781_2852, &mut capacity);
                let consumed = arg.read();
                if consumed > 0 && consumed > capacity {
                    let row = sim
                        .consumption_rows()
                        .wrapping_offset((x.wrapping_mul(12)) as isize)
                        .cast::<*mut i16>()
                        .read_unaligned();
                    let cell = row.wrapping_offset(z as isize);
                    cell.write_unaligned(cell.read_unaligned().wrapping_sub(consumed as i16));
                    let consumed = arg.read();
                    if consumed > 0 {
                        sim.set_consumers(sim.consumers().wrapping_sub(1).max(0));
                        sim.set_requirement(sim.requirement().wrapping_sub(consumed as i32).max(0));
                    }
                }
                let mut rect = core::mem::MaybeUninit::<Rect>::uninit();
                vcall!(occupant, 0x2C => fn(*mut Rect) -> u32, rect.as_mut_ptr());
                expand(this, rect.as_mut_ptr());
                invalidate(this, rect.as_mut_ptr());
                let budget = *budget();
                let mut record = core::mem::MaybeUninit::<[u32; 8]>::uninit();
                let record_ptr = record.as_mut_ptr().cast::<u32>();
                record_ptr.add(7).write(0);
                if vcall!(budget, 0xCC => fn(*mut u8, u32, *mut u32) -> u32, holder, 0xCA58_E540, record_ptr) as u8 != 0
                {
                    let department = vcall!(budget, 0x70 => fn(u32) -> *mut u8, record_ptr.read());
                    if !department.is_null() {
                        let item = vcall!(department, 0x50 => fn(u32) -> *mut u8, record_ptr.add(1).read());
                        if !item.is_null() {
                            vcall!(department, 0x5C => fn(*mut u8, u32) -> u32, holder, 0xCA58_E540);
                            // Both calls use the table selected before the count getter.
                            let table = item.cast::<*const usize>().read();
                            let get = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8) -> u64>(
                                *table.add(0x20 / 4),
                            );
                            let count = get(item).wrapping_sub(1);
                            let set = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u64) -> u32>(
                                *table.add(0x24 / 4),
                            );
                            set(item, count);
                        }
                    }
                }
                let header = power.plant_header();
                let mut node = PlantLink::from_ptr(header).next();
                while node != header {
                    let link = PlantLink::from_ptr(node);
                    if link.occupant() == occupant {
                        let mut kind = 0i32;
                        get_int(holder, 0x2781_2853, &mut kind);
                        vcall!(link.occupant(), 8 => fn() -> u32);
                        let (next, previous) = (link.next(), link.previous());
                        PlantLink::from_ptr(previous).set_next(next);
                        PlantLink::from_ptr(next).set_previous(previous);
                        free(node);
                        let counter = this
                            .wrapping_offset(0x2E0i32.wrapping_add(kind.wrapping_mul(4)) as isize)
                            .cast::<u32>();
                        counter.write_unaligned(counter.read_unaligned().wrapping_sub(1));
                        sim.set_plant_count(sim.plant_count().wrapping_sub(1));
                        power.set_recompute(1);
                        return kind as u32;
                    }
                    node = link.next();
                }
                header as u32
            }
            _ => kind,
        }
    }
}

port! {
    /// Add a wire footprint and account for consumers, poles, and plants. Interface queries and the
    /// requirement lookup reuse the original caller's argument word. Active poles retain their queried
    /// reference in the pole vector; other queried interfaces are released before returning.
    #[port(address = 0x006E_CC50, owner = "power", name = "cSC4PowerSimulator::DoOccupantInserted", naked)]
    pub unsafe extern "C" fn fc4p_006ecc50() {
        core::arch::naked_asm!(
            "mov edx, eax", "lea eax, [esp + 4]", "push eax", "call {body}", "ret 4",
            body = sym inserted,
        )
    }
}

unsafe extern "fastcall" fn inserted(this: *mut u8, entry_eax: u32, arg: *mut u32) -> u32 {
    unsafe {
        let sim = OccupantSim::from_ptr(this);
        let power = PowerSim::from_ptr(this);
        if sim.initialized() == 0 {
            return entry_eax & !255;
        }
        let occupant = arg.read() as *mut u8;
        let kind = vcall!(occupant, 0x1C => fn() -> u32);
        match kind {
            0x2890_D4DE | 0x0990_B2EA | 0xC772_BF98 => {
                let iid = match kind {
                    0x2890_D4DE => 0xA8C8_6142,
                    0x0990_B2EA => 0xC98F_3B24,
                    _ => {
                        arg.write(0);
                        0xA821_EF94
                    }
                };
                let eax = vcall!(occupant, 0 => fn(u32, *mut u32) -> u32, iid, arg);
                if eax as u8 == 0 {
                    return eax;
                }
                let interface = arg.read() as *mut u8;
                let relevant = match kind {
                    0x2890_D4DE => vcall!(interface, 0x34 => fn() -> u32) as u8 == 0,
                    0x0990_B2EA => vcall!(interface, 0x18 => fn() -> u32) as u8 == 0,
                    _ => {
                        vcall!(interface, 0x20 => fn(u32) -> u32, 0x8000_0000) as u8 == 0
                            && vcall!(arg.read() as *mut u8, 0x20 => fn(u32) -> u32, 0x40000) as u8 != 0
                    }
                };
                if relevant {
                    let slot = match kind {
                        0x2890_D4DE => 0x20,
                        0x0990_B2EA => 0x14,
                        _ => 0xEC,
                    };
                    let mut rect = cell_rect(arg.read() as *mut u8, slot);
                    if kind != 0xC772_BF98 {
                        expand(this, &mut rect);
                    }
                    set_wire(this, &mut rect);
                    if kind == 0x2890_D4DE {
                        push_pole(this.add(0x29C), arg);
                        power.set_pole_count(power.pole_count().wrapping_add(1));
                        let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                        let mut cost = 0u64;
                        get_int64(holder, 0x0913_2DE8, &mut cost);
                        power.set_infrastructure_cost(power.infrastructure_cost().wrapping_add(cost));
                        power.set_recompute(1);
                        return (cost >> 32) as u32;
                    }
                    power.set_recompute(1);
                }
                vcall!(arg.read() as *mut u8, 8 => fn() -> u32)
            }
            0x2781_28A0 => {
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                let (x, z) = centre(sim, occupant);
                let mut rect = core::mem::MaybeUninit::<Rect>::uninit();
                vcall!(occupant, 0x2C => fn(*mut Rect) -> u32, rect.as_mut_ptr());
                expand(this, rect.as_mut_ptr());
                set_wire(this, rect.as_mut_ptr());
                arg.write(0);
                let mut capacity = 0u32;
                get_uint(holder, 0x2781_2854, arg);
                get_uint(holder, 0x2781_2852, &mut capacity);
                let consumed = arg.read();
                if consumed > 0 && consumed > capacity {
                    let row = sim
                        .consumption_rows()
                        .wrapping_offset(x.wrapping_mul(12) as isize)
                        .cast::<*mut i16>()
                        .read_unaligned();
                    let cell = row.wrapping_offset(z as isize);
                    cell.write_unaligned(cell.read_unaligned().wrapping_add(consumed as i16));
                    let consumed = arg.read();
                    if consumed > 0 {
                        sim.set_consumers(sim.consumers().wrapping_add(1));
                        sim.set_requirement(sim.requirement().wrapping_add(consumed as i32));
                        if vcall!(this, 0x94 => fn(i32, i32) -> u32, x, z) as u8 != 0 {
                            let lot = vcall!(*lots(), 0x38 => fn(*mut u8) -> *mut u8, occupant);
                            if !lot.is_null() {
                                vcall!(lot, 0x10C => fn(u32) -> u32, 1);
                            }
                        }
                    }
                }
                let consumed = arg.read();
                if capacity == 0 || capacity <= consumed {
                    return consumed;
                }
                let mut kind = 0i32;
                get_int(holder, 0x2781_2853, &mut kind);
                let mut value = [0u32, occupant as u32];
                vcall!(occupant, 4 => fn() -> u32);
                push_plant(this.add(0x298), value.as_mut_ptr());
                sim.set_plant_count(sim.plant_count().wrapping_add(1));
                let counter = this
                    .wrapping_offset(0x2E0i32.wrapping_add(kind.wrapping_mul(4)) as isize)
                    .cast::<u32>();
                counter.write_unaligned(counter.read_unaligned().wrapping_add(1));
                power.set_recompute(1);
                let budget = *budget();
                let mut record = core::mem::MaybeUninit::<[u32; 8]>::uninit();
                let record_ptr = record.as_mut_ptr().cast::<u32>();
                record_ptr.add(7).write(0);
                let eax = vcall!(budget, 0xCC => fn(*mut u8, u32, *mut u32) -> u32, holder, 0xCA58_E540, record_ptr);
                if eax as u8 == 0 {
                    return eax;
                }
                // The first word of the plant payload becomes the queried interface pointer.
                let eax = vcall!(occupant, 0 => fn(u32, *mut u32) -> u32, 0x87DD_A3A9, value.as_mut_ptr());
                if eax as u8 == 0 {
                    return eax;
                }
                let department = vcall!(budget, 0x70 => fn(u32) -> *mut u8, record_ptr.read());
                let mut line = record_ptr.add(1).read();
                if line == 0 {
                    line = vcall!(value[0] as *mut u8, 0x10 => fn() -> u32);
                    record_ptr.add(1).write(line);
                    vcall!(budget, 0xD0 => fn(*mut u8, u32, u32) -> u32, holder, record_ptr.add(6).read(), line);
                    line = record_ptr.add(1).read();
                }
                if line != 0 {
                    let mut item = vcall!(department, 0x50 => fn(u32) -> *mut u8, line);
                    if item.is_null() {
                        item = vcall!(department, 0x48 => fn(u32, u32) -> *mut u8, record_ptr.add(1).read(), 1);
                        vcall!(item, 0x48 => fn(u32, u32) -> u32, 2, 1);
                    }
                    vcall!(department, 0x58 => fn(*mut u8, u32) -> u32, holder, 0xCA58_E540);
                    let table = item.cast::<*const usize>().read();
                    let get = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8) -> u64>(
                        *table.add(0x20 / 4),
                    );
                    let count = get(item).wrapping_add(1);
                    let set = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u64) -> u32>(
                        *table.add(0x24 / 4),
                    );
                    set(item, count);
                    let cost = u64::from(record_ptr.add(4).read()) | (u64::from(record_ptr.add(5).read()) << 32);
                    set_int64(holder, 0x2A58_D350, cost);
                }
                vcall!(value[0] as *mut u8, 8 => fn() -> u32)
            }
            _ => kind,
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006ebf60::PORT, fc4p_006ecc50::PORT];

#[cfg(test)]
mod tests;

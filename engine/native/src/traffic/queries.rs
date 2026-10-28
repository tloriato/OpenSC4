// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Interfaces, boundary cells and ferry searches.

use super::*;

game_fn! { fn language_service() -> *mut u8 = "C" 0x0091_3D7A; }

port! {
    /// Published congestion grid.
    #[port(address = 0x0069_8CF0, owner = "traffic", name = "cSC4TrafficSimulator::GetCongestionMap", naked)]
    pub unsafe extern "C" fn fc4p_00698cf0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x48]", "ret");
    }
}
port! {
    /// Published air-polluting traffic grid.
    #[port(address = 0x006A_AC50, owner = "traffic", name = "cSC4TrafficSimulator::GetAirPollutingTrafficMap", naked)]
    pub unsafe extern "C" fn fc4p_006aac50() {
        core::arch::naked_asm!("mov eax, [ecx + 0x30]", "ret");
    }
}
port! {
    /// Orders two cells by x, then z. Only the low byte is the direction on the first two branches.
    #[port(address = 0x006D_6000, owner = "traffic", name = "CompareCellDirection")]
    pub unsafe extern "C" fn fc4p_006d6000(x0: i32, z0: i32, x1: i32, z1: i32) -> u32 {
        if x1 < x0 { x1 as u32 & !255 }
        else if x1 > x0 { (x1 as u32 & !255) | 2 }
        else { ((z1 as u32 & !255) | u32::from(z1 >= z0)).wrapping_mul(2).wrapping_add(1) }
    }
}
port! {
    /// A cardinal step, comparing the low sixteen bits of the coordinate differences.
    #[port(address = 0x0070_D780, owner = "traffic", name = "CardinalStep")]
    pub unsafe extern "C" fn fc4p_0070d780(x0: i32, z0: i32, x1: i32, z1: i32) -> i32 {
        match (x1.wrapping_sub(x0) as i16, z1.wrapping_sub(z0) as i16) {
            (0, -1) => 1, (0, 1) => 3, (-1, 0) => 0, (1, 0) => 2, _ => -1,
        }
    }
}
port! {
    /// Class identifier, leaving EDX unchanged.
    #[port(address = 0x0070_D7D0, owner = "traffic", name = "cSC4TrafficSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_0070d7d0() {
        core::arch::naked_asm!("mov eax, 0x6990c1aa", "ret");
    }
}
port! {
    /// Writes a supported interface before retaining the primary interface. Rejection keeps the output word.
    #[port(address = 0x0070_D7E0, owner = "traffic", name = "cSC4TrafficSimulator::QueryInterface")]
    pub unsafe extern "fastcall" fn fc4p_0070d7e0(this: *mut u8, entry_edx: u32, iid: u32, out: *mut *mut u8) -> u64 {
        let offset = match iid {
            1 | 0x6802_7F37 => 0,
            0x4522_94AA => 4,
            0xE4FD_A3D4 => 8,
            _ => {
                let eax = if iid > 0x6802_7F37 { iid } else { iid.wrapping_sub(0x4522_94AA) };
                return (u64::from(entry_edx) << 32) | u64::from(eax & !255);
            }
        };
        unsafe {
            out.write_unaligned(if this.is_null() { this } else { this.wrapping_add(offset) });
            let result = virtual_registers(this, 4, &[], word(this, 0), out as u32);
            (u64::from(result.edx) << 32) | u64::from((result.eax & !255) | 1)
        }
    }
}
port! {
    /// Releases one reference, or shuts down and invokes the deleting destructor when the signed count is at most one.
    #[port(address = 0x0070_E2F0, owner = "traffic", name = "cSC4TrafficSimulator::Release")]
    pub unsafe extern "fastcall" fn fc4p_0070e2f0(this: *mut u8, entry_edx: u32) -> u64 {
        unsafe {
            let sim = TrafficSim::from_ptr(this);
            if sim.references() > 1 {
                let next = sim.references().wrapping_sub(1);
                sim.set_references(next);
                (u64::from(entry_edx) << 32) | u64::from(next as u32)
            } else {
                let eax = vcall!(this, 0x10 => fn() -> u32);
                let result = virtual_registers(this, 0xAC, &[1], eax, word(this, 0));
                u64::from(result.edx) << 32
            }
        }
    }
}
port! {
    /// Moves one cell along a highway return lane, using the configured driving side and optional reversal.
    #[port(address = 0x0070_DD30, owner = "traffic", name = "cSC4TrafficSimulator::FindHighwayReturnTile")]
    pub unsafe extern "thiscall" fn fc4p_0070dd30(this: *mut u8, x: *mut i32, z: *mut i32, reverse: u32) -> u32 {
        unsafe {
            let sim = TrafficSim::from_ptr(this);
            let service = language_service();
            let language = vcall!(service, 0x98 => fn(u32) -> *mut u8, 0);
            let mut right = language.is_null() || vcall!(language, 0x64 => fn() -> u32) as u8 == 1;
            if reverse as u8 != 0 { right = !right; }
            let mut value = x.read_unaligned();
            if value < 0 || value > sim.max_x() {
                let delta: i32 = if (value < 0) == right { 1 } else { -1 };
                z.write_unaligned(z.read_unaligned().wrapping_add(delta));
                return z as u32;
            }
            let zv = z.read_unaligned();
            if zv < 0 || zv > sim.max_z() {
                let delta: i32 = if (zv > sim.max_z()) == right { 1 } else { -1 };
                value = value.wrapping_add(delta);
                x.write_unaligned(value);
            }
            value as u32
        }
    }
}
port! {
    /// Exactly one outside coordinate selects an edge and clamps it. Interior cells and corners leave outputs untouched.
    #[port(address = 0x0070_DDD0, owner = "traffic", name = "cSC4TrafficSimulator::GetNeighborConnectionInnerCell", naked)]
    pub unsafe extern "C" fn fc4p_0070ddd0() {
        core::arch::naked_asm!("sub esp, 12", "mov [esp + 8], edx", "mov edx, esp",
            "push [esp + 28]", "push [esp + 28]", "push [esp + 28]", "push [esp + 28]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 16",
            sym inner_cell);
    }
}
unsafe extern "fastcall" fn inner_cell(
    this: *mut u8,
    regs: *mut Registers,
    x: i32,
    z: i32,
    ox: *mut i32,
    oz: *mut i32,
) {
    unsafe {
        let sim = TrafficSim::from_ptr(this);
        let outside_x = x < 0 || x > sim.max_x();
        let outside_z = z < 0 || z > sim.max_z();
        let mut result = Registers {
            eax: u32::MAX,
            ecx: this as u32,
            edx: (*regs).edx,
        };
        if outside_x != outside_z {
            ox.write_unaligned(x);
            oz.write_unaligned(z);
            result.edx = ox as u32;
            result.eax = if outside_x {
                if x < 0 {
                    ox.write_unaligned(0);
                    0
                } else {
                    ox.write_unaligned(sim.max_x());
                    2
                }
            } else if z < 0 {
                oz.write_unaligned(0);
                1
            } else {
                result.ecx = sim.max_z() as u32;
                oz.write_unaligned(result.ecx as i32);
                3
            };
        }
        regs.write(result);
    }
}
port! {
    /// Returns an eight-byte edge record. East/west tests take precedence over the sign of z.
    #[port(address = 0x0070_F970, owner = "traffic", name = "cSC4TrafficSimulator::GetTrafficNeighborConnection")]
    pub unsafe extern "thiscall" fn fc4p_0070f970(this: *mut u8, x: i32, z: i32) -> *mut u8 {
        let s = unsafe { TrafficSim::from_ptr(this) };
        let (base, index) = if x < 0 { (s.west_connections(), z) }
            else if x > s.max_cell_x() { (s.east_connections(), z) }
            else if z < 0 { (s.north_connections(), x) } else { (s.south_connections(), x) };
        base.wrapping_add(index.wrapping_mul(8) as usize)
    }
}
port! {
    /// Finds the first ferry link with the requested sixteen-bit ID.
    #[port(address = 0x0070_F9C0, owner = "traffic", name = "cSC4TrafficSimulator::FindFerryLink", naked)]
    pub unsafe extern "C" fn fc4p_0070f9c0() {
        core::arch::naked_asm!("sub esp, 12", "mov edx, esp", "push [esp + 20]", "push [esp + 20]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 8", sym ferry_link);
    }
}
unsafe extern "fastcall" fn ferry_link(_this: *mut u8, regs: *mut Registers, terminal: *mut u8, id: u32) {
    unsafe {
        let header = word(terminal, 0x10) as *mut u8;
        let mut node = word(header, 0) as *mut u8;
        let mut eax = 0;
        while node != header {
            if node.add(8).cast::<u16>().read_unaligned() == id as u16 {
                eax = node.wrapping_add(8) as u32;
                break;
            }
            node = word(node, 0) as *mut u8;
        }
        regs.write(Registers {
            eax,
            ecx: node as u32,
            edx: header as u32,
        });
    }
}
port! {
    /// Finds the first ferry neighbor link containing either endpoint pair.
    #[port(address = 0x0070_F9F0, owner = "traffic", name = "cSC4TrafficSimulator::FindFerryNeighborLink", naked)]
    pub unsafe extern "C" fn fc4p_0070f9f0() {
        core::arch::naked_asm!("sub esp, 12", "mov [esp + 8], edx", "mov edx, esp",
            "push [esp + 24]", "push [esp + 24]", "push [esp + 24]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 12", sym ferry_neighbor);
    }
}
unsafe extern "fastcall" fn ferry_neighbor(_this: *mut u8, regs: *mut Registers, terminal: *mut u8, x: u32, z: u32) {
    unsafe {
        let header = word(terminal, 0x14) as *mut u8;
        let mut node = word(header, 0) as *mut u8;
        let edx = if node == header {
            (*regs).edx
        } else {
            ((*regs).edx & 0xFFFF_0000) | (x & 0xFFFF)
        };
        let mut eax = 0;
        while node != header {
            let a = node.add(8).cast::<u16>();
            if (a.read_unaligned() == x as u16 && a.add(1).read_unaligned() == z as u16)
                || (a.add(2).read_unaligned() == x as u16 && a.add(3).read_unaligned() == z as u16)
            {
                eax = a as u32;
                break;
            }
            node = word(node, 0) as *mut u8;
        }
        regs.write(Registers {
            eax,
            ecx: node as u32,
            edx,
        });
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00698cf0::PORT,
    fc4p_006aac50::PORT,
    fc4p_006d6000::PORT,
    fc4p_0070d780::PORT,
    fc4p_0070d7d0::PORT,
    fc4p_0070d7e0::PORT,
    fc4p_0070dd30::PORT,
    fc4p_0070ddd0::PORT,
    fc4p_0070e2f0::PORT,
    fc4p_0070f970::PORT,
    fc4p_0070f9c0::PORT,
    fc4p_0070f9f0::PORT,
];

#[cfg(test)]
mod tests;

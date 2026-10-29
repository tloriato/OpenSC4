// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction, service initialization and boundary texture rules.

use super::{Port, ZoneDeveloper};
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn boundary_rule(this: *mut u8, key: *const u32) -> *mut u32 = "thiscall" 0x0072_F850; }
game_fn! {
    #[allow(dead_code)]
    fn initialize_game(this: *mut u8) -> u32 = "thiscall" 0x0073_4030;
}
game_fn! { fn allocate_grid(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn grid_new(this: *mut u8, iid: u32) -> *mut u8 = "thiscall" 0x0045_F9B0; }
game_fn! { fn resize_rows(this: *mut u8, count: i32) -> u32 = "thiscall" 0x0073_4000; }
game_fn! { fn resize_cells(this: *mut u8, count: i32, value: *const u32) -> u32 = "thiscall" 0x0049_FC70; }
game_fn! { fn initialize_rules(this: *mut u8) -> u32 = "thiscall" 0x0072_FF80; }
game_fn! { fn string_new(this: *mut u8, text: *const u8) -> *mut u8 = "thiscall" 0x0040_8480; }
game_fn! { fn free(data: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn reference_set_new(this: *mut u8, buckets: u32) -> u32 = "thiscall" 0x0082_9BC0; }
game_fn! { fn reference_set_clear(this: *mut u8) -> u32 = "thiscall" 0x008E_BA60; }
game_fn! { fn reference_set_reserve(this: *mut u8, count: u32) -> u32 = "thiscall" 0x0044_8250; }
game_fn! { fn reference_set_insert(this: *mut u8, result: *mut u8, value: *const u32) -> u32 = "thiscall" 0x0072_BA50; }
game_fn! { fn rows_resize(this: *mut u8, count: u32, value: *const u32) -> u32 = "thiscall" 0x005D_1290; }
game_fn! { fn cells_append(this: *mut u8, end: *mut u8, count: u32, value: *const u32) -> u32 = "thiscall" 0x004F_BA20; }
game_fn! {
    fn read_reference(stream: *mut u8, iid: u32, out: *mut *mut u8, immediate: u32) -> u32
        = "C" 0x005D_9730;
}

game_data! { static started: u8 = 0x00B4_B390; }
game_data! { static lot_configuration: *mut u8 = 0x00B4_3D20; }
game_data! { static service_1c: *mut u8 = 0x00B4_3DD0; }
game_data! { static service_20: *mut u8 = 0x00B4_3D68; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_data! { static occupant_manager: *mut u8 = 0x00B4_3D0C; }
game_data! { static terrain: *mut u8 = 0x00B4_3CF4; }
game_data! { static service_3c: *mut u8 = 0x00B4_3DA8; }
game_data! { static zone_manager: *mut u8 = 0x00B4_3D14; }
game_data! { static message_server: *mut u8 = 0x00B4_3CCC; }
game_data! { static commands: *mut u8 = 0x00B4_3CAC; }
game_data! { static application: *mut u8 = 0x00B4_3C94; }
game_data! { static command_name_a: *const u8 = 0x00B0_D4A4; }
game_data! { static command_name_b: *const u8 = 0x00B0_D4A8; }

layout! {
    /// Initialization fields of the primary zone developer interface.
    struct Initialization {
        0x10 => initialized / set_initialized: u8,
        0x1C => service_1c / set_service_1c: *mut u8,
        0x20 => service_20 / set_service_20: *mut u8,
        0x3C => service_3c / set_service_3c: *mut u8,
        0x54 => rows / set_rows: *mut u8,
        0x58 => rows_end / set_rows_end: *mut u8,
        0x74 => grid / set_grid: *mut u8,
        0xD4 => service_data / set_service_data: u32,
    }
}

layout! {
    /// Loading fields relative to the serializable interface, twelve bytes after the primary interface.
    struct SerializableDeveloper {
        0x05 => loading / set_loading: u8,
        0x48 => rows / set_rows: *mut u8,
        0x54 => lot_list / set_lot_list: *mut u8,
        0x5C => lot_count / set_lot_count: u32,
    }
}

port! {
    /// Restore the lot rows, retain each distinct lot once, and return the stream's success byte.
    #[port(address = 0x0073_4380, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::Read", naked)]
    pub unsafe extern "C" fn fc4p_00734380() {
        core::arch::naked_asm!(
            "mov edx, esp", "sub esp, 76", "call {body}", "add esp, 76", "ret 4",
            body = sym read,
        );
    }
}

unsafe extern "fastcall" fn read(this: *mut u8, stack: *mut u8) -> u32 {
    unsafe {
        let dev = SerializableDeveloper::from_ptr(this);
        let stream = read_unaligned(stack.add(4).cast::<*mut u8>());
        let version = stack.sub(60).cast::<u16>();
        dev.set_loading(1);
        let eax = vcall!(stream, 0x1C => fn(*mut u16) -> u32, version);
        if version.read() != 1 {
            return eax & !0xFF;
        }
        let set = stack.sub(20);
        for offset in [4, 8, 12, 16] {
            store(set, offset, 0);
        }
        reference_set_new(set, 100);
        let x_count = stack.sub(44).cast::<u32>();
        let z_count = stack.sub(52).cast::<u32>();
        vcall!(stream, 0x24 => fn(*mut u32) -> u32, x_count);
        vcall!(stream, 0x24 => fn(*mut u32) -> u32, z_count);
        let result = stack.sub(32);
        for offset in [0, 4, 8] {
            store(result, offset, 0);
        }
        rows_resize(this.add(0x48), x_count.read(), result.cast());
        let x_index = stack.sub(40).cast::<u32>();
        x_index.write(0);
        if x_count.read() != 0 {
            let zero = stack.sub(36).cast::<u32>();
            zero.write(0);
            let row_offset = stack.sub(48).cast::<u32>();
            row_offset.write(0);
            loop {
                let row = dev.rows().wrapping_add(row_offset.read() as usize);
                let begin = read_unaligned(row.cast::<*mut u8>());
                let end = read_unaligned(row.add(4).cast::<*mut u8>());
                let size = ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32;
                let z = z_count.read();
                if z < size {
                    store(row, 4, (begin as u32).wrapping_add(z.wrapping_mul(4)));
                } else {
                    cells_append(row, end, z.wrapping_sub(size), zero);
                }
                let mut z = 0u32;
                while z < z_count.read() {
                    let value = stack.sub(56).cast::<*mut u8>();
                    value.write(core::ptr::null_mut());
                    read_reference(stream, 0xE818_FCED, value, 0);
                    let cells = read_unaligned(row.cast::<*mut u32>());
                    write_unaligned(cells.wrapping_add(z as usize), value.read() as u32);
                    if !value.read().is_null() {
                        let count = read_unaligned(set.add(16).cast::<u32>());
                        reference_set_reserve(set, count.wrapping_add(1));
                        reference_set_insert(set, result, value.cast());
                        if *result.add(8) == 0 {
                            vcall!(value.read(), 8 => fn() -> u32);
                        } else {
                            let header = dev.lot_list();
                            let node = allocate(12);
                            let payload = node.wrapping_add(8).cast::<u32>();
                            if !payload.is_null() {
                                write_unaligned(payload, value.read() as u32);
                            }
                            let previous = read_unaligned(header.add(4).cast::<*mut u8>());
                            store(node, 0, header as u32);
                            store(node, 4, previous as u32);
                            store(previous, 0, node as u32);
                            store(header, 4, node as u32);
                            dev.set_lot_count(dev.lot_count().wrapping_add(1));
                        }
                    }
                    z = z.wrapping_add(1);
                }
                let next = x_index.read().wrapping_add(1);
                row_offset.write(row_offset.read().wrapping_add(12));
                x_index.write(next);
                if next >= x_count.read() {
                    break;
                }
            }
        }
        let success = u32::from(vcall!(stream, 0x48 => fn() -> u32) as u8 == 0);
        reference_set_clear(set);
        let data = read_unaligned(set.add(4).cast::<*mut u8>());
        let eax = if data.is_null() { 0 } else { free(data) };
        (eax & !0xFF) | success
    }
}

port! {
    /// Retain city services, initialize the lot grid and register messages and commands.
    /// Preserve the upper return bytes and the frame passed to row and string helpers.
    #[port(address = 0x0073_4030, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::Init", naked)]
    pub unsafe extern "C" fn fc4p_00734030() {
        core::arch::naked_asm!(
            "mov edx, eax", "mov eax, esp", "sub esp, 40", "push eax",
            "call {body}", "add esp, 40", "ret", body = sym initialize,
        );
    }
}

unsafe extern "fastcall" fn initialize(this: *mut u8, entry_eax: u32, stack: *mut u8) -> u32 {
    unsafe {
        let dev = ZoneDeveloper::from_ptr(this);
        let init = Initialization::from_ptr(this);
        if init.initialized() != 0 {
            return (entry_eax & !0xFF) | 1;
        }
        let mut eax = (entry_eax & !0xFF) | u32::from(*started());
        if *started() == 0 {
            *started() = 1;
        }
        init.set_service_data(0);
        let incoming = *lot_configuration();
        let previous = dev.lot_configuration();
        if incoming != previous {
            if !incoming.is_null() {
                eax = vcall!(incoming, 4 => fn() -> u32);
            }
            dev.set_lot_configuration(incoming);
            if !previous.is_null() {
                eax = vcall!(previous, 8 => fn() -> u32);
            }
        }
        if init.service_1c().is_null() {
            let p = *service_1c();
            init.set_service_1c(p);
            eax = p as u32;
            if p.is_null() {
                return eax & !0xFF;
            }
            eax = vcall!(p, 4 => fn() -> u32);
            let p = *service_1c();
            if !p.is_null() {
                eax = vcall!(p, 0x28 => fn() -> u32);
            }
            init.set_service_data(if p.is_null() || eax == 0 {
                0
            } else {
                read_unaligned((eax as *const u8).add(0x14).cast::<u32>())
            });
        }
        if init.service_20().is_null() {
            let p = *service_20();
            init.set_service_20(p);
            if p.is_null() {
                return eax & !0xFF;
            }
            eax = vcall!(p, 4 => fn() -> u32);
        }
        if dev.city().is_null() {
            let p = *city();
            dev.set_city(p);
            if p.is_null() {
                return eax & !0xFF;
            }
            vcall!(p, 4 => fn() -> u32);
            let x = vcall!(dev.city(), 0x174 => fn() -> i32);
            dev.set_cells_x(x);
            eax = vcall!(dev.city(), 0x178 => fn() -> u32);
            dev.set_cells_z(eax as i32);
        }
        for (offset, global) in [
            (0x30, occupant_manager()),
            (0x34, terrain()),
            (0x3C, service_3c()),
            (0x40, zone_manager()),
        ] {
            if read_unaligned(this.add(offset).cast::<*mut u8>()).is_null() {
                let p = *global;
                store(this, offset, p as u32);
                if p.is_null() {
                    return 0;
                }
                vcall!(p, 4 => fn() -> u32);
            }
        }
        if init.rows() == init.rows_end() {
            resize_rows(this.add(0x54), dev.cells_x());
            let count = (init.rows_end() as u32).wrapping_sub(init.rows() as u32) as i32 / 12;
            if count > 0 {
                let zero = stack.sub(24).cast::<u32>();
                zero.write(0);
                for row in 0..count {
                    resize_cells(
                        init.rows().wrapping_offset(row.wrapping_mul(12) as isize),
                        dev.cells_z(),
                        zero,
                    );
                }
            }
        } else {
            let mut x = 0i32;
            while x < dev.cells_x() {
                let mut z = 0i32;
                while z < dev.cells_z() {
                    let lot = vcall!(this.add(4), 0x18 => fn(i32, i32, u32) -> *mut u8, x, z, 1);
                    if !lot.is_null() {
                        vcall!(lot, 0x14 => fn() -> u32);
                    }
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
        }
        if init.grid().is_null() {
            let p = allocate_grid(0x44);
            let p = if p.is_null() { p } else { grid_new(p, 0x49B9_E600) };
            init.set_grid(p);
            vcall!(p, 4 => fn() -> u32);
            vcall!(init.grid(), 0x0C => fn() -> u32);
            vcall!(init.grid(), 0x24 => fn(u32) -> u32, 4);
            vcall!(init.grid(), 0x18 => fn(u32) -> u32, 0x69D5_C456);
        }
        initialize_rules(this);
        eax = dev.message_server() as u32;
        if eax == 0 {
            let p = *message_server();
            dev.set_message_server(p);
            if p.is_null() {
                return 0;
            }
            vcall!(p, 4 => fn() -> u32);
            for id in [0x99EF_1142, 0x26D3_1EC1, 0xA6B7_9602] {
                eax = vcall!(dev.message_server(), 0x14 => fn(*mut u8, u32) -> u32, this.add(8), id);
            }
        }
        let command = *commands();
        if !command.is_null() {
            let app = *application();
            eax = super::virtual_registers(app, 0x54, eax, read_unaligned(app.cast()), &[]) as u32;
            if eax as u8 != 0 {
                for (id, text) in [(0x8929_E68E, command_name_a()), (0x4AB5_69C9, command_name_b())] {
                    let string = stack.sub(20);
                    let string = string_new(string, *text);
                    super::virtual_registers(command, 0x0C, string as u32, 0, &[id, string as u32]);
                    let data = read_unaligned(stack.sub(16).cast::<*mut u8>());
                    store(stack.sub(20), 0, 0x00A8_0810);
                    eax = if data.is_null() { 0 } else { free(data) };
                    eax = super::virtual_registers(
                        command,
                        0x1C,
                        eax,
                        read_unaligned(command.cast()),
                        &[this.add(8) as u32, id],
                    ) as u32;
                }
            }
        }
        init.set_initialized(1);
        (eax & !0xFF) | 1
    }
}

unsafe fn store(this: *mut u8, offset: usize, value: u32) {
    unsafe { write_unaligned(this.add(offset).cast::<u32>(), value) }
}

port! {
    /// Initialize interfaces, empty containers and zoning defaults. Retain padding and unused fields.
    #[port(address = 0x0073_48D0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::cSC4ZoneDeveloper")]
    pub unsafe extern "thiscall" fn fc4p_007348d0(this: *mut u8) -> *mut u8 {
        unsafe {
            for (offset, value) in [(4, 0x00AB_3710), (8, 0x00A8_1174), (12, 0x00A8_0784)] {
                this.add(offset).cast::<u32>().write_volatile(value);
            }
            let dev = ZoneDeveloper::from_ptr(this);
            dev.set_primary_vtable(0x00AB_3830);
            dev.set_lot_manager_vtable(0x00AB_37C0);
            dev.set_message_vtable(0x00AB_37AC);
            dev.set_serializable_vtable(0x00AB_3794);
            *this.add(0x10) = 0;
            *this.add(0x11) = 0;
            for offset in (0x14..=0x60).step_by(4) { store(this, offset, 0); }
            let list = allocate(12);
            store(list, 0, list as u32);
            store(list, 4, list as u32);
            store(this, 0x60, list as u32);
            store(this, 0x64, 0);
            let list = allocate(12);
            store(list, 0, list as u32);
            store(list, 4, list as u32);
            store(this, 0x64, list as u32);
            for offset in [0x68, 0x6C, 0x70, 0x74, 0x78] { store(this, offset, 0); }
            construct_tree(this, 0x78, 0x7C, 0x20);
            for (offset, value) in [(0x84, 2), (0x88, 2), (0x8C, 1), (0x90, 3), (0x94, 6)] {
                store(this, offset, value);
            }
            for (offset, value) in [(0x98, 0), (0x99, 1), (0x9A, 1), (0x9B, 0), (0x9C, 0)] {
                *this.add(offset) = value;
            }
            for offset in [0xA0, 0xA4, 0xA8, 0xCC, 0xD0, 0xD8] { store(this, offset, 0); }
            construct_tree(this, 0xD8, 0xDC, 0x24);
            store(this, 0xE4, 0);
            construct_tree(this, 0xE4, 0xE8, 0x24);
            store(this, 0xF0, 0);
            construct_tree(this, 0xF0, 0xF4, 0x1C);
            for offset in [0xAC, 0xB0, 0xB4, 0xB8] { store(this, offset, 0); }
            this
        }
    }
}

unsafe fn construct_tree(this: *mut u8, header: usize, count: usize, size: u32) {
    unsafe {
        let node = allocate(size);
        store(this, header, node as u32);
        store(this, count, 0);
        *node = 0;
        store(node, 4, 0);
        store(node, 8, node as u32);
        store(node, 12, node as u32);
    }
}

port! {
    /// Forward the lot manager interface to the primary initialization method.
    #[port(address = 0x0073_48C0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::Init (lot manager interface)", naked)]
    pub unsafe extern "C" fn fc4p_007348c0() {
        core::arch::naked_asm!("sub ecx, 4", "push 0x00734030", "ret");
    }
}

port! {
    /// Populate the boundary map in texture order. Rule padding retains the incoming frame bytes.
    #[port(address = 0x0072_FF80, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::InitBoundaryRules", naked)]
    pub unsafe extern "C" fn fc4p_0072ff80() {
        core::arch::naked_asm!(
            "mov edx, esp", "sub esp, 24", "call {body}", "add esp, 24", "ret",
            body = sym initialize_boundary_rules,
        );
    }
}

unsafe extern "fastcall" fn initialize_boundary_rules(this: *mut u8, stack: *mut u8) -> *mut u32 {
    unsafe {
        let key = stack.sub(12).cast::<u32>();
        let flags = stack.sub(4);
        let mut result = core::ptr::null_mut();
        for &(mask, texture, orientation, mirrored) in BOUNDARY_RULES {
            write_unaligned(key, mask);
            *flags = orientation;
            *flags.add(1) = mirrored;
            result = boundary_rule(this.add(0xF0), key);
            write_unaligned(result, texture);
            write_unaligned(result.add(1), read_unaligned(flags.cast::<u32>()));
        }
        result
    }
}

const BOUNDARY_RULES: &[(u32, u32, u8, u8)] = &[
    (0x0000, 0x3000, 0, 0),
    (0x1000, 0x2000, 0, 0),
    (0x0100, 0x2000, 1, 0),
    (0x0010, 0x2000, 2, 0),
    (0x0001, 0x2000, 3, 0),
    (0x2000, 0x2600, 0, 0),
    (0x0200, 0x2600, 1, 0),
    (0x0020, 0x2600, 2, 0),
    (0x0002, 0x2600, 3, 0),
    (0x1100, 0x2100, 1, 0),
    (0x0110, 0x2100, 2, 0),
    (0x0011, 0x2100, 3, 0),
    (0x1001, 0x2100, 0, 0),
    (0x1200, 0x2400, 1, 0),
    (0x0120, 0x2400, 2, 0),
    (0x0012, 0x2400, 3, 0),
    (0x2001, 0x2400, 0, 0),
    (0x2100, 0x2400, 0, 1),
    (0x0210, 0x2400, 3, 1),
    (0x0021, 0x2400, 2, 1),
    (0x1002, 0x2400, 1, 1),
    (0x1010, 0x2500, 0, 0),
    (0x0101, 0x2500, 1, 0),
    (0x1020, 0x2900, 2, 0),
    (0x0102, 0x2900, 3, 0),
    (0x2010, 0x2900, 0, 0),
    (0x0201, 0x2900, 1, 0),
    (0x1110, 0x2200, 1, 0),
    (0x0111, 0x2200, 2, 0),
    (0x1011, 0x2200, 3, 0),
    (0x1101, 0x2200, 0, 0),
    (0x1120, 0x2700, 2, 0),
    (0x0112, 0x2700, 3, 0),
    (0x2011, 0x2700, 0, 0),
    (0x1201, 0x2700, 1, 0),
    (0x1210, 0x2800, 1, 0),
    (0x0121, 0x2800, 2, 0),
    (0x1012, 0x2800, 3, 0),
    (0x2101, 0x2800, 0, 0),
    (0x2110, 0x2700, 0, 1),
    (0x0211, 0x2700, 3, 1),
    (0x1021, 0x2700, 2, 1),
    (0x1102, 0x2700, 1, 1),
    (0x1112, 0x2300, 3, 0),
    (0x2111, 0x2300, 0, 0),
    (0x1211, 0x2300, 1, 0),
    (0x1121, 0x2300, 2, 0),
];

pub const PORTS: &[Port] = &[
    fc4p_0072ff80::PORT,
    fc4p_007348c0::PORT,
    fc4p_007348d0::PORT,
];

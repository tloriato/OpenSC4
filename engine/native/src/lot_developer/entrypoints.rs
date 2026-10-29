// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction, empty textures and messages on the game's lot developer.

use super::*;

game_fn! { #[allow(dead_code)] fn pick_unit(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_B260; }
game_fn! { #[allow(dead_code)] fn post_message(kind: u32, a: u32, b: u32, c: u32, d: u32, object: *mut u8) -> u64 = "C" 0x005F_98C0; }
game_fn! { #[allow(dead_code)] fn remove_prop(this: *mut u8, lot: *mut u8, kind: u32) -> u64 = "thiscall" 0x006C_9780; }
game_fn! { #[allow(dead_code)] fn clear_blockers(this: *mut u8, lot: *mut u8, config: *mut u8, kind: u32, foundation: u32) -> u64 = "thiscall" 0x006C_9AD0; }
game_fn! { #[allow(dead_code)] fn com() -> u64 = "C" 0x0090_DDF1; }
game_fn! { #[allow(dead_code)] fn allocate_construction(size: u32) -> u64 = "C" 0x005C_A8A0; }
game_fn! { #[allow(dead_code)] fn initialize_construction(this: *mut u8, enabled: u32) -> u64 = "thiscall" 0x005D_00F0; }
game_data! { static zero: f32 = 0x00A8_1054; }

unsafe fn direct(address: u32, conv: Conv, this: u32, args: &[u32], previous: RawCall) -> RawCall {
    let mut call = RawCall::new(conv, crate::abi::target(address), this, args);
    call.eax = previous.eax;
    call.edx = previous.edx;
    unsafe { call_raw(&mut call) };
    call
}

unsafe fn word(frame: *mut u8, offset: usize) -> u32 {
    unsafe { frame.add(offset).cast::<u32>().read_unaligned() }
}

unsafe fn put(frame: *mut u8, offset: usize, value: u32) {
    unsafe { frame.add(offset).cast::<u32>().write_unaligned(value) };
}

fn compare(value: F80, operand: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand,
            out("ax") status, out("st(0)") _,
        );
    }
    status
}

fn compare_zero(value: F80, operand: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fld dword ptr [{operand}]", "fucompp", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand,
            out("ax") status, out("st(0)") _, out("st(1)") _,
        );
    }
    status
}

fn status_eax(call: &mut RawCall, status: u16) {
    call.eax = (call.eax & 0xFFFF_0000) | u32::from(status);
}

port! {
    /// Keeps the original argument slots and local scratch addresses available to the Rust body.
    #[port(address = 0x006C_C490, owner = "lotdeveloper", name = "cSC4LotDeveloper::StartConstruction", naked)]
    pub unsafe extern "C" fn fc4p_006cc490() {
        core::arch::naked_asm!(
            "sub esp, 0x30", "push ebx", "push ebp", "push esi", "push edi",
            "push edx", "lea edx, [esp + 4]", "call {}", "add esp, 4",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 0x30", "ret 16",
            sym start_construction,
        );
    }
}

pub(super) unsafe extern "fastcall" fn start_construction(this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let lot = word(frame, 0x44) as *mut u8;
        let config = word(frame, 0x48) as *mut u8;
        let kind = word(frame, 0x4C);
        frame.add(0x13).write(0);
        let mut call = virtual_call(lot, 4, &[], table(lot), frame.sub(4).cast::<u32>().read(), false);
        call = virtual_call(lot, 0x28, &[config as u32], call.eax, table(lot), false);
        call = virtual_call(lot, 0xBC, &[kind], table(lot), call.edx, false);
        call = virtual_call(lot, 0x88, &[2, 0, 0], call.eax, table(lot), false);
        call = virtual_call(lot, 0x90, &[1], table(lot), call.edx, false);
        call = direct(0x006C_B260, Conv::Thiscall, this as u32, &[lot as u32], call);
        call = virtual_call(lot, 0x30, &[], call.eax, table(lot), false);
        let flags = call.eax & !7;
        call = virtual_call(lot, 0x34, &[flags], flags, table(lot), false);
        call = virtual_call(lot, 0x114, &[1], table(lot), call.edx, false);
        call = virtual_call(lot, 0x0C, &[], call.eax, table(lot), false);
        let properties = call.eax as *mut u8;
        call = virtual_call(properties, 0x24, &[0xC999_C45E], call.eax, table(properties), false);
        if call.eax != 0 {
            let property = call.eax as *mut u8;
            call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
            let zot = call.eax as *mut u8;
            call = virtual_call(zot, 0x124, &[], table(zot), call.edx, false);
            put(frame, 0x44, (call.eax as *const u32).read_unaligned());
            call = direct(
                0x005F_98C0,
                Conv::Cdecl,
                word(frame, 0x44),
                &[0x4C04_DE69, 2, 0, 0, 0, lot as u32],
                call,
            );
            call.eax = word(frame, 0x44);
            if call.eax == 6 {
                call = direct(0x006C_9780, Conv::Thiscall, this as u32, &[lot as u32, 6], call);
                call = virtual_call(zot, 0x68, &[0], call.eax, table(zot), false);
            }
        }
        frame.add(0x14).write(0);
        let mut success = true;
        if kind != 0 {
            let bds = developer.building_development();
            call = virtual_call(bds, 0x9C, &[kind], table(bds), call.edx, false);
            frame
                .add(0x4C)
                .write(u8::from((call.eax as *const u32).read_unaligned() != 5));
            call = virtual_call(config, 0x38, &[], call.eax, table(config), false);
            frame.add(0x44).write(call.eax as u8);
            let mut foundation = false;
            if call.eax as u8 != 0 {
                call = virtual_call(config, 0x28, &[], table(config), call.edx, true);
                let status = compare_zero(call.st0, zero().read());
                status_eax(&mut call, status);
                if status & 0x4400 == 0x4000 {
                    call = virtual_call(
                        this,
                        0xD8,
                        &[lot as u32, config as u32, 0, 1, 1],
                        call.eax,
                        table(this),
                        false,
                    );
                    frame.add(0x14).write(call.eax as u8);
                    foundation = call.eax as u8 != 0;
                }
            }
            if !foundation {
                if *frame.add(0x4C) != 0 && *this.add(0xE1) != 0 {
                    virtual_call(lot, 0x38, &[frame.add(0x30) as u32], table(lot), call.edx, false);
                    if (word(frame, 0x38).wrapping_sub(word(frame, 0x30)) as i32) > 1
                        && (word(frame, 0x3C).wrapping_sub(word(frame, 0x34)) as i32) > 1
                    {
                        virtual_call(
                            this,
                            0x4C,
                            &[
                                frame.add(0x30) as u32,
                                (-1f32).to_bits(),
                                word(frame, 0x44),
                                0,
                                lot as u32,
                            ],
                            word(frame, 0x44),
                            table(this),
                            false,
                        );
                    }
                }
                call = virtual_call(
                    this,
                    0x78,
                    &[lot as u32, word(frame, 0x44)],
                    word(frame, 0x44),
                    table(this),
                    true,
                );
                put(frame, 0x18, call.st0.to_f32().to_bits());
                let status = compare(call.st0, developer.foundation_slope());
                status_eax(&mut call, status);
                if status & 0x4100 == 0 {
                    call = virtual_call(config, 0x28, &[], call.eax, table(config), true);
                    put(frame, 0x1C, call.st0.to_f32().to_bits());
                    let status = compare(
                        F80::from_f32(f32::from_bits(word(frame, 0x18))),
                        f32::from_bits(word(frame, 0x1C)),
                    );
                    status_eax(&mut call, status);
                    if status & 0x4100 == 0 {
                        call.eax = (call.eax & !255) | u32::from(*frame.add(0x4C));
                        if *frame.add(0x4C) != 0 {
                            call.eax = (call.eax & !255) | u32::from(*this.add(0xE0));
                            if *this.add(0xE0) != 0 {
                                virtual_call(lot, 0x38, &[frame.add(0x30) as u32], table(lot), call.edx, false);
                                call = virtual_call(
                                    this,
                                    0x54,
                                    &[
                                        frame.add(0x30) as u32,
                                        word(frame, 0x1C),
                                        1,
                                        word(frame, 0x44),
                                        developer.level_max_slope().to_bits(),
                                        lot as u32,
                                    ],
                                    word(frame, 0x1C),
                                    table(this),
                                    false,
                                );
                                if call.eax != 0 {
                                    call = virtual_call(
                                        this,
                                        0x78,
                                        &[lot as u32, word(frame, 0x44)],
                                        word(frame, 0x44),
                                        table(this),
                                        true,
                                    );
                                    put(frame, 0x18, call.st0.to_f32().to_bits());
                                }
                            }
                        }
                        let status = compare(
                            F80::from_f32(f32::from_bits(word(frame, 0x18))),
                            f32::from_bits(word(frame, 0x1C)),
                        );
                        status_eax(&mut call, status);
                        if status & 0x4100 == 0 {
                            call = virtual_call(
                                this,
                                0xD8,
                                &[lot as u32, config as u32, 0, 1, 1],
                                call.eax,
                                table(this),
                                false,
                            );
                            frame.add(0x14).write(call.eax as u8);
                        }
                    }
                }
            }
            call.eax = word(frame, 0x14);
            direct(
                0x006C_9AD0,
                Conv::Thiscall,
                this as u32,
                &[lot as u32, config as u32, kind, call.eax],
                call,
            );
            put(frame, 0x20, 0);
            virtual_call(
                this,
                0xA0,
                &[lot as u32, frame.add(0x24) as u32, frame.add(0x20) as u32],
                frame.add(0x20) as u32,
                table(this),
                false,
            );
            let bds = developer.building_development();
            call = virtual_call(
                bds,
                0x1C,
                &[kind, frame.add(0x24) as u32, word(frame, 0x20)],
                frame.add(0x24) as u32,
                table(bds),
                false,
            );
            let building = call.eax as *mut u8;
            success = !building.is_null();
            if success {
                call.eax = call.eax & !255 | u32::from(*frame.add(0x14));
                if *frame.add(0x14) == 0 {
                    let developer_table = table(this);
                    call = virtual_call(building, 0x0C, &[], call.eax, table(building), false);
                    let args = [call.eax, word(frame, 0x44)];
                    call = virtual_at(this, developer_table, 0x7C, &args, call.eax, call.edx, true);
                    frame.add(0x4C).cast::<f32>().write_unaligned(call.st0.to_f32());
                    let status = compare(call.st0, developer.foundation_slope());
                    status_eax(&mut call, status);
                    if status & 0x4100 == 0 {
                        let developer_table = table(this);
                        call = virtual_call(building, 0x0C, &[], frame.add(0x30) as u32, table(building), false);
                        virtual_at(
                            this,
                            developer_table,
                            0x58,
                            &[call.eax, frame.add(0x30) as u32],
                            call.eax,
                            call.edx,
                            false,
                        );
                        call = virtual_call(
                            this,
                            0x54,
                            &[
                                frame.add(0x30) as u32,
                                developer.foundation_slope().to_bits(),
                                1,
                                word(frame, 0x44),
                                developer.level_max_slope().to_bits(),
                                lot as u32,
                            ],
                            table(this),
                            developer.foundation_slope().to_bits(),
                            false,
                        );
                        let slope = if call.eax != 0 {
                            let developer_table = table(this);
                            put(frame, 0x4C, developer_table);
                            call = virtual_call(building, 0x0C, &[], table(this), table(building), false);
                            call = virtual_at(
                                this,
                                developer_table,
                                0x7C,
                                &[call.eax, word(frame, 0x44)],
                                developer_table,
                                call.edx,
                                true,
                            );
                            call.st0
                        } else {
                            F80::from_f32(f32::from_bits(word(frame, 0x4C)))
                        };
                        let status = compare(slope, developer.foundation_slope());
                        status_eax(&mut call, status);
                        if status & 0x4100 == 0 {
                            call = virtual_call(
                                this,
                                0xE8,
                                &[building as u32, config as u32],
                                table(this),
                                call.edx,
                                false,
                            );
                        }
                    }
                }
                call.eax = call.eax & !255 | u32::from(*frame.add(0x50));
                if *frame.add(0x50) != 0 {
                    call = direct(0x0090_DDF1, Conv::Cdecl, this as u32, &[], call);
                    let factory = call.eax as *mut u8;
                    call = virtual_call(
                        factory,
                        0x0C,
                        &[0xA97F_909E, 0x496E_7636, frame.add(0x4C) as u32],
                        call.eax,
                        table(factory),
                        false,
                    );
                    if call.eax as u8 == 0 {
                        call = direct(0x005C_A8A0, Conv::Cdecl, factory as u32, &[0x1A0], call);
                        if call.eax != 0 {
                            call = direct(0x005D_00F0, Conv::Thiscall, call.eax, &[1], call);
                        }
                        put(frame, 0x4C, call.eax);
                        let construction = call.eax as *mut u8;
                        call = virtual_call(construction, 4, &[], call.eax, table(construction), false);
                    }
                    let construction = word(frame, 0x4C) as *mut u8;
                    call = virtual_call(construction, 0x10, &[lot as u32], table(construction), call.edx, false);
                    let manager_table = table(developer.occupant_manager());
                    call = virtual_call(construction, 0x0C, &[0], call.eax, table(construction), false);
                    call = virtual_at(
                        developer.occupant_manager(),
                        manager_table,
                        0x50,
                        &[call.eax],
                        call.eax,
                        call.edx,
                        false,
                    );
                    call = virtual_call(construction, 8, &[], table(construction), call.edx, false);
                }
            }
        }
        if success {
            frame.add(0x13).write(1);
        }
        call.eax = call.eax & !255 | u32::from(*frame.add(0x50));
        if *frame.add(0x50) == 0 {
            call = virtual_call(lot, 0x88, &[3, 0, 0], call.eax, table(lot), false);
        }
        call = virtual_call(lot, 8, &[], table(lot), call.edx, false);
        call.eax = call.eax & !255 | u32::from(*frame.add(0x13));
        result(call)
    }
}

unsafe fn virtual_at(
    object: *mut u8,
    vtable: u32,
    slot: usize,
    args: &[u32],
    eax: u32,
    edx: u32,
    st0: bool,
) -> RawCall {
    unsafe {
        let address = (vtable as *const u8).add(slot).cast::<usize>().read_unaligned();
        let mut call = RawCall::new(Conv::Thiscall, address, object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call.take_st0 = u32::from(st0);
        call_raw(&mut call);
        call
    }
}

game_fn! { #[allow(dead_code)] fn set_lighting(this: *mut u8, mode: u32) -> u64 = "thiscall" 0x006C_BE50; }
game_fn! { #[allow(dead_code)] fn allocate_message(size: u32) -> u64 = "C" 0x0091_33DA; }
game_fn! { #[allow(dead_code)] fn initialize_message(this: *mut u8) -> u64 = "thiscall" 0x0091_34D6; }
game_fn! { #[allow(dead_code)] fn daily_simulation(this: *mut u8) -> u64 = "thiscall" 0x006C_D770; }
game_fn! { #[allow(dead_code)] fn building_removed(this: *mut u8, occupant: u32) -> u64 = "thiscall" 0x006C_8F80; }
game_fn! { #[allow(dead_code)] fn power_changed(this: *mut u8, rectangle: *const u32) -> u64 = "thiscall" 0x006C_BF40; }
game_fn! { #[allow(dead_code)] fn lighting_tick(this: *mut u8, milliseconds: u32) -> u64 = "thiscall" 0x006C_BD60; }
game_fn! { #[allow(dead_code)] fn building_inserted(this: *mut u8, occupant: u32) -> u64 = "thiscall" 0x006C_8830; }
game_fn! { #[allow(dead_code)] fn create_props(this: *mut u8, lot: *mut u8, config: *mut u8, occupied: u32) -> u64 = "thiscall" 0x006C_7860; }
game_fn! { #[allow(dead_code)] fn update_textures(this: *mut u8, lot: *mut u8, config: *mut u8, state: u32) -> u64 = "thiscall" 0x006C_D2B0; }
game_fn! { #[allow(dead_code)] fn create_flora(this: *mut u8, lot: *mut u8, config: *mut u8, occupied: u32) -> u64 = "thiscall" 0x006C_9C80; }
game_data! { static lighting_manager: *mut u8 = 0x00B4_3DBC; }
game_data! { static sim_clock: *mut u8 = 0x00B4_3D5C; }
game_data! { static demand_manager: *mut u8 = 0x00B4_3D74; }

port! {
    /// Receives the message interface at primary +4 and preserves the live message argument slot.
    #[port(address = 0x006C_D940, owner = "lotdeveloper", name = "cSC4LotDeveloper::DoMessage", naked)]
    pub unsafe extern "C" fn fc4p_006cd940() {
        core::arch::naked_asm!(
            "sub esp, 0x40", "push ebx", "push ebp", "push esi", "push edi",
            "push edx", "lea edx, [esp + 4]", "call {}", "add esp, 4",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 0x40", "ret 4",
            sym do_message,
        );
    }
}

pub(super) unsafe extern "fastcall" fn do_message(interface: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let this = interface.sub(4);
        let message = word(frame, 0x54) as *mut u8;
        let mut call = virtual_call(
            message,
            0x10,
            &[],
            table(message),
            frame.sub(4).cast::<u32>().read(),
            false,
        );
        let kind = call.eax;
        match kind {
            0x4477_8234 => {
                call = virtual_call(message, 0x38, &[], call.eax, table(message), false);
                let mode = if call.eax == 0 {
                    2
                } else {
                    call.eax = call.eax.wrapping_sub(1);
                    if call.eax != 0 {
                        call.eax = call.eax & !255 | 1;
                        return result(call);
                    }
                    1
                };
                let manager = lighting_manager().read();
                if mode == 1 {
                    call = virtual_call(manager, 0x18, &[], table(manager), call.edx, false);
                } else {
                    call = virtual_call(manager, 0x18, &[], call.eax, table(manager), false);
                }
                if call.eax != mode {
                    return super::lighting_transition::transition(interface, frame, [call.eax, call.edx], mode);
                }
                call = direct(0x006C_BE50, Conv::Thiscall, this as u32, &[mode], call);
                call = direct(0x0091_33DA, Conv::Cdecl, this as u32, &[0x2C], call);
                let mut notification = call.eax as *mut u8;
                if !notification.is_null() {
                    call = direct(0x0091_34D6, Conv::Thiscall, notification as u32, &[], call);
                    notification = call.eax as *mut u8;
                }
                if mode == 1 {
                    call = virtual_call(notification, 4, &[], call.eax, table(notification), false);
                    call = virtual_call(notification, 0x14, &[0xA6B7_9621], table(notification), call.edx, false);
                    let server = word(interface, 0x10) as *mut u8;
                    call = virtual_call(server, 0x10, &[notification as u32, 0], call.eax, table(server), false);
                    call = virtual_call(notification, 8, &[], table(notification), call.edx, false);
                } else {
                    call = virtual_call(notification, 4, &[], table(notification), call.edx, false);
                    call = virtual_call(notification, 0x14, &[0xA6B7_9621], call.eax, table(notification), false);
                    let server = word(interface, 0x10) as *mut u8;
                    call = virtual_call(server, 0x10, &[notification as u32, 0], table(server), call.edx, false);
                    call = virtual_call(notification, 8, &[], call.eax, table(notification), false);
                }
            }
            0x278E_6003 => {
                call = virtual_call(message, 0x24, &[], table(message), call.edx, false);
                call = virtual_call(message, 0x38, &[], call.eax, table(message), false);
                call = virtual_call(message, 0x60, &[], table(message), call.edx, false);
            }
            0x26D3_1EC4 => {
                let clock = sim_clock().read();
                call = virtual_call(clock, 0x20, &[], call.eax, table(clock), false);
                call.eax = call.eax.wrapping_add(31);
                put(interface, 0x5C, call.eax);
            }
            0x6695_6814 => {
                call.eax = 0;
                return result(direct(0x006C_D770, Conv::Thiscall, this as u32, &[], call));
            }
            0x6924_7DC7 => {
                let manager = demand_manager().read();
                call = virtual_call(manager, 0x18, &[0x2000, 0x20000], table(manager), call.edx, false);
                let demand = call.eax as *mut u8;
                call = virtual_call(demand, 0x24, &[], call.eax, table(demand), true);
                let status = compare(call.st0, zero().read());
                call.eax = u32::from(status & 0x4100 == 0);
                interface.add(0x5B).write(call.eax as u8);
            }
            0x99EF_1142 | 0x99EF_1143 => {
                if kind == 0x99EF_1142 {
                    call = virtual_call(message, 0x4C, &[], table(message), call.edx, false);
                } else {
                    call = virtual_call(message, 0x4C, &[], call.eax, table(message), false);
                }
                if call.eax == 0x2781_28A0 {
                    if kind == 0x99EF_1142 {
                        call = virtual_call(message, 0x28, &[], call.eax, table(message), false);
                        call = direct(0x006C_8F80, Conv::Thiscall, this as u32, &[call.eax], call);
                    } else {
                        call = virtual_call(message, 0x28, &[], table(message), call.edx, false);
                        call = direct(0x006C_8830, Conv::Thiscall, this as u32, &[call.eax], call);
                    }
                } else if call.eax == 0xC772_BF98 {
                    interface.add(0x59).write(1);
                    interface.add(0x5A).write(1);
                }
            }
            0x8A07_AE00 => {
                call = virtual_call(message, 0x60, &[], table(message), call.edx, false);
                let last = call.eax;
                call = virtual_call(message, 0x4C, &[], call.eax, table(message), false);
                let third = call.eax;
                call = virtual_call(message, 0x38, &[], table(message), call.edx, false);
                put(frame, 0x54, call.eax);
                call = virtual_call(message, 0x24, &[], call.eax, table(message), false);
                put(frame, 0x20, call.eax);
                put(frame, 0x24, word(frame, 0x54));
                put(frame, 0x28, third);
                put(frame, 0x2C, last);
                call.eax = word(frame, 0x54);
                call = direct(
                    0x006C_BF40,
                    Conv::Thiscall,
                    this as u32,
                    &[frame.add(0x20) as u32],
                    call,
                );
            }
            0x86AD_10EE => {
                call.eax = word(interface, 0x98);
                if call.eax == 0 {
                    return result(call);
                }
                call = virtual_call(message, 0x38, &[], call.eax, table(message), false);
                let product = u64::from(call.eax) * 0x1062_4DD3;
                call.eax = product as u32;
                call.edx = (product >> 38) as u32;
                call = direct(0x006C_BD60, Conv::Thiscall, this as u32, &[call.edx], call);
            }
            0xE99B_3D4D => {
                call = virtual_call(message, 0x80, &[], call.eax, table(message), false);
                let object = call.eax as *mut u8;
                call = virtual_call(
                    object,
                    0,
                    &[0xE818_FCED, frame.add(0x54) as u32],
                    call.eax,
                    table(object),
                    false,
                );
                if call.eax as u8 != 0 {
                    let lot_interface = word(frame, 0x54) as *mut u8;
                    virtual_call(
                        lot_interface,
                        0x54,
                        &[frame.add(0x18) as u32, frame.add(0x14) as u32],
                        frame.add(0x18) as u32,
                        table(lot_interface),
                        false,
                    );
                    let manager = word(interface, 0x20) as *mut u8;
                    call = virtual_call(
                        manager,
                        0x18,
                        &[word(frame, 0x18), word(frame, 0x14), 1],
                        word(frame, 0x18),
                        table(manager),
                        false,
                    );
                    let lot = call.eax as *mut u8;
                    if !lot.is_null() {
                        call = virtual_call(lot, 0x84, &[], call.eax, table(lot), false);
                        if matches!(call.eax, 3 | 4) {
                            let developer_table = table(this);
                            call = virtual_call(lot, 0x24, &[], table(lot), call.edx, false);
                            call = virtual_at(
                                this,
                                developer_table,
                                0xB0,
                                &[lot as u32, call.eax, 1],
                                call.eax,
                                call.edx,
                                false,
                            );
                        }
                    }
                    call = virtual_call(lot_interface, 8, &[], call.eax, table(lot_interface), false);
                }
            }
            0xE8BD_A2E7 => {
                call = virtual_call(message, 0x24, &[], table(message), call.edx, false);
                let old = call.eax;
                call = virtual_call(message, 0x38, &[], call.eax, table(message), false);
                let occupied = matches!(call.eax, 3 | 4);
                let reoccupied = old == 2 && call.eax == 3;
                frame.add(0x54).write(u8::from(occupied));
                call.eax = call.eax & !255 | u32::from(occupied);
                if occupied || reoccupied {
                    put(frame, 0x14, 0);
                    call = virtual_call(message, 0x80, &[], table(message), call.edx, false);
                    let object = call.eax as *mut u8;
                    call = virtual_call(
                        object,
                        0,
                        &[0xE818_FCED, frame.add(0x14) as u32],
                        call.eax,
                        table(object),
                        false,
                    );
                    if call.eax as u8 != 0 {
                        let lot_interface = word(frame, 0x14) as *mut u8;
                        virtual_call(
                            lot_interface,
                            0x54,
                            &[frame.add(0x1C) as u32, frame.add(0x18) as u32],
                            frame.add(0x1C) as u32,
                            table(lot_interface),
                            false,
                        );
                        virtual_call(
                            lot_interface,
                            0x58,
                            &[frame.add(0x10) as u32, frame.add(0x12) as u32],
                            frame.add(0x10) as u32,
                            table(lot_interface),
                            false,
                        );
                        let manager = word(interface, 0x20) as *mut u8;
                        call = virtual_call(
                            manager,
                            0x18,
                            &[word(frame, 0x1C), word(frame, 0x18), 1],
                            word(frame, 0x1C),
                            table(manager),
                            false,
                        );
                        let lot = call.eax as *mut u8;
                        if !lot.is_null() {
                            call = virtual_call(lot, 0x24, &[], call.eax, table(lot), false);
                            let config = call.eax as *mut u8;
                            if !config.is_null() {
                                call = virtual_call(
                                    lot,
                                    0x58,
                                    &[frame.add(0x11) as u32, frame.add(0x13) as u32],
                                    table(lot),
                                    frame.add(0x11) as u32,
                                    false,
                                );
                                call.eax = call.eax & !255 | u32::from(*frame.add(0x10));
                                if *frame.add(0x10) == *frame.add(0x11) && *frame.add(0x12) == *frame.add(0x13) {
                                    call.eax = call.eax & !255 | u32::from(*frame.add(0x54));
                                    if occupied {
                                        call = virtual_call(
                                            this,
                                            0xB0,
                                            &[lot as u32, config as u32, 1],
                                            call.eax,
                                            table(this),
                                            false,
                                        );
                                    }
                                    if reoccupied {
                                        call = direct(
                                            0x006C_7860,
                                            Conv::Thiscall,
                                            this as u32,
                                            &[lot as u32, config as u32, 1],
                                            call,
                                        );
                                        call = direct(
                                            0x006C_D2B0,
                                            Conv::Thiscall,
                                            this as u32,
                                            &[lot as u32, config as u32, 1],
                                            call,
                                        );
                                        call = direct(
                                            0x006C_9C80,
                                            Conv::Thiscall,
                                            this as u32,
                                            &[lot as u32, config as u32, 1],
                                            call,
                                        );
                                    }
                                }
                            }
                        }
                        call = virtual_call(lot_interface, 8, &[], table(lot_interface), call.edx, false);
                    }
                }
            }
            _ => {
                if kind > 0x6695_6810 && kind < 0x6924_7DC7 {
                    call.eax = kind.wrapping_sub(0x6695_6814);
                }
            }
        }
        call.eax = call.eax & !255 | 1;
        result(call)
    }
}

game_fn! { #[allow(dead_code)] fn clear_texture(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_AAF0; }
game_fn! { #[allow(dead_code)] fn allocate_cells(bytes: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free_cells(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_data! { static zone_manager: *mut u8 = 0x00B4_3D14; }
game_data! { static game_city: *mut u8 = 0x00B4_3CEC; }

port! {
    /// Builds the original local cell records while retaining their untouched padding bytes.
    #[port(address = 0x006C_C970, owner = "lotdeveloper", name = "cSC4LotDeveloper::SetEmptyLotBaseTexture", naked)]
    pub unsafe extern "C" fn fc4p_006cc970() {
        core::arch::naked_asm!(
            "sub esp, 0x70", "push ebx", "push ebp", "push edi", "push esi",
            "push edx", "lea edx, [esp + 4]", "call {}", "add esp, 4",
            "pop esi", "pop edi", "pop ebp", "pop ebx", "add esp, 0x70", "ret 4",
            sym empty_textures,
        );
    }
}

pub(super) unsafe extern "fastcall" fn empty_textures(this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let lot = word(frame, 0x84) as *mut u8;
        put(frame, 0x30, this as u32);
        let mut call = virtual_call(lot, 0x7C, &[], table(lot), frame.sub(4).cast::<u32>().read(), false);
        let zone = call.eax;
        if matches!(zone, 0 | 11 | 12 | 15) {
            return result(direct(0x006C_AAF0, Conv::Thiscall, this as u32, &[lot as u32], call));
        }
        let zones = zone_manager().read();
        call = virtual_call(zones, 0x3C, &[zone], call.eax, table(zones), false);
        let color = call.eax;
        frame.add(0x18).write((color >> 24) as u8);
        frame.add(0x19).write((color >> 16) as u8);
        frame.add(0x1A).write((color >> 8) as u8);
        call = virtual_call(zones, 0x40, &[], table(zones), color >> 16, false);
        frame.add(0x1B).write(call.eax as u8);
        let city = game_city().read();
        call = virtual_call(city, 0x178, &[], call.eax, table(city), false);
        put(frame, 0x24, call.eax.wrapping_sub(1));
        call = virtual_call(
            game_city().read(),
            0x174,
            &[],
            table(game_city().read()),
            call.edx,
            false,
        );
        put(frame, 0x78, call.eax.wrapping_sub(1));
        virtual_call(
            lot,
            0x38,
            &[frame.add(0x60) as u32],
            frame.add(0x60) as u32,
            table(lot),
            false,
        );
        frame.add(0x11).write(1);
        if (7..=9).contains(&(zone as i32)) {
            virtual_call(
                lot,
                0x58,
                &[frame.add(0x12) as u32, frame.add(0x13) as u32],
                frame.add(0x13) as u32,
                table(lot),
                false,
            );
            if *frame.add(0x12) == 1 && *frame.add(0x13) == 1 {
                frame.add(0x11).write(0);
            }
        }
        put(frame, 0x34, 0);
        put(frame, 0x38, 0);
        put(frame, 0x3C, 0);
        let mut x = word(frame, 0x60) as i32;
        put(frame, 0x14, x as u32);
        let mut end = 0u32;
        let mut begin = 0u32;
        if x <= word(frame, 0x68) as i32 {
            loop {
                let mut z = word(frame, 0x64) as i32;
                put(frame, 0x20, z as u32);
                if z <= word(frame, 0x6C) as i32 {
                    loop {
                        if (x as u32) < word(this, 0x50) && (z as u32) < word(this, 0x4C) {
                            for offset in [0x50, 0x54, 0x58, 0x5C] {
                                put(frame, offset, 1);
                            }
                            if *frame.add(0x11) != 0 {
                                // North and south bounds use city depth; west and east use city width.
                                let north = z.wrapping_sub(1);
                                call.eax = 0;
                                if north >= 0 && north <= word(frame, 0x24) as i32 {
                                    let manager = LotDeveloper::from_ptr(this).lot_manager();
                                    call = virtual_call(
                                        manager,
                                        0x18,
                                        &[x as u32, north as u32, 0],
                                        table(manager),
                                        x as u32,
                                        false,
                                    );
                                }
                                if call.eax == lot as u32 {
                                    put(frame, 0x50, 0);
                                } else {
                                    call = virtual_call(lot, 0x5C, &[], call.eax, table(lot), false);
                                    if call.eax == 0 {
                                        put(frame, 0x50, 2);
                                    }
                                }
                                let south = north.wrapping_add(2);
                                call.eax = 0;
                                if south >= 0 && south <= word(frame, 0x24) as i32 {
                                    let manager = LotDeveloper::from_ptr(this).lot_manager();
                                    call = virtual_call(
                                        manager,
                                        0x18,
                                        &[x as u32, south as u32, 0],
                                        table(manager),
                                        x as u32,
                                        false,
                                    );
                                }
                                if call.eax == lot as u32 {
                                    put(frame, 0x58, 0);
                                } else {
                                    call = virtual_call(lot, 0x5C, &[], call.eax, table(lot), false);
                                    if call.eax == 2 {
                                        put(frame, 0x58, 2);
                                    }
                                }
                                x = x.wrapping_sub(1);
                                put(frame, 0x14, x as u32);
                                put(frame, 0x20, z as u32);
                                call.eax = 0;
                                if x >= 0 && x <= word(frame, 0x78) as i32 {
                                    let manager = LotDeveloper::from_ptr(this).lot_manager();
                                    call = virtual_call(
                                        manager,
                                        0x18,
                                        &[x as u32, z as u32, 0],
                                        x as u32,
                                        table(manager),
                                        false,
                                    );
                                }
                                if call.eax == lot as u32 {
                                    put(frame, 0x5C, 0);
                                } else {
                                    call = virtual_call(lot, 0x5C, &[], table(lot), call.edx, false);
                                    if call.eax == 3 {
                                        put(frame, 0x5C, 2);
                                    }
                                }
                                x = x.wrapping_add(2);
                                put(frame, 0x14, x as u32);
                                call.eax = 0;
                                if x >= 0 && x <= word(frame, 0x78) as i32 {
                                    let manager = LotDeveloper::from_ptr(this).lot_manager();
                                    call = virtual_call(
                                        manager,
                                        0x18,
                                        &[x as u32, z as u32, 0],
                                        x as u32,
                                        table(manager),
                                        false,
                                    );
                                }
                                if call.eax == lot as u32 {
                                    put(frame, 0x54, 0);
                                } else {
                                    call = virtual_call(lot, 0x5C, &[], table(lot), call.edx, false);
                                    if call.eax == 1 {
                                        put(frame, 0x54, 2);
                                    }
                                }
                                x = x.wrapping_sub(1);
                                put(frame, 0x14, x as u32);
                            } else {
                                for offset in [0x50, 0x54, 0x58, 0x5C] {
                                    put(frame, offset, 0);
                                }
                            }
                            put(frame, 0x40, 0);
                            frame.add(0x44).write(x as u8);
                            frame.add(0x45).write(z as u8);
                            frame.add(0x46).write(0);
                            frame.add(0x47).write(0);
                            for offset in 0x48..=0x4D {
                                frame.add(offset).write(255);
                            }
                            call = virtual_call(
                                this,
                                0xA8,
                                &[
                                    frame.add(0x50) as u32,
                                    frame.add(0x40) as u32,
                                    frame.add(0x46) as u32,
                                    frame.add(0x13) as u32,
                                ],
                                frame.add(0x40) as u32,
                                table(this),
                                false,
                            );
                            put(frame, 0x48, word(frame, 0x18));
                            call.edx = call.edx & !255 | u32::from(*frame.add(0x13) != 0);
                            frame.add(0x47).write(call.edx as u8);
                            frame.add(0x4C).write(3);
                            if end != word(frame, 0x3C) {
                                if end != 0 {
                                    for i in 0..4 {
                                        (end as *mut u32).add(i).write_unaligned(word(frame, 0x40 + i * 4));
                                    }
                                }
                                end = end.wrapping_add(16);
                                put(frame, 0x38, end);
                            } else {
                                let length = end.wrapping_sub(begin) as i32 >> 4;
                                put(frame, 0x28, 1);
                                put(frame, 0x2C, length as u32);
                                let capacity =
                                    (length as u32).wrapping_add(if (length as u32) < 1 { 1 } else { length as u32 });
                                call.eax = length as u32;
                                let storage = if capacity != 0 {
                                    call = direct(
                                        0x0090_CF54,
                                        Conv::Cdecl,
                                        capacity.wrapping_shl(4),
                                        &[capacity.wrapping_shl(4)],
                                        call,
                                    );
                                    call.eax
                                } else {
                                    0
                                };
                                let mut destination = storage;
                                let mut source = begin;
                                while source != end {
                                    if destination != 0 {
                                        for i in 0..4 {
                                            let value = (source as *const u32).add(i).read_unaligned();
                                            (destination as *mut u32).add(i).write_unaligned(value);
                                            if i == 3 {
                                                call.edx = value;
                                            }
                                        }
                                    }
                                    source = source.wrapping_add(16);
                                    destination = destination.wrapping_add(16);
                                }
                                if destination != 0 {
                                    for i in 0..4 {
                                        call.edx = word(frame, 0x40 + i * 4);
                                        (destination as *mut u32).add(i).write_unaligned(call.edx);
                                    }
                                }
                                end = destination.wrapping_add(16);
                                call.eax = begin;
                                if begin != 0 {
                                    call = direct(0x0090_CF63, Conv::Cdecl, destination, &[begin], call);
                                }
                                begin = storage;
                                put(frame, 0x38, end);
                                put(frame, 0x3C, storage.wrapping_add(capacity.wrapping_shl(4)));
                                put(frame, 0x34, begin);
                            }
                        }
                        z = z.wrapping_add(1);
                        put(frame, 0x20, z as u32);
                        if z > word(frame, 0x6C) as i32 {
                            break;
                        }
                    }
                }
                x = x.wrapping_add(1);
                put(frame, 0x14, x as u32);
                if x > word(frame, 0x68) as i32 {
                    break;
                }
            }
        }
        put(frame, 0x1C, 0);
        call = virtual_call(
            this,
            0xA4,
            &[word(frame, 0x84), frame.add(0x1C) as u32, 1],
            table(this),
            x as u32,
            false,
        );
        if call.eax as u8 != 0 {
            let texture = word(frame, 0x1C) as *mut u8;
            call = virtual_call(
                texture,
                0x20,
                &[begin, (end.wrapping_sub(begin) as i32 >> 4) as u32, 0],
                call.eax,
                table(texture),
                false,
            );
            call = virtual_call(texture, 0x0C, &[], table(texture), call.edx, false);
            let occupant = call.eax as *mut u8;
            call = virtual_call(occupant, 0x80, &[1], call.eax, table(occupant), false);
            let exists = call.eax as u8 != 0;
            let manager = LotDeveloper::from_ptr(this).occupant_manager();
            call.eax = table(manager);
            if exists {
                call = virtual_call(manager, 0x60, &[occupant as u32, 0], call.eax, call.edx, false);
                call = virtual_call(occupant, 0x3C, &[0x99EF_1145, 0], call.eax, table(occupant), false);
            } else {
                call = virtual_call(manager, 0x50, &[occupant as u32, 0], call.eax, call.edx, false);
            }
            call = virtual_call(texture, 8, &[], call.eax, table(texture), false);
        }
        if begin != 0 {
            call = direct(0x0090_CF63, Conv::Cdecl, 0, &[begin], call);
        }
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006cc490::PORT, fc4p_006cc970::PORT, fc4p_006cd940::PORT];

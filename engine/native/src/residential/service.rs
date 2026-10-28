// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Department serialization, history messages, and coverage geometry.

use super::ResidentialSim;
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// A health or education department's persisted parameters.
    pub struct Department {
        0x00 => first / set_first: u32,
        0x04 => second / set_second: u32,
        0x08 => third / set_third: u32,
        0x0C => count / set_count: u32,
        0x10 => fourth / set_fourth: u32,
        0x44 => strike / set_strike: u8,
        0x48 => strike_day / set_strike_day: u32,
        0x4C => first_curve / set_first_curve: *mut u8,
        0x50 => second_curve / set_second_curve: *mut u8,
        0x54 => fifth / set_fifth: u32,
        0x58 => sixth / set_sixth: u32,
        0x5C => final_word / set_final_word: u32,
    }
}

game_fn! { fn write_reference(stream:*mut u8, object:*mut u8)->u32 = "C" 0x005D_99A0; }
game_fn! { fn allocate(size:u32)->*mut u8 = "C" 0x0091_33DA; }
game_fn! { fn message_new(this:*mut u8)->*mut u8 = "thiscall" 0x0091_34D6; }
game_fn! {
    fn tracts_in_radius(x:f32,z:f32,radius:f32,a:u32,b:u32,c:u32,d:u32)->u32 = "C" 0x005E_5200;
}
game_data! { static lot_manager:*mut u8 = 0x00B4_3D08; }

/// Keep the second operand in memory so a signaling NaN retains its original arithmetic role.
fn multiply_stored(value: F80, operand: f32) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fmul dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value=in(reg) &value,operand=in(reg) &operand,result=in(reg) &mut result,out("st(0)") _);
    }
    result
}

port! {
    /// Write department scalars and curve references in their saved order; ignore individual failures.
    /// Float fields are transferred as their raw bits. Only AL of the final result is replaced by one.
    #[port(address=0x006F_7B00,owner="residential",name="WriteResidentialDepartment")]
    pub unsafe extern "thiscall" fn fc4p_006f7b00(_this:*mut u8,stream:*mut u8,block:*mut u8)->u32 {
        unsafe {
            let d=Department::from_ptr(block);
            vcall!(stream,0x30 => fn(u32)->u32,d.first());
            vcall!(stream,0x30 => fn(u32)->u32,d.second());
            vcall!(stream,0x30 => fn(u32)->u32,d.third());
            vcall!(stream,0x20 => fn(u32)->u32,d.count());
            vcall!(stream,0x30 => fn(u32)->u32,d.fourth());
            vcall!(stream,0x14 => fn(u32)->u32,u32::from(d.strike()));
            vcall!(stream,0x20 => fn(u32)->u32,d.strike_day());
            write_reference(stream,d.first_curve());
            write_reference(stream,d.second_curve());
            vcall!(stream,0x30 => fn(u32)->u32,d.fifth());
            vcall!(stream,0x30 => fn(u32)->u32,d.sixth());
            (vcall!(stream,0x24 => fn(u32)->u32,d.final_word()) & !0xFF)|1
        }
    }
}

port! {
    /// Allocate and retain a history message, fill its ID and payload, post it, then release it.
    /// An absent server skips allocation. The server is reread immediately before posting.
    #[port(address=0x006F_7D00,owner="residential",name="cSC4ResidentialSimulator::RecordHistoryData")]
    pub unsafe extern "thiscall" fn fc4p_006f7d00(this:*mut u8,id:u32,value:u32)->u32 {
        unsafe {
            let sim=ResidentialSim::from_ptr(this);
            if sim.message_server().is_null(){return 0;}
            let storage=allocate(0x2C);
            let message=if storage.is_null(){core::ptr::null_mut()}else{message_new(storage)};
            vcall!(message,4 => fn()->u32);
            vcall!(message,0x14 => fn(u32)->u32,0x89EF_A536);
            vcall!(message,0x2C => fn(u32)->u32,id);
            vcall!(message,0x40 => fn(u32)->u32,value);
            vcall!(sim.message_server(),0x0C => fn(*mut u8)->u32,message);
            vcall!(message,8 => fn()->u32)
        }
    }
}

port! {
    /// Prefer the lot centre; use the occupant position when no lot exists. Scale the supplied radius
    /// by the simulator's coverage slop, then forward the centre and the remaining four words.
    #[port(address=0x006F_7E00,owner="residential",name="cSC4ResidentialSimulator::GetCoveredTractList")]
    pub unsafe extern "thiscall" fn fc4p_006f7e00(this:*mut u8,building:*mut u8,radius:f32,a:u32,b:u32,c:u32,d:u32)->u32 {
        unsafe {
            let sim=ResidentialSim::from_ptr(this);
            let lot=vcall!(*lot_manager(),0x38 => fn(*mut u8)->*mut u8,building);
            let mut position=MaybeUninit::<[f32;3]>::uninit();
            if lot.is_null(){vcall!(building,0x20 => fn(*mut f32)->u32,position.as_mut_ptr().cast());}
            else{vcall!(lot,0x3C => fn(*mut f32)->u32,position.as_mut_ptr().cast());}
            let position=position.assume_init();
            let radius=multiply_stored(F80::from_f32(radius),sim.coverage_slop()).to_f32();
            tracts_in_radius(position[0],position[2],radius,a,b,c,d)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006f7b00::PORT, fc4p_006f7d00::PORT, fc4p_006f7e00::PORT];

#[cfg(test)]
mod tests;

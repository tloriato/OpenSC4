// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Shared MMX code generation. Packed option words select the emitted operations and branch patches.

use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

#[repr(C)]
struct Operand([u8; 12]);

impl Operand {
    fn new(mask: u32, length: u8, mode: u8, tail: u64) -> Self {
        Self([
            mask as u8,
            (mask >> 8) as u8,
            (mask >> 16) as u8,
            (mask >> 24) as u8,
            length,
            mode,
            tail as u8,
            (tail >> 8) as u8,
            (tail >> 16) as u8,
            (tail >> 24) as u8,
            (tail >> 32) as u8,
            (tail >> 40) as u8,
        ])
    }

    fn memory(address: *mut u8) -> Self {
        Self::new(0, 4, 5, address as u64)
    }
}

layout! {
    /// The code buffer and its instruction queue share one stack object.
    struct Emitter {
        0x00 => start / set_start: u32,
        0x04 => cursor / set_cursor: u32,
    }
}

game_fn! { fn emit_mov(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_CE20; }
game_fn! { fn emit_mov_imm(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_CEC0; }
game_fn! { fn emit_load(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_CF80; }
game_fn! { fn emit_load_byte(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_D010; }
game_fn! { fn emit_load_word(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_D0B0; }
game_fn! { fn emit_store_word(this: *mut u8, memory: *const Operand, reg: u32) = "thiscall" 0x008E_D150; }
game_fn! { fn emit_store(this: *mut u8, memory: *const Operand, reg: u32) = "thiscall" 0x008E_D1F0; }
game_fn! { fn emit_and(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_D280; }
game_fn! { fn emit_and_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_D330; }
game_fn! { fn emit_and_imm(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_D3C0; }
game_fn! { fn emit_xor(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_D480; }
game_fn! { fn emit_add(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_D530; }
game_fn! { fn emit_add_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_D5E0; }
game_fn! { fn emit_add_imm(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_D670; }
game_fn! { fn emit_adc_zero(this: *mut u8) = "thiscall" 0x008E_D730; }
game_fn! { fn emit_cmp_word(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_D7B0; }
game_fn! { fn emit_cmp_word_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_D860; }
game_fn! { fn emit_cmp_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_D900; }
game_fn! { fn emit_sub(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_D990; }
game_fn! { fn emit_sub_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_DA40; }
game_fn! { fn emit_shr(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_DAD0; }
game_fn! { fn emit_sar(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_DB70; }
game_fn! { fn emit_shl(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_DC10; }
game_fn! { fn emit_inc(this: *mut u8, a: u32) = "thiscall" 0x008E_DCB0; }
game_fn! { fn emit_dec(this: *mut u8, a: u32) = "thiscall" 0x008E_DD30; }
game_fn! { fn emit_lea(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_DDB0; }
game_fn! { fn emit_imul_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_DE40; }
game_fn! { fn emit_jump(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E0A0; }
game_fn! { fn emit_jump_short(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E160; }
game_fn! { fn emit_ret(this: *mut u8) = "thiscall" 0x008E_E200; }
game_fn! { fn emit_movd_to_gpr(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E270; }
game_fn! { fn emit_movd_from_gpr(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E320; }
game_fn! { fn emit_movd_load(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_E3D0; }
game_fn! { fn emit_movd_store(this: *mut u8, memory: *const Operand, reg: u32) = "thiscall" 0x008E_E470; }
game_fn! { fn emit_movq(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E510; }
game_fn! { fn emit_movq_load(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_E5C0; }
game_fn! { fn emit_movq_store(this: *mut u8, memory: *const Operand, reg: u32) = "thiscall" 0x008E_E660; }
game_fn! { fn emit_pand(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E700; }
game_fn! { fn emit_pand_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_E7B0; }
game_fn! { fn emit_pandn(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E850; }
game_fn! { fn emit_pxor(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E900; }
game_fn! { fn emit_paddw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_E9D0; }
game_fn! { fn emit_paddsw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_EA80; }
game_fn! { fn emit_paddsw_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_EB30; }
game_fn! { fn emit_paddd(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_EBD0; }
game_fn! { fn emit_paddd_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_EC80; }
game_fn! { fn emit_psubw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_ED20; }
game_fn! { fn emit_psubw_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_EDD0; }
game_fn! { fn emit_psubd_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_EE70; }
game_fn! { fn emit_psubd(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_EF10; }
game_fn! { fn emit_psrlw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_EFC0; }
game_fn! { fn emit_psrld(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F070; }
game_fn! { fn emit_psllw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F120; }
game_fn! { fn emit_pslld(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F1D0; }
game_fn! { fn emit_psllq(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F280; }
game_fn! { fn emit_psraw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F330; }
game_fn! { fn emit_psrad(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F3E0; }
game_fn! { fn emit_pmullw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F490; }
game_fn! { fn emit_pmulhw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F540; }
game_fn! { fn emit_pmulhw_mem(this: *mut u8, reg: u32, memory: *const Operand) = "thiscall" 0x008E_F5F0; }
game_fn! { fn emit_pmaddwd(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F690; }
game_fn! { fn emit_packssdw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F740; }
game_fn! { fn emit_packuswb(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F7F0; }
game_fn! { fn emit_punpcklbw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F8A0; }
game_fn! { fn emit_punpcklwd(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_F950; }
game_fn! { fn emit_punpckldq(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_FA00; }
game_fn! { fn emit_punpckhbw(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_FAB0; }
game_fn! { fn emit_punpckhwd(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_FB60; }
game_fn! { fn emit_pcmpeqb(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_FC10; }
game_fn! { fn emit_pcmpeqd(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_FCC0; }
game_fn! { fn emit_pshufw(this: *mut u8, a: u32, b: u32, selector: u32) = "thiscall" 0x008E_FD70; }
game_fn! { fn emit_maskmovq_mem(this: *mut u8, memory: *const Operand, reg: u32) = "thiscall" 0x008E_FE30; }
game_fn! { fn emit_maskmovq(this: *mut u8, a: u32, b: u32) = "thiscall" 0x008E_FEC0; }
game_fn! { fn emit_sfence(this: *mut u8) = "thiscall" 0x008E_FF70; }
game_fn! { fn emit_flush(this: *mut u8) = "thiscall" 0x008E_CCA0; }
game_fn! { fn emit_align(this: *mut u8) = "thiscall" 0x008E_CD50; }

impl Emitter {
    fn mov(self, a: u32, b: u32) {
        unsafe { emit_mov(self.as_ptr(), a, b) }
    }
    fn mov_imm(self, a: u32, b: u32) {
        unsafe { emit_mov_imm(self.as_ptr(), a, b) }
    }
    fn load(self, reg: u32, memory: *const Operand) {
        unsafe { emit_load(self.as_ptr(), reg, memory) }
    }
    fn load_byte(self, reg: u32, memory: *const Operand) {
        unsafe { emit_load_byte(self.as_ptr(), reg, memory) }
    }
    fn load_word(self, reg: u32, memory: *const Operand) {
        unsafe { emit_load_word(self.as_ptr(), reg, memory) }
    }
    fn store_word(self, memory: *const Operand, reg: u32) {
        unsafe { emit_store_word(self.as_ptr(), memory, reg) }
    }
    fn store(self, memory: *const Operand, reg: u32) {
        unsafe { emit_store(self.as_ptr(), memory, reg) }
    }
    fn and(self, a: u32, b: u32) {
        unsafe { emit_and(self.as_ptr(), a, b) }
    }
    fn and_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_and_mem(self.as_ptr(), reg, memory) }
    }
    fn and_imm(self, a: u32, b: u32) {
        unsafe { emit_and_imm(self.as_ptr(), a, b) }
    }
    fn xor(self, a: u32, b: u32) {
        unsafe { emit_xor(self.as_ptr(), a, b) }
    }
    fn add(self, a: u32, b: u32) {
        unsafe { emit_add(self.as_ptr(), a, b) }
    }
    fn add_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_add_mem(self.as_ptr(), reg, memory) }
    }
    fn add_imm(self, a: u32, b: u32) {
        unsafe { emit_add_imm(self.as_ptr(), a, b) }
    }
    fn adc_zero(self) {
        unsafe { emit_adc_zero(self.as_ptr()) }
    }
    fn cmp_word(self, a: u32, b: u32) {
        unsafe { emit_cmp_word(self.as_ptr(), a, b) }
    }
    fn cmp_word_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_cmp_word_mem(self.as_ptr(), reg, memory) }
    }
    fn cmp_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_cmp_mem(self.as_ptr(), reg, memory) }
    }
    fn sub(self, a: u32, b: u32) {
        unsafe { emit_sub(self.as_ptr(), a, b) }
    }
    fn sub_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_sub_mem(self.as_ptr(), reg, memory) }
    }
    fn shr(self, a: u32, b: u32) {
        unsafe { emit_shr(self.as_ptr(), a, b) }
    }
    fn sar(self, a: u32, b: u32) {
        unsafe { emit_sar(self.as_ptr(), a, b) }
    }
    fn shl(self, a: u32, b: u32) {
        unsafe { emit_shl(self.as_ptr(), a, b) }
    }
    fn inc(self, a: u32) {
        unsafe { emit_inc(self.as_ptr(), a) }
    }
    fn dec(self, a: u32) {
        unsafe { emit_dec(self.as_ptr(), a) }
    }
    fn lea(self, reg: u32, memory: *const Operand) {
        unsafe { emit_lea(self.as_ptr(), reg, memory) }
    }
    fn imul_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_imul_mem(self.as_ptr(), reg, memory) }
    }
    fn jump(self, a: u32, b: u32) {
        unsafe { emit_jump(self.as_ptr(), a, b) }
    }
    fn jump_short(self, a: u32, b: u32) {
        unsafe { emit_jump_short(self.as_ptr(), a, b) }
    }
    fn ret(self) {
        unsafe { emit_ret(self.as_ptr()) }
    }
    fn movd_to_gpr(self, a: u32, b: u32) {
        unsafe { emit_movd_to_gpr(self.as_ptr(), a, b) }
    }
    fn movd_from_gpr(self, a: u32, b: u32) {
        unsafe { emit_movd_from_gpr(self.as_ptr(), a, b) }
    }
    fn movd_load(self, reg: u32, memory: *const Operand) {
        unsafe { emit_movd_load(self.as_ptr(), reg, memory) }
    }
    fn movd_store(self, memory: *const Operand, reg: u32) {
        unsafe { emit_movd_store(self.as_ptr(), memory, reg) }
    }
    fn movq(self, a: u32, b: u32) {
        unsafe { emit_movq(self.as_ptr(), a, b) }
    }
    fn movq_load(self, reg: u32, memory: *const Operand) {
        unsafe { emit_movq_load(self.as_ptr(), reg, memory) }
    }
    fn movq_store(self, memory: *const Operand, reg: u32) {
        unsafe { emit_movq_store(self.as_ptr(), memory, reg) }
    }
    fn pand(self, a: u32, b: u32) {
        unsafe { emit_pand(self.as_ptr(), a, b) }
    }
    fn pand_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_pand_mem(self.as_ptr(), reg, memory) }
    }
    fn pandn(self, a: u32, b: u32) {
        unsafe { emit_pandn(self.as_ptr(), a, b) }
    }
    fn pxor(self, a: u32, b: u32) {
        unsafe { emit_pxor(self.as_ptr(), a, b) }
    }
    fn paddw(self, a: u32, b: u32) {
        unsafe { emit_paddw(self.as_ptr(), a, b) }
    }
    fn paddsw(self, a: u32, b: u32) {
        unsafe { emit_paddsw(self.as_ptr(), a, b) }
    }
    fn paddsw_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_paddsw_mem(self.as_ptr(), reg, memory) }
    }
    fn paddd(self, a: u32, b: u32) {
        unsafe { emit_paddd(self.as_ptr(), a, b) }
    }
    fn paddd_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_paddd_mem(self.as_ptr(), reg, memory) }
    }
    fn psubw(self, a: u32, b: u32) {
        unsafe { emit_psubw(self.as_ptr(), a, b) }
    }
    fn psubw_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_psubw_mem(self.as_ptr(), reg, memory) }
    }
    fn psubd_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_psubd_mem(self.as_ptr(), reg, memory) }
    }
    fn psubd(self, a: u32, b: u32) {
        unsafe { emit_psubd(self.as_ptr(), a, b) }
    }
    fn psrlw(self, a: u32, b: u32) {
        unsafe { emit_psrlw(self.as_ptr(), a, b) }
    }
    fn psrld(self, a: u32, b: u32) {
        unsafe { emit_psrld(self.as_ptr(), a, b) }
    }
    fn psllw(self, a: u32, b: u32) {
        unsafe { emit_psllw(self.as_ptr(), a, b) }
    }
    fn pslld(self, a: u32, b: u32) {
        unsafe { emit_pslld(self.as_ptr(), a, b) }
    }
    fn psllq(self, a: u32, b: u32) {
        unsafe { emit_psllq(self.as_ptr(), a, b) }
    }
    fn psraw(self, a: u32, b: u32) {
        unsafe { emit_psraw(self.as_ptr(), a, b) }
    }
    fn psrad(self, a: u32, b: u32) {
        unsafe { emit_psrad(self.as_ptr(), a, b) }
    }
    fn pmullw(self, a: u32, b: u32) {
        unsafe { emit_pmullw(self.as_ptr(), a, b) }
    }
    fn pmulhw(self, a: u32, b: u32) {
        unsafe { emit_pmulhw(self.as_ptr(), a, b) }
    }
    fn pmulhw_mem(self, reg: u32, memory: *const Operand) {
        unsafe { emit_pmulhw_mem(self.as_ptr(), reg, memory) }
    }
    fn pmaddwd(self, a: u32, b: u32) {
        unsafe { emit_pmaddwd(self.as_ptr(), a, b) }
    }
    fn packssdw(self, a: u32, b: u32) {
        unsafe { emit_packssdw(self.as_ptr(), a, b) }
    }
    fn packuswb(self, a: u32, b: u32) {
        unsafe { emit_packuswb(self.as_ptr(), a, b) }
    }
    fn punpcklbw(self, a: u32, b: u32) {
        unsafe { emit_punpcklbw(self.as_ptr(), a, b) }
    }
    fn punpcklwd(self, a: u32, b: u32) {
        unsafe { emit_punpcklwd(self.as_ptr(), a, b) }
    }
    fn punpckldq(self, a: u32, b: u32) {
        unsafe { emit_punpckldq(self.as_ptr(), a, b) }
    }
    fn punpckhbw(self, a: u32, b: u32) {
        unsafe { emit_punpckhbw(self.as_ptr(), a, b) }
    }
    fn punpckhwd(self, a: u32, b: u32) {
        unsafe { emit_punpckhwd(self.as_ptr(), a, b) }
    }
    fn pcmpeqb(self, a: u32, b: u32) {
        unsafe { emit_pcmpeqb(self.as_ptr(), a, b) }
    }
    fn pcmpeqd(self, a: u32, b: u32) {
        unsafe { emit_pcmpeqd(self.as_ptr(), a, b) }
    }
    fn pshufw(self, a: u32, b: u32, selector: u32) {
        unsafe { emit_pshufw(self.as_ptr(), a, b, selector) }
    }
    fn maskmovq_mem(self, memory: *const Operand, reg: u32) {
        unsafe { emit_maskmovq_mem(self.as_ptr(), memory, reg) }
    }
    fn maskmovq(self, a: u32, b: u32) {
        unsafe { emit_maskmovq(self.as_ptr(), a, b) }
    }
    fn sfence(self) {
        unsafe { emit_sfence(self.as_ptr()) }
    }
    fn flush(self) {
        unsafe { emit_flush(self.as_ptr()) }
    }
    fn align(self) {
        unsafe { emit_align(self.as_ptr()) }
    }
}

game_fn! { fn queue_new(this: *mut u8, count: u32) = "thiscall" 0x008E_C270; }
game_fn! { fn queue_delete(this: *mut u8) = "thiscall" 0x008E_C950; }
game_fn! { fn jump_patch(this: *mut u8, output: *mut u32) -> *const u32 = "thiscall" 0x008E_DEE0; }
game_fn! { fn condition_patch(this: *mut u8, output: *mut u32, condition: u32) -> *const u32 = "thiscall" 0x008E_DF70; }
game_fn! { fn short_patch(this: *mut u8, output: *mut u32, condition: u32) -> *const u32 = "thiscall" 0x008E_E020; }
game_fn! { fn indexed_operand(this: *mut Operand, base: u32, scale: u32, table: *mut u8) -> *const Operand = "thiscall" 0x008E_C150; }

game_data! { static condition_table: [u32; 8] = 0x00AC_C920; }
game_data! { static loop_condition_table: [u32; 8] = 0x00AC_C940; }
game_data! { static operand_00b5c9e8: u32 = 0x00B5_C9E8; }
game_data! { static operand_00b5c5ac: u32 = 0x00B5_C5AC; }
game_data! { static operand_00b5ca08: u32 = 0x00B5_CA08; }
game_data! { static operand_00b5ca10: u32 = 0x00B5_CA10; }
game_data! { static operand_00b5c9ec: u32 = 0x00B5_C9EC; }
game_data! { static operand_00b5c9e0: u32 = 0x00B5_C9E0; }
game_data! { static operand_00b5c9e4: u32 = 0x00B5_C9E4; }
game_data! { static operand_00b5ca28: u32 = 0x00B5_CA28; }
game_data! { static operand_00b5ca30: u32 = 0x00B5_CA30; }
game_data! { static operand_00b5ca00: u32 = 0x00B5_CA00; }
game_data! { static operand_00b5c580: u32 = 0x00B5_C580; }
game_data! { static operand_00b5c588: u32 = 0x00B5_C588; }
game_data! { static operand_00b5c5c8: u32 = 0x00B5_C5C8; }
game_data! { static operand_00b5c7c8: u32 = 0x00B5_C7C8; }
game_data! { static operand_00b160e0: u32 = 0x00B1_60E0; }
game_data! { static operand_00b5c9c8: u32 = 0x00B5_C9C8; }
game_data! { static operand_00b5c9d8: u32 = 0x00B5_C9D8; }
game_data! { static operand_00b5c9d0: u32 = 0x00B5_C9D0; }
game_data! { static operand_00b5c9de: u32 = 0x00B5_C9DE; }
game_data! { static operand_00b160d8: u32 = 0x00B1_60D8; }
game_data! { static operand_00b160d0: u32 = 0x00B1_60D0; }
game_data! { static operand_00b5c5bc: u32 = 0x00B5_C5BC; }
game_data! { static operand_00b5ca3c: u32 = 0x00B5_CA3C; }
game_data! { static operand_00b5ca40: u32 = 0x00B5_CA40; }
game_data! { static operand_00b5c5a0: u32 = 0x00B5_C5A0; }
game_data! { static operand_00b5c5a4: u32 = 0x00B5_C5A4; }

impl Emitter {
    fn patch(self, condition: Option<u32>) -> u32 {
        let mut output = 0;
        unsafe {
            read_unaligned(match condition {
                Some(c) => condition_patch(self.as_ptr(), &mut output, c),
                None => jump_patch(self.as_ptr(), &mut output),
            })
        }
    }

    fn patch_short(self, condition: u32) -> u32 {
        let mut output = 0;
        unsafe { read_unaligned(short_patch(self.as_ptr(), &mut output, condition)) }
    }

    fn restore_constants(self, enabled: bool) {
        if enabled {
            self.paddd_mem(5, &Operand::memory(operand_00b5ca08().cast()));
            self.paddd_mem(7, &Operand::memory(operand_00b5ca10().cast()));
        } else {
            self.paddd(5, 0);
            self.paddd(7, 1);
        }
    }

    fn expand(self, options: u32, shuffle: bool) {
        if options & 0x100000 == 0 {
            if shuffle {
                self.pshufw(2, 1, 0xd);
                self.psllw(2, 8);
            } else {
                self.punpckldq(2, 1);
                self.punpckhbw(2, 1);
            }
        } else {
            self.movq(2, 1);
            self.paddsw_mem(2, &Operand::memory(operand_00b5c588().cast()));
            self.psubd_mem(2, &Operand::memory(operand_00b5c588().cast()));
        }
    }

    fn scale(self, options: u32, shuffle: bool) {
        if options & 0x100000 != 0 {
            self.pslld(2, 0x14);
            self.packssdw(2, 2);
            self.packuswb(2, 2);
        } else if shuffle {
            self.packuswb(2, 2);
        } else {
            self.psllq(2, 0x30);
        }
    }

    fn indexed(self, reg: u32, table: *mut u8) {
        let mut scratch = Operand([0; 12]);
        unsafe {
            let memory = indexed_operand(&mut scratch, 7, 3, table);
            self.pmulhw_mem(reg, memory);
        }
    }
}

unsafe fn patch_relative(address: u32, target: u32) {
    if address != 0 {
        unsafe { write_unaligned(address as *mut u32, target.wrapping_sub(address).wrapping_sub(4)) };
    }
}

port! {
    /// Build a packed-pixel routine, patch its local branches, and report its byte count and feature bits.
    /// A zero loop or test mode emits just a return and reports one byte.
    /// The entry reserves the original scratch area before entering Rust. Unused instruction bytes
    /// in that area are copied to the game's queue, and must retain the caller's stack contents.
    #[port(address = 0x008E_FFF0, owner = "shared", name = "BuildPackedPixelRoutine", naked)]
    pub unsafe extern "C" fn fc4p_008efff0() {
        core::arch::naked_asm!(
            "mov edx, esp",
            "sub esp, 0xe4",
            "push dword ptr [edx + 16]",
            "push dword ptr [edx + 12]",
            "push dword ptr [edx + 8]",
            "push dword ptr [edx + 4]",
            "push edx",
            "call {body}",
            "add esp, 20",
            "add esp, 0xe4",
            "ret",
            body = sym build_packed_pixel_routine,
        )
    }
}

unsafe extern "C" fn build_packed_pixel_routine(
    entry: *mut u32,
    output: *mut u8,
    options: u32,
    extra: u32,
    length: *mut u32,
) -> u32 {
    unsafe {
        let mut options = options;
        let mut extra = extra;
        let scratch = entry.cast::<u8>().sub(0xc0);
        let b = Emitter::from_ptr(scratch);
        b.set_start(output as u32);
        b.set_cursor(output as u32);
        for offset in [
            0x0c, 0x24, 0x28, 0x30, 0x34, 0x38, 0x3c, 0x40, 0x44, 0x48, 0x4c, 0x50, 0x54,
        ] {
            write_unaligned(scratch.add(offset).cast::<u32>(), 0);
        }
        queue_new(b.as_ptr().add(0x30), 0);
        let loop_mode = options >> 9 & 7;
        let first_condition = (*condition_table())[loop_mode as usize];
        let loop_condition = (*loop_condition_table())[loop_mode as usize];
        let has_loop = loop_mode != 7 || options & 0x1000 != 0;
        let primary_mode = options & 7;
        let masked = options & 0x2000 != 0 && (primary_mode != 0 || options & 0x38 != 8);
        let pack_secondary = primary_mode >= 6 || options & 0x38 != 0;
        let constants = options & 0x8000 != 0 && (options & 0x4000 == 0 || options & 0xe0000 != 0) && masked;
        let pack_primary =
            constants || (primary_mode != 0 && primary_mode != 1) || options & 0x38 != 0 || extra & 0x1000 != 0;
        if loop_mode == 0 || options & 0x1c0 == 0 {
            b.ret();
            write_unaligned(length, 1);
            queue_delete(b.as_ptr().add(0x30));
            return 0;
        }
        if options & 0xe0000 == 0x80000 {
            let mode = options >> 21 & 7;
            let left = options >> 27 & 3;
            let right = options >> 29 & 3;
            if mode != 0 && mode != 4 && left > right {
                options = (options >> 2 & 0x18000000) | (options & 0x87ffffff) | (left << 29);
                write_unaligned(entry.add(2), options);
            }
            let mode = options >> 24 & 7;
            let left = extra >> 2 & 3;
            let right = extra >> 4 & 3;
            if mode != 0 && mode != 4 && left > right {
                extra = (left << 4) | (extra >> 2 & 0xc) | (extra & 0xffffffc3);
                write_unaligned(entry.add(3), extra);
            }
        }
        let (primary_reg, secondary_reg) = if primary_mode == 0 && options & 0x38 != 0 {
            (6, 3)
        } else {
            (3, 6)
        };
        let mut skip_patch = 0;
        let mut loop_patch = 0;
        let mut first_patch = 0;
        let mut second_patch = 0;
        let mut short_patch_address = 0;
        let mut test_patch = 0;
        let mut loop_target = 0;
        let mut test_target = 0;
        b.load(1, &Operand::new(4, 0, 2, 0));
        b.load(6, &Operand::new(8, 0, 3, 0));
        b.mov(2, 7);
        let auxiliary = options & 0x4000 != 0;
        if auxiliary {
            b.store(&Operand::memory(operand_00b5c9e8().cast()), 4);
            b.load(4, &Operand::memory(operand_00b5c5ac().cast()));
            b.movq_load(1, &Operand::new(8, 1, 0x43, 8));
        }
        if constants {
            b.movq_load(5, &Operand::new(8, 1, 0x43, 0x18));
            b.movq_load(7, &Operand::new(8, 1, 0x43, 0x20));
            if !auxiliary && options & 0x10000 != 0 {
                b.movq_load(0, &Operand::memory(operand_00b5ca08().cast()));
                b.movq_load(1, &Operand::memory(operand_00b5ca10().cast()));
            }
        }
        if has_loop {
            b.load(0, &Operand::new(8, 1, 0x43, 0x38));
            b.load(7, &Operand::memory(operand_00b5c9ec().cast()));
        }
        b.flush();
        let entry_target = b.cursor();
        b.mov(3, 6);
        b.add_imm(6, 0x7fff);
        b.and_imm(6, 0xffff8000);
        b.add_imm(1, 0x7fff);
        b.shr(1, 15);
        b.sub(3, 6);
        b.store(&Operand::memory(operand_00b5c9e0().cast()), 2);
        if has_loop {
            b.lea(2, &Operand::new(0x82, 1, 4, 0x4f));
        }
        b.movd_from_gpr(2, 3);
        let shuffle = extra & 0x2000 != 0;
        if shuffle {
            b.pshufw(2, 2, 0);
        } else {
            b.punpcklwd(2, 2);
            b.punpckldq(2, 2);
        }
        b.store(&Operand::memory(operand_00b5c9e4().cast()), 5);
        b.shr(6, 15);
        b.sub(6, 1);
        let exit_patch = b.patch(Some(13));
        b.add(6, 6);
        b.lea(1, &Operand::new(0x22, 2, 0x44, 0x8d));
        if auxiliary {
            b.movq_load(3, &Operand::memory(operand_00b5ca28().cast()));
            b.pmaddwd(3, 2);
            b.psrld(3, 2);
            b.psubd(1, 3);
        }
        if constants {
            if options & 0x10000 != 0 {
                b.movq_load(3, &Operand::memory(operand_00b5ca08().cast()));
                b.paddd(3, 3);
                b.pslld(2, 16);
                b.movq(6, 3);
                b.psllw(3, 1);
                b.pmaddwd(3, 2);
                b.psrld(2, 16);
                b.pmaddwd(6, 2);
                b.pslld(2, 16);
                b.pslld(3, 15);
                b.paddd(6, 3);
                b.psubd(5, 6);
                b.movq_load(3, &Operand::memory(operand_00b5ca10().cast()));
                b.paddd(3, 3);
                b.movq(6, 3);
                b.psllw(3, 1);
                b.pmaddwd(3, 2);
                b.psrld(2, 16);
                b.pmaddwd(6, 2);
                b.pslld(2, 16);
                b.pslld(3, 15);
                b.paddd(6, 3);
                b.psubd(7, 6);
            } else if !auxiliary || options & 0xe0000 == 0x20000 {
                let shift = if auxiliary { 12 } else { 19 };
                b.psrad(5, shift);
                b.psrad(7, shift);
                b.packssdw(5, 7);
            }
        }
        if has_loop {
            b.sar(3, 7);
            b.imul_mem(3, &Operand::memory(operand_00b5ca30().cast()));
            b.sub(0, 3);
            b.load(5, &Operand::memory(operand_00b5ca00().cast()));
            b.shl(0, 16);
            if options & 0xe00 == 0xe00 {
                if options & 0x1c0 != 0x1c0 {
                    skip_patch = b.patch(None);
                }
            } else {
                b.flush();
                let first_target = b.cursor();
                b.cmp_word_mem(0, &Operand::new(0x44, 1, 4, 0x32));
                first_patch = b.patch(Some(first_condition));
                b.flush();
                let second_target = b.cursor();
                if constants && options & 0x10000 != 0 {
                    b.restore_constants(auxiliary);
                }
                b.add(0, 5);
                b.adc_zero();
                if auxiliary {
                    b.paddd(1, 0);
                }
                b.add_imm(6, 2);
                b.jump_short(12, first_target);
                loop_patch = b.patch(None);
                b.align();
                b.flush();
                loop_target = b.cursor();
                if masked {
                    if pack_primary {
                        b.packuswb(primary_reg, primary_reg);
                    }
                    if !shuffle || pack_secondary {
                        let reg = if primary_mode == 0 && options & 0x38 != 0 { 6 } else { 3 };
                        b.movd_store(&Operand::new(0x42, 1, 4, 0x71), reg);
                    } else {
                        b.pcmpeqd(secondary_reg, secondary_reg);
                        b.psllq(secondary_reg, 0x20);
                        b.lea(7, &Operand::new(0x42, 1, 4, 0x71));
                        b.maskmovq(primary_reg, secondary_reg);
                    }
                }
                b.flush();
                test_target = b.cursor();
                if constants && options & 0x10000 != 0 {
                    b.restore_constants(auxiliary);
                }
                b.add_imm(6, 2);
                b.jump_short(12, second_target);
                second_patch = b.patch(None);
            }
            b.align();
        } else if options & 0x1c0 != 0x1c0 {
            skip_patch = b.patch(None);
        }
        let test_mode = options >> 6 & 7;
        let mut test_loop_target = 0;
        if test_mode != 7 {
            b.align();
            b.flush();
            test_loop_target = b.cursor();
            if has_loop {
                let mode = options >> 9 & 7;
                if mode != 7 {
                    b.load_word(7, &Operand::new(0x44, 2, 0x44, 0x232));
                }
                b.add(0, 5);
                b.adc_zero();
                if mode != 7 {
                    b.cmp_word(0, 7);
                    b.jump(loop_condition, test_target);
                }
            }
            if auxiliary {
                b.scale(options, shuffle);
                if extra & 0x1000 == 0 {
                    b.paddd(1, 0);
                }
                b.pand_mem(2, &Operand::memory(operand_00b5c580().cast()));
                b.movd_to_gpr(3, 2);
            }
            if constants && options & 0x10000 != 0 {
                b.restore_constants(auxiliary);
            }
            b.add_imm(6, 2);
            short_patch_address = b.patch_short(12);
            test_patch = b.patch(None);
            b.align();
        }
        b.flush();
        let body_target = b.cursor();
        if auxiliary {
            b.expand(options, shuffle);
            if options & 0x100000 != 0 {
                b.pslld(2, 20);
                b.packssdw(2, 2);
                b.packuswb(2, 2);
            } else if shuffle {
                b.packuswb(2, 2);
            } else {
                b.psllq(2, 48);
            }
            b.pand_mem(2, &Operand::memory(operand_00b5c580().cast()));
            b.movd_to_gpr(3, 2);
            if extra & 0x1000 == 0 {
                b.paddd(1, 0);
            }
        }
        b.flush();
        let color_target = b.cursor();
        if short_patch_address != 0 {
            *(short_patch_address as *mut u8) = color_target.wrapping_sub(short_patch_address).wrapping_sub(1) as u8;
        }
        if masked && pack_secondary && shuffle {
            b.maskmovq_mem(&Operand::new(0x42, 2, 0x44, 0x2071), 1);
        }
        if !auxiliary {
            if !constants {
                b.pcmpeqb(3, 3);
            } else {
                b.movq(3, 5);
                if options & 0x10000 != 0 {
                    b.movq(6, 7);
                    b.psrad(3, 19);
                    b.psrad(6, 19);
                    b.packssdw(3, 6);
                }
            }
        } else {
            b.movd_load(3, &Operand::new(0x18, 1, 4, 0x9c));
            let extra_color = extra & 0x1000 != 0;
            if extra_color {
                b.movq(6, 1);
                let shift = if options & 0x100000 == 0 {
                    8
                } else {
                    b.paddsw_mem(6, &Operand::memory(operand_00b5c588().cast()));
                    b.psubd_mem(6, &Operand::memory(operand_00b5c588().cast()));
                    4
                };
                b.pslld(6, shift);
                if shuffle {
                    b.pshufw(6, 6, 0xaa);
                } else {
                    b.punpckhwd(6, 6);
                    b.punpckldq(6, 6);
                }
                b.psllw(6, 1);
                b.punpcklbw(3, 4);
                b.movd_load(2, &Operand::new(0x18, 5, 0x84, 0x4009c));
                b.punpcklbw(2, 4);
                b.psubw(2, 3);
                b.paddw(2, 2);
                b.pmulhw(2, 6);
                b.paddw(3, 2);
                b.mov_imm(7, 0x80);
                b.and(7, 3);
                b.and_imm(3, 0xff7f);
                b.inc(3);
                b.xor(3, 7);
                b.and_mem(3, &Operand::memory(operand_00b5c580().cast()));
                b.movd_load(2, &Operand::new(0x18, 1, 4, 0x9c));
                b.punpcklbw(2, 4);
                b.movd_load(4, &Operand::new(0x18, 5, 0x84, 0x4009c));
                b.punpcklbw(4, 4);
                b.psllw(4, 8);
                b.psubw(4, 2);
                b.paddw(4, 4);
                b.pmulhw(4, 6);
                b.paddw(2, 4);
                b.movq(6, 1);
                let shift = if options & 0x100000 == 0 {
                    8
                } else {
                    b.paddsw_mem(6, &Operand::memory(operand_00b5c588().cast()));
                    b.psubd_mem(6, &Operand::memory(operand_00b5c588().cast()));
                    4
                };
                b.pslld(6, shift);
                if shuffle {
                    b.pshufw(6, 6, 0);
                } else {
                    b.punpcklwd(6, 6);
                    b.punpckldq(6, 6);
                }
                b.psllw(6, 1);
                b.psubw(2, 3);
                b.paddw(2, 2);
                b.pmulhw(2, 6);
                b.paddw(3, 2);
                b.pxor(4, 4);
                b.paddd(1, 0);
            }
            let combine_mode = options >> 17 & 7;
            if combine_mode != 4 {
                b.expand(options, shuffle);
            }
            if !extra_color {
                if combine_mode == 3 {
                    b.punpcklbw(3, 3);
                } else if pack_primary {
                    b.punpcklbw(3, 4);
                }
            } else if combine_mode == 3 {
                b.packuswb(3, 3);
                b.punpcklbw(3, 3);
            }
            if combine_mode != 4 {
                match combine_mode {
                    1 if options & 0x8000 != 0 => {
                        b.paddw(3, 3);
                        if options & 0x10000 == 0 {
                            b.pmulhw(3, 5);
                        } else {
                            b.movq(4, 5);
                            b.movq(6, 7);
                            b.psrad(4, 12);
                            b.psrad(6, 12);
                            b.packssdw(4, 6);
                            b.pmulhw(3, 4);
                            b.pxor(4, 4);
                        }
                    }
                    2 => {
                        b.movq(6, 3);
                        b.psrlw(3, 2);
                        b.psllq(6, 50);
                        b.movd_to_gpr(7, 6);
                        b.movq(4, 7);
                        b.movq(6, 5);
                        b.psrad(4, 17);
                        b.psrad(6, 17);
                        b.indexed(3, operand_00b5c5c8().cast());
                        b.packssdw(6, 4);
                        b.indexed(6, operand_00b5c7c8().cast());
                        b.pxor(4, 4);
                        b.paddw(3, 6);
                    }
                    3 => {
                        b.psllw(3, 1);
                        b.movq(4, 7);
                        b.movq(6, 5);
                        b.psrad(4, 12);
                        b.psrad(6, 12);
                        b.packssdw(6, 4);
                        b.movq_load(4, &Operand::memory(operand_00b160e0().cast()));
                        b.pxor(4, 3);
                        b.pmulhw_mem(3, &Operand::memory(operand_00b5c9c8().cast()));
                        b.pmulhw(6, 4);
                        b.pxor(4, 4);
                        b.paddw(3, 6);
                        b.psraw(3, 6);
                    }
                    _ => {}
                }
            } else {
                // Two independently selected packed sources feed the color and alpha operations.
                let source = options >> 27 & 3;
                if source == 1 || options & 0x60000000 == 0x20000000 {
                    if !constants {
                        b.pcmpeqb(6, 6);
                    } else {
                        b.movq(6, 5);
                        b.movq(4, 7);
                        b.psrad(6, 19);
                        b.psrad(4, 19);
                        b.packssdw(6, 4);
                    }
                }
                match source {
                    0 => b.movq(2, 3),
                    1 => b.movq(2, 6),
                    2 => b.movq_load(2, &Operand::memory(operand_00b5c9d8().cast())),
                    _ => {}
                }
                let operation = options >> 21 & 7;
                let other = options >> 29 & 3;
                match operation {
                    1 => {
                        b.paddw(2, 2);
                        match other {
                            0 | 1 => {
                                b.movq(4, if other == 0 { 3 } else { 6 });
                                b.psrlw(4, 7);
                                b.pmulhw(2, 4);
                            }
                            2 => b.pmulhw_mem(2, &Operand::memory(operand_00b5c9d0().cast())),
                            _ => {}
                        }
                    }
                    2 | 3 => {
                        if operation == 3 {
                            b.psubw_mem(2, &Operand::memory(operand_00b160d8().cast()));
                        }
                        match other {
                            0 => b.paddsw(2, 3),
                            1 => b.paddsw(2, 6),
                            2 => b.paddsw_mem(2, &Operand::memory(operand_00b5c9d8().cast())),
                            _ => {}
                        }
                    }
                    4 if source != other => {
                        match extra & 3 {
                            mode @ (0 | 1) => {
                                b.packuswb(4, if mode == 0 { 3 } else { 6 });
                                b.psllq(4, 58);
                                b.movd_to_gpr(7, 4);
                            }
                            2 => {
                                b.load_word(7, &Operand::memory(operand_00b5c9de().cast()));
                                b.shr(7, 2);
                            }
                            _ => {}
                        }
                        match other {
                            0 => b.movq(4, 3),
                            1 => b.movq(4, 6),
                            2 => b.movq_load(4, &Operand::memory(operand_00b5c9d8().cast())),
                            _ => {}
                        }
                        b.paddw(2, 2);
                        b.indexed(2, operand_00b5c5c8().cast());
                        b.paddw(4, 4);
                        b.indexed(4, operand_00b5c7c8().cast());
                        b.paddw(2, 4);
                    }
                    _ => {}
                }
                let alpha_operation = options >> 24 & 7;
                let same = alpha_operation == operation
                    && source == extra >> 2 & 3
                    && (options >> 25 ^ extra) & 0x30 == 0
                    && (extra >> 6 ^ extra) & 3 == 0
                    && extra >> 8 & 3 == extra >> 10 & 3;
                if same {
                    match extra >> 8 & 3 {
                        1 => b.paddw(2, 2),
                        2 => b.psrlw(2, 2),
                        _ => {}
                    }
                    b.movq(3, 2);
                } else {
                    let alpha_source = extra >> 2 & 3;
                    match alpha_source {
                        1 => b.movq(3, 6),
                        2 => b.movq_load(3, &Operand::memory(operand_00b5c9d8().cast())),
                        _ => {}
                    }
                    let alpha_other = extra >> 4 & 3;
                    match alpha_operation {
                        1 => match alpha_other {
                            0 => {
                                b.pmullw(3, 3);
                                b.psllw(3, 8);
                            }
                            1 => {
                                b.movq(4, 7);
                                b.packuswb(3, 3);
                                b.punpcklbw(3, 3);
                                b.psllw(3, 1);
                                b.psrad(4, 19);
                                b.packssdw(4, 4);
                                b.pmulhw(3, 4);
                            }
                            2 => {
                                b.paddw(3, 3);
                                b.pmulhw_mem(3, &Operand::memory(operand_00b5c9d0().cast()));
                            }
                            _ => {}
                        },
                        2 | 3 => {
                            match alpha_other {
                                0 => b.paddsw(3, 3),
                                1 => b.paddsw(3, 6),
                                2 => b.paddsw_mem(3, &Operand::memory(operand_00b5c9d8().cast())),
                                _ => {}
                            }
                            if alpha_operation == 3 {
                                b.psubw_mem(3, &Operand::memory(operand_00b160d8().cast()));
                            }
                        }
                        4 if alpha_source != alpha_other => {
                            match extra >> 6 & 3 {
                                0 => {
                                    b.load_byte(7, &Operand::new(0x18, 1, 4, 0x9c));
                                    b.shr(7, 26);
                                }
                                1 => {
                                    b.packuswb(4, 6);
                                    b.psllq(4, 58);
                                    b.movd_to_gpr(7, 4);
                                }
                                2 => {
                                    let address = read_unaligned(operand_00b5c9d8()).wrapping_add(6);
                                    b.load_word(7, &Operand::new(0, 4, 5, address as u64));
                                    b.shr(7, 2);
                                }
                                _ => {}
                            }
                            match alpha_other {
                                0 => {
                                    b.movd_load(4, &Operand::new(0x18, 1, 4, 0x9c));
                                    b.punpcklbw(4, 4);
                                    b.psllw(4, 8);
                                }
                                1 => b.movq(4, 6),
                                2 => b.movq_load(4, &Operand::memory(operand_00b5c9d8().cast())),
                                _ => {}
                            }
                            b.paddw(3, 3);
                            b.indexed(3, operand_00b5c5c8().cast());
                            b.paddw(4, 4);
                            b.indexed(4, operand_00b5c7c8().cast());
                            b.paddw(3, 4);
                        }
                        _ => {}
                    }
                    match extra >> 8 & 3 {
                        1 => b.paddw(2, 2),
                        2 => b.psrlw(2, 2),
                        _ => {}
                    }
                    match extra >> 10 & 3 {
                        1 => b.paddw(3, 3),
                        2 => b.psrlw(3, 2),
                        _ => {}
                    }
                    b.movq_load(4, &Operand::memory(operand_00b160d0().cast()));
                    b.pand(3, 4);
                    b.pandn(4, 2);
                    b.paddw(3, 4);
                }
                b.pxor(4, 4);
                b.expand(options, shuffle);
            }
        }
        if test_mode != 7 {
            if !pack_primary {
                b.movd_to_gpr(7, 3);
                b.shr(7, 24);
            } else {
                b.packuswb(6, 3);
                b.psllq(6, 56);
                b.movd_to_gpr(7, 6);
            }
            b.cmp_mem(7, &Operand::memory(operand_00b5c5bc().cast()));
            let condition = match test_mode {
                1 => 5,
                2 => 13,
                3 => 15,
                4 => 14,
                5 => 12,
                6 => 4,
                _ => 0,
            };
            if test_mode != 0 {
                b.jump_short(condition, test_loop_target);
            }
        }
        if masked {
            let secondary_mode = options >> 3 & 7;
            if primary_mode == 4 || primary_mode == 5 || secondary_mode == 4 || secondary_mode == 5 {
                b.movq(6, 3);
                b.packuswb(6, 6);
                b.psllq(6, 58);
                b.movd_to_gpr(7, 6);
            }
            if pack_secondary {
                b.movd_load(6, &Operand::new(0x42, 1, 4, 0x71));
                b.punpcklbw(6, 4);
            }
            match primary_mode {
                4 | 5 => {
                    b.psrlw(3, 2);
                    b.indexed(
                        3,
                        if primary_mode == 4 {
                            operand_00b5c5c8().cast()
                        } else {
                            operand_00b5c7c8().cast()
                        },
                    );
                }
                6 => {
                    b.pmullw(3, 6);
                    b.psllw(3, 8);
                }
                7 => {
                    b.pmullw(3, 6);
                    b.movq(4, 6);
                    b.psllw(3, 8);
                    b.psubw(4, 6);
                    b.movq(3, 4);
                    b.pxor(4, 4);
                }
                _ => {}
            }
            match secondary_mode {
                2 => {
                    b.pmullw(6, 3);
                    b.psllw(6, 8);
                }
                3 => {
                    b.movq(4, 6);
                    b.pmullw(6, 3);
                    b.psllw(6, 8);
                    b.psubw(4, 6);
                    b.movq(6, 4);
                    b.pxor(4, 4);
                }
                4 | 5 => {
                    b.psrlw(6, 2);
                    b.indexed(
                        6,
                        if secondary_mode == 4 {
                            operand_00b5c5c8().cast()
                        } else {
                            operand_00b5c7c8().cast()
                        },
                    );
                }
                _ => {}
            }
            if primary_mode == 0 {
                if secondary_mode == 0 {
                    b.pxor(3, 3);
                }
            } else if secondary_mode != 0 {
                b.paddw(3, 6);
            }
        }
        if has_loop {
            let mode = options >> 9 & 7;
            if mode != 7 {
                b.load_word(7, &Operand::new(0x44, 2, 0x44, 0x232));
            }
            if options & 0x1000 != 0 {
                b.store_word(&Operand::new(0x44, 1, 4, 0x32), 0);
            }
            b.add(0, 5);
            b.adc_zero();
            if mode != 7 {
                b.cmp_word(0, 7);
                b.jump(loop_condition, loop_target);
            }
        }
        if auxiliary {
            b.scale(options, shuffle);
            if extra & 0x1000 == 0 {
                b.paddd(1, 0);
            }
            b.pand_mem(2, &Operand::memory(operand_00b5c580().cast()));
        }
        if masked && pack_primary {
            b.packuswb(primary_reg, primary_reg);
        }
        if auxiliary {
            b.movd_to_gpr(3, 2);
        }
        if masked {
            if !shuffle || pack_secondary {
                b.movd_store(&Operand::new(0x42, 1, 4, 0x71), primary_reg);
            } else {
                b.pcmpeqd(secondary_reg, secondary_reg);
                b.psllq(secondary_reg, 32);
                b.lea(7, &Operand::new(0x42, 1, 4, 0x71));
                b.maskmovq(primary_reg, secondary_reg);
            }
        }
        if constants && options & 0x10000 != 0 {
            b.restore_constants(auxiliary);
        }
        b.add_imm(6, 2);
        b.jump(12, color_target);
        b.flush();
        let exit_target = b.cursor();
        b.load(3, &Operand::memory(operand_00b5ca3c().cast()));
        b.load(7, &Operand::memory(operand_00b5ca40().cast()));
        b.load(5, &Operand::memory(operand_00b5c9e4().cast()));
        b.load(2, &Operand::memory(operand_00b5c9e0().cast()));
        b.load(1, &Operand::new(0x80, 0, 7, 0));
        b.load(6, &Operand::new(8, 0, 3, 0));
        b.add_mem(1, &Operand::new(0x80, 1, 0x47, 4));
        b.add_mem(6, &Operand::new(8, 1, 0x43, 4));
        b.store(&Operand::new(0x80, 0, 7, 0), 1);
        b.load(0, &Operand::memory(operand_00b5c5a0().cast()));
        b.store(&Operand::new(8, 0, 3, 0), 6);
        b.sub(5, 0);
        if auxiliary {
            b.movq_load(1, &Operand::new(8, 1, 0x43, 8));
            b.paddd_mem(1, &Operand::new(8, 1, 0x43, 16));
            b.movq_store(&Operand::new(8, 1, 0x43, 8), 1);
        }
        b.movq_load(5, &Operand::new(8, 1, 0x43, 24));
        b.movq_load(7, &Operand::new(8, 1, 0x43, 32));
        if options & 0x10000 != 0 {
            b.paddd_mem(5, &Operand::new(8, 1, 0x43, 40));
            b.paddd_mem(7, &Operand::new(8, 1, 0x43, 48));
            b.movq_store(&Operand::new(8, 1, 0x43, 24), 5);
            b.movq_store(&Operand::new(8, 1, 0x43, 32), 7);
        }
        if has_loop {
            b.load(0, &Operand::new(8, 1, 0x43, 56));
            b.load(7, &Operand::memory(operand_00b5c9ec().cast()));
            b.add_mem(0, &Operand::new(8, 1, 0x43, 60));
            b.sub_mem(7, &Operand::memory(operand_00b5c5a4().cast()));
            b.store(&Operand::new(8, 1, 0x43, 56), 0);
            b.store(&Operand::memory(operand_00b5c9ec().cast()), 7);
        }
        b.dec(2);
        b.jump(15, entry_target);
        if auxiliary {
            b.load(4, &Operand::memory(operand_00b5c9e8().cast()));
        }
        if shuffle {
            b.sfence();
        }
        b.ret();
        patch_relative(skip_patch, body_target);
        patch_relative(exit_patch, exit_target);
        patch_relative(first_patch, body_target);
        patch_relative(loop_patch, exit_target);
        patch_relative(second_patch, exit_target);
        patch_relative(test_patch, exit_target);
        b.flush();
        write_unaligned(length, b.cursor().wrapping_sub(b.start()));
        queue_delete(b.as_ptr().add(0x30));
        let return_options = read_unaligned(entry.add(2));
        ((return_options >> 20 & 1) << 3)
            | u32::from(constants)
            | (u32::from(has_loop) << 2)
            | (u32::from(auxiliary) << 1)
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[fc4p_008efff0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, fake_data, Fake};
    use std::cell::RefCell;

    const SEED: u64 = 0xcbf29ce484222325;

    fn hash(hash: u64, word: u32) -> u64 {
        (hash ^ u64::from(word)).wrapping_mul(0x100000001b3)
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Kind {
        Plain,
        MemoryFirst,
        MemoryLast,
        Indexed,
        New,
        Delete,
        JumpPatch,
        ConditionPatch,
        ShortPatch,
        Align,
        Flush,
    }

    #[derive(Default)]
    struct State {
        base: u32,
        builder: u32,
        constant: u32,
        position: u32,
        trace: u64,
        calls: Vec<Vec<u32>>,
        check_unused_bytes: bool,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    // Every ordinary emitter writes a distinct sixteen-byte token. Branch emitters return addresses
    // inside those tokens, making every byte of the port's later branch patches observable.
    fn invoke(address: u32, kind: Kind, this: *mut u8, args: &[u32]) -> u32 {
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            unsafe {
                if kind == Kind::New {
                    s.builder = this as u32 - 0x30;
                    assert_eq!(read_unaligned((s.builder as *const u32).add(0)), s.base);
                    assert_eq!(read_unaligned((s.builder as *const u32).add(1)), s.base);
                    for offset in [12, 36, 40, 48, 52, 56, 60, 64, 68, 72, 76, 80, 84] {
                        assert_eq!(read_unaligned((s.builder + offset) as *const u32), 0);
                    }
                    if s.check_unused_bytes {
                        assert_eq!(read_unaligned((s.builder + 16) as *const [u8; 16]), [0xa5; 16]);
                    }
                }
                let tag = match kind {
                    Kind::New | Kind::Delete => 1,
                    Kind::Indexed => 2,
                    _ => 0,
                };
                if tag == 0 {
                    assert_eq!(this as u32, s.builder);
                }
                if tag == 1 {
                    assert_eq!(this as u32, s.builder + 0x30);
                }
                let mut words = vec![address, tag, args.len() as u32];
                for (i, &arg) in args.iter().enumerate() {
                    if kind == Kind::MemoryFirst && i == 0 || kind == Kind::MemoryLast && i == 1 {
                        let operand = &*(arg as *const [u8; 12]);
                        let n = operand[4] as usize;
                        assert!(n <= 5);
                        let mut tail = [0u8; 8];
                        tail[..n].copy_from_slice(&operand[6..6 + n]);
                        let mut tail = u64::from_le_bytes(tail);
                        if n == 4 && tail as u32 == s.constant {
                            tail = 0xb5c9d8;
                        }
                        words.extend_from_slice(&[
                            u32::from_le_bytes(operand[..4].try_into().unwrap()),
                            n as u32,
                            u32::from(operand[5]),
                            tail as u32,
                            (tail >> 32) as u32,
                        ]);
                    } else if matches!(kind, Kind::JumpPatch | Kind::ConditionPatch | Kind::ShortPatch) && i == 0 {
                        words.push(0x80000001);
                    } else if matches!(address, 0x008ee0a0 | 0x008ee160) && i == 1 {
                        words.push(arg.wrapping_sub(s.base));
                    } else {
                        words.push(arg);
                    }
                }
                for &word in &words {
                    s.trace = hash(s.trace, word);
                }
                s.calls.push(words);
                if kind == Kind::Indexed {
                    let operand = &mut *(this.cast::<[u8; 12]>());
                    operand[..4].copy_from_slice(&(1u32 << (args[0] & 31)).to_le_bytes());
                    operand[4] = 5;
                    operand[5] = 4;
                    operand[6] = ((args[1] * 8 + args[0]) * 8 + 5) as u8;
                    operand[7..11].copy_from_slice(&args[2].to_le_bytes());
                    this as u32
                } else if matches!(kind, Kind::New | Kind::Delete | Kind::Flush) {
                    0xdeadbeef
                } else {
                    let cursor = read_unaligned((s.builder + 4) as *const u32);
                    let count = if kind == Kind::Align {
                        cursor.wrapping_neg() & 15
                    } else if address == 0x008ee200 {
                        1
                    } else {
                        16
                    };
                    let token = if kind == Kind::Align {
                        0x90
                    } else {
                        (address ^ (address >> 8)) as u8
                    };
                    core::ptr::write_bytes(cursor as *mut u8, token, count as usize);
                    write_unaligned((s.builder + 4) as *mut u32, cursor + count);
                    s.position = cursor + count;
                    if matches!(kind, Kind::JumpPatch | Kind::ConditionPatch | Kind::ShortPatch) {
                        write_unaligned(
                            args[0] as *mut u32,
                            cursor + count - if kind == Kind::ShortPatch { 1 } else { 4 },
                        );
                        args[0]
                    } else {
                        0xdeadbeef
                    }
                }
            }
        })
    }

    macro_rules! emitter_fake {
        ($name:ident, $address:literal, $kind:ident $(, $arg:ident)*) => {
            extern "thiscall" fn $name(this: *mut u8 $(, $arg: u32)*) -> u32 {
                invoke($address, Kind::$kind, this, &[$($arg),*])
            }
        };
    }

    fn run(options: u32, extra: u32, alignment: usize) -> (u32, u32, usize, u64, u64) {
        run_entry(options, extra, alignment, false)
    }

    fn run_entry(options: u32, extra: u32, alignment: usize, probe: bool) -> (u32, u32, usize, u64, u64) {
        #[repr(align(16))]
        struct Buffer([u8; 65536]);
        let mut buffer = Box::new(Buffer([0xcc; 65536]));
        let output = buffer.0.as_mut_ptr().wrapping_add(alignment);
        let mut constant = 0xfffffffcu32;
        let _functions = install_functions();
        let _globals = install_globals(&mut constant);
        let mut conditions = [5, 13, 15, 14, 12, 4, 6, 7];
        let mut loops = [1, 3, 2, 4, 5, 6, 7, 8];
        let _conditions = fake_data(0x00acc920, conditions.as_mut_ptr().cast());
        let _loops = fake_data(0x00acc940, loops.as_mut_ptr().cast());
        STATE.with(|s| {
            *s.borrow_mut() = State {
                base: output as u32,
                constant: &mut constant as *mut u32 as u32,
                trace: SEED,
                check_unused_bytes: probe,
                ..State::default()
            }
        });
        let mut length = 0xdeadbeef;
        let entry: unsafe extern "C" fn(*mut u8, u32, u32, *mut u32) -> u32 =
            unsafe { core::mem::transmute(fc4p_008efff0 as *const ()) };
        let result = if probe {
            let mut record = [
                output as u32,
                options,
                extra,
                &mut length as *mut u32 as u32,
                0,
                0,
                0,
                0,
                0,
                0,
            ];
            unsafe { probe_entry(record.as_mut_ptr()) };
            assert_eq!(record[7..], [0x12345678, 0x87654321, 0xabcdef01]);
            let expected = if options == 0x3128e3c0 {
                (0x4928e3c0, 0x24)
            } else {
                (options, extra)
            };
            assert_eq!((record[5], record[6]), expected);
            record[4]
        } else {
            unsafe { entry(output, options, extra, &mut length) }
        };
        STATE.with(|state| {
            let s = state.borrow();
            let end = s.position.wrapping_sub(buffer.0.as_ptr() as u32) as usize;
            let memory = buffer.0[..end + 32].iter().fold(SEED, |h, &v| hash(h, u32::from(v)));
            assert!(buffer.0[end..].iter().all(|&b| b == 0xcc));
            assert!(buffer.0[..alignment].iter().all(|&b| b == 0xcc));
            if options >> 9 & 7 == 0 || options & 0x1c0 == 0 {
                assert_eq!(length, 1);
                assert_eq!(result, 0);
            } else {
                assert_eq!(length as usize, end - alignment);
            }
            (result, length, s.calls.len(), s.trace, memory)
        })
    }

    // Seed the future scratch area, call with ordinary cdecl arguments, and inspect the argument slots
    // before the caller removes them. Register sentinels also check the callee-saved convention.
    #[unsafe(naked)]
    unsafe extern "C" fn probe_entry(_record: *mut u32) {
        core::arch::naked_asm!(
            "push ebp", "push ebx", "push esi", "push edi",
            "mov edi, dword ptr [esp + 20]",
            "mov ebx, 0x12345678", "mov esi, 0x87654321", "mov ebp, 0xabcdef01",
            "sub esp, 0x100", "mov edx, esp", "mov ecx, 64",
            "2:", "mov dword ptr [edx], 0xa5a5a5a5", "add edx, 4", "dec ecx", "jnz 2b",
            "add esp, 0x100",
            "push dword ptr [edi + 12]", "push dword ptr [edi + 8]",
            "push dword ptr [edi + 4]", "push dword ptr [edi]",
            "call {entry}",
            "mov dword ptr [edi + 16], eax",
            "mov eax, dword ptr [esp + 4]", "mov dword ptr [edi + 20], eax",
            "mov eax, dword ptr [esp + 8]", "mov dword ptr [edi + 24], eax",
            "mov dword ptr [edi + 28], ebx", "mov dword ptr [edi + 32], esi",
            "mov dword ptr [edi + 36], ebp", "add esp, 16",
            "pop edi", "pop esi", "pop ebx", "pop ebp", "ret",
            entry = sym fc4p_008efff0,
        )
    }

    #[test]
    fn scratch_argument_slots_registers_and_float_environment() {
        let _fpu = crate::testing::GameFpu::new();
        let (mut before_cw, mut before_sw, mut before_mxcsr) = (0u16, 0u16, 0u32);
        let (mut after_cw, mut after_sw, mut after_mxcsr) = (0u16, 0u16, 0u32);
        let mut sentinel = 0.0f64;
        unsafe {
            core::arch::asm!(
                "fld1", "fnstcw [{cw}]", "fnstsw [{sw}]", "stmxcsr [{mx}]",
                cw = in(reg) &mut before_cw, sw = in(reg) &mut before_sw,
                mx = in(reg) &mut before_mxcsr, out("st(0)") _,
            );
        }
        run_entry(0x3128e3c0, 0x18, 0, true);
        run_entry(0, 0, 1, true);
        unsafe {
            core::arch::asm!(
                "fnstcw [{cw}]", "fnstsw [{sw}]", "stmxcsr [{mx}]", "fstp qword ptr [{value}]",
                cw = in(reg) &mut after_cw, sw = in(reg) &mut after_sw,
                mx = in(reg) &mut after_mxcsr, value = in(reg) &mut sentinel, out("st(0)") _,
            );
        }
        assert_eq!((after_cw, after_sw, after_mxcsr), (before_cw, before_sw, before_mxcsr));
        assert_eq!(sentinel.to_bits(), 1.0f64.to_bits());
    }

    #[test]
    fn routine_call_order_operands_and_patches() {
        let cases = [
            (
                0x00000000,
                0x00000000,
                0,
                0,
                1,
                3,
                0x5940de72dc383b12,
                0x78111619f0e70d25,
            ),
            (
                0xffffffff,
                0xffffffff,
                1,
                15,
                2656,
                174,
                0x045a377288dd1bdb,
                0xc5cc212dff6137ec,
            ),
            (
                0x000003c0,
                0x00000000,
                2,
                4,
                1087,
                81,
                0xc728a24f592ea257,
                0x8bbb535516490a46,
            ),
            (
                0x00000fc0,
                0x00000000,
                3,
                0,
                609,
                46,
                0x6140ac91ebf0fcc6,
                0x3b6b06a1cfd73557,
            ),
            (
                0x000043c0,
                0x00000000,
                4,
                6,
                1469,
                105,
                0xbb059c392c4e8ed1,
                0x94b1d5856412016b,
            ),
            (
                0x000043c0,
                0x00002000,
                5,
                6,
                1468,
                105,
                0x121be083fcc9dd5e,
                0xfd679ade3f99a92e,
            ),
            (
                0x008c43c0,
                0x00000000,
                6,
                6,
                1467,
                105,
                0x644e194d53ddcced,
                0x01984d45bc7126a5,
            ),
            (
                0x008c43c0,
                0x00002000,
                7,
                6,
                1466,
                105,
                0xc375d2ca4edf95c2,
                0xf3fd23488cf14ca0,
            ),
            (
                0x001003c0,
                0x00000000,
                8,
                12,
                1081,
                81,
                0xe500c67dfe7e9e3f,
                0x39a79e6a94f18f48,
            ),
            (
                0x1fffffff,
                0x00003fff,
                9,
                15,
                2648,
                174,
                0x6b913d6312fd6863,
                0xe93a28495b69c214,
            ),
            (
                0x00923ff8,
                0x0000ffff,
                10,
                12,
                967,
                69,
                0x96ed0cc52ddfd56c,
                0x3f8220f6646008d2,
            ),
            (
                0x008e43c0,
                0x00000000,
                11,
                6,
                1462,
                105,
                0x0c343b3e2e3c4e21,
                0xe74735b54a415204,
            ),
        ];
        for (options, extra, alignment, result, length, count, trace, memory) in cases {
            let actual = run(options, extra, alignment);
            assert_eq!(
                actual,
                (result, length, count, trace, memory),
                "options {options:08x}, extra {extra:08x}"
            );
        }
    }

    fn option_cases(mut visit: impl FnMut(u32, u32, usize)) {
        for source in 0..4 {
            for other in 0..4 {
                for operation in 0..8 {
                    for alpha in 0..8 {
                        for flags in 0..4 {
                            let options = 0x9e3c0 | source << 27 | other << 29 | operation << 21 | alpha << 24;
                            let extra = ((source + 1) & 3) << 2
                                | ((other + 2) & 3) << 4
                                | (operation & 3) << 6
                                | (alpha & 3)
                                | (source & 3) << 8
                                | other << 10
                                | flags << 12;
                            visit(
                                options,
                                extra,
                                ((source + other + operation + alpha + flags) & 15) as usize,
                            );
                        }
                    }
                }
            }
        }
        for source in 0..4 {
            for other in 0..4 {
                for operation in 0..8 {
                    for selector in 0..4 {
                        for shift in 0..4 {
                            let options = 0x9e3c0 | source << 27 | other << 29 | operation << 21 | operation << 24;
                            let extra = source << 2 | other << 4 | selector << 6 | selector | shift << 8 | shift << 10;
                            visit(options, extra, ((selector + shift) & 15) as usize);
                        }
                    }
                }
            }
        }
        for primary in 0..8 {
            for secondary in 0..8 {
                for loop_mode in 0..8 {
                    for test in 0..8 {
                        let options = primary
                            | secondary << 3
                            | test << 6
                            | loop_mode << 9
                            | ((primary + secondary + test + loop_mode) & 255) << 12;
                        let extra = (primary ^ test) << 10
                            | (secondary ^ loop_mode) << 6
                            | (test ^ loop_mode) << 2
                            | ((primary + loop_mode) & 3);
                        visit(options, extra, ((primary + secondary + test + loop_mode) & 15) as usize);
                    }
                }
            }
        }
        let mut random = 0x31415926u32;
        for i in 0..8192 {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let options = random;
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            visit(options, random, i & 15);
        }
    }

    // Sweep every color/alpha operation and source selector, matched-source paths, all low option
    // fields, all sixteen output alignments, and 8,192 seeded pairs. The digest includes every call
    // argument, operand byte, branch patch, byte count, and full feature word from the reference cases.
    #[test]
    fn option_field_combinations() {
        let mut aggregate = SEED;
        let mut count = 0;
        option_cases(|options, extra, alignment| {
            let (result, length, calls, trace, memory) = run(options, extra, alignment);
            for word in [
                result,
                length,
                calls as u32,
                trace as u32,
                (trace >> 32) as u32,
                memory as u32,
                (memory >> 32) as u32,
            ] {
                aggregate = hash(aggregate, word);
            }
            count += 1;
        });
        assert_eq!(count, 18432);
        assert_eq!(aggregate, 0xd0f0f613567b8686);
    }
    emitter_fake!(fake_emit_mov, 0x008ece20, Plain, a, b);
    emitter_fake!(fake_emit_mov_imm, 0x008ecec0, Plain, a, b);
    emitter_fake!(fake_emit_load, 0x008ecf80, MemoryLast, a, b);
    emitter_fake!(fake_emit_load_byte, 0x008ed010, MemoryLast, a, b);
    emitter_fake!(fake_emit_load_word, 0x008ed0b0, MemoryLast, a, b);
    emitter_fake!(fake_emit_store_word, 0x008ed150, MemoryFirst, a, b);
    emitter_fake!(fake_emit_store, 0x008ed1f0, MemoryFirst, a, b);
    emitter_fake!(fake_emit_and, 0x008ed280, Plain, a, b);
    emitter_fake!(fake_emit_and_mem, 0x008ed330, MemoryLast, a, b);
    emitter_fake!(fake_emit_and_imm, 0x008ed3c0, Plain, a, b);
    emitter_fake!(fake_emit_xor, 0x008ed480, Plain, a, b);
    emitter_fake!(fake_emit_add, 0x008ed530, Plain, a, b);
    emitter_fake!(fake_emit_add_mem, 0x008ed5e0, MemoryLast, a, b);
    emitter_fake!(fake_emit_add_imm, 0x008ed670, Plain, a, b);
    emitter_fake!(fake_emit_adc_zero, 0x008ed730, Plain);
    emitter_fake!(fake_emit_cmp_word, 0x008ed7b0, Plain, a, b);
    emitter_fake!(fake_emit_cmp_word_mem, 0x008ed860, MemoryLast, a, b);
    emitter_fake!(fake_emit_cmp_mem, 0x008ed900, MemoryLast, a, b);
    emitter_fake!(fake_emit_sub, 0x008ed990, Plain, a, b);
    emitter_fake!(fake_emit_sub_mem, 0x008eda40, MemoryLast, a, b);
    emitter_fake!(fake_emit_shr, 0x008edad0, Plain, a, b);
    emitter_fake!(fake_emit_sar, 0x008edb70, Plain, a, b);
    emitter_fake!(fake_emit_shl, 0x008edc10, Plain, a, b);
    emitter_fake!(fake_emit_inc, 0x008edcb0, Plain, a);
    emitter_fake!(fake_emit_dec, 0x008edd30, Plain, a);
    emitter_fake!(fake_emit_lea, 0x008eddb0, MemoryLast, a, b);
    emitter_fake!(fake_emit_imul_mem, 0x008ede40, MemoryLast, a, b);
    emitter_fake!(fake_emit_jump, 0x008ee0a0, Plain, a, b);
    emitter_fake!(fake_emit_jump_short, 0x008ee160, Plain, a, b);
    emitter_fake!(fake_emit_ret, 0x008ee200, Plain);
    emitter_fake!(fake_emit_movd_to_gpr, 0x008ee270, Plain, a, b);
    emitter_fake!(fake_emit_movd_from_gpr, 0x008ee320, Plain, a, b);
    emitter_fake!(fake_emit_movd_load, 0x008ee3d0, MemoryLast, a, b);
    emitter_fake!(fake_emit_movd_store, 0x008ee470, MemoryFirst, a, b);
    emitter_fake!(fake_emit_movq, 0x008ee510, Plain, a, b);
    emitter_fake!(fake_emit_movq_load, 0x008ee5c0, MemoryLast, a, b);
    emitter_fake!(fake_emit_movq_store, 0x008ee660, MemoryFirst, a, b);
    emitter_fake!(fake_emit_pand, 0x008ee700, Plain, a, b);
    emitter_fake!(fake_emit_pand_mem, 0x008ee7b0, MemoryLast, a, b);
    emitter_fake!(fake_emit_pandn, 0x008ee850, Plain, a, b);
    emitter_fake!(fake_emit_pxor, 0x008ee900, Plain, a, b);
    emitter_fake!(fake_emit_paddw, 0x008ee9d0, Plain, a, b);
    emitter_fake!(fake_emit_paddsw, 0x008eea80, Plain, a, b);
    emitter_fake!(fake_emit_paddsw_mem, 0x008eeb30, MemoryLast, a, b);
    emitter_fake!(fake_emit_paddd, 0x008eebd0, Plain, a, b);
    emitter_fake!(fake_emit_paddd_mem, 0x008eec80, MemoryLast, a, b);
    emitter_fake!(fake_emit_psubw, 0x008eed20, Plain, a, b);
    emitter_fake!(fake_emit_psubw_mem, 0x008eedd0, MemoryLast, a, b);
    emitter_fake!(fake_emit_psubd_mem, 0x008eee70, MemoryLast, a, b);
    emitter_fake!(fake_emit_psubd, 0x008eef10, Plain, a, b);
    emitter_fake!(fake_emit_psrlw, 0x008eefc0, Plain, a, b);
    emitter_fake!(fake_emit_psrld, 0x008ef070, Plain, a, b);
    emitter_fake!(fake_emit_psllw, 0x008ef120, Plain, a, b);
    emitter_fake!(fake_emit_pslld, 0x008ef1d0, Plain, a, b);
    emitter_fake!(fake_emit_psllq, 0x008ef280, Plain, a, b);
    emitter_fake!(fake_emit_psraw, 0x008ef330, Plain, a, b);
    emitter_fake!(fake_emit_psrad, 0x008ef3e0, Plain, a, b);
    emitter_fake!(fake_emit_pmullw, 0x008ef490, Plain, a, b);
    emitter_fake!(fake_emit_pmulhw, 0x008ef540, Plain, a, b);
    emitter_fake!(fake_emit_pmulhw_mem, 0x008ef5f0, MemoryLast, a, b);
    emitter_fake!(fake_emit_pmaddwd, 0x008ef690, Plain, a, b);
    emitter_fake!(fake_emit_packssdw, 0x008ef740, Plain, a, b);
    emitter_fake!(fake_emit_packuswb, 0x008ef7f0, Plain, a, b);
    emitter_fake!(fake_emit_punpcklbw, 0x008ef8a0, Plain, a, b);
    emitter_fake!(fake_emit_punpcklwd, 0x008ef950, Plain, a, b);
    emitter_fake!(fake_emit_punpckldq, 0x008efa00, Plain, a, b);
    emitter_fake!(fake_emit_punpckhbw, 0x008efab0, Plain, a, b);
    emitter_fake!(fake_emit_punpckhwd, 0x008efb60, Plain, a, b);
    emitter_fake!(fake_emit_pcmpeqb, 0x008efc10, Plain, a, b);
    emitter_fake!(fake_emit_pcmpeqd, 0x008efcc0, Plain, a, b);
    emitter_fake!(fake_emit_pshufw, 0x008efd70, Plain, a, b, c);
    emitter_fake!(fake_emit_maskmovq_mem, 0x008efe30, MemoryFirst, a, b);
    emitter_fake!(fake_emit_maskmovq, 0x008efec0, Plain, a, b);
    emitter_fake!(fake_emit_sfence, 0x008eff70, Plain);
    emitter_fake!(fake_emit_flush, 0x008ecca0, Flush);
    emitter_fake!(fake_emit_align, 0x008ecd50, Align);
    emitter_fake!(fake_queue_new, 0x008ec270, New, a);
    emitter_fake!(fake_queue_delete, 0x008ec950, Delete);
    emitter_fake!(fake_jump_patch, 0x008edee0, JumpPatch, a);
    emitter_fake!(fake_condition_patch, 0x008edf70, ConditionPatch, a, b);
    emitter_fake!(fake_short_patch, 0x008ee020, ShortPatch, a, b);
    emitter_fake!(fake_indexed_operand, 0x008ec150, Indexed, a, b, c);

    fn install_functions() -> Vec<Fake> {
        vec![
            fake(0x008ece20, fake_emit_mov as *const () as usize),
            fake(0x008ecec0, fake_emit_mov_imm as *const () as usize),
            fake(0x008ecf80, fake_emit_load as *const () as usize),
            fake(0x008ed010, fake_emit_load_byte as *const () as usize),
            fake(0x008ed0b0, fake_emit_load_word as *const () as usize),
            fake(0x008ed150, fake_emit_store_word as *const () as usize),
            fake(0x008ed1f0, fake_emit_store as *const () as usize),
            fake(0x008ed280, fake_emit_and as *const () as usize),
            fake(0x008ed330, fake_emit_and_mem as *const () as usize),
            fake(0x008ed3c0, fake_emit_and_imm as *const () as usize),
            fake(0x008ed480, fake_emit_xor as *const () as usize),
            fake(0x008ed530, fake_emit_add as *const () as usize),
            fake(0x008ed5e0, fake_emit_add_mem as *const () as usize),
            fake(0x008ed670, fake_emit_add_imm as *const () as usize),
            fake(0x008ed730, fake_emit_adc_zero as *const () as usize),
            fake(0x008ed7b0, fake_emit_cmp_word as *const () as usize),
            fake(0x008ed860, fake_emit_cmp_word_mem as *const () as usize),
            fake(0x008ed900, fake_emit_cmp_mem as *const () as usize),
            fake(0x008ed990, fake_emit_sub as *const () as usize),
            fake(0x008eda40, fake_emit_sub_mem as *const () as usize),
            fake(0x008edad0, fake_emit_shr as *const () as usize),
            fake(0x008edb70, fake_emit_sar as *const () as usize),
            fake(0x008edc10, fake_emit_shl as *const () as usize),
            fake(0x008edcb0, fake_emit_inc as *const () as usize),
            fake(0x008edd30, fake_emit_dec as *const () as usize),
            fake(0x008eddb0, fake_emit_lea as *const () as usize),
            fake(0x008ede40, fake_emit_imul_mem as *const () as usize),
            fake(0x008ee0a0, fake_emit_jump as *const () as usize),
            fake(0x008ee160, fake_emit_jump_short as *const () as usize),
            fake(0x008ee200, fake_emit_ret as *const () as usize),
            fake(0x008ee270, fake_emit_movd_to_gpr as *const () as usize),
            fake(0x008ee320, fake_emit_movd_from_gpr as *const () as usize),
            fake(0x008ee3d0, fake_emit_movd_load as *const () as usize),
            fake(0x008ee470, fake_emit_movd_store as *const () as usize),
            fake(0x008ee510, fake_emit_movq as *const () as usize),
            fake(0x008ee5c0, fake_emit_movq_load as *const () as usize),
            fake(0x008ee660, fake_emit_movq_store as *const () as usize),
            fake(0x008ee700, fake_emit_pand as *const () as usize),
            fake(0x008ee7b0, fake_emit_pand_mem as *const () as usize),
            fake(0x008ee850, fake_emit_pandn as *const () as usize),
            fake(0x008ee900, fake_emit_pxor as *const () as usize),
            fake(0x008ee9d0, fake_emit_paddw as *const () as usize),
            fake(0x008eea80, fake_emit_paddsw as *const () as usize),
            fake(0x008eeb30, fake_emit_paddsw_mem as *const () as usize),
            fake(0x008eebd0, fake_emit_paddd as *const () as usize),
            fake(0x008eec80, fake_emit_paddd_mem as *const () as usize),
            fake(0x008eed20, fake_emit_psubw as *const () as usize),
            fake(0x008eedd0, fake_emit_psubw_mem as *const () as usize),
            fake(0x008eee70, fake_emit_psubd_mem as *const () as usize),
            fake(0x008eef10, fake_emit_psubd as *const () as usize),
            fake(0x008eefc0, fake_emit_psrlw as *const () as usize),
            fake(0x008ef070, fake_emit_psrld as *const () as usize),
            fake(0x008ef120, fake_emit_psllw as *const () as usize),
            fake(0x008ef1d0, fake_emit_pslld as *const () as usize),
            fake(0x008ef280, fake_emit_psllq as *const () as usize),
            fake(0x008ef330, fake_emit_psraw as *const () as usize),
            fake(0x008ef3e0, fake_emit_psrad as *const () as usize),
            fake(0x008ef490, fake_emit_pmullw as *const () as usize),
            fake(0x008ef540, fake_emit_pmulhw as *const () as usize),
            fake(0x008ef5f0, fake_emit_pmulhw_mem as *const () as usize),
            fake(0x008ef690, fake_emit_pmaddwd as *const () as usize),
            fake(0x008ef740, fake_emit_packssdw as *const () as usize),
            fake(0x008ef7f0, fake_emit_packuswb as *const () as usize),
            fake(0x008ef8a0, fake_emit_punpcklbw as *const () as usize),
            fake(0x008ef950, fake_emit_punpcklwd as *const () as usize),
            fake(0x008efa00, fake_emit_punpckldq as *const () as usize),
            fake(0x008efab0, fake_emit_punpckhbw as *const () as usize),
            fake(0x008efb60, fake_emit_punpckhwd as *const () as usize),
            fake(0x008efc10, fake_emit_pcmpeqb as *const () as usize),
            fake(0x008efcc0, fake_emit_pcmpeqd as *const () as usize),
            fake(0x008efd70, fake_emit_pshufw as *const () as usize),
            fake(0x008efe30, fake_emit_maskmovq_mem as *const () as usize),
            fake(0x008efec0, fake_emit_maskmovq as *const () as usize),
            fake(0x008eff70, fake_emit_sfence as *const () as usize),
            fake(0x008ecca0, fake_emit_flush as *const () as usize),
            fake(0x008ecd50, fake_emit_align as *const () as usize),
            fake(0x008ec270, fake_queue_new as *const () as usize),
            fake(0x008ec950, fake_queue_delete as *const () as usize),
            fake(0x008edee0, fake_jump_patch as *const () as usize),
            fake(0x008edf70, fake_condition_patch as *const () as usize),
            fake(0x008ee020, fake_short_patch as *const () as usize),
            fake(0x008ec150, fake_indexed_operand as *const () as usize),
        ]
    }

    fn install_globals(constant: &mut u32) -> Vec<Fake> {
        vec![
            fake_data(0x00b5c9e8, 0x00b5c9e8 as *mut u8),
            fake_data(0x00b5c5ac, 0x00b5c5ac as *mut u8),
            fake_data(0x00b5ca08, 0x00b5ca08 as *mut u8),
            fake_data(0x00b5ca10, 0x00b5ca10 as *mut u8),
            fake_data(0x00b5c9ec, 0x00b5c9ec as *mut u8),
            fake_data(0x00b5c9e0, 0x00b5c9e0 as *mut u8),
            fake_data(0x00b5c9e4, 0x00b5c9e4 as *mut u8),
            fake_data(0x00b5ca28, 0x00b5ca28 as *mut u8),
            fake_data(0x00b5ca30, 0x00b5ca30 as *mut u8),
            fake_data(0x00b5ca00, 0x00b5ca00 as *mut u8),
            fake_data(0x00b5c580, 0x00b5c580 as *mut u8),
            fake_data(0x00b5c588, 0x00b5c588 as *mut u8),
            fake_data(0x00b5c5c8, 0x00b5c5c8 as *mut u8),
            fake_data(0x00b5c7c8, 0x00b5c7c8 as *mut u8),
            fake_data(0x00b160e0, 0x00b160e0 as *mut u8),
            fake_data(0x00b5c9c8, 0x00b5c9c8 as *mut u8),
            fake_data(0x00b5c9d8, (constant as *mut u32).cast()),
            fake_data(0x00b5c9d0, 0x00b5c9d0 as *mut u8),
            fake_data(0x00b5c9de, 0x00b5c9de as *mut u8),
            fake_data(0x00b160d8, 0x00b160d8 as *mut u8),
            fake_data(0x00b160d0, 0x00b160d0 as *mut u8),
            fake_data(0x00b5c5bc, 0x00b5c5bc as *mut u8),
            fake_data(0x00b5ca3c, 0x00b5ca3c as *mut u8),
            fake_data(0x00b5ca40, 0x00b5ca40 as *mut u8),
            fake_data(0x00b5c5a0, 0x00b5c5a0 as *mut u8),
            fake_data(0x00b5c5a4, 0x00b5c5a4 as *mut u8),
        ]
    }
}

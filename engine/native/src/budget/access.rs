// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
//! Funds, tax and department queries.

use super::*;

game_data! { static zero_rate: f32 = 0x00A8_1054; }
game_fn! {
    // The helper also receives the original caller-saved register inputs.
    fn multiply_i64(ecx: u32, edx: u32, left: i64, right: i64) -> i64 = "fastcall" 0x009F_0D10;
}
game_fn! {
    fn find_department(this: *mut u8, out: *mut *mut u8, key: *const u32) -> u64 = "thiscall" 0x0049_7520;
}
game_fn! {
    #[allow(dead_code)]
    fn group_changed(this: *mut u8, group: u32) -> u64 = "thiscall" 0x0069_0E80;
}

port! {
    /// Returns all 64 bits of current funds.
    #[port(address = 0x0068_FD10, owner = "budget", name = "cSC4BudgetSimulator::GetTotalFunds")]
    pub unsafe extern "thiscall" fn fc4p_0068fd10(this: *mut u8) -> i64 {
        // SAFETY: the live simulator.
        unsafe { BudgetSim::from_ptr(this) }.funds()
    }
}
port! {
    /// Returns the most recently computed total tax income.
    #[port(address = 0x0068_FD30, owner = "budget", name = "cSC4BudgetSimulator::GetTaxIncome")]
    pub unsafe extern "thiscall" fn fc4p_0068fd30(this: *mut u8) -> i64 {
        // SAFETY: the live simulator.
        unsafe { BudgetSim::from_ptr(this) }.tax_income()
    }
}
port! {
    /// This funding interface always returns a 64-bit zero and removes one argument.
    #[port(address = 0x0068_FD40, owner = "budget", name = "icf::cSC4BudgetSimulator::GetFunding")]
    pub unsafe extern "thiscall" fn fc4p_0068fd40(_this: *mut u8, _purpose: u32) -> u64 { 0 }
}
port! {
    /// Returns the selected rate in ST0, with the shifted index in EAX and incoming EDX retained.
    #[port(address = 0x0068_FD50, owner = "budget", name = "cSC4BudgetSimulator::GetTaxRate", naked)]
    pub unsafe extern "C" fn fc4p_0068fd50() {
        // The Rust body chooses the operand; this entry only marshals the two integer results and ST0.
        core::arch::naked_asm!(
            "push edx", "push [esp + 8]", "call {body}",
            "fld dword ptr [eax]", "mov eax, edx", "pop edx", "ret 4",
            body = sym tax_rate_operand,
        );
    }
}
unsafe extern "thiscall" fn tax_rate_operand(this: *mut u8, index: i32) -> u64 {
    let (operand, eax) = if index < 12 {
        let shifted = index.wrapping_shl(7) as u32;
        (this.wrapping_add(shifted as usize).wrapping_add(0x70), shifted)
    } else {
        (zero_rate().cast(), index as u32)
    };
    pair(operand as u32, eax)
}
port! {
    /// Multiplies the signed 32-bit limit by the 64-bit borrowing base through the game helper.
    #[port(address = 0x0068_FDA0, owner = "budget", name = "cSC4BudgetSimulator::GetCurrentBorrowingLimit")]
    pub unsafe extern "thiscall" fn fc4p_0068fda0(this: *mut u8) -> i64 {
        // SAFETY: the simulator's fields and the game's four-word multiplication helper.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let multiplier = sim.borrowing_multiplier();
            let base = sim.borrowing_base();
            multiply_i64(base as u32, (multiplier >> 31) as u32, i64::from(multiplier), base)
        }
    }
}
port! {
    /// Indices below twelve read their assessed value; all others return zero.
    #[port(address = 0x0068_FDE0, owner = "budget", name = "cSC4BudgetSimulator::GetWeightedAssessedTaxValue")]
    pub unsafe extern "thiscall" fn fc4p_0068fde0(this: *mut u8, index: i32) -> i64 {
        if index >= 12 { return 0; }
        // SAFETY: the caller's selected tax record.
        unsafe { BudgetSim::from_ptr(this).tax_group(index) }.assessed()
    }
}
port! {
    /// Looks up the item's department and then its line, retaining both integer return registers.
    #[port(address = 0x0068_FF80, owner = "budget", name = "cSC4BudgetSimulator::GetLineItemFromBudgetItem")]
    pub unsafe extern "thiscall" fn fc4p_0068ff80(this: *mut u8, item: *mut u8) -> u64 {
        // SAFETY: the simulator, item descriptor, and returned department are live game objects.
        unsafe {
            let department_id: u32 = field(item, 0);
            let result = virtual_regs(this, 0x70, &[department_id], field(this, 0), department_id);
            let department = result as u32 as *mut u8;
            if department.is_null() { return result & 0xFFFF_FFFF_0000_0000; }
            let line: u32 = field(item, 4);
            virtual_regs(department, 0x50, &[line], department as u32, field(department, 0))
        }
    }
}
port! {
    /// Uses the game's map lookup, returning null for its end iterator.
    #[port(address = 0x0069_1000, owner = "budget", name = "cSC4BudgetSimulator::GetDepartmentBudget")]
    pub unsafe extern "thiscall" fn fc4p_00691000(this: *mut u8, key: u32) -> u64 {
        // SAFETY: the map helper fills one iterator word; the caller supplies a live simulator.
        unsafe {
            let mut node = core::mem::MaybeUninit::<*mut u8>::uninit();
            let registers = find_department(this.add(0x54), node.as_mut_ptr(), &key);
            let node = node.assume_init();
            let value = if node == BudgetSim::from_ptr(this).departments() { 0 } else { field(node, 0x14) };
            pair(value, (registers >> 32) as u32)
        }
    }
}
port! {
    /// A nonnull department triggers its group update. A null input leaves both result registers alone.
    /// The group update runs at its original stack depth, including its missing-group pointer result.
    #[port(address = 0x0069_1030, owner = "budget", name = "icf::cSC4BudgetSimulator::FundingPercentageChanged", naked)]
    pub unsafe extern "C" fn fc4p_00691030() {
        // The private buffer carries register values, the target and the continuation chosen by Rust.
        // Remove it before dispatch so the game call keeps the original frame depth.
        core::arch::naked_asm!(
            "push esi", "mov esi, ecx", "sub esp, 16",
            "mov [esp], eax", "mov [esp + 4], edx",
            "mov eax, [esp + 24]", "mov [esp + 8], eax",
            "mov edx, esp", "call {body}", "mov [esp + 12], eax",
            "mov eax, [esp]", "mov edx, [esp + 4]", "mov ecx, esi",
            "add esp, 16", "jmp dword ptr [esp - 4]",
            body = sym prepare_funding_change,
        );
    }
}
unsafe extern "fastcall" fn prepare_funding_change(_this: *mut u8, frame: *mut u32) -> usize {
    // SAFETY: the entry supplies four writable words, including its saved EAX, EDX and department argument.
    unsafe {
        let department = frame.add(2).read() as *mut u8;
        if department.is_null() {
            return finish_funding_change as *const () as usize;
        }
        let group = virtual_regs(department, 0x18, &[], field(department, 0), frame.add(1).read());
        frame.write(group as u32);
        frame.add(1).write((group >> 32) as u32);
        frame.add(2).write(crate::abi::target(0x0069_0E80) as u32);
        dispatch_funding_change as *const () as usize
    }
}

#[unsafe(naked)]
unsafe extern "C" fn finish_funding_change() {
    core::arch::naked_asm!("xor ecx, ecx", "pop esi", "ret 4");
}

#[unsafe(naked)]
unsafe extern "C" fn dispatch_funding_change() {
    // The target is read before the call's return address reuses that word.
    core::arch::naked_asm!("push eax", "call dword ptr [esp - 4]", "pop esi", "ret 4");
}

pub const PORTS: &[Port] = &[
    fc4p_0068fd10::PORT,
    fc4p_0068fd30::PORT,
    fc4p_0068fd40::PORT,
    fc4p_0068fd50::PORT,
    fc4p_0068fda0::PORT,
    fc4p_0068fde0::PORT,
    fc4p_0068ff80::PORT,
    fc4p_00691000::PORT,
    fc4p_00691030::PORT,
];

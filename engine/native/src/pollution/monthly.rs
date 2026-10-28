// SPDX-License-Identifier: GPL-3.0-or-later
//! Monthly deal usage, sanitation expense, counter rollover and facility aging.

use super::*;

game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_interface(pointer: *mut *mut u8) = "thiscall" 0x0041_1870; }
game_fn! { fn clear_deals(list: *mut *mut u8) = "thiscall" 0x0042_1660; }
game_fn! { fn destroy_deals(list: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0042_1760; }
game_fn! { fn age(this: *mut u8, list: *mut u8, used: u32, capacity: u32) = "thiscall" 0x006E_3D00; }
game_fn! { fn reset_used(this: *mut u8, list: *mut u8, used: u32, previous: u32) = "thiscall" 0x006E_3C80; }
game_fn! { fn energy_capacity(this: *mut u8) = "thiscall" 0x006E_3F50; }
game_fn! { fn visuals(this: *mut u8) = "thiscall" 0x006E_63C0; }
game_data! { static allocator: *mut u8 = 0x00B2_0D58; }
game_data! { static history: *mut u8 = 0x00B4_3DC0; }
game_data! { static neighbors: *mut u8 = 0x00B4_3D8C; }
game_data! { static zero: f64 = 0x00A8_0990; }

layout! {
    /// Monthly totals and services in the primary pollution object.
    struct MonthView {
        0x30 => neighbors / set_neighbors: *mut u8,
        0xD8 => department / set_department: *mut u8,
        0x140 => produced / set_produced: i64,
        0x148 => imported / set_imported: i64,
        0x150 => exported / set_exported: i64,
        0x158 => recycled / set_recycled: f64,
        0x160 => incinerated / set_incinerated: i64,
        0x168 => energy / set_energy: i64,
        0x170 => landfilled / set_landfilled: i64,
        0x1B0 => export_capacity / set_export_capacity: u32,
        0x1B4 => abducted / set_abducted: u32,
        0x1C8 => cost / set_cost: i64,
        0x1D0 => cost_per_unit / set_cost_per_unit: f32,
    }
}

/// The allocator is acquired again if a callback cleared its global pointer.
unsafe fn get_allocator() -> *mut u8 {
    // SAFETY: the game owns the allocator global and framework interface.
    unsafe {
        let mut current = *allocator();
        if current.is_null() {
            let mut acquired = core::ptr::null_mut();
            let framework = framework();
            if !framework.is_null() {
                vcall!(framework, 0x14 => fn(u32, u32, *mut *mut u8),
                    0x3AE4_BEA3, 0x3AE4_BEAB, &mut acquired);
            }
            *allocator() = acquired;
            release_interface(&mut acquired);
            current = *allocator();
        }
        current
    }
}

/// Removes the current first node, releases its current occupant and frees the node through the game.
unsafe fn remove_first(list: *mut *mut u8) {
    // SAFETY: the game's populated circular list and twelve-byte nodes.
    unsafe {
        let node = ListNode::from_ptr(ListNode::from_ptr(read_unaligned(list)).next());
        let previous = node.previous();
        let next = node.next();
        ListNode::from_ptr(previous).set_next(next);
        ListNode::from_ptr(next).set_previous(previous);
        let occupant = node.occupant();
        if !occupant.is_null() {
            vcall!(occupant, 0x08 => fn());
        }
        vcall!(get_allocator(), 0x10 => fn(*mut u8), node.as_ptr());
    }
}

fn multiply_float(value: F80, operand: f32) -> F80 {
    let input = value.to_bytes();
    let mut output = [0u8; 10];
    // SAFETY: live operands and a balanced x87 stack.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{input}]", "fmul dword ptr [{operand}]", "fstp tbyte ptr [{output}]",
            input = in(reg) &input, operand = in(reg) &operand, output = in(reg) &mut output,
            out("st(0)") _);
    }
    F80::from_bytes(output)
}

port! {
    /// Resets deal usage, books signed disposal units at the configured rate, snapshots the counters,
    /// records seven histories and clears the month. Ages and rolls facility usage in the original
    /// order, refreshes effects, marks deals dirty and destroys the temporary list. The final list
    /// destructor supplies EDX:EAX, including the upper return bytes retained by message dispatch.
    #[port(address = 0x006E_7B60, owner = "pollution", name = "cSC4PollutionSimulator::DoMonthlySimulation")]
    pub unsafe extern "thiscall" fn fc4p_006e7b60(this: *mut u8) -> u64 {
        // SAFETY: the simulator, deal lists and service interfaces retain their original layouts.
        unsafe {
            let month = MonthView::from_ptr(this);
            let sentinel = vcall!(get_allocator(), 0x0C => fn(u32) -> *mut u8, 12);
            ListNode::from_ptr(sentinel).set_next(sentinel);
            ListNode::from_ptr(sentinel).set_previous(sentinel);
            let mut list = sentinel;
            vcall!(month.neighbors(), 0x78 => fn(u32, *mut *mut u8), 0x4773_FFE5, &mut list);
            while ListNode::from_ptr(list).next() != list {
                let occupant = ListNode::from_ptr(ListNode::from_ptr(list).next()).occupant();
                vcall!(occupant, 0x54 => fn(u32), 0);
                remove_first(&mut list);
            }
            clear_deals(&mut list);
            vcall!(month.neighbors(), 0x78 => fn(u32, *mut *mut u8), 0x4773_FFE4, &mut list);
            let mut export_amount = 0i64;
            while ListNode::from_ptr(list).next() != list {
                let occupant = ListNode::from_ptr(ListNode::from_ptr(list).next()).occupant();
                vcall!(occupant, 0x54 => fn(u32), 0);
                let amount = vcall!(occupant, 0x28 => fn() -> i32);
                export_amount = export_amount.wrapping_add(i64::from(amount));
                remove_first(&mut list);
            }
            let units = month.energy().wrapping_add(month.landfilled())
                .wrapping_add(export_amount).wrapping_add(month.incinerated());
            let cost = multiply_float(F80::from_i64(units), month.cost_per_unit()).ftol();
            let item = vcall!(month.department(), 0x50 => fn(u32) -> *mut u8, 20);
            vcall!(item, 0x30 => fn(u32, u32), cost as u32, (cost as u64 >> 32) as u32);
            let saved_recycled = F80::from_f64(month.recycled()).to_f64();
            write_unaligned(this.add(0x190).cast::<f64>(), saved_recycled);
            let abducted = month.abducted();
            let produced = month.produced();
            write_unaligned(this.add(0x1B8).cast::<u32>(), abducted);
            write_unaligned(this.add(0x180).cast::<i64>(), month.imported());
            write_unaligned(this.add(0x178).cast::<i64>(), produced);
            write_unaligned(this.add(0x188).cast::<i64>(), month.exported());
            write_unaligned(this.add(0x1A0).cast::<i64>(), month.energy());
            write_unaligned(this.add(0x198).cast::<i64>(), month.incinerated());
            write_unaligned(this.add(0x1A8).cast::<i64>(), month.landfilled());
            month.set_cost(cost);
            let history = *history();
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_0158, month.produced() as u32);
            vcall!(history, 0x30 => fn(u32, u32), 0x8AB2_F258, month.imported() as u32);
            vcall!(history, 0x30 => fn(u32, u32), 0x6AB2_F260, month.exported() as u32);
            let recycled = F80::from_f64(month.recycled()).ftol();
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_016F, recycled as u32);
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_0197, month.incinerated() as u32);
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_01A1, month.energy() as u32);
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_01AA, month.landfilled() as u32);
            month.set_recycled(F80::from_f64(*zero()).to_f64());
            month.set_produced(0);
            month.set_imported(0);
            month.set_exported(0);
            month.set_incinerated(0);
            month.set_energy(0);
            month.set_landfilled(0);
            month.set_abducted(0);
            month.set_export_capacity(0);
            age(this, this.add(0x68), 0x28EC_F510, 0xC8EB_D131);
            age(this, this.add(0x6C), 0xE8EC_F641, 0xA8EB_CF0C);
            age(this, this.add(0x74), 0, 0);
            reset_used(this, this.add(0x68), 0x28EC_F510, 0x08FC_DB69);
            reset_used(this, this.add(0x6C), 0xE8EC_F641, 0xA8FC_DBA8);
            energy_capacity(this);
            reset_used(this, this.add(0x70), 0x88EC_F76C, 0x28FC_DBB0);
            visuals(this);
            let result = vcall!(*neighbors(), 0x7C => fn() -> u64);
            destroy_deals(&mut list, (result >> 32) as u32)
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e7b60::PORT];

#[cfg(test)]
mod tests;

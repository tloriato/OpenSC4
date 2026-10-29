// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
//! Monthly income and expense totals in container order.

use super::*;

game_data! { static neighbor_deals: *mut u8 = 0x00B4_3D8C; }

/// Advances the ordered department map's iterator, retaining the header's special parent relation.
unsafe fn next_node(mut node: *mut u8) -> *mut u8 {
    // SAFETY: the caller supplies a live map iterator.
    unsafe {
        let right: *mut u8 = field(node, 0x0C);
        if !right.is_null() {
            node = right;
            loop {
                let left: *mut u8 = field(node, 8);
                if left.is_null() {
                    return node;
                }
                node = left;
            }
        }
        let mut parent: *mut u8 = field(node, 4);
        while node == field::<*mut u8>(parent, 0x0C) {
            node = parent;
            parent = field(parent, 4);
        }
        if field::<*mut u8>(node, 0x0C) != parent {
            node = parent;
        }
        node
    }
}

port! {
    /// Adds every bond's monthly payment in list order with 64-bit wrapping.
    #[port(address = 0x0069_0500, owner = "budget", name = "cSC4BudgetSimulator::GetTotalMonthlyBondPayments")]
    pub unsafe extern "thiscall" fn fc4p_00690500(this: *mut u8) -> i64 {
        // SAFETY: the simulator's circular bond list.
        unsafe {
            let header = BudgetSim::from_ptr(this).bonds();
            let mut node = field(header, 0);
            let mut sum = 0i64;
            while node != header {
                sum = sum.wrapping_add(field(node, 0x30));
                node = field(node, 0);
            }
            sum
        }
    }
}
port! {
    /// Adds ordinance expenses, neighbor deals, departments in map order, then bond payments.
    #[port(address = 0x0069_0990, owner = "budget", name = "cSC4BudgetSimulator::GetTotalMonthlyExpense")]
    pub unsafe extern "thiscall" fn fc4p_00690990(this: *mut u8) -> i64 {
        // SAFETY: the simulator's live services and containers.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let mut sum = vcall!(sim.ordinances(), 0x58 => fn() -> i64);
            sum = sum.wrapping_add(vcall!(*neighbor_deals(), 0x84 => fn() -> i64));
            let mut node = field(sim.departments(), 8);
            while node != sim.departments() {
                sum = sum.wrapping_add(vcall!(field(node, 0x14), 0x3C => fn() -> i64));
                node = next_node(node);
            }
            let header = sim.bonds();
            let mut bond = field(header, 0);
            while bond != header {
                sum = sum.wrapping_add(field(bond, 0x30));
                bond = field(bond, 0);
            }
            sum
        }
    }
}
port! {
    /// Adds twelve virtual tax queries, ordinance income, neighbor deals, then departments in map order.
    #[port(address = 0x0069_0A30, owner = "budget", name = "cSC4BudgetSimulator::GetTotalMonthlyIncome")]
    pub unsafe extern "thiscall" fn fc4p_00690a30(this: *mut u8) -> i64 {
        // SAFETY: the simulator's live services and department map.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let mut sum = 0i64;
            for index in 0..12 {
                sum = sum.wrapping_add(vcall!(this, 0x48 => fn(i32) -> i64, index));
            }
            sum = sum.wrapping_add(vcall!(sim.ordinances(), 0x5C => fn() -> i64));
            sum = sum.wrapping_add(vcall!(*neighbor_deals(), 0x80 => fn() -> i64));
            let mut node = field(sim.departments(), 8);
            while node != sim.departments() {
                sum = sum.wrapping_add(vcall!(field(node, 0x14), 0x40 => fn() -> i64));
                node = next_node(node);
            }
            sum
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00690500::PORT, fc4p_00690990::PORT, fc4p_00690a30::PORT];

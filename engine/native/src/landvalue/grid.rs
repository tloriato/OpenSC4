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

//! `cSC4LandValueSimulator`: the two-grid pass and its callbacks (006bda30 ProcessTwoGridsWithCallback<uint8,int16>, 006bd9f0, 006bdd10).

use super::*;
use core::ptr::read_unaligned;

layout! {
    /// Links shared by ordinary developer-map nodes and the sentinel.
    struct DeveloperNode {
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
    }
}

/// The first word of a vector holds its first element. Each row is a three-word vector.
unsafe fn row_data(vector: *mut u8, x: i32) -> *mut u8 {
    // SAFETY: the caller supplies the game's vector storage and an accessible row.
    unsafe {
        let rows = read_unaligned(vector.cast::<*mut u8>());
        read_unaligned(rows.wrapping_offset(x.wrapping_mul(12) as isize).cast::<*mut u8>())
    }
}

/// A callback receives a byte's address, one full stack word of source data, and the caller's context.
pub type GridCallback = unsafe extern "C" fn(*mut u8, i32, *mut u8);

port! {
    /// Visits matching grid tracts, x before z. Equal shifts pass an unsigned 16-bit stack word;
    /// coarser destinations pass a signed, wrapping sum shifted right twice; finer destinations
    /// receive each signed source value throughout its covered rectangle. Bounds are unchecked.
    /// Grid geometry and storage are queried once, destination first, in slots 11, 12, 13 and 24/25.
    /// Storage and row pointers are read again at each point that can follow a callback.
    /// EAX retains the original loop result, including on empty and negative dimensions.
    #[port(address = 0x006B_DA30, owner = "landvalue", name = "ProcessTwoGridsWithCallback<uint8,int16>")]
    pub unsafe extern "C" fn fc4p_006bda30(
        a: *mut u8, b: *mut u8, callback: GridCallback, context: *mut u8,
    ) -> u32 {
        // SAFETY: the game supplies live grids with these virtual methods and vector storage.
        unsafe {
            let sa = vcall!(a, 0x2C => fn() -> i32);
            let ax = vcall!(a, 0x30 => fn() -> i32);
            let az = vcall!(a, 0x34 => fn() -> i32);
            let av = vcall!(a, 0x60 => fn() -> *mut u8);
            let sb = vcall!(b, 0x2C => fn() -> i32);
            let bx = vcall!(b, 0x30 => fn() -> i32);
            let bz = vcall!(b, 0x34 => fn() -> i32);
            let bv = vcall!(b, 0x64 => fn() -> *mut u8);
            if sa == sb {
                if ax <= 0 {
                    return ax as u32;
                }
                let mut x = 0i32;
                while x < ax {
                    let mut z = 0i32;
                    while z < az {
                        let source = row_data(bv, x).wrapping_offset(z.wrapping_mul(2) as isize);
                        let value = i32::from(read_unaligned(source.cast::<u16>()));
                        let dest = row_data(av, x).wrapping_offset(z as isize);
                        callback(dest, value, context);
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
                0
            } else if sa > sb {
                let d = sa.wrapping_sub(sb) as u32;
                let mut x = 0i32;
                while x < ax {
                    let x0 = x.wrapping_shl(d);
                    let x1 = x.wrapping_add(1).wrapping_shl(d);
                    let mut z = 0i32;
                    while z < az {
                        let z0 = z.wrapping_shl(d);
                        let z1 = z.wrapping_add(1).wrapping_shl(d);
                        let mut sum = 0i32;
                        let mut sx = x0;
                        while sx < x1 {
                            if z0 < z1 {
                                let source = row_data(bv, sx);
                                let mut sz = z0;
                                while sz < z1 {
                                    let p = source.wrapping_offset(sz.wrapping_mul(2) as isize);
                                    sum = sum.wrapping_add(i32::from(read_unaligned(p.cast::<i16>())));
                                    sz = sz.wrapping_add(1);
                                }
                            }
                            sx = sx.wrapping_add(1);
                        }
                        let value = sum.wrapping_shr(d).wrapping_shr(d);
                        callback(row_data(av, x).wrapping_offset(z as isize), value, context);
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
                x as u32
            } else {
                let d = sb.wrapping_sub(sa) as u32;
                let mut x = 0i32;
                while x < bx {
                    let x0 = x.wrapping_shl(d);
                    let x1 = x.wrapping_add(1).wrapping_shl(d);
                    let mut z = 0i32;
                    while z < bz {
                        let source = row_data(bv, x).wrapping_offset(z.wrapping_mul(2) as isize);
                        let value = i32::from(read_unaligned(source.cast::<i16>()));
                        let z0 = z.wrapping_shl(d);
                        let z1 = z.wrapping_add(1).wrapping_shl(d);
                        let mut dx = x0;
                        while dx < x1 {
                            let mut dz = z0;
                            while dz < z1 {
                                callback(row_data(av, dx).wrapping_offset(dz as isize), value, context);
                                dz = dz.wrapping_add(1);
                            }
                            dx = dx.wrapping_add(1);
                        }
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
                bx as u32
            }
        }
    }
}

port! {
    /// Erases the right subtree through the game's helper, deletes the node, then walks its left link.
    /// A null node leaves EAX unchanged; otherwise EAX is the last delete call's result.
    #[port(address = 0x006B_D9F0, owner = "landvalue", name = "cSC4LandValueSimulator::EraseDeveloperSubtree", entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006bd9f0(this: *mut u8, [entry_eax: u32], node: *mut u8) -> u32 {
        let mut result = entry_eax;
        let mut current = node;
        // SAFETY: the game supplies the map and its live nodes; links are read before deleting each node.
        unsafe {
            while !current.is_null() {
                let node = DeveloperNode::from_ptr(current);
                game::land_value_erase_subtree(this, node.right());
                let left = node.left();
                result = game::delete_storage(current);
                current = left;
            }
        }
        result
    }
}

port! {
    /// Restores the simulator's three interface tables, clears and frees its developer map, then frees
    /// altitude histogram storage. Borrowed developer objects are not released. Stored ownership pointers
    /// are left in place; only a nonempty map has its links and count reset before its sentinel is freed.
    #[port(address = 0x006B_DD10, owner = "landvalue", name = "cSC4LandValueSimulator::DestroyOwnedStorage")]
    pub unsafe extern "thiscall" fn fc4p_006bdd10(this: *mut u8) -> u32 {
        // SAFETY: the game's simulator, owned map nodes and histogram.
        unsafe {
            let lv = LandValueSim::from_ptr(this);
            lv.set_primary_vtable(0x00AB_1670);
            lv.set_message_vtable(0x00AB_1660);
            lv.set_serializable_vtable(0x00AB_1648);
            let mut result = lv.developer_count();
            if result != 0 {
                let header = DeveloperNode::from_ptr(lv.developer_header());
                game::land_value_erase_subtree(this.add(0x8C), header.parent());
                let header = DeveloperNode::from_ptr(lv.developer_header());
                header.set_left(header.as_ptr());
                DeveloperNode::from_ptr(lv.developer_header()).set_parent(core::ptr::null_mut());
                let header = DeveloperNode::from_ptr(lv.developer_header());
                header.set_right(header.as_ptr());
                lv.set_developer_count(0);
            }
            let header = lv.developer_header();
            if !header.is_null() {
                result = game::delete_storage(header);
            }
            let histogram = lv.histogram();
            if !histogram.is_null() {
                result = game::delete_storage(histogram);
            }
            result
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006bd9f0::PORT, fc4p_006bda30::PORT, fc4p_006bdd10::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, Object};
    use core::ptr::{null_mut, write_unaligned};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Grid(usize, usize),
        Visit(usize, i32, usize),
        Free(usize),
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static FREE_SNAPSHOTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
        static OBSERVED_SIM: RefCell<Option<LandValueSim>> = const { RefCell::new(None) };
        static FREE_MUTATION: RefCell<Option<(*mut u8, *mut u8, *mut u8)>> = const { RefCell::new(None) };
    }

    fn log(call: Call) {
        CALLS.with(|calls| calls.borrow_mut().push(call));
    }

    fn take_calls() -> Vec<Call> {
        CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
    }

    /// Vectors and their rows use the game's three-word layout, including unused end/capacity words.
    struct Storage<T> {
        values: Vec<Vec<T>>,
        rows: Vec<[usize; 3]>,
        vector: Box<[usize; 3]>,
    }

    impl<T: Copy> Storage<T> {
        fn new(x: usize, z: usize, fill: T) -> Self {
            let mut values = vec![vec![fill; z]; x];
            let rows: Vec<_> = values.iter_mut().map(|row| [row.as_mut_ptr() as usize, 0, 0]).collect();
            let vector = Box::new([rows.as_ptr() as usize, 0, 0]);
            Self { values, rows, vector }
        }

        fn ptr(&mut self) -> *mut u8 {
            self.vector.as_mut_ptr().cast()
        }

        fn cell(&mut self, x: usize, z: usize) -> *mut T {
            self.values[x].as_mut_ptr().wrapping_add(z)
        }
    }

    #[repr(C)]
    struct Grid {
        vtable: *const usize,
        shift: i32,
        x: i32,
        z: i32,
        storage: *mut u8,
    }

    impl Grid {
        fn new(vtable: &[usize; 26], shift: i32, x: i32, z: i32, storage: *mut u8) -> Self {
            Self {
                vtable: vtable.as_ptr(),
                shift,
                x,
                z,
                storage,
            }
        }

        fn ptr(&mut self) -> *mut u8 {
            (self as *mut Self).cast()
        }
    }

    extern "thiscall" fn shift(this: *mut Grid) -> i32 {
        log(Call::Grid(this as usize, 0x2C));
        // SAFETY: a test grid.
        unsafe { (*this).shift }
    }

    extern "thiscall" fn count_x(this: *mut Grid) -> i32 {
        log(Call::Grid(this as usize, 0x30));
        // SAFETY: a test grid.
        unsafe { (*this).x }
    }

    extern "thiscall" fn count_z(this: *mut Grid) -> i32 {
        log(Call::Grid(this as usize, 0x34));
        // SAFETY: a test grid.
        unsafe { (*this).z }
    }

    extern "thiscall" fn bytes(this: *mut Grid) -> *mut u8 {
        log(Call::Grid(this as usize, 0x60));
        // SAFETY: a test grid.
        unsafe { (*this).storage }
    }

    extern "thiscall" fn shorts(this: *mut Grid) -> *mut u8 {
        log(Call::Grid(this as usize, 0x64));
        // SAFETY: a test grid.
        unsafe { (*this).storage }
    }

    fn grid_vtable() -> [usize; 26] {
        let mut table = [0usize; 26];
        for (slot, f) in [
            (0x2C, shift as *const () as usize),
            (0x30, count_x as *const () as usize),
            (0x34, count_z as *const () as usize),
            (0x60, bytes as *const () as usize),
            (0x64, shorts as *const () as usize),
        ] {
            table[slot / 4] = f;
        }
        table
    }

    #[derive(Default)]
    struct Context {
        write: bool,
        first: Option<unsafe fn(*mut u8)>,
        mutation: *mut u8,
    }

    unsafe extern "C" fn callback(dest: *mut u8, value: i32, context: *mut u8) {
        log(Call::Visit(dest as usize, value, context as usize));
        // SAFETY: the test's destination and context; mutations stay in allocated storage.
        unsafe {
            let context = &mut *context.cast::<Context>();
            if context.write {
                *dest = value as u8;
            }
            if let Some(first) = context.first.take() {
                first(context.mutation);
            }
        }
    }

    fn run(a: &mut Grid, b: &mut Grid, context: &mut Context) -> (u32, Vec<(usize, i32)>) {
        take_calls();
        let (ap, bp, cp) = (a.ptr(), b.ptr(), (context as *mut Context).cast());
        // SAFETY: the test's grids, storage and callback context.
        let result = unsafe { fc4p_006bda30(ap, bp, callback, cp) };
        let calls = take_calls();
        assert_eq!(
            calls[..8],
            [
                Call::Grid(ap as usize, 0x2C),
                Call::Grid(ap as usize, 0x30),
                Call::Grid(ap as usize, 0x34),
                Call::Grid(ap as usize, 0x60),
                Call::Grid(bp as usize, 0x2C),
                Call::Grid(bp as usize, 0x30),
                Call::Grid(bp as usize, 0x34),
                Call::Grid(bp as usize, 0x64),
            ]
        );
        let visits = calls[8..]
            .iter()
            .map(|call| {
                let &Call::Visit(dest, value, ctx) = call else {
                    panic!("unexpected call")
                };
                assert_eq!(ctx, cp as usize);
                (dest, value)
            })
            .collect();
        (result, visits)
    }

    #[test]
    fn core_two_grids_floor_means_and_exact_destination_addresses() {
        // The core grid test's 4-by-4 signed input, with the additional 100 from g10_17.
        let table = grid_vtable();
        let mut a = Storage::new(2, 2, 0xA5u8);
        let mut b = Storage::new(4, 4, 0i16);
        for (x, z, value) in [
            (0, 0, 1),
            (1, 0, 2),
            (0, 1, 3),
            (1, 1, -7),
            (2, 0, 1),
            (3, 0, 1),
            (2, 1, 1),
            (3, 1, 2),
            (0, 2, 100),
        ] {
            b.values[x][z] = value;
        }
        let mut ga = Grid::new(&table, 1, 2, 2, a.ptr());
        let mut gb = Grid::new(&table, 0, 4, 4, b.ptr());
        let (result, visits) = run(&mut ga, &mut gb, &mut Context::default());
        assert_eq!(result, 2);
        assert_eq!(
            visits,
            [
                (a.cell(0, 0) as usize, -1),
                (a.cell(0, 1) as usize, 25),
                (a.cell(1, 0) as usize, 1),
                (a.cell(1, 1) as usize, 0)
            ]
        );
        assert_eq!(a.values, vec![vec![0xA5; 2]; 2]);
    }

    #[test]
    fn equal_shifts_pass_zero_extended_words_and_ignore_source_dimensions() {
        let table = grid_vtable();
        let mut a = Storage::new(2, 3, 0xA5u8);
        let mut b = Storage::new(2, 3, 0i16);
        b.values = vec![vec![-1, i16::MIN, i16::MAX], vec![0, 1, -5]];
        for (row, values) in b.rows.iter_mut().zip(&mut b.values) {
            row[0] = values.as_mut_ptr() as usize;
        }
        let mut ga = Grid::new(&table, -2, 2, 3, a.ptr());
        let mut gb = Grid::new(&table, -2, -99, -99, b.ptr());
        let (result, visits) = run(
            &mut ga,
            &mut gb,
            &mut Context {
                write: true,
                ..Context::default()
            },
        );
        assert_eq!(result, 0);
        let want: Vec<_> = [65535, 32768, 32767, 0, 1, 65531]
            .into_iter()
            .enumerate()
            .map(|(i, v)| (a.cell(i / 3, i % 3) as usize, v))
            .collect();
        assert_eq!(visits, want);
        assert_eq!(a.values, [vec![255, 0, 255], vec![0, 1, 251]]);
    }

    #[test]
    fn finer_destination_broadcast_order_and_smaller_source_coverage() {
        // Land-value lv16_17 and lv16_18: size-4 source, size-2 destination.
        let table = grid_vtable();
        let mut a = Storage::new(8, 8, 0xA5u8);
        let mut b = Storage::new(2, 3, -5i16);
        b.values[1][2] = 40;
        let mut ga = Grid::new(&table, 1, 8, 8, a.ptr());
        let mut gb = Grid::new(&table, 2, 2, 3, b.ptr());
        let (result, visits) = run(
            &mut ga,
            &mut gb,
            &mut Context {
                write: true,
                ..Context::default()
            },
        );
        let mut want = Vec::new();
        for x in 0..2 {
            for z in 0..3 {
                for dx in x * 2..x * 2 + 2 {
                    for dz in z * 2..z * 2 + 2 {
                        want.push((a.cell(dx, dz) as usize, if (x, z) == (1, 2) { 40 } else { -5 }));
                    }
                }
            }
        }
        assert_eq!(result, 2);
        assert_eq!(visits, want);
        for x in 0..8 {
            for z in 0..8 {
                let want = if x >= 4 || z >= 6 {
                    0xA5
                } else if x >= 2 && z >= 4 {
                    40
                } else {
                    251
                };
                assert_eq!(a.values[x][z], want);
            }
        }
        // Destination geometry does not clip the raw writes when its backing storage is larger.
        ga.x = 1;
        ga.z = 1;
        assert_eq!(run(&mut ga, &mut gb, &mut Context::default()).1, visits);
    }

    #[test]
    fn empty_and_negative_dimensions_keep_branch_specific_eax() {
        let table = grid_vtable();
        for (sa, sb, ax, az, bx, bz, want) in [
            (2, 2, -257, 1, 1, 1, (-257i32) as u32),
            (2, 2, 0, 1, 1, 1, 0),
            (2, 2, 257, -1, 1, 1, 0),
            (3, 2, -257, 1, 1, 1, 0),
            (3, 2, 257, 0, 1, 1, 257),
            (1, 2, 1, 1, -257, 1, (-257i32) as u32),
            (1, 2, 1, 1, 257, -1, 257),
            (1, 2, 1, 1, 0, 1, 0),
        ] {
            let mut ga = Grid::new(&table, sa, ax, az, null_mut());
            let mut gb = Grid::new(&table, sb, bx, bz, null_mut());
            let (result, visits) = run(&mut ga, &mut gb, &mut Context::default());
            assert_eq!(result, want);
            assert!(visits.is_empty());
        }
    }

    #[test]
    fn shift_counts_are_modulo_32_and_wrapped_blocks_can_be_empty() {
        let table = grid_vtable();
        let mut a = Storage::new(1, 1, 0u8);
        let mut b = Storage::new(1, 1, -123i16);
        for (sa, sb, want) in [(32, 0, -123), (0, 32, -123), (31, 0, 0), (i32::MAX, -1, -123)] {
            let mut ga = Grid::new(&table, sa, 1, 1, a.ptr());
            let mut gb = Grid::new(&table, sb, 1, 1, b.ptr());
            let (_, visits) = run(&mut ga, &mut gb, &mut Context::default());
            assert_eq!(visits, [(a.cell(0, 0) as usize, want)]);
        }
        let mut ga = Grid::new(&table, 0, 1, 1, a.ptr());
        let mut gb = Grid::new(&table, 31, 1, 1, b.ptr());
        assert!(run(&mut ga, &mut gb, &mut Context::default()).1.is_empty());
    }

    #[test]
    fn averaging_sum_wraps_before_the_two_arithmetic_shifts() {
        let table = grid_vtable();
        let mut a = Storage::new(1, 1, 0u8);
        let mut b = Storage::new(512, 512, i16::MAX);
        let mut ga = Grid::new(&table, 9, 1, 1, a.ptr());
        let mut gb = Grid::new(&table, 0, 0, 0, b.ptr());
        let (_, visits) = run(&mut ga, &mut gb, &mut Context::default());
        assert_eq!(visits, [(a.cell(0, 0) as usize, -1)]);
    }

    unsafe fn replace_word(p: *mut u8) {
        // SAFETY: the test's source word.
        unsafe { write_unaligned(p.cast::<i16>(), -123) };
    }

    #[test]
    fn equal_path_reads_source_after_each_callback_but_broadcast_keeps_each_value() {
        let table = grid_vtable();
        for (sa, sb, az, bz, want) in [(1, 1, 2, 2, vec![7, 65413]), (0, 1, 2, 1, vec![7, 7, 7, 7])] {
            let mut a = Storage::new(2, 2, 0u8);
            let mut b = Storage::new(1, 2, 7i16);
            let mut ga = Grid::new(&table, sa, 1, az, a.ptr());
            let mut gb = Grid::new(&table, sb, 1, bz, b.ptr());
            let mut context = Context {
                first: Some(replace_word),
                mutation: b.cell(0, if sa == sb { 1 } else { 0 }).cast(),
                ..Context::default()
            };
            let visits = run(&mut ga, &mut gb, &mut context).1;
            assert_eq!(visits.iter().map(|v| v.1).collect::<Vec<_>>(), want);
        }
    }

    unsafe fn replace_vector(p: *mut u8) {
        // SAFETY: two test pointer-word pairs: destination address, replacement value.
        unsafe {
            let words = p.cast::<usize>();
            for i in [0, 2] {
                write_unaligned(*words.add(i) as *mut usize, *words.add(i + 1));
            }
        }
    }

    #[test]
    fn callback_can_replace_both_outer_row_tables() {
        let table = grid_vtable();
        for (sa, sb) in [(0, 0), (1, 0), (0, 1)] {
            let mut a = Storage::new(4, 4, 0u8);
            let mut b = Storage::new(4, 4, 7i16);
            let mut next_a = Storage::new(4, 4, 0u8);
            let next_b = Storage::new(4, 4, -3i16);
            let mut mutation = [
                a.ptr() as usize,
                next_a.rows.as_ptr() as usize,
                b.ptr() as usize,
                next_b.rows.as_ptr() as usize,
            ];
            let mut ga = Grid::new(&table, sa, 1, 2, a.ptr());
            let mut gb = Grid::new(&table, sb, 1, 2, b.ptr());
            let mut context = Context {
                first: Some(replace_vector),
                mutation: mutation.as_mut_ptr().cast(),
                ..Context::default()
            };
            let visits = run(&mut ga, &mut gb, &mut context).1;
            assert_eq!(visits[0], (a.cell(0, 0) as usize, 7));
            if sa == sb {
                assert_eq!(visits[1], (next_a.cell(0, 1) as usize, 65533));
            } else if sa > sb {
                assert_eq!(visits[1], (next_a.cell(0, 1) as usize, -3));
            } else {
                assert_eq!(visits[1], (next_a.cell(0, 1) as usize, 7));
                assert_eq!(visits[4], (next_a.cell(0, 2) as usize, -3));
            }
        }
    }

    extern "C" fn delete(storage: *mut u8) -> u32 {
        log(Call::Free(storage as usize));
        OBSERVED_SIM.with(|sim| {
            if let Some(lv) = *sim.borrow() {
                // SAFETY: the observed simulator remains live through this call.
                let bytes = unsafe { std::slice::from_raw_parts(lv.as_ptr(), 0xA4) }.to_vec();
                FREE_SNAPSHOTS.with(|snapshots| snapshots.borrow_mut().push(bytes));
            }
        });
        FREE_MUTATION.with(|mutation| {
            if let Some((when, node, left)) = *mutation.borrow() {
                if when == storage {
                    // SAFETY: live test node, changed while its right subtree is being deleted.
                    unsafe { DeveloperNode::from_ptr(node) }.set_left(left);
                }
            }
        });
        storage as u32 ^ 0xAABB_CCDD
    }

    fn node(left: *mut u8, right: *mut u8) -> Object {
        let mut memory = Object::new(0x18);
        // SAFETY: 0x18 bytes cover all links, with payload words kept as canaries.
        let n = unsafe { DeveloperNode::from_ptr(memory.ptr()) };
        n.set_left(left);
        n.set_right(right);
        memory
    }

    #[test]
    fn subtree_deletion_visits_right_node_left_and_keeps_null_entry_eax() {
        let _delete = fake(0x0090_CF63, delete as *const () as usize);
        let mut map = Object::new(8);
        let mut left = node(null_mut(), null_mut());
        let mut right = node(null_mut(), null_mut());
        let mut root = node(left.ptr(), right.ptr());
        take_calls();
        // SAFETY: the test's map and live tree.
        unsafe {
            assert_eq!(
                call_thiscall_with_eax(fc4p_006bd9f0 as *const () as usize, map.ptr(), &[0], 0x1234_ABCD).0,
                0x1234_ABCD
            );
            assert_eq!(
                call_thiscall_with_eax(fc4p_006bd9f0 as *const () as usize, map.ptr(), &[root.ptr() as u32], 0).0,
                left.ptr() as u32 ^ 0xAABB_CCDD
            );
        }
        assert_eq!(
            take_calls(),
            [
                Call::Free(right.ptr() as usize),
                Call::Free(root.ptr() as usize),
                Call::Free(left.ptr() as usize)
            ]
        );
    }

    #[test]
    fn left_link_is_loaded_after_the_right_subtree_returns() {
        let _delete = fake(0x0090_CF63, delete as *const () as usize);
        let mut map = Object::new(8);
        let mut old_left = node(null_mut(), null_mut());
        let mut new_left = node(null_mut(), null_mut());
        let mut right = node(null_mut(), null_mut());
        let mut root = node(old_left.ptr(), right.ptr());
        FREE_MUTATION.with(|m| *m.borrow_mut() = Some((right.ptr(), root.ptr(), new_left.ptr())));
        take_calls();
        // SAFETY: the test's map and live tree.
        unsafe { game::land_value_erase_subtree(map.ptr(), root.ptr()) };
        FREE_MUTATION.with(|m| *m.borrow_mut() = None);
        assert_eq!(
            take_calls(),
            [
                Call::Free(right.ptr() as usize),
                Call::Free(root.ptr() as usize),
                Call::Free(new_left.ptr() as usize)
            ]
        );
    }

    #[test]
    fn destruction_resets_only_nonempty_map_then_deletes_sentinel_and_histogram() {
        let _delete = fake(0x0090_CF63, delete as *const () as usize);
        for populated in [false, true] {
            let mut memory = Object::new(0xA4);
            // SAFETY: full simulator layout, initialized with byte canaries.
            unsafe { core::ptr::write_bytes(memory.ptr(), 0xA5, 0xA4) };
            let lv = unsafe { LandValueSim::from_ptr(memory.ptr()) };
            let mut child = node(null_mut(), null_mut());
            let mut header = node(child.ptr(), child.ptr());
            let h = unsafe { DeveloperNode::from_ptr(header.ptr()) };
            h.set_parent(child.ptr());
            let mut histogram = Object::new(16);
            lv.set_developer_header(header.ptr());
            lv.set_developer_count(u32::from(populated));
            lv.set_histogram(histogram.ptr());
            let mut expected = unsafe { std::slice::from_raw_parts(memory.ptr(), 0xA4) }.to_vec();
            for (off, value) in [(0, 0x00AB_1670u32), (4, 0x00AB_1660), (8, 0x00AB_1648)] {
                expected[off..off + 4].copy_from_slice(&value.to_le_bytes());
            }
            let expected_during_node = expected.clone();
            if populated {
                expected[0x90..0x94].fill(0);
            }
            OBSERVED_SIM.with(|s| *s.borrow_mut() = Some(lv));
            FREE_SNAPSHOTS.with(|s| s.borrow_mut().clear());
            take_calls();
            // SAFETY: simulator with owned test map and histogram.
            let result = unsafe { fc4p_006bdd10(memory.ptr()) };
            OBSERVED_SIM.with(|s| *s.borrow_mut() = None);
            let mut want_calls = Vec::new();
            if populated {
                want_calls.push(Call::Free(child.ptr() as usize));
            }
            want_calls.extend([Call::Free(header.ptr() as usize), Call::Free(histogram.ptr() as usize)]);
            assert_eq!(take_calls(), want_calls);
            assert_eq!(result, histogram.ptr() as u32 ^ 0xAABB_CCDD);
            assert_eq!(unsafe { std::slice::from_raw_parts(memory.ptr(), 0xA4) }, expected);
            FREE_SNAPSHOTS.with(|s| {
                let snapshots = s.borrow();
                if populated {
                    assert_eq!(snapshots[0], expected_during_node);
                }
                assert_eq!(snapshots[snapshots.len() - 2], expected);
                assert_eq!(snapshots[snapshots.len() - 1], expected);
            });
            if populated {
                assert_eq!(
                    (h.parent(), h.left(), h.right()),
                    (null_mut(), header.ptr(), header.ptr())
                );
            } else {
                assert_eq!(
                    (h.parent(), h.left(), h.right()),
                    (child.ptr(), child.ptr(), child.ptr())
                );
            }
        }
    }

    #[test]
    fn destruction_handles_null_storage_independently_and_does_not_clear_ownership_pointers() {
        let _delete = fake(0x0090_CF63, delete as *const () as usize);
        for (has_header, has_histogram) in [(false, false), (true, false), (false, true)] {
            let mut memory = Object::new(0xA4);
            let lv = unsafe { LandValueSim::from_ptr(memory.ptr()) };
            let mut header = node(null_mut(), null_mut());
            let mut histogram = Object::new(16);
            let hp = if has_header { header.ptr() } else { null_mut() };
            let ap = if has_histogram { histogram.ptr() } else { null_mut() };
            lv.set_developer_header(hp);
            lv.set_histogram(ap);
            take_calls();
            let result = unsafe { fc4p_006bdd10(memory.ptr()) };
            let want: Vec<_> = [hp, ap]
                .into_iter()
                .filter(|p| !p.is_null())
                .map(|p| Call::Free(p as usize))
                .collect();
            assert_eq!(take_calls(), want);
            assert_eq!((lv.developer_header(), lv.histogram()), (hp, ap));
            let last = if has_histogram { ap } else { hp };
            assert_eq!(result, if last.is_null() { 0 } else { last as u32 ^ 0xAABB_CCDD });
        }
    }
}

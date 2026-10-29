// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration and building selection for a growth candidate.

use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_fn! { fn capacity(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_55B0; }
game_fn! { fn configuration_compatible(this: u32, configuration: u32, candidate: u32, corner: u32) -> u32 = "thiscall" 0x0070_6500; }
game_fn! { fn building_compatible(this: u32, building: u32, candidate: u32) -> u32 = "thiscall" 0x0070_4E00; }
game_fn! { fn random_index(this: u32, bound: u32) -> u32 = "thiscall" 0x0091_36B1; }
game_fn! { fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn insert_configuration(this: u32, output: u32, value: u32) -> u32 = "thiscall" 0x0049_A050; }
game_fn! { fn erase_configuration_branch(this: u32, node: u32) -> u32 = "thiscall" 0x0070_49D0; }
game_fn! { fn reserve_buckets(this: u32, count: u32) -> u32 = "thiscall" 0x0045_D4F0; }
game_fn! { fn fill_buckets(this: u32, position: u32, count: u32, value: u32) -> u32 = "thiscall" 0x004F_BA20; }
game_fn! { fn resize_set(this: u32, count: u32) -> u32 = "thiscall" 0x004F_0940; }
game_fn! { fn insert_building(this: u32, output: u32, value: u32) -> u32 = "thiscall" 0x0062_4DD0; }
game_fn! { fn clear_set(this: u32) -> u32 = "thiscall" 0x008E_BA60; }
game_fn! { fn copy_set(this: u32, source: u32) -> u32 = "thiscall" 0x0050_DA70; }
game_fn! { fn occupant_group(key: u32, group: u32) -> u32 = "C" 0x005E_8F70; }
game_data! { static city: u32 = 0x00B4_3D68; }
game_data! { static random: u8 = 0x00B4_AFD8; }
game_data! { static percent_scale: f32 = 0x00A8_0AA0; }
game_data! { static integer_bias: f32 = 0x00AB_3048; }
game_data! { static bucket_primes: [u32; 28] = 0x00A8_29B0; }

/// Advance an ordered node without calling an iterator helper.
unsafe fn next_node(mut node: u32) -> u32 {
    unsafe {
        let mut link = word(node, 12);
        if link != 0 {
            node = link;
            while word(node, 8) != 0 {
                node = word(node, 8);
            }
        } else {
            link = word(node, 4);
            while node == word(link, 12) {
                node = link;
                link = word(link, 4);
            }
            if word(node, 12) != link {
                node = link;
            }
        }
        node
    }
}

/// The hash table has one unused leading word, a vector of buckets, and an element count.
#[repr(C)]
struct BuildingSet([MaybeUninit<u32>; 5]);

impl BuildingSet {
    unsafe fn new() -> Self {
        let mut set = Self([MaybeUninit::uninit(); 5]);
        for i in 1..5 {
            set.0[i].write(0);
        }
        unsafe {
            let mut first = 0usize;
            let mut remaining = 28usize;
            while remaining != 0 {
                let half = remaining / 2;
                if read_unaligned(bucket_primes().cast::<u32>().add(first + half)) < 100 {
                    first += half + 1;
                    remaining -= half + 1;
                } else {
                    remaining = half;
                }
            }
            let count = if first == 28 {
                0xFFFF_FFFB
            } else {
                read_unaligned(bucket_primes().cast::<u32>().add(first))
            };
            let p = set.ptr();
            reserve_buckets(p + 4, count);
            let zero = 0u32;
            fill_buckets(p + 4, word(p, 8), count, &zero as *const u32 as u32);
            store(p, 16, 0);
        }
        set
    }

    fn ptr(&mut self) -> u32 {
        self.0.as_mut_ptr() as u32
    }

    unsafe fn count(&mut self) -> u32 {
        unsafe { word(self.ptr(), 16) }
    }

    unsafe fn first(&mut self) -> u32 {
        unsafe {
            let p = self.ptr();
            let begin = word(p, 4);
            let n = (word(p, 8).wrapping_sub(begin) as i32 >> 2) as u32;
            for i in 0..n {
                let node = word(begin, i.wrapping_mul(4));
                if node != 0 {
                    return node;
                }
            }
            0
        }
    }

    unsafe fn next(&mut self, node: u32) -> u32 {
        unsafe {
            let next = word(node, 0);
            if next != 0 {
                return next;
            }
            let p = self.ptr();
            let begin = word(p, 4);
            let n = (word(p, 8).wrapping_sub(begin) as i32 >> 2) as u32;
            let mut bucket: u32;
            let quotient = word(node, 4);
            core::arch::asm!("xor edx, edx", "div {divisor}",
                divisor = in(reg) n, inout("eax") quotient => _, out("edx") bucket,
                options(nomem, nostack));
            loop {
                bucket = bucket.wrapping_add(1);
                if bucket >= n {
                    return 0;
                }
                let next = word(begin, bucket.wrapping_mul(4));
                if next != 0 {
                    return next;
                }
            }
        }
    }

    unsafe fn insert(&mut self, value: &u32, output: u32) {
        unsafe {
            let p = self.ptr();
            resize_set(p, word(p, 16).wrapping_add(1));
            insert_building(p, output, value as *const u32 as u32);
        }
    }

    unsafe fn dispose(&mut self) {
        unsafe {
            let p = self.ptr();
            clear_set(p);
            let buckets = word(p, 4);
            if buckets != 0 {
                free(buckets);
            }
        }
    }
}

unsafe fn dispose_configurations(tree: &mut [u32; 2]) {
    unsafe {
        let p = tree.as_mut_ptr() as u32;
        let header = tree[0];
        if tree[1] != 0 {
            let mut node = word(header, 4);
            while node != 0 {
                erase_configuration_branch(p, word(node, 12));
                let left = word(node, 8);
                free(node);
                node = left;
            }
            store(header, 8, header);
            store(header, 4, 0);
            store(header, 12, header);
            tree[1] = 0;
        }
        if header != 0 {
            free(header);
        }
    }
}

unsafe fn wrong_style(td: TractDeveloper) {
    unsafe {
        let row = word(td.debug_context(), 0x78);
        store(row, 0x24, word(row, 0x24).wrapping_add(1));
    }
}

port! {
    /// Filter configurations by anchor/style and compatibility, then choose one uniformly in tree
    /// order. Collect its compatible buildings in game hash sets, falling back to monopoly buildings
    /// only in mode one. Choose in bucket order and release all temporary game allocations.
    #[port(address = 0x0070_9BA0, owner = "tractdeveloper", name = "cSC4TractDeveloper::PickBuilding")]
    pub unsafe extern "thiscall" fn fc4p_00709ba0(this: *mut u8, candidate: u32) -> u32 {
        unsafe {
            let td = TractDeveloper::from_ptr(this);
            capacity(this as u32, candidate);
            let purpose = vcall!(td.developer() as *mut u8, 0x1C => fn() -> u32);
            let anchors = vcall!(read_unaligned(city()) as *mut u8, 0x90 => fn() -> *mut u8);
            let mut group = 0u32;
            if vcall!(anchors, 0x90 => fn(u32, u32) -> u32, candidate.wrapping_add(0x38), purpose) as u8 != 0 {
                group = 0x3000;
            } else {
                let percentages = vcall!(td.developer() as *mut u8, 0x68 => fn() -> u32);
                if word(percentages, 4) != 0 {
                    let mut draw = random_index(random() as u32, 100);
                    let header = word(percentages, 0);
                    let mut node = word(header, 8);
                    while node != header {
                        let weight = ((F80::from_f32(read_unaligned(node.wrapping_add(0x14) as *const f32))
                            * F80::from_f32(read_unaligned(percent_scale())))
                            + F80::from_f32(read_unaligned(integer_bias()))).to_f32().to_bits()
                            .wrapping_add(0xB4C0_0000);
                        if draw < weight { group = word(node, 0x10); break; }
                        draw = draw.wrapping_sub(weight);
                        node = next_node(node);
                    }
                }
            }
            let mut configurations = [0u32; 2];
            let header = allocate(20);
            configurations[0] = header;
            write_unaligned(header as *mut u8, 0);
            store(header, 4, 0); store(header, 8, header); store(header, 12, header);
            let mut output = MaybeUninit::<[u32; 3]>::uninit();
            let mut node = word(td.configurations_header(), 8);
            while node != td.configurations_header() {
                let configuration = word(node, 16);
                if group != 0 && vcall!(configuration as *mut u8, 0x48 => fn(u32) -> u32, group) as u8 == 0 {
                    wrong_style(td);
                } else {
                    let corner = u32::from(read_unaligned(candidate.wrapping_add(0xA0) as *const u8));
                    if configuration_compatible(this as u32, configuration, candidate, corner) == 27 {
                        insert_configuration(configurations.as_mut_ptr() as u32, output.as_mut_ptr() as u32,
                            &configuration as *const u32 as u32);
                    }
                }
                node = next_node(node);
            }
            if configurations[1] == 0 { dispose_configurations(&mut configurations); return 0; }
            let mut draw = random_index(random() as u32, configurations[1]);
            td.set_chosen_configuration(0);
            node = word(configurations[0], 8);
            while node != configurations[0] {
                if draw == 0 { td.set_chosen_configuration(word(node, 16)); break; }
                draw = draw.wrapping_sub(1);
                node = next_node(node);
            }
            let occupied = word(candidate, 0x24) as i32;
            let mut minimum = occupied.wrapping_add(1).max(occupied);
            if word(candidate, 0) == 4 {
                let pieces = u32::from(read_unaligned(candidate.wrapping_add(0x6C) as *const u8));
                core::arch::asm!("cdq", "idiv {pieces}", pieces = in(reg) pieces,
                    inout("eax") minimum, out("edx") _, options(nomem, nostack));
            }
            let maximum = td.desired_maximum().wrapping_sub(td.built()).wrapping_add(occupied)
                .max(minimum.wrapping_add(1));
            td.set_maximum_building_capacity(maximum);
            td.set_minimum_building_capacity(minimum);
            let primary = vcall!(td.chosen_configuration() as *mut u8, 0x60 => fn() -> u32);
            let mut compatible = BuildingSet::new();
            let mut monopoly = BuildingSet::new();
            let mut last = 0;
            let mut position = word(primary, 0x28);
            while position != word(primary, 0x2C) {
                let family = word(position, 0);
                let mut family_set = BuildingSet::new();
                vcall!(td.building_development() as *mut u8, 0x4C => fn(u32, u32) -> u32,
                    family_set.ptr(), family);
                let mut building_node = family_set.first();
                while building_node != 0 {
                    let building = word(building_node, 4);
                    let mut matches = true;
                    if group != 0 {
                        let mut key = [0u32; 3];
                        vcall!(td.building_development() as *mut u8, 0x38 => fn(u32, u32) -> u32,
                            building, key.as_mut_ptr() as u32);
                        if occupant_group(key.as_mut_ptr() as u32, group) as u8 == 0 {
                            wrong_style(td);
                            matches = false;
                        }
                    }
                    if matches {
                        last = building_compatible(this as u32, building, candidate);
                        if last == 27 { compatible.insert(&building, output.as_mut_ptr() as u32); }
                        else if last == 2 && vcall!(td.developer() as *mut u8, 0x64 => fn() -> u32) == 1 {
                            monopoly.insert(&building, output.as_mut_ptr() as u32);
                        }
                    }
                    building_node = family_set.next(building_node);
                }
                family_set.dispose();
                position = position.wrapping_add(4);
            }
            if compatible.count() == 0 {
                if monopoly.count() == 0 {
                    monopoly.dispose(); compatible.dispose(); dispose_configurations(&mut configurations);
                    return last;
                }
                clear_set(compatible.ptr());
                copy_set(compatible.ptr(), monopoly.ptr());
            }
            let mut draw = random_index(random() as u32, compatible.count());
            td.set_chosen_building(0);
            let mut building_node = compatible.first();
            while building_node != 0 {
                if draw == 0 { td.set_chosen_building(word(building_node, 4)); break; }
                draw = draw.wrapping_sub(1);
                building_node = compatible.next(building_node);
            }
            monopoly.dispose(); compatible.dispose(); dispose_configurations(&mut configurations);
            27
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00709ba0::PORT];

#[cfg(test)]
mod tests;

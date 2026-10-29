// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Directional subnetwork labels and the two seed passes.

use super::*;

layout! {
    /// Fields used by the directional flood fill.
    pub struct SubnetworkSim {
        0x58 => intersections / set_intersections: *mut u8,
        0x60 => next_label / set_next_label: u16,
        0x7C => rows / set_rows: *mut u8,
        0x18C => min_x / set_min_x: i32,
        0x190 => min_z / set_min_z: i32,
        0x194 => max_x / set_max_x: i32,
        0x198 => max_z / set_max_z: i32,
        0x704 => lots / set_lots: *mut u8,
    }
}
layout! {
    /// One linked network record, with four independent 16-bit labels.
    struct Info {
        0x04 => connections / set_connections: u8,
        0x06 => kind / set_kind: u8,
        0x0C => labels / set_labels: [u16; 4],
        0x14 => next / set_next: *mut u8,
    }
}
layout! {
    /// The finish iterator of the game's ten-word deque.
    struct WorkDeque {
        0x00 => begin / set_begin: *mut u32,
        0x10 => end / set_end: *mut u32,
        0x14 => first / set_first: *mut u32,
        0x18 => last / set_last: *mut u32,
        0x1C => node / set_node: *mut *mut u32,
    }
}

game_fn! { fn clear_labels(this: *mut u8) = "thiscall" 0x0070_FCE0; }
game_fn! { fn fill_labels(this: *mut u8, x: i32, z: i32, kind: u32, entry: u32) = "thiscall" 0x0071_7EC0; }
game_fn! { fn deque_new(this: *mut u8, size: u32) = "thiscall" 0x0074_5980; }
game_fn! { fn deque_drop(this: *mut u8) = "thiscall" 0x0071_3E90; }
game_fn! { fn deque_push(this: *mut u8, value: *const u32) = "thiscall" 0x0062_66E0; }
game_fn! { fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn find_info(this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 = "thiscall" 0x0070_FB30; }
game_fn! { fn find_tunnel(this: *mut u8, result: *mut u32, key: *const u32) = "thiscall" 0x0060_D170; }
game_data! { static travel: u32 = 0x00AB_3244; }
game_data! { static dx: i32 = 0x00AB_325C; }
game_data! { static dz: i32 = 0x00AB_326C; }
game_data! { static opposite: u32 = 0x00AB_3394; }

unsafe fn first_info(sim: SubnetworkSim, x: i32, z: i32, kind: u32) -> *mut u8 {
    unsafe {
        let row = word(sim.rows(), x.wrapping_mul(12) as usize) as *mut u8;
        let mut info = word(row, z.wrapping_mul(4) as usize) as *mut u8;
        while !info.is_null() {
            let view = Info::from_ptr(info);
            if u32::from(view.kind()) == kind {
                break;
            }
            info = view.next();
        }
        info
    }
}

unsafe fn label(info: *mut u8, entry: u32) -> *mut u16 {
    info.wrapping_add(12).cast::<u16>().wrapping_add(entry as usize)
}

fn inside(sim: SubnetworkSim, x: i32, z: i32) -> bool {
    x >= sim.min_x() && z >= sim.min_z() && x <= sim.max_x() && z <= sim.max_z()
}

unsafe fn intersection(sim: SubnetworkSim, x: i32, z: i32) -> *mut u8 {
    sim.intersections()
        .wrapping_offset((x.wrapping_shl(8) | z).wrapping_mul(46) as isize)
}

super::network::register_port!(
    0x0071_83B0,
    fc4p_007183b0,
    "cSC4TrafficSimulator::UpdateSubnetworks",
    update,
    0,
    []
);

unsafe extern "fastcall" fn update(this: *mut u8, result: *mut Registers) {
    // SAFETY: the simulator owns the row descriptors, records and helper arguments.
    unsafe {
        let sim = SubnetworkSim::from_ptr(this);
        clear_labels(this);
        let mut x = 0i32;
        while x < sim.max_x() {
            let mut z = 0i32;
            while z < sim.max_z() {
                let info = first_info(sim, x, z, 1);
                if !info.is_null() {
                    if label(info, 0).read_unaligned() == 0 {
                        fill_labels(this, x, z, 1, 0);
                    }
                    if label(info, 1).read_unaligned() == 0 {
                        fill_labels(this, x, z, 1, 1);
                    }
                }
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        x = sim.max_x().wrapping_sub(1);
        while x > -1 {
            let mut z = sim.max_z().wrapping_sub(1);
            while z > -1 {
                let info = first_info(sim, x, z, 1);
                if !info.is_null() {
                    if label(info, 2).read_unaligned() == 0 {
                        fill_labels(this, x, z, 1, 2);
                    }
                    if label(info, 3).read_unaligned() == 0 {
                        fill_labels(this, x, z, 1, 3);
                    }
                }
                z = z.wrapping_sub(1);
            }
            x = x.wrapping_sub(1);
        }
        // The caller uses ECX after the call; the saved entry word restores it.
        result.write(Registers {
            eax: 0,
            ecx: this as u32,
            edx: 0,
        });
    }
}

port! {
    /// Labels reachable entry sides in depth-first order. Work items retain their next exit, including
    /// the resume point after a tunnel. Only ordinary connected exits advance the label counter.
    #[port(address = 0x0071_7EC0, owner = "traffic", name = "FillDirectionalSubnetwork")]
    pub unsafe extern "thiscall" fn fc4p_00717ec0(this: *mut u8, x: i32, z: i32, kind: u32, entry: u32) {
        // SAFETY: all storage belongs to the supplied simulator or the game's temporary deque.
        unsafe {
            let sim = SubnetworkSim::from_ptr(this);
            let mut storage = [0u32; 10];
            let deque = WorkDeque::from_ptr(storage.as_mut_ptr().cast());
            deque_new(deque.as_ptr(), 0);
            let travel = travel().wrapping_add(kind as usize).read();
            let mut info = first_info(sim, x, z, kind);
            if info.is_null() || label(info, entry).read_unaligned() != 0
                || Info::from_ptr(info).connections() > 2
                || intersection(sim, x, z).add(44).read() & 2 != 0 {
                deque_drop(deque.as_ptr());
                return;
            }
            let lots = vcall!(sim.lots(), 0x60 => fn() -> *mut u8);
            let mut state = [x as u8, z as u8, entry as u8, 0u8];
            let mut labelled = false;
            'states: loop {
                let cx = i32::from(state[0]);
                let cz = i32::from(state[1]);
                let record = intersection(sim, cx, cz);
                let mut next = None;
                while state[3] < 4 {
                    let k = u32::from(state[3]);
                    let e = u32::from(state[2]);
                    let bits = record.cast::<u16>().wrapping_add(travel as usize).read_unaligned();
                    if u32::from(bits) & 1u32.wrapping_shl(k.wrapping_add(e.wrapping_mul(4))) != 0 {
                        label(info, e).write_unaligned(sim.next_label());
                        labelled = true;
                        let sx = dx().wrapping_add(k as usize).read();
                        let sz = dz().wrapping_add(k as usize).read();
                        let mut nx = cx.wrapping_add(sx);
                        let mut nz = cz.wrapping_add(sz);
                        if !inside(sim, nx, nz) {
                            let bits = record.cast::<u16>().wrapping_add(travel as usize).read_unaligned();
                            if u32::from(bits) & 1u32.wrapping_shl(e.wrapping_add(k.wrapping_mul(4))) != 0
                                && label(info, k).read_unaligned() == 0 {
                                next = Some((info, cx, cz, k));
                            }
                        } else {
                            let back = opposite().wrapping_add(k as usize).read();
                            let mut found = find_info(this, kind, nx, nz);
                            if found.is_null() {
                                let rows = word(lots, 0) as *mut u8;
                                let mut row = rows.wrapping_offset(nx.wrapping_mul(12) as isize);
                                let id = word(word(row, 0) as *mut u8, nz.wrapping_mul(4) as usize);
                                if id != 0 {
                                    loop {
                                        nx = nx.wrapping_add(sx);
                                        nz = nz.wrapping_add(sz);
                                        row = row.wrapping_offset(sx.wrapping_mul(12) as isize);
                                        if !inside(sim, nx, nz) { break; }
                                        if word(word(row, 0) as *mut u8, nz.wrapping_mul(4) as usize) != id {
                                            found = find_info(this, kind, nx, nz);
                                            break;
                                        }
                                    }
                                }
                            }
                            if !found.is_null() && label(found, back).read_unaligned() == 0 {
                                next = Some((found, nx, nz, back));
                            }
                        }
                    }
                    state[3] = state[3].wrapping_add(1);
                    if next.is_some() { break; }
                }
                if next.is_none() && state[3] < 5 && record.add(44).read() & 2 != 0 {
                    state[3] = state[3].wrapping_add(1);
                    let key = (u32::from(state[0]) << 8) | u32::from(state[1]);
                    let mut tunnel = [key, 0];
                    find_tunnel(this.add(0xC8), tunnel.as_mut_ptr(), &key);
                    let tunnel = tunnel[1] as *mut u8;
                    if !tunnel.is_null()
                        && u32::from(tunnel.add(10).cast::<u16>().read_unaligned()) & 1u32.wrapping_shl(travel) != 0 {
                        let e = u32::from(state[2]);
                        label(info, e).write_unaligned(sim.next_label());
                        let nx = i32::from(tunnel.add(8).read());
                        let nz = i32::from(tunnel.add(9).read());
                        let found = find_info(this, kind, nx, nz);
                        if !found.is_null() && label(found, e).read_unaligned() == 0 {
                            next = Some((found, nx, nz, e));
                        }
                    }
                }
                if let Some((found, nx, nz, e)) = next {
                    let packed = u32::from_le_bytes(state);
                    let end = deque.end();
                    if end != deque.last().wrapping_sub(1) {
                        if !end.is_null() { end.write(packed); }
                        deque.set_end(end.wrapping_add(1));
                    } else {
                        deque_push(deque.as_ptr(), &packed);
                    }
                    state = [nx as u8, nz as u8, e as u8, 0];
                    info = found;
                    continue 'states;
                }
                let end = deque.end();
                if end == deque.begin() {
                    if labelled { sim.set_next_label(sim.next_label().wrapping_add(1)); }
                    break;
                }
                let first = deque.first();
                let previous = if end == first {
                    let value = deque.node().wrapping_sub(1).read().wrapping_add(31).read();
                    if !first.is_null() { free(first.cast()); }
                    let previous_node = deque.node().wrapping_sub(1);
                    let previous_first = previous_node.read();
                    deque.set_node(previous_node);
                    deque.set_last(previous_first.wrapping_add(32));
                    deque.set_first(previous_first);
                    deque.set_end(previous_first.wrapping_add(31));
                    value
                } else {
                    let end = end.wrapping_sub(1);
                    let value = end.read();
                    deque.set_end(end);
                    value
                };
                state = previous.to_le_bytes();
                info = first_info(sim, i32::from(state[0]), i32::from(state[1]), kind);
                if info.is_null() { break; }
            }
            deque_drop(deque.as_ptr());
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00717ec0::PORT, fc4p_007183b0::PORT];

#[cfg(test)]
mod tests;

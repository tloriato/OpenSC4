// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Building notifications, budget counts and pending service lists.

use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// The budget service, dirty flag, categorized arrays and pending lists.
    struct BuildingSim {
        0xA0 => dirty / set_dirty: u8,
        0xD0 => budget / set_budget: *mut u8,
    }
}
layout! {
    /// A circular owning list of occupant pointers.
    struct Node {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => occupant / set_occupant: *mut u8,
    }
}
layout! {
    /// Only these fields of the eight-word budget item are consumed by this notification.
    struct Item {
        0x00 => department / set_department: u32,
        0x04 => line / set_line: u32,
        0x1C => index / set_index: u32,
    }
}

game_fn! { fn allocate_node(this:*mut u8,count:u32,tag:u32)->*mut u8 = "thiscall" 0x0041_3DD0; }
game_fn! { fn erase_node(this:*mut u8,out:*mut u32,node:*mut u8) = "thiscall" 0x0041_3F70; }
game_fn! { fn read_uint(holder:*mut u8,id:u32,out:*mut u32)->u32 = "C" 0x005F_D480; }
game_fn! { fn read_float(holder:*mut u8,id:u32,out:*mut f32)->u32 = "C" 0x005F_D4F0; }
game_fn! { fn write_uint(holder:*mut u8,id:u32,value:u32) = "C" 0x005F_E3C0; }
game_fn! { fn write_float(holder:*mut u8,id:u32,value:f32) = "C" 0x005F_E4F0; }
game_data! { static zero:f32 = 0x00A8_1054; }

#[derive(Clone, Copy)]
struct Service {
    staff: u32,
    vehicle: u32,
    groups: [u32; 2],
    pending: usize,
    lists: usize,
    served: u32,
    grade: u32,
    ratio: u32,
    capacity: u32,
    effective: u32,
    school: bool,
}

const HOSPITAL: Service = Service {
    staff: 0xCA56_5486,
    vehicle: 0xEA56_549E,
    groups: [0x1507, 0x151C],
    pending: 0x1A0,
    lists: 0x1B4,
    served: 0xC922_015C,
    grade: 0x0924_53CB,
    ratio: 0xAA97_6BE7,
    capacity: 0x6922_0415,
    effective: 0x4924_4B21,
    school: false,
};
const SCHOOL: Service = Service {
    staff: 0xEA56_54B6,
    vehicle: 0x4A56_54BA,
    groups: [0x1503, 0x151B],
    pending: 0x19C,
    lists: 0x1A8,
    served: 0x0922_0161,
    grade: 0x2924_53C5,
    ratio: 0x2A97_6C20,
    capacity: 0x8922_041B,
    effective: 0xE924_4B2E,
    school: true,
};

unsafe fn pointer(this: *mut u8, offset: usize) -> *mut u8 {
    unsafe { this.add(offset).cast::<*mut u8>().read_unaligned() }
}

unsafe fn as_occupant(building: *mut u8) -> *mut u8 {
    unsafe { vcall!(building,0x0C => fn()->*mut u8) }
}

unsafe fn count(this: *mut u8, offset: usize) -> u32 {
    unsafe { ((pointer(this, offset + 4) as u32).wrapping_sub(pointer(this, offset) as u32) as i32 >> 2) as u32 }
}

unsafe fn find(head: *mut u8, occupant: *mut u8) -> *mut u8 {
    unsafe {
        let mut node = Node::from_ptr(head).next();
        while node != head {
            if Node::from_ptr(node).occupant() == occupant {
                break;
            }
            node = Node::from_ptr(node).next();
        }
        node
    }
}

unsafe fn append(container: *mut u8, occupant: *mut u8) {
    unsafe {
        let head = pointer(container, 0);
        let node = allocate_node(container, 1, 0);
        if !node.wrapping_add(8).is_null() {
            Node::from_ptr(node).set_occupant(occupant);
        }
        let previous = Node::from_ptr(head).previous();
        Node::from_ptr(node).set_next(head);
        Node::from_ptr(node).set_previous(previous);
        Node::from_ptr(previous).set_next(node);
        Node::from_ptr(head).set_previous(node);
    }
}

unsafe fn remove_service(this: *mut u8, building: *mut u8, service: Service, scratch: &mut u32) {
    unsafe {
        let mut index = 0u32;
        if count(this, service.lists) != 0 {
            loop {
                let occupant = as_occupant(building);
                let array = pointer(this, service.lists);
                let head = pointer(array, index as usize * 4);
                let node = find(head, occupant);
                if node != head {
                    erase_node(
                        pointer(this, service.lists).wrapping_add(index as usize * 4),
                        scratch,
                        node,
                    );
                    let occupant = as_occupant(building);
                    vcall!(occupant,8 => fn());
                    BuildingSim::from_ptr(this).set_dirty(1);
                    return;
                }
                index = index.wrapping_add(1);
                if index >= count(this, service.lists) {
                    break;
                }
            }
        }
        let occupant = as_occupant(building);
        let head = pointer(this, service.pending);
        let node = find(head, occupant);
        if node != head {
            erase_node(this.add(service.pending), scratch, node);
            let occupant = as_occupant(building);
            vcall!(occupant,8 => fn());
            BuildingSim::from_ptr(this).set_dirty(1);
        }
    }
}

unsafe fn register(holder: *mut u8, id: u32) {
    unsafe {
        vcall!(holder,0x28 => fn(u32,u32,u32,u32),id,0,1,0);
    }
}

/// The same local word is reused for departments, line items, iterator results and optional capacity.
unsafe fn update_service(
    this: *mut u8,
    building: *mut u8,
    holder: *mut u8,
    inserted: bool,
    service: Service,
    scratch: &mut u32,
) -> bool {
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        let mut record = MaybeUninit::<[u32; 8]>::uninit();
        let record = record.as_mut_ptr().cast::<u8>();
        let item = Item::from_ptr(record);
        item.set_index(0);
        let has_item = vcall!(sim.budget(),0xCC => fn(*mut u8,u32,*mut u8)->u32,holder,service.staff,record) as u8 != 0;
        if has_item {
            if item.line() == 0 {
                let line = vcall!(building,0x10 => fn()->u32);
                item.set_line(line);
                vcall!(sim.budget(),0xD0 => fn(*mut u8,u32,u32),holder,item.index(),line);
            }
            let department = vcall!(sim.budget(),0x70 => fn(u32)->*mut u8,item.department());
            if !service.school {
                *scratch = department as u32;
            }
            if department.is_null() {
                return false;
            }
            if inserted {
                let line = vcall!(department,0x50 => fn(u32)->*mut u8,item.line());
                if line.is_null() {
                    let line = vcall!(department,0x48 => fn(u32,u32)->*mut u8,item.line(),1);
                    vcall!(line,0x48 => fn(u32,u32),2,1);
                }
                vcall!(department,0x58 => fn(*mut u8,u32),holder,service.staff);
            } else {
                vcall!(department,0x5C => fn(*mut u8,u32),holder,service.staff);
            }
            let line = vcall!(department,0x50 => fn(u32)->*mut u8,item.line());
            let table = line.cast::<*const usize>().read_unaligned();
            if service.school {
                *scratch = line as u32;
            }
            let value = vcall!(line,0x20 => fn()->u64);
            let value = if inserted {
                value.wrapping_add(1)
            } else {
                value.wrapping_sub(1)
            };
            let setter = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, u32)>(
                table.add(0x24 / 4).read(),
            );
            setter(line, value as u32, (value >> 32) as u32);
            if !inserted && vcall!(line,0x20 => fn()->u64) == 0 {
                vcall!(department,0x4C => fn(u32),item.line());
            }
            let mut vehicle = MaybeUninit::<[u32; 8]>::uninit();
            let vehicle = vehicle.as_mut_ptr().cast::<u8>();
            let vehicle_item = Item::from_ptr(vehicle);
            vehicle_item.set_index(0);
            if vcall!(sim.budget(),0xCC => fn(*mut u8,u32,*mut u8)->u32,holder,service.vehicle,vehicle) as u8 != 0 {
                let department = vcall!(sim.budget(),0x70 => fn(u32)->*mut u8,vehicle_item.department());
                if department.is_null() {
                    return false;
                }
                if inserted {
                    vcall!(department,0x58 => fn(*mut u8,u32),holder,service.vehicle);
                } else {
                    vcall!(department,0x5C => fn(*mut u8,u32),holder,service.vehicle);
                }
            }
        } else {
            let occupant = as_occupant(building);
            if vcall!(occupant,0x58 => fn(u32)->u32,service.groups[0]) as u8 == 0 {
                let occupant = as_occupant(building);
                if vcall!(occupant,0x58 => fn(u32)->u32,service.groups[1]) as u8 == 0 {
                    return true;
                }
            }
        }
        if inserted {
            register(holder, service.served);
            write_uint(holder, service.served, u32::MAX);
            register(holder, service.grade);
            write_float(holder, service.grade, 1.0);
            register(holder, service.ratio);
            write_float(holder, service.ratio, 1.0);
            read_uint(holder, service.capacity, scratch);
            register(holder, service.effective);
            write_uint(holder, service.effective, *scratch);
            vcall!(building,4 => fn());
            let occupant = as_occupant(building);
            append(this.add(service.pending), occupant);
            sim.set_dirty(1);
        } else {
            remove_service(this, building, service, scratch);
        }
        true
    }
}

unsafe fn positive(value: f32) -> bool {
    let cmp = F80::from_f32(value).compare(F80::from_f32(unsafe { *zero() }));
    cmp.status_word() & 0x4100 == 0
}

port! {
    /// Update hospital and school budgets and owning lists in that order, then health-affecting lists.
    /// Null departments abort the notification. Missing capacity retains the local word's earlier value.
    #[port(address=0x006F_B120,owner="residential",name="cSC4ResidentialSimulator::DoBuildingChanged",naked)]
    pub unsafe extern "C" fn fc4p_006fb120() {
        core::arch::naked_asm!("mov edx,dword ptr [esp-0x6c]","jmp {body}",body=sym building_changed);
    }
}

unsafe extern "fastcall" fn building_changed(this: *mut u8, entry_scratch: u32, building: *mut u8, inserted: u32) {
    unsafe {
        let mut scratch = entry_scratch;
        let occupant = as_occupant(building);
        let holder = vcall!(occupant,0x18 => fn()->*mut u8);
        let inserted = inserted as u8 != 0;
        if !update_service(this, building, holder, inserted, HOSPITAL, &mut scratch) {
            return;
        }
        if !update_service(this, building, holder, inserted, SCHOOL, &mut scratch) {
            return;
        }
        let mut first = MaybeUninit::<f32>::uninit();
        let mut second = MaybeUninit::<f32>::uninit();
        if read_float(holder, 0x6A19_F6B5, first.as_mut_ptr()) as u8 != 0
            && read_float(holder, 0xAA19_F6EA, second.as_mut_ptr()) as u8 != 0
            && positive(first.assume_init())
            && positive(second.assume_init())
        {
            if inserted {
                let occupant = as_occupant(building);
                vcall!(occupant,4 => fn());
                let occupant = as_occupant(building);
                append(this.add(0x1A4), occupant);
            } else {
                let occupant = as_occupant(building);
                let head = pointer(this, 0x1C0);
                let node = find(head, occupant);
                if node != head {
                    erase_node(this.add(0x1C0), &mut scratch, node);
                    let occupant = as_occupant(building);
                    vcall!(occupant,8 => fn());
                } else {
                    let occupant = as_occupant(building);
                    let head = pointer(this, 0x1A4);
                    let node = find(head, occupant);
                    if node != head {
                        erase_node(this.add(0x1A4), &mut scratch, node);
                        let occupant = as_occupant(building);
                        vcall!(occupant,8 => fn());
                    }
                }
            }
            BuildingSim::from_ptr(this).set_dirty(1);
        }
        let purpose = vcall!(building,0x2C => fn()->*const i32);
        if purpose.read_unaligned() == 1 {
            BuildingSim::from_ptr(this).set_dirty(1);
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006fb120::PORT];

#[cfg(test)]
mod tests;

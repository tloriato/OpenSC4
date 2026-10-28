// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Building collection queries and the owning lists of residential tuning records.

use crate::port::Port;
use core::ptr::{null_mut, read_unaligned, write_unaligned};

layout! {
    /// The two arrays of categorized building lists and their pending lists.
    pub struct CollectionsSim {
        0x19C => pending_school / set_pending_school: *mut u8,
        0x1A0 => pending_hospital / set_pending_hospital: *mut u8,
        0x1A8 => schools / set_schools: *mut *mut u8,
        0x1AC => schools_end / set_schools_end: *mut *mut u8,
        0x1B4 => hospitals / set_hospitals: *mut *mut u8,
        0x1B8 => hospitals_end / set_hospitals_end: *mut *mut u8,
    }
}
layout! {
    /// A circular list link, with an optional building pointer following its links.
    pub struct Node {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => value / set_value: *mut u8,
    }
}
layout! {
    /// A tuning record's two curves, nested list and trailing scalar.
    pub struct Record {
        0x00 => first / set_first: u32,
        0x14 => second / set_second: u32,
        0x18 => third / set_third: u32,
        0x2C => list / set_list: *mut u8,
        0x30 => last / set_last: u32,
    }
}
layout! {
    /// The storage words released by the owning list of tuning records.
    struct OwnedRecordNode {
        0x0C => first_curve / set_first_curve: *mut u8,
        0x24 => second_curve / set_second_curve: *mut u8,
        0x34 => nested_head / set_nested_head: *mut u8,
    }
}
layout! {
    /// Group filter storage, reloaded after every group callback.
    struct Groups {
        0x00 => begin / set_begin: *mut u32,
        0x04 => end / set_end: *mut u32,
    }
}

game_fn! { fn framework()->*mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_temporary(this:*mut *mut u8)->u32 = "thiscall" 0x0041_1870; }
// EDX is an unused register input, kept because an empty input list leaves it untouched.
game_fn! { fn get_buildings(this:*mut u8,edx:u32,list:*mut *mut u8,out:*mut *mut u8,groups:*mut u8)->u64 = "fastcall" 0x006F_ACE0; }
game_fn! { fn copy_curve(this:*mut u8,source:*mut u8)->u32 = "thiscall" 0x005F_4430; }
game_fn! { fn allocate(size:u32)->*mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(pointer:*mut u8)->u32 = "C" 0x0090_CF63; }
game_fn! { fn insert_range(this:*mut u8,position:*mut u8,first:*mut u8,last:*mut u8,tag:*const u8)->u32 = "thiscall" 0x006F_1570; }
game_data! { static node_allocator:*mut u8 = 0x00B2_07E4; }

fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

fn count(begin: *mut u8, end: *mut u8) -> u32 {
    ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32
}

port! {
    /// Append buildings that match any requested group, retaining each before allocating its node.
    /// A null filter accepts all buildings; an empty filter accepts none. The allocator is acquired lazily.
    /// EAX is the input sentinel. The unused EDX input is retained when the list is empty.
    #[port(address=0x006F_ACE0,owner="residential",name="cSC4ResidentialSimulator::GetBuildings")]
    pub unsafe extern "fastcall" fn fc4p_006face0(_this:*mut u8,entry_edx:u32,list:*mut *mut u8,out:*mut *mut u8,groups:*mut u8)->u64 {
        unsafe {
            let head=read_unaligned(list);
            let mut node=Node::from_ptr(head).next();
            let mut edx=entry_edx;
            while node!=head {
                let building=Node::from_ptr(node).value();
                let mut accept=groups.is_null();
                if !groups.is_null() {
                    let filter=Groups::from_ptr(groups);
                    edx=count(filter.begin().cast(),filter.end().cast());
                    let mut index=0u32;
                    while index<count(filter.begin().cast(),filter.end().cast()) {
                        if accept {break;}
                        let result=vcall!(building,0x58 => fn(u32)->u64,read_unaligned(filter.begin().wrapping_add(index as usize)));
                        edx=(result>>32) as u32;
                        if result as u8!=0 {accept=true;}
                        index=index.wrapping_add(1);
                    }
                }
                if accept {
                    vcall!(building,4 => fn()->u32);
                    let destination=read_unaligned(out);
                    if (*node_allocator()).is_null() {
                        let mut temporary=null_mut();
                        let fw=framework();
                        if !fw.is_null() {
                            vcall!(fw,0x14 => fn(u32,u32,*mut *mut u8)->u32,0x3AE4_BEA3,0x3AE4_BEAB,&mut temporary);
                        }
                        *node_allocator()=temporary;
                        release_temporary(&mut temporary);
                    }
                    let result=vcall!(*node_allocator(),0x0C => fn(u32)->u64,12);
                    let allocation=result as u32 as *mut u8;
                    edx=(result>>32) as u32;
                    if !(allocation.wrapping_add(8)).is_null() {
                        write_unaligned(allocation.wrapping_add(8).cast::<*mut u8>(),building);
                    }
                    let previous=Node::from_ptr(destination).previous();
                    let new=Node::from_ptr(allocation);
                    new.set_next(destination);
                    new.set_previous(previous);
                    Node::from_ptr(previous).set_next(allocation);
                    Node::from_ptr(destination).set_previous(allocation);
                }
                node=Node::from_ptr(node).next();
            }
            registers(head as u32,edx)
        }
    }
}

unsafe fn service_buildings(this: *mut u8, entry_edx: u32, out: *mut *mut u8, groups: *mut u8, school: bool) -> u64 {
    unsafe {
        let sim = CollectionsSim::from_ptr(this);
        let mut index = 0u32;
        let mut edx = entry_edx;
        loop {
            let (begin, end) = if school {
                (sim.schools(), sim.schools_end())
            } else {
                (sim.hospitals(), sim.hospitals_end())
            };
            if index >= count(begin.cast(), end.cast()) {
                break;
            }
            let list = begin.wrapping_add(index as usize);
            edx = (get_buildings(this, list as u32, list, out, groups) >> 32) as u32;
            index = index.wrapping_add(1);
        }
        get_buildings(
            this,
            edx,
            this.add(if school { 0x19C } else { 0x1A0 }).cast(),
            out,
            groups,
        )
    }
}
port! {
    /// Append school groups in table order, then the pending school list, with the same filter.
    #[port(address=0x006F_ADE0,owner="residential",name="cSC4ResidentialSimulator::GetSchoolBuildings")]
    pub unsafe extern "fastcall" fn fc4p_006fade0(this:*mut u8,edx:u32,out:*mut *mut u8,groups:*mut u8)->u64 {
        unsafe {service_buildings(this,edx,out,groups,true)}
    }
}
port! {
    /// Append hospital groups in table order, then the pending hospital list, with the same filter.
    #[port(address=0x006F_AE50,owner="residential",name="cSC4ResidentialSimulator::GetHospitalBuildings")]
    pub unsafe extern "fastcall" fn fc4p_006fae50(this:*mut u8,edx:u32,out:*mut *mut u8,groups:*mut u8)->u64 {
        unsafe {service_buildings(this,edx,out,groups,false)}
    }
}

port! {
    /// Copy both curves through their constructors, create a circular list, copy its range, then its scalar.
    #[port(address=0x006F_B9F0,owner="residential",name="CopyResidentialTuningRecord")]
    pub unsafe extern "thiscall" fn fc4p_006fb9f0(this:*mut u8,source:*mut u8)->u64 {
        unsafe {
            let destination=Record::from_ptr(this);
            let original=Record::from_ptr(source);
            destination.set_first(original.first());
            copy_curve(this.add(4),source.add(4));
            destination.set_second(original.second());
            destination.set_third(original.third());
            copy_curve(this.add(0x1C),source.add(0x1C));
            destination.set_list(null_mut());
            let head=allocate(12);
            Node::from_ptr(head).set_next(head);
            Node::from_ptr(head).set_previous(head);
            destination.set_list(head);
            let source_head=original.list();
            let tag=source;
            insert_range(this.add(0x2C),Node::from_ptr(head).next(),Node::from_ptr(source_head).next(),source_head,(&tag as *const *mut u8).cast());
            let last=original.last();
            destination.set_last(last);
            registers(this as u32,last)
        }
    }
}

port! {
    /// Delete nested list nodes and sentinels, curve storage, and record nodes in traversal order.
    /// Leave the outer sentinel allocated and reset its links without changing its payload.
    #[port(address=0x006F_CB40,owner="residential",name="ClearResidentialTuningRecords")]
    pub unsafe extern "thiscall" fn fc4p_006fcb40(this:*mut *mut u8)->u32 {
        unsafe {
            let mut node=Node::from_ptr(read_unaligned(this)).next();
            while node!=read_unaligned(this) {
                let current=node;
                let record=OwnedRecordNode::from_ptr(current);
                let mut child=Node::from_ptr(record.nested_head()).next();
                node=Node::from_ptr(node).next();
                while child!=record.nested_head() {
                    let old=child;
                    child=Node::from_ptr(child).next();
                    free(old);
                }
                Node::from_ptr(record.nested_head()).set_next(record.nested_head());
                Node::from_ptr(record.nested_head()).set_previous(record.nested_head());
                let head=record.nested_head();
                if !head.is_null() {free(head);}
                let curve=record.second_curve();
                if !curve.is_null() {free(curve);}
                let curve=record.first_curve();
                if !curve.is_null() {free(curve);}
                free(current);
            }
            Node::from_ptr(read_unaligned(this)).set_next(read_unaligned(this));
            let head=read_unaligned(this);
            Node::from_ptr(head).set_previous(head);
            head as u32
        }
    }
}
port! {
    /// Delete each nonnull owned payload before its list node, then reset the retained sentinel's links.
    #[port(address=0x006F_CBD0,owner="residential",name="ClearResidentialOwnedPointers")]
    pub unsafe extern "thiscall" fn fc4p_006fcbd0(this:*mut *mut u8)->u32 {
        unsafe {
            let mut node=Node::from_ptr(read_unaligned(this)).next();
            while node!=read_unaligned(this) {
                let old=node;
                let payload=Node::from_ptr(old).value();
                node=Node::from_ptr(node).next();
                if !payload.is_null() {free(payload);}
                free(old);
            }
            let head=read_unaligned(this);
            Node::from_ptr(head).set_next(head);
            let last_head=read_unaligned(this);
            Node::from_ptr(last_head).set_previous(last_head);
            head as u32
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006face0::PORT,
    fc4p_006fade0::PORT,
    fc4p_006fae50::PORT,
    fc4p_006fb9f0::PORT,
    fc4p_006fcb40::PORT,
    fc4p_006fcbd0::PORT,
];

#[cfg(test)]
mod tests;

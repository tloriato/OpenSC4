// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Civic coverage increments and newly covered population.

use super::civic::multiply;
use super::served::{divide, Tract};
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// Population grids used for newly covered tracts.
    pub struct CoverageSim {
        0x48 => populations / set_populations: [*mut u8;3],
    }
}
game_fn! { fn read_float(properties:*mut u8,id:u32,out:*mut f32)->u32 = "C" 0x005F_D4F0; }
game_fn! { fn read_uint(properties:*mut u8,id:u32,out:*mut u32)->u32 = "C" 0x005F_D480; }
game_fn! { fn allocate(bytes:u32)->*mut u8 = "C" 0x0090_CF54; }
game_fn! { fn clear_list(list:*mut *mut u8)->u32 = "thiscall" 0x0050_A2A0; }
game_fn! { fn destroy_list(list:*mut *mut u8)->u32 = "thiscall" 0x0056_50A0; }
game_fn! { fn covered_tracts(this:*mut u8,building:*mut u8,radius:f32,scale:f32,x:u32,z:u32,list:*mut *mut u8)->u32 = "thiscall" 0x006F_7E00; }
game_data! { static zero:f32 = 0x00A8_1054; }

/// Capture the caller-saved ECX as well as EAX from a byte grid query.
unsafe fn grid_value_and_ecx(grid: *mut u8, x: u32, z: u32) -> (u32, u32) {
    let mut call = [unsafe { crate::abi::vslot(grid, 0x58) } as u32, grid as u32, x, z, 0, 0];
    unsafe {
        core::arch::asm!("push dword ptr [edi+12]","push dword ptr [edi+8]","mov ecx,dword ptr [edi+4]","call dword ptr [edi]","mov dword ptr [edi+16],eax","mov dword ptr [edi+20],ecx",in("edi") call.as_mut_ptr(),clobber_abi("C"));
    }
    (call[4], call[5])
}
port! {
    /// Increment the selected coverage grid and count population only on the first covered tract
    /// across all service grids. Saturation tests the low word of the incremented value.
    #[port(address=0x006F_9BF0,owner="residential",name="cSC4ResidentialSimulator::MarkBuildingCoverage")]
    pub unsafe extern "thiscall" fn fc4p_006f9bf0(this:*mut u8,grids:*mut *mut *mut u8,index:u32,building:*mut u8,radius_property:u32,multiplier:f32,capacity_property:u32,chunk:u32,served:*mut u32) {
        unsafe {
            let grid=*(*grids).wrapping_add(index as usize);
            let properties=vcall!(building,0x18 => fn()->*mut u8);
            let mut radius=MaybeUninit::<f32>::uninit();
            if read_float(properties,radius_property,radius.as_mut_ptr()) as u8==0{return;}
            let mut capacity=MaybeUninit::<u32>::uninit();
            if read_uint(properties,capacity_property,capacity.as_mut_ptr()) as u8==0{return;}
            let capacity=capacity.assume_init();
            let mut position=MaybeUninit::<[f32;3]>::uninit();
            vcall!(building,0x20 => fn(*mut f32)->u32,position.as_mut_ptr().cast());
            vcall_st0!(grid,0x40);
            let product=multiply(F80::from_f32(radius.assume_init()),multiplier);
            let radius=product.to_f32();
            let comparison=product.compare(F80::from_f32(*zero()));
            if comparison.less()||comparison.equal()||comparison.unordered(){return;}
            let mut head=allocate(16);
            head.cast::<*mut u8>().write(head);head.add(4).cast::<*mut u8>().write(head);
            clear_list(&mut head);
            let z=vcall!(grid,0x34 => fn()->u32);
            let x=vcall!(grid,0x30 => fn()->u32);
            let scale=vcall_st0!(grid,0x38).to_f32();
            covered_tracts(this,building,radius,scale,x,z,&mut head);
            let sentinel=head;let mut node=Tract::from_ptr(sentinel).next();
            while node!=sentinel {
                let tract=Tract::from_ptr(node);let (x,z)=(tract.x(),tract.z());
                let (value,ecx)=grid_value_and_ecx(grid,x,z);let old=value as u8;
                let increment=divide(capacity,chunk);
                let mut value=increment.wrapping_add((ecx & 0xFFFF_0000)|u32::from(old));
                if value as u16>255{value=255;}
                vcall!(grid,0x68 => fn(u32,u32,u32)->u32,x,z,value);
                if old==0 {
                    let mut any=false;let mut i=0i32;
                    while i<(index as i32) {
                        let other=*(*grids).wrapping_add(i as usize);
                        if vcall!(other,0x58 => fn(u32,u32)->u32,x,z) as u8>0 {any=true;break;}
                        i=i.wrapping_add(1);
                    }
                    if !any {
                        let mut sum=0u32;
                        for i in 0..3 {
                            let population=*this.add(0x48+i*4).cast::<*mut u8>();
                            sum=sum.wrapping_add(vcall!(population,0x58 => fn(u32,u32)->u32,x,z)&0xFFFF);
                        }
                        *served=(*served).wrapping_add(sum);
                    }
                }
                node=Tract::from_ptr(node).next();
            }
            destroy_list(&mut head);
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_006f9bf0::PORT];

#[cfg(test)]
mod tests;

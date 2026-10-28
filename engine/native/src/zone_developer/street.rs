// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Street runs clipped to the current restriction region.

use super::{region::Region, Port, ZoneDeveloper};

game_fn! { fn includes(region: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0046_6610; }
game_fn! { fn place_last_fallback(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, mask: u32) = "thiscall" 0x0072_A0D0; }

unsafe fn place(dev: ZoneDeveloper, a: (i32, i32), b: (i32, i32)) -> bool {
    unsafe {
        let tool = dev.street_tool();
        vcall!(tool,0x1C => fn(i32,i32,i32,i32,u32,*mut u8,u32,i32) -> u32,
            a.0,a.1,b.0,b.1,0xC49,tool,u32::from(dev.real_mode()),-1) as u8
            != 0
    }
}

port! {
    /// Walk toward the endpoint in unit steps, skipping clear restriction cells. Place each contiguous
    /// run, with three endpoint-trimming fallbacks; optionally place every run cell as well. A final
    /// singleton uses the developer's final fallback helper after three failed tool placements.
    #[port(address = 0x0072_A2A0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::DrawStreet")]
    pub unsafe extern "thiscall" fn fc4p_0072a2a0(this: *mut u8,x1:i32,z1:i32,x2:i32,z2:i32,per_cell:u32) {
        unsafe {
            let dev=ZoneDeveloper::from_ptr(this);
            let end=(x2,z2);
            let mut current=(x1,z1);
            if current==end {
                if includes(dev.restriction(),x1,z1) as u8 != 0 { place(dev,current,end); }
                return;
            }
            let dx=x2.wrapping_sub(x1).signum();
            let dz=z2.wrapping_sub(z1).signum();
            let step=|(x,z):(i32,i32)| (x.wrapping_add(dx),z.wrapping_add(dz));
            let back=|(x,z):(i32,i32)| (x.wrapping_sub(dx),z.wrapping_sub(dz));
            loop {
                let region=Region::from_ptr(dev.restriction());
                while !region.includes(current.0,current.1) && current!=end { current=step(current); }
                if current==end {
                    let region=Region::from_ptr(dev.restriction());
                    if region.includes(current.0,current.1)
                        && !place(dev,current,end) && !place(dev,step(current),end)
                        && !place(dev,current,back(end)) {
                        let a=step(current);let b=back(end);
                        place_last_fallback(this,a.0,a.1,b.0,b.1,0xC49);
                    }
                    return;
                }
                let mut run_end=current;
                loop {
                    if run_end==end {break;}
                    run_end=step(run_end);
                    if !region.includes(run_end.0,run_end.1) { run_end=back(run_end);break; }
                }
                if !(per_cell as u8 != 0 && current==run_end)
                    && !place(dev,current,run_end) && !place(dev,step(current),end)
                    && !place(dev,current,back(end)) {
                    place(dev,step(current),back(end));
                }
                if per_cell as u8 != 0 {
                    while current!=run_end { place(dev,current,current);current=step(current); }
                    place(dev,run_end,run_end);
                }
                if run_end==end { return; }
                current=step(run_end);
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0072a2a0::PORT];

#[cfg(test)]
mod tests;

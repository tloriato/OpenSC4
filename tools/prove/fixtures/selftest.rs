//! Independent land-value fixtures: thiscall methods, raw object layouts, no imports.
#![no_std]
use core::arch::asm;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[used]
pub static _fltused: i32 = 0;

unsafe fn at<T: Copy>(p: *const u8, off: usize) -> T {
    *(p.add(off) as *const T)
}
unsafe fn vslot(obj: *const u8, off: usize) -> usize {
    *((*(obj as *const *const usize)).add(off / 4))
}

#[derive(Clone, Copy)]
struct Wide([u8; 10]);
impl From<f32> for Wide {
    fn from(f: f32) -> Self {
        let mut value = [0u8; 10];
        unsafe {
            asm!("fld dword ptr [{input}]","fstp tbyte ptr [{output}]",input=in(reg) &f, output=in(reg) value.as_mut_ptr(),options(nostack));
        }
        Self(value)
    }
}
impl Wide {
    fn plus(self, add: f32) -> Self {
        let mut value = [0u8; 10];
        unsafe {
            asm!("fld tbyte ptr [{input}]","fadd dword ptr [{add}]","fstp tbyte ptr [{output}]",input=in(reg) self.0.as_ptr(),add=in(reg) &add,output=in(reg) value.as_mut_ptr(),options(nostack));
        }
        Self(value)
    }
    fn scalar(self) -> f32 {
        let mut value = 0f32;
        unsafe {
            asm!("fld tbyte ptr [{input}]","fstp dword ptr [{output}]",input=in(reg) self.0.as_ptr(),output=in(reg) &mut value,options(nostack));
        }
        value
    }
}
// Truncate through a signed 64-bit integer and retain its low word.
#[inline(always)]
fn trunc<T: Into<Wide>>(f: T) -> i32 {
    let f = f.into();
    let mut result: i64 = 0;
    let mut saved: u16 = 0;
    let chop: u16;
    unsafe {
        asm!("fnstcw word ptr [{cw}]", cw=in(reg) &mut saved, options(nostack));
        chop = saved | 0x0c00;
        asm!("fldcw word ptr [{chop}]", "fld tbyte ptr [{value}]",
             "fistp qword ptr [{result}]", "fldcw word ptr [{cw}]",
             chop=in(reg) &chop, value=in(reg) f.0.as_ptr(), result=in(reg) &mut result,
             cw=in(reg) &saved, options(nostack));
    }
    result as i32
}

type VCall0 = unsafe extern "thiscall" fn(*const u8) -> i32;
type VCall2 = unsafe extern "thiscall" fn(*const u8, i32, i32) -> i32;
type CurveOut = unsafe extern "thiscall" fn(*const u8, f32) -> f32;

#[no_mangle]
pub unsafe extern "thiscall" fn fc4p_006bd540(this: *const u8, tx: i32, tz: i32) -> u8 {
    let grid: *const u8 = at(this, 0x24);
    let shift = core::mem::transmute::<usize, VCall0>(vslot(grid, 0x2C))(grid);
    let grid: *const u8 = at(this, 0x24);
    let size = core::mem::transmute::<usize, VCall0>(vslot(grid, 0x28))(grid);
    let (x0, z0) = (
        tx.wrapping_shl(shift as u32 & 31),
        tz.wrapping_shl(shift as u32 & 31),
    );
    let (x1, z1) = (x0.wrapping_add(size), z0.wrapping_add(size));
    let mut x = x0;
    while x < x1 {
        let mut z = z0;
        while z < z1 {
            let zones: *const u8 = at(this, 0x20);
            if core::mem::transmute::<usize, VCall2>(vslot(zones, 0x30))(zones, x, z) != 0 {
                return 1;
            }
            z += 1;
        }
        x += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "thiscall" fn fc4p_006bd810(this: *const u8, altitude: f32) -> u8 {
    let k = *(0x00A8_4D24 as *const f32);
    if altitude < at::<f32>(this, 0x98) {
        return 0;
    }
    let mut wide = [0u8; 10];
    if altitude < at::<f32>(this, 0x60) {
        asm!("fld dword ptr [{a}]", "fsub dword ptr [{lo}]", "fdiv dword ptr [{range}]",
             "fmul dword ptr [{k}]", "fstp tbyte ptr [{out}]",
             a=in(reg) &altitude, lo=in(reg) this.add(0x98), range=in(reg) this.add(0x64),
             k=in(reg) &k, out=in(reg) wide.as_mut_ptr(), options(nostack));
    } else {
        let top = at::<i32>(this, 0x6c).wrapping_sub(50);
        asm!("fild dword ptr [{top}]", "fdiv dword ptr [{range}]", "fld dword ptr [{a}]",
             "fsub dword ptr [{pivot}]", "fmulp st(1), st(0)", "fadd dword ptr [{k}]", "fstp tbyte ptr [{out}]",
             top=in(reg) &top, range=in(reg) this.add(0x68), a=in(reg) &altitude,
             pivot=in(reg) this.add(0x60), k=in(reg) &k, out=in(reg) wide.as_mut_ptr(), options(nostack));
    }
    let position = Wide(wide);
    let curve: *const u8 = at(this, 0x84);
    let out = core::mem::transmute::<usize, CurveOut>(0x005F_4250)(curve, trunc(position) as f32);
    trunc(out) as u8
}

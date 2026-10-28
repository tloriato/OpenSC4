// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, GameFpu};
use std::cell::RefCell;

#[derive(Default)]
struct State {
    tool: usize,
    preview: usize,
    filter: usize,
    dummy: usize,
    city_service: usize,
    controller: usize,
    width: u32,
    height: u32,
    info: usize,
    calls: Vec<(u32, usize, Vec<u32>)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(tag: u32, this: *mut u8, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push((tag, this as usize, args.to_vec())));
}
extern "thiscall" fn register(this: *mut u8, callback: u32, context: u32) {
    unsafe {
        assert_eq!(*rules_initialized(), 1);
    }
    assert_eq!((callback, context), (0x0062_32F0, 0));
    log(1, this, &[callback, context]);
}
extern "C" fn rules() {
    log(2, core::ptr::null_mut(), &[]);
}
extern "thiscall" fn specific(this: *mut u8, id: u32) {
    assert_eq!(id, 0xDEAD_BEEF);
    STATE.with(|s| {
        assert_eq!(
            unsafe { NetworkTool::from_ptr(this) }.type_info() as usize,
            s.borrow().info
        )
    });
    log(3, this, &[id]);
}
extern "C" fn alloc(size: u32) -> *mut u8 {
    log(4, core::ptr::null_mut(), &[size]);
    STATE.with(|s| match size {
        0x58 => s.borrow().preview as *mut u8,
        0x10 => s.borrow().filter as *mut u8,
        _ => unreachable!(),
    })
}
extern "thiscall" fn preview_new(this: *mut u8) -> *mut u8 {
    log(5, this, &[]);
    this
}
extern "thiscall" fn addref(this: *mut u8) {
    log(6, this, &[]);
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    log(7, this, &[]);
    registers(0x1234_5678, 0x9ABC_DEF0)
}
extern "thiscall" fn preview_init(this: *mut u8) {
    log(8, this, &[]);
}
extern "thiscall" fn preview_register(this: *mut u8, preview: *mut u8, mode: u32, rate: u32) {
    assert_eq!((mode, rate), (5, 1000));
    log(9, this, &[preview as u32, mode, rate]);
}
extern "thiscall" fn filter_new(this: *mut u8) -> *mut u8 {
    log(10, this, &[]);
    this
}
extern "thiscall" fn assign(this: *mut u8, source: *mut u8) {
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!((this as usize, source as usize), (s.tool + 0x290, s.info + 0x50));
    });
    log(11, this, &[source as u32]);
    unsafe {
        write(this, 0x18, 0.25f32);
        write(this, 0x34, 0.125f32);
    }
}
extern "C" fn bridges(source: *mut u8, first: *mut u8, second: *mut u8) {
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(
            (source as usize, first as usize, second as usize),
            (s.info + 0x30, s.tool + 0x33C, s.tool + 0x348)
        );
    });
    log(12, source, &[first as u32, second as u32]);
}
extern "thiscall" fn width(this: *mut u8) -> u32 {
    log(13, this, &[]);
    STATE.with(|s| s.borrow().width)
}
extern "thiscall" fn height(this: *mut u8) -> u32 {
    log(14, this, &[]);
    STATE.with(|s| s.borrow().height)
}
#[unsafe(naked)]
unsafe extern "C" fn scale() {
    core::arch::naked_asm!("fld tbyte ptr [ecx + 8]", "ret")
}
extern "thiscall" fn city_service(this: *mut u8) -> *mut u8 {
    log(15, this, &[]);
    STATE.with(|s| s.borrow().city_service as *mut u8)
}
extern "thiscall" fn controller(this: *mut u8) -> *mut u8 {
    log(16, this, &[]);
    STATE.with(|s| s.borrow().controller as *mut u8)
}
extern "C" fn alloc_dummy(size: u32) -> *mut u8 {
    assert_eq!(size, 0x180);
    log(17, core::ptr::null_mut(), &[size]);
    STATE.with(|s| s.borrow().dummy as *mut u8)
}
extern "thiscall" fn new_dummy(this: *mut u8, initialize: u32) -> *mut u8 {
    assert_eq!(initialize, 1);
    log(18, this, &[initialize]);
    this
}
extern "thiscall" fn dummy_init(this: *mut u8) {
    log(19, this, &[]);
}
extern "thiscall" fn dummy_shutdown(this: *mut u8) -> u64 {
    log(20, this, &[]);
    registers(0x4444_5555, 0x6666_7777)
}
extern "thiscall" fn subscribe(this: *mut u8, target: *mut u8, id: u32) -> u64 {
    assert_eq!(id, 0xA6B7_9602);
    log(21, this, &[target as u32, id]);
    registers(0xAABB_CCDD, 0xEEFF_1122)
}

#[test]
fn init_reuses_globals_copies_parameters_builds_geometry_and_orders_service_lifetimes() {
    let _fpu = GameFpu::new();
    let _f = [
        fake(0x0062_53D0, rules as *const () as usize),
        fake(0x005E_55E0, alloc as *const () as usize),
        fake(0x0062_0AD0, preview_new as *const () as usize),
        fake(0x0061_FD10, preview_init as *const () as usize),
        fake(0x0061_F510, filter_new as *const () as usize),
        fake(0x0062_6860, assign as *const () as usize),
        fake(0x0062_F9D0, bridges as *const () as usize),
        fake(0x0061_9E90, alloc_dummy as *const () as usize),
        fake(0x0061_BDD0, new_dummy as *const () as usize),
    ];
    for (type_id, mask, bridge_len, optional, same_preview, width_value) in [
        (0usize, 1u32, 0u32, true, false, 64u32),
        (4, 16, 0x178, false, true, u32::MAX),
        (7, 0x80, 0x2F0, true, true, 0),
        (12, 0x1000, 0, false, false, 127),
    ] {
        let mut tool = [0xA5A5_A5A5u32; 0x368 / 4];
        let t = tool.as_mut_ptr().cast::<u8>();
        let mut main_table = [0usize; 80];
        main_table[79] = specific as *const () as usize;
        let mut service_table = [0usize; 96];
        service_table[1] = addref as *const () as usize;
        service_table[2] = release as *const () as usize;
        service_table[3] = dummy_init as *const () as usize;
        service_table[4] = dummy_shutdown as *const () as usize;
        service_table[5] = subscribe as *const () as usize;
        service_table[6] = scale as *const () as usize;
        service_table[0x28 / 4] = controller as *const () as usize;
        service_table[0x80 / 4] = preview_register as *const () as usize;
        service_table[0x84 / 4] = city_service as *const () as usize;
        service_table[0x114 / 4] = register as *const () as usize;
        service_table[0x174 / 4] = width as *const () as usize;
        service_table[0x178 / 4] = height as *const () as usize;
        let vt = service_table.as_ptr() as u32;
        let mut preview = [vt; 0x58 / 4];
        let pp = preview.as_mut_ptr().cast::<u8>();
        let mut old_preview = [vt];
        let op = old_preview.as_mut_ptr().cast::<u8>();
        let mut filter = [vt; 4];
        let fp = filter.as_mut_ptr().cast::<u8>();
        let mut dummy = [vt; 0x180 / 4];
        let dp = dummy.as_mut_ptr().cast::<u8>();
        let mut service = [vt; 8];
        let sp = service.as_mut_ptr().cast::<u8>();
        let mut terrain_object = [vt; 8];
        let tp = terrain_object.as_mut_ptr().cast::<u8>();
        unsafe {
            core::ptr::copy_nonoverlapping(F80::from_f64(16.0000001).to_bytes().as_ptr(), tp.add(8), 10);
        }
        let mut manager = sp;
        let mut city = sp;
        let mut resource = sp;
        let mut terrain = if optional { tp } else { core::ptr::null_mut() };
        let mut occupants = if optional { sp } else { core::ptr::null_mut() };
        let mut messages = if optional { sp } else { core::ptr::null_mut() };
        let mut initialized = u8::from(type_id == 4);
        let mut infos = [0xA5u8; 0xE04];
        let info = unsafe { infos.as_mut_ptr().add(type_id * 0x114) };
        unsafe {
            write(info, 0x30, 0x1000u32);
            write(info, 0x34, 0x1000u32 + bridge_len);
        }
        let mut ids = [0u32; 13];
        ids[type_id] = 0xDEAD_BEEF;
        let mut masks = [0u32; 13];
        masks[type_id] = mask;
        let _g = [
            fake_data(0x00B4_52C8, infos.as_mut_ptr()),
            fake_data(0x00B4_52A8, &mut initialized),
            fake_data(0x00B4_3C94, (&mut resource as *mut *mut u8).cast()),
            fake_data(0x00AA_8428, ids.as_mut_ptr().cast()),
            fake_data(0x00AA_835C, masks.as_mut_ptr().cast()),
            fake_data(0x00B4_3DD0, (&mut manager as *mut *mut u8).cast()),
            fake_data(0x00B4_3CEC, (&mut city as *mut *mut u8).cast()),
            fake_data(0x00B4_3CF4, (&mut terrain as *mut *mut u8).cast()),
            fake_data(0x00B4_3D0C, (&mut occupants as *mut *mut u8).cast()),
            fake_data(0x00B4_3CCC, (&mut messages as *mut *mut u8).cast()),
        ];
        tool[0] = main_table.as_ptr() as u32;
        tool[6] &= !0xFF;
        tool[0x35C / 4] = type_id as u32;
        tool[0x1F0 / 4] = if same_preview { pp as u32 } else { op as u32 };
        tool[0x238 / 4] = 4f32.to_bits();
        STATE.with(|s| {
            *s.borrow_mut() = State {
                tool: t as usize,
                preview: pp as usize,
                filter: fp as usize,
                dummy: dp as usize,
                city_service: if optional { sp as usize } else { 0 },
                controller: if optional { sp as usize } else { 0 },
                width: width_value,
                height: 40,
                info: info as usize,
                ..State::default()
            }
        });
        let before = tool;
        let mut c = RawCall::new(Conv::Thiscall, fc4p_006269c0 as *const () as usize, t as u32, &[]);
        c.eax = 0x1234_5678;
        c.edx = 0x9ABC_DEF0;
        let cw = crate::x87::control_word();
        let top = crate::x87::status_word() & 0x3800;
        let (mut mxcsr_before, mut mxcsr_after) = (0u32, 0u32);
        let mut canary = F80::ZERO;
        unsafe {
            core::arch::asm!("stmxcsr [{p}]", "fld1", p = in(reg) &mut mxcsr_before, out("st(0)") _);
            call_raw(&mut c);
            core::arch::asm!("fstp tbyte ptr [{value}]", "stmxcsr [{p}]",
                value = in(reg) &mut canary, p = in(reg) &mut mxcsr_after, out("st(0)") _);
        }
        assert_eq!(canary.to_bytes(), F80::from_i32(1).to_bytes());
        assert_eq!(crate::x87::control_word(), cw);
        assert_eq!(crate::x87::status_word() & 0x3800, top);
        assert_eq!(mxcsr_after, mxcsr_before);
        assert_eq!(
            (c.eax, c.edx),
            if optional {
                (0xAABB_CC01, 0xEEFF_1122)
            } else {
                (0x1234_5601, 0x9ABC_DEF0)
            }
        );
        assert_eq!(initialized, 1);
        let layer_mask = if mask & 0x1F4F != 0 { 0x1F4F } else { mask };
        assert_eq!((tool[0x360 / 4], filter[3]), (layer_mask, layer_mask));
        assert_eq!(
            (tool[0x240 / 4], tool[0x244 / 4], tool[0x248 / 4], tool[0x24C / 4]),
            (width_value, 40, width_value.wrapping_add(1), 41)
        );
        let stride = width_value.wrapping_add(1);
        let diagonal = stride.wrapping_add(1);
        assert_eq!(
            &tool[0x250 / 4..0x290 / 4],
            &[diagonal, stride, stride, 0, 0, 1, 1, diagonal, 1, 1, diagonal, diagonal, stride, stride, 0, 0]
        );
        assert_eq!(unsafe { read::<u8>(t, 0x354) }, u8::from(bridge_len != 0));
        assert_eq!(
            &unsafe { core::slice::from_raw_parts(t.add(0x355), 3) },
            &&[0xA5; 3][..]
        );
        assert_eq!(tool[0xFC / 4], if optional { service[5] } else { 0 });
        assert_eq!(tool[0x238 / 4], if optional { 16f32.to_bits() } else { 4f32.to_bits() });
        let expected_slope = (F80::from_f32(0.25).tan() * F80::from_f32(if optional { 16. } else { 4. })).to_f32();
        assert_eq!(tool[0x23C / 4], expected_slope.to_bits());
        assert_eq!(
            (tool[0x358 / 4], tool[0x364 / 4], tool[0x1F0 / 4]),
            (info as u32, fp as u32, pp as u32)
        );
        for offset in [0x50, 0x70, 0x74, 0x78, 0x98, 0x9C, 0x208, 0x20C, 0x220] {
            assert_eq!(tool[offset / 4], before[offset / 4]);
        }
        STATE.with(|s| {
            let s = s.borrow();
            let tags: Vec<u32> = s.calls.iter().map(|e| e.0).collect();
            let mut expected = if type_id == 4 {
                vec![3, 4, 5]
            } else {
                vec![1, 2, 3, 4, 5]
            };
            if !same_preview {
                expected.extend([6, 7]);
            }
            expected.extend([8, 9, 4, 10, 6, 11, 12, 13, 14]);
            if optional {
                expected.extend([6, 6]);
            }
            expected.push(15);
            if optional {
                expected.push(6);
            }
            expected.extend([16, 17, 18, 6, 19, 20, 7]);
            if optional {
                expected.push(21);
            }
            assert_eq!(tags, expected);
        });
        STATE.with(|s| s.borrow_mut().calls.clear());
        let expected = tool;
        c.eax = 0xABCD_EF99;
        c.edx = 0x8765_4321;
        unsafe {
            call_raw(&mut c);
        }
        assert_eq!((c.eax, c.edx), (0xABCD_EF01, 0x8765_4321));
        assert_eq!(tool, expected);
        STATE.with(|s| assert!(s.borrow().calls.is_empty()));
    }
}

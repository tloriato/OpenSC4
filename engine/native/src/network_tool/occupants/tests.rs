// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Class(usize),
    Id(usize),
    Resource(usize, [u32; 3], u32, usize, u32, u32),
    Query(usize, u32),
    Release(usize),
    Exemplar(usize, u32, u32),
    Model(usize, usize, u32, u32),
    Cell(usize, u32),
    Manager(usize, u32, u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    class: u32,
    id: usize,
    success: u32,
    output: Option<usize>,
    exemplar: usize,
    cells: Vec<usize>,
    manager_success: u32,
    manager_output: usize,
    query_success: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn class(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Class(this as usize));
        registers(s.class, 0x8765_4321)
    })
}
extern "thiscall" fn id(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Id(this as usize));
        registers(s.id as u32, 0x1234_5678)
    })
}
extern "thiscall" fn resource(this: *mut u8, tgi: *const u32, iid: u32, output: *mut *mut u8, a: u32, b: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Resource(
            this as usize,
            unsafe { [*tgi, *tgi.add(1), *tgi.add(2)] },
            iid,
            unsafe { *output } as usize,
            a,
            b,
        ));
        if let Some(value) = s.output {
            unsafe {
                *output = value as *mut u8;
            }
        }
        s.success
    })
}
extern "thiscall" fn query(this: *mut u8, iid: u32, output: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Query(this as usize, iid));
        if let Some(value) = s.output {
            unsafe {
                *output = value as *mut u8;
            }
        }
        registers(s.query_success, 0x1020_3040)
    })
}
extern "stdcall" fn release_log(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(this as usize)));
}
#[unsafe(naked)]
unsafe extern "thiscall" fn release(_this: *mut u8) -> u64 {
    core::arch::naked_asm!("push ecx", "call {log}", "mov eax, 0xaabbccdd", "mov edx, 0x12345678", "mov ecx, 0x87654321", "ret", log = sym release_log)
}
extern "thiscall" fn exemplar(this: *mut u8, instance: u32, group: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Exemplar(this as usize, instance, group));
        s.exemplar as *mut u8
    })
}
extern "thiscall" fn model(this: *mut u8, exemplar: *mut u8, a: u32, b: u32) {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Model(this as usize, exemplar as usize, a, b))
    });
}
extern "thiscall" fn cell(this: *mut u8, key: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Cell(this as usize, key));
        registers(s.cells.remove(0) as u32, 0xFACE_1234)
    })
}
extern "thiscall" fn manager(this: *mut u8, output: *mut *mut u8, x: u32, z: u32, flags: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Manager(this as usize, x, z, flags));
        assert!(unsafe { *output }.is_null());
        unsafe {
            *output = s.manager_output as *mut u8;
        }
        registers(s.manager_success, 0x1020_3040)
    })
}
fn invoke(function: usize, this: *mut u8, args: &[u32]) -> Registers {
    unsafe {
        call(
            function,
            args,
            Registers {
                eax: 0xA1B2_C3D4,
                ecx: this as u32,
                edx: 0xB1C2_D3E4,
            },
        )
    }
}
#[test]
fn id_callback_checks_class_writes_one_word_and_keeps_low_byte_success() {
    let mut table = [0usize; 16];
    table[4] = class as *const () as usize;
    table[15] = id as *const () as usize;
    let mut object = [table.as_ptr() as usize];
    let p = object.as_mut_ptr().cast::<u8>();
    let mut identifier = 0x1234_56ABu32;
    for class in [0, 0xA6B7_9602, 0xA6B7_9603] {
        let mut tool = [0xA5u8; 0x100];
        let t = tool.as_mut_ptr();
        let mut expected = tool;
        STATE.with(|s| {
            *s.borrow_mut() = State {
                class,
                id: &mut identifier as *mut u32 as usize,
                ..State::default()
            }
        });
        let result = invoke(fc4p_00624480 as *const () as usize, t, &[p as u32]);
        if class == 0xA6B7_9602 {
            expected[0xF8..0xFC].copy_from_slice(&identifier.to_le_bytes());
            assert_eq!((result.eax, result.edx), (0x1234_5601, 0x1234_5678));
            assert_eq!(calls(), [Call::Class(p as usize), Call::Id(p as usize)]);
        } else {
            assert_eq!((result.eax, result.edx), ((class & !255) | 1, 0x8765_4321));
            assert_eq!(calls(), [Call::Class(p as usize)]);
        }
        assert_eq!(tool, expected);
    }
}
#[test]
fn exemplar_lookup_uses_active_group_only_for_zero_and_low_success_byte() {
    let table = [0usize, 0, 0, resource as *const () as usize];
    let mut resource = [table.as_ptr() as usize];
    let mut global = resource.as_mut_ptr().cast::<u8>();
    let _g = fake_data(0x00B4_3CA8, (&mut global as *mut *mut u8).cast());
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    let mut info = [0u32; 4];
    info[3] = 0xCDEF_1234;
    unsafe {
        NetworkTool::from_ptr(p).set_type_info(info.as_mut_ptr().cast());
    }
    for group in [0u32, 0xABCD_9876] {
        for success in [0u32, 0xABCD_0000, 0xABCD_0080] {
            for output in [None, Some(0), Some(0x1234_5678)] {
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        success,
                        output,
                        ..State::default()
                    }
                });
                assert_eq!(
                    unsafe { fc4p_006244b0(p, 0x1020_3040, group) } as usize,
                    if success as u8 != 0 {
                        output.unwrap_or(group as usize)
                    } else {
                        0
                    }
                );
                assert_eq!(
                    calls(),
                    [Call::Resource(
                        global as usize,
                        [0x6534_284A, if group == 0 { info[3] } else { group }, 0x1020_3040],
                        0xA521_60F5,
                        group as usize,
                        0,
                        0
                    )]
                );
            }
        }
    }
}
#[test]
fn model_assignment_short_circuits_and_releases_exemplar_before_interface() {
    let _a = fake(0x0062_44B0, exemplar as *const () as usize);
    let _b = fake(0x0061_D120, model as *const () as usize);
    let table = [query as *const () as usize, 0, release as *const () as usize];
    let mut object = [table.as_ptr() as usize];
    let mut interface = [table.as_ptr() as usize];
    let mut resource = [table.as_ptr() as usize];
    let o = object.as_mut_ptr().cast::<u8>();
    let i = interface.as_mut_ptr().cast::<u8>();
    let e = resource.as_mut_ptr().cast::<u8>();
    for success in [0u32, 0xABCD_0000, 0xABCD_0001] {
        for present in [false, true] {
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    query_success: success,
                    output: Some(i as usize),
                    exemplar: if present { e as usize } else { 0 },
                    ..State::default()
                }
            });
            unsafe {
                fc4p_00624860(0x8765_4321 as *mut u8, o, 17, 19, 23, 29);
            }
            let mut expected = vec![Call::Query(o as usize, 0x49C1_A034)];
            if success as u8 != 0 {
                expected.push(Call::Exemplar(0x8765_4321, 17, 29));
                if present {
                    expected.push(Call::Model(i as usize, e as usize, 19, 23));
                    expected.push(Call::Release(e as usize));
                }
            }
            expected.push(Call::Release(i as usize));
            assert_eq!(calls(), expected);
        }
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            output: Some(0),
            ..State::default()
        }
    });
    unsafe {
        fc4p_00624860(core::ptr::null_mut(), o, 0, 0, 0, 0);
    }
    assert_eq!(calls(), [Call::Query(o as usize, 0x49C1_A034)]);
}
#[test]
fn adjacent_probes_check_lower_before_upper_apply_mask_and_stop_on_any_low_thirteen_bit() {
    let _f = fake(0x0063_3220, cell as *const () as usize);
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    t.set_width(64);
    t.set_height(64);
    for along_z in [false, true] {
        for flags_a in [0u32, 1, 0x2000, 0x8000_0004] {
            for flags_b in [0u32, 1, 0x2000, 0x8000_0004] {
                for mask in [0u32, 1, 0x1FFF] {
                    let mut a = [0u32, 0, flags_a];
                    let mut b = [0u32, 0, flags_b];
                    let ap = a.as_mut_ptr() as usize;
                    let bp = b.as_mut_ptr() as usize;
                    STATE.with(|s| {
                        *s.borrow_mut() = State {
                            cells: vec![ap, bp],
                            ..State::default()
                        }
                    });
                    let result = invoke(
                        if along_z {
                            fc4p_00624590 as *const () as usize
                        } else {
                            fc4p_00624510 as *const () as usize
                        },
                        p,
                        &[7, 9, mask],
                    );
                    let found_a = flags_a & !mask & 0x1FFF != 0;
                    let found_b = flags_b & !mask & 0x1FFF != 0;
                    assert_eq!(result.eax as u8 != 0, found_a || found_b);
                    let lower = if along_z { (8 << 16) + 7 } else { (9 << 16) + 6 };
                    let upper = if along_z { (10 << 16) + 7 } else { (9 << 16) + 8 };
                    let mut expected = vec![Call::Cell(p as usize, lower)];
                    if !found_a {
                        expected.push(Call::Cell(p as usize, upper));
                    }
                    assert_eq!(calls(), expected);
                    assert_eq!(
                        result.edx,
                        if found_a {
                            if along_z {
                                flags_a & !mask
                            } else {
                                !mask
                            }
                        } else {
                            0xFACE_1234
                        }
                    );
                }
            }
        }
    }
}
#[test]
fn adjacent_probes_unsigned_edges_wrapped_coordinates_null_cells_and_no_call_registers() {
    let _f = fake(0x0063_3220, cell as *const () as usize);
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    t.set_width(1);
    t.set_height(1);
    STATE.with(|s| *s.borrow_mut() = State::default());
    let x = invoke(fc4p_00624510 as *const () as usize, p, &[0, 0, 0]);
    let z = invoke(fc4p_00624590 as *const () as usize, p, &[0, 0, 0]);
    assert_eq!((x.eax, x.edx), (0xA1B2_C300, 0xB1C2_D3E4));
    assert_eq!((z.eax, z.edx), (0, 0xB1C2_D3E4));
    assert!(calls().is_empty());
    t.set_width(0);
    t.set_height(0);
    for along_z in [false, true] {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                cells: vec![0, 0],
                ..State::default()
            }
        });
        let result = invoke(
            if along_z {
                fc4p_00624590 as *const () as usize
            } else {
                fc4p_00624510 as *const () as usize
            },
            p,
            &[u32::MAX, u32::MAX, 0],
        );
        assert_eq!(result.eax as u8, 0);
        assert_eq!(
            calls(),
            [Call::Cell(p as usize, if along_z { 0xFFFD_FFFF } else { 0xFFFE_FFFE })]
        );
    }
}
#[test]
fn occupant_query_unsigned_bounds_do_not_call_services_or_touch_output() {
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    t.set_width(64);
    t.set_height(64);
    let mut out = 0xDEAD_BEEFu32;
    for (x, z) in [(64, 0), (u32::MAX, 0), (0, 64), (0, u32::MAX)] {
        let result = invoke(
            fc4p_00624b70 as *const () as usize,
            p,
            &[x, z, 0x1234, &mut out as *mut u32 as u32],
        );
        assert_eq!(
            (result.eax, result.ecx, result.edx),
            (if x >= 64 { 0xA1B2_C300 } else { z & !255 }, p as u32, x)
        );
        assert_eq!(out, 0xDEAD_BEEF);
    }
}
#[test]
fn occupant_query_releases_failed_manager_outputs_and_restores_query_low_byte_after_release() {
    let mut table = [0usize; 33];
    table[0] = query as *const () as usize;
    table[2] = release as *const () as usize;
    table[32] = manager as *const () as usize;
    let mut manager = [table.as_ptr() as usize];
    let mut occupant = [table.as_ptr() as usize];
    let m = manager.as_mut_ptr().cast::<u8>();
    let o = occupant.as_mut_ptr().cast::<u8>();
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    t.set_width(64);
    t.set_height(64);
    unsafe {
        write(p, 0x48, m);
    }
    for manager_success in [0u32, 0xABCD_0000, 0xABCD_0001] {
        for present in [false, true] {
            for query_success in [0xBBCC_DD00u32, 0xBBCC_DD80] {
                if manager_success as u8 != 0 && !present {
                    continue;
                }
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        manager_success,
                        manager_output: if present { o as usize } else { 0 },
                        query_success,
                        output: Some(0x1234_5678),
                        ..State::default()
                    }
                });
                let mut output = 0xDEAD_BEEFu32;
                let result = invoke(
                    fc4p_00624b70 as *const () as usize,
                    p,
                    &[7, 9, 0x8123, &mut output as *mut u32 as u32],
                );
                let mut expected = vec![Call::Manager(m as usize, 7, 9, 0x8123)];
                if manager_success as u8 != 0 {
                    expected.push(Call::Query(o as usize, 0xA821_EF94));
                }
                if present {
                    expected.push(Call::Release(o as usize));
                }
                assert_eq!(calls(), expected);
                assert_eq!(
                    result.eax,
                    if present {
                        0xAABB_CC00
                            | if manager_success as u8 != 0 {
                                query_success & 255
                            } else {
                                0
                            }
                    } else {
                        manager_success & !255
                    }
                );
                assert_eq!(result.edx, if present { 0x1234_5678 } else { 0x1020_3040 });
                assert_eq!(result.ecx, if present { 0x8765_4321 } else { 0 });
                assert_eq!(
                    output,
                    if manager_success as u8 != 0 {
                        0x1234_5678
                    } else {
                        0xDEAD_BEEF
                    }
                );
            }
        }
    }
}

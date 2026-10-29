// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::queue::QueueIterator;
use crate::lot_developer::updates::*;

slot!(clear_flora, rectangle, flags);

unsafe extern "thiscall" fn remove_zot_prop(this: *mut u8, lot: u32, kind: u32) -> u64 {
    run(this, "remove_zot_prop", &[lot, kind])
}

unsafe extern "thiscall" fn remove_network_nodes(this: *mut u8, lot: u32) -> u64 {
    run(this, "remove_network_nodes", &[lot])
}

unsafe extern "thiscall" fn begin_demolition(this: *mut u8, lot: u32, state: u32, immediate: u32, reason: u32) -> u64 {
    run(this, "begin_demolition", &[lot, state, immediate, reason])
}

#[test]
fn zot_removal_finds_existing_property_removes_nonzero_prop_and_clears_even_zero() {
    let _remove = fake(0x006C_9780, remove_zot_prop as *const () as usize);
    for current in [None, Some(0u32), Some(4), Some(u32::MAX)] {
        let mut developer = Object::new(0x108);
        let mut lot = mock!(0x0C => get_holder);
        let mut holder = mock!(0x24 => get_property);
        let mut property = mock!(0x14 => get_variant);
        let mut variant = mock!(0x124 => get_data, 0x68 => set_uint);
        let mut value = current.unwrap_or(0);
        expect(lot.ptr(), "get_holder", vec![], holder.ptr() as u64);
        expect(
            holder.ptr(),
            "get_property",
            vec![w(0xC999_C45E)],
            if current.is_some() {
                property.ptr() as u64
            } else {
                RETURN & !0xFFFF_FFFF
            },
        );
        if let Some(current) = current {
            expect(property.ptr(), "get_variant", vec![], variant.ptr() as u64);
            expect(variant.ptr(), "get_data", vec![], &mut value as *mut u32 as u64);
            if current != 0 {
                expect(
                    developer.ptr(),
                    "remove_zot_prop",
                    vec![p(lot.ptr()), w(current)],
                    RETURN,
                );
            }
            expect(variant.ptr(), "set_uint", vec![w(0)], RETURN);
        }
        let returned = unsafe { fc4p_006cb940(developer.ptr(), 0xABCD, lot.ptr()) };
        assert_eq!(
            returned,
            if current.is_some() {
                RETURN
            } else {
                RETURN & !0xFFFF_FFFF
            }
        );
        assert_eq!(value, current.unwrap_or(0));
        finished();
    }
}

#[test]
fn demolition_start_checks_only_foundation_bit_and_orders_flora_props_zot_nodes_and_clear() {
    let _nodes = fake(0x006C_9FB0, remove_network_nodes as *const () as usize);
    let _clear = fake(0x006C_3C50, begin_demolition as *const () as usize);
    for flags in [0u32, 1, 2, 0x100, 0x101, u32::MAX] {
        for has_property in [false, true] {
            let mut developer = Object::new(0x108);
            let mut lot = mock!(0x30 => get_flags, 0x38 => get_rect, 0x0C => get_holder);
            let mut flora = mock!(0x50 => clear_flora);
            let mut props = mock!(0x70 => remove_props);
            let mut replacement = mock!(0x70 => remove_props);
            let mut holder = mock!(0x24 => get_property);
            let mut property = mock!(0x14 => get_variant);
            let mut variant = mock!(0x68 => set_uint);
            unsafe {
                UpdateState::from_ptr(developer.ptr()).set_flora(flora.ptr());
                LotDeveloper::from_ptr(developer.ptr()).set_prop_manager(props.ptr());
            }
            expect(lot.ptr(), "get_flags", vec![], u64::from(flags));
            if flags & 1 != 0 {
                expect(lot.ptr(), "get_rect", vec![out(&[0xFFFF_FFFE, 3, 8, 9])], RETURN);
                expect(
                    flora.ptr(),
                    "clear_flora",
                    vec![Arg::In(vec![0xFFFF_FFFE, 3, 8, 9]), w(0)],
                    RETURN,
                );
                edit_last(unsafe { developer.ptr().add(0x30).cast() }, replacement.ptr() as u32);
            }
            expect(
                if flags & 1 != 0 { replacement.ptr() } else { props.ptr() },
                "remove_props",
                vec![p(lot.ptr())],
                RETURN,
            );
            expect(lot.ptr(), "get_holder", vec![], holder.ptr() as u64);
            expect(
                holder.ptr(),
                "get_property",
                vec![w(0xC999_C45E)],
                if has_property { property.ptr() as u64 } else { 0 },
            );
            if has_property {
                expect(property.ptr(), "get_variant", vec![], variant.ptr() as u64);
                expect(variant.ptr(), "set_uint", vec![w(0)], RETURN);
            }
            expect(developer.ptr(), "remove_network_nodes", vec![p(lot.ptr())], RETURN);
            expect(
                developer.ptr(),
                "begin_demolition",
                vec![p(lot.ptr()), w(5), w(1), w(0)],
                RETURN,
            );
            assert_eq!(unsafe { fc4p_006cc1d0(developer.ptr(), 0, lot.ptr()) }, RETURN);
            finished();
        }
    }
}

slot!(get_rect, output);

thread_local! {
    static LIGHT_CALLS: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "thiscall" fn light(this: *mut u8, building: u32, enabled: u32) -> u64 {
    unsafe {
        LIGHT_CALLS.with(|calls| {
            calls.borrow_mut().push((
                this.add(0x8C).cast::<u32>().read_unaligned(),
                UpdateState::from_ptr(this).lighting_mode(),
            ))
        });
    }
    run(this, "light", &[building, enabled])
}

/// Calls with a real stack word and records its contents after the callee pops it.
unsafe fn call_lighting(function: usize, this: *mut u8, argument: u32) -> [u32; 4] {
    let mut frame = [this as u32, function as u32, argument, 0x1234_ABCD, 0, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 8]", "mov ecx, dword ptr [edi]", "mov edx, dword ptr [edi + 12]",
            "call dword ptr [edi + 4]", "mov dword ptr [edi + 16], eax", "mov dword ptr [edi + 20], edx",
            "mov dword ptr [edi + 28], ecx", "mov eax, dword ptr [esp - 4]", "mov dword ptr [edi + 24], eax",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"),
        );
    }
    frame[4..8].try_into().unwrap()
}

fn take_lights() -> Vec<(u32, u32)> {
    LIGHT_CALLS.with(|calls| core::mem::take(&mut *calls.borrow_mut()))
}

struct Lighting {
    developer: Object,
    first: Box<[u32; 32]>,
    second: Box<[u32; 32]>,
    map: Box<[*mut u32; 2]>,
}

impl Lighting {
    fn new() -> Self {
        let mut first = Box::new([0u32; 32]);
        let mut second = Box::new([0u32; 32]);
        first[31] = 0x1111;
        second[0] = 0x2222;
        let map = Box::new([first.as_mut_ptr(), second.as_mut_ptr()]);
        let mut world = Self {
            developer: Object::new(0x108),
            first,
            second,
            map,
        };
        unsafe {
            let begin = QueueIterator {
                current: world.first.as_mut_ptr().add(31),
                first: world.first.as_mut_ptr(),
                last: world.first.as_mut_ptr().add(32),
                node: world.map.as_mut_ptr(),
            };
            let end = QueueIterator {
                current: world.second.as_mut_ptr().add(1),
                first: world.second.as_mut_ptr(),
                last: world.second.as_mut_ptr().add(32),
                node: world.map.as_mut_ptr().add(1),
            };
            let developer = LotDeveloper::from_ptr(world.developer.ptr());
            developer.set_night_begin(begin);
            developer.set_night_cursor(begin);
            developer.set_night_end(end);
            UpdateState::from_ptr(world.developer.ptr()).set_lighting_mode(2);
        }
        take_lights();
        world
    }
}

#[test]
fn lighting_batch_threshold_wrap_zero_quota_and_empty_queue_match_core_cases() {
    for (initial, tick, quota, elapsed, mode) in [
        (100, 150, 10, 0, 0),
        (0, 99, 10, 99, 2),
        (0, 200, 10, 200, 2),
        (u32::MAX, 1, 10, 0, 2),
        (100, 150, 0, 0, 2),
    ] {
        let mut world = Lighting::new();
        let developer = unsafe { LotDeveloper::from_ptr(world.developer.ptr()) };
        developer.set_night_cursor(developer.night_end());
        let state = unsafe { UpdateState::from_ptr(world.developer.ptr()) };
        state.set_lighting_elapsed(initial);
        state.set_lighting_batch(quota);
        let returned = unsafe { call_lighting(fc4p_006cbd60 as *const () as usize, world.developer.ptr(), tick) };
        assert_eq!(state.lighting_elapsed(), elapsed);
        assert_eq!(state.lighting_mode(), mode);
        assert_eq!(
            returned[2],
            if initial.wrapping_add(tick) > 200 {
                (tick & !255) | 1
            } else {
                tick
            }
        );
        if initial.wrapping_add(tick) <= 200 || quota == 0 {
            assert_eq!(&returned[..2], &[initial.wrapping_add(tick), 0x1234_ABCD]);
        } else {
            assert_eq!(
                &returned[..2],
                &[developer.night_end().current as u32, developer.night_end().node as u32]
            );
        }
        finished();
    }
}

#[test]
fn lighting_batch_advances_before_callbacks_crosses_blocks_and_stops_at_fixed_quota() {
    let _light = fake(0x006C_8E10, light as *const () as usize);
    let mut world = Lighting::new();
    let state = unsafe { UpdateState::from_ptr(world.developer.ptr()) };
    state.set_lighting_batch(2);
    expect(world.developer.ptr(), "light", vec![w(0x1111), w(0x1234_0001)], RETURN);
    edit_last(unsafe { world.developer.ptr().add(0x9C).cast() }, 0x102);
    edit_last(unsafe { world.developer.ptr().add(0xA0).cast() }, 0);
    expect(world.developer.ptr(), "light", vec![w(0x2222), w(0x1234_0001)], RETURN);
    let returned = unsafe { call_lighting(fc4p_006cbd60 as *const () as usize, world.developer.ptr(), 0x1234_0002) };
    assert_eq!(&returned[..3], &[RETURN as u32, (RETURN >> 32) as u32, 0x1234_0001]);
    assert_eq!(
        take_lights(),
        [
            (world.second.as_mut_ptr() as u32, 2),
            (unsafe { world.second.as_mut_ptr().add(1) } as u32, 0x102)
        ]
    );
    assert_eq!(
        state.lighting_mode(),
        0x102,
        "reaching the end exactly does not complete the mode"
    );
    finished();
}

#[test]
fn full_lighting_updates_before_advance_rereads_end_and_restores_live_begin_and_ecx() {
    let _light = fake(0x006C_8E10, light as *const () as usize);
    for mode in [2u32, 0x102, 0x1234_5602] {
        let mut world = Lighting::new();
        let developer = unsafe { LotDeveloper::from_ptr(world.developer.ptr()) };
        developer.set_night_cursor(developer.night_end());
        let flags = (mode & !255) | u32::from(mode == 2);
        expect(world.developer.ptr(), "light", vec![w(0x1111), w(flags)], RETURN);
        // End changes during the first callback, so only one building is visited. The new begin
        // is also the final cursor, even though it differs from the begin captured on entry.
        edit_last(
            unsafe { world.developer.ptr().add(0x74).cast() },
            world.second.as_mut_ptr() as u32,
        );
        edit_last(unsafe { world.developer.ptr().add(0x64).cast() }, 0x3333);
        let returned = unsafe { call_lighting(fc4p_006cbe50 as *const () as usize, world.developer.ptr(), mode) };
        let begin = developer.night_begin();
        assert_eq!(
            returned,
            [begin.first as u32, begin.node as u32, flags, begin.last as u32]
        );
        assert_eq!(take_lights(), [(unsafe { world.first.as_mut_ptr().add(31) } as u32, 2)]);
        assert_eq!(developer.night_cursor(), begin);
        assert_eq!(
            unsafe { UpdateState::from_ptr(world.developer.ptr()).lighting_mode() },
            0
        );
        finished();
    }
}

#[test]
fn full_lighting_covers_empty_and_multiple_block_queues() {
    let _light = fake(0x006C_8E10, light as *const () as usize);
    for empty in [false, true] {
        let mut world = Lighting::new();
        let developer = unsafe { LotDeveloper::from_ptr(world.developer.ptr()) };
        let begin = developer.night_begin();
        if empty {
            developer.set_night_end(begin);
        } else {
            expect(world.developer.ptr(), "light", vec![w(0x1111), w(1)], RETURN);
            expect(world.developer.ptr(), "light", vec![w(0x2222), w(1)], RETURN);
        }
        let returned = unsafe { call_lighting(fc4p_006cbe50 as *const () as usize, world.developer.ptr(), 2) };
        assert_eq!(returned, [begin.first as u32, begin.node as u32, 1, begin.last as u32]);
        assert_eq!(developer.night_cursor(), begin);
        let calls = take_lights();
        if empty {
            assert!(calls.is_empty());
        } else {
            assert_eq!(
                calls,
                [(begin.current as u32, 2), (world.second.as_mut_ptr() as u32, 2)]
            );
        }
        finished();
    }
}

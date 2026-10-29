// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake};
use std::cell::{Cell, RefCell};

#[derive(Debug, PartialEq)]
enum Call {
    BaseConstruct(usize),
    Initialize(usize, u32),
    CopyMap(usize, usize),
    Clear(usize),
    Free(usize),
    BaseDestroy(usize),
    ReleasePage(usize, usize),
    QueuePage(usize, usize),
    Allocate(usize),
    CopyConstruct(usize, usize),
    AddRef(usize),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static CLEAR_BUCKET: Cell<Option<usize>> = const { Cell::new(None) };
    static ALLOCATION: Cell<usize> = const { Cell::new(0) };
    static NEXT_PAGE: Cell<usize> = const { Cell::new(0) };
    static PAGE_GLOBAL: Cell<usize> = const { Cell::new(0) };
    static COPY_RESULT: Cell<usize> = const { Cell::new(0) };
    static COPY_VTABLE: Cell<usize> = const { Cell::new(0) };
}

fn log(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}
fn take_calls() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn base_construct(this: *mut u8) {
    log(Call::BaseConstruct(this as usize));
    unsafe {
        assert_eq!(read_unaligned(this.sub(4).cast::<u32>()), 0x00A9_9D78);
        for offset in 4..0x18 {
            write_unaligned(this.add(offset), 0x5Au8);
        }
    }
}

extern "thiscall" fn initialize(map: *mut u8, count: u32) {
    log(Call::Initialize(map as usize, count));
    unsafe {
        let info = PathInfo::from_ptr(map.sub(0x1C));
        assert_eq!(info.vtable(), 0x00A9_9E70);
        assert_eq!(info.resource_vtable(), 0x00A9_9E38);
        assert!(info.buckets().is_null());
        assert!(info.buckets_end().is_null());
        assert!(info.buckets_capacity().is_null());
        assert_eq!(info.path_count(), 0);
        assert_eq!(info.flags(), 0xA5);
        assert_eq!(read_unaligned(map.cast::<u32>()), 0xA5A5_A5A5);
        info.set_buckets(0x1000 as *mut *mut u8);
        info.set_buckets_end(0x1194 as *mut *mut u8);
        info.set_buckets_capacity(0x1194 as *mut *mut u8);
    }
}

extern "thiscall" fn copy_map(map: *mut u8, source: *mut u8) {
    log(Call::CopyMap(map as usize, source as usize));
    unsafe {
        let info = PathInfo::from_ptr(map.sub(0x1C));
        assert_eq!(info.vtable(), 0x00A9_9E70);
        assert_eq!(info.resource_vtable(), 0x00A9_9E38);
        assert!(info.buckets().is_null());
        assert_eq!(info.path_count(), 0);
        info.set_path_count(7);
        PathInfo::from_ptr(source.sub(0x1C)).set_flags(0xD3);
        write_unaligned(source.add(0x15), 0x7Eu8);
    }
}

extern "thiscall" fn clear(map: *mut u8) -> u64 {
    log(Call::Clear(map as usize));
    unsafe {
        let info = PathInfo::from_ptr(map.sub(0x1C));
        assert_eq!(info.vtable(), 0x00A9_9E70);
        assert_eq!(info.resource_vtable(), 0x00A9_9E38);
        CLEAR_BUCKET.with(|p| {
            if let Some(pointer) = p.take() {
                info.set_buckets(pointer as *mut *mut u8);
            }
        });
    }
    0x1234_5678_AABB_CCDD
}
extern "C" fn fake_free(pointer: *mut u8) -> u64 {
    log(Call::Free(pointer as usize));
    0x2345_6789_BBCC_DDEE
}
extern "fastcall" fn base_destroy(this: *mut u8, edx: u32) -> u64 {
    log(Call::BaseDestroy(this as usize));
    unsafe { write_unaligned(this.cast::<u32>(), 0x1122_3344) };
    u64::from(edx) << 32
}
extern "thiscall" fn release_page(this: *mut u8, page: *mut u8) -> u64 {
    log(Call::ReleasePage(this as usize, page as usize));
    0x3456_789A_0000_0000
}
extern "thiscall" fn queue_page(this: *mut u8, page: *mut u8) -> u64 {
    log(Call::QueuePage(this as usize, page as usize));
    0x4567_89AB_0000_0000
}
extern "thiscall" fn allocate(this: *mut u8) -> *mut u8 {
    log(Call::Allocate(this as usize));
    PAGE_GLOBAL.with(|p| {
        NEXT_PAGE.with(|n| unsafe {
            write_unaligned(p.get() as *mut usize, n.get());
        })
    });
    ALLOCATION.with(|p| p.get() as *mut u8)
}
extern "thiscall" fn copy_construct(this: *mut u8, source: *mut u8) -> u64 {
    log(Call::CopyConstruct(this as usize, source as usize));
    let result = COPY_RESULT.with(|p| p.get() as *mut u8);
    unsafe {
        assert_eq!(read_unaligned(this.cast::<u32>()), 0x1234_5678);
        write_unaligned(result.cast::<usize>(), COPY_VTABLE.with(Cell::get));
    }
    0xFEDC_BA98_0000_0000 | u64::from(result as u32)
}
extern "thiscall" fn add_ref(this: *mut u8) -> u64 {
    log(Call::AddRef(this as usize));
    0x9876_5432_7654_3210
}

fn fakes() -> Vec<Fake> {
    take_calls();
    CLEAR_BUCKET.with(|p| p.set(None));
    [
        (0x0097_5633, base_construct as *const () as usize),
        (0x0082_9BC0, initialize as *const () as usize),
        (0x0054_0160, copy_map as *const () as usize),
        (0x0058_4660, clear as *const () as usize),
        (0x0090_CF63, fake_free as *const () as usize),
        (0x0097_538E, base_destroy as *const () as usize),
        (0x0090_D031, release_page as *const () as usize),
        (0x0090_CFF9, queue_page as *const () as usize),
        (0x0090_CF9A, allocate as *const () as usize),
        (0x0054_08B0, copy_construct as *const () as usize),
    ]
    .into_iter()
    .map(|(address, function)| fake(address, function))
    .collect()
}

#[test]
fn constructor_initializes_only_owned_fields_in_call_order() {
    let _fakes = fakes();
    let mut object = [0xA5u8; 0x40];
    let this = object.as_mut_ptr();
    assert_eq!(unsafe { fc4p_005407d0(this) }, this);
    assert_eq!(
        take_calls(),
        [
            Call::BaseConstruct(this as usize + 4),
            Call::Initialize(this as usize + 0x1C, 100)
        ]
    );
    let mut expected = [0xA5u8; 0x40];
    expected[0..4].copy_from_slice(&0x00A9_9E70u32.to_le_bytes());
    expected[4..8].copy_from_slice(&0x00A9_9E38u32.to_le_bytes());
    expected[8..0x1C].fill(0x5A);
    for (offset, value) in [(0x20, 0x1000u32), (0x24, 0x1194), (0x28, 0x1194), (0x2C, 0)] {
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    expected[0x30..0x39].fill(0);
    assert_eq!(object, expected);
}

#[test]
fn copy_constructor_reads_flags_after_map_copy_and_keeps_unowned_bytes() {
    let _fakes = fakes();
    let mut object = [0xA5u8; 0x40];
    let mut source = [0xCCu8; 0x40];
    source[0x31..0x39].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let this = object.as_mut_ptr();
    let src = source.as_mut_ptr();
    assert_eq!(unsafe { fc4p_005408b0(this, src) }, this);
    assert_eq!(
        take_calls(),
        [
            Call::BaseConstruct(this as usize + 4),
            Call::CopyMap(this as usize + 0x1C, src as usize + 0x1C)
        ]
    );
    assert_eq!(&object[0x30..0x39], &source[0x30..0x39]);
    assert_eq!(object[0x30], 0xD3);
    assert_eq!(object[0x31], 0x7E);
    assert_eq!(&object[0x39..], &[0xA5; 7]);
    assert_eq!(&object[0x1C..0x20], &[0xA5; 4]);
    assert_eq!(unsafe { PathInfo::from_ptr(this) }.path_count(), 7);
}

#[test]
fn destructor_rereads_bucket_after_clear_and_ignores_higher_flag_bits() {
    let _fakes = fakes();
    for flag in [0, 2, 0x8000_0000, 0xFFFF_FFFE] {
        let mut object = [0xA5u8; 0x40];
        let this = object.as_mut_ptr();
        CLEAR_BUCKET.with(|p| p.set(Some(0x1234_5678)));
        assert_eq!(
            unsafe { fc4p_00540820(this, 0xFEDC_BA98, flag) },
            0x2345_6789_0000_0000 | u64::from(this as u32)
        );
        assert_eq!(
            take_calls(),
            [
                Call::Clear(this as usize + 0x1C),
                Call::Free(0x1234_5678),
                Call::BaseDestroy(this as usize + 4)
            ]
        );
        let mut expected = [0xA5u8; 0x40];
        expected[0..4].copy_from_slice(&0x00A9_9E70u32.to_le_bytes());
        expected[4..8].copy_from_slice(&0x1122_3344u32.to_le_bytes());
        expected[0x20..0x24].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        assert_eq!(object, expected);
    }
    let mut object = [0u8; 0x40];
    let this = object.as_mut_ptr();
    unsafe { fc4p_00540820(this, 0xFEDC_BA98, 0) };
    assert_eq!(
        take_calls(),
        [Call::Clear(this as usize + 0x1C), Call::BaseDestroy(this as usize + 4)]
    );
}

#[repr(C, align(4096))]
struct Page([u8; 4096]);

#[test]
fn destructor_returns_pool_storage_and_handles_page_count_boundaries() {
    let _fakes = fakes();
    let mut pool_storage = 0u8;
    let pool_address = &mut pool_storage as *mut u8 as usize;
    let _pool = fake_data(0x00B0_9FEC, &mut pool_storage);
    for (count, action) in [(1u32, 1), (3, 0), (0x4000_0034, 2), (0, 0)] {
        let mut page = Box::new(Page([0xA5; 4096]));
        let base = page.0.as_mut_ptr();
        let this = unsafe { base.add(0x100) };
        unsafe {
            write_unaligned(base.add(8).cast::<u32>(), 0x7654_3210);
            write_unaligned(base.add(12).cast::<u32>(), count);
            PathInfo::from_ptr(this).set_buckets(core::ptr::null_mut());
        }
        let mut expected = page.0;
        let edx = match action {
            1 => 0x3456_789Au64,
            2 => 0x4567_89AB,
            _ => 0x1234_5678,
        };
        assert_eq!(
            unsafe { fc4p_00540820(this, 0xFEDC_BA98, 0xFFFF_FFFF) },
            (edx << 32) | u64::from(this as u32)
        );
        let mut calls = vec![Call::Clear(this as usize + 0x1C), Call::BaseDestroy(this as usize + 4)];
        if action == 1 {
            calls.push(Call::ReleasePage(pool_address, base as usize));
        }
        if action == 2 {
            calls.push(Call::QueuePage(pool_address, base as usize));
        }
        assert_eq!(take_calls(), calls);
        expected[8..12].copy_from_slice(&(this as u32).to_le_bytes());
        expected[12..16].copy_from_slice(&count.wrapping_sub(1).to_le_bytes());
        expected[0x100..0x104].copy_from_slice(&0x7654_3210u32.to_le_bytes());
        expected[0x104..0x108].copy_from_slice(&0x1122_3344u32.to_le_bytes());
        assert_eq!(page.0, expected);
    }
}

#[test]
fn clone_updates_pool_then_uses_constructor_return_pointer_and_addref_edx() {
    let _fakes = fakes();
    let mut pool_storage = 0u8;
    let pool_address = &mut pool_storage as *mut u8 as usize;
    let _pool = fake_data(0x00B0_9FEC, &mut pool_storage);
    let vtable = [0, add_ref as *const () as usize];
    for grow in [false, true] {
        let mut source = [0xA5u8; 0x40];
        let mut storage = [0u32; 16];
        storage[0] = 0x1234_5678;
        let mut copied = [0u32; 16];
        let storage_pointer = storage.as_mut_ptr().cast::<u8>();
        let result_pointer = copied.as_mut_ptr().cast::<u8>();
        let mut first_page = [0u32; 4];
        let mut second_page = [0u32; 4];
        first_page[2] = if grow { 0 } else { storage_pointer as u32 };
        first_page[3] = 17;
        second_page[3] = u32::MAX;
        let mut current = first_page.as_mut_ptr().cast::<u8>();
        let _current = fake_data(0x00B0_9FF0, (&mut current as *mut *mut u8).cast());
        ALLOCATION.with(|p| p.set(storage_pointer as usize));
        NEXT_PAGE.with(|p| p.set(second_page.as_mut_ptr() as usize));
        PAGE_GLOBAL.with(|p| p.set(&mut current as *mut _ as usize));
        COPY_RESULT.with(|p| p.set(result_pointer as usize));
        COPY_VTABLE.with(|p| p.set(vtable.as_ptr() as usize));
        let result = unsafe { fc4p_00540910(source.as_mut_ptr()) };
        assert_eq!(result, 0x9876_5432_0000_0000 | u64::from(result_pointer as u32));
        let mut calls = Vec::new();
        if grow {
            calls.push(Call::Allocate(pool_address));
            assert_eq!(first_page[2..], [0, 17]);
            assert_eq!(second_page[2..], [0x1234_5678, 0]);
        } else {
            assert_eq!(first_page[2..], [0x1234_5678, 18]);
        }
        calls.push(Call::CopyConstruct(
            storage_pointer as usize,
            source.as_mut_ptr() as usize,
        ));
        calls.push(Call::AddRef(result_pointer as usize));
        assert_eq!(take_calls(), calls);
        assert_eq!(source, [0xA5; 0x40]);
    }
}

#[test]
fn resource_thunks_adjust_this_and_pop_the_original_arguments() {
    let _fakes = fakes();
    let mut object = [0u32; 16];
    let this = object.as_mut_ptr().cast::<u8>();
    let vtable = [0, add_ref as *const () as usize];
    unsafe { PathInfo::from_ptr(this) }.set_vtable(vtable.as_ptr() as u32);
    let mut out = core::ptr::null_mut::<u8>();
    let result = unsafe {
        call_thiscall_with_eax(
            fc4p_005403d0 as *const () as usize,
            this.add(4),
            &[0x456B_8F1D, &mut out as *mut _ as u32],
            0xA5A5_A5A5,
        )
    };
    assert_eq!(out, this.wrapping_add(4));
    assert_eq!(result, (0x7654_3201, 0x9876_5432));
    assert_eq!(take_calls(), [Call::AddRef(this as usize)]);
    let result = unsafe { call_thiscall_with_eax(fc4p_005403e0 as *const () as usize, this.add(4), &[0], 0xA5A5_A5A5) };
    assert_eq!(result.0, this as u32);
    assert_eq!(
        take_calls(),
        [Call::Clear(this as usize + 0x1C), Call::BaseDestroy(this as usize + 4)]
    );
}

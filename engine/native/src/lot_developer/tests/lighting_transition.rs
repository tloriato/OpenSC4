// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::lighting_transition::*;
use crate::lot_developer::queue::QueueIterator;

unsafe extern "thiscall" fn begin(this: *mut u8, output: *mut u32) -> u64 {
    let returned = run(this, "begin", &[output as u32]);
    returned & !0xFFFF_FFFF | output as u64
}
unsafe extern "thiscall" fn end(this: *mut u8, output: *mut u32) -> u64 {
    let returned = run(this, "end", &[output as u32]);
    returned & !0xFFFF_FFFF | output as u64
}
unsafe extern "C" fn shuffle(first: QueueIterator, last: QueueIterator) -> u64 {
    run(
        core::ptr::null_mut(),
        "shuffle",
        &[
            first.current as u32,
            first.first as u32,
            first.last as u32,
            first.node as u32,
            last.current as u32,
            last.first as u32,
            last.last as u32,
            last.node as u32,
        ],
    )
}

/// Builds the continuation's entry frame, then receives the return through that frame's own address.
#[unsafe(naked)]
unsafe extern "C" fn invoke(words: *mut u32) {
    core::arch::naked_asm!(
        "push ebp",
        "push ebx",
        "push esi",
        "push edi",
        "mov ebx, [esp + 20]",
        "push dword ptr [ebx + 8]",
        "lea eax, [2f]",
        "push eax",
        "sub esp, 64",
        "push ebx",
        "push ebp",
        "push esi",
        "push edi",
        "mov edi, [ebx + 4]",
        "jmp dword ptr [ebx]",
        "2:",
        "mov [ebx + 12], eax",
        "mov [ebx + 16], edx",
        "mov eax, [esp - 4]",
        "mov [ebx + 20], eax",
        "pop edi",
        "pop esi",
        "pop ebx",
        "pop ebp",
        "ret",
    );
}

#[test]
fn lighting_continuations_set_mode_shuffle_then_reread_the_queue_beginning() {
    let _begin = fake(0x006C_5FE0, begin as *const () as usize);
    let _end = fake(0x0043_1AC0, end as *const () as usize);
    let _shuffle = fake(0x006C_68C0, shuffle as *const () as usize);
    for (entry, mode) in [(fc4p_006cda13 as *const (), 1u32), (fc4p_006cdad7 as *const (), 2)] {
        let mut object = Object::new(0x108);
        let this = object.ptr();
        let interface = unsafe { this.add(4) };
        let queue = unsafe { interface.add(0x60) };
        let old = [0x1000, 0x1100, 0x1200, 0x1300];
        let last = [0x2000, 0x2100, 0x2200, 0x2300];
        let new = [0x3000, 0x3100, 0x3200, 0x3300];
        expect(queue, "end", vec![out(&last)], RETURN);
        expect(queue, "begin", vec![out(&old)], RETURN);
        expect(
            core::ptr::null_mut(),
            "shuffle",
            old.into_iter().chain(last).map(w).collect(),
            RETURN,
        );
        expect(queue, "begin", vec![out(&new)], RETURN);
        let mut words = [entry as u32, interface as u32, 0x1234_5678, 0, 0, 0];
        unsafe { invoke(words.as_mut_ptr()) };
        assert_eq!(unsafe { interface.add(0x98).cast::<u32>().read_unaligned() }, mode);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(interface.add(0x88).cast::<u32>(), 4) },
            new
        );
        if mode == 1 {
            assert_eq!(&words[3..5], &[0x3301, 0x3200]);
        } else {
            assert_eq!(words[3] as u8, 1);
            assert_eq!(words[4], 0x3300);
        }
        assert_eq!(words[5], 0x1234_5678);
        finished();
    }
}

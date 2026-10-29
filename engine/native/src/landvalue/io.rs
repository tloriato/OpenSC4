// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! `cSC4LandValueSimulator`: saving and loading (006bd8d0 Write, 006bd950 Read).

use super::LandValueSim;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
game_fn! {
    #[allow(dead_code)]
    fn write_serializable(stream: *mut u8, object: *mut u8) -> u32 = "C" 0x005D_99A0;
}
game_fn! {
    fn read_serializable(stream: *mut u8, iid: u32, out: *mut *mut u8, immediate: u32) -> u32 = "C" 0x005D_9730;
}

// The reference writer retains addresses from its frame. Tail forwarding keeps the caller's
// argument words at their original depths, including the words pending from earlier calls.
#[unsafe(naked)]
unsafe extern "C" fn write_reference() {
    core::arch::naked_asm!("call {target}", "jmp eax", target = sym write_reference_target);
}

extern "C" fn write_reference_target() -> usize {
    crate::abi::target(0x005D_99A0)
}

/// Transfers the mean-altitude word, supplying EAX and EDX as well as the stack argument. A stream may
/// leave EDX alone when it cannot transfer the value, and the final error getter may replace only DL.
unsafe fn transfer_mean(stream: *mut u8, word: u32) -> u64 {
    let args = [word];
    // SAFETY: the live stream's mean-altitude slot takes one word (bits for writing, address for reading).
    let mut call = RawCall::new(Conv::Thiscall, unsafe { vslot(stream, 0x30) }, stream as u32, &args);
    call.eax = word;
    // SAFETY: the stream starts with its vtable pointer.
    call.edx = unsafe { core::ptr::read_unaligned(stream.cast::<u32>()) };
    // SAFETY: the stream's thiscall word transfer.
    unsafe { call_raw(&mut call) };
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}

/// Calls the error slot with the register values left by the preceding stream operation. Its EDX is
/// returned alongside the normalized success value in EAX.
unsafe fn finish(stream: *mut u8, eax: u32, edx: u32) -> u64 {
    // SAFETY: the caller supplies the live stream and its error slot takes no arguments.
    let mut call = RawCall::new(Conv::Thiscall, unsafe { vslot(stream, 0x48) }, stream as u32, &[]);
    call.eax = eax;
    call.edx = edx;
    // SAFETY: the stream's thiscall error getter.
    unsafe { call_raw(&mut call) };
    (u64::from(call.edx) << 32) | u64::from(call.eax as u8 == 0)
}

port! {
    /// Saves version 1, intrinsic/total/slope references, dirty-or-running, average, and mean-altitude bits.
    /// `this` is the serializable interface, eight bytes after the primary interface. Intermediate results
    /// are ignored; only the low byte of the final stream error determines success. EDX is retained.
    #[port(address = 0x006B_D8D0, owner = "landvalue", name = "cSC4LandValueSimulator::Write", naked)]
    pub unsafe extern "C" fn fc4p_006bd8d0() {
        // Only call-frame marshaling lives here; the remaining state decisions are in write_fields.
        core::arch::naked_asm!(
            "push esi", "mov esi, [esp + 8]", "push edi", "mov edi, ecx",
            "mov eax, [esi]", "push 1", "mov ecx, esi", "call dword ptr [eax + 0x1c]",
            "push dword ptr [edi + 0x1c]", "push esi", "call {reference}",
            "push dword ptr [edi + 0x20]", "push esi", "call {reference}",
            "push dword ptr [edi + 0x28]", "push esi", "call {reference}",
            "add esp, 24", "mov ecx, edi", "pop edi", "pop esi", "jmp {fields}",
            reference = sym write_reference, fields = sym write_fields,
        )
    }
}

unsafe extern "thiscall" fn write_fields(this: *mut u8, stream: *mut u8) -> u64 {
    // SAFETY: the entry has written the version and references, retaining the original arguments.
    unsafe {
        let lv = LandValueSim::from_ptr(this.sub(8));
        let dirty = u32::from(lv.dirty() != 0 || lv.state() != 9);
        vcall!(stream, 0x14 => fn(u32) -> u32, (stream as u32 & !0xFF) | dirty);
        vcall!(stream, 0x14 => fn(u32) -> u32, u32::from(lv.average()));
        let mean = core::ptr::read_unaligned(this.add(0x70).cast::<u32>());
        let result = transfer_mean(stream, mean);
        finish(stream, result as u32, core::ptr::read_unaligned(stream.cast::<u32>()))
    }
}

port! {
    /// Restores version 1 through deferred grid-reference reads and direct byte/word destinations. Dirty
    /// is normalized, while the pipeline and all other fields stay as they are. A rejected version keeps
    /// the version reader's EDX and upper EAX bytes; a completed read keeps the error getter's EDX.
    /// The hidden EDX parameter uses fastcall to receive that register without changing thiscall's ECX,
    /// stack argument, or four-byte callee pop.
    #[port(address = 0x006B_D950, owner = "landvalue", name = "cSC4LandValueSimulator::Read", naked)]
    pub unsafe extern "C" fn fc4p_006bd950() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push eax", "call {body}", "ret 4",
            body = sym read_fields,
        )
    }
}

unsafe extern "fastcall" fn read_fields(this: *mut u8, entry_edx: u32, stream_slot: *mut u32) -> u64 {
    let stream = unsafe { stream_slot.read() as *mut u8 };
    // SAFETY: the game's serializable interface and live input stream. Destinations are the original
    // fields, so a reference read's effects are visible to every subsequent call.
    unsafe {
        let lv = LandValueSim::from_ptr(this.sub(8));
        let mut version = this as u32;
        let args = [&mut version as *mut u32 as u32];
        let mut call = RawCall::new(Conv::Thiscall, vslot(stream, 0x1C), stream as u32, &args);
        call.eax = core::ptr::read_unaligned(stream.cast::<u32>());
        call.edx = entry_edx;
        call_raw(&mut call);
        if version as u16 != 1 {
            return (u64::from(call.edx) << 32) | u64::from(call.eax & !0xFF);
        }
        read_serializable(stream, 0xC9BD_939A, this.add(0x1C).cast(), 0);
        read_serializable(stream, 0xC9BD_939A, this.add(0x20).cast(), 0);
        read_serializable(stream, 0xC9BD_939A, this.add(0x28).cast(), 0);
        vcall!(stream, 0x14 => fn(*mut u8) -> u32, stream_slot.cast());
        lv.set_dirty(u8::from(stream_slot.cast::<u8>().read() != 0));
        vcall!(stream, 0x14 => fn(*mut u8) -> u32, this.add(0x6C));
        let result = transfer_mean(stream, this.add(0x70) as u32);
        finish(
            stream,
            core::ptr::read_unaligned(stream.cast::<u32>()),
            (result >> 32) as u32,
        )
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006bd8d0::PORT, fc4p_006bd950::PORT];

#[cfg(test)]
mod tests {
    //! Persistence cases from the core's land-value save/load vectors, with objects at the game layout.

    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, Fake, Object};

    const VERSION_RESULT: u64 = 0x1234_5678_AABB_CCDD;
    const ERROR_EDX: u32 = 0xDEAD_BEEF;
    const RECORD: [u8; 32] = [
        1, 0, 0x11, 0, 0, 0, 2, 0xE6, 0xB9, 0x49, 0x12, 0, 0, 0, 2, 0xE6, 0xB9, 0x49, 0x13, 0, 0, 0, 0x0A, 0xE6, 0xB9,
        0x49, 0, 42, 0x9A, 0x99, 0x8F, 0x43,
    ];

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        PutVersion(u32),
        PutRef(usize, usize),
        PutByte(u32),
        PutBits(u32),
        GetVersion,
        GetRef(usize, u32, usize, u32),
        GetByte(Option<usize>),
        GetBits(usize),
        Error,
    }

    #[repr(C)]
    struct Stream {
        vtable: *const usize,
        bytes: Vec<u8>,
        position: usize,
        refs: usize,
        error: u32,
        calls: Vec<Call>,
        lv: *mut u8,
        // Changes made by callees let tests check when the simulator reads its fields.
        change_after_ref: bool,
        dirty_seen_by_average: Option<u8>,
        reference_frames: Vec<usize>,
        dirty_destination: usize,
    }

    impl Stream {
        fn put(&mut self, bytes: &[u8]) {
            self.bytes.extend_from_slice(bytes);
        }

        fn get<const N: usize>(&mut self) -> Option<[u8; N]> {
            let Some(bytes) = self.bytes.get(self.position..self.position + N) else {
                self.error = 1;
                return None;
            };
            let value = bytes.try_into().unwrap();
            self.position += N;
            Some(value)
        }
    }

    extern "thiscall" fn put_version(this: *mut Stream, value: u32) -> u32 {
        // SAFETY: the test's stream.
        let s = unsafe { &mut *this };
        s.calls.push(Call::PutVersion(value));
        s.put(&(value as u16).to_le_bytes());
        0
    }

    #[unsafe(naked)]
    unsafe extern "C" fn put_ref() {
        core::arch::naked_asm!(
            "mov eax, esp", "push eax", "push dword ptr [esp + 12]",
            "push dword ptr [esp + 12]", "call {body}", "add esp, 12", "ret",
            body = sym put_ref_at,
        )
    }

    extern "C" fn put_ref_at(stream: *mut u8, object: *mut u8, frame: usize) -> u32 {
        // SAFETY: the test installs this fake only for its stream.
        let s = unsafe { &mut *stream.cast::<Stream>() };
        s.calls.push(Call::PutRef(stream as usize, object as usize));
        s.reference_frames.push(frame);
        s.put(&(object as u32).to_le_bytes());
        let class: u32 = if s.refs == 2 { 0x49B9_E60A } else { 0x49B9_E602 };
        s.put(&class.to_le_bytes());
        s.refs += 1;
        if s.change_after_ref {
            // SAFETY: the test's simulator; each change must be seen by the next call.
            let lv = unsafe { LandValueSim::from_ptr(s.lv) };
            match s.refs {
                1 => lv.set_total_grid(0x21 as *mut u8),
                2 => lv.set_slope_grid(0x31 as *mut u8),
                3 => {
                    lv.set_dirty(0);
                    lv.set_state(9);
                    lv.set_average(99);
                    lv.set_mean_altitude(f32::from_bits(0x8000_0000));
                }
                _ => unreachable!(),
            }
        }
        0 // Intermediate failures do not stop Write.
    }

    extern "thiscall" fn put_byte(this: *mut Stream, value: u32) -> u32 {
        // SAFETY: the test's stream.
        let s = unsafe { &mut *this };
        s.calls.push(Call::PutByte(value));
        s.put(&[value as u8]);
        0
    }

    extern "thiscall" fn put_bits(this: *mut Stream, value: u32) -> u32 {
        // SAFETY: the test's stream.
        let s = unsafe { &mut *this };
        s.calls.push(Call::PutBits(value));
        s.put(&value.to_le_bytes());
        0
    }

    extern "thiscall" fn get_version(this: *mut Stream, out: *mut u16) -> u64 {
        // SAFETY: the test's stream and the port's writable version destination.
        let s = unsafe { &mut *this };
        s.calls.push(Call::GetVersion);
        if let Some(bytes) = s.get::<2>() {
            unsafe { out.write_unaligned(u16::from_le_bytes(bytes)) };
        }
        VERSION_RESULT
    }

    extern "C" fn get_ref(stream: *mut u8, iid: u32, out: *mut *mut u8, immediate: u32) -> u32 {
        // SAFETY: the test's stream and a simulator pointer field.
        let s = unsafe { &mut *stream.cast::<Stream>() };
        s.calls
            .push(Call::GetRef(stream as usize, iid, out as usize, immediate));
        let token = s
            .get::<8>()
            .map_or(0, |b| u32::from_le_bytes(b[..4].try_into().unwrap()));
        unsafe { out.write_unaligned(token as *mut u8) };
        0 // Intermediate failures do not stop Read.
    }

    extern "thiscall" fn get_byte(this: *mut Stream, out: *mut u8) -> u32 {
        // SAFETY: the test's stream and a writable byte destination.
        let s = unsafe { &mut *this };
        let average = out == s.lv.wrapping_add(0x74);
        s.calls.push(Call::GetByte(average.then_some(out as usize)));
        if !average {
            s.dirty_destination = out as usize;
        }
        if average {
            // SAFETY: the test's simulator; dirty is stored before the average read.
            s.dirty_seen_by_average = Some(unsafe { LandValueSim::from_ptr(s.lv) }.dirty());
        }
        if let Some([value]) = s.get::<1>() {
            unsafe { out.write(value) };
        }
        0
    }

    extern "thiscall" fn get_bits(this: *mut Stream, out: *mut u32) -> u64 {
        // SAFETY: the test's stream and the simulator's mean-altitude field.
        let s = unsafe { &mut *this };
        s.calls.push(Call::GetBits(out as usize));
        if let Some(bytes) = s.get::<4>() {
            unsafe { out.write_unaligned(u32::from_le_bytes(bytes)) };
        }
        0x8765_4321_0000_0000
    }

    extern "thiscall" fn error(this: *mut Stream) -> u64 {
        // SAFETY: the test's stream.
        let s = unsafe { &mut *this };
        s.calls.push(Call::Error);
        (u64::from(ERROR_EDX) << 32) | u64::from(s.error)
    }

    // Record the actual export-entry ESP while retaining the thiscall argument and return registers.
    #[unsafe(naked)]
    unsafe extern "thiscall" fn invoke_io(
        _this: *mut u8,
        _stream: *mut u8,
        _entry_stack: *mut usize,
        _entry: usize,
    ) -> u64 {
        core::arch::naked_asm!(
            "mov eax, [esp + 8]",
            "lea edx, [esp - 8]",
            "mov [eax], edx",
            "mov eax, [esp + 12]",
            "push dword ptr [esp + 4]",
            "call eax",
            "ret 12",
        )
    }

    #[test]
    fn reference_writes_retain_the_three_original_call_frame_addresses() {
        let mut world = World::new(None);
        world.set_saved_fields(0, 9);
        let mut entry_stack = 0;
        // SAFETY: one live stream argument; the helper records ESP before calling the export.
        unsafe {
            invoke_io(
                world.lv.ptr().add(8),
                (&mut *world.stream as *mut Stream).cast(),
                &mut entry_stack,
                fc4p_006bd8d0 as *const () as usize,
            );
        }
        assert_eq!(
            world.stream.reference_frames,
            [entry_stack - 20, entry_stack - 28, entry_stack - 36]
        );
        assert_eq!(world.stream.bytes, RECORD);
    }

    #[test]
    fn dirty_read_destination_is_the_incoming_stream_argument_word() {
        let mut world = World::new(Some(&RECORD));
        let mut entry_stack = 0;
        // SAFETY: one live stream argument; the byte reader records the destination address.
        unsafe {
            invoke_io(
                world.lv.ptr().add(8),
                (&mut *world.stream as *mut Stream).cast(),
                &mut entry_stack,
                fc4p_006bd950 as *const () as usize,
            );
        }
        assert_eq!(world.stream.dirty_destination, entry_stack + 4);
        assert_eq!(world.sim().dirty(), 0);
    }

    struct World {
        lv: Object,
        stream: Box<Stream>,
        _vtable: Box<[usize; 19]>,
        _fakes: [Fake; 2],
    }

    impl World {
        fn new(input: Option<&[u8]>) -> Self {
            let mut vtable = Box::new([0usize; 19]);
            let reading = input.is_some();
            vtable[0x14 / 4] = if reading {
                get_byte as *const () as usize
            } else {
                put_byte as *const () as usize
            };
            vtable[0x1C / 4] = if reading {
                get_version as *const () as usize
            } else {
                put_version as *const () as usize
            };
            vtable[0x30 / 4] = if reading {
                get_bits as *const () as usize
            } else {
                put_bits as *const () as usize
            };
            vtable[0x48 / 4] = error as *const () as usize;
            let mut lv = Object::new(0xA4);
            let stream = Box::new(Stream {
                vtable: vtable.as_ptr(),
                bytes: input.unwrap_or_default().to_vec(),
                position: 0,
                refs: 0,
                error: 0,
                calls: Vec::new(),
                lv: lv.ptr(),
                change_after_ref: false,
                dirty_seen_by_average: None,
                reference_frames: Vec::new(),
                dirty_destination: 0,
            });
            Self {
                lv,
                stream,
                _vtable: vtable,
                _fakes: [
                    fake(0x005D_9730, get_ref as *const () as usize),
                    fake(0x005D_99A0, put_ref as *const () as usize),
                ],
            }
        }

        fn sim(&mut self) -> LandValueSim {
            // SAFETY: the allocated simulator covers every field.
            unsafe { LandValueSim::from_ptr(self.lv.ptr()) }
        }

        fn set_saved_fields(&mut self, dirty: u8, state: i32) {
            let lv = self.sim();
            lv.set_intrinsic_grid(0x11 as *mut u8);
            lv.set_total_grid(0x12 as *mut u8);
            lv.set_slope_grid(0x13 as *mut u8);
            lv.set_dirty(dirty);
            lv.set_state(state as u32);
            lv.set_average(42);
            lv.set_mean_altitude(f32::from_bits(0x438F_999A));
        }

        fn bytes(&mut self) -> Vec<u8> {
            // SAFETY: the allocated object is 0xA4 bytes.
            unsafe { core::slice::from_raw_parts(self.lv.ptr(), 0xA4) }.to_vec()
        }

        fn run(&mut self, reading: bool) -> (u32, u32) {
            let entry = if reading {
                fc4p_006bd950 as *const ()
            } else {
                fc4p_006bd8d0 as *const ()
            };
            // SAFETY: the test's serializable interface, stream and one stack word.
            unsafe {
                call_thiscall_with_eax(
                    entry as usize,
                    self.lv.ptr().add(8),
                    &[(&mut *self.stream as *mut Stream) as u32],
                    0xFACE_CAFE,
                )
            }
        }

        fn expected_calls(&self, reading: bool, dirty: u8) -> Vec<Call> {
            let stream = &*self.stream as *const Stream as usize;
            if reading {
                let lv = self.stream.lv as usize;
                vec![
                    Call::GetVersion,
                    Call::GetRef(stream, 0xC9BD_939A, lv + 0x24, 0),
                    Call::GetRef(stream, 0xC9BD_939A, lv + 0x28, 0),
                    Call::GetRef(stream, 0xC9BD_939A, lv + 0x30, 0),
                    Call::GetByte(None),
                    Call::GetByte(Some(lv + 0x74)),
                    Call::GetBits(lv + 0x78),
                    Call::Error,
                ]
            } else {
                vec![
                    Call::PutVersion(1),
                    Call::PutRef(stream, 0x11),
                    Call::PutRef(stream, 0x12),
                    Call::PutRef(stream, 0x13),
                    Call::PutByte((stream as u32 & !0xFF) | u32::from(dirty)),
                    Call::PutByte(42),
                    Call::PutBits(0x438F_999A),
                    Call::Error,
                ]
            }
        }
    }

    #[test]
    fn save_record_bytes_and_calls() {
        for (dirty, state, saved_dirty) in [
            (0, 9, 0),
            (0, 5, 1),
            (1, 9, 1),
            (0x80, 9, 1),
            (0, -1, 1),
            (0, i32::MIN, 1),
            (0, i32::MAX, 1),
        ] {
            let mut w = World::new(None);
            w.set_saved_fields(dirty, state);
            let before = w.bytes();
            assert_eq!(w.run(false), (1, ERROR_EDX));
            let mut expected = RECORD;
            expected[26] = saved_dirty;
            assert_eq!(w.stream.bytes, expected);
            assert_eq!(w.stream.calls, w.expected_calls(false, saved_dirty));
            assert_eq!(w.bytes(), before, "saving does not change simulator memory");
        }
    }

    #[test]
    fn load_record_only_writes_saved_fields_and_normalizes_dirty() {
        for dirty in [0, 1, 2, 0x80, 0xFF] {
            let mut record = RECORD;
            record[26] = dirty;
            let mut w = World::new(Some(&record));
            // Fill unrelated fields and padding with sentinels to catch extra writes.
            unsafe { core::ptr::write_bytes(w.lv.ptr(), 0xA5, 0xA4) };
            w.sim().set_state(7);
            let mut expected = w.bytes();
            for (offset, token) in [(0x24, 0x11u32), (0x28, 0x12), (0x30, 0x13)] {
                expected[offset..offset + 4].copy_from_slice(&token.to_le_bytes());
            }
            expected[0x34] = u8::from(dirty != 0);
            expected[0x74] = 42;
            expected[0x78..0x7C].copy_from_slice(&record[28..32]);
            assert_eq!(w.run(true), (1, ERROR_EDX));
            assert_eq!(w.bytes(), expected);
            assert_eq!(w.stream.position, 32);
            assert_eq!(w.stream.dirty_seen_by_average, Some(u8::from(dirty != 0)));
            assert_eq!(w.stream.calls, w.expected_calls(true, 0));
        }
    }

    #[test]
    fn unsupported_versions_stop_after_version_and_keep_return_registers() {
        for version in [0u16, 2, 0x100, 0xFFFF] {
            let mut record = RECORD;
            record[..2].copy_from_slice(&version.to_le_bytes());
            let mut w = World::new(Some(&record));
            w.set_saved_fields(0x80, 6);
            let before = w.bytes();
            assert_eq!(
                w.run(true),
                (VERSION_RESULT as u32 & !0xFF, (VERSION_RESULT >> 32) as u32)
            );
            assert_eq!(w.stream.position, 2);
            assert_eq!(w.stream.calls, [Call::GetVersion]);
            assert_eq!(w.bytes(), before);
        }
    }

    #[test]
    fn only_the_low_error_byte_controls_success() {
        for reading in [false, true] {
            for error in [0, 1, 0x80, 0xFF, 0x100, 0xFFFF_FF00, u32::MAX] {
                let mut w = World::new(reading.then_some(&RECORD));
                w.set_saved_fields(0, 9);
                w.stream.error = error;
                assert_eq!(w.run(reading), (u32::from(error as u8 == 0), ERROR_EDX));
                assert_eq!(w.stream.calls, w.expected_calls(reading, 0));
            }
        }
    }

    #[test]
    fn truncated_reads_continue_and_preserve_unwritten_destinations() {
        for length in [2, 10, 18, 26, 27, 28, 29, 30, 31] {
            let mut w = World::new(Some(&RECORD[..length]));
            w.set_saved_fields(0x80, 6);
            let mean = w.sim().mean_altitude().to_bits();
            assert_eq!(w.run(true), (0, ERROR_EDX));
            assert_eq!(w.sim().mean_altitude().to_bits(), mean);
            assert_eq!(w.sim().state(), 6);
            assert_eq!(w.stream.calls, w.expected_calls(true, 0));
            if length == 26 {
                // The failed dirty-byte read uses the stream argument's low byte.
                let low = (&*w.stream as *const Stream as usize) as u8;
                assert_eq!(w.sim().dirty(), u8::from(low != 0));
                assert_eq!(w.sim().average(), 42);
            }
        }
    }

    #[test]
    fn saving_reads_each_field_after_the_preceding_call() {
        let mut w = World::new(None);
        w.set_saved_fields(1, 5);
        w.stream.change_after_ref = true;
        assert_eq!(w.run(false), (1, ERROR_EDX));
        let stream = &*w.stream as *const Stream as usize;
        assert_eq!(
            w.stream.calls,
            [
                Call::PutVersion(1),
                Call::PutRef(stream, 0x11),
                Call::PutRef(stream, 0x21),
                Call::PutRef(stream, 0x31),
                Call::PutByte(stream as u32 & !0xFF),
                Call::PutByte(99),
                Call::PutBits(0x8000_0000),
                Call::Error
            ]
        );
    }

    #[test]
    fn raw_float_bits_and_null_references_survive_save_and_load() {
        for bits in [0, 0x8000_0000, 1, 0x7F80_0000, 0xFF80_0000, 0x7F80_0123, 0xFFC0_1234] {
            let mut writer = World::new(None);
            writer.set_saved_fields(0, 9);
            writer.sim().set_intrinsic_grid(core::ptr::null_mut());
            writer.sim().set_total_grid(core::ptr::null_mut());
            writer.sim().set_slope_grid(core::ptr::null_mut());
            unsafe { writer.lv.ptr().add(0x78).cast::<u32>().write_unaligned(bits) };
            assert_eq!(writer.run(false), (1, ERROR_EDX));
            assert_eq!(&writer.stream.bytes[28..32], &bits.to_le_bytes());
            let mut reader = World::new(Some(&writer.stream.bytes));
            assert_eq!(reader.run(true), (1, ERROR_EDX));
            assert!(reader.sim().intrinsic_grid().is_null());
            assert!(reader.sim().total_grid().is_null());
            assert!(reader.sim().slope_grid().is_null());
            assert_eq!(&reader.bytes()[0x78..0x7C], &bits.to_le_bytes());
        }
    }

    #[unsafe(naked)]
    unsafe extern "thiscall" fn unchanged_edx_word(_this: *mut u8, _word: u32) -> u64 {
        core::arch::naked_asm!("mov eax, 0xAABBCCDD", "ret 4");
    }

    #[unsafe(naked)]
    unsafe extern "thiscall" fn partial_edx_error(_this: *mut u8) -> u64 {
        core::arch::naked_asm!("mov dl, 0x5A", "xor eax, eax", "ret");
    }

    #[test]
    fn rejected_version_retains_entry_edx_when_the_reader_leaves_it_alone() {
        let mut w = World::new(Some(&[]));
        w._vtable[0x1C / 4] = unchanged_edx_word as *const () as usize;
        let before = w.bytes();
        let args = [&mut *w.stream as *mut Stream as u32];
        // SAFETY: the allocated simulator and fake input stream.
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_006bd950 as *const () as usize,
            unsafe { w.lv.ptr().add(8) } as u32,
            &args,
        );
        call.edx = 0x5566_7788;
        unsafe { call_raw(&mut call) };
        assert_eq!((call.eax, call.edx), (0xAABB_CC00, 0x5566_7788));
        assert_eq!(w.bytes(), before);
    }

    #[test]
    fn mean_and_error_calls_keep_partially_written_edx() {
        for reading in [false, true] {
            let mut w = World::new(reading.then_some(&RECORD));
            w.set_saved_fields(0, 9);
            w._vtable[0x30 / 4] = unchanged_edx_word as *const () as usize;
            w._vtable[0x48 / 4] = partial_edx_error as *const () as usize;
            let expected_edx = (w._vtable.as_ptr() as u32 & !0xFF) | 0x5A;
            assert_eq!(w.run(reading), (1, expected_edx));
        }
    }

    #[test]
    fn io_keeps_the_x87_stack_control_word_and_mxcsr() {
        #[repr(C, align(16))]
        struct FpuState([u8; 512]);
        for reading in [false, true] {
            let mut w = World::new(reading.then_some(&RECORD));
            w.set_saved_fields(0, 9);
            let mut before = FpuState([0; 512]);
            let mut after = FpuState([0; 512]);
            // SAFETY: one free x87 slot, aligned 512-byte destinations, and the pushed value is popped
            // before any assertion. The stream fakes only transfer integer words.
            unsafe {
                core::arch::asm!("fld1", "fxsave [{p}]", p = in(reg) &mut before,
                    out("st(0)") _, options(nostack));
                w.run(reading);
                core::arch::asm!("fxsave [{p}]", "fstp st(0)", p = in(reg) &mut after,
                    out("st(0)") _, options(nostack));
            }
            // The first 160 bytes contain the x87 environment, MXCSR, and the eight x87 registers.
            assert_eq!(before.0[..160], after.0[..160]);
        }
    }
}

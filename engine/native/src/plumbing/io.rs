// SPDX-License-Identifier: GPL-3.0-or-later
//! The saved portion of one water-producer record.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;

game_fn! { fn write_serializable(stream: *mut u8, object: *mut u8) -> u64 = "C" 0x005D_99A0; }

port! {
    /// Writes six signed coordinate words, capacity and production, then the occupant reference.
    /// Transient fields are skipped. All intermediate results are ignored; only AL is set on return.
    #[port(address = 0x006D_A4F0, owner = "plumbing", name = "cSC4PlumbingSimulator::WriteWaterProducer")]
    pub unsafe extern "stdcall" fn fc4p_006da4f0(stream: *mut u8, record: *mut u8) -> u64 {
        // SAFETY: the game supplies a stream and a readable producer record. Each field and vtable is
        // read after the preceding callback so callback changes remain visible.
        unsafe {
            for (i, offset) in [0usize,4,8,12,16,20,28,32].into_iter().enumerate() {
                let value = record.add(offset).cast::<u32>().read_unaligned();
                let table = stream.cast::<u32>().read_unaligned();
                let args = [value];
                let mut c = RawCall::new(Conv::Thiscall, vslot(stream, if i < 6 {0x20} else {0x24}), stream as u32, &args);
                c.eax = if i == 0 {table} else {value};
                // The first call's EDX is not consumed by this stream operation.
                c.edx = table;
                call_raw(&mut c);
            }
            let object = record.add(0x2C).cast::<*mut u8>().read_unaligned();
            (write_serializable(stream, object) & !0xFF) | 1
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006da4f0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fake;
    use std::cell::RefCell;

    #[derive(Default)]
    struct State {
        calls: Vec<(u32, u32, u32)>,
        record: usize,
        next_table: usize,
        stream: usize,
    }
    thread_local! { static STATE:RefCell<State> = RefCell::new(State::default()); }
    extern "thiscall" fn signed(this: *mut u8, value: u32) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push((0x20, this as u32, value));
            if s.calls.len() == 1 {
                // The next field and stream vtable must be read after this callback.
                unsafe {
                    (s.record as *mut u32).add(1).write(0xFFFF_FFFF);
                    this.cast::<usize>().write(s.next_table);
                }
            }
        });
        0xABCD_EF00
    }
    extern "thiscall" fn unsigned(this: *mut u8, value: u32) -> u32 {
        STATE.with(|s| s.borrow_mut().calls.push((0x24, this as u32, value)));
        0
    }
    extern "C" fn reference(stream: *mut u8, object: *mut u8) -> u64 {
        STATE.with(|s| {
            assert_eq!(stream as usize, s.borrow().stream);
            s.borrow_mut().calls.push((0x99, stream as u32, object as u32));
        });
        0x7654_3210_AABB_CC00
    }
    #[test]
    fn producer_record_preserves_the_saved_field_order_and_ignores_errors() {
        // ProducerRecord's persistence vectors: six cells, maximum capacity, production, reference.
        let _reference = fake(0x005D_99A0, reference as *const () as usize);
        let mut first = [0usize; 10];
        first[8] = signed as *const () as usize;
        let mut next = [0usize; 10];
        next[8] = signed as *const () as usize;
        next[9] = unsigned as *const () as usize;
        let mut stream = [first.as_ptr() as usize];
        let mut record = [0xDEAD_BEEFu32; 12];
        record[..6].copy_from_slice(&[i32::MIN as u32, 64, 5, 20, 63, 7]);
        record[7] = 1000;
        record[8] = 300;
        record[11] = 0x1122_3344;
        let sp = stream.as_mut_ptr().cast::<u8>();
        STATE.with(|s| {
            *s.borrow_mut() = State {
                calls: vec![],
                record: record.as_mut_ptr() as usize,
                next_table: next.as_ptr() as usize,
                stream: sp as usize,
            }
        });
        assert_eq!(
            unsafe { fc4p_006da4f0(sp, record.as_mut_ptr().cast()) },
            0x7654_3210_AABB_CC01
        );
        let expected: Vec<_> = [
            (0x20, i32::MIN as u32),
            (0x20, u32::MAX),
            (0x20, 5),
            (0x20, 20),
            (0x20, 63),
            (0x20, 7),
            (0x24, 1000),
            (0x24, 300),
            (0x99, 0x1122_3344),
        ]
        .into_iter()
        .map(|(slot, value)| (slot, sp as u32, value))
        .collect();
        STATE.with(|s| assert_eq!(s.borrow().calls, expected));
        assert_eq!(
            record,
            [
                i32::MIN as u32,
                u32::MAX,
                5,
                20,
                63,
                7,
                0xDEAD_BEEF,
                1000,
                300,
                0xDEAD_BEEF,
                0xDEAD_BEEF,
                0x1122_3344
            ]
        );
    }
}

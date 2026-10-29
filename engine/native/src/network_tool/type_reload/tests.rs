// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data};
use std::cell::RefCell;

const ORDER: [usize; 13] = [0, 10, 11, 1, 8, 9, 3, 2, 12, 6, 4, 7, 5];
const METADATA: [[u32; 8]; 13] = [
    [1, 0xa766c101, 0x2821ed93, 0, 0xe17bb266, 0x49e13260, 1, 1],
    [2, 0xc8433ecb, 0xe8347989, 0, 0x217bb308, 0, 1, 1],
    [0, 0x08433f47, 0xa8434037, 0, 0x8bf30a8b, 0, 2, 2],
    [1, 0x092a047d, 0xa92a02ea, 0, 0x2a920bf9, 0, 1, 1],
    [1, 0x09511847, 0xa951171c, 0xf0, 0x82c2ff49, 0, 1, 1],
    [0, 0xa88e1592, 0x088e1962, 0, 0xa16f1463, 0x69e13770, 1, 2],
    [0, 0x0b730a26, 0xcb730fac, 0, 0x6b730b83, 0, 2, 2],
    [1, 0x8a15ee13, 0x8a15f3f2, 0xf1, 0xc17bb329, 0, 1, 1],
    [2, 0x0b79e2e1, 0x2b79dffb, 0, 0xab79e761, 0, 1, 1],
    [2, 0xabe08433, 0xebe084c2, 0, 0xebe091b6, 0, 1, 1],
    [1, 0x2be08438, 0xcbe084cb, 0, 0xabe091a8, 0, 1, 1],
    [1, 0x4be08afb, 0x6be08658, 0, 0x4be091b2, 0, 1, 1],
    [0, 0x6be0843b, 0xebe084d1, 0, 0x6be091b4, 0, 2, 2],
];
const PLACEMENTS: [u32; 13] = [
    0x0781214f, 0x0814e612, 0xa8433f6b, 0x492a03a3, 0x095117db, 0, 0x2b730a17, 0x0a367ca7, 0xcb79e089, 0xebe08882,
    0x8bda0c2b, 0xebe08cda, 0x0be08c87,
];
const KEY_ADDRESSES: [u32; 13] = [
    0xb0b418, 0xb0b448, 0xb0b460, 0xb0b478, 0xb0b430, 0, 0xb0b4a8, 0xb0b490, 0xb0b4c0, 0xb0b508, 0xb0b4d8, 0xb0b4f0,
    0xb0b520,
];
const SECTIONS: [u32; 13] = [
    0xaa86d4, 0xaa868c, 0xaa8628, 0xaa8640, 0xaa85dc, 0, 0xaa85f0, 0xaa85c4, 0xaa8670, 0xaa8658, 0xaa86b8, 0xaa86a0,
    0xaa8608,
];

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Clear(usize),
    Evaluator(usize, usize),
    Bridges(usize),
    Supports(usize),
    Placement(usize),
    Complete(usize),
    Verifiers,
}

#[derive(Default)]
struct State {
    table: usize,
    keys: usize,
    sections: usize,
    ini: usize,
    expected: Vec<u8>,
    pending: Option<usize>,
    calls: Vec<Call>,
    reply: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

impl State {
    fn record(&self, this: *mut u8) -> usize {
        let offset = this as usize - self.table;
        assert_eq!(offset % 0x114, 0);
        assert!(offset / 0x114 < 13);
        offset / 0x114
    }

    fn check(&mut self) {
        if let Some(i) = self.pending.take() {
            for (word, value) in METADATA[i].iter().enumerate() {
                let offset = i * 0x114 + if word == 0 { 0 } else { word * 4 + 4 };
                self.expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        unsafe {
            assert_eq!(
                core::slice::from_raw_parts(self.table as *const u8, 13 * 0x114),
                self.expected
            );
        }
    }

    fn effect(&mut self, i: usize, offset: usize, value: u8) {
        self.expected[i * 0x114 + offset] = value;
        unsafe {
            *((self.table as *mut u8).add(i * 0x114 + offset)) = value;
        }
    }
}

extern "thiscall" fn clear_fake(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.check();
        let i = s.record(this);
        s.calls.push(Call::Clear(i));
        s.effect(i, 0x40, 0);
        s.effect(i, 0x75, 0);
        s.pending = Some(i);
        s.reply
    })
}

extern "thiscall" fn evaluator_fake(this: *mut u8, key: *const u8, a: u32, b: u32) -> u32 {
    assert_eq!((a, b), (0, 0));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.check();
        let i = s.record(this);
        let key = key as usize - s.keys + 0xb0b418;
        s.calls.push(Call::Evaluator(i, key));
        // A callee may change metadata; later reload steps must retain it.
        s.effect(i, 0x0F, 0xEC);
        s.reply
    })
}

extern "thiscall" fn bridges_fake(this: *mut u8, ini: *mut u8, section: *const u8, instance: u32, width: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.check();
        let i = s.record(this);
        assert_eq!(ini as usize, s.ini);
        assert_eq!(section as usize - s.sections + 0xaa85b4, SECTIONS[i] as usize);
        assert_eq!(instance, 0x1000 + i as u32);
        assert_eq!(width, METADATA[i][6]);
        s.calls.push(Call::Bridges(i));
        s.reply
    })
}

extern "thiscall" fn supports_fake(this: *mut u8, group: u32) -> u32 {
    assert_eq!(group, 0x8974f80f);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.check();
        let i = s.record(this);
        s.calls.push(Call::Supports(i));
        s.reply
    })
}

extern "thiscall" fn placement_fake(this: *mut u8, instance: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.check();
        let i = s.record(this);
        assert_eq!(instance, PLACEMENTS[i]);
        s.calls.push(Call::Placement(i));
        // Placement can overwrite the byte that street explicitly sets afterwards.
        s.effect(i, 0x75, 0x96);
        s.reply
    })
}

extern "thiscall" fn complete_fake(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.record(this);
        if i == 3 {
            s.expected[i * 0x114 + 0x75] = 1;
        }
        s.check();
        s.calls.push(Call::Complete(i));
        s.effect(i, 0x40, 1);
        s.reply
    })
}

extern "C" fn verifiers_fake(ini: *mut u8, section: *const u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.check();
        assert_eq!(ini as usize, s.ini);
        assert_eq!(section as usize, s.sections);
        s.calls.push(Call::Verifiers);
    });
    registers(0x1234_abcd, 0x5678_ef90)
}

// Carries the core type-record reload order, literal metadata, widths, power-line exception,
// and repeated-reload coverage onto the game's storage and call boundaries.
#[test]
fn reload_metadata_resources_order_and_repeated_calls_preserve_all_other_bytes() {
    let _f = [
        fake(0x6421d0, clear_fake as *const () as usize),
        fake(0x63d360, evaluator_fake as *const () as usize),
        fake(0x6437a0, bridges_fake as *const () as usize),
        fake(0x63f780, supports_fake as *const () as usize),
        fake(0x63d460, placement_fake as *const () as usize),
        fake(0x63bf90, complete_fake as *const () as usize),
        fake(0x63faf0, verifiers_fake as *const () as usize),
    ];
    let mut table = [0xA5u8; 13 * 0x114];
    let mut keys = [0x3Cu8; 0x120];
    let mut sections = [0x5Au8; 0x121];
    let mut guards = Vec::new();
    for i in 0..13 {
        guards.push(fake_data(0xb452c8 + i * 0x114, unsafe {
            table.as_mut_ptr().add(i as usize * 0x114)
        }));
    }
    for address in KEY_ADDRESSES.into_iter().filter(|&a| a != 0) {
        guards.push(fake_data(address, unsafe {
            keys.as_mut_ptr().add((address - 0xb0b418) as usize)
        }));
    }
    for address in SECTIONS.into_iter().filter(|&a| a != 0).chain([0xaa85b4]) {
        guards.push(fake_data(address, unsafe {
            sections.as_mut_ptr().add((address - 0xaa85b4) as usize)
        }));
    }
    for (ini, reply) in [(0usize, 0), (0x12345678, 0xffffffff), (0x98765432, 0x12345600)] {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                table: table.as_mut_ptr() as usize,
                keys: keys.as_mut_ptr() as usize,
                sections: sections.as_mut_ptr() as usize,
                ini,
                expected: table.to_vec(),
                reply,
                ..State::default()
            }
        });
        assert_eq!(
            unsafe { fc4p_00623310(ini as *mut u8) },
            registers(0x1234_abcd, 0x5678_ef90)
        );
        let mut expected = Vec::new();
        for i in ORDER {
            expected.push(Call::Clear(i));
            if i != 5 {
                expected.push(Call::Evaluator(i, KEY_ADDRESSES[i] as usize + 12));
                expected.push(Call::Evaluator(i, KEY_ADDRESSES[i] as usize));
                expected.push(Call::Bridges(i));
                if matches!(i, 0 | 10 | 11 | 1 | 8 | 9 | 3) {
                    expected.push(Call::Supports(i));
                }
                expected.push(Call::Placement(i));
            }
            expected.push(Call::Complete(i));
        }
        expected.push(Call::Verifiers);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.check();
            assert_eq!(s.calls, expected);
        });
    }
    assert_eq!(keys, [0x3C; 0x120]);
    assert_eq!(sections, [0x5A; 0x121]);
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::fake;
use std::cell::RefCell;

struct Tree {
    header: Box<[u32; 24]>,
    nodes: Vec<[u32; 24]>,
}
impl Tree {
    fn new(keys: &[u32]) -> Self {
        let mut t = Self {
            header: Box::new([0; 24]),
            nodes: keys
                .iter()
                .enumerate()
                .map(|(i, &key)| {
                    let mut n = [0; 24];
                    n[4] = key;
                    n[5] = i as u32;
                    n
                })
                .collect(),
        };
        let head = t.header.as_mut_ptr() as u32;
        let root = t.link(0, keys.len(), head);
        t.header[1] = root;
        t.header[2] = if keys.is_empty() {
            head
        } else {
            t.nodes[0].as_ptr() as u32
        };
        t.header[3] = if keys.is_empty() {
            head
        } else {
            t.nodes[keys.len() - 1].as_ptr() as u32
        };
        t
    }
    fn link(&mut self, begin: usize, end: usize, parent: u32) -> u32 {
        if begin == end {
            return 0;
        }
        let mid = (begin + end) / 2;
        let p = self.nodes[mid].as_mut_ptr() as u32;
        self.nodes[mid][1] = parent;
        self.nodes[mid][2] = self.link(begin, mid, p);
        self.nodes[mid][3] = self.link(mid + 1, end, p);
        p
    }
    fn ptr(&mut self) -> *mut u8 {
        self.header.as_mut_ptr().cast()
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Call {
    Mode(usize, u32, u32),
    Value(usize, u32),
    Insert(usize, usize, [u32; 3], u32, u32, u32),
    Predicate(usize, u32),
    Match(usize, usize),
}
#[derive(Default)]
struct State {
    entry_location: usize,
    calls: Vec<Call>,
    values: Vec<u32>,
    predicates: Vec<u32>,
    match_index: Option<usize>,
    append_swap: Option<(usize, usize, usize)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn mode(this: *mut u8, a: u32, b: u32) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Mode(this as usize, a, b)));
}
extern "thiscall" fn value(this: *mut u8, key: u32, output: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Value(this as usize, key));
        let value = s.values.remove(0);
        unsafe {
            output.write(value);
        }
        value
    })
}
extern "thiscall" fn cache_value(this: *mut u8, key: u32, output: *mut u32) {
    STATE.with(|s| {
        let location = s.borrow().entry_location;
        if location != 0 {
            assert_eq!(output as usize, unsafe { *(location as *const usize) } - 8);
        }
    });
    let _ = value(this, key, output);
    unsafe {
        output.add(1).write(0x8765_4321);
    }
    STATE.with(|s| {
        if let Some((context, position, capacity)) = s.borrow_mut().append_swap.take() {
            unsafe {
                write(context as *mut u8, 0xC, position as u32);
                write(context as *mut u8, 0x10, capacity as u32);
            }
        }
    });
}
extern "thiscall" fn insert(
    this: *mut u8,
    position: *mut u8,
    record: *mut u8,
    scratch: *mut u32,
    count: u32,
    stride: u32,
) {
    STATE.with(|s| {
        let location = s.borrow().entry_location;
        if location != 0 {
            let entry = unsafe { *(location as *const usize) };
            assert_eq!(record as usize, entry - 12);
            assert_eq!(scratch as usize, entry + 8);
        }
    });
    let r = unsafe { [u32::from(read::<u8>(record, 0)), read(record, 4), read(record, 8)] };
    STATE.with(|s| {
        s.borrow_mut().calls.push(Call::Insert(
            this as usize,
            position as usize,
            r,
            unsafe { *scratch },
            count,
            stride,
        ))
    });
}
extern "thiscall" fn predicate(this: *mut u8, key: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Predicate(this as usize, key));
        s.predicates.remove(0)
    })
}
extern "thiscall" fn matches(this: *mut u8, context: *mut u8, output: *mut *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Match(this as usize, context as usize));
        if Some(unsafe { read::<u32>(this, 0) } as usize) == s.match_index {
            unsafe {
                *output = this.add(0x10);
            }
            0xABCD_EF01
        } else {
            0xABCD_EF00
        }
    })
}
#[test]
fn tree_successors_walk_balanced_and_duplicate_keys_through_the_header() {
    for n in 0..20 {
        let keys: Vec<u32> = (0..n).map(|k| k / 2).collect();
        let mut t = Tree::new(&keys);
        let head = t.ptr();
        let mut p = t.header[2] as *mut u8;
        for i in 0..n as usize {
            assert_eq!(p, t.nodes[i].as_mut_ptr().cast());
            p = unsafe { next(p) };
        }
        assert_eq!(p, head);
    }
}
#[test]
fn occupant_set_modes_merge_in_pointer_order_and_skip_common_members() {
    let mut table = [0usize; 18];
    table[17] = mode as *const () as usize;
    let mut objects = [[table.as_ptr() as usize]; 6];
    let keys: Vec<u32> = objects.iter_mut().map(|o| o.as_mut_ptr() as u32).collect();
    for mask_a in 0..64u32 {
        for mask_b in 0..64u32 {
            let ka: Vec<u32> = keys
                .iter()
                .enumerate()
                .filter(|(i, _)| mask_a & (1 << i) != 0)
                .map(|(_, &k)| k)
                .collect();
            let kb: Vec<u32> = keys
                .iter()
                .enumerate()
                .filter(|(i, _)| mask_b & (1 << i) != 0)
                .map(|(_, &k)| k)
                .collect();
            let mut a = Tree::new(&ka);
            let mut b = Tree::new(&kb);
            let mut ap = a.ptr();
            let mut bp = b.ptr();
            let before_a: Vec<_> = a.nodes.to_vec();
            let before_b: Vec<_> = b.nodes.to_vec();
            STATE.with(|s| *s.borrow_mut() = State::default());
            unsafe {
                fc4p_0061f5d0(&mut ap, &mut bp);
            }
            let expected: Vec<_> = keys
                .iter()
                .enumerate()
                .filter_map(|(i, &k)| {
                    let a = mask_a & (1 << i) != 0;
                    let b = mask_b & (1 << i) != 0;
                    (a != b).then_some(Call::Mode(k as usize, if a { 0 } else { 5 }, 0))
                })
                .collect();
            assert_eq!(calls(), expected);
            assert_eq!(a.nodes.to_vec(), before_a);
            assert_eq!(b.nodes.to_vec(), before_b);
        }
    }
}
#[test]
fn cache_hit_uses_only_key_byte_and_writes_only_output() {
    let mut records = [0xAABB_CC12u32, 0x1234_5678, 7, 0xAABB_CC34, 0x8765_4321, 8];
    let mut context = [
        0u32,
        0,
        records.as_mut_ptr() as u32,
        unsafe { records.as_mut_ptr().add(6) } as u32,
        0,
    ];
    let before = context;
    let before_records = records;
    let mut output = [0xA5A5_A5A5u32; 3];
    assert_eq!(
        unsafe { CACHED(context.as_mut_ptr().cast(), 0xDEAD_BE34, output.as_mut_ptr().add(1)) },
        0x8765_4321
    );
    assert_eq!(context, before);
    assert_eq!(records, before_records);
    assert_eq!(output, [0xA5A5_A5A5, 0x8765_4321, 0xA5A5_A5A5]);
}
#[test]
fn cache_miss_forwards_full_key_appends_without_padding_and_grows_only_when_full() {
    let _f = fake(0x0062_1A40, insert as *const () as usize);
    let table = [0usize, cache_value as *const () as usize];
    for full in [false, true] {
        let mut records = [0xA5A5_A5A5u32; 6];
        let p = records.as_mut_ptr();
        let mut context = [
            table.as_ptr() as u32,
            0,
            p as u32,
            p as u32,
            unsafe { p.add(if full { 0 } else { 6 }) } as u32,
        ];
        let c = context.as_mut_ptr().cast::<u8>();
        let mut output = 0;
        STATE.with(|s| {
            *s.borrow_mut() = State {
                values: vec![0x1234_5678],
                ..State::default()
            }
        });
        assert_eq!(unsafe { CACHED(c, 0xDEAD_BE12, &mut output) }, 0x1234_5678);
        assert_eq!(output, 0x1234_5678);
        let mut expected = vec![Call::Value(c as usize, 0xDEAD_BE12)];
        if full {
            expected.push(Call::Insert(
                c as usize + 8,
                p as usize,
                [0x12, 0x1234_5678, 0x8765_4321],
                &mut output as *mut u32 as u32,
                1,
                1,
            ));
            assert_eq!(records, [0xA5A5_A5A5; 6]);
        } else {
            assert_eq!(
                records,
                [
                    0xA5A5_A512,
                    0x1234_5678,
                    0x8765_4321,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5
                ]
            );
            assert_eq!(context[3], p as u32 + 12);
        }
        assert_eq!(calls(), expected);
    }
}
#[test]
fn cache_append_reloads_cursor_after_callback_and_handles_null_spare_position() {
    let table = [0usize, cache_value as *const () as usize];
    for null in [false, true] {
        let mut records = [0xA5A5_A5A5u32; 3];
        let p = records.as_mut_ptr();
        let mut context = [table.as_ptr() as u32, 0, 0, 0, 0];
        let c = context.as_mut_ptr().cast::<u8>();
        let mut output = 0;
        let position = if null { 0 } else { p as usize };
        STATE.with(|s| {
            *s.borrow_mut() = State {
                values: vec![42],
                append_swap: Some((c as usize, position, position + 12)),
                ..State::default()
            }
        });
        unsafe {
            CACHED(c, 9, &mut output);
        }
        assert_eq!(output, 42);
        assert_eq!(context[3] as usize, position + 12);
        assert_eq!(
            records,
            if null {
                [0xA5A5_A5A5; 3]
            } else {
                [0xA5A5_A509, 42, 0x8765_4321]
            }
        );
        assert_eq!(calls(), [Call::Value(c as usize, 9)]);
    }
}
#[test]
fn rule_mask_and_predicate_failures_short_circuit_and_success_alone_sets_output() {
    let _f = fake(0x0062_1D30, value as *const () as usize);
    let table = [predicate as *const () as usize];
    let mut context = [table.as_ptr() as u32];
    let c = context.as_mut_ptr().cast::<u8>();
    let mut terms = [0x12u32, 0xABCD, 0xFF, 0x34, 0x1234, 0xFFFF];
    let mut predicates = [99u32, 0x56, 88, 0x78];
    let mut rule = [0u32; 6];
    rule[1] = terms.as_mut_ptr() as u32;
    rule[2] = unsafe { terms.as_mut_ptr().add(6) } as u32;
    rule[4] = predicates.as_mut_ptr() as u32;
    rule[5] = unsafe { predicates.as_mut_ptr().add(4) } as u32;
    let r = rule.as_mut_ptr().cast::<u8>();
    for stage in 0..5 {
        let values = vec![
            if stage == 0 { 0 } else { 0xFFFF_ABCD },
            if stage == 1 { 0 } else { 0xFFFF_1234 },
        ];
        let predicates = vec![
            if stage == 2 { 0xABCD_0000 } else { 0xABCD_0001 },
            if stage == 3 { 0 } else { 0x80 },
        ];
        STATE.with(|s| {
            *s.borrow_mut() = State {
                values,
                predicates,
                ..State::default()
            }
        });
        let mut output = 0xDEAD_BEEFu32 as *mut u8;
        assert_eq!(unsafe { fc4p_00622040(r, c, &mut output) }, u8::from(stage == 4));
        assert_eq!(
            output,
            if stage == 4 {
                unsafe { r.add(0x10) }
            } else {
                0xDEAD_BEEF as *mut u8
            }
        );
        let mut expected = vec![Call::Value(c as usize, 0x12)];
        if stage >= 1 {
            expected.push(Call::Value(c as usize, 0x34));
        }
        if stage >= 2 {
            expected.push(Call::Predicate(c as usize, (c as u32 & !255) | 0x56));
        }
        if stage >= 3 {
            expected.push(Call::Predicate(c as usize, (c as u32 & !255) | 0x78));
        }
        assert_eq!(calls(), expected);
    }
}
#[test]
fn empty_rule_succeeds_and_equal_key_search_is_unsigned_and_stops_at_first_match() {
    let _f = fake(0x0062_2040, matches as *const () as usize);
    let mut empty = [0u32; 6];
    let mut output = core::ptr::null_mut();
    assert_eq!(
        unsafe { fc4p_00622040(empty.as_mut_ptr().cast(), core::ptr::null_mut(), &mut output) },
        1
    );
    assert_eq!(output as usize, empty.as_ptr() as usize + 0x10);
    for keys in [vec![], vec![0, 1, 1, 1, 0x8000_0000, u32::MAX]] {
        for key in [0, 1, 2, 0x8000_0000, u32::MAX] {
            for success in [None, Some(1), Some(2), Some(5)] {
                let mut tree = Tree::new(&keys);
                let mut rules = [0u32, tree.ptr() as u32];
                let mut context = [0u32, key];
                let c = context.as_mut_ptr().cast::<u8>();
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        match_index: success,
                        ..State::default()
                    }
                });
                output = 0xDEAD_BEEF as *mut u8;
                let candidates: Vec<usize> = keys
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| **k == key)
                    .map(|(i, _)| i)
                    .collect();
                let found = candidates.iter().copied().find(|&i| Some(i) == success);
                assert_eq!(
                    unsafe { fc4p_006220d0(rules.as_mut_ptr().cast(), c, &mut output) },
                    u8::from(found.is_some())
                );
                assert_eq!(
                    output as usize,
                    found.map_or(0, |i| tree.nodes[i].as_ptr() as usize + 0x24)
                );
                let mut expected = vec![];
                for i in candidates {
                    expected.push(Call::Match(tree.nodes[i].as_ptr() as usize + 0x14, c as usize));
                    if Some(i) == found {
                        break;
                    }
                }
                assert_eq!(calls(), expected);
            }
        }
    }
}

const CACHED: unsafe extern "thiscall" fn(*mut u8, u32, *mut u32) -> u32 =
    unsafe { core::mem::transmute(fc4p_00621d30 as unsafe extern "C" fn()) };

#[test]
fn cache_growth_keeps_record_and_caller_output_at_their_original_addresses() {
    let _fake = fake(0x0062_1A40, insert as *const () as usize);
    let table = [0usize, cache_value as *const () as usize];
    let mut context = [table.as_ptr() as u32, 0, 0, 0, 0];
    let mut output = 0u32;
    let args = [0xABCD_0012, &mut output as *mut u32 as u32];
    let mut call = super::super::stack_tests::StackCall::new(
        fc4p_00621d30 as *const () as usize,
        context.as_mut_ptr().cast(),
        &args,
    );
    STATE.with(|s| {
        *s.borrow_mut() = State {
            entry_location: &mut call.entry as *mut usize as usize,
            values: vec![0x1234_5678],
            ..State::default()
        }
    });
    assert_eq!(unsafe { call.run() }.0, 0x1234_5678);
    assert_eq!(output, 0x1234_5678);
}

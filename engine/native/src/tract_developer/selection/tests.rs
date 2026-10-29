// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, PartialEq, Eq)]
struct Call(&'static str, u32, Vec<u32>);
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: BTreeMap<u32, Box<[u32]>>,
    draws: VecDeque<u32>,
    anchor: bool,
    anchor_object: u32,
    styles: u32,
    primary: u32,
    families: BTreeMap<u32, Vec<u32>>,
    outcomes: BTreeMap<u32, u32>,
    rejected_styles: Vec<u32>,
    rejected_configurations: Vec<u32>,
    rejected_buildings: Vec<u32>,
    mode: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(name: &'static str, this: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(name, this, args.to_vec())));
}
fn memory(bytes: u32) -> u32 {
    let mut data = vec![0xA5A5_A5A5; bytes.div_ceil(4) as usize].into_boxed_slice();
    let p = data.as_mut_ptr() as u32;
    STATE.with(|s| assert!(s.borrow_mut().allocations.insert(p, data).is_none()));
    p
}
fn release(p: u32) {
    STATE.with(|s| assert!(s.borrow_mut().allocations.remove(&p).is_some(), "free {p:08x}"));
}
extern "C" fn alloc(bytes: u32) -> u32 {
    log("allocate", 0, &[bytes]);
    memory(bytes)
}
extern "C" fn dealloc(p: u32) -> u32 {
    log("free", 0, &[p]);
    release(p);
    0xDEAD_BEEF
}
extern "thiscall" fn capacity_fake(this: u32, candidate: u32) -> u32 {
    log("capacity", this, &[candidate]);
    0x1234_5678
}
extern "thiscall" fn purpose(this: u32) -> u32 {
    log("purpose", this, &[]);
    5
}
extern "thiscall" fn anchors(this: u32) -> u32 {
    log("anchors", this, &[]);
    STATE.with(|s| s.borrow().anchor_object)
}
extern "thiscall" fn needs_anchor(this: u32, rect: u32, purpose: u32) -> u32 {
    log("anchor", this, &[rect, purpose]);
    0xFFFF_0000 | STATE.with(|s| u32::from(s.borrow().anchor))
}
extern "thiscall" fn styles(this: u32) -> u32 {
    log("styles", this, &[]);
    STATE.with(|s| s.borrow().styles)
}
extern "thiscall" fn draw(this: u32, bound: u32) -> u32 {
    log("draw", this, &[bound]);
    STATE.with(|s| s.borrow_mut().draws.pop_front().expect("unexpected draw"))
}
extern "thiscall" fn cfg_style(this: u32, group: u32) -> u32 {
    log("configuration_style", this, &[group]);
    0xABCD_0000 | STATE.with(|s| u32::from(!s.borrow().rejected_styles.contains(&this)))
}
extern "thiscall" fn cfg_compatible(this: u32, cfg: u32, candidate: u32, corner: u32) -> u32 {
    log("configuration_compatible", this, &[cfg, candidate, corner]);
    STATE.with(|s| {
        if s.borrow().rejected_configurations.contains(&cfg) {
            8
        } else {
            27
        }
    })
}
extern "thiscall" fn primary(this: u32) -> u32 {
    log("primary", this, &[]);
    STATE.with(|s| s.borrow().primary)
}
extern "thiscall" fn mode(this: u32) -> u32 {
    log("mode", this, &[]);
    STATE.with(|s| s.borrow().mode)
}
extern "thiscall" fn building_result(this: u32, building: u32, candidate: u32) -> u32 {
    log("building_compatible", this, &[building, candidate]);
    STATE.with(|s| s.borrow().outcomes.get(&building).copied().unwrap_or(27))
}
extern "thiscall" fn building_key(this: u32, building: u32, output: u32) -> u32 {
    log("key", this, &[building]);
    unsafe {
        assert_eq!([word(output, 0), word(output, 4), word(output, 8)], [0; 3]);
        store(output, 0, building);
        store(output, 4, 0x1234);
        store(output, 8, 0x5678);
    }
    output
}
extern "C" fn group_fake(key: u32, group: u32) -> u32 {
    let building = unsafe { word(key, 0) };
    log("group", building, &[group]);
    0xFFFF_0000 | STATE.with(|s| u32::from(!s.borrow().rejected_buildings.contains(&building)))
}

unsafe fn tree_insert(tree: u32, value: u32) {
    unsafe {
        let header = word(tree, 0);
        let mut parent = header;
        let mut node = word(header, 4);
        while node != 0 {
            parent = node;
            let key = word(node, 16);
            if value == key {
                return;
            }
            node = word(node, if value < key { 8 } else { 12 });
        }
        let node = memory(20);
        store(node, 0, 1);
        store(node, 4, parent);
        store(node, 8, 0);
        store(node, 12, 0);
        store(node, 16, value);
        if parent == header {
            store(header, 4, node);
            store(header, 8, node);
            store(header, 12, node);
        } else {
            store(parent, if value < word(parent, 16) { 8 } else { 12 }, node);
            if value < word(word(header, 8), 16) {
                store(header, 8, node);
            }
            if value > word(word(header, 12), 16) {
                store(header, 12, node);
            }
        }
        store(tree, 4, word(tree, 4) + 1);
    }
}
extern "thiscall" fn insert_cfg(tree: u32, output: u32, value: u32) -> u32 {
    log("insert_configuration", tree, &[unsafe { word(value, 0) }]);
    unsafe {
        tree_insert(tree, word(value, 0));
        store(output, 0, 0);
    }
    output
}
unsafe fn release_branch(node: u32) {
    unsafe {
        if node != 0 {
            release_branch(word(node, 8));
            release_branch(word(node, 12));
            release(node);
        }
    }
}
extern "thiscall" fn erase_cfg(tree: u32, node: u32) -> u32 {
    log("erase_configuration_branch", tree, &[node]);
    unsafe {
        release_branch(node);
    }
    0
}
extern "thiscall" fn reserve(vector: u32, count: u32) -> u32 {
    log("reserve", vector, &[count]);
    unsafe {
        assert_eq!([word(vector, 0), word(vector, 4), word(vector, 8)], [0; 3]);
        let p = memory(count * 4);
        store(vector, 0, p);
        store(vector, 4, p);
        store(vector, 8, p + count * 4);
    }
    0
}
extern "thiscall" fn fill(vector: u32, position: u32, count: u32, value: u32) -> u32 {
    log("fill", vector, &[count, unsafe { word(value, 0) }]);
    unsafe {
        assert_eq!(position, word(vector, 4));
        for i in 0..count {
            store(position, i * 4, word(value, 0));
        }
        store(vector, 4, position + count * 4);
    }
    0
}
extern "thiscall" fn resize(set: u32, count: u32) -> u32 {
    log("resize", set, &[count]);
    assert!(count <= 193);
    0
}
unsafe fn hash_insert(set: u32, value: u32) {
    unsafe {
        let buckets = word(set, 4);
        let n = (word(set, 8) - buckets) / 4;
        let offset = value % n * 4;
        let mut node = word(buckets, offset);
        while node != 0 {
            if word(node, 4) == value {
                return;
            }
            node = word(node, 0);
        }
        let node = memory(8);
        store(node, 0, word(buckets, offset));
        store(node, 4, value);
        store(buckets, offset, node);
        store(set, 16, word(set, 16) + 1);
    }
}
extern "thiscall" fn insert(set: u32, output: u32, value: u32) -> u32 {
    log("insert_building", set, &[unsafe { word(value, 0) }]);
    unsafe {
        hash_insert(set, word(value, 0));
        store(output, 0, set);
    }
    output
}
extern "thiscall" fn family(this: u32, set: u32, id: u32) -> u32 {
    log("family", this, &[id]);
    let buildings = STATE.with(|s| s.borrow().families.get(&id).cloned().unwrap_or_default());
    for building in buildings {
        unsafe {
            hash_insert(set, building);
        }
    }
    0
}
unsafe fn hash_clear(set: u32) {
    unsafe {
        let buckets = word(set, 4);
        let n = (word(set, 8) - buckets) / 4;
        for i in 0..n {
            let mut node = word(buckets, i * 4);
            while node != 0 {
                let next = word(node, 0);
                release(node);
                node = next;
            }
            store(buckets, i * 4, 0);
        }
        store(set, 16, 0);
    }
}
extern "thiscall" fn clear(set: u32) -> u32 {
    log("clear", set, &[unsafe { word(set, 16) }]);
    unsafe {
        hash_clear(set);
    }
    0
}
extern "thiscall" fn copy(to: u32, from: u32) -> u32 {
    log("copy", to, &[from]);
    unsafe {
        assert_eq!(word(to, 16), 0);
        let buckets = word(from, 4);
        let n = (word(from, 8) - buckets) / 4;
        for i in 0..n {
            let mut source = word(buckets, i * 4);
            let mut tail = word(to, 4) + i * 4;
            while source != 0 {
                let node = memory(8);
                store(node, 0, 0);
                store(node, 4, word(source, 4));
                store(tail, 0, node);
                tail = node;
                source = word(source, 0);
            }
        }
        store(to, 16, word(from, 16));
    }
    to
}

struct World {
    td: Object,
    candidate: Object,
    developer: Box<[u32; 1]>,
    city: Box<[u32; 1]>,
    anchor: Box<[u32; 1]>,
    bds: Box<[u32; 1]>,
    configurations: Vec<[u32; 1]>,
    tables: Vec<[usize; 40]>,
    header: Box<[u32; 5]>,
    nodes: Vec<[u32; 5]>,
    style_header: Box<[u32; 6]>,
    style_nodes: Vec<[u32; 6]>,
    percentages: Box<[u32; 2]>,
    primary: Object,
    ids: Vec<u32>,
    debug: Box<[u32; 40]>,
    histogram: Box<[u32; 32]>,
    random: Box<u8>,
    city_global: Box<u32>,
    scale: Box<f32>,
    bias: Box<f32>,
    primes: Box<[u32; 28]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(configuration_count: usize) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180),
            candidate: Object::new(0xA4),
            developer: Box::new([0]),
            city: Box::new([0]),
            anchor: Box::new([0]),
            bds: Box::new([0]),
            configurations: vec![[0]; configuration_count],
            tables: vec![[0; 40]; 5],
            header: Box::new([0; 5]),
            nodes: vec![[0; 5]; configuration_count],
            style_header: Box::new([0; 6]),
            style_nodes: vec![],
            percentages: Box::new([0; 2]),
            primary: Object::new(0x30),
            ids: vec![],
            debug: Box::new([0; 40]),
            histogram: Box::new([0; 32]),
            random: Box::new(0),
            city_global: Box::new(0),
            scale: Box::new(100.0),
            bias: Box::new(12_582_912.0),
            primes: Box::new([
                53, 97, 193, 389, 769, 1543, 3079, 6151, 12289, 24593, 49157, 98317, 196613, 393241, 786433, 1572869,
                3145739, 6291469, 12582917, 25165843, 50331653, 100663319, 201326611, 402653189, 805306457, 1610612741,
                3221225473, 4294967291,
            ]),
            _fakes: vec![],
        };
        for (slot, function) in [
            (0x1C, purpose as *const ()),
            (0x68, styles as *const ()),
            (0x64, mode as *const ()),
        ] {
            w.tables[0][slot / 4] = function as usize;
        }
        w.tables[1][0x90 / 4] = anchors as *const () as usize;
        w.tables[2][0x90 / 4] = needs_anchor as *const () as usize;
        w.tables[3][0x4C / 4] = family as *const () as usize;
        w.tables[3][0x38 / 4] = building_key as *const () as usize;
        w.tables[4][0x48 / 4] = cfg_style as *const () as usize;
        w.tables[4][0x60 / 4] = primary as *const () as usize;
        w.developer[0] = w.tables[0].as_ptr() as u32;
        w.city[0] = w.tables[1].as_ptr() as u32;
        w.anchor[0] = w.tables[2].as_ptr() as u32;
        w.bds[0] = w.tables[3].as_ptr() as u32;
        *w.city_global = w.city.as_ptr() as u32;
        let header = w.header.as_mut_ptr() as u32;
        w.header[1] = 0;
        w.header[2] = header;
        w.header[3] = header;
        let mut values: Vec<_> = w
            .configurations
            .iter_mut()
            .map(|cfg| {
                cfg[0] = w.tables[4].as_ptr() as u32;
                cfg.as_mut_ptr() as u32
            })
            .collect();
        values.sort();
        for (i, &value) in values.iter().enumerate() {
            w.nodes[i] = [
                1,
                if i == 0 { header } else { w.nodes[i - 1].as_ptr() as u32 },
                0,
                0,
                value,
            ];
            if i != 0 {
                w.nodes[i - 1][3] = w.nodes[i].as_ptr() as u32;
            }
        }
        if !values.is_empty() {
            w.header[1] = w.nodes[0].as_ptr() as u32;
            w.header[2] = w.header[1];
            w.header[3] = w.nodes[values.len() - 1].as_ptr() as u32;
        }
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_developer(w.developer.as_ptr() as u32);
        td.set_building_development(w.bds.as_ptr() as u32);
        td.set_configurations_header(header);
        td.set_desired_maximum(100);
        td.set_built(10);
        td.set_chosen_configuration(0xF0F0_F0F0);
        td.set_chosen_building(0xF4F4_F4F4);
        w.debug[0x78 / 4] = w.histogram.as_mut_ptr() as u32;
        td.set_debug_context(w.debug.as_ptr() as u32);
        let candidate = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
        candidate.set_kind(2);
        candidate.set_occupied(5);
        candidate.set_corner(0xFE);
        let style_header = w.style_header.as_mut_ptr() as u32;
        w.style_header[2] = style_header;
        w.percentages[0] = style_header;
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.styles = w.percentages.as_ptr() as u32;
            s.anchor_object = w.anchor.as_ptr() as u32;
            s.primary = w.primary.ptr() as u32;
            s.draws = VecDeque::from([0, 0]);
        });
        for (address, function) in [
            (0x0070_55B0, capacity_fake as *const ()),
            (0x0070_6500, cfg_compatible as *const ()),
            (0x0070_4E00, building_result as *const ()),
            (0x0091_36B1, draw as *const ()),
            (0x0090_CF54, alloc as *const ()),
            (0x0090_CF63, dealloc as *const ()),
            (0x0049_A050, insert_cfg as *const ()),
            (0x0070_49D0, erase_cfg as *const ()),
            (0x0045_D4F0, reserve as *const ()),
            (0x004F_BA20, fill as *const ()),
            (0x004F_0940, resize as *const ()),
            (0x0062_4DD0, insert as *const ()),
            (0x008E_BA60, clear as *const ()),
            (0x0050_DA70, copy as *const ()),
            (0x005E_8F70, group_fake as *const ()),
        ] {
            w._fakes.push(fake(address, function as usize));
        }
        for (address, ptr) in [
            (0x00B4_3D68, (&mut *w.city_global as *mut u32).cast()),
            (0x00B4_AFD8, &mut *w.random),
            (0x00A8_0AA0, (&mut *w.scale as *mut f32).cast()),
            (0x00AB_3048, (&mut *w.bias as *mut f32).cast()),
            (0x00A8_29B0, w.primes.as_mut_ptr().cast()),
        ] {
            w._fakes.push(fake_data(address, ptr));
        }
        w
    }
    fn view(&mut self) -> TractDeveloper {
        unsafe { TractDeveloper::from_ptr(self.td.ptr()) }
    }
    fn with_families(&mut self, families: &[(u32, &[u32])]) {
        self.ids = families.iter().map(|f| f.0).collect();
        let begin = self.ids.as_ptr() as u32;
        unsafe {
            store(self.primary.ptr() as u32, 0x28, begin);
            store(self.primary.ptr() as u32, 0x2C, begin + self.ids.len() as u32 * 4);
        }
        STATE.with(|s| s.borrow_mut().families = families.iter().map(|(k, v)| (*k, v.to_vec())).collect());
    }
    fn with_styles(&mut self, entries: &[(u32, f32)]) {
        self.style_nodes = vec![[0; 6]; entries.len()];
        let header = self.style_header.as_ptr() as u32;
        for (i, &(group, percent)) in entries.iter().enumerate() {
            self.style_nodes[i] = [
                1,
                if i == 0 {
                    header
                } else {
                    self.style_nodes[i - 1].as_ptr() as u32
                },
                0,
                0,
                group,
                percent.to_bits(),
            ];
            if i != 0 {
                self.style_nodes[i - 1][3] = self.style_nodes[i].as_ptr() as u32;
            }
        }
        if !entries.is_empty() {
            self.style_header[1] = self.style_nodes[0].as_ptr() as u32;
            self.style_header[2] = self.style_header[1];
            self.style_header[3] = self.style_nodes[entries.len() - 1].as_ptr() as u32;
        }
        self.percentages[1] = entries.len() as u32;
    }
    fn run(&mut self) -> u32 {
        let before_candidate = crate::tract_developer::testing::snapshot(self.candidate.ptr(), 0xA4);
        let result = unsafe { fc4p_00709ba0(self.td.ptr(), self.candidate.ptr() as u32) };
        assert_eq!(
            crate::tract_developer::testing::snapshot(self.candidate.ptr(), 0xA4),
            before_candidate
        );
        STATE.with(|s| assert!(s.borrow().allocations.is_empty(), "temporary allocation leaked"));
        result
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}

#[test]
fn no_configuration_preserves_prior_choice_and_only_releases_header() {
    let mut w = World::new(0);
    assert_eq!(w.run(), 0);
    let td = w.view();
    assert_eq!(
        (td.chosen_configuration(), td.chosen_building()),
        (0xF0F0_F0F0, 0xF4F4_F4F4)
    );
    assert_eq!(
        w.calls().iter().map(|c| c.0).collect::<Vec<_>>(),
        ["capacity", "purpose", "anchors", "anchor", "styles", "allocate", "free"]
    );
}

#[test]
fn configuration_draw_uses_pointer_order_and_building_draw_uses_hash_order() {
    let _fpu = GameFpu::new();
    let _machine = crate::tract_developer::testing::PreserveMachine::new();
    let mut w = World::new(3);
    w.with_families(&[(41, &[194, 1, 2]), (42, &[1, 3])]);
    STATE.with(|s| s.borrow_mut().draws = VecDeque::from([1, 1]));
    let before = crate::tract_developer::testing::snapshot(w.td.ptr(), 0x180);
    assert_eq!(w.run(), 27);
    let td = w.view();
    assert_eq!(td.chosen_configuration(), w.nodes[1][4]);
    // Family order: 1,194,2 then 1,3; insertion into the result reverses the collision chain.
    assert_eq!(td.chosen_building(), 1);
    assert_eq!(
        (td.minimum_building_capacity(), td.maximum_building_capacity()),
        (6, 95)
    );
    let mut expected = before;
    expected[0xF0..0xF4].copy_from_slice(&td.chosen_configuration().to_le_bytes());
    expected[0xF4..0xF8].copy_from_slice(&1u32.to_le_bytes());
    expected[0xFC..0x100].copy_from_slice(&6u32.to_le_bytes());
    expected[0x100..0x104].copy_from_slice(&95u32.to_le_bytes());
    assert_eq!(crate::tract_developer::testing::snapshot(w.td.ptr(), 0x180), expected);
    let calls = w.calls();
    assert_eq!(
        calls.iter().map(|c| c.0).collect::<Vec<_>>(),
        [
            "capacity",
            "purpose",
            "anchors",
            "anchor",
            "styles",
            "allocate",
            "configuration_compatible",
            "insert_configuration",
            "configuration_compatible",
            "insert_configuration",
            "configuration_compatible",
            "insert_configuration",
            "draw",
            "primary",
            "reserve",
            "fill",
            "reserve",
            "fill",
            "reserve",
            "fill",
            "family",
            "building_compatible",
            "resize",
            "insert_building",
            "building_compatible",
            "resize",
            "insert_building",
            "building_compatible",
            "resize",
            "insert_building",
            "clear",
            "free",
            "reserve",
            "fill",
            "family",
            "building_compatible",
            "resize",
            "insert_building",
            "building_compatible",
            "resize",
            "insert_building",
            "clear",
            "free",
            "draw",
            "clear",
            "free",
            "clear",
            "free",
            "erase_configuration_branch",
            "free",
            "free",
        ]
    );
    assert_eq!(
        calls[0],
        Call("capacity", td.as_ptr() as u32, vec![w.candidate.ptr() as u32])
    );
    assert_eq!(calls[1], Call("purpose", td.developer(), vec![]));
    assert_eq!(calls[2], Call("anchors", w.city.as_ptr() as u32, vec![]));
    assert_eq!(
        calls[3],
        Call(
            "anchor",
            w.anchor.as_ptr() as u32,
            vec![w.candidate.ptr() as u32 + 0x38, 5]
        )
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.0 == "draw")
            .map(|c| c.2[0])
            .collect::<Vec<_>>(),
        [3, 4]
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.0 == "building_compatible")
            .map(|c| c.2[0])
            .collect::<Vec<_>>(),
        [1, 194, 2, 1, 3]
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.0 == "reserve")
            .map(|c| c.2[0])
            .collect::<Vec<_>>(),
        [193; 4]
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.0 == "fill")
            .map(|c| c.2.clone())
            .collect::<Vec<_>>(),
        vec![vec![193, 0]; 4]
    );
    for pair in calls.windows(2) {
        if pair[0].0 == "resize" {
            assert_eq!(pair[1].0, "insert_building");
            assert_eq!(pair[0].1, pair[1].1);
        }
    }
    let last_clears: Vec<_> = calls.iter().filter(|c| c.0 == "clear").map(|c| c.2[0]).collect();
    assert_eq!(last_clears, [3, 2, 0, 4]);
}

#[test]
fn anchor_and_weighted_style_filter_before_compatibility() {
    let _fpu = GameFpu::new();
    for (anchor, draw_a, expected_group) in [
        (true, 99, 0x3000),
        (false, 24, 0x1000),
        (false, 25, 0x2000),
        (false, 74, 0x2000),
        (false, 75, 0),
    ] {
        let mut w = World::new(2);
        w.with_styles(&[(0x1000, 0.25), (0x2000, 0.5)]);
        w.with_families(&[(41, &[7, 8])]);
        let rejected = w.nodes[0][4];
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.anchor = anchor;
            s.rejected_styles = vec![rejected];
            s.rejected_buildings = vec![7];
            s.draws = if anchor {
                VecDeque::from([0, 0])
            } else {
                VecDeque::from([draw_a, 0, 0])
            };
        });
        assert_eq!(w.run(), 27);
        let calls = w.calls();
        let style_calls: Vec<_> = calls.iter().filter(|c| c.0 == "configuration_style").collect();
        if expected_group == 0 {
            assert!(style_calls.is_empty());
            assert_eq!(w.histogram[9], 0);
        } else {
            assert_eq!(style_calls.len(), 2);
            assert!(style_calls.iter().all(|c| c.2 == [expected_group]));
            assert_eq!(w.histogram[9], 2);
            assert_eq!(w.view().chosen_configuration(), w.nodes[1][4]);
            assert_eq!(w.view().chosen_building(), 8);
        }
        assert_eq!(calls.iter().filter(|c| c.0 == "styles").count(), usize::from(!anchor));
        let rect = w.candidate.ptr() as u32 + 0x38;
        assert!(calls.iter().any(|c| c.0 == "anchor" && c.2 == [rect, 5]));
        for c in calls.iter().filter(|c| c.0 == "configuration_compatible") {
            assert_eq!(c.2[2], 0xFE);
        }
    }
}

#[test]
fn monopoly_fallback_requires_mode_one_and_retains_last_failure() {
    for mode in [0, 1, 2, u32::MAX] {
        let mut w = World::new(1);
        w.with_families(&[(41, &[5, 6])]);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.mode = mode;
            s.outcomes = BTreeMap::from([(5, 2), (6, 13)]);
        });
        assert_eq!(w.run(), if mode == 1 { 27 } else { 13 });
        let td = w.view();
        assert_eq!(td.chosen_building(), if mode == 1 { 5 } else { 0xF4F4_F4F4 });
        let calls = w.calls();
        assert_eq!(calls.iter().filter(|c| c.0 == "mode").count(), 1);
        assert_eq!(calls.iter().filter(|c| c.0 == "copy").count(), usize::from(mode == 1));
        if mode == 1 {
            let copy = calls.iter().position(|c| c.0 == "copy").unwrap();
            assert_eq!(calls[copy - 1].0, "clear");
            assert_eq!(calls[copy + 1].0, "draw");
        }
    }
    let mut w = World::new(1);
    w.with_families(&[(41, &[])]);
    assert_eq!(w.run(), 0);
    assert_eq!(w.view().chosen_building(), 0xF4F4_F4F4);
}

#[test]
fn capacity_window_wraps_and_subdivision_uses_unsigned_piece_byte() {
    for (occupied, desired, built, pieces, expected) in [
        (i32::MAX, i32::MAX, i32::MIN, 0, (i32::MAX, i32::MAX - 1)),
        (5, 0, 0, 0, (6, 7)),
        (5, 100, 10, 3, (2, 95)),
        (i32::MIN, i32::MIN, i32::MAX, 255, (-8_421_504, -8_421_503)),
    ] {
        let mut w = World::new(1);
        let td = w.view();
        td.set_desired_maximum(desired);
        td.set_built(built);
        let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
        c.set_occupied(occupied);
        if pieces != 0 {
            c.set_kind(4);
            c.set_access_flag(pieces);
        }
        w.with_families(&[]);
        assert_eq!(w.run(), 0);
        assert_eq!(
            (td.minimum_building_capacity(), td.maximum_building_capacity()),
            expected
        );
    }
}

#[test]
fn rejected_configurations_stop_before_building_sets_and_success_prevents_monopoly_copy() {
    let mut w = World::new(2);
    STATE.with(|s| s.borrow_mut().rejected_configurations = w.nodes.iter().map(|node| node[4]).collect());
    assert_eq!(w.run(), 0);
    let calls = w.calls();
    assert_eq!(calls.iter().filter(|c| c.0 == "configuration_compatible").count(), 2);
    assert!(!calls
        .iter()
        .any(|c| matches!(c.0, "draw" | "reserve" | "insert_configuration")));
    let mut w = World::new(1);
    w.with_families(&[(41, &[1, 2])]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.mode = 1;
        s.outcomes.insert(1, 2);
    });
    assert_eq!(w.run(), 27);
    assert_eq!(w.view().chosen_building(), 2);
    assert!(!w.calls().iter().any(|c| c.0 == "copy"));
}

#[test]
fn style_rounding_matches_the_core_points_and_retains_x87_control_mode() {
    let _fpu = GameFpu::new();
    let _machine = crate::tract_developer::testing::PreserveMachine::new();
    for (percentage, draw_a, matched) in [
        (0.3, 29, true),
        (0.3, 30, false),
        (0.005, 0, false),
        (0.015, 1, true),
        (-0.5, 99, true),
        (f32::INFINITY, 99, true),
        (f32::NAN, 99, true),
    ] {
        let mut w = World::new(0);
        w.with_styles(&[(0x1000, percentage)]);
        // A rejected style query makes the chosen group visible even though no configuration survives.
        let mut configuration = [w.tables[4].as_ptr() as u32];
        let cfg = configuration.as_mut_ptr() as u32;
        w.nodes = vec![[1, w.header.as_ptr() as u32, 0, 0, cfg]];
        w.header[1] = w.nodes[0].as_ptr() as u32;
        w.header[2] = w.header[1];
        w.header[3] = w.header[1];
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.rejected_styles.push(cfg);
            s.rejected_configurations.push(cfg);
            s.draws = VecDeque::from([draw_a]);
        });
        assert_eq!(w.run(), 0);
        let calls = w.calls();
        assert_eq!(
            calls.iter().any(|c| c.0 == "configuration_style"),
            matched,
            "{percentage:?} draw {draw_a}"
        );
        assert_eq!(w.histogram[9], u32::from(matched));
    }
}

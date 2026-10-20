// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The effects manager's tests (`docs/specs/effects.md` §17).

use super::brush::{BrushParams, EffectsRandom};
use super::component_type;
use super::directory::{ComponentDesc, EffectDesc, EffectDirectory, NameEntry};
use super::game::{GameAction, GameEffect, PauseAction};
use super::manager::{EffectsHost, EffectsManager, StartMode};
use super::shipped::{self, GameRecord};

/// `EffectsHost` needs `&self` for `post`, so the log is behind a cell.
use std::cell::RefCell;

struct Host {
    dilation: f32,
    cells: (u32, u32),
    log: RefCell<Vec<String>>,
}

impl Host {
    fn new() -> Host {
        Host {
            dilation: 1.0,
            cells: (64, 64),
            log: RefCell::new(Vec::new()),
        }
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }

    fn posts(&self) -> usize {
        self.log.borrow().iter().filter(|l| l.starts_with("post")).count()
    }

    fn push(&self, s: impl Into<String>) {
        self.log.borrow_mut().push(s.into());
    }
}

impl EffectsHost for Host {
    fn animation_time_dilation(&self) -> f32 {
        self.dilation
    }
    fn city_cells(&self) -> (u32, u32) {
        self.cells
    }
    fn location_in_bounds(&self, x: f32, z: f32) -> bool {
        x >= 0.0 && z >= 0.0 && x < self.cells.0 as f32 * 16.0 && z < self.cells.1 as f32 * 16.0
    }
    fn post(&self, msg_type: u32, data1: u64, _data2: u32, _data3: u32) {
        self.push(format!("post {msg_type:08X} {data1}"));
    }
    fn begin_long_terrain_operation(&self) {
        self.push("long-begin");
    }
    fn end_long_terrain_operation(&self) {
        self.push("long-end");
    }
    fn apply_terrain_brush(&self, brush: u32, params: &BrushParams) {
        self.push(format!(
            "brush {brush:08X} redisplay={} strength={}",
            params.redisplay, params.strength
        ));
    }
    fn perform(&self, action: &GameAction) {
        self.push(format!("{action:?}"));
    }
    fn pause(&self, action: PauseAction) {
        self.push(format!("{action:?}"));
    }
}

/// A generator that returns a fixed value, so the select draw and the brush
/// draws are deterministic.
struct Fixed(f64);
impl EffectsRandom for Fixed {
    fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        if (a - b).abs() < f64::EPSILON {
            a
        } else {
            self.0
        }
    }
}

/// §10.1: the nine `volcanocreator*` children of `volcano_control`, in the
/// order the select group lists them.
const CREATOR_NAMES: [&str; 9] = [
    "volcanocreator1a",
    "volcanocreator1b",
    "volcanocreator2a",
    "volcanocreator2b",
    "volcanocreator3a",
    "volcanocreator3b",
    "volcanocreator4a",
    "volcanocreator4b",
    "volcanocreator5",
];

fn component(kind: u8, index: u32) -> ComponentDesc {
    ComponentDesc {
        name: Vec::new(),
        kind,
        flags: 0,
        lod_min: 1,
        lod_max: 6,
        select_group: 0,
        select_weight: 0,
        index,
    }
}

fn named(index: u32, name: &str) -> NameEntry {
    NameEntry {
        name: name.as_bytes().to_vec(),
        description_index: index,
    }
}

fn desc(name: &str, priority: u32, components: Vec<ComponentDesc>) -> EffectDesc {
    EffectDesc {
        flags: 0,
        priority,
        components,
        name: name.as_bytes().to_vec(),
    }
}

/// A directory with one effect `fx`, holding one brush component (brush record
/// 2: rate 30, length 3.0, a stroke) and one game component (record 3).
fn simple_directory() -> EffectDirectory {
    EffectDirectory::from_parts(
        vec![desc(
            "fx",
            1,
            vec![component(component_type::BRUSH, 2), component(component_type::GAME, 3)],
        )],
        vec![named(0, "fx")],
    )
}

/// §17 test 9: two descriptions named `x`, with priorities 1 and 3. With max
/// priority 2, `x` resolves to the priority-1 one. With max priority 0,
/// `CreateVisualEffect("x")` returns true with index −1, and the effect never
/// posts its end message.
#[test]
fn the_priority_filter_through_create_visual_effect() {
    let dir = EffectDirectory::from_parts(
        vec![desc("x", 1, vec![]), desc("x", 3, vec![])],
        vec![named(0, "x"), named(1, "x")],
    );

    let mut m = EffectsManager::new(dir.clone());
    m.set_max_priority(2);
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    let id = m.create_visual_effect(b"x", &mut host, &mut random).expect("created");
    assert_eq!(m.effect(id).unwrap().index, 0, "the priority-1 description");

    let mut m = EffectsManager::new(dir);
    m.set_max_priority(3);
    let id = m.create_visual_effect(b"x", &mut host, &mut random).expect("created");
    assert_eq!(m.effect(id).unwrap().index, 1, "the priority-3 description");

    // max priority 0: the call still succeeds with index -1.
    let mut m = EffectsManager::new(EffectDirectory::from_parts(
        vec![desc("x", 1, vec![])],
        vec![named(0, "x")],
    ));
    m.set_max_priority(0);
    let id = m
        .create_visual_effect(b"x", &mut host, &mut random)
        .expect("§4.1 step 3.3: the call still succeeds");
    assert_eq!(m.effect(id).unwrap().index, -1);

    // ...and it never posts its end message (§4.1 quirk).
    m.set_end_message(id, 0xCA49_3C1D, 7);
    assert!(m.start(id, StartMode::Normal, &mut host));
    for _ in 0..200 {
        m.animation_tick(33_333, &mut host, &mut random);
    }
    let finished = m.draw_pass_sweep(&mut host);
    assert_eq!(finished.len(), 1, "the draw pass counts it as done at once");
    assert_eq!(host.posts(), 0, "an index -1 effect never posts its end message");
}

/// §17 test 10: **no end without the draw pass**. Start an effect with an end
/// message, then tick past its brush length without calling the draw pass: no
/// end message may be posted. EF2 quirk 1 is proven (§7, §15), so the finish
/// test is unreachable from the tick.
#[test]
fn no_end_without_the_draw_pass() {
    let mut m = EffectsManager::new(simple_directory());
    let mut host = Host::new();
    let mut random = Fixed(1.0);

    let id = m.create_visual_effect(b"fx", &mut host, &mut random).expect("created");
    m.set_end_message(id, 0xCA49_3C1D, 42);
    assert!(m.start(id, StartMode::Normal, &mut host));
    assert!(m.is_started(id));
    host.take();

    // Tick well past the brush record's 3.0 effect-seconds at dilation 1.
    for _ in 0..200 {
        m.animation_tick(33_333, &mut host, &mut random);
    }
    assert_eq!(
        host.posts(),
        0,
        "the tick never finishes an effect and never posts (EF2 quirk 1)"
    );
    assert!(m.is_started(id), "it is still started until a draw pass runs");

    // The draw pass is where it finishes and posts.
    let finished = m.draw_pass_sweep(&mut host);
    assert_eq!(finished, vec![id]);
    assert_eq!(host.posts(), 1, "exactly one queued end message");
    assert!(host.take().iter().any(|l| l == "post CA493C1D 42"));
    assert!(!m.is_started(id), "Stop(1) clears started, so it never posts twice");
}

/// §17 test 11: the finish rule through the manager. A component whose
/// description bit is clear holds the effect while it is active; one whose bit
/// is set never does.
#[test]
fn the_finish_rule_through_the_manager() {
    // An effect whose only component is an ignored game record never blocks.
    let dir = EffectDirectory::from_parts(
        vec![desc(
            "g",
            1,
            vec![ComponentDesc {
                flags: 1, // the ignore bit (§7)
                ..component(component_type::GAME, 3)
            }],
        )],
        vec![named(0, "g")],
    );
    let mut m = EffectsManager::new(dir);
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    let id = m.create_visual_effect(b"g", &mut host, &mut random).expect("created");
    m.set_end_message(id, 1, 0);
    m.start(id, StartMode::Normal, &mut host);
    // The game record is started, but it is ignored, so it cannot hold the
    // effect.
    assert!(m.effect(id).unwrap().components[0].is_active());
    assert!(m.effect(id).unwrap().components[0].ignored());
    assert_eq!(m.draw_pass_sweep(&mut host), vec![id]);
    assert_eq!(host.posts(), 1);

    // The same with the bit clear: the active component holds it.
    let dir = EffectDirectory::from_parts(
        vec![desc("g", 1, vec![component(component_type::GAME, 3)])],
        vec![named(0, "g")],
    );
    let mut m = EffectsManager::new(dir);
    let id = m.create_visual_effect(b"g", &mut host, &mut random).expect("created");
    m.set_end_message(id, 1, 0);
    m.start(id, StartMode::Normal, &mut host);
    host.take();
    assert!(m.effect(id).unwrap().components[0].blocks_finish());
    assert!(m.draw_pass_sweep(&mut host).is_empty(), "it is still running");
    assert_eq!(host.posts(), 0);

    // Once the component is inactive it finishes and posts once.
    m.stop(id, 1, &mut host);
    m.start(id, StartMode::Normal, &mut host);
    let c = &mut m.effects_mut_for_test(id).components[0];
    if let super::manager::ComponentState::Game(g) = &mut c.state {
        *g = GameEffect::new(GameRecord {
            flags: 8, // single: applies once, then inactive
            ..shipped::GAME_RECORDS[0]
        });
    }
    m.animation_tick(33_333, &mut host, &mut random);
    assert!(!m.effect(id).unwrap().components[0].is_active());
    assert_eq!(m.draw_pass_sweep(&mut host), vec![id]);
    assert_eq!(host.posts(), 1);
}

/// §6: the tick dilates the step by the simulator's animation time dilation and
/// keeps the totals at `+0x1B8` (µs) and `+0x1BC` (seconds). It does **not**
/// stop while the game is paused (§15 quirk 3) — that is the host's dilation,
/// which is 1 while paused.
#[test]
fn the_tick_dilates_and_keeps_the_totals() {
    let mut m = EffectsManager::new(simple_directory());
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    m.create_visual_effect(b"fx", &mut host, &mut random).expect("created");

    host.dilation = 1.0;
    m.animation_tick(33_333, &mut host, &mut random);
    assert_eq!(m.totals(), (33_333, 0.033333), "dilation 1 adds us verbatim");

    host.dilation = 1.5;
    m.animation_tick(33_333, &mut host, &mut random);
    // trunc(r24(33333 * 1.5)) = trunc(49999.5) = 49999.
    assert_eq!(m.totals().0, 33_333 + 49_999);

    host.dilation = 3.0;
    m.animation_tick(33_333, &mut host, &mut random);
    assert_eq!(m.totals().0, 33_333 + 49_999 + 99_999);
    assert!((m.totals().1 - 0.183331).abs() < 1e-4, "seconds = {}", m.totals().1);
}

/// §6: the effect's components update in component order, and only those whose
/// `IsActive` is true. The brush acts inside the tick and applies its brushes.
#[test]
fn the_tick_drives_the_brush_component() {
    let dir = EffectDirectory::from_parts(
        vec![desc("fx", 1, vec![component(component_type::BRUSH, 2)])],
        vec![named(0, "fx")],
    );
    let mut m = EffectsManager::new(dir);
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    let id = m.create_visual_effect(b"fx", &mut host, &mut random).expect("created");
    m.start(id, StartMode::Normal, &mut host);
    host.take();

    m.animation_tick(100_000, &mut host, &mut random);
    let log = host.take();
    assert_eq!(log[0], "long-begin", "the stroke opens a long terrain operation");
    let brushes: Vec<&String> = log.iter().filter(|l| l.starts_with("brush")).collect();
    assert_eq!(brushes.len(), 3, "100,000 us at dilation 1 applies exactly 3");
    assert!(brushes[0].contains("redisplay=false"), "{}", brushes[0]);
    assert!(brushes[1].contains("redisplay=false"), "{}", brushes[1]);
    assert!(brushes[2].contains("redisplay=true"), "redisplay is on the last");
}

/// §10.1: `volcano_control` cannot finish before a **previous** sweep finished
/// its chosen child, because a meta component's `IsActive` is the child's
/// `started` byte. It waits a full extra sweep.
#[test]
fn a_meta_parent_waits_a_full_extra_sweep() {
    let dir = EffectDirectory::from_parts(
        vec![
            // 0: the parent, one meta component pointing at "child".
            desc(
                "parent",
                1,
                vec![ComponentDesc {
                    name: b"child".to_vec(),
                    ..component(component_type::META, 0)
                }],
            ),
            // 1: the child, one brush stroke.
            desc("child", 1, vec![component(component_type::BRUSH, 2)]),
        ],
        vec![named(0, "parent"), named(1, "child")],
    );
    let mut m = EffectsManager::new(dir);
    let mut host = Host::new();
    let mut random = Fixed(1.0);

    let parent = m
        .create_visual_effect(b"parent", &mut host, &mut random)
        .expect("created");
    m.set_end_message(parent, 0xCA49_3C1D, 99);
    m.start(parent, StartMode::Normal, &mut host);
    // §5.1: the meta child lands on the new list and is built in the next
    // round, so one activation pass settles both.
    m.activate_new(&mut host, &mut random);
    assert_eq!(m.new_list().len(), 0, "the child was activated");

    let child = *m
        .active_list()
        .iter()
        .find(|&&id| id != parent)
        .expect("the child is on the active list");
    assert!(m.is_started(child), "the parent's Start started its components");

    // Tick past the child's brush length; both are still started.
    for _ in 0..200 {
        m.animation_tick(33_333, &mut host, &mut random);
    }
    host.take();

    // First sweep: the child finishes (its brush is done), the parent cannot.
    let finished = m.draw_pass_sweep(&mut host);
    assert_eq!(finished, vec![child], "only the child finishes on this sweep");
    assert!(!m.is_started(child));
    assert!(m.is_started(parent), "the parent is still started");
    assert_eq!(host.posts(), 0, "the child posts nothing");

    // Second sweep: the meta component now reports the child's `started` as 0,
    // so the parent finishes and posts.
    let finished = m.draw_pass_sweep(&mut host);
    assert_eq!(finished, vec![parent], "one full extra sweep later");
    assert_eq!(host.posts(), 1, "exactly one end message");
    assert!(host.take().iter().any(|l| l == "post CA493C1D 99"));
}

/// §5.1: activation is deferred while the draw pass is sweeping (`+0x1B4`).
#[test]
fn activation_is_deferred_while_the_draw_pass_sweeps() {
    let mut m = EffectsManager::new(simple_directory());
    let mut host = Host::new();
    let mut random = Fixed(1.0);

    let id = m.create_visual_effect(b"fx", &mut host, &mut random).expect("created");
    m.stop(id, 1, &mut host);
    m.set_end_message(id, 1, 0);
    m.start(id, StartMode::Normal, &mut host);

    // While sweeping, a newly created effect stays on the new list.
    m.begin_sweep_for_test();
    assert!(m.sweeping());
    let late = m.create_visual_effect(b"fx", &mut host, &mut random).expect("created");
    assert!(m.new_list().contains(&late), "it waits for the next activation");
    m.end_sweep_for_test();
    m.activate_new(&mut host, &mut random);
    assert!(m.active_list().contains(&late));
}

/// §5.2: a component exists only while the level of detail is in its range, and
/// the shipped data means the damage does not depend on the zoom for the brush
/// and game records (they are all `[1, 6)`).
#[test]
fn components_follow_the_level_of_detail() {
    // §2: `max(zoom - offset + 1, 1)` with a 0-based zoom.
    assert_eq!(EffectsManager::level_of_detail(0, 0), 1);
    assert_eq!(EffectsManager::level_of_detail(4, 0), 5);
    assert_eq!(EffectsManager::level_of_detail(4, 2), 3);
    assert_eq!(EffectsManager::level_of_detail(0, 2), 1, "never below 1");
    // ...and the user-facing zoom level 1-5 maps to LOD 1-5 at offset 0.
    for level in 1..=5u8 {
        assert_eq!(EffectsManager::lod_for_zoom_level(level, 0), level);
    }

    let dir = EffectDirectory::from_parts(
        vec![desc(
            "fx",
            1,
            vec![
                ComponentDesc {
                    lod_min: 1,
                    lod_max: 3,
                    ..component(component_type::BRUSH, 2)
                },
                ComponentDesc {
                    lod_min: 4,
                    lod_max: 6,
                    ..component(component_type::GAME, 3)
                },
            ],
        )],
        vec![named(0, "fx")],
    );
    let mut m = EffectsManager::new(dir);
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    let id = m.create_visual_effect(b"fx", &mut host, &mut random).expect("created");
    assert_eq!(m.effect(id).unwrap().components.len(), 1, "only the brush at LOD 1");
    assert_eq!(m.effect(id).unwrap().components[0].desc.kind, component_type::BRUSH);

    m.set_zoom(3, &mut host, &mut random); // LOD 4
                                           // set_zoom rebuilds through UpdateActiveVisualEffects; rebuild explicitly.
    m.build_components_for_test(id, &mut host, &mut random);
    assert_eq!(m.effect(id).unwrap().components.len(), 1);
    assert_eq!(m.effect(id).unwrap().components[0].desc.kind, component_type::GAME);
}

/// §5.3 through the manager: a select group builds exactly one member, and a
/// fresh manager's first draw picks `volcanocreator4a` (§17 test 3).
#[test]
fn a_select_group_builds_exactly_one_member() {
    let weights = [7281u16, 7281, 7281, 7281, 7281, 7281, 7281, 7281, 7287];
    let mut components = Vec::new();
    for (i, w) in weights.iter().enumerate() {
        components.push(ComponentDesc {
            name: CREATOR_NAMES[i].as_bytes().to_vec(),
            kind: component_type::META,
            flags: 0,
            lod_min: 1,
            lod_max: 6,
            select_group: 1,
            select_weight: *w,
            index: 0,
        });
    }
    let dir = EffectDirectory::from_parts(
        vec![desc("volcano_control", 1, components)],
        vec![named(0, "volcano_control")],
    );
    let mut m = EffectsManager::new(dir);
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    let id = m
        .create_visual_effect(b"volcano_control", &mut host, &mut random)
        .expect("created");
    let built = &m.effect(id).unwrap().components;
    assert_eq!(built.len(), 1, "exactly one creator is built");
    // §17 test 3: r = 43750 picks the 7th member, volcanocreator4a.
    assert_eq!(built[0].desc.name, b"volcanocreator4a");
}

/// §7: an effect that is not started counts as done at once and posts nothing.
#[test]
fn an_unstarted_effect_counts_as_done_without_a_message() {
    let mut m = EffectsManager::new(simple_directory());
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    let id = m.create_visual_effect(b"fx", &mut host, &mut random).expect("created");
    m.set_end_message(id, 7, 1);
    host.take();
    assert_eq!(m.draw_pass_sweep(&mut host), vec![id]);
    assert_eq!(host.posts(), 0, "rule 1: not started, so nothing is posted");
}

/// §7: the end message is posted **once**, because `Stop(1)` clears `started`
/// and rule 1 then applies.
#[test]
fn the_end_message_is_posted_exactly_once() {
    let dir = EffectDirectory::from_parts(
        vec![desc("g", 1, vec![component(component_type::GAME, 1)])],
        vec![named(0, "g")],
    );
    let mut m = EffectsManager::new(dir);
    let mut host = Host::new();
    let mut random = Fixed(1.0);
    // Game record 1 is `single`: it applies once and goes inactive.
    let id = m.create_visual_effect(b"g", &mut host, &mut random).expect("created");
    m.set_end_message(id, 0x2A7B_582D, 5);
    m.start(id, StartMode::Normal, &mut host);
    host.take();

    for _ in 0..4 {
        m.animation_tick(33_333, &mut host, &mut random);
        m.draw_pass_sweep(&mut host);
    }
    assert_eq!(host.posts(), 1, "posted once and never again");
}

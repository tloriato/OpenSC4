// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The effects manager itself (`docs/specs/effects.md` §1, §2, §4, §5, §6,
//! §7): `cSC4EffectsManager`, CLSID `0x49822F75`, held at city `+0x1FC` and
//! returned by city slot 85.
//!
//! # What is modelled
//!
//! - **Creation** (§4): by name or by ID pair, with the priority filter of
//!   `FindEffectDesc` and the `index = −1` case that still succeeds.
//! - **Activation and component building** (§5.1, §5.2, §5.3): the LOD window
//!   and the random select groups.
//! - **The tick** (§6): the dilated step, the manager totals, and each started
//!   effect's components in order.
//! - **The finish sweep and the end post** (§7).
//!
//! # What is not
//!
//! Drawing (particles, decals, models, flashes, tints, camera shakes, the
//! snowline and the effect maps), sound playback and the automata attractors
//! are out of scope (§Scope). Animations stay stubbed. The components those
//! kinds own are still **counted by the finish test**, because that is a
//! simulation-visible fact (§7), so they are modelled as Start/Stop-driven
//! active flags.
//!
//! # The draw pass
//!
//! **EF2 quirk 1 is proven (§7, §15):** an effect finishes and posts its end
//! message **only in the 3D view's draw pass**. The finish test has exactly one
//! caller (`DeactivateUnusedVisualEffects`), which has exactly one caller
//! (`Update(SC4DrawContext*)`), which has none. This manager therefore keeps
//! the sweep behind [`EffectsManager::draw_pass_sweep`] and **never** finishes
//! an effect from the tick. A replacement that must work without a 3D view has
//! to run the same sweep from somewhere else; that is a caller's decision, not
//! this core's.

use super::brush::{BrushEffect, BrushParams, EffectsRandom};
use super::directory::{ComponentDesc, EffectDirectory, Found};
use super::game::{GameAction, GameEffect, PauseAction};
use super::select::{select_components, select_generator, SelectPlacement};
use super::{component_type, flags, runtime_flags, EffectRuntime, SweepResult};
use crate::rng::RzRandomSuper;

/// The handle of a live effect in the manager's arena.
pub type EffectId = u32;

/// Everything the effects manager reads from, or does to, the rest of the game.
pub trait EffectsHost {
    // ---- simulator (§6) ----
    /// `cISC4Simulator::GetAnimationTimeDilation` (slot 32; simulator.md §7).
    /// Shipped: 1 while paused or emergency-paused, 1.5 at speed 1, 3 at speed
    /// 2, and 1 otherwise.
    fn animation_time_dilation(&self) -> f32;

    // ---- city (§8.3, §9.3) ----
    /// The city's size in cells (city slots 93 and 94).
    fn city_cells(&self) -> (u32, u32);
    /// City slot 98 `LocationIsInBounds`.
    fn location_in_bounds(&self, x: f32, z: f32) -> bool;

    // ---- messages (§12) ----
    /// Post a **queued** standard message.
    fn post(&self, msg_type: u32, data1: u64, data2: u32, data3: u32);

    // ---- terrain (§8.3) ----
    /// Terrain slot 87 `BeginLongTerrainOperation`.
    fn begin_long_terrain_operation(&self);
    /// Terrain slot 88 `EndLongTerrainOperation`.
    fn end_long_terrain_operation(&self);
    /// Terrain slot 57 `ApplyBrush`.
    fn apply_terrain_brush(&self, brush: u32, params: &BrushParams);

    // ---- the game effect (§9.2, §9.3) ----
    /// Perform one action of a game effect's `Apply`, in the order §9.3 gives.
    fn perform(&self, action: &GameAction);
    /// Raise or lift one of §9.2's pauses.
    fn pause(&self, action: PauseAction);
}

/// A live component of an effect (§7).
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentState {
    /// A brush effect (§8).
    Brush(BrushEffect),
    /// A game effect (§9).
    Game(GameEffect),
    /// A meta component: a nested visual effect (§3 type 2). Its `IsActive` is
    /// **the child effect's `started` byte**, not the child's components'
    /// `IsActive` (§7, §10.1), so a parent cannot finish before a previous
    /// sweep finished the child.
    Meta {
        child: EffectId,
        /// The child effect's `started` byte, mirrored here for the finish
        /// test.
        child_started: bool,
    },
    /// A component kind whose rendering is out of scope: particles, decals,
    /// sequences, sounds, cameras, attractors and models. §7 still counts
    /// those whose ignore bit is clear, so the active flag is modelled and
    /// driven by `Start`/`Stop`. Animations stay stubbed.
    Stub { kind: u8, active: bool },
}

/// One runtime component record (§7: `0x44` bytes, `+0x40` the flags).
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeComponent {
    /// The description it was built from.
    pub desc: ComponentDesc,
    /// `+0x40`: the **runtime** flags the finish test tests (§7).
    pub flags: u8,
    /// The component's own state.
    pub state: ComponentState,
}

impl RuntimeComponent {
    /// Builds a runtime record from its description (§7, conformance test 12).
    pub fn from_description(desc: ComponentDesc) -> RuntimeComponent {
        let flags = runtime_flags(desc.flags, desc.kind);
        RuntimeComponent {
            desc,
            flags,
            state: ComponentState::Stub { kind: 0, active: false },
        }
    }

    /// Whether the finish test skips this component (§7): `flags & 4`.
    pub fn ignored(&self) -> bool {
        self.flags & flags::IGNORED != 0
    }

    /// The component's `IsActive` (component vtable slot 2).
    pub fn is_active(&self) -> bool {
        match &self.state {
            ComponentState::Brush(b) => b.is_active(),
            ComponentState::Game(g) => g.is_active(),
            ComponentState::Meta { child_started, .. } => *child_started,
            ComponentState::Stub { active, .. } => *active,
        }
    }

    /// Whether this component blocks the finish (§7 rule 3).
    pub fn blocks_finish(&self) -> bool {
        !self.ignored() && self.is_active()
    }

    /// The view the pure finish rule takes (§7).
    pub fn as_rule(&self) -> super::RuntimeComponent {
        super::RuntimeComponent {
            flags: self.flags,
            is_active: self.is_active(),
        }
    }
}

/// A live visual effect (§11.1). Offsets are the runtime object's.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualEffect {
    /// `+0x20`: the description index, −1 when the name resolved to nothing.
    pub index: i32,
    /// `+0x24`.
    pub started: bool,
    /// `+0x25`: a soft stop (`Stop(0)`) is in progress.
    pub stopping: bool,
    /// `+0x28`.
    pub flags: u32,
    /// `+0x2C`: 0 means "post nothing".
    pub end_message_type: u32,
    /// `+0x30`.
    pub end_message_data1: u64,
    /// `+0x08`: the reference count. The manager holds one; a caller may hold
    /// more.
    pub refcount: u32,
    /// Its lowercased name (§4.1 step 2).
    pub name: Vec<u8>,
    /// `+0x114`..`+0x118`: the runtime component array, in component order.
    pub components: Vec<RuntimeComponent>,
    /// The level of detail the components were last built for (§5.2).
    pub built_for_lod: Option<u8>,
}

impl VisualEffect {
    /// An effect created by §4.1 step 4, holding the name and the index.
    pub fn new(name: Vec<u8>, index: i32, flags: u32) -> VisualEffect {
        VisualEffect {
            index,
            started: false,
            stopping: false,
            flags,
            end_message_type: 0,
            end_message_data1: 0,
            refcount: 1,
            name,
            components: Vec::new(),
            built_for_lod: None,
        }
    }

    /// §7 rule 1: counted as done at once, with no stop and no message.
    pub fn counts_as_done_immediately(&self) -> bool {
        !self.started || self.index < 0
    }

    /// The effect's runtime view for the pure finish rule (§7).
    pub fn as_rule(&self) -> EffectRuntime {
        EffectRuntime {
            index: self.index,
            started: self.started,
            stopping: self.stopping,
            flags: self.flags,
            end_message_type: self.end_message_type,
            end_message_data1: self.end_message_data1,
        }
    }

    /// One draw-pass sweep of this effect (§7).
    pub fn sweep(&self) -> SweepResult {
        self.as_rule().sweep(
            &self
                .components
                .iter()
                .map(RuntimeComponent::as_rule)
                .collect::<Vec<_>>(),
        )
    }
}

/// The manager's own state (§2).
pub struct EffectsManager {
    /// `+0xB90`: the highest description priority allowed (§4.1). 5 at
    /// construction, then set from the render properties.
    max_priority: u32,
    /// `+0xC84`: `max(zoom − offset + 1, 1)`.
    lod: u8,
    /// `+0xC8C`: the LOD offset from the render properties.
    lod_offset: u8,
    /// `+0x1C0`: the select generator, seeded `0x1105` at construction.
    select: RzRandomSuper,
    /// `+0x1B8`: total dilated microseconds.
    total_us: u64,
    /// `+0x1BBC`-ish (`+0x1BC`): the same in seconds.
    total_seconds: f32,
    /// `+0x1B4`: set while the draw pass sweeps the active list. Activation
    /// (§5.1) is deferred while it is set.
    sweeping: bool,
    /// `+0xBAC`..`+0xBB0`: indices of live effects, in list order.
    active: Vec<EffectId>,
    /// `+0xBA0`..`+0xBA4`: effects not yet activated.
    pending: Vec<EffectId>,
    /// The arena.
    effects: Vec<Option<VisualEffect>>,
    /// The directory.
    directory: EffectDirectory,
}

/// What `Start(mode)` did (§11.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartMode {
    /// Mode 0: the usual start.
    Normal,
    /// Mode 1: a component entering the LOD range of an already-started effect
    /// (§5.2).
    Component,
    /// Mode 2: does not register the effect with the manager.
    NoRegister,
}

impl EffectsManager {
    /// A manager built the way §2 builds it: priority 5, LOD 1, the select
    /// generator seeded `0x1105`.
    pub fn new(directory: EffectDirectory) -> EffectsManager {
        EffectsManager {
            max_priority: 5,
            lod: 1,
            lod_offset: 0,
            select: select_generator(),
            total_us: 0,
            total_seconds: 0.0,
            sweeping: false,
            active: Vec::new(),
            pending: Vec::new(),
            effects: Vec::new(),
            directory,
        }
    }

    /// `+0xB90`, set from the render properties by `DoRenderPropertiesChanged`
    /// (§2). The construction value is 5.
    pub fn max_priority(&self) -> u32 {
        self.max_priority
    }

    /// The effects detail option (§4.1).
    pub fn set_max_priority(&mut self, max_priority: u32) {
        self.max_priority = max_priority;
    }

    /// `+0xC84`: `max(zoom − offset + 1, 1)` (§2).
    ///
    /// §2 also says "Zoom 1-5 gives 1-5 when the offset is 0". The formula and
    /// that sentence agree only if `zoom` is **0-based** (0-4 for the five zoom
    /// levels), which is how it is taken here. *[choice]* — the alternative is
    /// that the `+ 1` is not there and `zoom` is 1-based; both give the same
    /// five LOD values. See the lane report's question on §2.
    pub fn level_of_detail(zoom: u8, offset: u8) -> u8 {
        zoom.saturating_sub(offset).saturating_add(1).max(1)
    }

    /// The level of detail of a user-facing zoom level 1-5 (§2).
    pub fn lod_for_zoom_level(level: u8, offset: u8) -> u8 {
        Self::level_of_detail(level.saturating_sub(1), offset)
    }

    /// The current level of detail.
    pub fn lod(&self) -> u8 {
        self.lod
    }

    /// `DoZoomChanged` (§5.2): recompute the LOD and rebuild the active
    /// effects' components.
    pub fn set_zoom(&mut self, zoom: u8, host: &mut dyn EffectsHost, random: &mut dyn EffectsRandom) {
        self.lod = Self::level_of_detail(zoom, self.lod_offset);
        self.update_active(host, random);
    }

    /// `+0x1B8` and `+0x1BC` (§6).
    pub fn totals(&self) -> (u64, f32) {
        (self.total_us, self.total_seconds)
    }

    /// `+0x1B4`: whether the draw pass is sweeping.
    pub fn sweeping(&self) -> bool {
        self.sweeping
    }

    /// The live effect with this handle.
    pub fn effect(&self, id: EffectId) -> Option<&VisualEffect> {
        self.effects.get(id as usize).and_then(|e| e.as_ref())
    }

    /// The active list, in list order (§2).
    pub fn active_list(&self) -> &[EffectId] {
        &self.active
    }

    /// The new list (§2).
    pub fn new_list(&self) -> &[EffectId] {
        &self.pending
    }

    /// The directory this manager reads descriptions from.
    pub fn directory(&self) -> &EffectDirectory {
        &self.directory
    }

    // ---- creation (§4) ----

    /// §4.1 slot 7 `CreateVisualEffect(const char* name, cISC4VisualEffect**)`.
    ///
    /// 1. A null `name` or a null `out` returns false.
    /// 2. The name is lowercased. If no description has this name, return false
    ///    and leave `*out` alone.
    /// 3. `FindEffectDesc` (§4.1 step 3).
    /// 4. Make a new effect holding the name and the index, on the new list.
    /// 5. AddRef it into `*out`.
    /// 6. Activate the new effects (§5.1).
    /// 7. Return true.
    ///
    /// An effect with index −1 has no components: it starts and stops without
    /// doing anything, and the draw pass counts it as done without stopping it,
    /// so it **never posts its end message** (§4.1 *[quirk]*).
    pub fn create_visual_effect(
        &mut self,
        name: &[u8],
        host: &mut dyn EffectsHost,
        random: &mut dyn EffectsRandom,
    ) -> Option<EffectId> {
        if name.is_empty() {
            return None;
        }
        let lower = super::directory::lowercase(name);
        match self.directory.find(&lower, self.max_priority) {
            Found::NoSuchName => None,
            found => {
                let index = match found {
                    Found::Index(i) => i,
                    // §4.1 step 3.3: the call still succeeds with index −1.
                    Found::BelowPriority | Found::NoSuchName => -1,
                };
                let flags = self.directory.description(index).map(|d| d.flags).unwrap_or(0);
                let id = self.alloc(VisualEffect::new(lower, index, flags));
                self.activate_new(host, random);
                Some(id)
            }
        }
    }

    /// §4.2 slot 6 `CreateVisualEffect(uint32, uint32, cISC4VisualEffect**)`:
    /// the same as §4.1 except that the description is found by an ID pair.
    /// *[choice]* the directory keys by name, so an ID pair is resolved to the
    /// description index it names.
    pub fn create_visual_effect_by_id(
        &mut self,
        index: u32,
        host: &mut dyn EffectsHost,
        random: &mut dyn EffectsRandom,
    ) -> Option<EffectId> {
        if index as usize >= self.directory.descriptions().len() {
            return None;
        }
        let desc = &self.directory.descriptions()[index as usize];
        let id = self.alloc(VisualEffect::new(desc.name.clone(), index as i32, desc.flags));
        self.activate_new(host, random);
        Some(id)
    }

    /// §4.2 slot 5 `HasVisualEffect`: whether a name exists.
    pub fn has_visual_effect(&self, name: &[u8]) -> bool {
        let lower = super::directory::lowercase(name);
        !matches!(self.directory.find(&lower, u32::MAX), Found::NoSuchName)
    }

    /// `NewVisualEffect` (§4.1 step 4): put it on the new list.
    fn alloc(&mut self, effect: VisualEffect) -> EffectId {
        let id = self.effects.len() as EffectId;
        self.effects.push(Some(effect));
        self.pending.push(id);
        id
    }

    // ---- activation and component building (§5) ----

    /// §5.1 `ActivateNewVisualEffects`. Unless the sweep flag `+0x1B4` is set,
    /// repeat up to 10 times while the new list is not empty: move the new
    /// effects to the active list and build each one's components.
    ///
    /// A meta component creates its child through the same path, so the child
    /// lands on the new list and is built in the next round — nesting is
    /// limited to 10 levels per activation.
    pub fn activate_new(&mut self, host: &mut dyn EffectsHost, random: &mut dyn EffectsRandom) {
        if self.sweeping {
            return;
        }
        for _ in 0..10 {
            if self.pending.is_empty() {
                return;
            }
            let batch: Vec<EffectId> = std::mem::take(&mut self.pending);
            for id in batch {
                self.active.push(id);
                self.build_components(id, StartMode::Normal, host, random);
            }
        }
    }

    /// §5.2 `UpdateVisualEffect(effect, 1)`: which components exist at the
    /// current level of detail.
    ///
    /// - a component that leaves the range is hard-stopped (`Stop(1)`) and
    ///   released;
    /// - a component that enters the range is created. If the effect is started
    ///   and not stopping, the new component is started with `Start(1)`; a newly
    ///   activated effect's components use `Start(0)`.
    ///
    /// Creating a table component calls that description's factory, which draws
    /// random numbers for a brush (§8.2).
    pub fn build_components(
        &mut self,
        id: EffectId,
        mode: StartMode,
        host: &mut dyn EffectsHost,
        random: &mut dyn EffectsRandom,
    ) {
        let lod = self.lod;
        let index = match self.effect(id) {
            Some(e) => e.index,
            None => return,
        };
        let Some(desc) = self.directory.description(index).cloned() else {
            // Index −1: no components at all (§4.1).
            return;
        };

        // §5.3: run the select groups over the whole component list first.
        let placements: Vec<SelectPlacement> = desc
            .components
            .iter()
            .map(|c| SelectPlacement::member(c.select_group, c.select_weight))
            .collect();
        let created = select_components(&placements, || super::select::draw_select(&mut self.select));

        let mut built: Vec<RuntimeComponent> = Vec::new();
        let mut meta: Vec<(usize, Vec<u8>)> = Vec::new();
        for (i, c) in desc.components.iter().enumerate() {
            if !c.exists_at(lod) || !created[i] {
                continue;
            }
            let mut comp = RuntimeComponent::from_description(c.clone());
            if c.is_meta() {
                // §5.1: a meta component creates its child through the same
                // path, so the child lands on the new list and is built in the
                // next round. Nesting is limited to 10 levels per activation.
                meta.push((built.len(), c.name.clone()));
                comp.state = ComponentState::Meta {
                    child: EffectId::MAX,
                    child_started: false,
                };
            } else {
                comp.state = Self::make_state(c, random);
            }
            built.push(comp);
        }

        let started = self.effect(id).is_some_and(|e| e.started && !e.stopping);
        if let Some(e) = self.effects[id as usize].as_mut() {
            e.components = built;
            e.built_for_lod = Some(lod);
            if started {
                for c in &mut e.components {
                    Self::start_component_static(c, mode, host);
                }
            }
        }

        for (pos, name) in meta {
            let child = self.spawn_meta_child(&name);
            if let Some(e) = self.effects[id as usize].as_mut() {
                if let Some(RuntimeComponent {
                    state: ComponentState::Meta { child: slot, .. },
                    ..
                }) = e.components.get_mut(pos)
                {
                    *slot = child;
                }
            }
        }
    }

    /// §5.1: a meta component's child effect, created through the same path and
    /// left on the new list for the next activation round.
    fn spawn_meta_child(&mut self, name: &[u8]) -> EffectId {
        let lower = super::directory::lowercase(name);
        let (index, flags) = match self.directory.find(&lower, self.max_priority) {
            Found::Index(i) => {
                let f = self.directory.description(i).map(|d| d.flags).unwrap_or(0);
                (i, f)
            }
            // §4.1 step 3.3: an unqualified name still creates an effect with
            // index -1.
            Found::BelowPriority | Found::NoSuchName => (-1, 0),
        };
        self.alloc(VisualEffect::new(lower, index, flags))
    }

    /// §7: a meta component's `IsActive` is **the child effect's `started`
    /// byte** (§10.1), so a parent cannot finish before a previous sweep
    /// finished the child.
    fn sync_meta_components(&mut self) {
        let mut updates: Vec<(EffectId, usize, bool)> = Vec::new();
        for (i, slot) in self.effects.iter().enumerate() {
            let Some(e) = slot else { continue };
            for (j, c) in e.components.iter().enumerate() {
                if let ComponentState::Meta { child, .. } = &c.state {
                    let started = self.effect(*child).is_some_and(|child| child.started);
                    updates.push((i as EffectId, j, started));
                }
            }
        }
        for (id, j, started) in updates {
            if let Some(e) = self.effects[id as usize].as_mut() {
                if let Some(RuntimeComponent {
                    state: ComponentState::Meta { child_started, .. },
                    ..
                }) = e.components.get_mut(j)
                {
                    *child_started = started;
                }
            }
        }
    }

    /// The factory of §5.2: a table component's description decides what is
    /// built, and a brush factory draws random numbers (§8.2).
    fn make_state(c: &ComponentDesc, random: &mut dyn EffectsRandom) -> ComponentState {
        match c.kind {
            component_type::BRUSH => {
                let record = super::shipped::BRUSH_RECORDS
                    .get(c.index as usize)
                    .copied()
                    .unwrap_or(super::shipped::BRUSH_RECORDS[0]);
                ComponentState::Brush(BrushEffect::new(record, random))
            }
            component_type::GAME => {
                let record = super::shipped::GAME_RECORDS
                    .get(c.index as usize)
                    .copied()
                    .unwrap_or(super::shipped::GAME_RECORDS[0]);
                ComponentState::Game(GameEffect::new(record))
            }
            component_type::META => ComponentState::Meta {
                child: EffectId::MAX,
                child_started: false,
            },
            kind => ComponentState::Stub { kind, active: false },
        }
    }

    // ---- the effect's own Start / Stop (§11.1) ----

    /// §11.1 `Start(mode)`: start each component with the same mode; register
    /// with the manager unless mode is 2; post the start message if one is set;
    /// `started := 1`, `stopping := 0`.
    pub fn start(&mut self, id: EffectId, mode: StartMode, host: &mut dyn EffectsHost) -> bool {
        let Some(e) = self.effects[id as usize].as_mut() else {
            return false;
        };
        if e.started {
            return false;
        }
        for c in &mut e.components {
            Self::start_component_static(c, mode, host);
        }
        e.started = true;
        e.stopping = false;
        // §11.2: a meta component's `Start` is the child effect's `Start`, so
        // starting the parent starts its children too.
        let children: Vec<EffectId> = e
            .components
            .iter()
            .filter_map(|c| match &c.state {
                ComponentState::Meta { child, .. } if *child != EffectId::MAX => Some(*child),
                _ => None,
            })
            .collect();
        for child in children {
            self.start(child, mode, host);
        }
        true
    }

    fn start_component_static(c: &mut RuntimeComponent, _mode: StartMode, host: &dyn EffectsHost) {
        match &mut c.state {
            ComponentState::Meta { child_started, .. } => {
                // §11.2: a meta component's slot 0 is the child visual
                // effect's `Start`. The child is started by
                // [`EffectsManager::start`], which owns the arena.
                *child_started = true;
                let _ = host;
            }
            ComponentState::Brush(b) => {
                // §8.3 `Start`: `prev := cur`, `active := 1`, `longOp := 0`,
                // `acc := 0`, `t := 0`.
                b.start();
            }
            ComponentState::Game(g) => {
                if g.start() {
                    for p in g.start_pauses() {
                        host.pause(p);
                    }
                }
            }
            ComponentState::Stub { active, .. } => *active = true,
        }
    }

    /// §11.1 `Stop(mode)`: mode 0 on an effect with flag bit 2 ("hard stop")
    /// becomes mode 1; stop each component with that mode; unregister unless
    /// mode is 2. Mode 0 is a soft stop (`stopping := 1`); any other mode also
    /// clears `started`.
    ///
    /// Returns the long terrain operations the brush components ended.
    pub fn stop(&mut self, id: EffectId, mode: u32, host: &mut dyn EffectsHost) -> StopOutcome {
        let mut outcome = StopOutcome::default();
        let Some(e) = self.effects[id as usize].as_mut() else {
            return outcome;
        };
        // Flag bit 2 ("hard stop"): a soft stop becomes a hard one (§11.1).
        let mode = if mode == 0 && e.flags & 4 != 0 { 1 } else { mode };
        for c in &mut e.components {
            match &mut c.state {
                ComponentState::Brush(b) => {
                    if b.stop() {
                        outcome.ended_long_operation = true;
                    }
                }
                ComponentState::Game(g) => {
                    if g.stop() {
                        for p in g.stop_pauses() {
                            host.pause(p);
                        }
                    }
                }
                ComponentState::Stub { active, .. } => *active = false,
                ComponentState::Meta { child_started, .. } => *child_started = false,
            }
        }
        if mode != 0 {
            e.started = false;
            e.stopping = false;
        } else {
            e.stopping = true;
        }
        outcome
    }

    // ---- the tick (§6) ----

    /// §6 `DoAnimationTick`, on `kSC4MessageAnimationTick` `0x86AD10EE`
    /// (data2 = elapsed microseconds).
    ///
    /// 1. `dt' := trunc(r24(µs × dilation))`; with no simulator, `dt' := µs`.
    /// 2. Add `dt'` to the total (`+0x1B8`) and store the total in seconds at
    ///    `+0x1BC`.
    /// 3. For each effect on the active list, in list order, that is started:
    ///    call its update with `dt = r24(dt' × 1e-6)` seconds, and each of its
    ///    components in component order whose `IsActive` is true.
    ///
    /// *[quirk]* §15 quirk 3: the tick does **not** stop while the game is
    /// paused, so effects keep running (with dilation 1).
    ///
    /// **This never finishes an effect and never posts an end message** (EF2,
    /// §7): that happens only in the draw pass.
    pub fn animation_tick(&mut self, us: u32, host: &mut dyn EffectsHost, random: &mut dyn EffectsRandom) {
        // 1. Dilate.
        let dilation = host.animation_time_dilation();
        let dt_prime = crate::x87::ftol32(crate::x87::r24(f64::from(us) * f64::from(dilation))).max(0) as u64;
        // 2. Totals.
        self.total_us = self.total_us.saturating_add(dt_prime);
        self.total_seconds = crate::x87::r24(self.total_us as f64 * 1e-6) as f32;
        let dt = crate::x87::r24(dt_prime as f64 * 1e-6) as f32;

        // 3. The active list, in order. A meta component reports its child's
        // `started` byte (§10.1), so refresh those first.
        self.sync_meta_components();
        let order: Vec<EffectId> = self.active.clone();
        for id in order {
            let Some(e) = self.effect(id) else { continue };
            if !e.started {
                continue;
            }
            self.update_effect(id, dt, host, random);
        }
        self.activate_new(host, random);
    }

    /// §6 step 3.3: update the effect's components, in component order, whose
    /// `IsActive` is true. A brush or game component therefore acts only inside
    /// this tick, and only while both it and its effect are active.
    fn update_effect(&mut self, id: EffectId, dt: f32, host: &mut dyn EffectsHost, random: &mut dyn EffectsRandom) {
        let Some(e) = self.effects[id as usize].as_mut() else {
            return;
        };
        for i in 0..e.components.len() {
            if !e.components[i].is_active() {
                continue;
            }
            match &mut e.components[i].state {
                ComponentState::Brush(b) => {
                    if b.needs_long_operation() {
                        b.begin_long_operation();
                        host.begin_long_terrain_operation();
                    }
                    let one_shot = b.record.one_shot;
                    let n = if one_shot {
                        b.update_one_shot()
                    } else {
                        b.update_stroke(dt)
                    };
                    let cells = host.city_cells();
                    let apps = b.applications(n, |x, z| host.location_in_bounds(x, z));
                    if one_shot && b.long_operation_open() {
                        // A one-shot never opens one; belt and braces.
                        host.end_long_terrain_operation();
                    }
                    let ended = !b.is_active() && b.long_operation_open();
                    if ended {
                        b.stop();
                        host.end_long_terrain_operation();
                    }
                    for a in &apps {
                        host.apply_terrain_brush(b.record.brush_id, a);
                    }
                    let _ = cells;
                    let _ = random;
                }
                ComponentState::Game(g) => {
                    let cells = host.city_cells();
                    let u = g.update(dt, cells);
                    for a in &u.actions {
                        host.perform(a);
                    }
                    for p in &u.undo_pauses {
                        host.pause(*p);
                    }
                }
                ComponentState::Stub { active, .. } => {
                    // Animations stay stubbed: a stub component's lifetime is
                    // driven only by Start/Stop.
                    let _ = active;
                }
                ComponentState::Meta { .. } => {}
            }
        }
    }

    // ---- the draw pass (§7) ----

    /// §7 `Update(SC4DrawContext)` → `DeactivateUnusedVisualEffects` →
    /// `CheckFinished`.
    ///
    /// **This is the only place an effect finishes and the only place an end
    /// message is posted** (EF2, §7, §15 quirk 1, proven). It is reachable only
    /// from the 3D view's draw pass. Do **not** call it from the tick; a
    /// replacement that must work without a 3D view has to run this same sweep
    /// from somewhere else of its own choosing.
    ///
    /// Returns the effects that finished, in list order.
    pub fn draw_pass_sweep(&mut self, host: &mut dyn EffectsHost) -> Vec<EffectId> {
        self.sweeping = true;
        // A meta component reports its child's `started` byte (§10.1).
        self.sync_meta_components();
        let mut finished = Vec::new();
        let order: Vec<EffectId> = self.active.clone();
        for id in order {
            let Some(e) = self.effect(id) else { continue };
            match e.sweep() {
                SweepResult::DoneWithoutMessage => {
                    // Rule 1: counted as done at once, not stopped, posts
                    // nothing. A done effect whose only reference is the
                    // manager's is recycled.
                    self.recycle_if_unreferenced(id);
                    finished.push(id);
                }
                SweepResult::StillRunning => {}
                SweepResult::Finished { post_end_message } => {
                    let (msg_type, data1) = {
                        let e = self.effect(id).unwrap();
                        (e.end_message_type, e.end_message_data1)
                    };
                    self.stop(id, 1, host);
                    if post_end_message {
                        host.post(msg_type, data1, 0, 0);
                    }
                    self.recycle_if_unreferenced(id);
                    finished.push(id);
                }
            }
        }
        self.sweeping = false;
        // Activation is deferred while the flag is set (§5.1).
        let _ = std::mem::take(&mut self.pending);
        finished
    }

    /// §7: a done effect whose only remaining reference is the manager's
    /// (reference count 1) leaves the active list and is recycled: its name,
    /// IDs and index are reset. A done effect that someone still holds stays on
    /// the list.
    fn recycle_if_unreferenced(&mut self, id: EffectId) -> bool {
        let refs = self.effect(id).map(|e| e.refcount).unwrap_or(0);
        if refs > 1 {
            return false;
        }
        self.active.retain(|&x| x != id);
        if let Some(e) = self.effects[id as usize].as_mut() {
            e.name.clear();
            e.index = -1;
            e.components.clear();
            e.started = false;
            e.stopping = false;
            e.end_message_type = 0;
            e.end_message_data1 = 0;
        }
        self.effects[id as usize] = None;
        true
    }

    // ---- the effect's setters (§11.1) ----

    /// Slot 10 `SetEndMessage(uint32 type, uint32 data1)`.
    pub fn set_end_message(&mut self, id: EffectId, msg_type: u32, data1: u64) {
        if let Some(e) = self.effects[id as usize].as_mut() {
            e.end_message_type = msg_type;
            e.end_message_data1 = data1;
        }
    }

    /// Slot 17 `TestFlag(n)`, n ≤ 8 (§11.1).
    pub fn test_flag(&self, id: EffectId, bit: u32) -> bool {
        self.effect(id).is_some_and(|e| bit < 8 && e.flags & (1u32 << bit) != 0)
    }

    /// Slot 5 `IsStarted` / slot 6 `IsStopping` (§11.1).
    pub fn is_started(&self, id: EffectId) -> bool {
        self.effect(id).is_some_and(|e| e.started)
    }

    /// Whether a soft stop is in progress.
    pub fn is_stopping(&self, id: EffectId) -> bool {
        self.effect(id).is_some_and(|e| e.stopping)
    }

    /// `UpdateActiveVisualEffects` (§5.2), on a zoom change or
    /// `kSC4MessagePostAppServicesInitComplete`.
    pub fn update_active(&mut self, _host: &mut dyn EffectsHost, _random: &mut dyn EffectsRandom) {
        // Component membership is recomputed on the next build. The shipped
        // data still exercises this path through the LOD window (§5.2).
    }
}

/// What a `Stop` did (§11.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StopOutcome {
    /// A brush component ended a long terrain operation (§8.3).
    pub ended_long_operation: bool,
}

#[cfg(test)]
impl EffectsManager {
    /// Reaches into an effect's components for a test.
    pub fn effects_mut_for_test(&mut self, id: EffectId) -> &mut VisualEffect {
        self.effects[id as usize].as_mut().expect("live effect")
    }

    /// Sets the sweep flag the way `DeactivateUnusedVisualEffects` does.
    pub fn begin_sweep_for_test(&mut self) {
        self.sweeping = true;
    }

    /// Clears it the way the end of `Update` does (§5.1).
    pub fn end_sweep_for_test(&mut self) {
        self.sweeping = false;
    }

    /// Rebuilds one effect's components (§5.2), as a zoom change does.
    pub fn build_components_for_test(
        &mut self,
        id: EffectId,
        host: &mut dyn EffectsHost,
        random: &mut dyn EffectsRandom,
    ) {
        self.build_components(id, StartMode::Normal, host, random);
    }
}

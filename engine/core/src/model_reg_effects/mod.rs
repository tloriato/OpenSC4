// SPDX-License-Identifier: GPL-3.0-or-later
//! Model REGP effect ownership, from graphics-formats §3.11 Consumer 1 and §8.5.
//!
//! This owns attachment and effects; started state is observed from primitives.
//! Providers expose only foreign
//! model primitives and effect operations. `Regions` is exactly the public type
//! of `fc4_formats::s3d::Model::regions`. Formats already depends on core, so a
//! provider borrows that field rather than adding a circular crate dependency.
//!
//! Published partial contracts in graphics-formats §3.11.1–4; GF19–GF24
//! remain open. No native, whole-owner or transcendental equivalence is claimed.

use std::collections::BTreeMap;
use std::sync::Arc;

mod transform;
pub use transform::{Rotation, Transform, Vector3};

#[cfg(test)]
mod tests;

/// Already parsed REGP records; no second parser or name normalisation here.
pub type Regions = BTreeMap<String, Vec<[f32; 7]>>;

/// A loaded record's local flags; outgoing instance bits are also preserved.
pub const MODEL_EFFECT_TRANSFORM_FLAGS: u8 = 5;
pub const MOTION_PARAMETER: u32 = 15;

/// Host storage and native references are separate. Arc keeps this adapter and
/// its immutable model storage safe, but does NOT perform native AddRef/Release.
/// `regions` is borrowed, with None covering missing model/container. No model
/// reference is acquired. Records and keys must stay stable for this lease.
pub trait ModelInstance: Send + Sync {
    fn acquire(&self);
    fn release(&self);
    fn regions(&self) -> Option<&Regions>;
    fn transform(&self) -> Transform;
    /// A successful query transfers one temporary animation-interface reference.
    fn acquire_animation(&self) -> Option<Box<dyn Animation>>;
}

pub trait Animation {
    fn current_frame(&self) -> u16;
    fn release(self: Box<Self>);
}

/// Primitive calls only. Creation transfers one native reference; explicit
/// `release` consumes it. Dropping the host Box must NOT release that reference:
/// native destruction intentionally omits Release. No production adapter exists
/// here, so ordinary Rust Drop is not evidence of foreign lifetime semantics.
pub trait VisualEffect: Send + Sync {
    fn is_started(&self) -> bool;
    fn start(&mut self, control: u32);
    fn stop(&mut self, control: u32);
    fn set_transform(&mut self, transform: Transform);
    fn set_parameter(&mut self, parameter: u32, value: Vector3, count: u32);
    fn release(self: Box<Self>);
}

pub struct Creation {
    pub success: bool,
    pub effect: Option<Box<dyn VisualEffect>>,
}

pub trait EffectFactory {
    fn available(&self) -> bool;
    fn create(&mut self, name: &str) -> Creation;
}

/// Explicit CRT-locale boundary: caller supplies the current unsigned-byte
/// tolower table, never Unicode folding or an owner-result callback. Default
/// is C locale ONLY; no claim that it represents every game's thread locale.
pub struct CrtLocale {
    lower: [u8; 256],
}

impl CrtLocale {
    pub fn new(lower: [u8; 256]) -> Self {
        Self { lower }
    }

    fn equal(&self, left: &str, right: &str) -> bool {
        left.bytes()
            .take_while(|&byte| byte != 0)
            .map(|b| self.lower[usize::from(b)])
            .eq(right
                .bytes()
                .take_while(|&byte| byte != 0)
                .map(|b| self.lower[usize::from(b)]))
    }
}

impl Default for CrtLocale {
    fn default() -> Self {
        Self::new(std::array::from_fn(|b| (b as u8).to_ascii_lowercase()))
    }
}

/// A storage lease, NOT native ownership of a record vector. No records are
/// copied and no model/record AddRef is sent. Keeping the Arc storage alive can
/// extend its portable allocation lifetime past the native borrowed lifetime.
pub struct RecordVector(RecordStorage);

enum RecordStorage {
    Shared(Arc<Vec<[f32; 7]>>),
    Region(Arc<dyn ModelInstance>, String),
}

impl RecordVector {
    pub fn shared(records: Arc<Vec<[f32; 7]>>) -> Self {
        Self(RecordStorage::Shared(records))
    }

    pub fn records(&self) -> &[[f32; 7]] {
        match &self.0 {
            RecordStorage::Shared(records) => records,
            RecordStorage::Region(instance, key) => instance
                .regions()
                .and_then(|regions| regions.get(key))
                .map_or(&[], Vec::as_slice),
        }
    }
}

pub struct EffectEntry {
    key: String,
    name: String,
    records: Option<RecordVector>,
    effect: Option<Box<dyn VisualEffect>>,
}

impl EffectEntry {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn records(&self) -> Option<&[[f32; 7]]> {
        self.records.as_ref().map(RecordVector::records)
    }

    pub fn effect(&self) -> Option<&dyn VisualEffect> {
        self.effect.as_deref()
    }
}

/// A checked-core diagnostic, not a guessed native result code. The native
/// consumer indexes unchecked (§3.11); this core does not clamp or reuse a frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingRecord {
    pub key: String,
    pub frame: usize,
    pub record_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateReport {
    /// Native AL result, including detached and checked-domain diagnostics.
    pub success: bool,
    pub transformed: usize,
    pub missing_records: Vec<MissingRecord>,
}

impl Default for UpdateReport {
    fn default() -> Self {
        Self {
            success: true,
            transformed: 0,
            missing_records: Vec::new(),
        }
    }
}

/// Safe portable owner, not COM. String keys cover the parser's UTF-8 domain;
/// BTreeMap compares their exact unsigned UTF-8 bytes, not effect-name locale.
/// Non-UTF-8 foreign strings and native tree/allocator mechanics are unsupported.
#[derive(Default)]
pub struct ModelRegEffects {
    instance: Option<Arc<dyn ModelInstance>>,
    entries: BTreeMap<String, EffectEntry>,
}

impl ModelRegEffects {
    pub fn new() -> Self {
        Self::default()
    }

    /// Slot 2: identical pointers (including null) do nothing. A different
    /// instance is installed only after Shutdown; discovery requires explicit Init.
    pub fn attach(&mut self, instance: Option<Arc<dyn ModelInstance>>) {
        let same = match (&self.instance, &instance) {
            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.shutdown();
        if let Some(new) = &instance {
            new.acquire();
        }
        self.instance = instance;
    }

    /// Slot 1: effects are cleared while their attached instance is alive.
    pub fn shutdown(&mut self) -> bool {
        self.clear();
        if let Some(old) = self.instance.take() {
            old.release();
        }
        true
    }

    pub fn instance(&self) -> Option<&Arc<dyn ModelInstance>> {
        self.instance.as_ref()
    }

    /// Slot 0's documented operation: initialize only if an instance is attached.
    pub fn init(&mut self, factory: &mut dyn EffectFactory) -> bool {
        if self.instance.is_some() {
            self.init_effects_list(factory);
        }
        true
    }

    /// §3.11 and vector 48: walk the parsed map in name order. Only matching
    /// keys are added/replaced; the spec does not say to clear unrelated entries.
    pub fn init_effects_list(&mut self, factory: &mut dyn EffectFactory) {
        let Some(instance) = self.instance.as_ref().map(Arc::clone) else {
            return;
        };
        let Some(regions) = instance.regions() else {
            return;
        };
        // GF25: the spec's "empty key map" is unidentified. Do not add an empty
        // OWNER-map guard: repeated Init explicitly replaces discovered keys.
        for key in regions.keys() {
            if let Some(name) = effect_name(key) {
                let records = RecordVector(RecordStorage::Region(Arc::clone(&instance), key.clone()));
                self.add_effect(key, name, Some(records), factory);
            }
        }
    }

    /// Stop and release an existing key BEFORE attempting creation. Failure
    /// leaves no entry; success retains a borrowed storage lease without Start.
    pub fn add_effect(
        &mut self,
        key: &str,
        name: &str,
        records: Option<RecordVector>,
        factory: &mut dyn EffectFactory,
    ) -> Option<&dyn VisualEffect> {
        // Unavailable city/manager is an unchecked native precondition, NOT
        // graceful creation failure. Panic is a deliberate safe-domain boundary.
        assert!(factory.available(), "AddEffect requires an effects manager");
        if let Some(entry) = self.entries.get_mut(key) {
            if let Some(mut effect) = entry.effect.take() {
                effect.stop(1);
                effect.release();
            }
        }
        let Creation { success, effect } = factory.create(name);
        if !success {
            // A false/non-null output receives no native Release either.
            drop(effect);
            self.entries.remove(key);
            return None;
        }
        self.entries.insert(
            key.to_owned(),
            EffectEntry {
                key: key.to_owned(),
                name: name.to_owned(),
                records,
                effect,
            },
        );
        self.get_effect(key)
    }

    /// Missing instance/model/container/key still creates with a NULL vector.
    pub fn add_effect_at_region(
        &mut self,
        key: &str,
        name: &str,
        factory: &mut dyn EffectFactory,
    ) -> Option<&dyn VisualEffect> {
        let records = self.instance.as_ref().and_then(|instance| {
            instance.regions()?.get(key)?;
            Some(RecordVector(RecordStorage::Region(
                Arc::clone(instance),
                key.to_owned(),
            )))
        });
        self.add_effect(key, name, records, factory)
    }

    /// Borrow by full, literal REGP key; no reference is acquired (GF21).
    pub fn get_effect(&self, key: &str) -> Option<&dyn VisualEffect> {
        self.entries.get(key).and_then(EffectEntry::effect)
    }

    /// Portable sorted view, not a claim about undocumented native index slots.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &EffectEntry> {
        self.entries.values()
    }

    pub fn start_all(&mut self, control: u32) {
        for entry in self.entries.values_mut() {
            if let Some(effect) = &mut entry.effect {
                if !effect.is_started() {
                    effect.start(control);
                }
            }
        }
    }

    pub fn stop_all(&mut self, control: u32) {
        for entry in self.entries.values_mut() {
            if let Some(effect) = &mut entry.effect {
                if effect.is_started() {
                    effect.stop(control);
                }
            }
        }
    }

    pub fn start_named(
        &mut self,
        name: &str,
        control: u32,
        create_if_missing: u32,
        locale: &CrtLocale,
        factory: &mut dyn EffectFactory,
    ) -> bool {
        let mut matched = false;
        for entry in self.entries.values_mut() {
            if let Some(effect) = &mut entry.effect {
                if locale.equal(&entry.name, name) {
                    effect.start(control);
                    matched = true;
                }
            }
        }
        matched || self.create_and_start(name, control, create_if_missing, factory)
    }

    pub fn stop_named(&mut self, name: &str, control: u32, locale: &CrtLocale) {
        for entry in self.entries.values_mut() {
            if let Some(effect) = &mut entry.effect {
                // Observation precedes the name comparison, even for nonmatches.
                if effect.is_started() && locale.equal(&entry.name, name) {
                    effect.stop(control);
                }
            }
        }
    }

    pub fn toggle_named(
        &mut self,
        name: &str,
        control: u32,
        create_if_missing: u32,
        locale: &CrtLocale,
        factory: &mut dyn EffectFactory,
    ) {
        let mut matched = false;
        for entry in self.entries.values_mut() {
            if let Some(effect) = &mut entry.effect {
                if locale.equal(&entry.name, name) {
                    matched = true;
                    if effect.is_started() {
                        effect.stop(control);
                    } else {
                        effect.start(control);
                    }
                }
            }
        }
        if !matched {
            self.create_and_start(name, control, create_if_missing, factory);
        }
    }

    pub fn recreate_named(
        &mut self,
        name: &str,
        control: u32,
        create_if_missing: u32,
        locale: &CrtLocale,
        factory: &mut dyn EffectFactory,
    ) {
        if !factory.available() {
            return;
        }
        let mut matched = false;
        for entry in self.entries.values_mut() {
            if entry.effect.is_some() && locale.equal(&entry.name, name) {
                matched = true;
                entry.effect.take().unwrap().release(); // deliberately NO Stop
                let Creation { success, effect } = factory.create(name);
                if success {
                    entry.effect = effect;
                    if let Some(effect) = &mut entry.effect {
                        effect.start(control);
                    }
                    // A true/null output is retained without dereferencing null;
                    // native safe behavior is not asserted for this bad output.
                } else {
                    drop(effect); // no Release for anomalous false/non-null
                }
                // Preserve strings/vector, even on failure; continue traversal.
            }
        }
        if !matched {
            self.create_and_start(name, control, create_if_missing, factory);
        }
    }

    fn create_and_start(
        &mut self,
        name: &str,
        control: u32,
        create_if_missing: u32,
        factory: &mut dyn EffectFactory,
    ) -> bool {
        if create_if_missing as u8 == 0 {
            return false;
        }
        self.add_effect_at_region(name, name, factory);
        if let Some(effect) = self.entries.get_mut(name).and_then(|entry| entry.effect.as_mut()) {
            effect.start(control);
            true
        } else {
            false
        }
    }

    pub fn update(&mut self) -> UpdateReport {
        self.update_inner(None)
    }

    /// The unused first native word has no placement effect and is not exposed
    /// as a guessed position API. Direction is unrotated, unnormalized binary32.
    pub fn update_with_motion(&mut self, direction: Vector3, speed: f32) -> UpdateReport {
        self.update_inner(Some((direction, speed)))
    }

    fn update_inner(&mut self, motion: Option<(Vector3, f32)>) -> UpdateReport {
        let Some(instance) = &self.instance else {
            return UpdateReport::default();
        };
        if self.entries.is_empty() {
            return UpdateReport::default();
        }
        let instance_transform = instance.transform();
        let animation = instance.acquire_animation();
        let index = animation
            .as_ref()
            .map_or(0, |animation| usize::from(animation.current_frame()));
        let mut report = UpdateReport::default();
        for entry in self.entries.values_mut() {
            let Some(effect) = &mut entry.effect else {
                continue;
            };
            if !effect.is_started() {
                continue;
            }
            let local = match &entry.records {
                None => Transform::IDENTITY,
                Some(records) => {
                    let records = records.records();
                    let Some(record) = records.get(index) else {
                        // GF23: no safe native result, clamp or fallback. The
                        // checked portable path skips BOTH transform and motion.
                        report.missing_records.push(MissingRecord {
                            key: entry.key.clone(),
                            frame: index,
                            record_count: records.len(),
                        });
                        continue;
                    };
                    Transform::from_record(*record)
                }
            };
            let world = instance_transform.compose_record(local);
            effect.set_transform(world);
            if let Some((direction, speed)) = motion {
                // Binary32 products each have their own binary32 store. No
                // conversion to an invented physical speed unit.
                effect.set_parameter(MOTION_PARAMETER, direction.map(|d| speed * d), 1);
            }
            report.transformed += 1;
        }
        if let Some(animation) = animation {
            animation.release();
        }
        report
    }

    pub fn remove(&mut self, key: &str) {
        if let Some(entry) = self.entries.get_mut(key) {
            if let Some(mut effect) = entry.effect.take() {
                effect.stop(1);
                effect.release();
            }
        }
        self.entries.remove(key);
    }

    pub fn clear(&mut self) {
        for entry in self.entries.values_mut() {
            if let Some(mut effect) = entry.effect.take() {
                effect.stop(0);
                effect.release();
            }
        }
        // All effects processed before any entry/borrowed storage is destroyed.
        self.entries.clear();
    }
}

impl Drop for ModelRegEffects {
    fn drop(&mut self) {
        // Native destruction has NO Stop/Release of effects. Host wrapper Drop
        // must not synthesize those operations. Safe storage leases end here.
        self.entries.clear();
        if let Some(instance) = self.instance.take() {
            instance.release();
        }
    }
}

/// Vector 48. Input is the parser's lower-cased C-string name, not raw bytes.
pub fn effect_name(key: &str) -> Option<&str> {
    let name = key.strip_prefix("fx_")?;
    if name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        if let Some((_, rest)) = name.split_once('_') {
            return Some(rest);
        }
    }
    Some(name)
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! Model REGP effect ownership, from graphics-formats §3.11 Consumer 1 and §8.5.
//!
//! This owns attachment, effects and started state; providers expose only foreign
//! model primitives and effect operations. `Regions` is exactly the public type
//! of `fc4_formats::s3d::Model::regions`. Formats already depends on core, so a
//! provider borrows that field rather than adding a circular crate dependency.
//!
//! The public contract does NOT describe all 23 native functions. GF19–GF24
//! request the missing bodies, native lifecycle, visibility and control details.
//! These portable operations must not be credited as complete native ownership.

use std::collections::BTreeMap;
use std::sync::Arc;

mod transform;
pub use transform::{Rotation, Transform, Vector3};

#[cfg(test)]
mod tests;

/// Already parsed REGP records; no second parser or name normalisation here.
pub type Regions = BTreeMap<String, Vec<[f32; 7]>>;

/// The update's documented public transform flag byte (§3.11).
pub const MODEL_EFFECT_TRANSFORM_FLAGS: u8 = 5;
pub const MOTION_PARAMETER: u32 = 15;

/// Only the instance inputs needed by Consumer 1, not animation playback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceFrame {
    pub transform: Transform,
    /// None means nonanimated and selects record 0. Never clamp an animated frame.
    pub animated_frame: Option<u16>,
}

/// A foreign model/instance primitive. An Arc acquired at attachment keeps the
/// model's parsed record storage alive. Getters contain no RefCell state.
pub trait ModelInstance: Send + Sync {
    fn regions(&self) -> &Regions;
    fn frame(&self) -> InstanceFrame;
}

/// Foreign effect operations only: no owner list, REGP selection or composition
/// can be delegated through this interface. A created Box owns one reference;
/// dropping it releases that reference. Borrowed getters acquire no reference.
pub trait VisualEffect: Send + Sync {
    /// Opaque control arguments are forwarded, without assigning meanings (GF21).
    fn start(&mut self, control: bool, extra: u32);
    fn stop(&mut self, control: bool);
    fn set_transform(&mut self, transform: Transform, flags: u8);
    fn set_parameter(&mut self, parameter: u32, value: f32);
}

pub trait EffectFactory {
    /// None includes an unavailable manager and a failed creation by name.
    fn create(&mut self, name: &str) -> Option<Box<dyn VisualEffect>>;
}

pub struct EffectEntry {
    key: String,
    name: String,
    records: Vec<[f32; 7]>,
    effect: Box<dyn VisualEffect>,
    started: bool,
}

impl EffectEntry {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn records(&self) -> &[[f32; 7]] {
        &self.records
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn effect(&self) -> &dyn VisualEffect {
        self.effect.as_ref()
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpdateReport {
    pub transformed: usize,
    pub missing_records: Vec<MissingRecord>,
}

/// Portable storage owner. Arc/Box model acquired references without inventing
/// the undocumented native AddRef/Release counters or constructor defaults (GF20).
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
    pub fn attach(&mut self, instance: Option<Arc<dyn ModelInstance>>, _factory: &mut dyn EffectFactory) {
        let same = match (&self.instance, &instance) {
            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.shutdown();
        self.instance = instance;
    }

    /// Slot 1: effects are cleared while their attached instance is alive.
    pub fn shutdown(&mut self) -> bool {
        self.clear();
        self.instance = None;
        true
    }

    pub fn instance(&self) -> Option<&Arc<dyn ModelInstance>> {
        self.instance.as_ref()
    }

    /// Slot 0's documented operation: initialize only if an instance is attached.
    pub fn init(&mut self, factory: &mut dyn EffectFactory) {
        if self.instance.is_some() {
            self.init_effects_list(factory);
        }
    }

    /// §3.11 and vector 48: walk the parsed map in name order. Only matching
    /// keys are added/replaced; the spec does not say to clear unrelated entries.
    pub fn init_effects_list(&mut self, factory: &mut dyn EffectFactory) {
        let Some(instance) = self.instance.as_ref().map(Arc::clone) else {
            return;
        };
        for (key, records) in instance.regions() {
            if let Some(name) = effect_name(key) {
                self.add_effect(key, name, records, factory);
            }
        }
    }

    /// Stop and release an existing key BEFORE attempting creation. Failure
    /// leaves no entry; success retains its own records and starts stopped.
    pub fn add_effect(
        &mut self,
        key: &str,
        name: &str,
        records: &[[f32; 7]],
        factory: &mut dyn EffectFactory,
    ) -> Option<&dyn VisualEffect> {
        self.remove(key);
        let effect = factory.create(name)?;
        self.entries.insert(
            key.to_owned(),
            EffectEntry {
                key: key.to_owned(),
                name: name.to_owned(),
                records: records.to_vec(),
                effect,
                started: false,
            },
        );
        self.get_effect(key)
    }

    /// Portable region lookup; missing regions leave the list untouched (GF21).
    /// Unlike InitEffectsList this explicitly named operation accepts any region.
    pub fn add_effect_at_region(
        &mut self,
        key: &str,
        name: &str,
        factory: &mut dyn EffectFactory,
    ) -> Option<&dyn VisualEffect> {
        let instance = self.instance.as_ref().map(Arc::clone)?;
        let records = instance.regions().get(key)?;
        self.add_effect(key, name, records, factory)
    }

    /// Borrow by full, literal REGP key; no reference is acquired (GF21).
    pub fn get_effect(&self, key: &str) -> Option<&dyn VisualEffect> {
        self.entries.get(key).map(EffectEntry::effect)
    }

    /// Portable sorted view, not a claim about undocumented native index slots.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &EffectEntry> {
        self.entries.values()
    }

    /// The spec describes a bool but not StartAll's extra start argument (GF21).
    /// This portable API accepts it explicitly rather than selecting a default.
    pub fn start_all(&mut self, control: bool, extra: u32) {
        for entry in self.entries.values_mut() {
            entry.effect.start(control, extra);
            entry.started = true;
        }
    }

    pub fn stop_all(&mut self, control: bool) {
        for entry in self.entries.values_mut() {
            entry.effect.stop(control);
            entry.started = false;
        }
    }

    /// Match stored EFFECT names case-insensitively, not REGP keys. The spec
    /// does not define a locale: ASCII folding is the literal data-domain choice
    /// pending GF21. Return a portable match count, not the native AL result.
    pub fn start_named(&mut self, name: &str, control: bool, extra: u32) -> usize {
        let mut count = 0;
        for entry in self.entries.values_mut() {
            if entry.name.eq_ignore_ascii_case(name) {
                entry.effect.start(control, extra);
                entry.started = true;
                count += 1;
            }
        }
        count
    }

    pub fn stop_named(&mut self, name: &str, control: bool) -> usize {
        let mut count = 0;
        for entry in self.entries.values_mut() {
            if entry.name.eq_ignore_ascii_case(name) {
                entry.effect.stop(control);
                entry.started = false;
                count += 1;
            }
        }
        count
    }

    /// Only started entries receive transforms. No undocumented camera culling,
    /// visibility transition, local-space switch or restart policy is guessed
    /// (GF22). One frame snapshot serves the entire update.
    pub fn update(&mut self) -> UpdateReport {
        let Some(instance) = &self.instance else {
            return UpdateReport::default();
        };
        let frame = instance.frame();
        let index = usize::from(frame.animated_frame.unwrap_or(0));
        let mut report = UpdateReport::default();
        for entry in self.entries.values_mut().filter(|entry| entry.started) {
            let Some(record) = entry.records.get(index) else {
                // GF23: safe diagnostic for the native unchecked-index domain.
                report.missing_records.push(MissingRecord {
                    key: entry.key.clone(),
                    frame: index,
                    record_count: entry.records.len(),
                });
                continue;
            };
            let local = Transform::from_record(*record);
            let world = frame.transform.compose(local);
            entry.effect.set_transform(world, MODEL_EFFECT_TRANSFORM_FLAGS);
            report.transformed += 1;
        }
        report
    }

    /// Only the specified scalar parameter product, not a binding of slot 4's
    /// unresolved (pos, dir, speed) signature/order/types (GF24). §3.11 says
    /// "each effect", so even stopped entries receive this parameter.
    pub fn update_with_motion(&mut self, speed: f32, direction: f32) -> UpdateReport {
        let report = self.update();
        for entry in self.entries.values_mut() {
            entry.effect.set_parameter(MOTION_PARAMETER, speed * direction);
        }
        report
    }

    /// Portable removal policy (GF20): stop(false) even if already stopped,
    /// then release exactly the created reference. Never release a borrowed one.
    pub fn remove(&mut self, key: &str) -> bool {
        if let Some(mut entry) = self.entries.remove(key) {
            entry.effect.stop(false);
            drop(entry);
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        while let Some((_, mut entry)) = self.entries.pop_first() {
            entry.effect.stop(false);
            drop(entry);
        }
    }
}

impl Drop for ModelRegEffects {
    fn drop(&mut self) {
        self.clear();
        // Effects die before the acquired instance/model reference does.
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

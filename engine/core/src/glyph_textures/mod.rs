// SPDX-License-Identifier: GPL-3.0-or-later
//! Portable owners from `docs/specs/glyph-textures.md` §§1–3.
//!
//! Tokens replace native pointers; AddRef/Release remain explicit (copying a token
//! does not acquire a reference). Storage is exclusively mutated, with ordinary
//! shared getters and no RefCell. A host can put the owner behind a Mutex.
//! Providers implement only foreign string, font, renderer and framework services.
//! No provider implements a glyph map, cache decision, owner size or lifetime.
//!
//! GT1 (font source identity) and GT2 (framework registration semantics) remain
//! foreign boundaries. This is bounded portable computation, not integration or
//! whole-owner/native-default credit. Native invalid memory and nontermination
//! are explicit unsupported paths, never simulated with a hang or unsafe access.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

use crate::fonts::FontRef;

#[cfg(test)]
mod tests;

pub const IID_GLYPH_TEXTURE_MANAGER: u32 = 0x0B54_C519;
pub const SERVICE_ID: u32 = 0xABEB_5E37;
pub const SERVICE_PRIORITY: i32 = -2_000_000;
pub const RESTORE_TEXTURES: u32 = 0x8ACA_A95F;
pub const DEFAULT_CACHE_BYTES: u32 = 1_048_576;

static CACHE_MISSES: AtomicU32 = AtomicU32::new(0);

/// §1.4's process-wide miss counter (including failed render attempts).
pub fn global_cache_misses() -> u32 {
    CACHE_MISSES.load(AtomicOrdering::Relaxed)
}

/// Owned foreign font reference. Dropping this value releases precisely the
/// reference transferred by lookup; the provider must not transfer a borrowed one.
pub trait TextFont {
    fn text_width(&self, text: &[u8]) -> u32;
    fn height(&self) -> u32;
}

/// Adapter to the existing portable font owner; no duplicate font measurement.
pub struct CoreFont(FontRef);

impl CoreFont {
    pub fn acquire(font: &FontRef) -> Self {
        font.add_ref();
        Self(font.clone())
    }
}

impl TextFont for CoreFont {
    fn text_width(&self, text: &[u8]) -> u32 {
        self.0.lock().get_string_width(text) as u32
    }

    fn height(&self) -> u32 {
        self.0.lock().char_height() as u32
    }
}

impl Drop for CoreFont {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// An owned foreign binding reference, released by Drop. Handle zero is allowed;
/// rendering success is distinct from the first handle's value.
pub trait TextureBinding {
    fn first_texture(&self) -> u32;
}

/// Only primitive foreign operations. `source_rect` is initially (0,0,1,1) for
/// glyphs; None denotes the string call, whose rectangle is not specified.
pub trait TextureServices {
    type Font: TextFont;
    type Binding: TextureBinding;

    fn lookup_font(&mut self, font_id: u32) -> Option<Self::Font>;
    fn render_text(
        &mut self,
        font: Option<&Self::Font>,
        text: &[u8],
        source_rect: Option<[f32; 4]>,
        binding: &mut Option<Self::Binding>,
    ) -> RenderOutputWords;
    fn compare_strings(&self, a: &[u8], b: &[u8], case_sensitive: bool) -> Ordering;
    fn char_length(&self, remaining: &[u8]) -> usize;
    fn binding_factory_exists(&self) -> bool;
    fn register_framework_slot(&mut self, slot: u32);
    fn subscribe(&mut self, message: u32);
    fn unsubscribe(&mut self, message: u32);
    /// Reference primitives for another manager, not delegated owner decisions.
    fn add_ref_manager(&mut self, manager: u64);
    fn release_manager(&mut self, manager: u64);
}

/// Raw renderer return/metric words; the binding is written through its slot.
pub struct RenderOutputWords {
    pub succeeded: bool,
    pub texture_size: [u32; 2],
    pub extents: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagerLink {
    Local,
    Foreign(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphKey {
    pub character: Vec<u8>,
    pub font_id: u32,
}

/// Tokens are local to one owner, like native pointers. Stale-token operations
/// return None/false rather than dereferencing freed native storage [choice].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlyphId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StringId(u64);

pub struct GlyphTextureInfo<B> {
    key: GlyphKey,
    texture_size: Option<[u16; 2]>,
    glyph_size: Option<[u16; 2]>,
    uv: Option<[f32; 4]>,
    binding: Option<B>,
    refs: u32,
    manager: Option<ManagerLink>,
    destroyed: bool,
}

impl<B> GlyphTextureInfo<B> {
    pub fn character(&self) -> &[u8] {
        &self.key.character
    }

    /// Slot 3 replaces the destination, rather than appending to it.
    pub fn copy_character(&self, out: &mut Vec<u8>) {
        out.clone_from(&self.key.character);
    }

    pub fn texture_size(&self) -> Option<[u32; 2]> {
        self.texture_size.map(|words| words.map(u32::from))
    }

    pub fn glyph_size(&self) -> Option<[u32; 2]> {
        self.glyph_size.map(|words| words.map(u32::from))
    }

    pub fn uv(&self) -> Option<[f32; 4]> {
        self.uv
    }

    /// Slot 7 is a borrowed load, without AddRef (§3.1).
    pub fn binding(&self) -> Option<&B> {
        self.binding.as_ref()
    }

    pub fn reference_count(&self) -> u32 {
        self.refs
    }

    pub fn has_manager(&self) -> bool {
        self.manager.is_some()
    }

    /// Both info slot-0 bodies ignore IID and output, including IID 1.
    pub fn query_interface<T>(&self, _iid: u32, _out: &mut T) -> bool {
        false
    }
}

pub struct StringTextureInfo<F> {
    size: Option<[u16; 2]>,
    refs: u32,
    manager: Option<ManagerLink>,
    text: Vec<u8>,
    font: Option<F>,
    destroyed: bool,
}

impl<F> StringTextureInfo<F> {
    pub fn size(&self) -> Option<[u32; 2]> {
        self.size.map(|words| words.map(u32::from))
    }

    pub fn reference_count(&self) -> u32 {
        self.refs
    }

    pub fn has_manager(&self) -> bool {
        self.manager.is_some()
    }

    fn release(&mut self) -> u32 {
        self.refs = self.refs.wrapping_sub(1);
        self.refs
    }

    pub fn query_interface<T>(&self, _iid: u32, _out: &mut T) -> bool {
        false
    }
}

struct CacheEntry<B> {
    binding: B,
    owner: StringId,
    bytes: u32,
    used: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringError {
    /// Native dereferences a null font; this safe model cannot execute that path.
    UnsupportedNullFont,
    /// Native signed P loops forever on this domain; this is not a native error.
    UnsupportedNonterminatingPower,
    UnsupportedManagerLink,
    UnsupportedUninitializedInfo,
    UnsupportedNullBinding,
    StaleToken,
    DeletedManager,
}

/// §3.2 x87 arithmetic, after the single float32 B store. Integer fixed-point
/// evaluation preserves all bits of the float32 constants and intermediate
/// products (at most 59 significant bits here, within x87's 64-bit significand).
/// No intermediate float32/f64 rounding is introduced before truncation.
pub fn sweep_target(bytes: u32, budget: u32) -> u32 {
    let b = (bytes as i32 as f32) as i64;
    let l = i64::from(budget as i32);
    let above = match l.cmp(&0) {
        Ordering::Greater => b * 8_388_608 > l * 9_227_469,
        Ordering::Less => b * 8_388_608 < l * 9_227_469,
        Ordering::Equal => b > 0, // +infinity; 0/0 is unordered.
    };
    if above {
        ((b * 134_217_728 - l * 13_421_773) / 134_217_728) as u32
    } else {
        budget
    }
}

fn padded_dimension(value: u32) -> Result<u16, StringError> {
    let n = value.wrapping_add(2) as i32;
    if n <= 1 {
        Ok(1)
    } else if n <= 0x4000_0000 {
        Ok((n as u32).next_power_of_two() as u16)
    } else {
        Err(StringError::UnsupportedNonterminatingPower)
    }
}

fn narrow_metric(left: f32, right: f32) -> u16 {
    // Error-free TwoSum retains the subtraction tail lost by a single f64
    // operation. Only an integer rounded sum can cross a truncation boundary
    // when rounded to x87's 64-bit significand. At such a boundary, compare the
    // tail with half the x87 spacing toward zero. Ties round to the integer
    // (its significand is even throughout the signed conversion domain).
    let a = f64::from(left);
    let b = -f64::from(right);
    let sum = a + b;
    let part = sum - a;
    let tail = (a - (sum - part)) + (b - part);
    let mut integer = sum.trunc();
    if sum != 0.0 && sum.fract() == 0.0 && tail.signum() != sum.signum() {
        let bits = sum.abs().to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
        let power_of_two = bits & ((1_u64 << 52) - 1) == 0;
        let half_spacing = 2.0_f64.powi(exponent - 64 - i32::from(power_of_two));
        if tail.abs() > half_spacing {
            integer -= sum.signum();
        }
    }
    // Only the published finite signed-integer conversion domain is supported;
    // do not promise Rust's saturating float cast as an x87 invalid conversion.
    assert!(
        integer.is_finite() && (-2_147_483_648.0..2_147_483_648.0).contains(&integer),
        "unsupported x87 invalid integer conversion"
    );
    (integer as i32) as u16
}

/// Correctly round a float32 word divided by a signed dimension to float32.
/// Integer quotient/remainder avoids a possible f64-to-f32 double rounding.
/// These 24/31-bit inputs also fit the x87 precision before the final store.
fn stored_uv(value: f32, dimension: u32) -> f32 {
    let signed = dimension as i32;
    if signed == 0 || !value.is_finite() || value == 0.0 {
        return (f64::from(value) / f64::from(signed)) as f32;
    }
    let bits = value.to_bits();
    let sign = (bits & 0x8000_0000) ^ if signed < 0 { 0x8000_0000 } else { 0 };
    let exponent = ((bits >> 23) & 0xff) as i32;
    let numerator = u128::from((bits & 0x7f_ffff) | if exponent == 0 { 0 } else { 1 << 23 });
    let denominator = u128::from(signed.unsigned_abs());
    let base = if exponent == 0 { -149 } else { exponent - 150 };
    let mut ratio_exponent = (127 - numerator.leading_zeros()) as i32 - (127 - denominator.leading_zeros()) as i32;
    if (ratio_exponent >= 0 && numerator < denominator << ratio_exponent)
        || (ratio_exponent < 0 && numerator << -ratio_exponent < denominator)
    {
        ratio_exponent -= 1;
    }
    let output_exponent = base + ratio_exponent;
    let shift = if output_exponent < -126 {
        base + 149
    } else {
        23 - ratio_exponent
    };
    let (scaled, divisor) = if shift >= 0 {
        (numerator << shift, denominator)
    } else {
        (numerator, denominator << -shift)
    };
    let mut quotient = scaled / divisor;
    let remainder = scaled % divisor;
    if remainder * 2 > divisor || (remainder * 2 == divisor && quotient & 1 != 0) {
        quotient += 1;
    }
    let magnitude = if output_exponent < -126 {
        quotient as u32 // Can round upward into the smallest normal.
    } else {
        ((output_exponent + 126) as u32) * (1 << 23) + quotient as u32
    };
    f32::from_bits(sign | magnitude)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayUnsupported {
    NativeNontermination,
    NativeInvalidRead,
}

/// Owns the glyph map and string cache. Ordered IDs are safe pointer surrogates;
/// key comparison is the framework string primitive, then unsigned font ID.
pub struct GlyphTextureManager<P: TextureServices> {
    services: P,
    refs: u32,
    deleted: bool,
    initialized: bool,
    service_id: u32,
    priority: u32,
    second_priority: u32,
    running: u8,
    // The published constructor does not specify the auxiliary counter word.
    auxiliary_count: Option<u32>,
    allocation_freed: bool,
    glyph_map: Vec<(GlyphKey, GlyphId)>,
    tree_count: u32,
    glyphs: BTreeMap<GlyphId, GlyphTextureInfo<P::Binding>>,
    strings: BTreeMap<StringId, StringTextureInfo<P::Font>>,
    cache: Vec<CacheEntry<P::Binding>>,
    entry_count: u32,
    // Native destruction frees nodes without releasing their pointer values.
    // Keep those references in the safe host arena until Rust context teardown.
    unreleased_bindings: Vec<P::Binding>,
    budget: u32,
    bytes: u32,
    last_sweep: u32,
    misses: u32,
    next_id: u64,
}

impl<P: TextureServices> GlyphTextureManager<P> {
    pub fn new(services: P) -> Self {
        Self {
            services,
            refs: 0,
            deleted: false,
            initialized: false,
            service_id: SERVICE_ID,
            priority: SERVICE_PRIORITY as u32,
            second_priority: SERVICE_PRIORITY as u32,
            running: 0,
            auxiliary_count: None,
            allocation_freed: false,
            glyph_map: Vec::new(),
            tree_count: 0,
            glyphs: BTreeMap::new(),
            strings: BTreeMap::new(),
            cache: Vec::new(),
            entry_count: 0,
            unreleased_bindings: Vec::new(),
            budget: DEFAULT_CACHE_BYTES,
            bytes: 0,
            last_sweep: 0,
            misses: 0,
            next_id: 0,
        }
    }

    pub fn services(&self) -> &P {
        &self.services
    }

    pub fn service_id(&self) -> u32 {
        self.service_id
    }
    pub fn set_service_id(&mut self, word: u32) {
        self.service_id = word;
    }
    pub fn priority(&self) -> u32 {
        self.priority
    }
    pub fn second_priority(&self) -> u32 {
        self.second_priority
    }
    pub fn running(&self) -> u8 {
        self.running
    }
    pub fn set_running(&mut self, word: u32) {
        self.running = word as u8;
    }
    pub fn service_slot_11(&self, _word: u32) -> bool {
        true
    }
    pub fn auxiliary_count(&self) -> Option<u32> {
        self.auxiliary_count
    }
    pub fn auxiliary_release(&mut self) -> Option<u32> {
        let count = self.auxiliary_count.as_mut()?;
        *count = count.saturating_sub(1);
        Some(*count)
    }

    /// +4/+8 AddRef and message-target AddRef/Release/QI use the same primary
    /// methods, with receiver adjustment only. No neighboring ordinance thunks
    /// are exposed here. Auxiliary slot 3 uses deleting_destructor below.
    pub fn reference_count(&self) -> u32 {
        self.refs
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn deleted(&self) -> bool {
        self.deleted
    }

    pub fn budget(&self) -> u32 {
        self.budget
    }

    pub fn cached_bytes(&self) -> u32 {
        self.bytes
    }

    pub fn cached_entries(&self) -> u32 {
        self.entry_count
    }

    pub fn last_sweep(&self) -> u32 {
        self.last_sweep
    }

    /// This manager's contribution to [`global_cache_misses`].
    pub fn cache_misses(&self) -> u32 {
        self.misses
    }

    pub fn glyph(&self, id: GlyphId) -> Option<&GlyphTextureInfo<P::Binding>> {
        self.glyphs.get(&id)
    }

    pub fn string(&self, id: StringId) -> Option<&StringTextureInfo<P::Font>> {
        self.strings.get(&id)
    }

    pub fn glyph_keys(&self) -> impl Iterator<Item = &GlyphKey> {
        self.glyph_map.iter().map(|(key, _)| key)
    }

    pub fn cache_order(&self) -> impl Iterator<Item = (StringId, bool)> + '_ {
        self.cache.iter().map(|entry| (entry.owner, entry.used))
    }

    pub fn add_ref(&mut self) -> Option<u32> {
        if self.deleted {
            return None;
        }
        self.refs = self.refs.wrapping_add(1);
        Some(self.refs)
    }

    pub fn query_interface(&mut self, iid: u32) -> bool {
        iid == IID_GLYPH_TEXTURE_MANAGER && self.add_ref().is_some()
    }

    pub fn release(&mut self) -> Option<u32> {
        if self.deleted {
            return None;
        }
        self.refs = self.refs.wrapping_sub(1);
        if self.refs == 0 {
            self.deleting_destructor(1);
        }
        Some(self.refs)
    }

    /// `cache_kb` is the render-properties value, or 1024 when absent. The
    /// provider performs the two published registration calls in their order.
    pub fn init(&mut self, cache_kb: u32) -> bool {
        if !self.initialized && !self.deleted {
            self.initialized = true;
            self.budget = cache_kb.wrapping_mul(1024);
            self.services.register_framework_slot(3);
            self.services.register_framework_slot(9);
            self.services.subscribe(RESTORE_TEXTURES);
        }
        true
    }

    pub fn shutdown(&mut self) -> bool {
        if self.initialized {
            let entries: Vec<_> = self.glyph_map.iter().map(|(_, id)| *id).collect();
            for id in entries {
                self.set_glyph_manager(id, None);
                self.release_glyph(id);
            }
            self.glyph_map.clear();
            self.tree_count = 0;
            self.services.unsubscribe(RESTORE_TEXTURES);
            self.initialized = false;
        }
        // §1.2 explicitly leaves every string cache entry alone.
        true
    }

    fn allocate_id(&mut self) -> u64 {
        self.next_id = self.next_id.checked_add(1).expect("portable token space exhausted");
        self.next_id
    }

    fn find_key(&self, key: &GlyphKey) -> Result<usize, usize> {
        self.glyph_map.binary_search_by(|(existing, _)| {
            self.services
                .compare_strings(&existing.character, &key.character, true)
                .then_with(|| existing.font_id.cmp(&key.font_id))
        })
    }

    /// The two private tree helpers' portable result. A sorted Vec models node
    /// ordering/sentinel endpoints without claiming a native tree allocation.
    /// Duplicate insertion preserves the first pointer and makes no AddRef.
    fn insert_unique(&mut self, key: GlyphKey, id: GlyphId) -> (GlyphId, u8) {
        match self.find_key(&key) {
            Ok(index) => (self.glyph_map[index].1, 0),
            Err(index) => {
                self.glyph_map.insert(index, (key, id));
                self.tree_count = self.tree_count.wrapping_add(1);
                (id, 1)
            }
        }
    }

    fn acquire_manager_link(&mut self, link: ManagerLink) {
        match link {
            ManagerLink::Local => {
                self.add_ref();
            }
            ManagerLink::Foreign(id) => self.services.add_ref_manager(id),
        }
    }

    fn release_manager_link(&mut self, link: ManagerLink) {
        match link {
            ManagerLink::Local => {
                self.release();
            }
            ManagerLink::Foreign(id) => self.services.release_manager(id),
        }
    }

    pub fn set_glyph_manager(&mut self, id: GlyphId, new: Option<ManagerLink>) -> Option<()> {
        let old = self.glyphs.get(&id)?.manager;
        if old == new {
            return Some(());
        }
        if let Some(link) = new {
            self.acquire_manager_link(link);
        }
        self.glyphs.get_mut(&id)?.manager = new;
        if let Some(link) = old {
            self.release_manager_link(link);
        }
        Some(())
    }

    /// Constructor state has no initialized size/UV fields and no references.
    pub fn create_glyph_info(&mut self) -> GlyphId {
        let id = GlyphId(self.allocate_id());
        self.glyphs.insert(
            id,
            GlyphTextureInfo {
                key: GlyphKey {
                    character: Vec::new(),
                    font_id: 0,
                },
                texture_size: None,
                glyph_size: None,
                uv: None,
                binding: None,
                refs: 0,
                manager: None,
                destroyed: false,
            },
        );
        id
    }

    pub fn create_string_info(&mut self) -> StringId {
        let id = StringId(self.allocate_id());
        self.strings.insert(
            id,
            StringTextureInfo {
                size: None,
                refs: 0,
                manager: None,
                text: Vec::new(),
                font: None,
                destroyed: false,
            },
        );
        id
    }

    /// Native failures leave the caller's output untouched.
    pub fn get_glyph_texture_out(&mut self, key: GlyphKey, out: &mut Option<GlyphId>) -> bool {
        if let Some(id) = self.get_glyph_texture(key) {
            *out = Some(id);
            true
        } else {
            false
        }
    }

    pub fn get_glyph_texture(&mut self, key: GlyphKey) -> Option<GlyphId> {
        if self.deleted {
            return None;
        }
        if let Ok(index) = self.find_key(&key) {
            let id = self.glyph_map[index].1;
            self.add_ref_glyph(id);
            return Some(id);
        }
        let font = self.services.lookup_font(key.font_id)?;
        let mut binding = None;
        let rendered = self
            .services
            .render_text(Some(&font), &key.character, Some([0.0, 0.0, 1.0, 1.0]), &mut binding);
        if !rendered.succeeded {
            return None;
        }
        let [w, h] = rendered.texture_size;
        let [r0, r1, r2, r3] = rendered.extents;
        let id = self.create_glyph_info();
        self.set_glyph_manager(id, Some(ManagerLink::Local));
        let info = self.glyphs.get_mut(&id).expect("constructed glyph");
        info.key = key.clone();
        info.texture_size = Some(rendered.texture_size.map(|n| n as u16));
        // Keep x87 subtraction precision until truncation; no float32 store.
        info.glyph_size = Some([narrow_metric(r0, r2), narrow_metric(r1, r3)]);
        let [v0, v1, v2, v3] = rendered.extents;
        info.uv = Some([stored_uv(v2, w), stored_uv(v3, h), stored_uv(v0, w), stored_uv(v1, h)]);
        info.binding = binding;
        self.insert_unique(key, id);
        self.add_ref_glyph(id); // Separate map and caller references, not helper work.
        self.add_ref_glyph(id);
        Some(id)
    }

    pub fn add_ref_glyph(&mut self, id: GlyphId) -> Option<u32> {
        let info = self.glyphs.get_mut(&id)?;
        if info.destroyed {
            return None;
        }
        info.refs = info.refs.wrapping_add(1);
        Some(info.refs)
    }

    pub fn release_glyph(&mut self, id: GlyphId) -> Option<u32> {
        let info = self.glyphs.get_mut(&id)?;
        if info.destroyed {
            return None;
        }
        info.refs = info.refs.wrapping_sub(1);
        let count = info.refs;
        if count == 1 && info.manager.is_some() {
            // Foreign managers are primitives; the local owner computes its own
            // identity scan/erasure/Release. A foreign link is not integrated.
            assert_eq!(
                info.manager,
                Some(ManagerLink::Local),
                "unsupported foreign manager RemoveGlyph dispatch"
            );
            self.remove_glyph(id);
            return Some(self.glyphs.get(&id).map_or(0, |glyph| glyph.refs));
        }
        if count == 0 {
            self.destroy_glyph(id, 1);
        }
        Some(count)
    }

    /// Erase the first matching pointer before Release. An absent pointer does
    /// not consume a reference, even when another glyph has an equal key.
    pub fn remove_glyph(&mut self, id: GlyphId) {
        if let Some(index) = self.glyph_map.iter().position(|(_, existing)| *existing == id) {
            self.glyph_map.remove(index);
            self.tree_count = self.tree_count.wrapping_sub(1);
            self.release_glyph(id);
        }
    }

    pub fn destroy_glyph(&mut self, id: GlyphId, flags: u32) -> Option<GlyphId> {
        let info = self.glyphs.get_mut(&id)?;
        if info.destroyed {
            return None;
        }
        info.destroyed = true;
        let manager = info.manager.take();
        if let Some(link) = manager {
            self.release_manager_link(link);
        }
        let info = self.glyphs.get_mut(&id)?;
        drop(info.binding.take());
        info.key.character.clear();
        if flags & 1 != 0 {
            self.glyphs.remove(&id);
        }
        Some(id)
    }

    pub fn destroy_string(&mut self, id: StringId, flags: u32) -> Option<StringId> {
        let info = self.strings.get_mut(&id)?;
        if info.destroyed {
            return None;
        }
        info.destroyed = true;
        drop(info.font.take());
        info.text.clear();
        let manager = info.manager.take();
        if let Some(link) = manager {
            self.release_manager_link(link);
        }
        if flags & 1 != 0 {
            self.strings.remove(&id);
        }
        Some(id)
    }

    /// Clear and Release the old vector first. Unsupported reports native
    /// nontermination/invalid memory; it is never a claimed native safety stop.
    pub fn string_to_glyph_texture_array(
        &mut self,
        font_id: u32,
        text: &[u8],
        out: &mut Vec<GlyphId>,
    ) -> Result<(), ArrayUnsupported> {
        for id in out.drain(..) {
            self.release_glyph(id);
        }
        let mut offset = 0;
        let mut remaining = text.len() as u32;
        while (remaining as i32) > 0 {
            let length = self.services.char_length(&text[offset..]);
            if length == 0 {
                return Err(ArrayUnsupported::NativeNontermination);
            }
            if length > text.len() - offset {
                return Err(ArrayUnsupported::NativeInvalidRead);
            }
            let key = GlyphKey {
                character: text[offset..offset + length].to_vec(),
                font_id,
            };
            if let Some(id) = self.get_glyph_texture(key) {
                out.push(id);
            }
            offset += length;
            remaining = remaining.wrapping_sub(length as u32);
        }
        Ok(())
    }

    pub fn get_cached_string_texture(&mut self, font_id: u32, text: &[u8]) -> Result<StringId, StringError> {
        if self.deleted {
            return Err(StringError::DeletedManager);
        }
        // Native has no null-font branch: report unsupported invalid memory,
        // rather than presenting a safe rejection as native behavior.
        let font = self
            .services
            .lookup_font(font_id)
            .ok_or(StringError::UnsupportedNullFont)?;
        let size = [
            padded_dimension(font.text_width(text))?,
            padded_dimension(font.height())?,
        ];
        let id = self.create_string_info();
        self.acquire_manager_link(ManagerLink::Local);
        let info = self.strings.get_mut(&id).expect("constructed string");
        info.manager = Some(ManagerLink::Local);
        info.font = Some(font);
        info.text = text.to_vec();
        info.size = Some(size);
        self.add_ref_string(id);
        Ok(id)
    }

    pub fn add_ref_string(&mut self, id: StringId) -> Option<u32> {
        let info = self.strings.get_mut(&id)?;
        if info.destroyed {
            return None;
        }
        info.refs = info.refs.wrapping_add(1);
        Some(info.refs)
    }

    pub fn release_string(&mut self, id: StringId) -> Option<u32> {
        let info = self.strings.get_mut(&id)?;
        if info.destroyed {
            return None;
        }
        let count = info.release();
        if count == 0 || (count == 1 && info.manager.is_none()) {
            self.destroy_string(id, 1);
        }
        Some(if count <= 1 { 0 } else { count })
    }

    /// §3 slot 4 / §1.4: a token identifies a string object, not its text content.
    /// Panics on an explicitly unsupported native path; valid native render
    /// failures still return zero. Use try_string_texture to inspect the boundary.
    pub fn string_texture(&mut self, id: StringId) -> u32 {
        self.try_string_texture(id)
            .expect("unsupported native string texture path")
    }

    pub fn try_string_texture(&mut self, id: StringId) -> Result<u32, StringError> {
        if self.deleted {
            return Err(StringError::DeletedManager);
        }
        let info = self.strings.get(&id).ok_or(StringError::StaleToken)?;
        // An absent manager/font is a native invalid dereference, unsupported
        // by this token API; no independent standalone renderer is invented.
        if info.manager != Some(ManagerLink::Local) {
            return Err(StringError::UnsupportedManagerLink);
        }
        if let Some(index) = self.cache.iter().position(|entry| entry.owner == id) {
            let mut entry = self.cache.remove(index);
            entry.used = true;
            self.cache.push(entry);
            return Ok(self
                .cache
                .last()
                .expect("just appended cache hit")
                .binding
                .first_texture());
        }
        self.misses = self.misses.wrapping_add(1);
        CACHE_MISSES.fetch_add(1, AtomicOrdering::Relaxed);
        let info = &self.strings[&id];
        let font = info.font.as_ref().ok_or(StringError::UnsupportedNullFont)?;
        let mut binding = None;
        let rendered = self.services.render_text(Some(font), &info.text, None, &mut binding);
        if !rendered.succeeded {
            return Ok(0);
        }
        let binding = binding.ok_or(StringError::UnsupportedNullBinding)?;
        let [w, h] = info.size().ok_or(StringError::UnsupportedUninitializedInfo)?;
        let bytes = w.wrapping_mul(h).wrapping_mul(4);
        self.add_ref_string(id);
        self.cache.push(CacheEntry {
            binding,
            owner: id,
            bytes,
            used: true,
        });
        self.bytes = self.bytes.wrapping_add(bytes);
        self.entry_count = self.entry_count.wrapping_add(1);
        Ok(self
            .cache
            .last()
            .expect("just appended cache miss")
            .binding
            .first_texture())
    }

    fn drop_cache_entry(&mut self, index: usize) {
        let entry = self.cache.remove(index);
        self.bytes = self.bytes.wrapping_sub(entry.bytes);
        self.entry_count = self.entry_count.wrapping_sub(1);
        drop(entry.binding);
        self.release_string(entry.owner);
    }

    pub fn on_tick(&mut self, time: u32) {
        if self.deleted {
            return;
        }
        if time < self.last_sweep {
            self.last_sweep = time;
        }
        if time - self.last_sweep > 50 {
            self.last_sweep = time;
            if (self.bytes as i32) > (self.budget as i32) {
                let target = sweep_target(self.bytes, self.budget);
                let mut index = 0;
                // One oldest-to-newest pass; do not revisit just-cleared entries.
                while index < self.cache.len() && (self.bytes as i32) > (target as i32) {
                    if self.cache[index].used {
                        self.cache[index].used = false;
                        index += 1;
                    } else {
                        self.drop_cache_entry(index);
                    }
                }
            }
        }
    }

    pub fn do_message(&mut self, message: u32) -> bool {
        if message == RESTORE_TEXTURES && !self.deleted {
            self.clear_cache();
            if self.services.binding_factory_exists() {
                for (key, id) in &self.glyph_map {
                    let info = self.glyphs.get_mut(id).expect("live glyph map reference");
                    drop(info.binding.take()); // Must precede lookup and re-render.
                                               // Font identity belongs to the map key, not glyph-info's
                                               // stored character/size/UV payload (§§1.1, 2).
                    let font = self.services.lookup_font(key.font_id);
                    // The renderer gets the actual cleared slot, regardless of
                    // lookup/success. Ignore metric outputs and success status.
                    self.services.render_text(
                        font.as_ref(),
                        &info.key.character,
                        Some([0.0, 0.0, 1.0, 1.0]),
                        &mut info.binding,
                    );
                }
            }
        }
        true
    }

    fn clear_cache(&mut self) {
        while !self.cache.is_empty() {
            self.drop_cache_entry(0);
        }
        self.bytes = 0;
        self.entry_count = 0;
    }

    /// The returned pointer is an identity observation only. Rust owns the host
    /// allocation; bit 0 records native deletion without freeing a borrowed self.
    pub fn deleting_destructor(&mut self, flags: u32) -> *mut Self {
        let address = self as *mut Self;
        if self.deleted {
            return address;
        }
        self.deleted = true;
        for entry in self.cache.drain(..) {
            self.unreleased_bindings.push(entry.binding);
        }
        self.glyph_map.clear(); // Destroy key strings/nodes, never mapped glyphs.
        self.allocation_freed = flags & 1 != 0;
        address
    }
}

impl<P: TextureServices> Drop for GlyphTextureManager<P> {
    fn drop(&mut self) {
        // Native destruction does not release the arena's referenced objects.
        // Rust allocation teardown after this call drops remaining safe arena
        // values; that host-only resource cleanup is not native Release behavior.
        self.deleting_destructor(0);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::sync::{Arc, Mutex};

type Log = Arc<Mutex<Vec<String>>>;

fn record(log: &Log, message: String) {
    log.lock().unwrap().push(message);
}

struct Font {
    width: u32,
    height: u32,
    log: Log,
    id: u32,
}

impl TextFont for Font {
    fn text_width(&self, _text: &[u8]) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        record(&self.log, format!("font-release:{}", self.id));
    }
}

struct Binding {
    handle: u32,
    log: Log,
}

impl TextureBinding for Binding {
    fn first_texture(&self) -> u32 {
        self.handle
    }
}

impl Drop for Binding {
    fn drop(&mut self) {
        record(&self.log, format!("binding-release:{}", self.handle));
    }
}

struct Services {
    log: Log,
    width: u32,
    height: u32,
    missing_font: Option<u32>,
    failed_text: Vec<Vec<u8>>,
    serial: u32,
    factory: bool,
    texture_size: [u32; 2],
    extents: [f32; 4],
    char_length_override: Option<usize>,
    write_on_failure: bool,
}

impl Default for Services {
    fn default() -> Self {
        Self {
            log: Log::default(),
            width: 61,
            height: 13,
            missing_font: None,
            failed_text: Vec::new(),
            serial: 0,
            factory: true,
            texture_size: [64, 16],
            extents: [62.0, 15.0, 1.0, 2.0],
            char_length_override: None,
            write_on_failure: false,
        }
    }
}

impl TextureServices for Services {
    type Font = Font;
    type Binding = Binding;

    fn lookup_font(&mut self, id: u32) -> Option<Font> {
        record(&self.log, format!("font-lookup:{id}"));
        (self.missing_font != Some(id)).then(|| Font {
            width: self.width,
            height: self.height,
            log: self.log.clone(),
            id,
        })
    }

    fn render_text(&mut self, font: Option<&Font>, text: &[u8], rect: Option<[f32; 4]>,
        binding: &mut Option<Binding>) -> RenderOutputWords {
        record(&self.log, format!("render:{}:{text:?}:{rect:?}", font.map_or(0, |f| f.id)));
        let succeeded = !self.failed_text.iter().any(|failed| failed == text);
        if succeeded || self.write_on_failure {
            self.serial += 1;
            *binding = Some(Binding {
                handle: self.serial,
                log: self.log.clone(),
            });
        }
        RenderOutputWords {
            succeeded,
            texture_size: self.texture_size,
            extents: self.extents,
        }
    }

    fn compare_strings(&self, a: &[u8], b: &[u8], sensitive: bool) -> Ordering {
        assert!(sensitive);
        a.cmp(b)
    }

    fn char_length(&self, remaining: &[u8]) -> usize {
        // A test code page with a two-byte lead; deliberately not UTF-8.
        self.char_length_override
            .unwrap_or(if remaining[0] == 0x81 { 2 } else { 1 })
    }

    fn binding_factory_exists(&self) -> bool {
        self.factory
    }

    fn register_framework_slot(&mut self, slot: u32) {
        record(&self.log, format!("register:{slot}"));
    }

    fn subscribe(&mut self, message: u32) {
        record(&self.log, format!("subscribe:{message:08x}"));
    }

    fn unsubscribe(&mut self, message: u32) {
        record(&self.log, format!("unsubscribe:{message:08x}"));
    }

    fn add_ref_manager(&mut self, id: u64) {
        record(&self.log, format!("manager-addref:{id}"));
    }
    fn release_manager(&mut self, id: u64) {
        record(&self.log, format!("manager-release:{id}"));
    }
}

fn manager() -> GlyphTextureManager<Services> {
    let mut m = GlyphTextureManager::new(Services::default());
    m.add_ref();
    assert!(m.init(1024));
    m
}

fn key(character: &[u8], font_id: u32) -> GlyphKey {
    GlyphKey {
        character: character.to_vec(),
        font_id,
    }
}

fn log(m: &GlyphTextureManager<Services>) -> Vec<String> {
    m.services.log.lock().unwrap().clone()
}

#[test]
fn published_sharing_and_last_external_reference() {
    let mut m = manager();
    let a = m.get_glyph_texture(key(b"A", 7)).unwrap();
    assert_eq!(m.glyph(a).unwrap().reference_count(), 2);
    assert_eq!(m.reference_count(), 2);
    let b = m.get_glyph_texture(key(b"A", 7)).unwrap();
    assert_eq!(a, b);
    assert_eq!(m.services.serial, 1);
    assert_eq!(m.release_glyph(a), Some(2));
    assert_eq!(m.release_glyph(b), Some(0));
    assert!(m.glyph(a).is_none());
    assert_eq!(m.glyph_keys().count(), 0);
    assert_eq!(m.reference_count(), 1);
    let c = m.get_glyph_texture(key(b"A", 7)).unwrap();
    assert_ne!(a, c);
    assert_eq!(m.services.serial, 2);
    assert!(log(&m).contains(&"binding-release:1".to_string()));
}

#[test]
fn identity_init_once_shutdown_leaves_cache_and_external_glyphs() {
    let mut m = manager();
    assert!(!m.query_interface(1));
    assert!(!m.query_interface(SERVICE_ID));
    assert_eq!(m.reference_count(), 1);
    assert!(m.query_interface(IID_GLYPH_TEXTURE_MANAGER));
    assert_eq!(m.release(), Some(1));
    m.init(1);
    assert_eq!(m.budget(), DEFAULT_CACHE_BYTES);
    assert_eq!(&log(&m)[..3], ["register:3", "register:9", "subscribe:8acaa95f"]);
    assert_eq!(log(&m).iter().filter(|s| s.starts_with("register:")).count(), 2);
    let g = m.get_glyph_texture(key(b"G", 1)).unwrap();
    let s = m.get_cached_string_texture(1, b"S").unwrap();
    m.string_texture(s);
    assert_eq!(m.reference_count(), 3);
    m.shutdown();
    assert!(!m.initialized());
    assert!(m.glyph_keys().next().is_none());
    assert_eq!(m.glyph(g).unwrap().reference_count(), 1);
    assert!(!m.glyph(g).unwrap().has_manager());
    assert_eq!(m.reference_count(), 2);
    assert_eq!((m.cached_entries(), m.cached_bytes()), (1, 4096));
    assert_eq!(m.release_glyph(g), Some(0));
    m.shutdown();
    assert_eq!(log(&m).iter().filter(|s| s.starts_with("unsubscribe:")).count(), 1);
    m.init(2);
    assert_eq!(m.budget(), 2048);
}

#[test]
fn glyph_outputs_sizes_uv_and_case_font_order() {
    let mut m = manager();
    let g = m.get_glyph_texture(key(b"Z", 7)).unwrap();
    let info = m.glyph(g).unwrap();
    let mut out = b"old content".to_vec();
    info.copy_character(&mut out);
    assert_eq!(out, b"Z");
    assert_eq!(info.texture_size(), Some([64, 16]));
    assert_eq!(info.glyph_size(), Some([61, 13]));
    assert_eq!(info.uv(), Some([0.015625, 0.125, 0.96875, 0.9375]));
    assert_eq!(info.binding().unwrap().first_texture(), 1);
    m.get_glyph_texture(key(b"a", 2)).unwrap();
    m.get_glyph_texture(key(b"A", 9)).unwrap();
    m.get_glyph_texture(key(b"A", 1)).unwrap();
    assert_eq!(
        m.glyph_keys().cloned().collect::<Vec<_>>(),
        [key(b"A", 1), key(b"A", 9), key(b"Z", 7), key(b"a", 2)]
    );
    assert!(log(&m).iter().any(|s| s.ends_with("Some([0.0, 0.0, 1.0, 1.0])")));
}

#[test]
fn missing_font_and_failed_glyph_render_leave_no_map_or_manager_reference() {
    let mut m = manager();
    m.services.missing_font = Some(3);
    assert!(m.get_glyph_texture(key(b"A", 3)).is_none());
    m.services.failed_text.push(b"bad".to_vec());
    assert!(m.get_glyph_texture(key(b"bad", 7)).is_none());
    assert_eq!(m.reference_count(), 1);
    assert_eq!(m.glyph_keys().count(), 0);
    assert_eq!(m.services.serial, 0);
    assert_eq!(log(&m).last().unwrap(), "font-release:7");
}

#[test]
fn character_array_clears_releases_then_uses_code_page_and_skips_failure() {
    let mut m = manager();
    let first = m.get_glyph_texture(key(b"old", 1)).unwrap();
    let mut output = vec![first];
    m.services.failed_text.push(b"!".to_vec());
    m.string_to_glyph_texture_array(7, b"A\x81\x40!A", &mut output).unwrap();
    assert!(m.glyph(first).is_none());
    assert_eq!(output.len(), 3);
    assert_eq!(output[0], output[2]);
    assert_eq!(m.glyph(output[1]).unwrap().character(), b"\x81\x40");
    assert_eq!(m.glyph(output[0]).unwrap().reference_count(), 3);
    m.string_to_glyph_texture_array(7, b"", &mut output).unwrap();
    assert!(output.is_empty());
    m.services.char_length_override = Some(0);
    assert_eq!(m.string_to_glyph_texture_array(7, b"A", &mut output), Err(ArrayUnsupported::NativeNontermination));
    m.services.char_length_override = Some(8);
    assert_eq!(m.string_to_glyph_texture_array(7, b"A", &mut output), Err(ArrayUnsupported::NativeInvalidRead));
    assert!(output.is_empty());
}

#[test]
fn published_size_vectors_and_padded_dimension_boundaries() {
    for line in include_str!("size-vectors.txt")
        .lines()
        .filter(|s| !s.starts_with('#') && !s.is_empty())
    {
        let v: Vec<u32> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        let mut m = manager();
        m.services.width = v[0];
        m.services.height = v[1];
        let id = m.get_cached_string_texture(7, b"test").unwrap();
        assert_eq!(m.string(id).unwrap().size(), Some([v[2], v[3]]), "{line}");
        assert_eq!(m.services.serial, 0);
    }
    assert_eq!(padded_dimension(u32::MAX), Ok(1));
    assert_eq!(
        padded_dimension(0x8000_0000),
        Ok(1)
    );
}

#[test]
fn published_lazy_render_identity_and_cache_reference() {
    let mut m = manager();
    let a = m.get_cached_string_texture(7, b"same").unwrap();
    let b = m.get_cached_string_texture(7, b"same").unwrap();
    assert_ne!(a, b); // GetCachedStringTexture creates an object on every call.
    assert_eq!(m.services.serial, 0);
    assert_eq!(m.string(a).unwrap().reference_count(), 1);
    assert_eq!(m.string_texture(a), 1);
    assert_eq!(m.string_texture(a), 1);
    assert_eq!(m.services.serial, 1);
    assert_eq!(m.cache_misses(), 1);
    assert_eq!(m.string(a).unwrap().reference_count(), 2);
    assert_eq!((m.cached_bytes(), m.cached_entries()), (4096, 1));
    assert_eq!(m.string_texture(b), 2);
    assert_eq!(m.cached_entries(), 2);
    assert_eq!(m.release_string(a), Some(0));
    assert!(m.string(a).is_some());
    m.do_message(RESTORE_TEXTURES);
    assert!(m.string(a).is_none());
    assert_eq!(m.string(b).unwrap().reference_count(), 1);
    assert_eq!(m.string_texture(b), 3);
}

#[test]
fn failed_string_render_retries_and_typed_precreation_failures() {
    let mut m = manager();
    m.services.missing_font = Some(9);
    assert_eq!(m.get_cached_string_texture(9, b"x"), Err(StringError::UnsupportedNullFont));
    assert_eq!(m.reference_count(), 1);
    m.services.width = 0x4000_0000;
    assert_eq!(
        m.get_cached_string_texture(1, b"x"),
        Err(StringError::UnsupportedNonterminatingPower)
    );
    assert_eq!(m.reference_count(), 1);
    m.services.width = 61;
    let s = m.get_cached_string_texture(1, b"x").unwrap();
    m.services.failed_text.push(b"x".to_vec());
    assert_eq!(m.string_texture(s), 0);
    assert_eq!(m.string_texture(s), 0);
    assert_eq!((m.cached_entries(), m.cached_bytes(), m.cache_misses()), (0, 0, 2));
    assert_eq!(m.string(s).unwrap().reference_count(), 1);
    m.services.failed_text.clear();
    assert_eq!(m.string_texture(s), 1);
    assert_eq!(m.cache_misses(), 3);
}

#[test]
fn published_sweep_target_vectors_distinguish_rounding_and_ratio() {
    // Fixed independent expected values, not recomputed with the owner formula.
    for (bytes, budget, target) in [
        (1_200_000, 1_048_576, 1_095_142),
        (1_100_000, 1_048_576, 1_048_576),
        (110, 100, 100),
        (111, 100, 101),
        (1_153_435, 1_048_576, 1_048_577),
        (12, 11, 11),
        (13, 11, 11),
    ] {
        assert_eq!(sweep_target(bytes, budget), target);
    }
    // GT3 literal IEEE zero-budget extension: no invented minimum budget.
    assert_eq!(sweep_target(5, 0), 5);
}

fn over_budget_pair() -> (GlyphTextureManager<Services>, StringId, StringId) {
    let mut m = manager();
    m.services.width = 510;
    m.services.height = 510;
    let a = m.get_cached_string_texture(1, b"a").unwrap();
    let b = m.get_cached_string_texture(1, b"b").unwrap();
    m.string_texture(a);
    m.string_texture(b);
    assert_eq!(m.cached_bytes(), 2_097_152);
    (m, a, b)
}

#[test]
fn published_second_chance_oldest_first_and_strict_sweep_rate() {
    let (mut m, a, b) = over_budget_pair();
    for t in [0, 30, 50] {
        m.on_tick(t);
    }
    assert_eq!(m.last_sweep(), 0);
    assert_eq!(m.cache_order().collect::<Vec<_>>(), [(a, true), (b, true)]);
    m.on_tick(60);
    assert_eq!(m.last_sweep(), 60);
    assert_eq!(m.cache_order().collect::<Vec<_>>(), [(a, false), (b, false)]);
    assert_eq!(m.cached_bytes(), 2_097_152);
    m.on_tick(110);
    assert_eq!(m.cached_entries(), 2);
    m.on_tick(111);
    assert_eq!(m.cache_order().collect::<Vec<_>>(), [(b, false)]);
    assert_eq!(m.cached_bytes(), 1_048_576);
    assert_eq!(m.string(a).unwrap().reference_count(), 1);
    assert!(log(&m).contains(&"binding-release:1".to_string()));
    m.on_tick(200);
    assert_eq!(m.cached_entries(), 1); // At budget: don't clear flags or evict.
}

#[test]
fn cache_hit_moves_newest_and_resets_used_without_changing_bytes() {
    let (mut m, a, b) = over_budget_pair();
    m.on_tick(60);
    assert_eq!(m.string_texture(a), 1);
    assert_eq!(m.cache_order().collect::<Vec<_>>(), [(b, false), (a, true)]);
    assert_eq!((m.cached_bytes(), m.cache_misses()), (2_097_152, 2));
    m.on_tick(120);
    assert_eq!(m.cache_order().collect::<Vec<_>>(), [(a, true)]);
    assert_eq!(m.string(b).unwrap().reference_count(), 1);
}

#[test]
fn rollback_resets_time_before_delta_and_unsigned_time_wrap() {
    let mut m = manager();
    m.on_tick(100);
    assert_eq!(m.last_sweep(), 100);
    m.on_tick(40);
    assert_eq!(m.last_sweep(), 40);
    m.on_tick(90);
    assert_eq!(m.last_sweep(), 40);
    m.on_tick(91);
    assert_eq!(m.last_sweep(), 91);
    m.on_tick(u32::MAX);
    m.on_tick(0);
    assert_eq!(m.last_sweep(), 0);
}

#[test]
fn restore_clears_cache_then_releases_glyphs_in_key_order_before_rendering() {
    let mut m = manager();
    let z = m.get_glyph_texture(key(b"Z", 7)).unwrap();
    let a = m.get_glyph_texture(key(b"A", 7)).unwrap();
    let s = m.get_cached_string_texture(7, b"s").unwrap();
    m.string_texture(s);
    m.release_string(s); // Cache now owns the last string reference.
    m.services.log.lock().unwrap().clear();
    assert!(m.do_message(RESTORE_TEXTURES));
    assert_eq!((m.cached_entries(), m.cached_bytes()), (0, 0));
    assert!(m.string(s).is_none());
    let events = log(&m);
    assert_eq!(
        &events[..3],
        ["binding-release:3", "font-release:7", "binding-release:2"]
    );
    assert_eq!(
        &events[3..5],
        ["font-lookup:7", "render:7:[65]:Some([0.0, 0.0, 1.0, 1.0])"]
    );
    assert_eq!(m.glyph(a).unwrap().binding().unwrap().first_texture(), 4);
    assert_eq!(m.glyph(z).unwrap().binding().unwrap().first_texture(), 5);
    let serial = m.services.serial;
    assert!(m.do_message(0));
    assert_eq!(m.services.serial, serial);
}

#[test]
fn restore_no_factory_preserves_glyph_binding_but_drops_strings() {
    let mut m = manager();
    let g = m.get_glyph_texture(key(b"A", 7)).unwrap();
    let s = m.get_cached_string_texture(7, b"s").unwrap();
    m.string_texture(s);
    m.services.factory = false;
    m.do_message(RESTORE_TEXTURES);
    assert_eq!(m.glyph(g).unwrap().binding().unwrap().first_texture(), 1);
    assert_eq!(m.cached_entries(), 0);
    assert_eq!(m.string_texture(s), 3);
}

#[test]
fn restore_failure_releases_old_binding_and_preserves_literal_metadata_gt5() {
    let mut m = manager();
    let g = m.get_glyph_texture(key(b"A", 7)).unwrap();
    let uv = m.glyph(g).unwrap().uv();
    m.services.failed_text.push(b"A".to_vec());
    m.do_message(RESTORE_TEXTURES);
    assert!(m.glyph(g).unwrap().binding().is_none());
    assert_eq!(m.glyph(g).unwrap().uv(), uv);
    m.services.failed_text.clear();
    m.services.texture_size = [128, 32];
    m.do_message(RESTORE_TEXTURES);
    assert!(m.glyph(g).unwrap().binding().is_some());
    assert_eq!(m.glyph(g).unwrap().texture_size(), Some([64, 16]));
    assert_eq!(m.glyph(g).unwrap().uv(), uv);
}

#[test]
fn string_no_manager_count_one_quirk_and_normal_lifetimes() {
    let mut m = manager();
    let s = m.get_cached_string_texture(7, b"unrendered").unwrap();
    assert_eq!(m.add_ref_string(s), Some(2));
    assert_eq!(m.release_string(s), Some(0));
    assert_eq!(m.release_string(s), Some(0));
    assert_eq!(m.reference_count(), 1);
    let s = m.get_cached_string_texture(7, b"detached state").unwrap();
    m.add_ref_string(s);
    // Explicitly exercise §3's specified manager-less state, not a claimed
    // native public setter. Balance the manager link before detaching.
    m.strings.get_mut(&s).unwrap().manager = None;
    m.release();
    assert_eq!(m.release_string(s), Some(0));
    assert!(m.string(s).is_none());
    assert_eq!(m.release_string(s), None);
}

#[test]
fn final_manager_reference_released_by_glyph_destruction() {
    let mut m = manager();
    let g = m.get_glyph_texture(key(b"A", 7)).unwrap();
    assert_eq!(m.release(), Some(1));
    assert!(!m.deleted());
    assert_eq!(m.release_glyph(g), Some(0));
    assert!(m.deleted());
    assert!(!m.query_interface(IID_GLYPH_TEXTURE_MANAGER));
    assert_eq!(m.release(), None);
    assert_eq!(m.get_cached_string_texture(7, b"x"), Err(StringError::DeletedManager));
    let events = log(&m);
    assert!(!events.iter().any(|s| s.starts_with("unsubscribe:")));
    assert!(events.iter().any(|s| s == "binding-release:1"));
}

#[test]
fn getters_are_send_sync_when_the_foreign_primitives_are() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<GlyphTextureManager<Services>>();
    send_sync::<GlyphTextureInfo<Binding>>();
    send_sync::<StringTextureInfo<Font>>();
}

#[test]
fn misses_increment_process_counter_for_success_and_failure_only() {
    let mut m = manager();
    let a = m.get_cached_string_texture(1, b"a").unwrap();
    let b = m.get_cached_string_texture(1, b"b").unwrap();
    m.services.failed_text.push(b"b".to_vec());
    let before = global_cache_misses();
    assert_eq!(m.string_texture(a), 1);
    assert_eq!(m.string_texture(a), 1);
    assert_eq!(m.string_texture(b), 0);
    assert_eq!(m.cache_misses(), 2);
    // Other tests may increment the process counter concurrently; this owner's
    // exact contribution is two, and the global value includes at least that.
    assert!(global_cache_misses() >= before + 2);
}

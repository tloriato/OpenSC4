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

    fn render_text(
        &mut self,
        font: Option<&Font>,
        text: &[u8],
        rect: Option<[f32; 4]>,
        binding: &mut Option<Binding>,
    ) -> RenderOutputWords {
        record(
            &self.log,
            format!("render:{}:{text:?}:{rect:?}", font.map_or(0, |f| f.id)),
        );
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
    m.services.log.lock().unwrap().clear();
    m.string_to_glyph_texture_array(7, b"A\x81\x40!A", &mut output).unwrap();
    assert!(m.glyph(first).is_none());
    assert_eq!(&log(&m)[..2], ["binding-release:1", "font-lookup:7"]);
    assert_eq!(output.len(), 3);
    assert_eq!(output[0], output[2]);
    assert_eq!(m.glyph(output[1]).unwrap().character(), b"\x81\x40");
    assert_eq!(m.glyph(output[0]).unwrap().reference_count(), 3);
    m.string_to_glyph_texture_array(7, b"", &mut output).unwrap();
    assert!(output.is_empty());
    m.services.char_length_override = Some(0);
    assert_eq!(
        m.string_to_glyph_texture_array(7, b"A", &mut output),
        Err(ArrayUnsupported::NativeNontermination)
    );
    m.services.char_length_override = Some(8);
    assert_eq!(
        m.string_to_glyph_texture_array(7, b"A", &mut output),
        Err(ArrayUnsupported::NativeInvalidRead)
    );
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
    assert_eq!(padded_dimension(0x8000_0000), Ok(1));
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
    assert_eq!(
        m.get_cached_string_texture(9, b"x"),
        Err(StringError::UnsupportedNullFont)
    );
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

#[test]
fn constructor_inherited_slots_and_wrapping_references() {
    let mut m = GlyphTextureManager::new(Services::default());
    assert_eq!((m.reference_count(), m.last_sweep()), (0, 0));
    assert_eq!(m.budget(), 1_048_576);
    assert!(!m.initialized());
    assert_eq!(m.release(), Some(0xffff_ffff));
    assert!(!m.deleted());
    assert_eq!(m.add_ref(), Some(0));
    assert_eq!(m.service_id(), 0xabeb_5e37);
    assert_eq!((m.priority(), m.second_priority()), (0xffe1_7b80, 0xffe1_7b80));
    assert_eq!(m.running(), 0);
    m.set_service_id(123);
    m.set_running(0x1234_56ab);
    assert_eq!((m.service_id(), m.running()), (123, 0xab));
    assert!(m.service_slot_11(u32::MAX));
    // The auxiliary word is separate from manager references; these are
    // independent receiver-state inputs, not guessed constructor initialization.
    m.auxiliary_count = Some(0);
    assert_eq!(m.auxiliary_release(), Some(0));
    m.auxiliary_count = Some(2);
    assert_eq!(m.auxiliary_release(), Some(1));
    assert_eq!(m.auxiliary_count(), Some(1));
    assert_eq!(m.reference_count(), 0);
    m.add_ref();
    m.init(0x0040_0001);
    assert_eq!(m.budget(), 1024);
}

#[test]
fn info_constructors_qi_untouched_underflow_and_borrowed_binding() {
    let mut m = manager();
    let g = m.create_glyph_info();
    let s = m.create_string_info();
    let glyph = m.glyph(g).unwrap();
    assert_eq!(glyph.character(), b"");
    assert_eq!(
        (glyph.texture_size(), glyph.glyph_size(), glyph.uv()),
        (None, None, None)
    );
    assert!(!glyph.has_manager());
    assert!(glyph.binding().is_none());
    let string = m.string(s).unwrap();
    assert_eq!(string.size(), None);
    assert!(string.font.is_none());
    assert!(!string.has_manager());
    for iid in [0, 1, IID_GLYPH_TEXTURE_MANAGER, u32::MAX] {
        let mut out = 0x1234_5678;
        assert!(!m.glyph(g).unwrap().query_interface(iid, &mut out));
        assert_eq!(out, 0x1234_5678);
        assert!(!m.string(s).unwrap().query_interface(iid, &mut out));
        assert_eq!(out, 0x1234_5678);
    }
    assert_eq!(m.release_glyph(g), Some(u32::MAX));
    assert_eq!(m.release_string(s), Some(u32::MAX));
    assert_eq!(m.add_ref_glyph(g), Some(0));
    assert_eq!(m.add_ref_string(s), Some(0));
    let live = m.get_glyph_texture(key(b"live", 7)).unwrap();
    m.services.log.lock().unwrap().clear();
    let info = m.glyph(live).unwrap();
    assert_eq!(info.binding().unwrap().first_texture(), 1);
    assert_eq!(info.binding().unwrap().first_texture(), 1);
    assert!(log(&m).is_empty());
    assert_eq!(info.reference_count(), 2);
}

#[test]
fn unsigned_font_order_unique_insertion_and_absent_identity_removal() {
    let mut m = manager();
    let high = m.get_glyph_texture(key(b"A", 0x8000_0000)).unwrap();
    let low = m.get_glyph_texture(key(b"A", 0x7fff_ffff)).unwrap();
    assert_eq!(m.glyph_map.iter().map(|(_, id)| *id).collect::<Vec<_>>(), [low, high]);
    let other = m.create_glyph_info();
    let refs = m.glyph(low).unwrap().reference_count();
    assert_eq!(m.insert_unique(key(b"A", 0x7fff_ffff), other), (low, 0));
    assert_eq!(m.glyph_map.len(), 2);
    assert_eq!(m.tree_count, 2);
    assert_eq!(m.glyph(low).unwrap().reference_count(), refs);
    assert_eq!(m.glyph(other).unwrap().reference_count(), 0);
    m.remove_glyph(other);
    assert_eq!(m.glyph(other).unwrap().reference_count(), 0);
    assert_eq!(m.insert_unique(key(b"B", 7), other), (other, 1));
    assert_eq!(m.tree_count, 3);
    assert_eq!(m.glyph(other).unwrap().reference_count(), 0);
    // Remove scans values, even when a glyph's own key differs from map key.
    m.add_ref_glyph(other);
    m.remove_glyph(other);
    assert!(m.glyph(other).is_none());
    assert_eq!(m.glyph_map.len(), 2);
    assert_eq!(m.tree_count, 2);
}

#[test]
fn failed_glyph_lookup_and_render_preserve_prefilled_output() {
    let mut m = manager();
    let first = m.get_glyph_texture(key(b"old", 7)).unwrap();
    let mut out = Some(first);
    m.services.missing_font = Some(8);
    assert!(!m.get_glyph_texture_out(key(b"missing", 8), &mut out));
    assert_eq!(out, Some(first));
    m.services.failed_text.push(b"bad".to_vec());
    m.services.write_on_failure = true;
    assert!(!m.get_glyph_texture_out(key(b"bad", 7), &mut out));
    assert_eq!(out, Some(first));
    assert_eq!(m.glyph_map.len(), 1);
    assert_eq!(m.reference_count(), 2);
    assert!(m.get_glyph_texture_out(key(b"old", 7), &mut out));
    assert_eq!(m.glyph(first).unwrap().reference_count(), 3);
}

#[test]
fn signed_original_renderer_dimensions_raw_uv_order_and_extended_subtraction() {
    let mut m = manager();
    m.services.texture_size = [0xffff_0000, 16];
    m.services.extents = [16_777_216.0, 7.5, -1.0, -2.25];
    let g = m.get_glyph_texture(key(b"x87", 7)).unwrap();
    let info = m.glyph(g).unwrap();
    assert_eq!(info.texture_size(), Some([0, 16]));
    assert_eq!(info.glyph_size(), Some([1, 9]));
    assert_eq!(info.uv(), Some([0.0000152587890625, -0.140625, -256.0, 0.46875]));
}

#[test]
fn restore_keeps_failure_written_binding_and_all_metadata_even_without_font() {
    let mut m = manager();
    let g = m.get_glyph_texture(key(b"A", 7)).unwrap();
    let before = m.glyph(g).unwrap();
    let metadata = (before.texture_size(), before.glyph_size(), before.uv());
    m.services.failed_text.push(b"A".to_vec());
    m.services.write_on_failure = true;
    m.services.missing_font = Some(7);
    m.services.texture_size = [123, 456];
    m.services.extents = [9.0, 8.0, 7.0, 6.0];
    m.services.log.lock().unwrap().clear();
    m.do_message(RESTORE_TEXTURES);
    let info = m.glyph(g).unwrap();
    assert_eq!(info.binding().unwrap().first_texture(), 2);
    assert_eq!((info.texture_size(), info.glyph_size(), info.uv()), metadata);
    assert_eq!(
        log(&m),
        [
            "binding-release:1",
            "font-lookup:7",
            "render:0:[65]:Some([0.0, 0.0, 1.0, 1.0])"
        ]
    );
}

#[test]
fn destruction_is_storage_only_not_shutdown_and_flags_only_test_bit_zero() {
    for flags in [0, 1, 2, 3, 0xffff_fffe] {
        let mut m = manager();
        let g = m.get_glyph_texture(key(b"A", 7)).unwrap();
        let s = m.get_cached_string_texture(7, b"cache").unwrap();
        m.string_texture(s);
        let refs = m.reference_count();
        m.services.log.lock().unwrap().clear();
        let address = &mut m as *mut _;
        assert_eq!(m.deleting_destructor(flags), address);
        assert!(m.deleted());
        assert_eq!(m.allocation_freed, flags & 1 != 0);
        assert!(m.initialized()); // No Shutdown mutation or unsubscription.
        assert!(m.glyph_map.is_empty());
        assert!(m.cache.is_empty());
        assert_eq!(m.glyph(g).unwrap().reference_count(), 2);
        assert!(m.glyph(g).unwrap().has_manager());
        assert_eq!(m.string(s).unwrap().reference_count(), 2);
        assert_eq!(m.reference_count(), refs);
        assert!(log(&m).is_empty()); // No binding/font/manager Release.
        assert_eq!(m.unreleased_bindings.len(), 1);
    }
}

#[test]
fn set_manager_acquire_store_release_equal_noop_and_info_destructor_order() {
    let mut m = manager();
    let g = m.get_glyph_texture(key(b"A", 7)).unwrap();
    m.set_glyph_manager(g, Some(ManagerLink::Foreign(10)));
    m.services.log.lock().unwrap().clear();
    m.set_glyph_manager(g, Some(ManagerLink::Foreign(20)));
    assert_eq!(log(&m), ["manager-addref:20", "manager-release:10"]);
    assert_eq!(m.glyph(g).unwrap().manager, Some(ManagerLink::Foreign(20)));
    m.set_glyph_manager(g, Some(ManagerLink::Foreign(20)));
    assert_eq!(log(&m).len(), 2);
    m.services.log.lock().unwrap().clear();
    assert_eq!(m.destroy_glyph(g, 2), Some(g));
    assert_eq!(log(&m), ["manager-release:20", "binding-release:1"]);
    assert!(m.glyph(g).unwrap().destroyed); // allocation kept, destructor ran.
    assert!(m.glyph(g).unwrap().character().is_empty());
    let s = m.get_cached_string_texture(7, b"text").unwrap();
    m.strings.get_mut(&s).unwrap().manager = Some(ManagerLink::Foreign(30));
    m.release(); // Balance the replaced local link in this constructed state.
    m.services.log.lock().unwrap().clear();
    assert_eq!(m.destroy_string(s, 1), Some(s));
    assert!(m.string(s).is_none());
    assert_eq!(log(&m), ["font-release:7", "manager-release:30"]);
}

#[test]
fn signed_float32_sweep_and_zero_budget_no_eviction_use_stored_dimensions() {
    assert_eq!(sweep_target(1_100_000_001, 1_000_000_000), 1_000_000_000);
    assert_eq!(sweep_target(1_200_000_000, 1_000_000_000), 1_099_999_998);
    assert_eq!(sweep_target(16_777_217, 0), 16_777_216); // B is stored as f32 first.
    assert_eq!(sweep_target(100, 0), 100);
    let mut m = manager();
    m.services.width = 6;
    m.services.height = 6;
    m.services.texture_size = [999, 777];
    let s = m.get_cached_string_texture(7, b"cost").unwrap();
    m.string_texture(s);
    assert_eq!(m.cached_bytes(), 256); // stored 8*8*4, not renderer dimensions.
    m.cache[0].bytes = 100;
    m.bytes = 100;
    m.budget = 0;
    m.cache[0].used = false;
    m.on_tick(51);
    assert_eq!(m.cached_entries(), 1);
    assert_eq!(m.cached_bytes(), 100);
    m.bytes = 0x8000_0000;
    m.budget = 1;
    m.on_tick(102);
    assert_eq!(m.cached_entries(), 1); // signed B is below L.
}

#[test]
fn uv_stores_round_once_ties_even_and_preserve_signs_and_subnormals() {
    for (input, dimension, expected) in [
        (0x3f80_0000, 3, 0x3eaa_aaab),
        (0x3f80_0000, 0xffff_fffd, 0xbeaa_aaab),
        (0x0000_0001, 2, 0x0000_0000),
        (0x0000_0003, 2, 0x0000_0002),
        (0x0080_0000, 3, 0x002a_aaab),
        (0x8000_0000, 3, 0x8000_0000),
        (0x0000_0000, 0xffff_ffff, 0x8000_0000),
        (0x3f80_0000, 0, 0x7f80_0000),
    ] {
        assert_eq!(stored_uv(f32::from_bits(input), dimension).to_bits(), expected);
    }
}

#[test]
fn unsupported_string_slot_never_uses_a_standalone_renderer() {
    let mut m = manager();
    let raw = m.create_string_info();
    assert_eq!(m.try_string_texture(raw), Err(StringError::UnsupportedManagerLink));
    assert_eq!(m.services.serial, 0);
    assert_eq!(m.string(raw).unwrap().size(), None);
}

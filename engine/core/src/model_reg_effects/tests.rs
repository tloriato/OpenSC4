// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent expectations from graphics-formats §3.11, not captured outputs.
use super::*;
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq)]
enum Event {
    Acquire(u32),
    InstanceRelease(u32),
    StorageDrop(u32),
    Create(String),
    Started(usize),
    Start(usize, u32),
    Stop(usize, u32),
    Release(usize),
    WrapperDrop(usize),
    InstanceTransform,
    AnimationQuery,
    Frame,
    AnimationRelease,
    Transform(usize, Transform),
    Parameter(usize, u32, Vector3, u32),
}
type Log = Arc<Mutex<Vec<Event>>>;
fn push(log: &Log, event: Event) {
    log.lock().unwrap().push(event);
}

struct Effect {
    id: usize,
    log: Log,
    state: Arc<Mutex<bool>>,
}
impl VisualEffect for Effect {
    fn is_started(&self) -> bool {
        push(&self.log, Event::Started(self.id));
        *self.state.lock().unwrap()
    }
    fn start(&mut self, control: u32) {
        push(&self.log, Event::Start(self.id, control));
        *self.state.lock().unwrap() = true;
    }
    fn stop(&mut self, control: u32) {
        push(&self.log, Event::Stop(self.id, control));
        // Published deferred stop remains started; there is no owner mirror.
        if control != 0 {
            *self.state.lock().unwrap() = false;
        }
    }
    fn set_transform(&mut self, transform: Transform) {
        push(&self.log, Event::Transform(self.id, transform));
    }
    fn set_parameter(&mut self, id: u32, value: Vector3, count: u32) {
        push(&self.log, Event::Parameter(self.id, id, value, count));
    }
    fn release(self: Box<Self>) {
        push(&self.log, Event::Release(self.id));
    }
}
impl Drop for Effect {
    fn drop(&mut self) {
        push(&self.log, Event::WrapperDrop(self.id));
    }
}

struct Factory {
    log: Log,
    states: Vec<Arc<Mutex<bool>>>,
    results: VecDeque<(bool, bool)>, // success and nonnull output, separately
    available: bool,
}
impl Factory {
    fn new() -> Self {
        Self {
            log: Arc::default(),
            states: vec![],
            results: VecDeque::new(),
            available: true,
        }
    }
    fn take(&self) -> Vec<Event> {
        std::mem::take(&mut *self.log.lock().unwrap())
    }
    fn set_started(&self, id: usize, value: bool) {
        *self.states[id].lock().unwrap() = value;
    }
}
impl EffectFactory for Factory {
    fn available(&self) -> bool {
        self.available
    }
    fn create(&mut self, name: &str) -> Creation {
        push(&self.log, Event::Create(name.to_owned()));
        let (success, nonnull) = self.results.pop_front().unwrap_or((true, true));
        let effect = nonnull.then(|| {
            let state = Arc::new(Mutex::new(false));
            let id = self.states.len();
            self.states.push(Arc::clone(&state));
            Box::new(Effect {
                id,
                log: Arc::clone(&self.log),
                state,
            }) as Box<dyn VisualEffect>
        });
        Creation { success, effect }
    }
}

struct Instance {
    id: u32,
    regions: Option<Regions>,
    transform: Mutex<Transform>,
    frame: Mutex<Option<u16>>,
    log: Log,
}
struct Frame {
    value: u16,
    log: Log,
}
impl Animation for Frame {
    fn current_frame(&self) -> u16 {
        push(&self.log, Event::Frame);
        self.value
    }
    fn release(self: Box<Self>) {
        push(&self.log, Event::AnimationRelease);
    }
}
impl ModelInstance for Instance {
    fn acquire(&self) {
        push(&self.log, Event::Acquire(self.id));
    }
    fn release(&self) {
        push(&self.log, Event::InstanceRelease(self.id));
    }
    fn regions(&self) -> Option<&Regions> {
        self.regions.as_ref()
    }
    fn transform(&self) -> Transform {
        push(&self.log, Event::InstanceTransform);
        *self.transform.lock().unwrap()
    }
    fn acquire_animation(&self) -> Option<Box<dyn Animation>> {
        push(&self.log, Event::AnimationQuery);
        self.frame.lock().unwrap().map(|value| {
            Box::new(Frame {
                value,
                log: Arc::clone(&self.log),
            }) as Box<dyn Animation>
        })
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        push(&self.log, Event::StorageDrop(self.id));
    }
}

const RECORD: [f32; 7] = [1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0];
fn instance(id: u32, regions: Option<Regions>, log: &Log) -> Arc<Instance> {
    Arc::new(Instance {
        id,
        regions,
        transform: Mutex::new(Transform {
            flags: 0x80,
            version: 7,
            translation: [100.0, 0.0, 200.0],
            ..Transform::IDENTITY
        }),
        frame: Mutex::new(None),
        log: Arc::clone(log),
    })
}
fn regions(keys: &[&str]) -> Option<Regions> {
    Some(keys.iter().map(|&key| (key.to_owned(), vec![RECORD])).collect())
}
fn records(values: Vec<[f32; 7]>) -> Option<RecordVector> {
    Some(RecordVector::shared(Arc::new(values)))
}
fn add(owner: &mut ModelRegEffects, factory: &mut Factory, key: &str, name: &str) {
    owner.add_effect(key, name, None, factory);
}
fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}
fn rotation(actual: Rotation, expected: Rotation) {
    for (a, e) in actual.into_iter().flatten().zip(expected.into_iter().flatten()) {
        close(a, e);
    }
}

#[test]
fn vector48_names_and_literal_prefix() {
    for (key, expected) in [
        ("fx_00_planeprop", Some("planeprop")),
        ("fx_00_rotor", Some("rotor")),
        (
            "fx_straight_road_bridge_collumn_foam",
            Some("straight_road_bridge_collumn_foam"),
        ),
        ("fx_7", Some("7")),
        ("fx_1a_b_c", Some("b_c")),
        ("regpoint01", None),
        ("stop_car_x", None),
        ("fxa", None),
        ("fx_", Some("")),
        ("fx_0_", Some("")),
        ("fx_a_b", Some("a_b")),
        ("fx_\u{0661}_b", Some("\u{0661}_b")),
        ("FX_00_rotor", None),
    ] {
        assert_eq!(effect_name(key), expected);
    }
}

#[test]
fn attachment_same_null_and_explicit_init_shutdown_reference_order() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    owner.attach(None);
    assert!(owner.init(&mut factory));
    assert!(factory.take().is_empty());
    let first = instance(1, regions(&["fx_a"]), &factory.log);
    owner.attach(Some(first.clone()));
    assert_eq!(factory.take(), [Event::Acquire(1)]);
    owner.init(&mut factory);
    assert_eq!(factory.take(), [Event::Create("a".into())]);
    let effect = owner.get_effect("fx_a").unwrap() as *const dyn VisualEffect as *const ();
    assert!(owner.instance().is_some());
    owner.attach(Some(first.clone()));
    assert_eq!(
        effect,
        owner.get_effect("fx_a").unwrap() as *const dyn VisualEffect as *const ()
    );
    assert!(factory.take().is_empty());
    drop(first);
    owner.attach(Some(instance(2, regions(&["fx_b"]), &factory.log)));
    assert_eq!(
        factory.take(),
        [
            Event::Stop(0, 0),
            Event::Release(0),
            Event::WrapperDrop(0),
            Event::InstanceRelease(1),
            Event::StorageDrop(1),
            Event::Acquire(2)
        ]
    );
    assert_eq!(owner.entries().len(), 0);
    owner.init(&mut factory);
    factory.take();
    assert!(owner.shutdown());
    assert_eq!(
        factory.take(),
        [
            Event::Stop(1, 0),
            Event::Release(1),
            Event::WrapperDrop(1),
            Event::InstanceRelease(2),
            Event::StorageDrop(2)
        ]
    );
    assert!(owner.shutdown());
    assert!(factory.take().is_empty());
}

#[test]
fn clear_processes_all_effects_before_record_storage_and_keeps_instance() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    owner.attach(Some(instance(1, None, &factory.log)));
    let storage = instance(9, regions(&["r"]), &factory.log);
    owner.add_effect(
        "b",
        "b",
        Some(RecordVector(RecordStorage::Region(storage.clone(), "r".into()))),
        &mut factory,
    );
    owner.add_effect(
        "a",
        "a",
        Some(RecordVector(RecordStorage::Region(storage, "r".into()))),
        &mut factory,
    );
    factory.take();
    owner.clear();
    assert_eq!(
        factory.take(),
        [
            Event::Stop(1, 0),
            Event::Release(1),
            Event::WrapperDrop(1),
            Event::Stop(0, 0),
            Event::Release(0),
            Event::WrapperDrop(0),
            Event::StorageDrop(9)
        ]
    );
    assert!(owner.instance().is_some());
    drop(owner);
    assert_eq!(factory.take(), [Event::InstanceRelease(1), Event::StorageDrop(1)]);
}

#[test]
fn null_to_null_preserves_manual_entries_but_new_and_null_attachments_shutdown() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    add(&mut owner, &mut factory, "manual", "smoke");
    factory.take();
    owner.attach(None);
    assert!(factory.take().is_empty());
    assert!(owner.get_effect("manual").is_some());
    owner.attach(Some(instance(1, regions(&["fx_a"]), &factory.log)));
    assert_eq!(
        factory.take(),
        [
            Event::Stop(0, 0),
            Event::Release(0),
            Event::WrapperDrop(0),
            Event::Acquire(1)
        ]
    );
    owner.init(&mut factory);
    assert_eq!(factory.take(), [Event::Create("a".into())]);
    owner.attach(None);
    assert_eq!(
        factory.take(),
        [
            Event::Stop(1, 0),
            Event::Release(1),
            Event::WrapperDrop(1),
            Event::InstanceRelease(1),
            Event::StorageDrop(1)
        ]
    );
    assert!(owner.instance().is_none());
    assert_eq!(owner.entries().len(), 0);
}

#[test]
fn destruction_has_no_effect_stop_or_release_and_ends_storage_before_instance() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    owner.attach(Some(instance(1, regions(&["fx_a"]), &factory.log)));
    owner.init(&mut factory);
    factory.take();
    drop(owner);
    assert_eq!(
        factory.take(),
        [Event::WrapperDrop(0), Event::InstanceRelease(1), Event::StorageDrop(1)]
    );
}

#[test]
fn init_discovers_key_byte_order_replaces_and_continues_after_failure() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    owner.attach(Some(instance(
        1,
        regions(&["fx_b", "fx_a", "regpoint", "fx_c"]),
        &factory.log,
    )));
    add(&mut owner, &mut factory, "manual", "other");
    factory.take();
    factory.results.extend([(false, false), (true, true), (true, true)]);
    owner.init(&mut factory);
    assert_eq!(
        factory.take(),
        [
            Event::Create("a".into()),
            Event::Create("b".into()),
            Event::Create("c".into())
        ]
    );
    assert_eq!(
        owner.entries().map(EffectEntry::key).collect::<Vec<_>>(),
        ["fx_b", "fx_c", "manual"]
    );
    owner.init(&mut factory);
    assert_eq!(
        factory.take(),
        [
            Event::Create("a".into()),
            Event::Stop(1, 1),
            Event::Release(1),
            Event::WrapperDrop(1),
            Event::Create("b".into()),
            Event::Stop(2, 1),
            Event::Release(2),
            Event::WrapperDrop(2),
            Event::Create("c".into())
        ]
    );
    assert!(owner.get_effect("manual").is_some());
    assert!(owner.entries().find(|e| e.key() == "fx_a").unwrap().records().is_some());
}

#[test]
fn all_controls_use_primitive_state_full_words_and_deferred_stops() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    add(&mut owner, &mut factory, "b", "b");
    add(&mut owner, &mut factory, "a", "a");
    factory.set_started(0, true);
    factory.take();
    owner.start_all(2);
    owner.start_all(0x8000_0002);
    owner.stop_all(0);
    owner.stop_all(0);
    owner.stop_all(0xffff_0002);
    owner.stop_all(2);
    assert_eq!(
        factory.take(),
        [
            Event::Started(1),
            Event::Start(1, 2),
            Event::Started(0),
            Event::Started(1),
            Event::Started(0),
            Event::Started(1),
            Event::Stop(1, 0),
            Event::Started(0),
            Event::Stop(0, 0),
            Event::Started(1),
            Event::Stop(1, 0),
            Event::Started(0),
            Event::Stop(0, 0),
            Event::Started(1),
            Event::Stop(1, 0xffff_0002),
            Event::Started(0),
            Event::Stop(0, 0xffff_0002),
            Event::Started(1),
            Event::Started(0),
        ]
    );
    owner.start_all(0x8000_0002);
    assert_eq!(
        factory.take(),
        [
            Event::Started(1),
            Event::Start(1, 0x8000_0002),
            Event::Started(0),
            Event::Start(0, 0x8000_0002)
        ]
    );
}

#[test]
fn named_controls_repeat_starts_stop_started_only_and_distinguish_key_from_name() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let locale = CrtLocale::default();
    add(&mut owner, &mut factory, "Rotor", "unrelated");
    add(&mut owner, &mut factory, "fx_a", "rotor");
    add(&mut owner, &mut factory, "fx_b", "ROTOR");
    assert!(owner.get_effect("Rotor").is_some());
    assert!(owner.get_effect("rotor").is_none());
    factory.take();
    assert!(owner.start_named("RoToR", 2, 0, &locale, &mut factory));
    assert!(owner.start_named("RoToR", 0x8000_0000, 1, &locale, &mut factory));
    owner.stop_named("ROTOR", 2, &locale);
    owner.stop_named("ROTOR", 2, &locale);
    assert_eq!(
        factory.take(),
        [
            Event::Start(1, 2),
            Event::Start(2, 2),
            Event::Start(1, 0x8000_0000),
            Event::Start(2, 0x8000_0000),
            Event::Started(0),
            Event::Started(1),
            Event::Stop(1, 2),
            Event::Started(2),
            Event::Stop(2, 2),
            Event::Started(0),
            Event::Started(1),
            Event::Started(2)
        ]
    );
}

#[test]
fn crt_locale_byte_table_and_c_string_boundary_are_explicit() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    add(&mut owner, &mut factory, "a", "Ä");
    factory.take();
    assert!(!owner.start_named("ä", 1, 0, &CrtLocale::default(), &mut factory));
    let mut lower = std::array::from_fn(|b| (b as u8).to_ascii_lowercase());
    lower[0x84] = 0xa4; // explicit TEST table, not an asserted native locale
    let locale = CrtLocale::new(lower);
    assert!(owner.start_named("ä\0ignored", 2, 0, &locale, &mut factory));
    assert_eq!(factory.take(), [Event::Start(0, 2)]);
    add(&mut owner, &mut factory, "ä", "x");
    add(&mut owner, &mut factory, "z", "x");
    assert_eq!(
        owner.entries().map(EffectEntry::key).collect::<Vec<_>>(),
        ["a", "z", "ä"]
    );
}

#[test]
fn low_byte_create_missing_region_and_missing_model_create_null_records() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let locale = CrtLocale::default();
    assert!(!owner.start_named("new", 2, 0x100, &locale, &mut factory));
    assert!(factory.take().is_empty());
    assert!(owner.start_named("new", 0xdead_beef, 0x101, &locale, &mut factory));
    assert_eq!(
        factory.take(),
        [Event::Create("new".into()), Event::Start(0, 0xdead_beef)]
    );
    assert_eq!(owner.entries().next().unwrap().records(), None);
    owner.attach(Some(instance(1, None, &factory.log)));
    factory.take();
    assert!(owner.add_effect_at_region("absent", "smoke", &mut factory).is_some());
    assert_eq!(factory.take(), [Event::Create("smoke".into())]);
    assert!(owner.entries().next().unwrap().records().is_none());
}

#[test]
fn replacement_failure_removes_and_remove_uses_stop_one_even_when_stopped() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    add(&mut owner, &mut factory, "key", "old");
    factory.take();
    factory.results.push_back((false, false));
    assert!(owner.add_effect("key", "new", None, &mut factory).is_none());
    assert_eq!(
        factory.take(),
        [
            Event::Stop(0, 1),
            Event::Release(0),
            Event::WrapperDrop(0),
            Event::Create("new".into())
        ]
    );
    assert_eq!(owner.entries().len(), 0);
    add(&mut owner, &mut factory, "key", "new");
    factory.take();
    owner.remove("KEY");
    assert!(factory.take().is_empty());
    owner.remove("key");
    owner.remove("key");
    assert_eq!(
        factory.take(),
        [Event::Stop(1, 1), Event::Release(1), Event::WrapperDrop(1)]
    );
}

#[test]
fn toggle_observes_matching_states_and_only_missing_falls_back() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let locale = CrtLocale::default();
    add(&mut owner, &mut factory, "a", "smoke");
    add(&mut owner, &mut factory, "b", "SMOKE");
    add(&mut owner, &mut factory, "c", "other");
    factory.set_started(0, true);
    factory.take();
    owner.toggle_named("Smoke", 2, 1, &locale, &mut factory);
    assert_eq!(
        factory.take(),
        [
            Event::Started(0),
            Event::Stop(0, 2),
            Event::Started(1),
            Event::Start(1, 2)
        ]
    );
    owner.toggle_named("missing", 0x8000_0002, 0x100, &locale, &mut factory);
    assert!(factory.take().is_empty());
    owner.toggle_named("missing", 0x8000_0002, 0x101, &locale, &mut factory);
    assert_eq!(
        factory.take(),
        [Event::Create("missing".into()), Event::Start(3, 0x8000_0002)]
    );
}

#[test]
fn recreate_release_without_stop_failure_keeps_null_entry_and_continues() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let locale = CrtLocale::default();
    owner.add_effect("a", "smoke", records(vec![RECORD]), &mut factory);
    add(&mut owner, &mut factory, "b", "SMOKE");
    factory.take();
    factory.results.extend([(false, false), (true, true)]);
    owner.recreate_named("Smoke", 0x1234_0002, 1, &locale, &mut factory);
    assert_eq!(
        factory.take(),
        [
            Event::Release(0),
            Event::WrapperDrop(0),
            Event::Create("Smoke".into()),
            Event::Release(1),
            Event::WrapperDrop(1),
            Event::Create("Smoke".into()),
            Event::Start(2, 0x1234_0002)
        ]
    );
    let first = owner.entries().next().unwrap();
    assert_eq!(first.key(), "a");
    assert_eq!(first.name(), "smoke");
    assert_eq!(first.records(), Some([RECORD].as_slice()));
    assert!(first.effect().is_none());
    assert_eq!(owner.entries().len(), 2);
    owner.start_all(2);
    assert_eq!(factory.take(), [Event::Started(2)]);
    factory.available = false;
    owner.recreate_named("Smoke", 1, 1, &locale, &mut factory);
    assert!(factory.take().is_empty());
    factory.available = true;
    owner.clear();
    assert_eq!(
        factory.take(),
        [Event::Stop(2, 0), Event::Release(2), Event::WrapperDrop(2)]
    );
}

#[test]
fn recreate_failed_match_suppresses_fallback_but_null_matches_do_not() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let locale = CrtLocale::default();
    add(&mut owner, &mut factory, "a", "smoke");
    factory.take();
    factory.results.push_back((false, false));
    owner.recreate_named("SMOKE", 2, 1, &locale, &mut factory);
    assert_eq!(
        factory.take(),
        [Event::Release(0), Event::WrapperDrop(0), Event::Create("SMOKE".into())]
    );
    owner.recreate_named("SMOKE", 2, 1, &locale, &mut factory);
    assert_eq!(factory.take(), [Event::Create("SMOKE".into()), Event::Start(1, 2)]);
    assert_eq!(owner.entries().len(), 2);
}

#[test]
fn creation_status_and_output_are_separate_no_guessed_reference_cleanup() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    factory.results.extend([(true, false), (false, true)]);
    assert!(owner.add_effect("a", "null-success", None, &mut factory).is_none());
    assert_eq!(owner.entries().len(), 1);
    assert!(owner.add_effect("b", "bad-failure", None, &mut factory).is_none());
    assert_eq!(owner.entries().len(), 1);
    assert_eq!(
        factory.take(),
        [
            Event::Create("null-success".into()),
            Event::Create("bad-failure".into()),
            Event::WrapperDrop(0)
        ]
    );
    owner.clear();
    assert!(factory.take().is_empty());
}

#[test]
fn records_are_shared_not_copied_and_getters_do_not_acquire() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let values = Arc::new(vec![RECORD]);
    let address = values.as_ptr();
    owner.add_effect("key", "name", Some(RecordVector::shared(values.clone())), &mut factory);
    factory.take();
    assert_eq!(owner.entries().next().unwrap().records().unwrap().as_ptr(), address);
    assert!(owner.get_effect("key").is_some());
    assert!(factory.take().is_empty());
    assert_eq!(Arc::strong_count(&values), 2); // host lease, NOT native AddRef
    owner.clear();
    assert_eq!(Arc::strong_count(&values), 1);
}

#[test]
fn region_records_borrow_original_storage_and_missing_key_creates_anyway() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let model = instance(1, regions(&["r"]), &factory.log);
    let address = model.regions.as_ref().unwrap()["r"].as_ptr();
    owner.attach(Some(model));
    factory.take();
    owner.add_effect_at_region("r", "smoke", &mut factory);
    assert_eq!(factory.take(), [Event::Create("smoke".into())]);
    assert_eq!(owner.entries().next().unwrap().records().unwrap().as_ptr(), address);
    owner.add_effect_at_region("missing", "other", &mut factory);
    assert_eq!(factory.take(), [Event::Create("other".into())]);
    assert!(owner
        .entries()
        .find(|e| e.key() == "missing")
        .unwrap()
        .records()
        .is_none());
}

#[test]
fn replacement_keeps_old_record_lease_until_creation_finishes() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let storage = instance(9, regions(&["r"]), &factory.log);
    owner.add_effect(
        "key",
        "old",
        Some(RecordVector(RecordStorage::Region(storage, "r".into()))),
        &mut factory,
    );
    factory.take();
    factory.results.push_back((false, false));
    owner.add_effect("key", "new", None, &mut factory);
    assert_eq!(
        factory.take(),
        [
            Event::Stop(0, 1),
            Event::Release(0),
            Event::WrapperDrop(0),
            Event::Create("new".into()),
            Event::StorageDrop(9)
        ]
    );
}

#[test]
fn null_record_shortcut_empty_out_of_range_and_animation_lifetime() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let model = instance(1, None, &factory.log);
    *model.frame.lock().unwrap() = Some(1);
    owner.attach(Some(model));
    add(&mut owner, &mut factory, "a-null", "a");
    owner.add_effect("b-empty", "b", records(vec![]), &mut factory);
    owner.add_effect("c-short", "c", records(vec![RECORD]), &mut factory);
    owner.add_effect(
        "d-valid",
        "d",
        records(vec![RECORD, [9.0, 8.0, 7.0, 0.0, 0.0, 1.0, 0.0]]),
        &mut factory,
    );
    add(&mut owner, &mut factory, "e-stopped", "e");
    owner.start_all(2);
    factory.set_started(4, false);
    factory.take();
    let report = owner.update_with_motion([1.0, -2.0, 0.5], 3.0);
    assert!(report.success);
    assert_eq!(report.transformed, 2);
    assert_eq!(
        report.missing_records,
        [
            MissingRecord {
                key: "b-empty".into(),
                frame: 1,
                record_count: 0
            },
            MissingRecord {
                key: "c-short".into(),
                frame: 1,
                record_count: 1
            }
        ]
    );
    let copied = Transform {
        version: 0,
        flags: 0x80,
        translation: [100.0, 0.0, 200.0],
        ..Transform::IDENTITY
    };
    let combined = Transform {
        flags: 0x85,
        translation: [109.0, 8.0, 207.0],
        ..Transform::IDENTITY
    };
    assert_eq!(
        factory.take(),
        [
            Event::InstanceTransform,
            Event::AnimationQuery,
            Event::Frame,
            Event::Started(0),
            Event::Transform(0, copied),
            Event::Parameter(0, 15, [3.0, -6.0, 1.5], 1),
            Event::Started(1),
            Event::Started(2),
            Event::Started(3),
            Event::Transform(3, combined),
            Event::Parameter(3, 15, [3.0, -6.0, 1.5], 1),
            Event::Started(4),
            Event::AnimationRelease
        ]
    );
}

#[test]
fn empty_detached_updates_and_external_state_observations() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    assert_eq!(owner.update(), UpdateReport::default());
    owner.attach(Some(instance(1, None, &factory.log)));
    factory.take();
    assert_eq!(owner.update_with_motion([1.0; 3], 1.0), UpdateReport::default());
    assert!(factory.take().is_empty());
    add(&mut owner, &mut factory, "a", "a");
    factory.take();
    owner.update();
    assert_eq!(
        factory.take(),
        [Event::InstanceTransform, Event::AnimationQuery, Event::Started(0)]
    );
    factory.set_started(0, true); // downstream change without owner Start
    owner.update();
    assert_eq!(
        factory.take(),
        [
            Event::InstanceTransform,
            Event::AnimationQuery,
            Event::Started(0),
            Event::Transform(
                0,
                Transform {
                    version: 0,
                    flags: 0x80,
                    translation: [100.0, 0.0, 200.0],
                    ..Transform::IDENTITY
                }
            )
        ]
    );
}

#[test]
fn owner_zero_instance_flags_shortcut_and_null_effect_update_gate() {
    let mut factory = Factory::new();
    let mut owner = ModelRegEffects::new();
    let model = instance(1, None, &factory.log);
    *model.transform.lock().unwrap() = Transform {
        version: 9,
        translation: [100.0, 0.0, 200.0],
        scale: 99.0,
        ..Transform::IDENTITY
    };
    owner.attach(Some(model));
    owner.add_effect("a-loaded", "a", records(vec![RECORD]), &mut factory);
    add(&mut owner, &mut factory, "b-null-record", "b");
    factory.results.push_back((true, false));
    add(&mut owner, &mut factory, "c-null-effect", "c");
    owner.start_all(2);
    factory.take();
    let report = owner.update();
    assert!(report.success);
    assert_eq!(report.transformed, 2);
    assert_eq!(
        factory.take(),
        [
            Event::InstanceTransform,
            Event::AnimationQuery,
            Event::Started(0),
            Event::Transform(
                0,
                Transform {
                    flags: 5,
                    translation: [1.0, 2.0, 3.0],
                    ..Transform::IDENTITY
                }
            ),
            Event::Started(1),
            Event::Transform(
                1,
                Transform {
                    translation: [100.0, 0.0, 200.0],
                    scale: 99.0,
                    ..Transform::IDENTITY
                }
            )
        ]
    );
}

#[test]
fn flag_shortcuts_ignore_invalid_instance_payload_only_when_flags_zero() {
    let local = Transform::from_record([1.0, 2.0, 3.0, 0.0, 0.0, 1.0, 0.0]);
    assert_eq!(local.flags, 5);
    let unused = Transform {
        version: 9,
        translation: [f32::NAN; 3],
        scale: 99.0,
        ..Transform::IDENTITY
    };
    assert_eq!(unused.compose_record(local), local);
    let instance = Transform {
        flags: 0x40,
        version: 9,
        translation: [7.0, 8.0, 9.0],
        scale: 2.0,
        ..Transform::IDENTITY
    };
    assert_eq!(
        instance.compose_record(Transform::IDENTITY),
        Transform { version: 0, ..instance }
    );
    let world = instance.compose_record(local);
    assert_eq!(world.translation, [9.0, 12.0, 15.0]);
    assert_eq!(world.scale, 2.0);
    assert_eq!(world.flags, 0x45);
    assert_eq!(world.version, 0);
}

#[test]
fn independent_axes_geometry_and_documented_zero_axis_choice() {
    for (axis, expected) in [
        ([1.0, 0.0, 0.0], [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]),
        ([0.0, 1.0, 0.0], [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]),
        ([0.0, 0.0, 7.0], [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]),
    ] {
        rotation(
            Transform::from_record([0.0, 0.0, 0.0, axis[0], axis[1], axis[2], std::f32::consts::FRAC_PI_2]).rotation,
            expected,
        );
    }
    let zero = Transform::from_record(RECORD);
    assert_eq!(zero.rotation, Transform::IDENTITY.rotation);
    assert_eq!(zero.translation, [1.0, 2.0, 3.0]);
    assert_eq!(zero.flags, 5);
}

#[test]
fn axis_normalization_retains_wide_exponent_before_binary32_component_stores() {
    // Exact power-of-two axes have normalized components (1,0,0). Squaring
    // either magnitude in binary32 would overflow/underflow; neither is zero.
    let expected = [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]];
    for axis in [2f32.powi(100), 2f32.powi(-100), f32::from_bits(1)] {
        let transform = Transform::from_record([0.0, 0.0, 0.0, axis, 0.0, 0.0, std::f32::consts::FRAC_PI_2]);
        rotation(transform.rotation, expected);
    }
}

#[test]
fn noncommuting_rotation_scaled_translation_and_effect_placement() {
    let model = Transform {
        flags: 0x80,
        rotation: [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
        translation: [10.0, 20.0, 30.0],
        scale: 2.0,
        ..Transform::IDENTITY
    };
    let local = Transform::from_record([1.0, 2.0, 3.0, 0.0, 0.0, 7.0, std::f32::consts::FRAC_PI_2]);
    let world = model.compose_record(local);
    assert_eq!(world.translation, [12.0, 14.0, 34.0]);
    rotation(world.rotation, [[0.0, -1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]]);
    let placement = Transform {
        rotation: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
        translation: [-5.0, 4.0, 9.0],
        scale: 3.0,
        ..Transform::IDENTITY
    };
    let placed = placement.compose(world);
    assert_eq!(placed.translation, [97.0, 46.0, -27.0]);
    rotation(placed.rotation, [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]);
    assert_eq!(placed.scale, 6.0);
}

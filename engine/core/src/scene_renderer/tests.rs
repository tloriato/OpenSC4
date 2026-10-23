// SPDX-License-Identifier: GPL-3.0-or-later
use super::fixtures as f;
use super::*;
#[derive(Default)]
struct Refs(Vec<(bool, Object)>);
impl References for Refs {
    fn add_ref(&mut self, object: Object) {
        self.0.push((true, object));
    }
    fn release(&mut self, object: Object) {
        self.0.push((false, object));
    }
}
#[test]
fn literal_dirty_fixtures() {
    for f in f::DIRTY {
        let mut s = SceneRenderer::default();
        let mut refs = Refs::default();
        for &(mode, rect) in f.queued {
            s.add_dirty_rectangle_internal(mode, rect, Some(Object(9)), 0, false, &mut refs, None)
                .unwrap();
        }
        s.add_dirty_rectangle_internal(f.incoming.0, f.incoming.1, Some(Object(10)), 0, true, &mut refs, None)
            .unwrap();
        let actual: Vec<_> = s.dirty_nodes().iter().map(|n| (n.mode, n.rect)).collect();
        assert_eq!(actual, f.expected, "{}", f.name);
        assert_eq!(s.dirty_area(), f.area, "{}", f.name);
        if f.name.starts_with("D3") {
            assert_eq!(refs.0, [(true, Object(9)), (false, Object(9))]);
        }
    }
}
#[test]
fn signed_filter_literal_cases() {
    for &(before, accepted, after) in f::FILTER {
        let mut rect = before;
        assert_eq!(filter_rect(&mut rect, 800, 600), accepted);
        assert_eq!(rect, after);
    }
}

#[test]
fn dirty_strict_ties_mode_zero_penalty_deadline_and_threshold() {
    let mut s = SceneRenderer::default();
    let mut refs = Refs::default();
    s.set_dirty_limits(i32::MAX, 100, 37).unwrap();
    for object in [Object(1), Object(2)] {
        s.add_dirty_rectangle_internal(0, [0, 0, 10, 10], Some(object), 5, false, &mut refs, None)
            .unwrap();
    }
    assert_eq!(s.dirty_deadline(), 95); // merging off leaves later deadlines alone
    s.add_dirty_rectangle_internal(1, [0, 0, 10, 10], None, 3, true, &mut refs, None)
        .unwrap();
    assert_eq!(s.dirty_deadline(), 92);
    assert_eq!(s.dirty_nodes()[0].mode, 6);
    assert_eq!(s.dirty_nodes()[1].model, Some(Object(2)));
    assert_eq!(s.dirty_nodes()[0].value_15c, 37);
    assert_eq!(refs.0, [(true, Object(1)), (true, Object(2)), (false, Object(1))]);
    // An equal-area ordinary node beats a mode-0 node even though it is later.
    let mut s = SceneRenderer::default();
    s.add_dirty_rectangle_internal(0, [0, 0, 100, 100], Some(Object(1)), 0, false, &mut refs, None)
        .unwrap();
    s.add_dirty_rectangle_internal(1, [0, 0, 100, 100], None, 0, false, &mut refs, None)
        .unwrap();
    s.add_dirty_rectangle_internal(1, [0, 0, 100, 100], None, 0, true, &mut refs, None)
        .unwrap();
    assert_eq!(s.dirty_nodes()[0].mode, 0);
    assert_eq!(s.dirty_nodes()[1].mode, 6);
    // Exactly -2500 merges. One pixel row farther (-2510) does not.
    for (x, count) in [(260, 1), (261, 2)] {
        let mut s = SceneRenderer::default();
        s.add_dirty_rectangle_internal(1, [0, 0, 10, 10], None, 0, true, &mut refs, None)
            .unwrap();
        s.add_dirty_rectangle_internal(1, [x, 0, x + 10, 10], None, 0, true, &mut refs, None)
            .unwrap();
        assert_eq!(s.dirty_nodes().len(), count);
    }
    let mut s = SceneRenderer::default();
    for i in 0..f::D5_NODE_BOUNDARY.0 as i32 {
        s.add_dirty_rectangle_internal(1, [i * 1000, 0, i * 1000 + 10, 10], None, 0, true, &mut refs, None)
            .unwrap();
    }
    s.add_dirty_rectangle_internal(1, [15000, 0, 15010, 10], None, 0, true, &mut refs, None)
        .unwrap();
    assert_eq!(s.dirty_nodes().len(), f::D5_NODE_BOUNDARY.1);
    s.add_dirty_rectangle_internal(1, [16000, 0, 16010, 10], None, 0, true, &mut refs, None)
        .unwrap();
    assert_eq!(s.dirty_nodes().len(), 16);
    assert_eq!(s.dirty_nodes().last().unwrap().rect, [15000, 0, 16010, 10]);
    assert_eq!(s.dirty_area(), 21700);
}

#[derive(Default)]
struct Driver {
    trace: Vec<(u8, u32, [i32; 6])>,
    fail_at: Option<usize>,
    calls: usize,
    fail_kind: Option<u32>,
}
impl backing::BufferDriver for Driver {
    fn create(&mut self, kind: u32) -> Option<u32> {
        self.trace.push((4, kind, [0; 6]));
        if self.fail_kind == Some(kind) {
            None
        } else {
            Some(kind + 10)
        }
    }
    fn free(&mut self, h: u32) {
        self.trace.push((5, h, [0; 6]));
    }
    fn viewport_size(&mut self) -> (i32, i32) {
        (800, 600)
    }
    fn save(&mut self, h: u32, args: [i32; 6]) -> bool {
        self.trace.push((6, h, args));
        self.calls += 1;
        self.fail_at != Some(self.calls)
    }
    fn restore(&mut self, h: u32, args: [i32; 6]) -> bool {
        self.trace.push((7, h, args));
        self.calls += 1;
        self.fail_at != Some(self.calls)
    }
}
fn store() -> backing::BackingStore<Driver> {
    let mut b = backing::BackingStore::new(Some(Driver::default()));
    assert!(b.create());
    let probe: Vec<_> = b
        .driver()
        .unwrap()
        .trace
        .iter()
        .filter(|t| t.0 == 6 || t.0 == 7)
        .map(|t| (t.0, t.1))
        .collect();
    assert_eq!(probe, f::PROBE_ORDER);
    b.driver_mut().unwrap().trace.clear();
    b
}
#[test]
fn backing_b1_b4_and_both_ignored_results() {
    let mut b = store();
    assert!(b.save([0, 0, 800, 600], (800, 600), (0, 0), false).unwrap());
    assert_eq!(b.driver().unwrap().trace, [(6, 10, f::B1), (6, 11, f::B1)]);
    b.driver_mut().unwrap().trace.clear();
    assert!(!b.save(f::B2, (800, 600), (0, 0), false).unwrap());
    assert!(b.driver().unwrap().trace.is_empty());
    assert!(b.restore((800, 600), (3, -2)).unwrap());
    assert_eq!(b.driver().unwrap().trace, [(7, 10, f::B3), (7, 11, f::B3)]);
    b.driver_mut().unwrap().trace.clear();
    assert!(b.save([2, 0, 10, 600], (800, 600), (5, 0), true).unwrap());
    assert_eq!(b.driver().unwrap().trace, [(6, 10, f::B4), (6, 11, f::B4)]);
    let d = b.driver_mut().unwrap();
    d.trace.clear();
    d.fail_at = Some(d.calls + 1);
    assert!(b.save([0, 0, 800, 600], (800, 600), (0, 0), true).unwrap());
    assert_eq!(b.driver().unwrap().trace.len(), 2);
    let d = b.driver_mut().unwrap();
    d.trace.clear();
    d.fail_at = Some(d.calls + 1);
    assert!(b.restore_rect([1, 2, 3, 4, 5, 6]));
    assert_eq!(
        b.driver().unwrap().trace,
        [(7, 10, [1, 2, 3, 4, 5, 6]), (7, 11, [1, 2, 3, 4, 5, 6])]
    );
}
#[test]
fn backing_probe_failure_frees_both_short_circuits_and_missing_buffer_cleanup() {
    for fail in 1..=4 {
        let mut b = backing::BackingStore::new(Some(Driver {
            fail_at: Some(fail),
            ..Driver::default()
        }));
        assert!(!b.create());
        assert_eq!(b.handles(), [None, None]);
        let d = b.driver().unwrap();
        assert_eq!(d.calls, fail);
        assert_eq!(&d.trace[d.trace.len() - 2..], [(5, 10, [0; 6]), (5, 11, [0; 6])]);
    }
    for kind in 0..=1 {
        let mut b = backing::BackingStore::new(Some(Driver {
            fail_kind: Some(kind),
            ..Driver::default()
        }));
        assert!(!b.create());
        assert_eq!(
            b.driver().unwrap().trace,
            [(4, 0, [0; 6]), (4, 1, [0; 6]), (5, 11 - kind, [0; 6])]
        );
    }
    let mut absent = backing::BackingStore::<Driver>::new(None);
    absent.set_enabled(true);
    assert!(!absent.enabled());
    // Native no-visible early true happens BEFORE service/handle/partial gates.
    assert!(absent.save([2, 0, 3, 600], (800, 600), (5, 0), false).unwrap());
    assert!(!absent.save([0, 0, 3, 600], (800, 600), (0, 0), false).unwrap());
}
#[test]
fn shared_backing_buffers_are_owned_once_and_switch_is_process_wide() {
    let shared = store().shared();
    let other = shared.clone();
    shared.lock().unwrap().set_enabled(true);
    assert!(other.lock().unwrap().enabled());
    assert!(other.lock().unwrap().destroy());
    assert_eq!(shared.lock().unwrap().handles(), [None, None]);
    shared.lock().unwrap().set_enabled(true);
    assert!(!other.lock().unwrap().enabled());
}
#[test]
fn lists_unsigned_stable_duplicate_refs_first_per_list_and_null_terrain() {
    let mut s = SceneRenderer::default();
    let mut r = Refs::default();
    assert!(s.find_view_object(None));
    assert!(s.remove_view_object(None, &mut r));
    assert!(!s.add_view_object(Object(8), 1, 0, &mut r).unwrap());
    for &(object, key) in f::LIST_ADDS {
        s.add_view_object(Object(object), 3, key, &mut r).unwrap();
    }
    let actual: Vec<_> = s.view_list(3).unwrap().iter().map(|n| n.object.0).collect();
    assert_eq!(actual, f::LIST_ORDER);
    assert_eq!(r.0.len(), 5);
    for kind in [5, 2, 0] {
        s.add_view_object(Object(1), kind, 0, &mut r).unwrap();
    }
    s.set_backing_saved(true);
    r.0.clear();
    assert!(s.remove_view_object(Some(Object(1)), &mut r));
    assert_eq!(r.0, vec![(false, Object(1)); 4]);
    assert!(!s.backing_saved());
    assert_eq!(
        s.view_list(3).unwrap().iter().filter(|n| n.object == Object(1)).count(),
        1
    );
    r.0.clear();
    s.remove_view_object(Some(Object(1)), &mut r);
    assert_eq!(r.0, [(false, Object(1))]);
    // Distinct objects make clear order +188,+18c,+190,+194 observable.
    s.clear_view_lists(&mut r);
    r.0.clear();
    for (kind, id) in [(2, 4), (0, 3), (5, 2), (3, 1)] {
        s.add_view_object(Object(id), kind, 0, &mut r).unwrap();
    }
    r.0.clear();
    s.clear_view_lists(&mut r);
    assert_eq!(
        r.0,
        [
            (false, Object(1)),
            (false, Object(2)),
            (false, Object(3)),
            (false, Object(4))
        ]
    );
}
#[test]
fn identity_lifetime_reaches_destructor_only_after_shutdown_boundary() {
    let mut s = SceneRenderer::default();
    assert_eq!(s.ref_count(), 0);
    assert_eq!(s.defaults.bytes[0], (0x60, 1));
    assert_eq!(s.defaults.words[0], (0x1a4, 0x5000));
    assert!(s.shutdown().unwrap());
    let mut out = Some(InterfaceFace::Serializable);
    assert!(!s.query_interface(0x12345678, &mut out).unwrap());
    assert_eq!(out, Some(InterfaceFace::Serializable));
    assert_eq!(s.ref_count(), 0);
    for (id, face) in [
        (1, InterfaceFace::Renderer),
        (0x681bd529, InterfaceFace::Renderer),
        (SceneRenderer::CLSID, InterfaceFace::Renderer),
        (0x452294aa, InterfaceFace::MessageTarget),
        (0xe4fda3d4, InterfaceFace::Serializable),
    ] {
        assert!(s.query_interface(id, &mut out).unwrap());
        assert_eq!(out, Some(face));
    }
    assert_eq!(s.ref_count(), 5);
    for count in (1..5).rev() {
        assert_eq!(s.release().unwrap(), count);
    }
    assert_eq!(s.release(), Err(Unsupported::Algorithm("destructor 0x007ca020")));
    assert_eq!(s.ref_count(), 1); // do not invent deletion or last decrement
    assert!(s.draw().is_err());
    assert!(s.init().is_err());
}
#[test]
fn shake_s1_s6_stop_fade_strict_expiry_and_rounding() {
    use shake::*;
    let mut s = Shakes::default();
    let patterns = Patterns::default();
    let (h, restart) = s.start_overload(5.0, 2.0, 0.0).unwrap();
    assert!(restart);
    let (h2, restart) = s.start_overload(5.0, 2.0, 0.0).unwrap();
    assert!(!restart);
    assert_eq!([h, h2], f::S1_HANDLES);
    assert_eq!(s.nodes().iter().map(|n| n.handle).collect::<Vec<_>>(), f::S1_ORDER);
    let p = &s.nodes()[0].parameters;
    assert_eq!(
        (
            sample_exact(&p.amplitude, 0.5).unwrap(),
            sample_exact(&p.rate, 0.5).unwrap()
        ),
        f::S2
    );
    let before = s.nodes().to_vec();
    s.set_offsets(false, 3.0, [0.0; 2], 1.0, 4, &patterns).unwrap();
    assert_eq!(s.nodes(), before);
    assert_eq!(s.offsets(), f::S6_OFFSETS);
    assert_eq!(s.last_time(), 0.0);
    s.set_offsets(true, 2.0, [0.0; 2], 1.0, 4, &patterns).unwrap();
    assert_eq!(s.nodes().len(), 2); // t==duration, rate=0, amplitude=0
    s.set_offsets(true, 2.25, [0.0; 2], 1.0, 4, &patterns).unwrap();
    assert!(s.nodes().is_empty());
    s.start_overload(5.0, 2.0, 0.0).unwrap();
    let h = s.nodes()[0].handle;
    s.stop(h, 0.5).unwrap();
    assert!(s.nodes().is_empty()); // RND4 fade=0
    let p = Parameters {
        duration: 8.0,
        fade: 2.0,
        amplitude: vec![0.0],
        rate: vec![1.0],
        aspect: 1.0,
        pattern: 2,
    };
    let (h, _) = s.start(&p, None, 0.0).unwrap();
    s.stop(h, 1.0).unwrap();
    assert_eq!(s.nodes()[0].start, -5.0);
    s.stop(h, 2.0).unwrap();
    assert_eq!(s.nodes()[0].start, -5.0); // min, never moves forward
    for (distance, (w, advances)) in [1.0, 2.0].into_iter().zip(f::S3) {
        assert_eq!(positional_weight([distance, 0.0], [0.0; 2], 1.0).unwrap(), w);
        let mut s = Shakes::default();
        s.start(&p, Some([distance, 0.0]), 0.0).unwrap();
        s.set_offsets(true, 1.0, [0.0; 2], 1.0, 4, &patterns).unwrap();
        assert_eq!(s.nodes()[0].phase, if advances { 1.0 } else { 0.0 });
    }
    for &(input, expected) in f::S5 {
        assert_eq!(round_offset(input).unwrap(), expected);
    }
    for &(zoom, scale) in f::S4 {
        if zoom >= 3 {
            assert_eq!(if zoom == 3 { f64::from(0.6f32) } else { 1.0 }, scale);
        }
        let mut s = Shakes::default();
        s.start(&p, None, 0.0).unwrap();
        s.set_offsets(true, 1.0, [0.0; 2], 1.0, zoom, &patterns).unwrap();
        assert_eq!(s.offsets(), (0, 0));
    }
    assert!(round_offset(f64::NAN).is_err());
    assert!(round_offset(4194304.0).is_err());
    let mut s = Shakes::default();
    s.start_overload(5.0, 2.0, 0.0).unwrap();
    assert_eq!(
        s.set_offsets(true, 1.0, [0.0; 2], 1.0, 4, &patterns),
        Err(Unsupported::Algorithm("extended shake table interpolation"))
    );
}

fn bbox(value: ([f32; 3], [f32; 3])) -> crate::view_utilities::Aabb {
    use crate::view_utilities::{Aabb, Vec3};
    Aabb {
        min: Vec3::new(value.0[0], value.0[1], value.0[2]),
        max: Vec3::new(value.1[0], value.1[1], value.1[2]),
    }
}
#[derive(Default)]
struct Models {
    refs: Refs,
    flags: u16,
    world_queries: Vec<Object>,
    removed: usize,
}
impl References for Models {
    fn add_ref(&mut self, o: Object) {
        self.refs.add_ref(o);
    }
    fn release(&mut self, o: Object) {
        self.refs.release(o);
    }
}
impl models::ModelPrimitives for Models {
    fn element(&mut self, _: Object) -> Option<models::Element> {
        Some(models::Element {
            flags: self.flags,
            bounds: bbox(f::OLD),
        })
    }
    fn write_flags(&mut self, _: Object, flags: u16) {
        self.flags = flags;
    }
    fn world_box(&mut self, o: Object, out: &mut crate::view_utilities::Aabb) {
        self.world_queries.push(o);
        *out = bbox(f::CURRENT);
    }
    fn property_byte_one(&mut self, _: Object) -> u8 {
        0
    }
    fn remove_grid_element(&mut self, _: Object) -> bool {
        self.removed += 1;
        true
    }
    fn clear_model_element(&mut self, _: Object) {
        self.flags = 0;
    }
}
#[test]
fn model_queue_kinds_old_current_boxes_refs_predicate_and_boundaries() {
    use models::*;
    let mut scene = SceneRenderer::default();
    let mut queue = ModelQueue::default();
    let mut io = Models {
        flags: 8,
        ..Models::default()
    };
    assert!(queue.change_model(Object(1), 4, &mut io).unwrap());
    assert_eq!(queue.items()[0].kind, Kind::Move);
    assert_eq!(queue.items()[0].old_box, bbox(f::OLD));
    assert_eq!(queue.items()[0].unused_byte, Some(0));
    assert_eq!(queue.items()[0].unused_stack, UnusedStackFields::UnspecifiedNeverRead);
    assert_eq!(
        queue.process(&mut scene, false, &mut io, None),
        Err(Unsupported::Algorithm("MoveGridElement"))
    );
    assert_eq!(queue.items().len(), 1);
    assert!(!queue.processing());
    assert_eq!(io.refs.0, [(true, Object(1))]);
    let regions = moved_dirty_regions(bbox(f::OLD), bbox(f::CURRENT), 0x200).unwrap();
    assert_eq!(regions, [(1, Some(bbox(f::OLD))), (1, Some(bbox(f::CURRENT)))]);
    let overlapping = bbox(([0.5, 0.0, 0.0], [1.5, 1.0, 1.0]));
    assert_eq!(
        moved_dirty_regions(bbox(f::OLD), overlapping, 0).unwrap(),
        [(0, Some(bbox(([0.0, 0.0, 0.0], [1.5, 1.0, 1.0])))), (0, None)]
    );
    queue.clear_uninitialized(&mut io).unwrap();
    io.refs.0.clear();
    io.flags = 0;
    assert!(queue.change_model(Object(1), 2, &mut io).unwrap());
    assert!(queue.items().is_empty());
    io.flags = 8;
    for (type_, kind) in [
        (2, Kind::Change2),
        (3, Kind::Change3),
        (5, Kind::Reclassify),
        (99, Kind::Reclassify),
    ] {
        queue.change_model(Object(1), type_, &mut io).unwrap();
        let item = queue.items().last().unwrap();
        assert_eq!(item.kind, kind);
        assert_eq!(item.old_box, bbox(f::EMPTY));
        assert_eq!(item.unused_byte, None);
    }
    // Complete prefix kinds 3 and 4 query CURRENT; reclassification stays queued.
    assert_eq!(
        queue.process(&mut scene, false, &mut io, None),
        Err(Unsupported::Algorithm("UpdateGridElementObjectFlags"))
    );
    assert_eq!(io.world_queries, [Object(1), Object(1)]);
    assert_eq!(queue.items().len(), 2);
    assert_eq!(io.refs.0.iter().filter(|t| !t.0).count(), 2);
    queue.clear_uninitialized(&mut io).unwrap();
    io.refs.0.clear();
    io.flags = 0x18;
    queue
        .push(Object(7), Kind::Remove, bbox(f::OLD), Some(0), &mut io)
        .unwrap();
    assert!(
        !queue
            .inserted_decision(
                Object(7),
                Some(Element {
                    flags: 0x18,
                    bounds: bbox(f::OLD)
                })
            )
            .inserted
    );
    queue.process(&mut scene, false, &mut io, None).unwrap();
    assert_eq!(io.removed, 1);
    assert_eq!(io.refs.0, [(true, Object(7)), (false, Object(7)), (false, Object(7))]);
    for i in 1..=201 {
        assert_eq!(
            queue
                .push(Object(i), Kind::Insert, empty_box(), Some(0), &mut io)
                .unwrap(),
            i == 200
        );
    }
    assert!(
        queue
            .inserted_decision(
                Object(1),
                Some(Element {
                    flags: 0x10,
                    bounds: empty_box()
                })
            )
            .process_queue
    );
    for (flags, value, expected) in [
        (0x440, 2, true),
        (0x440, 3, false),
        (0x540, 0, false),
        (0x460, 0, false),
        (0x400, 0, false),
        (0x840, -1, true),
        (0x2040, -2, true),
    ] {
        assert_eq!(model_should_go_into_backing_store(flags, value, [3, 0, -1]), expected);
    }
}

struct Eye;
impl crate::view_utilities::CameraRayProvider for Eye {
    fn eye_ray(&self, x: f32, y: f32) -> Option<crate::view_utilities::Ray> {
        use crate::view_utilities::{Ray, Vec3};
        Some(Ray {
            origin: Vec3::new(x, 100.0, y),
            direction: Vec3::new(0.0, -1.0, 0.0),
        })
    }
}
fn configured_scene() -> SceneRenderer {
    use crate::view_utilities::{CameraState, Vec3, ViewVolume};
    let mut s = SceneRenderer::default();
    s.set_viewport_size(800, 600);
    s.view_utilities.set_renderer_present(true);
    s.view_utilities.set_city_size(8192.0, 8192.0);
    assert!(s.view_utilities.update_camera_position(Some(&CameraState {
        viewport_width: 800,
        viewport_height: 600,
        zoom: 4,
        rotation: 0,
        position: Vec3::new(0.0, 100.0, 100.0),
        look_at: Vec3::ZERO,
        volume: ViewVolume {
            left: -400.0,
            right: 400.0,
            top: 300.0,
            bottom: -300.0,
            near: 1.0,
            far: 1000.0
        },
        metres_per_pixel: 1.0,
        pixels_per_metre: 1.0,
        ..CameraState::default()
    })));
    s
}
#[test]
fn full_redraw_area_boundary_coalesces_actual_existing_frustum_and_queue_clearing() {
    let mut s = configured_scene();
    let mut r = Refs::default();
    s.set_dirty_limits(100, 10, 37).unwrap();
    s.set_backing_saved(true);
    s.add_dirty_rectangle_internal(0, [0, 0, 9, 10], Some(Object(1)), 3, false, &mut r, Some(&Eye))
        .unwrap();
    assert_eq!(s.dirty_area(), 90);
    s.add_dirty_rectangle_internal(1, [20, 0, 21, 10], None, 0, false, &mut r, Some(&Eye))
        .unwrap();
    assert!(s.full_redraw_pending());
    assert_eq!(s.redraw_reason(), 4);
    assert_eq!(s.dirty_area(), 0);
    assert!(s.dirty_nodes().is_empty());
    assert!(!s.backing_saved());
    assert_eq!(r.0, [(true, Object(1)), (false, Object(1))]);
    s.force_full_redraw(&mut r, Some(&Eye)).unwrap();
    s.force_full_redraw(&mut r, None).unwrap();
    assert_eq!(s.redraw_reason(), 10);
    assert_eq!(r.0.len(), 2);
    s.add_dirty_rectangle(0, [0, 0, 10, 10], Some(Object(2)), 0, &mut r, Some(&Eye))
        .unwrap();
    assert!(s.dirty_nodes().is_empty());
    assert_eq!(r.0.len(), 2);
}
#[test]
fn typed_record_version_and_sticky_error_partial_writes_are_literal() {
    struct Input {
        version: u16,
        words: Vec<u32>,
        at: usize,
        calls: usize,
    }
    impl record::Input for Input {
        fn version(&mut self) -> u16 {
            self.calls += 1;
            self.version
        }
        fn word(&mut self) -> u32 {
            let i = self.at;
            self.at += 1;
            self.words.get(i).copied().unwrap_or(0)
        }
        fn camera_control_token(&mut self, class: u32) -> Option<u64> {
            assert_eq!(class, 0xc9c628ec);
            self.calls += 1;
            None
        }
        fn error(&self) -> bool {
            true
        }
    }
    let mut record = record::Record {
        camera_derived: 99,
        ..record::Record::default()
    };
    let mut input = Input {
        version: 3,
        words: vec![800, 600],
        at: 0,
        calls: 0,
    };
    assert!(!record.read(&mut input));
    assert_eq!(input.calls, 1);
    assert_eq!(input.at, 0);
    assert_eq!(record.camera_derived, 99);
    input.version = 2;
    assert!(!record.read(&mut input));
    assert_eq!(input.at, 9);
    assert_eq!((record.width_bits, record.height_bits), (800, 600));
    assert_eq!(record.camera_derived, 0);
    assert_eq!(record.camera_control_token, None);
}

#[test]
fn failed_frustum_stays_unsupported_on_repeat() {
    let mut s = SceneRenderer::default();
    let mut refs = Refs::default();
    s.set_viewport_size(800, 600);
    assert!(s.force_full_redraw(&mut refs, None).is_err());
    assert!(s.force_full_redraw(&mut refs, None).is_err());
    assert!(s.full_redraw_pending());
}

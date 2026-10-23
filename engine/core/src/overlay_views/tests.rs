// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::sync::{Arc, Mutex};

mod contracts { include!("contracts.rs"); }

const RED: Colour = Colour([211, 19, 73, 129]);
const OUTLINE: Colour = Colour([7, 59, 101, 233]);
// Historical 27 tests retained. Changed expectations below follow the published
// DIFFERS answers OVV2/3/5/7, not observed portable output.
fn no_texture() -> Factory { Factory { binding: Arc::new(Binding), fail: true } }
fn p(x: f32, y: f32, z: f32) -> Point {
    Point::new(x, y, z)
}
fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 0.0001, "{a} != {b}");
}

#[derive(Default)]
struct Context {
    offset: f32,
    offsets: Vec<f32>,
    identities: usize,
    states: Vec<DrawState>,
    textures: Vec<bool>,
    wraps: usize,
    draws: Vec<Draw>,
    fail_at: Option<usize>,
}
impl DrawContext for Context {
    fn depth_offset(&self) -> f32 {
        self.offset
    }
    fn set_depth_offset(&mut self, value: f32) {
        self.offset = value;
        self.offsets.push(value);
    }
    fn set_identity_model_transform(&mut self) {
        self.identities += 1;
    }
    fn set_state(&mut self, state: DrawState) {
        self.states.push(state);
    }
    fn set_texture(&mut self, texture: Option<&Arc<dyn Texture>>) {
        self.textures.push(texture.is_some());
    }
    fn set_repeat_wrapping(&mut self) {
        self.wraps += 1;
    }
    fn draw(&mut self, draw: &Draw) -> Result<(), Error> {
        self.draws.push(draw.clone());
        if self.fail_at == Some(self.draws.len()) {
            Err(Error::DrawFailed)
        } else {
            Ok(())
        }
    }
}

struct Binding;
impl Texture for Binding {}
impl Model for Binding {}
struct Factory {
    binding: Arc<Binding>,
    fail: bool,
}
impl TextureFactory for Factory {
    fn bind(&self, _key: u32) -> Option<Arc<dyn Texture>> {
        if self.fail {
            None
        } else {
            Some(self.binding.clone())
        }
    }
}

fn path_view() -> cSC4PathView {
    let mut v = cSC4PathView::new(2.0).unwrap();
    v.init(false);
    v
}
fn add_path(v: &mut cSC4PathView, id: u32, priority: i32, points: &[Point]) {
    v.new_path(id, priority as f32, 0).unwrap();
    v.set_path_texture(0, 3.0, &no_texture()).unwrap();
    for &point in points {
        assert!(v.add_path_vertex(point).unwrap());
    }
}

#[test]
fn prescribed_order_and_newest_survives_priority_insertion_and_duplicate_ids() {
    let mut v = path_view();
    add_path(&mut v, 9, 2, &[p(1.0, 2.0, 3.0)]);
    add_path(&mut v, 9, 1, &[p(4.0, 5.0, 6.0)]);
    add_path(&mut v, 7, 2, &[p(7.0, 8.0, 9.0)]);
    assert_eq!(
        v.paths().iter().map(|p| (p.id(), p.priority())).collect::<Vec<_>>(),
        [(9, 1.0), (7, 2.0), (9, 2.0)] // OVV2 DIFFERS: equal-newest-first
    );
    assert!(v.add_path_vertex(p(8.0, 100.0, 9.0)).unwrap());
    assert_eq!(v.paths()[1].points().len(), 2);
    assert_eq!(v.paths()[0].points().len(), 1);
}

#[test]
fn prescribed_straight_strip_width_and_asymmetric_height_uv_distance() {
    let mut v = path_view();
    add_path(&mut v, 1, 0, &[p(3.0, 7.0, 5.0), p(6.0, 11.0, 5.0), p(12.0, 11.0, 5.0)]);
    let draw = v.draw_path(&v.paths()[0]).unwrap().unwrap();
    assert_eq!(draw.format, Some(10));
    assert_eq!(draw.vertices.len(), 6);
    assert_eq!(draw.vertices[0].position, p(3.0, 7.0, 3.0));
    assert_eq!(draw.vertices[1].position, p(3.0, 7.0, 7.0));
    assert_eq!(draw.vertices[2].uv, [3.75, 0.0]);
    assert_eq!(draw.vertices[5].uv, [8.25, 1.0]);
    assert!(draw.vertices.iter().all(|v| v.colour == Colour(1u32.to_le_bytes())));
}

#[test]
fn joint_bisector_and_doubleback_are_independently_expected() {
    let n = cSC4PathView::joint_normal(p(0.0, 0.0, 0.0), p(3.0, 8.0, 0.0), p(3.0, 9.0, 4.0)).unwrap();
    near(n[0], 1.0); // OVV3 DIFFERS: squared miter, published handedness
    near(n[1], -1.0);
    assert_eq!(
        cSC4PathView::joint_normal(p(0.0, 0.0, 0.0), p(3.0, 0.0, 0.0), p(0.0, 9.0, 0.0)),
        Ok([0.0, 1.0])
    );
}

#[test]
fn prescribed_duplicate_ignores_height_and_no_current_path_is_an_error() {
    let mut v = path_view();
    assert_eq!(v.add_path_vertex(p(1.0, 2.0, 3.0)), Ok(false));
    add_path(&mut v, 1, 0, &[p(0.0, 3.0, 0.0)]);
    assert!(!v.add_path_vertex(p(0.001, 900.0, 0.0)).unwrap());
    assert!(v.add_path_vertex(p(0.004, 900.0, 0.0)).unwrap());
    assert_eq!(v.add_path_vertex(p(f32::NAN, 0.0, 0.0)), Ok(true));
    v.clear_paths();
    assert!(v.paths().is_empty());
    assert_eq!(v.add_path_vertex(p(1.0, 2.0, 3.0)), Err(Error::UnsupportedDomain));
}

#[test]
fn paths_capture_texture_refs_and_binding_failure_preserves_current_texture() {
    let factory = Factory {
        binding: Arc::new(Binding),
        fail: false,
    };
    let mut v = path_view();
    add_path(&mut v, 1, 0, &[]);
    v.set_path_texture(0xFEED0193, 4.0, &factory).unwrap();
    assert_eq!(Arc::strong_count(&factory.binding), 2);
    let failing = Factory {
        binding: factory.binding.clone(),
        fail: true,
    };
    assert_eq!(v.set_path_texture(0, 7.0, &failing), Ok(()));
    assert!(v.paths()[0].texture().is_none());
    assert_eq!(v.paths()[0].repeat(), Some(7.0));
    add_path(&mut v, 2, 0, &[]);
    assert!(v.paths()[0].texture().is_none()); // OVV2: no view-wide capture
    drop(failing);
    v.set_path_texture(1, 1.0, &factory).unwrap();
    assert_eq!(Arc::strong_count(&factory.binding), 2);
    v.clear_paths();
    assert_eq!(Arc::strong_count(&factory.binding), 1);
}

#[test]
fn path_update_identity_texture_state_order_and_depth_restore_on_failure() {
    let factory = Factory {
        binding: Arc::new(Binding),
        fail: false,
    };
    let mut v = path_view();
    add_path(&mut v, 1, 0, &[p(1.0, 2.0, 3.0), p(4.0, 2.0, 3.0)]);
    v.set_path_texture(7, 3.0, &factory).unwrap();
    let mut context = Context {
        offset: -7.0,
        fail_at: Some(1),
        ..Context::default()
    };
    assert_eq!(v.update(&mut context), Err(Error::DrawFailed));
    assert_eq!(context.offsets, [15.0, -7.0]);
    assert_eq!(context.identities, 1);
    assert_eq!(context.states.iter().map(|s| s.textures).collect::<Vec<_>>(), [0, 1, 0]);
    assert_eq!(context.textures, [true]);
    assert_eq!(context.wraps, 1);
    v.init(true);
    assert_eq!(v.states().unwrap()[0].blend, Blend::Disabled);
}

#[test]
fn unspecified_path_attributes_and_arrows_fail_before_device_changes() {
    let mut v = path_view();
    v.new_path(1, 0.0, 0x80000000).unwrap();
    v.add_path_vertex(p(0.0, 0.0, 0.0)).unwrap();
    v.add_path_vertex(p(5.0, 0.0, 0.0)).unwrap();
    let mut context = Context::default();
    assert_eq!(v.update(&mut context), Err(Error::Gap(Gap::PathAttributesOvv2)));
    assert!(context.offsets.is_empty());
    v.new_path(2, 0.0, 1).unwrap();
    v.set_path_texture(0, 1.0, &no_texture()).unwrap();
    v.add_path_vertex(p(0.0, 0.0, 0.0)).unwrap();
    v.add_path_vertex(p(5.0, 0.0, 0.0)).unwrap();
    assert_eq!(v.path_draws(&v.paths()[0]).unwrap().len(), 2); // OVV3 now settled
}

struct TestLot {
    flags: u32,
    shows: bool,
}
impl Lot for TestLot {
    fn flags(&self) -> u32 {
        self.flags
    }
    fn shows_marks(&self) -> bool {
        self.shows
    }
    fn altitude_at(&self, x: f32, z: f32) -> f32 {
        70.0 + x / 8.0 + z / 4.0
    }
}
struct Filter(bool);
impl LotFilter for Filter {
    fn accepts(&self, _: &dyn Lot) -> bool {
        self.0
    }
}
struct Terrain {
    dims: [u32; 2],
    flipped: bool,
    visible: bool,
    lot: Option<TestLot>,
}
impl Default for Terrain {
    fn default() -> Self {
        Self {
            dims: [64, 64],
            flipped: false,
            visible: true,
            lot: None,
        }
    }
}
impl CellTerrain for Terrain {
    fn dimensions(&self) -> [u32; 2] {
        self.dims
    }
    fn vertex(&self, x: u32, z: u32) -> Point {
        p(x as f32 * 16.0, 2.0 + 3.0 * x as f32 + 5.0 * z as f32, z as f32 * 16.0)
    }
    fn split(&self, _: u32, _: u32) -> Split {
        if self.flipped {
            Split::Flipped
        } else {
            Split::Unflipped
        }
    }
    fn water_altitude(&self) -> f32 {
        13.0
    }
    fn in_frustum(&self, _: Point) -> bool {
        self.visible
    }
    fn lot_at(&self, _: u32, _: u32) -> Option<&dyn Lot> {
        self.lot.as_ref().map(|v| v as &dyn Lot)
    }
    fn ground_altitude(&self, x: f32, z: f32) -> f32 {
        2.0 + x * 3.0 + z * 5.0
    }
}
fn marked() -> SC4MarkedCellView {
    let mut v = SC4MarkedCellView::new([16.0, 16.0], RED, OUTLINE, false, false).unwrap();
    v.init(Arc::new(Filter(true)));
    v
}

#[test]
fn prescribed_100_and_300_cells_batch_without_duplicate_or_missing_indices() {
    for (count, sizes) in [(100, vec![400]), (300, vec![1024, 176])] {
        let mut v = marked();
        v.replace_cells((0..count).map(|i| (Cell { x: i % 50, z: i / 50 }, None)).collect());
        let draws = v.draw_cells(&Terrain::default(), None).unwrap();
        assert_eq!(draws.iter().map(|d| d.vertices.len()).collect::<Vec<_>>(), sizes);
        assert_eq!(draws.iter().map(|d| d.indices.len()).sum::<usize>(), count as usize * 6);
        for d in &draws {
            assert!(d.indices.iter().all(|&i| (i as usize) < d.vertices.len()));
        }
    }
}

#[test]
fn cell_heights_colours_and_both_splits_have_independent_expected_values() {
    let mut v = marked();
    v.replace_cells(vec![(Cell { x: 2, z: 1 }, Some(OUTLINE))]);
    let terrain = Terrain::default();
    let d = v.draw_cells(&terrain, None).unwrap().remove(0);
    assert_eq!(
        d.vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
        [
            p(32.0, 13.0, 16.0),
            p(32.0, 18.0, 32.0),
            p(48.0, 21.0, 32.0),
            p(48.0, 16.0, 16.0)
        ]
    );
    assert_eq!(d.indices, [0, 1, 2, 0, 2, 3]); // OVV5 DIFFERS
    assert!(d.vertices.iter().all(|v| v.colour == OUTLINE));
    let terrain = Terrain {
        flipped: true,
        ..terrain
    };
    assert_eq!(v.draw_cells(&terrain, None).unwrap()[0].indices, [0, 1, 3, 3, 1, 2]);
}

#[test]
fn water_lot_opaque_flags_and_filter_are_separate_branches() {
    let mut v = marked();
    v.replace_cells(vec![(Cell { x: 0, z: 0 }, None)]);
    v.set_should_use_water_altitude(true);
    let terrain = Terrain::default();
    assert!(v.draw_cells(&terrain, None).unwrap()[0]
        .vertices
        .iter()
        .all(|v| v.position.y == 13.0));
    for flag in [4, 0x100, 0x200] {
        let terrain = Terrain {
            lot: Some(TestLot {
                flags: flag,
                shows: true,
            }),
            ..Terrain::default()
        };
        let d = v.draw_cells(&terrain, None).unwrap().remove(0);
        assert_eq!(d.vertices[0].position.y, 85.5);
        assert_eq!(d.vertices[2].position.y, 91.5);
    }
    let terrain = Terrain {
        lot: Some(TestLot {
            flags: 0x100000,
            shows: false,
        }),
        ..Terrain::default()
    };
    assert!(v.draw_cells(&terrain, None).unwrap().is_empty());
    v.init(Arc::new(Filter(false)));
    assert_eq!(v.draw_cells(&terrain, None).unwrap()[0].vertices[0].position.y, 13.0);
}

#[test]
fn cell_bounds_frustum_and_non_square_clamp_nearest_vertex() {
    let mut v = marked();
    v.replace_cells(vec![(Cell { x: -1, z: 0 }, None), (Cell { x: 64, z: 0 }, None)]);
    assert!(v.draw_cells(&Terrain::default(), None).unwrap().is_empty());
    v.replace_cells(vec![(Cell { x: 0, z: 0 }, None)]);
    assert!(v
        .draw_cells(
            &Terrain {
                visible: false,
                ..Terrain::default()
            },
            None
        )
        .unwrap()
        .is_empty());
    v.set_cell_size([21.0, 11.0]).unwrap();
    assert_eq!(v.inverse_cell_size(), [1.0 / 21.0, 1.0 / 11.0]);
    v.replace_cells(vec![(Cell { x: 1, z: 2 }, None)]);
    let d = v
        .draw_cells(
            &Terrain {
                dims: [2, 2],
                visible: true,
                ..Terrain::default()
            },
            None,
        )
        .unwrap()
        .remove(0);
    assert_eq!(
        d.vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
        [
            p(21.0, 10.0, 22.0),
            p(21.0, 10.0, 22.0),
            p(42.0, 13.0, 22.0),
            p(42.0, 13.0, 22.0)
        ]
    );
}

#[test]
fn marked_update_hidden_pass_and_depth_restore_including_failure() {
    let mut v = marked();
    v.replace_cells(vec![(Cell { x: 1, z: 2 }, None)]);
    let mut c = Context {
        offset: 9.0,
        fail_at: Some(2),
        ..Context::default()
    };
    assert_eq!(v.update(&mut c, &Terrain::default(), false), Err(Error::DrawFailed));
    assert_eq!(c.offsets, [4.0, 9.0]);
    assert_eq!(c.identities, 1);
    assert_eq!(c.states[1].depth, DepthTest::Greater);
    assert_eq!(c.states[1].blend, Blend::Additive);
    assert_eq!(c.draws[0].vertices[0].colour, RED);
    assert_eq!(c.draws[1].vertices[0].colour, OUTLINE);
    c = Context::default();
    v.set_should_ignore_depth_buffer(true);
    v.update(&mut c, &Terrain::default(), true).unwrap();
    assert_eq!(c.draws.len(), 1);
    assert_eq!(c.states[0].depth, DepthTest::Disabled);
    assert_eq!(
        v.set_cell_color(1.0, 0.3, 0.8, 1.0),
        Err(Error::Gap(Gap::PackedColourOvv4))
    );
    assert_eq!(v.colour(), RED);
}

#[derive(Default, Debug)]
struct BufferLog {
    creates: Vec<(u32, u32)>,
    locks: usize,
    unlocks: usize,
    drops: usize,
    writes: Vec<(usize, Vec<[u8; 32]>)>,
    draws: Vec<Vec<u32>>,
    lock_results: Vec<bool>,
}
struct Buffer {
    log: Arc<Mutex<BufferLog>>,
    capacity: usize,
    format: u32,
    lock_ok: bool,
    draw_ok: bool,
}
impl VertexBuffer for Buffer {
    fn format(&self) -> u32 {
        self.format
    }
    fn capacity(&self) -> usize {
        self.capacity
    }
    fn lock(&mut self) -> bool {
        let mut log = self.log.lock().unwrap();
        let at = log.locks;
        log.locks += 1;
        log.lock_results.get(at).copied().unwrap_or(self.lock_ok)
    }
    fn write_vertices(&mut self, offset: usize, vertices: &[[u8; 32]]) {
        self.log.lock().unwrap().writes.push((offset, vertices.to_vec()));
    }
    fn unlock(&mut self) {
        self.log.lock().unwrap().unlocks += 1;
    }
    fn draw_triangles(&mut self, indices: &[u16]) -> bool {
        self.log.lock().unwrap().draws.push(indices.iter().map(|&v| u32::from(v)).collect());
        self.draw_ok
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        self.log.lock().unwrap().drops += 1;
    }
}
struct Driver {
    log: Arc<Mutex<BufferLog>>,
    capacity: usize,
    format: u32,
    lock_ok: bool,
    draw_ok: bool,
}
impl VertexBufferDriver for Driver {
    fn create(&self, iid: u32, format: u32) -> Option<Box<dyn VertexBuffer>> {
        self.log.lock().unwrap().creates.push((iid, format));
        Some(Box::new(Buffer {
            log: self.log.clone(),
            capacity: self.capacity,
            format: self.format,
            lock_ok: self.lock_ok,
            draw_ok: self.draw_ok,
        }))
    }
}
fn driver(capacity: usize) -> Driver {
    Driver {
        log: Arc::new(Mutex::new(BufferLog::default())),
        capacity,
        format: 11,
        lock_ok: true,
        draw_ok: true,
    }
}
struct Rows;
impl TerrainRows for Rows {
    fn vertex_bytes(&self, row: usize, column: usize) -> [u8; 32] {
        let mut bytes = [0xA9; 32];
        bytes[0] = row as u8;
        bytes[1] = column as u8;
        bytes
    }
}
fn indices(w: usize, h: usize) -> Vec<u16> {
    (0..h)
        .flat_map(|r| {
            (0..w).flat_map(move |c| {
                let _ = r; // OVV7 DIFFERS: row-pair-relative u16 source
                let a = c as u16;
                let b = a + w as u16 + 1;
                [a, b, b + 1, a, b + 1, a + 1]
            })
        })
        .collect()
}

#[test]
fn prescribed_disabled_vertex_buffers_never_query_driver_and_invalid_ones_release_once() {
    let d = driver(12);
    assert!(cSTEVertBufExtn::get_vertex_buffer_extension(&d, false, 5, 3)
        .unwrap()
        .is_none());
    assert!(d.log.lock().unwrap().creates.is_empty());
    for (capacity, format) in [(11, 11), (12, 10)] {
        let d = Driver {
            capacity,
            format,
            ..driver(12)
        };
        assert!(cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 5, 3)
            .unwrap()
            .is_none());
        let l = d.log.lock().unwrap();
        assert_eq!(l.drops, 1);
        assert_eq!(l.creates, [(0x09CD86F9, 11)]);
    }
}

#[test]
fn vertex_rows_compact_rebase_nonzero_column_and_second_append() {
    let d = driver(24);
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 5, 3)
        .unwrap()
        .unwrap();
    assert_eq!(b.index_capacity(), 90);
    let source = indices(5, 3);
    b.set_index_buffer(&source, 1);
    b.create_buffer().unwrap();
    b.draw_prims_indexed_using_vert_buf(&Rows, 1, 2, 2).unwrap();
    assert_eq!(b.pending_indices(), [0, 3, 4, 0, 4, 1, 1, 4, 5, 1, 5, 2]);
    b.draw_prims_indexed_using_vert_buf(&Rows, 2, 1, 1).unwrap();
    assert_eq!(&b.pending_indices()[12..], [6, 8, 9, 6, 9, 7]);
    let l = d.log.lock().unwrap();
    assert_eq!(l.writes[0].0, 0);
    assert_eq!(l.writes[1].0, 6);
    assert_eq!(
        l.writes[0].1.iter().map(|v| (v[0], v[1])).collect::<Vec<_>>(),
        [(1, 2), (1, 3), (1, 4), (2, 2), (2, 3), (2, 4)]
    );
    assert!(l.writes[0].1.iter().all(|v| v[2..] == [0xA9; 30]));
}

#[test]
fn vertex_flush_relock_exact_fit_lock_failure_and_no_implicit_draw_on_release() {
    let d = driver(8);
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 3, 4)
        .unwrap()
        .unwrap();
    let source = indices(3, 4);
    b.set_index_buffer(&source, 1);
    b.create_buffer().unwrap();
    b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 3).unwrap();
    assert!(d.log.lock().unwrap().draws.is_empty());
    b.draw_prims_indexed_using_vert_buf(&Rows, 1, 0, 1).unwrap();
    {
        let l = d.log.lock().unwrap();
        assert_eq!((l.locks, l.unlocks, l.draws.len()), (2, 1, 1));
    }
    b.release_vertex_buffer_extension();
    {
        let l = d.log.lock().unwrap();
        assert_eq!((l.unlocks, l.drops, l.draws.len()), (1, 1, 1)); // Release never unlocks
    }
    let d = Driver {
        lock_ok: false,
        ..driver(8)
    };
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 3, 2)
        .unwrap()
        .unwrap();
    let source = indices(3, 2);
    b.set_index_buffer(&source, 1);
    assert_eq!(
        b.create_buffer(),
        Err(Error::LockFailed)
    );
    assert!(!b.is_locked());
    assert!(d.log.lock().unwrap().writes.is_empty());
}

#[test]
fn vertex_failures_leave_no_duplicate_unlock_or_partial_write() {
    let d = Driver {
        draw_ok: false,
        ..driver(8)
    };
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 3, 2)
        .unwrap()
        .unwrap();
    b.create_buffer().unwrap();
    assert_eq!(
        b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1),
        Err(Error::MissingIndexSource)
    );
    assert!(d.log.lock().unwrap().writes.is_empty());
    let source = indices(3, 2);
    b.set_index_buffer(&source, 1);
    b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1).unwrap();
    assert_eq!(b.flush_buffer(), Ok(())); // OVV7 DIFFERS: ignored raw draw failure
    b.flush_buffer().unwrap();
    drop(b);
    assert_eq!(d.log.lock().unwrap().unlocks, 1);
}

#[test]
fn prescribed_network_yellow_limit_and_asymmetric_ground_height() {
    let v = SC4PathInfoView::new(-1, false);
    let path = NetworkPath {
        key: 4 << 24,
        flags: 0,
        points: (0..40).map(|i| p(i as f32, 99.0, 3.0)).collect(),
        has_heights: false,
    };
    let draws = v.draw_path(&path, &Terrain::default()).unwrap();
    assert_eq!(draws[0].vertices.len(), 25);
    assert_eq!(draws[0].vertices[24].position, p(24.0, 89.5, 3.0));
    assert!(draws[0].vertices.iter().all(|v| v.colour == Colour([255, 255, 0, 200])));
    let arrow = &draws[1].vertices;
    near(arrow[0].position.distance(arrow[1].position), 3.0);
    near(arrow[3].position.distance(arrow[2].position), 3.0);
    assert!(arrow[0].position.x < 24.0);
    assert!(arrow[0].position.z > 3.0);
}

struct Occupant(Vec<NetworkPath>);
impl NetworkOccupant for Occupant {
    fn paths(&self) -> &[NetworkPath] {
        &self.0
    }
    fn routes(&self, _: usize) -> RouteSet<'_> {
        RouteSet {
            routes: &[],
            chosen: None,
            track_type: 0,
        }
    }
}
struct Net(Occupant);
impl Network for Net {
    fn visible_cell_rectangle(&self) -> CellRect {
        CellRect {
            min: [2, 3],
            max: [3, 4],
        }
    }
    fn occupants(&self, cell: [i32; 2]) -> Vec<&dyn NetworkOccupant> {
        assert_eq!(cell, [2, 3]);
        vec![&self.0]
    }
}

#[test]
fn network_filter_byte_pulse_wrap_and_unknown_colours_fail_without_draw() {
    let network = Net(Occupant(vec![NetworkPath {
        key: (4 << 24) | (37 << 16),
        flags: 0,
        points: vec![p(1.0, 2.0, 3.0), p(5.0, 6.0, 9.0)],
        has_heights: true,
    }]));
    let mut v = SC4PathInfoView::new(38, true);
    v.init();
    let mut c = Context::default();
    for _ in 0..52 {
        v.update(&mut c, &network, &Terrain::default()).unwrap();
    }
    assert_eq!(v.pulse(), 4);
    assert!(c.draws.is_empty());
    v.set_path_type(37);
    v.update(&mut c, &network, &Terrain::default()).unwrap();
    assert_eq!(c.draws[0].vertices[0].position.y, 2.5);
    assert_eq!(v.state().unwrap().depth, DepthTest::Disabled);
    assert_eq!(v.path_colour(1 << 24, 0), Err(Error::Gap(Gap::PathInfoColourNov2)));
    assert_eq!(v.path_colour(4 << 24, 2), Err(Error::Gap(Gap::PathInfoColourNov2)));
}

#[test]
fn network_bars_cross_and_tiny_terminal_segments() {
    let v = SC4PathInfoView::new(-1, false);
    let path = NetworkPath {
        key: (2 << 24) | 0xFFFF,
        flags: 0,
        points: vec![p(2.0, 9.0, 3.0), p(7.0, 9.0, 3.0), p(7.0, 9.0, 3.0)],
        has_heights: true,
    };
    let draws = v.draw_path(&path, &Terrain::default()).unwrap();
    assert_eq!(draws.len(), 3); // strip, arrow using previous segment, start bar (zero end segment omitted)
    assert_eq!(draws[2].vertices[0].position, p(2.0, 9.5, 6.0));
    let cross = NetworkPath {
        key: 0xF0000000,
        ..path
    };
    let draws = v.draw_path(&cross, &Terrain::default()).unwrap();
    assert_eq!(
        draws[0].vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
        [p(-2.0, 9.5, 3.0), p(6.0, 9.5, 3.0), p(2.0, 9.5, -1.0), p(2.0, 9.5, 7.0)]
    );
}

#[test]
fn prescribed_train_choice_transparency_and_threshold_sides() {
    let route = [p(5.0, 7.0, 4.0), p(5.0, 7.0, 0.0)];
    assert_eq!(
        SC4TrainJunctionView::get_path_bitmap(&route, [1.0, 0.0], false),
        Ok(0x6BE09921)
    );
    assert_eq!(
        SC4TrainJunctionView::get_path_bitmap(&route, [1.0, 0.0], true),
        Ok(0x6BE09922)
    );
    assert_eq!(
        SC4TrainJunctionView::get_path_bitmap(&[p(0.0, 0.0, 0.0), p(4.0, 0.0, 0.0)], [1.0, 0.0], false),
        Ok(0x6BE09925)
    );
    assert_eq!(
        SC4TrainJunctionView::get_path_bitmap(&route, [-1.0, 0.0], true),
        Ok(0x6BE09924)
    );
    assert_eq!(SC4TrainJunctionView::transparency(false), 0.5);
    assert_eq!(SC4TrainJunctionView::transparency(true), 0.8);
    let mut v = SC4TrainJunctionView::new();
    assert_eq!(v.init(), Err(Error::Gap(Gap::TrainGeometryNov3)));
    assert_eq!(v.get_arrow_bitmap_quad(), Err(Error::Gap(Gap::TrainGeometryNov3)));
}

#[test]
fn empty_and_invalid_inputs_do_not_submit_or_change_state() {
    let mut context = Context::default();
    let v = path_view();
    v.update(&mut context).unwrap();
    let v = marked();
    v.update(&mut context, &Terrain::default(), false).unwrap();
    assert!(context.draws.is_empty());
    assert!(context.states.is_empty());
    assert!(context.offsets.is_empty());
    assert!(matches!(cSC4PathView::new(0.0), Err(Error::InvalidInput)));
    let mut v = marked();
    let old = v.cell_size();
    assert_eq!(v.set_cell_size([1.0, 0.0]), Err(Error::InvalidInput));
    assert_eq!(v.cell_size(), old);
}

struct TrainFactory {
    binding: Arc<Binding>,
    calls: Mutex<Vec<(u32, u32, u32)>>,
    fail_at: Option<usize>,
}
impl TrainFactory {
    fn record(&self, key: (u32, u32, u32)) -> bool {
        let mut calls = self.calls.lock().unwrap();
        calls.push(key);
        self.fail_at != Some(calls.len())
    }
}
impl TrainResourceFactory for TrainFactory {
    fn picture(&self, group: u32, instance: u32) -> Option<Arc<dyn Texture>> {
        self.record((0, group, instance))
            .then(|| self.binding.clone() as Arc<dyn Texture>)
    }
    fn model(&self, kind: u32, group: u32, instance: u32) -> Option<Arc<dyn Model>> {
        self.record((kind, group, instance))
            .then(|| self.binding.clone() as Arc<dyn Model>)
    }
}

#[test]
fn train_exact_resource_keys_direction_table_and_acquired_reference_rollback() {
    let factory = TrainFactory {
        binding: Arc::new(Binding),
        calls: Mutex::new(Vec::new()),
        fail_at: None,
    };
    let mut view = SC4TrainJunctionView::new();
    view.load_resources(&factory).unwrap();
    assert_eq!(
        *factory.calls.lock().unwrap(),
        [
            (0, 0x1ABE787D, 0x6BE09921),
            (0, 0x1ABE787D, 0x6BE09922),
            (0, 0x1ABE787D, 0x6BE09923),
            (0, 0x1ABE787D, 0x6BE09924),
            (0, 0x1ABE787D, 0x6BE09925),
            (0, 0x1ABE787D, 0x6BE09926),
            (0x5AD0E817, 0xBADB57F1, 0x8BB70000),
            (0x5AD0E817, 0xBADB57F1, 0x8BB80000),
        ]
    );
    assert_eq!(Arc::strong_count(&factory.binding), 9);
    assert_eq!(view.directions()[0][2], Some([1.0, 0.0]));
    assert_eq!(view.directions()[1][3], Some([0.0, 1.0]));
    assert_eq!(view.directions()[2][2], None);
    let d = view.directions()[0][1].unwrap();
    near(d[0], std::f32::consts::FRAC_1_SQRT_2);
    near(d[1], -std::f32::consts::FRAC_1_SQRT_2);
    let failing = TrainFactory {
        binding: factory.binding.clone(),
        calls: Mutex::new(Vec::new()),
        fail_at: Some(4),
    };
    assert_eq!(view.load_resources(&failing), Err(Error::ResourceUnavailable));
    assert_eq!(Arc::strong_count(&factory.binding), 10);
    assert!(view.resources().is_some());
    drop(failing);
    drop(view);
    assert_eq!(Arc::strong_count(&factory.binding), 1);
}

#[test]
fn train_chord_fallback_and_cross_product_boundaries() {
    let route = [
        p(0.0, 5.0, 10.0),
        p(5.0, 9.0, 0.0),
        p(5.03, 12.0, 0.02),
        p(5.05, 15.0, 0.04),
    ];
    assert_eq!(
        SC4TrainJunctionView::get_path_bitmap(&route, [1.0, 0.0], false),
        Ok(0x6BE09921)
    );
    // Unit segment directions (sqrt(15), ±1)/4 have exact boundary cross products ±0.25.
    for z in [-1.0, 1.0] {
        let route = [p(0.0, 0.0, 0.0), p(15.0_f32.sqrt(), 0.0, z)];
        assert_eq!(
            SC4TrainJunctionView::get_path_bitmap(&route, [1.0, 0.0], true),
            Ok(0x6BE09926)
        );
    }
    assert_eq!(
        SC4TrainJunctionView::get_path_bitmap(&[], [1.0, 0.0], false),
        Err(Error::InvalidInput)
    );
}

#[test]
fn arrow_flags_on_short_path_are_suppressed_and_other_bits_remain_opaque() {
    let mut view = path_view();
    view.new_path(0xFEDCBA98, -17.0, 0xA5000003).unwrap();
    view.set_path_texture(0, -3.0, &no_texture()).unwrap();
    view.add_path_vertex(p(3.0, 7.0, 11.0)).unwrap();
    view.add_path_vertex(p(4.0, 7.0, 11.0)).unwrap();
    assert_eq!(view.paths()[0].flags(), 0xA5000003);
    let draw = view.draw_path(&view.paths()[0]).unwrap().unwrap();
    assert_eq!(draw.vertices[2].uv[0], -0.75);
    assert_eq!(draw.vertices[2].position, p(4.0, 7.0, 9.0));
}

#[test]
fn all_owner_state_is_send_and_sync_without_refcell_getters() {
    fn check<T: Send + Sync>() {}
    check::<cSC4PathView>();
    check::<PathInfo>();
    check::<SC4MarkedCellView>();
    check::<cSTEVertBufExtn>();
    check::<SC4PathInfoView>();
    check::<SC4TrainJunctionView>();
}

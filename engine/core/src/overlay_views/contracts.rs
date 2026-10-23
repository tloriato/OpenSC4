// SPDX-License-Identifier: GPL-3.0-or-later
// Independently transcribed public overlay-views §7.1 expectations.
// Portable fixtures only; original execution remains PROPOSED/UNEXECUTED.
// L7 (OVV4) and L14 (OVV6) deliberately excluded.
use super::*;

const L1_ORDER: [u32; 5] = [2, 3, 4, 1, 5];
const L2_COLOUR: u32 = 0x12345678;
const L3_POS: [[f32; 3]; 4] = [[0., 0., -2.], [0., 0., 2.], [6., 8., -2.], [6., 8., 2.]];
const L3_UV: [[f32; 2]; 4] = [[0., 0.], [0., 1.], [10., 0.], [10., 1.]];
const L4_JOINT: [[f32; 3]; 2] = [[12., 0., -2.], [8., 0., 2.]];
const L4_UV: [[f32; 2]; 2] = [[8., 0.], [12., 1.]];
const L5_TRIANGLE: [[f32; 3]; 3] = [[0., 0., 0.], [3., 4., 10.], [3., 4., -10.]];
const L6_BASES: [f32; 2] = [9., 11.];
const L8_COLLAPSED: [f32; 2] = [32., 32.];
const L9_HEIGHTS: [f32; 2] = [10., 3.]; // frustum, final lot-replaced quad
const L10_INDICES: [u16; 6] = [4, 5, 7, 5, 8, 7];
const L11_INDEX: u16 = 65535;
const L12_COUNTS: [usize; 2] = [0, 0]; // failed relock after unlock/draw(12)
const L13_COUNTS: [usize; 2] = [4, 6]; // failed create, null flush retains counts

fn make_path(points: &[Point], flags: u32, t: f32, repeat: f32) -> cSC4PathView {
    let mut view = cSC4PathView::new(t).unwrap();
    view.new_path(L2_COLOUR, 0.0, flags).unwrap();
    view.set_path_texture(0, repeat, &no_texture()).unwrap();
    for &p in points {
        view.add_path_vertex(p).unwrap();
    }
    view
}
fn positions(draw: &Draw) -> Vec<[f32; 3]> {
    draw.vertices
        .iter()
        .map(|v| [v.position.x, v.position.y, v.position.z])
        .collect()
}
#[test]
fn l1_binary32_ties_and_unordered_search() {
    let mut view = path_view();
    for (colour, priority) in [(1, 2.0), (2, -1.0), (3, 0.5), (4, 2.0), (5, f32::from_bits(0x7FC00000))] {
        view.new_path(colour, priority, 0).unwrap();
    }
    assert_eq!(
        view.paths().iter().map(PathInfo::colour_bits).collect::<Vec<_>>(),
        L1_ORDER
    );
    view.new_path(6, 3.0, 0).unwrap(); // old NaN also continues searching
    assert_eq!(view.paths().last().unwrap().colour_bits(), 6);
    view.new_path(7, -0.0, 0).unwrap();
    view.new_path(8, 0.0, 0).unwrap();
    assert_eq!(view.paths()[1].colour_bits(), 8);
}
#[test]
fn l2_l3_packed_colour_three_dimensional_uv_and_side_order() {
    let view = make_path(&[p(0., 0., 0.), p(6., 8., 0.)], 0, 2., 4.);
    let draw = view.draw_path(&view.paths()[0]).unwrap().unwrap();
    assert_eq!(positions(&draw), L3_POS);
    assert_eq!(draw.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(), L3_UV);
    assert!(draw
        .vertices
        .iter()
        .all(|v| u32::from_le_bytes(v.colour.0) == L2_COLOUR));
}
#[test]
fn l4_miter_joint_projection_and_final_uv() {
    let view = make_path(&[p(0., 0., 0.), p(10., 0., 0.), p(10., 0., 10.)], 0, 2., 4.);
    let draw = view.draw_path(&view.paths()[0]).unwrap().unwrap();
    assert_eq!(&positions(&draw)[2..4], &L4_JOINT);
    assert_eq!(draw.vertices[2].uv, L4_UV[0]);
    assert_eq!(draw.vertices[3].uv, L4_UV[1]);
    assert_eq!(draw.vertices[4].uv, [20., 0.]);
    assert_eq!(draw.vertices[5].uv, [20., 1.]);
}
#[test]
fn l5_arrow_threshold_winding_and_three_dimensional_base() {
    let view = cSC4PathView::new(5.).unwrap();
    let mut tip = p(0., 0., 0.);
    let draw = view
        .draw_arrowhead(&mut tip, p(6., 8., 0.), L2_COLOUR)
        .unwrap()
        .unwrap();
    assert_eq!(positions(&draw), L5_TRIANGLE);
    assert_eq!(tip, p(3., 4., 0.));
    assert!(draw
        .vertices
        .iter()
        .all(|v| v.uv == [0.; 2] && u32::from_le_bytes(v.colour.0) == L2_COLOUR));
    let mut tip = p(0., 0., 0.);
    assert!(view.draw_arrowhead(&mut tip, p(5.9, 8., 0.), 1).unwrap().is_none());
    assert_eq!(tip, p(0., 0., 0.));
}
#[test]
fn l6_start_before_end_and_local_shortening() {
    for (length, triangles) in [(13., 2), (8., 1)] {
        let view = make_path(&[p(0., 0., 0.), p(length, 0., 0.)], 3, 2., 4.);
        let draws = view.path_draws(&view.paths()[0]).unwrap();
        assert_eq!(draws.len(), triangles + 1);
        assert!(draws[..triangles].iter().all(|d| d.primitive == Primitive::Triangles));
        if length == 13. {
            assert_eq!(draws[0].vertices[1].position.x, L6_BASES[0]);
            assert_eq!(draws[1].vertices[1].position.x, L6_BASES[1]);
        } else {
            assert_eq!(draws[1].vertices[0].position.x, 6.);
        }
        assert_eq!(view.paths()[0].points(), [p(0., 0., 0.), p(length, 0., 0.)]);
    }
}
#[test]
fn asymmetric_noncoplanar_miter_and_twisted_arrow_path() {
    let view = make_path(&[p(1., 2., 3.), p(7., 10., 3.), p(7., 13., 7.)], 0, 2., 4.);
    let draw = view.draw_path(&view.paths()[0]).unwrap().unwrap();
    assert_eq!(draw.vertices[2].position, p(9., 10., 1.));
    assert_eq!(draw.vertices[3].position, p(5., 10., 5.));
    near(draw.vertices[2].uv[0], 8.4);
    near(draw.vertices[3].uv[0], 11.6);
    near(draw.vertices[4].uv[0], 15.);
    let view = make_path(&[p(0., 0., 0.), p(3., 4., 0.), p(3., 4., 3.)], 3, 2., 4.);
    let draws = view.path_draws(&view.paths()[0]).unwrap();
    assert_eq!(draws.len(), 2); // start accepted in 3D; end too short
    near(draws[0].vertices[1].position.x, 1.8);
    near(draws[0].vertices[1].position.y, 2.4);
    // Shortened first segment is length 2: outgoing z projection at the joint
    // is s=2 plus offsets -2/+2, hence 0/4 (not the original-length 1/5).
    near(draws[1].vertices[2].uv[0], 0.);
    near(draws[1].vertices[3].uv[0], 4.);
}
#[test]
fn miter_squared_small_threshold_uses_unscaled_fallback() {
    let a = p(-1., 0., 0.);
    let b = p(0., 0., 0.);
    for (dz, fallback) in [(0.019, true), (0.021, false)] {
        let c = p(-1., 3., dz);
        let offset = cSC4PathView::joint_offset(a, b, c, 7.).unwrap();
        let outgoing = cSC4PathView::endpoint_normal(b, c).unwrap();
        if fallback {
            assert_eq!(offset, outgoing);
        } else {
            assert!(offset[0].hypot(offset[1]) > 100.);
        }
    }
}
#[test]
fn strict_binary64_duplicate_boundary_and_stale_selection() {
    let edge = (1.0e-5_f64.sqrt()) as f32;
    let below = f32::from_bits(edge.to_bits() - 1);
    let above = f32::from_bits(edge.to_bits() + 1);
    for (x, accept) in [(below, false), (above, true)] {
        let mut view = make_path(&[p(0., 0., 0.)], 0, 2., 4.);
        assert_eq!(view.add_path_vertex(p(x, 999., 0.)).unwrap(), accept);
    }
    let mut view = cSC4PathView::default();
    assert!(!view.add_path_vertex(p(0., 0., 0.)).unwrap());
    assert_eq!(view.set_path_texture(0, 1., &no_texture()), Err(Error::NoCurrentPath));
    view.new_path(1, 0., 0).unwrap();
    assert_eq!(view.paths()[0].repeat(), None);
    view.clear_paths();
    assert_eq!(view.add_path_vertex(p(0., 0., 0.)), Err(Error::UnsupportedDomain));
    view.new_path(2, 0., 0).unwrap();
    assert!(view.add_path_vertex(p(f32::NAN, 0., 0.)).unwrap());
}

struct OrderedTerrain {
    calls: Mutex<Vec<&'static str>>,
    lot: TestLot,
    dims: [u32; 2],
    visible: bool,
}
impl CellTerrain for OrderedTerrain {
    fn dimensions(&self) -> [u32; 2] {
        self.dims
    }
    fn vertex(&self, x: u32, z: u32) -> Point {
        self.calls.lock().unwrap().push("terrain");
        p(x as f32 * 16., 2., z as f32 * 16.)
    }
    fn split(&self, _: u32, _: u32) -> Split {
        Split::Unflipped
    }
    fn water_altitude(&self) -> f32 {
        self.calls.lock().unwrap().push("water");
        10.
    }
    fn in_frustum(&self, point: Point) -> bool {
        self.calls.lock().unwrap().push("frustum");
        assert_eq!(point.y, L9_HEIGHTS[0]);
        self.visible
    }
    fn lot_at(&self, _: u32, _: u32) -> Option<&dyn Lot> {
        self.calls.lock().unwrap().push("lot");
        Some(&self.lot)
    }
    fn ground_altitude(&self, _: f32, _: f32) -> f32 {
        0.
    }
}
struct LowLot;
impl Lot for LowLot {
    fn flags(&self) -> u32 {
        0
    }
    fn shows_marks(&self) -> bool {
        true
    }
    fn altitude_at(&self, _: f32, _: f32) -> f32 {
        3.
    }
}
struct L9Terrain(OrderedTerrain);
impl CellTerrain for L9Terrain {
    fn dimensions(&self) -> [u32; 2] {
        self.0.dimensions()
    }
    fn vertex(&self, x: u32, z: u32) -> Point {
        self.0.vertex(x, z)
    }
    fn split(&self, x: u32, z: u32) -> Split {
        self.0.split(x, z)
    }
    fn water_altitude(&self) -> f32 {
        self.0.water_altitude()
    }
    fn in_frustum(&self, p: Point) -> bool {
        self.0.in_frustum(p)
    }
    fn lot_at(&self, x: u32, z: u32) -> Option<&dyn Lot> {
        self.0.lot_at(x, z);
        Some(&LowLot)
    }
    fn ground_altitude(&self, x: f32, z: f32) -> f32 {
        self.0.ground_altitude(x, z)
    }
}
#[test]
fn l8_l9_grid_clamp_water_frustum_then_lot_and_lower_vertex() {
    for size in [8., 16.] {
        let terrain = L9Terrain(OrderedTerrain {
            calls: Mutex::new(Vec::new()),
            lot: TestLot { flags: 0, shows: true },
            dims: [4, 4],
            visible: true,
        });
        let mut view = marked();
        view.set_cell_size([size; 2]).unwrap();
        view.set_should_use_water_altitude(true);
        view.replace_cells(vec![(
            Cell {
                x: if size == 8. { 7 } else { 0 },
                z: if size == 8. { 7 } else { 0 },
            },
            None,
        )]);
        let draw = view.draw_cells(&terrain, None).unwrap().remove(0);
        let calls = terrain.0.calls.lock().unwrap();
        if size == 8. {
            assert!(draw
                .vertices
                .iter()
                .all(|v| [v.position.x, v.position.z] == L8_COLLAPSED && v.position.y == 10.));
            assert_eq!(*calls, ["terrain", "terrain", "terrain", "terrain", "water", "frustum"]);
        } else {
            assert!(draw.vertices.iter().all(|v| v.position.y == L9_HEIGHTS[1]));
            assert_eq!(
                *calls,
                ["terrain", "terrain", "terrain", "terrain", "water", "frustum", "lot"]
            );
        }
    }
    for (x, want) in [(0., 0), (0.5, 0), (0.75, 0), (1., 1), (1.5, 1), (2., 2)] {
        assert_eq!(super::super::cells::lower_vertex(x), Ok(want));
    }
    assert_eq!(LOT_FILTER_MASK, 0x1F4F);
    let mut view = marked();
    view.set_cell_size([f32::NAN, -1.]).unwrap();
    assert!(view.draw_cells(&Terrain::default(), None).unwrap().is_empty());
    view.replace_cells(vec![(Cell { x: 0, z: 0 }, None)]);
    assert_eq!(
        view.draw_cells(&Terrain::default(), None),
        Err(Error::UnsupportedDomain)
    );
}
#[test]
fn l10_l11_byte_flag_rebase_wrap_capacity_equality_and_raw_rows() {
    for flag in [0, 1] {
        let mut source = vec![0u16; 60];
        let start = if flag == 0 { 6 } else { 54 };
        source[start..start + 6].copy_from_slice(&[1, 2, 6, 2, 7, 6]);
        let d = driver(10);
        let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 4, 8)
            .unwrap()
            .unwrap();
        assert_eq!(b.index_capacity(), 48);
        assert_eq!(d.log.lock().unwrap().locks, 0);
        b.set_index_buffer(&source, flag);
        b.create_buffer().unwrap();
        b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1).unwrap(); // V=4
        b.draw_prims_indexed_using_vert_buf(&Rows, 2, 1, 2).unwrap();
        // L10 prints six q values although n=2 consumes twelve words (§3.1).
        // Check its six specified values; the remaining six raw zeros are explicit backing.
        assert_eq!(&b.pending_indices()[6..12], &L10_INDICES);
        assert_eq!(b.pending_indices().len(), 18);
        assert_eq!(b.used_vertices(), 10);
        let log = d.log.lock().unwrap();
        assert_eq!((log.locks, log.unlocks), (1, 0));
        assert_eq!(log.writes[1].0, 4);
        assert_eq!(
            log.writes[1].1[0],
            [
                2, 1, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9,
                0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9, 0xA9
            ]
        );
    }
    let d = driver(10);
    let source = [0u16; 18];
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 4, 8)
        .unwrap()
        .unwrap();
    b.set_index_buffer(&source, 0);
    b.create_buffer().unwrap();
    b.draw_prims_indexed_using_vert_buf(&Rows, 2, 1, 2).unwrap();
    assert_eq!(b.pending_indices()[0], L11_INDEX);
}
#[test]
fn l12_l13_failed_locks_retained_counts_and_release_without_flush() {
    let d = driver(10);
    d.log.lock().unwrap().lock_results = vec![true, false];
    let source = [0u16; 48];
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 4, 8)
        .unwrap()
        .unwrap();
    b.set_index_buffer(&source, 0);
    b.create_buffer().unwrap();
    for _ in 0..2 {
        b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1).unwrap();
    }
    b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 2).unwrap();
    assert_eq!([b.used_vertices(), b.pending_indices().len()], L12_COUNTS);
    assert!(!b.is_locked());
    {
        let log = d.log.lock().unwrap();
        assert_eq!((log.unlocks, log.locks, log.draws[0].len()), (1, 2, 12));
    }
    let d = driver(10);
    d.log.lock().unwrap().lock_results = vec![true, false];
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 4, 8)
        .unwrap()
        .unwrap();
    b.set_index_buffer(&source, 0);
    b.create_buffer().unwrap();
    b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1).unwrap();
    assert_eq!(b.create_buffer(), Err(Error::LockFailed));
    b.flush_buffer().unwrap();
    assert_eq!([b.used_vertices(), b.pending_indices().len()], L13_COUNTS);
    assert_eq!(d.log.lock().unwrap().unlocks, 0);
    b.create_buffer().unwrap();
    b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1).unwrap();
    b.release_vertex_buffer_extension();
    assert!(b.is_locked());
    assert_eq!([b.used_vertices(), b.pending_indices().len()], L13_COUNTS);
    let log = d.log.lock().unwrap();
    assert_eq!((log.unlocks, log.draws.len(), log.drops), (0, 0, 1));
}
#[test]
fn zero_n_copies_two_no_lazy_lock_and_index_growth_never_flushes() {
    let d = driver(100);
    let source = [0u16; 24];
    let mut b = cSTEVertBufExtn::get_vertex_buffer_extension(&d, true, 4, 1)
        .unwrap()
        .unwrap();
    b.set_index_buffer(&source, 0);
    b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1).unwrap();
    assert_eq!(b.used_vertices(), 0);
    b.create_buffer().unwrap();
    b.draw_prims_indexed_using_vert_buf(&Rows, 0, 2, 0).unwrap();
    assert_eq!(b.used_vertices(), 2);
    assert!(b.pending_indices().is_empty());
    for _ in 0..5 {
        b.draw_prims_indexed_using_vert_buf(&Rows, 0, 0, 1).unwrap();
    }
    assert_eq!(b.index_capacity(), 46);
    assert_eq!(d.log.lock().unwrap().unlocks, 0);
    b.flush_buffer().unwrap();
    assert_eq!(d.log.lock().unwrap().unlocks, 1);
}

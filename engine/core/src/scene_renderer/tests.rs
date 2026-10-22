// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use super::fixtures as f;
#[derive(Default)]
struct Refs(Vec<(bool, Object)>);
impl References for Refs {
    fn add_ref(&mut self, object: Object) { self.0.push((true, object)); }
    fn release(&mut self, object: Object) { self.0.push((false, object)); }
}
#[test]
fn literal_dirty_fixtures() {
    for f in f::DIRTY {
        let mut s = SceneRenderer::default(); let mut refs = Refs::default();
        for &(mode, rect) in f.queued {
            s.add_dirty_rectangle_internal(mode, rect, Some(Object(9)), 0, false, &mut refs, None).unwrap();
        }
        s.add_dirty_rectangle_internal(f.incoming.0, f.incoming.1, Some(Object(10)), 0, true, &mut refs, None).unwrap();
        let actual: Vec<_> = s.dirty_nodes().iter().map(|n| (n.mode, n.rect)).collect();
        assert_eq!(actual, f.expected, "{}", f.name);
        assert_eq!(s.dirty_area(), f.area, "{}", f.name);
        if f.name.starts_with("D3") { assert_eq!(refs.0, [(true, Object(9)), (false, Object(9))]); }
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

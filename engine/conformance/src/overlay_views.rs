// SPDX-License-Identifier: GPL-3.0-or-later
//! Stateless, language-neutral vectors for the prescribed overlay-view scenarios.
use crate::json::Json;
use crate::runner::{field, HostRef};
use crate::scalar::{check_expect, jf, ji, R};
use fc4_core::overlay_views::*;
use std::cell::Cell as Counter;

struct Unbound;
impl TextureFactory for Unbound {
    fn bind(&self, _: u32) -> Option<std::sync::Arc<dyn Texture>> { None }
}

pub(crate) fn exec(_host: &HostRef, name: &str, op: &Json) -> Option<R<()>> {
    let rest = name.strip_prefix("ov_")?;
    Some(run(rest, op))
}

fn number(v: &Json) -> R<f32> {
    v.as_f64()
        .map(|n| n as f32)
        .filter(|n| n.is_finite())
        .ok_or_else(|| "expected finite number".into())
}
fn integer(v: &Json) -> R<i64> {
    v.as_i64().ok_or_else(|| "expected integer".into())
}
fn array(v: &Json) -> R<&[Json]> {
    v.as_array().ok_or_else(|| "expected array".into())
}
fn boolean(v: &Json) -> R<bool> {
    v.as_bool().ok_or_else(|| "expected boolean".into())
}
fn points(v: &Json) -> R<Vec<Point>> {
    array(v)?
        .iter()
        .map(|v| {
            let a = array(v)?;
            if a.len() != 3 {
                return Err("point needs three components".into());
            }
            Ok(Point::new(number(&a[0])?, number(&a[1])?, number(&a[2])?))
        })
        .collect()
}
fn core<T>(result: Result<T, Error>) -> R<T> {
    result.map_err(|e| format!("{e:?}"))
}
fn jarray(values: impl IntoIterator<Item = Json>) -> Json {
    Json::Array(values.into_iter().collect())
}
fn jpoint(p: Point) -> Json {
    jarray([jf(p.x), jf(p.y), jf(p.z)])
}

fn run(name: &str, op: &Json) -> R<()> {
    let got = match name {
        "order" => {
            let mut view = core(cSC4PathView::new(1.0))?;
            for path in array(field(op, "paths")?)? {
                let path = array(path)?;
                if path.len() != 2 {
                    return Err("path needs id and priority".into());
                }
                let id = u32::try_from(integer(&path[0])?).map_err(|_| "id out of range")?;
                let priority = number(&path[1])?;
                core(view.new_path(id, priority, 0))?;
            }
            jarray(view.paths().iter().map(|p| ji(p.id())))
        }
        "strip" => {
            let mut view = core(cSC4PathView::new(number(field(op, "thickness")?)?))?;
            core(view.new_path(u32::MAX, 0.0, 0))?;
            core(view.set_path_texture(0, number(field(op, "repeat")?)?, &Unbound))?;
            for p in points(field(op, "points")?)? {
                core(view.add_path_vertex(p))?;
            }
            let draw = core(view.draw_path(&view.paths()[0]))?.ok_or("empty strip")?;
            jarray(
                draw.vertices
                    .into_iter()
                    .map(|v| jarray([jpoint(v.position), jarray(v.uv.map(jf))])),
            )
        }
        "duplicate" => {
            let mut view = core(cSC4PathView::new(1.0))?;
            core(view.new_path(1, 0.0, 0))?;
            let mut accepted = Vec::new();
            for p in points(field(op, "points")?)? {
                accepted.push(Json::Bool(core(view.add_path_vertex(p))?));
            }
            jarray(accepted)
        }
        "cells" => {
            let count = usize::try_from(integer(field(op, "count")?)?).map_err(|_| "invalid count")?;
            if count > 4096 {
                return Err("fixture count exceeds terrain".into());
            }
            let mut view = core(SC4MarkedCellView::new(
                [16.0; 2],
                Colour([255; 4]),
                Colour([0; 4]),
                false,
                false,
            ))?;
            view.replace_cells(
                (0..count)
                    .map(|n| {
                        (
                            Cell {
                                x: (n % 64) as i32,
                                z: (n / 64) as i32,
                            },
                            None,
                        )
                    })
                    .collect(),
            );
            jarray(
                core(view.draw_cells(&FlatTerrain, None))?
                    .iter()
                    .map(|d| ji(d.vertices.len() as u32)),
            )
        }
        "vb_disabled" => {
            let driver = DisabledDriver(Counter::new(0));
            if core(cSTEVertBufExtn::get_vertex_buffer_extension(&driver, false, 64, 64))?.is_some() {
                return Err("disabled extension was made".into());
            }
            ji(driver.0.get())
        }
        "path_info" => {
            let key = u32::try_from(integer(field(op, "key")?)?).map_err(|_| "invalid key")?;
            let count = usize::try_from(integer(field(op, "count")?)?).map_err(|_| "invalid count")?;
            if count > 1000 {
                return Err("fixture count too large".into());
            }
            let path = NetworkPath {
                key,
                flags: 0,
                has_heights: true,
                points: (0..count).map(|n| Point::new(n as f32, 7.0, 3.0)).collect(),
            };
            let view = SC4PathInfoView::new(-1, false);
            let draws = core(view.draw_path(&path, &FlatTerrain))?;
            let strip = draws.first().ok_or("missing strip")?;
            let colour = strip.vertices.first().ok_or("missing first vertex")?.colour;
            jarray([ji(strip.vertices.len() as u32), jarray(colour.0.map(ji))])
        }
        "train_arrow" => {
            let e = array(field(op, "entry")?)?;
            if e.len() != 2 {
                return Err("entry needs two components".into());
            }
            ji(core(SC4TrainJunctionView::get_path_bitmap(
                &points(field(op, "points")?)?,
                [number(&e[0])?, number(&e[1])?],
                boolean(field(op, "selected")?)?,
            ))?)
        }
        "train_alpha" => jf(SC4TrainJunctionView::transparency(boolean(field(op, "near")?)?)),
        _ => return Err(format!("unknown overlay op {name}")),
    };
    check_expect(op, got)
}

struct FlatTerrain;
impl CellTerrain for FlatTerrain {
    fn dimensions(&self) -> [u32; 2] {
        [64, 64]
    }
    fn vertex(&self, x: u32, z: u32) -> Point {
        Point::new(x as f32 * 16.0, 0.0, z as f32 * 16.0)
    }
    fn split(&self, _: u32, _: u32) -> Split {
        Split::Unflipped
    }
    fn water_altitude(&self) -> f32 {
        0.0
    }
    fn in_frustum(&self, _: Point) -> bool {
        true
    }
    fn lot_at(&self, _: u32, _: u32) -> Option<&dyn Lot> {
        None
    }
    fn ground_altitude(&self, _: f32, _: f32) -> f32 {
        0.0
    }
}
struct DisabledDriver(Counter<u32>);
impl VertexBufferDriver for DisabledDriver {
    fn create(&self, _: u32, _: u32) -> Option<Box<dyn VertexBuffer>> {
        self.0.set(self.0.get() + 1);
        None
    }
}

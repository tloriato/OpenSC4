// SPDX-License-Identifier: GPL-3.0-or-later
//! Owned model queue policy, with missing same-owner helpers kept as boundaries.
use super::{Object, References, Result, SceneRenderer, Unsupported};
use crate::view_utilities::{Aabb, CameraRayProvider, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element {
    pub flags: u16,
    pub bounds: Aabb,
}
pub trait ModelPrimitives: References {
    fn element(&mut self, model: Object) -> Option<Element>;
    fn write_flags(&mut self, model: Object, flags: u16);
    fn world_box(&mut self, model: Object, out: &mut Aabb);
    fn property_byte_one(&mut self, model: Object) -> u8;
    fn remove_grid_element(&mut self, model: Object) -> bool;
    fn clear_model_element(&mut self, model: Object);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Insert = 0,
    Remove = 1,
    Move = 2,
    Change2 = 3,
    Change3 = 4,
    Reclassify = 5,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnusedStackFields {
    UnspecifiedNeverRead,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub model: Option<Object>,
    pub kind: Kind,
    pub unused_byte: Option<u8>,
    pub unused_stack: UnusedStackFields,
    pub old_box: Aabb,
}
pub fn empty_box() -> Aabb {
    Aabb {
        min: Vec3::new(1.0, 1.0, 1.0),
        max: Vec3::new(0.0, 0.0, 0.0),
    }
}

/// Raw flags and signed resource thresholds. These are NOT semantic renamings
/// of undocumented bits; the published predicate is evaluated here, not by IO.
pub fn model_should_go_into_backing_store(flags: u16, value_14: i32, limits: [i32; 3]) -> bool {
    flags & 0x100 == 0
        && flags & 0x20 == 0
        && flags & 0x40 != 0
        && ((flags & 0x400 != 0 && value_14 < limits[0])
            || (flags & 0x1800 != 0 && value_14 < limits[1])
            || (flags & 0x2000 != 0 && value_14 < limits[2]))
}

#[derive(Default)]
pub struct ModelQueue {
    items: Vec<Item>,
    processing: bool,
}
impl ModelQueue {
    pub fn items(&self) -> &[Item] {
        &self.items
    }
    pub fn processing(&self) -> bool {
        self.processing
    }
    /// Portable typed copy of 0x007c2e50, not a guessed native call signature.
    pub fn push(
        &mut self,
        model: Object,
        kind: Kind,
        old_box: Aabb,
        unused_byte: Option<u8>,
        refs: &mut dyn References,
    ) -> Result<bool> {
        if self.processing {
            return Err(Unsupported::Reentry);
        }
        self.items.try_reserve(1).map_err(|_| Unsupported::Allocation)?;
        refs.add_ref(model);
        self.items.push(Item {
            model: Some(model),
            kind,
            unused_byte,
            unused_stack: UnusedStackFields::UnspecifiedNeverRead,
            old_box,
        });
        Ok(self.items.len() == 200) // equality, NOT >=
    }
    pub fn change_model(&mut self, model: Object, type_: u32, io: &mut dyn ModelPrimitives) -> Result<bool> {
        if self.processing {
            return Err(Unsupported::Reentry);
        }
        if type_ == 4 {
            return self.move_model(model, io);
        }
        let Some(element) = io.element(model) else {
            return Ok(false);
        };
        if type_ == 1 {
            let on = io.property_byte_one(model) != 0;
            io.write_flags(model, (element.flags & !0x80) | if on { 0x80 } else { 0 });
            if on {
                return Err(Unsupported::Algorithm("SetQuadrantFlagsForElement"));
            }
            return Ok(true);
        }
        if element.flags & 8 != 0 || type_ == 5 {
            let kind = match type_ {
                2 => Kind::Change2,
                3 => Kind::Change3,
                _ => Kind::Reclassify,
            };
            // The spec does not establish +5 for ChangeModel. Do not invent 0.
            let full = self.push(model, kind, empty_box(), None, io)?;
            io.write_flags(model, element.flags | 0x10);
            if full {
                return Err(Unsupported::Algorithm("queue length 200 requires DoFullViewUpdate"));
            }
        }
        Ok(true)
    }
    pub fn move_model(&mut self, model: Object, io: &mut dyn ModelPrimitives) -> Result<bool> {
        if self.processing {
            return Err(Unsupported::Reentry);
        }
        let Some(element) = io.element(model) else {
            return Err(Unsupported::Algorithm("InsertModel"));
        };
        if element.flags & 8 == 0 {
            return Err(Unsupported::Algorithm("MoveGridElement"));
        }
        let full = self.push(model, Kind::Move, element.bounds, Some(0), io)?;
        io.write_flags(model, element.flags | 0x10);
        if full {
            return Err(Unsupported::Algorithm("queue length 200 requires DoFullViewUpdate"));
        }
        Ok(true)
    }
    /// The queue-membership decision is complete; >8 pending items additionally
    /// requires ProcessModelChangeQueue. Return an effect explicitly, not an
    /// invented completed IsModelInserted when processing cannot run.
    pub fn inserted_decision(&self, model: Object, element: Option<Element>) -> InsertedDecision {
        let Some(element) = element else {
            return InsertedDecision {
                inserted: false,
                process_queue: false,
            };
        };
        let queued = element.flags & 0x10 != 0;
        let removed = queued
            && self
                .items
                .iter()
                .any(|i| i.model == Some(model) && i.kind == Kind::Remove);
        InsertedDecision {
            inserted: !removed,
            process_queue: queued && self.items.len() > 8,
        }
    }
    /// Complete kinds 0,1,3,4 within the existing projection domain. Kinds 2/5
    /// stop at missing own helpers. Completed prefix entries are released once;
    /// the reached unsupported entry and later entries remain owned in order.
    pub fn process(
        &mut self,
        scene: &mut SceneRenderer,
        backing_enabled: bool,
        io: &mut dyn ModelPrimitives,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<()> {
        if self.processing {
            return Err(Unsupported::Reentry);
        }
        self.processing = true;
        let result = self.process_inner(scene, backing_enabled, io, camera);
        self.processing = false;
        result
    }
    fn process_inner(
        &mut self,
        scene: &mut SceneRenderer,
        backing_enabled: bool,
        io: &mut dyn ModelPrimitives,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<()> {
        while let Some(item) = self.items.first().copied() {
            let model = item.model.ok_or(Unsupported::Domain("nulled model queue item"))?;
            let element = io
                .element(model)
                .ok_or(Unsupported::Domain("queued model without element"))?;
            if item.kind == Kind::Move {
                return Err(Unsupported::Algorithm("MoveGridElement"));
            }
            if item.kind == Kind::Reclassify {
                return Err(Unsupported::Algorithm("UpdateGridElementObjectFlags"));
            }
            let mut current = empty_box();
            let (mode, bounds, kept_model) = match item.kind {
                Kind::Insert => {
                    io.write_flags(model, element.flags | 8);
                    io.world_box(model, &mut current);
                    (u32::from(element.flags & 0x200 != 0), current, Some(model))
                }
                Kind::Remove => {
                    io.write_flags(model, element.flags & !8);
                    (2, item.old_box, None)
                }
                Kind::Change2 | Kind::Change3 => {
                    io.world_box(model, &mut current);
                    (if item.kind == Kind::Change2 { 0 } else { 3 }, current, Some(model))
                }
                Kind::Move | Kind::Reclassify => unreachable!(),
            };
            if !scene.full_redraw_pending() && backing_enabled {
                scene.add_dirty_box(mode, bounds, kept_model, 0, io, camera)?;
            }
            // Literal published rule: clear the queued bit when processed.
            // Precise intermediate flag timing is not established (RND7).
            let flags = io
                .element(model)
                .ok_or(Unsupported::Domain("element lost during queue processing"))?
                .flags;
            io.write_flags(model, flags & !0x10);
            if item.kind == Kind::Remove {
                self.items[0].model = None; // suppress recursive queue match
                if io.remove_grid_element(model) {
                    io.release(model);
                }
                io.clear_model_element(model);
            }
            self.items.remove(0);
            io.release(model);
        }
        Ok(())
    }
    pub fn clear_uninitialized(&mut self, refs: &mut dyn References) -> Result<()> {
        if self.processing {
            return Err(Unsupported::Reentry);
        }
        // This is explicit portable disposal, NOT the native Shutdown drain,
        // which also removes grid elements and is not implemented here.
        for item in self.items.drain(..) {
            if let Some(model) = item.model {
                refs.release(model);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsertedDecision {
    pub inserted: bool,
    pub process_queue: bool,
}

/// Published old/current-box decision AFTER a successful MoveGridElement.
/// This does not perform or claim the missing grid movement. Portable literal
/// f32 subtraction at the tolerance comparison is recorded in RND7.
pub fn moved_dirty_regions(old: Aabb, current: Aabb, flags: u16) -> Result<[(u32, Option<Aabb>); 2]> {
    let a = [old.min.x, old.min.y, old.min.z, old.max.x, old.max.y, old.max.z];
    let b = [
        current.min.x,
        current.min.y,
        current.min.z,
        current.max.x,
        current.max.y,
        current.max.z,
    ];
    if a.iter().chain(b.iter()).any(|v| !v.is_finite()) {
        return Err(Unsupported::Domain("nonfinite model box"));
    }
    let mode = u32::from(flags & 0x200 != 0);
    let separate = (0..3).any(|i| b[i] - a[i + 3] >= 0.0001 || a[i] - b[i + 3] >= 0.0001);
    if separate {
        return Ok([(mode, Some(old)), (mode, Some(current))]);
    }
    let bounds = Aabb {
        min: Vec3::new(
            old.min.x.min(current.min.x),
            old.min.y.min(current.min.y),
            old.min.z.min(current.min.z),
        ),
        max: Vec3::new(
            old.max.x.max(current.max.x),
            old.max.y.max(current.max.y),
            old.max.z.max(current.max.z),
        ),
    };
    Ok([(mode, Some(bounds)), (mode, None)])
}

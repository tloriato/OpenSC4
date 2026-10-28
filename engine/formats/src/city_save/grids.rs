// SPDX-License-Identifier: GPL-3.0-or-later
//! The factory inventory in simgrid.md §2. Read/Write belong to SimGrid<T>.
use fc4_core::grid::SimGrid;
use fc4_core::stream::SaveObject;
use std::rc::Rc;

pub(super) fn create(class: u32) -> Option<Rc<dyn SaveObject>> {
    Some(match class {
        0x49B9_E601 | 0x49B9_E602 => SimGrid::<u8>::new(),
        0x49B9_E603 => SimGrid::<i8>::new(),
        0x49B9_E604 => SimGrid::<u16>::new(),
        0x49B9_E605 => SimGrid::<i16>::new(),
        0x49B9_E606 => SimGrid::<u32>::new(),
        0x49B9_E607 => SimGrid::<i32>::new(),
        0x49B9_E608 => SimGrid::<u64>::new(),
        0x49B9_E609 => SimGrid::<i64>::new(),
        0x49B9_E60A => SimGrid::<f32>::new(),
        0x49B9_E60B => SimGrid::<f64>::new(),
        _ => return None,
    })
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::Object;
use crate::traffic::test_support::object;

/// Nested game vectors with independently allocated rows and padding between the row pointers.
pub struct Grid<T> {
    pub object: Object,
    pub cells: Vec<Vec<T>>,
    rows: Vec<[u32; 3]>,
    vector: Box<[u32; 3]>,
}
impl<T: Copy> Grid<T> {
    pub fn new(table: &[usize], id: u32, width: usize, height: usize, value: T) -> Self {
        let mut cells = vec![vec![value; height]; width];
        let mut rows: Vec<_> = cells
            .iter_mut()
            .map(|row| [row.as_mut_ptr() as u32, 0xABAB_ABAB, 0xCDCD_CDCD])
            .collect();
        let mut vector = Box::new([rows.as_mut_ptr() as u32, 0x1234_5678, 0x8765_4321]);
        let mut object = object(12, table);
        unsafe {
            store(object.ptr(), 4, vector.as_mut_ptr() as u32);
            store(object.ptr(), 8, id);
        }
        Self {
            object,
            cells,
            rows,
            vector,
        }
    }
    pub fn rows(&mut self) -> *mut u8 {
        self.rows.as_mut_ptr().cast()
    }
    pub fn padding_untouched(&self) {
        assert!(self.rows.iter().all(|r| r[1..] == [0xABAB_ABAB, 0xCDCD_CDCD]));
        assert_eq!(self.vector[1..], [0x1234_5678, 0x8765_4321]);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded typed fields shared by standalone owner codecs (foundations §2).

use super::ObjectReference;
use crate::stream::{ByteReader, InStream};

pub fn array<T: Copy + Default, const N: usize>(
    r: &mut dyn InStream,
    mut read: impl FnMut(&mut dyn InStream) -> Option<T>,
) -> Option<[T; N]> {
    let mut values = [T::default(); N];
    for value in &mut values {
        *value = read(r)?;
    }
    Some(values)
}

/// Bound allocation by the bytes still available, including each element's
/// shortest possible encoding (a null reference occupies four bytes).
pub fn count(r: &mut ByteReader<'_>, minimum_bytes: usize) -> Option<usize> {
    let n = r.get_u32()? as usize;
    (n <= r.remaining().checked_div(minimum_bytes)?).then_some(n)
}

pub fn references(r: &mut ByteReader<'_>) -> Option<Vec<ObjectReference>> {
    let n = count(r, 4)?;
    let mut values = Vec::with_capacity(n);
    for _ in 0..n {
        values.push(ObjectReference::read(r)?);
    }
    Some(values)
}

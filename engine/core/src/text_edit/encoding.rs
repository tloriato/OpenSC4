// SPDX-License-Identifier: GPL-3.0-or-later
//! The byte tables in text-edit-encoding.md, deliberately not UTF-8 validation.
use super::{Boundary, Result};

pub(super) fn lead(byte: u8, selector: i32) -> bool {
    match selector {
        932 => (0x81..=0x9f).contains(&byte) || (0xe0..=0xfc).contains(&byte),
        936 => (0xa1..=0xfc).contains(&byte),
        949 => (0xb0..=0xc8).contains(&byte),
        950 => (0x81..=0xfe).contains(&byte),
        _ => false,
    }
}
fn resolved(selector: i32) -> Result<i32> {
    match selector { -2 => Ok(8), -1 => Err(Boundary::Foreign), n => Ok(n) }
}
pub fn char_length(storage: &[u8], at: u32, logical_length: u32, selector: i32) -> Result<u32> {
    if logical_length == 0 { return Ok(0); }
    let selector = resolved(selector)?;
    if selector == 0 || selector == 16 { return Ok(1); }
    let b = *storage.get(at as usize).ok_or(Boundary::BufferRange)?;
    if selector != 8 { return Ok(if lead(b, selector) { 2 } else { 1 }); }
    let n = match b { 0xc0..=0xdf => 2, 0xe0..=0xef => 3, 0xf0..=0xf7 => 4,
        0xf8..=0xfb => 5, 0xfc..=0xfd => 6, _ => return Ok(1) };
    let next = *storage.get(at as usize + 1).ok_or(Boundary::BufferRange)?;
    Ok(if (0x80..=0xbf).contains(&next) { n } else { 1 })
}
pub fn previous_char_length(prefix: &[u8], selector: i32) -> Result<u32> {
    if prefix.is_empty() { return Ok(0); }
    let selector = resolved(selector)?;
    if selector == 0 || selector == 16 { return Ok(1); }
    if selector == 8 {
        return Ok(1 + prefix.iter().rev().take_while(|&&b| (0x80..=0xbf).contains(&b)).count() as u32);
    }
    let mut at = 0;
    let mut last = 1;
    while at < prefix.len() {
        last = if lead(prefix[at], selector) && at + 1 < prefix.len() { 2 } else { 1 };
        at += last;
    }
    Ok(last as u32)
}
pub fn glyph_count(bytes: &[u8], selector: i32) -> Result<u32> {
    if bytes.is_empty() { return Ok(0); }
    let selector = resolved(selector)?;
    if selector == 0 || selector == 16 { return Ok(bytes.len() as u32); }
    if selector == 8 { return Ok(bytes.iter().filter(|&&b| !(0x80..=0xbf).contains(&b)).count() as u32); }
    let mut at = 0; let mut count = 0;
    while at < bytes.len() { at += if lead(bytes[at], selector) { 2 } else { 1 }; count += 1; }
    Ok(count)
}
pub(super) fn input_lead(byte: u8, selector: i32) -> bool {
    if selector == 8 { (0xc0..=0xfd).contains(&byte) }
    else { selector >= 100 && lead(byte, selector) }
}
pub(super) fn trailing(byte: u8, selector: i32) -> u32 {
    if selector == 8 { match byte { 0xc2..=0xdf => 1, 0xe0..=0xef => 2,
        0xf0..=0xf7 => 3, 0xf8..=0xfb => 4, 0xfc..=0xfd => 5, _ => 0 } }
    else { u32::from(selector >= 100 && lead(byte, selector)) }
}

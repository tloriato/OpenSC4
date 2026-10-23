// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded raw-input transport to the existing production renderer, no pixel oracle.
use fc4_core::font_renderer::{self as renderer, Record, Rect, Target};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
impl From<RawRect> for Rect {
    fn from(r: RawRect) -> Self {
        Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}
#[repr(C)]
pub struct RawRecord {
    destination: RawRect,
    source: *const u8,
    source_len: u32,
    source_pitch: u32,
    source_width: i32,
    source_height: i32,
    character: u32,
}
#[repr(C)]
pub struct RawTarget {
    width: i32,
    height: i32,
    depth: u32,
    format: u32,
    format_depth: u32,
    pitch: u32,
    pixels: *mut u8,
    pixel_len: u32,
    lock_al: u8,
    unlock_al: u8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trace {
    slot: u32,
    a: u32,
    b: u32,
}
struct Host<'a> {
    raw: &'a RawTarget,
    pixels: &'a mut [u8],
    trace: Vec<Trace>,
}
impl Host<'_> {
    fn note(&mut self, slot: u32, a: u32, b: u32) {
        self.trace.push(Trace { slot, a, b });
    }
}
impl Target for Host<'_> {
    fn width(&mut self) -> i32 {
        self.note(0x24, self.raw.width as u32, 0);
        self.raw.width
    }
    fn height(&mut self) -> i32 {
        self.note(0x28, self.raw.height as u32, 0);
        self.raw.height
    }
    fn depth(&mut self) -> u32 {
        self.note(0x40, self.raw.depth, 0);
        self.raw.depth
    }
    fn format(&mut self, words: &mut [u32; 2]) -> u8 {
        *words = [self.raw.format, self.raw.format_depth];
        self.note(0x38, words[0], words[1]);
        0
    }
    fn pitch(&mut self) -> u32 {
        self.note(0x8c, self.raw.pitch, 0);
        self.raw.pitch
    }
    fn pixels(&mut self) -> &mut [u8] {
        self.note(0x88, 0, 0);
        self.pixels
    }
    fn lock(&mut self, flags: u32) -> u8 {
        self.note(0x18, flags, u32::from(self.raw.lock_al));
        self.raw.lock_al
    }
    fn unlock(&mut self, flags: u32) -> u8 {
        self.note(0x1c, flags, u32::from(self.raw.unlock_al));
        self.raw.unlock_al
    }
}
unsafe fn bytes<'a>(p: *const u8, len: u32) -> Option<&'a [u8]> {
    if len == 0 {
        Some(&[])
    } else if p.is_null() {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(p, len as usize) })
    }
}
/// # Safety
/// Nonempty pointers must cover their declared lengths; all input and output
/// ranges must be disjoint, except immutable shared inputs. No retention.
#[no_mangle]
pub unsafe extern "C" fn fc4_font_renderer_verify(
    operation: u32,
    target: *const RawTarget,
    records: *const RawRecord,
    count: u32,
    mode: u32,
    colours: *const u8,
    colour_len: u32,
    clip: *const RawRect,
    trace: *mut Trace,
    capacity: u32,
    trace_count: *mut u32,
    result: *mut u8,
) -> u32 {
    if trace_count.is_null() || result.is_null() || (capacity != 0 && trace.is_null()) {
        return 1;
    }
    // Bounds keep verification ephemeral and cannot be passed through to native code.
    if count > 16 || capacity > 128 || colour_len > 4096 {
        return 1;
    }
    let raw_records = if count == 0 {
        &[][..]
    } else {
        if records.is_null() {
            return 1;
        }
        unsafe { std::slice::from_raw_parts(records, count as usize) }
    };
    let mut typed = Vec::new();
    for r in raw_records {
        if r.source_len > 4096 {
            return 1;
        }
        let Some(source) = (unsafe { bytes(r.source, r.source_len) }) else {
            return 1;
        };
        typed.push(Record {
            destination: r.destination.into(),
            source,
            source_pitch: r.source_pitch,
            source_width: r.source_width,
            source_height: r.source_height,
            character: r.character,
        });
    }
    let Some(colours) = (unsafe { bytes(colours, colour_len) }) else {
        return 1;
    };
    let mut host = if target.is_null() {
        None
    } else {
        let r = unsafe { &*target };
        if r.pixel_len > 4096 || (r.pixel_len != 0 && r.pixels.is_null()) {
            return 1;
        }
        let pixels = if r.pixel_len == 0 {
            &mut [][..]
        } else {
            unsafe { std::slice::from_raw_parts_mut(r.pixels, r.pixel_len as usize) }
        };
        Some(Host {
            raw: r,
            pixels,
            trace: Vec::new(),
        })
    };
    let mut events = Vec::new();
    let value = match operation {
        0 => renderer::draw(
            host.as_mut().map(|h| h as &mut dyn Target),
            &typed,
            count,
            mode,
            colours,
            unsafe { clip.as_ref() }.copied().map(Into::into),
            &mut events,
        )
        .map(u8::from),
        1 => host
            .as_mut()
            .map(|h| renderer::lock(h))
            .ok_or(renderer::Refusal::Target),
        2 => host
            .as_mut()
            .map(|h| renderer::unlock(h))
            .ok_or(renderer::Refusal::Target),
        _ => return 1,
    };
    let empty = Vec::new();
    let notes = host.as_ref().map_or(&empty, |h| &h.trace);
    unsafe {
        *trace_count = notes.len() as u32;
    }
    if notes.len() > capacity as usize {
        return 2;
    }
    if !notes.is_empty() {
        unsafe {
            std::ptr::copy_nonoverlapping(notes.as_ptr(), trace, notes.len());
        }
    }
    match value {
        Ok(v) => {
            unsafe {
                *result = v;
            }
            0
        }
        Err(_) => 1,
    }
}

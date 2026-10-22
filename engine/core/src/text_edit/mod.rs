// SPDX-License-Identifier: GPL-3.0-or-later
//! Portable cGZWinTextEdit owner, from docs/specs/text-edit*.md only.
//!
//! Byte offsets and wrapping words are intentional. `Boundary` describes an
//! unsupported native call domain, NEVER a native false/no-op guarantee. An
//! error may follow a deletion or another observable effect; no rollback is
//! implied. TE5 conversion/composed-length and TE8 allocation/unwind remain
//! foreign. This module does not implement a GZCOM replacement or claim native
//! equivalence. No interior mutability is used by state getters.

mod encoding;
mod layout;
mod input;
mod paint;
mod resources;

pub use encoding::{char_length, glyph_count, previous_char_length};
pub use crate::fonts::layout::Rect;
pub use crate::ui_windows::win::WinToken;

pub type Result<T> = std::result::Result<T, Boundary>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    BufferRange,
    MissingFont,
    ZeroDivisor,
    NonProgress,
    LoweredOrdinaryLimit { wrapped_count: u32 },
    Foreign,
    InvalidIndex,
    ArithmeticDomain,
}

/// The exact call-site font/encoding primitives, not editor decisions. Existing
/// GzFont/CallbackTimer/WinStore adapters can supply these without transferring
/// layout, text, history or navigation ownership to a host.
pub trait Services {
    fn has_font(&self) -> bool;
    fn line_height(&mut self) -> Result<u32>;
    fn measure(&mut self, bytes: &[u8]) -> Result<u32>;
    /// Font +0xA4(bytes, remaining, width, 0, 2).
    fn fit(&mut self, bytes: &[u8], width: u32) -> Result<u32>;
    fn previous_utf16(&mut self, context: &[u8], index: u32, selector: i32) -> Result<u32>;
    /// Result is (code value, consumed byte count), not a replacement string.
    fn next_utf16(&mut self, bytes: &[u8], selector: i32) -> Result<(u32, u32)>;
    fn thai_level(&mut self, current: u16, previous: u16) -> Result<u32>;
    fn composed_length(&mut self, bytes: &[u8], index: u32, previous: bool, selector: i32)
        -> Result<u32>;
    /// Only service outputs: resource identity, clipboard bytes, base result,
    /// geometry, colors and timer outcomes. No final editor decision is delegated.
    fn call(&mut self, call: Call) -> Result<Reply>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Invalidate,
    Notify { id: u32, a: u32, b: u32, c: u32 },
    Base(&'static str, Vec<u32>),
    KeyState(u32),
    Capture(bool),
    Cursor(Option<u32>),
    Timer { caret: bool, start: bool, interval: u32, data: WinToken },
    Post(u32),
    ClipboardCopy(Vec<u8>, u32, u32),
    ClipboardPaste(u32),
    CreateBar { vertical: bool, args: [u32; 5] },
    BarWindow(u64),
    AddChild(u64),
    RemoveChild(u64),
    AddRef(u64),
    Release(u64),
    ImageRect(u64),
    BarImage { bar: u64, image: u64, rect: Rect },
    BarRect { bar: u64, rect: Rect },
    BarRange { bar: u64, max: u32 },
    BarPair { bar: u64, max: u32, step: u32 },
    BarValue { bar: u64, value: u32 },
    BarGetValue(u64),
    WindowSize(u64),
    LookupFont(u64),
    CodePage,
    AcquireClipboard,
    Color { slot: u32, value: u32 },
    Search { needle: Vec<u8>, position: u32, forward: bool, text: Vec<u8> },
    SoundAcquire { reason: u32, id: u32, args: [u32; 7] },
    SoundPlay(u64),
    Fill { rect: Rect, color: u32 },
    Draw { bytes: Vec<u8>, x: i32, y: i32, foreground: u32, background: u32, clip: Rect },
    Background { image: u64, edge: bool, rect: Rect, thirds: (u32, u32) },
    PenWidth(f32),
    Caret { x: i32, top: i32, bottom: i32, color: u32 },
    Outline { rect: Rect, color: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Unit,
    Bool(bool),
    Word(u32),
    Handle(Option<u64>),
    Bytes(Option<Vec<u8>>),
    Rectangle(Rect),
    Size(u32, u32),
}

impl Reply {
    fn boolean(self) -> Result<bool> {
        if let Self::Bool(v) = self { Ok(v) } else { Err(Boundary::Foreign) }
    }
    fn word(self) -> Result<u32> {
        if let Self::Word(v) = self { Ok(v) } else { Err(Boundary::Foreign) }
    }
    fn handle(self) -> Result<Option<u64>> {
        if let Self::Handle(v) = self { Ok(v) } else { Err(Boundary::Foreign) }
    }
}

/// The five saved words in each native action, including cached coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub index: u32,
    pub anchor: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Action {
    at: u32,
    remove: Vec<u8>,
    insert: Vec<u8>,
    saved: Position,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bar {
    pub handle: Option<u64>,
    pub image: Option<u64>,
    pub image_rect: Rect,
}

#[derive(Clone, Debug)]
pub struct TextEdit {
    text: Vec<u8>,
    lines: Vec<u32>,
    position: Position,
    flags: u32,
    selector: i32,
    maximum: u32,
    undo_limit: u32,
    undo: Vec<Action>,
    redo: Vec<Action>,
    pending: Vec<u8>,
    pub(crate) gutter_x: u32,
    pub(crate) gutter_y: u32,
    width: u32,
    height: u32,
    visible_width: u32,
    visible_height: u32,
    visible_rows: u32,
    line_height: u32,
    first_visible: u32,
    horizontal_byte: u32,
    horizontal_pixel: u32,
    preferred_x: u32,
    initialized: bool,
    enabled: bool,
    focused: bool,
    capturing: bool,
    caret_visible: bool,
    caret_running: bool,
    scroll_running: bool,
    caret_period: u32,
    double_interval: u32,
    double_running: bool,
    mask: u8,
    token: WinToken,
    font: Option<u64>,
    font_style: Option<u64>,
    clipboard: Option<u64>,
    background: Option<u64>,
    background_option: u8,
    vertical: Bar,
    horizontal: Bar,
    colors: [u32; 6],
    fill_color: u32,
    caret_color: u32,
    highlight_fill: u32,
    outline_color: u32,
    sound_ids: [u32; 4],
    auto_depth: u32,
}

impl TextEdit {
    /// Constructor fields published in section 1. Other native uninitialized
    /// words are represented as zero only as portable pre-init storage; methods
    /// requiring initialized layout retain explicit domain checks.
    pub fn new(flags: u32, selector: i32, token: WinToken) -> Self {
        Self {
            text: Vec::new(), lines: Vec::new(), position: Position::default(),
            flags: Self::normalize_flags(flags), selector, maximum: u32::MAX,
            undo_limit: 0, undo: Vec::new(), redo: Vec::new(), pending: Vec::new(),
            gutter_x: 5, gutter_y: 0, width: 0, height: 0,
            visible_width: 0, visible_height: 0, visible_rows: 0, line_height: 0,
            first_visible: 0, horizontal_byte: 0, horizontal_pixel: 0,
            preferred_x: u32::MAX, initialized: false, enabled: true,
            focused: false, capturing: false, caret_visible: false,
            caret_running: false, scroll_running: false, caret_period: 1000,
            double_interval: 500, double_running: false, mask: b'*', token,
            font: None, font_style: None, clipboard: None, background: None,
            background_option: 0, vertical: Bar::default(), horizontal: Bar::default(),
            colors: [0xff000000, 0x40000000, 0xff000000, 0x40000000, 0xffffffff, 0x40000000],
            fill_color: 0xffffffff, caret_color: 0, highlight_fill: 0, outline_color: 0,
            sound_ids: [0; 4], auto_depth: 0,
        }
    }

    pub fn text(&self) -> &[u8] { &self.text }
    pub fn text_length(&self) -> u32 { self.text.len() as u32 }
    pub fn line_starts(&self) -> &[u32] { &self.lines }
    pub fn position(&self) -> Position { self.position }
    pub fn flags(&self) -> u32 { self.flags }
    pub fn is_flag_set(&self, mask: u32) -> bool { self.flags & mask != 0 }
    pub fn maximum(&self) -> u32 { self.maximum }
    pub fn set_maximum(&mut self, maximum: u32) { self.maximum = maximum; }
    pub fn set_max_undo_levels(&mut self, maximum: u32) { self.undo_limit = maximum; }
    pub fn history_counts(&self) -> (usize, usize) { (self.undo.len(), self.redo.len()) }
    pub fn insert_mode(&self) -> bool { self.flags & 0x4000 == 0 }
    pub fn set_insert_mode(&mut self, insert: bool) -> bool {
        if insert { self.flags &= !0x4000; } else { self.flags |= 0x4000; }
        true
    }
    pub fn pending_input(&self) -> &[u8] { &self.pending }
    pub fn caret_visible(&self) -> bool { self.caret_visible }
    pub fn preferred_x(&self) -> u32 { self.preferred_x }
    pub fn set_focus_state(&mut self, focus: bool) { self.focused = focus; }
    pub fn set_capture_state(&mut self, capture: bool) { self.capturing = capture; }
    pub fn set_mask(&mut self, byte: u8) { self.mask = byte; }
    pub fn copy_text(&self, destination: &mut [u8], count: &mut u32) -> Result<()> {
        *count = (*count).min(self.text_length());
        let out = destination.get_mut(..*count as usize).ok_or(Boundary::BufferRange)?;
        out.copy_from_slice(&self.text[..*count as usize]);
        Ok(())
    }
    fn span(bytes: &[u8], start: u32, count: u32) -> Result<&[u8]> {
        let end = start.checked_add(count).ok_or(Boundary::BufferRange)?;
        bytes.get(start as usize..end as usize).ok_or(Boundary::BufferRange)
    }
    fn invalidate(&self, s: &mut impl Services) -> Result<()> {
        s.call(Call::Invalidate)?; Ok(())
    }
    fn notify(&self, s: &mut impl Services, id: u32, b: u32, c: u32) -> Result<bool> {
        s.call(Call::Notify { id, a: self.token as u32, b, c })?.boolean()
    }
    fn changed(&self, s: &mut impl Services) -> Result<()> {
        self.notify(s, 0x9b1552d8, 0, 0)?; Ok(())
    }
    fn reason(&self, s: &mut impl Services, reason: u32) -> Result<bool> {
        let id = self.sound_ids[reason as usize];
        if id == 0 { return Ok(false); }
        let acquired = s.call(Call::SoundAcquire { reason, id,
            args: [5, 0x5b523ba4, 0, id, 0, 0, 0] })?.handle()?;
        if let Some(handle) = acquired {
            let result = s.call(Call::SoundPlay(handle));
            s.call(Call::Release(handle))?;
            result?.boolean()
        } else { Ok(false) }
    }
    pub fn set_sound_id(&mut self, index: i32, id: u32) -> Result<bool> {
        if index >= 4 { return Ok(false); }
        let field = self.sound_ids.get_mut(index as usize).ok_or(Boundary::InvalidIndex)?;
        *field = id; Ok(true)
    }

    /// String-overload filtering. The TE8 _strncpy padding primitive is not
    /// simulated: only the resized string's retained chunks are observable here.
    pub fn remove_bad_characters(&self, bytes: &mut Vec<u8>) -> Result<u32> {
        if self.flags & 0x200 == 0 { return Ok(0); }
        let mut keep = Vec::new();
        let mut at = 0;
        while at < bytes.len() as u32 {
            let n = char_length(bytes, at, bytes.len() as u32 - at, self.selector)?;
            if n == 0 { return Err(Boundary::NonProgress); }
            let chunk = Self::span(bytes, at, n)?;
            if chunk[0] != b'\n' { keep.extend_from_slice(chunk); }
            at = at.wrapping_add(n);
        }
        let removed = (bytes.len() - keep.len()) as u32;
        *bytes = keep;
        Ok(removed)
    }

    pub fn fix_composition(&self, candidate: &mut Vec<u8>, context: &[u8], index: u32,
        s: &mut impl Services) -> Result<u32> {
        let mut previous = s.previous_utf16(context, index, self.selector)?;
        let mut at = 0usize;
        let mut removed = 0;
        while at < candidate.len() {
            let (current, n) = s.next_utf16(&candidate[at..], self.selector)?;
            if n == 0 {
                removed += (candidate.len() - at) as u32;
                candidate.truncate(at); break;
            }
            let end = at.checked_add(n as usize).ok_or(Boundary::BufferRange)?;
            if end > candidate.len() { return Err(Boundary::BufferRange); }
            if s.thai_level(current as u16, previous as u16)? == 0 {
                candidate.drain(at..end); removed += n;
            } else { at = end; previous = current; }
        }
        Ok(removed)
    }

    fn prepare(&self, bytes: &[u8], context: &[u8], at: u32, s: &mut impl Services)
        -> Result<Vec<u8>> {
        let mut candidate = bytes.to_vec();
        let mut corrected = self.remove_bad_characters(&mut candidate)?;
        corrected = corrected.wrapping_add(self.fix_composition(&mut candidate, context, at, s)?);
        if corrected != 0 { self.reason(s, 0)?; }
        Ok(candidate)
    }

    /// count=0 means strlen; None is an empty C string irrespective of count.
    pub fn set_text(&mut self, source: Option<&[u8]>, count: u32, notify: bool,
        s: &mut impl Services) -> Result<bool> {
        let bytes = match source {
            None => &[][..],
            Some(src) if count == 0 => &src[..src.iter().position(|&b| b == 0).unwrap_or(src.len())],
            Some(src) => Self::span(src, 0, count)?,
        };
        self.text.clear(); // Filtering/composition observe the already-empty destination.
        let mut candidate = self.prepare(bytes, &self.text, 0, s)?;
        if candidate.len() as u32 > self.maximum {
            let mut retained = self.maximum;
            if self.selector == 8 || self.selector == 100 {
                retained = 0;
                while retained < self.maximum {
                    let n = char_length(bytes, retained, (bytes.len() as u32).wrapping_sub(retained), self.selector)?;
                    if n == 0 { return Err(Boundary::NonProgress); }
                    retained = retained.wrapping_add(n);
                }
            }
            Self::span(&candidate, 0, retained)?;
            candidate.truncate(retained as usize);
            self.reason(s, 1)?;
        }
        if candidate.is_empty() {
            self.recalculate(s)?;
        } else {
            self.text = candidate;
            self.calculate_lines(s)?;
            self.bar_values(true, s)?;
            self.position = Position::default();
            self.check_automatic_bars(s)?;
        }
        self.invalidate(s)?;
        self.undo.clear(); self.redo.clear();
        if notify { self.changed(s)?; }
        Ok(true)
    }

    pub fn set_selection(&mut self, anchor: u32, end: u32, s: &mut impl Services) -> Result<bool> {
        self.position.anchor = anchor.min(self.text_length());
        self.position.end = end.min(self.text_length());
        self.position.index = self.position.end;
        self.update_coordinates()?;
        self.invalidate(s)?; Ok(true)
    }
    pub fn set_insertion_index(&mut self, index: u32, s: &mut impl Services) -> Result<bool> {
        self.position.index = index.min(self.text_length());
        self.update_coordinates()?;
        self.invalidate(s)?; Ok(true)
    }
    pub fn fix_insertion(&mut self) -> Result<()> {
        if self.text.is_empty() { self.position = Position::default(); }
        else {
            self.position.index = self.position.index.min(self.text_length());
            self.update_coordinates()?;
            self.position.end = self.position.end.min(self.text_length());
        }
        Ok(())
    }

    pub fn insert_raw(&mut self, bytes: &[u8], index: u32, s: &mut impl Services) -> Result<u32> {
        let index = index.min(self.text_length());
        let mut candidate = self.prepare(bytes, &self.text, index, s)?;
        if candidate.is_empty() { return Ok(0); }
        let combined = self.text_length().checked_add(candidate.len() as u32)
            .ok_or(Boundary::ArithmeticDomain)?;
        if combined > self.maximum {
            let retained = if self.selector == 8 || self.selector == 100 {
                let mut n = 0u32;
                while self.text_length().wrapping_add(n) < self.maximum && n < candidate.len() as u32 {
                    let step = char_length(&candidate, n, candidate.len() as u32 - n, self.selector)?;
                    if step == 0 { return Err(Boundary::NonProgress); }
                    n = n.wrapping_add(step);
                }
                n
            } else {
                let n = self.maximum.wrapping_sub(self.text_length());
                if self.text_length() > self.maximum {
                    return Err(Boundary::LoweredOrdinaryLimit { wrapped_count: n });
                }
                n
            };
            Self::span(&candidate, 0, retained)?;
            candidate.truncate(retained as usize);
            self.reason(s, 1)?;
        }
        let retained = candidate.len() as u32;
        let movement = glyph_count(&candidate, self.selector)?;
        self.text.splice(index as usize..index as usize, candidate);
        self.position.index = index.wrapping_add(movement);
        if self.initialized { self.repair_after_mutation(s)?; }
        Ok(retained)
    }

    pub fn remove_raw(&mut self, start: u32, count: u32, output: Option<&mut Vec<u8>>,
        s: &mut impl Services) -> Result<u32> {
        if start >= self.text_length() { return Ok(0); }
        let n = count.min(self.text_length() - start);
        let removed = Self::span(&self.text, start, n)?;
        if let Some(out) = output { *out = removed.to_vec(); }
        self.text.drain(start as usize..start as usize + n as usize);
        self.position.index = start;
        self.repair_after_mutation(s)?;
        Ok(n)
    }
    pub fn remove_char(&mut self, index: u32, ordinary: bool, output: Option<&mut Vec<u8>>,
        s: &mut impl Services) -> Result<bool> {
        if self.text.is_empty() { return Err(Boundary::InvalidIndex); }
        let index = if index >= self.text_length() { self.text_length() - 1 } else { index };
        let n = if ordinary { char_length(&self.text, index, self.text_length() - index, self.selector)? }
            else { s.composed_length(&self.text, index, false, self.selector)? };
        // Invalid helper spans are boundaries, not silently shortened RemoveChar.
        Self::span(&self.text, index, n)?;
        self.remove_raw(index, n, output, s)?; Ok(true)
    }

    fn record(&mut self, action: Action, insertion: bool) {
        if self.flags & 0x100 == 0 { return; }
        if insertion && self.undo.len() as u32 >= self.undo_limit {
            let quarter = self.undo.len() >> 2;
            self.undo.drain(..quarter);
        }
        self.undo.push(action); self.redo.clear();
    }
    pub fn insert_high_level(&mut self, bytes: &[u8], s: &mut impl Services) -> Result<bool> {
        if self.flags & 0x200 != 0 && bytes.first() == Some(&b'\n') {
            self.collapse(); self.invalidate(s)?; return Ok(true);
        }
        let saved = self.position;
        let mut removed = Vec::new();
        if self.position.anchor != self.position.end {
            let (a, b) = self.selection_range();
            self.remove_raw(a, b.wrapping_sub(a), Some(&mut removed), s)?;
        }
        if self.flags & 0x4000 != 0 && removed.is_empty() && self.position.index < self.text_length() {
            self.remove_char(self.position.index, false, Some(&mut removed), s)?;
        }
        let at = self.position.index;
        let inserted = self.insert_raw(bytes, at, s)?;
        if inserted == 0 { self.reason(s, 1)?; }
        else {
            self.record(Action { at, remove: Self::span(&self.text, at, inserted)?.to_vec(),
                insert: removed, saved }, true);
        }
        self.preferred_x = u32::MAX;
        self.collapse(); self.put_in_view(s)?;
        if self.flags & 0x800 != 0 { self.changed(s)?; }
        Ok(true)
    }
    fn selection_range(&self) -> (u32, u32) {
        (self.position.anchor.min(self.position.end), self.position.anchor.max(self.position.end))
    }
    fn collapse(&mut self) { self.position.anchor = self.position.index; self.position.end = self.position.index; }
    fn repair_after_mutation(&mut self, s: &mut impl Services) -> Result<()> {
        self.calculate_lines(s)?; self.update_coordinates()?;
        self.put_in_view(s)?; self.bar_values(true, s)?;
        self.check_automatic_bars(s)?; self.invalidate(s)
    }
    fn replay(&mut self, redo: bool, s: &mut impl Services) -> Result<bool> {
        let source = if redo { &mut self.redo } else { &mut self.undo };
        if self.flags & 0x100 == 0 || source.is_empty() { return Ok(false); }
        let action = source.last().ok_or(Boundary::InvalidIndex)?.clone();
        Self::span(&self.text, action.at, action.remove.len() as u32)?;
        let inverse = Action { at: action.at, remove: action.insert.clone(),
            insert: action.remove.clone(), saved: self.position };
        // Store inverse before raw replay; no filter, coalescing, notify or eviction.
        if redo { self.undo.push(inverse); } else { self.redo.push(inverse); }
        self.text.splice(action.at as usize..action.at as usize + action.remove.len(), action.insert);
        self.position = action.saved;
        if redo { self.redo.pop(); } else { self.undo.pop(); }
        self.calculate_lines(s)?; self.put_in_view(s)?; self.invalidate(s)?;
        Ok(true)
    }
    pub fn undo(&mut self, s: &mut impl Services) -> Result<bool> { self.replay(false, s) }
    pub fn redo(&mut self, s: &mut impl Services) -> Result<bool> { self.replay(true, s) }
}

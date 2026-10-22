// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

impl TextEdit {
    pub fn character(&mut self, mut byte: u8, s: &mut impl Services) -> Result<bool> {
        s.call(Call::KeyState(0x11))?; s.call(Call::KeyState(0x12))?;
        if self.flags & 1 == 0 { return Ok(false); }
        if byte == b'\r' { byte = b'\n'; }
        if byte < 0x20 && byte != b'\n' { return Ok(true); }
        if self.selector == 8 || self.selector >= 100 {
            if self.pending.is_empty() && encoding::input_lead(byte, self.selector) {
                self.pending.push(byte); return Ok(true);
            }
            self.pending.push(byte);
            let needed = 1 + encoding::trailing(self.pending[0], self.selector);
            if (self.pending.len() as u32) < needed { return Ok(true); }
        } else { self.pending.clear(); self.pending.push(byte); }
        if !self.capturing { self.insert_high_level(&self.pending.clone(), s)?; }
        self.pending.clear(); Ok(false)
    }
    fn finish_navigation(&mut self, shift: bool, reset_x: bool, s: &mut impl Services) -> Result<()> {
        self.position.end = self.position.index;
        if !shift { self.position.anchor = self.position.index; }
        self.put_in_view(s)?; self.make_caret_visible(s)?; self.invalidate(s)?;
        if reset_x { self.preferred_x = u32::MAX; }
        Ok(())
    }
    fn copy_allowed(&self, modifiers: u32) -> bool {
        modifiers & 7 == 2 && self.flags & 0x1000 != 0 && self.flags & 0x2000 == 0 && !self.capturing
    }
    fn copy_selection(&self, s: &mut impl Services) -> Result<()> {
        if self.clipboard.is_some() && self.position.anchor != self.position.end {
            let (a, b) = self.selection_range();
            s.call(Call::ClipboardCopy(Self::span(&self.text, a, b.wrapping_sub(a))?.to_vec(), 1, 0))?;
        }
        Ok(())
    }
    fn delete_key(&mut self, backspace: bool, s: &mut impl Services) -> Result<()> {
        if self.flags & 1 == 0 { return Ok(()); }
        let saved = self.position; let mut removed = Vec::new();
        let success = if self.position.anchor != self.position.end {
            let (a, b) = self.selection_range();
            self.remove_raw(a, b.wrapping_sub(a), Some(&mut removed), s)? != 0
        } else if backspace && self.position.index > 0 {
            let n = previous_char_length(Self::span(&self.text, 0, self.position.index)?, self.selector)?;
            let at = self.position.index.wrapping_sub(n);
            self.remove_char(at, true, Some(&mut removed), s)?
        } else if !backspace && self.position.index < self.text_length() {
            self.remove_char(self.position.index, false, Some(&mut removed), s)?
        } else { false };
        if success {
            self.record(Action { at: self.position.index, remove: Vec::new(), insert: removed, saved }, false);
        } else { self.reason(s, 1)?; }
        self.collapse();
        if self.flags & 0x800 != 0 { self.changed(s)?; }
        Ok(())
    }
    pub fn key_down(&mut self, key: u32, modifiers: u32, s: &mut impl Services) -> Result<bool> {
        let shift = modifiers & 1 != 0; let control = modifiers & 2 != 0; let alt = modifiers & 4 != 0;
        match key {
            0x43 | 0x58 if self.copy_allowed(modifiers) => {
                self.copy_selection(s)?;
                if key == 0x58 { self.key_down(0x2e, 0, s)?; }
            }
            0x56 if control && !shift && !alt && !self.capturing && self.flags & 0x1000 != 0 => {
                if self.clipboard.is_some() {
                    match s.call(Call::ClipboardPaste(0))? {
                        Reply::Bytes(Some(bytes)) => { self.insert_high_level(&bytes, s)?; }
                        Reply::Bytes(None) => (), _ => return Err(Boundary::Foreign),
                    }
                }
            }
            0x5a | 0x59 if control && !alt => {
                if if key == 0x5a { self.undo.is_empty() } else { self.redo.is_empty() } {
                    self.reason(s, 3)?;
                } else if key == 0x5a { self.undo(s)?; } else { self.redo(s)?; }
            }
            0x2d => { if self.flags & 0x81 == 0x81 { self.flags ^= 0x4000; } }
            0x2e => self.delete_key(false, s)?,
            0x08 => self.delete_key(true, s)?,
            0x25 | 0x27 if self.flags & 1 != 0 => {
                if alt {
                    self.set_insertion_line_column(self.position.line,
                        if key == 0x25 { 0 } else { self.line_bytes(self.position.line)? })?;
                    if key == 0x27 { self.increment_position(1, true, s)?; }
                } else if control { self.increment_word(if key == 0x25 { -1 } else { 1 })?; }
                else { self.increment_position(if key == 0x25 { -1 } else { 1 }, true, s)?; }
                self.finish_navigation(shift, true, s)?;
            }
            0x24 | 0x23 if self.flags & 1 != 0 => {
                let end = key == 0x23;
                if control {
                    self.position.index = if end { self.text_length() } else { 0 }; self.update_coordinates()?;
                } else { self.set_insertion_line_column(self.position.line,
                    if end { self.line_bytes(self.position.line)? } else { 0 })?; }
                if end { self.correct_preceding_lf(); }
                self.finish_navigation(shift, true, s)?;
            }
            0x21 | 0x22 | 0x26 | 0x28 => {
                let page = key == 0x21 || key == 0x22;
                let down = key == 0x22 || key == 0x28;
                if self.flags & 1 == 0 || self.flags & 0x200 != 0 {
                    if !page && self.flags & 0x800 != 0 { self.notify(s, 0x9b1552d5, key, 0)?; }
                    return Ok(true);
                }
                if self.lines.is_empty() { self.reason(s, 2)?; return Ok(true); }
                let last = self.line_count() - 1;
                if (down && self.position.line == last) || (!down && self.position.line == 0) {
                    if self.position.anchor == self.position.end { self.reason(s, 2)?; }
                    else { self.finish_navigation(shift, false, s)?; }
                    return Ok(true);
                }
                let (x, _) = self.position_from_line_column(self.position.line, self.position.column, s)?;
                if self.preferred_x == u32::MAX { self.preferred_x = x as u32; }
                let destination_x = if page { x } else { self.preferred_x as i32 };
                let distance = if page { self.visible_rows } else { 1 };
                let line = if down {
                    if self.position.line < self.line_count().wrapping_sub(distance) {
                        self.position.line.wrapping_add(distance)
                    } else { last }
                } else if self.position.line < distance { 0 }
                else { self.position.line.wrapping_sub(distance) };
                let column = self.column_from_position(line, destination_x, s)?;
                self.position.line = line; self.position.column = column;
                self.position.index = self.index_from_line_column(line, column).1;
                self.correct_preceding_lf(); self.finish_navigation(shift, false, s)?;
            }
            0x1b => {
                if self.flags & 1 != 0 {
                    if self.position.anchor != self.position.end { self.collapse(); self.invalidate(s)?; }
                    return s.call(Call::Notify { id: 5, a: 0x1b, b: modifiers, c: 0 })?.boolean();
                }
                if self.copy_allowed(modifiers) { self.copy_selection(s)?; }
            }
            0x0d => { if self.flags & 0x400 != 0 { self.notify(s, 0x9b1552d5, 13, 0)?; } }
            _ => (),
        }
        Ok(true)
    }
    pub fn key_up(&self, key: u32, modifiers: u32, s: &mut impl Services) -> Result<bool> {
        if key != 0x1b { return Ok(true); }
        s.call(Call::Notify { id: 6, a: key, b: modifiers, c: 0 })?.boolean()
    }
    pub fn wheel(&mut self, delta: i32, s: &mut impl Services) -> Result<bool> {
        if !self.capturing {
            for _ in 0..(delta / 100).unsigned_abs() {
                self.scroll(if delta > 0 { Scroll::LineUp } else { Scroll::LineDown }, s)?;
            }
        }
        Ok(true)
    }
    pub fn focus(&mut self, gained: bool, s: &mut impl Services) -> Result<()> {
        self.focused = gained; self.invalidate(s)?;
        if self.flags & if gained { 0x20000 } else { 0x8000 } != 0 {
            self.notify(s, if gained { 0x9b1552da } else { 0x9b1552d9 }, 0, 0)?;
        }
        if gained { s.call(Call::Base("OnSetFocus", Vec::new()))?; } Ok(())
    }
    fn mouse_insertion(&mut self, x: i32, y: i32, s: &mut impl Services) -> Result<()> {
        let (line, column) = self.line_column_from_position(x, y, s)?;
        self.position.line = line; self.position.column = column;
        self.position.index = self.index_from_line_column(line, column).1;
        self.correct_preceding_lf(); Ok(())
    }
    /// elapsed is the external elapsed-timer reading; no spatial double-click gate.
    pub fn mouse_down_left(&mut self, x: i32, y: i32, modifiers: u32, elapsed: u32,
        s: &mut impl Services) -> Result<bool> {
        if self.flags & 1 == 0 { self.notify(s, 0x9b1552d6, x as u32, y as u32)?; return Ok(true); }
        let double = self.double_running && elapsed < self.double_interval;
        self.double_running = !double;
        s.call(Call::Base(if double { "ResetElapsed" } else { "RestartElapsed" }, Vec::new()))?;
        s.call(Call::Capture(true))?; self.capturing = true;
        self.mouse_insertion(x, y, s)?; self.position.end = self.position.index;
        if modifiers & 1 == 0 { self.position.anchor = self.position.index; }
        if double { self.select_current_word(s)?; }
        self.invalidate(s)?; self.preferred_x = u32::MAX; Ok(true)
    }
    pub fn mouse_move(&mut self, x: i32, y: i32, s: &mut impl Services) -> Result<bool> {
        if self.flags & 1 != 0 {
            if self.capturing {
                self.start_scroll_timer(true, s)?;
                let previous = self.position.end;
                self.mouse_insertion(x, y, s)?; self.position.end = self.position.index;
                if previous != self.position.end { self.invalidate(s)?; }
            } else { self.start_scroll_timer(false, s)?; }
        }
        Ok(true)
    }
    pub fn mouse_up_left(&mut self, s: &mut impl Services) -> Result<bool> {
        if self.flags & 1 != 0 && self.capturing {
            s.call(Call::Capture(false))?; self.capturing = false; self.start_scroll_timer(false, s)?;
        }
        Ok(true)
    }
    pub fn mouse_down_right(&self, x: i32, y: i32, s: &mut impl Services) -> Result<bool> {
        if !self.capturing { self.notify(s, 0x9b1552d7, x as u32, y as u32)?; } Ok(true)
    }
    pub fn cursor_scroll(&mut self, x: i32, y: i32, s: &mut impl Services) -> Result<()> {
        if !self.capturing { return Ok(()); }
        let (direction, target_x, target_y) = if y < -2 { (Scroll::LineUp, x, (self.line_height >> 1) as i32) }
        else if y >= self.height.wrapping_add(2) as i32 {
            (Scroll::LineDown, x, self.height.wrapping_sub(self.line_height >> 1) as i32)
        } else if x < -2 { (Scroll::Left, 0, y) }
        else if x >= self.width.wrapping_add(2) as i32 { (Scroll::Right, self.width as i32, y) }
        else { return Ok(()); };
        if self.scroll(direction, s)? { self.mouse_move(target_x, target_y, s)?; } Ok(())
    }
    pub fn message(&mut self, kind: u32, secondary: u32, orientation: u32,
        cursor: (i32, i32), s: &mut impl Services) -> Result<bool> {
        if kind == 3 && secondary == 0x887113a3 && orientation <= 1 {
            let bar = self.bar(orientation == 0).handle;
            if let Some(bar) = bar {
                let value = s.call(Call::BarGetValue(bar))?.word()?;
                let field = if orientation == 0 { &mut self.first_visible } else { &mut self.horizontal_pixel };
                if *field != value { *field = value; self.invalidate(s)?; }
            }
            return Ok(true);
        }
        match kind {
            0x631b56cc => self.toggle_caret(s)?,
            0x631b56cd => self.cursor_scroll(cursor.0, cursor.1, s)?,
            _ => return s.call(Call::Base("DoMessage", vec![kind, secondary, orientation]))?.boolean(),
        }
        Ok(true)
    }
}

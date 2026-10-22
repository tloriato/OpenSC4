// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scroll {
    LineUp,
    LineDown,
    PageUp,
    PageDown,
    Start,
    End,
    Left,
    Right,
}

impl TextEdit {
    pub fn line_count(&self) -> u32 {
        self.lines.len() as u32
    }
    pub fn first_visible_line(&self) -> u32 {
        self.first_visible
    }
    pub fn line_height(&self) -> u32 {
        self.line_height
    }
    pub fn visible_size(&self) -> (u32, u32, u32) {
        (self.visible_width, self.visible_height, self.visible_rows)
    }
    pub fn scroll_offsets(&self) -> (u32, u32) {
        (self.horizontal_byte, self.horizontal_pixel)
    }
    pub fn text_area(&self) -> Rect {
        Rect {
            left: self.gutter_x as i32,
            top: self.gutter_y as i32,
            right: self.gutter_x.wrapping_add(self.visible_width) as i32,
            bottom: self.gutter_y.wrapping_add(self.visible_height) as i32,
        }
    }
    pub fn line_bytes(&self, line: u32) -> Result<u32> {
        if self.text.is_empty() {
            return Ok(0);
        }
        let last = self.line_count().checked_sub(1).ok_or(Boundary::InvalidIndex)?;
        let line = line.min(last) as usize;
        let end = self.lines.get(line + 1).copied().unwrap_or(self.text_length());
        Ok(end.wrapping_sub(self.lines[line]))
    }
    pub fn line_column_from_index(&self, index: u32) -> Result<(u32, u32)> {
        if self.lines.is_empty() || self.text.is_empty() || index == 0 {
            return Ok((0, 0));
        }
        if index > self.text_length() {
            return Err(Boundary::InvalidIndex);
        }
        for (line, starts) in self.lines.windows(2).enumerate() {
            if index >= starts[0] && index < starts[1] {
                return Ok((line as u32, index - starts[0]));
            }
        }
        let last = self.line_count() - 1;
        let column = if self.text[index as usize - 1] == b'\n' {
            0
        } else {
            index.wrapping_sub(self.lines[last as usize])
        };
        Ok((last, column))
    }
    pub fn index_from_line_column(&self, line: u32, column: u32) -> (bool, u32) {
        if self.text.is_empty() || self.lines.is_empty() {
            return (false, 0);
        }
        let line = line.min(self.line_count() - 1);
        let index = self.lines[line as usize].wrapping_add(column);
        if index > self.text_length() {
            (false, self.text_length())
        } else {
            (true, index)
        }
    }
    pub fn set_insertion_line_column(&mut self, line: u32, column: u32) -> Result<bool> {
        if self.lines.is_empty() {
            self.position.line = 0;
            self.position.column = 0;
            self.position.index = 0;
        } else {
            self.position.line = line.min(self.line_count() - 1);
            self.position.column = column.min(self.line_bytes(self.position.line)?);
            self.position.index = self.index_from_line_column(self.position.line, self.position.column).1;
        }
        Ok(true)
    }
    pub(super) fn update_coordinates(&mut self) -> Result<()> {
        let (line, column) = self.line_column_from_index(self.position.index)?;
        self.position.line = line;
        self.position.column = column;
        Ok(())
    }
    pub fn increment_position(&mut self, delta: i32, composed: bool, s: &mut impl Services) -> Result<()> {
        let use_helpers = if composed {
            self.selector == 8 || self.selector >= 100
        } else {
            self.selector == 8 || self.selector == 100
        };
        let mut index = self.position.index;
        if use_helpers {
            for _ in 0..delta.unsigned_abs() {
                let n = if composed {
                    s.composed_length(&self.text, index, delta < 0, self.selector)?
                } else if delta < 0 {
                    previous_char_length(Self::span(&self.text, 0, index)?, self.selector)?
                } else {
                    char_length(&self.text, index, self.text_length().wrapping_sub(index), self.selector)?
                };
                index = if delta < 0 {
                    index.wrapping_sub(n)
                } else {
                    index.wrapping_add(n)
                };
            }
        } else {
            index = index.wrapping_add(delta as u32);
        }
        self.position.index = if (index as i32) < 0 {
            0
        } else {
            index.min(self.text_length())
        };
        self.update_coordinates()
    }
    pub fn increment_word(&mut self, delta: i32) -> Result<()> {
        if self.text.is_empty() || delta == 0 {
            return Ok(());
        }
        let mut index = self.position.index;
        if index > self.text_length() {
            return Err(Boundary::InvalidIndex);
        }
        for _ in 0..delta.unsigned_abs() {
            if delta > 0 {
                if index >= self.text_length() {
                    break;
                }
                while index + 1 < self.text_length() {
                    let here = self.text[index as usize];
                    let next = self.text[index as usize + 1];
                    index += 1;
                    if Self::separator(here) && !Self::separator(next) {
                        break;
                    }
                }
                if index == self.text_length() - 1 {
                    index = self.text_length();
                }
            } else {
                if index == 0 {
                    break;
                }
                index -= 1;
                while index > 0 {
                    if !Self::separator(self.text[index as usize]) && Self::separator(self.text[index as usize - 1]) {
                        break;
                    }
                    index -= 1;
                }
            }
        }
        self.position.index = index.min(self.text_length());
        self.update_coordinates()
    }
    pub fn separator(byte: u8) -> bool {
        byte == b' ' || byte == b'\n'
    }
    pub fn correct_preceding_lf(&mut self) {
        if self.position.index > 0
            && self.position.column > 0
            && self.text.get(self.position.index as usize - 1) == Some(&b'\n')
        {
            self.position.index -= 1;
            self.position.column -= 1;
        }
    }
    pub fn select_current_word(&mut self, s: &mut impl Services) -> Result<bool> {
        if self.text.is_empty() {
            return Ok(false);
        }
        // Native requires a readable byte at insertion (possibly its terminator).
        let byte = *self
            .text
            .get(self.position.index as usize)
            .ok_or(Boundary::BufferRange)?;
        if Self::separator(byte) {
            if self.position.index == 0 || Self::separator(self.text[self.position.index as usize - 1]) {
                self.put_in_view(s)?;
                return Ok(true);
            }
            self.position.index -= 1;
            self.collapse();
        }
        let (mut a, mut b) = self.selection_range();
        while a > 0 {
            let previous = *self.text.get(a as usize - 1).ok_or(Boundary::BufferRange)?;
            if Self::separator(previous) {
                break;
            }
            a -= 1;
        }
        while b < self.text_length() && !Self::separator(self.text[b as usize]) {
            b += 1;
            self.position.index = b;
        }
        // Preserve endpoint orientation while extending the existing range.
        if self.position.anchor <= self.position.end {
            self.position.anchor = a;
            self.position.end = b;
        } else {
            self.position.anchor = b;
            self.position.end = a;
        }
        self.fix_insertion()?;
        self.put_in_view(s)?;
        Ok(true)
    }
    pub fn adjust_line_array(&mut self, increment: u32, count: u32) -> Result<()> {
        if count > self.line_count() {
            let at = count.wrapping_sub(2) as usize;
            let fill = self
                .lines
                .get(at)
                .ok_or(Boundary::InvalidIndex)?
                .wrapping_add(increment);
            self.lines.resize(count as usize, fill);
        } else if count > 1 {
            self.lines[count as usize - 1] = self.lines[count as usize - 2].wrapping_add(increment);
        } else {
            *self.lines.first_mut().ok_or(Boundary::InvalidIndex)? = 0;
        }
        Ok(())
    }

    pub fn calculate_lines(&mut self, s: &mut impl Services) -> Result<()> {
        if !s.has_font() {
            return Err(Boundary::MissingFont);
        }
        if self.text.is_empty() {
            self.lines.clear();
            self.position.line = 0;
            self.position.column = 0;
            self.position.index = 0;
            return Ok(());
        }
        let mut starts = vec![0];
        let mut at = 0;
        while at < self.text_length() {
            let remaining = Self::span(&self.text, at, self.text_length() - at)?;
            let count = if self.visible_width == 0 || self.flags & 0x200 != 0 {
                remaining.len() as u32
            } else if self.flags & 2 == 0 {
                remaining
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(remaining.len(), |p| p + 1) as u32
            } else {
                s.fit(remaining, self.visible_width)?
            };
            if count == 0 {
                return Err(Boundary::NonProgress);
            }
            let part = Self::span(&self.text, at, count)?;
            let lf = part.last() == Some(&b'\n');
            at = at.checked_add(count).ok_or(Boundary::ArithmeticDomain)?;
            if at < self.text_length() || lf {
                starts.push(at);
            }
        }
        self.lines = starts;
        Ok(())
    }
    pub(super) fn bar_size(bar: Option<u64>, s: &mut impl Services) -> Result<(u32, u32)> {
        let Some(bar) = bar else {
            return Ok((0, 0));
        };
        let window = s.call(Call::BarWindow(bar))?.handle()?.ok_or(Boundary::Foreign)?;
        if let Reply::Size(w, h) = s.call(Call::WindowSize(window))? {
            Ok((w, h))
        } else {
            Err(Boundary::Foreign)
        }
    }
    pub fn calculate_visible_width(&mut self, s: &mut impl Services) -> Result<u32> {
        let (bar, _) = Self::bar_size(self.vertical.handle, s)?;
        let sum = self.gutter_x.wrapping_mul(2).wrapping_add(bar);
        self.visible_width = if self.width < sum {
            0
        } else {
            self.width.wrapping_sub(sum)
        };
        Ok(self.visible_width)
    }
    pub fn calculate_visible_height(&mut self, s: &mut impl Services) -> Result<u32> {
        let (_, bar) = Self::bar_size(self.horizontal.handle, s)?;
        let sum = self.gutter_y.wrapping_mul(2).wrapping_add(bar);
        self.visible_height = if self.height < sum {
            0
        } else {
            self.height.wrapping_sub(sum)
        };
        Ok(self.visible_height)
    }
    pub fn recalculate(&mut self, s: &mut impl Services) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }
        self.line_height = if s.has_font() { s.line_height()? } else { 16 };
        self.calculate_visible_height(s)?;
        if self.line_height == 0 {
            return Err(Boundary::ZeroDivisor);
        }
        self.visible_rows = self.visible_height / self.line_height;
        if self.visible_rows == 0 && self.visible_height as i32 > 0 {
            self.visible_rows = 1;
        }
        self.calculate_visible_width(s)?;
        self.calculate_lines(s)?;
        self.bar_values(true, s)?;
        self.fix_insertion()?;
        self.first_visible = if self.lines.is_empty() {
            0
        } else {
            self.first_visible.min(self.line_count() - 1)
        };
        self.check_automatic_bars(s)
    }
    pub fn longest_line(&self, all: bool, s: &mut impl Services) -> Result<u32> {
        let (start, end) = if all {
            (0, self.line_count())
        } else {
            (self.first_visible, self.first_visible.wrapping_add(self.visible_rows))
        };
        let mut maximum = 0;
        for line in start..end {
            let at = *self.lines.get(line as usize).ok_or(Boundary::InvalidIndex)?;
            maximum = maximum.max(s.measure(Self::span(&self.text, at, self.line_bytes(line)?)?)?);
        }
        Ok(maximum)
    }
    pub fn horizontal_scroll_pixels(&self, s: &mut impl Services) -> Result<u32> {
        if self.flags & 0x200 != 0 {
            s.measure(Self::span(&self.text, 0, self.horizontal_byte)?)
        } else {
            Ok(self.horizontal_pixel)
        }
    }
    pub fn position_from_line_column(&self, line: u32, column: u32, s: &mut impl Services) -> Result<(i32, i32)> {
        let (line, width) = if self.lines.is_empty() {
            (0, 0)
        } else {
            let line = line.min(self.line_count() - 1);
            let at = self.lines[line as usize];
            (
                line,
                s.measure(Self::span(
                    &self.text,
                    at,
                    column.min(self.text_length().wrapping_sub(at)),
                )?)?,
            )
        };
        let x = width
            .wrapping_add(self.gutter_x)
            .wrapping_sub(self.horizontal_scroll_pixels(s)?) as i32;
        let y = line
            .wrapping_sub(self.first_visible)
            .wrapping_mul(self.line_height)
            .wrapping_add(self.gutter_y) as i32;
        Ok((x, y))
    }
    pub fn column_from_position(&self, line: u32, x: i32, s: &mut impl Services) -> Result<u32> {
        let x = (x as u32)
            .wrapping_add(self.horizontal_scroll_pixels(s)?)
            .wrapping_sub(self.gutter_x) as i32;
        if x <= 0 || self.lines.is_empty() {
            return Ok(0);
        }
        let at = *self.lines.get(line as usize).ok_or(Boundary::InvalidIndex)?;
        let count = self.line_bytes(line)?;
        let mut column = 0;
        while column < count {
            let n = char_length(&self.text, at.wrapping_add(column), count - column, self.selector)?;
            if n == 0 {
                return Err(Boundary::NonProgress);
            }
            let next = column.wrapping_add(n);
            if x <= s.measure(Self::span(&self.text, at, next)?)? as i32 {
                return Ok(column);
            }
            column = next;
        }
        Ok(count)
    }
    pub fn horizontal_metrics(&self, line: u32, s: &mut impl Services) -> Result<(u32, u32, i32)> {
        let at = *self.lines.get(line as usize).ok_or(Boundary::InvalidIndex)?;
        let count = self.line_bytes(line)?;
        if count == 0 {
            return Ok((at, at, 0));
        }
        let scroll = self.horizontal_scroll_pixels(s)?;
        let step = |offset: u32| {
            char_length(
                &self.text,
                at.wrapping_add(offset),
                count.wrapping_sub(offset),
                self.selector,
            )
        };
        let (start, displacement) = if scroll == 0 {
            (0, 0)
        } else {
            let mut n = step(0)?;
            let mut total = n;
            let mut measured = 0u32;
            let mut previous = 0;
            while total < count {
                previous = measured;
                measured = s.measure(Self::span(&self.text, at, total)?)?;
                if measured > scroll {
                    break;
                }
                n = step(total)?;
                if n == 0 {
                    return Err(Boundary::NonProgress);
                }
                total = total.wrapping_add(n);
            }
            (total.wrapping_sub(n), previous.wrapping_sub(scroll) as i32)
        };
        let mut end = start.wrapping_add(step(start)?);
        if end == start {
            return Err(Boundary::NonProgress);
        }
        while end < count {
            if s.measure(Self::span(&self.text, at.wrapping_add(start), end.wrapping_sub(start))?)?
                >= self.visible_width.wrapping_sub(displacement as u32)
            {
                break;
            }
            let n = step(end)?;
            if n == 0 {
                return Err(Boundary::NonProgress);
            }
            end = end.wrapping_add(n);
        }
        Ok((at.wrapping_add(start), at.wrapping_add(end), displacement))
    }
    pub fn put_in_view(&mut self, s: &mut impl Services) -> Result<()> {
        let line = self.position.line;
        let mut dirty = false;
        if line < self.first_visible {
            self.first_visible = line;
            dirty = true;
        } else if line >= self.first_visible.wrapping_add(self.visible_rows) {
            self.first_visible = line.wrapping_sub(self.visible_rows).wrapping_add(1);
            dirty = true;
        }
        if self.flags & 0x200 != 0 {
            if self.position.column < self.horizontal_byte {
                self.horizontal_byte = self.position.column;
            } else {
                let caret = s.measure(Self::span(&self.text, 0, self.position.column)?)?;
                let relative = caret.wrapping_sub(self.horizontal_scroll_pixels(s)?) as i32;
                if relative > self.visible_width as i32 {
                    let target = (self.visible_width.wrapping_mul(3) as i32) / 4;
                    while self.horizontal_byte < self.text_length() {
                        let n = char_length(
                            &self.text,
                            self.horizontal_byte,
                            self.text_length() - self.horizontal_byte,
                            self.selector,
                        )?;
                        if n == 0 {
                            return Err(Boundary::NonProgress);
                        }
                        self.horizontal_byte = self.horizontal_byte.wrapping_add(n);
                        if (caret.wrapping_sub(self.horizontal_scroll_pixels(s)?) as i32) < target {
                            break;
                        }
                    }
                }
            }
            self.bar_values(false, s)?;
        } else {
            if self.horizontal.handle.is_some() || self.flags & 2 == 0 {
                let at = self.lines.get(self.position.line as usize).copied().unwrap_or(0);
                let caret = s.measure(Self::span(&self.text, at, self.position.column)?)?;
                let relative = caret.wrapping_sub(self.horizontal_scroll_pixels(s)?) as i32;
                if relative < 0 {
                    self.horizontal_pixel = self.horizontal_pixel.wrapping_add(relative as u32);
                    dirty = true;
                } else if relative as u32 >= self.visible_width {
                    self.horizontal_pixel = self
                        .horizontal_pixel
                        .wrapping_add((relative as u32).wrapping_sub(self.visible_width));
                    dirty = true;
                }
            }
            if dirty {
                self.bar_values(false, s)?;
            }
        }
        Ok(())
    }
    pub fn bar_values(&self, ranges: bool, s: &mut impl Services) -> Result<()> {
        if let Some(bar) = self.vertical.handle {
            if ranges && self.visible_rows != 0 {
                let max = (self.line_count().wrapping_sub(self.visible_rows) as i32).max(0) as u32;
                s.call(Call::BarRange { bar, max })?;
                s.call(Call::BarPair {
                    bar,
                    max,
                    step: (self.line_count() / self.visible_rows).wrapping_add(1),
                })?;
            }
            s.call(Call::BarValue {
                bar,
                value: self.first_visible,
            })?;
        }
        if let Some(bar) = self.horizontal.handle {
            if ranges && self.visible_rows != 0 {
                let longest = self.longest_line(true, s)?;
                let max = (longest.wrapping_sub(self.visible_width) as i32).max(0) as u32;
                let divisor = self.visible_width as i32;
                if divisor == 0 {
                    return Err(Boundary::ZeroDivisor);
                }
                let step = (longest as i32)
                    .checked_div(divisor)
                    .ok_or(Boundary::ArithmeticDomain)?
                    .wrapping_add(1) as u32;
                s.call(Call::BarRange { bar, max })?;
                s.call(Call::BarPair { bar, max, step })?;
            }
            s.call(Call::BarValue {
                bar,
                value: self.horizontal_pixel,
            })?;
        }
        Ok(())
    }
    pub fn scroll(&mut self, direction: Scroll, s: &mut impl Services) -> Result<bool> {
        let remaining = self
            .line_count()
            .wrapping_sub(self.visible_rows)
            .wrapping_sub(self.first_visible) as i32;
        match direction {
            Scroll::LineUp => {
                if self.first_visible as i32 <= 0 {
                    return Ok(false);
                }
                self.first_visible -= 1;
            }
            Scroll::LineDown => {
                if remaining <= 0 {
                    return Ok(false);
                }
                self.first_visible = self.first_visible.wrapping_add(1);
            }
            Scroll::PageUp => {
                if self.first_visible == 0 {
                    return Ok(false);
                }
                self.first_visible = if (self.first_visible as i32) < self.visible_rows as i32 {
                    0
                } else {
                    self.first_visible.wrapping_sub(self.visible_rows)
                };
            }
            Scroll::PageDown => {
                if remaining <= 0 {
                    return Ok(false);
                }
                self.first_visible = if self.first_visible < self.line_count() {
                    self.first_visible.wrapping_add(self.visible_rows)
                } else {
                    self.line_count().wrapping_sub(1)
                };
            }
            Scroll::Start => {
                if self.first_visible == 0 && self.horizontal_byte == 0 {
                    return Ok(false);
                }
                self.first_visible = 0;
                self.horizontal_byte = 0;
                self.horizontal_pixel = 0;
            }
            Scroll::End => {
                if remaining <= 0 {
                    return Ok(false);
                }
                self.first_visible = self.line_count().wrapping_sub(self.visible_rows);
                self.horizontal_byte = 0;
                self.horizontal_pixel = 0;
            }
            Scroll::Left | Scroll::Right => {
                let right = direction == Scroll::Right;
                if self.flags & 0x200 != 0 {
                    if right && self.horizontal_byte < self.text_length() {
                        self.horizontal_byte = self.horizontal_byte.wrapping_add(char_length(
                            &self.text,
                            self.horizontal_byte,
                            self.text_length() - self.horizontal_byte,
                            self.selector,
                        )?);
                    } else if !right && self.horizontal_byte as i32 > 0 {
                        self.horizontal_byte = self.horizontal_byte.wrapping_sub(previous_char_length(
                            Self::span(&self.text, 0, self.horizontal_byte)?,
                            self.selector,
                        )?);
                    }
                    self.invalidate(s)?;
                    return Ok(true);
                }
                if self.flags & 2 != 0 && self.horizontal.handle.is_none() {
                    return Ok(true);
                }
                if right && self.horizontal_pixel <= self.visible_width {
                    self.horizontal_pixel = self
                        .horizontal_pixel
                        .wrapping_add(self.line_height)
                        .min(self.visible_width);
                } else if !right && self.horizontal_pixel as i32 > 0 {
                    self.horizontal_pixel = self.horizontal_pixel.wrapping_sub(self.line_height);
                }
            }
        }
        self.bar_values(false, s)?;
        self.invalidate(s)?;
        Ok(true)
    }
    pub fn resize_exact_line_heights(&mut self, s: &mut impl Services) -> Result<bool> {
        if self.line_height == 0 {
            return Err(Boundary::ZeroDivisor);
        }
        let inner = self.height.wrapping_sub(self.gutter_y.wrapping_mul(2));
        let mut rows = inner / self.line_height;
        if rows as i32 <= 0 {
            rows = 1;
        }
        let product = rows.wrapping_mul(self.line_height);
        if product != inner {
            s.call(Call::Base(
                "SetHeight",
                vec![product.wrapping_add(self.gutter_y.wrapping_mul(2))],
            ))?;
        }
        self.bar_sizes(s)?;
        self.bar_values(true, s)?;
        self.invalidate(s)?;
        Ok(true)
    }
}

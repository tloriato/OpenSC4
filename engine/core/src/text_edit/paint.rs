// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

/// Base-provided drawing geometry/colors. No text span or layout decision is
/// accepted from a drawing service. Font +0xCC always receives the owned span.
#[derive(Clone, Copy, Debug)]
pub struct PaintContext { pub draw: Rect, pub clip: Rect }
#[derive(Clone, Copy, Debug)]
pub struct PaintPalette {
    pub foreground: u32, pub background: u32,
    pub selected_foreground: u32, pub selected_background: u32,
}

impl TextEdit {
    pub fn password_buffer(&self) -> Result<Vec<u8>> {
        Ok(vec![self.mask; glyph_count(&self.text, self.selector)? as usize])
    }
    pub fn line_column_from_position(&self, mut x: i32, mut y: i32, s: &mut impl Services) -> Result<(u32, u32)> {
        if y < 0 { x = 0; y = 0; } if x < 0 { x = 0; }
        if self.line_height == 0 { return Err(Boundary::ZeroDivisor); }
        let row = self.first_visible.wrapping_add(y as u32 / self.line_height);
        let password = self.flags & 0x2000 != 0;
        let mask = if password { self.password_buffer()? } else { Vec::new() };
        let buffer = if password { &mask } else { &self.text };
        if buffer.is_empty() { return Ok((0, 0)); }
        let line = row.min(self.line_count().checked_sub(1).ok_or(Boundary::InvalidIndex)?);
        let at = self.lines[line as usize]; let count = self.line_bytes(line)?;
        let adjusted = (x as u32).wrapping_sub(self.gutter_x).wrapping_add(self.horizontal_scroll_pixels(s)?) as i32;
        let mut column = 0u32; let mut previous_width = 0i32;
        while column < count {
            let n = char_length(buffer, at.wrapping_add(column), count - column, if password { 0 } else { self.selector })?;
            if n == 0 { return Err(Boundary::NonProgress); }
            let next = column.wrapping_add(n);
            let width = s.measure(Self::span(buffer, at, next)?)? as i32;
            if width > adjusted {
                let left = adjusted.wrapping_sub(previous_width).wrapping_sub(1);
                let right = width.wrapping_sub(adjusted);
                let chosen = if left < right { column } else { next };
                if !password { return Ok((line, chosen)); }
                // LengthFromGlyphCount walks the WHOLE original text prefix.
                let mut bytes = 0;
                for _ in 0..chosen {
                    let n = char_length(&self.text, bytes, self.text_length().wrapping_sub(bytes), self.selector)?;
                    if n == 0 { return Err(Boundary::NonProgress); }
                    bytes = bytes.wrapping_add(n);
                }
                return Ok((line, bytes));
            }
            previous_width = width; column = next;
        }
        Ok((line, count))
    }
    pub fn index_from_position(&self, x: i32, y: i32, s: &mut impl Services) -> Result<u32> {
        if self.text.is_empty() { return Ok(0); }
        let (line, column) = self.line_column_from_position(x, y, s)?;
        Ok(self.lines.get(line as usize).ok_or(Boundary::InvalidIndex)?.wrapping_add(column))
    }
    fn draw_span(buffer: &[u8], at: u32, count: u32, x: i32, y: i32,
        palette: (u32, u32), clip: Rect, s: &mut impl Services) -> Result<()> {
        s.call(Call::Draw { bytes: Self::span(buffer, at, count)?.to_vec(), x, y,
            foreground: palette.0, background: palette.1, clip })?; Ok(())
    }
    pub fn paint(&self, context: PaintContext, palette: PaintPalette, s: &mut impl Services) -> Result<bool> {
        let draw = context.draw;
        if self.flags & 0x20 != 0 {
            if let Some(image) = self.background {
                let image_rect = match s.call(Call::ImageRect(image))? {
                    Reply::Rectangle(rect) => rect, _ => return Err(Boundary::Foreign),
                };
                let (vw, _) = Self::bar_size(self.vertical.handle, s)?;
                let (_, hh) = Self::bar_size(self.horizontal.handle, s)?;
                let rect = Rect { right: draw.right.wrapping_sub(vw as i32),
                    bottom: draw.bottom.wrapping_sub(hh as i32), ..draw };
                // [choice TE9] Nonzero option selects edge drawing; polarity is
                // not published. Geometry remains an external drawing primitive.
                s.call(Call::Background { image, edge: self.background_option != 0, rect,
                    thirds: ((image_rect.right.wrapping_sub(image_rect.left) as u32) / 3,
                        (image_rect.bottom.wrapping_sub(image_rect.top) as u32) / 3) })?;
            } else { s.call(Call::Fill { rect: draw, color: self.fill_color })?; }
        }
        let password = self.flags & 0x2000 != 0;
        let mask = if password { self.password_buffer()? } else { Vec::new() };
        let buffer = if password { &mask } else { &self.text };
        let (mut a, mut b) = self.selection_range();
        if password {
            a = glyph_count(Self::span(&self.text, 0, a)?, self.selector)?;
            b = glyph_count(Self::span(&self.text, 0, b)?, self.selector)?;
        }
        let text_left = draw.left.wrapping_add(self.gutter_x as i32);
        let text_top = draw.top.wrapping_add(self.gutter_y as i32);
        let area = Rect { left: text_left, top: text_top,
            right: text_left.wrapping_add(self.visible_width as i32),
            bottom: text_top.wrapping_add(self.visible_height as i32) };
        let clip = Rect { left: area.left.max(context.clip.left), top: area.top.max(context.clip.top),
            right: area.right.min(context.clip.right), bottom: area.bottom.min(context.clip.bottom) };
        let last = self.first_visible.wrapping_add(self.visible_rows).min(self.line_count());
        if self.visible_width != 0 {
            for line in self.first_visible..last {
                let y = text_top.wrapping_add(line.wrapping_sub(self.first_visible).wrapping_mul(self.line_height) as i32);
                let (start, end, displacement) = if self.flags & 2 != 0 && self.horizontal.handle.is_none() {
                    let start = self.lines[line as usize]; (start, start.wrapping_add(self.line_bytes(line)?), 0)
                } else { self.horizontal_metrics(line, s)? };
                let x = text_left.wrapping_add(displacement);
                let selected = (self.focused || self.flags & 0x10000 != 0) && a != b && a < end && b > start;
                let ordinary = (palette.foreground, palette.background);
                let highlighted = (palette.selected_foreground, palette.selected_background);
                if !selected { Self::draw_span(buffer, start, end.wrapping_sub(start), x, y, ordinary, clip, s)?; }
                else {
                    let lo = a.max(start); let hi = b.min(end);
                    let full = lo == start && hi == end;
                    if !full { Self::draw_span(buffer, start, end.wrapping_sub(start), x, y, ordinary, clip, s)?; }
                    let before = s.measure(Self::span(buffer, start, lo.wrapping_sub(start))?)?;
                    let through = s.measure(Self::span(buffer, start, hi.wrapping_sub(start))?)?;
                    let left = x.wrapping_add(before as i32);
                    s.call(Call::Fill { rect: Rect { left, top: y, right: x.wrapping_add(through as i32),
                        bottom: y.wrapping_add(self.line_height as i32) }, color: self.highlight_fill })?;
                    Self::draw_span(buffer, lo, hi.wrapping_sub(lo), left, y, highlighted, clip, s)?;
                }
            }
        }
        if self.focused && self.caret_visible && self.position.line >= self.first_visible && self.position.line < last {
            let start = self.lines[self.position.line as usize];
            // TE6: keep byte column/line start in the mask; reject invalid safe spans.
            let advance = s.measure(Self::span(buffer, start, self.position.column)?)?;
            let mut x = text_left.wrapping_add(advance as i32).wrapping_sub(self.horizontal_scroll_pixels(s)? as i32);
            if self.flags & 2 != 0 && x >= area.right { x = area.right.wrapping_sub(1); }
            let y = text_top.wrapping_add(self.position.line.wrapping_sub(self.first_visible).wrapping_mul(self.line_height) as i32);
            let top = y.max(draw.top.wrapping_add(1));
            let bottom = y.wrapping_add(self.line_height as i32).min(draw.bottom.wrapping_sub(1));
            let width = if self.line_height < 24 { 1.0 } else { 2.0 };
            s.call(Call::PenWidth(width))?;
            s.call(Call::Caret { x, top, bottom, color: self.caret_color })?;
            s.call(Call::PenWidth(1.0))?;
        }
        if self.focused && self.flags & 0x10 != 0 { s.call(Call::Outline { rect: draw, color: self.outline_color })?; }
        Ok(true)
    }
}

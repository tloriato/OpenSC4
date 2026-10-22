// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

impl TextEdit {
    pub(super) fn normalize_flags(mut flags: u32) -> u32 {
        if flags & 0x2000 != 0 {
            flags |= 0x200;
            flags &= !6;
        }
        if flags & 0x200 != 0 {
            flags &= !6;
        }
        if flags & 2 != 0 {
            flags &= !4;
        }
        flags
    }
    pub fn set_flags(&mut self, flags: u32, s: &mut impl Services) -> Result<()> {
        let changes = flags ^ self.flags;
        self.flags = Self::normalize_flags(flags);
        if self.initialized {
            if changes & 0x41 != 0 {
                self.start_caret_timer(self.enabled && self.flags & 0x41 == 0x41, s)?;
            }
            if changes & 0xb != 0 {
                self.recalculate(s)?;
                self.invalidate(s)?;
                s.call(Call::Cursor(if self.flags & 1 != 0 { Some(3) } else { None }))?;
            }
        }
        Ok(())
    }
    pub fn set_flag(&mut self, mask: u32, s: &mut impl Services) -> Result<()> {
        self.set_flags(self.flags | mask, s)
    }
    pub fn clear_flag(&mut self, mask: u32, s: &mut impl Services) -> Result<()> {
        if self.flags & mask != 0 {
            self.set_flags(self.flags & !mask, s)?;
        }
        Ok(())
    }
    pub fn base_set_flag(&mut self, mask: u32, on: bool, s: &mut impl Services) -> Result<()> {
        if mask == 1 {
            self.enabled = on;
            self.start_caret_timer(on && self.flags & 0x41 == 0x41, s)?;
        }
        s.call(Call::Base("SetFlag", vec![mask, u32::from(on)]))?;
        Ok(())
    }
    pub fn toggle_caret(&mut self, s: &mut impl Services) -> Result<()> {
        if self.focused {
            self.caret_visible = !self.caret_visible;
            self.invalidate(s)?;
        }
        Ok(())
    }
    pub fn make_caret_visible(&mut self, s: &mut impl Services) -> Result<()> {
        if self.focused && !self.caret_visible {
            self.caret_visible = true;
            self.invalidate(s)?;
        }
        Ok(())
    }
    pub fn start_caret_timer(&mut self, start: bool, s: &mut impl Services) -> Result<bool> {
        if self.caret_running == start {
            return Ok(false);
        }
        let result = s
            .call(Call::Timer {
                caret: true,
                start,
                interval: self.caret_period >> 1,
                data: self.token,
            })?
            .boolean()?;
        if result {
            self.caret_running = start;
        }
        Ok(result)
    }
    pub fn start_scroll_timer(&mut self, start: bool, s: &mut impl Services) -> Result<()> {
        if self.scroll_running != start {
            let result = s
                .call(Call::Timer {
                    caret: false,
                    start,
                    interval: 100,
                    data: 0,
                })?
                .boolean()?;
            if result {
                self.scroll_running = start;
            }
        }
        Ok(())
    }
    pub fn caret_period(&self) -> u32 {
        self.caret_period
    }
    pub fn set_caret_period(&mut self, period: u32, s: &mut impl Services) -> Result<bool> {
        self.caret_period = period.clamp(250, 10000);
        if self.initialized && self.flags & 0x40 != 0 {
            self.clear_flag(0x40, s)?;
            self.set_flag(0x40, s)?;
        }
        Ok(true)
    }
    pub fn post_timer_callback(&self, caret: bool, s: &mut impl Services) -> Result<()> {
        s.call(Call::Post(if caret { 0x631b56cc } else { 0x631b56cd }))?;
        Ok(())
    }
    pub fn init(&mut self, s: &mut impl Services) -> Result<bool> {
        if self.initialized {
            return Ok(true);
        }
        s.call(Call::Base("Init", Vec::new()))?;
        let requested = self.font_style.unwrap_or(0x68963c4c);
        let mut font = s.call(Call::LookupFont(requested))?.handle()?;
        if font.is_none() && self.font_style.is_some() {
            font = s.call(Call::LookupFont(0x68963c4c))?.handle()?;
        }
        self.set_font(font, s)?;
        if self.flags & 1 != 0 {
            s.call(Call::Cursor(Some(3)))?;
        }
        if self.enabled && self.flags & 0x41 == 0x41 {
            self.start_caret_timer(true, s)?;
        }
        self.selector = s.call(Call::CodePage)?.word()? as i32;
        if self.clipboard.is_none() {
            self.clipboard = s.call(Call::AcquireClipboard)?.handle()?;
            if let Some(handle) = self.clipboard {
                s.call(Call::AddRef(handle))?;
            }
        }
        self.initialized = true;
        // Init tolerates null fonts; reflow with null font only succeeds where
        // the published font-dependent calls are not reached (TE8).
        if s.has_font() {
            self.recalculate(s)?;
        } else {
            self.line_height = 16;
            self.calculate_visible_height(s)?;
            self.visible_rows = self.visible_height / 16;
            if self.visible_rows == 0 && self.visible_height as i32 > 0 {
                self.visible_rows = 1;
            }
        }
        Ok(true)
    }
    pub fn font(&self) -> Option<u64> {
        self.font
    }
    pub fn font_style(&self) -> Option<u64> {
        self.font_style
    }
    pub fn set_font(&mut self, font: Option<u64>, s: &mut impl Services) -> Result<()> {
        if self.font == font {
            return Ok(());
        }
        if let Some(handle) = font {
            s.call(Call::AddRef(handle))?;
        }
        if let Some(old) = self.font {
            s.call(Call::Release(old))?;
        }
        self.font = font;
        if self.initialized {
            self.recalculate(s)?;
            self.invalidate(s)?;
        }
        Ok(())
    }
    pub fn set_font_style(&mut self, style: Option<u64>, s: &mut impl Services) -> Result<bool> {
        self.font_style = style;
        if self.initialized {
            let font = s.call(Call::LookupFont(style.unwrap_or(0x68963c4c)))?.handle()?;
            if font.is_some() {
                self.set_font(font, s)?;
            }
        }
        Ok(true)
    }
    pub fn background(&self) -> (Option<u64>, u8) {
        (self.background, self.background_option)
    }
    pub fn set_background(&mut self, image: Option<u64>, option: u8, s: &mut impl Services) -> Result<()> {
        let changed = self.background != image || self.background_option != option;
        if self.background != image {
            if let Some(handle) = image {
                s.call(Call::AddRef(handle))?;
            }
            if let Some(old) = self.background {
                s.call(Call::Release(old))?;
            }
            self.background = image;
        }
        self.background_option = option;
        if changed {
            self.invalidate(s)?;
        }
        Ok(())
    }
    pub fn bar(&self, vertical: bool) -> Bar {
        if vertical {
            self.vertical
        } else {
            self.horizontal
        }
    }
    fn effective_image_rect(image: u64, rect: Rect, s: &mut impl Services) -> Result<Rect> {
        if rect.right <= rect.left || rect.bottom <= rect.top {
            match s.call(Call::ImageRect(image))? {
                Reply::Rectangle(r) => Ok(r),
                _ => Err(Boundary::Foreign),
            }
        } else {
            Ok(rect)
        }
    }
    pub fn set_bar_image(
        &mut self,
        vertical: bool,
        image: Option<u64>,
        rect: Rect,
        s: &mut impl Services,
    ) -> Result<bool> {
        let bar = if vertical {
            &mut self.vertical
        } else {
            &mut self.horizontal
        };
        if let Some(handle) = image {
            s.call(Call::AddRef(handle))?;
        }
        if let Some(old) = bar.image {
            s.call(Call::Release(old))?;
        }
        bar.image = image;
        bar.image_rect = rect;
        if let (Some(image), Some(handle)) = (image, bar.handle) {
            let rect = Self::effective_image_rect(image, rect, s)?;
            s.call(Call::BarImage {
                bar: handle,
                image,
                rect,
            })?;
        }
        Ok(true)
    }
    pub fn add_scrollbars(&mut self, vertical: bool, horizontal: bool, s: &mut impl Services) -> Result<bool> {
        let mut attempted = false;
        for (want, bit, is_vertical) in [(vertical, 8, true), (horizontal, 4, false)] {
            if !want || self.flags & bit == 0 || self.bar(is_vertical).handle.is_some() {
                continue;
            }
            attempted = true;
            let args = if is_vertical {
                [0, 2, 0, 100, 0]
            } else {
                [1, 1, 0, 100, 0]
            };
            let handle = s
                .call(Call::CreateBar {
                    vertical: is_vertical,
                    args,
                })?
                .handle()?;
            if is_vertical {
                self.vertical.handle = handle;
            } else {
                self.horizontal.handle = handle;
            }
            if let Some(handle) = handle {
                let bar = self.bar(is_vertical);
                if let Some(image) = bar.image {
                    let rect = Self::effective_image_rect(image, bar.image_rect, s)?;
                    s.call(Call::BarImage {
                        bar: handle,
                        image,
                        rect,
                    })?;
                }
                let window = s.call(Call::BarWindow(handle))?.handle()?.ok_or(Boundary::Foreign)?;
                s.call(Call::AddChild(window))?;
            } else {
                self.clear_flag(bit, s)?;
            }
        }
        if attempted {
            self.recalculate(s)?;
            self.bar_sizes(s)?;
            self.bar_values(true, s)?;
        }
        Ok(true)
    }
    pub fn remove_scrollbars(&mut self, vertical: bool, horizontal: bool, s: &mut impl Services) -> Result<()> {
        let mut handled = false;
        for (want, is_vertical) in [(vertical, true), (horizontal, false)] {
            if !want {
                continue;
            }
            if let Some(handle) = self.bar(is_vertical).handle {
                let window = s.call(Call::BarWindow(handle))?.handle()?.ok_or(Boundary::Foreign)?;
                s.call(Call::RemoveChild(window))?;
                s.call(Call::Release(handle))?;
                if is_vertical {
                    self.vertical.handle = None;
                } else {
                    self.horizontal.handle = None;
                }
                handled = true;
            }
        }
        if handled {
            self.recalculate(s)?;
        }
        Ok(())
    }
    pub fn bar_sizes(&self, s: &mut impl Services) -> Result<()> {
        if let Some(bar) = self.horizontal.handle {
            let (_, hh) = Self::bar_size(Some(bar), s)?;
            let (vw, _) = Self::bar_size(self.vertical.handle, s)?;
            s.call(Call::BarRect {
                bar,
                rect: Rect {
                    left: 0,
                    top: self.height.wrapping_sub(hh) as i32,
                    right: self.width.wrapping_sub(vw) as i32,
                    bottom: self.height as i32,
                },
            })?;
        }
        if let Some(bar) = self.vertical.handle {
            let (vw, _) = Self::bar_size(Some(bar), s)?;
            let (_, hh) = Self::bar_size(self.horizontal.handle, s)?;
            s.call(Call::BarRect {
                bar,
                rect: Rect {
                    left: self.width.wrapping_sub(vw).wrapping_add(1) as i32,
                    top: 0,
                    right: self.width as i32,
                    bottom: self.height.wrapping_sub(hh) as i32,
                },
            })?;
        }
        Ok(())
    }
    pub fn check_automatic_bars(&mut self, s: &mut impl Services) -> Result<()> {
        // [choice TE9] Spec names a shared recursion guard but gives no limit.
        // Portable nested automatic checks are withheld; explicit checks can run again.
        if self.auto_depth != 0 {
            return Ok(());
        }
        self.auto_depth += 1;
        let result = self.automatic_bars_inner(s);
        self.auto_depth -= 1;
        result
    }
    fn automatic_bars_inner(&mut self, s: &mut impl Services) -> Result<()> {
        let dimensions = self.visible_width != 0 && self.visible_height != 0;
        let vertical = self.flags & 8 != 0 && self.line_count() > self.visible_rows && dimensions;
        if vertical {
            self.add_scrollbars(true, false, s)?;
        } else {
            self.remove_scrollbars(true, false, s)?;
        }
        let dimensions = self.visible_width != 0 && self.visible_height != 0;
        let horizontal = self.flags & 4 != 0
            && self.flags & 2 == 0
            && dimensions
            && self.longest_line(true, s)? > self.visible_width;
        if horizontal {
            self.add_scrollbars(false, true, s)?;
        } else {
            self.remove_scrollbars(false, true, s)?;
        }
        Ok(())
    }
    pub fn set_area(&mut self, rect: Rect, s: &mut impl Services) -> Result<()> {
        s.call(Call::Base(
            "SetArea",
            vec![rect.left as u32, rect.top as u32, rect.right as u32, rect.bottom as u32],
        ))?;
        self.width = rect.right.wrapping_sub(rect.left) as u32;
        self.height = rect.bottom.wrapping_sub(rect.top) as u32;
        self.calculate_visible_width(s)?;
        self.recalculate(s)?;
        self.bar_sizes(s)?;
        self.bar_values(true, s)?;
        self.invalidate(s)
    }
    pub fn gutters(&self) -> (u32, u32) {
        (self.gutter_x, self.gutter_y)
    }
    pub fn set_gutter_x(&mut self, value: u32, s: &mut impl Services) -> Result<()> {
        if self.gutter_x != value {
            self.gutter_x = value;
            if self.initialized {
                self.calculate_visible_width(s)?;
                self.recalculate(s)?;
                self.invalidate(s)?;
            }
        }
        Ok(())
    }
    pub fn set_gutter_y(&mut self, value: u32, s: &mut impl Services) -> Result<()> {
        if self.gutter_y != value {
            self.gutter_y = value;
            if self.initialized {
                self.invalidate(s)?;
            }
        }
        Ok(())
    }
    pub fn update_scrollbars_and_position(&mut self, s: &mut impl Services) -> Result<()> {
        if self.initialized {
            self.put_in_view(s)?;
            self.check_automatic_bars(s)?;
        }
        self.invalidate(s)
    }
    pub fn show_window(&mut self, show: bool, s: &mut impl Services) -> Result<()> {
        self.update_scrollbars_and_position(s)?;
        s.call(Call::Base("ShowWindow", vec![u32::from(show)]))?;
        Ok(())
    }
    pub fn shutdown(&mut self, s: &mut impl Services) -> Result<()> {
        if let Some(font) = self.font.take() {
            s.call(Call::Release(font))?;
        }
        self.start_caret_timer(false, s)?;
        self.start_scroll_timer(false, s)?;
        for handle in [
            self.vertical.handle.take(),
            self.horizontal.handle.take(),
            self.vertical.image.take(),
            self.horizontal.image.take(),
            self.clipboard.take(),
        ]
        .into_iter()
        .flatten()
        {
            s.call(Call::Release(handle))?;
        }
        // TE7: no matching background release in native Shutdown/destructor.
        s.call(Call::Base("Shutdown", Vec::new()))?;
        self.initialized = false;
        Ok(())
    }
    pub fn destroy(&mut self, s: &mut impl Services) -> Result<()> {
        self.shutdown(s)?;
        s.call(Call::Base("DestroyScrollTimer", Vec::new()))?;
        s.call(Call::Base("DestroyCaretTimer", Vec::new()))?;
        self.redo.clear();
        self.undo.clear();
        self.pending.clear();
        self.lines.clear();
        self.text.clear();
        s.call(Call::Base("Destructor", Vec::new()))?;
        Ok(())
    }
    pub fn color(&self, index: usize) -> Result<u32> {
        self.colors.get(index).copied().ok_or(Boundary::InvalidIndex)
    }
    pub fn set_color(&mut self, index: usize, value: u32) -> Result<bool> {
        *self.colors.get_mut(index).ok_or(Boundary::InvalidIndex)? = value;
        Ok(true)
    }
    pub fn clear_font_color(&mut self, index: usize) -> Result<()> {
        self.set_color(
            index,
            match index {
                0 | 2 => 0xff000000,
                4 => 0xffffffff,
                _ => 0x40000000,
            },
        )?;
        Ok(())
    }
    pub fn set_font_color(&mut self, index: usize, value: u32, s: &mut impl Services) -> Result<bool> {
        let color = s.call(Call::Color { slot: 0x200, value })?.word()?;
        self.set_color(index, color)
    }
    pub fn font_color(&self, index: usize, s: &mut impl Services) -> Result<u32> {
        s.call(Call::Color {
            slot: 0x1fc,
            value: self.color(index)?,
        })?
        .word()
    }
    pub fn set_caret_color(&mut self, value: u32, s: &mut impl Services) -> Result<()> {
        self.caret_color = s.call(Call::Color { slot: 0x200, value })?.word()?;
        Ok(())
    }
    pub fn caret_color(&self, s: &mut impl Services) -> Result<u32> {
        s.call(Call::Color {
            slot: 0x1fc,
            value: self.caret_color,
        })?
        .word()
    }
    pub fn set_highlight_fill(&mut self, value: u32, s: &mut impl Services) -> Result<()> {
        self.highlight_fill = s.call(Call::Color { slot: 0x1fc, value })?.word()?;
        Ok(())
    }
    pub fn highlight_fill(&self, s: &mut impl Services) -> Result<u32> {
        s.call(Call::Color {
            slot: 0x1fc,
            value: self.highlight_fill,
        })?
        .word()
    }
    pub fn set_outline(&mut self, enabled: bool, color: u32, s: &mut impl Services) -> Result<()> {
        if enabled {
            self.flags |= 0x10;
        } else {
            self.flags &= !0x10;
        }
        self.outline_color = s
            .call(Call::Color {
                slot: 0x200,
                value: color,
            })?
            .word()?;
        Ok(())
    }
    pub fn outline(&self, s: &mut impl Services) -> Result<(bool, u32)> {
        Ok((
            self.flags & 0x10 != 0,
            s.call(Call::Color {
                slot: 0x1fc,
                value: self.outline_color,
            })?
            .word()?,
        ))
    }
    pub fn set_fill_color(&mut self, value: u32, s: &mut impl Services) -> Result<bool> {
        let result = s.call(Call::Base("SetFillColor", vec![value]))?.boolean()?;
        self.fill_color = value;
        if self.flags & 0x20 != 0 {
            self.invalidate(s)?;
        }
        Ok(result)
    }
    pub fn find_text(&self, needle: &[u8], position: u32, forward: bool, s: &mut impl Services) -> Result<u32> {
        let result = s
            .call(Call::Search {
                needle: needle.to_vec(),
                position,
                forward,
                text: self.text.clone(),
            })?
            .word()?;
        Ok(if result < self.text_length() { result } else { u32::MAX })
    }
}

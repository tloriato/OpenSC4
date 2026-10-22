// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent published TextEdit scenarios. All foreign outcomes are synthetic;
//! these are neither game/shadow measurements nor TE5/TE8 implementations.
use fc4_core::text_edit::*;

#[derive(Default)]
struct Host {
    calls: Vec<Call>,
    measured: Vec<Vec<u8>>,
    previous: Vec<u16>,
    selector: i32,
    height: u32,
    clipboard: bool,
    pasted: Option<Vec<u8>>,
    bars: bool,
    bar_value: u32,
    reject_a: bool,
    zero_consume: bool,
    search_result: u32,
}
impl Services for Host {
    fn has_font(&self) -> bool {
        true
    }
    fn line_height(&mut self) -> Result<u32> {
        Ok(if self.height == 0 { 10 } else { self.height })
    }
    fn measure(&mut self, bytes: &[u8]) -> Result<u32> {
        self.measured.push(bytes.to_vec());
        Ok(bytes.len() as u32 * 10)
    }
    fn fit(&mut self, bytes: &[u8], width: u32) -> Result<u32> {
        let to_lf = bytes.iter().position(|&b| b == b'\n').map_or(bytes.len(), |p| p + 1);
        Ok((to_lf as u32).min((width / 10).max(1)))
    }
    fn previous_utf16(&mut self, context: &[u8], index: u32, _: i32) -> Result<u32> {
        Ok(if index == 0 {
            b'P' as u32
        } else {
            context[index as usize - 1] as u32
        })
    }
    fn next_utf16(&mut self, bytes: &[u8], _: i32) -> Result<(u32, u32)> {
        Ok((bytes[0] as u32, u32::from(!self.zero_consume)))
    }
    fn thai_level(&mut self, current: u16, previous: u16) -> Result<u32> {
        self.previous.push(previous);
        Ok(u32::from(!(self.reject_a && current == b'A' as u16)))
    }
    fn composed_length(&mut self, bytes: &[u8], index: u32, previous: bool, selector: i32) -> Result<u32> {
        if previous {
            previous_char_length(&bytes[..index as usize], selector)
        } else {
            char_length(bytes, index, bytes.len() as u32 - index, selector)
        }
    }
    fn length_from_glyph_count(&mut self, bytes: &[u8], glyphs: u32, selector: i32) -> Result<u32> {
        let mut at = 0;
        for _ in 0..glyphs { at += char_length(bytes, at, bytes.len() as u32 - at, selector)?; }
        Ok(at)
    }
    fn call(&mut self, call: Call) -> Result<Reply> {
        let reply = match &call {
            Call::CodePage => Reply::Word(self.selector as u32),
            Call::LookupFont(_) => Reply::Handle(Some(1)),
            Call::AcquireClipboard => Reply::Handle(if self.clipboard { Some(2) } else { None }),
            Call::CreateBar { vertical, .. } => Reply::Handle(if self.bars {
                Some(if *vertical { 3 } else { 4 })
            } else {
                None
            }),
            Call::BarWindow(handle) => Reply::Handle(Some(*handle + 10)),
            Call::WindowSize(_) => Reply::Size(12, 10),
            Call::ImageRect(_) => Reply::Rectangle(Rect {
                left: 0,
                top: 0,
                right: 30,
                bottom: 60,
            }),
            Call::BarGetValue(_) => Reply::Word(self.bar_value),
            Call::ClipboardPaste(_) => Reply::Bytes(self.pasted.clone()),
            Call::Color { value, .. } => Reply::Word(*value),
            Call::Search { .. } => Reply::Word(self.search_result),
            Call::SoundAcquire { .. } => Reply::Sound { acquired: false, handle: Some(55) },
            Call::Timer { .. } | Call::Notify { .. } | Call::Base(_, _) | Call::SoundPlay(_) => Reply::Bool(true),
            _ => Reply::Unit,
        };
        self.calls.push(call);
        Ok(reply)
    }
}
fn editor(flags: u32, selector: i32, bytes: &[u8]) -> (TextEdit, Host) {
    let mut host = Host {
        selector,
        ..Host::default()
    };
    let mut edit = TextEdit::new(flags, selector, 42);
    edit.set_area(
        Rect {
            left: 0,
            top: 0,
            right: 210,
            bottom: 40,
        },
        &mut host,
    )
    .unwrap();
    edit.init(&mut host).unwrap();
    edit.set_text(Some(bytes), bytes.len() as u32, false, &mut host)
        .unwrap();
    host.calls.clear();
    host.measured.clear();
    (edit, host)
}
fn notify_count(host: &Host) -> usize {
    host.calls
        .iter()
        .filter(|c| matches!(c, Call::Notify { id: 0x9b1552d8, .. }))
        .count()
}
fn paint(edit: &TextEdit, host: &mut Host) -> Result<bool> {
    edit.paint(
        PaintContext {
            draw: Rect {
                left: 0,
                top: 0,
                right: 210,
                bottom: 40,
            },
            clip: Rect {
                left: 0,
                top: 0,
                right: 210,
                bottom: 40,
            },
        },
        PaintPalette {
            foreground: 1,
            background: 2,
            selected_foreground: 3,
            selected_background: 4,
        },
        host,
    )
}

#[test]
fn selection_is_ordered_and_unsigned_clamped() {
    let (mut e, mut h) = editor(1, 0, b"abcde");
    e.set_selection(4, 1, &mut h).unwrap();
    assert_eq!(
        e.position(),
        Position {
            line: 0,
            column: 1,
            index: 1,
            anchor: 4,
            end: 1
        }
    );
    e.set_selection(u32::MAX, 2, &mut h).unwrap();
    assert_eq!((e.position().anchor, e.position().end, e.position().index), (5, 2, 2));
    assert_eq!(notify_count(&h), 0);
}
#[test]
fn maximum_setter_preserves_existing_text_and_history() {
    let (mut e, mut h) = editor(0x101, 0, b"abcde");
    e.insert_high_level(b"x", &mut h).unwrap();
    h.calls.clear();
    let history = e.history_counts();
    e.set_maximum(2);
    assert_eq!(e.text(), b"xabcde");
    assert_eq!(e.history_counts(), history);
    assert!(h.calls.is_empty());
}
#[test]
fn set_text_count_zero_strlen_null_and_both_histories() {
    let (mut e, mut h) = editor(0x101, 0, b"old");
    e.insert_high_level(b"x", &mut h).unwrap();
    e.undo(&mut h).unwrap();
    e.set_text(Some(b"abc\0def"), 0, true, &mut h).unwrap();
    assert_eq!(e.text(), b"abc");
    assert_eq!(e.history_counts(), (0, 0));
    assert_eq!(notify_count(&h), 1);
    e.set_text(None, 99, false, &mut h).unwrap();
    assert!(e.text().is_empty());
    e.set_text(Some(b"a\0b"), 3, false, &mut h).unwrap();
    assert_eq!(e.text(), b"a\0b");
}
#[test]
fn set_text_multibyte_ceiling_uses_original_source_not_filtered_source() {
    let (mut e, mut h) = editor(0x201, 8, b"");
    e.set_maximum(1);
    e.set_text(Some(b"\xc3\xa9z"), 3, false, &mut h).unwrap();
    assert_eq!(e.text(), b"\xc3\xa9");
    e.set_text(Some(b"\n\xc3\xa9z"), 4, false, &mut h).unwrap();
    assert_eq!(e.text(), b"\xc3");
}
#[test]
fn ordinary_set_text_keeps_exact_byte_limit_including_zero() {
    let (mut e, mut h) = editor(1, 0, b"old");
    e.set_maximum(2);
    e.set_text(Some(b"abc"), 0, false, &mut h).unwrap();
    assert_eq!(e.text(), b"ab");
    e.set_maximum(0);
    e.set_text(Some(b"abc"), 0, false, &mut h).unwrap();
    assert!(e.text().is_empty());
}
#[test]
fn encoding_tables_preserve_malformed_disagreements() {
    assert_eq!(char_length(b"\xe0\x80", 0, 1, 8), Ok(3));
    assert_eq!(previous_char_length(b"\x80\xbf", 8), Ok(3));
    assert_eq!(glyph_count(b"\x80\xbf", 8), Ok(0));
    assert_eq!(char_length(b"\x81A", 0, 2, 100), Ok(1));
    assert_eq!(glyph_count(b"\x81A", 100), Ok(2));
    assert_eq!(char_length(b"\xe0", 0, 1, 8), Err(Boundary::BufferRange));
    assert_eq!(previous_char_length(b"\x81\x81A", 932), Ok(1));
    assert_eq!(previous_char_length(b"\x81A", 932), Ok(2));
    assert_eq!(glyph_count(b"\x81", 932), Ok(1));
}
#[test]
fn pending_input_c0_and_malformed_e0_are_not_unicode_validation() {
    let (mut e, mut h) = editor(1, 8, b"");
    assert!(e.character(0xc0, &mut h).unwrap());
    assert!(e.character(9, &mut h).unwrap());
    assert_eq!(e.pending_input(), b"\xc0");
    assert!(!e.character(b'A', &mut h).unwrap());
    assert_eq!(e.text(), b"\xc0A");
    assert!(e.character(0xe0, &mut h).unwrap());
    assert!(e.character(b'A', &mut h).unwrap());
    assert!(!e.character(b'B', &mut h).unwrap());
    assert_eq!(e.text(), b"\xc0A\xe0AB");
    assert!(e.pending_input().is_empty());
}
#[test]
fn captured_character_completion_clears_without_edit() {
    let (mut e, mut h) = editor(1, 8, b"");
    e.set_capture_state(true);
    assert!(e.character(0xc2, &mut h).unwrap());
    assert!(!e.character(0xa9, &mut h).unwrap());
    assert!(e.text().is_empty());
    assert!(e.pending_input().is_empty());
}
#[test]
fn lf_filter_retains_cr_and_composition_preserves_previous_on_erase() {
    let (e, mut h) = editor(0x201, 0, b"");
    let mut bytes = b"\r\nx".to_vec();
    assert_eq!(e.remove_bad_characters(&mut bytes), Ok(1));
    assert_eq!(bytes, b"\rx");
    h.reject_a = true;
    h.previous.clear();
    let mut candidate = b"AB".to_vec();
    assert_eq!(e.fix_composition(&mut candidate, b"", 0, &mut h), Ok(1));
    assert_eq!(candidate, b"B");
    assert_eq!(h.previous, vec![b'P' as u16, b'P' as u16]);
    h.zero_consume = true;
    assert_eq!(e.fix_composition(&mut candidate, b"", 0, &mut h), Ok(1));
    assert!(candidate.is_empty());
}
#[test]
fn raw_and_high_level_notify_history_and_glyph_movement_differ() {
    let (mut e, mut h) = editor(0x901, 8, b"");
    assert_eq!(e.insert_raw(b"\xc3\xa9", 0, &mut h), Ok(2));
    assert_eq!(e.position().index, 1);
    assert_eq!(e.history_counts(), (0, 0));
    assert_eq!(notify_count(&h), 0);
    e.set_insertion_index(2, &mut h).unwrap();
    e.insert_high_level(b"z", &mut h).unwrap();
    assert_eq!(e.history_counts(), (1, 0));
    assert_eq!(notify_count(&h), 1);
}
#[test]
fn replace_undo_redo_restores_all_five_words_without_notifications() {
    let (mut e, mut h) = editor(0x901, 0, b"abc\ndef");
    e.set_selection(6, 1, &mut h).unwrap();
    let saved = e.position();
    e.insert_high_level(b"XY", &mut h).unwrap();
    let after = e.position();
    assert_eq!(e.text(), b"aXYf");
    h.calls.clear();
    assert!(e.undo(&mut h).unwrap());
    assert_eq!(e.text(), b"abc\ndef");
    assert_eq!(e.position(), saved);
    assert!(e.redo(&mut h).unwrap());
    assert_eq!(e.text(), b"aXYf");
    assert_eq!(e.position(), after);
    assert_eq!(notify_count(&h), 0);
}
#[test]
fn insertion_only_quarter_eviction_and_zero_limit_sequence() {
    let (mut e, mut h) = editor(0x101, 0, b"");
    e.set_max_undo_levels(0);
    for count in [1, 2, 3, 4, 4] {
        e.insert_high_level(b"x", &mut h).unwrap();
        assert_eq!(e.history_counts().0, count);
    }
    let (mut e, mut h) = editor(0x101, 0, b"");
    e.set_max_undo_levels(8);
    for b in b"abcdefgh" {
        e.insert_high_level(&[*b], &mut h).unwrap();
    }
    assert_eq!(e.history_counts().0, 8);
    e.insert_high_level(b"i", &mut h).unwrap();
    assert_eq!(e.history_counts().0, 7);
    for _ in 0..7 {
        assert!(e.undo(&mut h).unwrap());
    }
    assert_eq!(e.text(), b"ab");
}
#[test]
fn deletion_recording_has_no_limit_and_lowering_does_not_prune() {
    let (mut e, mut h) = editor(0x101, 0, b"abcdefghi");
    e.set_max_undo_levels(0);
    for _ in 0..9 {
        e.key_down(0x2e, 0, &mut h).unwrap();
    }
    assert_eq!(e.history_counts().0, 9);
    e.set_max_undo_levels(1);
    assert_eq!(e.history_counts().0, 9);
}
#[test]
fn rejected_single_line_lf_does_not_delete_selection_or_create_history() {
    let (mut e, mut h) = editor(0x301, 0, b"abc");
    e.set_selection(0, 2, &mut h).unwrap();
    e.insert_high_level(b"\nxyz", &mut h).unwrap();
    assert_eq!(e.text(), b"abc");
    assert_eq!((e.position().anchor, e.position().end), (2, 2));
    assert_eq!(e.history_counts(), (0, 0));
}
#[test]
fn overwrite_and_zero_insert_do_not_roll_back_prior_deletion() {
    let (mut e, mut h) = editor(0x101, 0, b"abc");
    e.set_insert_mode(false);
    e.insert_high_level(b"Z", &mut h).unwrap();
    assert_eq!(e.text(), b"Zbc");
    assert!(e.undo(&mut h).unwrap());
    assert_eq!(e.text(), b"abc");
    e.set_selection(0, 2, &mut h).unwrap();
    e.set_maximum(1);
    e.insert_high_level(b"Q", &mut h).unwrap();
    assert_eq!(e.text(), b"c");
    assert_eq!(e.history_counts(), (0, 1));
}
#[test]
fn lowered_raw_limit_is_selector_specific_and_empty_remove_is_unsupported() {
    let (mut e, mut h) = editor(1, 0, b"abcde");
    e.set_maximum(2);
    assert_eq!(
        e.insert_raw(b"x", 0, &mut h),
        Err(Boundary::LoweredOrdinaryLimit {
            wrapped_count: 0xfffffffd
        })
    );
    let (mut e, mut h) = editor(1, 100, b"abcde");
    e.set_maximum(2);
    assert_eq!(e.insert_raw(b"x", 0, &mut h), Ok(0));
    assert_eq!(e.text(), b"abcde");
    let (mut e, mut h) = editor(1, 0, b"");
    assert_eq!(e.remove_char(0, true, None, &mut h), Err(Boundary::InvalidIndex));
}
#[test]
fn clipboard_cut_without_service_paste_password_and_no_ctrl_a() {
    let (mut e, mut h) = editor(0x1001, 0, b"abcde");
    e.set_selection(1, 3, &mut h).unwrap();
    e.key_down(0x58, 2, &mut h).unwrap();
    assert_eq!(e.text(), b"ade");
    assert!(!h.calls.iter().any(|c| matches!(c, Call::ClipboardCopy(..))));
    let mut host = Host {
        clipboard: true,
        pasted: Some(b"Q".to_vec()),
        ..Host::default()
    };
    let mut e = TextEdit::new(0x3000, 0, 42);
    e.init(&mut host).unwrap();
    e.set_text(Some(b"abc"), 3, false, &mut host).unwrap();
    e.set_selection(0, 2, &mut host).unwrap();
    e.key_down(0x43, 2, &mut host).unwrap();
    assert!(!host.calls.iter().any(|c| matches!(c, Call::ClipboardCopy(..))));
    e.key_down(0x56, 2, &mut host).unwrap();
    assert_eq!(e.text(), b"Qc");
    let p = e.position();
    e.key_down(0x41, 2, &mut host).unwrap();
    assert_eq!(e.position(), p);
}
#[test]
fn return_and_arrow_forwarding_do_not_require_editable_but_pages_do_not_forward() {
    let (mut e, mut h) = editor(0xc00, 0, b"abc");
    e.key_down(13, 0, &mut h).unwrap();
    e.key_down(0x28, 0, &mut h).unwrap();
    e.key_down(0x22, 0, &mut h).unwrap();
    let keys: Vec<_> = h
        .calls
        .iter()
        .filter_map(|c| {
            if let Call::Notify { id: 0x9b1552d5, b, .. } = c {
                Some(*b)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(keys, vec![13, 0x28]);
    assert_eq!(e.text(), b"abc");
}
#[test]
fn caret_focus_gate_period_clamp_timer_half_and_idempotence() {
    let (mut e, mut h) = editor(1, 0, b"abc");
    e.toggle_caret(&mut h).unwrap();
    assert!(!e.caret_visible());
    assert!(h.calls.is_empty());
    e.set_caret_period(1001, &mut h).unwrap();
    assert!(e.start_caret_timer(true, &mut h).unwrap());
    assert!(!e.start_caret_timer(true, &mut h).unwrap());
    assert_eq!(
        h.calls,
        vec![Call::Timer {
            caret: true,
            start: true,
            interval: 500,
            data: 42
        }]
    );
    e.set_focus_state(true);
    e.make_caret_visible(&mut h).unwrap();
    assert!(e.caret_visible());
    e.set_caret_period(0, &mut h).unwrap();
    assert_eq!(e.caret_period(), 250);
    e.set_caret_period(u32::MAX, &mut h).unwrap();
    assert_eq!(e.caret_period(), 10000);
}
#[test]
fn trailing_lf_line_counts_and_index_conversion_are_byte_based() {
    let (mut e, mut h) = editor(1, 0, b"a\nb");
    assert_eq!(e.line_starts(), &[0, 2]);
    assert_eq!(e.line_bytes(0), Ok(2));
    assert_eq!(e.line_bytes(99), Ok(1));
    assert_eq!(e.index_from_line_column(0, 3), (true, 3));
    e.set_text(Some(b"a\n"), 2, false, &mut h).unwrap();
    assert_eq!(e.line_starts(), &[0, 2]);
    e.set_area(
        Rect {
            left: 0,
            top: 0,
            right: 210,
            bottom: 7,
        },
        &mut h,
    )
    .unwrap();
    assert_eq!(e.visible_size().2, 1);
}
#[test]
fn distinct_pixel_hit_tests_and_no_vertical_gutter_subtraction() {
    let (mut e, mut h) = editor(1, 0, b"abc\ndef");
    e.set_gutter_x(0, &mut h).unwrap();
    e.set_gutter_y(7, &mut h).unwrap();
    assert_eq!(e.column_from_position(0, 5, &mut h), Ok(0));
    assert_eq!(e.column_from_position(0, 6, &mut h), Ok(0));
    assert_eq!(e.line_column_from_position(5, 0, &mut h), Ok((0, 0)));
    assert_eq!(e.line_column_from_position(6, 0, &mut h), Ok((0, 1)));
    assert_eq!(e.line_column_from_position(6, 10, &mut h), Ok((1, 1)));
    assert_eq!(e.line_column_from_position(100, -1, &mut h), Ok((0, 0)));
}
#[test]
fn page_uses_current_x_arrow_reuses_cache_and_both_preserve_it() {
    let text = b"abcdefgh\nab\nabcdefgh\nabcdefgh\nabcdefgh\nabcdefgh\nabcdefgh";
    let (mut e, mut h) = editor(1, 0, text);
    e.set_gutter_x(0, &mut h).unwrap();
    e.set_insertion_index(7, &mut h).unwrap();
    e.key_down(0x28, 0, &mut h).unwrap();
    assert_eq!(e.preferred_x(), 70);
    assert_eq!(e.position().line, 1);
    let mut page = e;
    let (mut arrow, mut arrow_host) = editor(1, 0, text);
    arrow.set_gutter_x(0, &mut arrow_host).unwrap();
    arrow.set_insertion_index(7, &mut arrow_host).unwrap();
    arrow.key_down(0x28, 0, &mut arrow_host).unwrap();
    page.key_down(0x22, 0, &mut h).unwrap();
    arrow.key_down(0x28, 0, &mut h).unwrap();
    assert_eq!(page.position().column, 1);
    assert_eq!(arrow.position().column, 6);
    assert_eq!(page.preferred_x(), 70);
    assert_eq!(arrow.preferred_x(), 70);
    arrow.key_down(0x24, 0, &mut h).unwrap();
    assert_eq!(arrow.preferred_x(), u32::MAX);
}
#[test]
fn single_line_three_quarter_threshold_is_strict() {
    let (mut e, mut h) = editor(0x201, 0, b"abcdefgh");
    e.set_area(
        Rect {
            left: 0,
            top: 0,
            right: 50,
            bottom: 40,
        },
        &mut h,
    )
    .unwrap();
    e.set_insertion_line_column(0, 6).unwrap();
    e.put_in_view(&mut h).unwrap();
    assert_eq!(e.scroll_offsets().0, 4);
    let (mut e, mut h) = editor(0x201, 0, b"abcdefgh");
    e.set_area(
        Rect {
            left: 0,
            top: 0,
            right: 50,
            bottom: 40,
        },
        &mut h,
    )
    .unwrap();
    e.set_insertion_line_column(0, 4).unwrap();
    e.put_in_view(&mut h).unwrap();
    assert_eq!(e.scroll_offsets().0, 0);
}
#[test]
fn word_selection_extends_preexisting_range_and_tabs_are_not_separators() {
    let (mut e, mut h) = editor(1, 0, b"abc def");
    e.set_selection(0, 5, &mut h).unwrap();
    e.select_current_word(&mut h).unwrap();
    assert_eq!((e.position().anchor, e.position().end), (0, 7));
    assert!(!TextEdit::separator(b'\t'));
    assert!(!TextEdit::separator(b','));
}
#[test]
fn buffer_get_text_copies_without_terminator() {
    let (e, _) = editor(1, 0, b"abc");
    let mut buffer = [0xee; 3];
    let mut n = 2;
    e.copy_text(&mut buffer, &mut n).unwrap();
    assert_eq!(buffer, [b'a', b'b', 0xee]);
    assert_eq!(n, 2);
}
#[test]
fn bars_fail_vertical_then_horizontal_clear_bits_and_still_return_true() {
    let mut e = TextEdit::new(12, 0, 42);
    let mut h = Host::default();
    assert!(e.add_scrollbars(true, true, &mut h).unwrap());
    assert_eq!(e.flags() & 12, 0);
    assert_eq!(
        h.calls,
        vec![
            Call::CreateBar {
                vertical: true,
                args: [0, 2, 0, 100, 0]
            },
            Call::CreateBar {
                vertical: false,
                args: [1, 1, 0, 100, 0]
            }
        ]
    );
}
#[test]
fn scrollbar_images_reference_even_same_identity_and_empty_rect_uses_image_rect() {
    let mut h = Host {
        bars: true,
        ..Host::default()
    };
    let mut e = TextEdit::new(8, 0, 42);
    e.add_scrollbars(true, false, &mut h).unwrap();
    e.set_bar_image(true, Some(90), Rect::default(), &mut h).unwrap();
    h.calls.clear();
    e.set_bar_image(true, Some(90), Rect::default(), &mut h).unwrap();
    assert_eq!(
        h.calls,
        vec![
            Call::AddRef(90),
            Call::Release(90),
            Call::ImageRect(90),
            Call::BarImage {
                bar: 3,
                image: 90,
                rect: Rect {
                    left: 0,
                    top: 0,
                    right: 30,
                    bottom: 60
                }
            }
        ]
    );
}
#[test]
fn background_identity_option_and_te7_unbalanced_teardown() {
    let (mut e, mut h) = editor(1, 0, b"abc");
    e.set_background(Some(90), 0, &mut h).unwrap();
    h.calls.clear();
    e.set_background(Some(90), 0, &mut h).unwrap();
    assert!(h.calls.is_empty());
    e.set_background(Some(90), 1, &mut h).unwrap();
    assert_eq!(h.calls, vec![Call::Invalidate]);
    h.calls.clear();
    e.shutdown(&mut h).unwrap();
    assert!(!h.calls.contains(&Call::Release(90)));
}
#[test]
fn paint_partial_selection_order_caret_float_width_reset_and_outline_last() {
    let (mut e, mut h) = editor(0x31, 0, b"abc");
    e.set_selection(0, 1, &mut h).unwrap();
    e.set_focus_state(true);
    e.make_caret_visible(&mut h).unwrap();
    h.calls.clear();
    paint(&e, &mut h).unwrap();
    let ordinary = h
        .calls
        .iter()
        .position(|c| matches!(c, Call::Draw { foreground: 1, .. }))
        .unwrap();
    let highlight = h
        .calls
        .iter()
        .enumerate()
        .find(|(i, c)| *i > ordinary && matches!(c, Call::Fill { .. }))
        .unwrap()
        .0;
    let selected = h
        .calls
        .iter()
        .position(|c| matches!(c, Call::Draw { foreground: 3, .. }))
        .unwrap();
    assert!(ordinary < highlight && highlight < selected);
    assert_eq!(
        h.calls.last(),
        Some(&Call::Outline {
            rect: Rect {
                left: 0,
                top: 0,
                right: 210,
                bottom: 40
            },
            color: 0
        })
    );
    for height in [23, 24] {
        h.height = height;
        e.recalculate(&mut h).unwrap();
        h.calls.clear();
        paint(&e, &mut h).unwrap();
        let widths: Vec<_> = h
            .calls
            .iter()
            .filter_map(|c| if let Call::PenWidth(w) = c { Some(*w) } else { None })
            .collect();
        assert_eq!(widths, vec![if height < 24 { 1.0 } else { 2.0 }, 1.0]);
    }
}
#[test]
fn password_mask_keeps_byte_column_and_unsafe_mixed_span_is_boundary() {
    let (mut e, mut h) = editor(0x2001, 8, b"\xc3\xa9z");
    e.set_selection(0, 2, &mut h).unwrap();
    e.set_focus_state(true);
    e.make_caret_visible(&mut h).unwrap();
    assert_eq!(e.password_buffer(), Ok(b"**".to_vec()));
    // Line drawing still requests original byte count 3 from a two-byte mask.
    assert_eq!(paint(&e, &mut h), Err(Boundary::BufferRange));
    // Inspect a safe prefix separately: stored byte column 2 measures two mask bytes.
    assert_eq!(e.position().column, 2);
}
#[test]
fn gutter_y_only_invalidates_and_color_conversion_slots_differ() {
    let (mut e, mut h) = editor(1, 0, b"abc");
    let rows = e.visible_size();
    e.set_gutter_y(3, &mut h).unwrap();
    assert_eq!(h.calls, vec![Call::Invalidate]);
    assert_eq!(e.visible_size(), rows);
    h.calls.clear();
    e.set_caret_color(11, &mut h).unwrap();
    e.set_highlight_fill(12, &mut h).unwrap();
    assert_eq!(
        h.calls,
        vec![
            Call::Color { slot: 0x200, value: 11 },
            Call::Color { slot: 0x1fc, value: 12 }
        ]
    );
}
#[test]
fn wheel_has_no_residual_and_any_flag_bit_counts() {
    let (mut e, mut h) = editor(1, 0, b"a\nb\nc\nd\ne");
    assert!(e.is_flag_set(3));
    e.wheel(99, &mut h).unwrap();
    e.wheel(99, &mut h).unwrap();
    assert!(h.calls.is_empty());
}
#[test]
fn find_filters_exact_end_without_inventing_search_semantics() {
    let (e, mut h) = editor(1, 0, b"abc");
    h.search_result = 3;
    assert_eq!(e.find_text(b"", 0, false, &mut h), Ok(u32::MAX));
    h.search_result = 2;
    assert_eq!(e.find_text(b"", 0, true, &mut h), Ok(2));
}

#[test]
fn sound_failure_still_releases_created_handle_without_playing() {
    let (mut e, mut h) = editor(0x201, 0, b"abc");
    e.set_sound_id(0, 77).unwrap();
    e.set_text(Some(b"\nx"), 2, false, &mut h).unwrap();
    let acquire = h.calls.iter().position(|c| matches!(c, Call::SoundAcquire { id: 77, .. })).unwrap();
    let release = h.calls.iter().position(|c| *c == Call::Release(55)).unwrap();
    assert!(acquire < release);
    assert!(!h.calls.iter().any(|c| matches!(c, Call::SoundPlay(_))));
}

#[test]
fn scroll_page_down_can_overshoot_last_full_page() {
    let mut h = Host { bars: true, ..Host::default() };
    let mut e = TextEdit::new(9, 0, 42);
    e.set_area(Rect { left: 0, top: 0, right: 210, bottom: 40 }, &mut h).unwrap();
    e.init(&mut h).unwrap();
    e.set_text(Some(b"0\n1\n2\n3\n4\n5\n6\n7\n8\n9"), 19, false, &mut h).unwrap();
    h.bar_value = 5;
    e.message(3, 0x887113a3, 0, (0, 0), &mut h).unwrap();
    assert_eq!(e.visible_size().2, 4);
    assert!(e.scroll(Scroll::PageDown, &mut h).unwrap());
    assert_eq!(e.first_visible_line(), 9);
}

#[test]
fn pixel_only_scroll_start_gate_and_left_signed_underflow() {
    let mut h = Host { bars: true, height: 16, ..Host::default() };
    let mut e = TextEdit::new(5, 0, 42);
    e.set_area(Rect { left: 0, top: 0, right: 50, bottom: 40 }, &mut h).unwrap();
    e.init(&mut h).unwrap();
    // Init removes an unnecessary horizontal bar but leaves its enable bit.
    e.set_text(Some(b"abcdefghij"), 10, false, &mut h).unwrap();
    h.bar_value = 5;
    e.message(3, 0x887113a3, 1, (0, 0), &mut h).unwrap();
    assert!(!e.scroll(Scroll::Start, &mut h).unwrap());
    assert_eq!(e.scroll_offsets().1, 5);
    e.scroll(Scroll::Left, &mut h).unwrap();
    assert_eq!(e.scroll_offsets().1 as i32, -11);
}

#[test]
fn insertion_repair_caps_active_end_but_retains_excessive_anchor() {
    let (mut e, mut h) = editor(1, 0, b"abc");
    e.set_selection(3, 3, &mut h).unwrap();
    e.remove_raw(0, 2, None, &mut h).unwrap();
    e.fix_insertion().unwrap();
    assert_eq!((e.position().anchor, e.position().end), (3, 1));
    e.set_text(Some(b"a\nb"), 3, false, &mut h).unwrap();
    e.adjust_line_array(3, 1).unwrap();
    assert_eq!(e.line_starts(), &[0, 2]);
}

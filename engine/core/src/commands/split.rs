// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `RZParseDelimitedText` (command-system.md §2).

/// Blanks are exactly space and tab (§2); CR, LF and NUL are ordinary characters.
fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// The `(text, length)` pair of the C interfaces: `length = 0xFFFFFFFF` means `strlen(text)`
/// (up to the first NUL), any other length reads that many bytes (a NUL inside is ordinary).
/// A length beyond the slice is clamped to it (a Rust slice carries its own bound).
pub fn line_of(text: &[u8], length: u32) -> &[u8] {
    if length == u32::MAX {
        let n = text.iter().position(|&b| b == 0).unwrap_or(text.len());
        &text[..n]
    } else {
        &text[..(length as usize).min(text.len())]
    }
}

/// The quote rule (§2.1 rule 5): a word that starts and ends with `"` loses its last
/// character and then, if not empty, its first.
fn strip_quotes(mut w: Vec<u8>) -> Vec<u8> {
    if w.first() == Some(&b'"') && w.last() == Some(&b'"') {
        w.pop();
        if !w.is_empty() {
            w.remove(0);
        }
    }
    w
}

/// `RZParseDelimitedText(text, length, delim, words, count)` (§2). `capacity` is `*count` on
/// entry. Returns the boolean result and the words written (`*count` on return is their
/// number): false when the text has more words than `capacity`, with the first `capacity`
/// words.
pub fn parse_delimited_text(text: &[u8], delim: u8, capacity: usize) -> (bool, Vec<Vec<u8>>) {
    let mut words: Vec<Vec<u8>> = Vec::new();
    let skip_blanks = |mut i: usize| {
        while i < text.len() && is_blank(text[i]) {
            i += 1;
        }
        i
    };
    // Rule 1: leading blanks. The original's right trim of the whole text tests the wrong
    // byte and never removes anything (§2.2) *[quirk]*, so there is none here.
    let mut start = skip_blanks(0);
    if start == text.len() {
        return (true, words);
    }
    let mut quotes = 0usize;
    let mut i = start;
    while i < text.len() {
        let b = text[i];
        if b == b'"' {
            quotes += 1;
        }
        let is_break = if delim == b' ' { is_blank(b) } else { b == delim };
        if is_break && quotes % 2 == 0 {
            let mut w = &text[start..i];
            if delim != b' ' {
                // A word ended by a delimiter loses its trailing blanks (§2.2).
                while let Some((&last, rest)) = w.split_last() {
                    if !is_blank(last) {
                        break;
                    }
                    w = rest;
                }
            }
            if words.len() == capacity {
                return (false, words);
            }
            words.push(strip_quotes(w.to_vec()));
            // The blanks after a break are skipped (a run of blanks is one break).
            i = skip_blanks(i + 1);
            start = i;
            quotes = 0;
            continue;
        }
        i += 1;
    }
    // Rule 4: the text after the last break is the last word; it keeps its trailing blanks
    // (they can remain only after an odd number of quotes, or with another delimiter).
    // [choice] GC6: an empty remainder adds no word, also after a trailing non-blank
    // delimiter (`a,` gives one word).
    if start < text.len() {
        if words.len() == capacity {
            return (false, words);
        }
        words.push(strip_quotes(text[start..].to_vec()));
    }
    (true, words)
}

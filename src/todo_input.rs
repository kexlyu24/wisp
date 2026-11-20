//! Single-line editor for the layered To-do field (native child controls cannot
//! participate in the parent's UpdateLayeredWindow surface).
use std::{cell::Cell, ops::Range};
// shortcut: navigation uses Unicode scalar boundaries, not grapheme clusters or
// bidi visual order; use a shaping-aware editor when those are required.

pub(crate) struct Editor {
    limit: usize,
    pub caret: usize,
    pub anchor: usize,
    pub dragging: bool,
    pub scroll: Cell<f64>,
    high_surrogate: u16,
}
impl Default for Editor {
    fn default() -> Self {
        Self::with_limit(120)
    }
}
impl Editor {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            caret: 0,
            anchor: 0,
            dragging: false,
            scroll: Cell::new(0.),
            high_surrogate: 0,
        }
    }
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }
    pub fn move_to(&mut self, text: &str, mut pos: usize, extend: bool) {
        pos = pos.min(text.len());
        while !text.is_char_boundary(pos) {
            pos -= 1;
        }
        self.caret = pos;
        if !extend {
            self.anchor = pos;
        }
        self.high_surrogate = 0;
    }
    pub fn navigate(&mut self, text: &str, key: usize, shift: bool) {
        let range = self.selection();
        let pos = match key {
            0x24 => 0,
            0x23 => text.len(),
            0x25 if !shift && !range.is_empty() => range.start,
            0x27 if !shift && !range.is_empty() => range.end,
            0x25 => text[..self.caret]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i),
            0x27 => text[self.caret..]
                .chars()
                .next()
                .map_or(self.caret, |c| self.caret + c.len_utf8()),
            _ => return,
        };
        self.move_to(text, pos, shift);
    }
    pub fn replace(&mut self, text: &mut String, value: &str) {
        let range = self.selection();
        let room = self.limit.saturating_sub(
            text[..range.start].chars().count() + text[range.end..].chars().count(),
        );
        let value: String = value
            .chars()
            .filter(|c| !c.is_control() && *c != '\u{2028}' && *c != '\u{2029}')
            .take(room)
            .collect();
        // An empty/unsupported paste must not erase the selection.
        if value.is_empty() {
            return;
        }
        text.replace_range(range.clone(), &value);
        self.move_to(text, range.start + value.len(), false);
    }
    pub fn delete(&mut self, text: &mut String, back: bool) {
        if self.selection().is_empty() {
            self.navigate(text, if back { 0x25 } else { 0x27 }, true);
        }
        let range = self.selection();
        text.replace_range(range.clone(), "");
        self.move_to(text, range.start, false);
    }
    pub fn character(&mut self, text: &mut String, unit: u16) {
        if (0xd800..=0xdbff).contains(&unit) {
            self.high_surrogate = unit;
            return;
        }
        let ch = if (0xdc00..=0xdfff).contains(&unit) {
            char::decode_utf16([self.high_surrogate, unit])
                .next()
                .and_then(Result::ok)
        } else {
            char::from_u32(unit as u32)
        };
        self.high_surrogate = 0;
        if let Some(ch) = ch {
            self.replace(text, ch.encode_utf8(&mut [0; 4]));
        }
    }
    pub fn end_interaction(&mut self) {
        self.dragging = false;
        self.high_surrogate = 0;
    }
    pub fn viewport(&self, caret_x: f64, total: f64, width: f64) -> f64 {
        let mut offset = self.scroll.get().min((total - width).max(0.));
        if caret_x < offset {
            offset = caret_x;
        }
        if caret_x > offset + width {
            offset = caret_x - width;
        }
        self.scroll.set(offset.max(0.));
        self.scroll.get()
    }
    pub fn hit(text: &str, x: f64, mut measure: impl FnMut(&str) -> f64) -> usize {
        let mut prev = 0.;
        for (i, c) in text.char_indices() {
            let next = measure(&text[..i + c.len_utf8()]);
            if x < (prev + next) / 2. {
                return i;
            }
            prev = next;
        }
        text.len()
    }
}

use std::ffi::c_void;
type Handle = *mut c_void;
#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(owner: Handle) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(format: u32) -> Handle;
    fn SetClipboardData(format: u32, data: Handle) -> Handle;
}
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalFree(mem: Handle) -> Handle;
    fn GlobalLock(mem: Handle) -> *mut c_void;
    fn GlobalUnlock(mem: Handle) -> i32;
    fn GlobalSize(mem: Handle) -> usize;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_selection_and_deletion_are_unicode_safe() {
        let mut e = Editor::default();
        let mut text = "a😀éz".to_string();
        e.navigate(&text, 0x23, false);
        e.navigate(&text, 0x25, true);
        e.navigate(&text, 0x25, true);
        assert_eq!(&text[e.selection()], "éz");
        e.navigate(&text, 0x27, false);
        assert_eq!(e.caret, text.len());
        e.navigate(&text, 0x24, false);
        e.delete(&mut text, false);
        assert_eq!(text, "😀éz");
        e.navigate(&text, 0x27, false);
        e.delete(&mut text, true);
        assert_eq!(text, "éz");
        e.navigate(&text, 0x23, true);
        e.delete(&mut text, true);
        assert_eq!(text, "");
        e.delete(&mut text, true);
        e.delete(&mut text, false);
        assert_eq!(e.caret, 0);
    }
    #[test]
    fn paste_filters_controls_and_limits_replacement_not_original_length() {
        let mut e = Editor::default();
        let mut text = "a".repeat(120);
        e.move_to(&text, 0, false);
        e.move_to(&text, text.len(), true);
        e.replace(&mut text, "\r\n\t\0");
        assert_eq!(text.len(), 120);
        e.replace(&mut text, &"中".repeat(121));
        assert_eq!(text.chars().count(), 120);
        assert_eq!(e.caret, text.len());
        e.move_to(&text, 0, false);
        e.move_to(&text, text.len(), true);
        e.replace(&mut text, "a\r\nb\tc\u{2028}d");
        assert_eq!(text, "abcd");
    }
    #[test]
    fn utf16_pairs_and_lifecycle_do_not_leak_surrogates() {
        let mut e = Editor::default();
        let mut text = String::new();
        e.character(&mut text, 0xd83d);
        e.character(&mut text, 0xde00);
        assert_eq!(text, "😀");
        e.character(&mut text, 0xd83d);
        e.end_interaction();
        e.character(&mut text, 0xde00);
        assert_eq!(text, "😀");
        e.character(&mut text, 0xd83d);
        e.character(&mut text, b'x' as u16);
        e.character(&mut text, 0xde00);
        assert_eq!(text, "😀x");
    }
    #[test]
    fn viewport_tracks_caret_and_hit_testing_uses_character_midpoints() {
        let e = Editor::default();
        assert_eq!(e.viewport(100., 100., 30.), 70.);
        assert_eq!(e.viewport(0., 100., 30.), 0.);
        assert_eq!(e.viewport(10., 10., 30.), 0.);
        let measure = |s: &str| s.chars().count() as f64 * 10.;
        assert_eq!(Editor::hit("a😀é", -10., measure), 0);
        assert_eq!(Editor::hit("a😀é", 14., measure), 1);
        assert_eq!(Editor::hit("a😀é", 16., measure), 5);
        assert_eq!(Editor::hit("a😀é", 100., measure), 7);
    }
}

struct Clipboard;
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

/// Failure leaves the editor unchanged; Windows takes ownership only on success.
pub unsafe fn copy(owner: Handle, text: &str) -> bool {
    if owner.is_null() || text.is_empty() {
        return false;
    }
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let mem = GlobalAlloc(2, wide.len() * 2); // GMEM_MOVEABLE
    if mem.is_null() {
        return false;
    }
    let ptr = GlobalLock(mem) as *mut u16;
    if ptr.is_null() {
        GlobalFree(mem);
        return false;
    }
    ptr.copy_from_nonoverlapping(wide.as_ptr(), wide.len());
    GlobalUnlock(mem);
    if OpenClipboard(owner) == 0 {
        GlobalFree(mem);
        return false;
    }
    let _clipboard = Clipboard;
    if EmptyClipboard() == 0 || SetClipboardData(13, mem).is_null() {
        GlobalFree(mem);
        return false;
    }
    true
}
pub unsafe fn paste(owner: Handle) -> Option<String> {
    if OpenClipboard(owner) == 0 {
        return None;
    }
    let _clipboard = Clipboard;
    let mem = GetClipboardData(13); // CF_UNICODETEXT
    if mem.is_null() {
        return None;
    }
    let len = GlobalSize(mem) / 2;
    if len == 0 {
        return None;
    }
    let ptr = GlobalLock(mem) as *const u16;
    if ptr.is_null() {
        return None;
    }
    let units = std::slice::from_raw_parts(ptr, len);
    // Bound all reads by the global allocation; reject malformed clipboard data.
    let result = units
        .iter()
        .position(|&u| u == 0)
        .and_then(|n| String::from_utf16(&units[..n]).ok());
    GlobalUnlock(mem);
    result
}

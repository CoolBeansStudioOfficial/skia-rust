// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `UText` as ICU's break iterators use it: a text with native indexes (UTF-8 bytes for
//! `utext_openUTF8`, UTF-16 units for `utext_openUChars` and `utext_openUnicodeString`), read code
//! point by code point (`utext.h`, `utext.cpp`).
//!
//! The text is decoded once: UTF-8 as ICU's UTF-8 provider does (each maximal ill-formed subpart is
//! one U+FFFD), UTF-16 with unpaired surrogates as themselves. The iteration position is always the
//! start of a code point, or the end.

use super::ubrk::{TextRef, code_points};

/// `U_SENTINEL`: what `utext_next32` and friends return past either end.
pub(crate) const U_SENTINEL: i32 = -1;

/// A `UText` over a text the shim owns (see the module documentation).
#[derive(Clone, Debug)]
pub(crate) struct UText {
    /// The native index of each code point.
    starts: Vec<i32>,
    /// The code points.
    cps: Vec<i32>,
    /// `utext_nativeLength`.
    len: i32,
    /// The index in `cps` of the code point at the iteration position (`cps.len()` at the end).
    idx: usize,
    /// Whether the text is one contiguous UTF-16 chunk with native indexes equal to its unit
    /// indexes (`UTEXT_PROVIDER_STABLE_CHUNKS` with the whole text in the chunk: `utext_openUChars`
    /// and `utext_openUnicodeString`, not `utext_openUTF8`).
    utf16: bool,
}

fn to_i32(n: usize) -> i32 {
    i32::try_from(n).expect("ICU text offsets are int32")
}

impl UText {
    /// `utext_openUTF8` or `utext_openUChars` over `text`.
    pub(crate) fn open(text: TextRef<'_>) -> Self {
        let (len, utf16) = match text {
            TextRef::Utf8(t) => (t.len(), false),
            TextRef::Utf16(t) => (t.len(), true),
        };
        let decoded = code_points(&text);
        UText {
            starts: decoded.iter().map(|&(p, _)| to_i32(p)).collect(),
            cps: decoded
                .iter()
                .map(|&(_, c)| i32::try_from(c).expect("code points fit in int32"))
                .collect(),
            len: to_i32(len),
            idx: 0,
            utf16,
        }
    }

    /// `utext_openUnicodeString` over UTF-16 units.
    pub(crate) fn open_utf16(units: &[u16]) -> Self {
        Self::open(TextRef::Utf16(units))
    }

    /// Whether the text is one stable UTF-16 chunk (see the field).
    pub(crate) fn is_utf16(&self) -> bool {
        self.utf16
    }

    /// `utext_nativeLength`.
    pub(crate) fn native_length(&self) -> i32 {
        self.len
    }

    /// `utext_getNativeIndex`.
    pub(crate) fn get_native_index(&self) -> i32 {
        self.starts.get(self.idx).copied().unwrap_or(self.len)
    }

    /// `utext_setNativeIndex`: the index pinned to the text and moved to the start of the code
    /// point that holds it.
    pub(crate) fn set_native_index(&mut self, index: i32) {
        self.idx = if index >= self.len {
            self.cps.len()
        } else {
            let index = index.max(0);
            self.starts.partition_point(|&s| s <= index) - 1
        };
    }

    /// `utext_current32`.
    pub(crate) fn current32(&self) -> i32 {
        self.cps.get(self.idx).copied().unwrap_or(U_SENTINEL)
    }

    /// `utext_next32` (and `UTEXT_NEXT32`).
    pub(crate) fn next32(&mut self) -> i32 {
        match self.cps.get(self.idx) {
            Some(&c) => {
                self.idx += 1;
                c
            }
            None => U_SENTINEL,
        }
    }

    /// `utext_previous32` (and `UTEXT_PREVIOUS32`).
    pub(crate) fn previous32(&mut self) -> i32 {
        if self.idx == 0 {
            U_SENTINEL
        } else {
            self.idx -= 1;
            self.cps[self.idx]
        }
    }

    /// `utext_moveIndex32(ut, delta)` for `delta >= 0`: false if the end came first.
    pub(crate) fn move_index32(&mut self, delta: i32) -> bool {
        for _ in 0..delta {
            if self.next32() == U_SENTINEL {
                return false;
            }
        }
        true
    }

    /// `utext_char32At`: the code point that holds `index`, which becomes the iteration position;
    /// `U_SENTINEL` outside the text.
    pub(crate) fn char32_at(&mut self, index: i32) -> i32 {
        if index < 0 || index >= self.len {
            self.set_native_index(index);
            return U_SENTINEL;
        }
        self.set_native_index(index);
        self.current32()
    }

    /// `utext_getPreviousNativeIndex`: the start of the code point before the iteration position
    /// (0 at the start).
    pub(crate) fn get_previous_native_index(&self) -> i32 {
        if self.idx == 0 {
            0
        } else {
            self.starts[self.idx - 1]
        }
    }
}

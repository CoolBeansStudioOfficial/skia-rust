// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `bidi` section: `ubidi_openSized`, `ubidi_setPara`, `ubidi_getLength`, `ubidi_getDirection` and
//! `ubidi_getLevelAt` of `SkUnicode_icu_bidi.cpp#L36-L60`, through `unicode-bidi =0.3.18` with ICU4X's
//! `BidiClass` data. The paragraph level is `UBIDI_LTR`, `UBIDI_RTL` (what Skia passes) or
//! `UBIDI_DEFAULT_LTR` / `UBIDI_DEFAULT_RTL`.

use std::fmt::Write as _;

use icu_properties::CodePointMapData;
use icu_properties::props::BidiClass;
use unicode_bidi::{BidiInfo, Direction, Level, get_base_direction_with_data_source};

use crate::{Source, row};

/// The paragraph level handed to `ubidi_setPara`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Para {
    /// `UBIDI_LTR`.
    Ltr,
    /// `UBIDI_RTL`.
    Rtl,
    /// `UBIDI_DEFAULT_LTR`: the first strong character decides, left to right without one.
    AutoLtr,
    /// `UBIDI_DEFAULT_RTL`.
    AutoRtl,
}

/// The embedding level of every UTF-16 code unit of `text`.
///
/// `unicode-bidi` has no "default RTL" paragraph level: for each paragraph the first strong
/// character is looked up with [`get_base_direction_with_data_source`] and the level passed
/// explicitly.
#[must_use]
pub fn levels_utf16(text: &str, para: Para) -> Vec<u8> {
    let data = CodePointMapData::<BidiClass>::new();
    let paragraphs = BidiInfo::new_with_data_source(&data, text, None).paragraphs;
    let mut out = Vec::new();
    for p in &paragraphs {
        let slice = &text[p.range.clone()];
        let level = match para {
            Para::Ltr => Some(Level::ltr()),
            Para::Rtl => Some(Level::rtl()),
            Para::AutoLtr => None,
            Para::AutoRtl => Some(match get_base_direction_with_data_source(&data, slice) {
                Direction::Ltr => Level::ltr(),
                _ => Level::rtl(),
            }),
        };
        let info = BidiInfo::new_with_data_source(&data, slice, level);
        // ICU's levels have rule L1 applied to the paragraph as one line (separators and trailing
        // white space at the paragraph level); `BidiInfo::levels` does not.
        let per_char = info.paragraphs.first().map_or_else(Vec::new, |para| {
            info.reordered_levels_per_char(para, 0..slice.len())
        });
        for (c, lv) in slice.chars().zip(per_char) {
            for _ in 0..c.len_utf16() {
                out.push(lv.number());
            }
        }
    }
    out
}

/// `ubidi_getDirection` from the levels: `UBIDI_LTR` (0) if all are 0, `UBIDI_RTL` (1) if all are 1,
/// else `UBIDI_MIXED` (2). This is the stock definition; ICU's is in `ubidi.cpp`.
#[must_use]
pub fn direction(levels: &[u8]) -> u8 {
    if levels.iter().all(|&l| l == 0) {
        0
    } else if levels.iter().all(|&l| l == 1) {
        1
    } else {
        2
    }
}

/// `dir=<d> len=<n> levels=<level>*<count>,...`.
#[must_use]
pub fn describe(text: &str, para: Para) -> (String, usize) {
    let levels = levels_utf16(text, para);
    let mut out = format!("dir={} len={} levels=", direction(&levels), levels.len());
    let mut i = 0;
    let mut first = true;
    while i < levels.len() {
        let mut j = i;
        while j < levels.len() && levels[j] == levels[i] {
            j += 1;
        }
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "{}*{}", levels[i], j - i);
        i = j;
    }
    (out, levels.len())
}

/// Prints the section.
#[must_use]
pub fn run(corpus: &[Source], dump: bool) -> String {
    let mut out = String::new();
    for (name, para) in [
        ("bidi-ltr", Para::Ltr),
        ("bidi-rtl", Para::Rtl),
        ("bidi-auto-ltr", Para::AutoLtr),
        ("bidi-auto-rtl", Para::AutoRtl),
    ] {
        for s in corpus {
            for (i, text) in s.strings.iter().enumerate() {
                let (detail, _) = describe(text, para);
                // The driver's `n` is the UTF-16 length of the string.
                out += &row(name, s, i, text.encode_utf16().count(), &detail, dump);
            }
        }
    }
    out
}

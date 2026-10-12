// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `props` section: the property calls of `SkUnicode_icu.cpp#L420-L470` over all code points
//! (0..=10FFFF, surrogates included), through `skia_rust_unicode::icu_shim` (M2), which answers
//! them from `icu_properties =2.3.0` with ICU's numbering and definitions:
//!
//! * `u_hasBinaryProperty` with `UCHAR_EMOJI`, `UCHAR_EMOJI_COMPONENT`, `UCHAR_EMOJI_MODIFIER_BASE`,
//!   `UCHAR_EMOJI_MODIFIER`, `UCHAR_REGIONAL_INDICATOR`, `UCHAR_IDEOGRAPHIC`;
//! * `u_getIntPropertyValue(UCHAR_LINE_BREAK)` as ICU's numeric `ULineBreak`;
//! * `uscript_getScript` as ICU's numeric `UScriptCode`;
//! * `u_iscntrl`, `u_isspace` and `u_isWhitespace`.

use std::fmt::Write as _;

use skia_rust_unicode::icu_shim::{
    UProperty, u_get_int_property_value, u_has_binary_property, u_is_whitespace, u_iscntrl,
    u_isspace, uscript_get_script,
};

/// One run per line, `<kind> <name> <first> <last> <value>`, for a function of the code point.
/// A run only spans code points when `mergeable(value)`; otherwise every code point is its own line
/// unless the next has the same value (then they are merged as well when `merge_all`).
pub(crate) fn runs(
    out: &mut String,
    kind: &str,
    name: &str,
    value: &dyn Fn(u32) -> String,
    mergeable: &dyn Fn(&str) -> bool,
) {
    let mut start = 0u32;
    let mut cur = value(0);
    for c in 1..=0x11_0000u32 {
        let v = if c < 0x11_0000 {
            value(c)
        } else {
            String::from("\u{1}end")
        };
        if v == cur && mergeable(&cur) {
            continue;
        }
        let _ = writeln!(out, "{kind} {name} {start:x} {:x} {cur}", c - 1);
        cur = v;
        start = c;
    }
}

/// The code point as ICU's `UChar32`.
fn uchar32(c: u32) -> i32 {
    i32::try_from(c).expect("code points fit in int32")
}

/// Prints the section.
#[must_use]
pub fn run() -> String {
    let mut out = String::new();
    let all = |_: &str| true;
    let flag = |b: bool| String::from(if b { "1" } else { "0" });

    for (name, prop) in [
        ("emoji", UProperty::Emoji),
        ("emoji-component", UProperty::EmojiComponent),
        ("emoji-modifier-base", UProperty::EmojiModifierBase),
        ("emoji-modifier", UProperty::EmojiModifier),
        ("regional-indicator", UProperty::RegionalIndicator),
        ("ideographic", UProperty::Ideographic),
    ] {
        runs(
            &mut out,
            "prop",
            name,
            &|c| flag(u_has_binary_property(uchar32(c), prop)),
            &all,
        );
    }
    runs(
        &mut out,
        "prop",
        "line-break",
        &|c| u_get_int_property_value(uchar32(c), UProperty::LineBreak).to_string(),
        &all,
    );
    runs(
        &mut out,
        "prop",
        "iscntrl",
        &|c| flag(u_iscntrl(uchar32(c))),
        &all,
    );
    runs(
        &mut out,
        "prop",
        "isspace",
        &|c| flag(u_isspace(uchar32(c))),
        &all,
    );
    runs(
        &mut out,
        "prop",
        "iswhitespace",
        &|c| flag(u_is_whitespace(uchar32(c))),
        &all,
    );
    runs(
        &mut out,
        "prop",
        "script",
        &|c| uscript_get_script(uchar32(c)).to_string(),
        &all,
    );
    out
}

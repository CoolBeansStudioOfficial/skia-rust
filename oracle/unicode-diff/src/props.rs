// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `props` section: the property calls of `SkUnicode_icu.cpp#L420-L470` over all code points
//! (0..=10FFFF, surrogates included), through `icu_properties =2.3.0`.
//!
//! * `u_hasBinaryProperty` with `UCHAR_EMOJI`, `UCHAR_EMOJI_COMPONENT`, `UCHAR_EMOJI_MODIFIER_BASE`,
//!   `UCHAR_EMOJI_MODIFIER`, `UCHAR_REGIONAL_INDICATOR`, `UCHAR_IDEOGRAPHIC`;
//! * `u_getIntPropertyValue(UCHAR_LINE_BREAK)` as ICU's numeric `ULineBreak`;
//! * `uscript_getScript` as ICU's numeric `UScriptCode`;
//! * `u_iscntrl`, `u_isspace` and `u_isWhitespace`, which ICU4X has no function for: they are the
//!   definitions of `uchar.cpp#L197-L240` over the General_Category.

// ICU4X deprecates `to_icu4c_value` (unicode-org/icu4x#6067). The comparison needs ICU's numbering,
// and M2's shim will need the same table, so it is used deliberately.
#![allow(deprecated)]

use std::fmt::Write as _;

use icu_properties::props::{
    Emoji, EmojiComponent, EmojiModifier, EmojiModifierBase, GeneralCategory, Ideographic,
    LineBreak, RegionalIndicator, Script,
};
use icu_properties::{CodePointMapData, CodePointSetData};

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

/// The general category of `c`.
fn gc(map: &CodePointMapData<GeneralCategory>, c: u32) -> GeneralCategory {
    map.as_borrowed().get32(c)
}

fn is_z(g: GeneralCategory) -> bool {
    matches!(
        g,
        GeneralCategory::SpaceSeparator
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    )
}

/// Prints the section.
#[must_use]
pub fn run() -> String {
    let mut out = String::new();
    let all = |_: &str| true;
    let flag = |b: bool| String::from(if b { "1" } else { "0" });

    macro_rules! binary {
        ($name:expr, $prop:ty) => {{
            let set = CodePointSetData::new::<$prop>();
            runs(&mut out, "prop", $name, &|c| flag(set.contains32(c)), &all);
        }};
    }
    binary!("emoji", Emoji);
    binary!("emoji-component", EmojiComponent);
    binary!("emoji-modifier-base", EmojiModifierBase);
    binary!("emoji-modifier", EmojiModifier);
    binary!("regional-indicator", RegionalIndicator);
    binary!("ideographic", Ideographic);

    let lb = CodePointMapData::<LineBreak>::new();
    runs(
        &mut out,
        "prop",
        "line-break",
        &|c| lb.get32(c).to_icu4c_value().to_string(),
        &all,
    );

    let cats = CodePointMapData::<GeneralCategory>::new().static_to_owned();
    let cat = |c: u32| gc(&cats, c);
    runs(
        &mut out,
        "prop",
        "iscntrl",
        &|c| {
            flag(matches!(
                cat(c),
                GeneralCategory::Control
                    | GeneralCategory::Format
                    | GeneralCategory::LineSeparator
                    | GeneralCategory::ParagraphSeparator
            ))
        },
        &all,
    );
    // `IS_THAT_CONTROL_SPACE` and `IS_THAT_ASCII_CONTROL_SPACE` of uchar.cpp#L197-L202.
    let control_space =
        |c: u32| c <= 0x9f && ((0x9..=0xd).contains(&c) || (0x1c..=0x1f).contains(&c) || c == 0x85);
    let ascii_control_space = |c: u32| (0x9..=0x1f).contains(&c) && (c <= 0xd || c >= 0x1c);
    runs(
        &mut out,
        "prop",
        "isspace",
        &|c| flag(is_z(cat(c)) || control_space(c)),
        &all,
    );
    runs(
        &mut out,
        "prop",
        "iswhitespace",
        &|c| {
            flag(
                (is_z(cat(c)) && c != 0xa0 && c != 0x2007 && c != 0x202f) || ascii_control_space(c),
            )
        },
        &all,
    );

    let script = CodePointMapData::<Script>::new();
    runs(
        &mut out,
        "prop",
        "script",
        &|c| script.get32(c).to_icu4c_value().to_string(),
        &all,
    );
    out
}

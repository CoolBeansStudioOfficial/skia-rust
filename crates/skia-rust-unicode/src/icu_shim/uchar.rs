// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The character-property functions of `SKICU_EMIT_FUNCS`: `u_hasBinaryProperty`,
//! `u_getIntPropertyValue`, `u_iscntrl`, `u_isspace`, `u_isWhitespace` and `uscript_getScript`, on
//! ICU4X's compiled property data with ICU's numbering and definitions.
//!
//! `oracle/unicode-diff`'s `props` section compares every function here with Chromium's ICU over all
//! code points (`docs/design/modules.md` §3.4): all exact.

// ICU4X deprecates `to_icu4c_value` (unicode-org/icu4x#6067), the only source of ICU's numbering of
// `Line_Break` and `Script`, which ICU's callers see. It is used deliberately.
#![allow(deprecated)]

use icu_properties::props::{
    Emoji, EmojiComponent, EmojiModifier, EmojiModifierBase, GeneralCategory, Ideographic,
    LineBreak, RegionalIndicator, Script,
};
use icu_properties::{CodePointMapData, CodePointSetData};

/// `UChar32`: a code point, or any `int32_t` a caller passes (ICU accepts out-of-range values).
pub type UChar32 = i32;

/// The `UProperty` values Skia passes to `u_hasBinaryProperty` and `u_getIntPropertyValue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UProperty {
    /// `UCHAR_IDEOGRAPHIC` (17).
    Ideographic,
    /// `UCHAR_EMOJI` (57).
    Emoji,
    /// `UCHAR_EMOJI_MODIFIER` (59).
    EmojiModifier,
    /// `UCHAR_EMOJI_MODIFIER_BASE` (60).
    EmojiModifierBase,
    /// `UCHAR_EMOJI_COMPONENT` (61).
    EmojiComponent,
    /// `UCHAR_REGIONAL_INDICATOR` (62).
    RegionalIndicator,
    /// `UCHAR_LINE_BREAK` (0x1008), an int property.
    LineBreak,
}

/// `ULineBreak` values Skia tests for (`uchar.h`).
pub mod u_lb {
    /// `U_LB_MANDATORY_BREAK` (`BK`).
    pub const MANDATORY_BREAK: i32 = 6;
    /// `U_LB_CARRIAGE_RETURN` (`CR`).
    pub const CARRIAGE_RETURN: i32 = 10;
    /// `U_LB_LINE_FEED` (`LF`).
    pub const LINE_FEED: i32 = 17;
    /// `U_LB_NEXT_LINE` (`NL`).
    pub const NEXT_LINE: i32 = 29;
}

/// The code point as ICU4X's `u32`, or `None` outside `0..=0x10FFFF`, where ICU's property tries
/// return their error value (no property, general category `Cn`).
fn code_point(c: UChar32) -> Option<u32> {
    u32::try_from(c).ok().filter(|&c| c <= 0x10_FFFF)
}

// Port of: icu4c/source/common/uprops.cpp u_hasBinaryProperty (chromium/deps/icu d578f2e8)
/// `u_hasBinaryProperty(c, which)`. `false` for a code point out of range and for a property that
/// is not binary, as in ICU.
#[doc(alias = "u_hasBinaryProperty")]
#[must_use]
pub fn u_has_binary_property(c: UChar32, which: UProperty) -> bool {
    let Some(c) = code_point(c) else {
        return false;
    };
    match which {
        UProperty::Ideographic => CodePointSetData::new::<Ideographic>().contains32(c),
        UProperty::Emoji => CodePointSetData::new::<Emoji>().contains32(c),
        UProperty::EmojiModifier => CodePointSetData::new::<EmojiModifier>().contains32(c),
        UProperty::EmojiModifierBase => CodePointSetData::new::<EmojiModifierBase>().contains32(c),
        UProperty::EmojiComponent => CodePointSetData::new::<EmojiComponent>().contains32(c),
        UProperty::RegionalIndicator => CodePointSetData::new::<RegionalIndicator>().contains32(c),
        UProperty::LineBreak => false,
    }
}

// Port of: icu4c/source/common/uprops.cpp u_getIntPropertyValue (chromium/deps/icu d578f2e8)
/// `u_getIntPropertyValue(c, which)`: ICU's numeric value (`ULineBreak` for
/// [`UProperty::LineBreak`]). A binary property gives 0 or 1, as in ICU; a code point out of range
/// gives 0.
#[doc(alias = "u_getIntPropertyValue")]
#[must_use]
pub fn u_get_int_property_value(c: UChar32, which: UProperty) -> i32 {
    match which {
        UProperty::LineBreak => code_point(c).map_or(0, |c| {
            i32::from(
                CodePointMapData::<LineBreak>::new()
                    .get32(c)
                    .to_icu4c_value(),
            )
        }),
        binary => i32::from(u_has_binary_property(c, binary)),
    }
}

/// The general category of `c` (`Cn` out of range, as ICU's `GET_PROPS`).
fn category(c: UChar32) -> GeneralCategory {
    code_point(c).map_or(GeneralCategory::Unassigned, |c| {
        CodePointMapData::<GeneralCategory>::new().get32(c)
    })
}

/// `CAT_MASK(props) & U_GC_Z_MASK`.
fn is_z(c: UChar32) -> bool {
    matches!(
        category(c),
        GeneralCategory::SpaceSeparator
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    )
}

// Port of: icu4c/source/common/uchar.cpp#L197-L198 (chromium/deps/icu d578f2e8)
/// `IS_THAT_CONTROL_SPACE(c)`.
fn is_that_control_space(c: UChar32) -> bool {
    c <= 0x9f && ((0x9..=0xd).contains(&c) || (0x1c..=0x1f).contains(&c) || c == 0x85)
}

// Port of: icu4c/source/common/uchar.cpp#L201-L202 (chromium/deps/icu d578f2e8)
/// `IS_THAT_ASCII_CONTROL_SPACE(c)`.
fn is_that_ascii_control_space(c: UChar32) -> bool {
    (0x9..=0x1f).contains(&c) && (c <= 0xd || c >= 0x1c)
}

// Port of: icu4c/source/common/uchar.cpp u_iscntrl (chromium/deps/icu d578f2e8)
/// `u_iscntrl(c)`: general category `Cc`, `Cf`, `Zl` or `Zp`.
#[must_use]
pub fn u_iscntrl(c: UChar32) -> bool {
    matches!(
        category(c),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    )
}

// Port of: icu4c/source/common/uchar.cpp u_isspace (chromium/deps/icu d578f2e8)
/// `u_isspace(c)`: `Z*`, or one of the C0/C1 control spaces.
#[must_use]
pub fn u_isspace(c: UChar32) -> bool {
    is_z(c) || is_that_control_space(c)
}

// Port of: icu4c/source/common/uchar.cpp u_isWhitespace (chromium/deps/icu d578f2e8)
/// `u_isWhitespace(c)`: Java's definition, `Z*` except the no-break spaces, or an ASCII control
/// space.
#[doc(alias = "u_isWhitespace")]
#[must_use]
pub fn u_is_whitespace(c: UChar32) -> bool {
    // NBSP, FIGURESP and NNBSP of uchar.cpp.
    (is_z(c) && c != 0xa0 && c != 0x2007 && c != 0x202f) || is_that_ascii_control_space(c)
}

/// `USCRIPT_INVALID_CODE`.
pub const USCRIPT_INVALID_CODE: i32 = -1;

// Port of: icu4c/source/common/uscript_props.cpp uscript_getScript (chromium/deps/icu d578f2e8)
/// `uscript_getScript(c, &status)`: ICU's `UScriptCode`. A code point out of range gives
/// `USCRIPT_INVALID_CODE` (ICU also sets `U_ILLEGAL_ARGUMENT_ERROR`).
#[doc(alias = "uscript_getScript")]
#[must_use]
pub fn uscript_get_script(c: UChar32) -> i32 {
    code_point(c).map_or(USCRIPT_INVALID_CODE, |c| {
        i32::from(CodePointMapData::<Script>::new().get32(c).to_icu4c_value())
    })
}

/// `Line_Break` of a code point in range, as ICU4X's value (for the break rules).
pub(crate) fn line_break(c: u32) -> LineBreak {
    CodePointMapData::<LineBreak>::new().get32(c)
}

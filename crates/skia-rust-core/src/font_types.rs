// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFontTypes.h, include/core/SkFourByteTag.h

//! Basic font enums and aliases: [`TextEncoding`], [`FontHinting`], [`GlyphId`], [`Unichar`]
//! and [`FourByteTag`] (`SkFontTypes.h`, `SkFourByteTag.h`).

pub use crate::utf::Unichar;

/// The glyph index in a font (`SkGlyphID`).
// Port of: include/core/SkTypes.h (SkGlyphID is uint16_t, chrome/m156)
#[doc(alias = "SkGlyphID")]
pub type GlyphId = u16;

/// A four character identifier, such as a font table tag or variation axis (`SkFourByteTag`).
// Port of: include/core/SkFourByteTag.h#L13 (chrome/m156)
#[doc(alias = "SkFourByteTag")]
pub type FourByteTag = u32;

/// `SkSetFourByteTag`: packs four characters, first character in the most significant byte.
// Port of: include/core/SkFourByteTag.h#L15-L17 (chrome/m156)
#[doc(alias = "SkSetFourByteTag")]
#[must_use]
pub const fn set_four_byte_tag(a: u8, b: u8, c: u8, d: u8) -> FourByteTag {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

/// How the bytes of a string are interpreted (`SkTextEncoding`).
// Port of: include/core/SkFontTypes.h#L11-L16 (chrome/m156)
#[doc(alias = "SkTextEncoding")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextEncoding {
    /// Uses bytes to represent UTF-8 or ASCII.
    #[doc(alias = "kUTF8")]
    #[default]
    UTF8,
    /// Uses two byte words to represent most of Unicode.
    #[doc(alias = "kUTF16")]
    UTF16,
    /// Uses four byte words to represent all of Unicode.
    #[doc(alias = "kUTF32")]
    UTF32,
    /// Uses two byte words to represent glyph indices.
    #[doc(alias = "kGlyphID")]
    GlyphId,
}

/// How much glyph outlines are modified by hinting (`SkFontHinting`).
// Port of: include/core/SkFontTypes.h#L18-L23 (chrome/m156)
#[doc(alias = "SkFontHinting")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum FontHinting {
    /// Glyph outlines unchanged.
    #[doc(alias = "kNone")]
    None,
    /// Minimal modification to improve contrast.
    #[doc(alias = "kSlight")]
    Slight,
    /// Glyph outlines modified to improve contrast.
    #[doc(alias = "kNormal")]
    Normal,
    /// Modifies glyph outlines for maximum contrast.
    #[doc(alias = "kFull")]
    Full,
}

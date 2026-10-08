// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPictureFlat.h (the op codes, flags and clip parameters used so far)

//! The flat encoding of a picture's op stream: the op codes, the bits of the save layer header,
//! and the packing of clip parameters.
//!
//! Each op is a word holding the op code in its top 8 bits and its size in the low 24 bits; a
//! size that does not fit (or equals the 24-bit mask) is followed by a word holding the size.
//!
//! skia-rust: only the op codes that the picture encoder and reader handle are here. The other
//! codes are numbered in Skia's enum, and an op with another code makes the picture unserializable
//! (see [`PictureRecord`](crate::picture_record)).

/// The 24-bit size mask (`MASK_24`).
// Port of: src/core/SkPictureRecord.h#L60 (chrome/m156)
pub(crate) const MASK_24: u32 = 0x00FF_FFFF;

/// Packs an op code and a size into the first word of an op (`PACK_8_24`).
// Port of: src/core/SkPictureRecord.h#L64 (chrome/m156)
pub(crate) fn pack_8_24(small: u8, large: u32) -> u32 {
    (u32::from(small) << 24) | large
}

/// The op code of `DrawType` in Skia's order: the value of each op in the op stream.
// Port of: src/core/SkPictureFlat.h#L32-L128 (chrome/m156), the DrawType enum
pub(crate) mod draw_type {
    /// `CLIP_PATH`.
    pub const CLIP_PATH: u8 = 1;
    /// `CLIP_RECT`.
    pub const CLIP_RECT: u8 = 3;
    /// `CLIP_RRECT`.
    pub const CLIP_RRECT: u8 = 4;
    /// `DRAW_OVAL`.
    pub const DRAW_OVAL: u8 = 12;
    /// `DRAW_PAINT`.
    pub const DRAW_PAINT: u8 = 13;
    /// `DRAW_PATH`.
    pub const DRAW_PATH: u8 = 14;
    /// `DRAW_POINTS`.
    pub const DRAW_POINTS: u8 = 16;
    /// `DRAW_TEXT_BLOB`.
    pub const DRAW_TEXT_BLOB: u8 = 45;
    /// `DRAW_RECT`.
    pub const DRAW_RECT: u8 = 21;
    /// `DRAW_RRECT`.
    pub const DRAW_RRECT: u8 = 22;
    /// `RESTORE`.
    pub const RESTORE: u8 = 28;
    /// `SAVE`.
    pub const SAVE: u8 = 30;
    /// `DRAW_DRRECT`.
    pub const DRAW_DRRECT: u8 = 40;
    /// `DRAW_ARC`.
    pub const DRAW_ARC: u8 = 60;
    /// `CONCAT44`.
    pub const CONCAT44: u8 = 68;
    /// `SAVE_LAYER_SAVELAYERREC`.
    pub const SAVE_LAYER_SAVELAYERREC: u8 = 52;
    /// `SET_M44`.
    pub const SET_M44: u8 = 71;
    /// `RESET_CLIP`.
    pub const RESET_CLIP: u8 = 76;
    /// `LAST_DRAWTYPE_ENUM`, the largest op code (`DRAW_SLUG`).
    pub const LAST_DRAWTYPE_ENUM: u8 = 77;
}

/// The flags of a save layer header (`SAVELAYERREC_HAS_*`).
// Port of: src/core/SkPictureFlat.h#L148-L156 (chrome/m156)
pub(crate) mod save_layer_rec {
    /// The layer has bounds (`SAVELAYERREC_HAS_BOUNDS`).
    pub const HAS_BOUNDS: u32 = 1 << 0;
    /// The layer has a paint (`SAVELAYERREC_HAS_PAINT`).
    pub const HAS_PAINT: u32 = 1 << 1;
    /// The layer has a backdrop filter (`SAVELAYERREC_HAS_BACKDROP`).
    pub const HAS_BACKDROP: u32 = 1 << 2;
    /// The layer has flags (`SAVELAYERREC_HAS_FLAGS`).
    pub const HAS_FLAGS: u32 = 1 << 3;
    /// The obsolete clip mask (`SAVELAYERREC_HAS_CLIPMASK_OBSOLETE`), which is an image.
    pub const HAS_CLIPMASK_OBSOLETE: u32 = 1 << 4;
    /// The obsolete clip matrix (`SAVELAYERREC_HAS_CLIPMATRIX_OBSOLETE`).
    pub const HAS_CLIPMATRIX_OBSOLETE: u32 = 1 << 5;
    /// The layer has a backdrop scale factor (`SAVELAYERREC_HAS_BACKDROP_SCALE`).
    pub const HAS_BACKDROP_SCALE: u32 = 1 << 6;
    /// The layer has several image filters (`SAVELAYERREC_HAS_MULTIPLE_FILTERS`).
    pub const HAS_MULTIPLE_FILTERS: u32 = 1 << 7;
    /// The layer has a backdrop tile mode (`SAVELAYERREC_HAS_BACKDROP_TILEMODE`).
    pub const HAS_BACKDROP_TILEMODE: u32 = 1 << 8;
}

/// The clip parameters of a clip op: the region op in the low 4 bits, and whether the clip is
/// anti-aliased in bit 4 (`ClipParams_pack`).
// Port of: src/core/SkPictureFlat.h#L168-L171 (chrome/m156)
pub(crate) fn clip_params_pack(op: u32, do_aa: bool) -> u32 {
    (u32::from(do_aa) << 4) | op
}

/// The anti-alias bit of clip parameters (`ClipParams_unpackDoAA`).
// Port of: src/core/SkPictureFlat.h#L185-L187 (chrome/m156)
pub(crate) fn clip_params_unpack_do_aa(packed: u32) -> bool {
    (packed >> 4) & 1 != 0
}

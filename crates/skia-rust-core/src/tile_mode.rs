// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkTileMode.h

//! `SkTileMode`: how a shader or layer handles coordinates outside its bounds.

/// Tile modes (`SkTileMode`).
// Port of: include/core/SkTileMode.h#L12-L35 (chrome/m156)
#[doc(alias = "SkTileMode")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Default)]
#[repr(i32)]
pub enum TileMode {
    /// Replicate the edge color (`kClamp`).
    #[default]
    Clamp,
    /// Repeat the image (`kRepeat`).
    Repeat,
    /// Repeat, flipping every other time (`kMirror`).
    Mirror,
    /// Transparent black outside (`kDecal`).
    Decal,
}

impl TileMode {
    /// The mode with the value `value`, as a `SkTileMode` is read from a buffer (`read32LE`), or
    /// `None` past the last mode.
    #[must_use]
    pub fn from_i32(value: i32) -> Option<TileMode> {
        match value {
            0 => Some(TileMode::Clamp),
            1 => Some(TileMode::Repeat),
            2 => Some(TileMode::Mirror),
            3 => Some(TileMode::Decal),
            _ => None,
        }
    }
}

// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/UniquePaintParamsID.h

//! [`UniquePaintParamsID`]: a small unique integer that stands in for a variable length
//! `PaintParamsKey`.

/// `SK_InvalidUniqueID`.
// Port of: include/core/SkTypes.h#L191 (chrome/m156)
const SK_INVALID_UNIQUE_ID: u32 = 0;

/// This class boils down to a unique uint that can be used instead of a variable length key
/// derived from a `PaintParams`.
// Port of: src/gpu/graphite/UniquePaintParamsID.h#L17-L33 (chrome/m156)
#[doc(alias = "skgpu::graphite::UniquePaintParamsID")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UniquePaintParamsID {
    id: u32,
}

impl UniquePaintParamsID {
    /// `UniquePaintParamsID(id)`.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self { id }
    }

    /// `Invalid()`.
    #[doc(alias = "Invalid")]
    #[must_use]
    pub const fn invalid() -> Self {
        Self::new(SK_INVALID_UNIQUE_ID)
    }

    /// `isValid()`.
    #[doc(alias = "isValid")]
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.id != SK_INVALID_UNIQUE_ID
    }

    /// `asUInt()`.
    #[doc(alias = "asUInt")]
    #[must_use]
    pub const fn as_uint(&self) -> u32 {
        self.id
    }
}

impl Default for UniquePaintParamsID {
    /// `UniquePaintParamsID()`: the invalid id.
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_and_valid() {
        assert!(!UniquePaintParamsID::invalid().is_valid());
        assert_eq!(
            UniquePaintParamsID::default(),
            UniquePaintParamsID::invalid()
        );
        assert!(UniquePaintParamsID::new(1).is_valid());
        assert_eq!(UniquePaintParamsID::new(7).as_uint(), 7);
    }
}

// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFontParameters.h

//! [`VariationAxis`]: the parameters of one axis of a variation font (`SkFontParameters.h`).

pub use variation::Axis as VariationAxis;

/// The axes of a variable font (`SkFontParameters::Variation`).
// Port of: include/core/SkFontParameters.h#L16-L41 (chrome/m156)
pub mod variation {
    use crate::font_types::FourByteTag;

    /// Parameters of one variation font axis.
    // Port of: include/core/SkFontParameters.h#L18-L38 (chrome/m156)
    #[doc(alias = "SkFontParameters::Variation::Axis")]
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct Axis {
        /// Four character identifier of the font axis (weight, width, slant, italic...).
        pub tag: FourByteTag,
        /// Minimum value supported by this axis.
        pub min: f32,
        /// Default value set by this axis.
        pub def: f32,
        /// Maximum value supported by this axis. The maximum can equal the minimum.
        pub max: f32,
        /// `HIDDEN` bit: whether user interfaces should keep this axis hidden.
        flags: u16,
    }

    /// `Axis::HIDDEN`.
    const HIDDEN: u16 = 0x0001;

    impl Axis {
        /// `SkFontParameters::Variation::Axis(tag, min, def, max, hidden)`.
        // Port of: include/core/SkFontParameters.h#L20-L21 (chrome/m156)
        #[must_use]
        pub const fn new(tag: FourByteTag, min: f32, def: f32, max: f32, hidden: bool) -> Self {
            Self {
                tag,
                min,
                def,
                max,
                flags: if hidden { HIDDEN } else { 0 },
            }
        }

        /// Whether this axis is recommended to remain hidden in user interfaces.
        // Port of: include/core/SkFontParameters.h#L32 (chrome/m156)
        #[must_use]
        pub const fn is_hidden(&self) -> bool {
            self.flags & HIDDEN != 0
        }

        /// Sets whether this axis should remain hidden in user interfaces.
        // Port of: include/core/SkFontParameters.h#L34 (chrome/m156)
        pub fn set_hidden(&mut self, hidden: bool) -> &mut Self {
            self.flags = if hidden {
                self.flags | HIDDEN
            } else {
                self.flags & !HIDDEN
            };
            self
        }
    }
}

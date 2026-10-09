// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Uniform.h

//! [`Uniform`]: the description of one uniform (name, type and array count).

use crate::sksl_type_shared::SkSLType;

/// The array count of a uniform that is not an array (`Uniform::kNonArray`).
// Port of: src/gpu/graphite/Uniform.h#L22 (chrome/m156)
#[doc(alias = "kNonArray")]
pub const K_NON_ARRAY: i32 = 0;

/// Describes a uniform: its constant name in the generated `SkSL`, its type and its array count.
///
/// Skia packs the fields into bit-fields (its comment explains why); here they are plain
/// fields, because the packing only saves memory in Skia's per-effect uniform tables.
// Port of: src/gpu/graphite/Uniform.h#L17-L72 (chrome/m156)
#[doc(alias = "skgpu::graphite::Uniform")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uniform {
    name: &'static str,
    ty: SkSLType,
    is_paint_color: bool,
    count: i32,
}

impl Uniform {
    /// A uniform that is not an array (`Uniform(name, type)`).
    // Port of: src/gpu/graphite/Uniform.h#L24-L26 (chrome/m156)
    #[must_use]
    pub const fn new(name: &'static str, ty: SkSLType) -> Self {
        Self::with_flags(name, ty, K_NON_ARRAY, false)
    }

    /// An array of `count` elements of `ty` (`Uniform(name, type, count)`).
    // Port of: src/gpu/graphite/Uniform.h#L24-L26 (chrome/m156)
    #[must_use]
    pub const fn new_array(name: &'static str, ty: SkSLType, count: i32) -> Self {
        Self::with_flags(name, ty, count, false)
    }

    /// The paint color uniform. It is treated specially: it is added to the uniform block once,
    /// and its name is not mangled.
    // Port of: src/gpu/graphite/Uniform.h#L30-L34 (chrome/m156)
    #[doc(alias = "PaintColor")]
    #[must_use]
    pub const fn paint_color() -> Self {
        Self::with_flags("paintColor", SkSLType::Half4, K_NON_ARRAY, true)
    }

    // Port of: src/gpu/graphite/Uniform.h#L56-L65 (chrome/m156), the private constructor
    const fn with_flags(
        name: &'static str,
        ty: SkSLType,
        count: i32,
        is_paint_color: bool,
    ) -> Self {
        assert!(ty.can_be_uniform_value());
        assert!(count >= 0);
        Self {
            name,
            ty,
            is_paint_color,
            count,
        }
    }

    /// The constant string name to use in the generated `SkSL`.
    // Port of: src/gpu/graphite/Uniform.h#L37 (chrome/m156)
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The type of the uniform.
    // Port of: src/gpu/graphite/Uniform.h#L38 (chrome/m156)
    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> SkSLType {
        self.ty
    }

    /// The number of elements of `ty` in the array, or [`K_NON_ARRAY`].
    // Port of: src/gpu/graphite/Uniform.h#L39 (chrome/m156)
    #[must_use]
    pub const fn count(&self) -> i32 {
        self.count
    }

    /// Whether this is the paint color uniform.
    // Port of: src/gpu/graphite/Uniform.h#L41 (chrome/m156)
    #[doc(alias = "isPaintColor")]
    #[must_use]
    pub const fn is_paint_color(&self) -> bool {
        self.is_paint_color
    }
}

// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkComposeColorFilter.{h,cpp}

//! `SkComposeColorFilter`: applies an inner filter, then an outer one. Made by
//! [`ColorFilter::composed`](crate::color_filter::ColorFilter::composed) and
//! [`color_filters::compose`](crate::color_filters::compose).

use crate::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use crate::effect_priv::StageRec;

/// The filter `outer(inner(color))` (`SkComposeColorFilter`).
// Port of: src/effects/colorfilters/SkComposeColorFilter.h#L19-L46 (chrome/m156)
#[doc(alias = "SkComposeColorFilter")]
#[derive(Clone, Debug)]
pub struct ComposeColorFilter {
    outer: ColorFilter,
    inner: ColorFilter,
}

impl ComposeColorFilter {
    /// `SkComposeColorFilter(outer, inner)`.
    // Port of: src/effects/colorfilters/SkComposeColorFilter.cpp#L19-L22 (chrome/m156)
    #[must_use]
    pub fn new(outer: ColorFilter, inner: ColorFilter) -> Self {
        Self { outer, inner }
    }

    /// The outer filter.
    #[must_use]
    pub fn outer(&self) -> &ColorFilter {
        &self.outer
    }

    /// The inner filter.
    #[must_use]
    pub fn inner(&self) -> &ColorFilter {
        &self.inner
    }
}

impl ColorFilterBase for ComposeColorFilter {
    // Port of: src/effects/colorfilters/SkComposeColorFilter.cpp#L24-L27 (chrome/m156)
    fn on_is_alpha_unchanged(&self) -> bool {
        // Can only claim alpha-unchanged support if both our proxies do.
        self.outer.is_alpha_unchanged() && self.inner.is_alpha_unchanged()
    }

    // Port of: src/effects/colorfilters/SkComposeColorFilter.cpp#L29-L35 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, shader_is_opaque: bool) -> bool {
        let mut inner_is_opaque = shader_is_opaque;
        if !self.inner.is_alpha_unchanged() {
            inner_is_opaque = false;
        }
        self.inner.as_base().append_stages(rec, shader_is_opaque)
            && self.outer.as_base().append_stages(rec, inner_is_opaque)
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::Compose
    }
}

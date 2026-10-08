// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkColorFilter.h, src/core/SkColorFilter.cpp,
// src/effects/colorfilters/SkColorFilterBase.{h,cpp}

//! `SkColorFilter`: the interface of color filters.
//!
//! skia-rust: the [`ColorFilter`] handle, the [`ColorFilterBase`] trait with Skia's default
//! virtuals, and the non-virtual helpers that paints need. The implementations are in
//! [`color_filters`](crate::color_filters) and the modules it names; they plug in by implementing
//! [`ColorFilterBase`].

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::alpha_type::AlphaType;
use crate::arena_alloc::ArenaAlloc;
use crate::blend_mode::BlendMode;
use crate::blend_mode_priv::{color_from_bytes, color_to_bytes};
use crate::color::{Color, Color4f, PMColor4f, colors};
use crate::color_space::ColorSpace;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::color_type::ColorType;
use crate::compose_color_filter::ComposeColorFilter;
use crate::effect_priv::StageRec;
use crate::raster_pipeline::{MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage};
use crate::rect::Rect;

/// The kinds of color filters (`SkColorFilterBase::Type`, from `SK_ALL_COLOR_FILTERS`).
// Port of: src/effects/colorfilters/SkColorFilterBase.h#L43-L50 (chrome/m156)
#[doc(alias = "SkColorFilterBase::Type")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColorFilterType {
    /// Used for stubs/tests (`kNoop`).
    Noop,
    /// `kBlendMode`.
    BlendMode,
    /// `kColorSpaceXform`.
    ColorSpaceXform,
    /// `kCompose`.
    Compose,
    /// `kGaussian`.
    Gaussian,
    /// `kMatrix`.
    Matrix,
    /// `kRuntime`.
    Runtime,
    /// `kTable`.
    Table,
    /// `kWorkingFormat`.
    WorkingFormat,
}

/// The virtual interface of a color filter (`SkColorFilterBase`).
///
/// Implementations are wrapped in a [`ColorFilter`] with [`ColorFilter::from_base`]. They are
/// `Any`, so they can be downcast once their [`color_filter_type`](Self::color_filter_type) is
/// known.
///
/// skia-rust: `asRuntimeEffect` and the flattening hooks are not ported yet.
// Port of: src/effects/colorfilters/SkColorFilterBase.h#L35-L88 (chrome/m156)
#[doc(alias = "SkColorFilterBase")]
pub trait ColorFilterBase: Any + fmt::Debug + Send + Sync {
    /// Appends the filter's stages; false on failure (`appendStages`).
    #[doc(alias = "appendStages")]
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, shader_is_opaque: bool) -> bool;

    /// True if the filter is guaranteed to never change the alpha of a color it filters
    /// (`onIsAlphaUnchanged`).
    #[doc(alias = "onIsAlphaUnchanged")]
    fn on_is_alpha_unchanged(&self) -> bool {
        false
    }

    /// The kind of filter (`type`).
    #[doc(alias = "type")]
    fn color_filter_type(&self) -> ColorFilterType;

    /// Filters one premultiplied color in the destination color space (`onFilterColor4f`).
    ///
    /// The default runs the filter's stages on the color through a one-pixel raster
    /// pipeline.
    // Port of: src/effects/colorfilters/SkColorFilterBase.cpp#L33-L58 (chrome/m156)
    #[doc(alias = "onFilterColor4f")]
    fn on_filter_color4f(&self, color: &PMColor4f, dst_cs: Option<&ColorSpace>) -> PMColor4f {
        let alloc = ArenaAlloc::new();
        let mut pipeline = RasterPipeline::new();
        pipeline.append_constant_color(&alloc, &color.as_array());
        // (SkSurfaceProps props{}; default OK; colorFilters don't render text)
        let dst = MemoryCtx::new(MemSlot(0));
        let mut rec = StageRec {
            pipeline: &mut pipeline,
            alloc: &alloc,
            dst_color_type: ColorType::RGBAF32,
            dst_cs,
            paint_color: color.unpremul(),
            surface_props: crate::surface_props::SurfaceProps::default(),
            dst_bounds: Rect::new_empty(),
        };

        #[allow(clippy::float_cmp)] // Skia compares alpha with 1 exactly
        if self.append_stages(&mut rec, color.a == 1.0) {
            pipeline.append(Stage::StoreF32(dst));
            let mut storage = color_to_bytes([0.0; 4]);
            let mut mem = MemoryBindings::new();
            mem.bind(dst.slot, MemView::write(&mut storage));
            pipeline.run(0, 0, 1, 1, &mut mem);
            let [r, g, b, a] = color_from_bytes(&storage);
            return PMColor4f::new(r, g, b, a);
        }

        debug_assert!(false, "onFilterColor4f unimplemented for this filter");
        PMColor4f::new(0.0, 0.0, 0.0, 0.0)
    }

    /// The 5x4 matrix the filter can be represented by, if any (`onAsAColorMatrix`).
    #[doc(alias = "onAsAColorMatrix")]
    fn on_as_a_color_matrix(&self) -> Option<[f32; 20]> {
        None
    }

    /// The source color and blend mode the filter can be represented by, if any
    /// (`onAsAColorMode`).
    #[doc(alias = "onAsAColorMode")]
    fn on_as_a_color_mode(&self) -> Option<(Color, BlendMode)> {
        None
    }
}

impl dyn ColorFilterBase {
    /// True if filtering transparent black gives something else (`affectsTransparentBlack`).
    // Port of: src/effects/colorfilters/SkColorFilterBase.h#L54-L57 (chrome/m156)
    #[doc(alias = "affectsTransparentBlack")]
    #[must_use]
    pub fn affects_transparent_black(&self) -> bool {
        filter_color4f(self, &colors::TRANSPARENT, None, None) != colors::TRANSPARENT
    }
}

// Port of: src/core/SkColorFilter.cpp#L44-L52 (chrome/m156)
fn filter_color4f(
    base: &dyn ColorFilterBase,
    orig_src_color: &Color4f,
    src_cs: Option<&ColorSpace>,
    dst_cs: Option<&ColorSpace>,
) -> Color4f {
    let mut color = orig_src_color.as_array();
    ColorSpaceXformSteps::new(src_cs, AlphaType::Unpremul, dst_cs, AlphaType::Premul)
        .apply(&mut color);
    let color = PMColor4f::new(color[0], color[1], color[2], color[3]);

    // SkColor4f will assert if we allow alpha outside [0,1]. (SkSL color filters might do this).
    base.on_filter_color4f(&color, dst_cs)
        .pin_alpha()
        .unpremul()
}

/// A shared color filter (`sk_sp<SkColorFilter>`): a cheaply clonable handle to a
/// [`ColorFilterBase`]. Color filters are applied to the source colors before blending.
///
/// Equality is identity, as Skia compares `sk_sp`s ([`ColorFilter::ptr_eq`]).
///
/// skia-rust: `makeWithWorkingColorSpace` needs `SkWorkingFormatColorFilter`, which is not ported.
// Port of: include/core/SkColorFilter.h#L35-L80 (chrome/m156)
#[doc(alias = "SkColorFilter")]
#[derive(Clone)]
pub struct ColorFilter(Arc<dyn ColorFilterBase>);

impl ColorFilter {
    /// Wraps a color filter implementation.
    #[must_use]
    pub fn from_base(base: impl ColorFilterBase) -> ColorFilter {
        ColorFilter(Arc::new(base))
    }

    /// The implementation (`as_CFB`).
    #[doc(alias = "as_CFB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn ColorFilterBase {
        &*self.0
    }

    /// True if `self` and `other` are the same filter (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &ColorFilter) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// `makeComposed`: this filter applied after `inner` (`None` gives `self`).
    // Port of: src/core/SkColorFilter.cpp#L48-L54 (chrome/m156)
    #[doc(alias = "makeComposed")]
    #[must_use]
    pub fn composed(&self, inner: Option<ColorFilter>) -> ColorFilter {
        match inner {
            None => self.clone(),
            Some(inner) => ColorFilter::from_base(ComposeColorFilter::new(self.clone(), inner)),
        }
    }

    /// If the filter can be represented by a source color plus a blend mode, those
    /// (`asAColorMode`).
    // Port of: src/core/SkColorFilter.cpp#L25-L27 (chrome/m156)
    #[doc(alias = "asAColorMode")]
    #[must_use]
    pub fn to_a_color_mode(&self) -> Option<(Color, BlendMode)> {
        self.0.on_as_a_color_mode()
    }

    /// If the filter can be represented by a 5x4 matrix, that matrix (`asAColorMatrix`).
    // Port of: src/core/SkColorFilter.cpp#L29-L31 (chrome/m156)
    #[doc(alias = "asAColorMatrix")]
    #[must_use]
    pub fn to_a_color_matrix(&self) -> Option<[f32; 20]> {
        self.0.on_as_a_color_matrix()
    }

    /// True if the filter is guaranteed to never change the alpha of a color it filters
    /// (`isAlphaUnchanged`).
    // Port of: src/core/SkColorFilter.cpp#L33-L35 (chrome/m156)
    #[doc(alias = "isAlphaUnchanged")]
    #[must_use]
    pub fn is_alpha_unchanged(&self) -> bool {
        self.0.on_is_alpha_unchanged()
    }

    /// Converts the src color (in `src_color_space`, sRGB if `None`) into the dst color space
    /// (the source space if `None`), then applies this filter to it, returning the filtered
    /// color in the dst color space (`filterColor4f`).
    #[doc(alias = "filterColor4f")]
    #[must_use]
    pub fn filter_color4f(
        &self,
        color: impl Into<Color4f>,
        src_color_space: Option<&ColorSpace>,
        dst_color_space: Option<&ColorSpace>,
    ) -> Color4f {
        filter_color4f(
            self.as_base(),
            &color.into(),
            src_color_space,
            dst_color_space,
        )
    }
}

impl PartialEq for ColorFilter {
    /// Identity, as Skia's `sk_sp<SkColorFilter>` `operator==`.
    fn eq(&self, other: &ColorFilter) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for ColorFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ColorFilter").field(&self.0).finish()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A test filter: appends `clear` if `clear`, else nothing (the identity).
    #[derive(Debug)]
    pub(crate) struct TestFilter {
        pub(crate) clear: bool,
        pub(crate) alpha_unchanged: bool,
    }

    impl ColorFilterBase for TestFilter {
        fn append_stages(&self, rec: &mut StageRec<'_, '_>, _shader_is_opaque: bool) -> bool {
            if self.clear {
                rec.pipeline.append(Stage::Clear);
            }
            true
        }
        fn on_is_alpha_unchanged(&self) -> bool {
            self.alpha_unchanged
        }
        fn color_filter_type(&self) -> ColorFilterType {
            ColorFilterType::Noop
        }
    }

    #[test]
    fn filter_color4f_runs_the_stages() {
        let identity = ColorFilter::from_base(TestFilter {
            clear: false,
            alpha_unchanged: true,
        });
        let c = Color4f::new(0.25, 0.5, 0.75, 0.5);
        // Premultiplied, stored and unpremultiplied again: exact for these values.
        assert_eq!(identity.filter_color4f(c, None, None), c);
        assert!(identity.is_alpha_unchanged());
        assert!(!identity.as_base().affects_transparent_black());
        assert_eq!(identity.to_a_color_matrix(), None);
        assert_eq!(identity.to_a_color_mode(), None);

        let clear = ColorFilter::from_base(TestFilter {
            clear: true,
            alpha_unchanged: false,
        });
        assert_eq!(clear.filter_color4f(c, None, None), colors::TRANSPARENT);
        assert!(!clear.as_base().affects_transparent_black());
        assert_ne!(clear, identity);
        assert_eq!(clear, clear.clone());
    }
}

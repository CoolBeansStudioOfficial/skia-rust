// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileColorFilter.h,
// src/gpu/graphite/precompile/PrecompileColorFilter.cpp and PrecompileColorFiltersPriv.h

//! `PrecompileColorFilter` and the `PrecompileColorFilters` factories.

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::PMColor4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::known_runtime_effects::StableKey;
use skia_rust_core::working_format_color_filter::WorkingFormatCalculator;
use skia_rust_skcms::{Matrix3x3, TransferFunction};

use crate::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::key_helpers_ii::{
    ColorSpaceTransformBlock, ColorSpaceTransformData, MatrixColorFilterBlock,
    MatrixColorFilterData, TableColorFilterBlock, TableColorFilterData,
    add_blend_mode_color_filter, compose,
};
use crate::graphite::precompile::base::{
    Combinable, PrecompileBaseImpl, PrecompileBaseType, add_to_key, select_option, sum_combinations,
};
use crate::graphite::precompile::blender::PrecompileBlenderList;
use crate::graphite::precompile::runtime_effect::{PrecompileBaseHandle, PrecompileRuntimeEffects};

/// `SK_PMColor4fWHITE`.
const PM_COLOR4F_WHITE: PMColor4f = PMColor4f {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// The virtual interface of `PrecompileColorFilter`: `isAlphaUnchanged` on top of the base
/// interface.
pub(crate) trait ColorFilterImpl: PrecompileBaseImpl {
    /// `isAlphaUnchanged(desiredCombination)`.
    fn is_alpha_unchanged(&self, desired_combination: i32) -> bool;
}

/// `PrecompileColorFilter`: a shared precompile color filter.
#[doc(alias = "SkColorFilter")]
#[derive(Clone)]
pub struct PrecompileColorFilter {
    pub(crate) imp: Arc<dyn ColorFilterImpl>,
}

impl std::fmt::Debug for PrecompileColorFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrecompileColorFilter")
            .field("type", &PrecompileBaseType::ColorFilter)
            .finish_non_exhaustive()
    }
}

impl PrecompileColorFilter {
    /// `PrecompileColorFilter::makeComposed(inner)`: `None` inner returns this filter.
    // Port of: src/gpu/graphite/precompile/PrecompileColorFilter.cpp#L24-L31 (chrome/m156)
    #[must_use]
    #[doc(alias = "makeComposed")]
    pub fn make_composed(
        &self,
        inner: Option<PrecompileColorFilter>,
    ) -> Option<PrecompileColorFilter> {
        match inner {
            None => Some(self.clone()),
            Some(inner) => PrecompileColorFilters::compose(&[Some(self.clone())], &[Some(inner)]),
        }
    }

    /// `priv().numCombinations()`.
    #[must_use]
    pub fn num_combinations(&self) -> i32 {
        self.imp.num_intrinsic_combinations() * self.imp.num_child_combinations()
    }

    /// `priv().numChildCombinations()`.
    #[must_use]
    pub fn num_child_combinations(&self) -> i32 {
        self.imp.num_child_combinations()
    }

    /// `priv().isAlphaUnchanged(desiredCombination)`.
    #[must_use]
    pub fn is_alpha_unchanged(&self, desired_combination: i32) -> bool {
        self.imp.is_alpha_unchanged(desired_combination)
    }

    /// `priv().addToKey(keyContext, desiredCombination)`.
    pub fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.imp.add_to_key(key_context, desired_combination);
    }
}

impl Combinable for PrecompileColorFilter {
    fn combinations(&self) -> i32 {
        self.num_combinations()
    }

    fn add_combination_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        self.add_to_key(key_context, desired_combination);
    }
}

/// `PrecompileComposeColorFilter`.
struct ComposeColorFilter {
    outer_options: Vec<Option<PrecompileColorFilter>>,
    inner_options: Vec<Option<PrecompileColorFilter>>,
    num_outer_combos: i32,
    num_inner_combos: i32,
}

impl ComposeColorFilter {
    // Port of: PrecompileComposeColorFilter::getChildCombinations (chrome/m156)
    fn child_combinations(&self, desired_combination: i32) -> (i32, i32) {
        let desired_outer = desired_combination % self.num_outer_combos;
        let mut remaining = desired_combination / self.num_outer_combos;
        let desired_inner = remaining % self.num_inner_combos;
        remaining /= self.num_inner_combos;
        debug_assert_eq!(remaining, 0);
        (desired_inner, desired_outer)
    }
}

impl PrecompileBaseImpl for ComposeColorFilter {
    fn num_child_combinations(&self) -> i32 {
        self.num_outer_combos * self.num_inner_combos
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let (desired_inner, desired_outer) = self.child_combinations(desired_combination);
        let (outer, outer_child) = select_option(&self.outer_options, desired_outer);
        let (inner, inner_child) = select_option(&self.inner_options, desired_inner);
        match (inner, outer) {
            (None, None) => key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::PriorOutput),
            (None, Some(outer)) => outer.add_to_key(key_context, outer_child),
            (Some(inner), None) => inner.add_to_key(key_context, inner_child),
            (Some(inner), Some(outer)) => compose(
                key_context,
                || inner.add_to_key(key_context, inner_child),
                || outer.add_to_key(key_context, outer_child),
            ),
        }
    }
}

impl ColorFilterImpl for ComposeColorFilter {
    fn is_alpha_unchanged(&self, desired_combination: i32) -> bool {
        let (desired_inner, desired_outer) = self.child_combinations(desired_combination);
        let (inner, inner_option) = select_option(&self.inner_options, desired_inner);
        let (outer, outer_option) = select_option(&self.outer_options, desired_outer);
        // A null option is the identity, which leaves alpha unchanged.
        inner.is_none_or(|f| f.is_alpha_unchanged(inner_option))
            && outer.is_none_or(|f| f.is_alpha_unchanged(outer_option))
    }
}

/// `PrecompileColorFilters::Compose`: `None` when every option is null or the lists are empty.
// Port of: src/gpu/graphite/precompile/PrecompileColorFilter.cpp#L56-L75 (chrome/m156)
#[must_use]
#[doc(alias = "Compose")]
pub fn compose_filters(
    outer_options: &[Option<PrecompileColorFilter>],
    inner_options: &[Option<PrecompileColorFilter>],
) -> Option<PrecompileColorFilter> {
    if is_empty(outer_options) && is_empty(inner_options) {
        return None;
    }
    let num_outer_combos = sum_combinations(outer_options);
    let num_inner_combos = sum_combinations(inner_options);
    Some(PrecompileColorFilter {
        imp: Arc::new(ComposeColorFilter {
            outer_options: outer_options.to_vec(),
            inner_options: inner_options.to_vec(),
            num_outer_combos,
            num_inner_combos,
        }),
    })
}

/// `is_empty` of the anonymous namespace: no options, or only null ones.
fn is_empty(options: &[Option<PrecompileColorFilter>]) -> bool {
    options.iter().all(Option::is_none)
}

/// `PrecompileBlendModeColorFilter`.
struct BlendModeColorFilter {
    blend_options: PrecompileBlenderList,
}

impl BlendModeColorFilter {
    // Port of: PrecompileBlendModeColorFilter::getDesiredBlendMode (chrome/m156)
    fn desired_blend_mode(&self, desired_combination: i32) -> BlendMode {
        let (blender, option) = self.blend_options.select_option(desired_combination);
        debug_assert_eq!(option, 0);
        blender
            .and_then(|b| b.as_blend_mode())
            .expect("a blend-mode option")
    }
}

impl PrecompileBaseImpl for BlendModeColorFilter {
    fn num_intrinsic_combinations(&self) -> i32 {
        self.blend_options.num_combinations()
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let representative = self.desired_blend_mode(desired_combination);
        add_blend_mode_color_filter(key_context, representative, &PM_COLOR4F_WHITE);
    }
}

impl ColorFilterImpl for BlendModeColorFilter {
    fn is_alpha_unchanged(&self, desired_combination: i32) -> bool {
        // Only kDst and kSrcATop keep the alpha.
        matches!(
            self.desired_blend_mode(desired_combination),
            BlendMode::Dst | BlendMode::SrcATop
        )
    }
}

/// `PrecompileMatrixColorFilter`.
struct MatrixColorFilter {
    in_hsla: bool,
    clamp: bool, // always true if in_hsla is true
}

impl PrecompileBaseImpl for MatrixColorFilter {
    fn num_intrinsic_combinations(&self) -> i32 {
        2
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        #[rustfmt::skip]
        static IDENTITY: [f32; 20] = [
            1.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        debug_assert!(desired_combination == 0 || desired_combination == 1);
        let matrix_cf_data = MatrixColorFilterData::new(&IDENTITY, self.in_hsla, self.clamp);
        MatrixColorFilterBlock::add_block(key_context, &matrix_cf_data);
    }
}

impl ColorFilterImpl for MatrixColorFilter {
    fn is_alpha_unchanged(&self, desired_combination: i32) -> bool {
        debug_assert!(desired_combination == 0 || desired_combination == 1);
        desired_combination == 1
    }
}

/// `PrecompileColorSpaceXformColorFilter`.
struct ColorSpaceXformColorFilter {
    src: Vec<Option<ColorSpace>>,
    dst: Vec<Option<ColorSpace>>,
    num_combinations: i32,
}

impl PrecompileBaseImpl for ColorSpaceXformColorFilter {
    fn num_intrinsic_combinations(&self) -> i32 {
        self.num_combinations
    }

    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        let n_src = self.src.len();
        let idx = usize::try_from(desired_combination).unwrap_or(0);
        let src_combination = idx % n_src;
        let dst_combination = idx / n_src;
        debug_assert!(dst_combination < self.dst.len());
        let cs_data = ColorSpaceTransformData::from_color_spaces(
            self.src[src_combination].as_ref(),
            AlphaType::Premul,
            self.dst[dst_combination].as_ref(),
            AlphaType::Premul,
        );
        ColorSpaceTransformBlock::add_block(key_context, &cs_data);
    }
}

impl ColorFilterImpl for ColorSpaceXformColorFilter {
    fn is_alpha_unchanged(&self, _desired_combination: i32) -> bool {
        true
    }
}

/// `PrecompileColorFiltersPriv::ColorSpaceXform`.
// Port of: src/gpu/graphite/precompile/PrecompileColorFilter.cpp#L240-L255 (chrome/m156)
#[must_use]
pub fn color_space_xform(src: &[ColorSpace], dst: &[ColorSpace]) -> PrecompileColorFilter {
    PrecompileColorFilter {
        imp: Arc::new(ColorSpaceXformColorFilter {
            src: src.iter().cloned().map(Some).collect(),
            dst: dst.iter().cloned().map(Some).collect(),
            num_combinations: i32::try_from(src.len() * dst.len()).unwrap_or(i32::MAX),
        }),
    }
}

/// `PrecompileTableColorFilter`.
struct TableColorFilter;

impl PrecompileBaseImpl for TableColorFilter {
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert_eq!(desired_combination, 0);
        let data = TableColorFilterData::new(None);
        TableColorFilterBlock::add_block(key_context, &data);
    }
}

impl ColorFilterImpl for TableColorFilter {
    fn is_alpha_unchanged(&self, _desired_combination: i32) -> bool {
        false
    }
}

/// `PrecompileGaussianColorFilter`.
struct GaussianColorFilter;

impl PrecompileBaseImpl for GaussianColorFilter {
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert_eq!(desired_combination, 0);
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_block(BuiltInCodeSnippetID::GaussianColorFilter);
    }
}

impl ColorFilterImpl for GaussianColorFilter {
    fn is_alpha_unchanged(&self, _desired_combination: i32) -> bool {
        false
    }
}

/// `PrecompileWithWorkingFormatColorFilter`.
struct WithWorkingFormatColorFilter {
    child_options: Vec<Option<PrecompileColorFilter>>,
    num_child_combos: i32,
    working_format: WorkingFormatCalculator,
}

impl PrecompileBaseImpl for WithWorkingFormatColorFilter {
    fn num_child_combinations(&self) -> i32 {
        self.num_child_combos
    }

    // Port of: PrecompileWithWorkingFormatColorFilter::addToKey (chrome/m156)
    fn add_to_key(&self, key_context: &KeyContext<'_>, desired_combination: i32) {
        debug_assert!(desired_combination < self.num_child_combos);
        let dst_info = key_context.dst_color_info();
        let dst_at = dst_info.alpha_type();
        let dst_cs = match dst_info.color_space() {
            Some(cs) => cs.clone(),
            None => ColorSpace::new_srgb(),
        };
        let (working_cs, working_at) = self.working_format.working_format(&dst_cs);
        let working_cs = working_cs.expect("a working color space");
        let cs_optimize = key_context.with_extra_flags(KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM);
        let working_info =
            ColorInfo::new(dst_info.color_type(), working_at, Some(working_cs.clone()));
        let working_context = cs_optimize.with_color_info(&working_info);
        compose(
            &cs_optimize,
            || {
                compose(
                    &cs_optimize,
                    || {
                        let data1 = ColorSpaceTransformData::from_color_spaces(
                            Some(&dst_cs),
                            dst_at,
                            Some(&working_cs),
                            working_at,
                        );
                        ColorSpaceTransformBlock::add_block(&cs_optimize, &data1);
                    },
                    || {
                        add_to_key(&working_context, &self.child_options, desired_combination);
                    },
                );
            },
            || {
                let data2 = ColorSpaceTransformData::from_color_spaces(
                    Some(&working_cs),
                    working_at,
                    Some(&dst_cs),
                    dst_at,
                );
                ColorSpaceTransformBlock::add_block(&cs_optimize, &data2);
            },
        );
    }
}

impl ColorFilterImpl for WithWorkingFormatColorFilter {
    fn is_alpha_unchanged(&self, desired_combination: i32) -> bool {
        let (child, child_option) = select_option(&self.child_options, desired_combination);
        child.is_none_or(|c| c.is_alpha_unchanged(child_option))
    }
}

/// `PrecompileColorFiltersPriv::WithWorkingFormat`.
// Port of: src/gpu/graphite/precompile/PrecompileColorFilter.cpp (chrome/m156)
#[must_use]
pub fn with_working_format(
    child_options: &[Option<PrecompileColorFilter>],
    tf: Option<&TransferFunction>,
    gamut: Option<&Matrix3x3>,
    at: Option<&AlphaType>,
) -> PrecompileColorFilter {
    PrecompileColorFilter {
        imp: Arc::new(WithWorkingFormatColorFilter {
            child_options: child_options.to_vec(),
            num_child_combos: sum_combinations(child_options),
            working_format: WorkingFormatCalculator::new(tf, gamut, at),
        }),
    }
}

/// `PrecompileColorFilters`: the factories for precompile color filters.
#[derive(Debug, Clone, Copy)]
pub struct PrecompileColorFilters;

impl PrecompileColorFilters {
    /// `PrecompileColorFilters::Compose`.
    #[must_use]
    pub fn compose(
        outer_options: &[Option<PrecompileColorFilter>],
        inner_options: &[Option<PrecompileColorFilter>],
    ) -> Option<PrecompileColorFilter> {
        compose_filters(outer_options, inner_options)
    }

    /// `PrecompileColorFilters::Blend(SkSpan<const SkBlendMode>)`.
    #[must_use]
    pub fn blend_modes(blend_modes: &[BlendMode]) -> PrecompileColorFilter {
        PrecompileColorFilter {
            imp: Arc::new(BlendModeColorFilter {
                blend_options: PrecompileBlenderList::from_blend_modes(blend_modes),
            }),
        }
    }

    /// `PrecompileColorFilters::Blend()`: the blend modes the default factory covers.
    // Port of: src/gpu/graphite/precompile/PrecompileColorFilter.cpp#L118-L140 (chrome/m156)
    #[must_use]
    pub fn blend() -> PrecompileColorFilter {
        Self::blend_modes(&[
            BlendMode::SrcOver, // Trigger porter-duff blends
            BlendMode::Hue,     // Trigger HSLC blends
            BlendMode::Plus,
            BlendMode::Modulate,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Multiply,
        ])
    }

    /// `PrecompileColorFilters::Matrix(clamp)`.
    #[must_use]
    pub fn matrix(clamp: bool) -> PrecompileColorFilter {
        PrecompileColorFilter {
            imp: Arc::new(MatrixColorFilter {
                in_hsla: false,
                clamp,
            }),
        }
    }

    /// `PrecompileColorFilters::HSLAMatrix()`.
    #[must_use]
    pub fn hsla_matrix() -> PrecompileColorFilter {
        PrecompileColorFilter {
            imp: Arc::new(MatrixColorFilter {
                in_hsla: true,
                clamp: true,
            }),
        }
    }

    /// `PrecompileColorFilters::LinearToSRGBGamma()`.
    #[must_use]
    pub fn linear_to_srgb_gamma() -> PrecompileColorFilter {
        color_space_xform(&[ColorSpace::new_srgb_linear()], &[ColorSpace::new_srgb()])
    }

    /// `PrecompileColorFilters::SRGBToLinearGamma()`.
    #[must_use]
    pub fn srgb_to_linear_gamma() -> PrecompileColorFilter {
        color_space_xform(&[ColorSpace::new_srgb()], &[ColorSpace::new_srgb_linear()])
    }

    /// `PrecompileColorFilters::Lerp`.
    // Port of: src/gpu/graphite/precompile/PrecompileColorFilter.cpp#L202-L220 (chrome/m156)
    #[must_use]
    pub fn lerp(
        dst_options: &[Option<PrecompileColorFilter>],
        src_options: &[Option<PrecompileColorFilter>],
    ) -> Option<PrecompileColorFilter> {
        if dst_options.is_empty() && src_options.is_empty() {
            return None;
        }
        let lerp_effect = PrecompileRuntimeEffects::known(StableKey::Lerp);
        let dsts = dst_options
            .iter()
            .map(|d| d.clone().map(PrecompileBaseHandle::ColorFilter))
            .collect::<Vec<_>>();
        let srcs = src_options
            .iter()
            .map(|s| s.clone().map(PrecompileBaseHandle::ColorFilter))
            .collect::<Vec<_>>();
        PrecompileRuntimeEffects::make_precompile_color_filter(lerp_effect, &[dsts, srcs])
    }

    /// `PrecompileColorFilters::Table()`.
    #[must_use]
    pub fn table() -> PrecompileColorFilter {
        PrecompileColorFilter {
            imp: Arc::new(TableColorFilter),
        }
    }

    /// `PrecompileColorFilters::Lighting()`: lighting filters are matrix filters.
    #[must_use]
    pub fn lighting() -> PrecompileColorFilter {
        Self::matrix(true)
    }

    /// `PrecompileColorFilters::HighContrast()`.
    // Port of: src/gpu/graphite/precompile/PrecompileColorFilter.cpp#L274-L287 (chrome/m156)
    #[must_use]
    pub fn high_contrast() -> Option<PrecompileColorFilter> {
        let cf = PrecompileRuntimeEffects::make_precompile_color_filter(
            PrecompileRuntimeEffects::known(StableKey::HighContrast),
            &[],
        )?;
        let linear = TransferFunction {
            g: 1.0,
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
        };
        let unpremul = AlphaType::Unpremul;
        Some(with_working_format(
            &[Some(cf)],
            Some(&linear),
            None,
            Some(&unpremul),
        ))
    }

    /// `PrecompileColorFilters::Luma()`.
    #[must_use]
    pub fn luma() -> Option<PrecompileColorFilter> {
        PrecompileRuntimeEffects::make_precompile_color_filter(
            PrecompileRuntimeEffects::known(StableKey::Luma),
            &[],
        )
    }

    /// `PrecompileColorFilters::Overdraw()`.
    #[must_use]
    pub fn overdraw() -> Option<PrecompileColorFilter> {
        PrecompileRuntimeEffects::make_precompile_color_filter(
            PrecompileRuntimeEffects::known(StableKey::Overdraw),
            &[],
        )
    }

    /// `PrecompileColorFiltersPriv::Gaussian()`.
    #[must_use]
    pub fn gaussian() -> PrecompileColorFilter {
        PrecompileColorFilter {
            imp: Arc::new(GaussianColorFilter),
        }
    }
}

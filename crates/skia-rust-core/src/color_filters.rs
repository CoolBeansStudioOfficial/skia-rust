// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkBlendModeColorFilter.cpp,
// src/effects/colorfilters/SkMatrixColorFilter.cpp, src/effects/colorfilters/SkTableColorFilter.cpp,
// src/effects/colorfilters/SkColorSpaceXformColorFilter.cpp, src/effects/SkColorMatrixFilter.cpp
// (SkColorFilters::Lighting), src/core/SkColorFilter.cpp, include/core/SkColorFilter.h

//! `SkColorFilters`: the color filter factories. The filters they make are in
//! [`blend`](self::blend), [`matrix`](self::matrix), [`table`](self::table), the sRGB gamma
//! filters, [`compose`](self::compose) and [`lerp`](self::lerp). The `SkSL` color filters `Luma`,
//! `HighContrast` and `Overdraw` are in `skia_rust_effects`.

use std::sync::OnceLock;

use crate::alpha_type::AlphaType;
use crate::blend_mode::BlendMode;
use crate::blend_mode_priv;
use crate::color::{Color, Color4f};
use crate::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use crate::color_matrix::ColorMatrix;
use crate::color_space::ColorSpace;
use crate::color_space_priv::{srgb_linear_singleton, srgb_singleton};
use crate::color_space_xform_color_filter::ColorSpaceXformColorFilter;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::color_table::ColorTable;
use crate::data::Data;
use crate::effect_priv::StageRec;
use crate::known_runtime_effects::{StableKey, get_known_runtime_effect};
use crate::matrix_color_filter::{Domain, make_matrix};
use crate::raster_pipeline::Stage;
use crate::runtime_effect::ChildPtr;
use crate::table_color_filter::TableColorFilter;

/// Whether a matrix filter clamps all its channels or only alpha (`SkColorFilters::Clamp`).
/// The default is [`Clamp::Yes`].
// Port of: include/core/SkColorFilter.h#L95-L95 (chrome/m156)
#[doc(alias = "SkColorFilters::Clamp")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Clamp {
    /// `kNo`: only alpha is clamped to `[0, 1]`.
    No,
    /// `kYes`: every channel is clamped to `[0, 1]`.
    Yes,
}

/// A color filter that blends a constant color with each filtered color (`SkBlendModeColorFilter`).
// Port of: src/effects/colorfilters/SkBlendModeColorFilter.h#L16-L45 (chrome/m156)
#[doc(alias = "SkBlendModeColorFilter")]
#[derive(Clone, Debug)]
pub struct BlendModeColorFilter {
    color: Color4f,
    mode: BlendMode,
}

impl ColorFilterBase for BlendModeColorFilter {
    // Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L71-L79 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _shader_is_opaque: bool) -> bool {
        rec.pipeline.append(Stage::MoveSrcDst);
        let mut color = self.color.as_array();
        ColorSpaceXformSteps::new(
            Some(srgb_singleton()),
            AlphaType::Unpremul,
            rec.dst_cs,
            AlphaType::Premul,
        )
        .apply(&mut color);
        rec.pipeline.append_constant_color(rec.alloc, &color);
        blend_mode_priv::append_stages(self.mode, rec.pipeline);
        true
    }

    // Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L35-L42 (chrome/m156)
    fn on_is_alpha_unchanged(&self) -> bool {
        // kDst: [Da, Dc]; kSrcATop: [Da, Sc * Da + (1 - Sa) * Dc]
        matches!(self.mode, BlendMode::Dst | BlendMode::SrcATop)
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::BlendMode
    }

    // Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L28-L33 (chrome/m156)
    fn on_as_a_color_mode(&self) -> Option<(Color, BlendMode)> {
        Some((self.color.to_color(), self.mode))
    }
}

/// A filter that blends `color` (in `color_space`, sRGB if `None`) with the filtered color using
/// `mode`, or `None` if the combination does nothing (`SkColorFilters::Blend(SkColor4f, ...)`).
// Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L83-L127 (chrome/m156)
#[doc(alias = "Blend")]
#[must_use]
pub fn blend(
    color: impl AsRef<Color4f>,
    color_space: Option<&ColorSpace>,
    mut mode: BlendMode,
) -> Option<ColorFilter> {
    // First map to sRGB to simplify storage in the actual SkColorFilter instance, staying
    // unpremul until the final dst color space is known when actually filtering. Also pin the
    // alpha to [0,1]
    let mut srgb = color.as_ref().pin_alpha();
    let mut arr = srgb.as_array();
    ColorSpaceXformSteps::new(
        color_space,
        AlphaType::Unpremul,
        Some(srgb_singleton()),
        AlphaType::Unpremul,
    )
    .apply(&mut arr);
    srgb = Color4f::new(arr[0], arr[1], arr[2], arr[3]);

    // Next collapse some modes if possible
    let alpha = srgb.a;
    if BlendMode::Clear == mode {
        srgb = Color4f::new(0.0, 0.0, 0.0, 0.0);
        mode = BlendMode::Src;
    } else if BlendMode::SrcOver == mode {
        #[allow(clippy::float_cmp)] // Skia compares the alpha with 0 and 1 exactly
        if 0.0 == alpha {
            mode = BlendMode::Dst;
        } else if 1.0 == alpha {
            mode = BlendMode::Src;
        }
        // else just stay srcover
    }

    // Finally weed out combinations that are noops, and just return null
    #[allow(clippy::float_cmp)] // Skia compares the alpha with 0 and 1 exactly
    if BlendMode::Dst == mode
        || (0.0 == alpha
            && matches!(
                mode,
                BlendMode::SrcOver
                    | BlendMode::DstOver
                    | BlendMode::DstOut
                    | BlendMode::SrcATop
                    | BlendMode::Xor
                    | BlendMode::Darken
            ))
        || (1.0 == alpha && BlendMode::DstIn == mode)
    {
        return None;
    }

    Some(ColorFilter::from_base(BlendModeColorFilter {
        color: srgb,
        mode,
    }))
}

/// [`blend`] with an sRGB 8-bit color (`SkColorFilters::Blend(SkColor, SkBlendMode)`).
// Port of: src/effects/colorfilters/SkBlendModeColorFilter.cpp#L129-L131 (chrome/m156)
#[doc(alias = "Blend")]
#[must_use]
pub fn blend_color(color: Color, mode: BlendMode) -> Option<ColorFilter> {
    blend(Color4f::from_color(color), None, mode)
}

/// A filter that multiplies the color by `color_matrix`, clamped per `clamp` (`Matrix(const
/// SkColorMatrix&, Clamp)`). `None` if a coefficient is not finite.
// Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L115-L117 (chrome/m156)
#[doc(alias = "Matrix")]
#[must_use]
pub fn matrix(color_matrix: &ColorMatrix, clamp: Clamp) -> Option<ColorFilter> {
    matrix_row_major(color_matrix.as_row_major(), clamp)
}

/// [`matrix`] from 20 row-major coefficients (`Matrix(const float rowMajor[20], Clamp)`).
///
/// skia-rust: Skia's `clamp = kYes` default is spelled out by the caller.
// Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L111-L113 (chrome/m156)
#[doc(alias = "Matrix")]
#[must_use]
pub fn matrix_row_major(array: &[f32; 20], clamp: Clamp) -> Option<ColorFilter> {
    make_matrix(array, Domain::Rgba, clamp).map(ColorFilter::from_base)
}

/// A filter that multiplies the color's HSLA form by `color_matrix` (`HSLAMatrix(const
/// SkColorMatrix&)`). The clamp is always [`Clamp::Yes`].
// Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L123-L125 (chrome/m156)
#[doc(alias = "HSLAMatrix")]
#[must_use]
pub fn hsla_matrix_of_color_matrix(color_matrix: &ColorMatrix) -> Option<ColorFilter> {
    hsla_matrix(color_matrix.as_row_major())
}

/// [`hsla_matrix_of_color_matrix`] from 20 row-major coefficients (`HSLAMatrix(const float
/// rowMajor[20])`).
// Port of: src/effects/colorfilters/SkMatrixColorFilter.cpp#L119-L121 (chrome/m156)
#[doc(alias = "HSLAMatrix")]
#[must_use]
pub fn hsla_matrix(row_major: &[f32; 20]) -> Option<ColorFilter> {
    make_matrix(row_major, Domain::Hsla, Clamp::Yes).map(ColorFilter::from_base)
}

/// The filter that converts linear colors to sRGB gamma (`LinearToSRGBGamma`). The same shared
/// filter is returned on every call.
// Port of: src/effects/colorfilters/SkColorSpaceXformColorFilter.cpp#L83-L87 (chrome/m156)
#[doc(alias = "LinearToSRGBGamma")]
#[must_use]
pub fn linear_to_srgb_gamma() -> ColorFilter {
    static FILTER: OnceLock<ColorFilter> = OnceLock::new();
    FILTER
        .get_or_init(|| {
            ColorFilter::from_base(ColorSpaceXformColorFilter::new(
                srgb_linear_singleton().clone(),
                srgb_singleton().clone(),
            ))
        })
        .clone()
}

/// The filter that converts sRGB gamma colors to linear (`SRGBToLinearGamma`). The same shared
/// filter is returned on every call.
// Port of: src/effects/colorfilters/SkColorSpaceXformColorFilter.cpp#L89-L93 (chrome/m156)
#[doc(alias = "SRGBToLinearGamma")]
#[must_use]
pub fn srgb_to_linear_gamma() -> ColorFilter {
    static FILTER: OnceLock<ColorFilter> = OnceLock::new();
    FILTER
        .get_or_init(|| {
            ColorFilter::from_base(ColorSpaceXformColorFilter::new(
                srgb_singleton().clone(),
                srgb_linear_singleton().clone(),
            ))
        })
        .clone()
}

/// `Lerp`: the filter that mixes `cf0` and `cf1` by `weight` (`0` gives `cf0`, `1` gives `cf1`).
/// Built from the `Lerp` known runtime effect.
// Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L140-L163 (chrome/m156)
#[doc(alias = "Lerp")]
#[must_use]
pub fn lerp(
    weight: f32,
    cf0: Option<ColorFilter>,
    cf1: Option<ColorFilter>,
) -> Option<ColorFilter> {
    if cf0.is_none() && cf1.is_none() {
        return None;
    }
    if weight.is_nan() {
        return None;
    }

    if cf0 == cf1 {
        return cf0; // or cf1
    }

    if weight <= 0.0 {
        return cf0;
    }
    if weight >= 1.0 {
        return cf1;
    }

    let lerp_effect = get_known_runtime_effect(StableKey::Lerp)?;

    let inputs = [
        cf0.map_or(ChildPtr::Empty, ChildPtr::from),
        cf1.map_or(ChildPtr::Empty, ChildPtr::from),
    ];
    lerp_effect.make_color_filter(Data::new_copy(&weight.to_ne_bytes()), &inputs)
}

/// `outer` applied after `inner` (`Compose`). A missing `outer` gives `inner`; a missing `inner`
/// gives `outer`.
// Port of: include/core/SkColorFilter.h#L84-L89 (chrome/m156)
#[doc(alias = "Compose")]
#[must_use]
pub fn compose(outer: Option<&ColorFilter>, inner: Option<ColorFilter>) -> Option<ColorFilter> {
    match outer {
        Some(outer) => Some(outer.composed(inner)),
        None => inner,
    }
}

/// A filter that maps each channel of the color through `table`, the same 256-entry map for
/// alpha, red, green and blue (`Table(const uint8_t table[256])`).
// Port of: src/effects/colorfilters/SkTableColorFilter.cpp#L56-L58 (chrome/m156)
#[doc(alias = "Table")]
#[must_use]
pub fn table(table: &[u8; 256]) -> ColorFilter {
    table_from_color_table(ColorTable::make(table))
}

/// A filter that maps each channel through its own 256-entry map. A `None` map is the identity;
/// `None` is returned when all four are the identity (`TableARGB`).
// Port of: src/effects/colorfilters/SkTableColorFilter.cpp#L60-L65 (chrome/m156)
#[doc(alias = "TableARGB")]
#[must_use]
pub fn table_argb(
    table_a: Option<&[u8; 256]>,
    table_r: Option<&[u8; 256]>,
    table_g: Option<&[u8; 256]>,
    table_b: Option<&[u8; 256]>,
) -> Option<ColorFilter> {
    ColorTable::make_argb(table_a, table_r, table_g, table_b).map(table_from_color_table)
}

/// A filter that maps the color through a [`ColorTable`] (`Table(sk_sp<SkColorTable>)`).
// Port of: src/effects/colorfilters/SkTableColorFilter.cpp#L67-L72 (chrome/m156)
#[doc(alias = "Table")]
#[must_use]
pub fn table_from_color_table(table: ColorTable) -> ColorFilter {
    ColorFilter::from_base(TableColorFilter::new(table))
}

/// `SkColorFilters::Lighting(mul, add)`: multiplies the RGB channels by `mul` and adds `add`.
/// `None` if the blend it reduces to is a no-op.
// Port of: src/effects/SkColorMatrixFilter.cpp#L26-L43 (chrome/m156)
#[doc(alias = "Lighting")]
#[must_use]
pub fn lighting(mul: Color, add: Color) -> Option<ColorFilter> {
    // omit the alpha and compare only the RGB values (`add & ~SK_ColorBLACK == 0`)
    if 0 == (add.r() | add.g() | add.b()) {
        // `mul | SK_ColorBLACK`: the same color with opaque alpha
        return blend_color(
            Color::from_argb(0xFF, mul.r(), mul.g(), mul.b()),
            BlendMode::Modulate,
        );
    }

    let mut matrix = ColorMatrix::default();
    matrix.set_scale(
        byte_to_unit_float(mul.r()),
        byte_to_unit_float(mul.g()),
        byte_to_unit_float(mul.b()),
        1.0,
    );
    matrix.post_translate(
        byte_to_unit_float(add.r()),
        byte_to_unit_float(add.g()),
        byte_to_unit_float(add.b()),
        0.0,
    );
    matrix_row_major(matrix.as_row_major(), Clamp::Yes)
}

/// `byte_to_unit_float` of `SkColorFilters::Lighting`: `1` for `0xFF`, else `byte / 255` (with
/// Skia's rounded constant).
// Port of: src/effects/SkColorMatrixFilter.cpp#L17-L24 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literal, rounded to f32 exactly as Skia does
fn byte_to_unit_float(byte: u8) -> f32 {
    if 0xFF == byte {
        // want to get this exact
        1.0
    } else {
        f32::from(byte) * 0.003_921_568_627_45_f32
    }
}

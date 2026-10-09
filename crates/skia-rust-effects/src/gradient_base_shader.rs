// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/gradients/SkGradientBaseShader.{h,cpp}

//! `SkGradientBaseShader`: what the linear, radial, sweep and conical gradients share: the
//! color stops (with the implicit first and last stops, and the hard-stop clean-up), tiling, the
//! interpolation color spaces and the pipeline stages that turn `t` into a color.
//!
//! skia-rust: Skia's subclasses of `SkGradientBaseShader` hold one as a field
//! ([`GradientBaseShader`]) and call its [`append_stages`](GradientBaseShader::append_stages)
//! with their own `appendGradientStages` as a closure. The cached colors-and-offsets bitmap is
//! kept for the Graphite key ([`GradientBaseShader::cached_bitmap`]). Serialization (`flatten`,
//! `unflatten`) and the GPU's `forceExplicitPositions` are not ported.

use std::sync::OnceLock;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_primaries, named_transfer_fn};
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::float_bits::{bits_to_float, float_to_bits};
use skia_rust_core::floating_point::{float_radians_to_degrees, ieee_float_divide, is_finite};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::raster_pipeline::contexts::{
    DecalTileCtx, EvenlySpaced2StopGradientCtx, GradientCtx,
};
use skia_rust_core::raster_pipeline::{RasterPipeline, Stage};
use skia_rust_core::scalar::{SCALAR_1, Scalar, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{self, MatrixRec};
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::tile_mode::TileMode;

use crate::gradient::interpolation::{ColorSpace as InterpColorSpace, HueMethod, InPremul};
use crate::gradient::{Colors, Gradient, Interpolation};

/// Four-wide `float4` helpers on arrays (`skvx::float4` as the gradient code uses it: lane-wise
/// IEEE arithmetic).
type Float4 = [f32; 4];

/// `kRGBAChannels`.
const RGBA_CHANNELS: usize = 4;

/// The state common to all gradient shaders (`SkGradientBaseShader`).
// Port of: src/shaders/gradients/SkGradientBaseShader.h#L54-L178 (chrome/m156)
#[doc(alias = "SkGradientBaseShader")]
#[derive(Clone, Debug)]
pub struct GradientBaseShader {
    pts_to_unit: Matrix,
    tile_mode: TileMode,
    /// `fColors` (`fColorCount` of them).
    colors: Vec<Color4f>,
    /// `fPositions`, or `None` if the stops are evenly spaced.
    positions: Option<Vec<f32>>,
    /// Color space of the gradient stops; never null in Skia (sRGB for none).
    color_space: ColorSpace,
    interpolation: Interpolation,
    first_stop_is_implicit: bool,
    last_stop_is_implicit: bool,
    colors_are_opaque: bool,
    /// `fCachedBitmap`: the colors-and-offsets bitmap of a gradient with more stops than fit
    /// inline, made once for the Graphite key (set through [`Self::set_cached_bitmap`]).
    cached_bitmap: OnceLock<Bitmap>,
}

/// `kDegenerateThreshold`: the default `SkScalarNearlyZero` threshold of .0024 is too big and
/// causes regressions for svg gradients defined in the wild.
// Port of: src/shaders/gradients/SkGradientBaseShader.h#L84-L86 (chrome/m156)
#[doc(alias = "kDegenerateThreshold")]
#[allow(clippy::cast_precision_loss)] // mirrors the C++ int -> float cast; 1 << 15 is exact
pub const DEGENERATE_THRESHOLD: scalar = SCALAR_1 / (1 << 15) as scalar;

impl GradientBaseShader {
    /// `SkGradientBaseShader(desc, ptsToUnit)`.
    ///
    /// # Panics
    /// If the gradient has fewer than two colors (the factories make color shaders for those).
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L170-L282 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp)] // Skia compares positions exactly
    #[allow(clippy::too_many_lines)] // one constructor in Skia
    pub fn new(desc: &Gradient<'_>, pts_to_unit: Matrix) -> GradientBaseShader {
        let colors = desc.colors().colors();
        let pos = desc.colors().positions().unwrap_or(&[]);

        // fPtsToUnit.getType(): Precache so reads are threadsafe.
        let _ = pts_to_unit.get_type();
        assert!(colors.len() > 1);

        let interpolation = *desc.interpolation();
        let tile_mode = desc.colors().tile_mode();

        // Note: we let the caller skip the first and/or last position.
        // i.e. pos[0] = 0.3, pos[1] = 0.7
        // In these cases, we insert entries to ensure that the final data
        // will be bracketed by [0, 1].
        // i.e. our_pos[0] = 0, our_pos[1] = 0.3, our_pos[2] = 0.7, our_pos[3] = 1
        //
        // Thus colorCount (the caller's value, and fColorCount (our value) may
        // differ by up to 2. In the above example:
        //     colorCount = 2
        //     fColorCount = 4
        let mut color_count = colors.len();
        let mut first_stop_is_implicit = false;
        let mut last_stop_is_implicit = false;

        // Check if we need to add in start and/or end position/colors
        if !pos.is_empty() {
            first_stop_is_implicit = pos[0] > 0.0;
            last_stop_is_implicit = pos[pos.len() - 1] != SCALAR_1;
            color_count += usize::from(first_stop_is_implicit) + usize::from(last_stop_is_implicit);
        }

        // Now copy over the colors, adding the duplicates at t=0 and t=1 as needed
        let mut f_colors = Vec::with_capacity(color_count);
        let mut colors_are_opaque = true;
        if first_stop_is_implicit {
            f_colors.push(colors[0]);
        }
        for c in colors {
            f_colors.push(*c);
            colors_are_opaque = colors_are_opaque && (c.a == 1.0);
        }
        if last_stop_is_implicit {
            f_colors.push(colors[colors.len() - 1]);
        }
        debug_assert_eq!(f_colors.len(), color_count);

        let mut f_positions: Option<Vec<f32>> = None;
        if !pos.is_empty() {
            let mut prev: scalar = 0.0;
            let mut positions = Vec::with_capacity(color_count);
            positions.push(prev); // force the first pos to 0

            let start_index = usize::from(!first_stop_is_implicit);
            let count = pos.len() + usize::from(last_stop_is_implicit);

            let mut uniform_stops = true;
            let uniform_step = pos[start_index] - prev;
            for i in start_index..count {
                // Pin the last value to 1.0, and make sure pos is monotonic.
                let mut curr = 1.0f32;
                if i != pos.len() {
                    curr = t_pin(pos[i], prev, 1.0f32);

                    // If a value is clamped to 1.0 before the last stop, the last stop
                    // actually isn't implicit if we thought it was.
                    if curr == 1.0 && last_stop_is_implicit {
                        last_stop_is_implicit = false;
                    }
                }

                uniform_stops &= <scalar as Scalar>::nearly_equal(uniform_step, curr - prev, None);

                prev = curr;
                positions.push(curr);
            }
            debug_assert_eq!(positions.len(), color_count);

            if uniform_stops {
                // If the stops are uniform, treat them as implicit.
                f_positions = None;
            } else {
                // Remove duplicate stops with more than two of the same stop,
                // keeping the leftmost and rightmost stop colors.
                // i.e.       0, 0, 0,   0.2, 0.2, 0.3, 0.3, 0.3, 1, 1
                // w/  clamp  0,    0,   0.2, 0.2, 0.3,      0.3, 1, 1
                // w/o clamp        0,   0.2, 0.2, 0.3,      0.3, 1
                let mut i = 0usize;
                let mut deduped_color_count = 0usize;
                for j in 1..=color_count {
                    // We can compare the current positions at i and j since once these fPosition
                    // values are overwritten, our i and j pointers will be past the overwritten
                    // values.
                    if j == color_count || positions[i] != positions[j] {
                        debug_assert!(j >= i);
                        let dup_stop = j - i > 1;

                        // Ignore the leftmost stop (i) if it is a non-clamp tilemode with
                        // a duplicate stop on t = 0.
                        let ignore_leftmost =
                            dup_stop && tile_mode != TileMode::Clamp && positions[i] == 0.0;
                        if !ignore_leftmost {
                            positions[deduped_color_count] = positions[i];
                            f_colors[deduped_color_count] = f_colors[i];
                            deduped_color_count += 1;
                        }

                        // Include the rightmost stop (j-1) only if the stop has a duplicate,
                        // ignoring the rightmost stop if it is a non-clamp tilemode with t = 1.
                        let ignore_rightmost =
                            tile_mode != TileMode::Clamp && positions[j - 1] == 1.0;
                        if dup_stop && !ignore_rightmost {
                            positions[deduped_color_count] = positions[j - 1];
                            f_colors[deduped_color_count] = f_colors[j - 1];
                            deduped_color_count += 1;
                        }
                        i = j;
                    }
                }
                positions.truncate(deduped_color_count);
                f_colors.truncate(deduped_color_count);
                f_positions = Some(positions);
            }
        }

        GradientBaseShader {
            pts_to_unit,
            tile_mode,
            colors: f_colors,
            positions: f_positions,
            color_space: desc
                .colors()
                .color_space()
                .cloned()
                .unwrap_or_else(ColorSpace::new_srgb),
            interpolation,
            first_stop_is_implicit,
            last_stop_is_implicit,
            colors_are_opaque,
            cached_bitmap: OnceLock::new(),
        }
    }

    /// The colors-and-offsets bitmap cached for the Graphite key, if one was set
    /// (`cachedBitmap()`). Its pixel ref identifies the gradient's texture in the proxy cache.
    // Port of: src/shaders/gradients/SkGradientBaseShader.h#L168-L169 (chrome/m156), `cachedBitmap`
    #[doc(alias = "cachedBitmap")]
    #[must_use]
    pub fn cached_bitmap(&self) -> Option<&Bitmap> {
        self.cached_bitmap.get()
    }

    /// Caches the colors-and-offsets bitmap (`setCachedBitmap`). The first bitmap set is kept:
    /// the key code only sets one when none is cached.
    // Port of: src/shaders/gradients/SkGradientBaseShader.h#L170-L171 (chrome/m156), `setCachedBitmap`
    #[doc(alias = "setCachedBitmap")]
    pub fn set_cached_bitmap(&self, bitmap: Bitmap) {
        let _ = self.cached_bitmap.set(bitmap);
    }

    /// The matrix mapping the gradient's points to the unit space (`getGradientMatrix`).
    #[doc(alias = "getGradientMatrix")]
    #[must_use]
    pub fn gradient_matrix(&self) -> &Matrix {
        &self.pts_to_unit
    }

    /// The tile mode (`getTileMode`).
    #[doc(alias = "getTileMode")]
    #[must_use]
    pub fn tile_mode(&self) -> TileMode {
        self.tile_mode
    }

    /// The number of colors (`getColorCount`), including implicit stops.
    #[doc(alias = "getColorCount")]
    #[must_use]
    pub fn color_count(&self) -> usize {
        self.colors.len()
    }

    /// The colors, including implicit stops (`colors`).
    #[must_use]
    pub fn colors(&self) -> &[Color4f] {
        &self.colors
    }

    /// The positions if they are not evenly spaced (`positions`).
    #[must_use]
    pub fn positions(&self) -> Option<&[f32]> {
        self.positions.as_deref()
    }

    /// Position of the `i`th stop (`getPos`).
    // Port of: src/shaders/gradients/SkGradientBaseShader.h#L124-L127 (chrome/m156)
    #[doc(alias = "getPos")]
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar(i) / (fColorCount - 1)
    pub fn get_pos(&self, i: usize) -> f32 {
        debug_assert!(i < self.colors.len());
        match &self.positions {
            Some(p) => p[i],
            None => (i as f32) / ((self.colors.len() - 1) as f32),
        }
    }

    /// The `i`th color as a legacy 8-bit color (`getLegacyColor`).
    #[doc(alias = "getLegacyColor")]
    #[must_use]
    pub fn get_legacy_color(&self, i: usize) -> skia_rust_core::color::Color {
        self.colors[i].to_color()
    }

    /// Whether the first stop was added by the constructor (`firstStopIsImplicit`).
    #[doc(alias = "firstStopIsImplicit")]
    #[must_use]
    pub fn first_stop_is_implicit(&self) -> bool {
        self.first_stop_is_implicit
    }

    /// Whether the last stop was added by the constructor (`lastStopIsImplicit`).
    #[doc(alias = "lastStopIsImplicit")]
    #[must_use]
    pub fn last_stop_is_implicit(&self) -> bool {
        self.last_stop_is_implicit
    }

    /// The color space of the gradient stops (`colorSpace`).
    #[doc(alias = "colorSpace")]
    #[must_use]
    pub fn color_space(&self) -> &ColorSpace {
        &self.color_space
    }

    /// The interpolation settings (`interpolation`).
    #[must_use]
    pub fn interpolation(&self) -> &Interpolation {
        &self.interpolation
    }

    /// Whether the interpolation is in premul (`interpolateInPremul`).
    #[doc(alias = "interpolateInPremul")]
    #[must_use]
    pub fn interpolate_in_premul(&self) -> bool {
        self.interpolation.in_premul == InPremul::Yes
    }

    /// Whether every color is opaque (`colorsAreOpaque`).
    #[doc(alias = "colorsAreOpaque")]
    #[must_use]
    pub fn colors_are_opaque(&self) -> bool {
        self.colors_are_opaque
    }

    /// `SkShader::isOpaque`.
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L520-L522 (chrome/m156)
    #[doc(alias = "isOpaque")]
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.colors_are_opaque && (self.tile_mode != TileMode::Decal)
    }

    /// The average color, as `onAsLuminanceColor` (`SkShaderBase::onAsLuminanceColor`).
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L524-L543 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // mirrors 1.0f / fColorCount
    pub fn on_as_luminance_color(&self) -> Color4f {
        // We just compute an average color. There are several things we could do better:
        // 1) We already have a different average_gradient_color helper later in this file, that
        //    weights contribution by the relative size of each band.
        // 2) Colors should be converted to some standard color space! These could be in any
        //    space.
        // 3) Do we want to average in the source space, sRGB, or some linear space?
        let mut color = Color4f::new(0.0, 0.0, 0.0, 1.0);
        for c in &self.colors {
            color.r += c.r;
            color.g += c.g;
            color.b += c.b;
        }
        let scale = 1.0f32 / self.colors.len() as f32;
        color.r *= scale;
        color.g *= scale;
        color.b *= scale;
        color
    }

    /// Fills `info` with the parameters common to all gradients (`commonAsAGradient`).
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L1006-L1025 (chrome/m156)
    #[doc(alias = "commonAsAGradient")]
    pub fn common_as_a_gradient(
        &self,
        info: Option<&mut skia_rust_core::shaders::GradientInfo<'_>>,
    ) {
        if let Some(info) = info {
            let color_count = self.colors.len();
            if info.color_count >= color_count {
                if let Some(out) = info.colors.as_deref_mut() {
                    out[..color_count].copy_from_slice(&self.colors);
                }
                if let Some(out) = info.color_offsets.as_deref_mut() {
                    for (i, o) in out[..color_count].iter_mut().enumerate() {
                        *o = self.get_pos(i);
                    }
                }
            }
            info.color_count = color_count;
            info.tile_mode = self.tile_mode;
            info.premul_interp = self.interpolate_in_premul();
        }
    }

    /// True if these parameters are valid/legal/safe to construct a gradient
    /// (`ValidGradient`).
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L1030-L1036 (chrome/m156)
    #[doc(alias = "ValidGradient")]
    #[must_use]
    pub fn valid_gradient(
        colors: &[Color4f],
        _tile_mode: TileMode,
        _interpolation: &Interpolation,
    ) -> bool {
        // (The tile mode and interpolation enums can hold no out-of-range values.)
        !colors.is_empty()
    }

    /// Except for special circumstances of clamped gradients, every gradient shape--when
    /// degenerate--can be mapped to the same fallbacks. The specific shape factories must
    /// account for special clamped conditions separately, this will always return the last color
    /// for clamped gradients (`MakeDegenerateGradient`).
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L1100-L1126 (chrome/m156)
    #[doc(alias = "MakeDegenerateGradient")]
    #[must_use]
    pub fn make_degenerate_gradient(c: &Colors<'_>) -> Option<Shader> {
        match c.tile_mode() {
            // normally this would reject the area outside of the interpolation region, so since
            // inside region is empty when the radii are equal, the entire draw region is empty
            TileMode::Decal => Some(shaders::empty()),
            // repeat and mirror are treated the same: the border colors are never visible,
            // but approximate the final color as infinite repetitions of the colors, so
            // it can be represented as the average color of the gradient.
            TileMode::Repeat | TileMode::Mirror => shaders::color_in_space(
                average_gradient_color(c.colors(), c.positions().unwrap_or(&[])),
                c.color_space().cloned(),
            ),
            // Depending on how the gradient shape degenerates, there may be a more specialized
            // fallback representation for the factories to use, but this is a reasonable
            // default.
            TileMode::Clamp => {
                shaders::color_in_space(c.colors()[c.colors().len() - 1], c.color_space().cloned())
            }
        }
    }

    /// Adds the stages of the gradient: the matrix to unit space, `append_gradient_stages`
    /// (the shape's `t` stages, and the stages that must run after the colors), tiling, the
    /// color lookup and the conversion to the destination (`appendStages`).
    ///
    /// Returns false if the matrix is not invertible.
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L437-L518 (chrome/m156)
    #[doc(alias = "appendStages")]
    pub fn append_stages<'a>(
        &self,
        rec: &mut StageRec<'_, 'a>,
        m_rec: &MatrixRec,
        append_gradient_stages: impl FnOnce(
            &'a ArenaAlloc,
            &mut RasterPipeline<'a>,
            &mut RasterPipeline<'a>,
        ),
    ) -> bool {
        let alloc = rec.alloc;
        let mut decal_ctx: Option<&'a DecalTileCtx> = None;

        let new_m_rec = m_rec.apply(rec, &self.pts_to_unit);
        if new_m_rec.is_none() {
            return false;
        }

        let mut post_pipeline = RasterPipeline::new();

        append_gradient_stages(alloc, rec.pipeline, &mut post_pipeline);

        let p = &mut *rec.pipeline;
        match self.tile_mode {
            TileMode::Mirror => p.append(Stage::MirrorX1),
            TileMode::Repeat => p.append(Stage::RepeatX1),
            TileMode::Decal | TileMode::Clamp => {
                if self.tile_mode == TileMode::Decal {
                    let ctx = alloc.make(DecalTileCtx {
                        limit_x: bits_to_float(float_to_bits(1.0f32) + 1),
                        ..DecalTileCtx::default()
                    });
                    decal_ctx = Some(ctx);
                    // reuse mask + limit_x stage, or create a custom decal_1 that just stores
                    // the mask
                    p.append(Stage::DecalX(ctx));
                }
                if self.positions.is_none() {
                    // We clamp only when the stops are evenly spaced.
                    // If not, there may be hard stops, and clamping ruins hard stops at 0
                    // and/or 1. In that case, we must make sure we're using the general
                    // "gradient" stage, which is the only stage that will correctly handle
                    // unclamped t.
                    p.append(Stage::ClampX1);
                }
            }
        }

        // Transform all of the colors to destination color space, possibly premultiplied
        let xformed_colors = Color4fXformer::new(self, rec.dst_cs);
        append_gradient_fill_stages(
            p,
            alloc,
            &xformed_colors.colors,
            xformed_colors.positions.as_deref(),
            xformed_colors.colors.len(),
        );
        append_interpolated_to_dst_stages(
            p,
            alloc,
            self.colors_are_opaque,
            &self.interpolation,
            xformed_colors.intermediate_color_space.as_ref(),
            rec.dst_cs,
        );

        if let Some(decal_ctx) = decal_ctx {
            p.append(Stage::CheckDecalMask(decal_ctx));
        }

        p.extend(&post_pipeline);

        true
    }
}

/// Calculates a factor F and a bias B so that color = F*t + B when t is in range of
/// the stop. Assume that all stops have width 1/gapCount and the stop parameter
/// refers to the nth stop (`init_stop_evenly`).
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L322-L351 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors (stop / gapCount): size_t converts to float
fn init_stop_evenly(
    ctx: &mut GradientCtx,
    gap_count: f32,
    stop: usize,
    left_c: PMColor4f,
    right_c: PMColor4f,
) {
    let left = [left_c.r, left_c.g, left_c.b, left_c.a];
    let right = [right_c.r, right_c.g, right_c.b, right_c.a];

    // We start with the following 2 linear equations and 2 unknowns (factor, bias)
    // left = factor * t + bias
    // right = factor * (t + gap) + bias
    // gap = 1/gapCount
    // t = gap * stop

    // right - left = factor * (t + gap) - factor * t
    // right - left = factor * gap
    // factor = (right - left) / gap  (and gap = 1/gapCount)
    let factor4: Float4 = core::array::from_fn(|i| (right[i] - left[i]) * gap_count);
    let bias: Float4 = core::array::from_fn(|i| left[i] - (factor4[i] * (stop as f32 / gap_count)));

    add_stop_color(ctx, stop, &factor4, &bias);
}

/// Calculates a factor F and a bias B so that color = F*t + B when t is in range of
/// the stop. Unlike `init_stop_evenly`, this handles stops at arbitrary positions
/// (`init_stop_pos`).
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L353-L380 (chrome/m156)
fn init_stop_pos(
    ctx: &mut GradientCtx,
    stop: usize,
    t_l: f32,
    gap_reciprocal: f32,
    left_c: PMColor4f,
    right_c: PMColor4f,
) {
    // gapReciprocal is 1/gapWidth. If two colors were on top of each other, we should
    // have skipped that as a "stop".
    debug_assert!(is_finite(gap_reciprocal));

    let left = [left_c.r, left_c.g, left_c.b, left_c.a];
    let right = [right_c.r, right_c.g, right_c.b, right_c.a];

    // See init_stop_evenly for this derivation, noting that gap = 1/gapReciprocal
    // and t = t_l
    let factor4: Float4 = core::array::from_fn(|i| (right[i] - left[i]) * gap_reciprocal);
    let bias: Float4 = core::array::from_fn(|i| left[i] - (factor4[i] * t_l));

    ctx.ts[stop] = t_l;
    add_stop_color(ctx, stop, &factor4, &bias);
}

/// `add_stop_color`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L292-L305 (chrome/m156)
fn add_stop_color(ctx: &mut GradientCtx, stop: usize, fs: &Float4, bs: &Float4) {
    for c in 0..RGBA_CHANNELS {
        ctx.factors[c][stop] = fs[c];
        ctx.biases[c][stop] = bs[c];
    }
}

/// `add_const_color`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L307-L311 (chrome/m156)
fn add_const_color(ctx: &mut GradientCtx, stop: usize, color: PMColor4f) {
    add_stop_color(ctx, stop, &[0.0; 4], &[color.r, color.g, color.b, color.a]);
}

/// Appends the stages that map `t` (in `r`) to a color of `pm_colors` (at `positions`, or
/// evenly spaced) (`AppendGradientFillStages`).
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L382-L508 (chrome/m156)
#[doc(alias = "AppendGradientFillStages")]
#[allow(clippy::cast_precision_loss)] // mirrors the size_t -> float conversions
#[allow(clippy::items_after_statements)] // keeps Skia's local constant where it is declared
#[allow(clippy::if_not_else, clippy::bool_to_int_with_if)] // mirrors Skia's stop selection
pub fn append_gradient_fill_stages<'a>(
    p: &mut RasterPipeline<'a>,
    alloc: &'a ArenaAlloc,
    pm_colors: &[PMColor4f],
    positions: Option<&[f32]>,
    count: usize,
) {
    // The two-stop case with stops at 0 and 1.
    if count == 2 && positions.is_none() {
        let (c_l, c_r) = (pm_colors[0], pm_colors[1]);
        let ctx = alloc.make(EvenlySpaced2StopGradientCtx {
            factor: [c_r.r - c_l.r, c_r.g - c_l.g, c_r.b - c_l.b, c_r.a - c_l.a],
            bias: [c_l.r, c_l.g, c_l.b, c_l.a],
        });

        p.append(Stage::EvenlySpaced2StopGradient(ctx));
        return;
    }
    // Linear gradients with evenly spaced stops involve doing calculations to interpolate
    // between color n and color n+1 based on t (in range [0.0,1.0]).
    //   color_n * (t - t_n) / gap_n + color_{n+1} * (t_{n+1} - t) / gap_n
    // We could just stick the colors and the gaps calculation in RP and do this calculation,
    // but instead we can precompute things to make the RP calculation simpler and faster.
    // For each gap, we calculate four linear equations in the form y = m*x + b, or rather
    //  color_channel = factor * t + bias
    // We do this pre-computation in init_stop_evenly and init_stop_pos.

    // Allocate at least enough for the AVX2 gather from a YMM register.
    const MAX_REGISTER_SIZE: usize = 8;

    // There are n - 1 gaps between n colors plus 2 regions to the left and right
    // of the gradient to account for colors. For evenly spaced gradients, we cheat
    // and skip the left gap, using one block of floats unused.
    let factor_bias_floats = core::cmp::max(count + 1, MAX_REGISTER_SIZE);
    let ts_for_arbitrary_stops = count + 1;

    // We need space for all factors and biases, and while we are at it, some space
    // if we need to include the arbitrary stops.
    let mut ctx = GradientCtx {
        stop_count: 0,
        factors: core::array::from_fn(|_| vec![0.0; factor_bias_floats]),
        biases: core::array::from_fn(|_| vec![0.0; factor_bias_floats]),
        ts: Vec::new(),
    };

    let Some(positions) = positions else {
        // Handle evenly distributed stops.

        let stop_count = count;
        let gap_count = (stop_count - 1) as f32;

        let mut c_l = pm_colors[0];
        let mut i = 0usize;
        while (i as f32) < gap_count {
            let c_r = pm_colors[i + 1];
            init_stop_evenly(&mut ctx, gap_count, i, c_l, c_r);
            c_l = c_r;
            i += 1;
        }
        add_const_color(&mut ctx, stop_count - 1, c_l);

        ctx.stop_count = stop_count;
        p.append(Stage::EvenlySpacedGradient(alloc.make(ctx)));
        return;
    };

    // Handle arbitrary stops.
    ctx.ts = vec![0.0; ts_for_arbitrary_stops];

    // Remove the default stops inserted by SkGradientBaseShader::SkGradientBaseShader
    // because they are naturally handled by the search method.
    let (first_stop, last_stop): (usize, usize);
    if count > 2 {
        first_stop = if pm_colors[0] != pm_colors[1] { 0 } else { 1 };
        last_stop = if pm_colors[count - 2] != pm_colors[count - 1] {
            count - 1
        } else {
            count - 2
        };
    } else {
        first_stop = 0;
        last_stop = 1;
    }

    let mut stop_count = 0usize;
    let mut t_l = positions[first_stop];
    let mut c_l = pm_colors[first_stop];
    add_const_color(&mut ctx, stop_count, c_l);
    stop_count += 1;

    for i in first_stop..last_stop {
        let t_r = positions[i + 1];
        let c_r = pm_colors[i + 1];
        debug_assert!(t_l <= t_r);
        if t_l < t_r {
            let c_scale = ieee_float_divide(1.0, t_r - t_l);
            if is_finite(c_scale) {
                init_stop_pos(&mut ctx, stop_count, t_l, c_scale, c_l, c_r);
                stop_count += 1;
            }
        }
        t_l = t_r;
        c_l = c_r;
    }

    ctx.ts[stop_count] = t_l;
    add_const_color(&mut ctx, stop_count, c_l);
    stop_count += 1;

    ctx.stop_count = stop_count;
    p.append(Stage::Gradient(alloc.make(ctx)));
}

/// Appends the stages that convert the interpolated colors from the interpolation color space
/// to the destination (`AppendInterpolatedToDstStages`).
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L510-L586 (chrome/m156)
#[doc(alias = "AppendInterpolatedToDstStages")]
pub fn append_interpolated_to_dst_stages<'a>(
    p: &mut RasterPipeline<'a>,
    alloc: &'a ArenaAlloc,
    colors_are_opaque: bool,
    interpolation: &Interpolation,
    intermediate_color_space: Option<&ColorSpace>,
    dst_color_space: Option<&ColorSpace>,
) {
    use InterpColorSpace as CS;
    let mut color_is_premul = interpolation.in_premul == InPremul::Yes;

    // If we interpolated premul colors in any of the special color spaces, we need to unpremul
    if color_is_premul && !colors_are_opaque {
        match interpolation.color_space {
            CS::Lab | CS::OKLab | CS::OKLabGamutMap => {
                p.append(Stage::Unpremul);
                color_is_premul = false;
            }
            CS::LCH | CS::OKLCH | CS::OKLCHGamutMap | CS::HSL | CS::HWB => {
                p.append(Stage::UnpremulPolar);
                color_is_premul = false;
            }
            _ => {}
        }
    }

    // Convert colors in exotic spaces back to their intermediate SkColorSpace
    match interpolation.color_space {
        CS::Lab => p.append(Stage::CssLabToXyz),
        CS::OKLab => p.append(Stage::CssOklabToLinearSrgb),
        CS::OKLabGamutMap => p.append(Stage::CssOklabGamutMapToLinearSrgb),
        CS::LCH => {
            p.append(Stage::CssHclToLab);
            p.append(Stage::CssLabToXyz);
        }
        CS::OKLCH => {
            p.append(Stage::CssHclToLab);
            p.append(Stage::CssOklabToLinearSrgb);
        }
        CS::OKLCHGamutMap => {
            p.append(Stage::CssHclToLab);
            p.append(Stage::CssOklabGamutMapToLinearSrgb);
        }
        CS::HSL => p.append(Stage::CssHslToSrgb),
        CS::HWB => p.append(Stage::CssHwbToSrgb),
        _ => {}
    }

    // Now transform from intermediate to destination color space.
    // See comments in GrGradientShader.cpp about the decisions here.
    let dst_color_space = dst_color_space.unwrap_or_else(|| srgb_singleton());
    let mut intermediate_alpha_type = if color_is_premul {
        AlphaType::Premul
    } else {
        AlphaType::Unpremul
    };
    // TODO(skbug.com/40044213): Get dst alpha type correctly
    let mut dst_alpha_type = AlphaType::Premul;

    if colors_are_opaque {
        intermediate_alpha_type = AlphaType::Unpremul;
        dst_alpha_type = AlphaType::Unpremul;
    }

    ColorSpaceXformSteps::new(
        intermediate_color_space,
        intermediate_alpha_type,
        Some(dst_color_space),
        dst_alpha_type,
    )
    .apply_to_pipeline(p, alloc);
}

/// `intermediate_color_space`: the color space the colors are converted to before they are
/// converted to the interpolation color space (if it is not an `SkColorSpace`).
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L558-L611 (chrome/m156)
fn intermediate_color_space(cs: InterpColorSpace, dst: Option<&ColorSpace>) -> Option<ColorSpace> {
    use InterpColorSpace as CS;
    match cs {
        CS::Destination => dst.cloned(),

        // css-color-4 allows XYZD50 and XYZD65. For gradients, those are redundant. Interpolating
        // in any linear RGB space, (regardless of white point), gives the same answer.
        CS::SRGBLinear => Some(ColorSpace::new_srgb_linear()),

        CS::SRGB | CS::HSL | CS::HWB => Some(ColorSpace::new_srgb()),

        CS::Lab | CS::LCH => {
            // Conversion to Lab (and LCH) starts with XYZD50
            ColorSpace::new_rgb(&named_transfer_fn::LINEAR, &named_gamut::XYZ)
        }

        CS::OKLab | CS::OKLabGamutMap | CS::OKLCH | CS::OKLCHGamutMap => {
            // The "standard" conversion to these spaces starts with XYZD65. That requires extra
            // effort to conjure. The author also has reference code for going directly from
            // linear sRGB, so we use that.
            // TODO(skbug.com/40044213): Even better would be to have an LMS color space, because
            // the first part of the conversion is a matrix multiply, which could be absorbed
            // into the color space xform.
            Some(ColorSpace::new_srgb_linear())
        }

        // These rectangular color spaces have their own transfer curves.
        CS::DisplayP3 => ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3),

        CS::Rec2020 => ColorSpace::new_rgb(&named_transfer_fn::REC2020, &named_gamut::REC2020),

        CS::ProphotoRGB => {
            let lin_pro_photo_to_xyz_d50 = named_primaries::PRO_PHOTO_RGB
                .to_xyzd50()
                .expect("the ProPhoto primaries are valid");
            ColorSpace::new_rgb(&named_transfer_fn::PRO_PHOTO_RGB, &lin_pro_photo_to_xyz_d50)
        }

        CS::A98RGB => ColorSpace::new_rgb(&named_transfer_fn::A98_RGB, &named_gamut::ADOBE_RGB),
    }
}

/// `std::max({a, b, c})`: the first of the largest, with `a < b` comparisons.
fn max3(a: f32, b: f32, c: f32) -> f32 {
    let mut result = a;
    if result < b {
        result = b;
    }
    if result < c {
        result = c;
    }
    result
}

/// `std::min({a, b, c})`.
fn min3(a: f32, b: f32, c: f32) -> f32 {
    let mut result = a;
    if b < result {
        result = b;
    }
    if c < result {
        result = c;
    }
    result
}

/// `std::min(a, b)`.
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

type ConvertColorProc = fn(PMColor4f, &mut bool) -> PMColor4f;
type PremulColorProc = fn(PMColor4f) -> PMColor4f;

/// `srgb_to_hsl`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L616-L641 (chrome/m156)
#[allow(clippy::float_cmp)] // Skia compares exactly
#[allow(clippy::manual_midpoint)] // Skia computes (mn + mx) / 2 with this rounding
fn srgb_to_hsl(rgb: PMColor4f, hue_is_powerless: &mut bool) -> PMColor4f {
    let mx = max3(rgb.r, rgb.g, rgb.b);
    let mn = min3(rgb.r, rgb.g, rgb.b);
    let mut hue = 0.0f32;
    let mut sat = 0.0f32;
    let light = (mn + mx) / 2.0;
    let d = mx - mn;

    if d != 0.0 {
        sat = if light == 0.0 || light == 1.0 {
            0.0
        } else {
            (mx - light) / std_min(light, 1.0 - light)
        };
        if mx == rgb.r {
            hue = (rgb.g - rgb.b) / d + (if rgb.g < rgb.b { 6.0 } else { 0.0 });
        } else if mx == rgb.g {
            hue = (rgb.b - rgb.r) / d + 2.0;
        } else {
            hue = (rgb.r - rgb.g) / d + 4.0;
        }

        hue *= 60.0;
    }
    if sat == 0.0 {
        *hue_is_powerless = true;
    }
    PMColor4f {
        r: hue,
        g: sat * 100.0,
        b: light * 100.0,
        a: rgb.a,
    }
}

/// `srgb_to_hwb`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L643-L648 (chrome/m156)
fn srgb_to_hwb(rgb: PMColor4f, hue_is_powerless: &mut bool) -> PMColor4f {
    let hsl = srgb_to_hsl(rgb, hue_is_powerless);
    let white = min3(rgb.r, rgb.g, rgb.b);
    let black = 1.0 - max3(rgb.r, rgb.g, rgb.b);
    PMColor4f {
        r: hsl.r,
        g: white * 100.0,
        b: black * 100.0,
        a: rgb.a,
    }
}

/// `xyzd50_to_lab`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L650-L665 (chrome/m156)
fn xyzd50_to_lab(xyz: PMColor4f, _hue_is_powerless: &mut bool) -> PMColor4f {
    const D50: [f32; 3] = [
        0.3457f32 / 0.3585f32,
        1.0f32,
        (1.0f32 - 0.3457f32 - 0.3585f32) / 0.3585f32,
    ];

    const E: f32 = 216.0f32 / 24389.0f32;
    const K: f32 = 24389.0f32 / 27.0f32;

    let xyz_a = [xyz.r, xyz.g, xyz.b];
    let mut f = [0.0f32; 3];
    for i in 0..3 {
        let v = xyz_a[i] / D50[i];
        // skia-rust: libm (`cbrtf`)
        f[i] = if v > E {
            v.cbrt()
        } else {
            (K * v + 16.0) / 116.0
        };
    }

    PMColor4f {
        r: (116.0 * f[1]) - 16.0,
        g: 500.0 * (f[0] - f[1]),
        b: 200.0 * (f[1] - f[2]),
        a: xyz.a,
    }
}

/// The color space is technically LCH, but we produce HCL, so that all polar spaces have hue in
/// the first component. This simplifies the hue handling for `HueMethod` and premul/unpremul
/// (`xyzd50_to_hcl`).
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L667-L681 (chrome/m156)
#[allow(clippy::items_after_statements)] // keeps Skia's local constant where it is declared
fn xyzd50_to_hcl(xyz: PMColor4f, hue_is_powerless: &mut bool) -> PMColor4f {
    let lab = xyzd50_to_lab(xyz, hue_is_powerless);
    // skia-rust: libm (`atan2f`)
    let hue = float_radians_to_degrees(lab.b.atan2(lab.g));
    let chroma = (lab.g * lab.g + lab.b * lab.b).sqrt();
    // The LCH math produces small-ish (but not tiny) chroma values for achromatic colors:
    const MAX_CHROMA_FOR_POWERLESS_HUE: f32 = 1e-2;
    if chroma <= MAX_CHROMA_FOR_POWERLESS_HUE {
        *hue_is_powerless = true;
    }
    PMColor4f {
        r: if hue >= 0.0 { hue } else { hue + 360.0 },
        g: chroma,
        b: lab.r,
        a: xyz.a,
    }
}

/// `lin_srgb_to_oklab`: <https://bottosson.github.io/posts/oklab/#converting-from-linear-srgb-to-oklab>.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L683-L696 (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // Skia's constants verbatim
fn lin_srgb_to_oklab(rgb: PMColor4f, _hue_is_powerless: &mut bool) -> PMColor4f {
    let mut l = 0.4122214708f32 * rgb.r + 0.5363325363f32 * rgb.g + 0.0514459929f32 * rgb.b;
    let mut m = 0.2119034982f32 * rgb.r + 0.6806995451f32 * rgb.g + 0.1073969566f32 * rgb.b;
    let mut s = 0.0883024619f32 * rgb.r + 0.2817188376f32 * rgb.g + 0.6299787005f32 * rgb.b;
    // skia-rust: libm (`cbrtf`)
    l = l.cbrt();
    m = m.cbrt();
    s = s.cbrt();
    PMColor4f {
        r: 0.2104542553f32 * l + 0.7936177850f32 * m - 0.0040720468f32 * s,
        g: 1.9779984951f32 * l - 2.4285922050f32 * m + 0.4505937099f32 * s,
        b: 0.0259040371f32 * l + 0.7827717662f32 * m - 0.8086757660f32 * s,
        a: rgb.a,
    }
}

/// The color space is technically `OkLCH`, but we produce HCL, so that all polar spaces have hue in
/// the first component. This simplifies the hue handling for `HueMethod` and premul/unpremul
/// (`lin_srgb_to_okhcl`).
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L698-L712 (chrome/m156)
#[allow(clippy::items_after_statements)] // keeps Skia's local constant where it is declared
fn lin_srgb_to_okhcl(rgb: PMColor4f, hue_is_powerless: &mut bool) -> PMColor4f {
    let ok_lab = lin_srgb_to_oklab(rgb, hue_is_powerless);
    // skia-rust: libm (`atan2f`)
    let hue = float_radians_to_degrees(ok_lab.b.atan2(ok_lab.g));
    let chroma = (ok_lab.g * ok_lab.g + ok_lab.b * ok_lab.b).sqrt();
    // The OKLCH math produces very small chroma values for achromatic colors:
    const MAX_CHROMA_FOR_POWERLESS_HUE: f32 = 1e-6;
    if chroma <= MAX_CHROMA_FOR_POWERLESS_HUE {
        *hue_is_powerless = true;
    }
    PMColor4f {
        r: if hue >= 0.0 { hue } else { hue + 360.0 },
        g: chroma,
        b: ok_lab.r,
        a: rgb.a,
    }
}

/// `premul_polar`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L714-L716 (chrome/m156)
fn premul_polar(hsl: PMColor4f) -> PMColor4f {
    PMColor4f {
        r: hsl.r,
        g: hsl.g * hsl.a,
        b: hsl.b * hsl.a,
        a: hsl.a,
    }
}

/// `premul_rgb`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L718-L720 (chrome/m156)
fn premul_rgb(rgb: PMColor4f) -> PMColor4f {
    PMColor4f {
        r: rgb.r * rgb.a,
        g: rgb.g * rgb.a,
        b: rgb.b * rgb.a,
        a: rgb.a,
    }
}

/// `color_space_is_polar`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L722-L732 (chrome/m156)
fn color_space_is_polar(cs: InterpColorSpace) -> bool {
    matches!(
        cs,
        InterpColorSpace::LCH
            | InterpColorSpace::OKLCH
            | InterpColorSpace::HSL
            | InterpColorSpace::HWB
    )
}

/// The colors of a gradient transformed for the destination (`SkColor4fXformer`): in the
/// intermediate color space and the interpolation color space, hue-adjusted and premultiplied
/// as the interpolation asks.
// Port of: src/shaders/gradients/SkGradientBaseShader.h#L180-L193 (chrome/m156)
#[doc(alias = "SkColor4fXformer")]
#[derive(Clone, Debug)]
pub struct Color4fXformer {
    /// `fColors`.
    pub colors: Vec<PMColor4f>,
    /// `fPositions`: the shader's positions, or the new ones if colors were added.
    pub positions: Option<Vec<f32>>,
    /// `fIntermediateColorSpace`.
    pub intermediate_color_space: Option<ColorSpace>,
}

impl Color4fXformer {
    /// Given `colors` in `src` color space, an interpolation space, and a `dst` color space,
    /// we are doing several things. First, some definitions:
    ///
    /// The interpolation color space is "special" if it can't be represented as an
    /// `SkColorSpace`. This applies to any color space that isn't an RGB space, like Lab or
    /// HSL. These need special handling because we have to run bespoke code to do the
    /// conversion (before interpolation here, and after interpolation in the backend
    /// shader/pipeline).
    ///
    /// The interpolation color space is "polar" if it involves hue (HSL, HWB, LCH, Oklch).
    /// These need special handling, becuase hue is never premultiplied, and because `HueMethod`
    /// comes into play.
    ///
    /// 1) Pick an `intermediate` `SkColorSpace`. If the interpolation color space is not
    ///    "special", (`kDestination`, `kSRGB`, etc... ), then `intermediate` is exact. Otherwise,
    ///    `intermediate` is the RGB space that prepares us to do the final conversion. For
    ///    example, conversion to Lab starts with XYZD50, so `intermediate` will be XYZD50 if
    ///    we're actually interpolating in Lab.
    /// 2) Transform all colors to the `intermediate` color space, leaving them unpremultiplied.
    /// 3) If the interpolation color space is "special", transform the colors to that space.
    /// 4) If the interpolation color space is "polar", adjust the angles to respect `HueMethod`.
    /// 5) If premul interpolation is requested, apply that. For "polar" interpolated colors,
    ///    don't premultiply hue, only the other two channels. Note that there are four polar
    ///    spaces. Two have hue as the first component, and two have it as the third component.
    ///    To reduce complexity, we always store hue in the first component, swapping it with
    ///    luminance for LCH and Oklch. The backend code (eg, shaders) needs to know about this.
    ///
    /// # Panics
    ///
    /// Panics if the color count does not fit an `i32` or the color conversion fails.
    // Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L734-L962 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_lines)] // one constructor in Skia
    pub fn new(shader: &GradientBaseShader, dst: Option<&ColorSpace>) -> Color4fXformer {
        use InterpColorSpace as CS;

        let mut color_count = shader.colors().len();
        let interpolation = shader.interpolation();

        // 0) Copy the shader's position pointer. Certain interpolation modes might force us to
        //    add new stops, in which case we'll allocate & edit the positions.
        let mut positions: Option<Vec<f32>> = shader.positions().map(<[f32]>::to_vec);

        // 1) Determine the color space of our intermediate colors.
        let intermediate = intermediate_color_space(interpolation.color_space, dst);

        // 2) Convert all colors to the intermediate color space
        let info = ImageInfo::new(
            (
                i32::try_from(color_count).expect("gradient color count fits an int"),
                1,
            ),
            ColorType::RGBAF32,
            AlphaType::Unpremul,
            None,
        );
        let dst_info = info.with_color_space(intermediate.clone());
        let src_info = info.with_color_space(shader.color_space().clone());

        let row_bytes = info.min_row_bytes();
        let mut src_bytes = Vec::with_capacity(row_bytes);
        for c in shader.colors() {
            for v in [c.r, c.g, c.b, c.a] {
                src_bytes.extend_from_slice(&v.to_ne_bytes());
            }
        }
        let mut dst_bytes = vec![0u8; row_bytes];
        let converted = convert_pixels(
            &dst_info,
            &mut dst_bytes,
            row_bytes,
            &src_info,
            &src_bytes,
            row_bytes,
        );
        assert!(converted, "SkConvertPixels of gradient colors");
        let mut colors: Vec<PMColor4f> = dst_bytes
            .as_chunks::<16>()
            .0
            .iter()
            .map(|px| {
                let f = |i: usize| f32::from_ne_bytes([px[i], px[i + 1], px[i + 2], px[i + 3]]);
                PMColor4f {
                    r: f(0),
                    g: f(4),
                    b: f(8),
                    a: f(12),
                }
            })
            .collect();

        // 3) Transform to the interpolation color space (if it's special)
        let convert_fn: Option<ConvertColorProc> = match interpolation.color_space {
            CS::HSL => Some(srgb_to_hsl),
            CS::HWB => Some(srgb_to_hwb),
            CS::Lab => Some(xyzd50_to_lab),
            CS::LCH => Some(xyzd50_to_hcl),
            CS::OKLab | CS::OKLabGamutMap => Some(lin_srgb_to_oklab),
            CS::OKLCH | CS::OKLCHGamutMap => Some(lin_srgb_to_okhcl),
            _ => None,
        };

        let mut hue_is_powerless = vec![false; color_count];
        let mut any_powerless_hue = false;
        if let Some(convert_fn) = convert_fn {
            for i in 0..color_count {
                colors[i] = convert_fn(colors[i], &mut hue_is_powerless[i]);
                any_powerless_hue = any_powerless_hue || hue_is_powerless[i];
            }
        }

        if any_powerless_hue {
            // In theory, if we knew we were just going to adjust the existing colors (without
            // adding new ones), we could do it all in-place. To keep things simple, we always
            // generate the new colors in separate storage.
            let mut new_colors: Vec<PMColor4f> = Vec::new();
            let mut new_positions: Vec<f32> = Vec::new();

            for i in 0..color_count {
                let cur_color = colors[i];
                let cur_pos = shader.get_pos(i);

                if !hue_is_powerless[i] {
                    new_colors.push(cur_color);
                    new_positions.push(cur_pos);
                    continue;
                }

                let color_with_hue_from = |color: PMColor4f, hue_color: PMColor4f| {
                    // If we have any powerless hue, then all colors are already in (some) polar
                    // space, and they all store their hue in the red channel.
                    PMColor4f {
                        r: hue_color.r,
                        g: color.g,
                        b: color.b,
                        a: color.a,
                    }
                };

                // In each case, we might be copying a powerless (invalid) hue from the
                // neighbor, but that should be fine, as it will match that neighbor perfectly,
                // and any hue is ok.
                if i != 0 {
                    new_positions.push(cur_pos);
                    new_colors.push(color_with_hue_from(cur_color, colors[i - 1]));
                }
                if i != color_count - 1 {
                    new_positions.push(cur_pos);
                    new_colors.push(color_with_hue_from(cur_color, colors[i + 1]));
                }
            }

            colors = new_colors;
            positions = Some(new_positions);
            color_count = colors.len();
        }

        // 4) For polar colors, adjust hue values to respect the hue method. We're using a
        //    trick here... The specification looks at adjacent colors, and adjusts one or the
        //    other. Because we store the stops in uniforms (and our backend conversions
        //    normalize the hue angle), we can instead always apply the adjustment to the
        //    *second* color. That lets us keep a running total, and do a single pass across
        //    all the colors to respect the requested hue method, without needing to do any
        //    extra work per-pixel.
        if color_space_is_polar(interpolation.color_space) {
            let mut delta = 0.0f32;
            for i in 0..color_count.saturating_sub(1) {
                let h1 = colors[i].r;
                let mut h2 = colors[i + 1].r;
                h2 += delta;
                match interpolation.hue_method {
                    HueMethod::Shorter => {
                        if h2 - h1 > 180.0 {
                            h2 -= 360.0; // i.e. h1 += 360
                            delta -= 360.0;
                        } else if h2 - h1 < -180.0 {
                            h2 += 360.0;
                            delta += 360.0;
                        }
                    }
                    HueMethod::Longer => {
                        if (i == 0 && shader.first_stop_is_implicit())
                            || (i == color_count - 2 && shader.last_stop_is_implicit())
                        {
                            // Do nothing. We don't want to introduce a full revolution for
                            // these stops. Full rationale at skbug.com/40044215
                        } else if 0.0 < h2 - h1 && h2 - h1 < 180.0 {
                            h2 -= 360.0; // i.e. h1 += 360
                            delta -= 360.0;
                        } else if -180.0 < h2 - h1 && h2 - h1 <= 0.0 {
                            h2 += 360.0;
                            delta += 360.0;
                        }
                    }
                    HueMethod::Increasing => {
                        if h2 < h1 {
                            h2 += 360.0;
                            delta += 360.0;
                        }
                    }
                    HueMethod::Decreasing => {
                        if h1 < h2 {
                            h2 -= 360.0; // i.e. h1 += 360;
                            delta -= 360.0;
                        }
                    }
                }
                colors[i + 1].r = h2;
            }
        }

        // 5) Apply premultiplication
        let premul_fn: Option<PremulColorProc> = if interpolation.in_premul == InPremul::Yes {
            match interpolation.color_space {
                CS::HSL | CS::HWB | CS::LCH | CS::OKLCH => Some(premul_polar),
                _ => Some(premul_rgb),
            }
        } else {
            None
        };

        if let Some(premul_fn) = premul_fn {
            for c in &mut colors {
                *c = premul_fn(*c);
            }
        }

        Color4fXformer {
            colors,
            positions,
            intermediate_color_space: intermediate,
        }
    }
}

/// `average_gradient_color`.
// Port of: src/shaders/gradients/SkGradientBaseShader.cpp#L1038-L1098 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors 1.f / (colors.size() - 1)
fn average_gradient_color(colors: &[Color4f], pos: &[f32]) -> Color4f {
    // The gradient is a piecewise linear interpolation between colors. For a given interval,
    // the integral between the two endpoints is 0.5 * (ci + cj) * (pj - pi), which provides
    // that intervals average color. The overall average color is thus the sum of each piece.
    // The thing to keep in mind is that the provided gradient definition may implicitly use
    // p=0 and p=1.
    let mut blend: Float4 = [0.0; 4];
    let load = |c: &Color4f| -> Float4 { [c.r, c.g, c.b, c.a] };
    for i in 0..colors.len() - 1 {
        // Calculate the average color for the interval between pos(i) and pos(i+1)
        let c0 = load(&colors[i]);
        let c1 = load(&colors[i + 1]);

        // when pos == null, there are colorCount uniformly distributed stops, going from 0 to
        // 1, so pos[i + 1] - pos[i] = 1/(colorCount-1)
        let w: f32;
        if pos.is_empty() {
            w = 1.0f32 / ((colors.len() - 1) as f32);
        } else {
            // Match position fixing in SkGradientShader's constructor, clamping positions
            // outside [0, 1] and forcing the sequence to be monotonic
            let p0 = t_pin(pos[i], 0.0f32, 1.0f32);
            let p1 = t_pin(pos[i + 1], p0, 1.0f32);
            w = p1 - p0;

            // And account for any implicit intervals at the start or end of the positions
            if i == 0 && p0 > 0.0 {
                // The first color is fixed between p = 0 to pos[0], so 0.5*(ci + cj)*(pj - pi)
                // becomes 0.5*(c + c)*(pj - 0) = c * pj
                let c = load(&colors[0]);
                for k in 0..4 {
                    blend[k] += p0 * c[k];
                }
            }
            if i == colors.len() - 2 && p1 < 1.0 {
                // The last color is fixed between pos[n-1] to p = 1, so
                // 0.5*(ci + cj)*(pj - pi) becomes 0.5*(c + c)*(1 - pi) = c * (1 - pi)
                let c = load(&colors[colors.len() - 1]);
                for k in 0..4 {
                    blend[k] += (1.0f32 - p1) * c[k];
                }
            }
        }

        let half_w = 0.5f32 * w;
        for k in 0..4 {
            blend[k] += half_w * (c1[k] + c0[k]);
        }
    }

    Color4f::new(blend[0], blend[1], blend[2], blend[3])
}

impl Gradient<'_> {
    /// `GRADIENT_FACTORY_EARLY_EXIT`: the checks and fallbacks every gradient factory makes
    /// first. `Err(result)` is what the factory returns right away (a null shader, or the
    /// single color of a one-color gradient).
    // Port of: src/shaders/gradients/SkGradientBaseShader.h#L190-L202 (chrome/m156)
    pub(crate) fn factory_early_exit(
        &self,
        local_matrix: Option<&Matrix>,
    ) -> Result<(), Option<Shader>> {
        let colors = self.colors();
        let interp = self.interpolation();
        if !GradientBaseShader::valid_gradient(colors.colors(), colors.tile_mode(), interp) {
            return Err(None);
        }
        if colors.colors().len() == 1 {
            return Err(shaders::color_in_space(
                colors.colors()[0],
                colors.color_space().cloned(),
            ));
        }
        if local_matrix.is_some_and(|lm| lm.invert().is_none()) {
            return Err(None);
        }
        Ok(())
    }
}

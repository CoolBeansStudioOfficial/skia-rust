// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkBlendImageFilter.cpp (the blender and the
// arithmetic variants)

//! `SkBlendImageFilter`: blends its background and foreground inputs with a blender.

#![allow(clippy::collapsible_if, clippy::similar_names)] // Keeps the C++ control flow and the foreground/background names.
use skia_rust_core::blend_mode::{BlendMode, BlendModeCoeff};
use skia_rust_core::blender::Blender;
use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::image_filter::{
    ImageFilter, ImageFilterBase, ImageFilterCommon, flatten_common, unflatten_common,
};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{
    Context, Mapping, MatrixCapability, irect_intersect_in_place,
};
use skia_rust_core::picture_priv;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::{IRect, Rect, rect_priv};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::blenders;
use crate::image_filters::crop_filter::{crop, empty};

/// Index of the background input (`kBackground`).
const BACKGROUND: usize = 0;
/// Index of the foreground input (`kForeground`).
const FOREGROUND: usize = 1;

/// The blend image filter (`SkBlendImageFilter`).
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L17-L51 (chrome/m156)
#[doc(alias = "SkBlendImageFilter")]
#[derive(Debug)]
pub struct BlendImageFilter {
    common: ImageFilterCommon,
    blender: Blender,
    /// The `k1..k4` of the arithmetic variant (`fArithmeticCoefficients`). Normally runtime
    /// blenders are pessimistic about the bounds they affect; the coefficients let the bounds be
    /// reasoned about.
    arithmetic_coefficients: Option<[f32; 4]>,
    /// Remembered to serialize the arithmetic variant (`fEnforcePremul`).
    enforce_premul: bool,
}

impl BlendImageFilter {
    /// The blend mode, if the blender is one (`asBlendMode`).
    fn blend_mode(&self) -> Option<BlendMode> {
        self.blender.as_base().as_blend_mode()
    }

    /// `makeBlendShader(bg, fg)`: the blended shader, with the transparent black filling a missing
    /// input unless the blend can skip it.
    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L186-L211 (chrome/m156)
    fn make_blend_shader(&self, bg: Option<Shader>, fg: Option<Shader>) -> Option<Shader> {
        let (mut bg, mut fg) = (bg, fg);
        if bg.is_none() || fg.is_none() {
            if !self.on_affects_transparent_black() && bg.is_none() && fg.is_none() {
                return None;
            }
            if let Some(bm) = self.blend_mode() {
                if let Some((src, dst)) = bm.as_coeff() {
                    if bg.is_some()
                        && matches!(
                            dst,
                            BlendModeCoeff::One | BlendModeCoeff::ISA | BlendModeCoeff::ISC
                        )
                    {
                        return bg;
                    }
                    if fg.is_some() && matches!(src, BlendModeCoeff::One | BlendModeCoeff::IDA) {
                        return fg;
                    }
                }
            }
            if bg.is_none() {
                bg = Some(shaders::color(skia_rust_core::color::Color::new(0)));
            }
            if fg.is_none() {
                fg = Some(shaders::color(skia_rust_core::color::Color::new(0)));
            }
        }
        shaders::blend_blender(&self.blender, bg?, fg?)
    }

    /// The `(transparentOutsideFG, transparentOutsideBG)` of the blend, or `None` when the output
    /// is unbounded: a blend mode (`SkBlendMode_AsCoeff`), or the arithmetic coefficients.
    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L271-L300 (chrome/m156)
    fn transparent_outside(&self) -> Option<(bool, bool)> {
        if let Some(bm) = self.blend_mode() {
            // NOTE: advanced blends use src-over for their alpha channel, which should produce the
            // union of FG and BG. That is the outcome if both flags stay false.
            return Some(match bm.as_coeff() {
                Some((src, dst)) => (
                    matches!(
                        dst,
                        BlendModeCoeff::Zero | BlendModeCoeff::SA | BlendModeCoeff::SC
                    ),
                    matches!(src, BlendModeCoeff::Zero | BlendModeCoeff::DA),
                ),
                None => (false, false),
            });
        }
        // A non-arithmetic runtime blender is pessimistically unbounded.
        let k = self.arithmetic_coefficients?;
        // The arithmetic equation produces non-transparent black everywhere.
        if k[3] != 0.0 {
            return None;
        }
        // Given k[1] == k[2] == 0 implies k[0] != 0, if only one of k[1] or k[2] is non-zero then
        // only that bounds has non-transparent content.
        Some((k[2] == 0.0, k[1] == 0.0))
    }

    /// The `(transparentOutsideFG, transparentOutsideBG)` used by `computeFastBounds`. Unlike
    /// [`Self::transparent_outside`], it does not count `SC` as `SA` for the foreground.
    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L319-L352 (chrome/m156)
    fn fast_bounds_transparent_outside(&self) -> Option<(bool, bool)> {
        if let Some(bm) = self.blend_mode() {
            return Some(match bm.as_coeff() {
                Some((src, dst)) => (
                    matches!(dst, BlendModeCoeff::Zero | BlendModeCoeff::SA),
                    matches!(src, BlendModeCoeff::Zero | BlendModeCoeff::DA),
                ),
                None => (false, false),
            });
        }
        self.transparent_outside()
    }
}

impl ImageFilterBase for BlendImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L17-L51 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkBlendImageFilter"
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L282-L296 (chrome/m156), flatten
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        flatten_common(&self.common, buffer);
        if let Some(k) = self.arithmetic_coefficients {
            buffer.write_uint(ARITHMETIC_BLEND_MODE);
            buffer.write_scalar(k[0]);
            buffer.write_scalar(k[1]);
            buffer.write_scalar(k[2]);
            buffer.write_scalar(k[3]);
            buffer.write_bool(self.enforce_premul);
        } else if let Some(bm) = self.blend_mode() {
            buffer.write_uint(bm as u32);
        } else {
            buffer.write_uint(CUSTOM_BLEND_MODE);
            buffer.write_blender(Some(&self.blender));
        }
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L36 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L36-L41 (chrome/m156)
    fn on_affects_transparent_black(&self) -> bool {
        // An arbitrary runtime blender, or an arithmetic runtime blender with k3 != 0, affects
        // transparent black.
        #[allow(clippy::float_cmp)] // mirrors the C++ `[3] != 0.f` test
        let arithmetic_affects = self.arithmetic_coefficients.is_none_or(|k| k[3] != 0.0);
        self.blend_mode().is_none() && arithmetic_affects
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L223-L247 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let mut required_input =
            self.on_get_output_layer_bounds(ctx.mapping(), Some(ctx.source().layer_bounds()));
        match required_input.as_mut() {
            Some(required) => {
                if !irect_intersect_in_place(required, &ctx.desired_output()) {
                    return FilterResult::default();
                }
            }
            None => required_input = Some(ctx.desired_output()),
        }
        let required_input = required_input.expect("set above");
        let input_ctx = ctx.with_new_desired_output(required_input);
        let mut builder = Builder::new(ctx);
        builder.add(
            self.get_child_output(BACKGROUND, &input_ctx),
            None,
            ShaderFlags::NONE,
            default_sampling(),
        );
        builder.add(
            self.get_child_output(FOREGROUND, &input_ctx),
            None,
            ShaderFlags::NONE,
            default_sampling(),
        );
        builder.eval(
            |inputs| self.make_blend_shader(inputs[BACKGROUND].clone(), inputs[FOREGROUND].clone()),
            Some(required_input),
            false,
        )
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L249-L269 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        let max_output = match content_bounds {
            Some(content) => self.on_get_output_layer_bounds(mapping, Some(content)),
            None => None,
        };
        let required_input = match (content_bounds, max_output) {
            (Some(_), Some(mut max)) => {
                if !irect_intersect_in_place(&mut max, &desired_output) {
                    return IRect::new_empty();
                }
                max
            }
            _ => desired_output,
        };
        let bg_input =
            self.get_child_input_layer_bounds(BACKGROUND, mapping, required_input, content_bounds);
        let fg_input =
            self.get_child_input_layer_bounds(FOREGROUND, mapping, required_input, content_bounds);
        IRect::join(&bg_input, &fg_input)
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L271-L317 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        let (transparent_outside_fg, transparent_outside_bg) = self.transparent_outside()?;
        let mut foreground_bounds =
            self.get_child_output_layer_bounds(FOREGROUND, mapping, content_bounds);
        let mut background_bounds =
            self.get_child_output_layer_bounds(BACKGROUND, mapping, content_bounds);
        if transparent_outside_fg {
            if transparent_outside_bg {
                if foreground_bounds.is_none() && background_bounds.is_some() {
                    foreground_bounds = background_bounds;
                } else if let (Some(fg), Some(bg)) = (foreground_bounds.as_mut(), background_bounds)
                {
                    if !irect_intersect_in_place(fg, &bg) {
                        return Some(IRect::new_empty());
                    }
                }
            }
            foreground_bounds
        } else {
            if !transparent_outside_bg {
                background_bounds = match (background_bounds, foreground_bounds) {
                    (Some(bg), Some(fg)) => Some(IRect::join(&bg, &fg)),
                    _ => None,
                };
            }
            background_bounds
        }
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L319-L353 (chrome/m156)
    fn compute_fast_bounds(&self, bounds: &Rect) -> Rect {
        let Some((transparent_outside_fg, transparent_outside_bg)) =
            self.fast_bounds_transparent_outside()
        else {
            return rect_priv::make_large_s32();
        };
        let mut foreground_bounds = match self.get_input(FOREGROUND) {
            Some(input) => input.compute_fast_bounds(bounds),
            None => *bounds,
        };
        let mut background_bounds = match self.get_input(BACKGROUND) {
            Some(input) => input.compute_fast_bounds(bounds),
            None => *bounds,
        };
        if transparent_outside_fg {
            if transparent_outside_bg && !foreground_bounds.intersect(background_bounds) {
                return Rect::new_empty();
            }
            foreground_bounds
        } else {
            if !transparent_outside_bg {
                background_bounds.join(foreground_bounds);
            }
            background_bounds
        }
    }
}

/// `make_blend(blender, background, foreground, cropRect)`: the blend filter, or the input it
/// reduces to for the `Src`, `Dst` and `Clear` modes.
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L54-L80 (chrome/m156)
fn make_blend(
    blender: Option<Blender>,
    background: Option<ImageFilter>,
    foreground: Option<ImageFilter>,
    crop_rect: Option<Rect>,
    coefficients: Option<[f32; 4]>,
    enforce_premul: bool,
) -> Option<ImageFilter> {
    let blender = blender.unwrap_or_else(|| Blender::mode(BlendMode::SrcOver));
    let cropped = |filter: Option<ImageFilter>| match crop_rect {
        Some(rect) => crop(&rect, skia_rust_core::tile_mode::TileMode::Decal, filter),
        None => filter,
    };
    if let Some(bm) = blender.as_base().as_blend_mode() {
        if bm == BlendMode::Src {
            return cropped(foreground);
        } else if bm == BlendMode::Dst {
            return cropped(background);
        } else if bm == BlendMode::Clear {
            return Some(empty());
        }
    }
    let filter = ImageFilter::from_base(BlendImageFilter {
        common: ImageFilterCommon::new(vec![background, foreground], None),
        blender,
        arithmetic_coefficients: coefficients,
        enforce_premul,
    });
    cropped(Some(filter))
}

/// `SkImageFilters::Blend(mode, background, foreground, cropRect)`.
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L434-L440 (chrome/m156)
#[doc(alias = "Blend")]
#[must_use]
pub fn blend(
    mode: BlendMode,
    background: Option<ImageFilter>,
    foreground: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_blend(
        Some(Blender::mode(mode)),
        background,
        foreground,
        crop_rect,
        None,
        false,
    )
}

/// `SkImageFilters::Blend(blender, background, foreground, cropRect)`.
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L442-L448 (chrome/m156)
#[doc(alias = "Blend")]
#[must_use]
pub fn blend_with_blender(
    blender: Blender,
    background: Option<ImageFilter>,
    foreground: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_blend(
        Some(blender),
        background,
        foreground,
        crop_rect,
        None,
        false,
    )
}

/// `SkImageFilters::Arithmetic(k1, k2, k3, k4, enforcePMColor, background, foreground, cropRect)`:
/// the blend `k1*FG*BG + k2*FG + k3*BG + k4`, clamped to `[0, 1]`.
///
/// Returns `None` if the arithmetic blender cannot be made.
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L152-L172 (chrome/m156)
#[doc(alias = "Arithmetic")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors SkImageFilters::Arithmetic's signature
pub fn arithmetic(
    k1: f32,
    k2: f32,
    k3: f32,
    k4: f32,
    enforce_pm_color: bool,
    background: Option<ImageFilter>,
    foreground: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    let blender = blenders::arithmetic(k1, k2, k3, k4, enforce_pm_color)?;
    make_blend(
        Some(blender),
        background,
        foreground,
        crop_rect,
        Some([k1, k2, k3, k4]),
        enforce_pm_color,
    )
}

/// `kCustom_SkBlendMode`: a blender that is not a blend mode, written with its body.
// Port of: src/core/SkBlendModePriv.h#L24 (chrome/m156)
const CUSTOM_BLEND_MODE: u32 = 0xFF;

/// `kArithmetic_SkBlendMode`: the arithmetic blender, written with its coefficients.
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L64 (chrome/m156)
const ARITHMETIC_BLEND_MODE: u32 = CUSTOM_BLEND_MODE + 1;

/// `SkBlendImageFilter::CreateProc`.
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L202-L234 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    registry: &FlattenableRegistry,
) -> Option<ImageFilter> {
    let (inputs, crop_rect) = unflatten_common(buffer, 2, registry)?;

    let mut blender: Option<Blender> = None;
    let mut coefficients: Option<[f32; 4]> = None;
    let mut enforce_premul = false;

    // The mode is read as a `uint32_t`.
    let mode = buffer.read32().cast_unsigned();
    if mode == ARITHMETIC_BLEND_MODE {
        // Should only see this sentinel value in newer SKPs
        let version_ok =
            !buffer.is_version_lt(picture_priv::VERSION_COMBINE_BLEND_ARITHMETIC_FILTERS);
        if buffer.validate(version_ok) {
            let k = [
                buffer.read_scalar(),
                buffer.read_scalar(),
                buffer.read_scalar(),
                buffer.read_scalar(),
            ];
            coefficients = Some(k);
            enforce_premul = buffer.read_bool();
            let Some(arithmetic_blender) =
                blenders::arithmetic(k[0], k[1], k[2], k[3], enforce_premul)
            else {
                // A null arithmetic blender is an error condition.
                buffer.validate(false);
                return None;
            };
            blender = Some(arithmetic_blender);
        }
    } else if mode == CUSTOM_BLEND_MODE {
        blender = buffer.read_blender(registry);
    } else {
        let bm = i32::try_from(mode).ok().and_then(BlendMode::from_i32);
        if !buffer.validate(bm.is_some()) {
            return None;
        }
        blender = bm.map(Blender::mode);
    }

    let mut inputs = inputs.into_iter();
    let background = inputs.next().flatten();
    let foreground = inputs.next().flatten();
    make_blend(
        blender,
        background,
        foreground,
        crop_rect,
        coefficients,
        enforce_premul,
    )
}

/// `SkBlendImageFilter::LegacyArithmeticCreateProc`: the arithmetic filter of the old name, which
/// newer pictures do not write.
// Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L174-L194 (chrome/m156)
pub fn legacy_arithmetic_create_proc(
    buffer: &mut ReadBuffer<'_>,
    registry: &FlattenableRegistry,
) -> Option<ImageFilter> {
    // Newer SKPs should be using the updated Blend CreateProc.
    let old_version = buffer.is_version_lt(picture_priv::VERSION_COMBINE_BLEND_ARITHMETIC_FILTERS);
    if !buffer.validate(old_version) {
        // SkASSERT(false): debug-only, so release will just see a failed deserialization.
        debug_assert!(false, "a legacy arithmetic filter in a newer picture");
        return None;
    }

    let (inputs, crop_rect) = unflatten_common(buffer, 2, registry)?;
    let k = [
        buffer.read_scalar(),
        buffer.read_scalar(),
        buffer.read_scalar(),
        buffer.read_scalar(),
    ];
    let enforce_premul = buffer.read_bool();
    let mut inputs = inputs.into_iter();
    let background = inputs.next().flatten();
    let foreground = inputs.next().flatten();
    arithmetic(
        k[0],
        k[1],
        k[2],
        k[3],
        enforce_premul,
        background,
        foreground,
        crop_rect,
    )
}

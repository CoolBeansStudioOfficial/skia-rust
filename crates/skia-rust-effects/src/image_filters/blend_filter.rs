// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkBlendImageFilter.cpp (the blender variant; the
// arithmetic variant needs `SkBlenders::Arithmetic`, which is not ported yet)

//! `SkBlendImageFilter`: blends its background and foreground inputs with a blender.

#![allow(clippy::collapsible_if, clippy::similar_names)] // Keeps the C++ control flow and the foreground/background names.
use skia_rust_core::blend_mode::{BlendMode, BlendModeCoeff};
use skia_rust_core::blender::Blender;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{
    Context, Mapping, MatrixCapability, irect_intersect_in_place,
};
use skia_rust_core::rect::{IRect, Rect, rect_priv};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;

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

    /// The `(transparentOutsideFG, transparentOutsideBG)` of a blend mode (`SkBlendMode_AsCoeff`).
    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L244-L252 (chrome/m156)
    fn transparent_outside(&self) -> Option<(bool, bool)> {
        self.blend_mode().map(|bm| match bm.as_coeff() {
            Some((src, dst)) => (
                matches!(
                    dst,
                    BlendModeCoeff::Zero | BlendModeCoeff::SA | BlendModeCoeff::SC
                ),
                matches!(src, BlendModeCoeff::Zero | BlendModeCoeff::DA),
            ),
            None => (false, false),
        })
    }
}

impl ImageFilterBase for BlendImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L36 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkBlendImageFilter.cpp#L37-L41 (chrome/m156)
    fn on_affects_transparent_black(&self) -> bool {
        self.blend_mode().is_none()
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
        let Some((transparent_outside_fg, transparent_outside_bg)) = self.transparent_outside()
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
    make_blend(Some(Blender::mode(mode)), background, foreground, crop_rect)
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
    make_blend(Some(blender), background, foreground, crop_rect)
}

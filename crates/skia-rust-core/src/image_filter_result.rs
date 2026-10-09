// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkImageFilterTypes.h (`FilterResult`, `FilterResult::Builder`),
// src/core/SkImageFilterTypes.cpp
//
// skia-rust: `FilterResult::Builder::eval` takes the shader function as a closure, as the C++
// template does.

#![allow(
    // The float casts and exact float comparisons mirror the C++ arithmetic of the Skia source
    // (`SkScalar` and `int` conversions, `==` on scalars); the control flow keeps the C++ shape
    // so the port can be reviewed line by line.
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::manual_let_else
)]

use std::sync::Arc;

use crate::blend_mode::BlendMode;
use crate::blender::Blender;
use crate::canvas::{Canvas, SrcRectConstraint};
use crate::clip_op::ClipOp;
use crate::color::Color4f;
use crate::color_filter::ColorFilter;
use crate::color_filters;
use crate::color_space::ColorSpace;
use crate::device::Device;
use crate::floating_point::{float_ceil2int, ieee_float_divide};
use crate::image::Image;
use crate::image_filter_types::{
    Context, ROUND_EPSILON, decompose_transform, inverse_map_irect, inverse_map_rect_f,
    irect_intersect_in_place, map_irect, map_size, relevant_subset, round_out, size_ceil,
};
use crate::known_runtime_effects::{StableKey, get_known_runtime_effect};
use crate::m44::M44;
use crate::math_priv::next_log2;
use crate::matrix::Matrix;
use crate::matrix::ScaleToFit;
use crate::paint::Paint;
use crate::picture::Picture;
use crate::point::{IPoint, Point, Vector};
use crate::rect::{Contains, IRect, Rect, rect_priv};
use crate::runtime_effect::RuntimeEffectBuilder;
use crate::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use crate::shader::Shader;
use crate::size::ISize;
use crate::size::Size;
use crate::special_image::SpecialImage;
use crate::surface_props::{PixelGeometry, SurfaceProps};
use crate::tile_mode::TileMode;

/// `kDefaultSampling`: bilinear, no mipmaps (`SkSamplingOptions{SkFilterMode::kLinear}`).
// Port of: src/core/SkImageFilterTypes.h#L802 (chrome/m156)
#[must_use]
pub fn default_sampling() -> SamplingOptions {
    SamplingOptions::new(FilterMode::Linear, MipmapMode::None)
}

/// `FilterResult::PixelBoundary`: what the pixels around the image subset are.
// Port of: src/core/SkImageFilterTypes.h#L689-L697 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PixelBoundary {
    /// Pixels outside the image subset are of unknown value, possibly uninitialized.
    Unknown,
    /// Pixels bordering the image subset are transparent black.
    Transparent,
    /// Pixels bordering the image are known to be initialized.
    Initialized,
}

bitflags::bitflags! {
    /// `FilterResult::ShaderFlags`.
    // Port of: src/core/SkImageFilterTypes.h#L727-L731 (chrome/m156)
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct ShaderFlags: u32 {
        /// `kNone`.
        const NONE = 0;
        /// `kSampledRepeatedly`.
        const SAMPLED_REPEATEDLY = 1 << 0;
        /// `kNonTrivialSampling`.
        const NON_TRIVIAL_SAMPLING = 1 << 1;
    }
}

bitflags::bitflags! {
    /// `FilterResult::BoundsAnalysis`.
    // Port of: src/core/SkImageFilterTypes.h#L894-L906 (chrome/m156)
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct BoundsAnalysis: u32 {
        /// `kSimple`.
        const SIMPLE = 0;
        /// `kDstBoundsNotCovered`.
        const DST_BOUNDS_NOT_COVERED = 1 << 0;
        /// `kHasLayerFillingEffect`.
        const HAS_LAYER_FILLING_EFFECT = 1 << 1;
        /// `kRequiresLayerCrop`.
        const REQUIRES_LAYER_CROP = 1 << 2;
        /// `kRequiresShaderTiling`.
        const REQUIRES_SHADER_TILING = 1 << 3;
        /// `kRequiresDecalInLayerSpace`.
        const REQUIRES_DECAL_IN_LAYER_SPACE = 1 << 4;
    }
}

/// `FilterResult::BoundsScope`: how the bounds analysis will be used.
// Port of: src/core/SkImageFilterTypes.h#L908-L917 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BoundsScope {
    /// The bounds analysis won't be used for any rendering yet.
    Deferred,
    /// The rendering may draw the image directly if analysis allows it.
    CanDrawDirectly,
    /// The rendering will always use a filling shader, e.g. `drawPaint()`.
    ShaderOnly,
    /// The rendering is controlled by rescaling logic, so ignores decal size.
    Rescale,
}

/// The result of a filter: an image, where its origin is, and how to draw it (`skif::FilterResult`).
// Port of: src/core/SkImageFilterTypes.h#L689-L847 (chrome/m156)
#[doc(alias = "skif::FilterResult")]
#[derive(Clone, Debug)]
pub struct FilterResult {
    image: Option<Arc<SpecialImage>>,
    boundary: PixelBoundary,
    sampling: SamplingOptions,
    tile_mode: TileMode,
    /// `LayerSpace<SkMatrix>`: the transform from the image's pixels to layer space.
    transform: Matrix,
    color_filter: Option<ColorFilter>,
    /// `LayerSpace<SkIRect>`: the layer-space bounds the result covers.
    layer_bounds: IRect,
}

impl Default for FilterResult {
    /// `FilterResult()`: an empty result with no image.
    fn default() -> FilterResult {
        FilterResult::with_boundary(None, IPoint::new(0, 0), PixelBoundary::Unknown)
    }
}

impl FilterResult {
    /// `FilterResult(image, origin)`: the image at `origin` in layer space.
    // Port of: src/core/SkImageFilterTypes.h#L705-L709 (chrome/m156)
    #[doc(alias = "FilterResult")]
    #[must_use]
    pub fn new(image: Option<Arc<SpecialImage>>, origin: IPoint) -> FilterResult {
        FilterResult::with_boundary(image, origin, PixelBoundary::Unknown)
    }

    // Port of: src/core/SkImageFilterTypes.h#L1040-L1055 (chrome/m156)
    fn with_boundary(
        image: Option<Arc<SpecialImage>>,
        origin: IPoint,
        boundary: PixelBoundary,
    ) -> FilterResult {
        let transform = Matrix::translate((origin.x as f32, origin.y as f32));
        let dims = image.as_ref().map_or(ISize::new(0, 0), |i| i.dimensions());
        let layer_bounds = map_irect(&IRect::from_size(dims), &transform);
        FilterResult {
            image,
            boundary,
            sampling: default_sampling(),
            tile_mode: TileMode::Decal,
            transform,
            color_filter: None,
            layer_bounds,
        }
    }

    /// True if there is an image (`explicit operator bool`).
    #[must_use]
    pub fn has_image(&self) -> bool {
        self.image.is_some()
    }

    /// The image, if any (`image()`).
    #[must_use]
    pub fn image(&self) -> Option<&SpecialImage> {
        self.image.as_deref()
    }

    /// A shared handle to the image (`refImage()`).
    #[must_use]
    pub fn ref_image(&self) -> Option<Arc<SpecialImage>> {
        self.image.as_ref().map(Arc::clone)
    }

    /// The layer-space bounds of the result (`layerBounds()`).
    #[must_use]
    pub fn layer_bounds(&self) -> IRect {
        self.layer_bounds
    }

    /// The tile mode of the result (`tileMode()`).
    #[must_use]
    pub fn tile_mode(&self) -> TileMode {
        self.tile_mode
    }

    /// The sampling of the result (`sampling()`).
    #[must_use]
    pub fn sampling(&self) -> SamplingOptions {
        self.sampling
    }

    /// The color filter applied when drawing the result (`colorFilter()`).
    #[must_use]
    pub fn color_filter(&self) -> Option<&ColorFilter> {
        self.color_filter.as_ref()
    }

    /// The transform from the image to layer space (`fTransform`).
    #[must_use]
    pub fn transform(&self) -> &Matrix {
        &self.transform
    }

    /// `MakeFromPicture`: renders a picture clipped to `cull_rect` (given in parameter space).
    // Port of: src/core/SkImageFilterTypes.cpp#L1899-L1924 (chrome/m156)
    #[doc(alias = "MakeFromPicture")]
    #[must_use]
    pub fn make_from_picture(ctx: &Context<'_>, pic: &Picture, cull_rect: &Rect) -> FilterResult {
        let dst_bounds = round_out(&ctx.mapping().param_to_layer_rect(cull_rect));
        let Some(dst_bounds) = intersect_copy(dst_bounds, ctx.desired_output()) else {
            return FilterResult::default();
        };
        let props = ctx
            .backend()
            .surface_props()
            .clone_with_pixel_geometry(PixelGeometry::Unknown);
        let mut surface =
            AutoSurface::new(ctx, dst_bounds, PixelBoundary::Unknown, true, Some(&props));
        if let Some(canvas) = surface.canvas() {
            canvas.clip_rect(*cull_rect, None::<ClipOp>, None::<bool>);
            canvas.draw_picture(pic, None, None);
        }
        surface.snap()
    }

    /// `MakeFromShader`: fills the output with `shader`.
    // Port of: src/core/SkImageFilterTypes.cpp#L1926-L1946 (chrome/m156)
    #[doc(alias = "MakeFromShader")]
    #[must_use]
    pub fn make_from_shader(ctx: &Context<'_>, shader: Shader, dither: bool) -> FilterResult {
        let boundary = if dither {
            PixelBoundary::Unknown
        } else {
            PixelBoundary::Transparent
        };
        let mut surface = AutoSurface::new(ctx, ctx.desired_output(), boundary, true, None);
        if let Some(canvas) = surface.canvas() {
            let mut paint = Paint::default();
            paint.set_shader(Some(shader));
            paint.set_dither(dither);
            paint.set_blend_mode(BlendMode::Src);
            canvas.draw_paint(&paint);
        }
        surface.snap()
    }

    /// `MakeFromImage`: draws `src_rect` of `image` into `dst_rect` (parameter space).
    // Port of: src/core/SkImageFilterTypes.cpp#L1948-L2002 (chrome/m156)
    #[doc(alias = "MakeFromImage")]
    #[must_use]
    pub fn make_from_image(
        ctx: &Context<'_>,
        image: &Image,
        mut src_rect: Rect,
        mut dst_rect: Rect,
        sampling: SamplingOptions,
    ) -> FilterResult {
        let image_bounds = Rect::from_isize(image.dimensions());
        if !image_bounds.contains(src_rect) {
            let src_to_dst = Matrix::rect_to_rect_or_identity(src_rect, dst_rect, ScaleToFit::Fill);
            if !src_rect.intersect(image_bounds) {
                return FilterResult::default(); // No overlap, so return an empty/transparent image
            }
            dst_rect = src_to_dst.map_rect(src_rect).0;
        }
        if dst_rect.is_empty() {
            return FilterResult::default(); // Output collapses to empty
        }
        let src_subset = round_out(&src_rect);
        if Rect::from_irect(src_subset) == src_rect {
            let special = ctx.backend().make_image(&src_subset, image).map(Arc::new);
            let subset = FilterResult::new(special, src_subset.top_left());
            let transform = M44::concat(
                ctx.mapping().layer_matrix(),
                &M44::rect_to_rect(src_rect, dst_rect),
            );
            return subset.apply_transform(ctx, &transform.to_m33(), sampling);
        }
        let dst_bounds = intersect_copy(
            round_out(&ctx.mapping().param_to_layer_rect(&dst_rect)),
            ctx.desired_output(),
        );
        let Some(dst_bounds) = dst_bounds else {
            return FilterResult::default();
        };
        let mut surface = AutoSurface::new(ctx, dst_bounds, PixelBoundary::Transparent, true, None);
        if let Some(canvas) = surface.canvas() {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            canvas.draw_image_rect_with_sampling_options(
                image,
                Some((&src_rect, SrcRectConstraint::Strict)),
                dst_rect,
                sampling,
                &paint,
            );
        }
        surface.snap()
    }

    /// `imageAndOffset(ctx, offset)`: the resolved image and its origin in layer space.
    // Port of: src/core/SkImageFilterTypes.cpp#L641-L645 (chrome/m156)
    #[doc(alias = "imageAndOffset")]
    #[must_use]
    pub fn image_and_offset(&self, ctx: &Context<'_>) -> (Option<Arc<SpecialImage>>, IPoint) {
        let resolved = self.resolve(ctx, ctx.desired_output(), false);
        (resolved.image, resolved.layer_bounds.top_left())
    }

    /// `FilterResult::insetForSaveLayer`: a copy inset by one pixel, for layer restores.
    // Port of: src/core/SkImageFilterTypes.cpp#L647-L665 (chrome/m156)
    #[doc(alias = "insetForSaveLayer")]
    #[must_use]
    pub fn inset_for_save_layer(&self) -> FilterResult {
        if self.image.is_none() {
            return FilterResult::default();
        }
        debug_assert_eq!(self.tile_mode, TileMode::Decal);
        let mut inset = self.inset_by_pixel();
        debug_assert!(
            inset.boundary == PixelBoundary::Initialized && inset.tile_mode == TileMode::Decal
        );
        inset.boundary = PixelBoundary::Transparent;
        inset
    }

    /// `insetByPixel`.
    // Port of: src/core/SkImageFilterTypes.cpp#L667-L675 (chrome/m156)
    fn inset_by_pixel(&self) -> FilterResult {
        let mut inset_bounds = self.layer_bounds;
        inset_bounds.inset(crate::point::IVector::new(1, 1));
        debug_assert!(!inset_bounds.is_empty());
        self.subset(self.layer_bounds.top_left(), inset_bounds, false)
    }

    /// `analyzeBounds(xtraTransform, dstBounds, scope)`.
    // Port of: src/core/SkImageFilterTypes.cpp#L677-L825 (chrome/m156)
    #[doc(alias = "analyzeBounds")]
    fn analyze_bounds(
        &self,
        xtra_transform: &Matrix,
        dst_bounds: IRect,
        scope: BoundsScope,
    ) -> BoundsAnalysis {
        const K_HALF_PIXEL: f32 = 0.5;
        const K_CUBIC_RADIUS: f32 = 1.5;
        let image = self.image.as_deref().expect("analyzeBounds needs an image");

        let mut analysis = BoundsAnalysis::SIMPLE;
        let fills_layer_bounds = self.tile_mode != TileMode::Decal
            || self
                .color_filter
                .as_ref()
                .is_some_and(|cf| cf.as_base().affects_transparent_black());

        let mut pixel_center_bounds = Rect::from_irect(dst_bounds);
        if !rect_priv::quad_contains_rect(
            xtra_transform,
            &self.layer_bounds,
            &dst_bounds,
            ROUND_EPSILON,
        ) {
            let mut require_layer_crop = fills_layer_bounds;
            if !fills_layer_bounds {
                let image_bounds =
                    map_irect(&IRect::from_size(image.dimensions()), &self.transform);
                require_layer_crop = !self.layer_bounds.contains(image_bounds);
            }
            if require_layer_crop {
                analysis |= BoundsAnalysis::REQUIRES_LAYER_CROP;
                let layer_bounds_in_dst = map_irect(&self.layer_bounds, xtra_transform);
                let _ = pixel_center_bounds.intersect(Rect::from_irect(layer_bounds_in_dst));
            }
        }

        let image_bounds = Rect::from_isize(image.dimensions());
        // `netTransform = fTransform; netTransform.postConcat(xtra)`, i.e. `xtra * fTransform`.
        let mut net_transform = self.transform.clone();
        net_transform.post_concat(xtra_transform);
        let net_m44 = M44::from(&net_transform);
        let (x_axis_aligned, y_axis_aligned) = are_axes_nearly_integer_aligned(&net_transform);
        let is_pixel_aligned = x_axis_aligned && y_axis_aligned;
        let decal_leaks = scope != BoundsScope::Rescale
            && self.tile_mode == TileMode::Decal
            && self.sampling != SamplingOptions::default()
            && !is_pixel_aligned;

        let sample_radius = if self.sampling.use_cubic {
            K_CUBIC_RADIUS
        } else {
            K_HALF_PIXEL
        };
        let mut safe_image_bounds = image_bounds.with_inset((sample_radius, sample_radius));
        if self.sampling == default_sampling() && !is_pixel_aligned {
            safe_image_bounds.inset((
                if x_axis_aligned { 0.0 } else { ROUND_EPSILON },
                if y_axis_aligned { 0.0 } else { ROUND_EPSILON },
            ));
        }

        let mut has_pixel_padding = self.boundary != PixelBoundary::Unknown;
        let covered_geometry = if decal_leaks {
            safe_image_bounds
        } else {
            image_bounds
        };
        if !rect_priv::quad_contains_rect_m44(
            &net_m44,
            &covered_geometry,
            &pixel_center_bounds,
            ROUND_EPSILON,
        ) {
            analysis |= BoundsAnalysis::DST_BOUNDS_NOT_COVERED;
            if fills_layer_bounds {
                analysis |= BoundsAnalysis::HAS_LAYER_FILLING_EFFECT;
            }
            if decal_leaks {
                let scales = net_transform.min_max_scales();
                let nearly_one = |s: f32| (s - 1.0).abs() <= 0.2;
                let ok = matches!(scales, Some((a, b)) if nearly_one(a) && nearly_one(b));
                if !ok {
                    analysis |= BoundsAnalysis::REQUIRES_DECAL_IN_LAYER_SPACE;
                    if self.boundary == PixelBoundary::Transparent {
                        has_pixel_padding = false;
                    }
                }
            }
        }

        if scope == BoundsScope::Deferred {
            return analysis; // skip sampling analysis
        } else if scope == BoundsScope::CanDrawDirectly
            && !analysis.contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT)
        {
            let nn_or_bilerp =
                self.sampling == default_sampling() || self.sampling == SamplingOptions::default();
            if nn_or_bilerp && (has_pixel_padding || is_pixel_aligned) {
                return analysis;
            }
        }

        if has_pixel_padding {
            safe_image_bounds.outset((1.0, 1.0));
        }
        pixel_center_bounds.inset((K_HALF_PIXEL, K_HALF_PIXEL));
        let edge_mask = rect_priv::quad_contains_rect_mask(
            &net_m44,
            &safe_image_bounds,
            &pixel_center_bounds,
            ROUND_EPSILON,
        );
        let edge_lanes: [bool; 4] = std::array::from_fn(|i| edge_mask[i] != 0);
        if !edge_lanes.iter().all(|&b| b) {
            let subset = image.subset();
            let backing = image.backing_store_dimensions();
            let mut hw_edge = [
                subset.top == 0,
                subset.right == backing.width,
                subset.bottom == backing.height,
                subset.left == 0,
            ];
            if self.tile_mode == TileMode::Repeat || self.tile_mode == TileMode::Mirror {
                // TRBL & BLTR
                hw_edge = [
                    hw_edge[0] && hw_edge[2],
                    hw_edge[1] && hw_edge[3],
                    hw_edge[2] && hw_edge[0],
                    hw_edge[3] && hw_edge[1],
                ];
            }
            if !(0..4).all(|i| edge_lanes[i] || hw_edge[i]) {
                analysis |= BoundsAnalysis::REQUIRES_SHADER_TILING;
            }
        }
        analysis
    }

    /// `updateTileMode`.
    // Port of: src/core/SkImageFilterTypes.cpp#L827-L834 (chrome/m156)
    fn update_tile_mode(&mut self, ctx: &Context<'_>, tile_mode: TileMode) {
        if self.image.is_some() {
            self.tile_mode = tile_mode;
            if tile_mode != TileMode::Decal {
                self.layer_bounds = ctx.desired_output();
            }
        }
    }

    /// `canClampToTransparentBoundary`.
    fn can_clamp_to_transparent_boundary(&self, analysis: BoundsAnalysis) -> bool {
        self.tile_mode == TileMode::Decal
            && self.boundary == PixelBoundary::Transparent
            && !analysis.contains(BoundsAnalysis::REQUIRES_DECAL_IN_LAYER_SPACE)
    }

    /// `applyCrop`: restricts the result to `crop` (layer space), tiling it with `tile_mode`.
    // Port of: src/core/SkImageFilterTypes.cpp#L836-L931 (chrome/m156)
    #[doc(alias = "applyCrop")]
    #[must_use]
    pub fn apply_crop(
        &self,
        ctx: &Context<'_>,
        crop: IRect,
        mut tile_mode: TileMode,
    ) -> FilterResult {
        if crop.is_empty() || ctx.desired_output().is_empty() {
            return FilterResult::default();
        }
        let mut crop_content = crop;
        if self.image.is_none() || !irect_intersect_in_place(&mut crop_content, &self.layer_bounds)
        {
            return FilterResult::default();
        }

        let mut fitted_crop = relevant_subset(crop, ctx.desired_output(), tile_mode);
        if !irect_intersect_in_place(&mut crop_content, &fitted_crop) {
            return FilterResult::default();
        }

        if let Some(periodic) =
            periodic_axis_transform(tile_mode, fitted_crop, ctx.desired_output())
        {
            return self.apply_transform(ctx, &periodic, default_sampling());
        }

        let mut preserve_transparency_in_crop = false;
        if tile_mode == TileMode::Decal {
            fitted_crop = crop_content;
        } else if fitted_crop.contains(ctx.desired_output()) {
            tile_mode = TileMode::Decal;
            fitted_crop = ctx.desired_output();
        } else if !crop_content.contains(fitted_crop) {
            preserve_transparency_in_crop = true;
            if self.tile_mode == TileMode::Decal && tile_mode == TileMode::Clamp {
                crop_content.outset((1, 1));
                let ok = irect_intersect_in_place(&mut fitted_crop, &crop_content);
                debug_assert!(ok);
            }
        } // Otherwise cropContent == fittedCrop

        let double_clamp = self.tile_mode == TileMode::Clamp && tile_mode == TileMode::Clamp;
        if !preserve_transparency_in_crop
            && let Some(origin) = nearly_integer_translation(&self.transform)
            && (double_clamp
                || !self
                    .analyze_bounds(&Matrix::new_identity(), fitted_crop, BoundsScope::Deferred)
                    .contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT))
        {
            let mut restricted_output = self.subset(origin, fitted_crop, double_clamp);
            restricted_output.update_tile_mode(ctx, tile_mode);
            if restricted_output.boundary == PixelBoundary::Initialized
                || tile_mode != TileMode::Decal
            {
                restricted_output.boundary = PixelBoundary::Unknown;
            }
            restricted_output
        } else if tile_mode == TileMode::Decal {
            debug_assert!(!preserve_transparency_in_crop);
            let mut restricted_output = self.clone();
            restricted_output.layer_bounds = fitted_crop;
            restricted_output
        } else {
            let mut tiled = self.resolve(ctx, fitted_crop, true);
            tiled.update_tile_mode(ctx, tile_mode);
            tiled
        }
    }

    /// `applyColorFilter`: applies `color_filter` to the result.
    // Port of: src/core/SkImageFilterTypes.cpp#L933-L1002 (chrome/m156)
    #[doc(alias = "applyColorFilter")]
    #[must_use]
    pub fn apply_color_filter(&self, ctx: &Context<'_>, color_filter: ColorFilter) -> FilterResult {
        if ctx.desired_output().is_empty() {
            return FilterResult::default();
        }
        let mut new_layer_bounds = self.layer_bounds;
        if color_filter.as_base().affects_transparent_black() {
            if self.image.is_none()
                || !irect_intersect_in_place(&mut new_layer_bounds, &ctx.desired_output())
            {
                let one_px =
                    IRect::from_xywh(ctx.desired_output().left, ctx.desired_output().top, 1, 1);
                let mut surface =
                    AutoSurface::new(ctx, one_px, PixelBoundary::Initialized, false, None);
                if let Some(canvas) = surface.canvas() {
                    let mut paint = Paint::default();
                    paint.set_color4f(Color4f::new(0.0, 0.0, 0.0, 0.0), None);
                    paint.set_color_filter(Some(color_filter));
                    paint.set_blend_mode(BlendMode::Src);
                    canvas.draw_paint(&paint);
                }
                let mut solid_color = surface.snap();
                solid_color.update_tile_mode(ctx, TileMode::Clamp);
                return solid_color;
            }
            if self
                .analyze_bounds(
                    &Matrix::new_identity(),
                    ctx.desired_output(),
                    BoundsScope::Deferred,
                )
                .contains(BoundsAnalysis::REQUIRES_LAYER_CROP)
            {
                new_layer_bounds = outset_irect(new_layer_bounds, 1, 1);
                let ok = irect_intersect_in_place(&mut new_layer_bounds, &ctx.desired_output());
                debug_assert!(ok);
                let mut filtered = self.resolve(ctx, new_layer_bounds, true);
                filtered.color_filter = Some(color_filter);
                filtered.update_tile_mode(ctx, TileMode::Clamp);
                return filtered;
            }
            new_layer_bounds = ctx.desired_output();
        } else if self.image.is_none()
            || !IRect::intersects(&new_layer_bounds, &ctx.desired_output())
        {
            return FilterResult::default();
        }

        let mut filtered = self.clone();
        filtered.layer_bounds = new_layer_bounds;
        // `SkColorFilters::Compose(colorFilter, fColorFilter)`.
        filtered.color_filter =
            color_filters::compose(Some(&color_filter), self.color_filter.clone());
        filtered
    }

    /// `applyTransform`: transforms the result by `transform` (layer space), sampling with `sampling`.
    // Port of: src/core/SkImageFilterTypes.cpp#L1058-L1122 (chrome/m156)
    #[doc(alias = "applyTransform")]
    #[must_use]
    pub fn apply_transform(
        &self,
        ctx: &Context<'_>,
        transform: &Matrix,
        sampling: SamplingOptions,
    ) -> FilterResult {
        if self.image.is_none() || ctx.desired_output().is_empty() {
            debug_assert!(self.color_filter.is_none());
            return FilterResult::default();
        }
        if transform.invert().is_none() {
            return FilterResult::default();
        }

        let current_xform_is_integer = nearly_integer_translation(&self.transform).is_some();
        let next_xform_is_integer = nearly_integer_translation(transform).is_some();
        debug_assert!(!current_xform_is_integer || self.sampling == default_sampling());

        let mut next_sampling = if next_xform_is_integer {
            default_sampling()
        } else {
            sampling
        };
        let is_cropped = !next_xform_is_integer
            && self
                .analyze_bounds(transform, ctx.desired_output(), BoundsScope::Deferred)
                .contains(BoundsAnalysis::REQUIRES_LAYER_CROP);

        let mut transformed: FilterResult;
        if !is_cropped
            && compatible_sampling(
                self.sampling,
                current_xform_is_integer,
                &mut next_sampling,
                next_xform_is_integer,
            )
        {
            transformed = self.clone();
        } else {
            let tight_bounds = inverse_map_irect(transform, &ctx.desired_output());
            match tight_bounds {
                Some(tight) => transformed = self.resolve(ctx, tight, false),
                None => return FilterResult::default(),
            }
            if transformed.image.is_none() {
                return FilterResult::default();
            }
        }

        transformed.sampling = next_sampling;
        transformed.transform.post_concat(transform);
        transformed.layer_bounds = transform_rect_irect(transform, transformed.layer_bounds);
        if !IRect::intersects(&transformed.layer_bounds, &ctx.desired_output()) {
            return FilterResult::default();
        }
        transformed
    }

    /// `resolve`: renders the result into an image covering `dst_bounds`.
    // Port of: src/core/SkImageFilterTypes.cpp#L1124-L1155 (chrome/m156)
    fn resolve(
        &self,
        ctx: &Context<'_>,
        mut dst_bounds: IRect,
        preserve_dst_bounds: bool,
    ) -> FilterResult {
        if self.image.is_none()
            || (!preserve_dst_bounds
                && !irect_intersect_in_place(&mut dst_bounds, &self.layer_bounds))
        {
            return FilterResult::default();
        }

        let subset_compatible = self.color_filter.is_none()
            && self.tile_mode == TileMode::Decal
            && !preserve_dst_bounds;
        if subset_compatible {
            if let Some(origin) = nearly_integer_translation(&self.transform) {
                return self.subset(origin, dst_bounds, false);
            }
        } // else fall through and attempt a draw

        let props = SurfaceProps::default();
        let boundary = if preserve_dst_bounds {
            PixelBoundary::Unknown
        } else {
            PixelBoundary::Transparent
        };
        let mut surface = AutoSurface::new(ctx, dst_bounds, boundary, false, Some(&props));
        if surface.has_canvas() {
            surface.with_device(|device| self.draw_into(ctx, device, false, None));
        }
        surface.snap()
    }

    /// `subset(knownOrigin, subsetBounds, clampSrcIfDisjoint)`.
    // Port of: src/core/SkImageFilterTypes.cpp#L1157-L1202 (chrome/m156)
    fn subset(
        &self,
        known_origin: IPoint,
        subset_bounds: IRect,
        clamp_src_if_disjoint: bool,
    ) -> FilterResult {
        let image = self.image.as_deref().expect("subset needs an image");
        let image_bounds = IRect::from_xywh(
            known_origin.x,
            known_origin.y,
            image.width(),
            image.height(),
        );
        let tile = if clamp_src_if_disjoint {
            TileMode::Clamp
        } else {
            TileMode::Decal
        };
        let image_bounds = relevant_subset(image_bounds, subset_bounds, tile);
        if image_bounds.is_empty() {
            return FilterResult::default();
        }

        let subset = IRect {
            left: image_bounds.left - known_origin.x,
            top: image_bounds.top - known_origin.y,
            right: image_bounds.right - known_origin.x,
            bottom: image_bounds.bottom - known_origin.y,
        };
        debug_assert!(
            subset.left >= 0
                && subset.top >= 0
                && subset.right <= image.width()
                && subset.bottom <= image.height()
        );

        let new_image = image.make_subset(&subset).map(Arc::new);
        let mut result =
            FilterResult::with_boundary(new_image, image_bounds.top_left(), PixelBoundary::Unknown);
        result.color_filter.clone_from(&self.color_filter);
        debug_assert_eq!(result.boundary, PixelBoundary::Unknown);
        let result_subset = result.image.as_deref().map(SpecialImage::subset);
        if Some(image.subset()) == result_subset {
            result.boundary = self.boundary;
        } else {
            let mut safe_subset = image.subset();
            if self.boundary == PixelBoundary::Unknown {
                safe_subset.inset((1, 1));
            }
            if result_subset.is_some_and(|r| safe_subset.contains(r)) {
                result.boundary = PixelBoundary::Initialized;
            }
        }
        result
    }

    /// `draw(ctx, target, blender)`: draws the result into `target` with `target`'s local-to-device
    /// matrix (the layer-to-device transform of `ctx`).
    // Port of: src/core/SkImageFilterTypes.cpp#L1204-L1207 (chrome/m156)
    pub fn draw(&self, ctx: &Context<'_>, target: &mut dyn Device, blender: Option<&Blender>) {
        let saved = *target.state().local_to_device44();
        target
            .state_mut()
            .set_local_to_device(ctx.mapping().layer_to_device());
        self.draw_into(ctx, target, true, blender);
        target.state_mut().set_local_to_device(&saved);
    }

    /// `draw(ctx, device, preserveDeviceState, blender)`.
    // Port of: src/core/SkImageFilterTypes.cpp#L1209-L1325 (chrome/m156)
    fn draw_into(
        &self,
        ctx: &Context<'_>,
        device: &mut dyn Device,
        preserve_device_state: bool,
        blender: Option<&Blender>,
    ) {
        let blend_affects_transparent_black =
            blender.is_some_and(|b| b.as_base().affects_transparent_black());
        let Some(image) = self.image.as_deref() else {
            if blend_affects_transparent_black {
                let mut clear = Paint::default();
                clear.set_color4f(Color4f::new(0.0, 0.0, 0.0, 0.0), None);
                clear.set_blender(blender.cloned());
                device.draw_paint(&clear);
            }
            return;
        };

        let scope = if blend_affects_transparent_black {
            BoundsScope::ShaderOnly
        } else {
            BoundsScope::CanDrawDirectly
        };
        let local_to_device = device.state().local_to_device().clone();
        let dev_clip = device.dev_clip_bounds();
        let analysis = self.analyze_bounds(&local_to_device, dev_clip, scope);

        if analysis.contains(BoundsAnalysis::REQUIRES_LAYER_CROP) {
            if blend_affects_transparent_black {
                // (`LayerSpace<SkMatrix>(device->localToDevice()).inverseMapRect(...)`)
                let Some(dst_bounds) = inverse_map_irect(&local_to_device, &dev_clip) else {
                    return;
                };
                let clipped = self.resolve(ctx, dst_bounds, false);
                clipped.draw_into(ctx, device, preserve_device_state, blender);
                return;
            }
            if preserve_device_state {
                device.push_clip_stack();
            }
            device.clip_rect(
                &Rect::from_irect(self.layer_bounds),
                crate::clip_op::ClipOp::Intersect,
                true,
            );
        }

        let pixel_aligned = nearly_integer_translation(&self.transform).is_some()
            && nearly_integer_translation(&local_to_device).is_some();
        let mut sampling = self.sampling;
        if sampling == default_sampling() && pixel_aligned {
            sampling = SamplingOptions::default();
        }

        if analysis.contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT)
            || (blend_affects_transparent_black
                && analysis.contains(BoundsAnalysis::DST_BOUNDS_NOT_COVERED))
        {
            let mut paint = Paint::default();
            if !preserve_device_state && blender.is_none() {
                paint.set_blend_mode(BlendMode::Src);
            } else {
                paint.set_blender(blender.cloned());
            }
            // `None` is the failure of `getAnalyzedShaderView`: nothing is drawn, rather than
            // a solid-color paint over the whole layer.
            if let Some(shader) = self.get_analyzed_shader_view(ctx, sampling, analysis) {
                paint.set_shader(Some(shader));
                device.draw_paint(&paint);
            }
        } else {
            let mut paint = Paint::default();
            paint.set_blender(blender.cloned());
            paint.set_color_filter(self.color_filter.clone());
            let mut net_transform = Matrix::concat(&local_to_device, &self.transform);
            if self.can_clamp_to_transparent_boundary(analysis)
                && self.sampling == default_sampling()
            {
                debug_assert!(!analysis.contains(BoundsAnalysis::REQUIRES_SHADER_TILING));
                if !preserve_device_state && blender.is_none() {
                    paint.set_blend_mode(BlendMode::Src);
                }
                net_transform.pre_translate((-1.0, -1.0));
                let outset = image.make_pixel_outset();
                device.draw_special(
                    &outset,
                    &net_transform,
                    &sampling,
                    &paint,
                    SrcRectConstraint::Fast,
                );
            } else {
                paint.set_anti_alias(true);
                let mut constraint = SrcRectConstraint::Fast;
                if analysis.contains(BoundsAnalysis::REQUIRES_SHADER_TILING) {
                    constraint = SrcRectConstraint::Strict;
                    ctx.mark_shader_based_tiling_required(TileMode::Clamp);
                }
                device.draw_special(image, &net_transform, &sampling, &paint, constraint);
            }
        }

        if preserve_device_state && analysis.contains(BoundsAnalysis::REQUIRES_LAYER_CROP) {
            device.pop_clip_stack();
        }
    }

    /// `asShader`: the result as a shader over the layer (`sampleBounds` are the layer-space
    /// bounds the shader will sample).
    // Port of: src/core/SkImageFilterTypes.cpp#L1327-L1389 (chrome/m156)
    #[doc(alias = "asShader")]
    fn as_shader(
        &self,
        ctx: &Context<'_>,
        xtra_sampling: SamplingOptions,
        flags: ShaderFlags,
        sample_bounds: IRect,
    ) -> Option<Shader> {
        let image = self.image.as_deref()?;
        let current_xform_is_integer = nearly_integer_translation(&self.transform).is_some();
        let next_xform_is_integer = !flags.contains(ShaderFlags::NON_TRIVIAL_SAMPLING);

        let analysis = self.analyze_bounds(
            &Matrix::new_identity(),
            sample_bounds,
            BoundsScope::ShaderOnly,
        );
        let mut sampling = xtra_sampling;
        // `fColorFilter && (!asAColorMode(...) || colorFilterMode > kLastCoeffMode)`
        let color_filter_needs_resolve = match &self.color_filter {
            None => false,
            Some(cf) => match cf.to_a_color_mode() {
                None => true,
                Some((_, mode)) => mode > BlendMode::LAST_COEFF_MODE,
            },
        };
        let color_space_differs = !crate::color_space::ColorSpace::equals(
            image.color_info().color_space().as_ref(),
            ctx.color_space(),
        );
        let needs_resolve = (flags.contains(ShaderFlags::SAMPLED_REPEATEDLY)
            && (color_filter_needs_resolve || color_space_differs))
            || !compatible_sampling(
                self.sampling,
                current_xform_is_integer,
                &mut sampling,
                next_xform_is_integer,
            )
            || analysis.contains(BoundsAnalysis::REQUIRES_LAYER_CROP);

        if sampling == default_sampling()
            && next_xform_is_integer
            && (needs_resolve || current_xform_is_integer)
        {
            sampling = SamplingOptions::default();
        }

        if needs_resolve {
            let resolved = self.resolve(ctx, sample_bounds, false);
            if resolved.has_image() {
                let analysis = resolved.analyze_bounds(
                    &Matrix::new_identity(),
                    sample_bounds,
                    BoundsScope::ShaderOnly,
                );
                debug_assert!(
                    (analysis
                        - (BoundsAnalysis::DST_BOUNDS_NOT_COVERED
                            | BoundsAnalysis::REQUIRES_SHADER_TILING))
                        .is_empty()
                );
                return resolved.get_analyzed_shader_view(ctx, sampling, analysis);
            }
            None
        } else {
            self.get_analyzed_shader_view(ctx, sampling, analysis)
        }
    }

    /// `getAnalyzedShaderView`: the shader that samples the image with the analysis' tiling.
    // Port of: src/core/SkImageFilterTypes.cpp#L1391-L1468 (chrome/m156)
    fn get_analyzed_shader_view(
        &self,
        ctx: &Context<'_>,
        final_sampling: SamplingOptions,
        analysis: BoundsAnalysis,
    ) -> Option<Shader> {
        let image = self.image.as_deref()?;
        let local_matrix = &self.transform;
        let image_bounds = Rect::from(IRect::from_size(image.dimensions()));
        // We need to apply the decal in a coordinate space that matches the resolution of the
        // layer space. If the transform preserves rectangles, map the image bounds by the
        // transform so we can apply it before we evaluate the shader. Otherwise decompose the
        // transform into a non-scaling post-decal transform and a scaling pre-decal transform.
        let (post_decal, mut pre_decal) = if local_matrix.rect_stays_rect()
            || !analysis.contains(BoundsAnalysis::REQUIRES_DECAL_IN_LAYER_SPACE)
        {
            (Matrix::new_identity(), local_matrix.clone())
        } else {
            decompose_transform(local_matrix, image_bounds.center())
        };

        let mut effective_tile_mode = self.tile_mode;
        let decal_clamp_to_transparent = self.can_clamp_to_transparent_boundary(analysis);
        let strict = analysis.contains(BoundsAnalysis::REQUIRES_SHADER_TILING);
        let mut image_shader: Option<Shader>;
        if strict && decal_clamp_to_transparent {
            pre_decal.pre_translate((-1.0, -1.0));
            let outset = image.make_pixel_outset();
            image_shader = outset.as_shader(TileMode::Clamp, final_sampling, &pre_decal, strict);
            effective_tile_mode = TileMode::Clamp;
        } else {
            if !analysis.contains(BoundsAnalysis::DST_BOUNDS_NOT_COVERED)
                || analysis.contains(BoundsAnalysis::REQUIRES_DECAL_IN_LAYER_SPACE)
            {
                effective_tile_mode = TileMode::Clamp;
            }
            image_shader = image.as_shader(effective_tile_mode, final_sampling, &pre_decal, strict);
        }
        if strict {
            ctx.mark_shader_based_tiling_required(effective_tile_mode);
        }

        if analysis.contains(BoundsAnalysis::REQUIRES_DECAL_IN_LAYER_SPACE) {
            debug_assert_eq!(self.tile_mode, TileMode::Decal);
            // The decal is applied by the `kDecal` known runtime effect, with the image shader
            // as its `image` child (null if there is none).
            let decal_effect = get_known_runtime_effect(StableKey::Decal)?;
            let mut builder = RuntimeEffectBuilder::new(decal_effect.clone());
            {
                let mut child = builder.child("image");
                match image_shader.take() {
                    Some(shader) => {
                        child.assign(shader);
                    }
                    None => {
                        child.assign_null();
                    }
                }
            }
            let decal_bounds = pre_decal.map_rect(image_bounds).0;
            builder.uniform("decalBounds").set_f32(&[
                decal_bounds.left,
                decal_bounds.top,
                decal_bounds.right,
                decal_bounds.bottom,
            ]);
            image_shader = builder.make_shader(None::<&Matrix>);
        }

        if analysis.contains(BoundsAnalysis::REQUIRES_DECAL_IN_LAYER_SPACE)
            && let Some(shader) = image_shader.take()
        {
            image_shader = Some(shader.with_local_matrix(&post_decal));
        }
        // `if (imageShader && fColorFilter) imageShader = imageShader->makeWithColorFilter(...)`.
        if let Some(cf) = self.color_filter.as_ref()
            && let Some(shader) = image_shader.take()
        {
            image_shader = Some(shader.with_color_filter(cf.clone()));
        }
        image_shader
    }
}

/// `AutoSurface`: a temporary device and canvas for one filter step, snapped to a `FilterResult`.
// Port of: src/core/SkImageFilterTypes.cpp#L512-L633 (chrome/m156)
struct AutoSurface {
    canvas: Option<Canvas>,
    dst_bounds: IRect, // includes padding, if any
    boundary: PixelBoundary,
}

impl AutoSurface {
    // Port of: src/core/SkImageFilterTypes.cpp#L514-L566 (chrome/m156)
    fn new(
        ctx: &Context<'_>,
        dst_bounds: IRect,
        boundary: PixelBoundary,
        render_in_parameter_space: bool,
        props: Option<&SurfaceProps>,
    ) -> AutoSurface {
        let mut surface = AutoSurface {
            canvas: None,
            dst_bounds,
            boundary,
        };
        if dst_bounds.is_empty() {
            return surface;
        }
        let padding = surface.padding();
        if padding != 0 {
            surface.dst_bounds = outset_irect(surface.dst_bounds, padding, padding);
            if surface.dst_bounds.left >= dst_bounds.left
                || surface.dst_bounds.right <= dst_bounds.right
                || surface.dst_bounds.top >= dst_bounds.top
                || surface.dst_bounds.bottom <= dst_bounds.bottom
            {
                return surface;
            }
        }
        let size = ISize::new(surface.dst_bounds.width(), surface.dst_bounds.height());
        let Some(device) = ctx
            .backend()
            .make_device(size, ctx.color_space().cloned(), props)
        else {
            return surface;
        };
        ctx.mark_new_surface();
        let canvas = Canvas::from_device(device);
        canvas.translate(Vector::new(
            -(surface.dst_bounds.left as f32),
            -(surface.dst_bounds.top as f32),
        ));
        canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
        if surface.boundary == PixelBoundary::Transparent {
            canvas.clip_irect(dst_bounds, None);
        } else {
            canvas.clip_irect(surface.dst_bounds, None);
        }
        if render_in_parameter_space {
            canvas.concat_44(ctx.mapping().layer_matrix());
        }
        surface.canvas = Some(canvas);
        surface
    }

    fn padding(&self) -> i32 {
        i32::from(self.boundary != PixelBoundary::Unknown)
    }

    fn has_canvas(&self) -> bool {
        self.canvas.is_some()
    }

    fn canvas(&mut self) -> Option<&Canvas> {
        self.canvas.as_ref()
    }

    /// Runs `f` with the device of the canvas (`SkCanvasPriv::TopDevice`).
    fn with_device(&mut self, f: impl FnOnce(&mut dyn Device)) {
        if let Some(canvas) = self.canvas.as_ref() {
            canvas.with_top_device(f);
        }
    }

    // Port of: src/core/SkImageFilterTypes.cpp#L545-L565 (chrome/m156)
    fn snap(mut self) -> FilterResult {
        let Some(canvas) = self.canvas.take() else {
            return FilterResult::default();
        };
        canvas.restore_to_count(0);
        let subset = IRect::from_wh(self.dst_bounds.width(), self.dst_bounds.height());
        let image = canvas.with_top_device(|device| {
            device.set_immutable();
            device.snap_special(&subset, false)
        });
        drop(canvas);
        match image {
            Some(image) if self.boundary != PixelBoundary::Unknown => {
                let padding = self.padding();
                let subset = IRect::from_size(image.dimensions()).with_inset((padding, padding));
                let origin = IPoint::new(
                    self.dst_bounds.left + padding,
                    self.dst_bounds.top + padding,
                );
                let sub = image.make_subset(&subset).map(Arc::new);
                FilterResult::with_boundary(sub, origin, self.boundary)
            }
            Some(image) => FilterResult::with_boundary(
                Some(Arc::new(image)),
                self.dst_bounds.top_left(),
                PixelBoundary::Unknown,
            ),
            None => FilterResult::default(),
        }
    }
}

/// `FilterResult::Builder`: combines several filter results with shaders or merging.
// Port of: src/core/SkImageFilterTypes.h#L882-L891 (chrome/m156)
#[derive(Debug)]
pub struct Builder<'c, 'a> {
    context: &'c Context<'a>,
    inputs: Vec<SampledFilterResult>,
}

#[derive(Debug)]
struct SampledFilterResult {
    image: FilterResult,
    sample_bounds: Option<IRect>,
    flags: ShaderFlags,
    sampling: SamplingOptions,
}

impl<'c, 'a> Builder<'c, 'a> {
    /// `Builder(context)`.
    // Port of: src/core/SkImageFilterTypes.cpp#L2007 (chrome/m156)
    #[must_use]
    pub fn new(context: &'c Context<'a>) -> Builder<'c, 'a> {
        Builder {
            context,
            inputs: Vec::new(),
        }
    }

    /// `add(input, sampleBounds, inputFlags, inputSampling)`.
    pub fn add(
        &mut self,
        input: FilterResult,
        sample_bounds: Option<IRect>,
        input_flags: ShaderFlags,
        input_sampling: SamplingOptions,
    ) -> &mut Self {
        self.inputs.push(SampledFilterResult {
            image: input,
            sample_bounds,
            flags: input_flags,
            sampling: input_sampling,
        });
        self
    }

    /// `outputBounds(explicitOutput)`.
    // Port of: src/core/SkImageFilterTypes.cpp#L2045-L2056 (chrome/m156)
    fn output_bounds(&self, explicit_output: Option<IRect>) -> IRect {
        let mut output = self.context.desired_output();
        if let Some(explicit) = explicit_output {
            if !irect_intersect_in_place(&mut output, &explicit) {
                return IRect::new_empty();
            }
        }
        output
    }

    /// `createInputShaders(outputBounds, evaluateInParameterSpace)`.
    // Port of: src/core/SkImageFilterTypes.cpp#L2010-L2043 (chrome/m156)
    fn create_input_shaders(
        &self,
        output_bounds: IRect,
        evaluate_in_parameter_space: bool,
    ) -> Vec<Option<Shader>> {
        let mut xtra_flags = ShaderFlags::NONE;
        let mut layer_to_param = Matrix::new_identity();
        if evaluate_in_parameter_space {
            layer_to_param = self
                .context
                .mapping()
                .layer_matrix()
                .to_m33()
                .invert()
                .unwrap_or_else(Matrix::new_identity);
            if nearly_integer_translation(&layer_to_param).is_none() {
                xtra_flags |= ShaderFlags::NON_TRIVIAL_SAMPLING;
            }
        }
        self.inputs
            .iter()
            .map(|input| {
                let sample_bounds = input.sample_bounds.unwrap_or(output_bounds);
                let shader = input.image.as_shader(
                    self.context,
                    input.sampling,
                    input.flags | xtra_flags,
                    sample_bounds,
                );
                if evaluate_in_parameter_space {
                    shader.map(|s| s.with_local_matrix(&layer_to_param))
                } else {
                    shader
                }
            })
            .collect()
    }

    /// `drawShader(shader, outputBounds, evaluateInParameterSpace)`.
    // Port of: src/core/SkImageFilterTypes.cpp#L2058-L2077 (chrome/m156)
    fn draw_shader(
        &self,
        shader: Option<Shader>,
        output_bounds: IRect,
        evaluate_in_parameter_space: bool,
    ) -> FilterResult {
        let Some(shader) = shader else {
            return FilterResult::default();
        };
        let mut surface = AutoSurface::new(
            self.context,
            output_bounds,
            PixelBoundary::Transparent,
            evaluate_in_parameter_space,
            None,
        );
        if let Some(canvas) = surface.canvas() {
            let mut paint = Paint::default();
            paint.set_shader(Some(shader));
            paint.set_blend_mode(BlendMode::Src);
            canvas.draw_paint(&paint);
        }
        surface.snap()
    }

    /// `eval(shaderFn, explicitOutput, evaluateInParameterSpace)`: draws the shader that
    /// `shader_fn` builds from the inputs' shaders.
    // Port of: src/core/SkImageFilterTypes.h#L849-L862 (chrome/m156)
    pub fn eval(
        &mut self,
        shader_fn: impl FnOnce(&[Option<Shader>]) -> Option<Shader>,
        explicit_output: Option<IRect>,
        evaluate_in_parameter_space: bool,
    ) -> FilterResult {
        let output_bounds = self.output_bounds(explicit_output);
        if output_bounds.is_empty() {
            return FilterResult::default();
        }
        let input_shaders = self.create_input_shaders(output_bounds, evaluate_in_parameter_space);
        let shader = shader_fn(&input_shaders);
        self.draw_shader(shader, output_bounds, evaluate_in_parameter_space)
    }

    /// `merge()`: source-over merges the inputs into one result.
    // Port of: src/core/SkImageFilterTypes.cpp#L2079-L2106 (chrome/m156)
    pub fn merge(&self) -> FilterResult {
        debug_assert!(!self.inputs.is_empty());
        if self.inputs.len() == 1 {
            debug_assert!(
                self.inputs[0].sample_bounds.is_none()
                    && self.inputs[0].sampling == default_sampling()
                    && self.inputs[0].flags == ShaderFlags::NONE
            );
            return self.inputs[0].image.clone();
        }
        let merged_bounds = self
            .inputs
            .iter()
            .map(|i| i.image.layer_bounds())
            .reduce(|acc, b| IRect::join(&acc, &b))
            .unwrap_or_else(IRect::new_empty);
        let output_bounds = self.output_bounds(Some(merged_bounds));
        let mut surface = AutoSurface::new(
            self.context,
            output_bounds,
            PixelBoundary::Transparent,
            false,
            None,
        );
        if surface.has_canvas() {
            for input in &self.inputs {
                debug_assert!(
                    input.sample_bounds.is_none()
                        && input.sampling == default_sampling()
                        && input.flags == ShaderFlags::NONE
                );
                let ctx = self.context;
                surface.with_device(|device| input.image.draw_into(ctx, device, true, None));
            }
        }
        surface.snap()
    }
}

/// `SkIRect::intersect` on copies: the intersection of `a` and `b`, or `None` if empty.
fn intersect_copy(a: IRect, b: IRect) -> Option<IRect> {
    let mut out = a;
    if irect_intersect_in_place(&mut out, &b) {
        Some(out)
    } else {
        None
    }
}

/// `SkIRect::outset(dx, dy)`.
pub(crate) fn outset_irect(r: IRect, dx: i32, dy: i32) -> IRect {
    r.with_outset((dx, dy))
}

/// `SkRect` of an `SkIRect` mapped by `m` (`LayerSpace<SkMatrix>::mapRect(LayerSpace<SkIRect>)`).
fn transform_rect_irect(m: &Matrix, r: IRect) -> IRect {
    map_irect(&r, m)
}

/// `nearlyIntegerTranslation`: the origin of a pure, near-integer translation (`LayerSpace` form).
// Port of: src/core/SkImageFilterTypes.cpp#L84-L91 (chrome/m156)
#[must_use]
pub(crate) fn nearly_integer_translation(m: &Matrix) -> Option<IPoint> {
    let (axis_x, axis_y, origin) = are_axes_nearly_integer_aligned_origin(m);
    if axis_x && axis_y { origin } else { None }
}

/// `SkScalarNearlyEqual(x, y, tol)`.
fn nearly_equal(x: f32, y: f32, tol: f32) -> bool {
    (x - y).abs() <= tol
}

/// `are_axes_nearly_integer_aligned` (without the origin output).
// Port of: src/core/SkImageFilterTypes.cpp#L56-L80 (chrome/m156)
#[must_use]
pub(crate) fn are_axes_nearly_integer_aligned(m: &Matrix) -> (bool, bool) {
    let (x, y, _) = are_axes_nearly_integer_aligned_origin(m);
    (x, y)
}

/// `are_axes_nearly_integer_aligned` with its `out` origin: `(xAxis, yAxis, origin-if-both)`.
// Port of: src/core/SkImageFilterTypes.cpp#L56-L80 (chrome/m156)
fn are_axes_nearly_integer_aligned_origin(m: &Matrix) -> (bool, bool, Option<IPoint>) {
    use crate::scalar::scalar_round_to_scalar;
    let inv_w = crate::floating_point::ieee_float_divide(1.0, m.rc(2, 2));
    let tx = scalar_round_to_scalar(m.rc(0, 2) * inv_w);
    let ty = scalar_round_to_scalar(m.rc(1, 2) * inv_w);
    let affine = nearly_equal(m.rc(2, 0) * inv_w, 0.0, ROUND_EPSILON)
        && nearly_equal(m.rc(2, 1) * inv_w, 0.0, ROUND_EPSILON);
    if !affine {
        return (false, false, None);
    }
    let x_axis = nearly_equal(1.0, m.rc(0, 0) * inv_w, ROUND_EPSILON)
        && nearly_equal(0.0, m.rc(0, 1) * inv_w, ROUND_EPSILON)
        && nearly_equal(tx, m.rc(0, 2) * inv_w, ROUND_EPSILON);
    let y_axis = nearly_equal(0.0, m.rc(1, 0) * inv_w, ROUND_EPSILON)
        && nearly_equal(1.0, m.rc(1, 1) * inv_w, ROUND_EPSILON)
        && nearly_equal(ty, m.rc(1, 2) * inv_w, ROUND_EPSILON);
    let origin = if x_axis && y_axis {
        Some(IPoint::new(tx as i32, ty as i32))
    } else {
        None
    };
    (x_axis, y_axis, origin)
}

/// `periodic_axis_transform`: the transform that tiles `crop` periodically over `output`, if the
/// output spans at most one period in each direction.
// Port of: src/core/SkImageFilterTypes.cpp#L118-L181 (chrome/m156)
fn periodic_axis_transform(tile_mode: TileMode, crop: IRect, output: IRect) -> Option<Matrix> {
    use crate::floating_point::double_saturate2int;
    if tile_mode == TileMode::Clamp || tile_mode == TileMode::Decal {
        return None;
    }
    let crop_l = f64::from(crop.left);
    let crop_t = f64::from(crop.top);
    let crop_width = f64::from(crop.right) - crop_l;
    let crop_height = f64::from(crop.bottom) - crop_t;
    let period_l = ((f64::from(output.left) - crop_l) / crop_width).floor();
    let period_t = ((f64::from(output.top) - crop_t) / crop_height).floor();
    let period_r = ((f64::from(output.right) - crop_l) / crop_width).ceil();
    let period_b = ((f64::from(output.bottom) - crop_t) / crop_height).ceil();
    if period_r - period_l <= 1.0 && period_b - period_t <= 1.0 {
        let mut sx = 1.0_f32;
        let mut sy = 1.0_f32;
        let mut tx = -crop_l;
        let mut ty = -crop_t;
        if tile_mode == TileMode::Mirror {
            if period_l % 2.0 > f64::from(crate::scalar::SCALAR_NEARLY_ZERO) {
                sx = -1.0;
                tx = crop_width - tx;
            }
            if period_t % 2.0 > f64::from(crate::scalar::SCALAR_NEARLY_ZERO) {
                sy = -1.0;
                ty = crop_height - ty;
            }
        }
        tx += period_l * crop_width + crop_l;
        ty += period_t * crop_height + crop_t;
        // `sk_double_saturate2int(tx) != (float) tx`: the int is converted to float first.
        if double_saturate2int(tx) as f32 != tx as f32
            || double_saturate2int(ty) as f32 != ty as f32
        {
            return None;
        }
        let mut periodic = Matrix::new_identity();
        periodic.set_scale_translate((sx, sy), (tx as f32, ty as f32));
        Some(periodic)
    } else {
        None
    }
}

/// `compatible_sampling`: whether a result sampled with `current` can be re-sampled with `next`,
/// updating `next` to the combined options.
// Port of: src/core/SkImageFilterTypes.cpp#L1004-L1056 (chrome/m156)
#[allow(clippy::if_same_then_else)] // Skia's branches are kept separate, in the same order, for review.
fn compatible_sampling(
    current: SamplingOptions,
    current_xform_wont_affect_nearest: bool,
    next: &mut SamplingOptions,
    next_xform_wont_affect_nearest: bool,
) -> bool {
    let cur_aniso = current.is_aniso();
    let next_aniso = next.is_aniso();
    if cur_aniso && next_aniso {
        *next = SamplingOptions::from_aniso(current.max_aniso.max(next.max_aniso));
        true
    } else if cur_aniso && next.filter == FilterMode::Linear {
        *next = current;
        true
    } else if next_aniso && current.filter == FilterMode::Linear {
        true
    } else if current.use_cubic
        && (next.filter == FilterMode::Linear
            || (next.use_cubic
                && current.cubic.b == next.cubic.b
                && current.cubic.c == next.cubic.c))
    {
        *next = current;
        true
    } else if next.use_cubic && current.filter == FilterMode::Linear {
        true
    } else if current.filter == FilterMode::Linear && next.filter == FilterMode::Linear {
        true
    } else if next.filter == FilterMode::Nearest && current_xform_wont_affect_nearest {
        debug_assert_eq!(current.filter, FilterMode::Linear);
        true
    } else if current.filter == FilterMode::Nearest && next_xform_wont_affect_nearest {
        debug_assert_eq!(next.filter, FilterMode::Linear);
        *next = current;
        true
    } else {
        false
    }
}

/// `downscale_step_count`: the number of halving steps needed to scale by `net_scale_factor`.
// Port of: src/core/SkImageFilterTypes.cpp#L1479-L1500 (chrome/m156)
fn downscale_step_count(net_scale_factor: f32) -> i32 {
    let ceil_steps = float_ceil2int(1.0f32 / net_scale_factor).max(0);
    let mut steps = next_log2(u32::try_from(ceil_steps).unwrap_or(0));
    // There are (steps-1) 1/2x steps and then one step that will be between 1/2-1x. If the final
    // step is practically the identity scale, we can save a render pass and not incur too much
    // sampling error by reducing the step count and using a final scale that's slightly less than
    // 1/2.
    if steps > 0 {
        // For a multipass rescale, we allow for a lot of tolerance when deciding to collapse the
        // final step. If there's only a single pass, we require the scale factor to be very close
        // to the identity since it causes the step count to go to 0.
        const K_MULTI_PASS_LIMIT: f32 = 0.9;
        const K_NEAR_IDENTITY_LIMIT: f32 = 1.0 - ROUND_EPSILON;

        let final_step_scale = net_scale_factor * ((1i32 << (steps - 1)) as f32);
        let limit = if steps == 1 {
            K_NEAR_IDENTITY_LIMIT
        } else {
            K_MULTI_PASS_LIMIT
        };
        if final_step_scale >= limit {
            steps -= 1;
        }
    }
    steps
}

/// `scale_about_center`: scales `src` by `(sx, sy)` about its center (on an axis that changes).
// Port of: src/core/SkImageFilterTypes.cpp#L1502-L1507 (chrome/m156)
fn scale_about_center(src: Rect, sx: f32, sy: f32) -> Rect {
    let cx = if sx == 1.0 {
        0.0
    } else {
        0.5 * src.left + 0.5 * src.right
    };
    let cy = if sy == 1.0 {
        0.0
    } else {
        0.5 * src.top + 0.5 * src.bottom
    };
    Rect::new(
        (src.left - cx) * sx,
        (src.top - cy) * sy,
        (src.right - cx) * sx,
        (src.bottom - cy) * sy,
    )
}

/// `draw_color_filtered_border`: fills the 1px border of `border` with the transparency-affecting
/// color filter `color_filter`.
// Port of: src/core/SkImageFilterTypes.cpp#L1509-L1531 (chrome/m156)
fn draw_color_filtered_border(canvas: &Canvas, border: IRect, color_filter: ColorFilter) {
    let mut cf_only = Paint::default();
    cf_only.set_color4f(Color4f::new(0.0, 0.0, 0.0, 0.0), None);
    cf_only.set_color_filter(Some(color_filter));
    cf_only.set_blend_mode(BlendMode::Src);

    // Top (with corners)
    canvas.draw_irect(
        IRect::new(border.left, border.top, border.right, border.top + 1),
        &cf_only,
    );
    // Bottom (with corners)
    canvas.draw_irect(
        IRect::new(border.left, border.bottom - 1, border.right, border.bottom),
        &cf_only,
    );
    // Left (no corners)
    canvas.draw_irect(
        IRect::new(
            border.left,
            border.top + 1,
            border.left + 1,
            border.bottom - 1,
        ),
        &cf_only,
    );
    // Right (no corners)
    canvas.draw_irect(
        IRect::new(
            border.right - 1,
            border.top + 1,
            border.right,
            border.bottom - 1,
        ),
        &cf_only,
    );
}

/// `draw_tiled_border`: samples the edge and corner pixels of the source directly into the 1px
/// padding of the destination, for the non-decal tile modes.
// Port of: src/core/SkImageFilterTypes.cpp#L1533-L1616 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors draw_tiled_border
fn draw_tiled_border(
    canvas: &Canvas,
    tile_mode: TileMode,
    paint: &Paint,
    src_to_dst: &Matrix,
    mut src_border: Rect,
    mut dst_border: Rect,
) {
    // Sample the border pixels directly, scaling only on an axis at a time for edges, and with
    // no scaling for corners.
    let draw_edge = |src: Rect, dst: Rect| {
        canvas.save();
        canvas.concat(&Matrix::rect_to_rect_or_identity(
            src,
            dst,
            ScaleToFit::Fill,
        ));
        canvas.draw_rect(src, paint);
        canvas.restore();
    };
    let draw_corner = |src: Point, dst: Point| {
        draw_edge(
            Rect::from_xywh(src.x, src.y, 1.0, 1.0),
            Rect::from_xywh(dst.x, dst.y, 1.0, 1.0),
        );
    };

    // 'dstBorder' includes the 1px padding that we are filling in. Inset to reconstruct the
    // original sampled dst.
    let mut dst_sample_bounds = dst_border;
    dst_sample_bounds.inset((1.0, 1.0));

    // Reconstruct the original source coordinate bounds
    let src_sample_bounds =
        inverse_map_rect_f(src_to_dst, &dst_sample_bounds).unwrap_or(dst_sample_bounds);

    if tile_mode == TileMode::Mirror || tile_mode == TileMode::Repeat {
        // Adjust 'srcBorder' to instead match the 1px rectangle centered over srcSampleBounds in
        // order to calculate the average of the two outermost sampled pixels. Inset by an extra
        // 1/2 so that the eventual sample coordinates average the outermost two rows/columns of
        // src pixels.
        src_border = dst_sample_bounds;
        src_border.inset((0.5, 0.5));
        src_border = inverse_map_rect_f(src_to_dst, &src_border).unwrap_or(src_border);
        src_border.outset((0.5, 0.5));
    }

    // Invert the dst coordinates for repeat so that the left edge is mapped to the right edge of
    // the output, etc.
    if tile_mode == TileMode::Repeat {
        dst_border = Rect::new(
            dst_border.right - 1.0,
            dst_border.bottom - 1.0,
            dst_border.left + 1.0,
            dst_border.top + 1.0,
        );
    }

    // Edges (excluding corners)
    draw_edge(
        Rect::new(
            src_border.left,
            src_sample_bounds.top,
            src_border.left + 1.0,
            src_sample_bounds.bottom,
        ),
        Rect::new(
            dst_border.left,
            dst_sample_bounds.top,
            dst_border.left + 1.0,
            dst_sample_bounds.bottom,
        ),
    ); // Left
    draw_edge(
        Rect::new(
            src_border.right - 1.0,
            src_sample_bounds.top,
            src_border.right,
            src_sample_bounds.bottom,
        ),
        Rect::new(
            dst_border.right - 1.0,
            dst_sample_bounds.top,
            dst_border.right,
            dst_sample_bounds.bottom,
        ),
    ); // Right
    draw_edge(
        Rect::new(
            src_sample_bounds.left,
            src_border.top,
            src_sample_bounds.right,
            src_border.top + 1.0,
        ),
        Rect::new(
            dst_sample_bounds.left,
            dst_border.top,
            dst_sample_bounds.right,
            dst_border.top + 1.0,
        ),
    ); // Top
    draw_edge(
        Rect::new(
            src_sample_bounds.left,
            src_border.bottom - 1.0,
            src_sample_bounds.right,
            src_border.bottom,
        ),
        Rect::new(
            dst_sample_bounds.left,
            dst_border.bottom - 1.0,
            dst_sample_bounds.right,
            dst_border.bottom,
        ),
    ); // Bottom

    // Corners (sampled directly to preserve their value since they can dominate the output of a
    // clamped blur with a large radius).
    draw_corner(
        Point::new(src_border.left, src_border.top),
        Point::new(dst_border.left, dst_border.top),
    ); // TL
    draw_corner(
        Point::new(src_border.right - 1.0, src_border.top),
        Point::new(dst_border.right - 1.0, dst_border.top),
    ); // TR
    draw_corner(
        Point::new(src_border.right - 1.0, src_border.bottom - 1.0),
        Point::new(dst_border.right - 1.0, dst_border.bottom - 1.0),
    ); // BR
    draw_corner(
        Point::new(src_border.left, src_border.bottom - 1.0),
        Point::new(dst_border.left, dst_border.bottom - 1.0),
    ); // BL
}

impl FilterResult {
    /// `FilterResult::rescale`: downscales the image by `scale` (in steps of 1/2, with a final
    /// partial step), resolving its effects along the way. With `enforce_decal`, the result is
    /// decal tiled.
    // Port of: src/core/SkImageFilterTypes.cpp#L1617-L1816 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors FilterResult::rescale
    #[must_use]
    pub fn rescale(
        &self,
        ctx: &Context<'_>,
        scale: Size,
        enforce_decal: bool,
        allow_overscaling: bool,
    ) -> FilterResult {
        let mut visible_layer_bounds = self.layer_bounds;
        let Some(image) = self.image.as_deref() else {
            return FilterResult::default();
        };
        if !irect_intersect_in_place(&mut visible_layer_bounds, &ctx.desired_output())
            || scale.width <= 0.0
            || scale.height <= 0.0
        {
            return FilterResult::default();
        }

        // NOTE: For the first pass, PixelSpace and LayerSpace are equivalent
        let origin_opt = nearly_integer_translation(&self.transform);
        let pixel_aligned = origin_opt.is_some();
        let origin = origin_opt.unwrap_or(IPoint::new(0, 0));
        let mut analysis = self.analyze_bounds(
            &Matrix::new_identity(),
            ctx.desired_output(),
            BoundsScope::Rescale,
        );

        // If there's no actual scaling, and no other effects that have to be resolved for blur(),
        // then just extract the necessary subset. Otherwise fall through and apply the effects
        // with scale factor (possibly identity).
        let can_defer_tiling = pixel_aligned
            && !analysis.contains(BoundsAnalysis::REQUIRES_LAYER_CROP)
            && !(enforce_decal && analysis.contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT));

        // To match legacy color space conversion logic, treat a null src as sRGB and a null dst as
        // as the src CS.
        let src_cs = image.color_info().color_space();
        let dst_cs = ctx.color_space().cloned().or_else(|| src_cs.clone());
        let has_effects_to_apply = !can_defer_tiling
            || self.color_filter.is_some()
            || image.color_info().color_type() != ctx.backend().color_type()
            || !ColorSpace::equals(src_cs.as_ref(), dst_cs.as_ref());

        let mut x_steps = downscale_step_count(scale.width);
        let mut y_steps = downscale_step_count(scale.height);
        if x_steps == 0 && y_steps == 0 && !has_effects_to_apply {
            if analysis.contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT) {
                // At this point, the only effects that could be visible is a non-decal mode, so
                // just return the image with adjusted layer bounds to match desired output.
                let mut noop = self.clone();
                noop.layer_bounds = visible_layer_bounds;
                return noop;
            }
            // The visible layer bounds represents a tighter bounds than the image itself
            return self.subset(origin, visible_layer_bounds, false);
        }

        let mut src_rect;
        let tile_mode;
        let mut cf_border = false;
        let mut defer_periodic_tiling = false;
        if can_defer_tiling && analysis.contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT) {
            // When we can defer tiling, and said tiling is visible, rescaling the original image
            // uses smaller textures.
            src_rect = IRect::from_xywh(origin.x, origin.y, image.width(), image.height());
            if self.tile_mode == TileMode::Decal
                && analysis.contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT)
            {
                // Like in applyColorFilter() evaluate the transparent CF'ed border and clamp to it.
                tile_mode = TileMode::Clamp;
                cf_border = true;
            } else {
                tile_mode = self.tile_mode;
                defer_periodic_tiling =
                    tile_mode == TileMode::Repeat || tile_mode == TileMode::Mirror;
            }
        } else {
            // Otherwise we either have to rescale the layer-bounds-sized image (!canDeferTiling)
            // or the tiling isn't visible so the layer bounds represents a smaller effective
            // image than the original image data.
            src_rect = visible_layer_bounds;
            tile_mode = TileMode::Decal;
        }

        src_rect = relevant_subset(src_rect, ctx.desired_output(), tile_mode);
        // To avoid incurring error from rounding up the dimensions at every step, the logical size
        // of the image is tracked in floats through the whole process; rounding to integers is
        // only done to produce a conservative pixel buffer and clamp-tiling is used so that
        // partially covered pixels are filled with the un-weighted color.
        let mut step_bounds_f = Rect::from_irect(src_rect);
        if step_bounds_f.is_empty() {
            return FilterResult::default();
        }
        // stepPixelBounds holds integer pixel values (as floats) and includes any padded outsetting
        // that was rendered by the previous step, while stepBoundsF does not have any padding.
        let mut step_pixel_bounds = Rect::from_irect(src_rect);

        // If we made it here, at least one iteration is required, even if xSteps and ySteps are 0.
        let mut image_result = self.clone();
        if !pixel_aligned && (x_steps > 0 || y_steps > 0) {
            // If the source image has a deferred transform with a downscaling factor, we don't
            // want to necessarily compose the first rescale step's transform with it because we
            // will then be missing pixels in the bilinear filtering and create sampling artifacts
            // during animations. NOTE: Force nextSteps counts to the max integer value when the
            // accumulated scale factor is not finite, to force the input image to be resolved.
            let net_scale = map_size(scale, &image_result.transform);
            let next_x_steps = if net_scale.width.is_finite() {
                downscale_step_count(net_scale.width)
            } else {
                i32::MAX
            };
            let next_y_steps = if net_scale.height.is_finite() {
                downscale_step_count(net_scale.height)
            } else {
                i32::MAX
            };
            // We only need to resolve the deferred transform if the rescaling along an axis is not
            // near identity (steps > 0). If it's near identity, there's no real difference in
            // sampling between resolving here and deferring it to the first rescale iteration.
            if (x_steps > 0 && next_x_steps > x_steps) || (y_steps > 0 && next_y_steps > y_steps) {
                // Resolve the deferred transform. We don't just fold the deferred scale factor
                // into the rescaling steps because, for better or worse, the deferred transform
                // does not otherwise participate in progressive scaling so we should be consistent.
                image_result = image_result.resolve(ctx, src_rect, false);
                if !image_result.has_image() {
                    // Early out if the resolve failed
                    return FilterResult::default();
                }
                if !cf_border {
                    // This sets the resolved image to match either kDecal or the deferred tile
                    // mode.
                    image_result.tile_mode = tile_mode;
                } // else leave it as kDecal when cfBorder is true
            }
        }

        let mut allow_overscaling = allow_overscaling;
        if defer_periodic_tiling {
            // The periodic tiling effect will be manually rendered into the lower resolution image
            // so that clamp tiling can be used at each decimation.
            image_result.tile_mode = TileMode::Clamp;
        } else {
            // When not deferring periodic tiling, it provides a better user behavior for animating
            // sigma values and matrix scale factors to not overscale to the next factor of 1/2 and
            // just scale the requisite amount between 1/2 and 1 for the final step.
            allow_overscaling = false;
        }

        // For now, if we are deferring periodic tiling, we need to ensure that the low-res image
        // bounds are pixel aligned. This is because the tiling is applied at the pixel level in
        // SkImageShader, and we need the period of the low-res image to align with the original
        // high-resolution period.
        let final_scale_x = if x_steps > 0 {
            if allow_overscaling {
                1.0f32 / ((1i32 << x_steps) as f32)
            } else {
                scale.width
            }
        } else {
            1.0
        };
        let final_scale_y = if y_steps > 0 {
            if allow_overscaling {
                1.0f32 / ((1i32 << y_steps) as f32)
            } else {
                scale.height
            }
        } else {
            1.0
        };

        loop {
            let mut sx = 1.0f32;
            if x_steps > 0 {
                sx = if x_steps > 1 {
                    0.5
                } else {
                    (src_rect.width() as f32) * final_scale_x / step_bounds_f.width()
                };
                x_steps -= 1;
            }

            let mut sy = 1.0f32;
            if y_steps > 0 {
                sy = if y_steps > 1 {
                    0.5
                } else {
                    (src_rect.height() as f32) * final_scale_y / step_bounds_f.height()
                };
                y_steps -= 1;
            }

            // Downscale relative to the center of the image, which better distributes any sort of
            // sampling errors across the image (vs. emphasizing the bottom right edges).
            let mut dst_bounds_f = scale_about_center(step_bounds_f, sx, sy);
            let final_x_step = x_steps == 0 && sx != 1.0;
            let final_y_step = y_steps == 0 && sy != 1.0;
            if defer_periodic_tiling && (final_x_step || final_y_step) {
                let dst_pixels = round_out(&dst_bounds_f);
                dst_bounds_f = Rect::new(
                    if final_x_step {
                        dst_pixels.left as f32
                    } else {
                        dst_bounds_f.left
                    },
                    if final_y_step {
                        dst_pixels.top as f32
                    } else {
                        dst_bounds_f.top
                    },
                    if final_x_step {
                        dst_pixels.right as f32
                    } else {
                        dst_bounds_f.right
                    },
                    if final_y_step {
                        dst_pixels.bottom as f32
                    } else {
                        dst_bounds_f.bottom
                    },
                );
            }

            // NOTE: Rounding out is overly conservative when dstBoundsF has an odd integer
            // width/height but with coordinates at 1/2. In this case, we could create a pixel grid
            // that has a fractional translation in the final FilterResult but that will best be
            // done when FilterResult tracks floating bounds.
            let dst_pixel_bounds = round_out(&dst_bounds_f);

            let boundary;
            let sample_bounds = dst_pixel_bounds;
            let mut dst_pixel_bounds = dst_pixel_bounds;
            if tile_mode == TileMode::Decal {
                boundary = PixelBoundary::Transparent;
            } else {
                // This is roughly equivalent to using PixelBoundary::kInitialized, but keeps some
                // of the later logic simpler.
                boundary = PixelBoundary::Unknown;
                dst_pixel_bounds = outset_irect(dst_pixel_bounds, 1, 1);
            }

            let mut surface = AutoSurface::new(ctx, dst_pixel_bounds, boundary, false, None);
            if surface.has_canvas() {
                let scale_xform =
                    Matrix::rect_to_rect_or_identity(step_bounds_f, dst_bounds_f, ScaleToFit::Fill);

                // Redo analysis with the actual scale transform and padded low res bounds. With the
                // padding added to dstPixelBounds, intermediate steps should not require shader
                // tiling. Unfortunately, when the last step requires a scale factor other than 1/2,
                // shader based clamping may still be necessary with just a single pixel of padding.
                analysis =
                    image_result.analyze_bounds(&scale_xform, sample_bounds, BoundsScope::Rescale);

                // Primary fill that will cover all of 'sampleBounds'
                let mut paint = Paint::default();
                if let Some(shader) =
                    image_result.get_analyzed_shader_view(ctx, image_result.sampling(), analysis)
                {
                    paint.set_shader(Some(shader));
                }
                paint.set_blend_mode(BlendMode::Src);

                let src_sampled =
                    inverse_map_rect_f(&scale_xform, &Rect::from_irect(sample_bounds))
                        .unwrap_or(Rect::from_irect(sample_bounds));

                if let Some(canvas) = surface.canvas() {
                    canvas.save();
                    canvas.concat(&scale_xform);
                    canvas.draw_rect(src_sampled, &paint);
                    canvas.restore();

                    if cf_border {
                        // Fill in the border with the transparency-affecting color filter, which is
                        // what the image shader's tile mode would have produced anyways but this
                        // avoids triggering shader-based tiling.
                        if let Some(cf) = self.color_filter.clone() {
                            draw_color_filtered_border(canvas, dst_pixel_bounds, cf);
                        }
                        // Clamping logic will preserve its values on subsequent rescale steps.
                        cf_border = false;
                    } else if tile_mode != TileMode::Decal {
                        // Draw the edges of the shader into the padded border, respecting the tile
                        // mode
                        draw_tiled_border(
                            canvas,
                            tile_mode,
                            &paint,
                            &scale_xform,
                            step_pixel_bounds,
                            Rect::from_irect(dst_pixel_bounds),
                        );
                    }
                }
            } else {
                // Rescaling can't complete, no sense in downscaling non-existent data
                return FilterResult::default();
            }

            image_result = surface.snap();
            // If we are deferring periodic tiling, use kClamp on subsequent steps to preserve the
            // border pixels. The original tile mode will be restored at the end.
            image_result.tile_mode = if defer_periodic_tiling {
                TileMode::Clamp
            } else {
                tile_mode
            };

            step_bounds_f = dst_bounds_f;
            step_pixel_bounds = Rect::from_irect(dst_pixel_bounds);

            if !(x_steps > 0 || y_steps > 0) {
                break;
            }
        }

        // Rebuild the downscaled image, including a transform back to the original layer-space
        // resolution, restoring the layer bounds it should fill, and setting tile mode.
        if defer_periodic_tiling {
            // Inset the image to undo the manually added border of pixels, which will allow the
            // result to have the kInitialized boundary state.
            image_result = image_result.inset_by_pixel();
        }
        image_result.tile_mode = tile_mode;
        let rect_to_rect = Matrix::rect_to_rect_or_identity(
            step_bounds_f,
            Rect::from_irect(src_rect),
            ScaleToFit::Fill,
        );
        image_result.transform = Matrix::concat(&rect_to_rect, &image_result.transform);
        image_result.layer_bounds = visible_layer_bounds;
        image_result
    }
}

impl Builder<'_, '_> {
    /// `FilterResult::Builder::blur(sigma)`: blurs the single input with the backend's blur
    /// engine, downscaling first for sigmas beyond the algorithm's maximum.
    // Port of: src/core/SkImageFilterTypes.cpp#L2108-L2237 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors FilterResult::Builder::blur
    #[must_use]
    pub fn blur(&self, sigma: Size) -> FilterResult {
        debug_assert_eq!(self.inputs.len(), 1);

        let backend = self.context.backend();
        let Some(blur_engine) = backend.blur_engine() else {
            return FilterResult::default();
        };
        let Some(algorithm) = blur_engine.find_algorithm(sigma, backend.color_type()) else {
            return FilterResult::default();
        };

        // TODO: De-duplicate this logic between SkBlurImageFilter, here, and skgpu::BlurUtils.
        let radii = size_ceil(Size::new(3.0 * sigma.width, 3.0 * sigma.height));
        let mut max_output = self.inputs[0].image.layer_bounds();
        max_output.outset((radii.width, radii.height));

        let mut output_bounds = self.output_bounds(Some(max_output));
        if output_bounds.is_empty() {
            return FilterResult::default();
        }

        // These are the source pixels that will be read from the input image, which can be
        // calculated internally because the blur's access pattern is well defined (vs. needing it
        // to be provided in Builder::add()).
        let mut sample_bounds = output_bounds;
        sample_bounds.outset((radii.width, radii.height));

        let max_sigma = algorithm.max_sigma();
        let sx = if sigma.width > max_sigma {
            max_sigma / sigma.width
        } else {
            1.0
        };
        let sy = if sigma.height > max_sigma {
            max_sigma / sigma.height
        } else {
            1.0
        };
        // For identity scale factors, this rescale() is a no-op when possible, but otherwise it
        // will also handle resolving any color filters or transform similar to a resolve() except
        // that it can defer the tile mode.
        //
        // Always allow overscaling for higher quality filtering, and because we can adjust the
        // blur sigma applied to the low res image to account for any extra scale factor.
        let low_res_image = self.inputs[0].image.rescale(
            &self.context.with_new_desired_output(sample_bounds),
            Size::new(sx, sy),
            algorithm.supports_only_decal_tiling(),
            true,
        );
        if !low_res_image.has_image() {
            return FilterResult::default();
        }
        debug_assert!(
            low_res_image.tile_mode() == TileMode::Decal || !algorithm.supports_only_decal_tiling()
        );

        // Map 'sigma' into the low-res image's pixel space to determine the low-res blur params to
        // pass into the blur engine. This relies on rescale() producing an image with a
        // scale+translate transform, so it's possible to derive the inverse scale factors directly.
        // We also clamp to be <= maxSigma just in case floating point error made it slightly higher.
        let inv_scale_x = ieee_float_divide(1.0, low_res_image.transform().rc(0, 0));
        let inv_scale_y = ieee_float_divide(1.0, low_res_image.transform().rc(1, 1));
        let low_res_sigma = Size::new(
            (sigma.width * inv_scale_x).min(max_sigma),
            (sigma.height * inv_scale_y).min(max_sigma),
        );
        let Some(low_res_image_ref) = low_res_image.ref_image() else {
            return FilterResult::default();
        };
        let mut low_res_max_output =
            IRect::from_wh(low_res_image_ref.width(), low_res_image_ref.height());

        let mut src_relative_output;
        if low_res_image.tile_mode() == TileMode::Repeat
            || low_res_image.tile_mode() == TileMode::Mirror
        {
            // The periodic tiling was deferred when down-sampling; we can further defer it to after
            // the blur. The low-res output is 1-to-1 with the low res image.
            src_relative_output = low_res_max_output;
        } else {
            // For decal and clamp tiling, the blurred image stops being interesting outside the
            // radii outset, so redo the max output analysis with the 'outputBounds' mapped into
            // pixel space.
            src_relative_output = inverse_map_irect(low_res_image.transform(), &output_bounds)
                .unwrap_or(IRect::new_empty());

            // NOTE: Since 'lowResMaxOutput' is based on the actual image and deferred tiling, this
            // can be smaller than the pessimistic filling for a clamp-tiled blur.
            low_res_max_output = low_res_max_output.with_outset((
                size_ceil(Size::new(
                    3.0 * low_res_sigma.width,
                    3.0 * low_res_sigma.height,
                ))
                .width,
                size_ceil(Size::new(
                    3.0 * low_res_sigma.width,
                    3.0 * low_res_sigma.height,
                ))
                .height,
            ));
            src_relative_output = relevant_subset(
                low_res_max_output,
                src_relative_output,
                low_res_image.tile_mode(),
            );

            // Clamp won't return empty from relevantSubset() and a non-intersecting decal should
            // have been caught earlier.
            if src_relative_output.is_empty() {
                return FilterResult::default();
            }

            // Include 1px of blur output so that it can be sampled during the upscale, which is
            // needed to correctly seam large blurs across crop/raster tiles (crbug.com/1500021).
            src_relative_output.outset((1, 1));
        }

        let mut blur_output_bounds = src_relative_output;
        let mut tile_mode = low_res_image.tile_mode();
        let mut blur_input = low_res_image_ref;
        if !algorithm.supports_only_decal_tiling()
            && low_res_image.can_clamp_to_transparent_boundary(BoundsAnalysis::SIMPLE)
        {
            // Have to manage this manually since the BlurEngine isn't aware of the known pixel
            // padding.
            blur_input = Arc::new(blur_input.make_pixel_outset());
            // This offset() is intentional; `blurOutputBounds` already includes an outset from an
            // earlier modification of `srcRelativeOutput`. This offset is to align the blur
            // algorithm output bounds with the adjusted source image.
            blur_output_bounds.offset((1, 1));
            tile_mode = TileMode::Clamp;
        }

        let input_dims = IRect::from_wh(
            blur_input.dimensions().width,
            blur_input.dimensions().height,
        );
        let Some(blurred) = algorithm.blur(
            low_res_sigma,
            &blur_input,
            input_dims,
            tile_mode,
            blur_output_bounds,
        ) else {
            // The blur output bounds may exceed max texture size even if the source image did not.
            return FilterResult::default();
        };

        let mut result = FilterResult::new(Some(Arc::new(blurred)), src_relative_output.top_left());
        if low_res_image.tile_mode() == TileMode::Clamp
            || low_res_image.tile_mode() == TileMode::Decal
        {
            // Undo the outset padding that was added to srcRelativeOutput before invoking the blur
            result = result.inset_by_pixel();
        }

        result.transform = Matrix::concat(low_res_image.transform(), &result.transform);
        if low_res_image.tile_mode() == TileMode::Decal {
            // Recalculate the output bounds based on the blur output; with rounding the final image
            // may be slightly larger than the original, which would unnecessarily add cropping to
            // the layer bounds. But so long as the `outputBounds` had been constrained by the
            // input's own layer, that crop is unnecessary. The result is still restricted to the
            // desired output bounds, which will induce clipping as needed for a rounded-out image.
            let dims = result
                .ref_image()
                .map_or(ISize::new(0, 0), |img| img.dimensions());
            let mapped = map_irect(&IRect::from_wh(dims.width, dims.height), &result.transform);
            output_bounds = self.output_bounds(Some(mapped));
        }
        result.layer_bounds = output_bounds;
        result.tile_mode = low_res_image.tile_mode();
        result
    }
}

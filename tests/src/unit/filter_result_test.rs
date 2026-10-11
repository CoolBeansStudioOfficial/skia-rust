// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FilterResultTest.cpp (chrome/m156)
//
// The raster-backed suites only. The Ganesh and Graphite variants are not ported: Ganesh is out of
// scope, and the C++ Graphite suite is commented out upstream (TODO(b/274901800)). The C++
// `FilterResultTestAccess` is `skia_rust_core::image_filter_result::test_access`.
//
// skia-rust: the C++ `LayerSpace<T>`/`ParameterSpace<T>` wrappers are not ported (see
// `skia_rust_core::image_filter_types`), so layer-space rectangles are plain `IRect`s. The
// `skiatest::ReporterContext` labels are a single context on the reporter, so a nested label
// replaces its parent. The base64 PNG dump of failing bitmaps (`logBitmaps`) is a debugging aid
// and is replaced by a summary line on stderr.

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas, SrcRectConstraint};
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device;
use skia_rust_core::floating_point::float_ceil2int;
use skia_rust_core::image_filter_result::{
    BoundsAnalysis, FilterResult, PixelBoundary, default_sampling, test_access,
};
use skia_rust_core::image_filter_types::{
    Backend, Context, Mapping, Stats, inverse_map_irect, irect_intersect_in_place, map_irect,
};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_priv;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect, Rect, rect_priv};
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar_is_int;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::image_filter_backend::make_raster_backend;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/FilterResultTest.cpp#L43-L56 (the per-channel tolerances)
/// `kDefaultRGBTolerance`: `8.f / 255.f`.
const DEFAULT_RGB_TOLERANCE: f32 = 8.0 / 255.0;
/// `kAATolerance`: `2.f / 255.f`.
const AA_TOLERANCE: f32 = 2.0 / 255.0;
/// `kDefaultMaxAllowedPercentImageDiff`.
const DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF: f32 = 1.0;
/// `kFuzzyKernel`.
const FUZZY_KERNEL: [[f32; 3]; 3] = [[0.9, 0.9, 0.9], [0.9, 1.0, 0.9], [0.9, 0.9, 0.9]];
/// `kKernelSize / 2`.
const KERNEL_OFFSET: i32 = 1;

/// `kTileModes`.
const TILE_MODES: [TileMode; 4] = [
    TileMode::Clamp,
    TileMode::Repeat,
    TileMode::Mirror,
    TileMode::Decal,
];

/// `kNearlyIdentity`: nearly identity rescales are treated as the identity.
const NEARLY_IDENTITY: Size = Size {
    width: 0.999,
    height: 0.999,
};

/// `LayerSpace<SkIRect>::Empty()`.
fn empty_irect() -> IRect {
    IRect::new_empty()
}

/// `to_string(SkTileMode)`.
fn tile_mode_name(tm: TileMode) -> &'static str {
    match tm {
        TileMode::Clamp => "clamp",
        TileMode::Repeat => "repeat",
        TileMode::Mirror => "mirror",
        TileMode::Decal => "decal",
    }
}

/// `colorfilter_equals`: the two filters are equal if they serialize to the same structure.
// Port of: tests/FilterResultTest.cpp#L195-L205 (chrome/m156)
fn colorfilter_equals(actual: Option<&ColorFilter>, expected: Option<&ColorFilter>) -> bool {
    match (actual, expected) {
        // both null
        (None, None) => true,
        (Some(actual), Some(expected)) => {
            let actual_data = serialize_color_filter(actual);
            let expected_data = serialize_color_filter(expected);
            actual_data.equals(Some(&expected_data))
        }
        _ => false,
    }
}

/// `SkColorFilter::serialize()`.
fn serialize_color_filter(cf: &ColorFilter) -> skia_rust_core::data::Data {
    let mut writer = skia_rust_core::write_buffer::BinaryWriteBuffer::new();
    writer.write_color_filter(Some(cf));
    writer.snapshot_as_data()
}

/// Zeroes the counters of `stats` before an action is evaluated (`Stats stats;` in C++ makes a
/// fresh, zeroed one per action).
fn reset_stats(stats: &Stats) {
    stats.num_visited_image_filters.set(0);
    stats.num_cache_hits.set(0);
    stats.num_offscreen_surfaces.set(0);
    stats.num_shader_clamped_draws.set(0);
    stats.num_shader_based_tiling_draws.set(0);
}

/// `clear_device`: fills the device with transparent black.
// Port of: tests/FilterResultTest.cpp#L207-L213 (chrome/m156)
fn clear_device(device: &mut dyn Device) {
    let mut p = Paint::default();
    p.set_color4f(Color4f::new(0.0, 0.0, 0.0, 0.0), None);
    p.set_blend_mode(BlendMode::Src);
    device.draw_paint(&p);
}

/// `Expect`: what an action's output should be.
// Port of: tests/FilterResultTest.cpp#L215-L219 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // the variants mirror the C++ Expect values (kDeferredImage, ...)
enum Expect {
    /// i.e. modified properties of `FilterResult` instead of rendering
    DeferredImage,
    /// i.e. rendered a new image before modifying other properties
    NewImage,
    /// i.e. everything is transparent black
    EmptyImage,
}

/// The `ShaderSampleMode` of `FilterResultTestAccess`.
// Port of: tests/FilterResultTest.cpp#L110-L114 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShaderSampleMode {
    Fast,
    ShaderClamp,
    ShaderTile,
}

// ---------------------------------------------------------------------------------------------
// FilterResultTestAccess: the test-only helpers on FilterResult.

/// `FilterResultTestAccess::StrictShader`.
// Port of: tests/FilterResultTest.cpp#L65-L84 (chrome/m156)
fn strict_shader(ctx: &Context<'_>, image: &FilterResult) -> Option<Shader> {
    let mut analysis = test_access::analyze_bounds(image, ctx.desired_output());
    if analysis.contains(BoundsAnalysis::REQUIRES_LAYER_CROP) {
        // getAnalyzedShaderView() doesn't include the layer crop, this will be handled by the
        // FilterResultImageResolver.
        None
    } else {
        // Add flags to ensure no deferred effects or clamping logic are optimized away.
        analysis |= BoundsAnalysis::DST_BOUNDS_NOT_COVERED;
        analysis |= BoundsAnalysis::REQUIRES_SHADER_TILING;
        if image.tile_mode() == TileMode::Decal {
            analysis |= BoundsAnalysis::REQUIRES_DECAL_IN_LAYER_SPACE;
        }
        test_access::get_analyzed_shader_view(image, ctx, image.sampling(), analysis)
    }
}

/// `FilterResultTestAccess::DeferredScaleFactors`: the min and max scales of the deferred
/// transform, if it has any.
// Port of: tests/FilterResultTest.cpp#L97-L104 (chrome/m156)
fn deferred_scale_factors(image: &FilterResult) -> Option<(f32, f32)> {
    image.transform().min_max_scales()
}

/// `FilterResultTestAccess::IsIntegerTransform`.
// Port of: tests/FilterResultTest.cpp#L86-L91 (chrome/m156)
fn is_integer_transform(image: &FilterResult) -> bool {
    let m = image.transform();
    m.is_translate() && scalar_is_int(m.translate_x()) && scalar_is_int(m.translate_y())
}

/// `FilterResultTestAccess::GetExpectedShaderSampleMode`.
// Port of: tests/FilterResultTest.cpp#L115-L149 (chrome/m156)
fn get_expected_shader_sample_mode(
    ctx: &Context<'_>,
    image: &FilterResult,
    action_supports_direct_drawing: bool,
) -> ShaderSampleMode {
    if !image.has_image() {
        return ShaderSampleMode::Fast;
    }
    let analysis = test_access::analyze_bounds(image, ctx.desired_output());
    let boundary = test_access::boundary(image);
    let must_fill_decal = image.tile_mode() == TileMode::Decal
        && analysis.contains(BoundsAnalysis::DST_BOUNDS_NOT_COVERED)
        && !action_supports_direct_drawing;
    if analysis.contains(BoundsAnalysis::HAS_LAYER_FILLING_EFFECT) || must_fill_decal {
        // The image won't be drawn directly so some form of shader is needed. The faster clamp
        // can be used when clamping explicitly or decal-with-transparent-padding.
        if image.tile_mode() == TileMode::Clamp
            || (image.tile_mode() == TileMode::Decal && boundary == PixelBoundary::Transparent)
        {
            return ShaderSampleMode::ShaderClamp;
        }
        // These cases should be covered by the more expensive shader tiling.
        return ShaderSampleMode::ShaderTile;
    }
    // If we got here, it will be drawn directly but a clamp can be needed if the data outside the
    // image is unknown and sampling might pull those values in accidentally.
    if boundary == PixelBoundary::Unknown {
        ShaderSampleMode::ShaderClamp
    } else {
        ShaderSampleMode::Fast
    }
}

// ---------------------------------------------------------------------------------------------
// ApplyAction: one action under test, with its expectations.

/// `ApplyAction::TransformParams`, `CropParams`, the color filter and `RescaleParams`.
// Port of: tests/FilterResultTest.cpp#L221-L233 (chrome/m156)
#[derive(Clone)]
enum Action {
    /// For `applyTransform()`.
    Transform {
        matrix: Matrix,
        sampling: SamplingOptions,
    },
    /// For `applyCrop()`. `expected_bounds` is set when the expected bounds are too hard to
    /// compute from the rest of the test case.
    Crop {
        rect: IRect,
        tile_mode: TileMode,
        expected_bounds: Option<IRect>,
    },
    /// For `applyColorFilter()`.
    ColorFilter(ColorFilter),
    /// For `rescale()`.
    Rescale(Size),
}

/// `ApplyAction`.
// Port of: tests/FilterResultTest.cpp#L221-L416 (chrome/m156)
#[derive(Clone)]
struct ApplyAction {
    action: Action,
    expectation: Expect,
    expected_sampling: SamplingOptions,
    expected_tile_mode: TileMode,
    expected_color_filter: Option<ColorFilter>,
}

impl ApplyAction {
    fn transform(
        matrix: Matrix,
        sampling: SamplingOptions,
        expectation: Expect,
        expected_sampling: SamplingOptions,
        expected_tile_mode: TileMode,
        expected_color_filter: Option<ColorFilter>,
    ) -> ApplyAction {
        ApplyAction {
            action: Action::Transform { matrix, sampling },
            expectation,
            expected_sampling,
            expected_tile_mode,
            expected_color_filter,
        }
    }

    fn crop(
        rect: IRect,
        tile_mode: TileMode,
        expected_bounds: Option<IRect>,
        expectation: Expect,
        expected_sampling: SamplingOptions,
        expected_tile_mode: TileMode,
        expected_color_filter: Option<ColorFilter>,
    ) -> ApplyAction {
        ApplyAction {
            action: Action::Crop {
                rect,
                tile_mode,
                expected_bounds,
            },
            expectation,
            expected_sampling,
            expected_tile_mode,
            expected_color_filter,
        }
    }

    fn color_filter(
        color_filter: ColorFilter,
        expectation: Expect,
        expected_sampling: SamplingOptions,
        expected_tile_mode: TileMode,
        expected_color_filter: Option<ColorFilter>,
    ) -> ApplyAction {
        ApplyAction {
            action: Action::ColorFilter(color_filter),
            expectation,
            expected_sampling,
            expected_tile_mode,
            expected_color_filter,
        }
    }

    fn rescale(
        scale: Size,
        expectation: Expect,
        expected_sampling: SamplingOptions,
        expected_tile_mode: TileMode,
        expected_color_filter: Option<ColorFilter>,
    ) -> ApplyAction {
        ApplyAction {
            action: Action::Rescale(scale),
            expectation,
            expected_sampling,
            expected_tile_mode,
            expected_color_filter,
        }
    }

    // Test-simplified logic for bounds propagation similar to how image filters calculate bounds
    // while evaluating a filter DAG, which is outside of skif::FilterResult's responsibilities.
    // Port of: tests/FilterResultTest.cpp#L238-L258 (chrome/m156)
    fn required_input(&self, desired_output: IRect) -> IRect {
        match &self.action {
            Action::Transform { matrix, .. } => {
                inverse_map_irect(matrix, &desired_output).unwrap_or_else(empty_irect)
            }
            Action::Crop {
                rect, tile_mode, ..
            } => {
                let mut intersection = *rect;
                if *tile_mode == TileMode::Decal
                    && !irect_intersect_in_place(&mut intersection, &desired_output)
                {
                    intersection = empty_irect();
                }
                intersection
            }
            Action::ColorFilter(_) | Action::Rescale(_) => desired_output,
        }
    }

    // Performs the action to be tested.
    // Port of: tests/FilterResultTest.cpp#L260-L272 (chrome/m156)
    fn apply(&self, ctx: &Context<'_>, input: &FilterResult) -> FilterResult {
        match &self.action {
            Action::Transform { matrix, sampling } => input.apply_transform(ctx, matrix, *sampling),
            Action::Crop {
                rect, tile_mode, ..
            } => input.apply_crop(ctx, *rect, *tile_mode),
            Action::ColorFilter(cf) => input.apply_color_filter(ctx, cf.clone()),
            Action::Rescale(scale) => input.rescale(ctx, *scale, false, false),
        }
    }

    fn expectation(&self) -> Expect {
        self.expectation
    }

    fn expected_sampling(&self) -> SamplingOptions {
        self.expected_sampling
    }

    fn expected_tile_mode(&self) -> TileMode {
        self.expected_tile_mode
    }

    fn expected_color_filter(&self) -> Option<&ColorFilter> {
        self.expected_color_filter.as_ref()
    }

    // Port of: tests/FilterResultTest.cpp#L274-L310 (chrome/m156)
    fn expected_offscreen_surfaces(&self, source: &FilterResult) -> Vec<i32> {
        if self.expectation != Expect::NewImage {
            return vec![0];
        }
        if let Action::Rescale(scale) = &self.action {
            let mut min_scale = scale.width.min(scale.height);
            if min_scale >= 1.0 - 0.001 {
                vec![1]
            } else {
                let deferred_scale = deferred_scale_factors(source);
                let mut steps = 0;
                if deferred_scale.is_some_and(|(min, _)| min <= 0.9) {
                    steps += 1;
                }
                loop {
                    steps += 1;
                    min_scale *= 2.0;
                    if min_scale >= 0.9 {
                        break;
                    }
                }

                // Rescaling periodic tiling may require scaling further than the value stored in
                // the action to hit pixel integer bounds, which may trigger one more pass.
                let src_tile_mode = source.tile_mode();
                if src_tile_mode == TileMode::Repeat || src_tile_mode == TileMode::Mirror {
                    vec![steps, steps + 1]
                } else {
                    vec![steps]
                }
            }
        } else {
            vec![1]
        }
    }

    // Port of: tests/FilterResultTest.cpp#L312-L329 (chrome/m156)
    fn expected_sample_mode(&self, ctx: &Context<'_>, source: &FilterResult) -> ShaderSampleMode {
        let mut action_supports_direct_drawing = true;
        if matches!(self.action, Action::Rescale(_)) {
            // rescale() normally does not draw directly; the exception is if the source image has
            // a scale factor that requires a pre-resolve. If that happens 'source' is not really
            // the source of the rescale steps, and `source` can be drawn directly by the resolve.
            match deferred_scale_factors(source) {
                Some((first, _)) if first <= 0.5 => {}
                _ => action_supports_direct_drawing = false, // no pre-resolve
            }
        }
        get_expected_shader_sample_mode(ctx, source, action_supports_direct_drawing)
    }

    // Port of: tests/FilterResultTest.cpp#L331-L366 (chrome/m156)
    fn expected_bounds(&self, input_bounds: IRect) -> IRect {
        // This assumes anything outside 'inputBounds' is transparent black.
        match &self.action {
            Action::Transform { matrix, .. } => {
                if input_bounds.is_empty() {
                    return empty_irect();
                }
                map_irect(&input_bounds, matrix)
            }
            Action::Crop {
                rect,
                tile_mode,
                expected_bounds,
            } => {
                if let Some(bounds) = expected_bounds {
                    return *bounds;
                }
                let mut intersection = *rect;
                if !irect_intersect_in_place(&mut intersection, &input_bounds) {
                    return empty_irect();
                }
                if *tile_mode == TileMode::Decal {
                    intersection
                } else {
                    rect_priv::make_i_large()
                }
            }
            Action::ColorFilter(cf) => {
                if cf.as_base().affects_transparent_black() {
                    // Fills out infinitely
                    rect_priv::make_i_large()
                } else {
                    input_bounds
                }
            }
            Action::Rescale(_) => input_bounds,
        }
    }

    // Port of: tests/FilterResultTest.cpp#L368-L477 (chrome/m156)
    #[allow(clippy::cast_precision_loss, clippy::float_cmp, clippy::too_many_lines)] // the C++ body is kept whole so it can be compared line by line; mirrors the C++ int-to-float conversions; the C++ compares the scales exactly
    fn render_expected_image(
        &self,
        ctx: &Context<'_>,
        source: Option<Arc<SpecialImage>>,
        origin: IPoint,
        desired_output: IRect,
    ) -> Option<Arc<SpecialImage>> {
        let mut effective_expectation = self.expectation;
        let mut size = ISize::new(desired_output.width(), desired_output.height());
        if desired_output.is_empty() {
            size = ISize::new(1, 1);
            effective_expectation = Expect::EmptyImage;
        }

        let device = ctx
            .backend()
            .make_device(size, ctx.color_space().cloned(), None)?;
        let canvas = Canvas::from_device(device);
        canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
        canvas.translate((
            -(desired_output.left() as f32),
            -(desired_output.top() as f32),
        ));

        let mut source = source;
        let mut origin = origin;
        if effective_expectation != Expect::EmptyImage {
            let source_image = source.clone().expect("SkASSERT(source)");
            let source_bounds = IRect::from_xywh(
                origin.x,
                origin.y,
                source_image.width(),
                source_image.height(),
            );
            let expected_bounds = self.expected_bounds(source_bounds);

            canvas.clip_irect(expected_bounds, None);

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_blend_mode(BlendMode::Src);
            // Start with NN to match exact subsetting FilterResult does for deferred images
            let mut sampling = SamplingOptions::default();
            let mut tile_mode = TileMode::Decal;
            match &self.action {
                Action::Transform {
                    matrix,
                    sampling: action_sampling,
                } => {
                    // FilterResult treats default/bilerp filtering as NN when it has an integer
                    // translation, so only change 'sampling' when that is not the case.
                    if !matrix.is_translate()
                        || !scalar_is_int(matrix.translate_x())
                        || !scalar_is_int(matrix.translate_y())
                    {
                        sampling = *action_sampling;
                    }
                    canvas.concat(matrix);
                }
                Action::Crop {
                    rect,
                    tile_mode: crop_tile_mode,
                    ..
                } => {
                    let mut image_bounds = IRect::from_xywh(
                        origin.x,
                        origin.y,
                        source_image.width(),
                        source_image.height(),
                    );
                    if *crop_tile_mode == TileMode::Decal || image_bounds.contains(*rect) {
                        // Extract a subset of the image
                        let ok = irect_intersect_in_place(&mut image_bounds, rect);
                        assert!(ok, "SkAssertResult(imageBounds.intersect(c->fRect))");
                        source = Some(Arc::new(
                            source_image
                                .make_subset(&IRect::new(
                                    image_bounds.left() - origin.x,
                                    image_bounds.top() - origin.y,
                                    image_bounds.right() - origin.x,
                                    image_bounds.bottom() - origin.y,
                                ))
                                .expect("makeSubset"),
                        ));
                        origin = image_bounds.top_left();
                    } else {
                        // A non-decal tile mode where the image doesn't cover the crop requires
                        // the image to be padded out with transparency so the tiling matches
                        // 'fRect'.
                        let padded_size = ISize::new(rect.width(), rect.height());
                        let padded_device = ctx
                            .backend()
                            .make_device(padded_size, ctx.color_space().cloned(), None)
                            .expect("makeDevice");
                        let padded_canvas = Canvas::from_device(padded_device);
                        padded_canvas.with_top_device(clear_device);
                        padded_canvas.with_top_device(|d| {
                            d.draw_special(
                                &source_image,
                                &Matrix::translate((
                                    (origin.x - rect.left()) as f32,
                                    (origin.y - rect.top()) as f32,
                                )),
                                &SamplingOptions::default(),
                                &Paint::default(),
                                SrcRectConstraint::Strict,
                            );
                        });
                        source = Some(Arc::new(
                            padded_canvas
                                .with_top_device(|d| {
                                    d.snap_special(&IRect::from_size(padded_size), false)
                                })
                                .expect("snapSpecial"),
                        ));
                        origin = rect.top_left();
                    }
                    tile_mode = *crop_tile_mode;
                }
                Action::ColorFilter(cf) => {
                    paint.set_color_filter(Some(cf.clone()));
                }
                Action::Rescale(scale) => {
                    // Don't redraw with an identity scale since sampling errors creep in on some
                    // GPUs
                    if scale.width != 1.0 || scale.height != 1.0 {
                        let orig_src_width = source_image.width();
                        let orig_src_height = source_image.height();
                        let low_res_size = ISize::new(
                            float_ceil2int(orig_src_width as f32 * scale.width),
                            float_ceil2int(orig_src_height as f32 * scale.height),
                        );

                        let mut step_source = source_image.clone();
                        while step_source.width() != low_res_size.width
                            || step_source.height() != low_res_size.height
                        {
                            let sx =
                                0.5f32.max(low_res_size.width as f32 / step_source.width() as f32);
                            let sy = 0.5f32
                                .max(low_res_size.height as f32 / step_source.height() as f32);
                            let step_size = ISize::new(
                                float_ceil2int(step_source.width() as f32 * sx),
                                float_ceil2int(step_source.height() as f32 * sy),
                            );
                            let step_canvas = Canvas::from_device(
                                ctx.backend()
                                    .make_device(step_size, ctx.color_space().cloned(), None)
                                    .expect("makeDevice"),
                            );
                            step_canvas.with_top_device(clear_device);
                            let scale_matrix = Matrix::scale((sx, sy));
                            step_canvas.with_top_device(|d| {
                                d.draw_special(
                                    &step_source,
                                    &scale_matrix,
                                    &SamplingOptions::from(FilterMode::Linear),
                                    &Paint::default(),
                                    SrcRectConstraint::Strict,
                                );
                            });
                            step_source = Arc::new(
                                step_canvas
                                    .with_top_device(|d| {
                                        d.snap_special(&IRect::from_size(step_size), false)
                                    })
                                    .expect("snapSpecial"),
                            );
                        }
                        source = Some(step_source);

                        // Adjust to draw the low-res image upscaled to fill the original image
                        // bounds
                        sampling = SamplingOptions::from(FilterMode::Linear);
                        tile_mode = TileMode::Clamp;
                        canvas.translate((origin.x as f32, origin.y as f32));
                        let src = source.as_ref().expect("source");
                        canvas.scale((
                            orig_src_width as f32 / src.width() as f32,
                            orig_src_height as f32 / src.height() as f32,
                        ));
                        origin = IPoint::new(0, 0);
                    }
                    // else it's a rescale action, but for the expected image leave it unmodified.
                }
            }
            let src = source.expect("source");
            let mut paint = paint;
            paint.set_shader(src.as_shader(
                tile_mode,
                sampling,
                &Matrix::translate((origin.x as f32, origin.y as f32)),
                true,
            ));
            canvas.draw_paint(&paint);
        }
        let snap_size = IRect::from_size(size);
        canvas
            .with_top_device(|d| d.snap_special(&snap_size, false))
            .map(Arc::new)
    }
}

// ---------------------------------------------------------------------------------------------
// FilterResultImageResolver: resolves a FilterResult to pixels in one of several ways.

/// `FilterResultImageResolver::Method`.
// Port of: tests/FilterResultTest.cpp#L421-L428 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ResolveMethod {
    ImageAndOffset,
    DrawToCanvas,
    Shader,
    ClippedShader,
    /// Only used to check image correctness when stats reported an optimization.
    StrictShader,
}

impl ResolveMethod {
    // Port of: tests/FilterResultTest.cpp#L435-L443 (chrome/m156)
    fn method_name(self) -> &'static str {
        match self {
            ResolveMethod::ImageAndOffset => "imageAndOffset",
            ResolveMethod::DrawToCanvas => "drawToCanvas",
            ResolveMethod::Shader => "asShader",
            ResolveMethod::ClippedShader => "asShaderClipped",
            ResolveMethod::StrictShader => "strictShader",
        }
    }

    // Port of: tests/FilterResultTest.cpp#L445-L503 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-float conversions
    fn resolve(
        self,
        ctx: &Context<'_>,
        image: &FilterResult,
    ) -> (Option<Arc<SpecialImage>>, IPoint) {
        if self == ResolveMethod::ImageAndOffset {
            let (resolved, origin) = image.image_and_offset(ctx);
            return (resolved, origin);
        }
        let desired = ctx.desired_output();
        if desired.is_empty() {
            return (None, IPoint::new(0, 0));
        }

        let device = ctx
            .backend()
            .make_device(
                ISize::new(desired.width(), desired.height()),
                ctx.color_space().cloned(),
                None,
            )
            .expect("SkASSERT(device)");
        let canvas = Canvas::from_device(device);
        canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
        canvas.translate((-(desired.left() as f32), -(desired.top() as f32)));

        if self > ResolveMethod::DrawToCanvas {
            let shader = if self == ResolveMethod::Shader {
                // asShader() applies layer bounds by resolving automatically (e.g.
                // kDrawToCanvas), if sampleBounds is larger than the layer bounds. Since we want
                // to test the unclipped shader version, pass in layerBounds() for sampleBounds
                // and add a clip to the canvas instead.
                canvas.clip_irect(image.layer_bounds(), None);
                test_access::as_shader(image, ctx, image.layer_bounds())
            } else if self == ResolveMethod::ClippedShader {
                test_access::as_shader(image, ctx, desired)
            } else {
                strict_shader(ctx, image).or_else(|| {
                    let (pixels, origin) =
                        self.resolve(&ctx.with_new_desired_output(image.layer_bounds()), image);
                    strict_shader(ctx, &FilterResult::new(pixels, origin))
                })
            };

            let mut paint = Paint::default();
            if let Some(shader) = shader {
                paint.set_shader(Some(shader));
            }
            canvas.draw_paint(&paint);
        } else {
            debug_assert_eq!(self, ResolveMethod::DrawToCanvas);
            canvas.with_top_device(|d| test_access::draw(image, ctx, d, false));
        }

        let snap = canvas.with_top_device(|d| {
            d.snap_special(&IRect::from_wh(desired.width(), desired.height()), false)
        });
        (
            snap.map(Arc::new),
            IPoint::new(desired.left(), desired.top()),
        )
    }
}

// ---------------------------------------------------------------------------------------------
// TestRunner: owns the backend and compares the actual results with expected images.

/// `colors` of `createSourceImage`: magenta, red, yellow, green, cyan and blue (`SkColor`s).
const SOURCE_COLORS: [u32; 6] = [
    0xFFFF_00FF, // SK_ColorMAGENTA
    0xFFFF_0000, // SK_ColorRED
    0xFFFF_FF00, // SK_ColorYELLOW
    0xFF00_FF00, // SK_ColorGREEN
    0xFF00_FFFF, // SK_ColorCYAN
    0xFF00_00FF, // SK_ColorBLUE
];

/// `TestRunner`: a raster-backed runner for the test cases.
// Port of: tests/FilterResultTest.cpp#L506-L590 (chrome/m156)
struct TestRunner<'r> {
    reporter: &'r mut Reporter,
    backend: Arc<dyn Backend>,
    logged_error_image: bool,
}

impl<'r> TestRunner<'r> {
    /// The raster-backed runner (`TestRunner(skiatest::Reporter*)`).
    // Port of: tests/FilterResultTest.cpp#L521-L523 (chrome/m156)
    pub fn new(reporter: &'r mut Reporter) -> Self {
        Self {
            reporter,
            backend: make_raster_backend(SurfaceProps::default(), ColorType::RGBA8888),
            logged_error_image: false,
        }
    }

    /// `ReporterContext`: the label prefixed to later failures.
    fn set_label(&mut self, label: impl Into<String>) {
        self.reporter.set_context(Some(label.into()));
    }

    /// `refBackend()`.
    fn ref_backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }

    /// `backend()`.
    fn backend(&self) -> &dyn Backend {
        self.backend.as_ref()
    }

    /// `compareImages(ctx, expectedImage, expectedOrigin, actual, ...)`: resolves `actual` with
    /// every method and compares each with the expected image.
    // Port of: tests/FilterResultTest.cpp#L592-L618 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn compare_special_images(
        &mut self,
        ctx: &Context<'_>,
        expected_image: Option<&SpecialImage>,
        expected_origin: IPoint,
        actual: &FilterResult,
        allowed_percent_image_diff: f32,
        allowed_rgb_tolerance: f32,
        transparent_check_border_padding: i32,
    ) -> bool {
        let Some(expected_image) = expected_image else {
            // For pathological desired outputs, we can't actually produce an expected image so
            // just carry on w/o validating.
            return true;
        };

        let expected_bm = read_pixels(Some(expected_image));

        let mut compare = |m: ResolveMethod| {
            self.compare_images(
                ctx,
                &expected_bm,
                expected_origin,
                actual,
                m,
                allowed_percent_image_diff,
                allowed_rgb_tolerance,
                transparent_check_border_padding,
            )
        };

        // Resolve actual using all 4 methods to ensure they are approximately equal to the
        // expected (which is used as a proxy for being approximately equal to each other).
        compare(ResolveMethod::ImageAndOffset)
            && compare(ResolveMethod::DrawToCanvas)
            && compare(ResolveMethod::Shader)
            && compare(ResolveMethod::ClippedShader)
    }

    /// `validateOptimizedImage`: a stats-optimized result must match the unoptimized strict
    /// resolve exactly within the default tolerance.
    // Port of: tests/FilterResultTest.cpp#L620-L632 (chrome/m156)
    fn validate_optimized_image(&mut self, ctx: &Context<'_>, actual: &FilterResult) -> bool {
        let (expected_image, expected_origin) = ResolveMethod::StrictShader.resolve(ctx, actual);
        let expected_bm = read_pixels(expected_image.as_deref());
        self.compare_images(
            ctx,
            &expected_bm,
            expected_origin,
            actual,
            ResolveMethod::ImageAndOffset,
            // allowedPercentImageDiff
            0.0,
            DEFAULT_RGB_TOLERANCE,
            // transparentCheckBorderPadding
            0,
        )
    }

    /// `createSourceImage`: a rotated checkerboard of `size` in `color_space`.
    // Port of: tests/FilterResultTest.cpp#L634-L663 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-float conversions
    fn create_source_image(&self, size: ISize, color_space: ColorSpace) -> Arc<SpecialImage> {
        let source_surface = self
            .backend
            .make_device(size, Some(color_space), None)
            .expect("makeDevice");

        let rotation =
            Matrix::rotate_deg_pivot(15.0, (size.width as f32 / 2.0, size.height as f32 / 2.0));

        let canvas = Canvas::from_device(source_surface);
        canvas.clear(Color::new(0xFF00_0000)); // SK_ColorBLACK
        canvas.concat(&rotation);

        let mut color = 0usize;
        let dst_bounds = Rect::from_wh(size.width as f32, size.height as f32);
        let cover_bounds = matrix_priv::inverse_map_rect(&rotation, &dst_bounds)
            .expect("SkAssertResult(SkMatrixPriv::InverseMapRect)");

        let sz: f32 = if size.width as f32 <= 16.0 || size.height as f32 <= 16.0 {
            2.0
        } else {
            8.0
        };
        let mut y = cover_bounds.top();
        while y < cover_bounds.bottom() {
            let mut x = cover_bounds.left();
            while x < cover_bounds.right() {
                let mut p = Paint::default();
                p.set_color(Color::new(SOURCE_COLORS[color % SOURCE_COLORS.len()]));
                color += 1;
                canvas.draw_rect(Rect::from_xywh(x, y, sz, sz), &p);
                x += sz;
            }
            y += sz;
        }

        canvas
            .with_top_device(|d| d.snap_special(&IRect::from_size(size), false))
            .map(Arc::new)
            .expect("snapSpecial")
    }

    /// `compareImages(ctx, SkBitmap expected, ...)`: resolves `actual` with `method` and compares.
    // Port of: tests/FilterResultTest.cpp#L665-L684 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn compare_images(
        &mut self,
        ctx: &Context<'_>,
        expected: &Bitmap,
        expected_origin: IPoint,
        actual: &FilterResult,
        method: ResolveMethod,
        allowed_percent_image_diff: f32,
        allowed_rgb_tolerance: f32,
        transparent_check_border_padding: i32,
    ) -> bool {
        let (actual_image, actual_origin) = method.resolve(ctx, actual);

        // empty if actualImage is null
        let actual_bm = read_pixels(actual_image.as_deref());
        let mut bad_pixels: Vec<IPoint> = Vec::new();
        if !self.compare_bitmaps(
            expected,
            expected_origin,
            &actual_bm,
            actual_origin,
            allowed_percent_image_diff,
            allowed_rgb_tolerance,
            transparent_check_border_padding,
            &mut bad_pixels,
        ) {
            if !self.logged_error_image {
                eprintln!(
                    "FilterResult comparison failed for method {}",
                    method.method_name()
                );
                // skia-rust: logBitmaps' base64 PNG dump is replaced by a summary (see the module
                // comment).
                eprintln!(
                    "expected {}x{} at {:?}, actual {}x{} at {:?}, {} bad pixels",
                    expected.width(),
                    expected.height(),
                    (expected_origin.x, expected_origin.y),
                    actual_bm.width(),
                    actual_bm.height(),
                    (actual_origin.x, actual_origin.y),
                    bad_pixels.len()
                );
                self.logged_error_image = true;
            }
            return false;
        }
        true
    }

    /// `compareBitmaps`: the actual pixels match the expected within the tolerances, and the
    /// expected pixels outside the actual bounds are transparent black.
    // Port of: tests/FilterResultTest.cpp#L686-L791 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // the C++ body is kept whole so it can be compared line by line; mirrors the C++ int-to-float conversions
    fn compare_bitmaps(
        &mut self,
        expected: &Bitmap,
        expected_origin: IPoint,
        actual: &Bitmap,
        actual_origin: IPoint,
        allowed_percent_image_diff: f32,
        allowed_rgb_tolerance: f32,
        transparent_check_border_padding: i32,
        bad_pixels: &mut Vec<IPoint>,
    ) -> bool {
        // region in expectedBM that can be non-transparent
        let mut exclude_transparent_check: IRect;
        if actual.width() == 0 || actual.height() == 0 {
            // A null image in a FilterResult is equivalent to transparent black, so we should
            // expect the contents of 'expectedImage' to be transparent black.
            exclude_transparent_check = IRect::new_empty();
        } else {
            // The actual image bounds should be contained in the expected image's bounds.
            let actual_bounds = IRect::from_xywh(
                actual_origin.x,
                actual_origin.y,
                actual.width(),
                actual.height(),
            );
            let expected_bounds = IRect::from_xywh(
                expected_origin.x,
                expected_origin.y,
                expected.width(),
                expected.height(),
            );
            let contained = expected_bounds.contains(actual_bounds);
            reporter_assert!(
                self.reporter,
                contained,
                "actual image [{} {} {} {}] not contained within expected [{} {} {} {}]",
                actual_bounds.left(),
                actual_bounds.top(),
                actual_bounds.right(),
                actual_bounds.bottom(),
                expected_bounds.left(),
                expected_bounds.top(),
                expected_bounds.right(),
                expected_bounds.bottom()
            );
            if !contained {
                return false;
            }

            // The actual pixels should match fairly closely with the expected, allowing for minor
            // differences from consolidating actions into a single render, etc.
            let mut error_count: i32 = 0;
            let mut error_delta: f32 = 0.0;
            let offset = IPoint::new(
                actual_origin.x - expected_origin.x,
                actual_origin.y - expected_origin.y,
            );
            for y in 0..actual.height() {
                for x in 0..actual.width() {
                    let ep = IPoint::new(x + offset.x, y + offset.y);
                    let expected_color = expected.get_color_4f(ep);
                    let actual_color = actual.get_color_4f(IPoint::new(x, y));
                    if actual_color != expected_color {
                        let delta = approx_color_delta(
                            box_filter(actual, x, y),
                            box_filter(expected, ep.x, ep.y),
                        );
                        if delta > allowed_rgb_tolerance {
                            error_delta += delta;
                            bad_pixels.push(ep);
                            error_count += 1;
                        }
                    }
                }
            }

            let total_count = expected.width() * expected.height();
            let percent_error = 100.0 * error_count as f32 / total_count as f32;
            let approx_match = percent_error <= allowed_percent_image_diff;

            reporter_assert!(
                self.reporter,
                approx_match,
                "{} pixels were too different from {} total ({} % vs. {} %), average delta {} (vs. {} allowed)",
                error_count,
                total_count,
                percent_error,
                allowed_percent_image_diff,
                error_delta / error_count as f32,
                allowed_rgb_tolerance
            );
            if !approx_match {
                return false;
            }

            // The expected pixels outside of the actual bounds should be transparent, otherwise
            // the actual image is not returning enough data.
            exclude_transparent_check =
                actual_bounds.with_offset(IPoint::new(-expected_origin.x, -expected_origin.y));
            // Add per-test padding to the exclusion, which is used when there is upscaling in the
            // expected image that bleeds beyond the layer bounds, but is hard to enforce in the
            // simplified expectation rendering.
            exclude_transparent_check.outset((
                transparent_check_border_padding,
                transparent_check_border_padding,
            ));
        }

        let mut bad_transparency_count = 0;
        for y in 0..expected.height() {
            for x in 0..expected.width() {
                if !exclude_transparent_check.is_empty()
                    && exclude_transparent_check.contains(IPoint::new(x, y))
                {
                    continue;
                }

                // If we are on the edge of the transparency exclusion bounds, allow pixels to be
                // up to 2 off to account for sloppy GPU rendering (seen on some Android devices).
                // This is still visually "transparent" and definitely make sure that
                // off-transparency does not extend across the entire surface (tolerance = 0).
                let on_edge = !exclude_transparent_check.is_empty() && {
                    let mut outset = exclude_transparent_check;
                    outset.outset((1, 1));
                    outset.contains(IPoint::new(x, y))
                };
                let delta = approx_color_delta(
                    expected.get_color_4f(IPoint::new(x, y)),
                    Color4f::new(0.0, 0.0, 0.0, 0.0),
                );
                if delta > if on_edge { AA_TOLERANCE } else { 0.0 } {
                    bad_pixels.push(IPoint::new(x, y));
                    bad_transparency_count += 1;
                }
            }
        }

        reporter_assert!(
            self.reporter,
            bad_transparency_count == 0,
            "Unexpected non-transparent pixels"
        );
        bad_transparency_count == 0
    }
}

impl std::fmt::Debug for TestRunner<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestRunner")
            .field("logged_error_image", &self.logged_error_image)
            .finish_non_exhaustive()
    }
}

/// `approxColorDelta`: the red-mean approximation of the color difference.
// Port of: tests/FilterResultTest.cpp#L793-L806 (chrome/m156)
#[allow(clippy::manual_midpoint)] // mirrors the C++ (a + b) / 2.f exactly
fn approx_color_delta(a: Color4f, b: Color4f) -> f32 {
    let apm = a.premul();
    let bpm = b.premul();
    // Calculate red-mean, a lowcost approximation of color difference that gives reasonable
    // results for the types of acceptable differences resulting from collapsing compatible
    // SkSamplingOptions or slightly different AA on shape boundaries.
    // See https://www.compuphase.com/cmetric.htm
    let r = (apm.r + bpm.r) / 2.0;
    let dr = apm.r - bpm.r;
    let dg = apm.g - bpm.g;
    let db = apm.b - bpm.b;
    ((2.0 + r) * dr * dr + 4.0 * dg * dg + (2.0 + (1.0 - r)) * db * db).sqrt()
}

/// `boxFilter`: the fuzzy-kernel average of the pixels around `(x, y)`.
// Port of: tests/FilterResultTest.cpp#L808-L832 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // the indices are within the 3x3 kernel, as in the C++
fn box_filter(bm: &Bitmap, x: i32, y: i32) -> Color4f {
    let mut sum = skia_rust_core::color::PMColor4f {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    let mut net_weight = 0.0f32;
    for sy in (y - KERNEL_OFFSET)..=(y + KERNEL_OFFSET) {
        for sx in (x - KERNEL_OFFSET)..=(x + KERNEL_OFFSET) {
            let weight =
                FUZZY_KERNEL[(sy - y + KERNEL_OFFSET) as usize][(sx - x + KERNEL_OFFSET) as usize];

            if sx < 0 || sx >= bm.width() || sy < 0 || sy >= bm.height() {
                // Treat outside image as transparent black, this is necessary to get consistent
                // comparisons between expected and actual images where the actual is cropped as
                // tightly as possible.
                net_weight += weight;
                continue;
            }

            let c = bm.get_color_4f(IPoint::new(sx, sy)).premul();
            sum.r += c.r * weight;
            sum.g += c.g * weight;
            sum.b += c.b * weight;
            sum.a += c.a * weight;
            net_weight += weight;
        }
    }
    debug_assert!(net_weight > 0.0);
    let unpremul = sum.unpremul();
    let scale = 1.0 / net_weight;
    Color4f::new(
        unpremul.r * scale,
        unpremul.g * scale,
        unpremul.b * scale,
        unpremul.a * scale,
    )
}

/// `readPixels`: the pixels of a special image (an empty bitmap for no image).
// Port of: tests/FilterResultTest.cpp#L834-L857 (chrome/m156)
fn read_pixels(special_image: Option<&SpecialImage>) -> Bitmap {
    let Some(special_image) = special_image else {
        return Bitmap::new(); // an empty bitmap
    };
    special_image
        .as_bitmap()
        .expect("SkSpecialImages::AsBitmap")
}

/// `TestCase`: a source, a chain of actions, and their expectations.
// Port of: tests/FilterResultTest.cpp#L859-L1078 (chrome/m156)
struct TestCase<'r, 'rep> {
    runner: &'r mut TestRunner<'rep>,
    name: String,
    allowed_percent_image_diff: f32,
    allowed_rgb_tolerance: f32,
    transparent_check_border_padding: i32,
    /// Used to construct an `SkSpecialImage` of the given size/location filled with the known
    /// pattern.
    source_bounds: IRect,
    actions: Vec<ApplyAction>,
}

impl<'r, 'rep> TestCase<'r, 'rep> {
    /// `TestCase(runner, name)` with the default tolerances.
    fn new(runner: &'r mut TestRunner<'rep>, name: &str) -> Self {
        Self::with_tolerances(
            runner,
            name,
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            0,
        )
    }

    /// `TestCase(runner, name, allowedPercentImageDiff, allowedRGBTolerance,
    /// transparentCheckBorderPadding)`.
    fn with_tolerances(
        runner: &'r mut TestRunner<'rep>,
        name: &str,
        allowed_percent_image_diff: f32,
        allowed_rgb_tolerance: f32,
        transparent_check_border_padding: i32,
    ) -> Self {
        TestCase {
            runner,
            name: name.to_owned(),
            allowed_percent_image_diff,
            allowed_rgb_tolerance,
            transparent_check_border_padding,
            source_bounds: empty_irect(),
            actions: Vec::new(),
        }
    }

    fn source(mut self, bounds: IRect) -> Self {
        self.source_bounds = bounds;
        self
    }

    /// `applyCrop(crop, expectation)`: the crop is decal.
    fn apply_crop_decal(self, crop: IRect, expectation: Expect) -> Self {
        self.apply_crop(crop, TileMode::Decal, expectation)
    }

    /// `applyCrop(crop, tileMode, expectation)` with the expectations filled in automatically.
    fn apply_crop(self, crop: IRect, tile_mode: TileMode, expectation: Expect) -> Self {
        self.apply_crop_expecting(crop, tile_mode, expectation, None, None)
    }

    /// `applyCrop(crop, tileMode, expectation, expectedTileMode, expectedBounds)`.
    fn apply_crop_expecting(
        mut self,
        crop: IRect,
        tile_mode: TileMode,
        expectation: Expect,
        expected_tile_mode: Option<TileMode>,
        expected_bounds: Option<IRect>,
    ) -> Self {
        // Fill-in automated expectations, which is to equal 'tileMode' when not overridden.
        let expected_tile_mode = expected_tile_mode.unwrap_or(tile_mode);
        let action = ApplyAction::crop(
            crop,
            tile_mode,
            expected_bounds,
            expectation,
            self.default_expected_sampling(expectation),
            expected_tile_mode,
            self.default_expected_color_filter(expectation),
        );
        self.actions.push(action);
        self
    }

    /// `applyTransform(matrix, expectation)`: the default sampling.
    fn apply_transform_default(self, matrix: Matrix, expectation: Expect) -> Self {
        self.apply_transform(matrix, default_sampling(), expectation)
    }

    /// `applyTransform(matrix, sampling, expectation)`.
    fn apply_transform(
        self,
        matrix: Matrix,
        sampling: impl Into<SamplingOptions>,
        expectation: Expect,
    ) -> Self {
        self.apply_transform_expecting(matrix, sampling, expectation, None)
    }

    /// `applyTransform(matrix, sampling, expectation, expectedSampling)`.
    fn apply_transform_expecting(
        mut self,
        matrix: Matrix,
        sampling: impl Into<SamplingOptions>,
        expectation: Expect,
        expected_sampling: Option<SamplingOptions>,
    ) -> Self {
        let sampling = sampling.into();
        // Fill-in automated expectations, which is simply that if it's not explicitly provided we
        // assume the result's sampling equals what was passed to applyTransform().
        let expected_sampling = expected_sampling.unwrap_or(sampling);
        let expected_tile_mode = self.default_expected_tile_mode(expectation, false);
        let expected_cf = self.default_expected_color_filter(expectation);
        self.actions.push(ApplyAction::transform(
            matrix,
            sampling,
            expectation,
            expected_sampling,
            expected_tile_mode,
            expected_cf,
        ));
        self
    }

    /// `applyColorFilter(colorFilter, expectation)`.
    fn apply_color_filter(self, color_filter: ColorFilter, expectation: Expect) -> Self {
        self.apply_color_filter_expecting(color_filter, expectation, None)
    }

    /// `applyColorFilter(colorFilter, expectation, expectedColorFilter)`. `Some(None)` is an
    /// expected null filter.
    #[allow(clippy::option_option)] // mirrors the C++ std::optional<sk_sp<SkColorFilter>> with an explicit null
    fn apply_color_filter_expecting(
        mut self,
        color_filter: ColorFilter,
        expectation: Expect,
        expected_color_filter: Option<Option<ColorFilter>>,
    ) -> Self {
        // The expected color filter is the composition of the default expectation (e.g. last
        // color filter or null for a new image) and the new 'colorFilter'. Compose() automatically
        // returns 'colorFilter' if the inner filter is null.
        let expected_color_filter = match expected_color_filter {
            Some(explicit) => explicit,
            None => color_filters::compose(
                Some(&color_filter),
                self.default_expected_color_filter(expectation),
            ),
        };
        let affects_transparent = color_filter.as_base().affects_transparent_black();
        let expected_tile_mode = self.default_expected_tile_mode(expectation, affects_transparent);
        let expected_sampling = self.default_expected_sampling(expectation);
        self.actions.push(ApplyAction::color_filter(
            color_filter,
            expectation,
            expected_sampling,
            expected_tile_mode,
            expected_color_filter,
        ));
        self
    }

    /// `rescale(scale, expectation, expectedTileMode)`: `None` for the automatic expectation.
    fn rescale(
        mut self,
        scale: Size,
        expectation: Expect,
        expected_tile_mode: Option<TileMode>,
    ) -> Self {
        debug_assert!(!self.actions.is_empty());
        let expected_tile_mode = expected_tile_mode
            .unwrap_or_else(|| self.default_expected_tile_mode(expectation, false));
        let expected_sampling = self.default_expected_sampling(expectation);
        let expected_cf = self.default_expected_color_filter(expectation);
        self.actions.push(ApplyAction::rescale(
            scale,
            expectation,
            expected_sampling,
            expected_tile_mode,
            expected_cf,
        ));
        self
    }

    // By default an action that doesn't define its own sampling options will not change sampling
    // unless it produces a new image. Otherwise it inherits the prior action's expectation.
    fn default_expected_sampling(&self, expectation: Expect) -> SamplingOptions {
        if expectation != Expect::DeferredImage || self.actions.is_empty() {
            default_sampling()
        } else {
            self.actions[self.actions.len() - 1].expected_sampling()
        }
    }

    // By default an action that doesn't define its own tiling will not change the tiling, unless
    // it produces a new image, at which point it becomes kDecal again.
    fn default_expected_tile_mode(
        &self,
        expectation: Expect,
        cf_affects_transparency: bool,
    ) -> TileMode {
        if expectation == Expect::NewImage && cf_affects_transparency {
            TileMode::Clamp
        } else if expectation != Expect::DeferredImage || self.actions.is_empty() {
            TileMode::Decal
        } else {
            self.actions[self.actions.len() - 1].expected_tile_mode()
        }
    }

    // By default an action that doesn't define its own color filter will not change filtering,
    // unless it produces a new image. Otherwise it inherits the prior action's expectations.
    fn default_expected_color_filter(&self, expectation: Expect) -> Option<ColorFilter> {
        if expectation != Expect::DeferredImage || self.actions.is_empty() {
            None
        } else {
            self.actions[self.actions.len() - 1]
                .expected_color_filter()
                .cloned()
        }
    }

    /// `run(requestedOutput)`: runs with and without back-propagated desired outputs.
    // Port of: tests/FilterResultTest.cpp#L1080-L1085 (chrome/m156)
    fn run(mut self, requested_output: IRect) {
        let name = self.name.clone();
        self.runner.set_label(name);
        self.run_with(requested_output, true);
        self.run_with(requested_output, false);
    }

    /// `run(requestedOutput, backPropagateDesiredOutput)`.
    // Port of: tests/FilterResultTest.cpp#L1087-L1239 (chrome/m156)
    #[allow(clippy::needless_range_loop, clippy::too_many_lines)] // the C++ body is kept whole so it can be compared line by line; the loops index parallel arrays, as the C++ does
    fn run_with(&mut self, requested_output: IRect, back_propagate_desired_output: bool) {
        debug_assert!(
            !self.actions.is_empty(),
            "It's a bad test case if there aren't any actions"
        );

        let back_label = format!(
            "backpropagate output: {}",
            i32::from(back_propagate_desired_output)
        );
        self.runner.set_label(back_label);

        let n = self.actions.len();
        let desired_output = requested_output;
        let mut desired_outputs: Vec<IRect> = vec![desired_output; n];
        if !back_propagate_desired_output {
            // Set the desired output to be equal to the expected output so that there is no
            // further restriction of what's computed for early actions to then be ruled out by
            // subsequent actions.
            let mut input_bounds = self.source_bounds;
            for i in 0..n.saturating_sub(1) {
                desired_outputs[i] = self.actions[i].expected_bounds(input_bounds);
                // If the output for the ith action is infinite, leave it for now and expand the
                // input bounds for action i+1. The infinite bounds will be replaced by the
                // back-propagated desired output of the next action.
                if desired_outputs[i] == rect_priv::make_i_large() {
                    input_bounds.outset((25, 25));
                } else {
                    input_bounds = desired_outputs[i];
                }
            }
        }
        // Fill out regular back-propagated desired outputs and cleanup infinite outputs
        for i in (0..n.saturating_sub(1)).rev() {
            if back_propagate_desired_output || desired_outputs[i] == rect_priv::make_i_large() {
                desired_outputs[i] = self.actions[i + 1].required_input(desired_outputs[i + 1]);
            }
        }

        // Create the source image
        let color_space = ColorSpace::new_srgb();
        let mut source = FilterResult::default();
        if !self.source_bounds.is_empty() {
            let image = self.runner.create_source_image(
                ISize::new(self.source_bounds.width(), self.source_bounds.height()),
                color_space.clone(),
            );
            source = FilterResult::new(Some(image), self.source_bounds.top_left());
        }

        let stats = Stats::default();
        let base_context = Context::new(
            self.runner.ref_backend(),
            Mapping::new(),
            empty_irect(),
            source.clone(),
            Some(color_space.clone()),
            None,
        );

        // Applying modifiers to FilterResult might produce a new image, but hopefully it's able to
        // merge properties and even re-order operations to minimize the number of offscreen
        // surfaces that it creates. To validate that this is producing an equivalent image, we
        // track what to expect by rendering each action every time without any optimization.
        let mut expected_image = source.ref_image();
        let mut expected_origin = source.layer_bounds().top_left();
        // The expected image can't ever be null, so we produce a transparent black image instead.
        if expected_image.is_none() {
            let expected_surface = self
                .runner
                .backend()
                .make_device(ISize::new(1, 1), Some(color_space.clone()), None)
                .expect("makeDevice");
            let canvas = Canvas::from_device(expected_surface);
            canvas.with_top_device(clear_device);
            expected_image = canvas
                .with_top_device(|d| d.snap_special(&IRect::from_wh(1, 1), false))
                .map(Arc::new);
            expected_origin = IPoint::new(0, 0);
        }
        debug_assert!(expected_image.is_some());

        // Apply each action and validate, from first to last action
        for i in 0..n {
            self.runner.set_label(format!("action {i}"));

            reset_stats(&stats);
            let mut ctx = base_context.with_new_desired_output(desired_outputs[i]);
            ctx.track_stats(&stats);

            let output = self.actions[i].apply(&ctx, &source);
            // Validate consistency of the output
            reporter_assert!(
                self.runner.reporter,
                output.has_image() != output.layer_bounds().is_empty()
            );

            let mut expected_bounds = self.actions[i].expected_bounds(source.layer_bounds());
            let mut corrected_expectation = self.actions[i].expectation();
            if expected_bounds == rect_priv::make_i_large() {
                // An expected image filling out to infinity should have an actual image that
                // fills the desired output.
                expected_bounds = desired_outputs[i];
                if desired_outputs[i].is_empty() {
                    corrected_expectation = Expect::EmptyImage;
                }
            } else if !irect_intersect_in_place(&mut expected_bounds, &desired_outputs[i]) {
                // Test cases should provide image expectations for the case where desired output
                // is not back-propagated. When desired output is back-propagated, it can lead to
                // earlier actions becoming empty actions.
                reporter_assert!(
                    self.runner.reporter,
                    self.actions[i].expectation() == Expect::EmptyImage
                        || back_propagate_desired_output
                );
                expected_bounds = empty_irect();
                corrected_expectation = Expect::EmptyImage;
            }

            let allowed_offscreen_surfaces = self.actions[i].expected_offscreen_surfaces(&source);

            let actual_shader_draws =
                stats.num_shader_based_tiling_draws.get() + stats.num_shader_clamped_draws.get();
            let mut expected_shader_tiled_draws = 0;
            let actual_new_image = output.has_image()
                && (!source.has_image()
                    || output.image().map(SpecialImage::unique_id)
                        != source.image().map(SpecialImage::unique_id));
            match corrected_expectation {
                Expect::NewImage => {
                    reporter_assert!(self.runner.reporter, actual_new_image);
                    if source.image().is_some_and(|src| !src.is_exact_fit()) {
                        // Even if we're rescaling and making multiple surfaces, shader tiling
                        // should only ever be needed on the first step.
                        expected_shader_tiled_draws =
                            std::cmp::min(1, allowed_offscreen_surfaces[0]);
                    }
                }
                Expect::DeferredImage => {
                    reporter_assert!(
                        self.runner.reporter,
                        !actual_new_image && output.has_image()
                    );
                }
                Expect::EmptyImage => {
                    reporter_assert!(
                        self.runner.reporter,
                        !actual_new_image && !output.has_image()
                    );
                }
            }
            // Verify stats behavior for the current action
            let offscreen = stats.num_offscreen_surfaces.get();
            reporter_assert!(
                self.runner.reporter,
                allowed_offscreen_surfaces.contains(&offscreen),
                "expected {} or {}, got {}",
                allowed_offscreen_surfaces[0],
                if allowed_offscreen_surfaces.len() > 1 {
                    allowed_offscreen_surfaces[1]
                } else {
                    -1
                },
                offscreen
            );
            reporter_assert!(
                self.runner.reporter,
                actual_shader_draws <= expected_shader_tiled_draws,
                "expected {}+{} <= {}",
                stats.num_shader_based_tiling_draws.get(),
                stats.num_shader_clamped_draws.get(),
                expected_shader_tiled_draws
            );

            let expected_sample_mode = self.actions[i].expected_sample_mode(&ctx, &source);
            reporter_assert!(
                self.runner.reporter,
                stats.num_shader_based_tiling_draws.get() == 0
                    || expected_sample_mode == ShaderSampleMode::ShaderTile
            );
            reporter_assert!(
                self.runner.reporter,
                stats.num_shader_clamped_draws.get() == 0
                    || expected_sample_mode == ShaderSampleMode::ShaderClamp
            );

            // Validate layer bounds and sampling when we expect a new or deferred image
            if output.has_image() {
                // Mutable: the C++ intersects it in the assertion below, and that clipped value is
                // what the equality check compares.
                let mut actual_bounds = output.layer_bounds();
                // A deferred action doesn't have to crop its layer bounds to the desired output to
                // preserve accuracy of later bounds analysis. New images however should restrict
                // themselves to the desired output to minimize memory of the surface. The
                // exception is a new image for applyTransform() because the new transform is
                // deferred to the resolved image, which can make its layer bounds larger than the
                // desired output.
                if corrected_expectation == Expect::DeferredImage || !is_integer_transform(&output)
                {
                    // Skip the check if the desiredOutputs's SkIRect reports empty.
                    reporter_assert!(
                        self.runner.reporter,
                        desired_outputs[i].is_empty()
                            || irect_intersect_in_place(&mut actual_bounds, &desired_outputs[i])
                    );
                }
                reporter_assert!(self.runner.reporter, !expected_bounds.is_empty());
                reporter_assert!(
                    self.runner.reporter,
                    actual_bounds == expected_bounds,
                    "layer bounds {:?}, expected {:?}",
                    actual_bounds,
                    expected_bounds
                );
                reporter_assert!(
                    self.runner.reporter,
                    output.sampling() == self.actions[i].expected_sampling()
                );
                reporter_assert!(
                    self.runner.reporter,
                    output.tile_mode() == self.actions[i].expected_tile_mode()
                );
                reporter_assert!(
                    self.runner.reporter,
                    colorfilter_equals(
                        output.color_filter(),
                        self.actions[i].expected_color_filter()
                    )
                );
                if actual_shader_draws < expected_shader_tiled_draws
                    || (source.tile_mode() != TileMode::Clamp
                        && stats.num_shader_clamped_draws.get() > 0)
                {
                    // Some tile draws were optimized to HW draws, or some tile draws were reduced
                    // to shader-clamped draws, so compare the output to a non-optimized image.
                    reporter_assert!(
                        self.runner.reporter,
                        self.runner.validate_optimized_image(&ctx, &output)
                    );
                }
            }

            expected_image = self.actions[i].render_expected_image(
                &ctx,
                expected_image.clone(),
                expected_origin,
                desired_outputs[i],
            );
            expected_origin = desired_outputs[i].top_left();
            let allowed_percent = self.allowed_percent_image_diff;
            let allowed_rgb = self.allowed_rgb_tolerance;
            let padding = self.transparent_check_border_padding;
            if !self.runner.compare_special_images(
                &ctx,
                expected_image.as_deref(),
                expected_origin,
                &output,
                allowed_percent,
                allowed_rgb,
                padding,
            ) {
                // If one iteration is incorrect, its failures will likely cascade to further
                // actions so end now as the test has failed.
                break;
            }
            source = output;
        }
    }
}

/// `alpha_modulate`: dst-in blending with src = (1,1,1,v) = dst * v.
// Port of: tests/FilterResultTest.cpp#L1100-L1106 (chrome/m156)
fn alpha_modulate(v: f32) -> ColorFilter {
    // dst-in blending with src = (1,1,1,v) = dst * v
    let cf = color_filters::blend(Color4f::new(1.0, 1.0, 1.0, v), None, BlendMode::DstIn)
        .expect("alpha_modulate");
    assert!(!cf.as_base().affects_transparent_black());
    cf
}

/// `affect_transparent`: plus-blending a color, which affects transparent black.
// Port of: tests/FilterResultTest.cpp#L1108-L1113 (chrome/m156)
fn affect_transparent(color: Color4f) -> ColorFilter {
    let cf = color_filters::blend(color, None, BlendMode::Plus).expect("affect_transparent");
    assert!(cf.as_base().affects_transparent_black());
    cf
}

// Test-suite bodies follow. Each is `DEF_TEST_SUITE(name, ...)` in the C++, registered as the
// raster `FilterResult_raster_<name>` test.

/// A `Size` of `width` by `height` (`SkSize`).
fn sz(width: f32, height: f32) -> Size {
    Size { width, height }
}

/// The sampling options of a `SkSamplingOptions`-typed argument.
fn so(sampling: impl Into<SamplingOptions>) -> SamplingOptions {
    sampling.into()
}

/// `DEF_TEST_SUITE(name, ...)`: the raster test that runs the suite body `$body`.
macro_rules! def_filter_result_suite {
    ($name:ident, $body:ident) => {
        def_test!($name, |reporter| {
            let mut r = TestRunner::new(reporter);
            $body(&mut r);
        });
    };
}

// ----------------------------------------------------------------------------
// Empty input/output tests

// Port of: tests/FilterResultTest.cpp#L1410-L1437 (chrome/m156)
fn test_suite_empty_source(r: &mut TestRunner<'_>) {
    // This is testing that an empty input image is handled by the applied actions without having
    // to generate new images, or that it can produce a new image from nothing when it affects
    // transparent black.
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "applyCrop() to empty source")
            .source(IRect::new_empty())
            .apply_crop(IRect::new(0, 0, 10, 10), tm, Expect::EmptyImage)
            .run(IRect::new(0, 0, 20, 20));
    }

    TestCase::new(r, "applyTransform() to empty source")
        .source(IRect::new_empty())
        .apply_transform_default(Matrix::translate((10.0, 10.0)), Expect::EmptyImage)
        .run(IRect::new(10, 10, 20, 20));

    TestCase::new(r, "applyColorFilter() to empty source")
        .source(IRect::new_empty())
        .apply_color_filter(alpha_modulate(0.5), Expect::EmptyImage)
        .run(IRect::new(0, 0, 10, 10));

    // CF applied ASAP to make a new img
    TestCase::new(
        r,
        "Transparency-affecting color filter overrules empty source",
    )
    .source(IRect::new_empty())
    .apply_color_filter_expecting(
        affect_transparent(Color4f::new(0.0, 0.0, 1.0, 1.0)),
        Expect::NewImage,
        Some(None),
    )
    .run(IRect::new(0, 0, 10, 10));
}
def_filter_result_suite!(EmptySource, test_suite_empty_source);

// Port of: tests/FilterResultTest.cpp#L1439-L1467 (chrome/m156)
fn test_suite_empty_desired_output(r: &mut TestRunner<'_>) {
    // This is testing that an empty requested output is propagated through the applied actions so
    // that no actual images are generated.
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "applyCrop() + empty output becomes empty")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(2, 2, 8, 8), tm, Expect::EmptyImage)
            .run(IRect::new_empty());
    }

    TestCase::new(r, "applyTransform() + empty output becomes empty")
        .source(IRect::new(0, 0, 10, 10))
        .apply_transform_default(Matrix::rotate_deg(10.0), Expect::EmptyImage)
        .run(IRect::new_empty());

    TestCase::new(r, "applyColorFilter() + empty output becomes empty")
        .source(IRect::new(0, 0, 10, 10))
        .apply_color_filter(alpha_modulate(0.5), Expect::EmptyImage)
        .run(IRect::new_empty());

    TestCase::new(
        r,
        "Transpency-affecting color filter + empty output is empty",
    )
    .source(IRect::new(0, 0, 10, 10))
    .apply_color_filter(
        affect_transparent(Color4f::new(0.0, 0.0, 1.0, 1.0)),
        Expect::EmptyImage,
    )
    .run(IRect::new_empty());
}
def_filter_result_suite!(EmptyDesiredOutput, test_suite_empty_desired_output);

// ----------------------------------------------------------------------------
// applyCrop() tests

// Port of: tests/FilterResultTest.cpp#L1472-L1518 (chrome/m156)
fn test_suite_crop(r: &mut TestRunner<'_>) {
    // This is testing all the combinations of how the src, crop, and requested output rectangles
    // can interact while still resulting in a deferred image. The exception is non-decal tile
    // modes where the crop rect includes transparent pixels not filled by the source, which
    // requires a new image to ensure tiling matches the crop geometry.
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        let non_decal_expects_new_image = if tm == TileMode::Decal {
            Expect::DeferredImage
        } else {
            Expect::NewImage
        };
        TestCase::new(r, "applyCrop() contained in source and output")
            .source(IRect::new(0, 0, 20, 20))
            .apply_crop(IRect::new(8, 8, 12, 12), tm, Expect::DeferredImage)
            .run(IRect::new(4, 4, 16, 16));

        TestCase::new(r, "applyCrop() contained in source, intersects output")
            .source(IRect::new(0, 0, 20, 20))
            .apply_crop(IRect::new(4, 4, 12, 12), tm, Expect::DeferredImage)
            .run(IRect::new(8, 8, 16, 16));

        TestCase::new(r, "applyCrop() intersects source, contained in output")
            .source(IRect::new(10, 10, 20, 20))
            .apply_crop(IRect::new(4, 4, 16, 16), tm, non_decal_expects_new_image)
            .run(IRect::new(0, 0, 20, 20));

        TestCase::new(r, "applyCrop() intersects source and output")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(5, -5, 15, 5), tm, non_decal_expects_new_image)
            .run(IRect::new(7, -2, 12, 8));

        TestCase::new(r, "applyCrop() contains source, intersects output")
            .source(IRect::new(4, 4, 16, 16))
            .apply_crop(IRect::new(0, 0, 20, 20), tm, non_decal_expects_new_image)
            .run(IRect::new(-5, -5, 18, 18));

        // In these cases, cropping with a non-decal tile mode can be discarded because the output
        // bounds are entirely within the crop so no tiled edges would be visible.
        TestCase::new(r, "applyCrop() intersects source, contains output")
            .source(IRect::new(0, 0, 20, 20))
            .apply_crop_expecting(
                IRect::new(-5, 5, 25, 15),
                tm,
                Expect::DeferredImage,
                Some(TileMode::Decal),
                None,
            )
            .run(IRect::new(0, 5, 20, 15));

        TestCase::new(r, "applyCrop() contains source and output")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop_expecting(
                IRect::new(-5, -5, 15, 15),
                tm,
                Expect::DeferredImage,
                Some(TileMode::Decal),
                None,
            )
            .run(IRect::new(1, 1, 9, 9));
    }
}
def_filter_result_suite!(Crop, test_suite_crop);

// Port of: tests/FilterResultTest.cpp#L1520-L1567 (chrome/m156)
fn test_suite_crop_disjoint_from_source_and_output(r: &mut TestRunner<'_>) {
    // This tests all the combinations of src, crop, and requested output rectangles that result in
    // an empty image without any of the rectangles being empty themselves. The exception is for
    // non-decal tile modes when the source and crop still intersect. In that case the non-empty
    // content is tiled into the disjoint output rect, producing a non-empty image.
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "applyCrop() disjoint from source, intersects output")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(11, 11, 20, 20), tm, Expect::EmptyImage)
            .run(IRect::new(0, 0, 15, 15));

        TestCase::new(
            r,
            "applyCrop() disjoint from source, intersects output disjoint from source",
        )
        .source(IRect::new(0, 0, 10, 10))
        .apply_crop(IRect::new(11, 11, 20, 20), tm, Expect::EmptyImage)
        .run(IRect::new(12, 12, 18, 18));

        TestCase::new(r, "applyCrop() disjoint from source and output")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(12, 12, 18, 18), tm, Expect::EmptyImage)
            .run(IRect::new(-1, -1, 11, 11));

        TestCase::new(
            r,
            "applyCrop() disjoint from source and output disjoint from source",
        )
        .source(IRect::new(0, 0, 10, 10))
        .apply_crop(IRect::new(-10, 10, -1, -1), tm, Expect::EmptyImage)
        .run(IRect::new(11, 11, 20, 20));

        // When the source and crop intersect but are disjoint from the output, the behavior depends
        // on the tile mode. For periodic tile modes, certain geometries can still be deferred by
        // conversion to a transform, but to keep expectations simple we pick bounds such that the
        // tiling can't be dropped. See PeriodicTileCrops for other scenarios.
        let non_decal_expects_image = if tm == TileMode::Decal {
            Expect::EmptyImage
        } else if tm == TileMode::Clamp {
            Expect::DeferredImage
        } else {
            Expect::NewImage
        };
        TestCase::new(
            r,
            "applyCrop() intersects source, disjoint from output disjoint from source",
        )
        .source(IRect::new(0, 0, 10, 10))
        .apply_crop(IRect::new(-5, -5, 5, 5), tm, non_decal_expects_image)
        .run(IRect::new(12, 12, 18, 18));

        TestCase::new(r, "applyCrop() intersects source, disjoint from output")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(-5, -5, 5, 5), tm, non_decal_expects_image)
            .run(IRect::new(6, 6, 18, 18));
    }
}
def_filter_result_suite!(
    CropDisjointFromSourceAndOutput,
    test_suite_crop_disjoint_from_source_and_output
);

// Port of: tests/FilterResultTest.cpp#L1569-L1583 (chrome/m156)
fn test_suite_empty_crop(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "applyCrop() is empty")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new_empty(), tm, Expect::EmptyImage)
            .run(IRect::new(0, 0, 10, 10));

        TestCase::new(r, "applyCrop() emptiness propagates")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(1, 1, 9, 9), tm, Expect::DeferredImage)
            .apply_crop(IRect::new_empty(), tm, Expect::EmptyImage)
            .run(IRect::new(0, 0, 10, 10));
    }
}
def_filter_result_suite!(EmptyCrop, test_suite_empty_crop);

// Port of: tests/FilterResultTest.cpp#L1585-L1609 (chrome/m156)
fn test_suite_disjoint_crops(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "Disjoint applyCrop() after kDecal become empty")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(
                IRect::new(0, 0, 4, 4),
                TileMode::Decal,
                Expect::DeferredImage,
            )
            .apply_crop(IRect::new(6, 6, 10, 10), tm, Expect::EmptyImage)
            .run(IRect::new(0, 0, 10, 10));

        if tm != TileMode::Decal {
            TestCase::new(
                r,
                "Disjoint tiling applyCrop() before kDecal is not empty and combines",
            )
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(0, 0, 4, 4), tm, Expect::DeferredImage)
            .apply_crop_expecting(
                IRect::new(6, 6, 10, 10),
                TileMode::Decal,
                Expect::DeferredImage,
                Some(tm),
                None,
            )
            .run(IRect::new(0, 0, 10, 10));

            let expect = if tm == TileMode::Clamp {
                Expect::DeferredImage
            } else {
                Expect::NewImage
            };
            TestCase::new(r, "Disjoint non-decal applyCrops() are not empty")
                .source(IRect::new(0, 0, 10, 10))
                .apply_crop(IRect::new(0, 0, 4, 4), tm, Expect::DeferredImage)
                .apply_crop(IRect::new(6, 6, 10, 10), tm, expect)
                .run(IRect::new(0, 0, 10, 10));
        }
    }
}
def_filter_result_suite!(DisjointCrops, test_suite_disjoint_crops);

// Port of: tests/FilterResultTest.cpp#L1611-L1639 (chrome/m156)
fn test_suite_intersecting_crops(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "Decal applyCrop() always combines with any other crop")
            .source(IRect::new(0, 0, 20, 20))
            .apply_crop(IRect::new(5, 5, 15, 15), tm, Expect::DeferredImage)
            .apply_crop_expecting(
                IRect::new(10, 10, 20, 20),
                TileMode::Decal,
                Expect::DeferredImage,
                Some(tm),
                None,
            )
            .run(IRect::new(0, 0, 20, 20));

        if tm != TileMode::Decal {
            TestCase::new(
                r,
                "Decal applyCrop() before non-decal crop requires new image",
            )
            .source(IRect::new(0, 0, 20, 20))
            .apply_crop(
                IRect::new(5, 5, 15, 15),
                TileMode::Decal,
                Expect::DeferredImage,
            )
            .apply_crop(IRect::new(10, 10, 20, 20), tm, Expect::NewImage)
            .run(IRect::new(0, 0, 20, 20));

            let expect = if tm == TileMode::Clamp {
                Expect::DeferredImage
            } else {
                Expect::NewImage
            };
            TestCase::new(r, "Consecutive non-decal crops combine if both are clamp")
                .source(IRect::new(0, 0, 20, 20))
                .apply_crop(IRect::new(5, 5, 15, 15), tm, Expect::DeferredImage)
                .apply_crop(IRect::new(10, 10, 20, 20), tm, expect)
                .run(IRect::new(0, 0, 20, 20));
        }
    }
}
def_filter_result_suite!(IntersectingCrops, test_suite_intersecting_crops);

// Port of: tests/FilterResultTest.cpp#L1641-L1696 (chrome/m156)
fn test_suite_periodic_tile_crops(r: &mut TestRunner<'_>) {
    for tm in [TileMode::Repeat, TileMode::Mirror] {
        r.set_label(tile_mode_name(tm));
        // In these tests, the crop periodically tiles such that it covers the desired output so
        // the prior image can be simply transformed.
        TestCase::new(r, "Periodic applyCrop() becomes a transform")
            .source(IRect::new(0, 0, 20, 20))
            .apply_crop_expecting(
                IRect::new(5, 5, 15, 15),
                tm,
                Expect::DeferredImage,
                Some(TileMode::Decal),
                None,
            )
            .run(IRect::new(25, 25, 35, 35));

        let expected_bounds = if tm == TileMode::Repeat {
            IRect::new(20, 20, 35, 35)
        } else {
            IRect::new(15, 15, 30, 30)
        };
        TestCase::new(
            r,
            "Periodic applyCrop() with partial transparency still becomes a transform",
        )
        .source(IRect::new(0, 0, 20, 20))
        .apply_crop_expecting(
            IRect::new(-5, -5, 15, 15),
            tm,
            Expect::DeferredImage,
            Some(TileMode::Decal),
            Some(expected_bounds),
        )
        .run(IRect::new(15, 15, 35, 35));

        TestCase::new(
            r,
            "Periodic applyCrop() after complex transform can still simplify",
        )
        .source(IRect::new(0, 0, 20, 20))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(15.0, (10.0, 10.0)),
            Expect::DeferredImage,
        )
        // source+15 degree rotation
        .apply_crop_expecting(
            IRect::new(-5, -5, 25, 25),
            tm,
            Expect::DeferredImage,
            Some(TileMode::Decal),
            Some(IRect::new(57, 57, 83, 83)),
        )
        .run(IRect::new(55, 55, 85, 85));

        // In these tests, the crop's periodic boundary intersects with the output so it should not
        // simplify to just a transform.
        TestCase::new(
            r,
            "Periodic applyCrop() with visible edge does not become a transform",
        )
        .source(IRect::new(0, 0, 20, 20))
        .apply_crop(IRect::new(5, 5, 15, 15), tm, Expect::DeferredImage)
        .run(IRect::new(10, 10, 20, 20));

        TestCase::new(
            r,
            "Periodic applyCrop() with visible edge and transparency creates new image",
        )
        .source(IRect::new(0, 0, 20, 20))
        .apply_crop(IRect::new(-5, -5, 15, 15), tm, Expect::NewImage)
        .run(IRect::new(10, 10, 20, 20));

        TestCase::new(
            r,
            "Periodic applyCrop() with visible edge and complex transform creates image",
        )
        .source(IRect::new(0, 0, 20, 20))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(15.0, (10.0, 10.0)),
            Expect::DeferredImage,
        )
        .apply_crop(IRect::new(-5, -5, 25, 25), tm, Expect::NewImage)
        .run(IRect::new(20, 20, 50, 50));

        // oss-fuzz:70128 ensure period calculations don't overflow (but will fail image creation)
        TestCase::new(r, "Pathologically large crop rect")
            .source(IRect::new(0, 0, 10, 10))
            .apply_crop(IRect::new(0, 0, 1, 2), tm, Expect::DeferredImage)
            .apply_transform_default(
                Matrix::rotate_deg_pivot(1.0, (5.0, 5.0)),
                Expect::EmptyImage,
            )
            .run(IRect::new(-726_713_344, 7, 1_464_866_662, 15));
    }
}
def_filter_result_suite!(PeriodicTileCrops, test_suite_periodic_tile_crops);

// Port of: tests/FilterResultTest.cpp#L1698-L1712 (chrome/m156)
fn test_suite_decal_then_clamp(r: &mut TestRunner<'_>) {
    TestCase::new(
        r,
        "Decal then clamp crop uses 1px buffer around intersection",
    )
    .source(IRect::new(0, 0, 20, 20))
    .apply_crop_decal(IRect::new(3, 3, 17, 17), Expect::DeferredImage)
    .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
    .apply_crop_expecting(
        IRect::new(3, 3, 20, 20),
        TileMode::Clamp,
        Expect::NewImage,
        Some(TileMode::Clamp),
        None,
    )
    .run(IRect::new(0, 0, 20, 20));

    TestCase::new(
        r,
        "Decal then clamp crop uses 1px buffer around intersection, w/ alpha color filter",
    )
    .source(IRect::new(0, 0, 20, 20))
    .apply_crop_decal(IRect::new(3, 3, 17, 17), Expect::DeferredImage)
    .apply_color_filter(affect_transparent(K_CYAN), Expect::DeferredImage)
    .apply_crop_expecting(
        IRect::new(0, 0, 17, 17),
        TileMode::Clamp,
        Expect::NewImage,
        Some(TileMode::Clamp),
        None,
    )
    .run(IRect::new(0, 0, 20, 20));
}
def_filter_result_suite!(DecalThenClamp, test_suite_decal_then_clamp);

// ----------------------------------------------------------------------------
// applyTransform() tests

// Port of: tests/FilterResultTest.cpp#L1717-L1738 (chrome/m156)
fn test_suite_transform(r: &mut TestRunner<'_>) {
    TestCase::new(r, "applyTransform() integer translate")
        .source(IRect::new(0, 0, 10, 10))
        .apply_transform_default(Matrix::translate((5.0, 5.0)), Expect::DeferredImage)
        .run(IRect::new(0, 0, 10, 10));

    TestCase::new(r, "applyTransform() fractional translate")
        .source(IRect::new(0, 0, 10, 10))
        .apply_transform_default(Matrix::translate((1.5, 3.24)), Expect::DeferredImage)
        .run(IRect::new(0, 0, 10, 10));

    TestCase::new(r, "applyTransform() scale")
        .source(IRect::new(0, 0, 24, 24))
        .apply_transform_default(Matrix::scale((2.2, 3.1)), Expect::DeferredImage)
        .run(IRect::new(-16, -16, 96, 96));

    // NOTE: complex is anything beyond a scale+translate. See SkImageFilter_Base::MatrixCapability.
    TestCase::new(r, "applyTransform() with complex transform")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(10.0, (4.0, 4.0)),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 16, 16));
}
def_filter_result_suite!(Transform, test_suite_transform);

/// `SkMatrix::RotateDeg(2.f, {4.f, 4.f})`, the transform of most sampling tests.
fn rotate_2_about_4() -> Matrix {
    Matrix::rotate_deg_pivot(2.0, (4.0, 4.0))
}

// Port of: tests/FilterResultTest.cpp#L1740-L1813 (chrome/m156)
#[allow(clippy::too_many_lines)] // the C++ body is kept whole so it can be compared line by line
fn test_suite_compatible_sampling_concats_transforms(r: &mut TestRunner<'_>) {
    TestCase::new(r, "linear + linear combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            FilterMode::Linear,
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            FilterMode::Linear,
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "equiv. bicubics combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "linear + bicubic becomes bicubic")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            FilterMode::Linear,
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "bicubic + linear becomes bicubic")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .apply_transform_expecting(
            rotate_2_about_4(),
            FilterMode::Linear,
            Expect::DeferredImage,
            Some(so(CubicResampler::mitchell())),
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "aniso picks max level to combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(4),
            Expect::DeferredImage,
        )
        .apply_transform_expecting(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(2),
            Expect::DeferredImage,
            Some(SamplingOptions::from_aniso(4)),
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "aniso picks max level to combine (other direction)")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(2),
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(4),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "linear + aniso becomes aniso")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            FilterMode::Linear,
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(2),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "aniso + linear stays aniso")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(4),
            Expect::DeferredImage,
        )
        .apply_transform_expecting(
            rotate_2_about_4(),
            FilterMode::Linear,
            Expect::DeferredImage,
            Some(SamplingOptions::from_aniso(4)),
        )
        .run(IRect::new(0, 0, 16, 16));

    // TODO: Add cases for mipmapping once that becomes relevant (SkSpecialImage does not have
    // mipmaps right now).
}
def_filter_result_suite!(
    CompatibleSamplingConcatsTransforms,
    test_suite_compatible_sampling_concats_transforms
);

// Port of: tests/FilterResultTest.cpp#L1815-L1866 (chrome/m156)
fn test_suite_incompatible_sampling_resolves_images(r: &mut TestRunner<'_>) {
    TestCase::new(r, "different bicubics do not combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::catmull_rom(),
            Expect::NewImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "nearest + linear do not combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            FilterMode::Nearest,
            Expect::DeferredImage,
        )
        .apply_transform(rotate_2_about_4(), FilterMode::Linear, Expect::NewImage)
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "linear + nearest do not combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            FilterMode::Linear,
            Expect::DeferredImage,
        )
        .apply_transform(rotate_2_about_4(), FilterMode::Nearest, Expect::NewImage)
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "bicubic + aniso do not combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(4),
            Expect::NewImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "aniso + bicubic do not combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            SamplingOptions::from_aniso(4),
            Expect::DeferredImage,
        )
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::NewImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "nearest + nearest do not combine")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            FilterMode::Nearest,
            Expect::DeferredImage,
        )
        .apply_transform(rotate_2_about_4(), FilterMode::Nearest, Expect::NewImage)
        .run(IRect::new(0, 0, 16, 16));
}
def_filter_result_suite!(
    IncompatibleSamplingResolvesImages,
    test_suite_incompatible_sampling_resolves_images
);

// Port of: tests/FilterResultTest.cpp#L1868-L1894 (chrome/m156)
fn test_suite_integer_offset_ignores_nearest_sampling(r: &mut TestRunner<'_>) {
    // Bicubic is used here to reflect that it should use the non-NN sampling and just needs to be
    // something other than the default to detect that it got carried through.
    TestCase::new(r, "integer translate+NN then bicubic combines")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform_expecting(
            Matrix::translate((2.0, 2.0)),
            FilterMode::Nearest,
            Expect::DeferredImage,
            Some(default_sampling()),
        )
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(r, "bicubic then integer translate+NN combines")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform(
            rotate_2_about_4(),
            CubicResampler::mitchell(),
            Expect::DeferredImage,
        )
        .apply_transform_expecting(
            Matrix::translate((2.0, 2.0)),
            FilterMode::Nearest,
            Expect::DeferredImage,
            Some(so(CubicResampler::mitchell())),
        )
        .run(IRect::new(0, 0, 16, 16));
}
def_filter_result_suite!(
    IntegerOffsetIgnoresNearestSampling,
    test_suite_integer_offset_ignores_nearest_sampling
);

// ----------------------------------------------------------------------------
// applyTransform() interacting with applyCrop()

// Port of: tests/FilterResultTest.cpp#L1896-L1916 (chrome/m156)
fn test_suite_transform_becomes_empty(r: &mut TestRunner<'_>) {
    TestCase::new(r, "Transform moves src image outside of requested output")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform_default(Matrix::translate((10.0, 10.0)), Expect::EmptyImage)
        .run(IRect::new(0, 0, 8, 8));

    TestCase::new(r, "Transform moves src image outside of crop")
        .source(IRect::new(0, 0, 8, 8))
        .apply_transform_default(Matrix::translate((10.0, 10.0)), Expect::DeferredImage)
        .apply_crop_decal(IRect::new(2, 2, 6, 6), Expect::EmptyImage)
        .run(IRect::new(0, 0, 20, 20));

    TestCase::new(
        r,
        "Transform moves cropped image outside of requested output",
    )
    .source(IRect::new(0, 0, 8, 8))
    .apply_crop_decal(IRect::new(1, 1, 4, 4), Expect::DeferredImage)
    .apply_transform_default(Matrix::translate((-5.0, -5.0)), Expect::EmptyImage)
    .run(IRect::new(0, 0, 8, 8));
}
def_filter_result_suite!(TransformBecomesEmpty, test_suite_transform_becomes_empty);

// Port of: tests/FilterResultTest.cpp#L1918-L1961 (chrome/m156)
fn test_suite_transform_and_crop(r: &mut TestRunner<'_>) {
    TestCase::new(r, "Crop after transform can always apply")
        .source(IRect::new(0, 0, 16, 16))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (3.0, 4.0)),
            Expect::DeferredImage,
        )
        .apply_crop_decal(IRect::new(2, 2, 15, 15), Expect::DeferredImage)
        .run(IRect::new(0, 0, 16, 16));

    // TODO: Expand this test case to be arbitrary float S+T transforms when FilterResult tracks
    // both a srcRect and dstRect.
    TestCase::new(r, "Crop after translate is lifted to image subset")
        .source(IRect::new(0, 0, 32, 32))
        .apply_transform_default(Matrix::translate((12.0, 8.0)), Expect::DeferredImage)
        .apply_crop_decal(IRect::new(16, 16, 24, 24), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (16.0, 16.0)),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "Transform after unlifted crop triggers new image")
        .source(IRect::new(0, 0, 16, 16))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (8.0, 8.0)),
            Expect::DeferredImage,
        )
        .apply_crop_decal(IRect::new(1, 1, 15, 15), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(-10.0, (8.0, 4.0)),
            Expect::NewImage,
        )
        .run(IRect::new(0, 0, 16, 16));

    TestCase::new(
        r,
        "Transform after unlifted crop with interior output does not trigger new image",
    )
    .source(IRect::new(0, 0, 16, 16))
    .apply_transform_default(
        Matrix::rotate_deg_pivot(45.0, (8.0, 8.0)),
        Expect::DeferredImage,
    )
    .apply_crop_decal(IRect::new(1, 1, 15, 15), Expect::DeferredImage)
    .apply_transform_default(
        Matrix::rotate_deg_pivot(-10.0, (8.0, 4.0)),
        Expect::DeferredImage,
    )
    .run(IRect::new(4, 4, 12, 12));

    TestCase::new(
        r,
        "Translate after unlifted crop does not trigger new image",
    )
    .source(IRect::new(0, 0, 16, 16))
    .apply_transform_default(
        Matrix::rotate_deg_pivot(5.0, (8.0, 8.0)),
        Expect::DeferredImage,
    )
    .apply_crop_decal(IRect::new(2, 2, 14, 14), Expect::DeferredImage)
    .apply_transform_default(Matrix::translate((4.0, 6.0)), Expect::DeferredImage)
    .run(IRect::new(0, 0, 16, 16));

    TestCase::new(
        r,
        "Transform after large no-op crop does not trigger new image",
    )
    .source(IRect::new(0, 0, 64, 64))
    .apply_transform_default(
        Matrix::rotate_deg_pivot(45.0, (32.0, 32.0)),
        Expect::DeferredImage,
    )
    .apply_crop_decal(IRect::new(-64, -64, 128, 128), Expect::DeferredImage)
    .apply_transform_default(
        Matrix::rotate_deg_pivot(-30.0, (32.0, 32.0)),
        Expect::DeferredImage,
    )
    .run(IRect::new(0, 0, 64, 64));
}
def_filter_result_suite!(TransformAndCrop, test_suite_transform_and_crop);

// Port of: tests/FilterResultTest.cpp#L1963-L1995 (chrome/m156)
fn test_suite_transform_and_tile(r: &mut TestRunner<'_>) {
    // Test interactions of non-decal tile modes and transforms
    for tm in TILE_MODES {
        if tm == TileMode::Decal {
            continue;
        }
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "Transform after tile mode does not trigger new image")
            .source(IRect::new(0, 0, 64, 64))
            .apply_crop(IRect::new(2, 2, 32, 32), tm, Expect::DeferredImage)
            .apply_transform_default(
                Matrix::rotate_deg_pivot(20.0, (16.0, 8.0)),
                Expect::DeferredImage,
            )
            .run(IRect::new(0, 0, 64, 64));

        TestCase::new(
            r,
            "Integer transform before tile mode does not trigger new image",
        )
        .source(IRect::new(0, 0, 32, 32))
        .apply_transform_default(Matrix::translate((16.0, 16.0)), Expect::DeferredImage)
        .apply_crop(IRect::new(20, 20, 40, 40), tm, Expect::DeferredImage)
        .run(IRect::new(0, 0, 64, 64));

        TestCase::new(
            r,
            "Non-integer transform before tile mode triggers new image",
        )
        .source(IRect::new(0, 0, 50, 40))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(-30.0, (20.0, 10.0)),
            Expect::DeferredImage,
        )
        .apply_crop(IRect::new(10, 10, 30, 30), tm, Expect::NewImage)
        .run(IRect::new(0, 0, 50, 50));

        TestCase::new(
            r,
            "Non-integer transform before tiling defers image if edges are hidden",
        )
        .source(IRect::new(0, 0, 64, 64))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (32.0, 32.0)),
            Expect::DeferredImage,
        )
        .apply_crop_expecting(
            IRect::new(10, 10, 50, 50),
            tm,
            Expect::DeferredImage,
            Some(TileMode::Decal),
            None,
        )
        .run(IRect::new(11, 11, 49, 49));
    }
}
def_filter_result_suite!(TransformAndTile, test_suite_transform_and_tile);

// ----------------------------------------------------------------------------
// applyColorFilter() and interactions with transforms/crops

/// `SkColors::kBlue`.
const K_BLUE: Color4f = Color4f {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};
/// `SkColors::kRed`.
const K_RED: Color4f = Color4f {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
/// `SkColors::kGreen`.
const K_GREEN: Color4f = Color4f {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
/// `SkColors::kCyan`.
const K_CYAN: Color4f = Color4f {
    r: 0.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

// Port of: tests/FilterResultTest.cpp#L2000-L2044 (chrome/m156)
fn test_suite_color_filter(r: &mut TestRunner<'_>) {
    TestCase::new(r, "applyColorFilter() defers image")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "applyColorFilter() composes with other color filters")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "Transparency-affecting color filter fills output")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
        .run(IRect::new(-8, -8, 32, 32));

    // Since there is no cropping between the composed color filters, transparency-affecting CFs
    // can still compose together.
    TestCase::new(r, "Transparency-affecting composition fills output (ATBx2)")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
        .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
        .run(IRect::new(-8, -8, 32, 32));

    TestCase::new(
        r,
        "Transparency-affecting composition fills output (ATB,reg)",
    )
    .source(IRect::new(0, 0, 24, 24))
    .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
    .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
    .run(IRect::new(-8, -8, 32, 32));

    TestCase::new(
        r,
        "Transparency-affecting composition fills output (reg,ATB)",
    )
    .source(IRect::new(0, 0, 24, 24))
    .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
    .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
    .run(IRect::new(-8, -8, 32, 32));

    // oss-fuzz:70134 Requesting the output of an infinite image (e.g. transparency affecting
    // color filter) at the limit of 32-bit ints doesn't leave room for border padding pixels.
    // This should cleanly fail to an empty image.
    TestCase::new(
        r,
        "Pathologic output bounds with transparency-affecting color filter is empty",
    )
    .source(IRect::new(0, 0, 16, 16))
    .apply_color_filter(affect_transparent(K_RED), Expect::EmptyImage)
    .run(IRect::new(-i32::MAX, 0, -i32::MAX + 10, 16));
}
def_filter_result_suite!(ColorFilter, test_suite_color_filter);

// Port of: tests/FilterResultTest.cpp#L2046-L2077 (chrome/m156)
fn test_suite_transformed_color_filter(r: &mut TestRunner<'_>) {
    TestCase::new(r, "Transform composes with regular CF")
        .source(IRect::new(0, 0, 24, 24))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .run(IRect::new(0, 0, 24, 24));

    TestCase::new(r, "Regular CF composes with transform")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 24, 24));

    TestCase::new(r, "Transform composes with transparency-affecting CF")
        .source(IRect::new(0, 0, 24, 24))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
        .run(IRect::new(0, 0, 24, 24));

    // NOTE: Because there is no explicit crop between the color filter and the transform,
    // output bounds propagation means the layer bounds of the applied color filter are never
    // visible post transform. This is detected and allows the transform to be composed without
    // producing an intermediate image. See later tests for when a crop prevents this optimization.
    TestCase::new(r, "Transparency-affecting CF composes with transform")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .run(IRect::new(-50, -50, 50, 50));
}
def_filter_result_suite!(TransformedColorFilter, test_suite_transformed_color_filter);

// Port of: tests/FilterResultTest.cpp#L2079-L2111 (chrome/m156)
fn test_suite_transform_between_color_filters(r: &mut TestRunner<'_>) {
    // NOTE: The lack of explicit crops allows all of these operations to be optimized as well.
    TestCase::new(r, "Transform between regular color filters")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(alpha_modulate(0.75), Expect::DeferredImage)
        .run(IRect::new(0, 0, 24, 24));

    TestCase::new(r, "Transform between transparency-affecting color filters")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
        .run(IRect::new(0, 0, 24, 24));

    TestCase::new(r, "Transform between ATB and regular color filters")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(affect_transparent(K_BLUE), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(alpha_modulate(0.75), Expect::DeferredImage)
        .run(IRect::new(0, 0, 24, 24));

    TestCase::new(r, "Transform between regular and ATB color filters")
        .source(IRect::new(0, 0, 24, 24))
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
        .run(IRect::new(0, 0, 24, 24));
}
def_filter_result_suite!(
    TransformBetweenColorFilters,
    test_suite_transform_between_color_filters
);

// Port of: tests/FilterResultTest.cpp#L2113-L2130 (chrome/m156)
fn test_suite_color_filter_between_transforms(r: &mut TestRunner<'_>) {
    TestCase::new(r, "Regular color filter between transforms")
        .source(IRect::new(0, 0, 24, 24))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(20.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(alpha_modulate(0.8), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(10.0, (5.0, 8.0)),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 24, 24));

    TestCase::new(r, "Transparency-affecting color filter between transforms")
        .source(IRect::new(0, 0, 24, 24))
        .apply_transform_default(
            Matrix::rotate_deg_pivot(20.0, (12.0, 12.0)),
            Expect::DeferredImage,
        )
        .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(10.0, (5.0, 8.0)),
            Expect::DeferredImage,
        )
        .run(IRect::new(0, 0, 24, 24));
}
def_filter_result_suite!(
    ColorFilterBetweenTransforms,
    test_suite_color_filter_between_transforms
);

// Port of: tests/FilterResultTest.cpp#L2132-L2176 (chrome/m156)
fn test_suite_cropped_color_filter(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "Regular color filter after empty crop stays empty")
            .source(IRect::new(0, 0, 16, 16))
            .apply_crop(IRect::new_empty(), tm, Expect::EmptyImage)
            .apply_color_filter(alpha_modulate(0.2), Expect::EmptyImage)
            .run(IRect::new(0, 0, 16, 16));

        // CF applied ASAP to new img
        TestCase::new(
            r,
            "Transparency-affecting color filter after empty crop creates new image",
        )
        .source(IRect::new(0, 0, 16, 16))
        .apply_crop(IRect::new_empty(), tm, Expect::EmptyImage)
        .apply_color_filter_expecting(affect_transparent(K_RED), Expect::NewImage, Some(None))
        .run(IRect::new(0, 0, 16, 16));

        TestCase::new(r, "Regular color filter composes with crop")
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(alpha_modulate(0.7), Expect::DeferredImage)
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));

        TestCase::new(r, "Crop composes with regular color filter")
            .source(IRect::new(0, 0, 32, 32))
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));

        TestCase::new(r, "Transparency-affecting color filter restricted by crop")
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));

        TestCase::new(r, "Crop composes with transparency-affecting color filter")
            .source(IRect::new(0, 0, 32, 32))
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));
    }
}
def_filter_result_suite!(CroppedColorFilter, test_suite_cropped_color_filter);

// Port of: tests/FilterResultTest.cpp#L2178-L2228 (chrome/m156)
fn test_suite_crop_between_color_filters(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "Crop between regular color filters")
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(alpha_modulate(0.8), Expect::DeferredImage)
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .apply_color_filter(alpha_modulate(0.4), Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));

        if tm == TileMode::Decal {
            TestCase::new(
                r,
                "Crop between transparency-affecting color filters requires new image",
            )
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
            .apply_crop(
                IRect::new(8, 8, 24, 24),
                TileMode::Decal,
                Expect::DeferredImage,
            )
            .apply_color_filter(affect_transparent(K_RED), Expect::NewImage)
            .run(IRect::new(0, 0, 32, 32));

            TestCase::new(
                r,
                "Output-constrained crop between transparency-affecting filters does not",
            )
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
            .apply_crop(
                IRect::new(8, 8, 24, 24),
                TileMode::Decal,
                Expect::DeferredImage,
            )
            .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
            .run(IRect::new(8, 8, 24, 24));
        } else {
            TestCase::new(
                r,
                "Tiling between transparency-affecting color filters defers image",
            )
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));
        }

        TestCase::new(r, "Crop between regular and ATB color filters")
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));

        TestCase::new(r, "Crop between ATB and regular color filters")
            .source(IRect::new(0, 0, 32, 32))
            .apply_color_filter(affect_transparent(K_RED), Expect::DeferredImage)
            .apply_crop(IRect::new(8, 8, 24, 24), tm, Expect::DeferredImage)
            .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
            .run(IRect::new(0, 0, 32, 32));
    }
}
def_filter_result_suite!(
    CropBetweenColorFilters,
    test_suite_crop_between_color_filters
);

// Port of: tests/FilterResultTest.cpp#L2230-L2258 (chrome/m156)
fn test_suite_color_filter_between_crops(r: &mut TestRunner<'_>) {
    for first_tm in TILE_MODES {
        for second_tm in TILE_MODES {
            let new_image_if_not_decal_or_double_clamp = if second_tm != TileMode::Decal
                && !(second_tm == TileMode::Clamp && first_tm == TileMode::Clamp)
            {
                Expect::NewImage
            } else {
                Expect::DeferredImage
            };
            let expected_tm = if second_tm == TileMode::Decal {
                first_tm
            } else {
                second_tm
            };

            TestCase::new(r, "Regular color filter between crops")
                .source(IRect::new(0, 0, 32, 32))
                .apply_crop(IRect::new(4, 4, 24, 24), first_tm, Expect::DeferredImage)
                .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
                .apply_crop_expecting(
                    IRect::new(15, 15, 32, 32),
                    second_tm,
                    new_image_if_not_decal_or_double_clamp,
                    Some(expected_tm),
                    None,
                )
                .run(IRect::new(0, 0, 32, 32));

            TestCase::new(r, "Transparency-affecting color filter between crops")
                .source(IRect::new(0, 0, 32, 32))
                .apply_crop(IRect::new(4, 4, 24, 24), first_tm, Expect::DeferredImage)
                .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
                .apply_crop_expecting(
                    IRect::new(15, 15, 32, 32),
                    second_tm,
                    new_image_if_not_decal_or_double_clamp,
                    Some(expected_tm),
                    None,
                )
                .run(IRect::new(0, 0, 32, 32));
        }
    }
}
def_filter_result_suite!(
    ColorFilterBetweenCrops,
    test_suite_color_filter_between_crops
);

// Port of: tests/FilterResultTest.cpp#L2260-L2305 (chrome/m156)
fn test_suite_cropped_transformed_color_filter(r: &mut TestRunner<'_>) {
    let rotate = || Matrix::rotate_deg_pivot(30.0, (16.0, 16.0));

    TestCase::new(r, "Transform -> crop -> regular color filter")
        .source(IRect::new(0, 0, 32, 32))
        .apply_transform_default(rotate(), Expect::DeferredImage)
        .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "Transform -> regular color filter -> crop")
        .source(IRect::new(0, 0, 32, 32))
        .apply_transform_default(rotate(), Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "Crop -> transform -> regular color filter")
        .source(IRect::new(0, 0, 32, 32))
        .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
        .apply_transform_default(rotate(), Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "Crop -> regular color filter -> transform")
        .source(IRect::new(0, 0, 32, 32))
        .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_transform_default(rotate(), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "Regular color filter -> transform -> crop")
        .source(IRect::new(0, 0, 32, 32))
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_transform_default(rotate(), Expect::DeferredImage)
        .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));

    TestCase::new(r, "Regular color filter -> crop -> transform")
        .source(IRect::new(0, 0, 32, 32))
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
        .apply_transform_default(rotate(), Expect::DeferredImage)
        .run(IRect::new(0, 0, 32, 32));
}
def_filter_result_suite!(
    CroppedTransformedColorFilter,
    test_suite_cropped_transformed_color_filter
);

// Port of: tests/FilterResultTest.cpp#L2307-L2374 (chrome/m156)
fn test_suite_cropped_transformed_transparency_affecting_color_filter(r: &mut TestRunner<'_>) {
    // When the crop is not between the transform and transparency-affecting color filter,
    // either the order of operations or the bounds propagation means that every action can be
    // deferred. Below, when the crop is between the two actions, new images are triggered.
    let rotate = || Matrix::rotate_deg_pivot(30.0, (16.0, 16.0));

    TestCase::new(
        r,
        "Transform -> transparency-affecting color filter -> crop",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_transform_default(rotate(), Expect::DeferredImage)
    .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .run(IRect::new(0, 0, 32, 32));

    TestCase::new(
        r,
        "Crop -> transform -> transparency-affecting color filter",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .apply_transform_default(rotate(), Expect::DeferredImage)
    .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
    .run(IRect::new(0, 0, 32, 32));

    TestCase::new(
        r,
        "Crop -> transparency-affecting color filter -> transform",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
    .apply_transform_default(rotate(), Expect::DeferredImage)
    .run(IRect::new(0, 0, 32, 32));

    TestCase::new(
        r,
        "Transparency-affecting color filter -> transform -> crop",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
    .apply_transform_default(rotate(), Expect::DeferredImage)
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .run(IRect::new(0, 0, 32, 32));

    // Since the crop is between the transform and color filter (or vice versa), transparency
    // outside the crop is introduced that should not be affected by the color filter were no
    // new image to be created.
    TestCase::new(
        r,
        "Transform -> crop -> transparency-affecting color filter",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_transform_default(rotate(), Expect::DeferredImage)
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .apply_color_filter(affect_transparent(K_GREEN), Expect::NewImage)
    .run(IRect::new(0, 0, 32, 32));

    TestCase::new(
        r,
        "Transparency-affecting color filter -> crop -> transform",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .apply_transform_default(rotate(), Expect::NewImage)
    .run(IRect::new(0, 0, 32, 32));

    // However if the output is small enough to fit within the transformed interior, the
    // transparency is not visible.
    TestCase::new(
        r,
        "Transform -> crop -> transparency-affecting color filter",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_transform_default(rotate(), Expect::DeferredImage)
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
    .run(IRect::new(15, 15, 21, 21));

    TestCase::new(
        r,
        "Transparency-affecting color filter -> crop -> transform",
    )
    .source(IRect::new(0, 0, 32, 32))
    .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
    .apply_crop_decal(IRect::new(2, 2, 30, 30), Expect::DeferredImage)
    .apply_transform_default(rotate(), Expect::DeferredImage)
    .run(IRect::new(15, 15, 21, 21));
}
def_filter_result_suite!(
    CroppedTransformedTransparencyAffectingColorFilter,
    test_suite_cropped_transformed_transparency_affecting_color_filter
);

// Port of: tests/FilterResultTest.cpp#L2376-L2401 (chrome/m156)
fn test_suite_backdrop_filter_rotated(r: &mut TestRunner<'_>) {
    // These values are extracted from a cc_unittest that had a 200x200 image, with a 10-degree
    // rotated 100x200 layer over the right half of the base image, with a backdrop blur. The
    // rotation forces SkCanvas to crop and transform the base device's content to be aligned with
    // the layer space of the blur. The rotation is such that the backdrop image must be clamped
    // (hence the first crop) and the clamp tiling remains visible in the layer image. However,
    // floating point precision in the layer bounds analysis was causing FilterResult to think that
    // the layer decal was also visible so the first crop would be resolved before the transform was
    // applied.
    //
    // While it's expected that the second clamping crop (part of the blur effect), forces the
    // transform and first clamp to be resolved, we were incorrectly producing two new images
    // instead of just one.
    TestCase::new(r, "Layer decal shouldn't be visible")
        .source(IRect::new(65, 0, 199, 200))
        .apply_crop(
            IRect::new(65, 0, 199, 200),
            TileMode::Clamp,
            Expect::DeferredImage,
        )
        .apply_transform_default(
            Matrix::new_all(
                0.984_808, 0.173_648, -98.4808, -0.173_648, 0.984_808, 17.3648, 0.0, 0.0, 1.0,
            ),
            Expect::DeferredImage,
        )
        .apply_crop(
            IRect::new(0, 0, 100, 200),
            TileMode::Clamp,
            Expect::NewImage,
        )
        .run(IRect::new(-15, -15, 115, 215));
}
def_filter_result_suite!(BackdropFilterRotated, test_suite_backdrop_filter_rotated);

// Port of: tests/FilterResultTest.cpp#L2406-L2573 (chrome/m156)
#[allow(clippy::too_many_lines)] // the C++ body is kept whole so it can be compared line by line
fn test_suite_rescale_with_tile_mode(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "Identity rescale is a no-op")
            .source(IRect::new(0, 0, 50, 50))
            .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
            .rescale(sz(1.0, 1.0), Expect::DeferredImage, None)
            .run(IRect::new(-5, -5, 55, 55));

        TestCase::with_tolerances(
            r,
            "Near identity rescale is a no-op",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            // transparentCheckBorderPadding
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(0, 0, 50, 50))
        .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
        .rescale(NEARLY_IDENTITY, Expect::DeferredImage, None)
        .run(IRect::new(-5, -5, 55, 55));

        // NOTE: As the scale factor decreases and more decimation steps are required, the testing
        // allowed tolerances increase greatly. These were chosen as "acceptable" after reviewing
        // the expected vs. actual images. The results diverge due to differences in the simple
        // expected decimation and the actual rescale() implementation, as well as how small the
        // final images become.
        //
        // Similarly, the allowed transparent border tolerance must be increased for kDecal tests
        // because the expected image's content is expanded by a larger and larger factor during its
        // upscale.
        TestCase::with_tolerances(
            r,
            "1-step rescale preserves tile mode",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.5, 0.5), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        let periodic = tm == TileMode::Repeat || tm == TileMode::Mirror;
        TestCase::with_tolerances(
            r,
            "2-step rescale preserves tile mode",
            if tm == TileMode::Decal {
                5.9
            } else if periodic {
                2.5
            } else {
                1.0
            },
            DEFAULT_RGB_TOLERANCE,
            if tm == TileMode::Decal { 2 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.25, 0.25), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "2-step rescale with near-identity elision",
            if periodic { 13.0 } else { 30.0 },
            DEFAULT_RGB_TOLERANCE + 2.0 / 255.0,
            if tm == TileMode::Decal { 8 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.23, 0.23), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "3-step rescale preserves tile mode",
            28.0,
            DEFAULT_RGB_TOLERANCE + 4.0 / 255.0,
            if tm == TileMode::Decal { 10 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.155, 0.155), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        // Non-uniform scales
        TestCase::with_tolerances(
            r,
            "Identity X axis, near-identity Y axis is a no-op",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(1.0, NEARLY_IDENTITY.height), Expect::DeferredImage, None)
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "Near-identity X axis, identity Y axis is a no-op",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(NEARLY_IDENTITY.width, 1.0), Expect::DeferredImage, None)
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "Identity X axis, 1-step Y axis preserves tile mode",
            if tm == TileMode::Mirror { 1.32 } else { 1.0 },
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(1.0, 0.5), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "Near-identity X axis, 1-step Y axis preserves tile mode",
            if tm == TileMode::Mirror { 1.7 } else { 1.0 },
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(NEARLY_IDENTITY.width, 0.5), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "Identity X axis, 2-step Y axis preserves tile mode",
            3.1,
            DEFAULT_RGB_TOLERANCE,
            if tm == TileMode::Decal { 2 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(1.0, 0.25), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "1-step X axis, 2-step Y axis preserves tile mode",
            if periodic { 16.5 } else { 14.0 },
            DEFAULT_RGB_TOLERANCE + 2.0 / 255.0,
            if tm == TileMode::Decal { 5 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.55, 0.27), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "1-step X axis, identity Y axis preserves tile mode",
            if tm == TileMode::Mirror { 1.2 } else { 1.0 },
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.5, 1.0), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "1-step X axis, near-identity Y axis preserves tile mode",
            if tm == TileMode::Mirror { 1.7 } else { 1.0 },
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.5, NEARLY_IDENTITY.height), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "2-step X axis, identity Y axis preserves tile mode",
            3.1,
            DEFAULT_RGB_TOLERANCE,
            if tm == TileMode::Decal { 2 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.25, 1.0), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "2-step X axis, 1-step Y axis preserves tile mode",
            if periodic { 13.0 } else { 10.5 },
            DEFAULT_RGB_TOLERANCE + 2.0 / 255.0,
            if tm == TileMode::Decal { 5 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .rescale(sz(0.27, 0.55), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        // Chained decal tile modes don't create the circumstances of interest.
        if tm == TileMode::Decal {
            continue;
        }
        TestCase::with_tolerances(
            r,
            "Rescale applies layer bounds",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            1,
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .apply_crop_expecting(
            IRect::new(4, 4, 76, 76),
            TileMode::Decal,
            Expect::DeferredImage,
            Some(tm),
            None,
        )
        .rescale(sz(0.5, 0.5), Expect::NewImage, Some(TileMode::Decal))
        .run(IRect::new(0, 0, 80, 80));
    }
}
def_filter_result_suite!(RescaleWithTileMode, test_suite_rescale_with_tile_mode);

// Port of: tests/FilterResultTest.cpp#L2575-L2674 (chrome/m156)
#[allow(clippy::too_many_lines)] // the C++ body is kept whole so it can be compared line by line
fn test_suite_rescale_with_transform(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(r, "Identity rescale defers integer translation")
            .source(IRect::new(0, 0, 50, 50))
            .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
            .apply_transform_default(Matrix::translate((-10.0, -10.0)), Expect::DeferredImage)
            .rescale(sz(1.0, 1.0), Expect::DeferredImage, None)
            .run(IRect::new(-15, -15, 45, 45));

        TestCase::new(r, "Identity rescale applies complex transform")
            .source(IRect::new(16, 16, 64, 64))
            .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
            .apply_transform_default(
                Matrix::rotate_deg_pivot(45.0, (16.0, 16.0)),
                Expect::DeferredImage,
            )
            .rescale(sz(1.0, 1.0), Expect::NewImage, Some(TileMode::Decal))
            .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "Near-identity rescale defers integer translation",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(0, 0, 50, 50))
        .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
        .apply_transform_default(Matrix::translate((-10.0, -10.0)), Expect::DeferredImage)
        .rescale(NEARLY_IDENTITY, Expect::DeferredImage, None)
        .run(IRect::new(-15, -15, 45, 45));

        TestCase::new(r, "Near-identity rescale applies complex transform")
            .source(IRect::new(0, 0, 50, 50))
            .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
            .apply_transform_default(
                Matrix::rotate_deg_pivot(15.0, (25.0, 25.0)),
                Expect::DeferredImage,
            )
            .rescale(NEARLY_IDENTITY, Expect::NewImage, Some(TileMode::Decal))
            .run(IRect::new(-5, -5, 55, 55));

        TestCase::new(
            r,
            "Identity rescale with deferred scale applies transform in first step",
        )
        .source(IRect::new(0, 0, 50, 50))
        .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
        .apply_transform_default(Matrix::scale((0.4, 0.4)), Expect::DeferredImage)
        .rescale(sz(1.0, 1.0), Expect::NewImage, Some(TileMode::Decal))
        .run(IRect::new(-10, -10, 30, 30));

        TestCase::with_tolerances(
            r,
            "Near-identity rescale with deferred scale applies transform in first step",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(0, 0, 50, 50))
        .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
        .apply_transform_default(Matrix::scale((0.4, 0.4)), Expect::DeferredImage)
        .rescale(NEARLY_IDENTITY, Expect::NewImage, Some(TileMode::Decal))
        .run(IRect::new(-10, -10, 30, 30));

        let periodic = tm == TileMode::Repeat || tm == TileMode::Mirror;

        TestCase::with_tolerances(
            r,
            "1-step rescale applies complex transform in first step",
            if periodic {
                1.1
            } else {
                DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF
            },
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (16.0, 16.0)),
            Expect::DeferredImage,
        )
        .rescale(sz(0.5, 0.5), Expect::NewImage, Some(TileMode::Decal))
        .run(IRect::new(0, 0, 80, 80));

        TestCase::with_tolerances(
            r,
            "2-step rescale applies complex transform",
            if periodic { 10.05 } else { 3.7 },
            DEFAULT_RGB_TOLERANCE,
            if tm == TileMode::Decal { 4 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .apply_transform_default(
            Matrix::rotate_deg_pivot(45.0, (16.0, 16.0)),
            Expect::DeferredImage,
        )
        .rescale(sz(0.25, 0.25), Expect::NewImage, Some(TileMode::Decal))
        .run(IRect::new(0, 0, 80, 80));

        // W/o resolving the deferred transform, the first rescale step could end up with a scale
        // that's much less than 1/2 and sampling would miss a lot of data.
        TestCase::with_tolerances(
            r,
            "Rescale with deferred downscale applies transform before first step",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .apply_transform_default(Matrix::scale((0.4, 0.4)), Expect::DeferredImage)
        .rescale(sz(0.5, 0.5), Expect::NewImage, Some(TileMode::Decal))
        .run(IRect::new(0, 0, 80, 80));

        // But for upscaling, it doesn't contribute to such sampling errors.
        TestCase::with_tolerances(
            r,
            "Rescale with deferred upscale applies transform with first step",
            2.55,
            DEFAULT_RGB_TOLERANCE,
            if tm == TileMode::Decal { 3 } else { 0 },
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .apply_transform_default(Matrix::scale((1.5, 1.5)), Expect::DeferredImage)
        .rescale(sz(0.5, 0.5), Expect::NewImage, Some(TileMode::Decal))
        .run(IRect::new(0, 0, 80, 80));
    }
}
def_filter_result_suite!(RescaleWithTransform, test_suite_rescale_with_transform);

// Port of: tests/FilterResultTest.cpp#L2676-L2720 (chrome/m156)
fn test_suite_rescale_with_color_filter(r: &mut TestRunner<'_>) {
    for tm in TILE_MODES {
        r.set_label(tile_mode_name(tm));
        TestCase::new(
            r,
            "Identity rescale applies color filter but defers tile mode",
        )
        .source(IRect::new(0, 0, 50, 50))
        .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .rescale(sz(1.0, 1.0), Expect::NewImage, Some(tm))
        .run(IRect::new(-5, -5, 55, 55));

        TestCase::with_tolerances(
            r,
            "Near-identity rescale applies color filter but defers tile mode",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(0, 0, 50, 50))
        .apply_crop(IRect::new(0, 0, 50, 50), tm, Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.5), Expect::DeferredImage)
        .rescale(NEARLY_IDENTITY, Expect::NewImage, Some(tm))
        .run(IRect::new(-5, -5, 55, 55));

        TestCase::with_tolerances(
            r,
            "Rescale applies color filter but defers tile mode",
            DEFAULT_MAX_ALLOWED_PERCENT_IMAGE_DIFF,
            DEFAULT_RGB_TOLERANCE,
            i32::from(tm == TileMode::Decal),
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .apply_color_filter(alpha_modulate(0.75), Expect::DeferredImage)
        .rescale(sz(0.5, 0.5), Expect::NewImage, Some(tm))
        .run(IRect::new(0, 0, 80, 80));

        // The color filter (simple and transparency-affecting) should be applied with a 1px
        // boundary around the rest of the image being rescaled when decal-tiled, so its result is
        // clamped tiled instead (vs. having to prepare and scale a larger, flood-filled image).
        let expected_tile_mode = if tm == TileMode::Decal {
            TileMode::Clamp
        } else {
            tm
        };
        TestCase::new(
            r,
            "Rescale applies transparency-affecting color filter but defers tile mode",
        )
        .source(IRect::new(16, 16, 64, 64))
        .apply_crop(IRect::new(16, 16, 64, 64), tm, Expect::DeferredImage)
        .apply_color_filter(affect_transparent(K_GREEN), Expect::DeferredImage)
        .rescale(sz(0.5, 0.5), Expect::NewImage, Some(expected_tile_mode))
        .run(IRect::new(0, 0, 80, 80));
    }
}
def_filter_result_suite!(RescaleWithColorFilter, test_suite_rescale_with_color_filter);

// Port of: tests/FilterResultTest.cpp#L2722-L2785 (chrome/m156)
fn test_suite_make_from_image(r: &mut TestRunner<'_>) {
    const SRC_SIZE: ISize = ISize {
        width: 128,
        height: 128,
    };
    let identity_src = IRect::new(0, 0, 128, 128);
    let subset_src = IRect::new(16, 16, 112, 112);
    let overlapping_src = IRect::new(-64, 16, 192, 112);
    let containing_src = IRect::new(-64, -64, 192, 192);
    let disjoint_src = IRect::new(0, -200, 128, -1);

    // For convenience, most tests will use kIdentitySrc as the dstRect so that the result's
    // layer bounds can be used to validate the src->dst transform is preserved.
    let dst_rect = identity_src;

    // Sufficiently large to not affect the layer bounds of a FilterResult.
    let desired_output = IRect::new(-400, -400, 400, 400);

    let color_space = ColorSpace::new_srgb();
    let ctx = Context::new(
        r.ref_backend(),
        Mapping::new(),
        desired_output,
        FilterResult::default(),
        Some(color_space.clone()),
        None,
    );

    let source = r.create_source_image(SRC_SIZE, color_space);
    debug_assert_eq!(source.subset(), identity_src);
    let source_image = source.as_image().expect("source->asImage()");

    let make_image = |src: IRect, dst: IRect| {
        FilterResult::make_from_image(
            &ctx,
            &source_image,
            Rect::from(src),
            Rect::from(dst),
            SamplingOptions::default(),
        )
    };

    // Failure cases should return an empty FilterResult
    reporter_assert!(
        r.reporter,
        !make_image(identity_src, IRect::new_empty()).has_image(),
        "Empty dst rect returns empty FilterResult"
    );
    reporter_assert!(
        r.reporter,
        !make_image(IRect::new_empty(), dst_rect).has_image(),
        "Empty src rect returns empty FilterResult"
    );
    reporter_assert!(
        r.reporter,
        !make_image(disjoint_src, dst_rect).has_image(),
        "Disjoint src rect returns empty FilterREsult"
    );

    let mut test_success =
        |src: IRect, expected_image_subset: IRect, expected_layer_bounds: IRect, label: &str| {
            let result = make_image(src, dst_rect);
            reporter_assert!(
                r.reporter,
                result.has_image(),
                "Image should not be empty: {label}"
            );
            reporter_assert!(
                r.reporter,
                result.image().map(SpecialImage::subset) == Some(expected_image_subset),
                "Result subset is incorrect: {label}"
            );
            reporter_assert!(
                r.reporter,
                result.layer_bounds() == expected_layer_bounds,
                "Result layer bounds are incorrect: {label}"
            );
        };

    test_success(
        identity_src,
        identity_src,
        dst_rect,
        "Identity src->dst preserves original image bounds",
    );
    test_success(
        subset_src,
        subset_src,
        dst_rect,
        "Contained src rect is preserved, stretched to original dst bounds",
    );
    test_success(
        overlapping_src,
        IRect::new(0, 16, 128, 112),
        IRect::new(32, 0, 96, 128),
        "Overlapping src rect is clipped and dst is scaled on clipped axis",
    );
    test_success(
        containing_src,
        identity_src,
        IRect::new(32, 32, 96, 96),
        "Containing src rect is clipped and dst is scaled on both axes",
    );
}
def_filter_result_suite!(MakeFromImage, test_suite_make_from_image);

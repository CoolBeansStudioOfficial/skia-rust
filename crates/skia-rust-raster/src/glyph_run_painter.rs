// Copyright 2018 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkGlyphRunPainter.{h,cpp} (chrome/m156), the CPU painter
// `skcpu::GlyphRunListPainter`.
//
// Glyph drawables are drawn with the canvas (`canvas->saveLayer(); drawable->draw(canvas)`),
// which a device cannot reach: the painter collects them (`PendingGlyphDrawable`) and the canvas
// draws them when the device call returns (`Device::take_pending_glyph_drawables`).

//! The CPU glyph painter: chooses, for each run, whether its glyphs draw as paths, as masks
//! at device positions, or as scaled masks, and hands the accepted glyphs to the device.

use std::sync::Arc;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::device::PendingGlyphDrawable;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::glyph::{
    ActionType, Glyph, GlyphAction, GlyphDigest, GlyphPositionRoundingSpec,
};
use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::AddPathMode;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{scalar, scalar_floor_to_scalar};
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike::StrikeGuard;
use skia_rust_core::strike_spec::{BulkGlyphMetrics, StrikeSpec};
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps};

/// The part of `skcpu::Draw` that the glyph painter paints through (`BitmapDevicePainter`).
// Port of: src/core/SkGlyphRunPainter.h#L20-L35 (chrome/m156), BitmapDevicePainter
pub trait BitmapDevicePainter {
    /// `paintMasks`: paints the accepted glyph masks at their device positions.
    fn paint_masks(&mut self, accepted: &[(&Glyph, Point)], paint: &Paint);

    /// `drawBitmap(bitmap, matrix, dstOrNull, sampling, paint, mips)`: draws a pre-rasterized
    /// bitmap (the color glyphs that are drawn scaled).
    fn draw_bitmap(
        &mut self,
        bitmap: &Bitmap,
        matrix: &Matrix,
        dst_or_null: Option<&Rect>,
        sampling: &SamplingOptions,
        paint: &Paint,
        mips: Option<Arc<Mipmap>>,
    );

    /// `canvas->concat(m); canvas->drawPath(path, paint)`: draws `path` with `matrix` applied
    /// before the device transform.
    fn draw_glyph_path_concat(&mut self, path: &Path, matrix: &Matrix, paint: &Paint);

    /// `canvas->drawPath(path, paint)` with the current transform: the path is already in
    /// device space.
    fn draw_glyph_path_device(&mut self, path: &Path, paint: &Paint);
}

/// The glyph painter of one device (`skcpu::GlyphRunListPainter`): the props, the color type and
/// the scaler context flags of the device.
// Port of: src/core/SkGlyphRunPainter.h#L22-L45 (chrome/m156)
#[derive(Clone, Debug)]
pub struct GlyphRunListPainter {
    /// The props as on the actual device (`fDeviceProps`).
    device_props: SurfaceProps,
    /// The props for when the bitmap device cannot draw LCD text (`fBitmapFallbackProps`).
    bitmap_fallback_props: SurfaceProps,
    /// The device's color type (`fColorType`).
    color_type: ColorType,
    /// The scaler context flags (`fScalerContextFlags`).
    scaler_context_flags: ScalerContextBuildFlags,
}

impl GlyphRunListPainter {
    /// `GlyphRunListPainter(props, colorType, cs)`.
    // Port of: src/core/SkGlyphRunPainter.cpp#L211-L217 (chrome/m156)
    #[must_use]
    pub fn new(props: SurfaceProps, color_type: ColorType, cs: Option<&ColorSpace>) -> Self {
        Self {
            device_props: props,
            bitmap_fallback_props: props.clone_with_pixel_geometry(PixelGeometry::Unknown),
            color_type,
            scaler_context_flags: compute_scaler_context_flags(cs),
        }
    }

    /// `drawForBitmapDevice`: draws every run of `list` on `device`, with `draw_matrix` as the
    /// device transform for choosing glyph images.
    ///
    /// # Panics
    ///
    /// When the paint has a path effect or mask filter (its strike descriptor is not ported).
    ///
    /// Glyph drawables are not drawn but added to `pending_drawables` (see the module docs).
    // Port of: src/core/SkGlyphRunPainter.cpp#L219-L423 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    pub fn draw_for_bitmap_device(
        &self,
        device: &mut dyn BitmapDevicePainter,
        list: &GlyphRunList<'_>,
        paint: &Paint,
        draw_matrix: &Matrix,
        pending_drawables: &mut Vec<PendingGlyphDrawable>,
    ) {
        // The bitmap blitters can only draw LCD text to a N32 bitmap in srcOver. Otherwise,
        // convert the lcd text into A8 text. The props communicate this to the scaler.
        let props = if self.color_type == ColorType::N32 && paint.is_src_over() {
            self.device_props
        } else {
            self.bitmap_fallback_props
        };
        let flags = self.scaler_context_flags;

        let draw_origin = list.origin();
        let mut position_matrix = draw_matrix.clone();
        position_matrix.pre_translate(draw_origin);

        for run in list.runs() {
            let run_font = run.font();
            let mut source: Vec<(GlyphId, Point)> = run.source().collect();

            if StrikeSpec::should_draw_as_path(paint, run_font, &position_matrix) {
                let (spec, strike_to_source_scale) =
                    StrikeSpec::make_path(run_font, paint, &props, flags);
                let strike = spec.find_or_create_strike();
                {
                    let mut guard = strike.lock();
                    let (accepted, rejected) =
                        prepare_for_drawing(&mut guard, ActionType::Path, &source);
                    source = rejected;

                    // The paint we draw paths with must have the same anti-aliasing state as the
                    // runFont allowing the paths to have the same edging as the glyph masks.
                    let mut path_paint = paint.clone();
                    path_paint.set_anti_alias(run_font.has_some_anti_aliasing());
                    let stroking = path_paint.style() != Style::Fill;
                    let hairline = path_paint.stroke_width() == 0.0;
                    let needs_exact_ctm = path_paint.shader().is_some()
                        || path_paint.path_effect().is_some()
                        || path_paint.mask_filter().is_some()
                        || (stroking && !hairline);

                    for (digest, pos) in accepted {
                        let glyph = guard.glyph(digest);
                        let Some(path) = glyph.path() else {
                            panic!("a glyph accepted for paths has a path");
                        };
                        let translate = Point::new(draw_origin.x + pos.x, draw_origin.y + pos.y);
                        let mut m = Matrix::default();
                        m.set_scale_translate(
                            (strike_to_source_scale, strike_to_source_scale),
                            translate,
                        );
                        if needs_exact_ctm {
                            let mut builder = PathBuilder::new();
                            builder.add_path_with_transform(path, &m, AddPathMode::Append);
                            builder.set_is_volatile(true);
                            device.draw_glyph_path_device(&builder.detach(), &path_paint);
                        } else {
                            device.draw_glyph_path_concat(path, &m, &path_paint);
                        }
                    }
                }
                if !source.is_empty() {
                    let mut guard = strike.lock();
                    let (accepted, rejected) =
                        prepare_for_drawing(&mut guard, ActionType::Drawable, &source);
                    source = rejected;

                    for (digest, pos) in accepted {
                        let glyph = guard.glyph(digest);
                        let Some(drawable) = glyph.drawable() else {
                            panic!("a glyph accepted for drawables has a drawable");
                        };
                        let translate = Point::new(draw_origin.x + pos.x, draw_origin.y + pos.y);
                        let mut m = Matrix::default();
                        m.set_scale_translate(
                            (strike_to_source_scale, strike_to_source_scale),
                            translate,
                        );
                        // The canvas draws it: saveLayer(m.mapRect(bounds), paint); draw(m).
                        pending_drawables.push(PendingGlyphDrawable {
                            drawable: drawable.clone(),
                            matrix: m,
                            paint: paint.clone(),
                        });
                    }
                }
            }

            if !source.is_empty() && !position_matrix.has_perspective() {
                let spec = StrikeSpec::make_mask(run_font, paint, &props, flags, &position_matrix);
                let strike = spec.find_or_create_strike();
                let mut guard = strike.lock();
                let rounding = *strike.rounding_spec();
                let (accepted, rejected) = prepare_for_direct_mask_drawing(
                    &mut guard,
                    &rounding,
                    &position_matrix,
                    &source,
                );
                source = rejected;
                let accepted: Vec<(&Glyph, Point)> = accepted
                    .iter()
                    .map(|(digest, pos)| (guard.glyph(*digest), *pos))
                    .collect();
                device.paint_masks(&accepted, paint);
            }

            if !source.is_empty() {
                // Create a strike in source space to calculate scale information.
                let scale_strike_spec =
                    StrikeSpec::make_mask(run_font, paint, &props, flags, Matrix::i());
                let glyph_ids: Vec<GlyphId> = source.iter().map(|(id, _)| *id).collect();
                let glyphs = BulkGlyphMetrics::new(&scale_strike_spec).glyphs(&glyph_ids);

                let mut max_scale = skia_rust_core::scalar::SCALAR_MIN;
                // Calculate the scale that makes the longest edge 1:1 with its side in the cache.
                for (glyph, (_, pos)) in glyphs.iter().zip(&source) {
                    if glyph.is_empty() {
                        continue;
                    }
                    let mut rect = glyph.rect();
                    rect.offset(Point::new(draw_origin.x + pos.x, draw_origin.y + pos.y));
                    let corners = position_matrix.map_rect_to_quad(rect);
                    // left top -> right top
                    let scale = (corners[1] - corners[0]).length() / rect.width();
                    max_scale = std_max(max_scale, scale);
                    // right top -> right bottom
                    let scale = (corners[2] - corners[1]).length() / rect.height();
                    max_scale = std_max(max_scale, scale);
                    // right bottom -> left bottom
                    let scale = (corners[3] - corners[2]).length() / rect.width();
                    max_scale = std_max(max_scale, scale);
                    // left bottom -> left top
                    let scale = (corners[0] - corners[3]).length() / rect.height();
                    max_scale = std_max(max_scale, scale);
                }
                if max_scale <= 0.0 {
                    continue; // to the next run.
                }
                if max_scale * run_font.size() > 256.0 {
                    max_scale = 256.0 / run_font.size();
                }
                let mut cache_scale = Matrix::default();
                cache_scale.set_scale((max_scale, max_scale), None);
                let strike_spec =
                    StrikeSpec::make_mask(run_font, paint, &props, flags, &cache_scale);
                let strike = strike_spec.find_or_create_strike();
                let rounding = *strike.rounding_spec();
                let mut guard = strike.lock();
                let (accepted, _rejected) = prepare_for_direct_bitmap_drawing(
                    &mut guard,
                    &rounding,
                    &position_matrix,
                    &source,
                );
                let inv_max_scale = 1.0 / max_scale;
                for (digest, src_pos) in accepted {
                    let mask = guard.glyph(digest).mask();
                    // TODO: is this needed will A8 and BW just work?
                    if mask.format != MaskFormat::Argb32 {
                        continue;
                    }
                    let mut bm = Bitmap::new();
                    let installed = bm.install_pixels(
                        &ImageInfo::new_n32_premul((mask.bounds.width(), mask.bounds.height()), None),
                        mask.image.to_vec(),
                        mask.row_bytes as usize,
                    );
                    debug_assert!(installed);
                    bm.set_immutable();

                    // Since the glyph in the cache is scaled by maxScale, its top left vector is
                    // too long. Reduce it to find proper positions on the device.
                    #[allow(clippy::cast_precision_loss)] // SkIRect ints to SkScalar
                    let pos = Point::new(
                        draw_origin.x + src_pos.x + (mask.bounds.left as scalar) * inv_max_scale,
                        draw_origin.y + src_pos.y + (mask.bounds.top as scalar) * inv_max_scale,
                    );

                    // Calculate the preConcat matrix for drawBitmap to get the rectangle from the
                    // glyph cache (which is multiplied by maxScale) to land in the right place.
                    let mut translate = Matrix::translate(pos);
                    translate.pre_scale((inv_max_scale, inv_max_scale), None);

                    // Draw the bitmap using the rect from the scaled cache, and not the source
                    // rectangle for the glyph.
                    device.draw_bitmap(
                        &bm,
                        &translate,
                        None,
                        &SamplingOptions::from(FilterMode::Linear),
                        paint,
                        None,
                    );
                }
            }
        }
    }
}

/// The glyphs accepted for a drawing (digest and position) and the glyphs rejected (id and
/// position).
type Prepared = (Vec<(GlyphDigest, Point)>, Vec<(GlyphId, Point)>);

/// `compute_scaler_context_flags`: a linear color space drops the gamma hacks; otherwise they stay
/// on, and the contrast boost always applies.
// Port of: src/core/SkGlyphRunPainter.cpp#L47-L56 (chrome/m156)
fn compute_scaler_context_flags(cs: Option<&ColorSpace>) -> ScalerContextBuildFlags {
    match cs {
        Some(cs) if cs.gamma_is_linear() => ScalerContextBuildFlags::BOOST_CONTRAST,
        _ => ScalerContextBuildFlags::FAKE_GAMMA_AND_BOOST_CONTRAST,
    }
}

/// `std::max(a, b)`: `b` only when `a < b`, so a NaN in `b` never wins.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

/// `SkIsFinite(pos.x(), pos.y())`.
fn is_finite_position(pos: Point) -> bool {
    pos.x.is_finite() && pos.y.is_finite()
}

/// The accepted glyphs (digest and position) and the rejected ones (id and position) of one
/// drawing action (`prepare_for_path_drawing` and `prepare_for_drawable_drawing`, which are the
/// same code with a different action). Non-finite positions are dropped.
// Port of: src/core/SkGlyphRunPainter.cpp#L60-L118 (chrome/m156)
fn prepare_for_drawing(
    guard: &mut StrikeGuard<'_>,
    action: ActionType,
    source: &[(GlyphId, Point)],
) -> Prepared {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for &(glyph_id, pos) in source {
        if !is_finite_position(pos) {
            continue;
        }
        let packed_id = PackedGlyphId::from_glyph_id(glyph_id);
        let digest = guard.digest_for(action, packed_id);
        match digest.action_for(action) {
            GlyphAction::Accept => accepted.push((digest, pos)),
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected)
}

/// `prepare_for_direct_mask_drawing`: maps each glyph's position to device space (plus the
/// rounding constant), packs the subpixel part into the glyph id, and accepts the glyph's mask at
/// the floored position.
// Port of: src/core/SkGlyphRunPainter.cpp#L121-L163 (chrome/m156)
fn prepare_for_direct_mask_drawing(
    guard: &mut StrikeGuard<'_>,
    rounding: &GlyphPositionRoundingSpec,
    creation_matrix: &Matrix,
    source: &[(GlyphId, Point)],
) -> Prepared {
    let mask = rounding.ignore_position_field_mask;
    let half_sample_freq = rounding.half_axis_sample_freq;
    // Build up the mapping from source space to device space. Add the rounding constant
    // halfSampleFreq, so we just need to floor to get the device result.
    let mut position_matrix_with_rounding = creation_matrix.clone();
    position_matrix_with_rounding.post_translate(half_sample_freq);

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for &(glyph_id, pos) in source {
        if !is_finite_position(pos) {
            continue;
        }
        let mapped_pos = position_matrix_with_rounding.map_point(pos);
        let packed_glyph_id = PackedGlyphId::from_point(glyph_id, mapped_pos, mask);
        let digest = guard.digest_for(ActionType::DirectMaskCpu, packed_glyph_id);
        match digest.action_for(ActionType::DirectMaskCpu) {
            GlyphAction::Accept => {
                let rounded_pos = Point::new(
                    scalar_floor_to_scalar(mapped_pos.x),
                    scalar_floor_to_scalar(mapped_pos.y),
                );
                accepted.push((digest, rounded_pos));
            }
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected)
}

/// `prepare_for_direct_bitmap_drawing`: as [`prepare_for_direct_mask_drawing`], but accepted
/// positions stay in source space.
// Port of: src/core/SkGlyphRunPainter.cpp#L165-L207 (chrome/m156)
fn prepare_for_direct_bitmap_drawing(
    guard: &mut StrikeGuard<'_>,
    rounding: &GlyphPositionRoundingSpec,
    creation_matrix: &Matrix,
    source: &[(GlyphId, Point)],
) -> Prepared {
    let mask = rounding.ignore_position_field_mask;
    let half_sample_freq = rounding.half_axis_sample_freq;
    let mut position_matrix_with_rounding = creation_matrix.clone();
    position_matrix_with_rounding.post_translate(half_sample_freq);

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for &(glyph_id, pos) in source {
        if !is_finite_position(pos) {
            continue;
        }
        let mapped_pos = position_matrix_with_rounding.map_point(pos);
        let packed_glyph_id = PackedGlyphId::from_point(glyph_id, mapped_pos, mask);
        let digest = guard.digest_for(ActionType::DirectMaskCpu, packed_glyph_id);
        match digest.action_for(ActionType::DirectMaskCpu) {
            GlyphAction::Accept => accepted.push((digest, pos)),
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected)
}

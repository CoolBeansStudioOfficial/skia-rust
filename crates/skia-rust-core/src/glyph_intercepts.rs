// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkGlyph.cpp (`calculate_path_gap`, `SkGlyph::ensureIntercepts`),
// src/core/SkTextBlob.cpp (`get_glyph_run_intercepts`) (chrome/m156)

//! The intercepts of a horizontal band with text: the x intervals where the band is inside the
//! outlines of the glyphs. These draw the gaps in an underline (`getIntercepts`).
//!
//! skia-rust: C++ caches each glyph's intercepts on the glyph (`PathData::fIntercept`). The
//! intervals here are computed on each request instead. A cached entry is the result of the same
//! computation for the same band, so the values are the same.

use crate::bezier_curves::{BezierCubic, BezierQuad};
use crate::floating_point::ieee_float_divide;
use crate::font_types::FontHinting;
use crate::glyph_run::GlyphRun;
use crate::paint::{Paint, Style};
use crate::path::{Iter, Path};
use crate::path_types::PathVerb;
use crate::point::Point;
use crate::scalar::scalar;
use crate::scaler_context::ScalerContextBuildFlags;
use crate::strike_spec::{BulkGlyphMetricsAndPaths, StrikeSpec};

/// The text size the outlines are cached at (`SkFontPriv::kCanonicalTextSizeForPaths`).
// Port of: src/core/SkFontPriv.h (chrome/m156), kCanonicalTextSizeForPaths
const CANONICAL_TEXT_SIZE_FOR_PATHS: scalar = 64.0;

/// The left and right ends of the gap around the path that a band `[top_offset, bottom_offset]`
/// crosses (`calculate_path_gap`): the smallest and largest x where the outline meets the band.
// Port of: src/core/SkGlyph.cpp#L450-L488 and L490-L509 (chrome/m156)
fn calculate_path_gap(top_offset: scalar, bottom_offset: scalar, path: &Path) -> (scalar, scalar) {
    // Left and Right of an ever expanding gap around the path.
    let mut left = f32::MAX;
    let mut right = f32::MIN;
    let mut expand_gap = |v: scalar| {
        left = left.min(v);
        right = right.max(v);
    };

    // Handle all the different verbs for the path.
    let add_line = |pts: &[Point], offset: scalar, expand_gap: &mut dyn FnMut(scalar)| {
        let t = ieee_float_divide(offset - pts[0].y, pts[1].y - pts[0].y);
        if (0.0..1.0).contains(&t) {
            // this handles divide by zero above
            expand_gap(pts[0].x + t * (pts[1].x - pts[0].x));
        }
    };

    // SkPath::Iter(path, false) turns a close into a line back to the start of the contour (when
    // the contour is open), unlike SkPathIter.
    let mut iter = Iter::new(path, false);
    while let Some(rec) = iter.next_rec() {
        let pts = rec.points();
        match rec.verb() {
            // A move or a close adds no gap.
            // There are no conic primitives in glyph outlines (SkDEBUGFAIL in C++).
            PathVerb::Move | PathVerb::Conic | PathVerb::Close => {}
            PathVerb::Line => {
                let (line_top, line_bottom) = min_max(&[pts[0].y, pts[1].y]);
                // The y-coordinates of the points intersect the top and bottom offsets.
                if top_offset <= line_bottom && line_top <= bottom_offset {
                    add_line(pts, top_offset, &mut expand_gap);
                    add_line(pts, bottom_offset, &mut expand_gap);
                    add_points(pts, top_offset, bottom_offset, &mut expand_gap);
                }
            }
            PathVerb::Quad => {
                let (quad_top, quad_bottom) = min_max(&[pts[0].y, pts[1].y, pts[2].y]);
                // The y-coordinates of the points intersect the top and bottom offsets.
                if top_offset <= quad_bottom && quad_top <= bottom_offset {
                    add_quad(pts, top_offset, &mut expand_gap);
                    add_quad(pts, bottom_offset, &mut expand_gap);
                    add_points(pts, top_offset, bottom_offset, &mut expand_gap);
                }
            }
            PathVerb::Cubic => {
                let (cubic_top, cubic_bottom) = min_max(&[pts[0].y, pts[1].y, pts[2].y, pts[3].y]);
                // The y-coordinates of the points intersect the top and bottom offsets.
                if top_offset <= cubic_bottom && cubic_top <= bottom_offset {
                    add_cubic(pts, top_offset, &mut expand_gap);
                    add_cubic(pts, bottom_offset, &mut expand_gap);
                    add_points(pts, top_offset, bottom_offset, &mut expand_gap);
                }
            }
        }
    }
    (left, right)
}

/// `std::minmax` of a list of values: the smallest and the largest.
fn min_max(values: &[scalar]) -> (scalar, scalar) {
    let mut lo = values[0];
    let mut hi = values[0];
    for &value in &values[1..] {
        lo = lo.min(value);
        hi = hi.max(value);
    }
    (lo, hi)
}

/// The x of each of `pts` that is strictly inside the band (`addPts`).
// Port of: src/core/SkGlyph.cpp#L490-L497 (chrome/m156)
fn add_points(
    pts: &[Point],
    top_offset: scalar,
    bottom_offset: scalar,
    expand_gap: &mut dyn FnMut(scalar),
) {
    for p in pts {
        if top_offset < p.y && p.y < bottom_offset {
            expand_gap(p.x);
        }
    }
}

/// The x where a quad crosses the horizontal line `offset` (`addQuad`).
// Port of: src/core/SkGlyph.cpp#L470-L478 (chrome/m156)
fn add_quad(pts: &[Point], offset: scalar, expand_gap: &mut dyn FnMut(scalar)) {
    let mut storage = [0.0; 2];
    for &intersection in BezierQuad::intersect_with_horizontal_line(pts, offset, &mut storage) {
        expand_gap(intersection);
    }
}

/// The x where a cubic crosses the horizontal line `offset` (`addCubic`).
// Port of: src/core/SkGlyph.cpp#L480-L488 (chrome/m156)
fn add_cubic(pts: &[Point], offset: scalar, expand_gap: &mut dyn FnMut(scalar)) {
    let mut storage = [0.0; 3];
    for &intersection in BezierCubic::intersect_with_horizontal_line(pts, offset, &mut storage) {
        expand_gap(intersection);
    }
}

/// Appends the interval of `path` that the band `bounds` crosses, offset by the glyph's `scale`
/// and `x_pos`, if there is one (`SkGlyph::ensureIntercepts`).
// Port of: src/core/SkGlyph.cpp#L553-L611 (chrome/m156), without the intercept cache
fn ensure_intercepts(
    path: &Path,
    bounds: [scalar; 2],
    scale: scalar,
    x_pos: scalar,
    intervals: &mut Vec<scalar>,
) {
    let path_bounds = path.bounds();
    if path_bounds.bottom < bounds[0] || bounds[1] < path_bounds.top {
        return;
    }
    let (interval_0, interval_1) = calculate_path_gap(bounds[0], bounds[1], path);
    if interval_0 >= interval_1 {
        return;
    }
    // offsetResults
    intervals.push(interval_0 * scale + x_pos);
    intervals.push(interval_1 * scale + x_pos);
}

/// Appends the intercepts of the glyph run `run` with the band `bounds`, with `paint` (its
/// stroke and path effect change the outlines). Glyphs without a path contribute nothing.
// Port of: src/core/SkTextBlob.cpp#L879-L931 (chrome/m156), get_glyph_run_intercepts
pub(crate) fn glyph_run_intercepts(
    run: &GlyphRun,
    paint: &Paint,
    bounds: [scalar; 2],
    intervals: &mut Vec<scalar>,
) {
    let mut scale: scalar = 1.0;
    let mut intercept_paint = paint.clone();
    let mut intercept_font = run.font().clone();
    // don't want this affecting our path-cache lookup
    intercept_paint.set_mask_filter(None);
    // can't use our canonical size if we need to apply path effects
    if intercept_paint.path_effect().is_none() {
        // If the wrong size is going to be used, don't hint anything.
        intercept_font.set_hinting(FontHinting::None);
        intercept_font.set_subpixel(true);
        scale = intercept_font.size() / CANONICAL_TEXT_SIZE_FOR_PATHS;
        intercept_font.set_size(CANONICAL_TEXT_SIZE_FOR_PATHS);
        // Note: `scale` can be zero here, so the divide is IEEE (downstream checks the result).
        if intercept_paint.stroke_width() > 0.0 && intercept_paint.style() != Style::Fill {
            intercept_paint
                .set_stroke_width(ieee_float_divide(intercept_paint.stroke_width(), scale));
        }
    }
    intercept_paint.set_style(Style::Fill);
    intercept_paint.set_path_effect(None);

    let spec = StrikeSpec::make_with_no_device(
        &intercept_font,
        Some(&intercept_paint),
        ScalerContextBuildFlags::FAKE_GAMMA_AND_BOOST_CONTRAST,
    );
    let metrics_and_paths = BulkGlyphMetricsAndPaths::new(&spec);
    let glyphs = metrics_and_paths.glyphs(run.glyph_ids());
    for (glyph, pos) in glyphs.iter().zip(run.positions()) {
        if let Some(path) = glyph.path() {
            // The typeface is scaled, so un-scale the bounds to be in the space of the typeface.
            // Also ensure the bounds are properly offset by the vertical positioning of the glyph.
            let scaled_bounds = [(bounds[0] - pos.y) / scale, (bounds[1] - pos.y) / scale];
            ensure_intercepts(path, scaled_bounds, scale, pos.x, intervals);
        }
    }
}

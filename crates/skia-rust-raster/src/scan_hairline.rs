// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan_Hairline.cpp

//! Non-antialiased hairlines, hairline rects/paths (with square and round caps), and the
//! hairline entry points that also serve the antialiased variants (`SkScan_Hairline.cpp`).
//!
//! Skia names are kept as snake case: `SkScan::HairLineRgn` is [`hair_line_rgn`],
//! `SkScan::HairPath` is [`hair_path`], `SkScan::AntiHairSquarePath` is
//! [`anti_hair_square_path`], and so on. A C++ `const SkRegion*` that may be null is an
//! `Option<&Region>`; `const SkRasterClip&` is a [`ScanClip`].
//!
//! Not ported: the `canDirectBlit` fast path of `horiline`/`vertline` (it writes pixels straight
//! into the destination for opaque solid-color blitters). The `blit_h` loop that follows it in
//! Skia produces the same pixels, and `canDirectBlit` is not part of the Rust `Blitter` trait
//! yet (see `blitter.rs`).

use skia_rust_core::fdot6::{
    FDOT6_ONE, Fdot6, fdot6_round, fdot6_to_fixed, float_to_fdot6, int_to_fdot6,
};
use skia_rust_core::fixed::{Fixed, fixed_div};
use skia_rust_core::geometry::{
    AutoConicToQuads, CubicCoeff, QuadCoeff, chop_cubic_at_max_curvature, from_point,
};
use skia_rust_core::line_clipper::intersect_line;
use skia_rust_core::math_priv::clz;
use skia_rust_core::paint::Cap;
use skia_rust_core::path::Path;
use skia_rust_core::path_iter::PathIter;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::{IPoint, Point, Vector};
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::region::Region;
use skia_rust_core::scalar::{SCALAR_PI, scalar_ceil_to_int, scalar_floor_to_int};
use skia_rust_simd::vx::{self, Float2};

use crate::blitter::{Blitter, BlitterClipper};
use crate::scan::fill_rect_clip;
use crate::scan_antihair::anti_hair_line_rgn;
use crate::scan_clip::ScanClip;

/// `SkScan::HairRgnProc`: draws `count - 1` line segments, one at a time:
/// `line(pts[0], pts[1])`, `line(pts[1], pts[2])`, ...
// Port of: src/core/SkScan.h#L35 (chrome/m156)
pub type HairRgnProc = fn(&[Point], Option<&Region>, &mut dyn Blitter);

// Port of: src/core/SkScan_Hairline.cpp#L55-L67 (chrome/m156)
fn horiline(x: i32, stopx: i32, fy: Fixed, dy: Fixed, blitter: &mut dyn Blitter) {
    debug_assert!(x < stopx);

    let mut x = x;
    let mut fy = fy;
    loop {
        blitter.blit_h(x, fy >> 16, 1);
        fy = fy.wrapping_add(dy);
        x += 1;
        if x >= stopx {
            break;
        }
    }
}

// Port of: src/core/SkScan_Hairline.cpp#L77-L89 (chrome/m156)
fn vertline(y: i32, stopy: i32, fx: Fixed, dx: Fixed, blitter: &mut dyn Blitter) {
    debug_assert!(y < stopy);

    let mut y = y;
    let mut fx = fx;
    loop {
        blitter.blit_h(fx >> 16, y, 1);
        fx = fx.wrapping_add(dx);
        y += 1;
        if y >= stopy {
            break;
        }
    }
}

/// Draws `src.len() - 1` hairline segments, clipped to `clip` (`None` for no clip)
/// (`SkScan::HairLineRgn`).
// Port of: src/core/SkScan_Hairline.cpp#L102-L214 (chrome/m156)
#[doc(alias = "HairLineRgn")]
pub fn hair_line_rgn(src: &[Point], clip: Option<&Region>, orig_blitter: &mut dyn Blitter) {
    if src.is_empty() {
        return;
    }

    let max = 32767.0f32;
    let fixed_bounds = Rect::new(-max, -max, max, max);

    let mut clip_bounds = Rect::default();
    if let Some(clip) = clip {
        clip_bounds = Rect::from(*clip.bounds());
    }

    for i in 0..src.len() - 1 {
        // We have to pre-clip the line to fit in a SkFixed, so we just chop
        // the line. TODO find a way to actually draw beyond that range.
        let Some(mut pts) = intersect_line(&[src[i], src[i + 1]], &fixed_bounds) else {
            continue;
        };

        // Perform a clip in scalar space, so we catch huge values which might
        // be missed after we convert to SkFDot6 (overflow)
        if clip.is_some() {
            let Some(p) = intersect_line(&pts, &clip_bounds) else {
                continue;
            };
            pts = p;
        }

        let mut x0 = float_to_fdot6(pts[0].x);
        let mut y0 = float_to_fdot6(pts[0].y);
        let mut x1 = float_to_fdot6(pts[1].x);
        let mut y1 = float_to_fdot6(pts[1].y);

        let mut use_clipper = false;
        if let Some(clip) = clip {
            // now perform clipping again, as the rounding to dot6 can wiggle us
            // our rects are really dot6 rects, but since we've already used
            // lineclipper, we know they will fit in 32bits (26.6)
            let bounds = clip.bounds();

            let clip_r = IRect::new(
                int_to_fdot6(bounds.left),
                int_to_fdot6(bounds.top),
                int_to_fdot6(bounds.right),
                int_to_fdot6(bounds.bottom),
            );
            let mut pts_r = IRect::new(x0, y0, x1, y1);
            pts_r.sort();

            // outset the right and bottom, to account for how hairlines are
            // actually drawn, which may hit the pixel to the right or below of
            // the coordinate
            pts_r.right += FDOT6_ONE;
            pts_r.bottom += FDOT6_ONE;

            if !IRect::intersects(&pts_r, &clip_r) {
                continue;
            }
            if !clip.is_rect() || !clip_r.contains(pts_r) {
                use_clipper = true;
            }
        }

        let mut clipper = BlitterClipper::new();
        let blitter: &mut dyn Blitter = if use_clipper {
            clipper.apply(&mut *orig_blitter, clip, None)
        } else {
            &mut *orig_blitter
        };

        let dx: Fdot6 = x1.wrapping_sub(x0);
        let dy: Fdot6 = y1.wrapping_sub(y0);

        if dx.wrapping_abs() > dy.wrapping_abs() {
            // mostly horizontal
            if x0 > x1 {
                // we want to go left-to-right
                std::mem::swap(&mut x0, &mut x1);
                std::mem::swap(&mut y0, &mut y1);
            }
            let ix0 = fdot6_round(x0);
            let ix1 = fdot6_round(x1);
            if ix0 == ix1 {
                // too short to draw
                continue;
            }
            let slope = fixed_div(dy, dx);
            let start_y = fdot6_to_fixed(y0).wrapping_add(slope.wrapping_mul((32 - x0) & 63) >> 6);

            horiline(ix0, ix1, start_y, slope, blitter);
        } else {
            // mostly vertical
            if y0 > y1 {
                // we want to go top-to-bottom
                std::mem::swap(&mut x0, &mut x1);
                std::mem::swap(&mut y0, &mut y1);
            }
            let iy0 = fdot6_round(y0);
            let iy1 = fdot6_round(y1);
            if iy0 == iy1 {
                // too short to draw
                continue;
            }
            let slope = fixed_div(dx, dy);
            let start_x = fdot6_to_fixed(x0).wrapping_add(slope.wrapping_mul((32 - y0) & 63) >> 6);

            vertline(iy0, iy1, start_x, slope, blitter);
        }
    }
}

// Draws the rect's hairline outline once `blitter` is clipped.
fn hair_rect_blit(r: &IRect, blitter: &mut dyn Blitter) {
    let width = r.right.wrapping_sub(r.left);
    let height = r.bottom.wrapping_sub(r.top);

    if (width | height) == 0 {
        return;
    }
    if width <= 2 || height <= 2 {
        blitter.blit_rect(r.left, r.top, width, height);
        return;
    }
    // if we get here, we know we have 4 segments to draw
    blitter.blit_h(r.left, r.top, width); // top
    blitter.blit_rect(r.left, r.top + 1, 1, height - 2); // left
    blitter.blit_rect(r.right - 1, r.top + 1, 1, height - 2); // right
    blitter.blit_h(r.left, r.bottom - 1, width); // bottom
}

// we don't just draw 4 lines, 'cause that can leave a gap in the bottom-right
// and double-hit the top-left.
/// Draws the hairline outline of `rect` (`SkScan::HairRect`).
// Port of: src/core/SkScan_Hairline.cpp#L226-L276 (chrome/m156)
#[doc(alias = "HairRect")]
pub fn hair_rect(rect: &Rect, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    // Create the enclosing bounds of the hairrect. i.e. we will stroke the interior of r.
    let r = IRect::new(
        scalar_floor_to_int(rect.left),
        scalar_floor_to_int(rect.top),
        scalar_floor_to_int(rect.right + 1.0),
        scalar_floor_to_int(rect.bottom + 1.0),
    );

    // Note: r might be crazy big, if rect was huge, possibly getting pinned to max/min s32.
    // We need to trim it back to something reasonable before we can query its width etc.
    // since r.fRight - r.fLeft might wrap around to negative even if fRight > fLeft.
    //
    // We outset the clip bounds by 1 before intersecting, since r is being stroked and not filled
    // so we don't want to pin an edge of it to the clip. The intersect's job is mostly to just
    // get the actual edge values into a reasonable range (e.g. so width() can't overflow).
    let Some(r) = IRect::intersect(&r, &clip.bounds().with_outset(IPoint::new(1, 1))) else {
        return;
    };

    if clip.quick_reject(&r) {
        return;
    }
    if !clip.quick_contains(&r) {
        if clip.is_bw() {
            let mut clipper = BlitterClipper::new();
            let blitter = clipper.apply(blitter, Some(clip.bw_rgn()), None);
            hair_rect_blit(&r, blitter);
        } else {
            clip.with_aa_wrapper(blitter, &mut |rgn, b| {
                let mut clipper = BlitterClipper::new();
                let b = clipper.apply(b, Some(rgn), None);
                hair_rect_blit(&r, b);
            });
        }
        return;
    }

    hair_rect_blit(&r, blitter);
}

///////////////////////////////////////////////////////////////////////////////

const MAX_CUBIC_SUBDIVIDE_LEVEL: i32 = 9;
const MAX_QUAD_SUBDIVIDE_LEVEL: i32 = 5;

// Port of: src/core/SkScan_Hairline.cpp#L285-L304 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // mirrors the assignments to uint32_t
#[allow(clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2.f`
fn compute_int_quad_dist(pts: &[Point]) -> u32 {
    // compute the vector between the control point ([1]) and the middle of the
    // line connecting the start and end ([0] and [2])
    let mut dx = ((pts[0].x + pts[2].x) / 2.0f32) - pts[1].x;
    let mut dy = ((pts[0].y + pts[2].y) / 2.0f32) - pts[1].y;
    // we want everyone to be positive
    dx = dx.abs();
    dy = dy.abs();
    // convert to whole pixel values (use ceiling to be conservative).
    // assign to unsigned so we can safely add 1/2 of the smaller and still fit in
    // uint32_t, since SkScalarCeilToInt() returns 31 bits at most.
    let idx = scalar_ceil_to_int(dx) as u32;
    let idy = scalar_ceil_to_int(dy) as u32;
    // use the cheap approx for distance
    if idx > idy {
        idx.wrapping_add(idy >> 1)
    } else {
        idy.wrapping_add(idx >> 1)
    }
}

// Draw a quadratic by subdividing it into a series of line segments.
// Assuming none of those points are infinite/nan, then draw the line.
// Port of: src/core/SkScan_Hairline.cpp#L315-L347 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors `1.0f / lines` (lines <= 32)
#[allow(clippy::many_single_char_names)] // A, B, C and t, as in the C++
fn hair_quad_lines(
    pts: &[Point],
    clip: Option<&Region>,
    blitter: &mut dyn Blitter,
    level: i32,
    lineproc: HairRgnProc,
) {
    debug_assert!(level <= MAX_QUAD_SUBDIVIDE_LEVEL);

    // Convert the quadratic points into coefficients for the form: p(t) = At^2 + Bt + C
    let coeff = QuadCoeff::from_points(pts);

    let lines: u32 = 1 << level;
    let mut t = Float2::splat(0.0);
    let dt = Float2::splat(1.0f32 / lines as f32);

    let mut tmp = [Point::default(); (1 << MAX_QUAD_SUBDIVIDE_LEVEL) + 1];

    tmp[0] = pts[0];
    let a = coeff.a;
    let b = coeff.b;
    let c = coeff.c;
    let mut is_finite = true; // start out as true
    for tmp_i in tmp.iter_mut().take(lines as usize).skip(1) {
        t += dt;
        let p = (a * t + b) * t + c;
        is_finite &= p[0].is_finite() && p[1].is_finite();
        *tmp_i = Point::new(p[0], p[1]);
    }
    if is_finite {
        tmp[lines as usize] = pts[2];
        lineproc(&tmp[..=lines as usize], clip, blitter);
    }
}

// Port of: src/core/SkScan_Hairline.cpp#L349-L360 (chrome/m156)
fn compute_nocheck_bounds(pts: &[Point]) -> Rect {
    let mut min = from_point(pts[0]);
    let mut max = min;
    for p in &pts[1..] {
        let pair = from_point(*p);
        min = min.min(pair);
        max = max.max(pair);
    }
    Rect::new(min[0], min[1], max[0], max[1])
}

// Port of: src/core/SkScan_Hairline.cpp#L362-L364 (chrome/m156)
fn is_inverted(r: &Rect) -> bool {
    r.left > r.right || r.top > r.bottom
}

// Can't call SkRect::intersects, since it cares about empty, and we don't (since we tracking
// something to be stroked, so empty can still draw something (e.g. horizontal line)
// Port of: src/core/SkScan_Hairline.cpp#L368-L372 (chrome/m156)
fn geometric_overlap(a: &Rect, b: &Rect) -> bool {
    debug_assert!(!is_inverted(a) && !is_inverted(b));
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

// Can't call SkRect::contains, since it cares about empty, and we don't (since we tracking
// something to be stroked, so empty can still draw something (e.g. horizontal line)
// Port of: src/core/SkScan_Hairline.cpp#L376-L380 (chrome/m156)
fn geometric_contains(outer: &Rect, inner: &Rect) -> bool {
    debug_assert!(!is_inverted(outer) && !is_inverted(inner));
    inner.right <= outer.right
        && inner.left >= outer.left
        && inner.bottom <= outer.bottom
        && inner.top >= outer.top
}

/// `DrawingParameters`: what the per-segment hairline code needs to draw and cull.
// Port of: src/core/SkScan_Hairline.cpp#L216-L222 (chrome/m156)
struct DrawingParameters<'a, 'b> {
    clip: Option<&'a Region>,
    inset_clip: Option<Rect>,
    outset_clip: Option<Rect>,
    blitter: &'b mut dyn Blitter,
    lineproc: HairRgnProc,
}

// Port of: src/core/SkScan_Hairline.cpp#L382-L394 (chrome/m156)
fn hairquad(pts: &[Point], d: &mut DrawingParameters<'_, '_>, level: i32) {
    let mut clip = d.clip;
    if let Some(inset_clip) = &d.inset_clip {
        let outset_clip = d.outset_clip.as_ref().expect("outset clip with inset clip");
        let bounds = compute_nocheck_bounds(&pts[..3]);
        if !geometric_overlap(outset_clip, &bounds) {
            return;
        } else if geometric_contains(inset_clip, &bounds) {
            clip = None;
        }
    }

    hair_quad_lines(pts, clip, &mut *d.blitter, level, d.lineproc);
}

// Port of: src/core/SkScan_Hairline.cpp#L396-L400 (chrome/m156)
fn max_component(value: Float2) -> f32 {
    let components = [value[0], value[1]];
    // std::max(a, b) is `(a < b) ? b : a`
    if components[0] < components[1] {
        components[1]
    } else {
        components[0]
    }
}

// Port of: src/core/SkScan_Hairline.cpp#L402-L424 (chrome/m156)
fn compute_cubic_segs(pts: &[Point]) -> usize {
    let p0 = from_point(pts[0]);
    let p1 = from_point(pts[1]);
    let p2 = from_point(pts[2]);
    let p3 = from_point(pts[3]);

    let one_third = Float2::splat(1.0f32 / 3.0f32);
    let two_third = Float2::splat(2.0f32 / 3.0f32);

    let p13 = one_third * p3 + two_third * p0;
    let p23 = one_third * p0 + two_third * p3;

    let diff = max_component(vx::abs(p1 - p13).max(vx::abs(p2 - p23)));
    let mut tol = 1.0f32 / 8.0f32;

    for i in 0..MAX_CUBIC_SUBDIVIDE_LEVEL {
        if diff < tol {
            return 1 << i;
        }
        tol *= 4.0;
    }
    1 << MAX_CUBIC_SUBDIVIDE_LEVEL
}

// Port of: src/core/SkScan_Hairline.cpp#L426-L428 (chrome/m156)
fn lt_90(p0: Point, pivot: Point, p2: Point) -> bool {
    Vector::dot_product(p0 - pivot, p2 - pivot) >= 0.0
}

// The off-curve points are "inside" the limits of the on-curve pts
// Port of: src/core/SkScan_Hairline.cpp#L431-L436 (chrome/m156)
fn quick_cubic_niceness_check(pts: &[Point]) -> bool {
    lt_90(pts[1], pts[0], pts[3])
        && lt_90(pts[2], pts[0], pts[3])
        && lt_90(pts[1], pts[3], pts[0])
        && lt_90(pts[2], pts[3], pts[0])
}

// Draw a cubic by subdividing it into a series of line segments.
// Assuming none of those points are infinite/nan, then draw the line.
// Port of: src/core/SkScan_Hairline.cpp#L440-L476 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors `1.0f / lines` (lines <= 512)
#[allow(clippy::many_single_char_names)] // A, B, C, D and t, as in the C++
fn hair_cubic(
    pts: &[Point],
    clip: Option<&Region>,
    blitter: &mut dyn Blitter,
    lineproc: HairRgnProc,
) {
    let lines = compute_cubic_segs(pts);
    debug_assert!(lines > 0);
    if 1 == lines {
        lineproc(&[pts[0], pts[3]], clip, blitter);
        return;
    }

    // Convert the cubic points into coefficients for the form: p(t) = At^3 + Bt^2 + Ct + D
    let coeff = CubicCoeff::new(pts);

    let mut t = Float2::splat(0.0);
    let dt = Float2::splat(1.0f32 / lines as f32);

    let mut tmp = [Point::default(); (1 << MAX_CUBIC_SUBDIVIDE_LEVEL) + 1];

    tmp[0] = pts[0];
    let a = coeff.a;
    let b = coeff.b;
    let c = coeff.c;
    let d = coeff.d;
    let mut is_finite = true; // start out as true
    for tmp_i in tmp.iter_mut().take(lines).skip(1) {
        t += dt;
        let p = ((a * t + b) * t + c) * t + d;
        is_finite &= p[0].is_finite() && p[1].is_finite();
        *tmp_i = Point::new(p[0], p[1]);
    }
    if is_finite {
        tmp[lines] = pts[3];
        lineproc(&tmp[..=lines], clip, blitter);
    } // else some point(s) are non-finite, so don't draw
}

// Port of: src/core/SkScan_Hairline.cpp#L491-L514 (chrome/m156)
fn haircubic(pts: &[Point], d: &mut DrawingParameters<'_, '_>, _level: i32) {
    let mut clip = d.clip;
    if let Some(inset_clip) = &d.inset_clip {
        let outset_clip = d.outset_clip.as_ref().expect("outset clip with inset clip");
        let bounds = compute_nocheck_bounds(&pts[..4]);
        if !geometric_overlap(outset_clip, &bounds) {
            return;
        } else if geometric_contains(inset_clip, &bounds) {
            clip = None;
        }
    }

    if quick_cubic_niceness_check(pts) {
        hair_cubic(pts, clip, &mut *d.blitter, d.lineproc);
    } else {
        let mut tmp = [Point::default(); 13];
        let mut t_values = [0.0f32; 3];

        let count = chop_cubic_at_max_curvature(pts, Some(&mut tmp), Some(&mut t_values));
        for i in 0..count {
            hair_cubic(&tmp[i * 3..i * 3 + 4], clip, &mut *d.blitter, d.lineproc);
        }
    }
}

// Port of: src/core/SkScan_Hairline.cpp#L516-L529 (chrome/m156)
fn compute_quad_level(pts: &[Point]) -> i32 {
    let d = compute_int_quad_dist(pts);
    /*  quadratics approach the line connecting their start and end points
    4x closer with each subdivision, so we compute the number of
    subdivisions to be the minimum need to get that distance to be less
    than a pixel.
    */
    let mut level = (33 - clz(d)) >> 1;
    // safety check on level (from the previous version)
    if level > MAX_QUAD_SUBDIVIDE_LEVEL {
        level = MAX_QUAD_SUBDIVIDE_LEVEL;
    }
    level
}

/* Extend the points in the direction of the starting or ending tangent by 1/2 unit to
account for a round or square cap. If there's no distance between the end point and
the control point, use the next control point to create a tangent. If the curve
is degenerate, move the cap out 1/2 unit horizontally. */
// Port of: src/core/SkScan_Hairline.cpp#L539-L588 (chrome/m156)
fn extend_pts(
    cap_style: Cap,
    prev_verb: Option<PathVerb>,
    next_verb: Option<PathVerb>,
    pts: &mut [Point],
) {
    debug_assert!(cap_style == Cap::Square || cap_style == Cap::Round);
    // The area of a circle is PI*R*R. For a unit circle, R=1/2, and the cap covers half of that.
    let cap_outset: f32 = if cap_style == Cap::Square {
        0.5
    } else {
        SCALAR_PI / 8.0
    };
    let size = pts.len();
    if prev_verb == Some(PathVerb::Move) {
        let mut first = 0usize;
        let mut ctrl = first;
        let mut controls = size - 1;
        let mut tangent: Vector;
        loop {
            ctrl += 1;
            tangent = pts[first] - pts[ctrl];
            if !tangent.is_zero() {
                break;
            }
            controls -= 1;
            if controls == 0 {
                break;
            }
        }
        if tangent.is_zero() {
            tangent.set(1.0, 0.0);
            controls = size - 1; // If all points are equal, move all but one
        } else {
            let _ = tangent.normalize();
        }
        loop {
            // If the end point and control points are equal, loop to move them in tandem.
            pts[first].x += tangent.x * cap_outset;
            pts[first].y += tangent.y * cap_outset;
            first += 1;
            controls += 1;
            if controls >= size {
                break;
            }
        }
    }
    if next_verb.is_none_or(|v| v == PathVerb::Move || v == PathVerb::Close) {
        let mut last = size - 1;
        let mut ctrl = last;
        let mut controls = size - 1;
        let mut tangent: Vector;
        loop {
            ctrl -= 1;
            tangent = pts[last] - pts[ctrl];
            if !tangent.is_zero() {
                break;
            }
            controls -= 1;
            if controls == 0 {
                break;
            }
        }
        if tangent.is_zero() {
            tangent.set(-1.0, 0.0);
            controls = size - 1;
        } else {
            let _ = tangent.normalize();
        }
        loop {
            pts[last].x += tangent.x * cap_outset;
            pts[last].y += tangent.y * cap_outset;
            // wraps past the start of the array on the final iteration, where it is not used
            last = last.wrapping_sub(1);
            controls += 1;
            if controls >= size {
                break;
            }
        }
    }
}

// Port of: src/core/SkScan_Hairline.cpp#L590-L600 (chrome/m156)
fn hairconic(p: &[Point], d: &mut DrawingParameters<'_, '_>, conic_weight: f32) {
    let mut converter = AutoConicToQuads::new();
    // how close should the quads be to the original conic?
    let tol = 1.0f32 / 4.0f32;
    let quad_storage = converter
        .compute_quads_with_weight(p, conic_weight, tol)
        .to_vec();
    for i in 0..converter.count_quads() {
        let quad_pts = &quad_storage[2 * i..2 * i + 3];
        let level = compute_quad_level(quad_pts);
        hairquad(quad_pts, d, level);
    }
}

// This function assumes that iter is currently ON a SkPathVerb::kMove verb
// Port of: src/core/SkScan_Hairline.cpp#L603-L619 (chrome/m156)
fn is_next_contour_closed(scanner: &PathIter<'_>) -> bool {
    let mut scanner = scanner.clone();
    // we assume the first verb is already a move, so do scanner.next() to proceed to the next verb.
    // This will ideally be a contour verb or a close
    let Some(rec) = scanner.next() else {
        // (C++ dereferences the empty optional here; a Move is never the last verb)
        return false;
    };
    if rec.verb() == PathVerb::Close {
        return true;
    }
    if rec.verb() == PathVerb::Move {
        return false;
    }

    while let Some(next_verb) = scanner.peek_next_verb() {
        // The current contour ends here, before this kMove.
        if next_verb == PathVerb::Move {
            return false;
        }
        // This kClose ends the current contour.
        if next_verb == PathVerb::Close {
            return true;
        }
        scanner.next(); // Consume the record
    }
    false
}

#[allow(clippy::too_many_lines)] // one function in C++
// What is left of `hair_path` once the clip has been resolved: `clip` is the region to clip
// segments to (`None`: no clipping needed), `clip_is_rect` is `rclip.isRect()`.
fn hair_path_draw(
    cap_style: Cap,
    raw: &PathRaw<'_>,
    clip: Option<&Region>,
    clip_is_rect: bool,
    blitter: &mut dyn Blitter,
    lineproc: HairRgnProc,
) {
    let mut inset_clip: Option<Rect> = None;
    let mut outset_clip: Option<Rect> = None;

    if let Some(clip) = clip {
        /*
         *  We now cache two scalar rects, to use for culling per-segment (e.g. cubic).
         *  Since we're hairlining, the "bounds" of the control points isn't necessairly the
         *  limit of where a segment can draw (it might draw up to 1 pixel beyond in aa-hairs).
         *
         *  Compute the pt-bounds per segment is easy, so we do that, and then inversely adjust
         *  the culling bounds so we can just do a straight compare per segment.
         *
         *  insetClip is use for quick-accept (i.e. the segment is not clipped), so we inset
         *  it from the clip-bounds (since segment bounds can be off by 1).
         *
         *  outsetClip is used for quick-reject (i.e. the segment is entirely outside), so we
         *  outset it from the clip-bounds.
         */
        let mut inset_storage = Rect::from(*clip.bounds());
        let outset_storage = inset_storage.with_outset(Point::new(1.0, 1.0));
        inset_storage.inset(Point::new(1.0, 1.0));
        if is_inverted(&inset_storage) {
            /*
             *  our bounds checks assume the rects are never inverted. If insetting has
             *  created that, we assume that the area is too small to safely perform a
             *  quick-accept, so we just mark the rect as empty (so the quick-accept check
             *  will always fail.
             */
            inset_storage.set_empty(); // just so we don't pass an inverted rect
        }
        if clip_is_rect {
            inset_clip = Some(inset_storage);
        }
        outset_clip = Some(outset_storage);
    }

    let mut pts = [Point::default(); 4];
    let mut first_pt = Point::default();
    let mut last_pt = Point::default();
    let mut is_closed = false;
    let is_butt_cap = cap_style == Cap::Butt;
    let mut params = DrawingParameters {
        clip,
        inset_clip,
        outset_clip,
        blitter,
        lineproc,
    };

    let mut prev_verb: Option<PathVerb> = None;
    let mut iter = raw.iter();
    while let Some(rec) = iter.next() {
        let src_pts = rec.points();
        let verb = rec.verb();
        let next_verb = iter.peek_next_verb();
        match verb {
            PathVerb::Move => {
                first_pt = src_pts[0];
                last_pt = src_pts[0];
                is_closed = !is_butt_cap && is_next_contour_closed(&iter);
            }
            PathVerb::Line => {
                const NUM_LINE_PTS: usize = 2;
                pts[..NUM_LINE_PTS].copy_from_slice(&src_pts[..NUM_LINE_PTS]);
                if !is_butt_cap && (!is_closed || Path::is_line_degenerate(pts[0], pts[1], true)) {
                    extend_pts(cap_style, prev_verb, next_verb, &mut pts[..NUM_LINE_PTS]);
                }
                (params.lineproc)(&pts[..NUM_LINE_PTS], clip, &mut *params.blitter);
                last_pt = pts[NUM_LINE_PTS - 1];
            }
            PathVerb::Quad => {
                const NUM_QUAD_PTS: usize = 3;
                pts[..NUM_QUAD_PTS].copy_from_slice(&src_pts[..NUM_QUAD_PTS]);
                if !is_butt_cap
                    && (!is_closed || Path::is_quad_degenerate(pts[0], pts[1], pts[2], true))
                {
                    extend_pts(cap_style, prev_verb, next_verb, &mut pts[..NUM_QUAD_PTS]);
                }
                let level = compute_quad_level(&pts);
                hairquad(&pts, &mut params, level);
                last_pt = pts[NUM_QUAD_PTS - 1];
            }
            PathVerb::Conic => {
                const NUM_CONIC_PTS: usize = 3;
                pts[..NUM_CONIC_PTS].copy_from_slice(&src_pts[..NUM_CONIC_PTS]);
                if !is_butt_cap
                    && (!is_closed || Path::is_quad_degenerate(pts[0], pts[1], pts[2], true))
                {
                    extend_pts(cap_style, prev_verb, next_verb, &mut pts[..NUM_CONIC_PTS]);
                }
                hairconic(&pts, &mut params, rec.conic_weight());
                last_pt = pts[NUM_CONIC_PTS - 1];
            }
            PathVerb::Cubic => {
                const NUM_CUBIC_PTS: usize = 4;
                pts[..NUM_CUBIC_PTS].copy_from_slice(&src_pts[..NUM_CUBIC_PTS]);
                if !is_butt_cap
                    && (!is_closed
                        || Path::is_cubic_degenerate(pts[0], pts[1], pts[2], pts[3], true))
                {
                    extend_pts(cap_style, prev_verb, next_verb, &mut pts[..NUM_CUBIC_PTS]);
                }
                haircubic(&pts, &mut params, MAX_CUBIC_SUBDIVIDE_LEVEL);
                last_pt = pts[NUM_CUBIC_PTS - 1];
            }
            PathVerb::Close => {
                pts[0] = last_pt;
                pts[1] = first_pt;
                if !is_butt_cap && prev_verb == Some(PathVerb::Move) {
                    // cap moveTo/close to match svg expectations for degenerate segments
                    extend_pts(cap_style, prev_verb, next_verb, &mut pts[..2]);
                }
                (params.lineproc)(&pts[..2], clip, &mut *params.blitter);
            }
        }
        if !is_butt_cap {
            if prev_verb == Some(PathVerb::Move)
                && matches!(
                    verb,
                    PathVerb::Line | PathVerb::Quad | PathVerb::Conic | PathVerb::Cubic
                )
            {
                first_pt = pts[0]; // the curve moved the initial point, so close to it instead
            }
            prev_verb = Some(verb);
        }
    }
}

// Port of: src/core/SkScan_Hairline.cpp#L621-L760 (chrome/m156)
fn hair_path_with_cap(
    cap_style: Cap,
    raw: &PathRaw<'_>,
    rclip: &dyn ScanClip,
    blitter: &mut dyn Blitter,
    lineproc: HairRgnProc,
) {
    if raw.is_empty() {
        return;
    }

    let cap_out = if cap_style == Cap::Butt { 1 } else { 2 };
    let ibounds_unoutset: IRect = raw.bounds.round_out();
    let ibounds = ibounds_unoutset.with_outset(IPoint::new(cap_out, cap_out));
    if rclip.quick_reject(&ibounds) {
        return;
    }
    if rclip.quick_contains(&ibounds) {
        hair_path_draw(cap_style, raw, None, rclip.is_rect(), blitter, lineproc);
    } else if rclip.is_bw() {
        hair_path_draw(
            cap_style,
            raw,
            Some(rclip.bw_rgn()),
            rclip.is_rect(),
            blitter,
            lineproc,
        );
    } else {
        let is_rect = rclip.is_rect();
        rclip.with_aa_wrapper(blitter, &mut |rgn, b| {
            hair_path_draw(cap_style, raw, Some(rgn), is_rect, b, lineproc);
        });
    }
}

/// Draws the path as non-antialiased hairlines with butt caps (`SkScan::HairPath`).
// Port of: src/core/SkScan_Hairline.cpp#L762-L764 (chrome/m156)
#[doc(alias = "HairPath")]
pub fn hair_path(raw: &PathRaw<'_>, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    hair_path_with_cap(Cap::Butt, raw, clip, blitter, hair_line_rgn);
}

/// Draws the path as antialiased hairlines with butt caps (`SkScan::AntiHairPath`).
// Port of: src/core/SkScan_Hairline.cpp#L766-L768 (chrome/m156)
#[doc(alias = "AntiHairPath")]
pub fn anti_hair_path(raw: &PathRaw<'_>, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    hair_path_with_cap(Cap::Butt, raw, clip, blitter, anti_hair_line_rgn);
}

/// Draws the path as non-antialiased hairlines with square caps (`SkScan::HairSquarePath`).
// Port of: src/core/SkScan_Hairline.cpp#L770-L772 (chrome/m156)
#[doc(alias = "HairSquarePath")]
pub fn hair_square_path(raw: &PathRaw<'_>, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    hair_path_with_cap(Cap::Square, raw, clip, blitter, hair_line_rgn);
}

/// Draws the path as antialiased hairlines with square caps (`SkScan::AntiHairSquarePath`).
// Port of: src/core/SkScan_Hairline.cpp#L774-L776 (chrome/m156)
#[doc(alias = "AntiHairSquarePath")]
pub fn anti_hair_square_path(raw: &PathRaw<'_>, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    hair_path_with_cap(Cap::Square, raw, clip, blitter, anti_hair_line_rgn);
}

/// Draws the path as non-antialiased hairlines with round caps (`SkScan::HairRoundPath`).
// Port of: src/core/SkScan_Hairline.cpp#L778-L780 (chrome/m156)
#[doc(alias = "HairRoundPath")]
pub fn hair_round_path(raw: &PathRaw<'_>, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    hair_path_with_cap(Cap::Round, raw, clip, blitter, hair_line_rgn);
}

/// Draws the path as antialiased hairlines with round caps (`SkScan::AntiHairRoundPath`).
// Port of: src/core/SkScan_Hairline.cpp#L782-L784 (chrome/m156)
#[doc(alias = "AntiHairRoundPath")]
pub fn anti_hair_round_path(raw: &PathRaw<'_>, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    hair_path_with_cap(Cap::Round, raw, clip, blitter, anti_hair_line_rgn);
}

///////////////////////////////////////////////////////////////////////////////

/// Strokes the frame of `r` with a non-antialiased stroke of `stroke_size` (`SkScan::FrameRect`).
// Port of: src/core/SkScan_Hairline.cpp#L788-L831 (chrome/m156)
#[doc(alias = "FrameRect")]
pub fn frame_rect(r: &Rect, stroke_size: &Point, clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    debug_assert!(stroke_size.x >= 0.0 && stroke_size.y >= 0.0);

    if stroke_size.x < 0.0 || stroke_size.y < 0.0 {
        return;
    }

    let dx = stroke_size.x;
    let dy = stroke_size.y;
    let rx = dx / 2.0f32;
    let ry = dy / 2.0f32;

    let mut outer = Rect::new(r.left - rx, r.top - ry, r.right + rx, r.bottom + ry);

    if r.width() <= dx || r.height() <= dy {
        // If we're empty on either axis, we remove the outset amount, to be sure
        // we stroke the same way a polygon would (i.e. it would just see a "line"
        // and not extend it for the miter join).
        if r.width() == 0.0 {
            outer.top = r.top;
            outer.bottom = r.bottom;
        }
        if r.height() == 0.0 {
            outer.left = r.left;
            outer.right = r.right;
        }
        fill_rect_clip(&outer, clip, blitter);
        return;
    }

    let mut tmp = Rect::new(outer.left, outer.top, outer.right, outer.top + dy);
    fill_rect_clip(&tmp, clip, blitter);
    tmp.top = outer.bottom - dy;
    tmp.bottom = outer.bottom;
    fill_rect_clip(&tmp, clip, blitter);

    tmp = Rect::new(
        outer.left,
        outer.top + dy,
        outer.left + dx,
        outer.bottom - dy,
    );
    fill_rect_clip(&tmp, clip, blitter);
    tmp.left = outer.right - dx;
    tmp.right = outer.right;
    fill_rect_clip(&tmp, clip, blitter);
}

/// Draws hairline segments through `pts` (`SkScan::HairLine`).
// Port of: src/core/SkScan_Hairline.cpp#L833-L849 (chrome/m156)
#[doc(alias = "HairLine")]
pub fn hair_line(pts: &[Point], clip: &dyn ScanClip, blitter: &mut dyn Blitter) {
    if clip.is_bw() {
        hair_line_rgn(pts, Some(clip.bw_rgn()), blitter);
    } else {
        let r = Rect::bounds_or_empty(pts).with_outset(Point::new(0.5, 0.5));

        let rounded: IRect = r.round_out();
        if clip.quick_contains(&rounded) {
            hair_line_rgn(pts, None, blitter);
        } else {
            clip.with_aa_wrapper(blitter, &mut |rgn, b| {
                hair_line_rgn(pts, Some(rgn), b);
            });
        }
    }
}

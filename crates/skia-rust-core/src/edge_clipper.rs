// Copyright 2009 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkEdgeClipper.h, src/core/SkEdgeClipper.cpp

//! Clipping path segments against a rectangle, for scan conversion (`SkEdgeClipper.h`).

use crate::geometry::AutoConicToQuads;
use crate::geometry::{
    chop_cubic_at, chop_cubic_at_x_extrema, chop_cubic_at_y_extrema, chop_mono_cubic_at_x,
    chop_mono_cubic_at_y, chop_quad_at, chop_quad_at_x_extrema, chop_quad_at_y_extrema,
    find_unit_quad_roots,
};
use crate::line_clipper;
use crate::path_priv::{Edge, PathEdgeIter};
use crate::path_raw::PathRaw;
use crate::path_types::PathVerb;
use crate::point::Point;
use crate::rect::Rect;
use crate::scalar::{SCALAR_MAX, scalar, scalar_abs};

const MAX_VERBS: usize = 18; // max curvature in X and Y split cubic into 9 pieces, * (line + cubic)
const MAX_POINTS: usize = 54; // 2 lines + 1 cubic require 6 points; times 9 pieces

/// Basically an iterator: initialized with an edge and a clip, then `next()` is called until it
/// returns `None`.
// Port of: src/core/SkEdgeClipper.h#L24-L67 (chrome/m156)
#[doc(alias = "SkEdgeClipper")]
#[derive(Clone, Debug)]
pub struct EdgeClipper {
    curr_point: usize,
    curr_verb: usize,
    curr_verb_stop: usize,
    can_cull_to_the_right: bool,
    points: [Point; MAX_POINTS],
    verbs: [PathVerb; MAX_VERBS],
}

// Port of: src/core/SkEdgeClipper.cpp#L22-L24 (chrome/m156)
fn quick_reject(bounds: &Rect, clip: &Rect) -> bool {
    bounds.top >= clip.bottom || bounds.bottom <= clip.top
}

// Port of: src/core/SkEdgeClipper.cpp#L26-L30 (chrome/m156)
fn clamp_le(value: &mut scalar, max: scalar) {
    if *value > max {
        *value = max;
    }
}

// Port of: src/core/SkEdgeClipper.cpp#L32-L36 (chrome/m156)
fn clamp_ge(value: &mut scalar, min: scalar) {
    if *value < min {
        *value = min;
    }
}

/*  src[] must be monotonic in Y. This routine copies src into dst, and sorts
it to be increasing in Y. If it had to reverse the order of the points,
it returns true, otherwise it returns false
*/
// Port of: src/core/SkEdgeClipper.cpp#L42-L53 (chrome/m156)
fn sort_increasing_y(dst: &mut [Point], src: &[Point], count: usize) -> bool {
    // we need the data to be monotonically increasing in Y
    if src[0].y > src[count - 1].y {
        for i in 0..count {
            dst[i] = src[count - i - 1];
        }
        true
    } else {
        dst[..count].copy_from_slice(&src[..count]);
        false
    }
}

// Port of: src/core/SkEdgeClipper.cpp#L74-L91 (chrome/m156)
fn chop_mono_quad_at(c0: scalar, c1: scalar, c2: scalar, target: scalar) -> Option<scalar> {
    /* Solve F(t) = y where F(t) := [0](1-t)^2 + 2[1]t(1-t) + [2]t^2
     *  We solve for t, using quadratic equation, hence we have to rearrange
     * our cooefficents to look like At^2 + Bt + C
     */
    let a = c0 - c1 - c1 + c2;
    let b = 2.0 * (c1 - c0);
    let c = c0 - target;

    let mut roots = [0.0; 2]; // we only expect one, but make room for 2 for safety
    let count = find_unit_quad_roots(a, b, c, &mut roots);
    if count != 0 {
        return Some(roots[0]);
    }
    None
}

// Port of: src/core/SkEdgeClipper.cpp#L93-L95 (chrome/m156)
fn chop_mono_quad_at_y(pts: &[Point; 3], y: scalar) -> Option<scalar> {
    chop_mono_quad_at(pts[0].y, pts[1].y, pts[2].y, y)
}

// Port of: src/core/SkEdgeClipper.cpp#L97-L99 (chrome/m156)
fn chop_mono_quad_at_x(pts: &[Point; 3], x: scalar) -> Option<scalar> {
    chop_mono_quad_at(pts[0].x, pts[1].x, pts[2].x, x)
}

// Modify pts[] in place so that it is clipped in Y to the clip rect
// Port of: src/core/SkEdgeClipper.cpp#L102-L148 (chrome/m156)
fn chop_quad_in_y(pts: &mut [Point; 3], clip: &Rect) {
    let mut tmp = [Point::default(); 5]; // for SkChopQuadAt

    // are we partially above
    if pts[0].y < clip.top {
        if let Some(t) = chop_mono_quad_at_y(pts, clip.top) {
            // take the 2nd chopped quad
            chop_quad_at(pts, &mut tmp, t);
            // clamp to clean up imprecise numerics in the chop
            tmp[2].y = clip.top;
            clamp_ge(&mut tmp[3].y, clip.top);

            pts[0] = tmp[2];
            pts[1] = tmp[3];
        } else {
            // if chopMonoQuadAtY failed, then we may have hit inexact numerics
            // so we just clamp against the top
            for p in pts.iter_mut() {
                if p.y < clip.top {
                    p.y = clip.top;
                }
            }
        }
    }

    // are we partially below
    if pts[2].y > clip.bottom {
        if let Some(t) = chop_mono_quad_at_y(pts, clip.bottom) {
            chop_quad_at(pts, &mut tmp, t);
            // clamp to clean up imprecise numerics in the chop
            clamp_le(&mut tmp[1].y, clip.bottom);
            tmp[2].y = clip.bottom;

            pts[1] = tmp[1];
            pts[2] = tmp[2];
        } else {
            // if chopMonoQuadAtY failed, then we may have hit inexact numerics
            // so we just clamp against the bottom
            for p in pts.iter_mut() {
                if p.y > clip.bottom {
                    p.y = clip.bottom;
                }
            }
        }
    }
}

// Port of: src/core/SkEdgeClipper.cpp#L256-L279 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in C++
#[allow(clippy::many_single_char_names)] // names follow the C++
fn mono_cubic_closest_t(src: [scalar; 4], x: scalar) -> scalar {
    let mut t = 0.5;
    let mut last_t;
    let mut best_t = 0.0;
    let mut step = 0.25;
    let d = src[0];
    let a = src[3] + 3.0 * (src[1] - src[2]) - d;
    let b = 3.0 * (src[2] - src[1] - src[1] + d);
    let c = 3.0 * (src[1] - d);
    let x = x - d;
    let mut closest = SCALAR_MAX;
    loop {
        let loc = ((a * t + b) * t + c) * t;
        let dist = scalar_abs(loc - x);
        if closest > dist {
            closest = dist;
            best_t = t;
        }
        last_t = t;
        t += if loc < x { step } else { -step };
        step *= 0.5;
        if !(closest > 0.25 && last_t != t) {
            break;
        }
    }
    best_t
}

// Port of: src/core/SkEdgeClipper.cpp#L281-L286 (chrome/m156)
fn chop_mono_cubic_at_y_or_closest(src: &[Point; 4], y: scalar, dst: &mut [Point; 7]) {
    if chop_mono_cubic_at_y(src, y, dst) {
        return;
    }
    chop_cubic_at(
        src,
        dst,
        mono_cubic_closest_t([src[0].y, src[1].y, src[2].y, src[3].y], y),
    );
}

// Modify pts[] in place so that it is clipped in Y to the clip rect
// Port of: src/core/SkEdgeClipper.cpp#L289-L333 (chrome/m156)
fn chop_cubic_in_y(pts: &mut [Point; 4], clip: &Rect) {
    // are we partially above
    if pts[0].y < clip.top {
        let mut tmp = [Point::default(); 7];
        chop_mono_cubic_at_y_or_closest(pts, clip.top, &mut tmp);

        /*
         *  For a large range in the points, we can do a poor job of chopping, such that the t
         *  we computed resulted in the lower cubic still being partly above the clip.
         *
         *  If just the first or first 2 Y values are above the fTop, we can just smash them
         *  down. If the first 3 Ys are above fTop, we can't smash all 3, as that can really
         *  distort the cubic. In this case, we take the first output (tmp[3..6] and treat it as
         *  a guess, and re-chop against fTop. Then we fall through to checking if we need to
         *  smash the first 1 or 2 Y values.
         */
        if tmp[3].y < clip.top && tmp[4].y < clip.top && tmp[5].y < clip.top {
            let tmp2 = [tmp[3], tmp[4], tmp[5], tmp[6]];
            chop_mono_cubic_at_y_or_closest(&tmp2, clip.top, &mut tmp);
        }

        // tmp[3, 4].fY should all be to the below clip.fTop.
        // Since we can't trust the numerics of the chopper, we force those conditions now
        tmp[3].y = clip.top;
        clamp_ge(&mut tmp[4].y, clip.top);

        pts[0] = tmp[3];
        pts[1] = tmp[4];
        pts[2] = tmp[5];
    }

    // are we partially below
    if pts[3].y > clip.bottom {
        let mut tmp = [Point::default(); 7];
        chop_mono_cubic_at_y_or_closest(pts, clip.bottom, &mut tmp);
        tmp[3].y = clip.bottom;
        clamp_le(&mut tmp[2].y, clip.bottom);

        pts[1] = tmp[1];
        pts[2] = tmp[2];
        pts[3] = tmp[3];
    }
}

// Port of: src/core/SkEdgeClipper.cpp#L335-L340 (chrome/m156)
fn chop_mono_cubic_at_x_or_closest(src: &[Point; 4], x: scalar, dst: &mut [Point; 7]) {
    if chop_mono_cubic_at_x(src, x, dst) {
        return;
    }
    chop_cubic_at(
        src,
        dst,
        mono_cubic_closest_t([src[0].x, src[1].x, src[2].x, src[3].x], x),
    );
}

// Port of: src/core/SkEdgeClipper.cpp#L406-L408 (chrome/m156)
fn compute_cubic_bounds(pts: &[Point]) -> Rect {
    Rect::bounds_or_empty(&pts[..4])
}

// Port of: src/core/SkEdgeClipper.cpp#L410-L419 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors the C++ int to float conversion
fn too_big_for_reliable_float_math(r: &Rect) -> bool {
    // limit set as the largest float value for which we can still reliably compute things like
    // - chopping at XY extrema
    // - chopping at Y or X values for clipping
    //
    // Current value chosen just by experiment. Larger (and still succeeds) is always better.
    //
    const LIMIT: scalar = (1 << 22) as scalar;
    r.left < -LIMIT || r.top < -LIMIT || r.right > LIMIT || r.bottom > LIMIT
}

impl EdgeClipper {
    /// A clipper; if `can_cull_to_the_right`, segments right of the clip are dropped.
    #[must_use]
    pub fn new(can_cull_to_the_right: bool) -> Self {
        Self {
            curr_point: 0,
            curr_verb: 0,
            curr_verb_stop: 0,
            can_cull_to_the_right,
            points: [Point::default(); MAX_POINTS],
            verbs: [PathVerb::Move; MAX_VERBS],
        }
    }

    /// `canCullToTheRight`.
    #[must_use]
    pub fn can_cull_to_the_right(&self) -> bool {
        self.can_cull_to_the_right
    }

    fn finish(&mut self) -> bool {
        self.curr_verb_stop = self.curr_verb;
        self.curr_point = 0;
        self.curr_verb = 0;
        self.curr_verb_stop != self.curr_verb
    }

    /// Clips the line `p0..p1`; returns true if anything remains.
    // Port of: src/core/SkEdgeClipper.cpp#L55-L70 (chrome/m156)
    #[doc(alias = "clipLine")]
    pub fn clip_line(&mut self, p0: Point, p1: Point, clip: &Rect) -> bool {
        self.curr_point = 0;
        self.curr_verb = 0;

        let mut lines = [Point::default(); line_clipper::MAX_POINTS];
        let pts = [p0, p1];
        let line_count =
            line_clipper::clip_line(&pts, clip, &mut lines, self.can_cull_to_the_right);
        for i in 0..line_count {
            self.append_line(lines[i], lines[i + 1]);
        }

        self.finish()
    }

    // srcPts[] must be monotonic in X and Y
    // Port of: src/core/SkEdgeClipper.cpp#L151-L226 (chrome/m156)
    fn clip_mono_quad(&mut self, src_pts: &[Point], clip: &Rect) {
        let mut pts = [Point::default(); 3];
        let mut reverse = sort_increasing_y(&mut pts, src_pts, 3);

        // are we completely above or below
        if pts[2].y <= clip.top || pts[0].y >= clip.bottom {
            return;
        }

        // Now chop so that pts is contained within clip in Y
        chop_quad_in_y(&mut pts, clip);

        if pts[0].x > pts[2].x {
            pts.swap(0, 2);
            reverse = !reverse;
        }
        debug_assert!(pts[0].x <= pts[1].x);
        debug_assert!(pts[1].x <= pts[2].x);

        // Now chop in X has needed, and record the segments

        if pts[2].x <= clip.left {
            // wholly to the left
            self.append_vline(clip.left, pts[0].y, pts[2].y, reverse);
            return;
        }
        if pts[0].x >= clip.right {
            // wholly to the right
            if !self.can_cull_to_the_right() {
                self.append_vline(clip.right, pts[0].y, pts[2].y, reverse);
            }
            return;
        }

        let mut tmp = [Point::default(); 5]; // for SkChopQuadAt

        // are we partially to the left
        if pts[0].x < clip.left {
            if let Some(t) = chop_mono_quad_at_x(&pts, clip.left) {
                chop_quad_at(&pts, &mut tmp, t);
                self.append_vline(clip.left, tmp[0].y, tmp[2].y, reverse);
                // clamp to clean up imprecise numerics in the chop
                tmp[2].x = clip.left;
                clamp_ge(&mut tmp[3].x, clip.left);

                pts[0] = tmp[2];
                pts[1] = tmp[3];
            } else {
                // if chopMonoQuadAtY failed, then we may have hit inexact numerics
                // so we just clamp against the left
                self.append_vline(clip.left, pts[0].y, pts[2].y, reverse);
                return;
            }
        }

        // are we partially to the right
        if pts[2].x > clip.right {
            if let Some(t) = chop_mono_quad_at_x(&pts, clip.right) {
                chop_quad_at(&pts, &mut tmp, t);
                // clamp to clean up imprecise numerics in the chop
                clamp_le(&mut tmp[1].x, clip.right);
                tmp[2].x = clip.right;

                self.append_quad(&tmp[..3], reverse);
                self.append_vline(clip.right, tmp[2].y, tmp[4].y, reverse);
            } else {
                // if chopMonoQuadAtY failed, then we may have hit inexact numerics
                // so we just clamp against the right
                pts[1].x = std_min(pts[1].x, clip.right);
                pts[2].x = std_min(pts[2].x, clip.right);
                self.append_quad(&pts, reverse);
            }
        } else {
            // wholly inside the clip
            self.append_quad(&pts, reverse);
        }
    }

    /// Clips the quad `src_pts`; returns true if anything remains.
    // Port of: src/core/SkEdgeClipper.cpp#L228-L252 (chrome/m156)
    #[doc(alias = "clipQuad")]
    pub fn clip_quad(&mut self, src_pts: &[Point], clip: &Rect) -> bool {
        self.curr_point = 0;
        self.curr_verb = 0;

        let bounds = Rect::bounds_or_empty(&src_pts[..3]);

        if !quick_reject(&bounds, clip) {
            let mut mono_y = [Point::default(); 5];
            let count_y = chop_quad_at_y_extrema(src_pts, &mut mono_y);
            for y in 0..=count_y {
                let mut mono_x = [Point::default(); 5];
                let count_x = chop_quad_at_x_extrema(&mono_y[y * 2..], &mut mono_x);
                for x in 0..=count_x {
                    self.clip_mono_quad(&mono_x[x * 2..x * 2 + 3], clip);
                    assert!(self.curr_verb < MAX_VERBS);
                    assert!(self.curr_point <= MAX_POINTS);
                }
            }
        }

        self.finish()
    }

    // srcPts[] must be monotonic in X and Y
    // Port of: src/core/SkEdgeClipper.cpp#L343-L404 (chrome/m156)
    fn clip_mono_cubic(&mut self, src: &[Point], clip: &Rect) {
        let mut pts = [Point::default(); 4];
        let mut reverse = sort_increasing_y(&mut pts, src, 4);

        // are we completely above or below
        if pts[3].y <= clip.top || pts[0].y >= clip.bottom {
            return;
        }

        // Now chop so that pts is contained within clip in Y
        chop_cubic_in_y(&mut pts, clip);

        if pts[0].x > pts[3].x {
            pts.swap(0, 3);
            pts.swap(1, 2);
            reverse = !reverse;
        }

        // Now chop in X has needed, and record the segments

        if pts[3].x <= clip.left {
            // wholly to the left
            self.append_vline(clip.left, pts[0].y, pts[3].y, reverse);
            return;
        }
        if pts[0].x >= clip.right {
            // wholly to the right
            if !self.can_cull_to_the_right() {
                self.append_vline(clip.right, pts[0].y, pts[3].y, reverse);
            }
            return;
        }

        // are we partially to the left
        if pts[0].x < clip.left {
            let mut tmp = [Point::default(); 7];
            chop_mono_cubic_at_x_or_closest(&pts, clip.left, &mut tmp);
            self.append_vline(clip.left, tmp[0].y, tmp[3].y, reverse);

            // tmp[3, 4].fX should all be to the right of clip.fLeft.
            // Since we can't trust the numerics of
            // the chopper, we force those conditions now
            tmp[3].x = clip.left;
            clamp_ge(&mut tmp[4].x, clip.left);

            pts[0] = tmp[3];
            pts[1] = tmp[4];
            pts[2] = tmp[5];
        }

        // are we partially to the right
        if pts[3].x > clip.right {
            let mut tmp = [Point::default(); 7];
            chop_mono_cubic_at_x_or_closest(&pts, clip.right, &mut tmp);
            tmp[3].x = clip.right;
            clamp_le(&mut tmp[2].x, clip.right);

            self.append_cubic(&tmp[..4], reverse);
            self.append_vline(clip.right, tmp[3].y, tmp[6].y, reverse);
        } else {
            // wholly inside the clip
            self.append_cubic(&pts, reverse);
        }
    }

    /// Clips the cubic `src_pts`; returns true if anything remains.
    // Port of: src/core/SkEdgeClipper.cpp#L421-L455 (chrome/m156)
    #[doc(alias = "clipCubic")]
    pub fn clip_cubic(&mut self, src_pts: &[Point], clip: &Rect) -> bool {
        self.curr_point = 0;
        self.curr_verb = 0;

        let bounds = compute_cubic_bounds(src_pts);
        // check if we're clipped out vertically
        if bounds.bottom > clip.top && bounds.top < clip.bottom {
            if too_big_for_reliable_float_math(&bounds) {
                // can't safely clip the cubic, so we give up and draw a line (which we can
                // safely clip)
                //
                // If we rewrote chopcubicat*extrema and chopmonocubic using doubles, we could very
                // likely always handle the cubic safely, but (it seems) at a big loss in speed, so
                // we'd only want to take that alternate impl if needed. Perhaps a TODO to try it.
                //
                return self.clip_line(src_pts[0], src_pts[3], clip);
            }
            let mut mono_y = [Point::default(); 10];
            let count_y = chop_cubic_at_y_extrema(src_pts, Some(&mut mono_y));
            for y in 0..=count_y {
                let mut mono_x = [Point::default(); 10];
                let count_x = chop_cubic_at_x_extrema(&mono_y[y * 3..], Some(&mut mono_x));
                for x in 0..=count_x {
                    self.clip_mono_cubic(&mono_x[x * 3..x * 3 + 4], clip);
                    debug_assert!(self.curr_verb < MAX_VERBS);
                    debug_assert!(self.curr_point <= MAX_POINTS);
                }
            }
        }

        self.finish()
    }

    // Port of: src/core/SkEdgeClipper.cpp#L459-L465 (chrome/m156)
    fn append_line(&mut self, p0: Point, p1: Point) {
        self.verbs[self.curr_verb] = PathVerb::Line;
        self.curr_verb += 1;
        assert!(self.curr_point + 2 <= MAX_POINTS);
        self.points[self.curr_point] = p0;
        self.points[self.curr_point + 1] = p1;
        self.curr_point += 2;
    }

    // Port of: src/core/SkEdgeClipper.cpp#L467-L478 (chrome/m156)
    fn append_vline(&mut self, x: scalar, y0: scalar, y1: scalar, reverse: bool) {
        self.verbs[self.curr_verb] = PathVerb::Line;
        self.curr_verb += 1;

        let (y0, y1) = if reverse { (y1, y0) } else { (y0, y1) };
        assert!(self.curr_point + 2 <= MAX_POINTS);
        self.points[self.curr_point].set(x, y0);
        self.points[self.curr_point + 1].set(x, y1);
        self.curr_point += 2;
    }

    // Port of: src/core/SkEdgeClipper.cpp#L480-L493 (chrome/m156)
    fn append_quad(&mut self, pts: &[Point], reverse: bool) {
        self.verbs[self.curr_verb] = PathVerb::Quad;
        self.curr_verb += 1;

        assert!(self.curr_point + 3 <= MAX_POINTS);
        let c = self.curr_point;
        if reverse {
            self.points[c] = pts[2];
            self.points[c + 2] = pts[0];
        } else {
            self.points[c] = pts[0];
            self.points[c + 2] = pts[2];
        }
        self.points[c + 1] = pts[1];
        self.curr_point += 3;
    }

    // Port of: src/core/SkEdgeClipper.cpp#L495-L507 (chrome/m156)
    fn append_cubic(&mut self, pts: &[Point], reverse: bool) {
        self.verbs[self.curr_verb] = PathVerb::Cubic;
        self.curr_verb += 1;

        assert!(self.curr_point + 4 <= MAX_POINTS);
        let c = self.curr_point;
        if reverse {
            for i in 0..4 {
                self.points[c + i] = pts[3 - i];
            }
        } else {
            self.points[c..c + 4].copy_from_slice(&pts[..4]);
        }
        self.curr_point += 4;
    }

    /// The next clipped segment (its points written to `pts`), or `None` when done.
    // Port of: src/core/SkEdgeClipper.cpp#L509-L537 (chrome/m156)
    pub fn next(&mut self, pts: &mut [Point]) -> Option<PathVerb> {
        debug_assert!(self.curr_verb <= self.curr_verb_stop);
        if self.curr_verb >= self.curr_verb_stop {
            return None;
        }

        let verb = self.verbs[self.curr_verb];
        self.curr_verb += 1;
        let n = match verb {
            PathVerb::Line => 2,
            PathVerb::Quad => 3,
            PathVerb::Cubic => 4,
            _ => {
                debug_assert!(false, "unexpected verb in quadclippper2 iter");
                0
            }
        };
        assert!(self.curr_point + n <= MAX_POINTS);
        pts[..n].copy_from_slice(&self.points[self.curr_point..self.curr_point + n]);
        self.curr_point += n;
        Some(verb)
    }

    /// Clips each segment of `raw` and passes the result (in a clipper) to `consume`, with a flag
    /// saying whether the segment starts a new contour.
    // Port of: src/core/SkEdgeClipper.cpp#L571-L611 (chrome/m156)
    #[doc(alias = "ClipPath")]
    #[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
    pub fn clip_path(
        raw: &PathRaw<'_>,
        clip: &Rect,
        can_cull_to_the_right: bool,
        mut consume: impl FnMut(&mut EdgeClipper, bool),
    ) {
        let mut quadder = AutoConicToQuads::new();
        const CONIC_TOL: f32 = 0.25;

        let mut iter = PathEdgeIter::new(raw);
        let mut clipper = EdgeClipper::new(can_cull_to_the_right);

        while let Some(e) = iter.next() {
            match e.edge {
                Edge::Line => {
                    if clipper.clip_line(e.pts[0], e.pts[1], clip) {
                        consume(&mut clipper, e.is_new_contour);
                    }
                }
                Edge::Quad => {
                    if clipper.clip_quad(&e.pts, clip) {
                        consume(&mut clipper, e.is_new_contour);
                    }
                }
                Edge::Conic => {
                    let quad_pts = quadder
                        .compute_quads_with_weight(&e.pts, iter.conic_weight(), CONIC_TOL)
                        .to_vec();
                    for i in 0..quadder.count_quads() {
                        if clipper.clip_quad(&quad_pts[i * 2..], clip) {
                            consume(&mut clipper, e.is_new_contour);
                        }
                    }
                }
                Edge::Cubic => {
                    if clipper.clip_cubic(&e.pts, clip) {
                        consume(&mut clipper, e.is_new_contour);
                    }
                }
            }
        }
    }
}

/// `std::min(a, b)`: `(b < a) ? b : a`.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

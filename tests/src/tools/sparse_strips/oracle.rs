// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/Oracle.h, .cpp (chrome/m156)

//! [`ScanlineOracle8x`]: the source of ground truth for testing the entire sparse strips pipeline
//! end to end. Builds the subsample windings for a single pixel row by raycasting at each of the 8
//! subsample locations in the sparse strips MSAA pattern.
//!
//! The casts between `f64`, `f32` and the integer types mirror the C++ conversions of the ported
//! arithmetic.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names
)]

use skia_rust_core::cubics;
use skia_rust_core::path::{Iter, Path, Verb};
use skia_rust_core::point::Point;
use skia_rust_core::quads;
use skia_rust_simd::vx::{Byte8, Float8, Int8, any, if_then_else};

use skia_rust_gpu::graphite::sparse_strips::types::K_MSAA_PATTERN_U8;

/// `ScanlineOracle8x::CurveIntersection`.
// Port of: tests/graphite/sparse_strips/Oracle.h#L26-L29 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct CurveIntersection {
    /// `x`.
    pub x: Float8,
    /// `dir`: +1 for downward, -1 for upward, 0 for no hit in this lane.
    pub dir: Int8,
}

impl Default for CurveIntersection {
    fn default() -> Self {
        Self {
            x: Float8::splat(0.0),
            dir: Int8::splat(0),
        }
    }
}

/// `ScanlineOracle8x::RowWindingInterval`.
// Port of: tests/graphite/sparse_strips/Oracle.h#L31-L34 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct RowWindingInterval {
    /// `x`.
    pub x: i32,
    /// `winding`.
    pub winding: Int8,
}

/// `kSubX`: the x position of each subsample within a pixel.
fn k_sub_x() -> Float8 {
    (Byte8::from_list(&K_MSAA_PATTERN_U8).cast::<f32>() + 0.5f32) / 8.0f32
}

/// `ScanlineOracle8x`.
// Port of: tests/graphite/sparse_strips/Oracle.h#L22-L108 (chrome/m156)
#[derive(Clone, Debug, Default)]
pub struct ScanlineOracle8x {
    path: Path,
    current_row: i32,
    built: bool,
    hits: Vec<CurveIntersection>,
    intervals: Vec<RowWindingInterval>,
}

impl ScanlineOracle8x {
    /// `ScanlineOracle8x()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_row: -1,
            ..Self::default()
        }
    }

    /// `ScanlineOracle8x(path)`.
    #[must_use]
    pub fn with_path(path: &Path) -> Self {
        Self {
            path: path.clone(),
            ..Self::new()
        }
    }

    /// `GetOracleWinding(px, intervals)`.
    // Port of: tests/graphite/sparse_strips/Oracle.h#L35-L48 (chrome/m156)
    #[doc(alias = "GetOracleWinding")]
    #[must_use]
    pub fn get_oracle_winding(px: i32, intervals: &[RowWindingInterval]) -> Int8 {
        let (Some(front), Some(back)) = (intervals.first(), intervals.last()) else {
            return Int8::splat(0);
        };
        if px < front.x || px >= back.x {
            return Int8::splat(0);
        }
        // `std::upper_bound`: the first interval with `px < interval.x`.
        let it = intervals.partition_point(|interval| interval.x <= px);
        if it == 0 {
            return Int8::splat(0);
        }
        intervals[it - 1].winding
    }

    /// `setPath(path)`.
    // Port of: tests/graphite/sparse_strips/Oracle.h#L53-L61 (chrome/m156)
    #[doc(alias = "setPath")]
    pub fn set_path(&mut self, path: &Path) {
        if self.path != *path {
            self.path = path.clone();
            self.built = false;
            self.current_row = -1;
            self.hits.clear();
            self.intervals.clear();
        }
    }

    /// `path()`.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `isBuilt()`.
    #[must_use]
    pub fn is_built(&self) -> bool {
        self.built
    }

    /// `currentRow()`.
    #[must_use]
    pub fn current_row(&self) -> i32 {
        self.current_row
    }

    /// `hits()`.
    #[must_use]
    pub fn hits(&self) -> &[CurveIntersection] {
        debug_assert!(self.built);
        &self.hits
    }

    /// `rowIntervals()`.
    #[doc(alias = "rowIntervals")]
    #[must_use]
    pub fn row_intervals(&self) -> &[RowWindingInterval] {
        debug_assert!(self.built);
        &self.intervals
    }

    /// `buildRowIntervals(py)`.
    #[doc(alias = "buildRowIntervals")]
    pub fn build_row_intervals(&mut self, py: i32) -> Vec<RowWindingInterval> {
        self.build_row(py);
        self.intervals.clone()
    }

    /// `buildRowIntervals(py, path)`.
    #[doc(alias = "buildRowIntervals")]
    pub fn build_row_intervals_for_path(
        &mut self,
        py: i32,
        path: &Path,
    ) -> Vec<RowWindingInterval> {
        self.build_row_for_path(py, path);
        self.intervals.clone()
    }

    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L26-L42 (chrome/m156)
    fn intersect_line(&mut self, pts: &[Point], target_y: Float8) {
        let dy = pts[1].y - pts[0].y;
        if dy != 0.0 {
            let inv_dy = 1.0f32 / dy;
            let dx = pts[1].x - pts[0].x;
            let dir: i32 = if dy > 0.0 { 1 } else { -1 };

            let t = (target_y - pts[0].y) * inv_dy;
            let valid = t.ge_mask(0.0f32) & t.lt_mask(1.0f32);
            if any(valid) {
                let hit = CurveIntersection {
                    x: pts[0].x + t * dx,
                    dir: if_then_else(valid, Int8::splat(dir), Int8::splat(0)),
                };
                self.hits.push(hit);
            }
        }
    }

    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L44-L85 (chrome/m156)
    fn intersect_quad(&mut self, pts: &[Point], target_y: Float8) {
        let ay = f64::from(pts[2].y) - 2.0 * f64::from(pts[1].y) + f64::from(pts[0].y);
        let by = 2.0 * (f64::from(pts[1].y) - f64::from(pts[0].y));
        let cy = f64::from(pts[0].y);

        let ax = f64::from(pts[2].x) - 2.0 * f64::from(pts[1].x) + f64::from(pts[0].x);
        let bx = 2.0 * (f64::from(pts[1].x) - f64::from(pts[0].x));
        let cx = f64::from(pts[0].x);

        let mut hits = [CurveIntersection::default(); 2];

        for k in 0..8 {
            let mut roots = [0.0f64; 2];
            let num_roots = quads::roots_real(ay, by, cy - f64::from(target_y[k]), &mut roots);
            let mut valid_count = 0;
            for &t in &roots[..num_roots] {
                if (0.0..1.0).contains(&t) {
                    let derivative_y = (2.0 * ay * t + by) as f32;
                    if derivative_y.abs() > 1e-5f32 {
                        let x = (ax * t * t + bx * t + cx) as f32;
                        let dir: i32 = if derivative_y > 0.0 { 1 } else { -1 };
                        if valid_count < 2 {
                            hits[valid_count].x[k] = x;
                            hits[valid_count].dir[k] = dir;
                            valid_count += 1;
                        }
                    }
                }
            }
        }

        for hit in hits {
            if any(hit.dir.ne_mask(0)) {
                self.hits.push(hit);
            }
        }
    }

    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L87-L137 (chrome/m156)
    fn intersect_conic(&mut self, pts: &[Point], weight: f32, target_y: Float8) {
        let mut hits = [CurveIntersection::default(); 2];
        let weight = f64::from(weight);

        for k in 0..8 {
            let d0 = f64::from(pts[0].y) - f64::from(target_y[k]);
            let d1 = f64::from(pts[1].y) - f64::from(target_y[k]);
            let d2 = f64::from(pts[2].y) - f64::from(target_y[k]);

            let ay = d2 - 2.0 * weight * d1 + d0;
            let by = 2.0 * (weight * d1 - d0);
            let cy = d0;

            let mut roots = [0.0f64; 2];
            let num_roots = quads::roots_real(ay, by, cy, &mut roots);
            let mut valid_count = 0;
            for &t in &roots[..num_roots] {
                if (0.0..1.0).contains(&t) {
                    let denom = (1.0 - t) * (1.0 - t) + 2.0 * weight * t * (1.0 - t) + t * t;
                    if denom != 0.0 {
                        let num_x = (1.0 - t) * (1.0 - t) * f64::from(pts[0].x)
                            + 2.0 * weight * t * (1.0 - t) * f64::from(pts[1].x)
                            + t * t * f64::from(pts[2].x);
                        let x = (num_x / denom) as f32;

                        // denom is always positive, so we only need the numerator to find the sign
                        let derivative_y = (2.0 * ay * t + by) as f32;
                        if derivative_y.abs() > 1e-5f32 {
                            let dir: i32 = if derivative_y > 0.0 { 1 } else { -1 };
                            if valid_count < 2 {
                                hits[valid_count].x[k] = x;
                                hits[valid_count].dir[k] = dir;
                                valid_count += 1;
                            }
                        }
                    }
                }
            }
        }

        for hit in hits {
            if any(hit.dir.ne_mask(0)) {
                self.hits.push(hit);
            }
        }
    }

    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L139-L182 (chrome/m156)
    fn intersect_cubic(&mut self, pts: &[Point], target_y: Float8) {
        let p = |i: usize| (f64::from(pts[i].x), f64::from(pts[i].y));
        let ay = p(3).1 - 3.0 * p(2).1 + 3.0 * p(1).1 - p(0).1;
        let by = 3.0 * (p(2).1 - 2.0 * p(1).1 + p(0).1);
        let cy = 3.0 * (p(1).1 - p(0).1);
        let dy = p(0).1;

        let ax = p(3).0 - 3.0 * p(2).0 + 3.0 * p(1).0 - p(0).0;
        let bx = 3.0 * (p(2).0 - 2.0 * p(1).0 + p(0).0);
        let cx = 3.0 * (p(1).0 - p(0).0);
        let dx = p(0).0;

        let mut hits = [CurveIntersection::default(); 3];

        for k in 0..8 {
            let mut roots = [0.0f64; 3];
            let num_roots = cubics::roots_real(ay, by, cy, dy - f64::from(target_y[k]), &mut roots);
            let mut valid_count = 0;
            for &t in &roots[..num_roots] {
                if (0.0..1.0).contains(&t) {
                    let x = cubics::eval_at(ax, bx, cx, dx, t) as f32;
                    let derivative_y = (3.0 * ay * t * t + 2.0 * by * t + cy) as f32;
                    if derivative_y.abs() > 1e-5f32 {
                        let dir: i32 = if derivative_y > 0.0 { 1 } else { -1 };
                        if valid_count < 3 {
                            hits[valid_count].x[k] = x;
                            hits[valid_count].dir[k] = dir;
                            valid_count += 1;
                        }
                    }
                }
            }
        }

        for hit in hits {
            if any(hit.dir.ne_mask(0)) {
                self.hits.push(hit);
            }
        }
    }

    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L184-L208 (chrome/m156)
    fn collect_path_intersections(&mut self, path: &Path, target_y: Float8) {
        self.hits.clear();
        let mut iter = Iter::new(path, /* forceClose = */ true);
        let mut pts = [Point::default(); 4];

        loop {
            let verb = iter.next_verb(&mut pts);
            match verb {
                Verb::Done => break,
                Verb::Line => self.intersect_line(&pts, target_y),
                Verb::Quad => self.intersect_quad(&pts, target_y),
                Verb::Conic => {
                    let weight = iter.conic_weight().unwrap_or(0.0);
                    self.intersect_conic(&pts, weight, target_y);
                }
                Verb::Cubic => self.intersect_cubic(&pts, target_y),
                _ => {}
            }
        }
    }

    /// `evaluatePixelWinding(px)`.
    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L210-L221 (chrome/m156)
    #[doc(alias = "evaluatePixelWinding")]
    #[must_use]
    pub fn evaluate_pixel_winding(&self, px: f32) -> Int8 {
        debug_assert!(self.built);
        let v_px = px + k_sub_x();
        let mut pixel_winding = Int8::splat(0);

        for hit in &self.hits {
            pixel_winding += if_then_else(v_px.ge_mask(hit.x), hit.dir, Int8::splat(0));
        }
        pixel_winding
    }

    /// `evaluate(px)`.
    #[must_use]
    pub fn evaluate_px(&self, px: f32) -> Int8 {
        self.evaluate_pixel_winding(px)
    }

    /// `buildRow(py, path)`.
    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L227-L267 (chrome/m156)
    #[doc(alias = "buildRow")]
    pub fn build_row_for_path(&mut self, py: i32, path: &Path) {
        self.path = path.clone();
        self.current_row = py;
        self.hits.clear();
        self.intervals.clear();

        let k_sub_y =
            (Float8::from_list(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]) + 0.5f32) / 8.0f32;
        let v_py = (py as f32) + k_sub_y;

        self.collect_path_intersections(path, v_py);
        self.built = true;

        if !self.hits.is_empty() {
            let bounds = path.bounds();
            let min_x = bounds.left.floor() as i32;
            let max_x = bounds.right.ceil() as i32;

            let mut last_winding = Int8::splat(0);
            let mut has_last = false;

            // TODO (thomsmit): Currently, we do not sort the raycast hits and instead perform an
            // O(n) search to sum all subsample points left of the current pixel position. This is
            // because there is no guarantee that subsamples will be crossed in the same pixel.
            // Moving to a sorting based technique would require N subsample vecs, and sorting
            // across each of them.
            for px in min_x..=max_x {
                let w = self.evaluate_pixel_winding(px as f32);
                if !has_last || any(w.ne_mask(last_winding)) {
                    self.intervals
                        .push(RowWindingInterval { x: px, winding: w });
                    last_winding = w;
                    has_last = true;
                }
            }

            // Append terminator at maxX + 1 to define the upper bound of the final span
            // [x_k, maxX + 1)
            self.intervals.push(RowWindingInterval {
                x: max_x + 1,
                winding: Int8::splat(0),
            });
        }
    }

    /// `buildRow(py)`.
    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L223-L260 (chrome/m156)
    #[doc(alias = "buildRow")]
    pub fn build_row(&mut self, py: i32) {
        let path = self.path.clone();
        self.build_row_for_path(py, &path);
    }

    /// `evaluate(pt)`.
    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L266-L268 (chrome/m156)
    pub fn evaluate(&mut self, pt: Point) -> Int8 {
        let path = self.path.clone();
        self.evaluate_with_path(pt, &path)
    }

    /// `evaluate(pt, path)`.
    // Port of: tests/graphite/sparse_strips/Oracle.cpp#L277-L283 (chrome/m156)
    pub fn evaluate_with_path(&mut self, pt: Point, path: &Path) -> Int8 {
        let py = pt.y.floor() as i32;
        if !self.built || self.current_row != py || self.path != *path {
            self.build_row_for_path(py, path);
        }
        self.evaluate_pixel_winding(pt.x)
    }
}

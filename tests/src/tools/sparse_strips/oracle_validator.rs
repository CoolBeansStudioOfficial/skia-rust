// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/OracleValidator.h, .cpp (chrome/m156)

//! [`OracleValidator`]: zero-copy reference validator that evaluates the sparse strips stream
//! against the analytical [`ScanlineOracle8x`] ground truth row by row, pixel by pixel, and
//! subsample by subsample.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    // `std::max(0.0f, std::min(1.0f, t))` of the C++ is not `clamp` for a NaN `t`.
    clippy::manual_clamp
)]

use std::cell::RefCell;
use std::collections::HashMap;

use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_gpu::graphite::geom::end_caps::EndCaps;
use skia_rust_gpu::graphite::geom::wide_tiles::WideTiles;
use skia_rust_gpu::graphite::sparse_strips::flatten::Flatten;
use skia_rust_gpu::graphite::sparse_strips::polyline::Polyline;
use skia_rust_gpu::graphite::sparse_strips::tiler::Tiles;
use skia_rust_gpu::graphite::sparse_strips::types::{IntersectionBits, K_MSAA_PATTERN_U8, Line};
use skia_rust_simd::vx::Int8;

use super::coverage_test_utils::print_winding_diagnostics;
use super::oracle::{RowWindingInterval, ScanlineOracle8x};
use crate::{Reporter, errorf};

/// `OracleValidator::Strictness`.
// Port of: tests/graphite/sparse_strips/OracleValidator.h#L36-L40 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strictness {
    /// `kStrict`: exact sample matching against oracle.
    Strict,
    /// `kNonStrict`: ignore winding errors on subsamples within flattening tolerance of any chord
    /// in tile.
    NonStrict,
}

/// `OracleValidator<kTileWidth, kTileHeight>`.
// Port of: tests/graphite/sparse_strips/OracleValidator.h#L33-L100 (chrome/m156)
#[derive(Debug)]
pub struct OracleValidator<'a, const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16> {
    wides: &'a WideTiles,
    ends: &'a EndCaps,
    exact_windings: &'a [Int8],
    polyline: Option<&'a Polyline>,
    tiler: Option<&'a Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>>,
    test_name: &'a str,
    strictness: Strictness,
    tolerance_sq: f32,
    tile_line_map: HashMap<u32, Vec<Line>>,
    oracle: RefCell<ScanlineOracle8x>,
}

impl<'a, const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>
    OracleValidator<'a, K_TILE_WIDTH, K_TILE_HEIGHT>
{
    // We only explicitly instantiate 4 and 8
    const _TILE_SIZES: () =
        assert!(K_TILE_WIDTH == K_TILE_HEIGHT && (K_TILE_WIDTH == 4 || K_TILE_WIDTH == 8));

    /// `OracleValidator(path, wides, ends, exactWindings, polyline, tiler, testName, strictness)`.
    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L31-L47 (chrome/m156)
    #[must_use]
    pub fn new(
        path: &Path,
        wides: &'a WideTiles,
        ends: &'a EndCaps,
        exact_windings: &'a [Int8],
        polyline: Option<&'a Polyline>,
        tiler: Option<&'a Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>>,
        test_name: &'a str,
        strictness: Strictness,
    ) -> Self {
        let () = Self::_TILE_SIZES;
        let mut validator = Self {
            wides,
            ends,
            exact_windings,
            polyline,
            tiler,
            test_name,
            strictness,
            tolerance_sq: Flatten::K_QUAD_TOLERANCE2 as f32,
            tile_line_map: HashMap::new(),
            oracle: RefCell::new(ScanlineOracle8x::with_path(path)),
        };
        validator.build_tile_line_map();
        validator
    }

    /// `GetOracleWinding(px, intervals)`.
    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L49-L53 (chrome/m156)
    #[doc(alias = "GetOracleWinding")]
    #[must_use]
    pub fn get_oracle_winding(px: i32, intervals: &[RowWindingInterval]) -> Int8 {
        ScanlineOracle8x::get_oracle_winding(px, intervals)
    }

    /// `validate(reporter, viewportWidth, viewportHeight, maxSampleDiffOut)`.
    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L55-L181 (chrome/m156)
    pub fn validate(
        &self,
        reporter: &mut Reporter,
        viewport_width: u16,
        viewport_height: u16,
        mut max_sample_diff_out: Option<&mut u32>,
    ) -> bool {
        if !self.validate_stream_invariants(reporter, viewport_width, viewport_height) {
            return false;
        }

        let mut all_pass = true;
        let caps = self.ends.caps();
        let wides = self.wides.tiles();

        let mut cap_idx = 0usize;
        let mut wide_idx = 0usize;

        // Walk row by row through all tile rows present in caps or wides
        while cap_idx < caps.len() || wide_idx < wides.len() {
            let mut row_y = u16::MAX;
            if cap_idx < caps.len() {
                row_y = row_y.min(caps[cap_idx].y);
            }
            if wide_idx < wides.len() {
                row_y = row_y.min(wides[wide_idx].y);
            }

            let row_cap_start = cap_idx;
            while cap_idx < caps.len() && caps[cap_idx].y == row_y {
                cap_idx += 1;
            }
            let row_cap_end = cap_idx;

            let row_wide_start = wide_idx;
            while wide_idx < wides.len() && wides[wide_idx].y == row_y {
                wide_idx += 1;
            }
            let row_wide_end = wide_idx;

            let is_gap_filled = |start_x: u16, end_x: u16| -> bool {
                for w in &wides[row_wide_start..row_wide_end] {
                    if w.x == start_x && u32::from(w.x) + u32::from(w.width) == u32::from(end_x) {
                        return true;
                    }
                }
                false
            };

            // Process each pixel row within this tile row [rowY, rowY + kTileHeight)
            let mut py = row_y;
            while u32::from(py) < u32::from(row_y) + u32::from(K_TILE_HEIGHT)
                && py < viewport_height
            {
                let oracle_intervals = self.oracle.borrow_mut().build_row_intervals(py.into());

                let mut prev_end_tile_x: i32 = 0;

                for cap in &caps[row_cap_start..row_cap_end] {
                    let alpha_start_tile_x = cap.x;
                    let alpha_end_tile_x = (u32::from(cap.x) + u32::from(cap.width)) as u16;
                    let spanned_tiles = cap.width / K_TILE_WIDTH;

                    // A. Validate gap preceding this cap [prevEndTileX, alphaStartTileX)
                    if i32::from(alpha_start_tile_x) > prev_end_tile_x {
                        let filled = is_gap_filled(prev_end_tile_x as u16, alpha_start_tile_x);
                        all_pass &= self.validate_gap_span(
                            reporter,
                            prev_end_tile_x,
                            i32::from(alpha_start_tile_x),
                            py,
                            &oracle_intervals,
                            filled,
                        );
                    }

                    // B. Validate boundary column left of the first tile in this cap run
                    if alpha_start_tile_x > 0 {
                        let left_col_x = alpha_start_tile_x - 1;
                        let mut left_fill = false;
                        for w in &wides[row_wide_start..row_wide_end] {
                            if u32::from(w.x) + u32::from(w.width) == u32::from(alpha_start_tile_x)
                            {
                                left_fill = true;
                                break;
                            }
                        }
                        all_pass &= self.validate_boundary_column(
                            reporter,
                            left_col_x,
                            py,
                            &oracle_intervals,
                            left_fill,
                        );
                    }

                    // C. Validate inside the alpha tiles span
                    all_pass &= self.validate_alpha_tile_span(
                        reporter,
                        cap.x,
                        cap.y,
                        cap.alpha_index as u32,
                        spanned_tiles,
                        py,
                        &oracle_intervals,
                        max_sample_diff_out.as_deref_mut(),
                    );

                    // D. Validate boundary column right of the last tile in this cap run
                    if alpha_end_tile_x < viewport_width {
                        let right_col_x = alpha_end_tile_x;
                        let mut right_fill = false;
                        for w in &wides[row_wide_start..row_wide_end] {
                            if w.x == alpha_end_tile_x {
                                right_fill = true;
                                break;
                            }
                        }
                        all_pass &= self.validate_boundary_column(
                            reporter,
                            right_col_x,
                            py,
                            &oracle_intervals,
                            right_fill,
                        );
                    }

                    prev_end_tile_x = i32::from(alpha_end_tile_x);
                }

                // E. Validate trailing gap to viewport edge [prevEndTileX, viewportWidth)
                if prev_end_tile_x < i32::from(viewport_width) {
                    let filled = is_gap_filled(prev_end_tile_x as u16, viewport_width);
                    all_pass &= self.validate_gap_span(
                        reporter,
                        prev_end_tile_x,
                        i32::from(viewport_width),
                        py,
                        &oracle_intervals,
                        filled,
                    );
                }

                py += 1;
            }
        }

        all_pass
    }

    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L183-L275 (chrome/m156)
    fn validate_stream_invariants(
        &self,
        reporter: &mut Reporter,
        _viewport_width: u16,
        _viewport_height: u16,
    ) -> bool {
        let caps = self.ends.caps();
        let wides = self.wides.tiles();
        let test_name = self.test_name;

        if caps.is_empty() && wides.is_empty() {
            return true;
        }

        // Validate EndCaps invariants
        let mut last_cap_y: u16 = 0;
        let mut last_cap_end_x: u16 = 0;
        let mut has_last_cap = false;
        for (i, cap) in caps.iter().enumerate() {
            if cap.x % K_TILE_WIDTH != 0 {
                errorf!(
                    reporter,
                    "[{}] Cap[{}] fX ({}) not aligned to kTileWidth ({})",
                    test_name,
                    i,
                    cap.x,
                    K_TILE_WIDTH
                );
                return false;
            }
            if cap.y % K_TILE_HEIGHT != 0 {
                errorf!(
                    reporter,
                    "[{}] Cap[{}] fY ({}) not aligned to kTileHeight ({})",
                    test_name,
                    i,
                    cap.y,
                    K_TILE_HEIGHT
                );
                return false;
            }
            if cap.width % K_TILE_WIDTH != 0 || cap.width == 0 {
                errorf!(
                    reporter,
                    "[{}] Cap[{}] fWidth ({}) not valid multiple of kTileWidth ({})",
                    test_name,
                    i,
                    cap.width,
                    K_TILE_WIDTH
                );
                return false;
            }
            if cap.alpha_index % (i32::from(K_TILE_WIDTH) * i32::from(K_TILE_HEIGHT)) != 0 {
                errorf!(
                    reporter,
                    "[{}] Cap[{}] fAlphaIndex ({}) not multiple of tile size ({})",
                    test_name,
                    i,
                    cap.alpha_index,
                    i32::from(K_TILE_WIDTH) * i32::from(K_TILE_HEIGHT)
                );
                return false;
            }
            if has_last_cap {
                if cap.y < last_cap_y {
                    errorf!(
                        reporter,
                        "[{}] Cap[{}] fY ({}) < previous row fY ({})",
                        test_name,
                        i,
                        cap.y,
                        last_cap_y
                    );
                    return false;
                }
                if cap.y == last_cap_y && cap.x < last_cap_end_x {
                    errorf!(
                        reporter,
                        "[{}] Cap[{}] fX ({}) overlaps with previous cap endX ({})",
                        test_name,
                        i,
                        cap.x,
                        last_cap_end_x
                    );
                    return false;
                }
            }
            last_cap_y = cap.y;
            last_cap_end_x = (u32::from(cap.x) + u32::from(cap.width)) as u16;
            has_last_cap = true;
        }

        // Validate WideTiles invariants
        let mut last_wide_y: u16 = 0;
        let mut last_wide_end_x: u16 = 0;
        let mut has_last_wide = false;
        for (i, wide) in wides.iter().enumerate() {
            if wide.x % K_TILE_WIDTH != 0 {
                errorf!(
                    reporter,
                    "[{}] Wide[{}] fX ({}) not aligned to kTileWidth ({})",
                    test_name,
                    i,
                    wide.x,
                    K_TILE_WIDTH
                );
                return false;
            }
            if wide.y % K_TILE_HEIGHT != 0 {
                errorf!(
                    reporter,
                    "[{}] Wide[{}] fY ({}) not aligned to kTileHeight ({})",
                    test_name,
                    i,
                    wide.y,
                    K_TILE_HEIGHT
                );
                return false;
            }
            if wide.width % K_TILE_WIDTH != 0 || wide.width == 0 {
                errorf!(
                    reporter,
                    "[{}] Wide[{}] fWidth ({}) not valid multiple of kTileWidth ({})",
                    test_name,
                    i,
                    wide.width,
                    K_TILE_WIDTH
                );
                return false;
            }
            if has_last_wide {
                if wide.y < last_wide_y {
                    errorf!(
                        reporter,
                        "[{}] Wide[{}] fY ({}) < previous row fY ({})",
                        test_name,
                        i,
                        wide.y,
                        last_wide_y
                    );
                    return false;
                }
                if wide.y == last_wide_y && wide.x < last_wide_end_x {
                    errorf!(
                        reporter,
                        "[{}] Wide[{}] fX ({}) overlaps with previous wide endX ({})",
                        test_name,
                        i,
                        wide.x,
                        last_wide_end_x
                    );
                    return false;
                }
            }
            last_wide_y = wide.y;
            last_wide_end_x = (u32::from(wide.x) + u32::from(wide.width)) as u16;
            has_last_wide = true;
        }

        true
    }

    /// The number of the oracle's covered subsamples at `(px, py)`, with the covered state of the
    /// subsamples within the flattening tolerance of a chord taken to be `expected_fill` if the
    /// validator is not strict.
    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L276-L291, L330-L345 (chrome/m156)
    fn active_count(&self, w: Int8, px: i32, py: u16, expected_fill: bool) -> i32 {
        let tile_x = (px / i32::from(K_TILE_WIDTH)) as u16;
        let tile_y = py / K_TILE_HEIGHT;
        let mut active_count = 0;
        for k in 0..8usize {
            let mut covered = w[k] != 0;
            if self.strictness == Strictness::NonStrict && covered != expected_fill {
                let sub_x = (f32::from(K_MSAA_PATTERN_U8[k]) + 0.5) / 8.0;
                let sub_y = (k as f32 + 0.5) / 8.0;
                let sub_pt = Point::new(px as f32 + sub_x, f32::from(py) + sub_y);
                if self.is_within_tolerance(sub_pt, tile_x, tile_y) {
                    covered = expected_fill;
                }
            }
            if covered {
                active_count += 1;
            }
        }
        active_count
    }

    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L277-L317 (chrome/m156)
    fn validate_boundary_column(
        &self,
        reporter: &mut Reporter,
        px: u16,
        py: u16,
        oracle_intervals: &[RowWindingInterval],
        expected_fill: bool,
    ) -> bool {
        let w = Self::get_oracle_winding(px.into(), oracle_intervals);
        let active_count = self.active_count(w, i32::from(px), py, expected_fill);

        // Allow 1-sample tolerance on boundary columns for curved paths due to flattening chord
        // error
        let matches = if expected_fill {
            active_count >= 7
        } else {
            active_count <= 1
        };
        if !matches {
            errorf!(
                reporter,
                "[{}] Boundary Column Winding Mismatch at (px={}, py={}): Expected {}, but Oracle \
                 has winding [{},{},{},{},{},{},{},{}] ({} non-zero)",
                self.test_name,
                px,
                py,
                if expected_fill {
                    "Solid Fill"
                } else {
                    "Empty Space"
                },
                w[0],
                w[1],
                w[2],
                w[3],
                w[4],
                w[5],
                w[6],
                w[7],
                active_count
            );
            return false;
        }
        true
    }

    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L319-L363 (chrome/m156)
    fn validate_gap_span(
        &self,
        reporter: &mut Reporter,
        start_px: i32,
        end_px: i32,
        py: u16,
        oracle_intervals: &[RowWindingInterval],
        expected_fill: bool,
    ) -> bool {
        let mut all_pass = true;
        for px in start_px..end_px {
            let w = Self::get_oracle_winding(px, oracle_intervals);
            let active_count = self.active_count(w, px, py, expected_fill);

            // Allow 1-sample tolerance on gap edges for curved paths due to flattening chord error
            let matches = if expected_fill {
                active_count >= 7
            } else {
                active_count <= 1
            };
            if !matches {
                errorf!(
                    reporter,
                    "[{}] Gap Span Winding Mismatch at (px={}, py={}): Expected {}, but Oracle has \
                     winding [{},{},{},{},{},{},{},{}] ({} non-zero)",
                    self.test_name,
                    px,
                    py,
                    if expected_fill {
                        "Solid Fill"
                    } else {
                        "Empty Space"
                    },
                    w[0],
                    w[1],
                    w[2],
                    w[3],
                    w[4],
                    w[5],
                    w[6],
                    w[7],
                    active_count
                );
                all_pass = false;
                break;
            }
        }
        all_pass
    }

    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L365-L463 (chrome/m156)
    fn validate_alpha_tile_span(
        &self,
        reporter: &mut Reporter,
        start_tile_x: u16,
        row_y: u16,
        start_alpha_idx: u32,
        spanned_tiles: u16,
        py: u16,
        oracle_intervals: &[RowWindingInterval],
        mut max_sample_diff_out: Option<&mut u32>,
    ) -> bool {
        let local_y = py - row_y;
        let tile_y = row_y / K_TILE_HEIGHT;
        let mut all_pass = true;

        for t in 0..spanned_tiles {
            let tile_start_x = start_tile_x + t * K_TILE_WIDTH;
            let tile_x = tile_start_x / K_TILE_WIDTH;
            let tile_start_alpha_idx = start_alpha_idx
                + u32::from(t) * (u32::from(K_TILE_WIDTH) * u32::from(K_TILE_HEIGHT));

            for local_x in 0..K_TILE_WIDTH {
                let px = tile_start_x + local_x;
                let alpha_idx = tile_start_alpha_idx
                    + u32::from(local_y) * u32::from(K_TILE_WIDTH)
                    + u32::from(local_x);

                if alpha_idx as usize >= self.exact_windings.len() {
                    errorf!(
                        reporter,
                        "[{}] Alpha index {} out of bounds (exactWindings size {}) at tile({},{}) \
                         pixel({},{})",
                        self.test_name,
                        alpha_idx,
                        self.exact_windings.len(),
                        tile_x,
                        tile_y,
                        local_x,
                        local_y
                    );
                    return false;
                }

                let actual_winding = self.exact_windings[alpha_idx as usize];
                let oracle_winding = Self::get_oracle_winding(px.into(), oracle_intervals);

                let mut sample_diff: i32 = 0;
                for k in 0..8usize {
                    if actual_winding[k] != oracle_winding[k] {
                        let mut is_error = true;
                        if self.strictness == Strictness::NonStrict {
                            let sub_x = (f32::from(K_MSAA_PATTERN_U8[k]) + 0.5) / 8.0;
                            let sub_y = (k as f32 + 0.5) / 8.0;
                            let sub_pt = Point::new(f32::from(px) + sub_x, f32::from(py) + sub_y);
                            if self.is_within_tolerance(sub_pt, tile_x, tile_y) {
                                is_error = false;
                            }
                        }
                        if is_error {
                            sample_diff += 1;
                        }
                    }
                }

                if let Some(max_sample_diff) = max_sample_diff_out.as_deref_mut() {
                    *max_sample_diff = (*max_sample_diff).max(sample_diff as u32);
                }

                if sample_diff > 4 {
                    print_winding_diagnostics::<K_TILE_WIDTH, K_TILE_HEIGHT>(
                        reporter,
                        self.test_name,
                        tile_start_x,
                        row_y,
                        px,
                        py,
                        actual_winding,
                        oracle_winding,
                        self.polyline,
                        self.tiler,
                        self.exact_windings,
                        tile_start_alpha_idx,
                        Some(&mut self.oracle.borrow_mut()),
                    );
                    errorf!(
                        reporter,
                        "[{}] Direct Winding Mismatch at tile({},{}) pixel({},{}) (px={}, py={}): \
                         Diff {} samples. Actual Winding [{},{},{},{},{},{},{},{}], Oracle Winding \
                         [{},{},{},{},{},{},{},{}]",
                        self.test_name,
                        tile_x,
                        tile_y,
                        local_x,
                        local_y,
                        px,
                        py,
                        sample_diff,
                        actual_winding[0],
                        actual_winding[1],
                        actual_winding[2],
                        actual_winding[3],
                        actual_winding[4],
                        actual_winding[5],
                        actual_winding[6],
                        actual_winding[7],
                        oracle_winding[0],
                        oracle_winding[1],
                        oracle_winding[2],
                        oracle_winding[3],
                        oracle_winding[4],
                        oracle_winding[5],
                        oracle_winding[6],
                        oracle_winding[7]
                    );
                    all_pass = false;
                }
            }
        }
        all_pass
    }

    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L465-L484 (chrome/m156)
    fn distance_sq_to_segment(p: Point, a: Point, b: Point) -> f32 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len_sq = dx * dx + dy * dy;
        if len_sq == 0.0 {
            let px = p.x - a.x;
            let py = p.y - a.y;
            return px * px + py * py;
        }
        let mut t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq;
        t = f32::max(0.0, f32::min(1.0, t));
        let proj_x = a.x + t * dx;
        let proj_y = a.y + t * dy;
        let dist_sq_x = p.x - proj_x;
        let dist_sq_y = p.y - proj_y;
        dist_sq_x * dist_sq_x + dist_sq_y * dist_sq_y
    }

    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L486-L509 (chrome/m156)
    fn build_tile_line_map(&mut self) {
        let (Some(tiler), Some(polyline)) = (self.tiler, self.polyline) else {
            return;
        };
        for tile in tiler.get_tiles() {
            let line_idx =
                tile.packed_line_idx_intersection_mask >> IntersectionBits::INT_MASK_SHIFT;
            if (line_idx as usize) + 1 < polyline.points().len() {
                let line = polyline.get_line(line_idx);
                let tile_key = (u32::from(tile.y) << 16) | u32::from(tile.x);
                let lines = self.tile_line_map.entry(tile_key).or_default();
                let mut exists = false;
                for existing in lines.iter() {
                    if existing.p0 == line.p0 && existing.p1 == line.p1 {
                        exists = true;
                        break;
                    }
                }
                if !exists {
                    lines.push(line);
                }
            }
        }
    }

    // Port of: tests/graphite/sparse_strips/OracleValidator.cpp#L511-L537 (chrome/m156)
    fn is_within_tolerance(&self, sub_pt: Point, tile_x: u16, tile_y: u16) -> bool {
        for dy in -1i32..=1 {
            let ny = i32::from(tile_y) + dy;
            if ny < 0 {
                continue;
            }
            for dx in -1i32..=1 {
                let nx = i32::from(tile_x) + dx;
                if nx < 0 {
                    continue;
                }
                let key = ((ny as u32) << 16) | (nx as u32);
                if let Some(lines) = self.tile_line_map.get(&key) {
                    for line in lines {
                        if Self::distance_sq_to_segment(sub_pt, line.p0, line.p1)
                            <= self.tolerance_sq
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

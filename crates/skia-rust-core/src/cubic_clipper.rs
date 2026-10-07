// Copyright 2009 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCubicClipper.h, src/core/SkCubicClipper.cpp

//! `SkCubicClipper`: clips cubics (monotonic in Y) against a horizontal clip band.

use crate::geometry::chop_cubic_at;
use crate::point::Point;
use crate::rect::{IRect, Rect};
use crate::scalar::{SCALAR_1, scalar, scalar_abs, scalar_interp};

/// This class is initialized with a clip rectangle, and then can be fed cubics, which must
/// already be monotonic in Y.
///
/// In the future, it might return a series of segments, allowing it to clip also in X, to ensure
/// that all segments fit in a finite coordinate system.
// Port of: src/core/SkCubicClipper.h#L18-L32 (chrome/m156)
#[doc(alias = "SkCubicClipper")]
#[derive(Copy, Clone, Debug)]
pub struct CubicClipper {
    clip: Rect,
}

impl Default for CubicClipper {
    fn default() -> Self {
        Self::new()
    }
}

impl CubicClipper {
    /// `SkCubicClipper()`: the clip is the empty rectangle.
    // Port of: src/core/SkCubicClipper.cpp#L17-L19 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        let mut clip = Rect::default();
        clip.set_empty();
        Self { clip }
    }

    /// `setClip`.
    // Port of: src/core/SkCubicClipper.cpp#L21-L24 (chrome/m156)
    #[doc(alias = "setClip")]
    pub fn set_clip(&mut self, clip: &IRect) {
        // convert to scalars, since that's where we'll see the points
        self.clip = Rect::from_irect(clip);
    }

    /// Finds the `t` of the monotonic cubic `pts` where its Y crosses `y` by bisection, if it
    /// does.
    // Port of: src/core/SkCubicClipper.cpp#L27-L113 (chrome/m156)
    #[doc(alias = "ChopMonoAtY")]
    #[must_use]
    #[allow(clippy::similar_names, clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2`; names follow the C++
    pub fn chop_mono_at_y(pts: &[Point], y: scalar) -> Option<scalar> {
        let ycrv = [pts[0].y - y, pts[1].y - y, pts[2].y - y, pts[3].y - y];

        // Check that the endpoints straddle zero.
        let mut t_neg: scalar; // Negative and positive function parameters.
        let mut t_pos: scalar;
        if ycrv[0] < 0.0 {
            if ycrv[3] < 0.0 {
                return None;
            }
            t_neg = 0.0;
            t_pos = SCALAR_1;
        } else if ycrv[0] > 0.0 {
            if ycrv[3] > 0.0 {
                return None;
            }
            t_neg = SCALAR_1;
            t_pos = 0.0;
        } else {
            return Some(0.0);
        }

        let tol = SCALAR_1 / 65536.0; // 1 for fixed, 1e-5 for float.
        loop {
            let t_mid = (t_pos + t_neg) / 2.0;
            let y01 = scalar_interp(ycrv[0], ycrv[1], t_mid);
            let y12 = scalar_interp(ycrv[1], ycrv[2], t_mid);
            let y23 = scalar_interp(ycrv[2], ycrv[3], t_mid);
            let y012 = scalar_interp(y01, y12, t_mid);
            let y123 = scalar_interp(y12, y23, t_mid);
            let y0123 = scalar_interp(y012, y123, t_mid);
            if y0123 == 0.0 {
                return Some(t_mid);
            }
            if y0123 < 0.0 {
                t_neg = t_mid;
            } else {
                t_pos = t_mid;
            }
            // Nan-safe
            if scalar_abs(t_pos - t_neg) <= tol {
                break;
            }
        }

        #[allow(clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2`
        let t = (t_neg + t_pos) / 2.0;
        Some(t)
    }

    /// Clips the cubic `src_pts` (which must be monotonic in Y) to the clip's top and bottom,
    /// writing the result to `dst`. Returns false if the cubic is completely outside.
    // Port of: src/core/SkCubicClipper.cpp#L116-L156 (chrome/m156)
    #[doc(alias = "clipCubic")]
    #[must_use]
    pub fn clip_cubic(&self, src_pts: &[Point], dst: &mut [Point]) -> bool {
        // we need the data to be monotonically descending in Y
        let reverse = if src_pts[0].y > src_pts[3].y {
            dst[0] = src_pts[3];
            dst[1] = src_pts[2];
            dst[2] = src_pts[1];
            dst[3] = src_pts[0];
            true
        } else {
            dst[..4].copy_from_slice(&src_pts[..4]);
            false
        };

        // are we completely above or below
        let ctop = self.clip.top;
        let cbot = self.clip.bottom;
        if dst[3].y <= ctop || dst[0].y >= cbot {
            return false;
        }

        let mut tmp = [Point::default(); 7]; // for SkChopCubicAt

        // are we partially above
        if dst[0].y < ctop
            && let Some(t) = Self::chop_mono_at_y(dst, ctop)
        {
            chop_cubic_at(dst, &mut tmp, t);
            dst[0] = tmp[3];
            dst[1] = tmp[4];
            dst[2] = tmp[5];
        }

        // are we partially below
        if dst[3].y > cbot
            && let Some(t) = Self::chop_mono_at_y(dst, cbot)
        {
            chop_cubic_at(dst, &mut tmp, t);
            dst[1] = tmp[1];
            dst[2] = tmp[2];
            dst[3] = tmp[3];
        }

        if reverse {
            dst.swap(0, 3);
            dst.swap(1, 2);
        }
        true
    }
}

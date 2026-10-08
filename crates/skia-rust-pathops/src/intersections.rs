// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkIntersections.h, src/pathops/SkIntersections.cpp

//! The intersection result of two curves (`SkIntersections`): up to 13 (t, point) pairs, with
//! coincidence bits, and the bookkeeping shared by the `intersect*` routines.

use crate::point::{DPoint, DVector};
use crate::types::{BUMP_EPSILON, between, more_roughly_equal, precisely_equal, precisely_zero};

/// `SkIntersections`.
// Port of: src/pathops/SkIntersections.h#L31-L31 (chrome/m156)
#[doc(alias = "SkIntersections")]
#[derive(Clone, Debug)]
pub struct Intersections {
    /// `SkDPoint fPt[13]`.
    pub(crate) pt: [DPoint; 13],
    /// `SkDPoint fPt2[2]`: alternate intersection points for nearly-same end points.
    pub(crate) pt2: [DPoint; 2],
    /// `double fT[2][13]`: the t values of each curve.
    pub(crate) t: [[f64; 13]; 2],
    /// `uint16_t fIsCoincident[2]`: a bit per intersection, set when coincident.
    pub(crate) is_coincident: [u16; 2],
    /// `bool fNearlySame[2]`.
    pub(crate) nearly_same: [bool; 2],
    /// `unsigned char fUsed`.
    pub(crate) used: u8,
    /// `unsigned char fMax`: the caller's limit on the number of intersections.
    pub(crate) max: u8,
    /// `bool fAllowNear`.
    pub(crate) allow_near: bool,
    /// `bool fSwap`.
    pub(crate) swap: bool,
    /// `int fDepth`: recursion depth (debug bookkeeping, kept for parity).
    pub(crate) depth: i32,
}

impl Default for Intersections {
    /// `SkIntersections()`: `reset()` and `fMax = 0`.
    fn default() -> Self {
        let mut i = Self {
            pt: [DPoint::default(); 13],
            pt2: [DPoint::default(); 2],
            t: [[0.0; 13]; 2],
            is_coincident: [0; 2],
            nearly_same: [false; 2],
            used: 0,
            max: 0,
            allow_near: true,
            swap: false,
            depth: 0,
        };
        i.reset();
        i.max = 0; // require that the caller set the max
        i
    }
}

impl Intersections {
    /// `SkIntersections::used()`.
    #[must_use]
    pub fn used(&self) -> usize {
        usize::from(self.used)
    }

    /// `operator[](int n)[index]`: the t value of curve `which` (0 or 1) at `index`.
    #[must_use]
    pub fn t(&self, which: usize, index: usize) -> f64 {
        self.t[which][index]
    }

    /// `const SkDPoint& pt(int index) const`.
    #[must_use]
    pub fn pt(&self, index: usize) -> DPoint {
        self.pt[index]
    }

    /// `const SkDPoint& pt2(int index) const`.
    #[must_use]
    pub fn pt2(&self, index: usize) -> DPoint {
        self.pt2[index]
    }

    /// `void allowNear(bool nearAllowed)`.
    pub fn allow_near(&mut self, near_allowed: bool) {
        self.allow_near = near_allowed;
    }

    /// `bool hasT(double t) const`.
    // Port of: src/pathops/SkIntersections.h#L123-L126 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn has_t(&self, t: f64) -> bool {
        debug_assert!(t == 0.0 || t == 1.0);
        let used = usize::from(self.used);
        used > 0
            && (if t == 0.0 {
                self.t[0][0] == 0.0
            } else {
                self.t[0][used - 1] == 1.0
            })
    }

    /// `bool hasOppT(double t) const`.
    // Port of: src/pathops/SkIntersections.h#L128-L131 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn has_opp_t(&self, t: f64) -> bool {
        debug_assert!(t == 0.0 || t == 1.0);
        let used = usize::from(self.used);
        used > 0 && (self.t[1][0] == t || self.t[1][used - 1] == t)
    }

    /// `int insertSwap(double one, double two, const SkDPoint& pt)`.
    pub fn insert_swap(&mut self, one: f64, two: f64, pt: DPoint) -> i32 {
        if self.swap {
            self.insert(two, one, pt)
        } else {
            self.insert(one, two, pt)
        }
    }

    /// `bool isCoincident(int index)`.
    #[doc(alias = "isCoincident")]
    #[must_use]
    pub fn is_coincident(&self, index: usize) -> bool {
        (i32::from(self.is_coincident[0]) & (1 << index)) != 0
    }

    /// `int debugCoincidentUsed() const`: the number of coincident intersections. Called only by
    /// test code.
    // Port of: src/pathops/SkPathOpsDebug.cpp#L2645-L2661 (chrome/m156)
    #[doc(alias = "debugCoincidentUsed")]
    #[must_use]
    pub fn debug_coincident_used(&self) -> i32 {
        if self.is_coincident[0] == 0 {
            debug_assert_eq!(self.is_coincident[1], 0);
            return 0;
        }
        let mut count = 0;
        let mut count2 = 0;
        for index in 0..usize::from(self.used) {
            if (i32::from(self.is_coincident[0]) & (1 << index)) != 0 {
                count += 1;
            }
            if (i32::from(self.is_coincident[1]) & (1 << index)) != 0 {
                count2 += 1;
            }
        }
        debug_assert_eq!(count, count2);
        count
    }

    /// `bool nearlySame(int index) const`.
    #[doc(alias = "nearlySame")]
    #[must_use]
    pub fn nearly_same(&self, index: usize) -> bool {
        debug_assert!(index == 0 || index == 1);
        self.nearly_same[index]
    }

    /// `void clearCoincidence(int index)`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn clear_coincidence(&mut self, index: usize) {
        let bit = 1i32 << index;
        self.is_coincident[0] = (i32::from(self.is_coincident[0]) & !bit) as u16;
        self.is_coincident[1] = (i32::from(self.is_coincident[1]) & !bit) as u16;
    }

    /// `void reset()`.
    pub fn reset(&mut self) {
        self.allow_near = true;
        self.used = 0;
        self.is_coincident = [0; 2];
    }

    /// `void set(bool swap, int tIndex, double t)`.
    pub fn set(&mut self, swap: bool, t_index: usize, t: f64) {
        self.t[usize::from(swap)][t_index] = t;
    }

    /// `void setMax(int max)`.
    #[allow(clippy::cast_possible_truncation)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn set_max(&mut self, max: usize) {
        debug_assert!(max <= 13);
        self.max = max as u8;
    }

    /// `void swap()`.
    pub fn swap(&mut self) {
        self.swap = !self.swap;
    }

    /// `bool swapped() const`.
    #[must_use]
    pub fn swapped(&self) -> bool {
        self.swap
    }

    /// `void downDepth()`.
    pub fn down_depth(&mut self) {
        self.depth -= 1;
        debug_assert!(self.depth >= 0);
    }

    /// `void upDepth()`.
    pub fn up_depth(&mut self) {
        self.depth += 1;
        debug_assert!(self.depth < 16);
    }

    /// `int depth() const`.
    #[must_use]
    pub fn depth(&self) -> i32 {
        self.depth
    }

    /// `bool unBumpT(int index)`.
    // Port of: src/pathops/SkIntersections.h#L236-L244 (chrome/m156)
    #[allow(clippy::manual_assert_eq)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn un_bump_t(&mut self, index: usize) -> bool {
        debug_assert!(self.used == 1);
        self.t[0][index] = self.t[0][index] * (1.0 + BUMP_EPSILON * 2.0) - BUMP_EPSILON;
        if !between(0.0, self.t[0][index], 1.0) {
            self.used = 0;
            return false;
        }
        true
    }

    /// `void flip()`: maps the second curve's t values to `1 - t`.
    // Port of: src/pathops/SkIntersections.cpp#L30-L34 (chrome/m156)
    pub fn flip(&mut self) {
        for index in 0..usize::from(self.used) {
            self.t[1][index] = 1.0 - self.t[1][index];
        }
    }

    /// `int closestTo(double rangeStart, double rangeEnd, const SkDPoint& testPt, double* dist)`.
    // Port of: src/pathops/SkIntersections.cpp#L12-L28 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn closest_to(&self, range_start: f64, range_end: f64, test_pt: DPoint) -> (i32, f64) {
        let mut closest = -1;
        let mut closest_dist = f64::from(f32::MAX);
        for index in 0..usize::from(self.used) {
            if !between(range_start, self.t[0][index], range_end) {
                continue;
            }
            let i_pt = self.pt[index];
            let dist = test_pt.distance_squared(i_pt);
            if closest_dist > dist {
                closest_dist = dist;
                closest = index as i32;
            }
        }
        (closest, closest_dist)
    }

    /// `int mostOutside(double rangeStart, double rangeEnd, const SkDPoint& origin) const`.
    // Port of: src/pathops/SkIntersections.cpp#L143-L160 (chrome/m156)
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn most_outside(&self, range_start: f64, range_end: f64, origin: DPoint) -> i32 {
        let mut result = -1i32;
        for index in 0..usize::from(self.used) {
            if !between(range_start, self.t[0][index], range_end) {
                continue;
            }
            if result < 0 {
                result = index as i32;
                continue;
            }
            let best: DVector = self.pt[result as usize] - origin;
            let test: DVector = self.pt[index] - origin;
            if test.cross_check(best) < 0.0 {
                result = index as i32;
            }
        }
        result
    }

    /// `void merge(const SkIntersections& a, int aIndex, const SkIntersections& b, int bIndex)`.
    // Port of: src/pathops/SkIntersections.cpp#L133-L141 (chrome/m156)
    pub fn merge(&mut self, a: &Self, a_index: usize, b: &Self, b_index: usize) {
        self.reset();
        self.t[0][0] = a.t[0][a_index];
        self.t[1][0] = b.t[0][b_index];
        self.pt[0] = a.pt[a_index];
        self.pt2[0] = b.pt[b_index];
        self.used = 1;
    }

    /// `void setCoincident(int index)`.
    // Port of: src/pathops/SkIntersections.cpp#L126-L131 (chrome/m156)
    pub fn set_coincident(&mut self, index: usize) {
        let bit = 1u16 << index;
        self.is_coincident[0] |= bit;
        self.is_coincident[1] |= bit;
    }

    /// `int insertCoincident(double one, double two, const SkDPoint& pt)`.
    // Port of: src/pathops/SkIntersections.cpp#L118-L124 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn insert_coincident(&mut self, one: f64, two: f64, pt: DPoint) -> i32 {
        let index = self.insert_swap(one, two, pt);
        if index >= 0 {
            self.set_coincident(index as usize);
        }
        index
    }

    /// `void insertNear(double one, double two, const SkDPoint& pt1, const SkDPoint& pt2)`.
    // Port of: src/pathops/SkIntersections.cpp#L109-L116 (chrome/m156)
    #[allow(clippy::float_cmp, clippy::manual_assert_eq)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn insert_near(&mut self, one: f64, two: f64, pt1: DPoint, pt2: DPoint) {
        debug_assert!(one == 0.0 || one == 1.0);
        debug_assert!(two == 0.0 || two == 1.0);
        debug_assert!(pt1 != pt2);
        let idx = usize::from(one != 0.0);
        self.nearly_same[idx] = true;
        let _ = self.insert(one, two, pt1);
        self.pt2[idx] = pt2;
    }

    /// `void removeOne(int index)`.
    // Port of: src/pathops/SkIntersections.cpp#L162-L175 (chrome/m156)
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::manual_assert_eq
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn remove_one(&mut self, index: usize) {
        self.used -= 1;
        let used = usize::from(self.used);
        if used <= index {
            return;
        }
        let remaining = used - index;
        self.pt.copy_within(index + 1..index + 1 + remaining, index);
        self.t[0].copy_within(index + 1..index + 1 + remaining, index);
        self.t[1].copy_within(index + 1..index + 1 + remaining, index);
        let index_i = index as i32;
        let co_bit = i32::from(self.is_coincident[0]) & (1 << index_i);
        let clear = !((1i32 << index_i) - 1);
        let c0 = i32::from(self.is_coincident[0]);
        self.is_coincident[0] = (c0 - (((c0 >> 1) & clear) + co_bit)) as u16;
        debug_assert!((co_bit ^ (i32::from(self.is_coincident[1]) & (1 << index_i))) == 0);
        let c1 = i32::from(self.is_coincident[1]);
        self.is_coincident[1] = (c1 - (((c1 >> 1) & clear) + co_bit)) as u16;
    }

    /// `int insert(double one, double two, const SkDPoint& pt)`: adds an intersection in t order,
    /// merging duplicates. Returns the index it was stored at, or -1.
    // Port of: src/pathops/SkIntersections.cpp#L36-L107 (chrome/m156)
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::float_cmp
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn insert(&mut self, one: f64, two: f64, pt: DPoint) -> i32 {
        if self.is_coincident[0] == 3 && between(self.t[0][0], one, self.t[0][1]) {
            return -1;
        }
        let mut index = 0usize;
        while index < usize::from(self.used) {
            let old_one = self.t[0][index];
            let old_two = self.t[1][index];
            if one == old_one && two == old_two {
                return -1;
            }
            if more_roughly_equal(old_one, one) && more_roughly_equal(old_two, two) {
                if (!precisely_zero(one) || precisely_zero(old_one))
                    && (!precisely_equal(one, 1.0) || precisely_equal(old_one, 1.0))
                    && (!precisely_zero(two) || precisely_zero(old_two))
                    && (!precisely_equal(two, 1.0) || precisely_equal(old_two, 1.0))
                {
                    return -1;
                }
                debug_assert!((0.0..=1.0).contains(&one));
                debug_assert!((0.0..=1.0).contains(&two));
                let remaining = usize::from(self.used) - index - 1;
                self.pt.copy_within(index + 1..index + 1 + remaining, index);
                self.t[0].copy_within(index + 1..index + 1 + remaining, index);
                self.t[1].copy_within(index + 1..index + 1 + remaining, index);
                let index_i = index as i32;
                let clear_mask = !((1i32 << index_i) - 1);
                let c0 = i32::from(self.is_coincident[0]);
                self.is_coincident[0] = (c0 - ((c0 >> 1) & clear_mask)) as u16;
                let c1 = i32::from(self.is_coincident[1]);
                self.is_coincident[1] = (c1 - ((c1 >> 1) & clear_mask)) as u16;
                self.used -= 1;
                break;
            }
            index += 1;
        }
        let mut index = 0usize;
        while index < usize::from(self.used) {
            if self.t[0][index] > one {
                break;
            }
            index += 1;
        }
        if self.used >= self.max {
            // SkOPASSERT(0): this error, if it is to be handled at runtime in release, must
            // be handled by the caller. Skia's debug assert is skipped here when the global
            // state's `debugSkipAssert` is set (the fuzz cases, `SkipAssert::kYes`). This port has
            // no such flag, so the assert is omitted and the release behavior is kept.
            self.used = 0;
            return 0;
        }
        let remaining = usize::from(self.used) - index;
        if remaining > 0 {
            self.pt.copy_within(index..index + remaining, index + 1);
            self.t[0].copy_within(index..index + remaining, index + 1);
            self.t[1].copy_within(index..index + remaining, index + 1);
            let index_i = index as i32;
            let clear_mask = !((1i32 << index_i) - 1);
            let c0 = i32::from(self.is_coincident[0]);
            self.is_coincident[0] = (c0 + (c0 & clear_mask)) as u16;
            let c1 = i32::from(self.is_coincident[1]);
            self.is_coincident[1] = (c1 + (c1 & clear_mask)) as u16;
        }
        self.pt[index] = pt;
        if !(0.0..=1.0).contains(&one) {
            return -1;
        }
        if !(0.0..=1.0).contains(&two) {
            return -1;
        }
        self.t[0][index] = one;
        self.t[1][index] = two;
        self.used += 1;
        index as i32
    }
}

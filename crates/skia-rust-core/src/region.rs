// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkRegion.h, src/core/SkRegion.cpp, src/core/SkRegionPriv.h

//! Describes the set of pixels used to clip a canvas (`SkRegion.h`).
//!
//! [`Region`] is compact, efficiently storing a single integer rectangle, or a run length encoded
//! array of rectangles. The [`Iterator`] returns the scan lines or rectangles contained by it,
//! optionally intersecting a bounding rectangle.
//!
//! `SkRegion_path.cpp` is split by what it needs: `addBoundaryPath`/`getBoundaryPath` are in
//! [`region_path`](crate::region_path) (inherent methods of [`Region`]); `setPath` needs the scan
//! converter, which lives in `skia-rust-raster`, so it is the extension method
//! `skia_rust_raster::region_path::RegionExt::set_path`.

use std::{cmp::Ordering, fmt, iter, sync::Arc};

use crate::point::{IPoint, IVector};
use crate::rect::{Contains, IRect};
use crate::safe_math::SafeMath;

/// A single run-array value (`SkRegion::RunType`).
type RunType = i32;

/// `SkRegion::kRectRegionRuns`: the number of run values describing one rectangle.
const RECT_REGION_RUNS: usize = 7;

/// `SkRegion_kRunTypeSentinel`.
const SENTINEL: RunType = 0x7FFF_FFFF;

/// Converts a run-array count or index that is non-negative in a valid region to `usize`.
#[allow(clippy::cast_sign_loss)] // mirrors the implicit int -> size_t conversions; values are >= 0
fn to_usize(v: i32) -> usize {
    debug_assert!(v >= 0);
    v as usize
}

/// Number of `i32` values of the stack-allocated run array in Skia (`kRunArrayStackCount`).
const RUN_ARRAY_STACK_COUNT: usize = 256;

/// The logical operations that can be performed when combining two regions.
// Port of: include/core/SkRegion.h#L330-L340 (chrome/m156)
#[doc(alias = "SkRegion::Op")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(i32)]
pub enum Op {
    /// Target minus operand.
    #[doc(alias = "kDifference_Op")]
    Difference = 0,
    /// Target intersected with operand.
    #[doc(alias = "kIntersect_Op")]
    Intersect = 1,
    /// Target unioned with operand.
    #[doc(alias = "kUnion_Op")]
    Union = 2,
    /// Target exclusive or with operand.
    #[doc(alias = "kXOR_Op")]
    XOR = 3,
    /// Operand minus target.
    #[doc(alias = "kReverseDifference_Op")]
    ReverseDifference = 4,
    /// Replace target with operand.
    #[doc(alias = "kReplace_Op")]
    Replace = 5,
}

/// `skia-safe` spells the operation enum `RegionOp`.
pub type RegionOp = Op;

/// Overload-style `quick_reject`, as in `skia-safe`'s `QuickReject`.
pub trait QuickReject<T> {
    /// Returns true if `other` can be proven not to intersect `self`.
    fn quick_reject(&self, other: &T) -> bool;
}

/// The shared, copy-on-write run array of a complex region (`SkRegion::RunHead`).
///
/// Skia stores a ref-count and the run count in front of the runs; here `Arc` is the ref-count
/// and `runs.len()` is `fRunCount`.
// Port of: src/core/SkRegionPriv.h#L66-L260 (chrome/m156)
#[derive(Clone, Debug)]
struct RunHead {
    y_span_count: i32,
    interval_count: i32,
    runs: Vec<RunType>,
}

impl RunHead {
    /// Given a scanline start (its Bottom value), returns the start of the next scanline.
    // Port of: src/core/SkRegionPriv.h#L143-L162 (chrome/m156)
    fn skip_entire_scanline(runs: &[RunType], pos: usize) -> usize {
        // we are not the Y Sentinel
        debug_assert!(runs[pos] < SENTINEL);
        let intervals = to_usize(runs[pos + 1]); // non-negative in a valid region
        debug_assert_eq!(runs[pos + 2 + intervals * 2], SENTINEL);
        // skip the entire line [B N [L R] S]
        pos + 1 + 1 + intervals * 2 + 1
    }

    /// Returns the index of the scanline (at its Bottom value) that contains `y`. `y` must be
    /// within the bounds of the region.
    // Port of: src/core/SkRegionPriv.h#L170-L186 (chrome/m156)
    fn find_scanline(&self, y: i32) -> usize {
        let runs = &self.runs;
        // if the top-check fails, we didn't do a quick check on the bounds
        debug_assert!(y >= runs[0]);
        let mut pos = 1; // skip top-Y
        loop {
            let bottom = runs[pos];
            // If we hit this, we've walked off the region, and our bounds check failed.
            debug_assert!(bottom < SENTINEL);
            if y < bottom {
                break;
            }
            pos = Self::skip_entire_scanline(runs, pos);
        }
        pos
    }

    /// Computes interval counts and bounds of the runs; stores the counts and returns the bounds.
    // Port of: src/core/SkRegionPriv.h#L189-L252 (chrome/m156)
    fn compute_run_bounds(&mut self) -> IRect {
        let runs = &self.runs;
        let mut bounds = IRect::default();
        let mut pos = 0;
        bounds.top = runs[pos];
        pos += 1;

        let mut bot;
        let mut y_span_count = 0;
        let mut interval_count = 0;
        let mut left = i32::MAX;
        let mut rite = i32::MIN;

        loop {
            bot = runs[pos];
            pos += 1;
            debug_assert!(bot < SENTINEL);
            y_span_count += 1;

            let intervals = runs[pos];
            pos += 1;
            debug_assert!(intervals >= 0);
            debug_assert!(intervals < SENTINEL);

            if intervals > 0 {
                let l = runs[pos];
                debug_assert!(l < SENTINEL);
                if left > l {
                    left = l;
                }

                pos += to_usize(intervals) * 2; // intervals > 0
                let r = runs[pos - 1];
                debug_assert!(r < SENTINEL);
                if rite < r {
                    rite = r;
                }

                interval_count += intervals;
            }
            debug_assert_eq!(runs[pos], SENTINEL);
            pos += 1; // skip x-sentinel

            // test Y-sentinel
            if runs[pos] == SENTINEL {
                break;
            }
        }

        // +1 to skip the last Y-sentinel
        debug_assert_eq!(pos + 1, runs.len());

        self.y_span_count = y_span_count;
        self.interval_count = interval_count;

        bounds.left = left;
        bounds.right = rite;
        bounds.bottom = bot;
        bounds
    }
}

#[derive(Clone, Debug)]
enum Runs {
    /// `SkRegion_gEmptyRunHeadPtr`.
    Empty,
    /// `SkRegion_gRectRunHeadPtr`.
    Rect,
    Complex(Arc<RunHead>),
}

/// Describes the set of pixels used to clip a canvas.
///
/// Copying a region is very efficient and never allocates: the run array is shared and copied
/// when modified.
// Port of: include/core/SkRegion.h#L22-L601 (chrome/m156)
#[doc(alias = "SkRegion")]
#[derive(Clone)]
pub struct Region {
    bounds: IRect,
    runs: Runs,
}

impl Default for Region {
    /// Constructs an empty region (`SkRegion::SkRegion`).
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Region {
    /// Compares the region and `rhs`; returns true if they enclose exactly the same area.
    // Port of: src/core/SkRegion.cpp#L582-L607 (chrome/m156)
    fn eq(&self, b: &Self) -> bool {
        self.validate();
        b.validate();

        if self.bounds != b.bounds {
            return false;
        }
        match (&self.runs, &b.runs) {
            // this catches empties and rects being equal
            (Runs::Empty, Runs::Empty) | (Runs::Rect, Runs::Rect) => true,
            (Runs::Complex(ah), Runs::Complex(bh)) => Arc::ptr_eq(ah, bh) || ah.runs == bh.runs,
            // now we insist that both are complex
            _ => false,
        }
    }
}

impl Eq for Region {}

impl fmt::Debug for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Region")
            .field("is_empty", &self.is_empty())
            .field("is_rect", &self.is_rect())
            .field("is_complex", &self.is_complex())
            .field("bounds", self.bounds())
            .finish()
    }
}

/// Reads a native-endian `i32` at `pos` (`SkRBuffer::readS32`).
fn read_i32_ne(bytes: &[u8], pos: usize) -> Option<i32> {
    let chunk = bytes.get(pos..pos.checked_add(4)?)?;
    Some(i32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

impl Region {
    /// Constructs an empty region. The region is set to empty bounds at (0, 0) with zero width and
    /// height.
    // Port of: src/core/SkRegion.cpp#L146-L149 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            bounds: IRect::new_empty(),
            runs: Runs::Empty,
        }
    }

    /// Constructs a rectangular region matching the bounds of `rect`.
    // Port of: src/core/SkRegion.cpp#L156-L159 (chrome/m156)
    #[must_use]
    pub fn from_rect(rect: impl AsRef<IRect>) -> Self {
        let mut r = Self::new();
        r.set_rect(rect);
        r
    }

    /// Sets the region to `src`, and returns true if `src` bounds is not empty.
    ///
    /// Internally, the region and `src` share the run array, which is copied when modified.
    // Port of: include/core/SkRegion.h#L145-L148 (chrome/m156)
    pub fn set(&mut self, src: &Region) -> bool {
        self.set_region(src);
        !self.is_empty()
    }

    /// Exchanges the rectangle array of the region and `other`.
    // Port of: src/core/SkRegion.cpp#L193-L197 (chrome/m156)
    pub fn swap(&mut self, other: &mut Region) {
        std::mem::swap(self, other);
    }

    /// Returns true if the region is empty.
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.runs, Runs::Empty)
    }

    /// Returns true if the region is one [`IRect`] with positive dimensions.
    #[doc(alias = "isRect")]
    #[must_use]
    pub fn is_rect(&self) -> bool {
        matches!(self.runs, Runs::Rect)
    }

    /// Returns true if the region is described by more than one rectangle.
    #[doc(alias = "isComplex")]
    #[must_use]
    pub fn is_complex(&self) -> bool {
        matches!(self.runs, Runs::Complex(_))
    }

    /// Returns the minimum and maximum axes values of the rectangle array. Returns (0, 0, 0, 0) if
    /// the region is empty.
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> &IRect {
        &self.bounds
    }

    /// Returns a value that increases with the number of elements in the region.
    // Port of: src/core/SkRegion.cpp#L199-L206 (chrome/m156)
    #[doc(alias = "computeRegionComplexity")]
    #[must_use]
    pub fn compute_region_complexity(&self) -> usize {
        match &self.runs {
            Runs::Empty => 0,
            Runs::Rect => 1,
            Runs::Complex(head) => usize::try_from(head.interval_count).unwrap_or(0),
        }
    }

    /// Constructs an empty region. Always returns false.
    // Port of: src/core/SkRegion.cpp#L208-L213 (chrome/m156)
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) -> bool {
        self.bounds.set_empty();
        self.runs = Runs::Empty;
        false
    }

    /// Constructs a rectangular region matching the bounds of `rect`. If `rect` is empty,
    /// constructs an empty region and returns false.
    // Port of: src/core/SkRegion.cpp#L215-L225 (chrome/m156)
    #[doc(alias = "setRect")]
    pub fn set_rect(&mut self, rect: impl AsRef<IRect>) -> bool {
        let r = rect.as_ref();
        if r.is_empty() || SENTINEL == r.right || SENTINEL == r.bottom {
            return self.set_empty();
        }
        self.bounds = *r;
        self.runs = Runs::Rect;
        true
    }

    /// Constructs a region as the union of the rectangles in `rects`. If `rects` is empty,
    /// constructs an empty region. Returns false if the constructed region is empty.
    // Port of: src/core/SkRegion.cpp#L680-L693 (chrome/m156)
    #[doc(alias = "setRects")]
    pub fn set_rects(&mut self, rects: &[IRect]) -> bool {
        if rects.is_empty() {
            return self.set_empty();
        }
        if rects.len() == 1 {
            return self.set_rect(rects[0]);
        }
        // The depth here is O(log_2(count)) so we shouldn't run out of stackframes.
        let mid = rects.len() / 2;
        let mut left = Region::new();
        let mut right = Region::new();
        left.set_rects(&rects[..mid]);
        right.set_rects(&rects[mid..]);
        self.op_region_region(&left, &right, Op::Union)
    }

    /// Sets the region to a copy of `region`; returns true if the region is not empty.
    // Port of: src/core/SkRegion.cpp#L227-L238 (chrome/m156)
    #[doc(alias = "setRegion")]
    pub fn set_region(&mut self, region: &Region) -> bool {
        self.bounds = region.bounds;
        self.runs = region.runs.clone();
        !self.is_empty()
    }

    // `setPath(const SkPath&, const SkRegion&)`: needs SkPath and scan conversion; not ported.

    /// Returns true if the region intersects `rect`. Returns false if either `rect` or the region
    /// is empty, or they do not intersect.
    // Port of: src/core/SkRegion.cpp#L525-L552 (chrome/m156)
    #[doc(alias = "intersects")]
    #[must_use]
    pub fn intersects_rect(&self, rect: impl AsRef<IRect>) -> bool {
        let r = rect.as_ref();
        self.validate();

        if self.is_empty() || r.is_empty() {
            return false;
        }

        let Some(sect) = IRect::intersect(&self.bounds, r) else {
            return false;
        };
        let Runs::Complex(head) = &self.runs else {
            return true; // a rect
        };

        let mut scanline = head.find_scanline(sect.top);
        loop {
            if scanline_intersects(&head.runs, scanline, sect.left, sect.right) {
                return true;
            }
            if sect.bottom <= head.runs[scanline] {
                break;
            }
            scanline = scanline_next(&head.runs, scanline);
        }
        false
    }

    /// Returns true if the region intersects `other`. Returns false if either `other` or the
    /// region is empty, or they do not intersect.
    // Port of: src/core/SkRegion.cpp#L554-L578 (chrome/m156)
    #[doc(alias = "intersects")]
    #[must_use]
    pub fn intersects_region(&self, other: &Region) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }

        if !IRect::intersects(&self.bounds, &other.bounds) {
            return false;
        }

        let we_are_a_rect = self.is_rect();
        let they_are_a_rect = other.is_rect();

        if we_are_a_rect && they_are_a_rect {
            return true;
        }
        if we_are_a_rect {
            return other.intersects_rect(self.bounds());
        }
        if they_are_a_rect {
            return self.intersects_rect(other.bounds());
        }

        // both of us are complex
        oper(self, other, Op::Intersect, None)
    }

    /// Returns true if the point (`point.x`, `point.y`) is inside the region. Returns false if the
    /// region is empty.
    // Port of: src/core/SkRegion.cpp#L387-L419 (chrome/m156)
    #[doc(alias = "contains")]
    #[must_use]
    pub fn contains_point(&self, point: IPoint) -> bool {
        self.validate();

        if !self.bounds.contains(point) {
            return false;
        }
        let Runs::Complex(head) = &self.runs else {
            return true; // a rect
        };

        let runs = &head.runs;
        // Skip the Bottom and IntervalCount
        let mut pos = head.find_scanline(point.y) + 2;

        // Just walk this scanline, checking each interval. The X-sentinel will
        // appear as a left-inteval (runs[0]) and should abort the search.
        //
        // We could do a bsearch, using interval-count (runs[1]), but need to time
        // when that would be worthwhile.
        loop {
            if point.x < runs[pos] {
                break;
            }
            if point.x < runs[pos + 1] {
                return true;
            }
            pos += 2;
        }
        false
    }

    /// Returns true if `rect` is completely inside the region. Returns false if the region or
    /// `rect` is empty.
    // Port of: src/core/SkRegion.cpp#L445-L467 (chrome/m156)
    #[doc(alias = "contains")]
    #[must_use]
    pub fn contains_rect(&self, rect: impl AsRef<IRect>) -> bool {
        let r = rect.as_ref();
        self.validate();

        if !self.bounds.contains(r) {
            return false;
        }
        let Runs::Complex(head) = &self.runs else {
            return true; // a rect
        };

        let mut scanline = head.find_scanline(r.top);
        loop {
            if !scanline_contains(&head.runs, scanline, r.left, r.right) {
                return false;
            }
            if r.bottom <= head.runs[scanline] {
                break;
            }
            scanline = scanline_next(&head.runs, scanline);
        }
        true
    }

    /// Returns true if `other` is completely inside the region. Returns false if the region or
    /// `other` is empty.
    // Port of: src/core/SkRegion.cpp#L469-L488 (chrome/m156)
    #[doc(alias = "contains")]
    #[must_use]
    pub fn contains_region(&self, other: &Region) -> bool {
        self.validate();
        other.validate();

        if self.is_empty() || other.is_empty() || !self.bounds.contains(&other.bounds) {
            return false;
        }
        if self.is_rect() {
            return true;
        }
        if other.is_rect() {
            return self.contains_rect(other.bounds());
        }

        // A contains B is equivalent to B - A == 0
        !oper(other, self, Op::Difference, None)
    }

    /// Returns true if the region is a single rectangle and contains `r`. May return false even
    /// though the region contains `r`.
    // Port of: include/core/SkRegion.h#L412-L420 (chrome/m156)
    #[doc(alias = "quickContains")]
    #[must_use]
    pub fn quick_contains(&self, r: impl AsRef<IRect>) -> bool {
        let r = r.as_ref();
        debug_assert_eq!(self.is_empty(), self.bounds.is_empty()); // valid region
        r.left < r.right
            && r.top < r.bottom
            && self.is_rect()
            && self.bounds.left <= r.left
            && self.bounds.top <= r.top
            && self.bounds.right >= r.right
            && self.bounds.bottom >= r.bottom
    }

    /// Returns true if the region does not intersect `rect`. Returns true if `rect` is empty or
    /// the region is empty. May return false even though the region does not intersect `rect`.
    // Port of: include/core/SkRegion.h#L432-L435 (chrome/m156)
    #[doc(alias = "quickReject")]
    #[must_use]
    pub fn quick_reject_rect(&self, rect: impl AsRef<IRect>) -> bool {
        let rect = rect.as_ref();
        self.is_empty() || rect.is_empty() || !IRect::intersects(&self.bounds, rect)
    }

    /// Returns true if the region does not intersect `rgn`. Returns true if `rgn` is empty or the
    /// region is empty. May return false even though the region does not intersect `rgn`.
    // Port of: include/core/SkRegion.h#L444-L447 (chrome/m156)
    #[doc(alias = "quickReject")]
    #[must_use]
    pub fn quick_reject_region(&self, rgn: &Region) -> bool {
        self.is_empty() || rgn.is_empty() || !IRect::intersects(&self.bounds, &rgn.bounds)
    }

    /// Offsets the region by the vector (`d.x`, `d.y`). Has no effect if the region is empty.
    // Port of: include/core/SkRegion.h#L453 (chrome/m156)
    pub fn translate(&mut self, d: impl Into<IVector>) {
        let d = d.into();
        let src = self.clone();
        src.translate_to(d.x, d.y, self);
    }

    /// Returns a copy of the region offset by the vector (`d.x`, `d.y`).
    #[must_use]
    pub fn translated(&self, d: impl Into<IVector>) -> Self {
        let mut r = self.clone();
        r.translate(d);
        r
    }

    /// Translates the region by (`dx`, `dy`), storing the result in `dst`.
    ///
    /// This is `SkRegion::translate(int, int, SkRegion*)`; if the region is empty, `dst` is set
    /// to empty.
    // Port of: src/core/SkRegion.cpp#L619-L676 (chrome/m156)
    #[doc(alias = "translate")]
    pub fn translate_to(&self, dx: i32, dy: i32, dst: &mut Region) {
        self.validate();

        if self.is_empty() {
            dst.set_empty();
            return;
        }
        // pin dx and dy so we don't overflow our existing bounds
        let dx = pin_offset_s32(self.bounds.left, self.bounds.right, dx);
        let dy = pin_offset_s32(self.bounds.top, self.bounds.bottom, dy);

        match &self.runs {
            Runs::Empty => unreachable!("empty regions return early"),
            Runs::Rect => {
                dst.set_rect(self.bounds.with_offset((dx, dy)));
            }
            Runs::Complex(head) => {
                let mut druns: Vec<RunType> = Vec::with_capacity(head.runs.len());

                let sruns = &head.runs;
                let mut s = 0;
                druns.push(sruns[s].wrapping_add(dy)); // top
                s += 1;
                loop {
                    let bottom = sruns[s];
                    s += 1;
                    if bottom == SENTINEL {
                        break;
                    }
                    druns.push(bottom.wrapping_add(dy)); // bottom
                    druns.push(sruns[s]); // copy intervalCount
                    s += 1;
                    loop {
                        let x = sruns[s];
                        s += 1;
                        if x == SENTINEL {
                            break;
                        }
                        druns.push(x.wrapping_add(dx));
                        druns.push(sruns[s].wrapping_add(dx));
                        s += 1;
                    }
                    druns.push(SENTINEL); // x sentinel
                }
                druns.push(SENTINEL); // y sentinel

                debug_assert_eq!(s, head.runs.len());
                debug_assert_eq!(druns.len(), head.runs.len());

                let new_head = RunHead {
                    y_span_count: head.y_span_count,
                    interval_count: head.interval_count,
                    runs: druns,
                };
                let mut bounds = self.bounds;
                bounds.offset((dx, dy));
                dst.bounds = bounds;
                dst.runs = Runs::Complex(Arc::new(new_head));
            }
        }

        self.validate();
    }

    /// Replaces the region with the result of the region `op` `rect`. Returns true if the replaced
    /// region is not empty.
    // Port of: include/core/SkRegion.h#L517-L525 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_rect(&mut self, rect: impl AsRef<IRect>, op: Op) -> bool {
        let rect = rect.as_ref();
        if self.is_rect() && Op::Intersect == op {
            return if let Some(b) = IRect::intersect(&self.bounds, rect) {
                self.bounds = b;
                true
            } else {
                self.set_empty()
            };
        }
        let this = self.clone();
        self.op_region_rect(&this, rect, op)
    }

    /// Replaces the region with the result of the region `op` `region`. Returns true if the
    /// replaced region is not empty.
    // Port of: include/core/SkRegion.h#L534 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_region(&mut self, region: &Region, op: Op) -> bool {
        let this = self.clone();
        self.op_region_region(&this, region, op)
    }

    /// Replaces the region with the result of `rect` `op` `region`. Returns true if the replaced
    /// region is not empty.
    // Port of: src/core/SkRegion.cpp#L240-L244 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_rect_region(&mut self, rect: impl AsRef<IRect>, region: &Region, op: Op) -> bool {
        let tmp = Region::from_rect(rect);
        self.op_region_region(&tmp, region, op)
    }

    /// Replaces the region with the result of `region` `op` `rect`. Returns true if the replaced
    /// region is not empty.
    // Port of: src/core/SkRegion.cpp#L246-L250 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_region_rect(&mut self, region: &Region, rect: impl AsRef<IRect>, op: Op) -> bool {
        let tmp = Region::from_rect(rect);
        self.op_region_region(region, &tmp, op)
    }

    /// Replaces the region with the result of `rgna` `op` `rgnb`. Returns true if the replaced
    /// region is not empty.
    // Port of: src/core/SkRegion.cpp#L1177-L1180 (chrome/m156)
    #[doc(alias = "op")]
    #[allow(clippy::similar_names)] // mirrors the C++ names rgna/rgnb
    pub fn op_region_region(&mut self, rgna: &Region, rgnb: &Region, op: Op) -> bool {
        self.validate();
        oper(rgna, rgnb, op, Some(self))
    }

    /// Returns the number of bytes `write_to_memory` writes (`writeToMemory(nullptr)`).
    // Port of: src/core/SkRegion.cpp#L1184-L1195 (chrome/m156)
    #[doc(alias = "writeToMemory")]
    #[must_use]
    pub fn write_to_memory_size(&self) -> usize {
        let mut size = 4; // -1 (empty), 0 (rect), runCount
        match &self.runs {
            Runs::Empty => {}
            Runs::Rect => size += 16, // sizeof(fBounds)
            Runs::Complex(head) => {
                size += 16; // sizeof(fBounds)
                size += 2 * 4; // ySpanCount + intervalCount
                size += head.runs.len() * 4;
            }
        }
        size
    }

    /// Writes the region to `buf` (replacing its contents) in Skia's serialized format, using
    /// native-endian `i32`s.
    // Port of: src/core/SkRegion.cpp#L1196-L1215 (chrome/m156)
    #[doc(alias = "writeToMemory")]
    pub fn write_to_memory(&self, buf: &mut Vec<u8>) {
        buf.clear();
        buf.reserve(self.write_to_memory_size());

        match &self.runs {
            Runs::Empty => buf.extend_from_slice(&(-1i32).to_ne_bytes()),
            Runs::Rect | Runs::Complex(_) => {
                let head = match &self.runs {
                    Runs::Complex(head) => Some(head),
                    _ => None,
                };
                // the run count always fits: Skia's `RunHead::Alloc` refuses larger sizes
                let count = head.map_or(0, |h| i32::try_from(h.runs.len()).unwrap_or(i32::MAX));
                let mut values = vec![
                    count,
                    self.bounds.left,
                    self.bounds.top,
                    self.bounds.right,
                    self.bounds.bottom,
                ];
                if let Some(head) = head {
                    values.push(head.y_span_count);
                    values.push(head.interval_count);
                    values.extend_from_slice(&head.runs);
                }
                for v in values {
                    buf.extend_from_slice(&v.to_ne_bytes());
                }
            }
        }
        debug_assert_eq!(buf.len(), self.write_to_memory_size());
    }

    /// Constructs the region from `buf`. Returns the bytes read; the value is a multiple of four,
    /// or zero if the data was invalid or too short (the region is then left unchanged).
    // Port of: src/core/SkRegion.cpp#L1323-L1364 (chrome/m156)
    #[doc(alias = "readFromMemory")]
    pub fn read_from_memory(&mut self, buf: &[u8]) -> usize {
        let mut tmp = Region::new();
        let mut pos = 0;

        // Serialized Region Format:
        //    Empty:
        //       -1
        //    Simple Rect:
        //       0  LEFT TOP RIGHT BOTTOM
        //    Complex Region:
        //       COUNT LEFT TOP RIGHT BOTTOM Y_SPAN_COUNT TOTAL_INTERVAL_COUNT [RUNS....]
        let Some(count) = read_i32_ne(buf, pos) else {
            return 0;
        };
        pos += 4;
        if count < -1 {
            return 0;
        }
        if count >= 0 {
            let mut b = [0i32; 4];
            for v in &mut b {
                let Some(x) = read_i32_ne(buf, pos) else {
                    return 0; // Short buffer
                };
                *v = x;
                pos += 4;
            }
            tmp.bounds = IRect::new(b[0], b[1], b[2], b[3]);
            if tmp.bounds.is_empty() {
                return 0; // bad bounds for non-empty region; report failure.
            }
            if count == 0 {
                tmp.runs = Runs::Rect;
            } else {
                let (Some(y_span_count), Some(interval_count)) =
                    (read_i32_ne(buf, pos), read_i32_ne(buf, pos + 4))
                else {
                    return 0;
                };
                pos += 8;
                let count = to_usize(count); // count > 0
                if buf.len() - pos < count * 4 {
                    return 0;
                }
                let runs: Vec<RunType> = (0..count)
                    .map(|i| read_i32_ne(buf, pos + i * 4).unwrap_or(0))
                    .collect();
                if !validate_run(&runs, &tmp.bounds, y_span_count, interval_count) {
                    return 0; // invalid runs, don't even allocate
                }
                tmp.runs = Runs::Complex(Arc::new(RunHead {
                    y_span_count,
                    interval_count,
                    runs,
                }));
                pos += count * 4;
            }
        }
        debug_assert!(tmp.is_valid());
        self.swap(&mut tmp);
        pos
    }

    // Port of: src/core/SkRegion.cpp#L1368-L1381 (chrome/m156)
    fn is_valid(&self) -> bool {
        match &self.runs {
            Runs::Empty => self.bounds == IRect::new(0, 0, 0, 0),
            _ if self.bounds.is_empty() => false,
            Runs::Rect => true,
            Runs::Complex(head) => validate_run(
                &head.runs,
                &self.bounds,
                head.y_span_count,
                head.interval_count,
            ),
        }
    }

    /// `SkRegionPriv::Validate`: only checked in debug builds (Skia: `SK_DEBUG`).
    fn validate(&self) {
        debug_assert!(self.is_valid());
    }

    /// Returns the runs of the region, using `tmp_storage` for the empty and rect cases.
    // Port of: src/core/SkRegion.cpp#L490-L506 (chrome/m156)
    fn get_runs<'a>(&'a self, tmp_storage: &'a mut [RunType; RECT_REGION_RUNS]) -> &'a [RunType] {
        match &self.runs {
            Runs::Empty => {
                tmp_storage[0] = SENTINEL;
                &tmp_storage[..]
            }
            Runs::Rect => {
                build_rect_runs(&self.bounds, tmp_storage);
                &tmp_storage[..]
            }
            Runs::Complex(head) => &head.runs,
        }
    }

    /// Replaces the region with the runs `runs[..count]` (`SkRegion::setRuns`), trimming empty
    /// top and bottom spans. The run array is modified.
    // Port of: src/core/SkRegion.cpp#L302-L374 (chrome/m156)
    fn set_runs(&mut self, runs: &mut [RunType], count: usize) -> bool {
        self.validate();
        debug_assert!(count > 0);

        if is_run_count_empty(count) {
            debug_assert_eq!(runs[count - 1], SENTINEL);
            return self.set_empty();
        }

        let mut start = 0;
        let mut stop = count;

        // trim off any empty spans from the top and bottom
        // weird I should need this, perhaps op() could be smarter...
        if count > RECT_REGION_RUNS {
            debug_assert!(runs[0] != SENTINEL); // top
            debug_assert!(runs[1] != SENTINEL); // bottom
            // runs[2] is uncomputed intervalCount

            if runs[3] == SENTINEL {
                // should be first left...
                start += 3; // skip empty initial span
                runs[start] = runs[start - 2]; // set new top to prev bottom
                debug_assert!(runs[start + 1] != SENTINEL); // bot: a sentinel would mean two in a row
                debug_assert!(runs[start + 2] != SENTINEL); // intervalcount
                debug_assert!(runs[start + 3] != SENTINEL); // left
                debug_assert!(runs[start + 4] != SENTINEL); // right
            }

            debug_assert_eq!(runs[stop - 1], SENTINEL);
            debug_assert_eq!(runs[stop - 2], SENTINEL);

            // now check for a trailing empty span
            if runs[stop - 5] == SENTINEL {
                // eek, stop[-4] was a bottom with no x-runs
                runs[stop - 4] = SENTINEL; // kill empty last span
                stop -= 3;
                debug_assert_eq!(runs[stop - 1], SENTINEL); // last y-sentinel
                debug_assert_eq!(runs[stop - 2], SENTINEL); // last x-sentinel
                debug_assert!(runs[stop - 3] != SENTINEL); // last right
                debug_assert!(runs[stop - 4] != SENTINEL); // last left
                debug_assert!(runs[stop - 5] != SENTINEL); // last interval-count
                debug_assert!(runs[stop - 6] != SENTINEL); // last bottom
            }
        }
        let runs = &runs[start..stop];

        debug_assert!(runs.len() >= RECT_REGION_RUNS);

        let mut bounds = IRect::default();
        if runs_are_a_rect(runs, &mut bounds) {
            return self.set_rect(bounds);
        }

        //  if we get here, we need to become a complex region
        let mut head = RunHead {
            y_span_count: 0,
            interval_count: 0,
            runs: runs.to_vec(),
        };
        self.bounds = head.compute_run_bounds();
        self.runs = Runs::Complex(Arc::new(head));

        // Our computed bounds might be too large, so we have to check here.
        if self.bounds.is_empty() {
            return self.set_empty();
        }

        self.validate();

        true
    }
}

impl Contains<IPoint> for Region {
    fn contains(&self, point: IPoint) -> bool {
        self.contains_point(point)
    }
}

impl Contains<&IRect> for Region {
    fn contains(&self, rect: &IRect) -> bool {
        self.contains_rect(rect)
    }
}

impl Contains<&Region> for Region {
    fn contains(&self, other: &Region) -> bool {
        self.contains_region(other)
    }
}

impl QuickReject<IRect> for Region {
    fn quick_reject(&self, rect: &IRect) -> bool {
        self.quick_reject_rect(rect)
    }
}

impl QuickReject<Region> for Region {
    fn quick_reject(&self, other: &Region) -> bool {
        self.quick_reject_region(other)
    }
}

/// Overload-style `intersects`, as in `skia-safe`'s `Intersects`.
pub trait Intersects<T> {
    /// Returns true if the region intersects `other`.
    fn intersects(&self, other: &T) -> bool;
}

impl Intersects<IRect> for Region {
    fn intersects(&self, rect: &IRect) -> bool {
        self.intersects_rect(rect)
    }
}

impl Intersects<Region> for Region {
    fn intersects(&self, other: &Region) -> bool {
        self.intersects_region(other)
    }
}

/// Overload-style static combination, as in `skia-safe`'s `Combine`.
pub trait Combine<A, B>: Sized {
    /// Returns `a op b` as a new region.
    fn combine(a: &A, op: Op, b: &B) -> Self;

    /// `a - b`.
    fn difference(a: &A, b: &B) -> Self {
        Self::combine(a, Op::Difference, b)
    }

    /// `a` intersected with `b`.
    fn intersect(a: &A, b: &B) -> Self {
        Self::combine(a, Op::Intersect, b)
    }

    /// `a` xor `b`.
    fn xor(a: &A, b: &B) -> Self {
        Self::combine(a, Op::XOR, b)
    }

    /// `a` unioned with `b`.
    fn union(a: &A, b: &B) -> Self {
        Self::combine(a, Op::Union, b)
    }

    /// `b - a`.
    fn reverse_difference(a: &A, b: &B) -> Self {
        Self::combine(a, Op::ReverseDifference, b)
    }

    /// `b`.
    fn replace(a: &A, b: &B) -> Self {
        Self::combine(a, Op::Replace, b)
    }
}

impl Combine<IRect, Region> for Region {
    fn combine(rect: &IRect, op: Op, region: &Region) -> Self {
        let mut r = Region::new();
        r.op_rect_region(rect, region, op);
        r
    }
}

impl Combine<Region, IRect> for Region {
    fn combine(region: &Region, op: Op, rect: &IRect) -> Self {
        let mut r = Region::new();
        r.op_region_rect(region, rect, op);
        r
    }
}

impl Combine<Region, Region> for Region {
    fn combine(a: &Region, op: Op, b: &Region) -> Self {
        let mut r = a.clone();
        r.op_region(b, op);
        r
    }
}

// Port of: src/core/SkRegion.cpp#L122-L142 (chrome/m156)
fn runs_are_a_rect(runs: &[RunType], bounds: &mut IRect) -> bool {
    let count = runs.len();
    debug_assert!(runs[0] != SENTINEL); // top
    debug_assert!(count >= RECT_REGION_RUNS);

    if count == RECT_REGION_RUNS {
        debug_assert!(runs[1] != SENTINEL); // bottom
        debug_assert_eq!(runs[2], 1);
        debug_assert!(runs[3] != SENTINEL); // left
        debug_assert!(runs[4] != SENTINEL); // right
        debug_assert_eq!(runs[5], SENTINEL);
        debug_assert_eq!(runs[6], SENTINEL);

        debug_assert!(runs[0] < runs[1]); // valid height
        debug_assert!(runs[3] < runs[4]); // valid width

        bounds.set_ltrb(runs[3], runs[0], runs[4], runs[1]);
        return true;
    }
    false
}

// Port of: src/core/SkRegion.cpp#L376-L385 (chrome/m156)
fn build_rect_runs(bounds: &IRect, runs: &mut [RunType; RECT_REGION_RUNS]) {
    runs[0] = bounds.top;
    runs[1] = bounds.bottom;
    runs[2] = 1; // 1 interval for this scanline
    runs[3] = bounds.left;
    runs[4] = bounds.right;
    runs[5] = SENTINEL;
    runs[6] = SENTINEL;
}

// Port of: src/core/SkRegion.cpp#L298-L300 (chrome/m156)
fn is_run_count_empty(count: usize) -> bool {
    count <= 2
}

// Port of: src/core/SkRegion.cpp#L425-L428 (chrome/m156)
fn scanline_next(runs: &[RunType], pos: usize) -> usize {
    // skip [B N [L R]... S]
    pos + 2 + to_usize(runs[pos + 1]) * 2 + 1
}

// Port of: src/core/SkRegion.cpp#L430-L443 (chrome/m156)
fn scanline_contains(runs: &[RunType], pos: usize, l: RunType, r: RunType) -> bool {
    let mut pos = pos + 2; // skip Bottom and IntervalCount
    loop {
        if l < runs[pos] {
            break;
        }
        if r <= runs[pos + 1] {
            return true;
        }
        pos += 2;
    }
    false
}

// Port of: src/core/SkRegion.cpp#L510-L523 (chrome/m156)
fn scanline_intersects(runs: &[RunType], pos: usize, l: RunType, r: RunType) -> bool {
    let mut pos = pos + 2; // skip Bottom and IntervalCount
    loop {
        if r <= runs[pos] {
            break;
        }
        if l < runs[pos + 1] {
            return true;
        }
        pos += 2;
    }
    false
}

/// Returns a (new) offset such that when applied (+=) to min and max, we don't overflow/underflow.
// Port of: src/core/SkRegion.cpp#L609-L617 (chrome/m156)
fn pin_offset_s32(min: i32, max: i32, offset: i32) -> i32 {
    debug_assert!(min <= max);
    let lo = i64::from(i32::MIN);
    let hi = i64::from(i32::MAX);
    let mut offset = offset;
    if i64::from(min) + i64::from(offset) < lo {
        // within i32: min >= i32::MIN
        offset = i32::try_from(lo - i64::from(min)).unwrap_or(i32::MAX);
    }
    if i64::from(max) + i64::from(offset) > hi {
        // within i32: max <= i32::MAX
        offset = i32::try_from(hi - i64::from(max)).unwrap_or(i32::MIN);
    }
    offset
}

/// A cursor over a run array: the slice and an index into it.
type Cursor<'a> = (&'a [RunType], usize);

// Port of: src/core/SkRegion.cpp#L711-L800 (chrome/m156)
struct SpanRec<'a> {
    a_runs: &'a [RunType],
    a_pos: usize,
    b_runs: &'a [RunType],
    b_pos: usize,
    a_left: i32,
    a_rite: i32,
    b_left: i32,
    b_rite: i32,
    left: i32,
    rite: i32,
    inside: i32,
}

impl<'a> SpanRec<'a> {
    fn new(a: Cursor<'a>, b: Cursor<'a>) -> Self {
        Self {
            a_runs: a.0,
            a_pos: a.1 + 2,
            b_runs: b.0,
            b_pos: b.1 + 2,
            a_left: a.0[a.1],
            a_rite: a.0[a.1 + 1],
            b_left: b.0[b.1],
            b_rite: b.0[b.1 + 1],
            left: 0,
            rite: 0,
            inside: 0,
        }
    }

    fn done(&self) -> bool {
        // (C++ asserts `fA_left <= kRunTypeSentinel`; the sentinel is `i32::MAX`)
        self.a_left == SENTINEL && self.b_left == SENTINEL
    }

    fn next(&mut self) {
        debug_assert!(self.a_left == SENTINEL || self.a_left < self.a_rite);
        debug_assert!(self.b_left == SENTINEL || self.b_left < self.b_rite);

        let inside;
        let left;
        let rite;
        let mut a_flush = false;
        let mut b_flush = false;

        let mut a_left = self.a_left;
        let mut a_rite = self.a_rite;
        let mut b_left = self.b_left;
        let mut b_rite = self.b_rite;

        match a_left.cmp(&b_left) {
            Ordering::Less => {
                inside = 1;
                left = a_left;
                if a_rite <= b_left {
                    // [...] <...>
                    rite = a_rite;
                    a_flush = true;
                } else {
                    // [...<..]...> or [...<...>...]
                    a_left = b_left;
                    rite = a_left;
                }
            }
            Ordering::Greater => {
                inside = 2;
                left = b_left;
                if b_rite <= a_left {
                    // [...] <...>
                    rite = b_rite;
                    b_flush = true;
                } else {
                    // [...<..]...> or [...<...>...]
                    b_left = a_left;
                    rite = b_left;
                }
            }
            Ordering::Equal => {
                // a_left == b_left
                inside = 3;
                left = a_left; // or b_left
                // C++ assigns `rite` in whichever of the two ifs below is true (both set the same
                // value when they are both true).
                rite = a_rite.min(b_rite);
                if a_rite <= b_rite {
                    b_left = a_rite;
                    a_flush = true;
                }
                if b_rite <= a_rite {
                    a_left = b_rite;
                    b_flush = true;
                }
            }
        }

        if a_flush {
            a_left = self.a_runs[self.a_pos];
            a_rite = self.a_runs[self.a_pos + 1];
            self.a_pos += 2;
        }
        if b_flush {
            b_left = self.b_runs[self.b_pos];
            b_rite = self.b_runs[self.b_pos + 1];
            self.b_pos += 2;
        }

        debug_assert!(left <= rite);

        // now update our state
        self.a_left = a_left;
        self.a_rite = a_rite;
        self.b_left = b_left;
        self.b_rite = b_rite;

        self.left = left;
        self.rite = rite;
        self.inside = inside;
    }
}

/// Returns the distance to the next X-sentinel.
///
/// The cursor points at the first interval `[Left, Right]`; the value before it is the number of
/// intervals for this scanline.
// Port of: src/core/SkRegion.cpp#L802-L816 (chrome/m156)
fn distance_to_sentinel(c: Cursor<'_>) -> usize {
    let (runs, pos) = c;
    let intervals = to_usize(runs[pos - 1]);
    assert_eq!(runs[pos + intervals * 2], SENTINEL);
    intervals * 2
}

/// Resizes the array to a size greater-than-or-equal-to `count` (`RunArray::resizeToAtLeast`).
// Port of: src/core/SkRegion.cpp#L77-L91 (chrome/m156)
fn resize_to_at_least(array: &mut Vec<RunType>, count: usize) {
    if count > array.len() {
        // leave at least 50% extra space for future growth (unless adding would overflow)
        let new_count = count + (count >> 1);
        let count = new_count.min(i32::MAX as usize);
        array.resize(count, 0);
    }
}

// Port of: src/core/SkRegion.cpp#L818-L854 (chrome/m156)
fn operate_on_span(
    a: Cursor<'_>,
    b: Cursor<'_>,
    array: &mut Vec<RunType>,
    dst_offset: usize,
    min: i32,
    max: i32,
) -> usize {
    // This is a worst-case for this span plus two for TWO terminating sentinels.
    resize_to_at_least(
        array,
        dst_offset + distance_to_sentinel(a) + distance_to_sentinel(b) + 2,
    );
    let mut dst = dst_offset;

    let mut first_interval = true;
    let mut rec = SpanRec::new(a, b);

    while !rec.done() {
        rec.next();

        let left = rec.left;
        let rite = rec.rite;

        // add left,rite to our dst buffer (checking for coincidence
        #[allow(clippy::cast_sign_loss)] // mirrors the (unsigned) casts: a single range test
        let in_range = (rec.inside - min) as u32 <= (max - min) as u32;
        if in_range && left < rite {
            // skip if equal
            if first_interval || array[dst - 1] < left {
                array[dst] = left;
                array[dst + 1] = rite;
                dst += 2;
                first_interval = false;
            } else {
                // update the right edge
                array[dst - 1] = rite;
            }
        }
    }
    debug_assert!(dst < array.len() - 1);
    array[dst] = SENTINEL;
    dst += 1;
    dst
}

/// `(fMin, fMax)` per `Difference`, `Intersect`, `Union`, `XOR`.
// Port of: src/core/SkRegion.cpp#L860-L874 (chrome/m156)
const OP_MIN_MAX: [(i32, i32); 4] = [
    (1, 1), // Difference
    (3, 3), // Intersection
    (1, 3), // Union
    (1, 2), // XOR
];

// Port of: src/core/SkRegion.cpp#L876-L933 (chrome/m156)
struct RgnOper {
    min: i32,
    max: i32,
    start_dst: usize,
    prev_dst: usize,
    prev_len: usize, // will never match a length from operate_on_span
    top: RunType,
}

impl RgnOper {
    fn new(top: i32, op: Op) -> Self {
        debug_assert!((op as i32) <= 3);
        let (min, max) = OP_MIN_MAX[op as usize];
        Self {
            min,
            max,
            start_dst: 0,
            prev_dst: 1,
            prev_len: 0,
            top, // just a first guess, we might update this
        }
    }

    fn add_span(&mut self, array: &mut Vec<RunType>, bottom: i32, a: Cursor<'_>, b: Cursor<'_>) {
        // skip X values and slots for the next Y+intervalCount
        let start = self.prev_dst + self.prev_len + 2;
        // start points to beginning of dst interval
        let stop = operate_on_span(a, b, array, start, self.min, self.max);
        let len = stop - start;
        debug_assert!(len >= 1 && (len & 1) == 1);
        debug_assert_eq!(SENTINEL, array[stop - 1]);

        if self.prev_len == len
            && (1 == len
                || array[self.prev_dst..self.prev_dst + len - 1] == array[start..start + len - 1])
        {
            // update Y value
            array[self.prev_dst - 2] = bottom;
        } else {
            // accept the new span
            if len == 1 && self.prev_len == 0 {
                self.top = bottom; // just update our bottom
            } else {
                array[start - 2] = bottom;
                // len is bounded by the i32 run array size
                array[start - 1] = i32::try_from(len >> 1).unwrap_or(i32::MAX);
                self.prev_dst = start;
                self.prev_len = len;
            }
        }
    }

    fn flush(&self, array: &mut [RunType]) -> usize {
        array[self.start_dst] = self.top;
        // Previously reserved enough for TWO sentinels.
        debug_assert!(array.len() > self.prev_dst + self.prev_len);
        array[self.prev_dst + self.prev_len] = SENTINEL;
        self.prev_dst - self.start_dst + self.prev_len + 1
    }

    fn is_empty(&self) -> bool {
        0 == self.prev_len
    }
}

/// Signals that `operate` exited early because the result is known to be non-empty.
// Port of: src/core/SkRegion.cpp#L935-L936 (chrome/m156)
const QUICK_EXIT_TRUE_COUNT: i64 = -1;

/// Pass in the beginning with the intervals. We back up 1 to read the interval-count. Returns the
/// beginning of the next scanline (i.e. the next Y-value).
// Port of: src/core/SkRegion.cpp#L99-L116 (chrome/m156)
fn skip_intervals(runs: &[RunType], pos: usize) -> usize {
    let intervals = to_usize(runs[pos - 1]);
    if intervals > 0 {
        debug_assert!(runs[pos] < runs[pos + 1]);
        debug_assert!(runs[pos + 1] < SENTINEL);
    } else {
        debug_assert_eq!(runs[pos], SENTINEL);
    }
    pos + intervals * 2 + 1
}

// Port of: src/core/SkRegion.cpp#L938-L1046 (chrome/m156)
fn operate(
    a_runs: &[RunType],
    b_runs: &[RunType],
    dst: &mut Vec<RunType>,
    op: Op,
    quick_exit: bool,
) -> i64 {
    let g_empty_scanline: [RunType; 4] = [
        0, // fake bottom value
        0, // zero intervals
        SENTINEL,
        // just need a 2nd value, since spanRec.init() reads 2 values, even
        // though if the first value is the sentinel, it ignores the 2nd value.
        0,
    ];
    let g_sentinel: Cursor<'_> = (&g_empty_scanline, 2);

    let mut a_pos = 0;
    let mut b_pos = 0;
    let mut a_top = a_runs[a_pos];
    a_pos += 1;
    let mut a_bot = a_runs[a_pos];
    a_pos += 1;
    let mut b_top = b_runs[b_pos];
    b_pos += 1;
    let mut b_bot = b_runs[b_pos];
    b_pos += 1;

    a_pos += 1; // skip the intervalCount;
    b_pos += 1; // skip the intervalCount;

    // Now a_pos and b_pos point to their intervals (or sentinel)

    debug_assert!(a_top != SENTINEL);
    debug_assert!(a_bot != SENTINEL);
    debug_assert!(b_top != SENTINEL);
    debug_assert!(b_bot != SENTINEL);

    let mut oper = RgnOper::new(a_top.min(b_top), op);

    let mut prev_bot = SENTINEL; // so we fail the first test

    while a_bot < SENTINEL || b_bot < SENTINEL {
        let top;
        let bot;
        let mut run0 = g_sentinel;
        let mut run1 = g_sentinel;
        let mut a_flush = false;
        let mut b_flush = false;

        match a_top.cmp(&b_top) {
            Ordering::Less => {
                top = a_top;
                run0 = (a_runs, a_pos);
                if a_bot <= b_top {
                    // [...] <...>
                    bot = a_bot;
                    a_flush = true;
                } else {
                    // [...<..]...> or [...<...>...]
                    a_top = b_top;
                    bot = a_top;
                }
            }
            Ordering::Greater => {
                top = b_top;
                run1 = (b_runs, b_pos);
                if b_bot <= a_top {
                    // [...] <...>
                    bot = b_bot;
                    b_flush = true;
                } else {
                    // [...<..]...> or [...<...>...]
                    b_top = a_top;
                    bot = b_top;
                }
            }
            Ordering::Equal => {
                // a_top == b_top
                top = a_top; // or b_top
                run0 = (a_runs, a_pos);
                run1 = (b_runs, b_pos);
                // C++ assigns `bot` in whichever of the two ifs below is true (both set the same
                // value when they are both true).
                bot = a_bot.min(b_bot);
                if a_bot <= b_bot {
                    b_top = bot;
                    a_flush = true;
                }
                if b_bot <= a_bot {
                    a_top = bot;
                    b_flush = true;
                }
            }
        }

        if top > prev_bot {
            oper.add_span(dst, top, g_sentinel, g_sentinel);
        }
        oper.add_span(dst, bot, run0, run1);

        if quick_exit && !oper.is_empty() {
            return QUICK_EXIT_TRUE_COUNT;
        }

        if a_flush {
            a_pos = skip_intervals(a_runs, a_pos);
            a_top = a_bot;
            a_bot = a_runs[a_pos];
            a_pos += 1;
            a_pos += 1; // skip uninitialized intervalCount
            if a_bot == SENTINEL {
                a_top = a_bot;
            }
        }
        if b_flush {
            b_pos = skip_intervals(b_runs, b_pos);
            b_top = b_bot;
            b_bot = b_runs[b_pos];
            b_pos += 1;
            b_pos += 1; // skip uninitialized intervalCount
            if b_bot == SENTINEL {
                b_top = b_bot;
            }
        }

        prev_bot = bot;
    }
    // the run count is bounded by the i32 run array size
    i64::try_from(oper.flush(dst)).unwrap_or(i64::MAX)
}

// Port of: src/core/SkRegion.cpp#L1065-L1075 (chrome/m156)
fn set_empty_check(result: Option<&mut Region>) -> bool {
    match result {
        Some(r) => r.set_empty(),
        None => false,
    }
}

fn set_rect_check(result: Option<&mut Region>, rect: &IRect) -> bool {
    match result {
        Some(r) => r.set_rect(rect),
        None => !rect.is_empty(),
    }
}

fn set_region_check(result: Option<&mut Region>, rgn: &Region) -> bool {
    match result {
        Some(r) => r.set_region(rgn),
        None => !rgn.is_empty(),
    }
}

/// Combines `rgna_orig` and `rgnb_orig` with `op`, storing the result in `result` if given;
/// otherwise only reports whether the result is not empty.
// Port of: src/core/SkRegion.cpp#L1077-L1175 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors the C++ names rgna/rgnb
fn oper(rgna_orig: &Region, rgnb_orig: &Region, op: Op, result: Option<&mut Region>) -> bool {
    if Op::Replace == op {
        return set_region_check(result, rgnb_orig);
    }

    // switch to using references, so we can swap them as needed
    let mut rgna = rgna_orig;
    let mut rgnb = rgnb_orig;
    let mut op = op;

    // collapse difference and reverse-difference into just difference
    if Op::ReverseDifference == op {
        std::mem::swap(&mut rgna, &mut rgnb);
        op = Op::Difference;
    }

    let a_empty = rgna.is_empty();
    let b_empty = rgnb.is_empty();
    let a_rect = rgna.is_rect();
    let b_rect = rgnb.is_rect();

    match op {
        Op::Difference => {
            if a_empty {
                return set_empty_check(result);
            }
            if b_empty || !IRect::intersects(&rgna.bounds, &rgnb.bounds) {
                return set_region_check(result, rgna);
            }
            if b_rect && rgnb.bounds.contains_no_empty_check(&rgna.bounds) {
                return set_empty_check(result);
            }
        }

        Op::Intersect => {
            if a_empty | b_empty {
                return set_empty_check(result);
            }
            let Some(bounds) = IRect::intersect(&rgna.bounds, &rgnb.bounds) else {
                return set_empty_check(result);
            };
            if a_rect & b_rect {
                return set_rect_check(result, &bounds);
            }
            if a_rect && rgna.bounds.contains(&rgnb.bounds) {
                return set_region_check(result, rgnb);
            }
            if b_rect && rgnb.bounds.contains(&rgna.bounds) {
                return set_region_check(result, rgna);
            }
        }

        Op::Union => {
            if a_empty {
                return set_region_check(result, rgnb);
            }
            if b_empty {
                return set_region_check(result, rgna);
            }
            if a_rect && rgna.bounds.contains(&rgnb.bounds) {
                return set_region_check(result, rgna);
            }
            if b_rect && rgnb.bounds.contains(&rgna.bounds) {
                return set_region_check(result, rgnb);
            }
        }

        Op::XOR => {
            if a_empty {
                return set_region_check(result, rgnb);
            }
            if b_empty {
                return set_region_check(result, rgna);
            }
        }

        Op::ReverseDifference | Op::Replace => unreachable!("handled above"),
    }

    let mut tmp_a = [0; RECT_REGION_RUNS];
    let mut tmp_b = [0; RECT_REGION_RUNS];

    let a_runs = rgna.get_runs(&mut tmp_a);
    let b_runs = rgnb.get_runs(&mut tmp_b);

    let mut array = vec![0; RUN_ARRAY_STACK_COUNT];
    let count = operate(a_runs, b_runs, &mut array, op, result.is_none());

    if let Some(result) = result {
        debug_assert!(count >= 0);
        result.set_runs(&mut array, usize::try_from(count).unwrap_or(0))
    } else {
        QUICK_EXIT_TRUE_COUNT == count || !is_run_count_empty(usize::try_from(count).unwrap_or(0))
    }
}

// Port of: src/core/SkRegion.cpp#L1217-L1232 (chrome/m156)
fn validate_run_count(y_span_count: i32, interval_count: i32, run_count: usize) -> bool {
    // return 2 + 3 * ySpanCount + 2 * intervalCount;
    if y_span_count < 1 || interval_count < 2 {
        return false;
    }
    let mut safe_math = SafeMath::new();
    let mut sum = 2;
    // 3 bytes per ySpan (stop Y, how many intervals, sentinel)
    sum = safe_math.add_int(sum, y_span_count);
    sum = safe_math.add_int(sum, y_span_count);
    sum = safe_math.add_int(sum, y_span_count);
    // 2 bytes per interval (startX, stop X)
    sum = safe_math.add_int(sum, interval_count);
    sum = safe_math.add_int(sum, interval_count);
    safe_math.ok() && usize::try_from(sum).is_ok_and(|s| s == run_count)
}

/// Validates that a memory sequence is a valid region. Tries to check all possible errors.
// Port of: src/core/SkRegion.cpp#L1234-L1322 (chrome/m156)
fn validate_run(
    runs: &[i32],
    given_bounds: &IRect,
    y_span_count: i32,
    interval_count: i32,
) -> bool {
    // Reads past the end of `runs` are guarded by asserts in C++; here they make the run invalid.
    validate_run_inner(runs, given_bounds, y_span_count, interval_count).unwrap_or(false)
}

fn validate_run_inner(
    runs: &[i32],
    given_bounds: &IRect,
    mut y_span_count: i32,
    mut interval_count: i32,
) -> Option<bool> {
    // Region Layout:
    //    Top ( Bottom Span_Interval_Count ( Left Right )* Sentinel )+ Sentinel
    let run_count = runs.len();
    if !validate_run_count(y_span_count, interval_count, run_count) {
        return Some(false);
    }
    debug_assert!(run_count >= RECT_REGION_RUNS);
    // quick safety check:
    if runs[run_count - 1] != SENTINEL || runs[run_count - 2] != SENTINEL {
        return Some(false);
    }
    let end = run_count;
    let mut pos = 0;
    let mut bounds = IRect::new(0, 0, 0, 0); // calulated bounds
    let mut rect = IRect::new(0, 0, 0, 0); // current rect
    let mut prev_was_empty = true; // If we start with an empty slice, that's corrupted data.
    rect.top = *runs.get(pos)?;
    pos += 1;
    if rect.top == SENTINEL {
        return Some(false); // no rect can contain SkRegion_kRunTypeSentinel
    }
    if rect.top != given_bounds.top {
        return Some(false); // Must not begin with empty span that does not contribute to bounds.
    }
    loop {
        y_span_count -= 1;
        if y_span_count < 0 {
            return Some(false); // too many yspans
        }
        rect.bottom = *runs.get(pos)?;
        pos += 1;
        if rect.bottom == SENTINEL {
            return Some(false);
        }
        if rect.bottom > given_bounds.bottom {
            return Some(false); // Must not end with empty span that does not contribute to bounds.
        }
        if rect.bottom <= rect.top {
            return Some(false); // y-intervals must be ordered; rects must be non-empty.
        }

        let mut x_intervals = *runs.get(pos)?;
        pos += 1;
        debug_assert!(pos < end);
        if x_intervals < 0
            || x_intervals > interval_count
            || pos + 1 + 2 * to_usize(x_intervals) > end
        {
            return Some(false);
        }
        if x_intervals == 0 {
            if prev_was_empty {
                // back to back empty spans are invalid; our serialization always has them together
                return Some(false);
            }
            prev_was_empty = true;
        } else {
            prev_was_empty = false;
        }
        interval_count -= x_intervals;
        let mut first_interval = true;
        let mut last_right = 0; // check that x-intervals are distinct and ordered.
        while x_intervals > 0 {
            x_intervals -= 1;
            rect.left = *runs.get(pos)?;
            rect.right = *runs.get(pos + 1)?;
            pos += 2;
            if rect.left == SENTINEL
                || rect.right == SENTINEL
                || rect.left >= rect.right // check non-empty rect
                || (!first_interval && rect.left <= last_right)
            {
                return Some(false);
            }
            last_right = rect.right;
            first_interval = false;
            bounds = IRect::join(&bounds, &rect);
        }
        let x_sentinel = *runs.get(pos)?;
        pos += 1;
        if x_sentinel != SENTINEL {
            return Some(false); // required check sentinel.
        }
        rect.top = rect.bottom;
        debug_assert!(pos < end);
        if *runs.get(pos)? == SENTINEL {
            break;
        }
    }
    pos += 1;
    if y_span_count != 0 || interval_count != 0 || *given_bounds != bounds {
        return Some(false);
    }
    debug_assert_eq!(pos, end); // if ySpanCount && intervalCount are right, must be correct length.
    Some(true)
}

/// Goes through the region one rectangle at a time. For each "strip" of one or more contiguous Y
/// values (scanlines) in ascending order, the iterator returns each rectangle in that strip (from
/// left to right) before advancing to the next strip (which may or may not have a gap).
// Port of: include/core/SkRegion.h#L552-L621 (chrome/m156)
#[doc(alias = "SkRegion::Iterator")]
#[derive(Clone)]
pub struct Iterator<'a> {
    rgn: Option<&'a Region>,
    /// The runs of a complex region; empty otherwise.
    runs: &'a [RunType],
    /// `fRuns`: `None` for the rect case (and before `reset`).
    pos: Option<usize>,
    rect: IRect,
    done: bool,
}

impl fmt::Debug for Iterator<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Iterator")
            .field("is_done", &self.is_done())
            .field("rect", self.rect())
            .finish()
    }
}

impl<'a> Iterator<'a> {
    /// Initializes an iterator with an empty region. [`Self::is_done`] on the iterator returns
    /// true. Call [`Self::reset`] to initialize the iterator at a later time.
    // Port of: include/core/SkRegion.h#L562 (chrome/m156)
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            rgn: None,
            runs: &[],
            pos: None,
            rect: IRect::new(0, 0, 0, 0),
            done: true,
        }
    }

    /// Sets the iterator to return elements of the region's rectangle array.
    // Port of: src/core/SkRegion.cpp#L1404-L1406 (chrome/m156)
    #[must_use]
    pub fn new(region: &'a Region) -> Iterator<'a> {
        let mut it = Self::new_empty();
        it.reset_in_place(region);
        it
    }

    /// Moves the iterator to the start of the region. Returns true if the region was set;
    /// otherwise, returns false.
    // Port of: src/core/SkRegion.cpp#L1408-L1414 (chrome/m156)
    pub fn rewind(&mut self) -> bool {
        if let Some(rgn) = self.rgn {
            self.reset_in_place(rgn);
            return true;
        }
        false
    }

    /// Resets the iterator, using the new region.
    // Port of: src/core/SkRegion.cpp#L1416-L1432 (chrome/m156)
    #[must_use]
    pub fn reset(self, region: &Region) -> Iterator<'_> {
        // The iterator may borrow a different region afterwards, so a new value is built; like
        // C++, the previous `fRect` is kept when the new region is empty.
        let mut it = Iterator {
            rgn: None,
            runs: &[],
            pos: None,
            rect: self.rect,
            done: true,
        };
        it.reset_in_place(region);
        it
    }

    fn reset_in_place(&mut self, rgn: &'a Region) {
        self.rgn = Some(rgn);
        if rgn.is_empty() {
            self.done = true;
        } else {
            self.done = false;
            if let Runs::Complex(head) = &rgn.runs {
                self.runs = &head.runs;
                let r = &head.runs;
                self.rect.set_ltrb(r[3], r[0], r[4], r[1]);
                self.pos = Some(5);
                // Now pos points to the 2nd interval (or x-sentinel)
            } else {
                self.rect = rgn.bounds;
                self.runs = &[];
                self.pos = None;
            }
        }
    }

    /// Returns true if the iterator is pointing past the final rectangle in the region.
    #[doc(alias = "done")]
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Advances the iterator to the next rectangle in the region if it is not done. This moves to
    /// the next rectangle to the right within the current horizontal strip. If the end of the
    /// strip is reached, it automatically advances to the first rectangle in the next strip,
    /// skipping any vertical gaps.
    // Port of: src/core/SkRegion.cpp#L1434-L1472 (chrome/m156)
    #[allow(clippy::should_implement_trait)] // mirrors SkRegion::Iterator::next()
    pub fn next(&mut self) {
        if self.done {
            return;
        }

        let Some(mut pos) = self.pos else {
            // rect case
            self.done = true;
            return;
        };

        let runs = self.runs;

        if runs[pos] < SENTINEL {
            // valid X value
            self.rect.left = runs[pos];
            self.rect.right = runs[pos + 1];
            pos += 2;
        } else {
            // we're at the end of a line
            pos += 1;
            if runs[pos] < SENTINEL {
                // valid Y value
                let intervals = runs[pos + 1];
                if 0 == intervals {
                    // empty line
                    self.rect.top = runs[pos];
                    pos += 3;
                } else {
                    self.rect.top = self.rect.bottom;
                }

                self.rect.bottom = runs[pos];
                debug_assert!(runs[pos + 2] != SENTINEL);
                debug_assert!(runs[pos + 3] != SENTINEL);
                self.rect.left = runs[pos + 2];
                self.rect.right = runs[pos + 3];
                pos += 4;
            } else {
                // end of rgn
                self.done = true;
            }
        }
        self.pos = Some(pos);
    }

    /// Returns the rectangle element in the region. Does not return predictable results if the
    /// region is empty.
    #[must_use]
    pub fn rect(&self) -> &IRect {
        &self.rect
    }

    /// Returns the region if set; otherwise, returns `None`.
    #[must_use]
    pub fn rgn(&self) -> Option<&Region> {
        self.rgn
    }
}

impl iter::Iterator for Iterator<'_> {
    type Item = IRect;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        let r = *self.rect();
        Iterator::next(self);
        Some(r)
    }
}

/// Returns the sequence of rectangles, sorted along the y-axis, then the x-axis, that make up the
/// region intersected with the specified clip rectangle.
// Port of: include/core/SkRegion.h#L626-L664 (chrome/m156)
#[doc(alias = "SkRegion::Cliperator")]
#[derive(Clone)]
pub struct Cliperator<'a> {
    iter: Iterator<'a>,
    clip: IRect,
    rect: IRect,
    done: bool,
}

impl fmt::Debug for Cliperator<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cliperator")
            .field("is_done", &self.is_done())
            .field("rect", self.rect())
            .finish()
    }
}

impl<'a> Cliperator<'a> {
    /// Sets the cliperator to return elements of the region's rectangle array within `clip`.
    // Port of: src/core/SkRegion.cpp#L1474-L1488 (chrome/m156)
    #[must_use]
    pub fn new(region: &'a Region, clip: impl AsRef<IRect>) -> Cliperator<'a> {
        let clip = *clip.as_ref();
        let mut c = Cliperator {
            iter: Iterator::new(region),
            clip,
            rect: IRect::new(0, 0, 0, 0),
            done: true,
        };

        while !c.iter.is_done() {
            let r = *c.iter.rect();
            if r.top >= clip.bottom {
                break;
            }
            if let Some(sect) = IRect::intersect(&clip, &r) {
                c.rect = sect;
                c.done = false;
                break;
            }
            c.iter.next();
        }
        c
    }

    /// Returns true if the cliperator is pointing past the final rectangle in the region.
    #[doc(alias = "done")]
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Advances the iterator to the next rectangle in the region contained by the clip.
    // Port of: src/core/SkRegion.cpp#L1490-L1509 (chrome/m156)
    #[allow(clippy::should_implement_trait)] // mirrors SkRegion::Cliperator::next()
    pub fn next(&mut self) {
        if self.done {
            return;
        }

        self.done = true;
        self.iter.next();
        while !self.iter.is_done() {
            let r = *self.iter.rect();
            if r.top >= self.clip.bottom {
                break;
            }
            if let Some(sect) = IRect::intersect(&self.clip, &r) {
                self.rect = sect;
                self.done = false;
                break;
            }
            self.iter.next();
        }
    }

    /// Returns the rectangle element in the region, intersected with the clip passed to
    /// [`Self::new`]. Does not return predictable results if the region is empty.
    #[must_use]
    pub fn rect(&self) -> &IRect {
        &self.rect
    }
}

impl iter::Iterator for Cliperator<'_> {
    type Item = IRect;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        let rect = *self.rect();
        Cliperator::next(self);
        Some(rect)
    }
}

/// Returns the line segment ends within the region that intersect a horizontal line.
// Port of: include/core/SkRegion.h#L669-L694 (chrome/m156)
#[doc(alias = "SkRegion::Spanerator")]
#[derive(Clone)]
pub struct Spanerator<'a> {
    runs: &'a [RunType],
    /// `fRuns`: `None` for the rect case.
    pos: Option<usize>,
    left: i32,
    right: i32,
    done: bool,
}

impl fmt::Debug for Spanerator<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spanerator").finish()
    }
}

impl<'a> Spanerator<'a> {
    /// Sets the spanerator to return line segments in the region on the scan line.
    // Port of: src/core/SkRegion.cpp#L1513-L1555 (chrome/m156)
    #[must_use]
    pub fn new(region: &'a Region, y: i32, left: i32, right: i32) -> Spanerator<'a> {
        region.validate();

        let r = region.bounds();

        let mut s = Spanerator {
            runs: &[],
            pos: None,
            left: 0,
            right: 0,
            done: true,
        };
        if !region.is_empty() && y >= r.top && y < r.bottom && right > r.left && left < r.right {
            if let Runs::Complex(head) = &region.runs {
                let mut pos = head.find_scanline(y);
                pos += 2; // skip Bottom and IntervalCount
                loop {
                    // runs[0..1] is to the right of the span, so we're done
                    if head.runs[pos] >= right {
                        break;
                    }
                    // runs[0..1] is to the left of the span, so continue
                    if head.runs[pos + 1] <= left {
                        pos += 2;
                        continue;
                    }
                    // runs[0..1] intersects the span
                    s.runs = &head.runs;
                    s.pos = Some(pos);
                    s.left = left;
                    s.right = right;
                    s.done = false;
                    break;
                }
            } else {
                s.left = left.max(r.left);
                s.right = right.min(r.right);
                s.pos = None; // means we're a rect, not a rgn
                s.done = false;
            }
        }
        s
    }
}

impl iter::Iterator for Spanerator<'_> {
    type Item = (i32, i32);

    /// Advances the iterator to the next span intersecting the region within the line segment
    /// provided in the constructor. Returns the `(left, right)` span if an interval was found.
    // Port of: src/core/SkRegion.cpp#L1557-L1590 (chrome/m156)
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let Some(pos) = self.pos else {
            // we're a rect
            self.done = true; // ok, now we're done
            return Some((self.left, self.right)); // this interval is legal
        };

        let runs = self.runs;

        if runs[pos] >= self.right {
            self.done = true;
            return None;
        }

        debug_assert!(runs[pos + 1] > self.left);

        let left = self.left.max(runs[pos]);
        let right = self.right.min(runs[pos + 1]);
        self.pos = Some(pos + 2);
        Some((left, right))
    }
}

/// Private helpers of `SkRegion` (`SkRegionPriv.h`), used by Skia's own code and tests.
#[doc(hidden)]
pub mod region_priv {
    use super::{Region, RunHead, Runs, SENTINEL};
    use crate::rect::IRect;
    use std::sync::Arc;

    /// `SkRegion::kRectRegionRuns`: the number of run values describing one rectangle.
    pub const RECT_REGION_RUNS: usize = super::RECT_REGION_RUNS;

    /// `SkRegion::RunType`.
    pub type RunType = i32;

    /// `SkRegion::count_runtype_values`: returns `(max transitions, top, bottom)`. The region
    /// must not be empty.
    // Port of: src/core/SkRegion.cpp#L284-L296 (chrome/m156)
    #[must_use]
    pub fn count_runtype_values(rgn: &Region) -> (i32, i32, i32) {
        let max_t = if rgn.is_rect() {
            2
        } else {
            debug_assert!(rgn.is_complex());
            match &rgn.runs {
                Runs::Complex(head) => head.interval_count * 2,
                _ => 0,
            }
        };
        (max_t, rgn.bounds.top, rgn.bounds.bottom)
    }

    /// Makes a complex region out of `runs` (`tmp.fRunHead = RunHead::Alloc(count);
    /// copy; tmp.fRunHead->computeRunBounds(&tmp.fBounds)` in `SkRegion::setPath`). The runs must
    /// be well formed and describe more than a rectangle.
    // Port of: src/core/SkRegion_path.cpp#L384-L388 (chrome/m156)
    #[must_use]
    pub fn make_complex(runs: Vec<RunType>) -> Region {
        let mut head = RunHead {
            y_span_count: 0,
            interval_count: 0,
            runs,
        };
        let bounds = head.compute_run_bounds();
        Region {
            bounds,
            runs: Runs::Complex(Arc::new(head)),
        }
    }

    /// `SkRegionPriv::kRunTypeSentinel` / `SkRegion_kRunTypeSentinel`.
    pub const RUN_TYPE_SENTINEL: i32 = SENTINEL;

    /// `SkRegionValueIsSentinel`.
    #[must_use]
    pub fn region_value_is_sentinel(value: i32) -> bool {
        value == RUN_TYPE_SENTINEL
    }

    /// Calls `visitor` with each span, in Y -> X ascending order (`SkRegionPriv::VisitSpans`).
    ///
    /// A rect is passed, but the Y->X ordering is still ensured, so often the height of the rect
    /// may be 1. It is never empty.
    // Port of: src/core/SkRegion.cpp#L1602-L1633 (chrome/m156)
    pub fn visit_spans(rgn: &Region, visitor: &mut impl FnMut(&IRect)) {
        match &rgn.runs {
            Runs::Empty => {}
            Runs::Rect => visitor(rgn.bounds()),
            Runs::Complex(head) => {
                let p = &head.runs;
                let mut pos = 0;
                let mut top = p[pos];
                pos += 1;
                let mut bot = p[pos];
                pos += 1;
                loop {
                    let pair_count = p[pos];
                    pos += 1;
                    if pair_count == 1 {
                        visitor(&IRect::new(p[pos], top, p[pos + 1], bot));
                        pos += 2;
                    } else if pair_count > 1 {
                        // we have to loop repeated in Y, sending each interval in Y -> X order
                        for y in top..bot {
                            visit_pairs(pair_count, y, &p[pos..], visitor);
                        }
                        pos += super::to_usize(pair_count) * 2; // pair_count > 1
                    }
                    debug_assert!(region_value_is_sentinel(p[pos]));
                    pos += 1; // skip sentinel

                    // read next bottom or sentinel
                    top = bot;
                    bot = p[pos];
                    pos += 1;
                    if region_value_is_sentinel(bot) {
                        break;
                    }
                }
            }
        }
    }

    // Port of: src/core/SkRegion.cpp#L1594-L1600 (chrome/m156)
    fn visit_pairs(pair_count: i32, y: i32, pairs: &[i32], visitor: &mut impl FnMut(&IRect)) {
        // pair_count > 1
        for i in 0..super::to_usize(pair_count) {
            visitor(&IRect::new(pairs[i * 2], y, pairs[i * 2 + 1], y + 1));
        }
    }

    /// `SkRegionPriv::Validate`: asserts (debug builds) that the region's runs are well formed.
    pub fn validate(rgn: &Region) {
        debug_assert!(rgn.is_valid());
    }
}

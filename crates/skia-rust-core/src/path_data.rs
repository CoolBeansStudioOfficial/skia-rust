// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPathData.h, src/core/SkPathData.cpp

//! Immutable, shared path geometry (`SkPathData.h`).
//!
//! All of the public factories check for valid input: a valid verb sequence, the corresponding
//! number of points and conic weights, and finite point and conic values. If any of these checks
//! fail, `None` is returned.
//!
//! A valid sequence of verbs (and corresponding points/conics) is any number of contours, each
//! beginning with a single Move verb, followed by any number of segments (lines, quads, conics,
//! cubics), followed by 0 or 1 Close verb. The last contour may end with a single Move verb.
//!
//! | Verb  | points | conic weights |
//! |-------|--------|---------------|
//! | Move  | 1      | 0             |
//! | Line  | 1      | 0             |
//! | Quad  | 2      | 0             |
//! | Conic | 2      | 1             |
//! | Cubic | 3      | 0             |
//! | Close | 0      | 0             |

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use crate::floating_point::is_finite;
use crate::matrix::Matrix;
use crate::path_builder::{PathBuilder, Reserve};
use crate::path_enums::{PathConvexity, ResolveConvexity, direction_to_convexity};
use crate::path_priv;
use crate::path_raw::PathRaw;
use crate::path_raw_shapes;
use crate::path_ref::{PathIsAData, PathIsAType, PathOvalInfo, PathRRectInfo, PathRectInfo};
use crate::path_types::{PathDirection, PathFillType, PathSegmentMask, PathVerb};
use crate::point::{Point, Vector};
use crate::rect::Rect;
use crate::rrect::RRect;
use crate::scalar::scalar;

/// Immutable container for path geometry: points, verbs, conic weights, bounds and segment mask.
///
/// Shared as `Arc<PathData>` (the C++ `sk_sp<SkPathData>`).
// Port of: src/core/SkPathData.h#L63-L243 (chrome/m156)
#[doc(alias = "SkPathData")]
#[derive(Debug)]
pub struct PathData {
    points: Box<[Point]>,
    conics: Box<[scalar]>,
    verbs: Box<[PathVerb]>,
    bounds: Rect,
    unique_id: u64, // never 0
    // Convexity can be slow to compute, and (in theory) it can't always survive a matrix
    // transform (due to numeric instability). Therefore we will lazily compute it as
    // requested. Since we are technically always immutable, we have to store this field
    // in an atomic.
    convexity: AtomicU8,
    segment_mask: u8,
    ty: PathIsAType,
    is_a: PathIsAData,
}

// Port of: src/core/SkPathData.cpp#L35-L44 (chrome/m156)
fn next_pathdata_unique_id() -> u64 {
    const HIGH_BITS_TO_MAKE_ROOM_FOR_FILL_TYPE: u32 = 2;
    const MAX_ID: u64 = u64::MAX >> HIGH_BITS_TO_MAKE_ROOM_FOR_FILL_TYPE;
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    assert!(id <= MAX_ID, "SkPathData unique ids exhausted");
    id
}

const PTS_PER_VERB: [usize; 6] = [1, 1, 2, 2, 3, 0]; // move, line, quad, conic, cubic, close

// Port of: src/core/SkPathData.cpp#L80-L82 (chrome/m156)
fn valid_conic_weight(w: f32) -> bool {
    w >= 0.0 && is_finite(w)
}

// Port of: src/core/SkPathData.cpp#L84-L137 (chrome/m156)
fn valid_path_data(pts: &[Point], vbs: &[PathVerb], conics: &[scalar]) -> bool {
    if vbs.is_empty() {
        return pts.is_empty() && conics.is_empty();
    }

    // We must begin with a Move (unless we're empty)
    if vbs[0] != PathVerb::Move {
        return false;
    }
    let mut prev = PathVerb::Move;
    let mut point_count = 1;
    let mut conic_count = 0;

    // Check that we have a valid sequence.
    for &curr in &vbs[1..] {
        // (Rust verbs are always in range, so the kLast_Verb check is implicit.)

        // Previous verb             Valid next verb
        // -----------------------------------------
        // Move                  --> not Move
        // Line/Quad/Conic/Cubic --> *
        // Close                 --> Move
        //
        if prev == PathVerb::Move && curr == PathVerb::Move {
            return false;
        }
        if prev == PathVerb::Close && curr != PathVerb::Move {
            return false;
        }

        point_count += PTS_PER_VERB[curr as usize];
        conic_count += usize::from(curr == PathVerb::Conic);

        prev = curr;
    }

    if pts.len() != point_count || conics.len() != conic_count {
        return false;
    }

    for &w in conics {
        if !valid_conic_weight(w) {
            return false;
        }
    }

    true
}

impl PathData {
    // This just sets up the storage; the remaining fields are set by finish_init().
    // Port of: src/core/SkPathData.cpp#L147-L182 (chrome/m156)
    fn alloc(points: Box<[Point]>, verbs: Box<[PathVerb]>, conics: Box<[scalar]>) -> Self {
        debug_assert!(
            (points.is_empty() && verbs.is_empty() && conics.is_empty())
                || (!points.is_empty() && !verbs.is_empty())
        );
        Self {
            points,
            conics,
            verbs,
            bounds: Rect::new_empty(),
            unique_id: next_pathdata_unique_id(),
            convexity: AtomicU8::new(PathConvexity::Unknown as u8),
            segment_mask: 0,
            ty: PathIsAType::General,
            is_a: PathIsAData::default(),
        }
    }

    // internal finisher when building a PathData.
    // If the optional value is not present, it will be computed (else checked in debug mode).
    //
    // In particular, if bounds is not provided, it will be computed, and if it proves
    // to be non-finite, false will be returned.
    // Port of: src/core/SkPathData.cpp#L217-L248 (chrome/m156)
    fn finish_init(&mut self, bounds: Option<Rect>, segment_mask: Option<u8>) -> bool {
        debug_assert!(valid_path_data(&self.points, &self.verbs, &self.conics));

        if self.points.is_empty() {
            self.bounds = Rect::new_empty();
            self.segment_mask = 0;
            return true;
        }

        self.segment_mask =
            segment_mask.unwrap_or_else(|| path_priv::compute_segment_mask(&self.verbs));

        if let Some(bounds) = bounds {
            self.bounds = bounds.sorted();
        } else if let Some(r) = path_priv::trimmed_bounds(&self.points, &self.verbs) {
            self.bounds = r;
        } else {
            // report_pathdata_make_failure("non-finite bounds");
            return false;
        }

        debug_assert!(self.bounds.is_sorted());
        true
    }

    fn finish(mut self, bounds: Option<Rect>, segment_mask: Option<u8>) -> Option<Self> {
        if self.finish_init(bounds, segment_mask) {
            Some(self)
        } else {
            None
        }
    }

    /// An empty path data. Since this is immutable, it returns the same object each time.
    // Port of: src/core/SkPathData.cpp#L29-L32, #L323-L325 (chrome/m156)
    #[doc(alias = "Empty")]
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // panics only if an internal invariant is broken
    pub fn empty() -> Arc<PathData> {
        static EMPTY: OnceLock<Arc<PathData>> = OnceLock::new();
        EMPTY
            .get_or_init(|| {
                Arc::new(Self::make_no_check(&[], &[], &[], None, None).expect("empty is valid"))
            })
            .clone()
    }

    /// A fresh empty path data, distinct from [`PathData::empty`] (for `SkPath`'s error singleton).
    pub(crate) fn new_empty_unshared() -> PathData {
        Self::make_no_check(&[], &[], &[], None, None).expect("empty is valid")
    }

    /// A path data with a copy of these buffers, or `None` if they are illegal (non-finite, or a
    /// non-sensical verb sequence).
    // Port of: src/core/SkPathData.cpp#L517-L527 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(pts: &[Point], vbs: &[PathVerb], conics: &[scalar]) -> Option<Arc<PathData>> {
        if !valid_path_data(pts, vbs, conics) {
            // report_pathdata_make_failure("invalid path data");
            return None;
        }

        // MakeNoCheck *does* compute/check bounds if we don't pass them in
        Self::make_no_check(pts, vbs, conics, None, None).map(Arc::new)
    }

    // Port of: src/core/SkPathData.cpp#L304-L317 (chrome/m156)
    pub(crate) fn make_no_check(
        pts: &[Point],
        vbs: &[PathVerb],
        conics: &[scalar],
        bounds: Option<Rect>,
        segment_mask: Option<u8>,
    ) -> Option<PathData> {
        debug_assert!(valid_path_data(pts, vbs, conics));
        Self::alloc(pts.into(), vbs.into(), conics.into()).finish(bounds, segment_mask)
    }

    /// Attempts to transform `src` by the matrix. On success returns a new path data.
    // Port of: src/core/SkPathData.cpp#L250-L287 (chrome/m156)
    #[doc(alias = "MakeTransform")]
    #[must_use]
    pub fn make_transform_raw(src: &PathRaw<'_>, mx: &Matrix) -> Option<Arc<PathData>> {
        if src.is_empty() {
            return Some(PathData::empty());
        }

        if mx.has_perspective() {
            let mut builder = PathBuilder::new();
            builder.add_raw(src, Reserve::Exact).transform(mx);
            return builder.detach_data();
        }

        // Allocate our result, so we can map the new points directly into it
        let mut points: Box<[Point]> = vec![Point::default(); src.points.len()].into();
        mx.map_points(&mut points, src.points);
        let mut result = Self::alloc(points, src.verbs.into(), src.conics.into());

        let mut transformed_bounds = None;
        if mx.rect_stays_rect() {
            // safe us from having to compute our transformed bounds in finishInit()
            let r = mx.map_rect(src.bounds).0;
            if !r.is_finite() {
                // report_pathdata_make_failure("transform created non-finite bounds");
                return None;
            }
            transformed_bounds = Some(r);
        }

        if !result.finish_init(transformed_bounds, Some(src.segment_mask)) {
            return None;
        }

        result.set_convexity(path_priv::transform_convexity(
            mx,
            src.points,
            src.convexity,
        ));

        Some(Arc::new(result))
    }

    /// Attempts to transform this path data by the matrix. This may have different verbs / number
    /// of points if the matrix has perspective. Returns this same object if the matrix has no
    /// effect (identity), and `None` if the result has non-finite coordinates.
    // Port of: src/core/SkPathData.cpp#L289-L317 (chrome/m156)
    #[doc(alias = "makeTransform")]
    #[must_use]
    pub fn make_transform(self: &Arc<Self>, mx: &Matrix) -> Option<Arc<PathData>> {
        if mx.is_identity() {
            return Some(Arc::clone(self));
        }

        // not important for transform, just need a value
        let ft = PathFillType::DEFAULT;

        let mut result = Self::make_transform_raw(&self.raw(ft, ResolveConvexity::No), mx)?;

        if self.ty == PathIsAType::General {
            return Some(result);
        }

        // See if we can maintain our IsA status ...
        let can_maintain_is_a = mx.rect_stays_rect()
            && path_priv::is_axis_aligned(&self.points)
            && !result.bounds().is_empty();
        if !can_maintain_is_a {
            return Some(result);
        }

        let (dir, start) = path_priv::transform_dir_and_start(
            mx,
            self.ty == PathIsAType::RRect,
            self.is_a.direction,
            u32::from(self.is_a.start_index),
        );
        Self::setup_is_a_arc(&mut result, self.ty, dir, start);
        Some(result)
    }

    /// Offsets this path data by `v`.
    // Port of: src/core/SkPathData.cpp#L319-L321 (chrome/m156)
    #[doc(alias = "makeOffset")]
    #[must_use]
    pub fn make_offset(self: &Arc<Self>, v: Vector) -> Option<Arc<PathData>> {
        self.make_transform(&Matrix::translate(v))
    }

    // If we know we're a special shape, call this after the normal initialization
    // Port of: src/core/SkPathData.cpp#L327-L339 (chrome/m156)
    fn setup_is_a(&mut self, ty: PathIsAType, dir: PathDirection, index: u32) {
        self.set_convexity(direction_to_convexity(dir));

        debug_assert!(ty == PathIsAType::Oval || ty == PathIsAType::RRect);
        self.ty = ty;

        debug_assert!(
            (ty == PathIsAType::Oval && index < 4) || (ty == PathIsAType::RRect && index < 8)
        );

        self.is_a.direction = dir;
        #[allow(clippy::cast_possible_truncation)] // index < 8 (SkTo<uint8_t>)
        let start = index as u8;
        self.is_a.start_index = start;
    }

    /// `setupIsA` on a freshly made (unshared) `Arc`.
    pub(crate) fn setup_is_a_arc(
        data: &mut Arc<PathData>,
        ty: PathIsAType,
        dir: PathDirection,
        index: u32,
    ) {
        if let Some(d) = Arc::get_mut(data) {
            d.setup_is_a(ty, dir, index);
        } else {
            // C++ would write into a shared object here; that only happens for the empty
            // singleton, which callers exclude by checking for empty bounds.
            debug_assert!(false, "setupIsA on shared SkPathData");
        }
    }

    /// A closed rectangle, or `None` if `r` is not finite.
    // Port of: src/core/SkPathData.cpp#L341-L347 (chrome/m156)
    #[doc(alias = "Rect")]
    #[must_use]
    pub fn rect(r: &Rect, dir: PathDirection, index: u32) -> Option<Arc<PathData>> {
        if !r.is_finite() {
            return None;
        }
        let raw = path_raw_shapes::Rect::new(r, dir, index);
        let raw = raw.raw();
        Self::make_no_check(
            raw.points,
            raw.verbs,
            raw.conics,
            Some(raw.bounds),
            Some(raw.segment_mask),
        )
        .map(Arc::new)
    }

    /// `SkPathData::Rect(r)`: clockwise, start index 0.
    #[must_use]
    pub fn rect_default(r: &Rect) -> Option<Arc<PathData>> {
        Self::rect(r, PathDirection::DEFAULT, 0)
    }

    /// An oval, or `None` if `r` is not finite.
    // Port of: src/core/SkPathData.cpp#L349-L358 (chrome/m156)
    #[doc(alias = "Oval")]
    #[must_use]
    pub fn oval(r: &Rect, dir: PathDirection, index: u32) -> Option<Arc<PathData>> {
        if !r.is_finite() {
            return None;
        }
        let raw = path_raw_shapes::Oval::new(r, dir, index);
        let raw = raw.raw();
        let mut path = Self::make_no_check(
            raw.points,
            raw.verbs,
            raw.conics,
            Some(raw.bounds),
            Some(raw.segment_mask),
        )?;

        path.setup_is_a(PathIsAType::Oval, dir, index);
        Some(Arc::new(path))
    }

    /// `SkPathData::Oval(r)`: clockwise, start index 1.
    #[must_use]
    pub fn oval_default(r: &Rect) -> Option<Arc<PathData>> {
        Self::oval(r, PathDirection::DEFAULT, 1)
    }

    /// A round rectangle, or `None` if `r` is not valid or not finite.
    // Port of: src/core/SkPathData.cpp#L360-L371 (chrome/m156)
    #[doc(alias = "RRect")]
    #[must_use]
    pub fn rrect(r: &RRect, dir: PathDirection, index: u32) -> Option<Arc<PathData>> {
        if !r.is_valid() {
            return None;
        }
        let raw = path_raw_shapes::RRect::new(r, dir, index);
        // we use Make, not MakeNoCheck, to confirm all points an conics are finite
        let mut path = Self::make(raw.points(), raw.raw().verbs, raw.raw().conics)?;
        Self::setup_is_a_arc(&mut path, PathIsAType::RRect, dir, index);
        Some(path)
    }

    /// `SkPathData::RRect(rrect, dir = kCW)`: start index 6 (CW) or 7 (CCW).
    // Port of: src/core/SkPathData.h#L98-L101 (chrome/m156)
    #[must_use]
    pub fn rrect_default(r: &RRect, dir: PathDirection) -> Option<Arc<PathData>> {
        Self::rrect(r, dir, if dir == PathDirection::CW { 6 } else { 7 })
    }

    /// A polygon through `pts`, closed or not.
    // Port of: src/core/SkPathData.cpp#L373-L395 (chrome/m156)
    #[doc(alias = "Polygon")]
    #[must_use]
    pub fn polygon(pts: &[Point], is_closed: bool) -> Option<Arc<PathData>> {
        if pts.is_empty() || (pts.len() == 1 && !is_closed) {
            return Some(Self::empty());
        }

        let nverbs = pts.len() + usize::from(is_closed); // +1 for the kClose verb
        let mut verbs = vec![PathVerb::Line; nverbs];
        verbs[0] = PathVerb::Move;
        if is_closed {
            verbs[nverbs - 1] = PathVerb::Close;
        }
        let path = Self::alloc(pts.into(), verbs.into(), Box::new([]));

        #[allow(clippy::cast_possible_truncation)] // the mask fits in 4 bits
        let line = PathSegmentMask::LINE.bits() as u8;
        path.finish(None, Some(line)).map(Arc::new)
    }

    /// A single line.
    // Port of: src/core/SkPathData.h#L103-L105 (chrome/m156)
    #[doc(alias = "Line")]
    #[must_use]
    pub fn line(a: Point, b: Point) -> Option<Arc<PathData>> {
        Self::polygon(&[a, b], false)
    }

    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    #[must_use]
    pub fn verbs(&self) -> &[PathVerb] {
        &self.verbs
    }

    #[must_use]
    pub fn conics(&self) -> &[scalar] {
        &self.conics
    }

    #[must_use]
    pub fn bounds(&self) -> &Rect {
        &self.bounds
    }

    #[doc(alias = "segmentMask")]
    #[must_use]
    pub fn segment_mask(&self) -> u8 {
        self.segment_mask
    }

    /// Never zero; the upper 2 bits are always zero (to store a fill type).
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u64 {
        self.unique_id
    }

    /// True if the path data has no points or verbs.
    #[doc(alias = "empty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub(crate) fn ty(&self) -> PathIsAType {
        self.ty
    }

    pub(crate) fn is_a(&self) -> PathIsAData {
        self.is_a
    }

    // Port of: src/core/SkPathData.cpp#L399-L401 (chrome/m156)
    pub(crate) fn get_convexity_or_unknown(&self) -> PathConvexity {
        PathConvexity::from_u8(self.convexity.load(Ordering::Relaxed))
    }

    // Port of: src/core/SkPathData.cpp#L403-L410 (chrome/m156)
    pub(crate) fn get_resolved_convexity(&self) -> PathConvexity {
        let mut convexity = self.get_convexity_or_unknown();
        if convexity == PathConvexity::Unknown {
            convexity = path_priv::compute_convexity(&self.points, &self.verbs, &self.conics);
            self.set_convexity(convexity);
        }
        convexity
    }

    // const -- but convexity is mutable
    // Port of: src/core/SkPathData.cpp#L412-L414 (chrome/m156)
    pub(crate) fn set_convexity(&self, convexity: PathConvexity) {
        self.convexity.store(convexity as u8, Ordering::Relaxed);
    }

    /// True if the path data is convex; computes (and caches) the convexity if needed.
    // Port of: src/core/SkPathData.cpp#L416-L418 (chrome/m156)
    #[doc(alias = "isConvex")]
    #[must_use]
    pub fn is_convex(&self) -> bool {
        self.get_resolved_convexity().is_convex()
    }

    /// The bounds of the curves (not just the control points).
    // Port of: src/core/SkPathData.cpp#L420-L422 (chrome/m156)
    #[doc(alias = "computeTightBounds")]
    #[must_use]
    pub fn compute_tight_bounds(&self) -> Rect {
        path_priv::compute_tight_bounds(&self.points, &self.verbs, &self.conics)
    }

    /// A raw view of this data with the given fill type.
    // Port of: src/core/SkPathData.cpp#L424-L436 (chrome/m156)
    #[must_use]
    pub fn raw(&self, ft: PathFillType, rc: ResolveConvexity) -> PathRaw<'_> {
        PathRaw {
            points: &self.points,
            verbs: &self.verbs,
            conics: &self.conics,
            bounds: self.bounds,
            fill_type: ft,
            convexity: if rc == ResolveConvexity::Yes {
                self.get_resolved_convexity()
            } else {
                self.get_convexity_or_unknown()
            },
            segment_mask: self.segment_mask,
        }
    }

    /// The two points if the data is a single line.
    // Port of: src/core/SkPathData.cpp#L438-L447 (chrome/m156)
    #[doc(alias = "asLine")]
    #[must_use]
    pub fn as_line(&self) -> Option<[Point; 2]> {
        if self.points.len() == 2
            && self.verbs.len() == 2
            && self.conics.is_empty()
            && self.verbs[1] == PathVerb::Line
        {
            debug_assert_eq!(self.verbs[0], PathVerb::Move);
            return Some([self.points[0], self.points[1]]);
        }
        None
    }

    /// The rectangle, direction and start index if the data is a rectangle.
    // Port of: src/core/SkPathData.cpp#L449-L459 (chrome/m156)
    #[doc(alias = "asRect")]
    #[must_use]
    pub fn as_rect(&self) -> Option<PathRectInfo> {
        let rc = path_priv::is_rect_contour(
            &self.points,
            &self.verbs,
            u32::from(self.segment_mask),
            false,
        )?;
        debug_assert_eq!(rc.rect, self.bounds);
        Some(PathRectInfo {
            rect: self.bounds,
            direction: rc.direction,
            start_index: 0, // start index???
        })
    }

    /// The oval info if the data was built as an oval.
    // Port of: src/core/SkPathData.cpp#L461-L470 (chrome/m156)
    #[doc(alias = "asOval")]
    #[must_use]
    pub fn as_oval(&self) -> Option<PathOvalInfo> {
        if self.ty == PathIsAType::Oval {
            return Some(PathOvalInfo {
                bounds: self.bounds,
                direction: self.is_a.direction,
                start_index: self.is_a.start_index,
            });
        }
        None
    }

    /// The round-rect info if the data was built as a round rectangle.
    // Port of: src/core/SkPathData.cpp#L472-L481 (chrome/m156)
    #[doc(alias = "asRRect")]
    #[must_use]
    pub fn as_rrect(&self) -> Option<PathRRectInfo> {
        if self.ty == PathIsAType::RRect {
            return Some(PathRRectInfo {
                rrect: path_priv::deduce_rrect_from_contour(
                    &self.bounds,
                    &self.points,
                    &self.verbs,
                ),
                direction: self.is_a.direction,
                start_index: self.is_a.start_index,
            });
        }
        None
    }

    /// True if `p` is inside the data filled with `ft`.
    // Port of: src/core/SkPathData.cpp#L483-L485 (chrome/m156)
    #[must_use]
    pub fn contains(&self, p: Point, ft: PathFillType) -> bool {
        path_priv::contains(&self.raw(ft, ResolveConvexity::No), p)
    }
}

// Port of: src/core/SkPathData.cpp#L292-L301 (chrome/m156)
impl PartialEq for PathData {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        self.points == other.points && self.conics == other.conics && self.verbs == other.verbs
    }
}

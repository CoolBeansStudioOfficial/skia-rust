// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathIter.h, src/core/SkPathIter.cpp

//! Iterators over raw path data (`SkPathIter.h`).

use crate::path_types::PathVerb;
use crate::point::Point;
use crate::scalar::scalar;

/// One verb returned by [`PathIter`], with its points.
///
/// | verb  | points |
/// |-------|--------|
/// | move  | `pts[0]` |
/// | line  | `pts[0..1]` |
/// | quad  | `pts[0..2]` |
/// | conic | `pts[0..2]` + [`conic_weight`](Self::conic_weight) |
/// | cubic | `pts[0..3]` |
/// | close | `pts[0..1]`, as if close were a line from `pts[0]` to `pts[1]` |
///
/// skia-rust: C++ returns a span into the path (or into the iterator, for close); this record
/// carries a copy of the (at most 4) points so it can be returned from an `Iterator`.
// Port of: include/core/SkPathIter.h#L21-L31 (chrome/m156)
#[doc(alias = "SkPathIter::Rec")]
#[derive(Copy, Clone, Debug)]
pub struct PathIterRec {
    pts: [Point; 4],
    count: u8,
    conic_weight: f32,
    verb: PathVerb,
}

impl PathIterRec {
    pub(crate) fn new(src: &[Point], conic_weight: f32, verb: PathVerb) -> Self {
        let mut pts = [Point::default(); 4];
        pts[..src.len()].copy_from_slice(src);
        #[allow(clippy::cast_possible_truncation)] // at most 4 points
        let count = src.len() as u8;
        Self {
            pts,
            count,
            conic_weight,
            verb,
        }
    }

    /// The points of this verb (see the type docs).
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.pts[..usize::from(self.count)]
    }

    /// The conic weight; only meaningful for [`PathVerb::Conic`] (C++ stores `-1` otherwise).
    #[doc(alias = "fConicWeight")]
    #[must_use]
    pub fn conic_weight(&self) -> f32 {
        self.conic_weight
    }

    /// The verb.
    #[must_use]
    pub fn verb(&self) -> PathVerb {
        self.verb
    }
}

/// Iterates a path's verbs, returning each with its points.
// Port of: include/core/SkPathIter.h#L19-L67 (chrome/m156)
#[doc(alias = "SkPathIter")]
#[derive(Clone, Debug)]
pub struct PathIter<'a> {
    p_index: usize,
    v_index: usize,
    c_index: usize,
    points: &'a [Point],
    verbs: &'a [PathVerb],
    conics: &'a [scalar],
    close_point_storage: [Point; 2],
}

impl<'a> PathIter<'a> {
    /// Iterates the given points, verbs and conic weights.
    // Port of: include/core/SkPathIter.h#L33-L42 (chrome/m156)
    #[must_use]
    pub fn new(points: &'a [Point], verbs: &'a [PathVerb], conics: &'a [scalar]) -> Self {
        let mut verbs = verbs;
        // For compat older iterators, we trim off a trailing Move.
        // SkPathData is defined to never create this pattern, so perhaps in the future
        // this check can be removed (or replaced by an assert)
        if let Some(PathVerb::Move) = verbs.last() {
            verbs = &verbs[..verbs.len() - 1];
        }
        Self {
            p_index: 0,
            v_index: 0,
            c_index: 0,
            points,
            verbs,
            conics,
            close_point_storage: [Point::default(); 2],
        }
    }

    /// The next verb, without advancing.
    // Port of: include/core/SkPathIter.h#L54-L59 (chrome/m156)
    #[doc(alias = "peekNextVerb")]
    #[must_use]
    pub fn peek_next_verb(&self) -> Option<PathVerb> {
        self.verbs.get(self.v_index).copied()
    }
}

impl Iterator for PathIter<'_> {
    type Item = PathIterRec;

    // Close is funny -- it has no explicit point data, but we return 2 points,
    // the logical 2 points that would make up the line connecting the end of
    // the contour, and its beginning.
    //
    // To do this, we have local storage (fClosePointStorage)
    // Port of: src/core/SkPathIter.cpp#L17-L47 (chrome/m156)
    fn next(&mut self) -> Option<PathIterRec> {
        if self.v_index >= self.verbs.len() {
            return None;
        }

        let mut w = -1.0;
        let v = self.verbs[self.v_index];
        self.v_index += 1;
        let n: usize = match v {
            PathVerb::Move => {
                self.close_point_storage[1] = self.points[self.p_index]; // remember for close
                self.p_index += 1;
                return Some(PathIterRec::new(&self.close_point_storage[1..2], w, v));
            }
            PathVerb::Line => 1,
            PathVerb::Quad => 2,
            PathVerb::Conic => {
                w = self.conics[self.c_index];
                self.c_index += 1;
                2
            }
            PathVerb::Cubic => 3,
            PathVerb::Close => {
                debug_assert!(self.p_index > 0);
                self.close_point_storage[0] = self.points[self.p_index - 1]; // the last point we saw
                return Some(PathIterRec::new(&self.close_point_storage, w, v));
            }
        };
        debug_assert!(self.p_index > 0);
        let start = self.p_index - 1;
        self.p_index += n;
        Some(PathIterRec::new(&self.points[start..=start + n], w, v))
    }
}

/// One contour returned by [`PathContourIter`].
// Port of: include/core/SkPathIter.h#L71-L75 (chrome/m156)
#[doc(alias = "SkPathContourIter::Rec")]
#[derive(Copy, Clone, Debug)]
pub struct PathContourIterRec<'a> {
    points: &'a [Point],
    verbs: &'a [PathVerb],
    conics: &'a [scalar],
}

impl<'a> PathContourIterRec<'a> {
    /// The contour's points.
    #[must_use]
    pub fn points(&self) -> &'a [Point] {
        self.points
    }

    /// The contour's verbs.
    #[must_use]
    pub fn verbs(&self) -> &'a [PathVerb] {
        self.verbs
    }

    /// The contour's conic weights.
    #[must_use]
    pub fn conics(&self) -> &'a [scalar] {
        self.conics
    }
}

/// Iterates a path one contour at a time.
// Port of: include/core/SkPathIter.h#L69-L88 (chrome/m156)
#[doc(alias = "SkPathContourIter")]
#[derive(Clone, Debug)]
pub struct PathContourIter<'a> {
    points: &'a [Point],
    verbs: &'a [PathVerb],
    conics: &'a [scalar],
}

impl<'a> PathContourIter<'a> {
    /// Iterates the contours of the given points, verbs and conic weights.
    #[must_use]
    pub fn new(points: &'a [Point], verbs: &'a [PathVerb], conics: &'a [scalar]) -> Self {
        Self {
            points,
            verbs,
            conics,
        }
    }
}

impl<'a> Iterator for PathContourIter<'a> {
    type Item = PathContourIterRec<'a>;

    // Port of: src/core/SkPathIter.cpp#L51-L79 (chrome/m156)
    fn next(&mut self) -> Option<PathContourIterRec<'a>> {
        if self.verbs.is_empty() {
            return None;
        }
        debug_assert_eq!(self.verbs[0], PathVerb::Move);
        let (mut npts, mut nvbs, mut nws) = (1, 1, 0);
        for &v in &self.verbs[1..] {
            match v {
                PathVerb::Move => break,
                PathVerb::Line => npts += 1,
                PathVerb::Quad => npts += 2,
                PathVerb::Conic => {
                    npts += 2;
                    nws += 1;
                }
                PathVerb::Cubic => npts += 3,
                PathVerb::Close => {
                    nvbs += 1;
                    break;
                }
            }
            nvbs += 1;
        }
        let rec = PathContourIterRec {
            points: &self.points[..npts],
            verbs: &self.verbs[..nvbs],
            conics: &self.conics[..nws],
        };
        self.points = &self.points[npts..];
        self.verbs = &self.verbs[nvbs..];
        self.conics = &self.conics[nws..];
        Some(rec)
    }
}

// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathRawTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::{PathConvexity, ResolveConvexity};
use skia_rust_core::path_priv;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_types::{PathFillType, PathSegmentMask, PathVerb};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;

// Port of: tests/PathRawTest.cpp#L22-L57 (chrome/m156)
struct SPathRawBuilder<'a> {
    pt_storage: &'a mut [Point],
    vb_storage: &'a mut [PathVerb],
    cn_storage: &'a mut [scalar],
    pts: usize,
    cns: usize,
    vbs: usize,
}

impl<'a> SPathRawBuilder<'a> {
    fn new(
        pt_store: &'a mut [Point],
        vb_store: &'a mut [PathVerb],
        cn_store: &'a mut [scalar],
    ) -> Self {
        Self {
            pt_storage: pt_store,
            vb_storage: vb_store,
            cn_storage: cn_store,
            pts: 0,
            cns: 0,
            vbs: 0,
        }
    }

    fn check_extend_pts(&self, n: usize) {
        debug_assert!(self.pts + n <= self.pt_storage.len());
    }
    fn check_extend_vbs(&self, n: usize) {
        debug_assert!(self.vbs + n <= self.vb_storage.len());
    }
    fn check_extend_cns(&self, n: usize) {
        debug_assert!(self.cns + n <= self.cn_storage.len());
    }

    fn push_pt(&mut self, p: Point) {
        self.pt_storage[self.pts] = p;
        self.pts += 1;
    }

    fn push_vb(&mut self, v: PathVerb) {
        self.vb_storage[self.vbs] = v;
        self.vbs += 1;
    }

    // Port of: tests/PathRawTest.cpp#L61-L66 (chrome/m156)
    fn move_to(&mut self, p: Point) {
        self.check_extend_pts(1);
        self.check_extend_vbs(1);
        self.push_pt(p);
        self.push_vb(PathVerb::Move);
    }

    // Port of: tests/PathRawTest.cpp#L68-L73 (chrome/m156)
    fn line_to(&mut self, p: Point) {
        self.check_extend_pts(1);
        self.check_extend_vbs(1);
        self.push_pt(p);
        self.push_vb(PathVerb::Line);
    }

    // Port of: tests/PathRawTest.cpp#L75-L81 (chrome/m156)
    fn quad_to(&mut self, p1: Point, p2: Point) {
        self.check_extend_pts(2);
        self.check_extend_vbs(1);
        self.push_pt(p1);
        self.push_pt(p2);
        self.push_vb(PathVerb::Quad);
    }

    // Port of: tests/PathRawTest.cpp#L83-L92 (chrome/m156)
    fn conic_to(&mut self, p1: Point, p2: Point, w: scalar) {
        self.check_extend_pts(2);
        self.check_extend_cns(1);
        self.check_extend_vbs(1);
        self.push_pt(p1);
        self.push_pt(p2);
        self.cn_storage[self.cns] = w;
        self.cns += 1;
        self.push_vb(PathVerb::Conic);
    }

    // Port of: tests/PathRawTest.cpp#L94-L101 (chrome/m156)
    fn cubic_to(&mut self, p1: Point, p2: Point, p3: Point) {
        self.check_extend_pts(3);
        self.check_extend_vbs(1);
        self.push_pt(p1);
        self.push_pt(p2);
        self.push_pt(p3);
        self.push_vb(PathVerb::Cubic);
    }

    // Port of: tests/PathRawTest.cpp#L103-L106 (chrome/m156)
    fn close(&mut self) {
        self.check_extend_vbs(1);
        self.push_vb(PathVerb::Close);
    }

    // Port of: tests/PathRawTest.cpp#L108-L119 (chrome/m156)
    fn raw(&self, ft: PathFillType, convexity: PathConvexity) -> PathRaw<'_> {
        let pt_span = &self.pt_storage[..self.pts];
        PathRaw {
            points: pt_span,
            verbs: &self.vb_storage[..self.vbs],
            conics: &self.cn_storage[..self.cns],
            bounds: Rect::bounds_or_empty(pt_span),
            fill_type: ft,
            convexity,
            segment_mask: path_priv::compute_segment_mask(&self.vb_storage[..self.vbs]),
        }
    }
}

// Port of: tests/PathRawTest.cpp#L121-L176 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn check_iter(
    reporter: &mut Reporter,
    raw: &PathRaw<'_>,
    pts: &[Point],
    vbs: &[PathVerb],
    cns: &[f32],
) {
    let (mut p_index, mut v_index, mut c_index) = (0usize, 0usize, 0usize);
    let mut move_to_index = 0; // track the start of each contour

    let mut iter = raw.iter();
    for r in iter.by_ref() {
        reporter_assert!(reporter, v_index < vbs.len());
        reporter_assert!(reporter, vbs[v_index] == r.verb());
        v_index += 1;
        let rp = r.points();
        match r.verb() {
            PathVerb::Move => {
                move_to_index = p_index;
                reporter_assert!(reporter, p_index < pts.len());
                reporter_assert!(reporter, pts[p_index] == rp[0]);
                p_index += 1;
            }
            PathVerb::Line => {
                reporter_assert!(reporter, p_index < pts.len());
                reporter_assert!(reporter, pts[p_index - 1] == rp[0]);
                reporter_assert!(reporter, pts[p_index] == rp[1]);
                p_index += 1;
            }
            PathVerb::Quad => {
                reporter_assert!(reporter, p_index + 1 < pts.len());
                reporter_assert!(reporter, pts[p_index - 1] == rp[0]);
                reporter_assert!(reporter, pts[p_index] == rp[1]);
                p_index += 1;
                reporter_assert!(reporter, pts[p_index] == rp[2]);
                p_index += 1;
            }
            PathVerb::Conic => {
                reporter_assert!(reporter, p_index + 1 < pts.len());
                reporter_assert!(reporter, pts[p_index - 1] == rp[0]);
                reporter_assert!(reporter, pts[p_index] == rp[1]);
                p_index += 1;
                reporter_assert!(reporter, pts[p_index] == rp[2]);
                p_index += 1;
                reporter_assert!(reporter, c_index < cns.len());
                reporter_assert!(reporter, cns[c_index] == r.conic_weight());
                c_index += 1;
            }
            PathVerb::Cubic => {
                reporter_assert!(reporter, p_index + 2 < pts.len());
                reporter_assert!(reporter, pts[p_index - 1] == rp[0]);
                reporter_assert!(reporter, pts[p_index] == rp[1]);
                p_index += 1;
                reporter_assert!(reporter, pts[p_index] == rp[2]);
                p_index += 1;
                reporter_assert!(reporter, pts[p_index] == rp[3]);
                p_index += 1;
            }
            PathVerb::Close => {
                reporter_assert!(reporter, pts[p_index - 1] == rp[0]); // last  pt
                reporter_assert!(reporter, pts[move_to_index] == rp[1]); // first pt
            }
        }
    }

    // make sure the iter is really done
    reporter_assert!(reporter, iter.next().is_none());
}

// Port of: tests/PathRawTest.cpp#L178-L233 (chrome/m156)
def_test!(
    #[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
    pathraw_iter,
    |reporter| {
        let mut pts = [Point::default(); 11];
        let mut vbs = [PathVerb::Move; 8];
        let mut cns = [0.0f32; 1];

        const N: usize = 11;
        let mut p = [Point::default(); N];
        for (i, pi) in p.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss)] // SkScalar(i)
            let f = i as scalar;
            *pi = Point::new(f, f);
        }

        let verbs = [
            PathVerb::Move,
            PathVerb::Line,
            PathVerb::Quad,
            PathVerb::Cubic,
            PathVerb::Close,
            PathVerb::Move,
            PathVerb::Line,
            PathVerb::Conic,
        ];

        let cns_copy;
        {
            let mut bu = SPathRawBuilder::new(&mut pts, &mut vbs, &mut cns);

            bu.move_to(p[0]);
            bu.line_to(p[1]);
            bu.quad_to(p[2], p[3]);
            bu.cubic_to(p[4], p[5], p[6]);
            bu.close();
            bu.move_to(p[7]);
            bu.line_to(p[8]);
            bu.conic_to(p[9], p[10], 2.0);

            let raw = bu.raw(PathFillType::Winding, PathConvexity::Unknown);

            reporter_assert!(reporter, raw.points.len() == N);
            reporter_assert!(reporter, raw.verbs.len() == 8);
            reporter_assert!(reporter, raw.conics.len() == 1);

            cns_copy = [bu.cn_storage[0]];
            check_iter(reporter, &raw, &p, &verbs, &cns_copy);
        }

        // now make sure pathbuilder generates the same results

        let mut pb = PathBuilder::new();

        pb.move_to(p[0]);
        pb.line_to(p[1]);
        pb.quad_to(p[2], p[3]);
        pb.cubic_to(p[4], p[5], p[6]);
        pb.close();
        pb.move_to(p[7]);
        pb.line_to(p[8]);
        pb.conic_to(p[9], p[10], 2.0);

        let path = pb.detach();
        let raw = path_priv::raw(&path, ResolveConvexity::No).expect("finite path");

        check_iter(reporter, &raw, &p, &verbs, &cns_copy);
    }
);

// Port of: tests/PathRawTest.cpp#L235-L300 (chrome/m156)
def_test!(pathraw_segmentmask, |reporter| {
    // skia-rust: `skiatest::ReporterContext` is not ported; `desc` is unused.
    let check_mask = |reporter: &mut Reporter,
                      _desc: &str,
                      expected_mask: u32,
                      build: &dyn Fn(&mut SPathRawBuilder<'_>)| {
        // Make these buffers plenty big to hold any of the paths in the tests
        let mut pts = [Point::default(); 20];
        let mut vbs = [PathVerb::Move; 20];
        let mut cns = [0.0f32; 10];
        let mut bu = SPathRawBuilder::new(&mut pts, &mut vbs, &mut cns);
        build(&mut bu);
        let raw = bu.raw(PathFillType::Winding, PathConvexity::Unknown);
        reporter_assert!(reporter, u32::from(raw.segment_mask) == expected_mask);
    };

    let p = Point::new;
    let line = PathSegmentMask::LINE.bits();
    let quad = PathSegmentMask::QUAD.bits();
    let conic = PathSegmentMask::CONIC.bits();
    let cubic = PathSegmentMask::CUBIC.bits();

    check_mask(reporter, "move-only", 0, &|bu| bu.move_to(p(0.0, 0.0)));

    check_mask(reporter, "line", line, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.line_to(p(1.0, 1.0));
    });

    check_mask(reporter, "quad", quad, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.quad_to(p(1.0, 1.0), p(2.0, 2.0));
    });

    check_mask(reporter, "conic", conic, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.conic_to(p(1.0, 1.0), p(2.0, 2.0), 0.5);
    });

    check_mask(reporter, "cubic", cubic, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.cubic_to(p(1.0, 1.0), p(2.0, 2.0), p(3.0, 3.0));
    });

    check_mask(reporter, "line-quad", line | quad, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.line_to(p(1.0, 1.0));
        bu.quad_to(p(2.0, 2.0), p(3.0, 3.0));
    });

    check_mask(reporter, "conic-cubic", conic | cubic, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.conic_to(p(1.0, 1.0), p(2.0, 2.0), 0.5);
        bu.cubic_to(p(3.0, 3.0), p(4.0, 4.0), p(5.0, 5.0));
    });

    check_mask(reporter, "all", line | quad | conic | cubic, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.line_to(p(1.0, 1.0));
        bu.quad_to(p(2.0, 2.0), p(3.0, 3.0));
        bu.conic_to(p(4.0, 4.0), p(5.0, 5.0), 0.5);
        bu.cubic_to(p(6.0, 6.0), p(7.0, 7.0), p(8.0, 8.0));
        bu.close();
    });

    check_mask(reporter, "empty path", 0, &|bu| {
        bu.move_to(p(0.0, 0.0));
        bu.close();
    });
});

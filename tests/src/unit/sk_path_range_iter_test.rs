// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkPathRangeIterTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, errorf, reporter_assert};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;

// Port of: tests/SkPathRangeIterTest.cpp#L22-L140 (chrome/m156)
def_test!(
    #[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
    #[allow(clippy::float_cmp)] // exact float comparisons, as in C++
    SkPath_RangeIter,
    |r| {
        #[derive(Copy, Clone, PartialEq, Eq)]
        #[allow(dead_code)] // mirrors the C++ enum (kQuad is not used by the test)
        enum Verb {
            Move,
            Line,
            Quad,
            Conic,
            Cubic,
            Close,
            ImplicitMove,
        }

        let verbs = [
            Verb::ImplicitMove,
            Verb::Line,
            Verb::Conic,
            Verb::Close,
            Verb::ImplicitMove,
            Verb::Cubic,
            Verb::Move,
            Verb::Conic,
            Verb::Line,
            Verb::Close,
            Verb::Move,
        ];

        struct Gen(Random);
        impl Gen {
            fn p(&mut self) -> Point {
                let x = self.0.next_f();
                let y = self.0.next_f();
                Point::new(x, y)
            }
            #[allow(clippy::float_cmp)] // exact float comparisons, as in C++
            fn w(&mut self) -> f32 {
                self.0.next_f()
            }
        }
        let mut gen_data = Gen(Random::default());
        let mut test_data = Gen(Random::default());

        for _ in 0..10 {
            if gen_data.p() != test_data.p() || gen_data.w() != test_data.w() {
                errorf!(r, "genData and testData not in sync.");
                return;
            }
        }

        // Build the path.
        let mut builder = PathBuilder::new();
        for verb in verbs {
            match verb {
                Verb::ImplicitMove => {}
                Verb::Move => {
                    builder.move_to(gen_data.p());
                }
                Verb::Line => {
                    builder.line_to(gen_data.p());
                }
                Verb::Quad => {
                    let a = gen_data.p();
                    let b = gen_data.p();
                    builder.quad_to(a, b);
                }
                Verb::Cubic => {
                    let a = gen_data.p();
                    let b = gen_data.p();
                    let c = gen_data.p();
                    builder.cubic_to(a, b, c);
                }
                Verb::Conic => {
                    let a = gen_data.p();
                    let b = gen_data.p();
                    let w = gen_data.w();
                    builder.conic_to(a, b, w);
                }
                Verb::Close => {
                    builder.close();
                }
            }
        }
        let path = builder.detach();

        // Verify sure the RangeIter works as expected.
        let mut iter = path_priv::iterate(&path);
        let mut start_pt = Point::new(0.0, 0.0);
        let mut last_pt = Point::new(0.0, 0.0);
        for verb in verbs {
            let (_path_verb, path_pts, path_wt) = iter.next().expect("a verb");
            match verb {
                Verb::ImplicitMove => {
                    reporter_assert!(r, path_pts[0] == start_pt);
                    last_pt = path_pts[0];
                }
                Verb::Move => {
                    reporter_assert!(r, path_pts[0] == test_data.p());
                    start_pt = path_pts[0];
                    last_pt = path_pts[0];
                }
                Verb::Line => {
                    reporter_assert!(r, path_pts[0] == last_pt);
                    reporter_assert!(r, path_pts[1] == test_data.p());
                    last_pt = path_pts[1];
                }
                Verb::Quad => {
                    reporter_assert!(r, path_pts[0] == last_pt);
                    reporter_assert!(r, path_pts[1] == test_data.p());
                    reporter_assert!(r, path_pts[2] == test_data.p());
                    last_pt = path_pts[2];
                }
                Verb::Cubic => {
                    reporter_assert!(r, path_pts[0] == last_pt);
                    reporter_assert!(r, path_pts[1] == test_data.p());
                    reporter_assert!(r, path_pts[2] == test_data.p());
                    reporter_assert!(r, path_pts[3] == test_data.p());
                    last_pt = path_pts[3];
                }
                Verb::Conic => {
                    reporter_assert!(r, path_pts[0] == last_pt);
                    reporter_assert!(r, path_pts[1] == test_data.p());
                    reporter_assert!(r, path_pts[2] == test_data.p());
                    reporter_assert!(r, path_wt == Some(test_data.w()));
                    last_pt = path_pts[2];
                }
                Verb::Close => {
                    reporter_assert!(r, path_pts[0] == last_pt);
                }
            }
        }
        reporter_assert!(r, iter.is_done());
    }
);

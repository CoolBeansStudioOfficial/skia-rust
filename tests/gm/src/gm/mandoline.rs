// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/mandoline.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::geometry::{Conic, chop_cubic_at, chop_quad_at};
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{SCALAR_PI, degrees_to_radians, scalar_cos, scalar_sin};

// Port of: C `scalbnf(1, -k)` (exact power of two, subnormal range included) as used by
// gm/mandoline.cpp#L130.
fn scalbn_one_neg(k: i32) -> f32 {
    if k <= 126 {
        f32::from_bits(((127 - k) as u32) << 23)
    } else {
        f32::from_bits(1u32 << (149 - k))
    }
}

// Slices paths into sliver-size contours shaped like ice cream cones.
// Port of: gm/mandoline.cpp#L26-L139 (chrome/m156)
struct MandolineSlicer {
    rand: Random,
    builder: PathBuilder,
    anchor_pt: Point,
    last_pt: Point,
}

impl MandolineSlicer {
    const DEFAULT_SUBDIVISIONS: i32 = 10;

    // Port of: gm/mandoline.cpp#L31-L33 (chrome/m156)
    fn new(anchor_pt: Point) -> Self {
        let mut s = Self {
            rand: Random::default(),
            builder: PathBuilder::new(),
            anchor_pt,
            last_pt: anchor_pt,
        };
        s.reset(anchor_pt);
        s
    }

    // Port of: gm/mandoline.cpp#L35-L42 (chrome/m156)
    fn reset(&mut self, anchor_pt: Point) {
        self.builder.reset();

        // see https://skia-review.googlesource.com/c/skia/+/1055736
        self.builder.set_is_volatile(true);

        self.anchor_pt = anchor_pt;
        self.last_pt = anchor_pt;
    }

    // Port of: gm/mandoline.cpp#L44-L60 (chrome/m156)
    fn slice_line(&mut self, pt: Point, num_subdivisions: i32) {
        if num_subdivisions <= 0 {
            self.builder.move_to(self.anchor_pt);
            self.builder.line_to(self.last_pt);
            self.builder.line_to(pt);
            self.builder.close();
            self.last_pt = pt;
            return;
        }
        let t = self.choose_chop_t(num_subdivisions);
        if 0.0 == t {
            return;
        }
        let midpt = self.last_pt * (1.0 - t) + pt * t;
        self.slice_line(midpt, num_subdivisions - 1);
        self.slice_line(pt, num_subdivisions - 1);
    }

    // Port of: gm/mandoline.cpp#L62-L79 (chrome/m156)
    fn slice_quadratic(&mut self, p1: Point, p2: Point, num_subdivisions: i32) {
        if num_subdivisions <= 0 {
            self.builder.move_to(self.anchor_pt);
            self.builder.line_to(self.last_pt);
            self.builder.quad_to(p1, p2);
            self.builder.close();
            self.last_pt = p2;
            return;
        }
        let t = self.choose_chop_t(num_subdivisions);
        if 0.0 == t {
            return;
        }
        let p = [self.last_pt, p1, p2];
        let mut pp = [Point::default(); 5];
        chop_quad_at(&p, &mut pp, t);
        self.slice_quadratic(pp[1], pp[2], num_subdivisions - 1);
        self.slice_quadratic(pp[3], pp[4], num_subdivisions - 1);
    }

    // Port of: gm/mandoline.cpp#L81-L99 (chrome/m156)
    fn slice_cubic(&mut self, p1: Point, p2: Point, p3: Point, num_subdivisions: i32) {
        if num_subdivisions <= 0 {
            self.builder.move_to(self.anchor_pt);
            self.builder.line_to(self.last_pt);
            self.builder.cubic_to(p1, p2, p3);
            self.builder.close();
            self.last_pt = p3;
            return;
        }
        let t = self.choose_chop_t(num_subdivisions);
        if 0.0 == t {
            return;
        }
        let p = [self.last_pt, p1, p2, p3];
        let mut pp = [Point::default(); 7];
        chop_cubic_at(&p, &mut pp, t);
        self.slice_cubic(pp[1], pp[2], pp[3], num_subdivisions - 1);
        self.slice_cubic(pp[4], pp[5], pp[6], num_subdivisions - 1);
    }

    // Port of: gm/mandoline.cpp#L101-L120 (chrome/m156)
    fn slice_conic(&mut self, p1: Point, p2: Point, w: f32, num_subdivisions: i32) {
        if num_subdivisions <= 0 {
            self.builder.move_to(self.anchor_pt);
            self.builder.line_to(self.last_pt);
            self.builder.conic_to(p1, p2, w);
            self.builder.close();
            self.last_pt = p2;
            return;
        }
        let t = self.choose_chop_t(num_subdivisions);
        if 0.0 == t {
            return;
        }
        let conic = Conic::new(self.last_pt, p1, p2, w);
        let mut halves = [
            Conic::new(Point::default(), Point::default(), Point::default(), 1.0),
            Conic::new(Point::default(), Point::default(), Point::default(), 1.0),
        ];
        assert!(conic.chop_at(t, &mut halves), "SkConic::chopAt failed");
        self.slice_conic(
            halves[0].pts[1],
            halves[0].pts[2],
            halves[0].w,
            num_subdivisions - 1,
        );
        self.slice_conic(
            halves[1].pts[1],
            halves[1].pts[2],
            halves[1].w,
            num_subdivisions - 1,
        );
    }

    fn path(&self) -> Path {
        self.builder.snapshot()
    }

    // Port of: gm/mandoline.cpp#L124-L133 (chrome/m156)
    fn choose_chop_t(&mut self, num_subdivisions: i32) -> f32 {
        assert!(num_subdivisions > 0);
        if num_subdivisions > 1 {
            return 0.5;
        }
        let t = if self.rand.next_u().is_multiple_of(10) {
            0.0
        } else {
            // The C++ `(int)` cast of nextRangeU's result, which is within 10..=149.
            #[allow(clippy::cast_possible_wrap)] // 10..=149
            let k = self.rand.next_range_u(10, 149) as i32;
            scalbn_one_neg(k)
        };
        assert!((0.0..1.0).contains(&t));
        t
    }
}

// Port of: gm/mandoline.cpp#L141-L196 (chrome/m156)
struct SliverPathsGm;

impl GM for SliverPathsGm {
    fn name(&self) -> String {
        "mandoline".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(560, 475)
    }

    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/mandoline.cpp#L152-L195 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int loop index, as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        paint.set_anti_alias(true);

        let mut mandoline = MandolineSlicer::new(Point::new(41.0, 43.0));
        mandoline.slice_cubic(
            Point::new(5.0, 277.0),
            Point::new(381.0, -74.0),
            Point::new(243.0, 162.0),
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        mandoline.slice_line(
            Point::new(41.0, 43.0),
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        canvas.draw_path(&mandoline.path(), &paint);

        mandoline.reset(Point::new(357.049_988, 446.049_988));
        mandoline.slice_cubic(
            Point::new(472.750_000, -71.950_012),
            Point::new(639.750_000, 531.950_012),
            Point::new(309.049_988, 347.950_012),
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        mandoline.slice_line(
            Point::new(309.049_988, 419.0),
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        mandoline.slice_line(
            Point::new(357.049_988, 446.049_988),
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        canvas.draw_path(&mandoline.path(), &paint);

        canvas.save();
        canvas.translate((421.0, 105.0));
        canvas.scale((100.0, 81.0));
        mandoline.reset(Point::new(
            -scalar_cos(degrees_to_radians(-60.0)),
            scalar_sin(degrees_to_radians(-60.0)),
        ));
        mandoline.slice_conic(
            Point::new(-2.0, 0.0),
            Point::new(
                -scalar_cos(degrees_to_radians(60.0)),
                scalar_sin(degrees_to_radians(60.0)),
            ),
            0.5,
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        mandoline.slice_conic(
            Point::new(
                -scalar_cos(degrees_to_radians(120.0)) * 2.0,
                scalar_sin(degrees_to_radians(120.0)) * 2.0,
            ),
            Point::new(1.0, 0.0),
            0.5,
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        mandoline.slice_line(Point::new(0.0, 0.0), MandolineSlicer::DEFAULT_SUBDIVISIONS);
        mandoline.slice_line(
            Point::new(
                -scalar_cos(degrees_to_radians(-60.0)),
                scalar_sin(degrees_to_radians(-60.0)),
            ),
            MandolineSlicer::DEFAULT_SUBDIVISIONS,
        );
        canvas.draw_path(&mandoline.path(), &paint);
        canvas.restore();

        canvas.save();
        canvas.translate((150.0, 300.0));
        canvas.scale((75.0, 75.0));
        mandoline.reset(Point::new(1.0, 0.0));
        const NQUADS: i32 = 5;
        for i in 0..NQUADS {
            let theta1 = 2.0 * SCALAR_PI / (NQUADS as f32) * ((i as f32) + 0.5);
            let theta2 = 2.0 * SCALAR_PI / (NQUADS as f32) * ((i as f32) + 1.0);
            mandoline.slice_quadratic(
                Point::new(scalar_cos(theta1) * 2.0, scalar_sin(theta1) * 2.0),
                Point::new(scalar_cos(theta2), scalar_sin(theta2)),
                MandolineSlicer::DEFAULT_SUBDIVISIONS,
            );
        }
        canvas.draw_path(&mandoline.path(), &paint);
        canvas.restore();
    }
}

crate::def_gm!(SliverPathsGM, SliverPathsGm);

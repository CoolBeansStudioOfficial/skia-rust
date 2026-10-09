// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/RectTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::rect::{Contains, IRect, Rect as SkRect, RoundOut};
use skia_rust_gpu::graphite::geom::rect::Rect;
use skia_rust_simd::vx::{self, Float2, Float4};

use crate::{def_test, reporter_assert};

// `all(a == b)` for `float2` lanes.
fn all_eq2(a: Float2, b: Float2) -> bool {
    vx::all(a.eq_mask(b))
}

// `all(a == b)` for `float4` lanes.
fn all_eq4(a: Float4, b: Float4) -> bool {
    vx::all(a.eq_mask(b))
}

def_test!(skgpu_Rect, |r| {
    // Port of: tests/graphite/RectTest.cpp#L15-L119 (chrome/m156)
    let sk_rect = SkRect::from_ltrb(1.0, -3.0, 4.0, 0.0);
    let rect = Rect::from(sk_rect);
    reporter_assert!(r, rect == rect);
    reporter_assert!(r, rect == sk_rect); // promotes 'skRect' to a Rect for ==
    reporter_assert!(r, rect.as_sk_rect() == sk_rect); // converts 'rect' to SkRect for ==

    for l in [0.0_f32, 1.0, 2.0] {
        for t in [-4.0_f32, -3.0, -2.0] {
            for rr in [3.0_f32, 4.0, 5.0] {
                for b in [-1.0_f32, 0.0, 1.0] {
                    let rect2 = Rect::new(l, t, rr, b);
                    let sk_rect2 = SkRect::new(l, t, rr, b);

                    reporter_assert!(r, rect2 == rect2);
                    reporter_assert!(
                        r,
                        rect2 == Rect::from_corners(Float2::new(l, t), Float2::new(rr, b))
                    );
                    reporter_assert!(r, rect2 == Rect::from(sk_rect2));
                    reporter_assert!(r, rect2.as_sk_rect() == sk_rect2);

                    reporter_assert!(r, (rect2 == rect) == (rect == rect2));
                    reporter_assert!(r, (rect2 != rect) == (rect != rect2));
                    reporter_assert!(r, (rect != rect2) == !(rect == rect2));

                    reporter_assert!(r, rect2 == Rect::xywh(l, t, rr - l, b - t));
                    reporter_assert!(
                        r,
                        rect2 == Rect::xywh_vec(Float2::new(l, t), Float2::new(rr - l, b - t))
                    );
                    if l == 0.0 && t == 0.0 {
                        reporter_assert!(r, rect2 == Rect::wh(rr - l, b - t));
                        reporter_assert!(r, rect2 == Rect::wh_vec(Float2::new(rr - l, b - t)));
                    }
                    reporter_assert!(r, rect2 == Rect::from_vals(rect2.vals()));

                    reporter_assert!(r, rect2.x() == l);
                    reporter_assert!(r, rect2.y() == t);
                    reporter_assert!(r, rect2.left() == l);
                    reporter_assert!(r, rect2.top() == t);
                    reporter_assert!(r, rect2.right() == rr);
                    reporter_assert!(r, rect2.bot() == b);
                    reporter_assert!(r, all_eq2(rect2.top_left(), Float2::new(l, t)));
                    reporter_assert!(r, all_eq2(rect2.bot_right(), Float2::new(rr, b)));
                    reporter_assert!(r, all_eq4(rect2.ltrb(), Float4::new(l, t, rr, b)));
                    reporter_assert!(r, all_eq4(rect2.vals(), Float4::new(l, t, -rr, -b)));

                    let mut set_test = Rect::new(-99.0, -99.0, 99.0, 99.0);
                    reporter_assert!(r, set_test != rect2);
                    set_test.set_left(l);
                    set_test.set_top(t);
                    set_test.set_right(rr);
                    set_test.set_bot(b);
                    reporter_assert!(r, set_test == rect2);

                    set_test = Rect::new(-99.0, -99.0, 99.0, 99.0);
                    reporter_assert!(r, set_test != rect2);
                    set_test.set_top_left(Float2::new(l, t));
                    set_test.set_bot_right(Float2::new(rr, b));
                    reporter_assert!(r, set_test == rect2);

                    for i in 0..4 {
                        let mut rnan = rect2;
                        reporter_assert!(r, !rnan.is_empty_negative_or_nan());
                        rnan.vals_mut()[i] = f32::NAN;
                        reporter_assert!(r, rnan.is_empty_negative_or_nan());
                    }

                    reporter_assert!(
                        r,
                        all_eq2(
                            rect2.size(),
                            Float2::new(sk_rect2.width(), sk_rect2.height())
                        )
                    );
                    reporter_assert!(
                        r,
                        all_eq2(
                            rect2.center(),
                            Float2::new(sk_rect2.center_x(), sk_rect2.center_y())
                        )
                    );
                    reporter_assert!(r, rect2.area() == sk_rect2.height() * sk_rect2.width());

                    reporter_assert!(r, rect.intersects(rect2) == rect2.intersects(rect));
                    reporter_assert!(r, rect.intersects(rect2) == sk_rect.intersects(sk_rect2));
                    reporter_assert!(r, rect.contains(rect2) == sk_rect.contains(sk_rect2));
                    reporter_assert!(r, rect2.contains(rect) == sk_rect2.contains(sk_rect));

                    reporter_assert!(
                        r,
                        rect2.make_round_in() == SkRect::from_irect(sk_rect2.round_in())
                    );
                    let round_out: IRect = sk_rect2.round_out();
                    reporter_assert!(r, rect2.make_round_out() == SkRect::from_irect(round_out));
                    reporter_assert!(
                        r,
                        rect2.make_inset(0.5) == sk_rect2.with_inset((0.5, 0.5))
                    );
                    reporter_assert!(
                        r,
                        rect2.make_inset_vec(Float2::new(0.5, -0.25))
                            == sk_rect2.with_inset((0.5, -0.25))
                    );
                    reporter_assert!(
                        r,
                        rect2.make_outset(0.5) == sk_rect2.with_outset((0.5, 0.5))
                    );
                    reporter_assert!(
                        r,
                        rect2.make_outset_vec(Float2::new(0.5, -0.25))
                            == sk_rect2.with_outset((0.5, -0.25))
                    );
                    reporter_assert!(
                        r,
                        rect2.make_offset(Float2::new(0.5, -0.25))
                            == sk_rect2.with_offset((0.5, -0.25))
                    );

                    let mut sk_join = sk_rect;
                    sk_join.join(sk_rect2);
                    reporter_assert!(r, rect.make_join(rect2) == sk_join);
                    reporter_assert!(r, rect.make_join(rect2) == rect2.make_join(rect));

                    reporter_assert!(
                        r,
                        rect.intersects(rect2) == !rect.make_intersect(rect2).is_empty_negative_or_nan()
                    );
                    reporter_assert!(r, rect.make_intersect(rect2) == rect2.make_intersect(rect));
                    if rect.intersects(rect2) {
                        reporter_assert!(r, sk_rect.intersects(sk_rect2));
                        let mut sk_isect = SkRect::new_empty();
                        reporter_assert!(r, sk_isect.intersect2(sk_rect, sk_rect2));
                        reporter_assert!(r, rect.make_intersect(rect2) == Rect::from(sk_isect));
                    }

                    let rect3 = Rect::new(rr, b, l, t); // intentionally out of order
                    let sk_rect3 = SkRect::new(rr, b, l, t);
                    reporter_assert!(r, rect3.is_empty_negative_or_nan());
                    reporter_assert!(r, sk_rect3.is_empty());
                    reporter_assert!(r, rect3.make_sorted() == sk_rect3.sorted());
                    reporter_assert!(r, rect3.make_sorted() == rect2);
                }
            }
        }
    }
});

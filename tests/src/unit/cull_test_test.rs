// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CullTestTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::float_cmp)] // the C++ compares scalars with ==

use crate::{def_test, reporter_assert};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{Contains, Rect};
use skia_rust_gpu::tessellate::cull_test::CullTest;

// Port of: tests/CullTestTest.cpp#L18-L31 (chrome/m156), `gMatrices`.
fn g_matrices() -> [Matrix; 6] {
    [
        Matrix::default(),
        Matrix::translate((25.0, -1000.0)),
        Matrix::scale((0.5, 1000.1)),
        Matrix::new_all(1000.1, 0.0, -100.0, 0.0, 0.5, -3000.0, 0.0, 0.0, 1.0),
        Matrix::new_all(0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        Matrix::new_all(2.0, 7.0, -100.0, -8000.0, 0.5, 2000.0, 0.0, 0.0, 1.0),
    ]
}

// Port of: tests/CullTestTest.cpp#L33-L82 (chrome/m156)
def_test!(CullTestTest, |reporter| {
    let mut rand = Random::default();
    let (l, t, r, b) = (10.0_f32, 2000.0_f32, 100.0_f32, 2064.0_f32);
    let viewport_rect = Rect::new(l, t, r, b);
    let values_l = [l - 20.0, l - 10.0, l + 10.0, l + 20.0];
    let values_t = [t - 20.0, t - 10.0, t + 10.0, t + 20.0];
    let values_r = [r + 20.0, r + 10.0, r - 10.0, r - 20.0];
    let values_b = [b + 20.0, b + 10.0, b - 10.0, b - 20.0];
    for m in g_matrices() {
        let cull_test = CullTest::new(&viewport_rect, &m);
        let inverse = m.invert().expect("SkAssertResult(m.invert(&inverse))");
        for y in [&values_t, &values_b] {
            for x in [&values_l, &values_r] {
                for _ in 0..500 {
                    let mask = rand.next_u();
                    // Two 2-bit selectors per point, four points.
                    let dev_pts: [Point; 4] = std::array::from_fn(|i| {
                        let shift = 4 * u32::try_from(i).expect("i < 4");
                        Point::new(
                            x[((mask >> shift) & 3) as usize],
                            y[((mask >> (shift + 2)) & 3) as usize],
                        )
                    });

                    let mut local_pts = [Point::new(0.0, 0.0); 4];
                    inverse.map_points(&mut local_pts, &dev_pts);

                    reporter_assert!(
                        reporter,
                        cull_test.is_visible(local_pts[0]) == viewport_rect.contains(dev_pts[0])
                    );

                    {
                        let mut dev_bounds3 = Rect::bounds_or_empty(&dev_pts[..3]);
                        // Outset devBounds because SkRect::intersects returns false on empty,
                        // which is NOT the behavior we want.
                        dev_bounds3.outset((1e-3, 1e-3));
                        reporter_assert!(
                            reporter,
                            cull_test.are_visible3(&local_pts[..3])
                                == viewport_rect.intersects(dev_bounds3)
                        );
                    }

                    {
                        let mut dev_bounds4 = Rect::bounds_or_empty(&dev_pts);
                        // Outset devBounds because SkRect::intersects returns false on empty,
                        // which is NOT the behavior we want.
                        dev_bounds4.outset((1e-3, 1e-3));
                        reporter_assert!(
                            reporter,
                            cull_test.are_visible4(&local_pts)
                                == viewport_rect.intersects(dev_bounds4)
                        );
                    }
                }
            }
        }
    }
});

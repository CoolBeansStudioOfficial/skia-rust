// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PolyUtilsTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::point::Point;
use skia_rust_core::scalar::{SCALAR_PI, int_to_scalar, scalar, scalar_cos, scalar_sin};
use skia_rust_core::utils::poly_utils::{
    get_polygon_winding, is_convex_polygon, is_simple_polygon, triangulate_simple_polygon,
};

use crate::{def_test, reporter_assert};

// Port of: tests/PolyUtilsTest.cpp#L17-L421 (chrome/m156)
def_test!(
    #[allow(
        clippy::excessive_precision,
        clippy::too_many_lines,
        clippy::unreadable_literal,
        clippy::float_cmp,
        clippy::similar_names
    )] // mirrors the C++ test: its float literals and its many asserts are copied as written
    PolyUtils,
    |reporter| {
        let mut poly: Vec<Point> = Vec::new();
        // init simple index map
        let index_map: [u16; 1024] =
            std::array::from_fn(|i| u16::try_from(i).expect("index map entries fit in uint16_t"));
        let mut triangle_indices: Vec<u16> = Vec::new();

        // skinny triangle
        poly.push(Point::new(-100.0, 55.0));
        poly.push(Point::new(100.0, 55.0));
        poly.push(Point::new(102.5_f32, 54.330127_f32));
        reporter_assert!(reporter, get_polygon_winding(&poly) < 0);
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // switch winding
        poly[2].set(102.5_f32, 55.330127_f32);
        reporter_assert!(reporter, get_polygon_winding(&poly) > 0);
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // make degenerate
        poly[2].set(100.0 + 2.5_f32, 55.0);
        reporter_assert!(reporter, get_polygon_winding(&poly) == 0);
        // TODO: should these fail?
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            !triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // giant triangle
        poly[0].set(-1.0e+37_f32, 1.0e+37_f32);
        poly[1].set(1.0e+37_f32, 1.0e+37_f32);
        poly[2].set(-1.0e+37_f32, -1.0e+37_f32);
        reporter_assert!(reporter, get_polygon_winding(&poly) < 0);
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // teeny triangle
        poly[0].set(-1.0e-38_f32, 1.0e-38_f32);
        poly[1].set(-1.0e-38_f32, -1.0e-38_f32);
        poly[2].set(1.0e-38_f32, 1.0e-38_f32);
        reporter_assert!(reporter, get_polygon_winding(&poly) == 0);
        // TODO: should these fail?
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            !triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // triangle way off in space (relative to size so we don't completely obliterate values)
        poly[0].set(-100.0 + 1.0e+9_f32, 55.0 - 1.0e+9_f32);
        poly[1].set(100.0 + 1.0e+9_f32, 55.0 - 1.0e+9_f32);
        poly[2].set(150.0 + 1.0e+9_f32, 100.0 - 1.0e+9_f32);
        reporter_assert!(reporter, get_polygon_winding(&poly) > 0);
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        ///////////////////////////////////////////////////////////////////////
        // round rect
        poly.clear();
        poly.push(Point::new(-100.0, 55.0));
        poly.push(Point::new(100.0, 55.0));
        poly.push(Point::new(100.0 + 2.5_f32, 50.0 + 4.330127_f32));
        poly.push(Point::new(100.0 + 3.535534_f32, 50.0 + 3.535534_f32));
        poly.push(Point::new(100.0 + 4.330127_f32, 50.0 + 2.5_f32));
        poly.push(Point::new(105.0, 50.0));
        poly.push(Point::new(105.0, -50.0));
        poly.push(Point::new(100.0 + 4.330127_f32, -50.0 - 2.5_f32));
        poly.push(Point::new(100.0 + 3.535534_f32, -50.0 - 3.535534_f32));
        poly.push(Point::new(100.0 + 2.5_f32, -50.0 - 4.330127_f32));
        poly.push(Point::new(100.0, -55.0));
        poly.push(Point::new(-100.0, -55.0));
        poly.push(Point::new(-100.0 - 2.5_f32, -50.0 - 4.330127_f32));
        poly.push(Point::new(-100.0 - 3.535534_f32, -50.0 - 3.535534_f32));
        poly.push(Point::new(-100.0 - 4.330127_f32, -50.0 - 2.5_f32));
        poly.push(Point::new(-105.0, -50.0));
        poly.push(Point::new(-105.0, 50.0));
        poly.push(Point::new(-100.0 - 4.330127_f32, 50.0 + 2.5_f32));
        poly.push(Point::new(-100.0 - 3.535534_f32, 50.0 + 3.535534_f32));
        poly.push(Point::new(-100.0 - 2.5_f32, 50.0 + 4.330127_f32));
        reporter_assert!(reporter, get_polygon_winding(&poly) < 0);
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // translate far enough to obliterate some low bits
        for point in &mut poly {
            point.offset((1.0e+7_f32, 1.0e+7_f32));
        }
        reporter_assert!(reporter, get_polygon_winding(&poly) < 0);
        // Due to floating point error it's no longer convex
        reporter_assert!(reporter, !is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // translate a little farther to create some coincident vertices
        for point in &mut poly {
            point.offset((4.0e+7_f32, 4.0e+7_f32));
        }
        reporter_assert!(reporter, get_polygon_winding(&poly) < 0);
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        // This can't handle coincident vertices
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            !triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // troublesome case -- clipped roundrect
        poly.clear();
        poly.push(Point::new(335.928101_f32, 428.219055_f32));
        poly.push(Point::new(330.414459_f32, 423.034912_f32));
        poly.push(Point::new(325.749084_f32, 417.395508_f32));
        poly.push(Point::new(321.931946_f32, 411.300842_f32));
        poly.push(Point::new(318.963074_f32, 404.750977_f32));
        poly.push(Point::new(316.842468_f32, 397.745850_f32));
        poly.push(Point::new(315.570068_f32, 390.285522_f32));
        poly.push(Point::new(315.145966_f32, 382.369965_f32));
        poly.push(Point::new(315.570068_f32, 374.454346_f32));
        poly.push(Point::new(316.842468_f32, 366.994019_f32));
        poly.push(Point::new(318.963074_f32, 359.988892_f32));
        poly.push(Point::new(321.931946_f32, 353.439056_f32));
        poly.push(Point::new(325.749084_f32, 347.344421_f32));
        poly.push(Point::new(330.414459_f32, 341.705017_f32));
        poly.push(Point::new(335.928101_f32, 336.520813_f32));
        poly.push(Point::new(342.289948_f32, 331.791901_f32));
        poly.push(Point::new(377.312134_f32, 331.791901_f32));
        poly.push(Point::new(381.195313_f32, 332.532593_f32));
        poly.push(Point::new(384.464935_f32, 334.754700_f32));
        poly.push(Point::new(386.687042_f32, 338.024292_f32));
        poly.push(Point::new(387.427765_f32, 341.907532_f32));
        poly.push(Point::new(387.427765_f32, 422.832367_f32));
        poly.push(Point::new(386.687042_f32, 426.715576_f32));
        poly.push(Point::new(384.464935_f32, 429.985168_f32));
        poly.push(Point::new(381.195313_f32, 432.207275_f32));
        poly.push(Point::new(377.312134_f32, 432.947998_f32));
        poly.push(Point::new(342.289948_f32, 432.947998_f32));
        reporter_assert!(reporter, get_polygon_winding(&poly) > 0);
        reporter_assert!(reporter, is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // a star is born
        poly.clear();
        poly.push(Point::new(0.0_f32, -50.0_f32));
        poly.push(Point::new(14.43_f32, -25.0_f32));
        poly.push(Point::new(43.30_f32, -25.0_f32));
        poly.push(Point::new(28.86_f32, 0.0_f32));
        poly.push(Point::new(43.30_f32, 25.0_f32));
        poly.push(Point::new(14.43_f32, 25.0_f32));
        poly.push(Point::new(0.0_f32, 50.0_f32));
        poly.push(Point::new(-14.43_f32, 25.0_f32));
        poly.push(Point::new(-43.30_f32, 25.0_f32));
        poly.push(Point::new(-28.86_f32, 0.0_f32));
        poly.push(Point::new(-43.30_f32, -25.0_f32));
        poly.push(Point::new(-14.43_f32, -25.0_f32));
        reporter_assert!(reporter, get_polygon_winding(&poly) > 0);
        reporter_assert!(reporter, !is_convex_polygon(&poly));
        reporter_assert!(reporter, is_simple_polygon(&poly));
        triangle_indices.clear();
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // many spiked star
        {
            let c = int_to_scalar(45);
            let r1 = int_to_scalar(20);
            let r2 = int_to_scalar(3);
            let n: i32 = 500;
            poly.clear();
            let mut rad: scalar = 0.0;
            let drad: scalar = SCALAR_PI / int_to_scalar(n);
            for _ in 0..n {
                poly.push(Point::new(
                    c + scalar_cos(rad) * r1,
                    c + scalar_sin(rad) * r1,
                ));
                rad += drad;
                poly.push(Point::new(
                    c + scalar_cos(rad) * r2,
                    c + scalar_sin(rad) * r2,
                ));
                rad += drad;
            }
            reporter_assert!(reporter, get_polygon_winding(&poly) > 0);
            reporter_assert!(reporter, !is_convex_polygon(&poly));
            reporter_assert!(reporter, is_simple_polygon(&poly));
            triangle_indices.clear();
            reporter_assert!(
                reporter,
                triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
            );
        }

        // self-intersecting polygon
        poly.clear();
        poly.push(Point::new(0.0_f32, -50.0_f32));
        poly.push(Point::new(14.43_f32, -25.0_f32));
        poly.push(Point::new(43.30_f32, -25.0_f32));
        poly.push(Point::new(-28.86_f32, 0.0_f32));
        poly.push(Point::new(43.30_f32, 25.0_f32));
        poly.push(Point::new(14.43_f32, 25.0_f32));
        poly.push(Point::new(0.0_f32, 50.0_f32));
        poly.push(Point::new(-14.43_f32, 25.0_f32));
        poly.push(Point::new(-43.30_f32, 25.0_f32));
        poly.push(Point::new(28.86_f32, 0.0_f32));
        poly.push(Point::new(-43.30_f32, -25.0_f32));
        poly.push(Point::new(-14.43_f32, -25.0_f32));
        reporter_assert!(reporter, get_polygon_winding(&poly) > 0);
        reporter_assert!(reporter, !is_convex_polygon(&poly));
        reporter_assert!(reporter, !is_simple_polygon(&poly));
        triangle_indices.clear();
        // running this just to make sure it doesn't crash
        // the fact that it succeeds doesn't mean anything since the input is not simple
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // self-intersecting polygon with coincident point
        poly.clear();
        poly.push(Point::new(0.0_f32, 0.0_f32));
        poly.push(Point::new(-50.0, -50.0));
        poly.push(Point::new(50.0, -50.0));
        poly.push(Point::new(0.00000001_f32, -0.00000001_f32));
        poly.push(Point::new(-50.0, 50.0));
        poly.push(Point::new(50.0, 50.0));
        reporter_assert!(reporter, get_polygon_winding(&poly) == 0);
        reporter_assert!(reporter, !is_convex_polygon(&poly));
        reporter_assert!(reporter, !is_simple_polygon(&poly));
        triangle_indices.clear();
        // running this just to make sure it doesn't crash
        reporter_assert!(
            reporter,
            !triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // self-intersecting polygon with two equal edges
        poly.clear();
        poly.push(Point::new(0.0_f32, 0.0_f32));
        poly.push(Point::new(10.0, 0.0));
        poly.push(Point::new(0.0, 10.0));
        poly.push(Point::new(10.0, 10.0));
        poly.push(Point::new(10.0, 0.0));
        poly.push(Point::new(0.0, 10.0));
        reporter_assert!(reporter, get_polygon_winding(&poly) == 0);
        reporter_assert!(reporter, !is_convex_polygon(&poly));
        reporter_assert!(reporter, !is_simple_polygon(&poly));
        triangle_indices.clear();
        // running this just to make sure it doesn't crash
        reporter_assert!(
            reporter,
            !triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );

        // absurd self-intersecting polygon
        poly.clear();
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(
            3284.8125_f32,
            -10411310938997512334153865557442560.0000_f32,
        ));
        poly.push(Point::new(-32768.7500_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3172.8125_f32));
        poly.push(Point::new(0.0000_f32, -2147485952.0000_f32));
        poly.push(Point::new(0.0000_f32, 170.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(137.0000_f32, 4105.6875_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3283.0000_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 821.1250_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -30897.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.5625_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3526523879424.0000_f32, 0.0000_f32));
        poly.push(Point::new(-32768.9375_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(129.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 859832320.0000_f32));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 129.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-33554468.0000_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 219.0000_f32));
        poly.push(Point::new(3220.8125_f32, 0.0000_f32));
        poly.push(Point::new(-35840.0625_f32, 0.0000_f32));
        poly.push(Point::new(0.0000_f32, 3284.8125_f32));
        poly.push(Point::new(
            0.0000_f32,
            -41625560509365411790244566154608640.0000_f32,
        ));
        poly.push(Point::new(0.0000_f32, 215282736.0000_f32));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(
            -41625560509365411790244566154608640.0000_f32,
            0.0000_f32,
        ));
        poly.push(Point::new(215282736.0000_f32, 0.0000_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3156.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 129.0000_f32));
        poly.push(Point::new(7433.6875_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(59324728941049917997056.0000_f32, 0.0000_f32));
        poly.push(Point::new(-35840.0625_f32, 0.0000_f32));
        poly.push(Point::new(0.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(137.0000_f32, 4105.6875_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3283.0000_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.5625_f32));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(
            -44882437151680690189392273689542656.0000_f32,
            134217728.0000_f32,
        ));
        poly.push(Point::new(0.0000_f32, 0.0000_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 217055232.0000_f32));
        poly.push(Point::new(138.3125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.5625_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(2152988672.0000_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        poly.push(Point::new(138.0000_f32, 3284.8125_f32));
        poly.push(Point::new(0.0000_f32, -32768.0625_f32));
        poly.push(Point::new(0.0000_f32, 138.0000_f32));
        poly.push(Point::new(3284.8125_f32, 0.0000_f32));
        poly.push(Point::new(-32768.0625_f32, 0.0000_f32));
        reporter_assert!(reporter, get_polygon_winding(&poly) < 0);
        reporter_assert!(reporter, !is_convex_polygon(&poly));
        reporter_assert!(reporter, !is_simple_polygon(&poly));
        triangle_indices.clear();
        // running this just to make sure it doesn't crash
        reporter_assert!(
            reporter,
            triangulate_simple_polygon(&poly, &index_map, &mut triangle_indices)
        );
    }
);

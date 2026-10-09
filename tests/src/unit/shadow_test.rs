// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ShadowTest.cpp (chrome/m156)

#![cfg(test)]
// The degenerate-path literals are copied verbatim from the C++ test, which uses float literals;
// the `i as f32` loop counter mirrors its `(SkScalar)i`.
#![allow(clippy::excessive_precision, clippy::cast_precision_loss)]

use skia_rust_core::color::Color;
use skia_rust_core::draw_shadow_info::{DrawShadowRec, get_local_bounds};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::{Contains, Rect};
use skia_rust_core::rrect::RRect;
use skia_rust_core::shadow_tessellator::{make_ambient, make_spot};
use skia_rust_core::utils::shadow_utils::ShadowFlags;
use skia_rust_core::vertices::Vertices;

use crate::{Reporter, def_test, reporter_assert};

/// `ExpectVerts` of the C++ test.
// Port of: tests/ShadowTest.cpp#L19-L22 (chrome/m156)
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExpectVerts {
    Dont,
    Do,
}

/// `check_result`: the tessellation succeeded (or failed) as expected, and produced (or did not
/// produce) vertices as expected.
// Port of: tests/ShadowTest.cpp#L24-L40 (chrome/m156)
fn check_result(reporter: &mut Reporter, verts: Option<&Vertices>, expect_verts: ExpectVerts) {
    let expect_success = expect_verts == ExpectVerts::Do;
    reporter_assert!(reporter, expect_success == verts.is_some());
    if let Some(verts) = verts {
        if expect_verts == ExpectVerts::Dont && verts.vertex_count() > 0 {
            reporter_assert!(reporter, false);
        } else if expect_verts == ExpectVerts::Do && verts.vertex_count() == 0 {
            reporter_assert!(reporter, false);
        }
    }
}

/// `tessellate_shadow`: runs every tessellator entry point on `path` and checks each result.
// Port of: tests/ShadowTest.cpp#L42-L70 (chrome/m156)
fn tessellate_shadow(
    reporter: &mut Reporter,
    path: &Path,
    ctm: &Matrix,
    height_params: Point3,
    expect_verts: ExpectVerts,
) {
    let verts = make_ambient(path, ctm, height_params, true);
    check_result(reporter, verts.as_ref(), expect_verts);
    let verts = make_ambient(path, ctm, height_params, false);
    check_result(reporter, verts.as_ref(), expect_verts);
    let light = Point3::new(0.0, 0.0, 128.0);
    let verts = make_spot(path, ctm, height_params, light, 128.0, true, false);
    check_result(reporter, verts.as_ref(), expect_verts);
    let verts = make_spot(path, ctm, height_params, light, 128.0, false, false);
    check_result(reporter, verts.as_ref(), expect_verts);
    let verts = make_spot(path, ctm, height_params, light, 128.0, true, true);
    check_result(reporter, verts.as_ref(), expect_verts);
    let verts = make_spot(path, ctm, height_params, light, 128.0, false, true);
    check_result(reporter, verts.as_ref(), expect_verts);
}

def_test!(ShadowUtils, |reporter| {
    let ctm = Matrix::new_identity();
    let mut path = PathBuilder::new()
        .cubic_to((100.0, 50.0), (20.0, 100.0), (0.0, 0.0))
        .detach();
    tessellate_shadow(
        reporter,
        &path,
        &ctm,
        Point3::new(0.0, 0.0, 4.0),
        ExpectVerts::Do,
    );

    // super high path
    tessellate_shadow(
        reporter,
        &path,
        &ctm,
        Point3::new(0.0, 0.0, 4.0e+37),
        ExpectVerts::Do,
    );

    // This line segment has no area and no shadow.
    path = PathBuilder::new().line_to((10.0, 10.0)).detach();
    tessellate_shadow(
        reporter,
        &path,
        &ctm,
        Point3::new(0.0, 0.0, 4.0),
        ExpectVerts::Dont,
    );

    // A series of collinear line segments
    let mut builder = PathBuilder::new();
    for i in 0..10 {
        builder.line_to((i as f32, i as f32));
    }
    tessellate_shadow(
        reporter,
        &builder.detach(),
        &ctm,
        Point3::new(0.0, 0.0, 4.0),
        ExpectVerts::Dont,
    );

    // ugly degenerate path
    path = PathBuilder::new()
        .move_to((-134_217_728.0, 2.222_651_53e21))
        .cubic_to(
            (-2.333_261_06e21, 7.362_982_65e-41),
            (3.722_377_38e-22, 5.995_026_92e-36),
            (1.136_319_43e22, 2.089_078_6e33),
        )
        .cubic_to(
            (1.033_976_26e-25, 5.995_026_92e-36),
            (9.183_549_62e-41, 0.0),
            (4.614_274_5e-37, -213_558_848.0),
        )
        .line_to((-134_217_728.0, 2.221_651_5e21))
        .detach();
    tessellate_shadow(
        reporter,
        &path,
        &ctm,
        Point3::new(0.0, 0.0, 9.0),
        ExpectVerts::Dont,
    );

    // simple concave path (star of David)
    let star = PathBuilder::new()
        .move_to((0.0, -50.0))
        .line_to((14.43, -25.0))
        .line_to((43.30, -25.0))
        .line_to((28.86, 0.0))
        .line_to((43.30, 25.0))
        .line_to((14.43, 25.0))
        .line_to((0.0, 50.0))
        .line_to((-14.43, 25.0))
        .line_to((-43.30, 25.0))
        .line_to((-28.86, 0.0))
        .line_to((-43.30, -25.0))
        .line_to((-14.43, -25.0))
        .detach();
    // uncomment when transparent concave shadows are working
    //    tessellate_shadow(reporter, &star, &ctm, {0, 0, 9}, kDo_ExpectVerts, true);
    let _ = star;

    // complex concave path (bowtie)
    builder.reset();
    builder
        .move_to((-50.0, -50.0))
        .line_to((-50.0, 50.0))
        .line_to((50.0, -50.0))
        .line_to((50.0, 50.0))
        .line_to((-50.0, -50.0));
    tessellate_shadow(
        reporter,
        &builder.snapshot(),
        &ctm,
        Point3::new(0.0, 0.0, 9.0),
        ExpectVerts::Dont,
    );

    // multiple contour path
    builder
        .close()
        .move_to((0.0, 0.0))
        .line_to((1.0, 0.0))
        .line_to((0.0, 1.0));
    path = builder.detach();
    tessellate_shadow(
        reporter,
        &path,
        &ctm,
        Point3::new(0.0, 0.0, 9.0),
        ExpectVerts::Dont,
    );
});

/// `check_xformed_bounds`: every tessellated shadow lies inside the bounds reported by
/// `GetLocalBounds` mapped to device space.
// Port of: tests/ShadowTest.cpp#L142-L183 (chrome/m156)
fn check_xformed_bounds(reporter: &mut Reporter, path: &Path, ctm: &Matrix) {
    let mut rec = DrawShadowRec {
        z_plane_params: Point3::new(0.0, 0.0, 4.0),
        light_pos: Point3::new(100.0, 0.0, 600.0),
        light_radius: 800.0,
        ambient_color: Color::new(0x0800_0000),
        spot_color: Color::new(0x4000_0000),
        flags: 0,
    };

    // point light
    let mut bounds = get_local_bounds(path, &rec, ctm);
    bounds = ctm.map_rect(bounds).0;
    let verts = make_ambient(path, ctm, rec.z_plane_params, true);
    if let Some(verts) = verts {
        reporter_assert!(reporter, bounds.contains(verts.bounds()));
    }
    let map_xy = ctm.map_point(Point::new(rec.light_pos.x, rec.light_pos.y));
    let mut dev_light_pos = Point3::new(map_xy.x, map_xy.y, rec.light_pos.z);
    let verts = make_spot(
        path,
        ctm,
        rec.z_plane_params,
        dev_light_pos,
        rec.light_radius,
        false,
        false,
    );
    if let Some(verts) = verts {
        reporter_assert!(reporter, bounds.contains(verts.bounds()));
    }

    // directional light
    rec.flags |= ShadowFlags::DIRECTIONAL_LIGHT.bits();
    rec.light_radius = 2.0;
    bounds = get_local_bounds(path, &rec, ctm);
    bounds = ctm.map_rect(bounds).0;
    let verts = make_ambient(path, ctm, rec.z_plane_params, true);
    if let Some(verts) = verts {
        reporter_assert!(reporter, bounds.contains(verts.bounds()));
    }
    dev_light_pos = rec.light_pos;
    dev_light_pos.normalize();
    let verts = make_spot(
        path,
        ctm,
        rec.z_plane_params,
        dev_light_pos,
        rec.light_radius,
        false,
        true,
    );
    if let Some(verts) = verts {
        reporter_assert!(reporter, bounds.contains(verts.bounds()));
    }
}

/// `check_bounds`: runs [`check_xformed_bounds`] under a set of transforms.
// Port of: tests/ShadowTest.cpp#L185-L211 (chrome/m156)
fn check_bounds(reporter: &mut Reporter, path: &Path) {
    let fixed_shadows_in_perspective = false; // skbug.com/40041026
    let mut ctm = Matrix::new_identity();
    ctm.set_translate((100.0, 100.0));
    check_xformed_bounds(reporter, path, &ctm);
    ctm.post_scale((2.0, 2.0), None);
    check_xformed_bounds(reporter, path, &ctm);
    ctm.pre_rotate(45.0, None);
    check_xformed_bounds(reporter, path, &ctm);
    ctm.pre_skew((40.0, -20.0), None);
    check_xformed_bounds(reporter, path, &ctm);
    if fixed_shadows_in_perspective {
        ctm.set_persp_x(0.0001);
        ctm.set_persp_y(12.0);
        check_xformed_bounds(reporter, path, &ctm);
        ctm.set_persp_x(0.0001);
        ctm.set_persp_y(-12.0);
        check_xformed_bounds(reporter, path, &ctm);
        ctm.set_persp_x(12.0);
        ctm.set_persp_y(0.0001);
        check_xformed_bounds(reporter, path, &ctm);
    }
}

def_test!(ShadowBounds, |reporter| {
    let mut path = Path::rrect(
        RRect::new_rect_xy(Rect::new(-50.0, -20.0, 40.0, 30.0), 4.0, 4.0),
        None,
    );
    check_bounds(reporter, &path);

    path = Path::oval(Rect::new(300.0, 300.0, 900.0, 900.0), None);
    check_bounds(reporter, &path);

    path = PathBuilder::new()
        .cubic_to((100.0, 50.0), (20.0, 100.0), (0.0, 0.0))
        .detach();
    check_bounds(reporter, &path);
});

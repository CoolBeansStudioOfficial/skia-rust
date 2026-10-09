// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/shadowutils.cpp (chrome/m156)

// The `as` casts convert small loop indices, colours and byte channels exactly as the C++ does.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use crate::prelude::*;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color4f;
use skia_rust_core::gaussian_color_filter::make_gaussian;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::shadow_utils::{self as sh, ShadowFlags};
use skia_rust_effects::gradient_shader::radial;

/// `SkColorSetARGB(a, r, g, b)` with the float alpha of the C++ call sites (truncated to a byte).
fn argb(a: f32, r: u8, g: u8, b: u8) -> Color {
    Color::from_argb(a as u8, r, g, b)
}

/// Draws one shadow of `path` (the helper at the top of `gm/shadowutils.cpp`).
// Port of: gm/shadowutils.cpp#L17-L31 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ helper's signature
fn draw_shadow(
    canvas: &Canvas,
    path: &Path,
    height: f32,
    color: Color,
    light_pos: Point3,
    light_r: f32,
    is_ambient: bool,
    flags: ShadowFlags,
) {
    let ambient_alpha: f32 = if is_ambient { 0.5 } else { 0.0 };
    let spot_alpha: f32 = if is_ambient { 0.0 } else { 0.5 };
    let a = u32::from(color) >> 24;
    let r = ((u32::from(color) >> 16) & 0xFF) as u8;
    let g = ((u32::from(color) >> 8) & 0xFF) as u8;
    let b = (u32::from(color) & 0xFF) as u8;
    let ambient_color = argb(ambient_alpha * a as f32, r, g, b);
    let spot_color = argb(spot_alpha * a as f32, r, g, b);
    sh::draw_shadow(
        canvas,
        path,
        Point3::new(0.0, 0.0, height),
        light_pos,
        light_r,
        ambient_color,
        spot_color,
        flags,
    );
}

const W: i32 = 800;
const K_PAD: f32 = 15.0;
const K_LIGHT_R: f32 = 100.0;
const K_HEIGHT: f32 = 50.0;

/// `ShadowMode` of the C++ GM.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShadowMode {
    DebugColorNoOccluders,
    DebugColorOccluders,
    Grayscale,
}

/// Draws the shadow grid used by the four `shadow_utils*` GMs (`draw_paths`).
// Port of: gm/shadowutils.cpp#L49-L226 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the length of the C++ draw_paths
fn draw_paths(canvas: &Canvas, mode: ShadowMode) {
    let mut paths: Vec<Path> = Vec::new();
    paths.push(Path::rrect_xy(
        Rect::new(0.0, 0.0, 50.0, 50.0),
        10.0,
        10.00002,
        None,
    ));
    let mut odd_rrect = RRect::default();
    odd_rrect.set_nine_patch(Rect::new(0.0, 0.0, 50.0, 50.0), 9.0, 13.0, 6.0, 16.0);
    paths.push(Path::rrect(odd_rrect, None));
    paths.push(Path::rect(Rect::new(0.0, 0.0, 50.0, 50.0), None));
    paths.push(Path::circle((25.0, 25.0), 25.0, None));
    paths.push(
        PathBuilder::new()
            .cubic_to((100.0, 50.0), (20.0, 100.0), (0.0, 0.0))
            .detach(),
    );
    paths.push(Path::oval(Rect::new(0.0, 0.0, 20.0, 60.0), None));

    // star
    let star = PathBuilder::new()
        .move_to((0.0, -33.3333))
        .line_to((9.62, -16.6667))
        .line_to((28.867, -16.6667))
        .line_to((19.24, 0.0))
        .line_to((28.867, 16.6667))
        .line_to((9.62, 16.6667))
        .line_to((0.0, 33.3333))
        .line_to((-9.62, 16.6667))
        .line_to((-28.867, 16.6667))
        .line_to((-19.24, 0.0))
        .line_to((-28.867, -16.6667))
        .line_to((-9.62, -16.6667))
        .close()
        .detach();
    // dumbbell
    let dumbbell = PathBuilder::new()
        .move_to((50.0, 0.0))
        .cubic_to((100.0, 25.0), (60.0, 50.0), (50.0, 0.0))
        .cubic_to((0.0, -25.0), (40.0, -50.0), (50.0, 0.0))
        .detach();
    let concave_paths: Vec<Path> = vec![star, dumbbell];

    // transform light position relative to canvas to handle tiling
    let light_xy = canvas.total_matrix().map_point(Point::new(250.0, 400.0));
    let light_pos = Point3::new(light_xy.x, light_xy.y, 500.0);

    canvas.translate((3.0 * K_PAD, 3.0 * K_PAD));
    canvas.save();
    let mut x: f32 = 0.0;
    let mut dy: f32 = 0.0;
    let mut matrices: Vec<Matrix> = Vec::new();
    matrices.push(Matrix::default());
    {
        let mut m = Matrix::default();
        m.set_rotate(33.0, Point::new(25.0, 25.0))
            .post_scale((1.2, 0.8), Point::new(25.0, 25.0));
        matrices.push(m);
    }
    for m in &matrices {
        for flags in [ShadowFlags::NONE, ShadowFlags::TRANSPARENT_OCCLUDER] {
            for (path_counter, path) in paths.iter().enumerate() {
                let post_m_bounds = m.map_rect(path.bounds()).0;
                let w = post_m_bounds.width() + K_HEIGHT;
                let dx = w + K_PAD;
                if x + dx > (W as f32) - 3.0 * K_PAD {
                    canvas.restore();
                    canvas.translate((0.0, dy));
                    canvas.save();
                    x = 0.0;
                    dy = 0.0;
                }

                canvas.save();
                canvas.concat(m);
                // flip a couple of paths to test 180° rotation
                let flipped = flags == ShadowFlags::TRANSPARENT_OCCLUDER && path_counter % 3 == 0;
                if flipped {
                    canvas.save();
                    canvas.rotate(180.0, Some(Point::new(25.0, 25.0)));
                }

                if mode == ShadowMode::DebugColorNoOccluders
                    || mode == ShadowMode::DebugColorOccluders
                {
                    draw_shadow(
                        canvas,
                        path,
                        K_HEIGHT,
                        Color::RED,
                        light_pos,
                        K_LIGHT_R,
                        true,
                        flags,
                    );
                    draw_shadow(
                        canvas,
                        path,
                        K_HEIGHT,
                        Color::BLUE,
                        light_pos,
                        K_LIGHT_R,
                        false,
                        flags,
                    );
                } else if mode == ShadowMode::Grayscale {
                    let ambient_color = argb(0.1 * 255.0, 0, 0, 0);
                    let spot_color = argb(0.25 * 255.0, 0, 0, 0);
                    sh::draw_shadow(
                        canvas,
                        path,
                        Point3::new(0.0, 0.0, K_HEIGHT),
                        light_pos,
                        K_LIGHT_R,
                        ambient_color,
                        spot_color,
                        flags,
                    );
                }

                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                if mode == ShadowMode::DebugColorNoOccluders {
                    // Draw the path outline in green on top of the ambient and spot shadows.
                    if flags.contains(ShadowFlags::TRANSPARENT_OCCLUDER) {
                        paint.set_color(Color::CYAN);
                    } else {
                        paint.set_color(Color::GREEN);
                    }
                    paint.set_style(Style::Stroke);
                    paint.set_stroke_width(0.0);
                } else {
                    paint.set_color(if mode == ShadowMode::DebugColorOccluders {
                        Color::LIGHT_GRAY
                    } else {
                        Color::WHITE
                    });
                    if flags.contains(ShadowFlags::TRANSPARENT_OCCLUDER) {
                        paint.set_alpha_f(0.5);
                    }
                    paint.set_style(Style::Fill);
                }
                canvas.draw_path(path, &paint);

                if flipped {
                    canvas.restore();
                }
                canvas.restore();
                canvas.translate((dx, 0.0));
                x += dx;
                let candidate = post_m_bounds.height() + K_PAD + K_HEIGHT;
                dy = if dy < candidate { candidate } else { dy };
            }
        }
    }

    // concave paths
    canvas.restore();
    canvas.translate((K_PAD, dy));
    canvas.save();
    dy = 0.0;
    for m in &matrices {
        // for the concave paths we are not clipping, so transparent and opaque are the same
        for path in &concave_paths {
            let post_m_bounds = m.map_rect(path.bounds()).0;
            let w = post_m_bounds.width() + K_HEIGHT;
            let dx = w + K_PAD;
            canvas.save();
            canvas.concat(m);
            if mode == ShadowMode::DebugColorNoOccluders || mode == ShadowMode::DebugColorOccluders
            {
                draw_shadow(
                    canvas,
                    path,
                    K_HEIGHT,
                    Color::RED,
                    light_pos,
                    K_LIGHT_R,
                    true,
                    ShadowFlags::NONE,
                );
                draw_shadow(
                    canvas,
                    path,
                    K_HEIGHT,
                    Color::BLUE,
                    light_pos,
                    K_LIGHT_R,
                    false,
                    ShadowFlags::NONE,
                );
            } else if mode == ShadowMode::Grayscale {
                let ambient_color = argb(0.1 * 255.0, 0, 0, 0);
                let spot_color = argb(0.25 * 255.0, 0, 0, 0);
                sh::draw_shadow(
                    canvas,
                    path,
                    Point3::new(0.0, 0.0, K_HEIGHT),
                    light_pos,
                    K_LIGHT_R,
                    ambient_color,
                    spot_color,
                    ShadowFlags::NONE,
                );
            }

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            if mode == ShadowMode::DebugColorNoOccluders {
                // Draw the path outline in green on top of the ambient and spot shadows.
                paint.set_color(Color::GREEN);
                paint.set_style(Style::Stroke);
                paint.set_stroke_width(0.0);
            } else {
                paint.set_color(if mode == ShadowMode::DebugColorOccluders {
                    Color::LIGHT_GRAY
                } else {
                    Color::WHITE
                });
                paint.set_style(Style::Fill);
            }
            canvas.draw_path(path, &paint);
            canvas.restore();
            canvas.translate((dx, 0.0));
            let candidate = post_m_bounds.height() + K_PAD + K_HEIGHT;
            dy = if dy < candidate { candidate } else { dy };
        }
    }

    // Show where the light is in x,y as a circle (specified in device space).
    if let Some(inv_canvas_m) = canvas.total_matrix().invert() {
        canvas.save();
        canvas.concat(&inv_canvas_m);
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.set_anti_alias(true);
        canvas.draw_circle((light_pos.x, light_pos.y), K_LIGHT_R / 10.0, &paint);
        canvas.restore();
    }
}

crate::def_simple_gm!(#[ignore = "see notes/gm_shadowutils_cpp-shadow_utils.md"] shadow_utils, canvas, 800, 960, {
    draw_paths(canvas, ShadowMode::DebugColorNoOccluders);
});

crate::def_simple_gm!(#[ignore = "see notes/gm_shadowutils_cpp-shadow_utils.md"] shadow_utils_occl, canvas, 800, 960, {
    draw_paths(canvas, ShadowMode::DebugColorOccluders);
});

crate::def_simple_gm!(#[ignore = "see notes/gm_shadowutils_cpp-shadow_utils.md"] shadow_utils_gray, canvas, 800, 960, {
    draw_paths(canvas, ShadowMode::Grayscale);
});

// Port of: gm/shadowutils.cpp#L245-L264 (chrome/m156)
crate::def_simple_gm!(shadow_utils_gaussian_colorfilter, canvas, 512, 256, {
    let r = Rect::new(0.0, 0.0, 256.0, 256.0);
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 0.0),
        Color4f::new(0.0, 0.0, 0.0, 1.0),
    ];
    let sh = radial(
        r.center(),
        r.width(),
        (&colors[..], None),
        None,
        TileMode::Clamp,
        None,
        None,
    );
    let mut red_paint = Paint::default();
    red_paint.set_color(Color::RED);
    let mut paint = Paint::default();
    paint.set_shader(sh);
    canvas.draw_rect(r, &red_paint);
    canvas.draw_rect(r, &paint);
    canvas.translate((256.0, 0.0));
    paint.set_color_filter(make_gaussian());
    canvas.draw_rect(r, &red_paint);
    canvas.draw_rect(r, &paint);
});

// Port of: gm/shadowutils.cpp#L266-L337 (chrome/m156)
crate::def_simple_gm!(#[ignore = "see notes/gm_shadowutils_cpp-shadow_utils.md"] shadow_utils_directional, canvas, 256, 384, {
    const K_LIGHT_R_DIR: f32 = 1.0;
    const K_HEIGHT_DIR: f32 = 12.0;
    let rrect = Path::rrect(
        RRect::new_rect_xy(Rect::new(-25.0, -25.0, 25.0, 25.0), 10.0, 10.0),
        None,
    );
    #[allow(clippy::excessive_precision)] // the C++ float literal, kept verbatim
    let light_pos = Point3::new(-45.0, -45.0, 77.942_286_34);
    let ambient_color = argb(0.02 * 255.0, 0, 0, 0);
    let spot_color = argb(0.35 * 255.0, 0, 0, 0);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::WHITE);
    paint.set_style(Style::Fill);

    // translation
    canvas.save();
    canvas.translate((35.0, 35.0));
    for _ in 0..3 {
        sh::draw_shadow(
            canvas,
            &rrect,
            Point3::new(0.0, 0.0, K_HEIGHT_DIR),
            light_pos,
            K_LIGHT_R_DIR,
            ambient_color,
            spot_color,
            ShadowFlags::DIRECTIONAL_LIGHT,
        );
        canvas.draw_path(&rrect, &paint);
        canvas.translate((80.0, 0.0));
    }
    canvas.restore();

    // rotation
    for i in 0..3 {
        canvas.save();
        canvas.translate((35.0 + 80.0 * i as f32, 105.0));
        canvas.rotate(20.0 * (i + 1) as f32, None);
        sh::draw_shadow(
            canvas,
            &rrect,
            Point3::new(0.0, 0.0, K_HEIGHT_DIR),
            light_pos,
            K_LIGHT_R_DIR,
            ambient_color,
            spot_color,
            ShadowFlags::DIRECTIONAL_LIGHT,
        );
        canvas.draw_path(&rrect, &paint);
        canvas.restore();
    }

    // scale
    for i in 0..3 {
        canvas.save();
        let scale_factor = 2.0f32.powi(-i);
        canvas.translate((35.0 + 80.0 * i as f32, 185.0));
        canvas.scale((scale_factor, scale_factor));
        sh::draw_shadow(
            canvas,
            &rrect,
            Point3::new(0.0, 0.0, K_HEIGHT_DIR),
            light_pos,
            K_LIGHT_R_DIR,
            ambient_color,
            spot_color,
            ShadowFlags::DIRECTIONAL_LIGHT,
        );
        canvas.draw_path(&rrect, &paint);
        canvas.restore();
    }

    // perspective
    for i in 0..3 {
        canvas.save();
        let mut mat = Matrix::default();
        mat.set_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.005, 1.005);
        canvas.translate((35.0 + 80.0 * i as f32, 265.0));
        canvas.concat(&mat);
        sh::draw_shadow(
            canvas,
            &rrect,
            Point3::new(0.0, 0.0, K_HEIGHT_DIR),
            light_pos,
            K_LIGHT_R_DIR,
            ambient_color,
            spot_color,
            ShadowFlags::DIRECTIONAL_LIGHT,
        );
        canvas.draw_path(&rrect, &paint);
        canvas.restore();
    }
});

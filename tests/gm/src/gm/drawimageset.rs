// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawimageset.cpp (chrome/m156)

// Casts and names mirror the C++ arithmetic and loop counters this file ports, so they stay
// as written in the source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names
)]

use crate::prelude::*;
use crate::tool_utils::draw_checkerboard;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::{ImageSetEntry, PointMode, QuadAAFlags, SrcRectConstraint};
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::surfaces;

const STRIPE_W: scalar = 10.0;
const STRIPE_SPACING: scalar = 30.0;

/// `SkScalar(v)` for the integer constants the C++ multiplies with.
fn sc(v: i32) -> scalar {
    v as scalar
}

/// The rect of `SkRect::MakeWH(w, h).makeInset(dx, dy)`.
fn inset_wh(w: scalar, h: scalar, dx: scalar, dy: scalar) -> Rect {
    let mut r = Rect::from_wh(w, h);
    r.inset((dx, dy));
    r
}

/// Makes a set of m x n tiled images to be drawn with `experimental_draw_edge_aa_image_set`.
// Port of: gm/drawimageset.cpp#L38-L106 (chrome/m156)
fn make_image_tiles(
    tile_w: i32,
    tile_h: i32,
    m: i32,
    n: i32,
    colors: &[Color4f; 4],
    bg_color: Color,
) -> Vec<ImageSetEntry> {
    let w = tile_w * m;
    let h = tile_h * n;
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surf = surfaces::raster(&info, None, None).expect("a raster surface");
    {
        let canvas = surf.canvas();
        canvas.clear(bg_color);

        let mut paint = Paint::default();
        let pts1 = [Point::new(0.0, 0.0), Point::new(sc(w), sc(h))];
        let grad = shaders::linear_gradient(
            (pts1[0], pts1[1]),
            &Gradient::new(
                Colors::new(&colors[0..2], None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
        paint.set_shader(grad);
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(STRIPE_W);
        let mut stripe_pts = [
            Point::new(-sc(w) - STRIPE_W, -STRIPE_W),
            Point::new(STRIPE_W, sc(h) + STRIPE_W),
        ];
        while stripe_pts[0].x <= sc(w) {
            canvas.draw_points(PointMode::Lines, &stripe_pts, &paint);
            stripe_pts[0].x += STRIPE_SPACING;
            stripe_pts[1].x += STRIPE_SPACING;
        }

        let pts2 = [Point::new(0.0, sc(h)), Point::new(sc(w), 0.0)];
        let grad = shaders::linear_gradient(
            (pts2[0], pts2[1]),
            &Gradient::new(
                Colors::new(&colors[2..4], None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
        paint.set_shader(grad);
        paint.set_blend_mode(BlendMode::Multiply);
        stripe_pts[0] = Point::new(-sc(w) - STRIPE_W, sc(h) + STRIPE_W);
        stripe_pts[1] = Point::new(STRIPE_W, -STRIPE_W);
        while stripe_pts[0].x <= sc(w) {
            canvas.draw_points(PointMode::Lines, &stripe_pts, &paint);
            stripe_pts[0].x += STRIPE_SPACING;
            stripe_pts[1].x += STRIPE_SPACING;
        }
    }
    let full_image = surf.image_snapshot().expect("a snapshot of the tiles");

    let mut set = Vec::with_capacity((m * n) as usize);
    for y in 0..n {
        for x in 0..m {
            // Images will have 1 pixel of overlap at interior seams for filtering continuity.
            let mut subset =
                IRect::from_xywh(x * tile_w - 1, y * tile_h - 1, tile_w + 2, tile_h + 2);
            let mut aa_flags = QuadAAFlags::NONE;
            if x == 0 {
                subset.left = 0;
                aa_flags |= QuadAAFlags::LEFT;
            }
            if x == m - 1 {
                subset.right = w;
                aa_flags |= QuadAAFlags::RIGHT;
            }
            if y == 0 {
                subset.top = 0;
                aa_flags |= QuadAAFlags::TOP;
            }
            if y == n - 1 {
                subset.bottom = h;
                aa_flags |= QuadAAFlags::BOTTOM;
            }
            let image = full_image
                .make_subset(subset, RequiredProperties::default())
                .expect("a subset of the tiles");
            let src_rect = Rect::from_xywh(
                if x == 0 { 0.0 } else { 1.0 },
                if y == 0 { 0.0 } else { 1.0 },
                sc(tile_w),
                sc(tile_h),
            );
            let dst_rect = Rect::from_xywh(sc(x * tile_w), sc(y * tile_h), sc(tile_w), sc(tile_h));
            set.push(ImageSetEntry::new_simple(
                image, src_rect, dst_rect, 1.0, aa_flags,
            ));
        }
    }
    set
}

// `DrawImageSetGM`: the set drawn with rotations, perspective, skew and nearest/linear sampling,
// plus an entry with an unusual blend mode, mixed AA flags and alpha, and a color filter.
const DIS_M: i32 = 4;
const DIS_N: i32 = 3;
const DIS_TILE_W: i32 = 30;
const DIS_TILE_H: i32 = 60;

#[derive(Default)]
struct DrawImageSetGm {
    set: Vec<ImageSetEntry>,
}

impl GM for DrawImageSetGm {
    fn name(&self) -> String {
        "draw_image_set".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1000, 725)
    }

    // Port of: gm/drawimageset.cpp#L114-L119 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let colors = [
            Color4f::new(0.0, 1.0, 1.0, 1.0), // kCyan
            Color4f::new(0.0, 0.0, 0.0, 1.0), // kBlack
            Color4f::new(1.0, 0.0, 1.0, 1.0), // kMagenta
            Color4f::new(0.0, 0.0, 0.0, 1.0), // kBlack
        ];
        self.set = make_image_tiles(
            DIS_TILE_W,
            DIS_TILE_H,
            DIS_M,
            DIS_N,
            &colors,
            Color::from_argb(0xFF, 0xCC, 0xCC, 0xCC), /* SK_ColorLTGRAY */
        );
    }

    // Port of: gm/drawimageset.cpp#L120-L211 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let d = Point::new(sc(DIS_M * DIS_TILE_W), sc(DIS_N * DIS_TILE_H)).length();
        let mut matrices = [(); 4].map(|()| Matrix::new_identity());
        // rotation
        matrices[0].set_rotate(30.0, None);
        matrices[0].post_translate((d / 3.0, 0.0));
        // perspective
        let src = Rect::from_wh(sc(DIS_M * DIS_TILE_W), sc(DIS_N * DIS_TILE_H)).to_quad(None);
        let mut dst = [
            Point::new(0.0, 0.0),
            Point::new(sc(DIS_M * DIS_TILE_W) + 10.0, -5.0),
            Point::new(sc(DIS_M * DIS_TILE_W) - 28.0, sc(DIS_N * DIS_TILE_H) + 40.0),
            Point::new(45.0, sc(DIS_N * DIS_TILE_H) - 25.0),
        ];
        assert!(matrices[1].set_poly_to_poly(&src, &dst));
        matrices[1].post_translate((d, 50.0));

        // skew
        matrices[2].set_rotate(-60.0, None);
        matrices[2].post_skew((0.5, -1.15), None);
        matrices[2].post_scale((0.6, 1.05), None);
        matrices[2].post_translate((d, 2.6 * d));

        // perspective + mirror in x.
        dst[1] = Point::new(-0.25 * sc(DIS_M * DIS_TILE_W), 0.0);
        dst[0] = Point::new(5.0 / 4.0 * sc(DIS_M * DIS_TILE_W), 0.0);
        dst[3] = Point::new(
            2.0 / 3.0 * sc(DIS_M * DIS_TILE_W),
            1.0 / 2.0 * sc(DIS_N * DIS_TILE_H),
        );
        dst[2] = Point::new(
            1.0 / 3.0 * sc(DIS_M * DIS_TILE_W),
            1.0 / 2.0 * sc(DIS_N * DIS_TILE_H) - 0.1 * sc(DIS_TILE_H),
        );
        assert!(matrices[3].set_poly_to_poly(&src, &dst));
        matrices[3].post_translate((100.0, d));

        for filter in [FilterMode::Nearest, FilterMode::Linear] {
            let mut set_paint = Paint::default();
            set_paint.set_blend_mode(BlendMode::SrcOver);
            let sampling = SamplingOptions::new(filter, MipmapMode::None);

            for matrix in &matrices {
                // Draw grid of red lines at interior tile boundaries.
                const LINE_OUTSET: scalar = 10.0;
                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                paint.set_color(Color::RED);
                paint.set_style(Style::Stroke);
                paint.set_stroke_width(0.0);
                for x in 1..DIS_M {
                    let mut pts = [
                        Point::new(sc(x * DIS_TILE_W), 0.0),
                        Point::new(sc(x * DIS_TILE_W), sc(DIS_N * DIS_TILE_H)),
                    ];
                    matrix.map_points_inplace(&mut pts);
                    let mut v = pts[1] - pts[0];
                    v.set_length(v.length() + LINE_OUTSET);
                    canvas.draw_line(pts[1] - v, pts[0] + v, &paint);
                }
                for y in 1..DIS_N {
                    let mut pts = [
                        Point::new(0.0, sc(y * DIS_TILE_H)),
                        Point::new(sc(DIS_TILE_W * DIS_M), sc(y * DIS_TILE_H)),
                    ];
                    matrix.map_points_inplace(&mut pts);
                    let mut v = pts[1] - pts[0];
                    v.set_length(v.length() + LINE_OUTSET);
                    canvas.draw_line(pts[1] - v, pts[0] + v, &paint);
                }
                canvas.save();
                canvas.concat(matrix);
                canvas.experimental_draw_edge_aa_image_set(
                    &self.set,
                    &[],
                    &[],
                    sampling,
                    Some(&set_paint),
                    SrcRectConstraint::Fast,
                );
                canvas.restore();
            }

            // A more exotic case with an unusual blend mode, mixed aa flags set, and alpha,
            // subsets the image. And another with all the above plus a color filter.
            let entry = ImageSetEntry::new(
                self.set[0].image.clone(),
                inset_wh(
                    sc(DIS_TILE_W),
                    sc(DIS_TILE_H),
                    sc(DIS_TILE_W) / 4.0,
                    sc(DIS_TILE_H) / 4.0,
                ),
                Rect::from_wh(1.5 * sc(DIS_TILE_W), 1.5 * sc(DIS_TILE_H))
                    .with_offset((d / 4.0, 2.0 * d)),
                None,
                0.7,
                QuadAAFlags::LEFT | QuadAAFlags::TOP,
                false,
            );
            canvas.save();
            canvas.rotate(3.0, None);
            set_paint.set_blend_mode(BlendMode::Exclusion);
            canvas.experimental_draw_edge_aa_image_set(
                std::slice::from_ref(&entry),
                &[],
                &[],
                sampling,
                Some(&set_paint),
                SrcRectConstraint::Fast,
            );
            canvas.translate((entry.dst_rect.width() as scalar + 8.0, 0.0));
            let mut cf_paint = set_paint.clone();
            cf_paint.set_color_filter(color_filters::linear_to_srgb_gamma());
            canvas.experimental_draw_edge_aa_image_set(
                std::slice::from_ref(&entry),
                &[],
                &[],
                sampling,
                Some(&cf_paint),
                SrcRectConstraint::Fast,
            );
            canvas.restore();
            canvas.translate((2.0 * d, 0.0));
        }
    }
}

// `DrawImageSetRectToRectGM`: rect-stays-rect matrices, to test that filtering and antialiasing
// are not incorrectly disabled.
const RTR_M: i32 = 2;
const RTR_N: i32 = 2;
const RTR_TILE_W: i32 = 40;
const RTR_TILE_H: i32 = 50;

#[derive(Default)]
struct DrawImageSetRectToRectGm {
    set: Vec<ImageSetEntry>,
}

impl GM for DrawImageSetRectToRectGm {
    fn name(&self) -> String {
        "draw_image_set_rect_to_rect".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1250, 850)
    }

    // Port of: gm/drawimageset.cpp#L217-L222 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let colors = [
            Color4f::new(0.0, 0.0, 1.0, 1.0), // kBlue
            Color4f::new(1.0, 1.0, 1.0, 1.0), // kWhite
            Color4f::new(1.0, 0.0, 0.0, 1.0), // kRed
            Color4f::new(1.0, 1.0, 1.0, 1.0), // kWhite
        ];
        self.set = make_image_tiles(
            RTR_TILE_W,
            RTR_TILE_H,
            RTR_M,
            RTR_N,
            &colors,
            Color::from_argb(0xFF, 0xCC, 0xCC, 0xCC), /* SK_ColorLTGRAY */
        );
    }

    // Port of: gm/drawimageset.cpp#L223-L296 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        draw_checkerboard(canvas, Color::BLACK, Color::WHITE, 50);
        let w = sc(RTR_M * RTR_TILE_W);
        let h = sc(RTR_N * RTR_TILE_H);
        let mut matrices = [(); 5].map(|()| Matrix::new_identity());
        // Identity
        matrices[0].reset();
        // 90 degree rotation
        matrices[1].set_rotate(90.0, Point::new(w / 2.0, h / 2.0));
        // Scaling
        matrices[2].set_scale((2.0, 0.5), None);
        // Mirror in x and y
        matrices[3].set_scale((-1.0, -1.0), None);
        matrices[3].post_translate((w, h));
        // Mirror in y, rotate, and scale.
        matrices[4].set_scale((1.0, -1.0), None);
        matrices[4].post_translate((0.0, h));
        matrices[4].post_rotate(90.0, Point::new(w / 2.0, h / 2.0));
        matrices[4].post_scale((2.0, 0.5), None);

        let paint = Paint::default();
        let k_translate = w.max(h) * 2.0 + 10.0;
        canvas.translate((5.0, 5.0));
        canvas.save();
        for frac in [0.0_f32, 0.5] {
            canvas.save();
            canvas.translate((frac, frac));
            for matrix in &matrices {
                canvas.save();
                canvas.concat(matrix);
                canvas.experimental_draw_edge_aa_image_set(
                    &self.set,
                    &[],
                    &[],
                    SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    Some(&paint),
                    SrcRectConstraint::Fast,
                );
                canvas.restore();
                canvas.translate((k_translate, 0.0));
            }
            canvas.restore();
            canvas.restore();
            canvas.translate((0.0, k_translate));
            canvas.save();
        }
        for scale in [Point::new(2.0, 0.5), Point::new(0.5, 2.0)] {
            let scaled_set: Vec<ImageSetEntry> = self
                .set
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let mut entry = entry.clone();
                    entry.dst_rect = Rect::new(
                        entry.dst_rect.left * scale.x,
                        entry.dst_rect.top * scale.y,
                        entry.dst_rect.right * scale.x,
                        entry.dst_rect.bottom * scale.y,
                    );
                    entry.alpha = if i % 3 == 0 { 0.4 } else { 1.0 };
                    entry
                })
                .collect();
            for matrix in &matrices {
                canvas.save();
                canvas.concat(matrix);
                canvas.experimental_draw_edge_aa_image_set(
                    &scaled_set,
                    &[],
                    &[],
                    SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    Some(&paint),
                    SrcRectConstraint::Fast,
                );
                canvas.restore();
                canvas.translate((k_translate, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, k_translate));
            canvas.save();
        }
    }
}

// `DrawImageSetAlphaOnlyGM`: alpha-only and color textures combined with the paint's color.
const AO_M: i32 = 4;
const AO_N: i32 = 4;
const AO_TILE_W: i32 = 50;
const AO_TILE_H: i32 = 50;

#[derive(Default)]
struct DrawImageSetAlphaOnlyGm {
    set: Vec<ImageSetEntry>,
}

impl GM for DrawImageSetAlphaOnlyGm {
    fn name(&self) -> String {
        "draw_image_set_alpha_only".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(AO_M * AO_TILE_W, 2 * AO_N * AO_TILE_H)
    }

    // Port of: gm/drawimageset.cpp#L303-L325 (chrome/m156)
    fn on_gpu_setup(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        // There is no recorder on the raster backend, so the conversion runs on the CPU.
        let colors = [
            Color4f::new(0.0, 0.0, 1.0, 1.0), // kBlue
            Color4f::new(0.0, 0.0, 0.0, 0.0), // kTransparent
            Color4f::new(1.0, 0.0, 0.0, 1.0), // kRed
            Color4f::new(0.0, 0.0, 0.0, 0.0), // kTransparent
        ];
        let bg = Color::from_argb(128, 128, 128, 128);
        self.set = make_image_tiles(AO_TILE_W, AO_TILE_H, AO_M, AO_N, &colors, bg);

        // Modify the alpha of the entries, decreasing by column, and convert even rows to
        // alpha-only textures.
        let alpha_space = ColorSpace::new_srgb();
        for y in 0..AO_N {
            for x in 0..AO_M {
                let i = (y * AO_M + x) as usize;
                self.set[i].alpha = (AO_M - x) as f32 / AO_M as f32;
                if y % 2 == 0 {
                    self.set[i].image = self.set[i]
                        .image
                        .make_color_type_and_color_space(
                            ColorType::Alpha8,
                            alpha_space.clone(),
                            RequiredProperties::default(),
                        )
                        .expect("an alpha-only copy of the tile");
                }
            }
        }
        DrawResult::Ok
    }

    // Port of: gm/drawimageset.cpp#L326-L360 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        draw_checkerboard(
            canvas,
            Color::GRAY,
            Color::from_argb(0xFF, 0x44, 0x44, 0x44), /* SK_ColorDKGRAY */
            25,
        );
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::SrcOver);
        // colorizes even rows, no effect on odd rows
        paint.set_color4f(Color4f::new(0.2, 0.8, 0.4, 1.0), None);
        // Top rows use experimental edge set API
        canvas.experimental_draw_edge_aa_image_set(
            &self.set,
            &[],
            &[],
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            Some(&paint),
            SrcRectConstraint::Fast,
        );
        canvas.translate((0.0, sc(AO_N * AO_TILE_H)));
        // Bottom rows draw each image from the set using the regular API
        for y in 0..AO_N {
            for x in 0..AO_M {
                let entry = &self.set[(y * AO_M + x) as usize];
                let mut entry_paint = paint.clone();
                entry_paint.set_alpha_f(entry.alpha * paint.alpha_f());
                // ToolUtils::MakeTextureImage returns the image itself on a raster canvas.
                canvas.draw_image_rect(
                    &entry.image,
                    Some((&entry.src_rect, SrcRectConstraint::Fast)),
                    entry.dst_rect,
                    &entry_paint,
                );
            }
        }
    }
}

// Port of: gm/drawimageset.cpp#L362-L362 (chrome/m156)
crate::def_gm!(
    DrawImageSetGM_ = "DrawImageSetGM()",
    DrawImageSetGm::default()
);
// Port of: gm/drawimageset.cpp#L363-L363 (chrome/m156)
crate::def_gm!(
    DrawImageSetRectToRectGM_ = "DrawImageSetRectToRectGM()",
    DrawImageSetRectToRectGm::default()
);
// Port of: gm/drawimageset.cpp#L364-L364 (chrome/m156)
crate::def_gm!(
    DrawImageSetAlphaOnlyGM_ = "DrawImageSetAlphaOnlyGM()",
    DrawImageSetAlphaOnlyGm::default()
);

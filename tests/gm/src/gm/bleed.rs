// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bleed.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    clippy::needless_range_loop
)]

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tiled_image_utils;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

// Port of: gm/bleed.cpp#L45-L111 (chrome/m156)
fn make_ringed_image(width: i32, height: i32) -> (Option<Image>, Rect) {
    // These are kRGBA_8888_SkColorType values.
    const K_OUTER_RING_COLOR: u32 = 0xFFFF0000;
    const K_INNER_RING_COLOR: u32 = 0xFF0000FF;
    const K_CHECK_COLOR1: u32 = 0xFF000000;
    const K_CHECK_COLOR2: u32 = 0xFFFFFFFF;

    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    // SkAlign4(info.minRowBytes())
    let row_bytes = info.min_row_bytes().div_ceil(4) * 4;
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&info, row_bytes);

    for x in 0..width {
        bitmap.set_addr32(x, 0, K_OUTER_RING_COLOR);
    }
    bitmap.set_addr32(0, 1, K_OUTER_RING_COLOR);
    for x in 1..width - 1 {
        bitmap.set_addr32(x, 1, K_INNER_RING_COLOR);
    }
    bitmap.set_addr32(width - 1, 1, K_OUTER_RING_COLOR);
    for y in 2..height / 2 {
        bitmap.set_addr32(0, y, K_OUTER_RING_COLOR);
        bitmap.set_addr32(1, y, K_INNER_RING_COLOR);
        for x in 2..width / 2 {
            bitmap.set_addr32(x, y, K_CHECK_COLOR1);
        }
        for x in width / 2..width - 2 {
            bitmap.set_addr32(x, y, K_CHECK_COLOR2);
        }
        bitmap.set_addr32(width - 2, y, K_INNER_RING_COLOR);
        bitmap.set_addr32(width - 1, y, K_OUTER_RING_COLOR);
    }
    for y in height / 2..height - 2 {
        bitmap.set_addr32(0, y, K_OUTER_RING_COLOR);
        bitmap.set_addr32(1, y, K_INNER_RING_COLOR);
        for x in 2..width / 2 {
            bitmap.set_addr32(x, y, K_CHECK_COLOR2);
        }
        for x in width / 2..width - 2 {
            bitmap.set_addr32(x, y, K_CHECK_COLOR1);
        }
        bitmap.set_addr32(width - 2, y, K_INNER_RING_COLOR);
        bitmap.set_addr32(width - 1, y, K_OUTER_RING_COLOR);
    }
    bitmap.set_addr32(0, height - 2, K_OUTER_RING_COLOR);
    for x in 1..width - 1 {
        bitmap.set_addr32(x, height - 2, K_INNER_RING_COLOR);
    }
    bitmap.set_addr32(width - 1, height - 2, K_OUTER_RING_COLOR);
    for x in 0..width {
        bitmap.set_addr32(x, height - 1, K_OUTER_RING_COLOR);
    }
    bitmap.set_immutable();
    (
        bitmap.as_image(),
        Rect::from_ltrb(2.0, 2.0, (width - 2) as f32, (height - 2) as f32),
    )
}

// Port of: gm/bleed.cpp#L113-L121 (chrome/m156)
const K_BLOCK_SIZE: i32 = 70;
const K_BLOCK_SPACING: i32 = 12;
const K_COL0_X: i32 = K_BLOCK_SPACING;
const K_COL1_X: i32 = 2 * K_BLOCK_SPACING + K_BLOCK_SIZE;
const K_COL2_X: i32 = 3 * K_BLOCK_SPACING + 2 * K_BLOCK_SIZE;
const K_WIDTH: i32 = 4 * K_BLOCK_SPACING + 3 * K_BLOCK_SIZE;
const K_ROW0_Y: i32 = K_BLOCK_SPACING;
const K_ROW1_Y: i32 = 2 * K_BLOCK_SPACING + K_BLOCK_SIZE;
const K_ROW2_Y: i32 = 3 * K_BLOCK_SPACING + 2 * K_BLOCK_SIZE;
const K_ROW3_Y: i32 = 4 * K_BLOCK_SPACING + 3 * K_BLOCK_SIZE;
const K_ROW4_Y: i32 = 5 * K_BLOCK_SPACING + 4 * K_BLOCK_SIZE;
const K_SMALL_SIZE: i32 = 6;
const K_MAX_TEXTURE_SIZE: i32 = 1024;

// Port of: gm/bleed.cpp#L122-L380 (chrome/m156)
struct SrcRectConstraintGm {
    short_name: String,
    big_image: Option<Image>,
    small_image: Option<Image>,
    big_src_rect: Rect,
    small_src_rect: Rect,
    constraint: SrcRectConstraint,
    manual: bool,
}

impl SrcRectConstraintGm {
    fn new(short_name: &str, constraint: SrcRectConstraint, manual: bool) -> Self {
        Self {
            short_name: short_name.to_string(),
            big_image: None,
            small_image: None,
            big_src_rect: Rect::default(),
            small_src_rect: Rect::default(),
            constraint,
            manual,
        }
    }

    // Port of: gm/bleed.cpp#L140-L152 (chrome/m156)
    fn draw_image(
        &self,
        canvas: &Canvas,
        image: &Image,
        src: Rect,
        dst: Rect,
        sampling: SamplingOptions,
        paint: &Paint,
    ) {
        if self.manual {
            tiled_image_utils::draw_image_rect(
                canvas,
                image,
                &src,
                &dst,
                &sampling,
                Some(paint),
                self.constraint,
            );
        } else {
            canvas.draw_image_rect_with_sampling_options(
                image,
                Some((&src, self.constraint)),
                dst,
                sampling,
                paint,
            );
        }
    }

    // Port of: gm/bleed.cpp#L154-L166 (chrome/m156)
    fn draw_case1(
        &self,
        canvas: &Canvas,
        trans_x: i32,
        trans_y: i32,
        aa: bool,
        sampling: SamplingOptions,
    ) {
        let dst = Rect::from_xywh(
            trans_x as f32,
            trans_y as f32,
            K_BLOCK_SIZE as f32,
            K_BLOCK_SIZE as f32,
        );
        let mut paint = Paint::default();
        paint.set_color(Color::BLUE);
        paint.set_anti_alias(aa);
        self.draw_image(
            canvas,
            self.small_image.as_ref().expect("small image"),
            self.small_src_rect,
            dst,
            sampling,
            &paint,
        );
    }

    // Port of: gm/bleed.cpp#L168-L178 (chrome/m156)
    fn draw_case2(
        &self,
        canvas: &Canvas,
        trans_x: i32,
        trans_y: i32,
        aa: bool,
        sampling: SamplingOptions,
    ) {
        let dst = Rect::from_xywh(
            trans_x as f32,
            trans_y as f32,
            K_BLOCK_SIZE as f32,
            K_BLOCK_SIZE as f32,
        );
        let mut paint = Paint::default();
        paint.set_color(Color::BLUE);
        paint.set_anti_alias(aa);
        self.draw_image(
            canvas,
            self.big_image.as_ref().expect("big image"),
            self.big_src_rect,
            dst,
            sampling,
            &paint,
        );
    }

    // Port of: gm/bleed.cpp#L180-L190 (chrome/m156)
    fn draw_case3(
        &self,
        canvas: &Canvas,
        trans_x: i32,
        trans_y: i32,
        aa: bool,
        sampling: SamplingOptions,
    ) {
        let src = Rect::from_xywh(
            self.big_src_rect.left,
            self.big_src_rect.top,
            self.big_src_rect.width() / 2.0,
            self.big_src_rect.height() / 2.0,
        );
        let dst = Rect::from_xywh(
            trans_x as f32,
            trans_y as f32,
            K_BLOCK_SIZE as f32,
            K_BLOCK_SIZE as f32,
        );
        let mut paint = Paint::default();
        paint.set_color(Color::BLUE);
        paint.set_anti_alias(aa);
        self.draw_image(
            canvas,
            self.big_image.as_ref().expect("big image"),
            src,
            dst,
            sampling,
            &paint,
        );
    }

    // Port of: gm/bleed.cpp#L191-L203 (chrome/m156)
    fn draw_case4(
        &self,
        canvas: &Canvas,
        trans_x: i32,
        trans_y: i32,
        aa: bool,
        sampling: SamplingOptions,
    ) {
        let dst = Rect::from_xywh(
            trans_x as f32,
            trans_y as f32,
            K_BLOCK_SIZE as f32,
            K_BLOCK_SIZE as f32,
        );
        let mut paint = Paint::default();
        paint.set_mask_filter(
            MaskFilter::blur(
                BlurStyle::Normal,
                BlurMask::convert_radius_to_sigma(3.0),
                None,
            )
            .expect("a mask filter"),
        );
        paint.set_color(Color::BLUE);
        paint.set_anti_alias(aa);
        self.draw_image(
            canvas,
            self.small_image.as_ref().expect("small image"),
            self.small_src_rect,
            dst,
            sampling,
            &paint,
        );
    }

    // Port of: gm/bleed.cpp#L205-L217 (chrome/m156)
    fn draw_case5(
        &self,
        canvas: &Canvas,
        trans_x: i32,
        trans_y: i32,
        aa: bool,
        sampling: SamplingOptions,
    ) {
        let dst = Rect::from_xywh(
            trans_x as f32,
            trans_y as f32,
            K_BLOCK_SIZE as f32,
            K_BLOCK_SIZE as f32,
        );
        let mut paint = Paint::default();
        paint.set_mask_filter(
            MaskFilter::blur(
                BlurStyle::Outer,
                BlurMask::convert_radius_to_sigma(7.0),
                None,
            )
            .expect("a mask filter"),
        );
        paint.set_color(Color::BLUE);
        paint.set_anti_alias(aa);
        self.draw_image(
            canvas,
            self.small_image.as_ref().expect("small image"),
            self.small_src_rect,
            dst,
            sampling,
            &paint,
        );
    }
}

impl GM for SrcRectConstraintGm {
    fn name(&self) -> String {
        self.short_name.clone()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 1000)
    }

    // Port of: gm/bleed.cpp#L219-L342 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        if self.small_image.is_none() {
            let (big, big_src) = make_ringed_image(2 * K_MAX_TEXTURE_SIZE, 2 * K_MAX_TEXTURE_SIZE);
            self.big_image = big;
            self.big_src_rect = big_src;
            let (small, small_src) = make_ringed_image(K_SMALL_SIZE, K_SMALL_SIZE);
            self.small_image = small;
            self.small_src_rect = small_src;
        }

        canvas.clear(Color::GRAY);
        let mut matrices: Vec<Matrix> = Vec::new();
        // Draw with identity
        matrices.push(Matrix::new_identity());

        // Draw with rotation and scale down in x, up in y.
        let mut m = Matrix::new_identity();
        let k_bottom = (K_ROW4_Y + K_BLOCK_SIZE + K_BLOCK_SPACING) as f32;
        m.set_translate((0.0, k_bottom));
        m.pre_rotate(15.0, Point::new(0.0, k_bottom + K_BLOCK_SPACING as f32));
        m.pre_scale((0.71, 1.22), None);
        matrices.push(m);

        // Align the next set with the middle of the previous in y, translated to the right in x.
        let mut corners = [
            Point::new(0.0, 0.0),
            Point::new(0.0, k_bottom),
            Point::new(K_WIDTH as f32, k_bottom),
            Point::new(K_WIDTH as f32, 0.0),
        ];
        matrices
            .last()
            .expect("a matrix")
            .map_points_inplace(&mut corners);
        let mut m = Matrix::new_identity();
        m.set_translate((
            corners[0]
                .x
                .max(corners[1].x)
                .max(corners[2].x)
                .max(corners[3].x),
            (corners[0].y + corners[1].y + corners[2].y + corners[3].y) / 4.0,
        ));
        m.pre_scale((0.2, 0.2), None);
        matrices.push(m);

        let none = SamplingOptions::new(FilterMode::Nearest, MipmapMode::None);
        let low = SamplingOptions::from(FilterMode::Linear);
        let high = SamplingOptions::from(CubicResampler::mitchell());
        let mut max_x: f32 = 0.0;
        for anti_alias in [false, true] {
            canvas.save();
            canvas.translate((max_x, 0.0));
            for matrix in &matrices {
                canvas.save();
                canvas.concat(matrix);
                // First draw a column with no filtering
                self.draw_case1(canvas, K_COL0_X, K_ROW0_Y, anti_alias, none);
                self.draw_case2(canvas, K_COL0_X, K_ROW1_Y, anti_alias, none);
                self.draw_case3(canvas, K_COL0_X, K_ROW2_Y, anti_alias, none);
                self.draw_case4(canvas, K_COL0_X, K_ROW3_Y, anti_alias, none);
                self.draw_case5(canvas, K_COL0_X, K_ROW4_Y, anti_alias, none);
                // Then draw a column with low filtering
                self.draw_case1(canvas, K_COL1_X, K_ROW0_Y, anti_alias, low);
                self.draw_case2(canvas, K_COL1_X, K_ROW1_Y, anti_alias, low);
                self.draw_case3(canvas, K_COL1_X, K_ROW2_Y, anti_alias, low);
                self.draw_case4(canvas, K_COL1_X, K_ROW3_Y, anti_alias, low);
                self.draw_case5(canvas, K_COL1_X, K_ROW4_Y, anti_alias, low);
                // Then draw a column with high filtering. Skip it if in kStrict mode and MIP
                // mapping will be used. On GPU we allow bleeding at non-base levels because
                // building a new MIP chain for the subset is expensive.
                let (scale0, _scale1) = matrix.min_max_scales().expect("finite scales");
                if self.constraint != SrcRectConstraint::Strict || scale0 >= 1.0 {
                    self.draw_case1(canvas, K_COL2_X, K_ROW0_Y, anti_alias, high);
                    self.draw_case2(canvas, K_COL2_X, K_ROW1_Y, anti_alias, high);
                    self.draw_case3(canvas, K_COL2_X, K_ROW2_Y, anti_alias, high);
                    self.draw_case4(canvas, K_COL2_X, K_ROW3_Y, anti_alias, high);
                    self.draw_case5(canvas, K_COL2_X, K_ROW4_Y, anti_alias, high);
                }
                let mut inner_corners = [
                    Point::new(0.0, 0.0),
                    Point::new(0.0, k_bottom),
                    Point::new(K_WIDTH as f32, k_bottom),
                    Point::new(K_WIDTH as f32, 0.0),
                ];
                matrix.map_points_inplace(&mut inner_corners);
                let x = K_BLOCK_SIZE as f32
                    + inner_corners[0]
                        .x
                        .max(inner_corners[1].x)
                        .max(inner_corners[2].x)
                        .max(inner_corners[3].x);
                max_x = max_x.max(x);
                canvas.restore();
            }
            canvas.restore();
        }
    }
}

// Port of: gm/bleed.cpp#L344-L357 (chrome/m156)
crate::def_gm!(
    SrcRectConstraintGM_strict_no_red = "SrcRectConstraintGM(\"strict_constraint_no_red_allowed\", SkCanvas::kStrict_SrcRectConstraint, /* manual= */ false)",
    SrcRectConstraintGm::new(
        "strict_constraint_no_red_allowed",
        SrcRectConstraint::Strict,
        false
    )
);
crate::def_gm!(
    SrcRectConstraintGM_strict_no_red_manual = "SrcRectConstraintGM(\"strict_constraint_no_red_allowed_manual\", SkCanvas::kStrict_SrcRectConstraint, /* manual= */ true)",
    SrcRectConstraintGm::new(
        "strict_constraint_no_red_allowed_manual",
        SrcRectConstraint::Strict,
        true
    )
);
crate::def_gm!(
    SrcRectConstraintGM_strict_batch = "SrcRectConstraintGM(\"strict_constraint_batch_no_red_allowed\", SkCanvas::kStrict_SrcRectConstraint, /* manual= */ false)",
    SrcRectConstraintGm::new(
        "strict_constraint_batch_no_red_allowed",
        SrcRectConstraint::Strict,
        false
    )
);
crate::def_gm!(
    SrcRectConstraintGM_strict_batch_manual = "SrcRectConstraintGM(\"strict_constraint_batch_no_red_allowed_manual\", SkCanvas::kStrict_SrcRectConstraint, /* manual= */ true)",
    SrcRectConstraintGm::new(
        "strict_constraint_batch_no_red_allowed_manual",
        SrcRectConstraint::Strict,
        true
    )
);
crate::def_gm!(
    SrcRectConstraintGM_fast = "SrcRectConstraintGM(\"fast_constraint_red_is_allowed\", SkCanvas::kFast_SrcRectConstraint, /* manual= */ false)",
    SrcRectConstraintGm::new(
        "fast_constraint_red_is_allowed",
        SrcRectConstraint::Fast,
        false
    )
);
crate::def_gm!(
    SrcRectConstraintGM_fast_manual = "SrcRectConstraintGM(\"fast_constraint_red_is_allowed_manual\", SkCanvas::kFast_SrcRectConstraint, /* manual= */ true)",
    SrcRectConstraintGm::new(
        "fast_constraint_red_is_allowed_manual",
        SrcRectConstraint::Fast,
        true
    )
);

// Port of: gm/bleed.cpp#L359-L370 (chrome/m156)
fn make_image(canvas: &Canvas, src_r: &mut Rect) -> Option<Image> {
    const N: i32 = 10 + 2 + 8 + 2 + 10;
    let info = ImageInfo::new_n32_premul((N, N), None);
    let mut surface = canvas
        .new_surface(&info, None)
        .or_else(|| surfaces::raster(&info, None, None))?;
    let mut r = Rect::from_iwh(N, N);
    {
        let c = surface.canvas();
        let mut paint = Paint::default();
        paint.set_color(Color::RED);
        c.draw_rect(r, &paint);
        r.inset((10.0, 10.0));
        paint.set_color(Color::BLUE);
        c.draw_rect(r, &paint);
    }
    *src_r = r;
    src_r.inset((2.0, 2.0));
    surface.image_snapshot()
}

// Port of: gm/bleed.cpp#L391-L420 (chrome/m156)
crate::def_simple_gm!(bleed_downscale, canvas, 360, 240, {
    let mut src = Rect::default();
    let img = make_image(canvas, &mut src).expect("an image");
    let paint = Paint::default();

    canvas.translate((10.0, 10.0));

    let constraints = [SrcRectConstraint::Strict, SrcRectConstraint::Fast];
    let samplings = [
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
        SamplingOptions::from(FilterMode::Linear),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
    ];
    for constraint in constraints {
        canvas.save();
        for sampling in samplings {
            let info = ImageInfo::new_n32_premul((1, 1), None);
            let mut surf = canvas
                .new_surface(&info, None)
                .or_else(|| surfaces::raster(&info, None, None))
                .expect("a surface");
            surf.canvas().draw_image_rect_with_sampling_options(
                &img,
                Some((&src, constraint)),
                Rect::from_wh(1.0, 1.0),
                sampling,
                &paint,
            );
            // now blow up the 1 pixel result
            let snapshot = surf.image_snapshot().expect("a snapshot");
            canvas.draw_image_rect_with_sampling_options(
                &snapshot,
                None,
                Rect::from_wh(100.0, 100.0),
                SamplingOptions::default(),
                &paint,
            );
            canvas.translate((120.0, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, 120.0));
    }
});

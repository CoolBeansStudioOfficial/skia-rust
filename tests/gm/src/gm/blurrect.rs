// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurrect.cpp (chrome/m156)
//
// Not ported here: `BlurRectCompareGM` (`skiagm::BlurRectCompareGM`). It computes its reference
// masks with `std::erf`, which Rust's standard library does not provide: the C++ uses the host
// libm, and the libm port (`port/libm`) is on hold. Its manifest entry stays `todo`.

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::color::Color4f;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{CreateMode, MaskBuilder};
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_interp;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};

const STROKE_WIDTH: f32 = 10.0;
const NUM_PROCS: i32 = 3;

// Port of: gm/blurrect.cpp#L22-L24 (chrome/m156), fill_rect
fn fill_rect(canvas: &Canvas, r: &Rect, p: &Paint) {
    canvas.draw_rect(r, p);
}

// Port of: gm/blurrect.cpp#L26-L38 (chrome/m156), draw_donut
fn draw_donut(canvas: &Canvas, r: &Rect, p: &Paint) {
    let mut rect = *r;
    rect.outset((STROKE_WIDTH / 2.0, STROKE_WIDTH / 2.0));
    let mut path = PathBuilder::new();
    path.add_rect(rect, PathDirection::CW, None);
    rect = *r;
    rect.inset((STROKE_WIDTH / 2.0, STROKE_WIDTH / 2.0));

    path.add_rect(rect, PathDirection::CW, None);
    path.set_fill_type(PathFillType::EvenOdd);

    canvas.draw_path(&path.detach(), p);
}

// Port of: gm/blurrect.cpp#L40-L55 (chrome/m156), draw_donut_skewed
fn draw_donut_skewed(canvas: &Canvas, r: &Rect, p: &Paint) {
    let mut rect = *r;
    rect.outset((STROKE_WIDTH / 2.0, STROKE_WIDTH / 2.0));
    let mut path = PathBuilder::new();
    path.add_rect(rect, PathDirection::CW, None);
    rect = *r;
    rect.inset((STROKE_WIDTH / 2.0, STROKE_WIDTH / 2.0));

    rect.offset((7.0, -7.0));

    path.add_rect(rect, PathDirection::CW, None);
    path.set_fill_type(PathFillType::EvenOdd);

    canvas.draw_path(&path.detach(), p);
}

// Spits out an arbitrary gradient to test blur with shader on paint
// Port of: gm/blurrect.cpp#L57-L76 (chrome/m156), make_radial
fn make_radial() -> Option<skia_rust_core::shader::Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
    let tm = TileMode::Clamp;
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
    ];
    let pos = [0.25_f32, 0.75];
    let mut scale = Matrix::new_identity();
    scale.set_scale((0.5, 0.5), None);
    scale.post_translate((25.0, 25.0));
    let center0 = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    let center1 = Point::new(
        scalar_interp(pts[0].x, pts[1].x, 3.0 / 5.0),
        scalar_interp(pts[0].y, pts[1].y, 1.0 / 4.0),
    );
    gradient_shaders::two_point_conical_gradient(
        (center1, (pts[1].x - pts[0].x) / 7.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), tm, None),
            Interpolation::default(),
        ),
        Some(&scale),
    )
}

// Port of: gm/blurrect.cpp#L78-L80 (chrome/m156), the BlurRectGM class
struct BlurRectGm {
    name: &'static str,
    alpha: u8,
    mask_filters: [Option<MaskFilter>; 4],
}

impl BlurRectGm {
    // Port of: gm/blurrect.cpp#L82-L84 (chrome/m156), the constructor
    fn new(name: &'static str, alpha: u8) -> Self {
        Self {
            name,
            alpha,
            mask_filters: [None, None, None, None],
        }
    }

    // Port of: gm/blurrect.cpp#L116-L135 (chrome/m156), drawProcs
    fn draw_procs(
        canvas: &Canvas,
        r: &Rect,
        paint: &Paint,
        do_clip: bool,
        procs: &[fn(&Canvas, &Rect, &Paint)],
    ) {
        canvas.save();
        for proc in procs {
            if do_clip {
                canvas.save();
                canvas.clip_rect(*r, None, None);
            }
            proc(canvas, r, paint);
            if do_clip {
                canvas.restore();
            }
            canvas.translate((0.0, r.height() * 4.0 / 3.0));
        }
        canvas.restore();
    }
}

impl GM for BlurRectGm {
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(860, 820)
    }

    // Port of: gm/blurrect.cpp#L92-L98 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        for (i, filter) in self.mask_filters.iter_mut().enumerate() {
            let style = match i {
                0 => BlurStyle::Normal,
                1 => BlurStyle::Solid,
                2 => BlurStyle::Outer,
                _ => BlurStyle::Inner,
            };
            *filter = MaskFilter::blur(
                style,
                BlurMask::convert_radius_to_sigma(STROKE_WIDTH / 2.0),
                None,
            );
        }
    }

    // Port of: gm/blurrect.cpp#L104-L140 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((STROKE_WIDTH * 3.0 / 2.0, STROKE_WIDTH * 3.0 / 2.0));

        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let scales = [1.0_f32, 0.6];

        let procs: [fn(&Canvas, &Rect, &Paint); 3] = [fill_rect, draw_donut, draw_donut_skewed];
        for scale in scales {
            canvas.save();
            for mask_filter in &self.mask_filters {
                let mut paint = Paint::default();
                paint.set_mask_filter(mask_filter.clone());
                paint.set_alpha(self.alpha);

                let mut paint_with_radial = paint.clone();
                paint_with_radial.set_shader(make_radial());

                canvas.save();
                canvas.scale((scale, scale));
                Self::draw_procs(canvas, &r, &paint, false, &procs);
                canvas.translate((r.width() * 4.0 / 3.0, 0.0));
                Self::draw_procs(canvas, &r, &paint_with_radial, false, &procs);
                canvas.translate((r.width() * 4.0 / 3.0, 0.0));
                Self::draw_procs(canvas, &r, &paint, true, &procs);
                canvas.translate((r.width() * 4.0 / 3.0, 0.0));
                Self::draw_procs(canvas, &r, &paint_with_radial, true, &procs);
                canvas.restore();

                // std::size(procs) * r.height() * 4/3 * scales[s]
                canvas.translate((
                    0.0,
                    int_to_scalar(NUM_PROCS) * r.height() * 4.0 / 3.0 * scale,
                ));
            }
            canvas.restore();
            canvas.translate((4.0 * r.width() * 4.0 / 3.0 * scale, 0.0));
        }
    }
}

// Port of: gm/blurrect.cpp#L326 (chrome/m156), DEF_GM(return new BlurRectGM("blurrects", 0xFF);)
crate::def_gm!(
    BlurRectGM_blurrects = "BlurRectGM(\"blurrects\", 0xFF)",
    BlurRectGm::new("blurrects", 0xFF)
);

// Port of: gm/blurrect.cpp#L392-L445 (chrome/m156), blur_matrix_rect
crate::def_simple_gm!(blur_matrix_rect, canvas, 650, 685, {
    let r_rect = Rect::new(0.0, 0.0, 14.0, 60.0);
    let sigmas: [f32; 5] = [0.5, 1.2, 2.3, 3.9, 7.4];

    let c = Point::new(r_rect.center_x(), r_rect.center_y());

    let mut matrices: Vec<Matrix> = Vec::new();

    matrices.push(Matrix::rotate_deg_pivot(4.0, c));

    matrices.push(Matrix::rotate_deg_pivot(63.0, c));

    matrices.push(Matrix::rotate_deg_pivot(30.0, c));
    if let Some(last) = matrices.last_mut() {
        last.pre_scale((1.1, 0.5), None);
    }

    matrices.push(Matrix::rotate_deg_pivot(147.0, c));
    if let Some(last) = matrices.last_mut() {
        last.pre_scale((3.0, 0.1), None);
    }

    let mut mirror = Matrix::new_identity();
    mirror.set_all(0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let mirrored = Matrix::concat(&mirror, matrices.last().expect("a matrix"));
    matrices.push(mirrored);

    matrices.push(Matrix::rotate_deg_pivot(197.0, c));
    if let Some(last) = matrices.last_mut() {
        last.pre_skew((0.3, -0.5), None);
    }

    let mut bounds = Rect::new(0.0, 0.0, 0.0, 0.0);
    for m in &matrices {
        let (mapped, _) = m.map_rect(r_rect);
        bounds.join_non_empty_arg(mapped.sorted());
    }
    let blur_pad = 2.0 * sigmas[sigmas.len() - 1];
    bounds.outset((blur_pad, blur_pad));
    canvas.translate((-bounds.left(), -bounds.top()));
    for sigma in sigmas {
        let mut paint = Paint::default();
        paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma, None));
        canvas.save();
        for m in &matrices {
            canvas.save();
            canvas.concat(m);
            canvas.draw_rect(r_rect, &paint);
            canvas.restore();
            canvas.translate((0.0, bounds.height()));
        }
        canvas.restore();
        canvas.translate((bounds.width(), 0.0));
    }
});

// Port of: gm/blurrect.cpp#L339-L389 (chrome/m156), blurrect_gallery
crate::def_simple_gm!(blurrect_gallery, canvas, 1200, 1024, {
    let g_gm_width: i32 = 1200;
    let g_padding: i32 = 10;
    let g_margin: i32 = 100;

    let widths: [i32; 6] = [25, 5, 5, 100, 150, 25];
    let heights: [i32; 6] = [100, 100, 5, 25, 150, 25];
    let styles = [BlurStyle::Normal, BlurStyle::Inner, BlurStyle::Outer];
    let radii: [f32; 3] = [20.0, 5.0, 10.0];

    canvas.translate((50.0, 20.0));

    let mut cur_x: i32 = 0;
    let mut cur_y: i32 = 0;

    let mut max_height: i32 = 0;

    for i in 0..widths.len() {
        let width = widths[i];
        let height = heights[i];
        let r = Rect::new(0.0, 0.0, int_to_scalar(width), int_to_scalar(height));
        canvas.save();

        for radius in radii {
            for style in styles {
                let mut mask = MaskBuilder::default();
                if !BlurMask::blur_rect(
                    BlurMask::convert_radius_to_sigma(radius),
                    &mut mask,
                    &r,
                    style,
                    None,
                    CreateMode::ComputeBoundsAndRenderImage,
                ) {
                    continue;
                }

                let mask_w = mask.bounds.width();
                let mask_h = mask.bounds.height();
                let mut bm = Bitmap::new();
                // SkAssertResult(bm.installPixels(...)): the call itself always runs.
                assert!(bm.install_pixels(
                    &ImageInfo::new_a8((mask_w, mask_h)),
                    Some(mask.image),
                    mask.row_bytes as usize,
                ));

                if cur_x + bm.width() >= g_gm_width - g_margin {
                    cur_x = 0;
                    cur_y += max_height + g_padding;
                    max_height = 0;
                }

                canvas.save();
                canvas.translate((int_to_scalar(cur_x), int_to_scalar(cur_y)));
                canvas.translate((
                    -(int_to_scalar(bm.width()) - r.width()) / 2.0,
                    -(int_to_scalar(bm.height()) - r.height()) / 2.0,
                ));
                if let Some(image) = bm.as_image() {
                    canvas.draw_image(&image, (0.0, 0.0), None);
                }
                canvas.restore();

                cur_x += bm.width() + g_padding;
                if bm.height() > max_height {
                    max_height = bm.height();
                }
            }
        }
        canvas.restore();
    }
});

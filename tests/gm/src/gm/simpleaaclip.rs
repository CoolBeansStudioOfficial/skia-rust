// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/simpleaaclip.cpp (chrome/m156)

// This GM tests anti aliased single operation booleans with SkAAClips, SkRect and SkPaths.

// The int-to-scalar casts of small constants mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::Font;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::aa_clip::AAClip;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/simpleaaclip.cpp#L12-L31 (chrome/m156), paint_rgn
fn paint_rgn(canvas: &Canvas, clip: &AAClip, paint: &Paint) {
    let mask = clip.copy_to_mask();

    let mut bm = Bitmap::new();
    let info = ImageInfo::new(
        (mask.bounds.width(), mask.bounds.height()),
        ColorType::Alpha8,
        AlphaType::Premul,
        None,
    );
    let installed = bm.install_pixels(&info, mask.image, mask.row_bytes as usize);
    assert!(installed, "the mask pixels install into an A8 bitmap");

    // need to copy for deferred drawing test to work
    let bm2 = images::make_image_from_raster_bitmap(
        &bm,
        skia_rust_core::image_raster::CopyPixelsMode::Always,
    )
    .expect("a copy of the mask");

    canvas.draw_image_with_sampling_options(
        &bm2,
        (mask.bounds.left as scalar, mask.bounds.top as scalar),
        SamplingOptions::default(),
        Some(paint),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GeomType {
    Rect,
    Path,
    AAClip,
}

// Port of: gm/simpleaaclip.cpp#L33-L40 (chrome/m156), class SimpleClipGM
struct SimpleClipGm {
    geom_type: GeomType,
    base: Rect,
    rect: Rect,
    base_path: Path,
    rect_path: Path,
}

impl SimpleClipGm {
    // Port of: gm/simpleaaclip.cpp#L36-L37 (chrome/m156), SimpleClipGM(SkGeomTypes)
    fn new(geom_type: GeomType) -> Self {
        Self {
            geom_type,
            base: Rect::from_ltrb(0.0, 0.0, 0.0, 0.0),
            rect: Rect::from_ltrb(0.0, 0.0, 0.0, 0.0),
            base_path: Path::new(),
            rect_path: Path::new(),
        }
    }

    // Port of: gm/simpleaaclip.cpp#L43-L93 (chrome/m156), buildRgn
    fn build_rgn(&self, clip: &mut AAClip, op: ClipOp) {
        clip.set_path(
            &self.base_path,
            &RoundOut::<IRect>::round_out(self.base_path.bounds()),
            true,
        );

        let mut clip2 = AAClip::new();
        clip2.set_path(
            &self.rect_path,
            &RoundOut::<IRect>::round_out(self.rect_path.bounds()),
            true,
        );
        clip.op_aa_clip(&clip2, op);
    }

    // Port of: gm/simpleaaclip.cpp (chrome/m156), drawOrig
    fn draw_orig(&self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_style(skia_rust_core::paint::Style::Stroke);
        paint.set_color(Color::BLACK);

        canvas.draw_rect(self.base, &paint);
        canvas.draw_rect(self.rect, &paint);
    }

    // Port of: gm/simpleaaclip.cpp (chrome/m156), drawRgnOped
    fn draw_rgn_oped(&self, canvas: &Canvas, op: ClipOp, color: Color) {
        let mut clip = AAClip::new();

        self.build_rgn(&mut clip, op);
        self.draw_orig(canvas);

        let mut paint = Paint::default();
        paint.set_color(color);
        paint_rgn(canvas, &clip, &paint);
    }

    // Port of: gm/simpleaaclip.cpp (chrome/m156), drawPathsOped
    fn draw_paths_oped(&self, canvas: &Canvas, op: ClipOp, color: Color) {
        self.draw_orig(canvas);

        canvas.save();

        // create the clip mask with the supplied boolean op
        if self.geom_type == GeomType::Path {
            // path-based case
            canvas.clip_path(&self.base_path, None, true);
            canvas.clip_path(&self.rect_path, op, true);
        } else {
            // rect-based case
            canvas.clip_rect(self.base, None, true);
            canvas.clip_rect(self.rect, op, true);
        }

        // draw a rect that will entirely cover the clip mask area
        let mut paint = Paint::default();
        paint.set_color(color);

        let r = Rect::from_ltrb(90.0, 90.0, 180.0, 180.0);
        canvas.draw_rect(r, &paint);

        canvas.restore();
    }
}

impl GM for SimpleClipGm {
    // Port of: gm/simpleaaclip.cpp#L95-L107 (chrome/m156), getName
    fn name(&self) -> String {
        let suffix = match self.geom_type {
            GeomType::Rect => "rect",
            GeomType::Path => "path",
            GeomType::AAClip => "aaclip",
        };
        format!("simpleaaclip_{suffix}")
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 240)
    }

    // Port of: gm/simpleaaclip.cpp#L53-L66 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // offset the rects a bit so we get anti-aliasing in the rect case
        self.base = Rect::from_ltrb(100.65, 100.65, 150.65, 150.65);
        self.rect = self.base;
        self.rect.inset((5.0, 5.0));
        self.rect.offset((25.0, 25.0));

        self.base_path = Path::rrect(RRect::new_rect_xy(self.base, 5.0, 5.0), None);
        self.rect_path = Path::rrect(RRect::new_rect_xy(self.rect, 5.0, 5.0), None);
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFDD_DDDD)
    }

    // Port of: gm/simpleaaclip.cpp#L181-L209 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let g_ops = [
            (Color::BLACK, "Difference", ClipOp::Difference),
            (Color::RED, "Intersect", ClipOp::Intersect),
        ];

        let text_paint = Paint::default();
        let font = Font::from_size(default_portable_typeface(), 24.0);
        let mut x_off = 0;

        for (color, name, op) in g_ops {
            canvas.draw_str(name, (75.0, 50.0), &font, &text_paint);

            if self.geom_type == GeomType::AAClip {
                self.draw_rgn_oped(canvas, op, color);
            } else {
                self.draw_paths_oped(canvas, op, color);
            }

            if x_off >= 400 {
                canvas.translate((-400.0, 250.0));
                x_off = 0;
            } else {
                canvas.translate((200.0, 0.0));
                x_off += 200;
            }
        }
    }
}

// Port of: gm/simpleaaclip.cpp#L212-L214 (chrome/m156), DEF_GM(return new SimpleClipGM(...))
crate::def_gm!(
    SimpleClipGM_kRect_GeomType = "SimpleClipGM(SimpleClipGM::kRect_GeomType)",
    SimpleClipGm::new(GeomType::Rect)
);
// Port of: gm/simpleaaclip.cpp#L212-L214 (chrome/m156), DEF_GM(return new SimpleClipGM(...))
crate::def_gm!(
    SimpleClipGM_kPath_GeomType = "SimpleClipGM(SimpleClipGM::kPath_GeomType)",
    SimpleClipGm::new(GeomType::Path)
);
// Port of: gm/simpleaaclip.cpp#L212-L214 (chrome/m156), DEF_GM(return new SimpleClipGM(...))
crate::def_gm!(
    SimpleClipGM_kAAClip_GeomType = "SimpleClipGM(SimpleClipGM::kAAClip_GeomType)",
    SimpleClipGm::new(GeomType::AAClip)
);

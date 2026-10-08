// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/picture.cpp (chrome/m156)

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
use skia_rust_core::bbh_factory::RTreeFactory;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// Port of: gm/picture.cpp#L22-L43 (chrome/m156)
fn make_picture() -> Picture {
    let mut rec = PictureRecorder::new();
    let canvas = rec.begin_recording(Rect::from_wh(100.0, 100.0), false);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    paint.set_color(Color::new(0x8000_00FF));
    canvas.draw_rect(Rect::from_wh(100.0, 100.0), &paint);

    paint.set_color(Color::new(0x80FF_0000));
    canvas.draw_path(
        &Path::polygon(
            &[
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
            ],
            false,
            None,
            None,
        ),
        &paint,
    );

    paint.set_color(Color::new(0x8000_FF00));
    canvas.draw_path(
        &Path::polygon(
            &[
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(0.0, 100.0),
            ],
            false,
            None,
            None,
        ),
        &paint,
    );

    paint.set_color(Color::new(0x80FF_FFFF));
    paint.set_blend_mode(BlendMode::Plus);
    canvas.draw_rect(Rect::from_xywh(25.0, 25.0, 50.0, 50.0), &paint);

    rec.finish_recording_as_picture(None)
        .expect("recording was started")
}

// Exercise the optional arguments to drawPicture
//
// Port of: gm/picture.cpp#L45-L85 (chrome/m156)
struct PictureGm {
    picture: Option<Picture>,
}

impl GM for PictureGm {
    fn name(&self) -> String {
        "pictures".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(450, 120)
    }

    fn on_once_before_draw(&mut self) {
        self.picture = Some(make_picture());
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let picture = self.picture.as_ref().expect("onOnceBeforeDraw ran");
        canvas.translate((10.0, 10.0));

        let mut paint = Paint::default();

        canvas.draw_picture(picture, None, None);

        let mut matrix = Matrix::translate((110.0, 0.0));
        canvas.draw_picture(picture, Some(&matrix), None);

        matrix.post_translate((110.0, 0.0));
        canvas.draw_picture(picture, Some(&matrix), Some(&paint));

        paint.set_alpha_f(0.5);
        matrix.post_translate((110.0, 0.0));
        canvas.draw_picture(picture, Some(&matrix), Some(&paint));
    }
}

crate::def_gm!(PictureGM, PictureGm { picture: None });

// Exercise drawing a picture with a cull rect of non-zero top-left corner.
//
// See skbug.com/40040654, which would fail
// ```
//   dm -m picture_cull_rect --config serialize-8888
// ```
// until that bug is fixed.
//
// Port of: gm/picture.cpp#L94-L136 (chrome/m156)
struct PictureCullRectGm {
    picture: Option<Picture>,
}

impl GM for PictureCullRectGm {
    fn name(&self) -> String {
        "picture_cull_rect".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(120, 120)
    }

    #[allow(clippy::float_cmp)] // SkASSERT(cullRect().top() == 80)
    fn on_once_before_draw(&mut self) {
        let mut rec = PictureRecorder::new();
        let canvas =
            rec.begin_recording_with_factory(Rect::from_wh(100.0, 100.0), Some(&RTreeFactory));

        let mut paint = Paint::default();
        paint.set_anti_alias(false);

        let rect = Rect::from_ltrb(0.0, 80.0, 100.0, 100.0);

        // Make picture complex enough to trigger the cull rect and bbh (RTree) computations.
        // (A single drawRect won't trigger it.)
        paint.set_color(Color::new(0x8000_00FF));
        canvas.draw_rect(rect, &paint);
        canvas.draw_oval(rect, &paint);

        let picture = rec
            .finish_recording_as_picture(None)
            .expect("recording was started");
        debug_assert_eq!(picture.cull_rect().top, 80.0);
        self.picture = Some(picture);
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let picture = self.picture.as_ref().expect("onOnceBeforeDraw ran");
        canvas.clip_rect(Rect::from_ltrb(0.0, 60.0, 120.0, 120.0), None, None);
        canvas.translate((10.0, 10.0));
        canvas.draw_picture(picture, None, None);
    }
}

crate::def_gm!(PictureCullRectGM, PictureCullRectGm { picture: None });

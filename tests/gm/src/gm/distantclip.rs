// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/distantclip.cpp (chrome/m156)

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
use skia_rust_core::path::Path;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

const K_OFFSET: f32 = 35_000.0;
const K_EXTENTS: f32 = 1_000.0;

// Port of: gm/distantclip.cpp#L23-L58 (chrome/m156)
struct DistantClipGm;

impl GM for DistantClipGm {
    fn name(&self) -> String {
        "distantclip".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(100, 100)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut recorder = PictureRecorder::new();
        // We record a picture of huge vertical extents in which we clear the canvas to red, create
        // a 'extents' by 'extents' round rect clip at a vertical offset of 'offset', then draw
        // green into that.
        let rec = recorder.begin_recording(Rect::from_wh(K_EXTENTS, K_OFFSET + K_EXTENTS), false);
        rec.draw_color(Color::RED, None);
        rec.save();
        let r = Rect::from_xywh(
            -K_EXTENTS,
            K_OFFSET - K_EXTENTS,
            2.0 * K_EXTENTS,
            2.0 * K_EXTENTS,
        );
        rec.clip_path(
            &Path::rrect(RRect::new_rect_xy(r, 5.0, 5.0), None),
            None,
            true,
        );
        rec.draw_color(Color::GREEN, None);
        rec.restore();
        let pict = recorder
            .finish_recording_as_picture(None)
            .expect("recording was started");

        // Next we play that picture into another picture of the same size.
        let cull = pict.cull_rect();
        let rec2 = recorder.begin_recording(Rect::from_wh(cull.width(), cull.height()), false);
        pict.playback(rec2);
        let pict2 = recorder
            .finish_recording_as_picture(None)
            .expect("recording was started");

        // Finally we play the part of that second picture that should be green into the canvas.
        canvas.save();
        canvas.translate((K_EXTENTS / 2.0, -(K_OFFSET - K_EXTENTS / 2.0)));
        pict2.playback(canvas);
        canvas.restore();

        // If the image is red, we erroneously decided the clipPath was empty and didn't record
        // the green drawColor, if it's green we're all good.
    }
}

crate::def_gm!(DistantClipGM, DistantClipGm);

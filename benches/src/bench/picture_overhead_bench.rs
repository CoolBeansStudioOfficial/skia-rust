// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PictureOverheadBench.cpp

//! `ClipOverheadRecordingBench`: records 1000 clipped, translated round rects per loop into a
//! picture (non-rendering).

use skia_rust_core::paint::Paint;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

use crate::def_bench;
use crate::prelude::*;

/// `class ClipOverheadRecordingBench`.
// Port of: bench/PictureOverheadBench.cpp#L14-L42 (chrome/m156)
struct ClipOverheadRecordingBench;

impl Benchmark for ClipOverheadRecordingBench {
    // onGetName(): "clip_overhead_recording".
    fn name(&self) -> String {
        "clip_overhead_recording".to_owned()
    }

    // isSuitableFor(Backend backend): kNonRendering only.
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // onDraw(int loops, SkCanvas*)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // SkPictureRecorder rec;
        let mut rec = PictureRecorder::new();
        // for (int loop = 0; loop < loops; loop++) {
        for _ in 0..loops {
            // SkCanvas* canvas = rec.beginRecording({0,0, 2000,3000});
            let canvas =
                rec.begin_recording_with_factory(Rect::from_xywh(0.0, 0.0, 2000.0, 3000.0), None);
            // SkPaint paint;
            let paint = Paint::default();
            // SkRRect rrect;
            // rrect.setOval({0, 0, 1000, 1000});
            let rrect = RRect::new_oval(Rect::from_xywh(0.0, 0.0, 1000.0, 1000.0));
            // for (int i = 0; i < 1000; i++) {
            for _ in 0..1000 {
                // canvas->save();
                canvas.save();
                // canvas->translate(10, 10);
                canvas.translate((10.0, 10.0));
                // canvas->clipRect({10,10, 1000, 1000});
                canvas.clip_rect(Rect::from_ltrb(10.0, 10.0, 1000.0, 1000.0), None, None);
                // canvas->drawRRect(rrect, paint);
                canvas.draw_rrect(rrect, &paint);
                // canvas->restore();
                canvas.restore();
            }
            // (void)rec.finishRecordingAsPicture();
            let _ = rec.finish_recording_as_picture(None);
        }
    }
}

// Port of: bench/PictureOverheadBench.cpp#L43-L43 (chrome/m156)
def_bench!(
    clip_overhead_recording_bench = "ClipOverheadRecordingBench",
    ClipOverheadRecordingBench
);

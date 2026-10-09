// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PictureNestingBench.cpp

//! `PictureNesting`: a Sierpinski triangle whose sub-triangles are recorded into nested pictures
//! down to `max_picture_level`, then either recorded (`PictureNestingRecording`, non-rendering) or
//! played back (`PictureNestingPlayback`, a rendering bench).

use skia_rust_core::color::Color;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::size::ISize;

use crate::def_bench;
use crate::prelude::*;

/// `class PictureNesting`: the shared Sierpinski drawing and naming.
// Port of: bench/PictureNestingBench.cpp#L17-L106 (chrome/m156)
struct PictureNesting {
    name: String,
    max_level: i32,
    max_picture_level: i32,
    paint: Paint,
}

impl PictureNesting {
    // PictureNesting(const char* name, int maxLevel, int maxPictureLevel)
    // Port of: bench/PictureNestingBench.cpp#L17-L106 (chrome/m156)
    fn new(name: &str, max_level: i32, max_picture_level: i32) -> Self {
        let mut this = Self {
            name: String::new(),
            max_level,
            max_picture_level,
            paint: Paint::default(),
        };
        // fName.printf("picture_nesting_%s_%d", name, this->countPics());
        this.name = format!("picture_nesting_{name}_{}", this.count_pics());
        // fPaint.setColor(SK_ColorRED);
        this.paint.set_color(Color::new(0xFFFF_0000));
        // fPaint.setAntiAlias(true);
        this.paint.set_anti_alias(true);
        // fPaint.setStyle(SkPaint::kStroke_Style);
        this.paint.set_style(Style::Stroke);
        this
    }

    // void doDraw(SkCanvas* canvas): `onGetSize()` is passed in, since `size` takes `&mut self`.
    // Port of: bench/PictureNestingBench.cpp#L17-L106 (chrome/m156)
    fn do_draw(&self, canvas: &Canvas, size: ISize) {
        // SkISize canvasSize = onGetSize();
        // canvas->save();
        canvas.save();
        // canvas->scale(SkIntToScalar(canvasSize.width()), SkIntToScalar(canvasSize.height()));
        canvas.scale((int_to_scalar(size.width), int_to_scalar(size.height)));
        // SkDEBUGCODE(int pics = ) this->sierpinsky(canvas, 0, fPaint);
        let pics = self.sierpinsky(canvas, 0, &self.paint);
        // SkASSERT(pics == this->countPics());
        debug_assert_eq!(pics, self.count_pics());
        // canvas->restore();
        canvas.restore();
    }

    // int sierpinsky(SkCanvas* canvas, int lvl, const SkPaint& paint)
    // Port of: bench/PictureNestingBench.cpp#L17-L106 (chrome/m156)
    fn sierpinsky(&self, canvas: &Canvas, lvl: i32, paint: &Paint) -> i32 {
        // if (++lvl > fMaxLevel) return 0;
        let lvl = lvl + 1;
        if lvl > self.max_level {
            return 0;
        }
        // bool recordPicture = lvl <= fMaxPictureLevel;
        let record_picture = lvl <= self.max_picture_level;
        // SkPictureRecorder recorder;
        let mut recorder = PictureRecorder::new();
        if record_picture {
            // c = recorder.beginRecording(1, 1);
            // pics++;
            let c = recorder.begin_recording_with_factory(Rect::from_wh(1.0, 1.0), None);
            let pics = 1 + self.sierpinsky_body(c, lvl, paint);
            // canvas->drawPicture(recorder.finishRecordingAsPicture());
            let picture: Option<Picture> = recorder.finish_recording_as_picture(None);
            canvas.draw_picture(picture.expect("1x1 recording"), None, None);
            pics
        } else {
            self.sierpinsky_body(canvas, lvl, paint)
        }
    }

    // The drawing part of `sierpinsky`, on `c` (either the recorder's canvas or `canvas`).
    // Port of: bench/PictureNestingBench.cpp#L17-L106 (chrome/m156)
    fn sierpinsky_body(&self, c: &Canvas, lvl: i32, paint: &Paint) -> i32 {
        let mut pics = 0;
        // c->drawLine(0.5, 0, 0, 1, paint);
        c.draw_line((0.5, 0.0), (0.0, 1.0), paint);
        // c->drawLine(0.5, 0, 1, 1, paint);
        c.draw_line((0.5, 0.0), (1.0, 1.0), paint);
        // c->drawLine(0,   1, 1, 1, paint);
        c.draw_line((0.0, 1.0), (1.0, 1.0), paint);
        // c->save();
        c.save();
        // c->scale(0.5, 0.5);
        c.scale((0.5, 0.5));
        // c->translate(0, 1);
        c.translate((0.0, 1.0));
        // pics += this->sierpinsky(c, lvl, paint);
        pics += self.sierpinsky(c, lvl, paint);
        // c->translate(1, 0);
        c.translate((1.0, 0.0));
        // pics += this->sierpinsky(c, lvl, paint);
        pics += self.sierpinsky(c, lvl, paint);
        // c->translate(-0.5, -1);
        c.translate((-0.5, -1.0));
        // pics += this->sierpinsky(c, lvl, paint);
        pics += self.sierpinsky(c, lvl, paint);
        // c->restore();
        c.restore();
        pics
    }

    // countPics(): f(m) = 1/2 (3^m - 1), by integer arithmetic.
    // Port of: bench/PictureNestingBench.cpp#L17-L106 (chrome/m156)
    fn count_pics(&self) -> i32 {
        // int pics = 1;
        let mut pics: i32 = 1;
        // for (int i = 0; i < fMaxPictureLevel; i++) pics *= 3;
        for _ in 0..self.max_picture_level {
            pics *= 3;
        }
        // pics--;
        pics -= 1;
        // pics /= 2;
        pics /= 2;
        pics
    }
}

/// `class PictureNestingRecording`: records the whole nest each loop; non-rendering.
// Port of: bench/PictureNestingBench.cpp#L108-L133 (chrome/m156)
struct PictureNestingRecording {
    base: PictureNesting,
}

impl PictureNestingRecording {
    // PictureNestingRecording(int maxLevel, int maxPictureLevel) : INHERITED("recording", ...)
    fn new(max_level: i32, max_picture_level: i32) -> Self {
        Self {
            base: PictureNesting::new("recording", max_level, max_picture_level),
        }
    }
}

impl Benchmark for PictureNestingRecording {
    // onGetName()
    fn name(&self) -> String {
        self.base.name.clone()
    }

    // isSuitableFor(Backend backend): kNonRendering only.
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // onDraw(int loops, SkCanvas*)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // SkISize canvasSize = onGetSize();
        let size = self.size();
        // SkPictureRecorder recorder;
        let mut recorder = PictureRecorder::new();
        // for (int i = 0; i < loops; i++) {
        for _ in 0..loops {
            // SkCanvas* c = recorder.beginRecording(w, h);
            let c = recorder.begin_recording_with_factory(
                Rect::from_wh(int_to_scalar(size.width), int_to_scalar(size.height)),
                None,
            );
            // this->doDraw(c);
            self.base.do_draw(c, size);
            // (void)recorder.finishRecordingAsPicture();
            let _ = recorder.finish_recording_as_picture(None);
        }
    }
}

/// `class PictureNestingPlayback`: plays the recorded nest back `loops` times; a rendering bench.
// Port of: bench/PictureNestingBench.cpp#L135-L163 (chrome/m156)
struct PictureNestingPlayback {
    base: PictureNesting,
    picture: Option<Picture>,
}

impl PictureNestingPlayback {
    // PictureNestingPlayback(int maxLevel, int maxPictureLevel) : INHERITED("playback", ...)
    fn new(max_level: i32, max_picture_level: i32) -> Self {
        Self {
            base: PictureNesting::new("playback", max_level, max_picture_level),
            picture: None,
        }
    }
}

impl Benchmark for PictureNestingPlayback {
    // onGetName()
    fn name(&self) -> String {
        self.base.name.clone()
    }

    // onDelayedSetup()
    // Port of: bench/PictureNestingBench.cpp#L135-L163 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // this->INHERITED::onDelayedSetup();  (the base default does nothing)
        // SkISize canvasSize = onGetSize();
        let size = self.size();
        // SkPictureRecorder recorder;
        let mut recorder = PictureRecorder::new();
        // SkCanvas* c = recorder.beginRecording(w, h);
        let c = recorder.begin_recording_with_factory(
            Rect::from_wh(int_to_scalar(size.width), int_to_scalar(size.height)),
            None,
        );
        // this->doDraw(c);
        self.base.do_draw(c, size);
        // fPicture = recorder.finishRecordingAsPicture();
        self.picture = recorder.finish_recording_as_picture(None);
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/PictureNestingBench.cpp#L135-L163 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("PictureNestingPlayback is a rendering bench");
        let picture = self.picture.as_ref().expect("set in onDelayedSetup");
        // for (int i = 0; i < loops; i++) canvas->drawPicture(fPicture);
        for _ in 0..loops {
            canvas.draw_picture(picture, None, None);
        }
    }
}

// Port of: bench/PictureNestingBench.cpp#L165-L165 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_0 = "PictureNestingRecording(8, 0)",
    PictureNestingRecording::new(8, 0)
);
// Port of: bench/PictureNestingBench.cpp#L166-L166 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_1 = "PictureNestingRecording(8, 1)",
    PictureNestingRecording::new(8, 1)
);
// Port of: bench/PictureNestingBench.cpp#L167-L167 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_2 = "PictureNestingRecording(8, 2)",
    PictureNestingRecording::new(8, 2)
);
// Port of: bench/PictureNestingBench.cpp#L168-L168 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_3 = "PictureNestingRecording(8, 3)",
    PictureNestingRecording::new(8, 3)
);
// Port of: bench/PictureNestingBench.cpp#L169-L169 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_4 = "PictureNestingRecording(8, 4)",
    PictureNestingRecording::new(8, 4)
);
// Port of: bench/PictureNestingBench.cpp#L170-L170 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_5 = "PictureNestingRecording(8, 5)",
    PictureNestingRecording::new(8, 5)
);
// Port of: bench/PictureNestingBench.cpp#L171-L171 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_6 = "PictureNestingRecording(8, 6)",
    PictureNestingRecording::new(8, 6)
);
// Port of: bench/PictureNestingBench.cpp#L172-L172 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_7 = "PictureNestingRecording(8, 7)",
    PictureNestingRecording::new(8, 7)
);
// Port of: bench/PictureNestingBench.cpp#L173-L173 (chrome/m156)
def_bench!(
    picture_nesting_recording_8_8 = "PictureNestingRecording(8, 8)",
    PictureNestingRecording::new(8, 8)
);
// Port of: bench/PictureNestingBench.cpp#L175-L175 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_0 = "PictureNestingPlayback(8, 0)",
    PictureNestingPlayback::new(8, 0)
);
// Port of: bench/PictureNestingBench.cpp#L176-L176 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_1 = "PictureNestingPlayback(8, 1)",
    PictureNestingPlayback::new(8, 1)
);
// Port of: bench/PictureNestingBench.cpp#L177-L177 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_2 = "PictureNestingPlayback(8, 2)",
    PictureNestingPlayback::new(8, 2)
);
// Port of: bench/PictureNestingBench.cpp#L178-L178 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_3 = "PictureNestingPlayback(8, 3)",
    PictureNestingPlayback::new(8, 3)
);
// Port of: bench/PictureNestingBench.cpp#L179-L179 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_4 = "PictureNestingPlayback(8, 4)",
    PictureNestingPlayback::new(8, 4)
);
// Port of: bench/PictureNestingBench.cpp#L180-L180 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_5 = "PictureNestingPlayback(8, 5)",
    PictureNestingPlayback::new(8, 5)
);
// Port of: bench/PictureNestingBench.cpp#L181-L181 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_6 = "PictureNestingPlayback(8, 6)",
    PictureNestingPlayback::new(8, 6)
);
// Port of: bench/PictureNestingBench.cpp#L182-L182 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_7 = "PictureNestingPlayback(8, 7)",
    PictureNestingPlayback::new(8, 7)
);
// Port of: bench/PictureNestingBench.cpp#L183-L183 (chrome/m156)
def_bench!(
    picture_nesting_playback_8_8 = "PictureNestingPlayback(8, 8)",
    PictureNestingPlayback::new(8, 8)
);

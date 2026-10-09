// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PicturePlaybackBench.cpp

//! `TiledPlaybackBench`: plays back a picture of 10 000 random translucent rectangles into 256×256
//! tiles at tiled or random positions, with or without a bounding-box hierarchy (a rendering
//! bench on a 1024×1024 canvas).

use skia_rust_core::bbh_factory::{BBHFactory, RTreeFactory};
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::color::Color;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::ISize;

use crate::def_bench;
use crate::prelude::*;

/// `enum BBH { kNone, kRTree }`.
// Port of: bench/PicturePlaybackBench.cpp#L28-L28 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bbh {
    None,
    RTree,
}

/// `enum Mode { kTiled, kRandom }`.
// Port of: bench/PicturePlaybackBench.cpp#L29-L29 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Tiled,
    Random,
}

/// `class TiledPlaybackBench`.
// Port of: bench/PicturePlaybackBench.cpp#L30-L95 (chrome/m156)
struct TiledPlaybackBench {
    bbh: Bbh,
    mode: Mode,
    name: String,
    pic: Option<Picture>,
}

impl TiledPlaybackBench {
    // TiledPlaybackBench(BBH bbh, Mode mode) : fBBH(bbh), fMode(mode), fName("tiled_playback")
    // Port of: bench/PicturePlaybackBench.cpp#L30-L95 (chrome/m156)
    fn new(bbh: Bbh, mode: Mode) -> Self {
        let mut name = String::from("tiled_playback");
        match bbh {
            Bbh::None => name.push_str("_none"),
            Bbh::RTree => name.push_str("_rtree"),
        }
        match mode {
            Mode::Tiled => name.push_str("_tiled"),
            Mode::Random => name.push_str("_random"),
        }
        Self {
            bbh,
            mode,
            name,
            pic: None,
        }
    }
}

impl Benchmark for TiledPlaybackBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onGetSize(): SkISize::Make(1024, 1024).
    // Port of: bench/PicturePlaybackBench.cpp#L44-L44 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(1024, 1024)
    }

    // onDelayedSetup()
    // Port of: bench/PicturePlaybackBench.cpp#L30-L95 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // std::unique_ptr<SkBBHFactory> factory; switch (fBBH) { ... }
        let factory: Option<RTreeFactory> = match self.bbh {
            Bbh::None => None,
            Bbh::RTree => Some(RTreeFactory),
        };

        // SkPictureRecorder recorder;
        let mut recorder = PictureRecorder::new();
        // SkCanvas* canvas = recorder.beginRecording(1024, 1024, factory.get());
        let factory_ref = factory.as_ref().map(|f| f as &dyn BBHFactory);
        let canvas = recorder
            .begin_recording_with_factory(Rect::from_xywh(0.0, 0.0, 1024.0, 1024.0), factory_ref);
        // SkRandom rand;
        let mut rand = Random::default();
        // for (int i = 0; i < 10000; i++) { ... }
        for _ in 0..10000 {
            // SkScalar x = rand.nextRangeScalar(0, 1024), y = ..., w = ..., h = ...;
            let x = rand.next_range_scalar(0.0, 1024.0);
            let y = rand.next_range_scalar(0.0, 1024.0);
            let w = rand.next_range_scalar(0.0, 128.0);
            let h = rand.next_range_scalar(0.0, 128.0);
            // SkPaint paint;
            let mut paint = skia_rust_core::paint::Paint::default();
            // paint.setColor(rand.nextU());
            paint.set_color(Color::new(rand.next_u()));
            // paint.setAlpha(0xFF);
            paint.set_alpha(0xFF);
            // canvas->drawRect(SkRect::MakeXYWH(x,y,w,h), paint);
            canvas.draw_rect(Rect::from_xywh(x, y, w, h), &paint);
        }
        // fPic = recorder.finishRecordingAsPicture();
        self.pic = recorder.finish_recording_as_picture(None);
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/PicturePlaybackBench.cpp#L30-L95 (chrome/m156)
    // SkScalar(256 * uint32) and nextRangeScalar are float conversions the C++ does as well.
    #[allow(clippy::cast_precision_loss)]
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("TiledPlaybackBench is a rendering bench");
        let pic = self.pic.as_ref().expect("set in onDelayedSetup");
        for _ in 0..loops {
            // This inner loop guarantees we make the same choices for all bench variants.
            // SkRandom rand;
            let mut rand = Random::default();
            for _ in 0..10 {
                let (x, y) = match self.mode {
                    // x = SkScalar(256 * rand.nextULessThan(4)); y = ...;
                    Mode::Tiled => {
                        let x = (256 * rand.next_u_less_than(4)) as f32;
                        let y = (256 * rand.next_u_less_than(4)) as f32;
                        (x, y)
                    }
                    // x = rand.nextRangeScalar(0, 768); y = rand.nextRangeScalar(0, 768);
                    Mode::Random => {
                        let x = rand.next_range_scalar(0.0, 768.0);
                        let y = rand.next_range_scalar(0.0, 768.0);
                        (x, y)
                    }
                };
                // SkAutoCanvasRestore ar(canvas, true/*save now*/);
                let _ar = AutoCanvasRestore::guard(canvas, true);
                // canvas->clipRect(SkRect::MakeXYWH(x,y,256,256));
                canvas.clip_rect(Rect::from_xywh(x, y, 256.0, 256.0), None, None);
                // fPic->playback(canvas);
                pic.playback(canvas);
            }
        }
    }
}

// Port of: bench/PicturePlaybackBench.cpp#L97-L97 (chrome/m156)
def_bench!(
    tiled_playback_bench_none_random = "TiledPlaybackBench(kNone, kRandom)",
    TiledPlaybackBench::new(Bbh::None, Mode::Random)
);
// Port of: bench/PicturePlaybackBench.cpp#L98-L98 (chrome/m156)
def_bench!(
    tiled_playback_bench_none_tiled = "TiledPlaybackBench(kNone, kTiled )",
    TiledPlaybackBench::new(Bbh::None, Mode::Tiled)
);
// Port of: bench/PicturePlaybackBench.cpp#L99-L99 (chrome/m156)
def_bench!(
    tiled_playback_bench_rtree_random = "TiledPlaybackBench(kRTree, kRandom)",
    TiledPlaybackBench::new(Bbh::RTree, Mode::Random)
);
// Port of: bench/PicturePlaybackBench.cpp#L100-L100 (chrome/m156)
def_bench!(
    tiled_playback_bench_rtree_tiled = "TiledPlaybackBench(kRTree, kTiled )",
    TiledPlaybackBench::new(Bbh::RTree, Mode::Tiled)
);

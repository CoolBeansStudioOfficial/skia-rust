// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/complexclip4.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::rrect::RRect;

// This test exercise SkCanvas::androidFramework_replaceClip behavior
// Port of: gm/complexclip4.cpp#L25-L120 (chrome/m156)
struct ComplexClip4GM {
    do_aa_clip: bool,
}

impl ComplexClip4GM {
    fn new(aaclip: bool) -> Self {
        Self { do_aa_clip: aaclip }
    }

    // Android Framework will still support the legacy kReplace SkClipOp on older devices, so
    // this represents how to do so while also respecting the device restriction using the newer
    // androidFramework_resetClip() API.
    // Port of: gm/complexclip4.cpp#L48-L53 (chrome/m156)
    fn emulate_device_restriction(canvas: &Canvas, device_restriction: &IRect) {
        // TODO(michaelludwig): It may make more sense for device clip restriction to move on to
        // the SkSurface (which would let this GM draw correctly in viewer).
        canvas.android_framework_set_device_clip_restriction(device_restriction);
    }

    // Port of: gm/complexclip4.cpp#L55-L60 (chrome/m156)
    fn emulate_clip_rect_replace(canvas: &Canvas, clip_rect: &Rect, aa: bool) {
        canvas.reset_clip();
        canvas.clip_rect(clip_rect, ClipOp::Intersect, aa);
    }

    // Port of: gm/complexclip4.cpp#L62-L67 (chrome/m156)
    fn emulate_clip_rrect_replace(canvas: &Canvas, clip_rrect: &RRect, aa: bool) {
        canvas.reset_clip();
        canvas.clip_rrect(clip_rrect, ClipOp::Intersect, aa);
    }

    // Port of: gm/complexclip4.cpp#L69-L74 (chrome/m156)
    fn emulate_clip_path_replace(canvas: &Canvas, path: &Path, aa: bool) {
        canvas.reset_clip();
        canvas.clip_path(path, ClipOp::Intersect, aa);
    }
}

impl GM for ComplexClip4GM {
    fn name(&self) -> String {
        format!("complexclip4_{}", if self.do_aa_clip { "aa" } else { "bw" })
    }

    fn size(&mut self) -> ISize {
        ISize::new(970, 780)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFDE_DFDE)
    }

    // Port of: gm/complexclip4.cpp#L76-L123 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut p = Paint::default();
        p.set_anti_alias(self.do_aa_clip);
        p.set_color(Color::YELLOW);

        canvas.save();
        // draw a yellow rect through a rect clip
        canvas.save();
        Self::emulate_device_restriction(canvas, &IRect::new(100, 100, 300, 300));
        canvas.draw_color(Color::GREEN, None);
        Self::emulate_clip_rect_replace(
            canvas,
            &Rect::from_ltrb(100.0, 200.0, 400.0, 500.0),
            self.do_aa_clip,
        );
        canvas.draw_rect(Rect::from_ltrb(100.0, 200.0, 400.0, 500.0), &p);
        canvas.restore();

        // draw a yellow rect through a diamond clip
        canvas.save();
        Self::emulate_device_restriction(canvas, &IRect::new(500, 100, 800, 300));
        canvas.draw_color(Color::GREEN, None);

        let path_clip = Path::polygon(
            &[
                Point::new(650.0, 200.0),
                Point::new(900.0, 300.0),
                Point::new(650.0, 400.0),
                Point::new(650.0, 300.0),
            ],
            true,
            None,
            None,
        );
        Self::emulate_clip_path_replace(canvas, &path_clip, self.do_aa_clip);
        canvas.draw_rect(Rect::from_ltrb(500.0, 200.0, 900.0, 500.0), &p);
        canvas.restore();

        // draw a yellow rect through a round rect clip
        canvas.save();
        Self::emulate_device_restriction(canvas, &IRect::new(500, 500, 800, 700));
        canvas.draw_color(Color::GREEN, None);

        Self::emulate_clip_rrect_replace(
            canvas,
            &RRect::new_oval(Rect::from_ltrb(500.0, 600.0, 900.0, 750.0)),
            self.do_aa_clip,
        );
        canvas.draw_rect(Rect::from_ltrb(500.0, 600.0, 900.0, 750.0), &p);
        canvas.restore();

        // fill the clip with yellow color showing that androidFramework_replaceClip is
        // in device space
        canvas.save();
        canvas.clip_rect(
            Rect::from_ltrb(100.0, 400.0, 300.0, 750.0),
            ClipOp::Intersect,
            self.do_aa_clip,
        );
        canvas.draw_color(Color::GREEN, None);
        // should not affect the device-space clip
        canvas.rotate(20.0, None);
        canvas.translate((50.0, 50.0));
        Self::emulate_device_restriction(canvas, &IRect::new(150, 450, 250, 700));
        canvas.draw_color(Color::YELLOW, None);
        canvas.restore();

        canvas.restore();
    }
}

// Port of: gm/complexclip4.cpp#L131-L132 (chrome/m156)
crate::def_gm!(ComplexClip4GM_false = "ComplexClip4GM(false)", ComplexClip4GM::new(false));
crate::def_gm!(ComplexClip4GM_true = "ComplexClip4GM(true)", ComplexClip4GM::new(true));

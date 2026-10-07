// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/filltypes.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;

// Port of: gm/filltypes.cpp#L20-L92 (chrome/m156)
struct FillTypeGM {
    path: Path,
}

impl FillTypeGM {
    // Port of: gm/filltypes.cpp#L23-L25 (chrome/m156)
    fn new() -> Self {
        Self {
            path: Path::default(),
        }
    }

    // Port of: gm/filltypes.cpp#L27-L34 (chrome/m156)
    fn make_path(&mut self) {
        if self.path.is_empty() {
            let radius = 45.0;
            self.path = PathBuilder::new()
                .add_circle((50.0, 50.0), radius, None)
                .add_circle((100.0, 100.0), radius, None)
                .detach();
        }
    }

    // Port of: gm/filltypes.cpp#L41-L55 (chrome/m156)
    fn show_path(
        &mut self,
        canvas: &Canvas,
        x: i32,
        y: i32,
        ft: PathFillType,
        scale: f32,
        paint: &Paint,
    ) {
        let r = Rect::new(0.0, 0.0, 150.0, 150.0);

        canvas.save();
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        canvas.translate((x as f32, y as f32));
        canvas.clip_rect(r, None, None);
        canvas.draw_color(Color::WHITE, None);
        self.path.set_fill_type(ft);
        canvas.translate((r.center_x(), r.center_y()));
        canvas.scale((scale, scale));
        canvas.translate((-r.center_x(), -r.center_y()));
        canvas.draw_path(&self.path, paint);
        canvas.restore();
    }

    // Port of: gm/filltypes.cpp#L57-L66 (chrome/m156)
    fn show_four(&mut self, canvas: &Canvas, scale: f32, paint: &Paint) {
        self.show_path(canvas, 0, 0, PathFillType::Winding, scale, paint);
        self.show_path(canvas, 200, 0, PathFillType::EvenOdd, scale, paint);
        self.show_path(canvas, 0, 200, PathFillType::InverseWinding, scale, paint);
        self.show_path(canvas, 200, 200, PathFillType::InverseEvenOdd, scale, paint);
    }
}

impl GM for FillTypeGM {
    fn name(&self) -> String {
        "filltypes".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(835, 840)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFDD_DDDD)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        self.make_path();

        canvas.translate((20.0, 20.0));

        let mut paint = Paint::default();
        let scale = 5.0 / 4.0;

        paint.set_anti_alias(false);

        self.show_four(canvas, 1.0, &paint);
        canvas.translate((450.0, 0.0));
        self.show_four(canvas, scale, &paint);

        paint.set_anti_alias(true);

        canvas.translate((-450.0, 450.0));
        self.show_four(canvas, 1.0, &paint);
        canvas.translate((450.0, 0.0));
        self.show_four(canvas, scale, &paint);
    }
}

// Port of: gm/filltypes.cpp#L96 (chrome/m156)
crate::def_gm!(FillTypeGM_ = "FillTypeGM", FillTypeGM::new());

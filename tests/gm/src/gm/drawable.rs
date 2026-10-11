// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawable.cpp (chrome/m156)

use std::sync::Arc;

use crate::prelude::*;
use skia_rust_core::drawable::{Drawable, DrawableBase};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;

// Port of: gm/drawable.cpp#L15-L31 (chrome/m156), MyDrawable
#[derive(Debug)]
struct MyDrawable;

impl DrawableBase for MyDrawable {
    // Port of: gm/drawable.cpp#L16 (chrome/m156), onGetBounds
    fn on_get_bounds(&self) -> Rect {
        Rect::from_wh(50.0, 100.0)
    }

    // Port of: gm/drawable.cpp#L18-L30 (chrome/m156), onDraw
    fn on_draw(&self, canvas: &Canvas) {
        let path = PathBuilder::new()
            .move_to((10.0, 10.0))
            .conic_to((10.0, 90.0), (50.0, 90.0), 0.9)
            .detach();

        let mut paint = Paint::default();
        paint.set_color(Color::BLUE);
        canvas.draw_rect(path.bounds(), &paint);

        paint.set_anti_alias(true);
        paint.set_color(Color::WHITE);
        canvas.draw_path(&path, &paint);
    }
}

// Test calling drawables w/ translate and matrices
// Port of: gm/drawable.cpp#L39-L52 (chrome/m156), DEF_SIMPLE_GM(drawable, canvas, 180, 275)
crate::def_simple_gm!(drawable, canvas, 180, 275, {
    let drawable = Drawable::new(Arc::new(MyDrawable));

    canvas.translate((10.0, 10.0));
    canvas.draw_drawable(&drawable, None);
    canvas.draw_drawable_at(&drawable, (0.0, 150.0));

    let mut m = Matrix::scale((1.5, 0.8));
    m.post_translate((70.0, 0.0));
    canvas.draw_drawable(&drawable, Some(&m));

    m.post_translate((0.0, 150.0));
    canvas.draw_drawable(&drawable, Some(&m));
});

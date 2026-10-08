// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_4868.cpp (chrome/m156)

use skia_rust_core::color::Color;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;

// clipRect and drawLine should line up exactly when they use the same point.
// When SkPDF rounds large floats, this doesn't always happen.
// Port of: gm/skbug_4868.cpp#L15-L26 (chrome/m156)
crate::def_simple_gm!(skbug_4868, canvas, 32, 32, {
    canvas.translate((-68.0, -3378.0));
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);
    canvas.scale((0.566_929_14, 0.566_929_14));
    #[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
    let rc = Rect::new(158.0, 5_994.802_73, 165.0, 5_998.802_25);
    canvas.clip_rect(rc, None, None);
    canvas.clear(Color::new(0xFFCE_CFCE));
    canvas.draw_line((rc.left, rc.top), (rc.right, rc.bottom), &paint);
    canvas.draw_line((rc.right, rc.top), (rc.left, rc.bottom), &paint);
});

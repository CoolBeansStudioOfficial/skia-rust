// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_691386.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::utils::parse_path;

// Port of: gm/crbug_691386.cpp#L15-L28 (chrome/m156)
crate::def_simple_gm_can_fail!(crbug_691386, canvas, error_msg, 256, 256, {
    if let Some(path) = parse_path::from_svg("M -1 0 A 1 1 0 0 0 1 0 Z") {
        let mut p = Paint::default();
        p.set_style(Style::Stroke);
        p.set_stroke_width(0.025);
        canvas.scale((96.0, 96.0));
        canvas.translate((1.25, 1.25));
        canvas.draw_path(&path, &p);
        DrawResult::Ok
    } else {
        *error_msg = "Failed to parse path.".to_string();
        DrawResult::Fail
    }
});

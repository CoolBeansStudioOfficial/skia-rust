// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/circle_sizes.cpp (chrome/m156)

use skia_rust_core::paint::Paint;

// https://crbug.com/772953
// Port of: gm/circle_sizes.cpp#L12-L19 (chrome/m156)
crate::def_simple_gm!(circle_sizes, canvas, 128, 128, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    for i in 0..16 {
        #[allow(clippy::cast_precision_loss)] // int -> float arithmetic as in C++
        canvas.draw_circle(
            (14.0 + 32.0 * (i % 4) as f32, 14.0 + 32.0 * (i / 4) as f32),
            i as f32 + 1.0,
            &p,
        );
    }
});

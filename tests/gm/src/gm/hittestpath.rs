// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hittestpath.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::random::Random;

// Port of: gm/hittestpath.cpp#L19-L36 (chrome/m156)
fn test_hittest(canvas: &Canvas, path: &Path) {
    let mut paint = Paint::default();
    let r = *path.bounds();

    paint.set_color(Color::RED);
    canvas.draw_path(path, &paint);

    let margin: f32 = 4.0;

    paint.set_color(Color::from(0x8000_00FF));
    let mut y = r.top + 0.5 - margin;
    while y < r.bottom + margin {
        let mut x = r.left + 0.5 - margin;
        while x < r.right + margin {
            if path.contains((x, y)) {
                canvas.draw_point((x, y), &paint);
            }
            x += 1.0;
        }
        y += 1.0;
    }
}

// Port of: gm/hittestpath.cpp#L38-L80 (chrome/m156)
crate::def_simple_gm_can_fail!(hittestpath, canvas, error_msg, 700, 460, {
    let _ = &error_msg;
    let mut b = PathBuilder::new();
    let mut rand = Random::default();

    let scale: i32 = 300;
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    let scale_f = scale as f32;
    for _ in 0..4 {
        // get the random values deterministically
        let mut randoms = [0.0f32; 12];
        for random in &mut randoms {
            *random = rand.next_u_scalar1();
        }
        b.line_to((randoms[0] * scale_f, randoms[1] * scale_f))
            .quad_to(
                (randoms[2] * scale_f, randoms[3] * scale_f),
                (randoms[4] * scale_f, randoms[5] * scale_f),
            );
        b.cubic_to(
            (randoms[6] * scale_f, randoms[7] * scale_f),
            (randoms[8] * scale_f, randoms[9] * scale_f),
            (randoms[10] * scale_f, randoms[11] * scale_f),
        );
    }

    b.set_fill_type(PathFillType::EvenOdd);
    b.offset((20.0, 20.0));

    let mut path = b.detach();

    test_hittest(canvas, &path);

    canvas.translate((scale_f, 0.0));
    path.set_fill_type(PathFillType::Winding);

    test_hittest(canvas, &path);
    DrawResult::Ok
});

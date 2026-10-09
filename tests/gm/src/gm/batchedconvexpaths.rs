// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/batchedconvexpaths.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::libm;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::scalar::SCALAR_PI;

// Port of: gm/batchedconvexpaths.cpp#L22-L65 (chrome/m156)
struct BatchedConvexPathsGM;

impl GM for BatchedConvexPathsGM {
    fn name(&self) -> String {
        "batchedconvexpaths".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(512, 512)
    }

    #[allow(clippy::cast_precision_loss)] // int -> float arithmetic as in C++
    #[allow(clippy::float_cmp)] // `j+2 == numPoints` in C++
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        canvas.clear(Color::BLACK);
        for i in 0u32..10 {
            canvas.save();

            let num_points = (i + 3) * 3;
            let np = num_points as f32;
            let mut builder = PathBuilder::new();
            builder.move_to((1.0, 0.0));
            let mut j: f32 = 1.0;
            while j < np {
                const K2PI: f32 = SCALAR_PI * 2.0;
                let last = j + 2.0 == np;
                builder.cubic_to(
                    (libm::cosf(j / np * K2PI), libm::sinf(j / np * K2PI)),
                    (
                        libm::cosf((j + 1.0) / np * K2PI),
                        libm::sinf((j + 1.0) / np * K2PI),
                    ),
                    (
                        if last {
                            1.0
                        } else {
                            libm::cosf((j + 2.0) / np * K2PI)
                        },
                        if last {
                            0.0
                        } else {
                            libm::sinf((j + 2.0) / np * K2PI)
                        },
                    ),
                );
                j += 3.0;
            }
            let scale: f32 = (256 - i * 24) as f32;
            canvas.translate((
                scale + (256.0 - scale) * 0.33,
                scale + (256.0 - scale) * 0.33,
            ));
            canvas.scale((scale, scale));

            let mut paint = Paint::default();
            paint.set_color(Color::from(
                (i.wrapping_add(123_458_383u32)).wrapping_mul(285_018_463u32) | 0xff80_8080,
            ));
            paint.set_alpha_f(0.3);
            paint.set_anti_alias(true);

            canvas.draw_path(&builder.detach(), &paint);
            canvas.restore();
        }
        DrawResult::Ok
    }
}

// Port of: gm/batchedconvexpaths.cpp#L67 (chrome/m156)
crate::def_gm!(
    BatchedConvexPathsGM_ = "BatchedConvexPathsGM",
    BatchedConvexPathsGM
);

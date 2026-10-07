// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/clip_sierpinski_region.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};

const K_SIZE: i32 = 3 * 3 * 3 * 3 * 3;
const K_TRANS: i32 = 10;

// Port of: gm/clip_sierpinski_region.cpp#L25-L51 (chrome/m156)
crate::def_simple_gm!(
    clip_sierpinski_region,
    canvas,
    2 * K_TRANS + K_SIZE,
    2 * K_TRANS + K_SIZE,
    {
        let mut region = Region::new();
        const K_STEPS: i32 = 4;
        let mut n: i32 = 1;
        #[allow(clippy::cast_precision_loss)] // SkScalar s = kSize/3.f
        let mut s: f32 = K_SIZE as f32 / 3.0;
        for _ in 0..K_STEPS {
            for x in 0..n {
                for y in 0..n {
                    // MakeXYWH takes ints: the float expressions are truncated, as in C++
                    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                    region.op_rect(
                        IRect::from_xywh(
                            ((3 * x + 1) as f32 * s) as i32,
                            ((3 * y + 1) as f32 * s) as i32,
                            s as i32,
                            s as i32,
                        ),
                        Op::Union,
                    );
                }
            }
            n *= 3;
            s /= 3.0;
        }
        // Test that a save layer with an offset works as expected.
        region.translate((K_TRANS, K_TRANS));
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        let bounds = Rect::from_xywh(K_TRANS as f32, K_TRANS as f32, 1000.0, 1000.0);
        canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
        // Make sure the clip call ignores the CTM.
        canvas.rotate(25.0, Some(Point::new(50.0, 50.0)));
        canvas.clip_region(&region, None);
        let mut red = Paint::default();
        red.set_color(Color::RED);
        canvas.draw_paint(&red);
        canvas.restore();
    }
);

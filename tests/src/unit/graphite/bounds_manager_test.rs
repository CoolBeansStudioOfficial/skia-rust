// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/BoundsManagerTest.cpp (chrome/m156)

#![cfg(test)]
// The grid coordinates are small integers converted to `float`, as the C++ `(x + 0.1f) * w` does.
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::size::ISize;
use skia_rust_gpu::graphite::draw_order::CompressedPaintersOrder;
use skia_rust_gpu::graphite::geom::bounds_manager::{BoundsManager, GridBoundsManager};
use skia_rust_gpu::graphite::geom::rect::Rect;

use crate::{def_test, reporter_assert};

def_test!(BoundsManager, |r| {
    // Port of: tests/graphite/BoundsManagerTest.cpp#L14-L48 (chrome/m156)
    // 64 grid cells, each 16x16
    let n = 8;
    let w = 16;
    let mut bm: Box<dyn BoundsManager> = GridBoundsManager::make_square(
        ISize {
            width: n * w,
            height: n * w,
        },
        n,
    );

    let mut order = CompressedPaintersOrder::first();
    for y in 0..n {
        for x in 0..n {
            order = order.next();

            // Should only modify a single cell
            let b = Rect::xywh(
                (x as f32 + 0.1) * w as f32,
                (y as f32 + 0.1) * w as f32,
                0.8 * w as f32,
                0.8 * w as f32,
            );
            bm.record_draw(b, order);
        }
    }

    // TODO: repeat these queries using bounds that intersect across levels as well
    order = CompressedPaintersOrder::first();
    for y in 0..n {
        for x in 0..n {
            order = order.next();

            // Should only read a single cell
            let b = Rect::xywh(
                (x as f32 + 0.2) * w as f32,
                (y as f32 + 0.2) * w as f32,
                0.6 * w as f32,
                0.6 * w as f32,
            );

            let actual = bm.get_most_recent_draw(b);
            reporter_assert!(r, actual == order);
        }
    }

    // TODO: Then call recordDraw with new values that write to multiple cells

    // TODO: Then test calls where the new value is not larger than the current max
});

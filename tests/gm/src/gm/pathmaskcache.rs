// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pathmaskcache.cpp (chrome/m156)
//
// `modifyGrContextOptions` (the GPU path-renderer and mask-caching options) has no raster effect
// and is not part of the raster harness.

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

const K_PAD: f32 = 5.0;

// Port of: gm/pathmaskcache.cpp#L16-L80 (chrome/m156), the onDraw helper `drawPathSet`
fn draw_path_set(canvas: &Canvas, path: &Path, m: &Matrix) -> f32 {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    // `bounds.roundOut()` in the C++ returns an SkIRect that is discarded: it has no effect.
    let bounds = m.map_rect(path.bounds()).0;
    canvas.save();
    canvas.translate((-bounds.left(), -bounds.top()));
    canvas.save();
    canvas.concat(m);
    canvas.draw_path(path, &paint);
    canvas.restore();
    // translate by integer
    canvas.translate((bounds.width() + K_PAD, 0.0));
    canvas.save();
    canvas.concat(m);
    canvas.draw_path(path, &paint);
    canvas.restore();
    // translate by non-integer
    canvas.translate((bounds.width() + K_PAD + 0.15, 0.0));
    canvas.save();
    canvas.concat(m);
    canvas.draw_path(path, &paint);
    canvas.restore();
    // translate again so total translate fraction is almost identical to previous.
    canvas.translate((bounds.width() + K_PAD + 0.002, 0.0));
    canvas.save();
    canvas.concat(m);
    canvas.draw_path(path, &paint);
    canvas.restore();
    canvas.restore();
    bounds.bottom() + K_PAD
}

// Port of: gm/pathmaskcache.cpp#L28-L30 (chrome/m156), the onDraw paths
fn make_paths() -> Vec<Path> {
    let first = PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((98.0, 100.0))
        .line_to((100.0, 100.0))
        .conic_to((150.0, 50.0), (100.0, 0.0), 0.6)
        .conic_to((148.0, 50.0), (100.0, 100.0), 0.6)
        .conic_to((50.0, 30.0), (0.0, 100.0), 0.9)
        .detach();
    let mut second_builder = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    second_builder.add_circle((30.0, 30.0), 30.0, PathDirection::CW);
    second_builder.add_rect(
        Rect::from_xywh(45.0, 45.0, 50.0, 60.0),
        PathDirection::CW,
        None,
    );
    vec![first, second_builder.detach()]
}

// Port of: gm/pathmaskcache.cpp#L11-L80 (chrome/m156), PathMaskCache
struct PathMaskCacheGm;

impl GM for PathMaskCacheGm {
    fn name(&self) -> String {
        "path_mask_cache".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(650, 950)
    }

    // Port of: gm/pathmaskcache.cpp#L19-L80 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((K_PAD, K_PAD));
        for path in make_paths() {
            let mut ty = draw_path_set(canvas, &path, &Matrix::new_identity());
            canvas.translate((0.0, ty));
            // Non-uniform scale.
            let mut s = Matrix::new_identity();
            s.set_scale((0.5, 2.0), None);
            ty = draw_path_set(canvas, &path, &s);
            canvas.translate((0.0, ty));
            // Rotation
            let mut r = Matrix::new_identity();
            let bounds = path.bounds();
            r.set_rotate(60.0, Some(Point::new(bounds.center_x(), bounds.center_y())));
            ty = draw_path_set(canvas, &path, &r);
            canvas.translate((0.0, ty));
        }
    }
}

// Port of: gm/pathmaskcache.cpp#L84 (chrome/m156)
crate::def_gm!(PathMaskCache_ = "PathMaskCache()", PathMaskCacheGm);

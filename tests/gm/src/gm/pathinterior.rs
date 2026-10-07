// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pathinterior.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Port of: gm/pathinterior.cpp#L22-L26 (chrome/m156)
fn inset(r: &Rect) -> Rect {
    let mut rect = *r;
    rect.inset((r.width() / 8.0, r.height() / 8.0));
    rect
}

// Port of: gm/pathinterior.cpp#L28-L97 (chrome/m156)
struct PathInteriorGM;

impl PathInteriorGM {
    // Port of: gm/pathinterior.cpp#L38-L61 (chrome/m156)
    fn show(canvas: &Canvas, path: &Path) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        let rect = Rect::default();
        let has_interior = false;

        paint.set_color(if has_interior {
            crate::tool_utils::color_to_565(0xFF88_88FF)
        } else {
            Color::GRAY
        });
        canvas.draw_path(path, &paint);
        paint.set_style(Style::Stroke);
        paint.set_color(Color::RED);
        canvas.draw_path(path, &paint);

        if has_interior {
            paint.set_style(Style::Fill);
            paint.set_color(Color::from(0x8800_FF00));
            canvas.draw_rect(rect, &paint);
        }
    }
}

impl GM for PathInteriorGM {
    fn name(&self) -> String {
        "pathinterior".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(770, 770)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFDD_DDDD)
    }

    // Port of: gm/pathinterior.cpp#L63-L104 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int * SkScalar in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((8.5, 8.5));

        let rect = Rect::new(0.0, 0.0, 80.0, 80.0);
        let rad = rect.width() / 8.0;

        let mut i = 0;
        for inset_first in 0..=1 {
            for do_even_odd in 0..=1 {
                for outer_rr in 0..=1 {
                    for inner_rr in 0..=1 {
                        for outer_cw in 0..=1 {
                            for inner_cw in 0..=1 {
                                let mut builder =
                                    PathBuilder::new_with_fill_type(if do_even_odd != 0 {
                                        PathFillType::EvenOdd
                                    } else {
                                        PathFillType::Winding
                                    });
                                let outer_dir = if outer_cw != 0 {
                                    PathDirection::CW
                                } else {
                                    PathDirection::CCW
                                };
                                let inner_dir = if inner_cw != 0 {
                                    PathDirection::CW
                                } else {
                                    PathDirection::CCW
                                };

                                let mut r = if inset_first != 0 { inset(&rect) } else { rect };
                                if outer_rr != 0 {
                                    builder.add_rrect(
                                        RRect::new_rect_xy(r, rad, rad),
                                        outer_dir,
                                        None,
                                    );
                                } else {
                                    builder.add_rect(r, outer_dir, None);
                                }
                                r = if inset_first != 0 { rect } else { inset(&rect) };
                                if inner_rr != 0 {
                                    builder.add_rrect(
                                        RRect::new_rect_xy(r, rad, rad),
                                        inner_dir,
                                        None,
                                    );
                                } else {
                                    builder.add_rect(r, inner_dir, None);
                                }

                                let dx = (i / 8) as f32 * rect.width() * 6.0 / 5.0;
                                let dy = (i % 8) as f32 * rect.height() * 6.0 / 5.0;
                                i += 1;
                                builder.offset((dx, dy));

                                Self::show(canvas, &builder.detach());
                            }
                        }
                    }
                }
            }
        }
    }
}

// Port of: gm/pathinterior.cpp#L112 (chrome/m156)
crate::def_gm!(PathInteriorGM_ = "PathInteriorGM", PathInteriorGM);

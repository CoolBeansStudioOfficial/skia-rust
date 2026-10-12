// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dftext_blob_persp.cpp (chrome/m156)
//
// This GM tests reusing the same text blobs with distance fields rendering using various
// combinations of perspective and non-perspetive matrices, scissor clips, and different x,y params
// passed to the draw. The raster sink has no offscreen surface (see `dftext`), so it draws on the
// input canvas.

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::font::Edging;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_font};

#[derive(Clone, Copy)]
enum PerspMode {
    None,
    X,
    Y,
    XY,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TranslateWithMatrix {
    No,
    Yes,
}

struct DfTextBlobPerspGm {
    blobs: Vec<TextBlob>,
}

impl DfTextBlobPerspGm {
    fn new() -> Self {
        DfTextBlobPerspGm { blobs: Vec::new() }
    }

    // Port of: gm/dftext_blob_persp.cpp#L87-L114 (chrome/m156), drawBlob
    fn draw_blob(
        canvas: &Canvas,
        blob: &TextBlob,
        color: Color,
        x: f32,
        y: f32,
        persp_mode: PerspMode,
        translate_with_matrix: TranslateWithMatrix,
    ) {
        let (mut x, mut y) = (x, y);
        canvas.save();
        let mut persp = Matrix::new_identity();
        match persp_mode {
            PerspMode::None => {}
            PerspMode::X => {
                persp.set_persp_x(0.005);
            }
            PerspMode::Y => {
                persp.set_persp_y(0.005);
            }
            PerspMode::XY => {
                persp.set_persp_x(-0.001);
                persp.set_persp_y(-0.0015);
            }
        }
        persp = Matrix::concat(&persp, &Matrix::translate((-x, -y)));
        persp = Matrix::concat(&Matrix::translate((x, y)), &persp);
        canvas.concat(&persp);

        if translate_with_matrix == TranslateWithMatrix::Yes {
            canvas.translate((x, y));
            x = 0.0;
            y = 0.0;
        }

        let mut paint = Paint::default();
        paint.set_color(color);
        canvas.draw_text_blob(blob, (x, y), &paint);
        canvas.restore();
    }
}

impl GM for DfTextBlobPerspGm {
    fn name(&self) -> String {
        "dftext_blob_persp".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(900, 350)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFFF_FFFF)
    }

    // Port of: gm/dftext_blob_persp.cpp#L38-L51 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        for i in 0..3 {
            let mut font = default_portable_font();
            font.set_size(32.0);
            font.set_edging(match i {
                0 => Edging::Alias,
                1 => Edging::AntiAlias,
                _ => Edging::SubpixelAntiAlias,
            });
            font.set_subpixel(true);

            let mut builder = TextBlobBuilder::new();
            add_to_text_blob(&mut builder, "SkiaText", &font, 0.0, 0.0);
            if let Some(blob) = builder.make() {
                self.blobs.push(blob);
            }
        }
    }

    // Port of: gm/dftext_blob_persp.cpp#L53-L84 (chrome/m156), onDraw (raster path)
    fn on_draw(&mut self, input_canvas: &Canvas) {
        let canvas = input_canvas;
        // init our new canvas with the old canvas's matrix
        canvas.set_matrix(&skia_rust_core::m44::M44::from(
            input_canvas.local_to_device_as_3x3(),
        ));

        let mut x = 0.0_f32;
        let mut y = 0.0_f32;
        let mut max_h = 0.0_f32;
        for twm in [TranslateWithMatrix::No, TranslateWithMatrix::Yes] {
            for pm in [PerspMode::None, PerspMode::X, PerspMode::Y, PerspMode::XY] {
                for blob in &self.blobs {
                    for clip in [false, true] {
                        let _acr = AutoCanvasRestore::guard(canvas, true);
                        let w = blob.bounds().width();
                        let h = blob.bounds().height();
                        if clip {
                            let rect =
                                Rect::from_xywh(x + 5.0, y + 5.0, w * 3.0 / 4.0, h * 3.0 / 4.0);
                            canvas.clip_rect(rect, None, false);
                        }
                        Self::draw_blob(canvas, blob, Color::BLACK, x, y + h, pm, twm);
                        x += w + 20.0;
                        max_h = max_h.max(h);
                    }
                }
                x = 0.0;
                y += max_h + 20.0;
                max_h = 0.0;
            }
        }
    }
}

crate::def_gm!(DFTextBlobPerspGM, DfTextBlobPerspGm::new());

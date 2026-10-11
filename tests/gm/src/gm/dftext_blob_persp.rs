// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dftext_blob_persp.cpp (chrome/m156)
//
// The offscreen `SkSurfaces::RenderTarget` path is GPU-only (it needs the Ganesh recording
// context, which a raster canvas never has), so on the raster sink `surface` is null and the GM
// draws straight onto the input canvas. Only that path is ported.

use crate::prelude::*;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::font::Edging;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_font};

// Port of: gm/dftext_blob_persp.cpp#L23-L28 (chrome/m156), PerspMode
#[derive(Clone, Copy)]
enum PerspMode {
    None,
    X,
    Y,
    XY,
}

// Port of: gm/dftext_blob_persp.cpp#L23-L28 (chrome/m156), TranslateWithMatrix
#[derive(Clone, Copy, PartialEq, Eq)]
enum TranslateWithMatrix {
    No,
    Yes,
}

/// This GM tests reusing the same text blobs with distance fields rendering using various
/// combinations of perspective and non-perspetive matrices, scissor clips, and different x,y params
/// passed to the draw.
// Port of: gm/dftext_blob_persp.cpp#L30-L110 (chrome/m156), DFTextBlobPerspGM
struct DFTextBlobPerspGm {
    blobs: Vec<TextBlob>,
}

impl DFTextBlobPerspGm {
    // Port of: gm/dftext_blob_persp.cpp#L84-L108 (chrome/m156), drawBlob
    fn draw_blob(
        canvas: &Canvas,
        blob: &TextBlob,
        color: Color,
        (mut x, mut y): (f32, f32),
        persp_mode: PerspMode,
        translate_with_matrix: TranslateWithMatrix,
    ) {
        canvas.save();
        let mut persp = Matrix::default();
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

impl GM for DFTextBlobPerspGm {
    // Port of: gm/dftext_blob_persp.cpp#L36 (chrome/m156), getName
    fn name(&self) -> String {
        "dftext_blob_persp".to_owned()
    }

    // Port of: gm/dftext_blob_persp.cpp#L37 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(900, 350)
    }

    // Port of: gm/dftext_blob_persp.cpp#L35 (chrome/m156), the constructor's setBGColor
    fn bg_color(&self) -> Color {
        Color::WHITE
    }

    // Port of: gm/dftext_blob_persp.cpp#L38-L51 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.blobs.clear();
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

    // Port of: gm/dftext_blob_persp.cpp#L53-L82 (chrome/m156), onDraw (raster sink: the surface
    // is null, so `canvas` is the input canvas and the offscreen blit is not run)
    fn on_draw(&mut self, input_canvas: &Canvas) {
        let canvas = input_canvas;
        // init our new canvas with the old canvas's matrix
        canvas.set_matrix(&M44::from(input_canvas.local_to_device_as_3x3()));
        let mut x: f32 = 0.0;
        let mut y: f32 = 0.0;
        let mut max_h: f32 = 0.0;
        for twm in [TranslateWithMatrix::No, TranslateWithMatrix::Yes] {
            for pm in [PerspMode::None, PerspMode::X, PerspMode::Y, PerspMode::XY] {
                for blob in &self.blobs {
                    for clip in [false, true] {
                        canvas.save();
                        let w = blob.bounds().width();
                        let h = blob.bounds().height();
                        if clip {
                            let rect =
                                Rect::from_xywh(x + 5.0, y + 5.0, w * 3.0 / 4.0, h * 3.0 / 4.0);
                            canvas.clip_rect(rect, ClipOp::Intersect, false);
                        }
                        Self::draw_blob(canvas, blob, Color::BLACK, (x, y + h), pm, twm);
                        x += w + 20.0;
                        max_h = max_h.max(h);
                        canvas.restore();
                    }
                }
                x = 0.0;
                y += max_h + 20.0;
                max_h = 0.0;
            }
        }
    }
}

// Port of: gm/dftext_blob_persp.cpp#L112 (chrome/m156), DEF_GM(return new DFTextBlobPerspGM;)
crate::def_gm!(DFTextBlobPerspGM, DFTextBlobPerspGm { blobs: Vec::new() });

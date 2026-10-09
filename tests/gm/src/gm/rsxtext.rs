// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rsxtext.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::create_portable_typeface;

// Exercises RSX text blobs + shader with various local matrix combinations.
// Yellow grid should stay aligned for text vs. background.
const SZ: f32 = 300.0;
const SCALE: f32 = 1.4;
const TXT: &str = "TEST";

// Port of: gm/rsxtext.cpp#L22 (chrome/m156), kFontSZ = kSZ * 0.38 (a double, stored as a float)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ double-to-float store of kFontSZ
const FONT_SZ: f32 = (300.0_f64 * 0.38) as f32;

// Port of: gm/rsxtext.cpp#L11-L14 (chrome/m156), RSXShaderGM
struct RsxShaderGm {
    blob: Option<TextBlob>,
}

impl RsxShaderGm {
    // Port of: gm/rsxtext.cpp#L13 (chrome/m156), the constructor
    fn new() -> Self {
        Self { blob: None }
    }

    // Port of: gm/rsxtext.cpp#L49-L63 (chrome/m156), draw_one
    fn draw_one(&self, canvas: &Canvas, pos: Point, lm: &Matrix, outer_lm: &Matrix) {
        let _acr = AutoCanvasRestore::guard(canvas, true);
        canvas.translate((pos.x, pos.y));
        let mut p = Paint::default();
        p.set_shader(make_shader(lm, outer_lm));
        p.set_alpha_f(0.75);
        canvas.draw_rect(Rect::from_xywh(0.0, 0.0, SZ, SZ), &p);
        p.set_alpha_f(1.0);
        let blob = self.blob.as_ref().expect("onOnceBeforeDraw made the blob");
        canvas.draw_text_blob(blob, (0.0, FONT_SZ * 1.0), &p);
        canvas.draw_text_blob(blob, (0.0, FONT_SZ * 2.0), &p);
    }
}

impl GM for RsxShaderGm {
    // Port of: gm/rsxtext.cpp#L16 (chrome/m156), getName
    fn name(&self) -> String {
        "rsx_blob_shader".to_owned()
    }

    // Port of: gm/rsxtext.cpp#L17 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        let side = SZ * SCALE * 2.1;
        ISize::new(float_to_int(side), float_to_int(side))
    }

    // Port of: gm/rsxtext.cpp#L18-L35 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let style = FontStyle::new(Weight::EXTRA_BLACK, Width::NORMAL, Slant::Upright);
        let typeface = create_portable_typeface(Some("Sans"), style);
        let mut font = Font::from_size(typeface, FONT_SZ);
        font.set_edging(Edging::AntiAlias);
        let mut glyphs = [GlyphId::default(); 16];
        let mut widths = [0.0_f32; 16];
        let glyph_count = font.text_to_glyphs(TXT.as_bytes(), TextEncoding::UTF8, &mut glyphs);
        font.get_widths(&glyphs[..glyph_count], &mut widths[..glyph_count]);
        let mut builder = TextBlobBuilder::new();
        let (buf_glyphs, buf_xforms) = builder.alloc_run_rsxform(&font, glyph_count);
        buf_glyphs.copy_from_slice(&glyphs[..glyph_count]);
        let mut x = 0.0_f32;
        for (i, xform) in buf_xforms.iter_mut().enumerate() {
            *xform = RSXform::new(1.0, 0.0, (x, 0.0));
            x += widths[i];
        }
        self.blob = builder.make();
    }

    // Port of: gm/rsxtext.cpp#L36-L45 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.scale((SCALE, SCALE));
        let identity = Matrix::new_identity();
        self.draw_one(canvas, Point::new(0.0, 0.0), &identity, &identity);
        self.draw_one(
            canvas,
            Point::new(SZ * 1.1, 0.0),
            &Matrix::scale((2.0, 2.0)),
            &identity,
        );
        self.draw_one(
            canvas,
            Point::new(0.0, SZ * 1.1),
            &identity,
            &Matrix::rotate_deg(45.0),
        );
        self.draw_one(
            canvas,
            Point::new(SZ * 1.1, SZ * 1.1),
            &Matrix::scale((2.0, 2.0)),
            &Matrix::rotate_deg(45.0),
        );
    }
}

// Port of: gm/rsxtext.cpp#L17 (chrome/m156), the SkISize::Make float-to-int conversion
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ truncates the float
fn float_to_int(v: f32) -> i32 {
    v as i32
}

// Port of: gm/rsxtext.cpp#L64-L80 (chrome/m156), make_shader
fn make_shader(lm: &Matrix, outer_lm: &Matrix) -> Option<Shader> {
    const TILE_W: i32 = 30;
    const TILE_H: i32 = 30;
    let mut surface = surfaces::raster_n32_premul((TILE_W, TILE_H)).expect("a surface");
    let mut p = Paint::default();
    p.set_color(Color::from(0xffff_ff00_u32));
    surface.canvas().draw_paint(&p);
    p.set_color(Color::from(0xff00_8000_u32));
    surface.canvas().draw_rect(
        Rect::from_ltrb(
            0.0,
            0.0,
            int_to_scalar(TILE_W) * 0.9,
            int_to_scalar(TILE_H) * 0.9,
        ),
        &p,
    );
    let image: Image = surface.image_snapshot().expect("a snapshot");
    let shader = image.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::from(FilterMode::Linear),
        lm,
    )?;
    Some(shader.with_local_matrix(outer_lm))
}

// Port of: gm/rsxtext.cpp#L81 (chrome/m156)
crate::def_gm!(
    #[ignore = "see notes/gm-rsxtext.cpp-RSXShaderGM.md"]
    RSXShaderGM,
    RsxShaderGm::new()
);

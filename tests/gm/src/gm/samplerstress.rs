// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/samplerstress.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::color::pre_multiply_color;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const X_SIZE: i32 = 16;
const Y_SIZE: i32 = 16;

/// `SamplerStressGM`: stress test the samplers by rendering a textured glyph with a mask and an
/// AA clip.
// Port of: gm/samplerstress.cpp#L13-L107 (chrome/m156), SamplerStressGM
struct SamplerStressGm {
    texture: Bitmap,
    texture_created: bool,
    shader: Option<Shader>,
    mask_filter: Option<MaskFilter>,
}

impl SamplerStressGm {
    fn new() -> Self {
        Self {
            texture: Bitmap::new(),
            texture_created: false,
            shader: None,
            mask_filter: None,
        }
    }

    // Port of: gm/samplerstress.cpp#L30-L47 (chrome/m156), createTexture
    /// Creates a red & green stripes on black texture.
    fn create_texture(&mut self) {
        if self.texture_created {
            return;
        }
        self.texture.alloc_n32_pixels((X_SIZE, Y_SIZE), None);
        for y in 0..Y_SIZE {
            for x in 0..X_SIZE {
                let mut value = pre_multiply_color(Color::BLACK);
                if y % 5 == 0 {
                    value = pre_multiply_color(Color::RED);
                }
                if x % 7 == 0 {
                    value = pre_multiply_color(Color::GREEN);
                }
                self.texture.set_addr32(x, y, value);
            }
        }
        self.texture_created = true;
    }

    // Port of: gm/samplerstress.cpp#L49-L56 (chrome/m156), createShader
    fn create_shader(&mut self) {
        if self.shader.is_some() {
            return;
        }
        self.create_texture();
        self.shader = self.texture.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            None,
        );
    }

    // Port of: gm/samplerstress.cpp#L58-L64 (chrome/m156), createMaskFilter
    fn create_mask_filter(&mut self) {
        if self.mask_filter.is_some() {
            return;
        }
        let sigma = 1.0;
        self.mask_filter = MaskFilter::blur(BlurStyle::Normal, sigma, None);
    }
}

impl GM for SamplerStressGm {
    fn name(&self) -> String {
        "gpusamplerstress".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/samplerstress.cpp#L66-L100 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        self.create_shader();
        self.create_mask_filter();
        canvas.save();
        // draw a letter "M" with a green & red striped texture and a
        // stipple mask with a round rect soft clip
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_shader(self.shader.clone());
        paint.set_mask_filter(self.mask_filter.clone());
        let font = skia_rust_core::font::Font::from_size(default_portable_typeface(), 72.0);
        let temp = Rect::from_ltrb(115.0, 75.0, 144.0, 110.0);
        let path = Path::rrect(RRect::new_rect_xy(temp, 5.0, 5.0), None);
        canvas.clip_path(&path, None, true); // AA is on
        canvas.draw_str("M", (100.0, 100.0), &font, &paint);
        canvas.restore();

        // Now draw stroked versions of the "M" and the round rect so we can
        // see what is going on
        let mut paint2 = Paint::default();
        paint2.set_color(Color::BLACK);
        paint2.set_anti_alias(true);
        paint2.set_style(Style::Stroke);
        paint2.set_stroke_width(1.0);
        canvas.draw_str("M", (100.0, 100.0), &font, &paint2);
        paint2.set_color(Color::GRAY);
        canvas.draw_path(&path, &paint2);
    }
}

// Port of: gm/samplerstress.cpp#L104 (chrome/m156), DEF_GM( return new SamplerStressGM; )
crate::def_gm!(SamplerStressGM, SamplerStressGm::new());

// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/xfermodes.cpp (chrome/m156)

// The int-to-scalar casts of small constants mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]
// Single-letter and similar names mirror the C++ GM (w, h, x, y; rect, rrect).
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use crate::prelude::*;
use crate::tool_utils::color_to_565;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/xfermodes.cpp#L9-L26 (chrome/m156), enum SrcType
const RECTANGLE_IMAGE: u32 = 0x01;
const RECTANGLE_IMAGE_WITH_ALPHA: u32 = 0x02;
const SMALL_RECTANGLE_IMAGE_WITH_ALPHA: u32 = 0x04;
const RECTANGLE: u32 = 0x08;
const QUARTER_CLEAR: u32 = 0x10;
const QUARTER_CLEAR_IN_LAYER: u32 = 0x20;
const SMALL_TRANSPARENT_IMAGE: u32 = 0x40;
const RECTANGLE_WITH_MASK: u32 = 0x80;
const ALL_SRC_TYPE: u32 = 0xFF;
const BASIC_SRC_TYPE: u32 = 0x03;

// Port of: gm/xfermodes.cpp#L40-L72 (chrome/m156), gModes
const MODES: [(BlendMode, u32); 29] = [
    (BlendMode::Clear, ALL_SRC_TYPE),
    (BlendMode::Src, ALL_SRC_TYPE),
    (BlendMode::Dst, ALL_SRC_TYPE),
    (BlendMode::SrcOver, ALL_SRC_TYPE),
    (BlendMode::DstOver, ALL_SRC_TYPE),
    (BlendMode::SrcIn, ALL_SRC_TYPE),
    (BlendMode::DstIn, ALL_SRC_TYPE),
    (BlendMode::SrcOut, ALL_SRC_TYPE),
    (BlendMode::DstOut, ALL_SRC_TYPE),
    (BlendMode::SrcATop, ALL_SRC_TYPE),
    (BlendMode::DstATop, ALL_SRC_TYPE),
    (BlendMode::Xor, BASIC_SRC_TYPE),
    (BlendMode::Plus, BASIC_SRC_TYPE),
    (BlendMode::Modulate, ALL_SRC_TYPE),
    (BlendMode::Screen, BASIC_SRC_TYPE),
    (BlendMode::Overlay, BASIC_SRC_TYPE),
    (BlendMode::Darken, BASIC_SRC_TYPE),
    (BlendMode::Lighten, BASIC_SRC_TYPE),
    (BlendMode::ColorDodge, BASIC_SRC_TYPE),
    (BlendMode::ColorBurn, BASIC_SRC_TYPE),
    (BlendMode::HardLight, BASIC_SRC_TYPE),
    (BlendMode::SoftLight, BASIC_SRC_TYPE),
    (BlendMode::Difference, BASIC_SRC_TYPE),
    (BlendMode::Exclusion, BASIC_SRC_TYPE),
    (BlendMode::Multiply, ALL_SRC_TYPE),
    (BlendMode::Hue, BASIC_SRC_TYPE),
    (BlendMode::Saturation, BASIC_SRC_TYPE),
    (BlendMode::Color, BASIC_SRC_TYPE),
    (BlendMode::Luminosity, BASIC_SRC_TYPE),
];

const W: i32 = 64;
const H: i32 = 64;

// Port of: gm/xfermodes.cpp#L74-L90 (chrome/m156), make_bitmaps
fn make_bitmaps(w: i32, h: i32) -> (Bitmap, Bitmap, Bitmap) {
    let mut src = Bitmap::new();
    src.alloc_n32_pixels((w, h), None);
    src.erase_color(Color::TRANSPARENT);

    let mut p = Paint::default();
    p.set_anti_alias(true);

    let ww = w as scalar;
    let hh = h as scalar;

    {
        let c = Canvas::from_bitmap(&mut src, None).expect("a canvas on the source bitmap");
        p.set_color(color_to_565(0xFFFF_CC44));
        let r = Rect::from_wh(ww * 3.0 / 4.0, hh * 3.0 / 4.0);
        c.draw_oval(r, &p);
    }

    let mut dst = Bitmap::new();
    dst.alloc_n32_pixels((w, h), None);
    dst.erase_color(Color::TRANSPARENT);

    {
        let c = Canvas::from_bitmap(&mut dst, None).expect("a canvas on the destination bitmap");
        p.set_color(color_to_565(0xFF66_AAFF));
        let r = Rect::from_ltrb(ww / 3.0, hh / 3.0, ww * 19.0 / 20.0, hh * 19.0 / 20.0);
        c.draw_rect(r, &p);
    }

    let mut transparent = Bitmap::new();
    transparent.alloc_n32_pixels((w, h), None);
    transparent.erase_color(Color::TRANSPARENT);

    (src, dst, transparent)
}

// Port of: gm/xfermodes.cpp#L92-L97 (chrome/m156), gData: 0xFFFFFFFF, 0xFFCCCCCC, 0xFFCCCCCC,
// 0xFFFFFFFF. The pixels are gray or white with alpha 0xFF, so the byte order of the words does
// not matter.
const G_DATA: [u8; 16] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xCC, 0xCC, 0xCC, 0xFF, 0xCC, 0xCC, 0xCC, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

// Port of: gm/xfermodes.cpp#L99-L186 (chrome/m156), class XfermodesGM
#[derive(Debug, Default)]
pub struct XfermodesGm {
    bg: Bitmap,
    src_b: Bitmap,
    dst_b: Bitmap,
    transparent: Bitmap,
}

impl XfermodesGm {
    // Port of: gm/xfermodes.cpp#L104-L146 (chrome/m156), draw_mode
    // The `kRectangleWithMask` and `kQuarterClear*` cases fall through in C++; the fall-through
    // is written out here.
    fn draw_mode(&self, canvas: &Canvas, mode: BlendMode, src_type: u32, x: scalar, y: scalar) {
        let mut p = Paint::default();
        let sampling = SamplingOptions::default();
        let mut m = Matrix::translate((x, y));
        let mut restore_needed = false;

        canvas.draw_image_with_sampling_options(
            self.src_b.as_image().expect("the source image"),
            (x, y),
            sampling,
            Some(&p),
        );
        p.set_blend_mode(mode);
        match src_type {
            SMALL_TRANSPARENT_IMAGE => {
                m.post_scale((0.5, 0.5), Point::new(x, y));

                canvas.save();
                canvas.concat(&m);
                canvas.draw_image_with_sampling_options(
                    self.transparent.as_image().expect("the transparent image"),
                    (0.0, 0.0),
                    sampling,
                    Some(&p),
                );
                canvas.restore();
            }
            QUARTER_CLEAR_IN_LAYER => {
                let bounds = Rect::from_xywh(x, y, W as scalar, H as scalar);
                canvas.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&p));
                restore_needed = true;
                p.set_blend_mode(BlendMode::SrcOver);
                Self::draw_quarter_clear(canvas, &mut p, x, y);
            }
            QUARTER_CLEAR => {
                Self::draw_quarter_clear(canvas, &mut p, x, y);
            }
            RECTANGLE_WITH_MASK => {
                canvas.save();
                restore_needed = true;
                let w = W as scalar;
                let h = H as scalar;
                let r = Rect::from_xywh(x, y + h / 4.0, w, h * 23.0 / 60.0);
                canvas.clip_rect(r, None, None);
                Self::draw_rectangle(canvas, &mut p, x, y);
            }
            RECTANGLE => {
                Self::draw_rectangle(canvas, &mut p, x, y);
            }
            SMALL_RECTANGLE_IMAGE_WITH_ALPHA => {
                m.post_scale((0.5, 0.5), Point::new(x, y));
                p.set_alpha(0x88);
                self.draw_dst_image(canvas, &m, sampling, &p);
            }
            RECTANGLE_IMAGE_WITH_ALPHA => {
                p.set_alpha(0x88);
                self.draw_dst_image(canvas, &m, sampling, &p);
            }
            RECTANGLE_IMAGE => {
                self.draw_dst_image(canvas, &m, sampling, &p);
            }
            _ => {}
        }

        if restore_needed {
            canvas.restore();
        }
    }

    // The body of `case kQuarterClear_SrcType` (also reached from kQuarterClearInLayer).
    fn draw_quarter_clear(canvas: &Canvas, p: &mut Paint, x: scalar, y: scalar) {
        let half_w = W as scalar / 2.0;
        let half_h = H as scalar / 2.0;
        p.set_color(color_to_565(0xFF66_AAFF));
        let r = Rect::from_xywh(x + half_w, y, half_w, H as scalar);
        canvas.draw_rect(r, p);
        p.set_color(color_to_565(0xFFAA_66FF));
        let r = Rect::from_xywh(x, y + half_h, W as scalar, half_h);
        canvas.draw_rect(r, p);
    }

    // The body of `case kRectangle_SrcType` (also reached from kRectangleWithMask).
    fn draw_rectangle(canvas: &Canvas, p: &mut Paint, x: scalar, y: scalar) {
        let w = W as scalar;
        let h = H as scalar;
        let r = Rect::from_xywh(x + w / 3.0, y + h / 3.0, w * 37.0 / 60.0, h * 37.0 / 60.0);
        p.set_color(color_to_565(0xFF66_AAFF));
        canvas.draw_rect(r, p);
    }

    // The body of `case kRectangleImage_SrcType` (also reached from the alpha cases).
    fn draw_dst_image(&self, canvas: &Canvas, m: &Matrix, sampling: SamplingOptions, p: &Paint) {
        canvas.save();
        canvas.concat(m);
        canvas.draw_image_with_sampling_options(
            self.dst_b.as_image().expect("the destination image"),
            (0.0, 0.0),
            sampling,
            Some(p),
        );
        canvas.restore();
    }
}

impl GM for XfermodesGm {
    fn name(&self) -> String {
        "xfermodes".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1990, 570)
    }

    // Port of: gm/xfermodes.cpp#L173-L179 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.bg = Bitmap::new();
        let info = ImageInfo::new(
            (2, 2),
            skia_rust_core::color_type::ColorType::RGBA8888,
            skia_rust_core::alpha_type::AlphaType::Opaque,
            None,
        );
        assert!(self.bg.install_pixels(&info, G_DATA.to_vec(), 8));

        let (src, dst, transparent) = make_bitmaps(W, H);
        self.src_b = src;
        self.dst_b = dst;
        self.transparent = transparent;
    }

    // Port of: gm/xfermodes.cpp#L181-L238 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((10.0, 20.0));

        let w = W as scalar;
        let h = H as scalar;
        let m = Matrix::scale((6.0, 6.0));
        let s: Option<Shader> = self.bg.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            &m,
        );

        let mut label_p = Paint::default();
        label_p.set_anti_alias(true);

        let font = default_portable_font();

        let k_wrap = 5;

        let mut x0: scalar = 0.0;
        let mut y0: scalar = 0.0;
        let mut source_type: u32 = 1;
        while source_type & ALL_SRC_TYPE != 0 {
            let mut x = x0;
            let mut y = y0;
            for (i, &(mode, mask)) in MODES.iter().enumerate() {
                if mask & source_type == 0 {
                    continue;
                }
                let mut r = Rect::from_ltrb(x, y, x + w, y + h);

                let mut p = Paint::default();
                p.set_style(Style::Fill);
                p.set_shader(s.clone());
                canvas.draw_rect(r, &p);

                canvas.save_layer(&SaveLayerRec::default().bounds(&r));
                self.draw_mode(canvas, mode, source_type, r.left(), r.top());
                canvas.restore();

                r.inset((-0.5, -0.5));
                p.set_style(Style::Stroke);
                p.set_shader(None);
                canvas.draw_rect(r, &p);

                let label = mode.name();
                text_utils::draw_string(
                    canvas,
                    label,
                    x + w / 2.0,
                    y - font.size() / 2.0,
                    &font,
                    &label_p,
                    Align::Center,
                );
                x += w + 10.0;
                if i % k_wrap == k_wrap - 1 {
                    x = x0;
                    y += h + 30.0;
                }
            }
            if y < 320.0 {
                if x > x0 {
                    y += h + 30.0;
                }
                y0 = y;
            } else {
                x0 += 400.0;
                y0 = 0.0;
            }
            source_type <<= 1;
        }
    }
}

// Port of: gm/xfermodes.cpp#L240-L240 (chrome/m156), DEF_GM( return new XfermodesGM; )
crate::def_gm!(XfermodesGM, XfermodesGm::default());

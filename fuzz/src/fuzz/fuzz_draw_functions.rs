// Copyright 2016 Mozilla Foundation
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzDrawFunctions.cpp (chrome/m156)
//
// skia-rust: the `SkDEBUGF` traces are not ported (diagnostics only).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::typeface::Typeface;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils;

use crate::{Fuzz, def_fuzz};

// Port of: fuzz/FuzzDrawFunctions.cpp#L22-L26 (chrome/m156)
const BMP_SIZE: i32 = 24;
const MAX_X: u8 = 250;
const MAX_Y: u8 = 250;
const PTS_LEN: usize = 10;
const TXT_LEN: usize = 5;

/// Fills `str` with printable ASCII and a terminating NUL.
///
/// `char` is signed on x86 (the oracle's and OSS-Fuzz's targets), so `nextRange` reads an `i8`:
/// bytes from 0x80 clamp up to 0x20.
// Port of: fuzz/FuzzDrawFunctions.cpp#L28-L33 (chrome/m156)
fn init_string(fuzz: &mut Fuzz<'_>, str: &mut [u8]) {
    let buf_size = str.len();
    for c in &mut str[..buf_size - 1] {
        let v: i8 = fuzz.next_range(0x20, 0x7E); // printable ASCII
        *c = v.cast_unsigned();
    }
    str[buf_size - 1] = 0;
}

/// `(SkPaint::Cap)v`, `(SkPaint::Join)v` and `(SkPaint::Style)v` for `v` in `[0, 2]`.
fn cap(v: u8) -> Cap {
    match v {
        0 => Cap::Butt,
        1 => Cap::Round,
        _ => Cap::Square,
    }
}

fn join(v: u8) -> Join {
    match v {
        0 => Join::Miter,
        1 => Join::Round,
        _ => Join::Bevel,
    }
}

fn style(v: u8) -> Style {
    match v {
        0 => Style::Fill,
        1 => Style::Stroke,
        _ => Style::StrokeAndFill,
    }
}

// make_paint mostly borrowed from FilterFuzz.cpp
// Port of: fuzz/FuzzDrawFunctions.cpp#L35-L68 (chrome/m156)
fn init_paint(fuzz: &mut Fuzz<'_>, p: &mut Paint) {
    let mut b = fuzz.next_bool();
    p.set_anti_alias(b);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // kLastMode is 28
    let mut tmp_u8: u8 = fuzz.next_range(0, BlendMode::Luminosity as u8);
    p.set_blend_mode(BlendMode::from_i32(i32::from(tmp_u8)).expect("tmp_u8 <= kLastMode"));

    let co: u32 = fuzz.next();
    p.set_color(Color::from(co));

    b = fuzz.next_bool();
    p.set_dither(b);

    tmp_u8 = fuzz.next_range(0, Cap::Square as u8);
    p.set_stroke_cap(cap(tmp_u8));

    tmp_u8 = fuzz.next_range(0, Join::Bevel as u8);
    p.set_stroke_join(join(tmp_u8));

    let mut sc: scalar = fuzz.next();
    p.set_stroke_miter(sc);

    sc = fuzz.next();
    p.set_stroke_width(sc);

    tmp_u8 = fuzz.next_range(0, Style::StrokeAndFill as u8);
    p.set_style(style(tmp_u8));
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L70-L101 (chrome/m156)
fn init_bitmap(fuzz: &mut Fuzz<'_>, bmp: &mut Bitmap) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // kLastEnum_SkColorType fits
    let mut color_type: u8 = fuzz.next_range(0, ColorType::LAST_ENUM as u8);
    // ColorType needs to match what the system configuration is.
    if color_type == ColorType::RGBA8888 as u8 || color_type == ColorType::BGRA8888 as u8 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a color type fits
        {
            color_type = ColorType::N32 as u8;
        }
    }
    let b = fuzz.next_bool();
    let info = ImageInfo::new(
        (BMP_SIZE, BMP_SIZE),
        ColorType::from_i32(i32::from(color_type)).expect("color_type <= kLastEnum_SkColorType"),
        if b {
            AlphaType::Opaque
        } else {
            AlphaType::Premul
        },
        None,
    );
    // skia-rust: `SkDEBUGF("Bitmap not allocated\n")` on failure is not ported.
    let _ = bmp.try_alloc_pixels_info(&info, None);
    let mut c: u32 = fuzz.next();
    bmp.erase_color(Color::from(c));

    let b = fuzz.next_bool();
    let mut p = Paint::default();
    if b {
        init_paint(fuzz, &mut p);
    } else {
        c = fuzz.next();
        p.set_color(Color::from(c));
    }
}

/// `SkSurfaces::Raster(SkImageInfo::MakeN32Premul(x, y))`. Skia falls back to `SkSurfaces::Null`
/// when the allocation fails (in memory-constrained fuzzing environments); a surface of at most
/// 250x250 always allocates here, so the fallback is not ported.
// Port of: fuzz/FuzzDrawFunctions.cpp#L103-L113 (chrome/m156)
fn init_surface(fuzz: &mut Fuzz<'_>) -> Surface<'static> {
    let x: u8 = fuzz.next_range(1, MAX_X);
    let y: u8 = fuzz.next_range(1, MAX_Y);
    surfaces::raster_n32_premul((i32::from(x), i32::from(y)))
        .expect("a raster surface of at most 250x250 always allocates")
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L116-L159 (chrome/m156)
fn fuzz_draw_text(fuzz: &mut Fuzz<'_>, typeface: Typeface) {
    let mut font = Font::from_typeface(Some(typeface));
    let mut p = Paint::default();
    init_paint(fuzz, &mut p);
    let mut surface = init_surface(fuzz);

    let mut text = [0u8; TXT_LEN];
    init_string(fuzz, &mut text);

    let (mut x, mut y): (scalar, scalar) = (fuzz.next(), fuzz.next());
    // populate pts array
    let mut pts = [Point::default(); PTS_LEN];
    for pt in &mut pts {
        pt.set(x, y);
        x += font.size();
    }

    let mut b = fuzz.next_bool();
    font.set_force_auto_hinting(b);
    b = fuzz.next_bool();
    font.set_embedded_bitmaps(b);
    b = fuzz.next_bool();
    font.set_embolden(b);
    b = fuzz.next_bool();
    font.set_edging(if b {
        Edging::AntiAlias
    } else {
        Edging::SubpixelAntiAlias
    });
    b = fuzz.next_bool();
    font.set_linear_metrics(b);
    b = fuzz.next_bool();
    font.set_subpixel(b);
    x = fuzz.next();
    font.set_scale_x(x);
    x = fuzz.next();
    font.set_skew_x(x);
    x = fuzz.next();
    font.set_size(x);

    let cnv = surface.canvas();
    x = fuzz.next();
    y = fuzz.next();
    // `drawTextBlob` returns early on a null blob.
    if let Some(blob) =
        TextBlob::from_pos_text(&text[..TXT_LEN - 1], TextEncoding::UTF8, &pts, &font)
    {
        cnv.draw_text_blob(&blob, (x, y), &p);
    }
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L161-L170 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn fuzz_draw_circle(fuzz: &mut Fuzz<'_>) {
    let mut p = Paint::default();
    init_paint(fuzz, &mut p);
    let mut surface = init_surface(fuzz);

    let (a, b, c): (scalar, scalar, scalar) = (fuzz.next(), fuzz.next(), fuzz.next());
    surface.canvas().draw_circle((a, b), c, &p);
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L172-L181 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn fuzz_draw_line(fuzz: &mut Fuzz<'_>) {
    let mut p = Paint::default();
    init_paint(fuzz, &mut p);
    let mut surface = init_surface(fuzz);

    let (a, b, c, d): (scalar, scalar, scalar, scalar) =
        (fuzz.next(), fuzz.next(), fuzz.next(), fuzz.next());
    surface.canvas().draw_line((a, b), (c, d), &p);
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L183-L201 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn fuzz_draw_rect(fuzz: &mut Fuzz<'_>) {
    let mut p = Paint::default();
    init_paint(fuzz, &mut p);
    let mut surface = init_surface(fuzz);

    let (mut a, mut b, mut c, mut d): (scalar, scalar, scalar, scalar) =
        (fuzz.next(), fuzz.next(), fuzz.next(), fuzz.next());
    let mut r = Rect::from_xywh(a, b, c, d);

    let cnv = surface.canvas();
    cnv.draw_rect(r, &p);

    let bl = fuzz.next_bool();
    (a, b, c, d) = (fuzz.next(), fuzz.next(), fuzz.next(), fuzz.next());
    r = Rect::from_xywh(a, b, c, d);
    cnv.clip_rect(r, ClipOp::Intersect, bl);
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L203-L253 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
#[allow(clippy::neg_cmp_op_on_partial_ord)] // `!(f > 0)`, true for NaN as in C++
fn fuzz_draw_path(fuzz: &mut Fuzz<'_>) {
    let mut p = Paint::default();
    init_paint(fuzz, &mut p);
    let mut surface = init_surface(fuzz);

    // TODO(kjlubick): put the ability to fuzz a path in shared file, with
    // other common things (e.g. rects, lines)
    let i: u8 = fuzz.next_range(0, 10); // set i to number of operations to perform
    let mut builder = PathBuilder::new();
    for _ in 0..i {
        let j: u8 = fuzz.next_range(0, 5); // set j to choose operation to perform
        match j {
            0 => {
                let (a, b): (scalar, scalar) = (fuzz.next(), fuzz.next());
                builder.move_to((a, b));
            }
            1 => {
                let (a, b): (scalar, scalar) = (fuzz.next(), fuzz.next());
                builder.line_to((a, b));
            }
            2 => {
                let (a, b, c, d): (scalar, scalar, scalar, scalar) =
                    (fuzz.next(), fuzz.next(), fuzz.next(), fuzz.next());
                builder.quad_to((a, b), (c, d));
            }
            3 => {
                let (a, b, c, d, mut e): (scalar, scalar, scalar, scalar, scalar) = (
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                );
                if !(e > 0.0) {
                    e = 1.0;
                }
                builder.conic_to((a, b), (c, d), e);
            }
            4 => {
                let (a, b, c, d, e, f): (scalar, scalar, scalar, scalar, scalar, scalar) = (
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                );
                builder.cubic_to((a, b), (c, d), (e, f));
            }
            5 => {
                let (a, b, c, d, e): (scalar, scalar, scalar, scalar, scalar) = (
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                );
                builder.arc_to_tangent((a, b), (c, d), e);
            }
            _ => {}
        }
    }
    let path = builder.close().detach();

    let cnv = surface.canvas();
    cnv.draw_path(&path, &p);

    let bl = fuzz.next_bool();
    cnv.clip_path(&path, ClipOp::Intersect, bl);
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L255-L281 (chrome/m156)
fn fuzz_draw_image(fuzz: &mut Fuzz<'_>) {
    let mut p = Paint::default();
    init_paint(fuzz, &mut p);
    let mut surface = init_surface(fuzz);
    let mut bmp = Bitmap::new();
    init_bitmap(fuzz, &mut bmp);

    let image = bmp.as_image();

    let bl = fuzz.next_bool();
    let (mut a, mut b): (scalar, scalar) = (fuzz.next(), fuzz.next());
    // `drawImage` and `drawImageRect` return early on a null image.
    if bl {
        if let Some(image) = &image {
            surface.canvas().draw_image_with_sampling_options(
                image,
                (a, b),
                SamplingOptions::default(),
                Some(&p),
            );
        }
    } else {
        let dst = Rect::from_wh(a, b);
        (a, b) = (fuzz.next(), fuzz.next());
        let src = Rect::from_wh(a, b);
        let x: u8 = fuzz.next_range(0, 1);
        let cst = if x == 0 {
            SrcRectConstraint::Strict
        } else {
            SrcRectConstraint::Fast
        };
        if let Some(image) = &image {
            surface.canvas().draw_image_rect_with_sampling_options(
                image,
                Some((&src, cst)),
                dst,
                SamplingOptions::default(),
                &p,
            );
        }
    }
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L283-L291 (chrome/m156)
fn fuzz_draw_paint(fuzz: &mut Fuzz<'_>) {
    let mut p = Paint::default();
    init_paint(fuzz, &mut p);
    let mut surface = init_surface(fuzz);

    surface.canvas().draw_paint(&p);
}

// Port of: fuzz/FuzzDrawFunctions.cpp#L293-L333 (chrome/m156)
def_fuzz!(DrawFunctions, fuzz_DrawFunctions, |fuzz| {
    let i: u8 = fuzz.next();

    match i {
        0 => {
            // `DefaultPortableTypeface` never returns null here, so the `signalBug` branch is
            // unreachable.
            let f = font_tool_utils::default_portable_typeface();
            fuzz_draw_text(fuzz, f);
        }
        1 => fuzz_draw_rect(fuzz),
        2 => fuzz_draw_circle(fuzz),
        3 => fuzz_draw_line(fuzz),
        4 => fuzz_draw_path(fuzz),
        5 => fuzz_draw_image(fuzz),
        6 => fuzz_draw_paint(fuzz),
        _ => {}
    }
});

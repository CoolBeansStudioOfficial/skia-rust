// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: fuzz/FuzzCanvasHelpers.cpp, fuzz/FuzzCanvasHelpers.h

//! The random builders that `FuzzCanvas` draws with (`FuzzCanvasHelpers.cpp`): colour filters,
//! shaders, path effects, mask filters, typefaces, image filters, images, pictures, paints, fonts
//! and text blobs, and `FuzzCanvas` itself.
//!
//! Every builder reads its bytes in the C++ order, so a given input makes the same reads as
//! Skia's. Where a call is not ported, its reads are kept and the call is left out: the reader
//! position stays in sync with Skia's, and only the drawing differs. Those places are marked
//! `skia-rust:` below.

// The reader's counts, sizes and enum values are clamped by `nextRange` and `nextEnum` before the
// `as` casts below, so they cannot wrap or change sign; the casts mirror the C++ ones.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::{Canvas, PointMode, SaveLayerRec, SrcRectConstraint};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::{Color, Color4f, ColorChannel, pre_multiply_color};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_mgr::FontStyleSet;
use skia_rust_core::font_types::{FontHinting, TextEncoding};
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::lattice_iter::Lattice;
use skia_rust_core::m44::M44;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::region::Region;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::vertices::{VertexMode, Vertices};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::high_contrast_filter::{
    HighContrastConfig, HighContrastFilter, InvertStyle,
};
use skia_rust_effects::image_filters;
use skia_rust_effects::luma_color_filter;
use skia_rust_effects::path_1d_path_effect::Style as Style1D;
use skia_rust_effects::perlin_noise_shader::shaders as perlin_shaders;
use skia_rust_effects::{
    corner_path_effect, dash_path_effect, discrete_path_effect, line_2d_path_effect,
    path_1d_path_effect, path_2d_path_effect,
};
use skia_rust_raster::bitmap_device::BitmapDevice;
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_tools::font_tool_utils::{default_font, test_font_mgr};

use crate::Fuzz;
use crate::common::{fuzz_nice_path, nice_matrix, nice_rrect};

/// `kMaxGlyphCount`.
const MAX_GLYPH_COUNT: i32 = 30;

/// The Unicode ranges `make_fuzz_text` draws characters from (`ranges`).
const RANGES: [[u32; 2]; 3] = [[0x0020, 0x007F], [0x00A1, 0x0250], [0x0400, 0x0500]];

// ---------------------------------------------------------------------------------------------
// Reads of Skia's value types, in their memory layout (`Fuzz::next(&T)` copies `sizeof(T)`).

/// `next(&SkPoint)`: two floats.
fn next_point(fuzz: &mut Fuzz<'_>) -> Point {
    let x: f32 = fuzz.next();
    let y: f32 = fuzz.next();
    Point::new(x, y)
}

/// `next(&SkRect)`: left, top, right, bottom, unsorted.
fn next_rect(fuzz: &mut Fuzz<'_>) -> Rect {
    let l: f32 = fuzz.next();
    let t: f32 = fuzz.next();
    let r: f32 = fuzz.next();
    let b: f32 = fuzz.next();
    Rect::new(l, t, r, b)
}

/// `next(&SkIRect)`: left, top, right, bottom, unsorted.
fn next_irect(fuzz: &mut Fuzz<'_>) -> IRect {
    let l: i32 = fuzz.next();
    let t: i32 = fuzz.next();
    let r: i32 = fuzz.next();
    let b: i32 = fuzz.next();
    IRect::new(l, t, r, b)
}

/// `next(&SkPoint3)`: three floats.
fn next_point3(fuzz: &mut Fuzz<'_>) -> Point3 {
    let x: f32 = fuzz.next();
    let y: f32 = fuzz.next();
    let z: f32 = fuzz.next();
    Point3::new(x, y, z)
}

/// `nextN(out, N)` for `N` floats.
fn next_f32s<const N: usize>(fuzz: &mut Fuzz<'_>) -> [f32; N] {
    std::array::from_fn(|_| fuzz.next())
}

// ---------------------------------------------------------------------------------------------
// Enums. Each is read as its underlying integer (`nextEnum` / `nextRange`) and mapped to the
// Rust enum with the same value. The Skia enums without a fixed type read as 4 bytes.

/// `SkBlendMode` from a value clamped to `[0, kLastMode]` (`SkBlendMode` has `int` as its type).
fn blend_mode(v: i32) -> BlendMode {
    BlendMode::from_i32(v).unwrap_or(BlendMode::Clear)
}

/// `SkTileMode` from a value clamped to `[0, kLastTileMode]`.
fn tile_mode(v: i32) -> TileMode {
    match v {
        0 => TileMode::Clamp,
        1 => TileMode::Repeat,
        2 => TileMode::Mirror,
        _ => TileMode::Decal,
    }
}

/// `SkColorChannel` from a value clamped to `[0, kLastEnum]`.
fn color_channel(v: i32) -> ColorChannel {
    match v {
        0 => ColorChannel::R,
        1 => ColorChannel::G,
        2 => ColorChannel::B,
        _ => ColorChannel::A,
    }
}

/// `SkFilterMode` from a value clamped to `[0, kLast]`.
fn filter_mode(v: i32) -> FilterMode {
    if v == 0 {
        FilterMode::Nearest
    } else {
        FilterMode::Linear
    }
}

/// `SkMipmapMode` from a value clamped to `[0, kLast]`.
fn mipmap_mode(v: i32) -> MipmapMode {
    match v {
        0 => MipmapMode::None,
        1 => MipmapMode::Nearest,
        _ => MipmapMode::Linear,
    }
}

/// `SkTextEncoding` from a value clamped to `[0, kUTF32]` or `kGlyphID`.
fn text_encoding(v: i32) -> TextEncoding {
    match v {
        0 => TextEncoding::UTF8,
        1 => TextEncoding::UTF16,
        2 => TextEncoding::UTF32,
        _ => TextEncoding::GlyphId,
    }
}

/// `SkFontHinting` from a value clamped to `[0, kFull]`.
fn font_hinting(v: i32) -> FontHinting {
    match v {
        0 => FontHinting::None,
        1 => FontHinting::Slight,
        2 => FontHinting::Normal,
        _ => FontHinting::Full,
    }
}

/// `SkFont::Edging` from a value clamped to `[0, kSubpixelAntiAlias]`.
fn edging(v: i32) -> Edging {
    match v {
        0 => Edging::Alias,
        1 => Edging::AntiAlias,
        _ => Edging::SubpixelAntiAlias,
    }
}

/// `SkPaint::Style` from a value clamped to `[0, kStrokeAndFill_Style]` (a `uint8_t`).
fn paint_style(v: u8) -> Style {
    match v {
        0 => Style::Fill,
        1 => Style::Stroke,
        _ => Style::StrokeAndFill,
    }
}

/// `SkPaint::Cap` from a value clamped to `[0, kLast_Cap]`.
fn paint_cap(v: i32) -> Cap {
    match v {
        0 => Cap::Butt,
        1 => Cap::Round,
        _ => Cap::Square,
    }
}

/// `SkPaint::Join` from a value clamped to `[0, kLast_Join]` (a `uint8_t`).
fn paint_join(v: u8) -> Join {
    match v {
        0 => Join::Miter,
        1 => Join::Round,
        _ => Join::Bevel,
    }
}

/// `SkBlurStyle` from a value clamped to `[0, kLastEnum_SkBlurStyle]`.
fn blur_style(v: i32) -> BlurStyle {
    BlurStyle::from_i32(v).unwrap_or(BlurStyle::Normal)
}

/// `SkClipOp` from `0` or `1`.
fn clip_op(v: i32) -> ClipOp {
    if v == 0 {
        ClipOp::Difference
    } else {
        ClipOp::Intersect
    }
}

// ---------------------------------------------------------------------------------------------
// Colour filters.

// Port of: fuzz/FuzzCanvasHelpers.cpp#L91-L159 (chrome/m156)
fn make_fuzz_colorfilter(fuzz: &mut Fuzz<'_>, depth: i32) -> Option<ColorFilter> {
    if depth <= 0 {
        return None;
    }
    let color_filter_type: i32 = fuzz.next_range(0, 8);
    match color_filter_type {
        // `0` is no colour filter, and the default is unreachable (the type is in `0..=8`).
        1 => {
            let color: u32 = fuzz.next();
            let mode = blend_mode(fuzz.next_enum(BlendMode::Luminosity as i32));
            color_filters::blend(Color4f::from_color(Color::from(color)), None, mode)
        }
        2 => {
            let outer = make_fuzz_colorfilter(fuzz, depth - 1)?;
            let inner = make_fuzz_colorfilter(fuzz, depth - 1);
            // makeComposed should be able to handle nullptr.
            Some(outer.composed(inner))
        }
        3 => {
            let array: [f32; 20] = next_f32s(fuzz);
            color_filters::matrix_row_major(&array, color_filters::Clamp::Yes)
        }
        4 => {
            let mul: u32 = fuzz.next();
            let add: u32 = fuzz.next();
            color_filters::lighting(Color::from(mul), Color::from(add))
        }
        5 => {
            let grayscale = fuzz.next_bool();
            let invert_style: i32 = fuzz.next_range(0, 2);
            let contrast: f32 = fuzz.next_range_f32(-1.0, 1.0);
            let config =
                HighContrastConfig::new(grayscale, InvertStyle::from_raw(invert_style), contrast);
            HighContrastFilter::make(&config)
        }
        6 => luma_color_filter::make(),
        7 => {
            let mut table = [0u8; 256];
            for v in &mut table {
                *v = fuzz.next();
            }
            Some(color_filters::table(&table))
        }
        8 => {
            let mut table_a = [0u8; 256];
            let mut table_r = [0u8; 256];
            let mut table_g = [0u8; 256];
            let mut table_b = [0u8; 256];
            for table in [&mut table_a, &mut table_r, &mut table_g, &mut table_b] {
                for v in table.iter_mut() {
                    *v = fuzz.next();
                }
            }
            color_filters::table_argb(
                Some(&table_a),
                Some(&table_r),
                Some(&table_g),
                Some(&table_b),
            )
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Shaders.

// Port of: fuzz/FuzzCanvasHelpers.cpp#L161-L173 (chrome/m156)
fn fuzz_gradient_stops(fuzz: &mut Fuzz<'_>, pos: &mut [scalar]) {
    let mut total_pos: scalar = 0.0;
    for p in pos.iter_mut() {
        *p = fuzz.next_range_f32(1.0, 1024.0);
        total_pos += *p;
    }
    total_pos = 1.0 / total_pos;
    for p in pos.iter_mut() {
        *p *= total_pos;
    }
    // SkASSERT(fabs(pos[colorCount - 1] - 1.0f) < 0.00001f);
    if let Some(last) = pos.last_mut() {
        *last = 1.0;
    }
}

/// `GradStorage::grad`: the colours as `Color4f`, with the positions if any.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L175-L190 (chrome/m156)
fn grad_storage(
    colors: &[u32],
    positions: Option<&[scalar]>,
    tm: TileMode,
) -> (Vec<Color4f>, TileMode, Option<Vec<scalar>>) {
    let c4: Vec<Color4f> = colors
        .iter()
        .map(|&c| Color4f::from_color(Color::from(c)))
        .collect();
    (c4, tm, positions.map(<[scalar]>::to_vec))
}

/// `make_fuzz_shader`: a shader of at most `depth` levels of nesting (no shader beyond it).
// Port of: fuzz/FuzzCanvasHelpers.cpp#L192-L375 (chrome/m156)
#[allow(clippy::too_many_lines)] // one arm per C++ `case`, as in the port
fn make_fuzz_shader(fuzz: &mut Fuzz<'_>, depth: i32) -> Option<Shader> {
    if depth <= 0 {
        return None;
    }
    let shader_type: i32 = fuzz.next_range(0, 14);
    match shader_type {
        // `0` and `4` are no shader, as is `9` (the deprecated gaussian-edge shader).
        1 => Some(shaders::empty()),
        2 => {
            let color: u32 = fuzz.next();
            Some(shaders::color(Color::from(color)))
        }
        3 => {
            let img = make_fuzz_image(fuzz);
            let tm_x = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let tm_y = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let use_matrix = fuzz.next_bool();
            let matrix = use_matrix.then(|| nice_matrix(fuzz));
            img.to_shader((tm_x, tm_y), SamplingOptions::default(), matrix.as_ref())
        }
        5 => {
            let shader1 = make_fuzz_shader(fuzz, depth - 1); // limit recursion.
            let matrix = nice_matrix(fuzz);
            shader1.map(|s| s.with_local_matrix(&matrix))
        }
        6 => {
            let shader1 = make_fuzz_shader(fuzz, depth - 1); // limit recursion.
            let color_filter = make_fuzz_colorfilter(fuzz, depth - 1);
            let shader1 = shader1?;
            // makeWithColorFilter(nullptr) returns the shader itself.
            Some(match color_filter {
                Some(cf) => shader1.with_color_filter(cf),
                None => shader1,
            })
        }
        7 => {
            let shader1 = make_fuzz_shader(fuzz, depth - 1); // limit recursion.
            let shader2 = make_fuzz_shader(fuzz, depth - 1);
            let blend = blend_mode(fuzz.next_enum(BlendMode::Luminosity as i32));
            // SkShaders::Blend returns nullptr when either shader is null.
            match (shader1, shader2) {
                (Some(dst), Some(src)) => Some(shaders::blend(blend, dst, src)),
                _ => None,
            }
        }
        8 => {
            let pic = make_fuzz_picture(fuzz, depth - 1);
            let tm_x = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let tm_y = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let use_matrix = fuzz.next_bool();
            let use_tile = fuzz.next_bool();
            let matrix = use_matrix.then(|| nice_matrix(fuzz));
            let tile = use_tile.then(|| next_rect(fuzz));
            // `pic->makeShader(tmX, tmY, SkFilterMode::kNearest, matrix, tile)`
            pic.and_then(|pic| {
                pic.to_shader(
                    (tm_x, tm_y),
                    FilterMode::Nearest,
                    matrix.as_ref(),
                    tile.as_ref(),
                )
            })
        }
        // EFFECTS:
        10 => {
            const MAX_COLORS: i32 = 12;
            let pts = [next_point(fuzz), next_point(fuzz)];
            let color_count: i32 = fuzz.next_range(2, MAX_COLORS);
            let colors: Vec<u32> = fuzz.next_n(color_count as usize);
            let tm_x = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let use_matrix = fuzz.next_bool();
            let use_pos = fuzz.next_bool();
            let matrix = use_matrix.then(|| nice_matrix(fuzz));
            let mut pos = vec![0.0; color_count as usize];
            if use_pos {
                fuzz_gradient_stops(fuzz, &mut pos);
            }
            let (c4, tm, p) = grad_storage(&colors, use_pos.then_some(&pos[..]), tm_x);
            let grad = Gradient::new(
                Colors::new(&c4, p.as_deref(), tm, None),
                Interpolation::default(),
            );
            gradient_shaders::linear_gradient((pts[0], pts[1]), &grad, matrix.as_ref())
        }
        11 => {
            const MAX_COLORS: i32 = 12;
            let tm_x = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let use_matrix = fuzz.next_bool();
            let use_pos = fuzz.next_bool();
            let center = next_point(fuzz);
            let radius: scalar = fuzz.next();
            let color_count: i32 = fuzz.next_range(2, MAX_COLORS);
            let colors: Vec<u32> = fuzz.next_n(color_count as usize);
            let matrix = use_matrix.then(|| nice_matrix(fuzz));
            let mut pos = vec![0.0; color_count as usize];
            if use_pos {
                fuzz_gradient_stops(fuzz, &mut pos);
            }
            let (c4, tm, p) = grad_storage(&colors, use_pos.then_some(&pos[..]), tm_x);
            let grad = Gradient::new(
                Colors::new(&c4, p.as_deref(), tm, None),
                Interpolation::default(),
            );
            gradient_shaders::radial_gradient((center, radius), &grad, matrix.as_ref())
        }
        12 => {
            const MAX_COLORS: i32 = 12;
            let tm_x = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let use_matrix = fuzz.next_bool();
            let use_pos = fuzz.next_bool();
            let start_radius: scalar = fuzz.next();
            let end_radius: scalar = fuzz.next();
            let start = next_point(fuzz);
            let end = next_point(fuzz);
            let color_count: i32 = fuzz.next_range(2, MAX_COLORS);
            let colors: Vec<u32> = fuzz.next_n(color_count as usize);
            let matrix = use_matrix.then(|| nice_matrix(fuzz));
            let mut pos = vec![0.0; color_count as usize];
            if use_pos {
                fuzz_gradient_stops(fuzz, &mut pos);
            }
            let (c4, tm, p) = grad_storage(&colors, use_pos.then_some(&pos[..]), tm_x);
            let grad = Gradient::new(
                Colors::new(&c4, p.as_deref(), tm, None),
                Interpolation::default(),
            );
            gradient_shaders::two_point_conical_gradient(
                (start, start_radius),
                (end, end_radius),
                &grad,
                matrix.as_ref(),
            )
        }
        13 => {
            const MAX_COLORS: i32 = 12;
            let cx: scalar = fuzz.next();
            let cy: scalar = fuzz.next();
            let use_matrix = fuzz.next_bool();
            let use_pos = fuzz.next_bool();
            let color_count: i32 = fuzz.next_range(2, MAX_COLORS);
            let colors: Vec<u32> = fuzz.next_n(color_count as usize);
            let matrix = use_matrix.then(|| nice_matrix(fuzz));
            let mut pos = vec![0.0; color_count as usize];
            if use_pos {
                fuzz_gradient_stops(fuzz, &mut pos);
            }
            let (c4, tm, p) = grad_storage(&colors, use_pos.then_some(&pos[..]), TileMode::Clamp);
            let grad = Gradient::new(
                Colors::new(&c4, p.as_deref(), tm, None),
                Interpolation::default(),
            );
            gradient_shaders::sweep_gradient((cx, cy), (0.0, 360.0), &grad, matrix.as_ref())
        }
        14 => {
            let base_frequency_x: scalar = fuzz.next();
            let base_frequency_y: scalar = fuzz.next();
            let seed: scalar = fuzz.next();
            let use_tile_size = fuzz.next_bool();
            let turbulence = fuzz.next_bool();
            let tile_size = use_tile_size.then(|| {
                let w: i32 = fuzz.next();
                let h: i32 = fuzz.next();
                skia_rust_core::size::ISize::new(w, h)
            });
            let num_octaves: i32 = fuzz.next_range(2, 7);
            let octaves = num_octaves as usize;
            if turbulence {
                perlin_shaders::turbulence(
                    (base_frequency_x, base_frequency_y),
                    octaves,
                    seed,
                    tile_size,
                )
            } else {
                perlin_shaders::fractal_noise(
                    (base_frequency_x, base_frequency_y),
                    octaves,
                    seed,
                    tile_size,
                )
            }
        }
        // `default: SkDEBUGFAILF("Unknown shader type")`: unreachable, the type is in `0..=14`.
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Path effects.

/// `SkPathEffect::MakeSum` / `MakeCompose` with a null operand: the other operand (a null pair
/// makes nothing).
// Port of: src/core/SkPathEffect.cpp#L84-L92 and #L146-L154 (chrome/m156), SkComposePathEffect::Make and SkSumPathEffect::Make
fn pair_path_effect(
    first: Option<PathEffect>,
    second: Option<PathEffect>,
    make: fn(PathEffect, PathEffect) -> PathEffect,
) -> Option<PathEffect> {
    match (first, second) {
        (None, second) => second,
        (first, None) => first,
        (Some(first), Some(second)) => Some(make(first, second)),
    }
}

// Port of: fuzz/FuzzCanvasHelpers.cpp#L377-L442 (chrome/m156)
fn make_fuzz_patheffect(fuzz: &mut Fuzz<'_>, depth: i32) -> Option<PathEffect> {
    if depth <= 0 {
        return None;
    }
    let path_effect_type: u8 = fuzz.next_range(0, 8);
    match path_effect_type {
        // `0` is no path effect, and the default is unreachable (the type is in `0..=8`).
        1 => {
            let first = make_fuzz_patheffect(fuzz, depth - 1);
            let second = make_fuzz_patheffect(fuzz, depth - 1);
            pair_path_effect(first, second, PathEffect::sum)
        }
        2 => {
            let first = make_fuzz_patheffect(fuzz, depth - 1);
            let second = make_fuzz_patheffect(fuzz, depth - 1);
            pair_path_effect(first, second, PathEffect::compose)
        }
        3 => {
            let path: Path = fuzz_nice_path(fuzz, 20);
            let advance: scalar = fuzz.next();
            let phase: scalar = fuzz.next();
            let style: i32 = fuzz.next_enum(Style1D::Morph as i32);
            let style = match style {
                0 => Style1D::Translate,
                1 => Style1D::Rotate,
                _ => Style1D::Morph,
            };
            path_1d_path_effect::new(&path, advance, phase, style)
        }
        4 => {
            let width: scalar = fuzz.next();
            let matrix = nice_matrix(fuzz);
            line_2d_path_effect::new(width, &matrix)
        }
        5 => {
            let path = fuzz_nice_path(fuzz, 20);
            let matrix = nice_matrix(fuzz);
            Some(path_2d_path_effect::new(&matrix, &path))
        }
        6 => {
            let radius: scalar = fuzz.next();
            corner_path_effect::new(radius)
        }
        7 => {
            let phase: scalar = fuzz.next();
            let mut intervals = [0.0; 20];
            let count: i32 = fuzz.next_range(0, intervals.len() as i32);
            for v in intervals.iter_mut().take(count as usize) {
                *v = fuzz.next();
            }
            dash_path_effect::new(&intervals[..count as usize], phase)
        }
        8 => {
            let seg_length: scalar = fuzz.next();
            let dev: scalar = fuzz.next();
            let seed: u32 = fuzz.next();
            discrete_path_effect::new(seg_length, dev, seed)
        }
        // `default: SkASSERT(false)`: unreachable, the type is in `0..=8`.
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Mask filters, typefaces.

// Port of: fuzz/FuzzCanvasHelpers.cpp#L444-L463 (chrome/m156)
fn make_fuzz_maskfilter(fuzz: &mut Fuzz<'_>) -> Option<MaskFilter> {
    let maskfilter_type: i32 = fuzz.next_range(0, 1);
    match maskfilter_type {
        1 => {
            let blur_style = blur_style(fuzz.next_enum(BlurStyle::Inner as i32));
            let sigma: scalar = fuzz.next();
            let respect_ctm = fuzz.next_bool();
            MaskFilter::blur(blur_style, sigma, respect_ctm)
        }
        _ => None,
    }
}

// Port of: fuzz/FuzzCanvasHelpers.cpp#L465-L477 (chrome/m156)
fn make_fuzz_typeface(fuzz: &mut Fuzz<'_>) -> Option<Typeface> {
    if fuzz.next_bool() {
        return None;
    }
    let mgr = test_font_mgr();
    let family_count = mgr.count_families() as i32;
    let i: i32 = fuzz.next_range(0, family_count - 1);
    // A negative index names no family: the empty style set, with no typeface in it.
    let family = usize::try_from(i).ok().map(|i| mgr.create_style_set(i));
    let style_count = family.as_ref().map_or(0, FontStyleSet::count) as i32;
    let j: i32 = fuzz.next_range(0, style_count - 1);
    let family = family?;
    usize::try_from(j)
        .ok()
        .and_then(|j| family.create_typeface(j))
}

// ---------------------------------------------------------------------------------------------
// Image filters.

// Port of: fuzz/FuzzCanvasHelpers.cpp#L479-L537 (chrome/m156)
#[allow(clippy::too_many_lines)] // one arm per C++ `case`, as in the port
fn make_fuzz_lighting_imagefilter(fuzz: &mut Fuzz<'_>, depth: i32) -> Option<ImageFilter> {
    if depth <= 0 {
        return None;
    }
    let image_filter_type: u8 = fuzz.next_range(1, 6);
    let use_crop_rect = fuzz.next_bool();
    let crop_rect = use_crop_rect.then(|| next_irect(fuzz));
    let crop = crop_rect.map(Rect::from);
    match image_filter_type {
        1 => {
            let p = next_point3(fuzz);
            let light_color: u32 = fuzz.next();
            let surface_scale: scalar = fuzz.next();
            let k: scalar = fuzz.next();
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::distant_lit_diffuse(
                p,
                Color::from(light_color),
                surface_scale,
                k,
                input,
                crop,
            )
        }
        2 => {
            let p = next_point3(fuzz);
            let light_color: u32 = fuzz.next();
            let surface_scale: scalar = fuzz.next();
            let k: scalar = fuzz.next();
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::point_lit_diffuse(
                p,
                Color::from(light_color),
                surface_scale,
                k,
                input,
                crop,
            )
        }
        3 => {
            let p = next_point3(fuzz);
            let q = next_point3(fuzz);
            let specular_exponent: scalar = fuzz.next();
            let cutoff_angle: scalar = fuzz.next();
            let light_color: u32 = fuzz.next();
            let surface_scale: scalar = fuzz.next();
            let k: scalar = fuzz.next();
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::spot_lit_diffuse(
                p,
                q,
                specular_exponent,
                cutoff_angle,
                Color::from(light_color),
                surface_scale,
                k,
                input,
                crop,
            )
        }
        4 => {
            let p = next_point3(fuzz);
            let light_color: u32 = fuzz.next();
            let surface_scale: scalar = fuzz.next();
            let k: scalar = fuzz.next();
            let shininess: scalar = fuzz.next();
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::distant_lit_specular(
                p,
                Color::from(light_color),
                surface_scale,
                k,
                shininess,
                input,
                crop,
            )
        }
        5 => {
            let p = next_point3(fuzz);
            let light_color: u32 = fuzz.next();
            let surface_scale: scalar = fuzz.next();
            let k: scalar = fuzz.next();
            let shininess: scalar = fuzz.next();
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::point_lit_specular(
                p,
                Color::from(light_color),
                surface_scale,
                k,
                shininess,
                input,
                crop,
            )
        }
        _ => {
            let p = next_point3(fuzz);
            let q = next_point3(fuzz);
            let specular_exponent: scalar = fuzz.next();
            let cutoff_angle: scalar = fuzz.next();
            let light_color: u32 = fuzz.next();
            let surface_scale: scalar = fuzz.next();
            let k: scalar = fuzz.next();
            let shininess: scalar = fuzz.next();
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::spot_lit_specular(
                p,
                q,
                specular_exponent,
                cutoff_angle,
                Color::from(light_color),
                surface_scale,
                k,
                shininess,
                input,
                crop,
            )
        }
    }
}

/// The `int` radii of `SkImageFilters::Dilate` and `Erode`, as the `SkScalar` they convert to.
#[allow(clippy::cast_precision_loss)] // `int` to `SkScalar`, as C++ converts them
fn int_radius(rx: i32, ry: i32) -> (f32, f32) {
    (rx as f32, ry as f32)
}

/// `next_sampling`: a cubic resampler (`B`, `C`), or a filter and a mipmap mode.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L539-L551 (chrome/m156)
fn next_sampling(fuzz: &mut Fuzz<'_>) -> SamplingOptions {
    if fuzz.next_bool() {
        let b: f32 = fuzz.next();
        let c: f32 = fuzz.next();
        SamplingOptions::from(CubicResampler { b, c })
    } else {
        let fm = filter_mode(fuzz.next_enum(FilterMode::Linear as i32));
        let mm = mipmap_mode(fuzz.next_enum(MipmapMode::Linear as i32));
        SamplingOptions::new(fm, mm)
    }
}

/// `MakeFuzzImageFilter`: an image filter of at most `depth` levels, or none.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L553-L809 (chrome/m156)
#[doc(alias = "MakeFuzzImageFilter")]
#[allow(clippy::too_many_lines)] // one arm per C++ `case`, as in the port
pub fn make_fuzz_image_filter(fuzz: &mut Fuzz<'_>, depth: i32) -> Option<ImageFilter> {
    if depth <= 0 || fuzz.exhausted() {
        return None;
    }
    let image_filter_type: u8 = fuzz.next_range(0, 22);
    match image_filter_type {
        // `0` is no image filter, and the default is unreachable (the type is in `0..=22`).
        1 => {
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            let sigma_x: scalar = fuzz.next();
            let sigma_y: scalar = fuzz.next();
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            // `SkImageFilters::Blur(sigmaX, sigmaY, input, cropRect)` defaults to the decal mode.
            image_filters::blur(
                sigma_x,
                sigma_y,
                TileMode::Decal,
                input,
                crop.map(Rect::from),
            )
        }
        2 => {
            let matrix = nice_matrix(fuzz);
            let sampling = next_sampling(fuzz);
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::matrix_transform(&matrix, sampling, input)
        }
        3 => {
            let k1: f32 = fuzz.next();
            let k2: f32 = fuzz.next();
            let k3: f32 = fuzz.next();
            let k4: f32 = fuzz.next();
            let enforce_pm_color = fuzz.next_bool();
            let use_crop_rect = fuzz.next_bool();
            let background = make_fuzz_image_filter(fuzz, depth - 1);
            let foreground = make_fuzz_image_filter(fuzz, depth - 1);
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            image_filters::arithmetic(
                k1,
                k2,
                k3,
                k4,
                enforce_pm_color,
                background,
                foreground,
                crop.map(Rect::from),
            )
        }
        4 => {
            let cf = make_fuzz_colorfilter(fuzz, depth - 1);
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            image_filters::color_filter(cf, input, crop.map(Rect::from))
        }
        5 => {
            let ifo = make_fuzz_image_filter(fuzz, depth - 1);
            let ifi = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::compose(ifo, ifi)
        }
        6 => {
            let x_channel_selector = color_channel(fuzz.next_enum(ColorChannel::A as i32));
            let y_channel_selector = color_channel(fuzz.next_enum(ColorChannel::A as i32));
            let scale: scalar = fuzz.next();
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let displacement = make_fuzz_image_filter(fuzz, depth - 1);
            let color = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::displacement_map(
                (x_channel_selector, y_channel_selector),
                scale,
                displacement,
                color,
                crop.map(Rect::from),
            )
        }
        7 => {
            let dx: scalar = fuzz.next();
            let dy: scalar = fuzz.next();
            let sigma_x: scalar = fuzz.next();
            let sigma_y: scalar = fuzz.next();
            let color: u32 = fuzz.next();
            let shadow_only = fuzz.next_bool();
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            let color = Color4f::from_color(Color::from(color));
            if shadow_only {
                image_filters::drop_shadow_only(
                    (dx, dy),
                    (sigma_x, sigma_y),
                    color,
                    None,
                    input,
                    crop.map(Rect::from),
                )
            } else {
                image_filters::drop_shadow(
                    (dx, dy),
                    (sigma_x, sigma_y),
                    color,
                    None,
                    input,
                    crop.map(Rect::from),
                )
            }
        }
        8 => {
            let image = make_fuzz_image(fuzz);
            image_filters::image_sampled(
                Some(image),
                SamplingOptions::from(CubicResampler::mitchell()),
            )
        }
        9 => {
            let image = make_fuzz_image(fuzz);
            let src_rect = next_rect(fuzz);
            let dst_rect = next_rect(fuzz);
            let sampling = next_sampling(fuzz);
            Some(image_filters::image(
                Some(image),
                src_rect,
                dst_rect,
                sampling,
            ))
        }
        10 => make_fuzz_lighting_imagefilter(fuzz, depth - 1),
        11 => {
            let lens_bounds = next_rect(fuzz);
            let zoom_amount: scalar = fuzz.next();
            let inset: scalar = fuzz.next();
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            let sampling = next_sampling(fuzz);
            image_filters::magnifier(
                &lens_bounds,
                zoom_amount,
                inset,
                sampling,
                input,
                crop.map(Rect::from),
            )
        }
        12 => {
            const MAX_KERNEL_SIZE: i32 = 5;
            let n: i32 = fuzz.next_range(1, MAX_KERNEL_SIZE);
            let m: i32 = fuzz.next_range(1, MAX_KERNEL_SIZE);
            let mut kernel = [0.0f32; (MAX_KERNEL_SIZE * MAX_KERNEL_SIZE) as usize];
            let count = (n * m) as usize;
            for v in kernel.iter_mut().take(count) {
                *v = fuzz.next();
            }
            let offset_x: i32 = fuzz.next_range(0, n - 1);
            let offset_y: i32 = fuzz.next_range(0, m - 1);
            let gain: scalar = fuzz.next();
            let bias: scalar = fuzz.next();
            let convolve_alpha = fuzz.next_bool();
            let use_crop_rect = fuzz.next_bool();
            let tile_mode = tile_mode(fuzz.next_enum(TileMode::Decal as i32));
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::matrix_convolution(
                skia_rust_core::size::ISize::new(n, m),
                &kernel[..count],
                gain,
                bias,
                skia_rust_core::point::IPoint::new(offset_x, offset_y),
                tile_mode,
                convolve_alpha,
                input,
                crop.map(Rect::from),
            )
        }
        13 => {
            let first = make_fuzz_image_filter(fuzz, depth - 1);
            let second = make_fuzz_image_filter(fuzz, depth - 1);
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            image_filters::merge(&[first, second], crop.map(Rect::from))
        }
        14 => {
            const MAX_COUNT: i32 = 4;
            let count: i32 = fuzz.next_range(1, MAX_COUNT);
            let mut ifs = Vec::with_capacity(count as usize);
            for _ in 0..count {
                ifs.push(make_fuzz_image_filter(fuzz, depth - 1));
            }
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            image_filters::merge(&ifs, crop.map(Rect::from))
        }
        15 => {
            let rx: i32 = fuzz.next();
            let ry: i32 = fuzz.next();
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::dilate(int_radius(rx, ry), input, crop.map(Rect::from))
        }
        16 => {
            let rx: i32 = fuzz.next();
            let ry: i32 = fuzz.next();
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::erode(int_radius(rx, ry), input, crop.map(Rect::from))
        }
        17 => {
            let dx: scalar = fuzz.next();
            let dy: scalar = fuzz.next();
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::offset((dx, dy), input, crop.map(Rect::from))
        }
        18 => {
            let picture = make_fuzz_picture(fuzz, depth - 1);
            image_filters::picture(picture, None)
        }
        19 => {
            let crop_rect = next_rect(fuzz);
            let picture = make_fuzz_picture(fuzz, depth - 1);
            image_filters::picture(picture, Some(&crop_rect))
        }
        20 => {
            let src = next_rect(fuzz);
            let dst = next_rect(fuzz);
            let input = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::tile(&src, &dst, input)
        }
        21 => {
            let use_crop_rect = fuzz.next_bool();
            let blend_mode = blend_mode(fuzz.next_enum(BlendMode::Luminosity as i32));
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            let bg = make_fuzz_image_filter(fuzz, depth - 1);
            let fg = make_fuzz_image_filter(fuzz, depth - 1);
            image_filters::blend(blend_mode, bg, fg, crop.map(Rect::from))
        }
        22 => {
            let shader = make_fuzz_shader(fuzz, depth - 1);
            let use_crop_rect = fuzz.next_bool();
            let crop = use_crop_rect.then(|| next_irect(fuzz));
            image_filters::shader(shader, image_filters::Dither::No, crop.map(Rect::from))
        }
        // `default: SkASSERT(false)`: unreachable, the type is in `0..=22`.
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Images, pictures, paints, fonts and text.

/// `make_fuzz_image`: an N32 premultiplied raster image of fuzzed size and pixels.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L811-L826 (chrome/m156)
fn make_fuzz_image(fuzz: &mut Fuzz<'_>) -> Image {
    let w: i32 = fuzz.next_range(1, 1024);
    let h: i32 = fuzz.next_range(1, 1024);
    let n = (w * h) as usize;
    let mut data = Vec::with_capacity(n * 4);
    for _ in 0..n {
        let c: u32 = fuzz.next();
        // The pixels are native N32 premultiplied words, as the C++ `SkPMColor` array.
        data.extend_from_slice(&pre_multiply_color(Color::from(c)).to_ne_bytes());
    }
    // The C++ pixmap owns its pixels and releases them with `sk_free`; the image takes the pixels
    // and frees them when it drops, so the release proc is empty.
    images::raster_from_pixmap(
        &ImageInfo::new_n32_premul((w, h), None),
        data,
        w as usize * 4,
        None::<fn()>,
    )
    .expect("a raster image of a valid pixmap")
}

/// `FuzzPaint`: sets the paint's antialiasing, dither, colour, blend mode, style, shader, path
/// effect, mask filter, image filter, colour filter and (for strokes) stroke parameters.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L835-L858 (chrome/m156)
#[doc(alias = "FuzzPaint")]
pub fn fuzz_paint(fuzz: &mut Fuzz<'_>, paint: &mut Paint, depth: i32) {
    if depth <= 0 || fuzz.exhausted() {
        return;
    }
    paint.set_anti_alias(fuzz.next_bool());
    paint.set_dither(fuzz.next_bool());
    let color: u32 = fuzz.next();
    paint.set_color(Color::from(color));
    paint.set_blend_mode(blend_mode(fuzz.next_enum(BlendMode::Luminosity as i32)));
    let style: u8 = fuzz.next_enum(Style::StrokeAndFill as u8);
    paint.set_style(paint_style(style));
    paint.set_shader(make_fuzz_shader(fuzz, depth - 1));
    paint.set_path_effect(make_fuzz_patheffect(fuzz, depth - 1));
    paint.set_mask_filter(make_fuzz_maskfilter(fuzz));
    paint.set_image_filter(make_fuzz_image_filter(fuzz, depth - 1));
    paint.set_color_filter(make_fuzz_colorfilter(fuzz, depth - 1));

    if paint.style() != Style::Fill {
        let width: scalar = fuzz.next();
        paint.set_stroke_width(width);
        let miter: scalar = fuzz.next();
        paint.set_stroke_miter(miter);
        paint.set_stroke_cap(paint_cap(fuzz.next_enum(Cap::Square as i32)));
        let join: u8 = fuzz.next_enum(Join::Bevel as u8);
        paint.set_stroke_join(paint_join(join));
    }
}

/// `fuzz_font`: a font with a fuzzed typeface and flags.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L860-L875 (chrome/m156)
fn fuzz_font(fuzz: &mut Fuzz<'_>) -> Font {
    let mut font = Font::default();
    font.set_typeface(make_fuzz_typeface(fuzz));
    font.set_size(fuzz.next());
    font.set_scale_x(fuzz.next());
    font.set_skew_x(fuzz.next());
    font.set_linear_metrics(fuzz.next_bool());
    font.set_subpixel(fuzz.next_bool());
    font.set_embedded_bitmaps(fuzz.next_bool());
    font.set_force_auto_hinting(fuzz.next_bool());
    font.set_embolden(fuzz.next_bool());
    font.set_hinting(font_hinting(fuzz.next_enum(FontHinting::Full as i32)));
    font.set_edging(edging(fuzz.next_enum(Edging::SubpixelAntiAlias as i32)));
    font
}

/// `FuzzPaint_text_encoding`.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L877-L879 (chrome/m156)
fn fuzz_paint_text_encoding(fuzz: &mut Fuzz<'_>) -> TextEncoding {
    text_encoding(fuzz.next_enum(TextEncoding::UTF32 as i32))
}

/// `make_fuzz_text`: glyph ids, or characters from three Unicode ranges, in `encoding`.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L883-L956 (chrome/m156)
fn make_fuzz_text(fuzz: &mut Fuzz<'_>, font: &Font, encoding: TextEncoding) -> Vec<u8> {
    if encoding == TextEncoding::GlyphId {
        let glyph_range = font.typeface().count_glyphs();
        if glyph_range == 0 {
            // Some fuzzing environments have no fonts, so empty array is the best we can do.
            return Vec::new();
        }
        let glyph_count: i32 = fuzz.next_range(1, MAX_GLYPH_COUNT);
        let mut bytes = Vec::with_capacity(glyph_count as usize * 2);
        for _ in 0..glyph_count {
            let glyph: u16 = fuzz.next_range(0, (glyph_range - 1) as u16);
            bytes.extend_from_slice(&glyph.to_ne_bytes());
        }
        return bytes;
    }
    let count: i32 = RANGES.iter().map(|r| (r[1] - r[0]) as i32).sum();
    let mut buffer = [0u32; MAX_GLYPH_COUNT as usize];
    let length: i32 = fuzz.next_range(1, MAX_GLYPH_COUNT);
    for slot in buffer.iter_mut().take(length as usize) {
        let mut value: i32 = fuzz.next_range(0, count - 1);
        for r in RANGES {
            if (value as u32) + r[0] < r[1] {
                *slot = (value as u32) + r[0];
                break;
            }
            value -= (r[1] - r[0]) as i32;
        }
    }
    let chars = buffer[..length as usize]
        .iter()
        .map(|&c| char::from_u32(c).expect("the ranges hold valid scalar values"));
    match encoding {
        TextEncoding::UTF8 => {
            let mut out = Vec::new();
            for c in chars {
                let mut utf8 = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
            }
            out
        }
        TextEncoding::UTF16 => {
            let mut out = Vec::new();
            for c in chars {
                let mut utf16 = [0u16; 2];
                for unit in c.encode_utf16(&mut utf16) {
                    out.extend_from_slice(&unit.to_ne_bytes());
                }
            }
            out
        }
        TextEncoding::UTF32 => {
            let mut out = Vec::new();
            for c in chars {
                out.extend_from_slice(&u32::from(c).to_ne_bytes());
            }
            out
        }
        // `default: SkASSERT(false)`: the enum has no other encoding here.
        TextEncoding::GlyphId => Vec::new(),
    }
}

/// `make_fuzz_string`: up to `kMaxGlyphCount` arbitrary bytes.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L958-L966 (chrome/m156)
fn make_fuzz_string(fuzz: &mut Fuzz<'_>) -> Vec<u8> {
    let len: i32 = fuzz.next_range(0, MAX_GLYPH_COUNT);
    (0..len).map(|_| fuzz.next::<u8>()).collect()
}

/// `make_fuzz_textblob`: up to eight runs of fuzzed text, positioned in one of three ways.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L968-L1014 (chrome/m156)
fn make_fuzz_textblob(fuzz: &mut Fuzz<'_>) -> Option<TextBlob> {
    let mut builder = TextBlobBuilder::new();
    let run_count: i8 = fuzz.next_range(1, 8);
    for _ in 0..run_count {
        let mut font = default_font();
        let encoding = fuzz_paint_text_encoding(fuzz);
        let alias = fuzz.next_bool();
        font.set_edging(if alias {
            Edging::Alias
        } else {
            Edging::AntiAlias
        });
        let text = make_fuzz_text(fuzz, &font, encoding);
        let glyph_count = font.count_text(&text, encoding);
        let run_type: u8 = fuzz.next_range(0, 2);
        match run_type {
            0 => {
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                // TODO: Test other variations of this.
                let glyphs = builder.alloc_run(&font, glyph_count, x, y, None);
                let _ = font.text_to_glyphs(&text, encoding, glyphs);
            }
            1 => {
                let y: scalar = fuzz.next();
                // TODO: Test other variations of this.
                let (glyphs, pos) = builder.alloc_run_pos_h(&font, glyph_count, y, None);
                let _ = font.text_to_glyphs(&text, encoding, glyphs);
                for p in pos.iter_mut() {
                    *p = fuzz.next();
                }
            }
            _ => {
                // TODO: Test other variations of this.
                let (glyphs, pos) = builder.alloc_run_pos(&font, glyph_count, None);
                let _ = font.text_to_glyphs(&text, encoding, glyphs);
                for p in pos.iter_mut() {
                    let x: f32 = fuzz.next();
                    let y: f32 = fuzz.next();
                    *p = Point::new(x, y);
                }
            }
        }
    }
    builder.make()
}

/// `make_fuzz_picture`: a picture recorded by `fuzz_canvas` at `depth - 1`.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L1520-L1526 (chrome/m156)
fn make_fuzz_picture(fuzz: &mut Fuzz<'_>, depth: i32) -> Option<Picture> {
    let w: scalar = fuzz.next();
    let h: scalar = fuzz.next();
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::new(0.0, 0.0, w, h), false);
    fuzz_canvas(fuzz, canvas, depth - 1);
    recorder.finish_recording_as_picture(None)
}

/// `FuzzCanvas`: runs up to 2000 random drawing commands on `canvas`, restoring the canvas's
/// save count afterwards.
// Port of: fuzz/FuzzCanvasHelpers.cpp#L1016-L1518 (chrome/m156)
#[doc(alias = "FuzzCanvas")]
#[allow(clippy::too_many_lines)] // one arm per C++ `case`, as in the port
pub fn fuzz_canvas(fuzz: &mut Fuzz<'_>, canvas: &Canvas, depth: i32) {
    if depth <= 0 || fuzz.exhausted() {
        return;
    }
    // `SkAutoCanvasRestore autoCanvasRestore(canvas, false)`.
    let _restore = skia_rust_core::canvas::AutoCanvasRestore::guard(canvas, false);
    let n: u32 = fuzz.next_range(0, 2000);
    for _ in 0..n {
        if fuzz.exhausted() {
            return;
        }
        let mut paint = Paint::default();
        let draw_command: u32 = fuzz.next_range(0, 62);
        match draw_command {
            1 => {
                canvas.save();
            }
            2 => {
                let bounds = next_rect(fuzz);
                fuzz_paint(fuzz, &mut paint, depth - 1);
                canvas.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&paint));
            }
            3 => {
                let bounds = next_rect(fuzz);
                canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
            }
            4 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                canvas.save_layer(&SaveLayerRec::default().paint(&paint));
            }
            5 => {
                canvas.save_layer(&SaveLayerRec::default());
            }
            6 => {
                let alpha: u8 = fuzz.next();
                canvas.save_layer_alpha(None, alpha);
            }
            7 => {
                let bounds = next_rect(fuzz);
                let alpha: u8 = fuzz.next();
                canvas.save_layer_alpha(Some(bounds), alpha);
            }
            8 => {
                let bounds = fuzz.next_bool().then(|| next_rect(fuzz));
                let has_paint = fuzz.next_bool();
                if has_paint {
                    fuzz_paint(fuzz, &mut paint, depth - 1);
                }
                let backdrop = if fuzz.next_bool() {
                    make_fuzz_image_filter(fuzz, depth - 1)
                } else {
                    None
                };
                // _DumpCanvas can't handle this.
                // if (make_fuzz_t<bool>(fuzz)) {
                //     saveLayerRec.fSaveLayerFlags |= SkCanvas::kPreserveLCDText_SaveLayerFlag;
                // }
                let mut rec = SaveLayerRec::default();
                if let Some(bounds) = bounds.as_ref() {
                    rec = rec.bounds(bounds);
                }
                if has_paint {
                    rec = rec.paint(&paint);
                }
                if let Some(backdrop) = backdrop.as_ref() {
                    rec = rec.backdrop(backdrop);
                }
                canvas.save_layer(&rec);
            }
            9 => {
                canvas.restore();
            }
            10 => {
                let save_count: i32 = fuzz.next();
                canvas.restore_to_count(usize::try_from(save_count).unwrap_or(0));
            }
            11 => {
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                canvas.translate((x, y));
            }
            12 => {
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                canvas.scale((x, y));
            }
            13 => {
                let v: scalar = fuzz.next();
                canvas.rotate(v, None);
            }
            14 => {
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                let v: scalar = fuzz.next();
                canvas.rotate(v, Some(Point::new(x, y)));
            }
            15 => {
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                canvas.skew((x, y));
            }
            16 => {
                let mat = nice_matrix(fuzz);
                canvas.concat(&mat);
            }
            17 => {
                let mat = nice_matrix(fuzz);
                canvas.set_matrix(&M44::from(&mat));
            }
            18 => {
                canvas.reset_matrix();
            }
            19 => {
                let r = next_rect(fuzz);
                let do_anti_alias = fuzz.next_bool();
                let op: i32 = fuzz.next_range(0, 1);
                let r = r.sorted();
                canvas.clip_rect(r, clip_op(op), do_anti_alias);
            }
            20 => {
                let rr = nice_rrect(fuzz);
                let do_anti_alias = fuzz.next_bool();
                let op: i32 = fuzz.next_range(0, 1);
                canvas.clip_rrect(rr, clip_op(op), do_anti_alias);
            }
            21 => {
                let path = fuzz_nice_path(fuzz, 30);
                let do_anti_alias = fuzz.next_bool();
                let op: i32 = fuzz.next_range(0, 1);
                canvas.clip_path(&path, clip_op(op), do_anti_alias);
            }
            22 => {
                let mut region = Region::new();
                fuzz.next_region(&mut region);
                let op: i32 = fuzz.next_range(0, 1);
                canvas.clip_region(&region, clip_op(op));
            }
            23 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                canvas.draw_paint(&paint);
            }
            24 => {
                const MAX_COUNT: u64 = 30;
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let point_mode: i32 = fuzz.next_range(0, 2);
                let point_mode = match point_mode {
                    0 => PointMode::Points,
                    1 => PointMode::Lines,
                    _ => PointMode::Polygon,
                };
                let count: u64 = fuzz.next_range(0, MAX_COUNT);
                let count = count as usize;
                let mut pts = [Point::new(0.0, 0.0); MAX_COUNT as usize];
                for p in pts.iter_mut().take(count) {
                    *p = next_point(fuzz);
                }
                canvas.draw_points(point_mode, &pts[..count], &paint);
            }
            25 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let r = next_rect(fuzz);
                if !r.is_finite() {
                    continue;
                }
                canvas.draw_rect(r, &paint);
            }
            26 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let mut region = Region::new();
                fuzz.next_region(&mut region);
                canvas.draw_region(&region, &paint);
            }
            27 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let r = next_rect(fuzz);
                if !r.is_finite() {
                    continue;
                }
                canvas.draw_oval(r, &paint);
            }
            29 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let rr = nice_rrect(fuzz);
                canvas.draw_rrect(rr, &paint);
            }
            30 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let orr = nice_rrect(fuzz);
                let irr = nice_rrect(fuzz);
                if orr.bounds().contains(irr.bounds()) {
                    canvas.draw_drrect(orr, irr, &paint);
                }
            }
            // Cases 31 and 59 read and draw the same (`drawArc`), with different names.
            31 | 59 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let r = next_rect(fuzz);
                let start: scalar = fuzz.next();
                let sweep: scalar = fuzz.next();
                let use_center = fuzz.next_bool();
                canvas.draw_arc(r, start, sweep, use_center, &paint);
            }
            32 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let path = fuzz_nice_path(fuzz, 60);
                canvas.draw_path(&path, &paint);
            }
            33 => {
                let img = make_fuzz_image(fuzz);
                let left: scalar = fuzz.next();
                let top: scalar = fuzz.next();
                let use_paint = fuzz.next_bool();
                if use_paint {
                    fuzz_paint(fuzz, &mut paint, depth - 1);
                }
                canvas.draw_image(&img, (left, top), use_paint.then_some(&paint));
            }
            35 => {
                let img = make_fuzz_image(fuzz);
                let src = next_irect(fuzz);
                let dst = next_rect(fuzz);
                let use_paint = fuzz.next_bool();
                if use_paint {
                    fuzz_paint(fuzz, &mut paint, depth - 1);
                }
                let constraint = if fuzz.next_bool() {
                    SrcRectConstraint::Strict
                } else {
                    SrcRectConstraint::Fast
                };
                // A null paint is the default paint, as in `SkCanvas`.
                canvas.draw_image_rect(&img, Some((&Rect::from(src), constraint)), dst, &paint);
            }
            37 => {
                let img = make_fuzz_image(fuzz);
                let use_paint = fuzz.next_bool();
                if use_paint {
                    fuzz_paint(fuzz, &mut paint, depth - 1);
                }
                let center = if fuzz.next_bool() {
                    next_irect(fuzz)
                } else {
                    // Make valid center, see SkLatticeIter::Valid().
                    let left: i32 = fuzz.next_range(0, img.width() - 1);
                    let top: i32 = fuzz.next_range(0, img.height() - 1);
                    let right: i32 = fuzz.next_range(left + 1, img.width());
                    let bottom: i32 = fuzz.next_range(top + 1, img.height());
                    IRect::new(left, top, right, bottom)
                };
                let dst = next_rect(fuzz);
                canvas.draw_image_nine(
                    &img,
                    center,
                    dst,
                    FilterMode::Nearest,
                    use_paint.then_some(&paint),
                );
            }
            44 => {
                const MAX: usize = 6;
                let img = make_fuzz_image(fuzz);
                let use_paint = fuzz.next_bool();
                let dst = next_rect(fuzz);
                if use_paint {
                    fuzz_paint(fuzz, &mut paint, depth - 1);
                }
                let x_count: i32 = fuzz.next_range(2, MAX as i32);
                let y_count: i32 = fuzz.next_range(2, MAX as i32);
                let mut x_divs = [0i32; MAX];
                let mut y_divs = [0i32; MAX];
                for v in x_divs.iter_mut().take(x_count as usize) {
                    *v = fuzz.next();
                }
                for v in y_divs.iter_mut().take(y_count as usize) {
                    *v = fuzz.next();
                }
                let lattice = Lattice {
                    x_divs: &x_divs[..x_count as usize],
                    y_divs: &y_divs[..y_count as usize],
                    rect_types: None,
                    bounds: None,
                    colors: None,
                };
                canvas.draw_image_lattice(
                    &img,
                    &lattice,
                    dst,
                    FilterMode::Linear,
                    use_paint.then_some(&paint),
                );
            }
            45 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let font = fuzz_font(fuzz);
                let encoding = fuzz_paint_text_encoding(fuzz);
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                let text = make_fuzz_text(fuzz, &font, encoding);
                canvas.draw_simple_text(&text, encoding, (x, y), &font, &paint);
            }
            51 => {
                let blob = make_fuzz_textblob(fuzz);
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                if let Some(blob) = blob {
                    canvas.draw_text_blob(&blob, (x, y), &paint);
                }
            }
            52 => {
                let use_paint = fuzz.next_bool();
                let use_matrix = fuzz.next_bool();
                if use_paint {
                    fuzz_paint(fuzz, &mut paint, depth - 1);
                }
                let matrix = use_matrix.then(|| nice_matrix(fuzz));
                let pic = make_fuzz_picture(fuzz, depth - 1);
                if let Some(pic) = pic {
                    canvas.draw_picture(&pic, matrix.as_ref(), use_paint.then_some(&paint));
                }
            }
            53 => {
                const MAX_COUNT: usize = 100;
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let vertex_mode: i32 = fuzz.next_range(0, VertexMode::TriangleFan as i32);
                let vertex_mode = match vertex_mode {
                    0 => VertexMode::Triangles,
                    1 => VertexMode::TriangleStrip,
                    _ => VertexMode::TriangleFan,
                };
                let blend: i32 = fuzz.next_range(0, BlendMode::Luminosity as i32);
                let vertex_count: i32 = fuzz.next_range(3, MAX_COUNT as i32);
                let vertex_count = vertex_count as usize;
                let mut vertices = [Point::new(0.0, 0.0); MAX_COUNT];
                for v in vertices.iter_mut().take(vertex_count) {
                    *v = next_point(fuzz);
                }
                let use_texs = fuzz.next_bool();
                let use_colors = fuzz.next_bool();
                let mut texs = [Point::new(0.0, 0.0); MAX_COUNT];
                if use_texs {
                    for t in texs.iter_mut().take(vertex_count) {
                        *t = next_point(fuzz);
                    }
                }
                let mut colors = [0u32; MAX_COUNT];
                if use_colors {
                    for c in colors.iter_mut().take(vertex_count) {
                        *c = fuzz.next();
                    }
                }
                let mut index_count = 0usize;
                let mut indices = [0u16; MAX_COUNT * 2];
                if fuzz.next_bool() {
                    let count: i32 = fuzz
                        .next_range(vertex_count as i32, vertex_count as i32 + MAX_COUNT as i32);
                    index_count = count as usize;
                    for index in indices.iter_mut().take(index_count) {
                        *index = fuzz.next_range(0, (vertex_count - 1) as u16);
                    }
                }
                let color_values: Vec<Color> = colors[..vertex_count]
                    .iter()
                    .map(|&c| Color::from(c))
                    .collect();
                let vertices_obj = Vertices::new_copy(
                    vertex_mode,
                    &vertices[..vertex_count],
                    use_texs.then_some(&texs[..vertex_count]),
                    use_colors.then_some(&color_values[..]),
                    (index_count > 0).then_some(&indices[..index_count]),
                );
                if let Some(vertices_obj) = vertices_obj {
                    canvas.draw_vertices(&vertices_obj, blend_mode(blend), &paint);
                }
            }
            54 => {
                let blend: i32 = fuzz.next_range(0, BlendMode::SrcOver as i32);
                let color: u32 = fuzz.next();
                canvas.draw_color(Color::from(color), blend_mode(blend));
            }
            55 => {
                let blend: i32 = fuzz.next_range(0, BlendMode::SrcOver as i32);
                let r: f32 = fuzz.next_range(-1.0, 2.0);
                let g: f32 = fuzz.next_range(-1.0, 2.0);
                let b: f32 = fuzz.next_range(-1.0, 2.0);
                let alpha: f32 = fuzz.next_range(0.0, 1.0);
                let color = Color4f::new(r, g, b, alpha);
                canvas.draw_color(color, blend_mode(blend));
            }
            56 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let p0 = next_point(fuzz);
                let p1 = next_point(fuzz);
                canvas.draw_line(p0, p1, &paint);
            }
            57 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let r = next_irect(fuzz);
                canvas.draw_irect(r, &paint);
            }
            58 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let radius: scalar = fuzz.next();
                let center = next_point(fuzz);
                canvas.draw_circle(center, radius, &paint);
            }
            60 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let rect = next_rect(fuzz);
                let rx: scalar = fuzz.next();
                let ry: scalar = fuzz.next();
                canvas.draw_round_rect(rect, rx, ry, &paint);
            }
            61 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let font = fuzz_font(fuzz);
                // `drawString(str.c_str(), …)`: the text ends at its first NUL byte.
                let mut text = make_fuzz_string(fuzz);
                if let Some(nul) = text.iter().position(|&b| b == 0) {
                    text.truncate(nul);
                }
                let x: scalar = fuzz.next();
                let y: scalar = fuzz.next();
                canvas.draw_simple_text(&text, TextEncoding::UTF8, (x, y), &font, &paint);
            }
            62 => {
                fuzz_paint(fuzz, &mut paint, depth - 1);
                let cubics: [Point; 12] = std::array::from_fn(|_| next_point(fuzz));
                let colors: [u32; 4] = std::array::from_fn(|_| fuzz.next());
                let use_tex_coords = fuzz.next_bool();
                let tex_coords: Option<[Point; 4]> =
                    use_tex_coords.then(|| std::array::from_fn(|_| next_point(fuzz)));
                let mode = blend_mode(fuzz.next_enum(BlendMode::Luminosity as i32));
                let colors = colors.map(Color::from);
                canvas.draw_patch(&cubics, &colors, tex_coords.as_ref(), mode, &paint);
            }
            // Cases 0 (the Ganesh flush), 28 ("must have deleted this some time earlier"), 46 to
            // 50 ("was drawPosText" and the other removed draws) and the values Skia has no case
            // for (34, 36, 38 to 43) read nothing. The `default` only asserts (`SkASSERT`, compiled
            // out in release builds).
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Things the fuzzers build a canvas or a font manager from.

/// A canvas on `bitmap`, as `SkCanvas(const SkBitmap&)` makes it (a raster device on the bitmap).
#[must_use]
pub fn canvas_on_bitmap(bitmap: Bitmap) -> Canvas {
    Canvas::from_device(Box::new(BitmapDevice::new(bitmap)))
}

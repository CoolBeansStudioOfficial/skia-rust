// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Paints with shaders, color filters and blenders, recorded in a picture, serialized and read back
//! with the effects registry, draw the same pixels as the paints drawn directly. This is our own
//! test, not a port: it covers the `SkPaintPriv::Flatten` arms of the shader, color filter and
//! blender flattenables, and their factories in `skia_rust_effects::flattenable::REGISTRY`.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::Color;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_effects::flattenable::REGISTRY;
use skia_rust_raster::surfaces;

const SIZE: i32 = 32;
const SIZE_F: f32 = 32.0;

/// The pixels of `picture` drawn on a transparent raster surface.
fn render(picture: &Picture) -> Vec<u8> {
    let info = ImageInfo::new_n32_premul((SIZE, SIZE), None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");
    picture.playback(surface.canvas());
    let image = surface.image_snapshot().expect("a snapshot");
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; row_bytes * usize::try_from(SIZE).unwrap_or(0)];
    assert!(image.read_pixels(&info, &mut pixels, row_bytes, (0, 0)));
    pixels
}

/// A picture that draws a rect and an oval with `paint`, over a second, paint-free rect.
fn picture_with(paint: &Paint) -> Picture {
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::from_wh(SIZE_F, SIZE_F), false);
    let mut plain = Paint::default();
    plain.set_color(Color::from_argb(0xFF, 0x20, 0x40, 0x80));
    canvas.draw_rect(Rect::new(0.0, 0.0, SIZE_F / 2.0, SIZE_F), &plain);
    canvas.draw_rect(Rect::new(SIZE_F / 2.0, 0.0, SIZE_F, SIZE_F), paint);
    canvas.draw_oval(Rect::new(4.0, 4.0, 28.0, 28.0), paint);
    recorder
        .finish_recording_as_picture(None)
        .expect("a picture")
}

/// Records `paint`, serializes the picture, reads it back with the effects registry, and checks
/// that the round trip draws what the recorded picture draws.
fn assert_round_trip(paint: &Paint) {
    let picture = picture_with(paint);
    let data = picture
        .serialize(None)
        .expect("the paint's flattenables are all ported");
    let read_back = Picture::from_data_with_registry(data.as_bytes(), None, &REGISTRY)
        .expect("the serialized picture reads back");
    let direct = render(&picture);
    // The comparison must not be between two empty drawings: the plain rect always draws.
    assert!(direct.iter().any(|&byte| byte != 0));
    assert_eq!(direct, render(&read_back));
}

#[test]
fn color_space_xform_filter_round_trips() {
    let mut paint = Paint::default();
    paint.set_shader(shaders::color(Color::from_argb(0xFF, 0x60, 0x90, 0xC0)));
    paint.set_color_filter(color_filters::linear_to_srgb_gamma());
    assert_round_trip(&paint);
}

/// The sRGB blend color filter of `color` with `mode`.
fn blend_filter(color: Color, mode: BlendMode) -> ColorFilter {
    color_filters::blend_color(color, mode).expect("a filter for this color and mode")
}

#[test]
fn color_shader_paint_round_trips() {
    let mut paint = Paint::default();
    paint.set_shader(shaders::color(Color::from_argb(0x80, 0x30, 0xA0, 0x40)));
    assert_round_trip(&paint);
}

#[test]
fn empty_shader_paint_round_trips() {
    let mut paint = Paint::default();
    paint.set_shader(shaders::empty());
    assert_round_trip(&paint);
}

#[test]
fn blend_shader_with_local_matrix_round_trips() {
    let dst = shaders::color(Color::from_argb(0xFF, 0xFF, 0x00, 0x00));
    let src = shaders::color(Color::from_argb(0x80, 0x00, 0x00, 0xFF));
    let blended: Shader = shaders::blend(BlendMode::Multiply, dst, src);
    let mut paint = Paint::default();
    paint.set_shader(blended.with_local_matrix(&Matrix::translate((3.0, 2.0))));
    assert_round_trip(&paint);
}

#[test]
fn color_filter_shader_round_trips() {
    let shader = shaders::color(Color::from_argb(0xFF, 0x10, 0x20, 0x30));
    let filtered = shader.with_color_filter(blend_filter(
        Color::from_argb(0x7F, 0xFF, 0x80, 0x00),
        BlendMode::SrcATop,
    ));
    let mut paint = Paint::default();
    paint.set_shader(filtered);
    assert_round_trip(&paint);
}

#[test]
fn blend_color_filter_round_trips() {
    let mut paint = Paint::default();
    paint.set_color(Color::from_argb(0xFF, 0x00, 0x80, 0xFF));
    paint.set_color_filter(blend_filter(
        Color::from_argb(0x7F, 0x40, 0x10, 0x90),
        BlendMode::Multiply,
    ));
    assert_round_trip(&paint);
}

#[test]
fn matrix_color_filter_round_trips() {
    let matrix = [
        0.5, 0.0, 0.0, 0.0, 0.1, //
        0.0, 0.5, 0.0, 0.0, 0.2, //
        0.0, 0.0, 0.5, 0.0, 0.3, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    let mut paint = Paint::default();
    paint.set_shader(shaders::color(Color::from_argb(0xFF, 0x80, 0x40, 0xC0)));
    paint.set_color_filter(color_filters::matrix_row_major(&matrix, Clamp::Yes));
    assert_round_trip(&paint);
}

#[test]
fn composed_color_filter_round_trips() {
    let outer = blend_filter(Color::from_argb(0x7F, 0x00, 0xFF, 0x00), BlendMode::SrcOver);
    let inner = blend_filter(Color::from_argb(0x7F, 0xFF, 0x00, 0x00), BlendMode::Screen);
    let mut paint = Paint::default();
    paint.set_shader(shaders::color(Color::from_argb(0xFF, 0x33, 0x66, 0x99)));
    paint.set_color_filter(color_filters::compose(Some(&outer), Some(inner)));
    assert_round_trip(&paint);
}

#[test]
fn table_color_filter_round_trips() {
    let mut table = [0u8; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        *entry = u8::try_from(255 - i).expect("i < 256");
    }
    let mut paint = Paint::default();
    paint.set_shader(shaders::color(Color::from_argb(0xFF, 0x12, 0x34, 0x56)));
    paint.set_color_filter(Some(color_filters::table(&table)));
    assert_round_trip(&paint);
}

#[test]
fn blender_with_an_effect_round_trips() {
    // A blend-mode blender is written with the effects, because the paint has a shader.
    let mut paint = Paint::default();
    paint.set_shader(shaders::color(Color::from_argb(0xFF, 0x90, 0x90, 0x10)));
    paint.set_blender(Blender::mode(BlendMode::Xor));
    assert_round_trip(&paint);
}

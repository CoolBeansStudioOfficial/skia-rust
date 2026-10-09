// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/KeyHelpers.{h,cpp} (part I)

//! Key strings and gathered uniform bytes of the `KeyHelpers` blocks.
//!
//! The expected strings are the `PaintParamsKey::toString` output that `KeyHelpers.cpp` produces
//! for these blocks: snippet names separated by spaces, children in brackets, `inner+outer` for a
//! compose, and `(0)` for a snippet that stores sampler data with no immutable sampler.
//!
//! The expected bytes follow the std140 layout the blocks' writes imply, with every value
//! little-endian. Two rules from `UniformManager` apply: a vec4 starts on a 16-byte boundary, and
//! the finished block is rounded up to the largest alignment that was written. For std140 a half
//! is written at full precision (`UseFullPrecision`), so it takes four bytes.

mod support;

// The shader the `GradientData` is made for (only its identity matters to the storage path).
fn a_gradient_shader() -> LinearGradient {
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
    ];
    LinearGradient::new(
        &[Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
    )
}

use std::cell::RefCell;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::color_shader::ColorShader;
use skia_rust_core::shaders::local_matrix_shader::LocalMatrixShader;
use skia_rust_core::shaders::shader_base::GradientType;
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation};
use skia_rust_effects::linear_gradient::LinearGradient;
use skia_rust_gpu::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::key_context::KeyContext;
use skia_rust_gpu::graphite::key_helpers::{
    CoordNormalizeData, CoordNormalizeShaderBlock, DitherData, DitherShaderBlock, GradientData,
    GradientShaderBlocks, ImageData, ImageShaderBlock, LMShaderData, LocalMatrixShaderBlock,
    SolidColorShaderBlock, add_to_key_shader,
};
use skia_rust_gpu::graphite::paint_params_key::{PaintParamsKeyBuilder, RootBlockType};
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::resource_types::{ImmutableSamplerInfo, Layout};
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use support::MockCaps;

fn srgb_info() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    )
}

/// The little-endian bytes of `values`, as the uniform buffer holds them.
fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// The little-endian bytes of `values` as 32-bit ints.
fn i32_bytes(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// The solid color the tests use as the color of a block.
const SOLID: [f32; 4] = [0.25, 0.5, 0.75, 1.0];

/// Rounds a finished uniform block up to 16 bytes, as `UniformManager::finish` does when a vec4
/// has been written.
fn pad_to_16(mut bytes: Vec<u8>) -> Vec<u8> {
    while !bytes.len().is_multiple_of(16) {
        bytes.push(0);
    }
    bytes
}

struct Fixture {
    caps: Arc<dyn Caps>,
    builder: RefCell<PaintParamsKeyBuilder>,
    gatherer: RefCell<PipelineDataGatherer>,
    dict: ShaderCodeDictionary,
}

impl Fixture {
    fn new() -> Self {
        let dict = ShaderCodeDictionary::new(Layout::Std140, &[]);
        let builder = RefCell::new(PaintParamsKeyBuilder::new(&dict));
        builder
            .borrow_mut()
            .add_root_block_header(RootBlockType::SrcColor);
        Self {
            caps: Arc::new(MockCaps::default()),
            builder,
            gatherer: RefCell::new(PipelineDataGatherer::new(Layout::Std140)),
            dict,
        }
    }

    fn context<'a>(&'a self, info: &ColorInfo) -> KeyContext<'a> {
        KeyContext::new(
            self.caps.clone(),
            &self.builder,
            &self.gatherer,
            &self.dict,
            Arc::new(RuntimeEffectDictionary::new()),
            info,
        )
    }

    /// The key built so far, as `PaintParamsKey::toString` renders it.
    fn key_string(&self) -> String {
        let mut builder = self.builder.borrow_mut();
        let lock = builder.lock_as_key();
        lock.key().to_string(&*self.caps, &self.dict)
    }

    /// The uniform bytes gathered so far for a shading draw.
    fn uniform_bytes(&self) -> Vec<u8> {
        let (uniforms, _textures) = self.gatherer.borrow_mut().end_combined_data(true);
        uniforms.data().to_vec()
    }
}

/// Adds the solid color block that the tests put inside or after another block.
fn add_solid(context: &KeyContext<'_>) {
    SolidColorShaderBlock::add_block(
        context,
        &PMColor4f {
            r: SOLID[0],
            g: SOLID[1],
            b: SOLID[2],
            a: SOLID[3],
        },
    );
}

#[test]
fn solid_color_writes_the_premultiplied_color() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    add_solid(&context);

    // A single vec4 at offset 0.
    assert_eq!(fixture.key_string(), "SolidColor ");
    assert_eq!(fixture.uniform_bytes(), f32_bytes(&SOLID));
}

#[test]
fn linear_gradient_with_two_stops_pads_to_four() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let colors = [
        PMColor4f {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        PMColor4f {
            r: 0.0,
            g: 0.0,
            b: 0.5,
            a: 0.25,
        },
    ];
    let shader = a_gradient_shader();
    let grad = GradientData::new(
        GradientType::Linear,
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        0.0,
        0.0,
        0.0,
        0.0,
        TileMode::Clamp,
        2,
        &colors,
        None,
        shader.base(),
        None,
        false,
        Interpolation::default(),
    );
    GradientShaderBlocks::add_block(&context, &grad);

    assert_eq!(fixture.key_string(), "LinearGradient4 ");
    // The colors are padded to four stops with the last one, then the offsets: 0, 1, and the
    // padding repeats the last offset. Then tilemode, colorspace and doUnPremul as ints.
    let mut expected = f32_bytes(&[
        1.0, 0.0, 0.0, 1.0, //
        0.0, 0.0, 0.5, 0.25, //
        0.0, 0.0, 0.5, 0.25, //
        0.0, 0.0, 0.5, 0.25,
    ]);
    expected.extend(f32_bytes(&[0.0, 1.0, 1.0, 1.0]));
    expected.extend(i32_bytes(&[0, 0, 0]));
    assert_eq!(fixture.uniform_bytes(), pad_to_16(expected));
}

#[test]
fn linear_gradient_with_five_stops_uses_eight_slots() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let colors: Vec<PMColor4f> = [0.0_f32, 0.25, 0.5, 0.75, 1.0]
        .iter()
        .map(|&r| PMColor4f {
            r,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
        .collect();
    let offsets = [0.0, 0.25, 0.5, 0.75, 1.0];
    let shader = a_gradient_shader();
    let grad = GradientData::new(
        GradientType::Linear,
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        0.0,
        0.0,
        0.0,
        0.0,
        TileMode::Repeat,
        5,
        &colors,
        Some(&offsets),
        shader.base(),
        None,
        false,
        Interpolation::default(),
    );
    GradientShaderBlocks::add_block(&context, &grad);

    assert_eq!(fixture.key_string(), "LinearGradient8 ");
    let mut expected = Vec::new();
    // Stops 5..8 repeat the last color.
    for i in 0..8 {
        let c = &colors[i.min(4)];
        expected.extend(f32_bytes(&[c.r, c.g, c.b, c.a]));
    }
    // The offsets are two vec4s; the padding repeats the last offset.
    expected.extend(f32_bytes(&[0.0, 0.25, 0.5, 0.75]));
    expected.extend(f32_bytes(&[1.0, 1.0, 1.0, 1.0]));
    // Tilemode is repeat (1), colorspace destination (0), and not premultiplied (0).
    expected.extend(i32_bytes(&[TileMode::Repeat as i32, 0, 0]));
    assert_eq!(fixture.uniform_bytes(), pad_to_16(expected));
}

#[test]
fn local_matrix_block_writes_the_matrix_and_its_inner_color() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    // The block takes the inverse of the matrix, which for a translation negates it.
    let inverse = Matrix::translate(Point::new(-3.0, -4.0));
    LocalMatrixShaderBlock::begin_block(&context, &LMShaderData::new(inverse));
    add_solid(&context);
    fixture.builder.borrow_mut().end_block();

    assert_eq!(fixture.key_string(), "LocalMatrix[SolidColor] ");
    // upper 2x2 (vec4), then the translation (vec2) at 16, then the next vec4 aligns to 32.
    let mut expected = f32_bytes(&[1.0, 0.0, 0.0, 1.0, -3.0, -4.0]);
    expected.extend([0u8; 8]);
    expected.extend(f32_bytes(&SOLID));
    assert_eq!(fixture.uniform_bytes(), expected);
}

#[test]
fn local_matrix_shader_inverts_its_matrix_into_the_key() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let inner = Shader::from_base(ColorShader::new(Color4f {
        r: SOLID[0],
        g: SOLID[1],
        b: SOLID[2],
        a: SOLID[3],
    }));
    let shader = Shader::from_base(LocalMatrixShader::new(
        inner,
        Matrix::translate(Point::new(3.0, 4.0)),
    ));
    add_to_key_shader(&context, Some(&shader));

    assert_eq!(fixture.key_string(), "LocalMatrix[SolidColor] ");
    let mut expected = f32_bytes(&[1.0, 0.0, 0.0, 1.0, -3.0, -4.0]);
    expected.extend([0u8; 8]);
    expected.extend(f32_bytes(&SOLID));
    assert_eq!(fixture.uniform_bytes(), expected);
}

#[test]
fn clamped_image_insets_its_subset_for_linear_filtering() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let image = ImageData::new(
        SamplingOptions {
            filter: FilterMode::Linear,
            ..SamplingOptions::default()
        },
        TileMode::Clamp,
        TileMode::Clamp,
        ISize::new(4, 4),
        Rect::new(0.0, 0.0, 2.0, 2.0),
        ImmutableSamplerInfo::default(),
    );
    // A compose puts the image inside, with a solid color after it so the key is well formed.
    fixture
        .builder
        .borrow_mut()
        .begin_block(BuiltInCodeSnippetID::Compose);
    ImageShaderBlock::add_block(&context, &image);
    add_solid(&context);
    fixture.builder.borrow_mut().end_block();

    // The subset does not cover the image, so the clamp variant is used. Its subset is inset by
    // kLinearInset = 0.5f + 0.00001f in float arithmetic, and it is not rounded out.
    assert_eq!(fixture.key_string(), "ImageShaderClamp(0)+SolidColor ");
    let inset = 0.5_f32 + 0.000_01_f32;
    let mut expected = f32_bytes(&[0.25, 0.25]);
    expected.extend([0u8; 8]);
    expected.extend(f32_bytes(&[inset, inset, 2.0 - inset, 2.0 - inset]));
    expected.extend(f32_bytes(&SOLID));
    assert_eq!(fixture.uniform_bytes(), expected);
}

#[test]
fn full_image_tiles_in_hardware_with_coord_normalization() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let image = ImageData::new(
        SamplingOptions::default(),
        TileMode::Clamp,
        TileMode::Clamp,
        ISize::new(4, 4),
        Rect::new(0.0, 0.0, 4.0, 4.0),
        ImmutableSamplerInfo::default(),
    );
    fixture
        .builder
        .borrow_mut()
        .begin_block(BuiltInCodeSnippetID::Compose);
    ImageShaderBlock::add_block(&context, &image);
    add_solid(&context);
    fixture.builder.borrow_mut().end_block();

    // The coordinates are normalized by 1/4 in the outer block, and the hardware image has no
    // uniforms of its own, so only the normalization and the solid color are written.
    assert_eq!(
        fixture.key_string(),
        "CoordNormalize[HardwareImage(0)]+SolidColor "
    );
    let mut expected = f32_bytes(&[0.25, 0.25]);
    expected.extend([0u8; 8]);
    expected.extend(f32_bytes(&SOLID));
    assert_eq!(fixture.uniform_bytes(), expected);
}

#[test]
fn coord_normalize_block_stores_the_inverse_dimensions() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let data = CoordNormalizeData::new(Size::new(8.0, 2.0));
    CoordNormalizeShaderBlock::begin_block(&context, &data);
    add_solid(&context);
    fixture.builder.borrow_mut().end_block();

    assert_eq!(fixture.key_string(), "CoordNormalize[SolidColor] ");
    let mut expected = f32_bytes(&[0.125, 0.5]);
    expected.extend([0u8; 8]);
    expected.extend(f32_bytes(&SOLID));
    assert_eq!(fixture.uniform_bytes(), expected);
}

#[test]
fn dither_block_writes_its_range() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    // With no look-up table bound, the block only contributes its range. Std140 writes a half at
    // full precision, so the range is a float.
    DitherShaderBlock::add_block(&context, &DitherData::new(0.5, None));

    assert_eq!(fixture.key_string(), "Dither ");
    assert_eq!(fixture.uniform_bytes(), f32_bytes(&[0.5]));
}

#[test]
fn snippet_names_match_the_dictionary() {
    // The names the expected strings rely on are the snippets' own names.
    let dict = ShaderCodeDictionary::new(Layout::Std140, &[]);
    assert_eq!(
        dict.get_entry_built_in(BuiltInCodeSnippetID::SolidColorShader)
            .name,
        "SolidColor"
    );
    assert_eq!(
        dict.get_entry_built_in(BuiltInCodeSnippetID::LocalMatrixShader)
            .name,
        "LocalMatrix"
    );
}

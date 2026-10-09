// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/PaintParams.{h,cpp}

//! Whole paints keyed through `PaintParams` and `ShadingParams`: the key string and the uniform
//! bytes a draw gathers.
//!
//! Each expectation is derived from the control flow of `PaintParams.cpp` for that paint, not
//! read back from the port. The key strings are the single-line `PaintParamsKey::toString` output,
//! which skips the root headers (`key_to_string`), so the roots show as the blocks they hold in
//! order: source color, then the final blend, then the clip. `inner+outer` is a compose. The
//! uniform bytes follow the std140 layout of the blocks, as in `key_helpers.rs`: a vec4 starts on a
//! 16-byte boundary and a scalar packs after the previous scalar.

mod support;

use std::cell::RefCell;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::runtime_color_filter::RuntimeColorFilter;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::color_shader::ColorShader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation};
use skia_rust_effects::linear_gradient::LinearGradient;
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::draw_types::DstUsage;
use skia_rust_gpu::graphite::key_context::KeyContext;
use skia_rust_gpu::graphite::paint_params::{PaintParams, ShadingParams};
use skia_rust_gpu::graphite::paint_params_key::PaintParamsKeyBuilder;
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::render_step::Coverage;
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID;
use support::MockCaps;

/// An sRGB destination, so that `Color4fPrepForDst` leaves colors unchanged.
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

/// A two-stop opaque linear gradient from red to blue. It is built as the bare `LinearGradient`,
/// so the key has no local-matrix wrapper.
fn red_to_blue_gradient() -> Shader {
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
    ];
    let desc = Gradient::new(
        Colors::new(&colors, None, TileMode::Clamp, None::<ColorSpace>),
        Interpolation::default(),
    );
    Shader::from_base(LinearGradient::new(
        &[Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
        &desc,
    ))
}

/// The gradient's uniforms as `LinearGradient4` writes them: the colors padded to four stops, the
/// offsets 0, 1 and the padding, then the tile mode, color space and unpremul flag as ints. The
/// block ends at 92 bytes.
fn red_to_blue_gradient_bytes() -> Vec<u8> {
    let mut bytes = f32_bytes(&[
        1.0, 0.0, 0.0, 1.0, //
        0.0, 0.0, 1.0, 1.0, //
        0.0, 0.0, 1.0, 1.0, //
        0.0, 0.0, 1.0, 1.0,
    ]);
    bytes.extend(f32_bytes(&[0.0, 1.0, 1.0, 1.0]));
    bytes.extend(i32_bytes(&[0, 0, 0]));
    bytes
}

/// The gradient as a shader's color, which goes through a color-space transform to the sRGB
/// premultiplied destination. Its `PreAlpha` stage writes one half, the mode 1 (premultiply
/// inline, since the conversion is the identity), after the gradient's own uniforms.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1246-L1252 (chrome/m156), the `PreAlpha` mode
fn red_to_blue_gradient_in_dst_bytes() -> Vec<u8> {
    let mut bytes = red_to_blue_gradient_bytes();
    bytes.extend(f32_bytes(&[1.0]));
    bytes
}

/// Keys `params` as a draw with `coverage` into an RGBA8 target, and returns the id and dst usage.
fn key_paint(
    fixture: &Fixture,
    params: &PaintParams,
    coverage: Coverage,
) -> Option<(UniquePaintParamsID, DstUsage)> {
    let info = srgb_info();
    let context = fixture.context(&info);
    let shading = ShadingParams::new(
        fixture.caps.as_ref(),
        params,
        None,
        None,
        coverage,
        TextureFormat::RGBA8,
    );
    shading.to_key(&context)
}

#[test]
fn solid_paint_keys_its_premultiplied_color_with_src_over() {
    // An unpremultiplied (0.25, 0.5, 0.75, 0.5) paint is not opaque, so the src-over final blend
    // keeps the dst (`DEPENDS_ON_DST`), and the color is the premultiplied one.
    let fixture = Fixture::new();
    let paint = Paint::new(Color4f::new(0.25, 0.5, 0.75, 0.5), None);
    let params = PaintParams::new(&paint, None, false, false);

    let (id, dst_usage) = key_paint(&fixture, &params, Coverage::None).expect("a valid key");

    assert!(id.is_valid());
    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(fixture.key_string(), "SolidColor SrcOver ");
    assert_eq!(
        fixture.uniform_bytes(),
        f32_bytes(&[0.125, 0.25, 0.375, 0.5])
    );
}

#[test]
fn constant_shader_folds_into_the_paint_color() {
    // `PaintParams` simplifies a constant shader into the color: the shader's color keeps its rgb
    // and the paint's alpha 0.5 multiplies its alpha 0.5. The shader is then removed, so the key
    // is the plain solid color.
    let fixture = Fixture::new();
    let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 0.5), None);
    paint.set_shader(Shader::from_base(ColorShader::new(Color4f::new(
        0.25, 0.5, 0.75, 0.5,
    ))));
    let params = PaintParams::new(&paint, None, false, false);

    let (_, dst_usage) = key_paint(&fixture, &params, Coverage::None).expect("a valid key");

    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(fixture.key_string(), "SolidColor SrcOver ");
    assert_eq!(
        fixture.uniform_bytes(),
        f32_bytes(&[0.0625, 0.125, 0.1875, 0.25])
    );
}

#[test]
fn clear_blend_mode_keys_a_transparent_src() {
    // kClear becomes kSrc over a transparent color, so the paint reads no dst.
    let fixture = Fixture::new();
    let params = PaintParams::from_color(Color4f::new(1.0, 0.0, 0.0, 1.0), BlendMode::Clear);

    let (_, dst_usage) = key_paint(&fixture, &params, Coverage::None).expect("a valid key");

    assert_eq!(dst_usage, DstUsage::NONE);
    assert_eq!(fixture.key_string(), "SolidColor Src ");
    assert_eq!(fixture.uniform_bytes(), vec![0; 16]);
}

#[test]
fn gradient_color_filter_and_blend_mode() {
    // The shader is the inner of the color filter's compose. The blend color filter's color is
    // premultiplied. Multiply is an advanced blend the hardware cannot do here, so it is a fixed
    // blend that reads the dst.
    let fixture = Fixture::new();
    let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
    paint.set_shader(red_to_blue_gradient());
    paint.set_color_filter(color_filters::blend(
        Color4f::new(0.25, 0.5, 0.75, 0.5),
        None,
        BlendMode::SrcOver,
    ));
    paint.set_blend_mode(BlendMode::Multiply);
    let params = PaintParams::new(&paint, None, false, false);

    let (id, dst_usage) = key_paint(&fixture, &params, Coverage::None).expect("a valid key");

    assert!(id.is_valid());
    assert_eq!(
        dst_usage,
        DstUsage::DEPENDS_ON_DST | DstUsage::DST_READ_REQUIRED | DstUsage::ADVANCED_BLEND
    );
    // The shader is the inner of the compose and the blend color filter the outer. A compose whose
    // inner child is itself a compose is not shortened to `A+B`.
    assert_eq!(
        fixture.key_string(),
        "Compose[LinearGradient4+PreAlpha, BlendCompose[SolidColor, Passthrough, PorterDuffBlender]] \
         Multiply "
    );
    // The blend color filter's source (premultiplied) comes first, then the src-over
    // coefficients, as in the composed color filter test of `key_helpers_ii.rs`.
    let mut expected = red_to_blue_gradient_in_dst_bytes();
    expected.extend(f32_bytes(&[0.125, 0.25, 0.375, 0.5, 1.0, 1.0, 0.0, -1.0]));
    assert_eq!(fixture.uniform_bytes(), pad_to_16(expected));
}

#[test]
fn dithered_gradient_composes_the_dither_block() {
    // A dithered paint with a non-constant shader gets a dither block around its color. The dither
    // range of an 8-bit target is 1/255, a scalar that packs after the gradient's ints.
    let fixture = Fixture::new();
    let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
    paint.set_shader(red_to_blue_gradient());
    paint.set_dither(true);
    let params = PaintParams::new(&paint, None, false, false);

    let (_, dst_usage) = key_paint(&fixture, &params, Coverage::None).expect("a valid key");

    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(
        fixture.key_string(),
        "Compose[LinearGradient4+PreAlpha, Dither] SrcOver "
    );
    let mut expected = red_to_blue_gradient_in_dst_bytes();
    expected.extend(f32_bytes(&[1.0 / 255.0]));
    assert_eq!(fixture.uniform_bytes(), pad_to_16(expected));
}

#[test]
fn runtime_color_filter_on_a_gradient() {
    // A runtime color filter is keyed as its own block, and its uniforms are written in order
    // after the shader's. The filter is not alpha-preserving, so the final blend keeps the dst.
    let effect = RuntimeEffect::make_for_color_filter(
        "uniform half4 c; half4 main(half4 color) { return color * c; }",
        None,
    )
    .expect("compiles");
    let uniforms = Data::new_copy(&f32_bytes(&[0.5, 0.25, 1.0, 0.75]));
    let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
    paint.set_shader(red_to_blue_gradient());
    paint.set_color_filter(ColorFilter::from_base(RuntimeColorFilter::new(
        effect,
        uniforms,
        &[],
    )));
    let params = PaintParams::new(&paint, None, false, false);

    let fixture = Fixture::new();
    let (_, dst_usage) = key_paint(&fixture, &params, Coverage::None).expect("a valid key");

    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(
        fixture.key_string(),
        "Compose[LinearGradient4+PreAlpha, RuntimeEffect] SrcOver "
    );
    let mut expected = red_to_blue_gradient_in_dst_bytes();
    expected.extend(f32_bytes(&[0.5, 0.25, 1.0, 0.75]));
    assert_eq!(fixture.uniform_bytes(), pad_to_16(expected));
}

#[test]
fn clip_shader_becomes_the_clip_root() {
    // The clip shader is keyed as the clipping root, after the source color and the final blend.
    // An analytic clip is present here, so the final src-over keeps the dst.
    let fixture = Fixture::new();
    let paint = Paint::new(Color4f::new(0.25, 0.5, 0.75, 1.0), None);
    let params = PaintParams::new(&paint, None, false, false);
    let clip = red_to_blue_gradient();

    let info = srgb_info();
    let context = fixture.context(&info);
    let shading = ShadingParams::new(
        fixture.caps.as_ref(),
        &params,
        None,
        Some(&clip),
        Coverage::None,
        TextureFormat::RGBA8,
    );
    let (_, dst_usage) = shading.to_key(&context).expect("a valid key");

    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(
        fixture.key_string(),
        "SolidColor SrcOver LinearGradient4+PreAlpha "
    );
    let mut expected = f32_bytes(&[0.25, 0.5, 0.75, 1.0]);
    expected.extend(red_to_blue_gradient_in_dst_bytes());
    assert_eq!(fixture.uniform_bytes(), pad_to_16(expected));
}

#[test]
fn optimize_for_opacity_rewrites_the_final_blend_to_src() {
    // An opaque paint under a renderer with coverage keeps kDstOnlyUsedByRenderer. Its opaque,
    // coverage-less version is the same key with the final blend changed to Src.
    let fixture = Fixture::new();
    let paint = Paint::new(Color4f::new(0.25, 0.5, 0.75, 1.0), None);
    let params = PaintParams::new(&paint, None, false, false);

    let info = srgb_info();
    let context = fixture.context(&info);
    let shading = ShadingParams::new(
        fixture.caps.as_ref(),
        &params,
        None,
        None,
        Coverage::SingleChannel,
        TextureFormat::RGBA8,
    );
    let (id, dst_usage) = shading.to_key(&context).expect("a valid key");
    assert_eq!(
        dst_usage,
        DstUsage::DEPENDS_ON_DST | DstUsage::DST_ONLY_USED_BY_RENDERER
    );
    assert_eq!(fixture.key_string(), "SolidColor SrcOver ");

    let opaque_id = shading.optimize_for_opacity(&context, id);

    assert!(opaque_id.is_valid());
    assert_eq!(fixture.key_string(), "SolidColor Src ");
}

#[test]
fn runtime_shader_paint_routes_through_add_to_key_shader() {
    // A runtime shader is not constant, so the paint keeps it. Its color is the shader output,
    // which is not known to be opaque, so the src-over final blend keeps the dst.
    let effect =
        RuntimeEffect::make_for_shader("uniform half4 c; half4 main(float2 p) { return c; }", None)
            .expect("compiles");
    let uniforms = Data::new_copy(&f32_bytes(&[0.5, 0.25, 1.0, 0.75]));
    let shader = effect
        .make_shader(uniforms, &[], None)
        .expect("a runtime shader");
    let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
    paint.set_shader(shader);
    let params = PaintParams::new(&paint, None, false, false);

    let fixture = Fixture::new();
    let (_, dst_usage) = key_paint(&fixture, &params, Coverage::None).expect("a valid key");

    assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
    assert_eq!(fixture.key_string(), "RuntimeEffect SrcOver ");
    assert_eq!(fixture.uniform_bytes(), f32_bytes(&[0.5, 0.25, 1.0, 0.75]));
}

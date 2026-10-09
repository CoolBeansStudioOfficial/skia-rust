// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/KeyHelpers.cpp (the color filter, blender, runtime effect,
//                    color space and primitive color blocks)

//! The `KeyHelpers` blocks for color filters and blenders: for a handful of them, the key's
//! `toString` and the uniform bytes the gatherer collects, each hand-derived from the C++.
//!
//! The uniform layout is `Std140` (full precision), so every `half` value is stored as a 32-bit
//! float. Floats are written in native byte order, as the gatherer writes them.

mod support;

use std::cell::RefCell;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::runtime_color_filter::RuntimeColorFilter;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::swizzle::Swizzle;
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::key_context::KeyContext;
use skia_rust_gpu::graphite::key_helpers_ii::{
    ColorSpaceTransformBlock, ColorSpaceTransformData, MatrixColorFilterBlock,
    MatrixColorFilterData, add_blend_mode, add_blend_mode_color_filter, add_primitive_color,
    add_to_key_blender, add_to_key_color_filter,
};
use skia_rust_gpu::graphite::paint_params_key::PaintParamsKeyBuilder;
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use support::MockCaps;

fn srgb_premul_info() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    )
}

/// The uniform bytes of `values`, as the gatherer lays out floats (native byte order).
fn floats(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// The bytes `UniformManager::finish` returns for `values` when only scalars and vec2s were
/// written: the floats, unpadded.
fn block(values: &[f32]) -> Vec<u8> {
    floats(values)
}

/// The bytes `UniformManager::finish` returns once a 16-byte-aligned value (a vec4 or a matrix
/// column) has been written: the floats, padded with zeros to 16 bytes.
// Port of: src/gpu/graphite/UniformManager.h#L245-L251 (chrome/m156), `finish`, which aligns the
// storage to `fReqAlignment`, the largest alignment written so far
fn padded16(values: &[f32]) -> Vec<u8> {
    let mut bytes = floats(values);
    bytes.resize(bytes.len().next_multiple_of(16), 0);
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
        Self {
            caps: Arc::new(MockCaps::default()),
            builder: RefCell::new(PaintParamsKeyBuilder::new(&dict)),
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

    /// The key the blocks built so far, as `toString` prints it.
    fn key_string(&self) -> String {
        let mut builder = self.builder.borrow_mut();
        let lock = builder.lock_as_key();
        lock.key().to_string(&*self.caps, &self.dict)
    }

    /// The uniform bytes gathered so far.
    fn uniform_bytes(&self) -> Vec<u8> {
        self.gatherer
            .borrow_mut()
            .uniform_manager()
            .finish()
            .to_vec()
    }
}

/// A 20-entry color matrix (`SkColorMatrix`, row-major, translation in the fifth column).
#[rustfmt::skip]
const IDENTITY_MATRIX: [f32; 20] = [
    1.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 1.0, 0.0,
];

/// Scales alpha by one half.
#[rustfmt::skip]
const HALF_ALPHA_MATRIX: [f32; 20] = [
    1.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.5, 0.0,
];

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1714-L1722 (chrome/m156), `add_to_key(SkBlendModeColorFilter)`
#[test]
fn blend_mode_color_filter_src_over_key_and_uniforms() {
    // Blend [ src: SolidColor(color), dst: PriorOutput, blend: PorterDuff(kSrcOver) ]
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());
    let color = PMColor4f::new(0.25, 0.5, 0.75, 1.0);

    add_blend_mode_color_filter(&key_context, BlendMode::SrcOver, &color);

    assert_eq!(
        fixture.key_string(),
        "BlendCompose[SolidColor, Passthrough, PorterDuffBlender] "
    );
    // SolidColor writes the premultiplied color; PorterDuff writes kSrcOver = {1, 1, 0, -1}.
    assert_eq!(
        fixture.uniform_bytes(),
        block(&[0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 0.0, -1.0])
    );
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1714-L1722 (chrome/m156), through the
// `AddToKey(SkColorFilter*)` dispatch (KeyHelpers.cpp#L1838-L1861)
#[test]
fn blend_mode_filter_through_add_to_key_matches_the_helper() {
    // A translucent color: an opaque one with kSrcOver is simplified to kSrc by the factory, which
    // is the factory's doing, not the key's. `map_color` takes the sRGB color to the dst color
    // space with the dst alpha type (premul here), so the color comes out premultiplied:
    // (0.25, 0.5, 0.75, 0.5) * 0.5 on the color channels.
    let filter = color_filters::blend(Color4f::new(0.25, 0.5, 0.75, 0.5), None, BlendMode::SrcOver)
        .expect("a blend filter");

    let via_dispatch = Fixture::new();
    add_to_key_color_filter(&via_dispatch.context(&srgb_premul_info()), Some(&filter));

    let direct = Fixture::new();
    add_blend_mode_color_filter(
        &direct.context(&srgb_premul_info()),
        BlendMode::SrcOver,
        &PMColor4f::new(0.125, 0.25, 0.375, 0.5),
    );

    assert_eq!(
        via_dispatch.key_string(),
        "BlendCompose[SolidColor, Passthrough, PorterDuffBlender] "
    );
    assert_eq!(
        via_dispatch.uniform_bytes(),
        block(&[0.125, 0.25, 0.375, 0.5, 1.0, 1.0, 0.0, -1.0])
    );
    assert_eq!(via_dispatch.key_string(), direct.key_string());
    assert_eq!(via_dispatch.uniform_bytes(), direct.uniform_bytes());
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L996-L1007 (chrome/m156), `MatrixColorFilterBlock::AddBlock`
#[test]
fn matrix_color_filter_identity_clamped() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());
    let data = MatrixColorFilterData::new(&IDENTITY_MATRIX, false, true);

    MatrixColorFilterBlock::add_block(&key_context, &data);

    assert_eq!(fixture.key_string(), "MatrixColorFilter ");
    // The 4x4 part as four half4 columns (the identity is its own transpose), the translation
    // (zeros), then the clamp bounds {0, 1} as a half2. The columns are vec4s, so the block is
    // padded to 16 bytes.
    assert_eq!(
        fixture.uniform_bytes(),
        padded16(&[
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, //
            0.0, 0.0, 0.0, 0.0, //
            0.0, 1.0,
        ])
    );
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L996-L1007 (chrome/m156), the unclamped bounds
#[test]
fn matrix_color_filter_unclamped_uses_the_max_half() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());
    let data = MatrixColorFilterData::new(&IDENTITY_MATRIX, false, false);

    MatrixColorFilterBlock::add_block(&key_context, &data);

    assert_eq!(fixture.key_string(), "MatrixColorFilter ");
    // The bounds {-65504, 65504} follow the matrix and the translation; the block is then padded.
    assert_eq!(
        fixture.uniform_bytes(),
        padded16(&[
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, //
            0.0, 0.0, 0.0, 0.0, //
            -65504.0, 65504.0,
        ])
    );
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L996-L1007 (chrome/m156), the HSLA branch
#[test]
fn matrix_color_filter_hsla_has_no_clamp_bounds() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());
    let data = MatrixColorFilterData::new(&IDENTITY_MATRIX, true, true);

    MatrixColorFilterBlock::add_block(&key_context, &data);

    assert_eq!(fixture.key_string(), "HSLMatrixColorFilter ");
    // Matrix and translation only: four half4 columns and the translation.
    assert_eq!(fixture.uniform_bytes().len(), 16 * 5);
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1041-L1116 (chrome/m156) and KeyHelpers.cpp#L1171-L1182
// (the alpha-only stage: `rToAlpha` is 1 when the read swizzle's alpha is the red channel)
#[test]
fn alpha_only_color_space_transform_writes_the_read_swizzle() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());
    let mut data = ColorSpaceTransformData::from_swizzle(Swizzle::new("000r"));
    data.is_alpha_only = true;

    ColorSpaceTransformBlock::add_block(&key_context, &data);

    assert_eq!(fixture.key_string(), "AlphaOnly ");
    assert_eq!(fixture.uniform_bytes(), block(&[1.0]));
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1171-L1315 (chrome/m156), the identity conversion
// without specialization: the pre-alpha stage alone, with mode 0 (a no-op)
#[test]
fn identity_color_space_transform_keeps_the_pre_alpha_stage() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());
    let data = ColorSpaceTransformData::from_swizzle(Swizzle::new("rgba"));

    ColorSpaceTransformBlock::add_block(&key_context, &data);

    assert_eq!(fixture.key_string(), "PreAlpha ");
    assert_eq!(fixture.uniform_bytes(), block(&[0.0]));
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1171-L1315 (chrome/m156), the specialized identity
// conversion: no stages, so the prior output passes through
#[test]
fn specialized_identity_color_space_transform_is_passthrough() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info()).with_extra_flags(
        skia_rust_gpu::graphite::key_context::KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM,
    );
    let data = ColorSpaceTransformData::from_swizzle(Swizzle::new("rgba"));

    ColorSpaceTransformBlock::add_block(&key_context, &data);

    assert_eq!(fixture.key_string(), "Passthrough ");
    assert_eq!(fixture.uniform_bytes(), Vec::<u8>::new());
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1437-L1464 (chrome/m156), `AddPrimitiveColor`
#[test]
fn primitive_color_in_the_destination_space_is_a_composed_pre_alpha() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());

    add_primitive_color(&key_context, false, None, AlphaType::Premul);

    // Compose [ PrimitiveColor, PreAlpha ], printed as `A+B` because the inner child is not a
    // Compose.
    assert_eq!(fixture.key_string(), "PrimitiveColor+PreAlpha ");
    assert_eq!(fixture.uniform_bytes(), block(&[0.0]));
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1437-L1442 (chrome/m156), `skipColorXform`
#[test]
fn primitive_color_that_skips_the_transform_is_one_block() {
    let fixture = Fixture::new();
    let key_context = fixture.context(&srgb_premul_info());

    add_primitive_color(&key_context, true, None, AlphaType::Premul);

    assert_eq!(fixture.key_string(), "PrimitiveColor ");
    assert_eq!(fixture.uniform_bytes(), Vec::<u8>::new());
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1617-L1659 and #L1528-L1553 (chrome/m156): a runtime
// color filter's uniforms are written in declaration order, as the bytes of its SkData
#[test]
fn runtime_color_filter_writes_its_uniform_bytes() {
    let effect = RuntimeEffect::make_for_color_filter(
        "uniform half4 c; half4 main(half4 color) { return color * c; }",
        None,
    )
    .expect("compiles");
    let uniforms = Data::new_copy(&floats(&[0.5, 0.25, 1.0, 0.75]));
    let filter = ColorFilter::from_base(RuntimeColorFilter::new(effect, uniforms, &[]));

    let fixture = Fixture::new();
    add_to_key_color_filter(&fixture.context(&srgb_premul_info()), Some(&filter));

    assert_eq!(fixture.key_string(), "RuntimeEffect ");
    assert_eq!(fixture.uniform_bytes(), block(&[0.5, 0.25, 1.0, 0.75]));
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1733-L1743 (chrome/m156), `add_to_key(SkComposeColorFilter*)`
// and #L1838-L1861 (chrome/m156): the inner filter's uniforms come before the outer's
#[test]
fn composed_color_filter_gathers_inner_then_outer() {
    let outer =
        color_filters::matrix_row_major(&HALF_ALPHA_MATRIX, Clamp::Yes).expect("a matrix filter");
    let inner = color_filters::blend(Color4f::new(0.25, 0.5, 0.75, 0.5), None, BlendMode::SrcOver)
        .expect("a blend filter");
    let composed = color_filters::compose(Some(&outer), Some(inner)).expect("a composed filter");

    let fixture = Fixture::new();
    add_to_key_color_filter(&fixture.context(&srgb_premul_info()), Some(&composed));

    assert_eq!(
        fixture.key_string(),
        "BlendCompose[SolidColor, Passthrough, PorterDuffBlender]+MatrixColorFilter "
    );
    // The inner blend's (src color, then its coefficients), then the outer matrix's four columns,
    // translation and clamp bounds: 32 + 88 bytes, padded to 128.
    assert_eq!(
        fixture.uniform_bytes(),
        padded16(&[
            0.125, 0.25, 0.375, 0.5, 1.0, 1.0, 0.0, -1.0, //
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 0.5, //
            0.0, 0.0, 0.0, 0.0, //
            0.0, 1.0,
        ])
    );
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L2703-L2724 (chrome/m156), `AddBlendMode`
#[test]
fn hsl_blend_mode_uses_the_hslc_blender_coefficients() {
    let fixture = Fixture::new();
    add_blend_mode(&fixture.context(&srgb_premul_info()), BlendMode::Hue);

    assert_eq!(fixture.key_string(), "HSLCBlender ");
    // blend_hslc's coefficients kHue = {0, 1}.
    assert_eq!(fixture.uniform_bytes(), block(&[0.0, 1.0]));
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L2703-L2724 (chrome/m156), the fixed-blend branch
#[test]
fn separable_blend_mode_is_a_fixed_blend_without_uniforms() {
    let fixture = Fixture::new();
    add_blend_mode(&fixture.context(&srgb_premul_info()), BlendMode::Screen);

    assert_eq!(fixture.key_string(), "Screen ");
    assert_eq!(fixture.uniform_bytes(), Vec::<u8>::new());
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L1683-L1702 (chrome/m156), `AddToKey(SkBlender*)`
#[test]
fn blend_mode_blender_is_its_fixed_blend() {
    let fixture = Fixture::new();
    let blender = Blender::mode(BlendMode::Multiply);

    add_to_key_blender(&fixture.context(&srgb_premul_info()), Some(&blender));

    assert_eq!(fixture.key_string(), "Multiply ");
    assert_eq!(fixture.uniform_bytes(), Vec::<u8>::new());
}

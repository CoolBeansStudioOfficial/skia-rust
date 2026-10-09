// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/PipelineData.{h,cpp} (the texture half of the gatherer) and
// src/gpu/graphite/KeyHelpers.cpp (the texture bindings and the shader AddToKey cases)

//! The sampled textures that the gatherer collects for the `KeyHelpers` blocks, and the keys of
//! the shaders whose blocks were completed in this step.
//!
//! The tests run without a `Recorder`, which is Skia's pre-compile path: texture bindings are
//! then `None` proxies with their samplers, and the table and gradient textures that need a
//! recorder's proxy cache add the fallback blocks Skia adds when `CreateCachedProxy` returns null.

mod support;

use std::cell::RefCell;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_table::ColorTable;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::raster_pipeline::contexts::PerlinNoiseShaderType;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::color_shader::ColorShader;
use skia_rust_core::shaders::{BlendShader, ColorFilterShader, CoordClampShader, CtmShader};
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;
use skia_rust_effects::perlin_noise_shader::PerlinNoiseShader;
use skia_rust_gpu::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::key_context::KeyContext;
use skia_rust_gpu::graphite::key_helpers::{
    DitherData, DitherShaderBlock, ImageData, ImageShaderBlock, PerlinNoiseData,
    PerlinNoiseShaderBlock, PerlinNoiseType, add_to_key_shader,
};
use skia_rust_gpu::graphite::key_helpers_ii::{
    TableColorFilterBlock, TableColorFilterData, add_to_key_color_filter,
};
use skia_rust_gpu::graphite::paint_params_key::{PaintParamsKeyBuilder, RootBlockType};
use skia_rust_gpu::graphite::pipeline_data::{PipelineDataGatherer, SampledTexture};
use skia_rust_gpu::graphite::resource_types::{ImmutableSamplerInfo, Layout, SamplerDesc};
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

/// The solid color the tests use.
const SOLID: [f32; 4] = [0.25, 0.5, 0.75, 1.0];

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

    /// Whether the key built so far has no error block.
    fn key_is_valid(&self) -> bool {
        let mut builder = self.builder.borrow_mut();
        let lock = builder.lock_as_key();
        lock.key().is_valid()
    }

    /// The uniform bytes gathered so far for a shading draw.
    fn uniform_bytes(&self) -> Vec<u8> {
        let (uniforms, _textures) = self.gatherer.borrow_mut().end_combined_data(true);
        uniforms.data().to_vec()
    }

    /// The samplers of the textures gathered so far for a shading draw, in binding order. The
    /// pre-compile path binds no proxies, so each one is checked to be `None`.
    fn bound_samplers(&self) -> Vec<SamplerDesc> {
        let (_uniforms, textures) = self.gatherer.borrow_mut().end_combined_data(true);
        samplers_without_proxies(textures.textures())
    }

    /// The name of a built-in snippet, as the key string spells it.
    fn name(&self, id: BuiltInCodeSnippetID) -> String {
        self.dict.get_entry_built_in(id).name.clone()
    }
}

/// The samplers of `textures`, each of which must have no proxy (the pre-compile path).
fn samplers_without_proxies(textures: &[SampledTexture]) -> Vec<SamplerDesc> {
    textures
        .iter()
        .map(|(proxy, sampler)| {
            assert!(proxy.is_none(), "the pre-compile path binds no proxies");
            *sampler
        })
        .collect()
}

fn solid_shader() -> Shader {
    Shader::from_base(ColorShader::new(Color4f {
        r: SOLID[0],
        g: SOLID[1],
        b: SOLID[2],
        a: SOLID[3],
    }))
}

fn nearest() -> SamplingOptions {
    SamplingOptions::from(FilterMode::Nearest)
}

#[test]
fn dither_binds_its_look_up_table_with_nearest_repeat_sampling() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    DitherShaderBlock::add_block(&context, &DitherData::new(0.5, None));

    // The table is a texture with no proxy on the pre-compile path, and its sampler is fixed.
    assert_eq!(
        fixture.bound_samplers(),
        vec![SamplerDesc::new(&nearest(), TileMode::Repeat)]
    );
}

#[test]
fn perlin_noise_binds_its_permutation_and_noise_tables_in_order() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let data = PerlinNoiseData::new(
        PerlinNoiseType::FractalNoise,
        Point::new(0.1, 0.2),
        2,
        ISize::new(0, 0),
    );
    PerlinNoiseShaderBlock::add_block(&context, &data);

    // Both tables repeat in x and clamp in y.
    let sampler = SamplerDesc::new_with_tile_modes(
        &nearest(),
        (TileMode::Repeat, TileMode::Clamp),
        ImmutableSamplerInfo::default(),
    );
    assert_eq!(fixture.bound_samplers(), vec![sampler, sampler]);
    assert_eq!(
        fixture.key_string(),
        format!("{} ", fixture.name(BuiltInCodeSnippetID::PerlinNoiseShader))
    );
}

#[test]
fn image_shader_binds_its_sampler_with_the_hardware_tiling_modes() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    // A full-image subset with clamp tiling is tiled in hardware, so the tile modes are kept.
    let data = ImageData::new(
        SamplingOptions::from(FilterMode::Linear),
        TileMode::Clamp,
        TileMode::Clamp,
        ISize::new(4, 4),
        Rect::from_wh(4.0, 4.0),
        ImmutableSamplerInfo::default(),
    );
    ImageShaderBlock::add_block(&context, &data);

    assert_eq!(
        fixture.bound_samplers(),
        vec![SamplerDesc::new_with_tile_modes(
            &SamplingOptions::from(FilterMode::Linear),
            (TileMode::Clamp, TileMode::Clamp),
            ImmutableSamplerInfo::default(),
        )]
    );
}

#[test]
fn table_color_block_binds_its_table_with_nearest_clamp_sampling() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    TableColorFilterBlock::add_block(&context, &TableColorFilterData::new(None));

    assert_eq!(
        fixture.bound_samplers(),
        vec![SamplerDesc::new(&nearest(), TileMode::Clamp)]
    );
    assert_eq!(
        fixture.key_string(),
        format!("{} ", fixture.name(BuiltInCodeSnippetID::TableColorFilter))
    );
}

#[test]
fn table_color_filter_without_a_recorder_passes_the_input_through() {
    // Skia's `RecorderPriv::CreateCachedProxy` returns null without a recorder, and the table
    // filter then keeps the input color (`kPriorOutput`).
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let table = ColorTable::make(&[0_u8; 256]);
    let filter = color_filters::table_from_color_table(table);
    add_to_key_color_filter(&context, Some(&filter));

    assert!(fixture.key_is_valid());
    assert_eq!(
        fixture.key_string(),
        format!("{} ", fixture.name(BuiltInCodeSnippetID::PriorOutput))
    );
    assert_eq!(fixture.bound_samplers().len(), 0);
}

#[test]
fn gradient_with_more_stops_than_fit_needs_a_recorder_for_its_texture() {
    // Above eight stops the colors and offsets live in a texture, which comes from the recorder's
    // proxy cache. Without a recorder Skia adds an error block.
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let colors: Vec<Color4f> = (0_u8..9)
        .map(|i| {
            let t = f32::from(i) / 8.0;
            Color4f {
                r: t,
                g: 1.0 - t,
                b: 0.5,
                a: 1.0,
            }
        })
        .collect();
    let shader = gradient_shader::linear(
        (Point::new(0.0, 0.0), Point::new(1.0, 0.0)),
        &colors[..],
        None,
        TileMode::Clamp,
        None,
        None,
    )
    .expect("a linear gradient with nine stops");
    add_to_key_shader(&context, Some(&shader));

    assert!(!fixture.key_is_valid());
    assert_eq!(fixture.bound_samplers().len(), 0);
}

#[test]
fn ctm_shader_keys_its_proxy_under_the_inverse_ctm() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let shader = Shader::from_base(CtmShader::new(
        solid_shader(),
        Matrix::translate(Point::new(3.0, 4.0)),
    ));
    add_to_key_shader(&context, Some(&shader));

    // The CTM is applied as a local matrix, so the key matches a local matrix of the same
    // transform (SkCTMShader's add_local_matrix_to_key with an identity post-inverse).
    assert_eq!(fixture.key_string(), "LocalMatrix[SolidColor] ");
    assert!(fixture.key_is_valid());
    // The inverse translation (-3, -4) and the identity upper 2x2, then the solid color; the
    // matrix block is padded to 8 bytes before the vec4 (std140), as in the local matrix test.
    let mut expected = f32_bytes(&[1.0, 0.0, 0.0, 1.0, -3.0, -4.0]);
    expected.extend([0u8; 8]);
    expected.extend(f32_bytes(&SOLID));
    assert_eq!(fixture.uniform_bytes(), expected);
}

#[test]
fn blend_shader_keys_its_src_and_dst_children_under_its_mode() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let shader = Shader::from_base(BlendShader::new(
        BlendMode::SrcOver,
        solid_shader(),
        solid_shader(),
    ));
    add_to_key_shader(&context, Some(&shader));

    // Skia's Blend() adds the src child, then the dst child, then the blend mode.
    assert!(fixture.key_is_valid());
    assert_eq!(
        fixture.key_string(),
        "BlendCompose[SolidColor, SolidColor, PorterDuffBlender] "
    );
}

#[test]
fn color_filter_shader_composes_its_shader_with_its_filter() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let filter = color_filters::blend(
        Color4f {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        None,
        BlendMode::SrcIn,
    )
    .expect("a blend color filter");
    let shader = ColorFilterShader::make(solid_shader(), 1.0, Some(filter));
    add_to_key_shader(&context, Some(&shader));

    // The filter is the outer child of a compose: its blend takes the filter color (src) over the
    // prior output (dst) with the filter's mode.
    assert!(fixture.key_is_valid());
    assert_eq!(
        fixture.key_string(),
        format!(
            "SolidColor+BlendCompose[SolidColor, {}, PorterDuffBlender] ",
            fixture.name(BuiltInCodeSnippetID::PriorOutput)
        )
    );
}

#[test]
fn coord_clamp_shader_keys_its_child_without_sampling_optimization() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let clamp = CoordClampShader::make(Some(solid_shader()), Rect::from_ltrb(0.0, 0.0, 4.0, 4.0))
        .expect("a sorted subset clamps");
    add_to_key_shader(&context, Some(&clamp));

    // The clamp block wraps its child, which is keyed with sampling optimization disabled (no
    // extra key text, so the structure is the block around the solid color).
    assert!(fixture.key_is_valid());
    assert_eq!(fixture.key_string(), "CoordClamp[SolidColor] ");
}

#[test]
fn coord_clamp_shader_rejects_an_unsorted_subset() {
    assert!(
        CoordClampShader::make(Some(solid_shader()), Rect::from_ltrb(4.0, 4.0, 0.0, 0.0),)
            .is_none()
    );
}

#[test]
fn gatherer_rewind_keeps_the_paint_textures_and_drops_the_render_step_ones() {
    let mut gatherer = PipelineDataGatherer::new(Layout::Std140);
    let paint = SamplerDesc::new(&nearest(), TileMode::Clamp);
    let step = SamplerDesc::new(&nearest(), TileMode::Repeat);
    let step_again = SamplerDesc::new(&nearest(), TileMode::Decal);

    gatherer.add(None, paint);
    gatherer.mark_offset_and_align(true, 16);
    gatherer.add(None, step);
    {
        let (_uniforms, textures) = gatherer.end_combined_data(false);
        // A non-shading render step only sees its own textures.
        assert_eq!(samplers_without_proxies(textures.textures()), vec![step]);
    }

    gatherer.rewind_for_render_step();
    gatherer.add(None, step_again);
    let (_uniforms, textures) = gatherer.end_combined_data(true);
    // A shading render step sees the paint textures and then its own.
    assert_eq!(
        samplers_without_proxies(textures.textures()),
        vec![paint, step_again]
    );
}

#[test]
fn texture_blocks_compare_by_their_samplers_and_proxies() {
    use skia_rust_gpu::graphite::pipeline_data::{TextureDataBlock, TextureDataCache};

    let a = SamplerDesc::new(&nearest(), TileMode::Clamp);
    let b = SamplerDesc::new(&nearest(), TileMode::Repeat);
    let block_a = TextureDataBlock::make(&[(None, a)]);
    let block_a_again = TextureDataBlock::make(&[(None, a)]);
    let block_b = TextureDataBlock::make(&[(None, b)]);

    assert_eq!(block_a, block_a_again);
    assert_ne!(block_a, block_b);

    let mut cache = TextureDataCache::new();
    let index_a = cache.insert(block_a);
    assert_eq!(cache.insert(block_a_again), index_a);
    let index_b = cache.insert(block_b);
    assert_ne!(index_a, index_b);
    assert_eq!(cache.binding_count(), 2);
    // No proxies on the pre-compile path, so no unique textures either.
    assert_eq!(cache.unique_texture_count(), 0);
    assert_eq!(
        cache.get_bindings()[index_b as usize],
        TextureDataBlock::make(&[(None, b)])
    );
}

/// A 4x1 RGBA bitmap with its pixels allocated, so it has a pixel ref of its own.
fn bitmap_4x1() -> skia_rust_core::bitmap::Bitmap {
    use skia_rust_core::bitmap::Bitmap;
    use skia_rust_core::image_info::ImageInfo;

    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(
        &ImageInfo::new((4, 1), ColorType::RGBA8888, AlphaType::Premul, None),
        None,
    );
    bitmap
}

#[test]
fn create_cached_proxy_makes_one_texture_per_bitmap_identity() {
    use skia_rust_gpu::graphite::recorder::RecorderPriv;

    let (recorder, _shared) = support::make_recorder(MockCaps::default());
    let bitmap = bitmap_4x1();

    // Without a recorder there is nothing to cache in: Skia returns a null proxy.
    assert!(RecorderPriv::create_cached_proxy(None, &bitmap, "Test").is_none());

    let first = RecorderPriv::create_cached_proxy(Some(&recorder), &bitmap, "Test")
        .expect("the bitmap uploads to a texture");
    assert_eq!(first.dimensions(), ISize::new(4, 1));

    // The same bitmap (same pixel ref) hits the proxy cache and gives back the same proxy.
    let second =
        RecorderPriv::create_cached_proxy(Some(&recorder), &bitmap, "Test").expect("the cache hit");
    assert!(Arc::ptr_eq(&first, &second));

    let provider = recorder.priv_().resource_provider().clone();
    let mut provider = provider.lock().unwrap();
    assert_eq!(provider.proxy_cache().unwrap().num_cached(), 1);
}

#[test]
fn nine_stop_gradient_binds_its_color_and_offset_texture_with_a_recorder() {
    use skia_rust_core::m44::M44;
    use skia_rust_gpu::graphite::key_context::KeyGenFlags;
    use skia_rust_gpu::graphite::texture_format::TextureFormat;

    let (recorder, _shared) = support::make_recorder(MockCaps::default());
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = KeyContext::new_with_recorder(
        &recorder,
        TextureFormat::RGBA8,
        &fixture.builder,
        &fixture.gatherer,
        &M44::default(),
        &Rect::new_empty(),
        &info,
        KeyGenFlags::DEFAULT,
        &Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );

    let colors: Vec<Color4f> = (0_u8..9)
        .map(|i| {
            let t = f32::from(i) / 8.0;
            Color4f {
                r: t,
                g: 1.0 - t,
                b: 0.5,
                a: 1.0,
            }
        })
        .collect();
    let shader = gradient_shader::linear(
        (Point::new(0.0, 0.0), Point::new(1.0, 0.0)),
        &colors[..],
        None,
        TileMode::Clamp,
        None,
        None,
    )
    .expect("a linear gradient with nine stops");
    add_to_key_shader(&context, Some(&shader));

    // The stops are a cached texture, bound with nearest filtering and clamped tiling, and the
    // key uses it with no error.
    assert!(fixture.key_is_valid());
    let (_uniforms, textures) = fixture.gatherer.borrow_mut().end_combined_data(true);
    let textures = textures.textures();
    assert_eq!(textures.len(), 1);
    let (proxy, sampler) = &textures[0];
    let proxy = proxy.as_ref().expect("the color and offset texture");
    assert_eq!(
        *sampler,
        SamplerDesc::new(&SamplingOptions::from(FilterMode::Nearest), TileMode::Clamp)
    );
    // CreateGradientColorAndOffsetBitmap makes an RGBA F16 bitmap of numStops x 2: the colors on
    // the first row and the offsets on the second (src/gpu/GradientBitmap.cpp).
    assert_eq!(proxy.dimensions(), ISize::new(9, 2));
}

#[test]
fn table_color_filter_with_a_recorder_binds_its_cached_table() {
    use skia_rust_core::m44::M44;
    use skia_rust_gpu::graphite::key_context::KeyGenFlags;
    use skia_rust_gpu::graphite::texture_format::TextureFormat;

    let (recorder, _shared) = support::make_recorder(MockCaps::default());
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = KeyContext::new_with_recorder(
        &recorder,
        TextureFormat::RGBA8,
        &fixture.builder,
        &fixture.gatherer,
        &M44::default(),
        &Rect::new_empty(),
        &info,
        KeyGenFlags::DEFAULT,
        &Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    let filter = color_filters::table_from_color_table(ColorTable::make(&[0_u8; 256]));
    add_to_key_color_filter(&context, Some(&filter));

    assert!(fixture.key_is_valid());
    assert_eq!(
        fixture.key_string(),
        format!("{} ", fixture.name(BuiltInCodeSnippetID::TableColorFilter))
    );
    // The table is the filter's 256x4 A8 bitmap, cached in the recorder and sampled with nearest
    // filtering and clamped tiling.
    let (_uniforms, textures) = fixture.gatherer.borrow_mut().end_combined_data(true);
    let textures = textures.textures();
    assert_eq!(textures.len(), 1);
    let (proxy, sampler) = &textures[0];
    let proxy = proxy.as_ref().expect("the cached table texture");
    assert_eq!(proxy.dimensions(), ISize::new(256, 4));
    assert_eq!(
        *sampler,
        SamplerDesc::new(&SamplingOptions::from(FilterMode::Nearest), TileMode::Clamp)
    );
}

fn perlin_shader() -> Shader {
    Shader::from_base(PerlinNoiseShader::new(
        PerlinNoiseShaderType::FractalNoise,
        0.1,
        0.1,
        2,
        0.0,
        None,
    ))
}

#[test]
fn perlin_noise_permutation_table_holds_each_lattice_value_once() {
    // The lattice selector starts as the identity and is shuffled by swaps, so each value 0..255
    // appears exactly once. The tables are immutable A8 (256x1) and RGBA8888 (256x4) bitmaps.
    let shader =
        PerlinNoiseShader::new(PerlinNoiseShaderType::FractalNoise, 0.1, 0.1, 2, 0.0, None);
    let tables = shader.painting_tables();

    assert!(tables.permutations.is_immutable());
    assert!(tables.noise.is_immutable());
    assert_eq!(tables.permutations.dimensions(), ISize::new(256, 1));
    assert_eq!(tables.noise.dimensions(), ISize::new(256, 4));

    let mut lattice: Vec<u8> = (0..256)
        .map(|x| tables.permutations.get_addr8(x, 0))
        .collect();
    lattice.sort_unstable();
    assert_eq!(lattice, (0..=255_u8).collect::<Vec<u8>>());
}

#[test]
fn perlin_noise_binds_its_cached_tables_with_a_recorder() {
    use skia_rust_core::m44::M44;
    use skia_rust_gpu::graphite::key_context::KeyGenFlags;
    use skia_rust_gpu::graphite::texture_format::TextureFormat;

    let (recorder, _shared) = support::make_recorder(MockCaps::default());
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = KeyContext::new_with_recorder(
        &recorder,
        TextureFormat::RGBA8,
        &fixture.builder,
        &fixture.gatherer,
        &M44::default(),
        &Rect::new_empty(),
        &info,
        KeyGenFlags::DEFAULT,
        &Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    add_to_key_shader(&context, Some(&perlin_shader()));

    assert!(fixture.key_is_valid());
    assert_eq!(
        fixture.key_string(),
        format!("{} ", fixture.name(BuiltInCodeSnippetID::PerlinNoiseShader))
    );
    // The permutation table (256x1) then the noise table (256x4), both repeating in x.
    let (_uniforms, textures) = fixture.gatherer.borrow_mut().end_combined_data(true);
    let textures = textures.textures();
    assert_eq!(textures.len(), 2);
    let sampler = SamplerDesc::new_with_tile_modes(
        &SamplingOptions::from(FilterMode::Nearest),
        (TileMode::Repeat, TileMode::Clamp),
        ImmutableSamplerInfo::default(),
    );
    assert_eq!(textures[0].1, sampler);
    assert_eq!(textures[1].1, sampler);
    assert_eq!(
        textures[0]
            .0
            .as_ref()
            .expect("the permutation table")
            .dimensions(),
        ISize::new(256, 1)
    );
    assert_eq!(
        textures[1]
            .0
            .as_ref()
            .expect("the noise table")
            .dimensions(),
        ISize::new(256, 4)
    );
}

#[test]
fn perlin_noise_without_a_recorder_adds_an_error_block() {
    // Skia's CreateCachedProxy returns null without a recorder, so the shader's key is invalid.
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    add_to_key_shader(&context, Some(&perlin_shader()));

    assert!(!fixture.key_is_valid());
    assert_eq!(fixture.bound_samplers().len(), 0);
}

#[test]
fn dither_block_with_a_recorder_shares_one_cached_look_up_table() {
    use skia_rust_core::m44::M44;
    use skia_rust_gpu::graphite::key_context::KeyGenFlags;
    use skia_rust_gpu::graphite::key_helpers::add_dither_block;
    use skia_rust_gpu::graphite::texture_format::TextureFormat;

    let (recorder, _shared) = support::make_recorder(MockCaps::default());
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = KeyContext::new_with_recorder(
        &recorder,
        TextureFormat::RGBA8,
        &fixture.builder,
        &fixture.gatherer,
        &M44::default(),
        &Rect::new_empty(),
        &info,
        KeyGenFlags::DEFAULT,
        &Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    add_dither_block(&context, ColorType::RGBA8888);
    add_dither_block(&context, ColorType::RGBA8888);

    assert!(fixture.key_is_valid());
    assert_eq!(
        fixture.key_string(),
        format!("{0} {0} ", fixture.name(BuiltInCodeSnippetID::DitherShader))
    );
    // Both blocks bind the same cached LUT texture, made once by the recorder's proxy cache.
    let (_uniforms, textures) = fixture.gatherer.borrow_mut().end_combined_data(true);
    let textures = textures.textures();
    assert_eq!(textures.len(), 2);
    let first = textures[0].0.as_ref().expect("the dither LUT");
    let second = textures[1].0.as_ref().expect("the dither LUT");
    assert!(Arc::ptr_eq(first, second));
}

#[test]
fn dither_range_for_eight_bit_colors_is_one_over_255() {
    // DitherRangeForConfig: `1 / 255.f` for the 8-bit color types. Without a recorder the LUT is
    // None, and the block writes only its range.
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    skia_rust_gpu::graphite::key_helpers::add_dither_block(&context, ColorType::RGBA8888);

    assert_eq!(
        fixture.key_string(),
        format!("{} ", fixture.name(BuiltInCodeSnippetID::DitherShader))
    );
    assert_eq!(fixture.uniform_bytes(), f32_bytes(&[1.0_f32 / 255.0_f32]));
    assert_eq!(fixture.bound_samplers().len(), 1);
}

// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/KeyContext.{h,cpp}

//! `KeyContext`: construction for the pre-compile and recorder code paths, the scoped copies, and
//! the `RuntimeEffectDictionary` it hands out.

mod support;

use std::cell::RefCell;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_gpu::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::key_context::{KeyContext, KeyGenFlags};
use skia_rust_gpu::graphite::paint_params::color4f_prep_for_dst;
use skia_rust_gpu::graphite::paint_params_key::{
    PaintParamsKey, PaintParamsKeyBuilder, RootBlockType,
};
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use support::{MockCaps, make_recorder};

fn srgb_info() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    )
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

    fn context(&self, info: &ColorInfo) -> KeyContext<'_> {
        KeyContext::new(
            self.caps.clone(),
            &self.builder,
            &self.gatherer,
            &self.dict,
            Arc::new(RuntimeEffectDictionary::new()),
            info,
        )
    }
}

#[test]
fn the_precompile_context_has_defaults() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    assert!(context.recorder().is_none());
    assert_eq!(context.flags(), KeyGenFlags::DEFAULT);
    assert!(context.local_matrix().is_none());
    assert_eq!(*context.local2dev(), M44::default());
    assert_eq!(*context.clip_draw_bounds(), Rect::default());
    // Opaque black.
    let black = context.paint_color();
    assert_eq!((black.r, black.g, black.b, black.a), (0.0, 0.0, 0.0, 1.0));
    assert_eq!(context.dst_color_info().color_type(), ColorType::RGBA8888);
    assert!(std::ptr::eq(
        context.paint_params_key_builder(),
        std::ptr::from_ref(&fixture.builder)
    ));
    assert!(std::ptr::eq(
        context.pipeline_data_gatherer(),
        std::ptr::from_ref(&fixture.gatherer)
    ));
    assert_eq!(
        context.caps().storage_buffer_support(),
        fixture.caps.storage_buffer_support()
    );
    assert!(context.rt_effect_dict().find(1000).is_none());
}

#[test]
fn the_builder_is_shared_between_copies() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let copy = context.with_extra_flags(KeyGenFlags::PREFER_FIXED_SRC_BLEND);
    copy.paint_params_key_builder()
        .borrow_mut()
        .add_root_block_header(RootBlockType::SrcColor);
    context
        .paint_params_key_builder()
        .borrow_mut()
        .add_block(BuiltInCodeSnippetID::SolidColorShader);
    let data = fixture
        .builder
        .borrow_mut()
        .lock_as_key()
        .key()
        .data()
        .to_vec();
    assert_eq!(
        data,
        vec![-1, BuiltInCodeSnippetID::SolidColorShader as i32]
    );
}

#[test]
fn extra_flags_accumulate_in_a_copy() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let a = context.with_extra_flags(KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION);
    let b = a.with_extra_flags(KeyGenFlags::PREFER_FIXED_SRC_BLEND);
    assert_eq!(context.flags(), KeyGenFlags::DEFAULT);
    assert_eq!(a.flags(), KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION);
    assert_eq!(
        b.flags(),
        KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION | KeyGenFlags::PREFER_FIXED_SRC_BLEND
    );
    assert_eq!(KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION.bits(), 0x1);
    assert_eq!(KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM.bits(), 0x2);
    assert_eq!(
        KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION.bits(),
        0x4
    );
    assert_eq!(KeyGenFlags::PREFER_FIXED_SRC_BLEND.bits(), 0x8);
}

#[test]
fn runtime_effect_children_get_flags_from_their_sample_usage() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    // `a` is sampled with the passed-in coordinates, `b` with coordinates the effect computes.
    let effect = RuntimeEffect::make_for_shader(
        "uniform shader a; uniform shader b;\n\
         half4 main(float2 p) { return a.eval(p) + b.eval(p * 2); }",
        None,
    )
    .expect("compiles");
    let a = context.for_runtime_effect(&effect, 0);
    assert_eq!(
        a.flags(),
        KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION
    );
    let b = context.for_runtime_effect(&effect, 1);
    assert_eq!(
        b.flags(),
        KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION
            | KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM
            | KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION
    );

    assert_eq!(
        context.for_mesh_spec_child().flags(),
        KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION
            | KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION
    );
    // The flags the context already had stay.
    let flagged = context.with_extra_flags(KeyGenFlags::PREFER_FIXED_SRC_BLEND);
    assert!(
        flagged
            .for_mesh_spec_child()
            .flags()
            .contains(KeyGenFlags::PREFER_FIXED_SRC_BLEND)
    );
}

#[test]
fn local_matrices_concatenate_with_the_child_matrix_on_the_left() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = fixture.context(&info);
    let outer = Matrix::translate((10.0, 0.0));
    let inner = Matrix::scale((2.0, 2.0));

    let with_outer = context.with_local_matrix(&outer);
    assert_eq!(with_outer.local_matrix(), Some(&outer));
    // The original is untouched.
    assert!(context.local_matrix().is_none());

    // childLM * existing (`SkMatrix::Concat(childLM, *fLocalMatrix)`): a point goes through the
    // existing matrix first, then through the child's.
    let with_both = with_outer.with_local_matrix(&inner);
    let both = with_both.local_matrix().unwrap();
    let p = both.map_xy(1.0, 1.0);
    assert_eq!((p.x, p.y), (22.0, 2.0));
    assert_eq!(*both, Matrix::concat(&inner, &outer));
}

#[test]
fn changing_the_color_info_converts_the_paint_color_but_not_its_alpha() {
    let (recorder, _shared_context) = make_recorder(MockCaps::default());
    let fixture = Fixture::new();
    let info = srgb_info();
    let color = Color4f::new(0.5, 0.25, 1.0, 0.5);
    let context = KeyContext::new_with_recorder(
        &recorder,
        TextureFormat::RGBA8,
        &fixture.builder,
        &fixture.gatherer,
        &M44::default(),
        &Rect::default(),
        &info,
        KeyGenFlags::DEFAULT,
        &color,
    );
    // sRGB to sRGB leaves the color, which is opaque and premultiplied (by 1), and the alpha.
    let p = context.paint_color();
    assert_eq!((p.r, p.g, p.b, p.a), (0.5, 0.25, 1.0, 0.5));

    // Into a linear color space the RGB changes and the alpha stays.
    let linear = ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb_linear()),
    );
    let converted = context.with_color_info(&linear);
    let q = converted.paint_color();
    assert!(q.r < p.r && q.g < p.g && q.b <= p.b);
    assert_eq!(q.a, 0.5);
    assert!(q.b > 0.99);
    assert!(
        converted
            .dst_color_info()
            .color_space_ref()
            .is_some_and(ColorSpace::gamma_is_linear)
    );
    // The original is unchanged.
    assert_eq!(context.paint_color().r, 0.5);
}

#[test]
fn the_recorder_context_takes_its_pieces_from_the_recorder() {
    let (recorder, shared_context) = make_recorder(MockCaps {
        storage_buffer_support: true,
        ..MockCaps::default()
    });
    let fixture = Fixture::new();
    let info = srgb_info();
    let context = KeyContext::new_with_recorder(
        &recorder,
        TextureFormat::BGRA8,
        &fixture.builder,
        &fixture.gatherer,
        &M44::default(),
        &Rect::new(0.0, 0.0, 16.0, 8.0),
        &info,
        KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM,
        &Color4f::new(0.0, 0.0, 0.0, 1.0),
    );
    assert!(context.recorder().is_some());
    assert_eq!(context.target_format(), TextureFormat::BGRA8);
    assert_eq!(context.flags(), KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM);
    assert_eq!(*context.clip_draw_bounds(), Rect::new(0.0, 0.0, 16.0, 8.0));
    assert!(context.caps().storage_buffer_support());

    // The dictionary is the shared context's: a key interned through the context is the one
    // the shared context's dictionary knows.
    let mut builder = fixture.builder.borrow_mut();
    builder.add_root_block_header(RootBlockType::SrcColor);
    builder.add_block(BuiltInCodeSnippetID::SolidColorShader);
    builder.add_root_block_header(RootBlockType::FinalBlend);
    builder.add_block(BuiltInCodeSnippetID::FixedBlendSrcOver);
    let id = context.dict().find_or_create_for_builder(&mut builder);
    assert_eq!(id.as_uint(), 1);
    assert_eq!(
        shared_context.shader_dictionary.lookup(id).len(),
        4,
        "the key is in the shared context's dictionary"
    );
    assert_eq!(
        &*recorder.priv_().shader_code_dictionary().lookup(id),
        &*shared_context.shader_dictionary.lookup(id)
    );
    assert!(PaintParamsKey::new(&shared_context.shader_dictionary.lookup(id)).is_valid());

    // The runtime effect dictionary is the recorder's.
    assert!(Arc::ptr_eq(
        &context.rt_effect_dict(),
        &recorder.priv_().runtime_effect_dictionary()
    ));
}

#[test]
#[should_panic(expected = "only a context with a recorder has a target")]
fn the_precompile_context_has_no_target() {
    let fixture = Fixture::new();
    let info = srgb_info();
    let _ = fixture.context(&info).target_format();
}

#[test]
fn prep_for_dst_converts_srgb_to_the_destination() {
    let srgb = srgb_info();
    let color = Color4f::new(0.2, 0.4, 0.6, 0.8);
    // sRGB to sRGB is the identity, and the result is not premultiplied.
    let same = color4f_prep_for_dst(color, &srgb);
    assert_eq!((same.r, same.g, same.b, same.a), (0.2, 0.4, 0.6, 0.8));

    let linear = ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb_linear()),
    );
    let converted = color4f_prep_for_dst(color, &linear);
    assert!(converted.r < 0.2 && converted.g < 0.4 && converted.b < 0.6);
    assert_eq!(converted.a, 0.8);
}

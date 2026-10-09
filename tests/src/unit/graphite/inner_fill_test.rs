// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/InnerFillTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::{Device as CoreDevice, clip_shader};
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::m44::M44;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::shaders;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped, Protected, Renderable};
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::draw_context::DrawContext;
use skia_rust_gpu::graphite::draw_types::DstUsage;
use skia_rust_gpu::graphite::geom::non_msaa_clip::{AnalyticClip, NonMSAAClip};
use skia_rust_gpu::graphite::key_context::{KeyContext, KeyGenFlags};
use skia_rust_gpu::graphite::paint_params::{PaintParams, ShadingParams};
use skia_rust_gpu::graphite::render_step::Coverage;
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// Port of: tests/graphite/InnerFillTest.cpp#L26-L123 (chrome/m156)
def_graphite_test_for_all_contexts!(InnerFillTest, |reporter, context| {
    if context.caps().avoid_depth_mode() {
        // "Skipping InnerFillTest under avoidDepthMode"
        return;
    }
    let recorder = context.make_recorder(None);
    let info = ImageInfo::new((512, 512), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut device = Device::make_with_info(
        Some(&recorder),
        &info,
        Budgeted::Yes,
        Mipmapped::No,
        skia_rust_gpu::gpu::backing_fit::BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "inner-fill",
        true,
        false,
    )
    .expect("the test device is created");

    // Default rrect size is sufficient to avoid the small-size exclusion criteria
    let mut test_rrect = |device: &mut Device,
                          recorder: &skia_rust_gpu::graphite::recorder::Recorder,
                          paint: &Paint,
                          inner_fill_expected: bool,
                          rrect_size: f32| {
        let rrect = RRect::new_rect_xy(
            Rect::from_xywh(0.0, 0.0, rrect_size, rrect_size),
            0.1 * rrect_size,
            0.1 * rrect_size,
        );
        let mut initial_render_steps = device.testing_only_pending_render_steps();
        let initial_tasks = recorder.priv_().num_root_tasks();
        CoreDevice::draw_rrect(device, &rrect, paint);
        let actual_render_steps = device.testing_only_pending_render_steps();
        if recorder.priv_().num_root_tasks() != initial_tasks {
            // Flushed for a dst read copy before appending the new draw and possibly inner fill.
            initial_render_steps = 0;
        }
        // TODO(michaelludwig): After migrating to the layer draw tracking system, having a way to
        // traverse the recorded draws and inspect them could let these checks be more robust, e.g.
        // one of the steps is actually the CoverBoundsRenderStep AND it's in the front.
        let expected_render_steps = initial_render_steps + if inner_fill_expected { 2 } else { 1 };
        reporter_assert!(
            reporter,
            actual_render_steps == expected_render_steps,
            "Expected ({expected_render_steps}) vs Actual ({actual_render_steps})"
        );
    };

    // ** Test cases that should not produce an inner fill:
    // 1. A transparent paint, so the draw is not eligible for an inner fill
    {
        // "transparent paint"
        let mut transparent_paint = Paint::default();
        transparent_paint.set_alpha_f(0.5);
        test_rrect(&mut device, &recorder, &transparent_paint, false, 128.0);
    }
    // 2. A very small rounded rectangle that would otherwise have an inner fill
    {
        // "small rrect"
        test_rrect(&mut device, &recorder, &Paint::default(), false, 8.0);
    }
    // 3. An opaque paint that has a blend mode that still mixes with the dst
    {
        // "fancy blend"
        let mut opaque_blended_paint = Paint::default();
        opaque_blended_paint.set_blend_mode(BlendMode::SrcIn);
        test_rrect(&mut device, &recorder, &opaque_blended_paint, false, 128.0);
    }

    // ** Test cases that shouldn't produce an inner fill due to analytic clipping
    CoreDevice::push_clip_stack(&mut device);
    let clip =
        shaders::color_in_space(Color4f::new(0.1, 0.2, 0.3, 0.5), None).expect("a color shader");
    clip_shader(&mut device, &clip, ClipOp::Intersect);
    // 4. An opaque paint but with an analytic clip
    {
        // "opaque src-over analytic clip"
        test_rrect(&mut device, &recorder, &Paint::default(), false, 128.0);
    }
    // 5. A kSrc paint but there is analytic clip
    {
        // "src w/ analytic clip"
        let mut src_paint = Paint::default();
        src_paint.set_blend_mode(BlendMode::Src);
        test_rrect(&mut device, &recorder, &src_paint, false, 128.0);
    }
    CoreDevice::pop_clip_stack(&mut device);

    // ** Test cases that should produce an inner fill:
    // 1. A kSrcOver paint with opaque color (or shader)
    {
        // "opaque src-over"
        test_rrect(&mut device, &recorder, &Paint::default(), true, 128.0);
    }
    // 2. A kSrc paint when the device supports HW blending regardless of coverage
    {
        // "src"
        let mut src_paint = Paint::default();
        src_paint.set_blend_mode(BlendMode::Src);
        src_paint.set_alpha_f(0.5); // emphasize it's src color is not opaque
        test_rrect(&mut device, &recorder, &src_paint, true, 128.0);
    }

    // We don't actually care about rendering, so just throw everything away
});

// Port of: tests/graphite/InnerFillTest.cpp#L125-L291 (chrome/m156)
def_graphite_test_for_all_contexts!(OptimizeForOpacity, |reporter, context| {
    const K_SEMI_TRANSPARENT: Color4f = Color4f {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.5,
    };
    const K_OPAQUE: Color4f = Color4f {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let caps = context.caps();
    let recorder = context.make_recorder(None);
    let target_info = ColorInfo::new(ColorType::RGBA8888, AlphaType::Premul, None);
    let resource_provider = recorder.priv_().resource_provider().clone();
    let target = {
        let mut provider = resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        TextureProxy::make(
            caps,
            &mut provider,
            ISize::new(16, 16),
            &caps.get_default_sampled_texture_info(
                target_info.color_type(),
                Mipmapped::No,
                Protected::No,
                Renderable::Yes,
            ),
            Budgeted::Yes,
            "OptimizeForOpacityTarget",
        )
    };
    let draw_context = DrawContext::make(
        caps,
        target,
        ISize::new(16, 16),
        &target_info,
        &SurfaceProps::default(),
        /*allow_unpremul=*/ false,
    )
    .expect("the target can be rendered into");

    let gen_paint_id = |reporter: &mut crate::Reporter,
                        paint: &PaintParams,
                        renderer_coverage: Coverage,
                        expected_dst_usage: DstUsage,
                        clip: Option<&NonMSAAClip>|
     -> (UniquePaintParamsID, Option<UniquePaintParamsID>) {
        let shading = ShadingParams::new(
            caps,
            paint,
            clip,
            None,
            renderer_coverage,
            TextureFormat::RGBA8,
        );
        let key_and_data = recorder.priv_().pop_or_create_key_and_data_builder();
        let (paint_id, opaque_id) = {
            let key_context = KeyContext::new_with_draw_context(
                &recorder,
                &draw_context,
                &key_and_data.builder,
                &key_and_data.gatherer,
                &M44::default(),
                &Rect::from_xywh(0.0, 0.0, 16.0, 16.0),
                &target_info,
                KeyGenFlags::PREFER_FIXED_SRC_BLEND,
                paint.color(),
            );
            let (paint_id, dst_usage) = shading
                .to_key(&key_context)
                .expect("the paint converts to a key");
            reporter_assert!(
                reporter,
                dst_usage.contains(expected_dst_usage),
                "dstUsage {dst_usage:?} lacks {expected_dst_usage:?}"
            );
            let mut opaque_id = None;
            if dst_usage == DstUsage::NONE
                || dst_usage.contains(DstUsage::DST_ONLY_USED_BY_RENDERER)
            {
                opaque_id = Some(shading.optimize_for_opacity(&key_context, paint_id));
            }
            (paint_id, opaque_id)
        };
        key_and_data.gatherer.borrow_mut().reset_for_draw();
        key_and_data.builder.borrow_mut().reset_for_draw();
        recorder.priv_().push_key_and_data_builder(key_and_data);
        (paint_id, opaque_id)
    };

    let (non_aa_src_id, opaque_non_aa_src_id) = gen_paint_id(
        reporter,
        &PaintParams::from_color(K_SEMI_TRANSPARENT, BlendMode::Src),
        Coverage::None,
        DstUsage::NONE,
        None,
    );
    reporter_assert!(
        reporter,
        opaque_non_aa_src_id.is_some(),
        "Non-AA + kSrc not detected as opaque"
    );
    reporter_assert!(
        reporter,
        Some(non_aa_src_id) == opaque_non_aa_src_id,
        "optimizeForOpacity() should be a no-op for non-AA+kSrc"
    );
    {
        let (non_aa_src_over_id, opaque_non_aa_src_over_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_OPAQUE, BlendMode::SrcOver),
            Coverage::None,
            DstUsage::NONE,
            None,
        );
        reporter_assert!(
            reporter,
            opaque_non_aa_src_over_id.is_some(),
            "Non-AA + kSrcOver not detected as opaque"
        );
        reporter_assert!(
            reporter,
            Some(non_aa_src_over_id) == opaque_non_aa_src_over_id,
            "optimizeForOpacity() should be a no-op for non-AA+kSrcOver"
        );
        reporter_assert!(
            reporter,
            non_aa_src_over_id == non_aa_src_id,
            "Opaque paint ID should match between non-AA+kSrcOver and non-AA+kSrc"
        );
    }
    {
        let (aa_src_id, opaque_aa_src_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_OPAQUE, BlendMode::Src),
            Coverage::SingleChannel,
            DstUsage::DEPENDS_ON_DST | DstUsage::DST_ONLY_USED_BY_RENDERER,
            None,
        );
        reporter_assert!(
            reporter,
            opaque_aa_src_id.is_some(),
            "AA + kSrc should have opaque variant"
        );
        reporter_assert!(
            reporter,
            Some(aa_src_id) == opaque_aa_src_id,
            "optimizeForOpacity() should be a no-op for AA+kSrc"
        );
        reporter_assert!(
            reporter,
            opaque_aa_src_id == Some(non_aa_src_id),
            "Opaque paint ID should match between AA+kSrc and non-AA+kSrc"
        );
    }
    {
        let (aa_src_over_id, opaque_aa_src_over_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_OPAQUE, BlendMode::SrcOver),
            Coverage::SingleChannel,
            DstUsage::DEPENDS_ON_DST | DstUsage::DST_ONLY_USED_BY_RENDERER,
            None,
        );
        reporter_assert!(
            reporter,
            opaque_aa_src_over_id.is_some(),
            "AA + kSrcOver should have opaque variant"
        );
        reporter_assert!(
            reporter,
            Some(aa_src_over_id) != opaque_aa_src_over_id,
            "optimizeForOpacity() should be different for AA+kSrcOver"
        );
        reporter_assert!(
            reporter,
            opaque_aa_src_over_id == Some(non_aa_src_id),
            "Opaque paint ID should match between AA+kSrcOver and non-AA+kSrc"
        );
    }
    {
        let (_, opaque_aa_src_over_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_SEMI_TRANSPARENT, BlendMode::SrcOver),
            Coverage::SingleChannel,
            DstUsage::DEPENDS_ON_DST,
            None,
        );
        reporter_assert!(
            reporter,
            opaque_aa_src_over_id.is_none(),
            "AA + kSrcOver should not have an opaque variant"
        );
    }
    let clip = NonMSAAClip {
        analytic_clip: AnalyticClip {
            bounds: Rect::from_ltrb(1.0, 1.0, 15.0, 15.0),
            ..AnalyticClip::default()
        },
        ..NonMSAAClip::default()
    };
    {
        let (_, clipped_opaque_non_aa_src_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_SEMI_TRANSPARENT, BlendMode::Src),
            Coverage::None,
            DstUsage::DEPENDS_ON_DST,
            Some(&clip),
        );
        reporter_assert!(
            reporter,
            clipped_opaque_non_aa_src_id.is_none(),
            "Non-AA + kSrc + AnalyticClip should not have an opaque variant"
        );
    }
    {
        let (_, clipped_opaque_non_aa_src_over_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_OPAQUE, BlendMode::SrcOver),
            Coverage::None,
            DstUsage::DEPENDS_ON_DST,
            Some(&clip),
        );
        reporter_assert!(
            reporter,
            clipped_opaque_non_aa_src_over_id.is_none(),
            "Non-AA + kSrcOver + AnalyticClip should not have an opaque variant"
        );
    }
    {
        let (_, clipped_opaque_aa_src_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_OPAQUE, BlendMode::Src),
            Coverage::SingleChannel,
            DstUsage::DEPENDS_ON_DST,
            Some(&clip),
        );
        reporter_assert!(
            reporter,
            clipped_opaque_aa_src_id.is_none(),
            "AA + kSrc + AnalyticClip should not have an opaque variant"
        );
    }
    {
        let (_, clipped_opaque_aa_src_over_id) = gen_paint_id(
            reporter,
            &PaintParams::from_color(K_OPAQUE, BlendMode::SrcOver),
            Coverage::SingleChannel,
            DstUsage::DEPENDS_ON_DST,
            Some(&clip),
        );
        reporter_assert!(
            reporter,
            clipped_opaque_aa_src_over_id.is_none(),
            "AA + kSrcOver + AnalyticClip should not have an opaque variant"
        );
    }
});

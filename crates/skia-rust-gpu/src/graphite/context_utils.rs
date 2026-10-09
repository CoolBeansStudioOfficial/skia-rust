// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ContextUtils.{h,cpp} (chrome/m156)

//! Helpers shared by the shader generators and the pipeline code: the intrinsic uniforms, the
//! hardware-blending decision, sampler layouts and pipeline labels.
//!
//! `BuildComputeSkSL` is not ported: it needs `ComputeStep` (the compute wave, G13).

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::rect::IRect;

use crate::gpu::blend_formula::{get_blend_formula, get_lcd_blend_formula};
use crate::graphite::caps::{Caps, ResourceBindingRequirements};
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::{Coverage, RenderStep};
use crate::graphite::resource_types::DstReadStrategy;
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::texture_format::{TextureFormat, texture_format_auto_clamps};
use crate::graphite::uniform::Uniform;
use crate::graphite::uniform_manager::UniformManager;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;
use crate::sksl_type_shared::SkSLType;

/// Intrinsic uniforms used by every program created in Graphite.
///
/// `viewport` should hold the actual viewport set as backend state (defining the NDC -> pixel
/// transform). The viewport's dimensions are used to define the device -> NDC transform applied in
/// the vertex shader, but this assumes that the (0,0) device coordinate maps to the corner of the
/// top-left of the NDC cube. The viewport's origin is used in the fragment shader to reconstruct
/// the logical fragment coordinate from the target's current frag coord (which are not relative to
/// active viewport).
///
/// It is assumed that `dstReadBounds` is in the same coordinate space as the `viewport` (e.g.
/// final backing target's pixel coords) and that its width and height match the dimensions of the
/// texture to be sampled for dst reads.
// Port of: src/gpu/graphite/ContextUtils.h#L56-L57 (chrome/m156)
#[doc(alias = "kIntrinsicUniforms")]
pub const INTRINSIC_UNIFORMS: [Uniform; 2] = [
    Uniform::new("viewport", SkSLType::Float4),
    Uniform::new("dstReadBounds", SkSLType::Float4),
];

/// `CollectIntrinsicUniforms(caps, viewport, dstReadBounds, uniforms)`.
// Port of: src/gpu/graphite/ContextUtils.cpp#L86-L128 (chrome/m156)
#[doc(alias = "CollectIntrinsicUniforms")]
#[allow(clippy::cast_precision_loss)] // mirrors the C++ int -> float conversions
pub fn collect_intrinsic_uniforms(
    caps: &dyn Caps,
    viewport: IRect,
    dst_read_bounds: IRect,
    uniforms: &mut UniformManager,
) {
    #[cfg(debug_assertions)]
    uniforms.set_expected_uniforms(&INTRINSIC_UNIFORMS, /* is_substruct= */ false);

    // viewport
    {
        // The vertex shader needs to divide by the dimension and then multiply by 2, so do this
        // once on the CPU. This is because viewport normalization wants to range from -1 to 1, and
        // not 0 to 1. If any other user of the viewport uniform requires the true reciprocal or
        // original dimensions, this can be adjusted.
        debug_assert!(!viewport.is_empty());
        let inv_two_w = 2.0_f32 / viewport.width() as f32;
        let mut inv_two_h = 2.0_f32 / viewport.height() as f32;

        // If the NDC Y axis points up (opposite normal skia convention and the underlying view
        // convention), upload the inverse height as a negative value. See ShaderInfo::Make
        // for how this is used.
        if !caps.ndc_y_axis_points_down() {
            inv_two_h *= -1.0_f32;
        }
        uniforms.write_vec([
            viewport.left as f32,
            viewport.top as f32,
            inv_two_w,
            inv_two_h,
        ]);
    }

    // dstReadBounds
    {
        // Unlike viewport, dstReadBounds can be empty so check for 0 dimensions and set the
        // reciprocal to 0. It is also not doubled since its purpose is to normalize texture coords
        // to 0 to 1, and not -1 to 1.
        let width = dst_read_bounds.width();
        let height = dst_read_bounds.height();
        uniforms.write_vec([
            dst_read_bounds.left as f32,
            dst_read_bounds.top as f32,
            if width != 0 { 1.0 / width as f32 } else { 0.0 },
            if height != 0 {
                1.0 / height as f32
            } else {
                0.0
            },
        ]);
    }

    #[cfg(debug_assertions)]
    uniforms.done_with_expected_uniforms();
}

/// Returns whether or not hardware blending can be used. If not, we must perform a dst read within
/// the shader. If a blender is used instead of a blend mode, it can never use hardware blending.
// Port of: src/gpu/graphite/ContextUtils.cpp#L44-L84 (chrome/m156)
#[doc(alias = "CanUseHardwareBlending")]
#[must_use]
pub fn can_use_hardware_blending(
    caps: &dyn Caps,
    target_format: TextureFormat,
    bm: BlendMode,
    coverage: Coverage,
) -> bool {
    // Check for special cases that would prevent the usage of direct hardware blending and
    // require us to fall back to using shader-based blending.
    let has_coverage = coverage != Coverage::None;
    let dst_is_fast = caps.get_dst_read_strategy() != DstReadStrategy::TextureCopy;
    if
    // Using LCD coverage (which must be applied after the blend equation) with any blend mode
    // besides SkBlendMode::kSrcOver
    // TODO(b/414597217): Add support to use dual-source blending with LCD coverage.
    (coverage == Coverage::Lcd && bm != BlendMode::SrcOver)

        // SkBlendMode::kPlus clamps its output to [0,1], e.g. clamp(D+S,0,1), which is then
        // combined with coverage (f) for a final written value of:
        //   (1-f)*D + f*clamp(D+S,0,1)
        //
        // This can be rewritten to min(D+f*S, D+f*(1-D)), which is not representable with *any*
        // hardware blend configuration. However, when the target format clamps to [0,1], we can
        // approximate the output as min(D+f*S, 1) with a slight degradation in AA quality.
        //
        // If access to D doesn't require a texture copy, prefer shader blending for the quality.
        || (bm == BlendMode::Plus && (dst_is_fast || !texture_format_auto_clamps(target_format)))

        // Using an advanced blend mode but the hardware does not support them
        || (bm > BlendMode::LAST_COEFF_MODE && !caps.supports_hardware_advanced_blending())

        // The blend formula requires dual-source blending, but it is not supported by hardware
        || (bm <= BlendMode::LAST_COEFF_MODE
            && (if coverage == Coverage::Lcd {
                get_lcd_blend_formula(bm).has_secondary_output()
            } else {
                get_blend_formula(false, has_coverage, bm).has_secondary_output()
            })
            && !caps.shader_caps().dual_source_blending_support)
    {
        return false;
    }

    // In all other cases (which are more commonly encountered; e.g. using a simple blend mode),
    // we can use direct HW blending.
    true
}

/// `EmitSamplerLayout(bindingReqs, &binding)`: the `layout(...)` of the next texture and sampler,
/// advancing `binding`.
// Port of: src/gpu/graphite/ContextUtils.cpp#L130-L148 (chrome/m156)
#[doc(alias = "EmitSamplerLayout")]
pub fn emit_sampler_layout(
    binding_reqs: &ResourceBindingRequirements,
    binding: &mut i32,
) -> String {
    if binding_reqs.separate_texture_and_sampler_binding {
        let sampler_index = *binding;
        *binding += 1;
        let texture_index = *binding;
        *binding += 1;
        format!(
            "layout(webgpu, set={}, sampler={}, texture={})",
            binding_reqs.texture_sampler_set_idx, sampler_index, texture_index
        )
    } else {
        let sampler_index = *binding;
        *binding += 1;
        format!(
            "layout(set={}, binding={})",
            binding_reqs.texture_sampler_set_idx, sampler_index
        )
    }
}

/// `GetPipelineLabel(caps, dict, renderPassDesc, renderStep, paintID)`.
// Port of: src/gpu/graphite/ContextUtils.cpp#L150-L163 (chrome/m156)
#[doc(alias = "GetPipelineLabel")]
#[must_use]
pub fn get_pipeline_label(
    caps: &dyn Caps,
    dict: &ShaderCodeDictionary,
    render_pass_desc: &RenderPassDesc,
    render_step: &dyn RenderStep,
    paint_id: UniquePaintParamsID,
) -> String {
    // KEEP IN SYNC with ShaderInfo::pipelineLabel()
    let mut label = render_pass_desc.to_pipeline_label(); // includes the write swizzle
    label.push_str(" + ");
    label.push_str(render_step.name());
    label.push_str(" + ");
    // the shader portion will be "(empty)" for depth-only draws
    label.push_str(&dict.id_to_string(caps, paint_id));
    label
}

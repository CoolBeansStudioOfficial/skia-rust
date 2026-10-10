// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/CoverageMaskRenderStep.h, CoverageMaskRenderStep.cpp

//! [`CoverageMaskRenderStep`]: draws a quad that samples a coverage mask texture (a path atlas
//! entry, or a mask image). Each draw is one instance of a four-vertex triangle strip.

use std::sync::Arc;

use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::SCALAR_NEARLY_ZERO;
use skia_rust_core::tile_mode::TileMode;

use crate::graphite::attribute::{Attribute, Interpolation, Varying};
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::context_utils::emit_sampler_layout;
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_types::{PrimitiveType, VertexAttribType};
use crate::graphite::draw_writer::{DrawWriter, Instances};
use crate::graphite::paint_params_key::RootNodesInfo;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render::common_depth_stencil_settings::DIRECT_DEPTH_LESS_PASS;
use crate::graphite::render_step::{RenderStep, RenderStepBase, RenderStepFlags, RenderStepID};
use crate::graphite::caps::ResourceBindingRequirements;
use crate::graphite::resource_types::{Layout, SamplerDesc};
use crate::graphite::uniform::Uniform;
use crate::sksl_type_shared::SkSLType;

/// `get_device_translation(localToDevice)`: the translation of `localToDevice` that is applied
/// before the remainder matrix, so that it can be combined with the mask's atlas origin.
// Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L23-L38 (chrome/m156)
fn get_device_translation(local_to_device: &M44) -> [f32; 2] {
    let m00 = local_to_device.rc(0, 0);
    let m01 = local_to_device.rc(0, 1);
    let m10 = local_to_device.rc(1, 0);
    let m11 = local_to_device.rc(1, 1);
    let det = m00 * m11 - m01 * m10;
    if det.abs() <= SCALAR_NEARLY_ZERO {
        // We can't extract any pre-translation, since the upper 2x2 is not invertible.
        return [0.0, 0.0];
    }
    // Calculate inv([[m00,m01][m10,m11]])*[[m30][m31]] to get the pre-remainder device translation.
    let tx = local_to_device.rc(0, 3);
    let ty = local_to_device.rc(1, 3);
    // skvx::float4{m11, -m10, -m01, m00} * skvx::float4{tx,tx,ty,ty}, then (xy + zw) / det.
    let inv_t = [m11 * tx, (-m10) * tx, (-m01) * ty, m00 * ty];
    [(inv_t[0] + inv_t[2]) / det, (inv_t[1] + inv_t[3]) / det]
}

const STATIC_ATTRS: [Attribute; 0] = [];

const APPEND_ATTRS: [Attribute; 8] = [
    Attribute::new("drawBounds", VertexAttribType::Float4, SkSLType::Float4),
    // ltrb of the mask bounds in the atlas, normalized (`UShort4_norm`).
    Attribute::new("maskBoundsIn", VertexAttribType::UShort4Norm, SkSLType::Float4),
    // Remaining translation extracted from the actual `maskToDevice` transform.
    Attribute::new("deviceOrigin", VertexAttribType::Float2, SkSLType::Float2),
    Attribute::new("depth", VertexAttribType::Float, SkSLType::Float),
    Attribute::new("ssboIndex", VertexAttribType::UInt, SkSLType::UInt),
    // localToDevice matrix for producing local coords for shader evaluation.
    Attribute::new("mat0", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat1", VertexAttribType::Float3, SkSLType::Float3),
    Attribute::new("mat2", VertexAttribType::Float3, SkSLType::Float3),
];

const UNIFORMS: [Uniform; 1] = [Uniform::new("maskToDeviceRemainder", SkSLType::Float3x3)];

/// `CoverageMaskRenderStep`: samples a coverage mask texture over the shape's bounds.
// Port of: src/gpu/graphite/render/CoverageMaskRenderStep.h#L16-L41 (chrome/m156)
#[doc(alias = "skgpu::graphite::CoverageMaskRenderStep")]
#[derive(Debug)]
pub struct CoverageMaskRenderStep {
    base: RenderStepBase,
}

impl CoverageMaskRenderStep {
    /// `CoverageMaskRenderStep(layout)`.
    // Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L53-L94 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        let base = RenderStepBase::new(
            layout,
            RenderStepID::CoverageMask,
            // The mask will have AA outsets baked in, but the original bounds for clipping still
            // require the outset for analytic coverage.
            RenderStepFlags::PERFORMS_SHADING
                | RenderStepFlags::HAS_TEXTURES
                | RenderStepFlags::EMITS_COVERAGE
                | RenderStepFlags::OUTSET_BOUNDS_FOR_AA
                | RenderStepFlags::INVERSE_FILLS_SCISSOR
                | RenderStepFlags::APPEND_INSTANCES,
            &UNIFORMS,
            PrimitiveType::TriangleStrip,
            DIRECT_DEPTH_LESS_PASS,
            &STATIC_ATTRS,
            &APPEND_ATTRS,
            &[],
            &[
                // `maskBounds` are the atlas-relative, sorted bounds of the coverage mask.
                // `textureCoords` are the atlas-relative UV coordinates of the draw, which can
                // spill beyond `maskBounds` for inverse fills.
                Varying::new("maskBounds", SkSLType::Float4, Interpolation::Perspective),
                Varying::new("textureCoords", SkSLType::Float2, Interpolation::Perspective),
                // 'invert' is set to 0 use unmodified coverage, and set to 1 for "1-c".
                Varying::new("invert", SkSLType::Half, Interpolation::Perspective),
            ],
        );
        Self { base }
    }
}

impl RenderStep for CoverageMaskRenderStep {
    fn base(&self) -> &RenderStepBase {
        &self.base
    }

    // Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L96-L107 (chrome/m156)
    fn vertex_sksl(&self, _roots: &RootNodesInfo) -> String {
        // Returns the body of a vertex function, which must define a float4 devPosition variable
        // and must write to an already-defined float2 stepLocalCoords variable.
        "float4 devPosition = coverage_mask_vertex_fn(\
            float2(sk_VertexID >> 1, sk_VertexID & 1), \
            maskToDeviceRemainder, drawBounds, maskBoundsIn, deviceOrigin, \
            depth, float3x3(mat0, mat1, mat2), \
            maskBounds, textureCoords, invert, stepLocalCoords);\n"
            .to_owned()
    }

    // Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L109-L113 (chrome/m156)
    fn textures_and_samplers_sksl(
        &self,
        binding_reqs: &ResourceBindingRequirements,
        next_binding_index: &mut i32,
    ) -> String {
        format!(
            "{} sampler2D pathAtlas;",
            emit_sampler_layout(binding_reqs, next_binding_index)
        )
    }

    // Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L115-L119 (chrome/m156)
    fn fragment_coverage_sksl(&self) -> &'static str {
        "half c = sample(pathAtlas, clamp(textureCoords, maskBounds.LT, maskBounds.RB)).r;\n\
         outputCoverage = half4(mix(c, 1 - c, invert));\n"
    }

    // Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L121 (chrome/m156)
    fn uses_uniforms_in_fragment_sksl(&self) -> bool {
        false
    }

    // Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L123-L201 (chrome/m156)
    fn write_vertices(&self, writer: &mut DrawWriter<'_>, params: &DrawParams, ssbo_index: u32) {
        let coverage_mask = params.geometry().coverage_mask_shape();
        let proxy_dims = coverage_mask.texture_proxy().dimensions();

        // A quad is a 4-vertex instance. The coordinates are derived from the vertex IDs.
        let mut instances = Instances::new(
            writer,
            BindBufferInfo::default(),
            BindBufferInfo::default(),
            4,
        );

        // The device origin is the translation extracted from the mask-to-device matrix so that
        // the remaining matrix uniform has less variance between draws.
        let mask_to_device = coverage_mask.mask_to_device();
        let mut device_origin = get_device_translation(mask_to_device);

        // Relative to mask space (device origin and mask-to-device remainder must be applied in
        // the shader).
        let mask_rect = coverage_mask.bounds();
        let mut mask_bounds = ltrb(&mask_rect);
        let mut draw_bounds: [f32; 4];
        if coverage_mask.inverted() {
            // Only mask filters trigger complex transforms, and they are never inverse filled.
            // Since we know this is an inverted mask, then we can exactly map the draw's clip
            // bounds to mask space so that the clip is still fully covered without branching in
            // the vertex shader.
            debug_assert!(
                *mask_to_device == M44::translate(device_origin[0], device_origin[1], 0.0)
            );
            draw_bounds = ltrb(&offset_rect(&params.draw_bounds(), device_origin));

            // If the mask is fully clipped out, then the shape's mask info should be (0,0,0,0).
            // If it's not fully clipped out, then the mask info should be non-empty.
            let empty_mask = mask_bounds.iter().all(|v| *v == 0.0);
            if empty_mask {
                // The inversion check is strict inequality, so (0,0,0,0) would not be detected.
                // Adjust to (0,0,1/2,1/2) to restrict sampling to the top-left quarter of the
                // top-left pixel, which should have a value of 0 regardless of filtering mode.
                mask_bounds = [0.0, 0.0, 0.5, 0.5];
            } else {
                // Add 1/2px outset to the mask bounds so that clamped coordinates sample the
                // texel center of the padding around the atlas entry.
                mask_bounds = add4(mask_bounds, [-0.5, -0.5, 0.5, 0.5]);
            }
            // and store RBLT so that the 'maskBoundsIn' attribute has xy > zw to detect inverse
            // fill.
            mask_bounds = [mask_bounds[2], mask_bounds[3], mask_bounds[0], mask_bounds[1]];
        } else {
            // If we aren't inverted, then the originally assigned values don't need to be
            // adjusted.
            debug_assert!(!mask_rect.is_empty_negative_or_nan());
            // Since the mask bounds and draw bounds are 1-to-1 with each other, the clamping of
            // texture coords is mostly a formality. We inset the mask bounds by 1/2px so that we
            // clamp to the texel center of the outer row/column of the mask.
            draw_bounds = mask_bounds;
            mask_bounds = add4(mask_bounds, [0.5, 0.5, -0.5, -0.5]);
        }

        // Move 'drawBounds' and 'maskBounds' into the atlas coordinate space, then adjust the
        // device translation to undo the atlas origin automatically in the vertex shader.
        let (ox, oy) = coverage_mask.texture_origin();
        let texture_origin = [f32::from(ox), f32::from(oy)];
        let origin4 = [
            texture_origin[0],
            texture_origin[1],
            texture_origin[0],
            texture_origin[1],
        ];
        mask_bounds = add4(mask_bounds, origin4);
        draw_bounds = add4(draw_bounds, origin4);
        device_origin[0] -= texture_origin[0];
        device_origin[1] -= texture_origin[1];

        // Normalize drawBounds and maskBounds after possibly correcting drawBounds for inverse
        // fills. The maskToDevice matrix uniform will handle de-normalizing drawBounds for vertex
        // positions.
        let atlas_size_inv = [
            1.0 / dims_f32(proxy_dims.width),
            1.0 / dims_f32(proxy_dims.height),
        ];
        let inv4 = [
            atlas_size_inv[0],
            atlas_size_inv[1],
            atlas_size_inv[0],
            atlas_size_inv[1],
        ];
        draw_bounds = mul4(draw_bounds, inv4);
        mask_bounds = mul4(mask_bounds, inv4);
        device_origin[0] *= atlas_size_inv[0];
        device_origin[1] *= atlas_size_inv[1];

        // Since the mask bounds define normalized texels of the texture, we can encode them as
        // ushort_norm without losing precision to save space.
        let mask_bounds_u16: [u16; 4] = mask_bounds.map(|v| (65535.0 * v + 0.5) as u16);

        let m = params.transform().matrix(); // local-to-device
        let mut vw = instances.append(1);
        vw.put(&draw_bounds)
            .put(&mask_bounds_u16)
            .put(&device_origin)
            .put(&params.order().depth_as_float())
            .put(&ssbo_index)
            .put(&[m.rc(0, 0), m.rc(1, 0), m.rc(3, 0)])
            .put(&[m.rc(0, 1), m.rc(1, 1), m.rc(3, 1)])
            .put(&[m.rc(0, 3), m.rc(1, 3), m.rc(3, 3)]);
    }

    // Port of: src/gpu/graphite/render/CoverageMaskRenderStep.cpp#L203-L249 (chrome/m156)
    fn write_uniforms_and_textures(
        &self,
        params: &DrawParams,
        gatherer: &mut PipelineDataGatherer,
    ) {
        #[cfg(debug_assertions)]
        gatherer.check_rewind();
        let coverage_mask = params.geometry().coverage_mask_shape();
        let proxy = Arc::clone(coverage_mask.texture_proxy_arc());
        let dims = proxy.dimensions();

        // Most coverage masks are aligned with the device pixels, so the params' transform is an
        // integer translation matrix. This translation is extracted as an instance attribute so
        // that the remaining transform has a much lower frequency of changing.
        let mask_to_device = coverage_mask.mask_to_device();
        let device_origin = get_device_translation(mask_to_device);
        let mut remainder: Matrix = mask_to_device.to_m33();
        remainder.pre_translate((-device_origin[0], -device_origin[1]));

        // Check pixel alignment before we fold in coord normalization scaling.
        let pixel_aligned = remainder.is_identity()
            && device_origin
                .iter()
                .all(|&o| o == (o + SCALAR_NEARLY_ZERO).floor());

        // The mask coordinates in the vertex shader will be normalized, so scale by the proxy size
        // to get back to Skia's texel-based coords.
        remainder.pre_scale((dims_f32(dims.width), dims_f32(dims.height)), None);

        // Write uniforms.
        gatherer.uniform_manager().write_matrix(&remainder);

        // Write textures and samplers.
        let filter = if pixel_aligned {
            FilterMode::Nearest
        } else {
            FilterMode::Linear
        };
        gatherer.add(
            Some(proxy),
            SamplerDesc::new(&SamplingOptions::from(filter), TileMode::Clamp),
        );
    }
}

/// `[l, t, r, b]` of a rect (`ltrb()`).
fn ltrb(rect: &crate::graphite::geom::rect::Rect) -> [f32; 4] {
    let v = rect.ltrb();
    [v[0], v[1], v[2], v[3]]
}

/// `makeOffset(-offset)`: the rect moved by `-offset`, as its `ltrb`.
fn offset_rect(
    rect: &crate::graphite::geom::rect::Rect,
    offset: [f32; 2],
) -> crate::graphite::geom::rect::Rect {
    let [l, t, r, b] = ltrb(rect);
    crate::graphite::geom::rect::Rect::new(
        l - offset[0],
        t - offset[1],
        r - offset[0],
        b - offset[1],
    )
}

fn add4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn mul4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]
}

/// `float(int)` for an atlas dimension.
#[allow(clippy::cast_precision_loss)] // texture dimensions are far below 2^24
fn dims_f32(v: i32) -> f32 {
    v as f32
}

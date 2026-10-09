// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/PaintParams.{h,cpp} (chrome/m156)

//! [`PaintParams`] and [`ShadingParams`]: the shading state of an `SkPaint`, and its conversion to a
//! `PaintParamsKey` (with the uniforms and textures it gathers) for a given draw.
//!
//! A `PaintParams` is short-lived. It is built from a paint, optionally with a primitive blender
//! (drawVertices, drawAtlas), a color override, or an image override (drawImageRect), and then
//! [`ShadingParams::to_key`] walks its effects into the key builder of the [`KeyContext`]. The
//! root blocks are, in order: the source color (paint color, shader, color filter and dither), the
//! final blend, the optional clip and the optional mesh shader.
//!
//! Deviations from the C++:
//!
//! - Skia holds `const SkBlender*`, `const SkShader*` and the other paint objects by raw pointer.
//!   Here a `PaintParams` owns clones of them (`Shader`, `Blender` and `ColorFilter` are cheap
//!   reference-counted handles), so no lifetime has to be threaded through the params.
//! - `SimpleImage` owns its image and local matrix, for the same reason.
//! - [`ShadingParams`] takes `Option<&NonMSAAClip>` where Skia takes a `const NonMSAAClip&`
//!   (`None` is the empty clip). The clip is keyed by `add_analytic_clip`; the atlas half of the
//!   clip is only ever non-empty once the clip atlas (G12a) exists.
//! - `AddToKey(const SimpleImage&)` needs `add_image_to_key`, the image-shader block builder that
//!   `key_helpers::add_to_key_shader` does not dispatch yet; a `SimpleImage` is keyed as an error
//!   block until then (see [`add_simple_image_to_key`]).
//! - `PaintParams::notifyImagesInUse` is not in the pinned Skia (m156): there is no such method to
//!   port, so the paint-to-key path has no image-usage notification.
//! - The `SkASSERT` that compares the builder with `lookup(origPaint)` in `optimizeForOpacity` is
//!   not reproduced, since `PaintParamsKeyBuilder` has no `==`.

#[cfg(debug_assertions)]
use std::cell::RefCell;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::mesh::{Mesh, MeshSpecification, mesh_priv};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::ChildPtr;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;

use crate::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;
use crate::graphite::caps::Caps;
use crate::graphite::context_utils::can_use_hardware_blending;
use crate::graphite::draw_types::DstUsage;
use crate::graphite::geom::non_msaa_clip::NonMSAAClip;
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::key_helpers::{
    AlphaOnlyPaintColorBlock, RGBPaintColorBlock, SolidColorShaderBlock, add_dither_block,
    add_to_key_shader,
};
use crate::graphite::key_helpers_ii::{
    MeshShaderBlock, add_analytic_clip, add_blend_mode, add_fixed_blend_mode, add_primitive_color,
    add_to_key_blender, add_to_key_color_filter, blend, compose,
};
#[cfg(debug_assertions)]
use crate::graphite::paint_params_key::PaintParamsKeyBuilder;
use crate::graphite::paint_params_key::RootBlockType;
#[cfg(debug_assertions)]
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::render_step::Coverage;
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

// Port of: src/gpu/graphite/PaintParams.cpp#L215-L223 (chrome/m156), `Color4fPrepForDst`

/// Converts an `SkColor4f` from sRGB to the destination color space, leaving it unpremultiplied
/// (`PaintParams::Color4fPrepForDst`).
#[doc(alias = "Color4fPrepForDst")]
#[must_use]
pub fn color4f_prep_for_dst(src_color: Color4f, dst_color_info: &ColorInfo) -> Color4f {
    // xform from sRGB to the destination colorspace
    let steps = ColorSpaceXformSteps::new(
        Some(srgb_singleton()),
        AlphaType::Unpremul,
        dst_color_info.color_space_ref(),
        AlphaType::Unpremul,
    );

    let mut result = src_color;
    let mut vec = result.as_array();
    steps.apply(&mut vec);
    result = Color4f::new(vec[0], vec[1], vec[2], vec[3]);
    result
}

/// The `SkColors::kTransparent` color.
const TRANSPARENT: Color4f = Color4f {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

/// `PaintParams::SimpleImage`: the parameters of the implicit image shader that `drawImageRect`
/// and the other image-drawing APIs use, without creating an `SkShader`. Assumes clamp tiling.
// Port of: src/gpu/graphite/PaintParams.h#L35-L49 (chrome/m156)
#[derive(Clone, Debug)]
pub struct SimpleImage {
    /// `fImage`: the image (required).
    pub image: Image,
    /// `fLocalMatrix`: optional.
    pub local_matrix: Option<Matrix>,
    /// `fSubset`: the post local-matrix strict clamping rectangle, relative to the image's texels.
    pub subset: Rect,
    /// `fSamplingOptions`.
    pub sampling_options: SamplingOptions,
}

/// The final blend of a paint: a blender (kept as a shader-blended `kSrc`), or a blend mode.
/// `PaintParams::fFinalBlend`, a `std::pair`.
type FinalBlend = (Option<Blender>, BlendMode);

/// Converts an `SkPaint`'s shading state into the inputs of a draw's key (`PaintParams`).
// Port of: src/gpu/graphite/PaintParams.h#L53-L170 (chrome/m156)
#[doc(alias = "skgpu::graphite::PaintParams")]
#[derive(Clone, Debug)]
pub struct PaintParams {
    color: Color4f,

    // Either a non-null blender for runtime blending, or the blend mode to use instead. If the
    // blender is non-null, the blend mode is set to kSrc to match the HW blend config used for
    // shader-based blending.
    final_blend: FinalBlend,

    shader: Option<Shader>,
    // Overrides `shader` for color images, mixes for alpha
    image_shader: Option<SimpleImage>,
    color_filter: Option<ColorFilter>,

    // A `None` primitive blender means there's no primitive color blending and it is skipped.
    primitive_blender: Option<Blender>,
    // When a primitive color override is present, it is used instead of any defined primitive
    // colors in the vertices for primitive color blending.
    primitive_color_override: Option<Color4f>,
    primitive_color_space: Option<ColorSpace>,
    primitive_alpha_type: AlphaType,

    mesh_spec: Option<Arc<MeshSpecification>>,
    mesh_children: Vec<ChildPtr>,

    skip_color_xform: bool,
    dither: bool,
}

impl PaintParams {
    /// A params with the given color and final blend, and no other effects.
    // Port of: src/gpu/graphite/PaintParams.cpp#L196-L205 (chrome/m156), with the defaults of
    // `PaintParams`'s member initializers
    fn with_color_and_blend(color: Color4f, final_blend: FinalBlend) -> Self {
        Self {
            color,
            final_blend,
            shader: None,
            image_shader: None,
            color_filter: None,
            primitive_blender: None,
            primitive_color_override: None,
            primitive_color_space: None,
            primitive_alpha_type: AlphaType::Premul,
            mesh_spec: None,
            mesh_children: Vec::new(),
            skip_color_xform: false,
            dither: false,
        }
    }

    /// Converts `paint` to params, possibly with a primitive blender (for example for drawVertices
    /// or text rendering). `ignore_shader` drops the paint's shader.
    // Port of: src/gpu/graphite/PaintParams.cpp#L115-L161 (chrome/m156), the private constructor
    // with an image override, and the public one that delegates to it
    fn from_paint_inner(
        paint: &Paint,
        image_override: Option<SimpleImage>,
        primitive_blender: Option<&Blender>,
        skip_color_xform: bool,
        ignore_shader: bool,
    ) -> Self {
        let final_blend = get_final_blend(paint.blender().as_ref());
        let mut params = Self {
            color: paint.color4f(),
            final_blend,
            shader: if ignore_shader { None } else { paint.shader() },
            image_shader: image_override,
            color_filter: paint.color_filter(),
            primitive_blender: primitive_blender.cloned(),
            skip_color_xform,
            dither: paint.is_dither(),
            ..Self::with_color_and_blend(TRANSPARENT, (None, BlendMode::SrcOver))
        };

        if params.final_blend.1 == BlendMode::Clear {
            // None of the other effects are relevant, the final src color for blending is
            // transparent and we can consolidate its blend mode to kSrc. This is helpful to
            // restrict the number of pipelines that will actually have DstUsage::kNone.
            params.final_blend.1 = BlendMode::Src;
            params.color = TRANSPARENT;
            params.shader = None;
            params.image_shader = None;
            params.color_filter = None;
            params.primitive_blender = None;
            params.dither = false;
        } else if params.primitive_blender.is_none() {
            // NOTE: We can still have an alpha-only image_shader and still want to try simplifying
            // the paint's shader to a solid color for the alpha image's colorization.
            if let Some(constant_color) = params
                .shader
                .as_ref()
                .and_then(|shader| shader.as_base().is_constant())
            {
                // The original color and `constant_color` are un-premul sRGB, but we need to
                // preserve the paint's alpha.
                let orig_a = params.color.a;
                params.color = constant_color;
                params.color.a *= orig_a;
                params.shader = None;
            }
            // We can't apply the color filter to the color if a shader modifies it, including when
            // the image shader is alpha-only. The image's per-pixel alpha modulates the paint color
            // *before* the color filter is evaluated.
            let filter_applies = params.shader.is_none() && params.image_shader.is_none();
            if let Some(color_filter) = params.color_filter.clone().filter(|_| filter_applies) {
                params.color = color_filter.filter_color4f(
                    params.color,
                    Some(srgb_singleton()),
                    Some(srgb_singleton()),
                );
                params.color_filter = None;
            }
        }

        params
    }

    /// Converts `paint` to params, possibly with a primitive blender (drawVertices, text).
    // Port of: src/gpu/graphite/PaintParams.cpp#L163-L172 (chrome/m156), public constructor
    #[must_use]
    pub fn new(
        paint: &Paint,
        primitive_blender: Option<&Blender>,
        skip_color_xform: bool,
        ignore_shader: bool,
    ) -> Self {
        Self::from_paint_inner(
            paint,
            None,
            primitive_blender,
            skip_color_xform,
            ignore_shader,
        )
    }

    /// Converts `paint` to params and accounts for the implicit image shader override of
    /// `drawImageRect` and related functions. Multiplies `xtra_alpha` with the paint's alpha.
    // Port of: src/gpu/graphite/PaintParams.cpp#L174-L184 (chrome/m156)
    #[must_use]
    pub fn from_paint_with_image(
        paint: &Paint,
        image_override: SimpleImage,
        xtra_alpha: f32,
    ) -> Self {
        // For color images, the paint's original shader is ignored.
        let ignore_shader = !skia_rust_core::image_info_priv::color_type_is_alpha_only(
            image_override.image.color_type(),
        );
        let mut params =
            Self::from_paint_inner(paint, Some(image_override), None, false, ignore_shader);
        // Multiply in the extra alpha that's allowed to be set on an ImageSetEntry. Accepting it
        // here avoids needing to modify the SkPaint providing the base color.
        params.color.a *= xtra_alpha;
        params
    }

    /// Converts `paint` to params, overriding its base color with `color_override` and
    /// multiplying the base and override alphas. Any shader in the paint is ignored.
    // Port of: src/gpu/graphite/PaintParams.cpp#L186-L194 (chrome/m156)
    #[must_use]
    pub fn from_paint_with_color(paint: &Paint, color_override: Color4f) -> Self {
        let mut params = Self::from_paint_inner(paint, None, None, false, true);
        let new_alpha = params.color.a * color_override.a;
        params.color = color_override.with_alpha(new_alpha);
        params
    }

    /// A constant color with the given blend mode.
    // Port of: src/gpu/graphite/PaintParams.cpp#L196-L205 (chrome/m156)
    #[must_use]
    pub fn from_color(color: Color4f, final_blend_mode: BlendMode) -> Self {
        let is_clear = final_blend_mode == BlendMode::Clear;
        Self::with_color_and_blend(
            if is_clear { TRANSPARENT } else { color },
            (
                None,
                if is_clear {
                    BlendMode::Src
                } else {
                    final_blend_mode
                },
            ),
        )
    }

    /// A copy of these params with a primitive blender and the primitive color `color_override`.
    // Port of: src/gpu/graphite/PaintParams.cpp#L207-L213 (chrome/m156), `makeWithPrimitiveColor`
    #[doc(alias = "makeWithPrimitiveColor")]
    #[must_use]
    pub fn make_with_primitive_color(
        &self,
        primitive_blender: Option<&Blender>,
        primitive_color_override: Color4f,
    ) -> Self {
        let mut copy = self.clone();
        copy.primitive_blender = primitive_blender.cloned();
        copy.primitive_color_override = Some(primitive_color_override);
        copy
    }

    /// A copy of these params that draws with `mesh`'s specification, children and color space.
    // Port of: src/gpu/graphite/PaintParams.cpp#L225-L233 (chrome/m156), `makeWithMesh`
    #[doc(alias = "makeWithMesh")]
    #[must_use]
    pub fn make_with_mesh(&self, mesh: &Mesh) -> Self {
        let mut copy = self.clone();
        if let Some(spec) = mesh.spec() {
            copy.mesh_spec = Some(spec.clone());
            copy.primitive_color_space = spec.color_space().cloned();
            copy.primitive_alpha_type = mesh_priv::alpha_type(spec);
        }
        copy.mesh_children = mesh.children().to_vec();
        copy
    }

    /// The opaque, src-blended version of `paint`, used for a dst-independent pipeline.
    // Port of: src/gpu/graphite/PaintParams.cpp#L235-L241 (chrome/m156), `MakeOpaque`
    #[cfg(debug_assertions)]
    #[must_use]
    pub fn make_opaque(paint: &PaintParams) -> Self {
        let mut opaque = paint.clone();
        opaque.final_blend = (None, BlendMode::Src);
        opaque.color = opaque.color.to_opaque();
        opaque
    }

    /// The paint's base color, unpremultiplied sRGB.
    #[must_use]
    pub fn color(&self) -> &Color4f {
        &self.color
    }

    /// The paint's shader.
    #[must_use]
    pub fn shader(&self) -> Option<&Shader> {
        self.shader.as_ref()
    }

    /// The implicit image shader of an image override.
    #[must_use]
    pub fn image_shader(&self) -> Option<&SimpleImage> {
        self.image_shader.as_ref()
    }

    /// The paint's color filter.
    #[must_use]
    pub fn color_filter(&self) -> Option<&ColorFilter> {
        self.color_filter.as_ref()
    }

    /// The primitive blender, if any.
    #[must_use]
    pub fn primitive_blender(&self) -> Option<&Blender> {
        self.primitive_blender.as_ref()
    }

    /// The primitive color override, if any.
    #[must_use]
    pub fn primitive_color_override(&self) -> Option<&Color4f> {
        self.primitive_color_override.as_ref()
    }

    /// Whether the primitive color's transform to the dst color space is skipped.
    #[must_use]
    pub fn skip_primitive_color_xform(&self) -> bool {
        self.skip_color_xform
    }

    /// The color space of the primitive color, if not sRGB.
    #[must_use]
    pub fn primitive_color_space(&self) -> Option<&ColorSpace> {
        self.primitive_color_space.as_ref()
    }

    /// The alpha type of the primitive color.
    #[must_use]
    pub fn primitive_alpha_type(&self) -> AlphaType {
        self.primitive_alpha_type
    }

    /// The mesh specification, if drawing a mesh.
    #[must_use]
    pub fn mesh_spec(&self) -> Option<&Arc<MeshSpecification>> {
        self.mesh_spec.as_ref()
    }

    /// The mesh's children.
    #[must_use]
    pub fn mesh_children(&self) -> &[ChildPtr] {
        &self.mesh_children
    }

    /// The final blender, which overrides `final_blend_mode` when present.
    #[must_use]
    pub fn final_blender(&self) -> Option<&Blender> {
        self.final_blend.0.as_ref()
    }

    /// The final blend mode. Only meaningful without a [`PaintParams::final_blender`].
    // Port of: src/gpu/graphite/PaintParams.h#L113 (chrome/m156)
    #[doc(alias = "finalBlendMode")]
    #[must_use]
    pub fn final_blend_mode(&self) -> BlendMode {
        debug_assert!(self.final_blend.0.is_none());
        self.final_blend.1
    }

    /// Whether the color is dithered.
    #[must_use]
    pub fn dither(&self) -> bool {
        self.dither
    }
}

/// The final blend for a blender: a blender that is a blend mode becomes that mode, otherwise the
/// blender itself is kept and the mode becomes `kSrc`.
// Port of: src/gpu/graphite/PaintParams.cpp#L52-L66 (chrome/m156), `get_final_blend`
fn get_final_blend(blender: Option<&Blender>) -> FinalBlend {
    let Some(blender) = blender else {
        return (None, BlendMode::SrcOver);
    };

    match blender.as_base().as_blend_mode() {
        Some(mode) => (None, mode),
        None => (Some(blender.clone()), BlendMode::Src),
    }
}

/// Whether `should_dither` applies to `p` for a dst of color type `dst_ct`.
// Port of: src/gpu/graphite/PaintParams.cpp#L33-L50 (chrome/m156), `should_dither`
fn should_dither(p: &PaintParams, dst_ct: ColorType) -> bool {
    // The paint dither flag can veto.
    if !p.dither() {
        return false;
    }

    if dst_ct == ColorType::Unknown {
        return false;
    }

    // We always dither 565 or 4444 when requested.
    if dst_ct == ColorType::RGB565 || dst_ct == ColorType::ARGB4444 {
        return true;
    }

    // Otherwise, dither is only needed for non-const paints.
    p.image_shader().is_some()
        || p.shader()
            .is_some_and(|shader| shader.as_base().is_constant().is_none())
}

/// `DstUsage` of `paint` for a renderer with `renderer_coverage`, before opacity analysis. For
/// src-over this assumes the paint is opaque; [`ShadingParams::to_key`] corrects it.
// Port of: src/gpu/graphite/PaintParams.cpp#L68-L113 (chrome/m156), `get_dst_usage`
fn get_dst_usage(
    caps: &dyn Caps,
    target_format: TextureFormat,
    paint: &PaintParams,
    renderer_coverage: Coverage,
    clip_shader: Option<&Shader>,
    non_msaa_clip: Option<&NonMSAAClip>,
) -> DstUsage {
    let mut dst_usage = DstUsage::DEPENDS_ON_DST;
    if paint.final_blender().is_some() {
        dst_usage |= DstUsage::DST_READ_REQUIRED;
    } else {
        let has_analytic_clip =
            clip_shader.is_some() || non_msaa_clip.is_some_and(|clip| !clip.is_empty());
        let mut effective_coverage = renderer_coverage;
        if effective_coverage == Coverage::None && has_analytic_clip {
            effective_coverage = Coverage::SingleChannel;
        }

        let final_blend_mode = paint.final_blend_mode();
        if !can_use_hardware_blending(caps, target_format, final_blend_mode, effective_coverage) {
            dst_usage |= DstUsage::DST_READ_REQUIRED;
        }
        if final_blend_mode > BlendMode::LAST_COEFF_MODE {
            dst_usage |= DstUsage::ADVANCED_BLEND;
        }

        if !has_analytic_clip
            && (final_blend_mode == BlendMode::Src || final_blend_mode == BlendMode::SrcOver)
        {
            if renderer_coverage == Coverage::None {
                // For kSrc, we definitely now do not depend on the dst so we can remove that flag
                // entirely. Optimistically we also remove it for kSrcOver under the assumption that
                // the paint is opaque; if to_key() finds that is not the case, it must restore the
                // flag.
                debug_assert_eq!(dst_usage, DstUsage::DEPENDS_ON_DST);
                dst_usage = DstUsage::NONE;
            } else {
                // For kSrc, kDstOnlyUsedByRenderer is the correct final usage. For kSrcOver, we
                // optimistically add it on the assumption the rest of the paint will be opaque.
                // to_key() must remove this flag if it's not opaque.
                dst_usage |= DstUsage::DST_ONLY_USED_BY_RENDERER;
            }
        }
    }
    dst_usage
}

/// The per-pixel state of a draw that is not part of its paint, and that decides how the paint's
/// key is built: the analytic and clip shader, the renderer's coverage, and the target format.
/// The key is produced by [`ShadingParams::to_key`].
// Port of: src/gpu/graphite/PaintParams.h#L172-L224 (chrome/m156)
#[doc(alias = "skgpu::graphite::ShadingParams")]
#[derive(Debug)]
pub struct ShadingParams<'a> {
    paint: &'a PaintParams,
    non_msaa_clip: Option<&'a NonMSAAClip>,
    clip_shader: Option<&'a Shader>,

    // Base (incomplete) dst usage that will be augmented by opacity analysis calculated in
    // to_key(). This is only relevant for kSrcOver, fDstUsage is set assuming the paint is opaque;
    // if it's not actually opaque it will be adjusted accordingly.
    dst_usage: DstUsage,

    coverage: Coverage,
    // The target's format (`keyContext.targetFormat()` in the C++, read here at construction so
    // that a context without a target can still key the paint).
    target_format: TextureFormat,
}

impl<'a> ShadingParams<'a> {
    /// The shading state of `paint` with the clip and coverage of a draw into `target_format`.
    /// Does not copy `paint`, `non_msaa_clip` or `clip_shader`: they must outlive the params.
    // Port of: src/gpu/graphite/PaintParams.cpp#L245-L258 (chrome/m156)
    #[must_use]
    pub fn new(
        caps: &dyn Caps,
        paint: &'a PaintParams,
        non_msaa_clip: Option<&'a NonMSAAClip>,
        clip_shader: Option<&'a Shader>,
        coverage: Coverage,
        target_format: TextureFormat,
    ) -> Self {
        let dst_usage = get_dst_usage(
            caps,
            target_format,
            paint,
            coverage,
            clip_shader,
            non_msaa_clip,
        );
        Self {
            paint,
            non_msaa_clip,
            clip_shader,
            dst_usage,
            coverage,
            target_format,
        }
    }

    /// Whether the final color depends on the dst, from the usage before opacity analysis.
    // Port of: src/gpu/graphite/PaintParams.h#L186 (chrome/m156), `dstReadRequired`
    #[doc(alias = "dstReadRequired")]
    #[must_use]
    pub fn dst_read_required(&self) -> bool {
        self.dst_usage.contains(DstUsage::DST_READ_REQUIRED)
    }

    /// Whether the non-MSAA clip is non-empty (the `!isEmpty()` of `NonMSAAClip`).
    fn has_analytic_clip(&self) -> bool {
        self.non_msaa_clip.is_some_and(|clip| !clip.is_empty())
    }

    /// Adds the paint's color (its shader, image or color) to the key. Returns whether the result
    /// is opaque.
    // Port of: src/gpu/graphite/PaintParams.cpp#L260-L301 (chrome/m156), `addPaintColorToKey`
    fn add_paint_color_to_key(&self, key_context: &KeyContext<'_>) -> bool {
        if let Some(simple_image) = self.paint.image_shader() {
            // There is an implicit image shader, match handling of SkModifyPaintForDrawImageRect
            if let Some(shader) = self.paint.shader() {
                // Alpha-only images for drawImageRect() get colorized with the paint's shader. This
                // differs from alpha-only image shaders that might be encountered within an
                // SkShader graph, which get colorized by the paint's opaque color.
                blend(
                    key_context,
                    || add_fixed_blend_mode(key_context, BlendMode::DstIn),
                    || {
                        // Since colorization is handled here, disable paint color-colorization
                        // later.
                        let image_context = key_context
                            .with_extra_flags(KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION);
                        add_simple_image_to_key(&image_context, simple_image);
                    },
                    || add_to_key_shader(key_context, Some(shader)),
                );
                false // Colorizing with an alpha-only texture probably isn't opaque
            } else {
                // Encode the image structure directly, which includes handling alpha-only images
                // that combine with the paint's color (RGB1) stored on `key_context`.
                add_simple_image_to_key(key_context, simple_image);
                simple_image.image.is_opaque()
            }
        } else if let Some(shader) = self.paint.shader() {
            add_to_key_shader(key_context, Some(shader));
            shader.is_opaque()
        } else {
            RGBPaintColorBlock::add_block(key_context);
            true // rgb1, always opaque
        }
    }

    /// Primitive blend blocks are used to blend either the paint color or the output of another
    /// shader with a primitive color emitted by certain draw geometry calls (drawVertices,
    /// drawAtlas, etc.). Dst: primitiveColor Src: Paint color/shader output. Returns whether the
    /// result is opaque.
    // Port of: src/gpu/graphite/PaintParams.cpp#L303-L357 (chrome/m156), `handlePrimitiveColor`
    fn handle_primitive_color(&self, key_context: &KeyContext<'_>) -> bool {
        // If no primitive blending is required, simply add the paint color.
        let Some(primitive_blender) = self.paint.primitive_blender() else {
            return self.add_paint_color_to_key(key_context);
        };

        // If no color space conversion is required and the primitive blend mode is kDst, the src
        // branch of the blend does not matter and we can simply emit the primitive color.
        let prim_blend = primitive_blender.as_base().as_blend_mode();
        let can_skip_blend_step =
            self.paint.skip_primitive_color_xform() && prim_blend == Some(BlendMode::Dst);

        let prim_color_override: Option<PMColor4f> = self
            .paint
            .primitive_color_override()
            .map(|color| color4f_prep_for_dst(*color, key_context.dst_color_info()).premul());

        let add_primitive_color_or_override = || {
            if let Some(color) = &prim_color_override {
                SolidColorShaderBlock::add_block(key_context, color);
            } else {
                add_primitive_color(
                    key_context,
                    self.paint.skip_primitive_color_xform(),
                    self.paint.primitive_color_space(),
                    self.paint.primitive_alpha_type(),
                );
            }
        };

        if can_skip_blend_step {
            add_primitive_color_or_override();
            return false;
        }

        let mut src_is_opaque = false;
        blend(
            key_context,
            /* addBlendToKey= */ || add_to_key_blender(key_context, Some(primitive_blender)),
            /* addSrcToKey= */ || src_is_opaque = self.add_paint_color_to_key(key_context),
            /* addDstToKey= */ add_primitive_color_or_override,
        );
        if let (Some(prim_blend), true) = (prim_blend, src_is_opaque) {
            // If the input paint/shader is opaque, the result is only opaque if the primitive blend
            // mode is kSrc or kSrcOver. All other modes can introduce transparency.
            return prim_blend == BlendMode::Src || prim_blend == BlendMode::SrcOver;
        }

        // If the input was already transparent, or if it's a runtime/complex blend mode, the
        // result cannot be considered opaque.
        false
    }

    /// Applies the paint's alpha. Returns whether the result is opaque.
    // Port of: src/gpu/graphite/PaintParams.cpp#L359-L385 (chrome/m156), `handlePaintAlpha`
    fn handle_paint_alpha(&self, key_context: &KeyContext<'_>) -> bool {
        if self.paint.shader().is_none()
            && self.paint.image_shader().is_none()
            && self.paint.primitive_blender().is_none()
        {
            // If there is no shader and no primitive blending the input to the colorFilter stage
            // is just the premultiplied paint color.
            let paint_color =
                color4f_prep_for_dst(*self.paint.color(), key_context.dst_color_info()).premul();
            SolidColorShaderBlock::add_block(key_context, &paint_color);
            return self.paint.color().is_opaque();
        }

        if self.paint.color().is_opaque() {
            self.handle_primitive_color(key_context)
        } else {
            blend(
                key_context,
                /* addBlendToKey= */ || add_fixed_blend_mode(key_context, BlendMode::SrcIn),
                /* addSrcToKey= */
                || {
                    self.handle_primitive_color(key_context);
                },
                /* addDstToKey= */ || AlphaOnlyPaintColorBlock::add_block(key_context),
            );
            // The result is guaranteed to be non-opaque because we're blending with fColor's alpha.
            false
        }
    }

    /// Applies the color filter, if any. Returns whether the result is opaque.
    // Port of: src/gpu/graphite/PaintParams.cpp#L387-L401 (chrome/m156), `handleColorFilter`
    fn handle_color_filter(&self, key_context: &KeyContext<'_>) -> bool {
        if let Some(color_filter) = self.paint.color_filter() {
            let mut src_is_opaque = false;
            compose(
                key_context,
                /* addInnerToKey= */ || src_is_opaque = self.handle_paint_alpha(key_context),
                /* addOuterToKey= */
                || add_to_key_color_filter(key_context, Some(color_filter)),
            );
            src_is_opaque && color_filter.is_alpha_unchanged()
        } else {
            self.handle_paint_alpha(key_context)
        }
    }

    /// Dithers the color if the paint and dst call for it. Returns whether the result is opaque.
    // Port of: src/gpu/graphite/PaintParams.cpp#L403-L422 (chrome/m156), `handleDithering`
    fn handle_dithering(&self, key_context: &KeyContext<'_>) -> bool {
        let ct = key_context.dst_color_info().color_type();
        if should_dither(self.paint, ct) {
            let mut src_is_opaque = false;
            compose(
                key_context,
                /* addInnerToKey= */
                || src_is_opaque = self.handle_color_filter(key_context),
                /* addOuterToKey= */ || add_dither_block(key_context, ct),
            );
            src_is_opaque
        } else {
            self.handle_color_filter(key_context)
        }
    }

    /// The clipping root node: the analytic clip and the clip shader, composed when both are
    /// present.
    // Port of: src/gpu/graphite/PaintParams.cpp#L424-L501 (chrome/m156), `handleClipping`
    fn handle_clipping(&self, key_context: &KeyContext<'_>) {
        debug_assert!(self.has_analytic_clip() || self.clip_shader.is_some());
        if self.has_analytic_clip() {
            // For both an analytic clip and clip shader, we need to compose them together into a
            // single clipping root node. Without a clip shader, the analytic clip can be the
            // clipping root node.
            if let Some(clip_shader) = self.clip_shader {
                blend(
                    key_context,
                    /* addBlendToKey= */
                    || add_fixed_blend_mode(key_context, BlendMode::Modulate),
                    /* addSrcToKey= */
                    || {
                        add_analytic_clip(
                            key_context,
                            self.non_msaa_clip.expect("a non-empty clip"),
                        );
                    },
                    /* addDstToKey= */ || add_to_key_shader(key_context, Some(clip_shader)),
                );
            } else {
                add_analytic_clip(key_context, self.non_msaa_clip.expect("a non-empty clip"));
            }
        } else {
            // Since there's no analytic clip, the clipping root node can be the clip shader
            // directly.
            add_to_key_shader(key_context, self.clip_shader);
        }
    }

    /// Builds the paint's key into `key_context`: the source color root, the final blend root, and
    /// the optional clip and mesh shader roots. Returns the paint's id and its dst usage, or `None`
    /// if the key cannot be interned.
    // Port of: src/gpu/graphite/PaintParams.cpp#L503-L614 (chrome/m156), `toKey`
    #[doc(alias = "toKey")]
    #[must_use]
    // One function in Skia, kept whole so each root's block order reads against the C++.
    #[allow(clippy::too_many_lines)]
    pub fn to_key(&self, key_context: &KeyContext<'_>) -> Option<(UniquePaintParamsID, DstUsage)> {
        #[cfg(debug_assertions)]
        {
            key_context
                .paint_params_key_builder()
                .borrow()
                .check_reset();
            key_context.pipeline_data_gatherer().borrow().check_reset();
        }
        let format = self.target_format;
        let mut paint_depends_on_dst = true;

        // Root Node 0 is the source color, which is the output of all effects post dithering
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_root_block_header(RootBlockType::SrcColor);
        #[cfg_attr(not(debug_assertions), allow(unused_mut))] // only the debug asserts reassign it
        let mut is_opaque = self.handle_dithering(key_context);

        // Root Node 1 is the final blender
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_root_block_header(RootBlockType::FinalBlend);
        let mut dst_usage = self.dst_usage;
        if let Some(final_blender) = self.paint.final_blender() {
            add_to_key_blender(key_context, Some(final_blender));
        } else {
            // We converted kClear blends to kSrc; the PaintParams constructor already set every
            // other paint effect to match a transparent solid color.
            let mut final_blend_mode = self.paint.final_blend_mode();
            debug_assert_ne!(final_blend_mode, BlendMode::Clear);

            // If the KeyContext has opted into prioritizing Src (no blending) and we actually don't
            // need blending or only need blending due to the renderer (e.g. inner fill eligible),
            // then try to keep the final blend snippet as Src when it wouldn't impact the
            // rendering.
            let optimize_src_blend = (dst_usage == DstUsage::NONE
                || dst_usage.contains(DstUsage::DST_ONLY_USED_BY_RENDERER))
                && key_context
                    .flags()
                    .contains(KeyGenFlags::PREFER_FIXED_SRC_BLEND);

            // dst_usage was almost fully specified, except for kSrcOver, which was assumed to be
            // opaque and eligible for conversion to kSrc. If we're src over and not opaque, or not
            // eligible for reducing to kSrc, we have to adjust flags.
            if final_blend_mode == BlendMode::SrcOver {
                if is_opaque {
                    if dst_usage == DstUsage::NONE && optimize_src_blend {
                        // We can change the blend mode here without re-checking
                        // CanUseHardwareBlending() because DstUsage::kNone implies there's no
                        // analytic coverage and we're just changing from one Porter-Duff blend
                        // mode to another.
                        debug_assert!(can_use_hardware_blending(
                            key_context.caps(),
                            format,
                            BlendMode::Src,
                            self.coverage,
                        ));
                        final_blend_mode = BlendMode::Src;
                    } else {
                        // We don't have to remove kDstOnlyUsedByRenderer, but since we aren't
                        // optimizing to Src, add the optimistically avoided kDependsOnDst
                        dst_usage |= DstUsage::DEPENDS_ON_DST;
                    }
                } else {
                    // Definitely not eligible for conversion to kSrc, remove optimistically added
                    // flag
                    dst_usage.remove(DstUsage::DST_ONLY_USED_BY_RENDERER);
                    dst_usage |= DstUsage::DEPENDS_ON_DST;
                }
            }

            paint_depends_on_dst = final_blend_mode != BlendMode::Src;
            // Reset is_opaque to false if we aren't src-over to ensure later assert logic is
            // narrow.
            #[cfg(debug_assertions)] // only the debug-only asserts below read it
            {
                is_opaque &= final_blend_mode == BlendMode::SrcOver;
            }

            if !dst_usage.contains(DstUsage::DST_READ_REQUIRED)
                || (final_blend_mode == BlendMode::Src && optimize_src_blend)
            {
                // With no shader blending, be as explicit as possible about the final blend. We
                // also keep a fixed Src mode if it means a follow-up inner fill could be used.
                add_fixed_blend_mode(key_context, final_blend_mode);
            } else {
                // With shader blending, use AddBlendMode() to select the more universal blend
                // functions when possible. Technically we could always use a fixed blend mode but
                // would then over-generate when encountering certain classes of blends. This is
                // most problematic on devices that wouldn't support dual-source blending, so help
                // them out by at least not requiring lots of pipelines.
                add_blend_mode(key_context, final_blend_mode);
            }
        }

        // Optional Root Node 2 is the clip
        if self.clip_shader.is_some() || self.has_analytic_clip() {
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_root_block_header(RootBlockType::Clip);
            self.handle_clipping(key_context);
        }

        // Optional Root Node 3 is a mesh shader
        if let Some(mesh_spec) = self.paint.mesh_spec() {
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_root_block_header(RootBlockType::MeshShader);
            MeshShaderBlock::add_block(key_context, mesh_spec, self.paint.mesh_children());
        }

        // If dst_usage is not kNone, then kDependsOnDst must be set (all other bits only apply
        // *because* the shading depends on dst).
        debug_assert!(dst_usage == DstUsage::NONE || dst_usage.contains(DstUsage::DEPENDS_ON_DST));

        // If dst_usage is kNone, then there cannot be renderer coverage, analytic clipping, or the
        // paint depending on the dst color.
        debug_assert!(
            dst_usage != DstUsage::NONE
                || !(paint_depends_on_dst
                    || self.clip_shader.is_some()
                    || self.has_analytic_clip()
                    || self.coverage != Coverage::None)
        );

        // If kDstOnlyUsedByRenderer is set, the paint shouldn't depend on the dst and the dst
        // usage when the Renderer has Coverage::kNone should equal kNone.
        #[cfg(debug_assertions)]
        {
            let dst_usage_no_coverage = get_dst_usage(
                key_context.caps(),
                format,
                self.paint,
                Coverage::None,
                self.clip_shader,
                self.non_msaa_clip,
            );
            // This checks is_opaque in addition to !paint_depends_on_dst to handle the case where
            // src-over + opaque wasn't converted to src for *this* pipeline but remains
            // kDstOnlyUsedByRenderer for a possible inner fill.
            debug_assert!(
                !dst_usage.contains(DstUsage::DST_ONLY_USED_BY_RENDERER)
                    || ((is_opaque || !paint_depends_on_dst)
                        && dst_usage_no_coverage == DstUsage::NONE)
            );
        }

        let paint_id = key_context
            .dict()
            .find_or_create_for_builder(&mut key_context.paint_params_key_builder().borrow_mut());
        if paint_id.is_valid() {
            Some((paint_id, dst_usage))
        } else {
            None
        }
    }

    /// The id of the same paint as `orig_paint`, but combined with a renderer of
    /// `Coverage::None`: the last block becomes `kFixedBlend_Src`. Only valid after `to_key`
    /// returned [`DstUsage::DST_ONLY_USED_BY_RENDERER`] for `orig_paint`, with the key builder
    /// unchanged since.
    // Port of: src/gpu/graphite/PaintParams.cpp#L616-L650 (chrome/m156), `optimizeForOpacity`
    #[doc(alias = "optimizeForOpacity")]
    #[must_use]
    pub fn optimize_for_opacity(
        &self,
        key_context: &KeyContext<'_>,
        orig_paint: UniquePaintParamsID,
    ) -> UniquePaintParamsID {
        // We only support optimizing opacity when our dst usage is none and the final blend mode
        // can be switched to kSrc. This requires that there is no analytic clipping that adds a 3rd
        // render node, or an SkBlender that may have child blocks. With these conditions, we can
        // simply replace the last block ID with kSrc.
        //
        // Assuming the originally generated key returned kNone or kDstOnlyUsedByRenderer, these
        // requirements should be met. If these assumptions are violated, it'll be detected in
        // debug-only builds that regenerate the opaque coverage-less PaintParams from scratch.
        let opaque_id = {
            let mut builder = key_context.paint_params_key_builder().borrow_mut();
            let old_id = builder.replace_last_block(BuiltInCodeSnippetID::FixedBlendSrc);
            // We should only be calling into optimize_for_opacity for src and src-over blends
            debug_assert!(
                old_id == BuiltInCodeSnippetID::FixedBlendSrc
                    || old_id == BuiltInCodeSnippetID::FixedBlendSrcOver
            );
            // And if we are already kSrc, the opaque paint ID is the original ID so skip lookup
            if old_id == BuiltInCodeSnippetID::FixedBlendSrc {
                return orig_paint;
            }

            // NOTE: If we choose to include paint-alpha multiplication in pipelines by default to
            // avoid 2x combinations just because the SkPaint changed from 1 to anything else, we
            // could include removing the paint alpha multiplication as part of this rewriting if
            // we restructure paint alpha handling to maintain an equivalent ShaderNode structure
            // while just eliding the multiply

            key_context.dict().find_or_create_for_builder(&mut builder)
        };
        #[cfg(debug_assertions)]
        debug_assert_eq!(opaque_id, self.validate_opacity_optimization(key_context));
        opaque_id
    }

    /// The id of the opaque, coverage-less version of this paint, regenerated from scratch with
    /// its own builder and gatherer. Also checks that the uniforms it gathers match.
    // Port of: src/gpu/graphite/PaintParams.cpp#L652-L698 (chrome/m156), `validateOpacityOptimization`
    #[cfg(debug_assertions)]
    fn validate_opacity_optimization(&self, key_context: &KeyContext<'_>) -> UniquePaintParamsID {
        // Validate that the modified paint ID matches what we would have reached with a
        // ShadingParams and PaintParams adjusted to use kSrc blending and have no renderer
        // coverage. These extracted uniforms should match what was originally extracted as well.
        let opaque_paint = PaintParams::make_opaque(self.paint);
        let opaque_shading = ShadingParams::new(
            key_context.caps(),
            &opaque_paint,
            None,
            None,
            Coverage::None,
            self.target_format,
        );

        // The opaque context writes to a different key builder and pipeline data gatherer.
        let opaque_builder = RefCell::new(PaintParamsKeyBuilder::new(key_context.dict()));
        let layout = key_context
            .pipeline_data_gatherer()
            .borrow_mut()
            .uniform_manager()
            .layout();
        let opaque_gatherer = RefCell::new(PipelineDataGatherer::new(layout));
        let opaque_context = key_context.with_new_key_storage(
            &opaque_builder,
            &opaque_gatherer,
            opaque_paint.color(),
        );

        let result = opaque_shading.to_key(&opaque_context);
        let (actual_opaque_id, actual_dst_usage) =
            result.expect("an opaque paint always has a key");
        debug_assert_eq!(actual_dst_usage, DstUsage::NONE);
        opaque_gatherer
            .borrow()
            .check_equivalent(&key_context.pipeline_data_gatherer().borrow());
        actual_opaque_id
    }
}

/// `AddToKey(const SimpleImage&)`: the image shader of an image override. Its block (an image
/// shader with a local matrix) needs `add_image_to_key`, which is not ported; until then the
/// structure is kept with an error block.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2688-L2702 (chrome/m156)
fn add_simple_image_to_key(key_context: &KeyContext<'_>, _simple_image: &SimpleImage) {
    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .add_error_block();
}

// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/precompile/PaintOption.{h,cpp}

//! `PaintOption`: one combination of a `PaintOptions`, reduced to the key it produces.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::PMColor4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::rect::Rect;

use crate::graphite::context_utils::can_use_hardware_blending;
use crate::graphite::geom::non_msaa_clip::NonMSAAClip;
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::key_helpers::{
    AlphaOnlyPaintColorBlock, RGBPaintColorBlock, SolidColorShaderBlock, add_dither_block,
};
use crate::graphite::key_helpers_ii::{
    add_analytic_clip, add_blend_mode, add_fixed_blend_mode, add_primitive_color,
    add_to_key_blender, blend, compose,
};
use crate::graphite::paint_params_key::RootBlockType;
use crate::graphite::precompile::blender::PrecompileBlender;
use crate::graphite::precompile::blender::PrecompileBlenders;
use crate::graphite::precompile::color_filter::PrecompileColorFilter;
use crate::graphite::precompile::shader::PrecompileShader;
use crate::graphite::render_step::Coverage;
use crate::graphite::texture_format::TextureFormat;
use skia_rust_core::blend_mode_blender::get_blend_mode_singleton;

/// `PaintOption`: the option a `PaintOptions` combination selects, as its key is built.
// Port of: src/gpu/graphite/precompile/PaintOption.h#L19-L74 (chrome/m156)
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ fields, which are flags
pub(crate) struct PaintOption {
    opaque_paint_color: bool,
    final_blender: (Option<PrecompileBlender>, i32),
    shader: (Option<PrecompileShader>, i32),
    color_filter: (Option<PrecompileColorFilter>, i32),
    has_primitive_blender: bool,
    primitive_blend_mode: BlendMode,
    skip_color_xform: bool,
    clip_shader: (Option<PrecompileShader>, i32),
    renderer_coverage: Coverage,
    target_format: TextureFormat,
    dither: bool,
    analytic_clip: bool,
}

impl PaintOption {
    // Port of: PaintOption::PaintOption (chrome/m156)
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)] // mirrors the C++ constructor
    pub(crate) fn new(
        opaque_paint_color: bool,
        final_blender: (Option<PrecompileBlender>, i32),
        shader: (Option<PrecompileShader>, i32),
        color_filter: (Option<PrecompileColorFilter>, i32),
        has_primitive_blender: bool,
        primitive_blend_mode: BlendMode,
        skip_color_xform: bool,
        clip_shader: (Option<PrecompileShader>, i32),
        coverage: Coverage,
        target_format: TextureFormat,
        dither: bool,
        analytic_clip: bool,
    ) -> Self {
        let mut option = Self {
            opaque_paint_color,
            final_blender,
            shader,
            color_filter,
            has_primitive_blender,
            primitive_blend_mode,
            skip_color_xform,
            clip_shader,
            renderer_coverage: coverage,
            target_format,
            dither,
            analytic_clip,
        };
        if option.final_blender_blend_mode() == Some(BlendMode::Clear) {
            option.final_blender = (Some(PrecompileBlenders::mode(BlendMode::Src)), 0);
            option.opaque_paint_color = false;
            option.shader = (None, 0);
            option.color_filter = (None, 0);
            option.has_primitive_blender = false;
            option.dither = false;
        } else if !option.has_primitive_blender {
            if option
                .shader
                .0
                .as_ref()
                .is_some_and(|shader| shader.is_constant(option.shader.1))
            {
                option.shader = (None, 0);
            }
            if option.shader.0.is_none() && option.color_filter.0.is_some() {
                option.color_filter = (None, 0);
            }
        }
        option
    }

    /// The blend mode of `finalBlender`, when it is one.
    fn final_blender_blend_mode(&self) -> Option<BlendMode> {
        self.final_blender
            .0
            .as_ref()
            .and_then(PrecompileBlender::as_blend_mode)
    }

    /// `PaintOption::toKey`.
    // Port of: src/gpu/graphite/precompile/PaintOption.cpp#L62-L104 (chrome/m156)
    pub(crate) fn to_key(&self, key_context: &KeyContext<'_>) {
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_root_block_header(RootBlockType::SrcColor);
        let is_opaque = self.handle_dithering(key_context);
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_root_block_header(RootBlockType::FinalBlend);

        let final_blend_mode = match &self.final_blender.0 {
            None => Some(BlendMode::SrcOver),
            Some(blender) => blender.as_blend_mode(),
        };
        match final_blend_mode {
            None => {
                let blender = self.final_blender.0.as_ref().expect("a final blender");
                blender.add_to_key(key_context, self.final_blender.1);
            }
            Some(mut final_blend_mode) => {
                debug_assert_ne!(final_blend_mode, BlendMode::Clear);
                let has_analytic_clip = self.clip_shader.0.is_some() || self.analytic_clip;
                let mut effective_coverage = self.renderer_coverage;
                if effective_coverage == Coverage::None && has_analytic_clip {
                    effective_coverage = Coverage::SingleChannel;
                }
                let optimize_src_blend = !has_analytic_clip
                    && (final_blend_mode == BlendMode::Src
                        || final_blend_mode == BlendMode::SrcOver)
                    && key_context
                        .flags()
                        .contains(KeyGenFlags::PREFER_FIXED_SRC_BLEND);
                let dst_read_req = !can_use_hardware_blending(
                    key_context.caps(),
                    self.target_format,
                    final_blend_mode,
                    effective_coverage,
                );
                if final_blend_mode == BlendMode::SrcOver && is_opaque {
                    let dst_usage_none =
                        !has_analytic_clip && self.renderer_coverage == Coverage::None;
                    if dst_usage_none && optimize_src_blend {
                        debug_assert!(can_use_hardware_blending(
                            key_context.caps(),
                            self.target_format,
                            BlendMode::Src,
                            self.renderer_coverage,
                        ));
                        final_blend_mode = BlendMode::Src;
                    }
                }
                if !dst_read_req || (final_blend_mode == BlendMode::Src && optimize_src_blend) {
                    add_fixed_blend_mode(key_context, final_blend_mode);
                } else {
                    add_blend_mode(key_context, final_blend_mode);
                }
            }
        }

        if self.clip_shader.0.is_some() || self.analytic_clip {
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_root_block_header(RootBlockType::Clip);
            self.handle_clipping(key_context);
        }
    }

    /// `PaintOption::addPaintColorToKey`: returns whether the source is opaque.
    fn add_paint_color_to_key(&self, key_context: &KeyContext<'_>) -> bool {
        if let Some(shader) = &self.shader.0 {
            shader.add_to_key(key_context, self.shader.1);
            shader.is_opaque(self.shader.1)
        } else {
            RGBPaintColorBlock::add_block(key_context);
            true
        }
    }

    /// `PaintOption::handlePrimitiveColor`.
    fn handle_primitive_color(&self, key_context: &KeyContext<'_>) -> bool {
        if !self.has_primitive_blender {
            return self.add_paint_color_to_key(key_context);
        }
        if self.skip_color_xform && self.primitive_blend_mode == BlendMode::Dst {
            add_primitive_color(
                key_context,
                self.skip_color_xform,
                None,
                skia_rust_core::alpha_type::AlphaType::Premul,
            );
            return false;
        }
        let mut src_is_opaque = false;
        blend(
            key_context,
            // TODO (C++): allow clients to provide precompile blender options for primitive
            // blending. For now there is a back door to internally specify a blend mode.
            || {
                add_to_key_blender(
                    key_context,
                    Some(get_blend_mode_singleton(self.primitive_blend_mode)),
                );
            },
            || src_is_opaque = self.add_paint_color_to_key(key_context),
            || {
                add_primitive_color(
                    key_context,
                    self.skip_color_xform,
                    None,
                    skia_rust_core::alpha_type::AlphaType::Premul,
                );
            },
        );
        if src_is_opaque {
            self.primitive_blend_mode == BlendMode::Src
                || self.primitive_blend_mode == BlendMode::SrcOver
        } else {
            false
        }
    }

    /// `PaintOption::handlePaintAlpha`.
    fn handle_paint_alpha(&self, key_context: &KeyContext<'_>) -> bool {
        if self.shader.0.is_none() && !self.has_primitive_blender {
            SolidColorShaderBlock::add_block(
                key_context,
                &PMColor4f {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            );
            return self.opaque_paint_color;
        }
        if self.opaque_paint_color {
            self.handle_primitive_color(key_context)
        } else {
            blend(
                key_context,
                || add_fixed_blend_mode(key_context, BlendMode::SrcIn),
                || {
                    self.handle_primitive_color(key_context);
                },
                || AlphaOnlyPaintColorBlock::add_block(key_context),
            );
            false
        }
    }

    /// `PaintOption::handleColorFilter`.
    fn handle_color_filter(&self, key_context: &KeyContext<'_>) -> bool {
        match &self.color_filter.0 {
            Some(color_filter) => {
                let mut src_is_opaque = false;
                compose(
                    key_context,
                    || src_is_opaque = self.handle_paint_alpha(key_context),
                    || color_filter.add_to_key(key_context, self.color_filter.1),
                );
                src_is_opaque && color_filter.is_alpha_unchanged(self.color_filter.1)
            }
            None => self.handle_paint_alpha(key_context),
        }
    }

    /// `PaintOption::shouldDither`.
    fn should_dither(&self, dst_ct: ColorType) -> bool {
        if !self.dither {
            return false;
        }
        if dst_ct == ColorType::Unknown {
            return false;
        }
        if dst_ct == ColorType::RGB565 || dst_ct == ColorType::ARGB4444 {
            return true;
        }
        self.shader
            .0
            .as_ref()
            .is_some_and(|shader| !shader.is_constant(self.shader.1))
    }

    /// `PaintOption::handleDithering`.
    fn handle_dithering(&self, key_context: &KeyContext<'_>) -> bool {
        let ct = key_context.dst_color_info().color_type();
        if self.should_dither(ct) {
            let mut src_is_opaque = false;
            compose(
                key_context,
                || src_is_opaque = self.handle_color_filter(key_context),
                || add_dither_block(key_context, ct),
            );
            src_is_opaque
        } else {
            self.handle_color_filter(key_context)
        }
    }

    /// `PaintOption::handleClipping`.
    // Port of: src/gpu/graphite/precompile/PaintOption.cpp#L252-L300 (chrome/m156), the
    // `!SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER` branch
    fn handle_clipping(&self, key_context: &KeyContext<'_>) {
        debug_assert!(self.analytic_clip || self.clip_shader.0.is_some());
        if self.analytic_clip {
            // `just needs to be non-empty`
            let mut clip = NonMSAAClip::default();
            clip.analytic_clip.bounds = Rect::from_ltrb(0.0, 0.0, 1.0, 1.0);
            if self.clip_shader.0.is_some() {
                // For both an analytic clip and clip shader, compose them into a single root node.
                blend(
                    key_context,
                    || add_fixed_blend_mode(key_context, BlendMode::Modulate),
                    || add_analytic_clip(key_context, &clip),
                    || self.add_clip_shader_to_key(key_context),
                );
            } else {
                // Without a clip shader, the analytic clip can be the clipping root node.
                add_analytic_clip(key_context, &clip);
            }
        } else {
            // Since there's no analytic clip, the clipping root node can be the clip shader.
            self.add_clip_shader_to_key(key_context);
        }
    }

    /// `fClipShader.first->priv().addToKey(keyContext, fClipShader.second)`.
    fn add_clip_shader_to_key(&self, key_context: &KeyContext<'_>) {
        if let Some(shader) = &self.clip_shader.0 {
            shader.add_to_key(key_context, self.clip_shader.1);
        }
    }
}

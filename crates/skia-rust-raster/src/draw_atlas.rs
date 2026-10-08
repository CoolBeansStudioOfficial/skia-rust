// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDraw_atlas.cpp

//! `skcpu::Draw::drawAtlas`: draws the sprites of an atlas shader, each as a transformed
//! rectangle (optionally blended with a color) through a raster pipeline blitter.
//!
//! skia-rust: Skia builds the pipeline once, and updates the matrix of the
//! `SkTransformShader` and the `uniform_color_dst` context in place between sprites. Pipeline
//! contexts are immutable here, so each sprite builds its own pipeline with its matrix and
//! color, and a blitter for it. The stages and the arithmetic are the same, so the pixels are
//! too.

use std::cell::Cell;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode_priv;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::Color;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::floating_point::float_round2int;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_raw_shapes;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::raster_pipeline::{RasterPipeline, Stage, contexts::UniformColorCtx};
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{ShaderBase, TransformShader};

use crate::blitter::Blitter;
use crate::draw::Draw;
use crate::raster_clip::RasterClip;
use crate::raster_pipeline_blitter::create_raster_pipeline_blitter_with_pipeline;
use crate::scan::{fill_path_clip, fill_rect_clip};

// Port of: src/core/SkDraw_atlas.cpp#L50-L62 (chrome/m156)
fn fill_rect(ctm: &Matrix, rc: &RasterClip, r: &Rect, blitter: &mut dyn Blitter) {
    if ctm.rect_stays_rect() {
        fill_rect_clip(&ctm.map_rect(r).0, rc, blitter);
    } else {
        let mut raw = path_raw_shapes::Rect::new(r, PathDirection::CW, 0);
        ctm.map_points_inplace(raw.points_mut());
        if let Some(bounds) = Rect::bounds(raw.points()) {
            let mut path = raw.raw();
            path.bounds = bounds;
            fill_path_clip(&path, rc, blitter);
        }
    }
}

// Port of: src/core/SkDraw_atlas.cpp#L64-L70 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// mirrors the implicit int -> uint16_t conversion of `ctx->rgba[i] = SkScalarRoundToInt(...)`
fn load_color(ctx: &mut UniformColorCtx, rgba: [f32; 4]) {
    // only need one of these. can I query the pipeline to know if its lowp or highp?
    ctx.rgba[0] = float_round2int(rgba[0] * 255.0) as u16;
    ctx.r = rgba[0];
    ctx.rgba[1] = float_round2int(rgba[1] * 255.0) as u16;
    ctx.g = rgba[1];
    ctx.rgba[2] = float_round2int(rgba[2] * 255.0) as u16;
    ctx.b = rgba[2];
    ctx.rgba[3] = float_round2int(rgba[3] * 255.0) as u16;
    ctx.a = rgba[3];
}

impl Draw<'_> {
    /// Draws the `textures` rectangles of the shader of `paint` (the atlas), each mapped by the
    /// matching `xform` and, if `colors` is not empty, blended with its color using `blender`
    /// (`drawAtlas`).
    // Port of: src/core/SkDraw_atlas.cpp#L73-L159 (chrome/m156)
    #[doc(alias = "drawAtlas")]
    #[allow(clippy::too_many_lines)] // mirrors drawAtlas
    pub fn draw_atlas(
        &mut self,
        xform: &[RSXform],
        textures: &[Rect],
        colors: &[Color],
        blender: &Blender,
        paint: &Paint,
    ) {
        debug_assert_eq!(xform.len(), textures.len());
        debug_assert!(colors.is_empty() || xform.len() == colors.len());

        let Some(atlas_shader) = paint.shader() else {
            return;
        };
        if xform.is_empty() {
            return;
        }

        let mut p = paint.clone();
        p.set_anti_alias(false); // we never respect this for drawAtlas(or drawVertices)
        p.set_style(Style::Fill);
        p.set_shader(None);
        p.set_mask_filter(None);

        // The RSXForms can't contain perspective - only the CTM can.
        let perspective = self.ctm.has_perspective();

        let props = self.props.copied().unwrap_or_default();
        let rc = self.rc;
        let ctm = self.ctm;
        let dst_cs = self.dst.color_space();
        let dst_color_type = self.dst.color_type();

        let steps = ColorSpaceXformSteps::new(
            Some(srgb_singleton()),
            AlphaType::Unpremul,
            dst_cs.as_ref(),
            AlphaType::Unpremul,
        );
        // The blend mode that combines the colors with the sprites.
        let color_blend_mode = if colors.is_empty() {
            None
        } else {
            let Some(bm) = blender.as_base().as_blend_mode() else {
                return;
            };
            Some(bm)
        };

        for (i, (xform, texture)) in xform.iter().zip(textures).enumerate() {
            let mut transform_shader = TransformShader::new(atlas_shader.clone(), perspective);

            let mut mx = Matrix::new_identity();
            mx.set_rsxform(xform);
            mx.pre_translate((-texture.left, -texture.top));
            mx.post_concat(ctm);
            let Some(inv) = mx.invert() else {
                return; // non-invertible
            };
            if !transform_shader.update(&inv) {
                continue;
            }

            // The pipeline of this sprite.
            let alloc = ArenaAlloc::new();
            let mut pipeline = RasterPipeline::new();
            let ok = {
                let mut rec = StageRec {
                    pipeline: &mut pipeline,
                    alloc: &alloc,
                    dst_color_type,
                    dst_cs: dst_cs.as_ref(),
                    paint_color: p.color4f(),
                    surface_props: props,
                    dst_bounds: Rect::new_empty(),
                };
                // We pass an identity matrix here rather than the CTM. The CTM gets folded into
                // the per-triangle matrix.
                Shader::from_base(transform_shader.clone())
                    .as_base()
                    .append_root_stages(&mut rec, &Matrix::new_identity())
            };
            if !ok {
                return;
            }

            if let Some(bm) = color_blend_mode {
                // the color of this sprite
                let mut c4 = Color4f::from_color(colors[i]);
                let mut vec = c4.as_array();
                steps.apply(&mut vec);
                c4 = Color4f::new(vec[0], vec[1], vec[2], vec[3]);
                let mut uniform_ctx = UniformColorCtx::default();
                load_color(&mut uniform_ctx, c4.premul().as_array());
                pipeline.append(Stage::UniformColorDst(alloc.make(uniform_ctx)));
                blend_mode_priv::append_stages(bm, &mut pipeline);
            }

            let mut is_opaque = colors.is_empty() && ShaderBase::is_opaque(&transform_shader);
            let alpha_f: scalar = p.alpha_f();
            #[allow(clippy::float_cmp)] // mirrors `p.getAlphaf() != 1`
            if alpha_f != 1.0 {
                pipeline.append(Stage::Scale1Float(alloc.make(Cell::new(alpha_f))));
                is_opaque = false;
            }

            let Some(mut blitter) = create_raster_pipeline_blitter_with_pipeline(
                self.dst.reborrow_mut(),
                &p,
                &pipeline,
                is_opaque,
                &alloc,
                rc.clip_shader(),
            ) else {
                return;
            };

            fill_rect(&mx, rc, texture, &mut blitter);
        }
    }
}

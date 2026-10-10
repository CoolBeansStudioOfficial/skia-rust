// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlitter.cpp (`UseLegacyBlitter`, `Choose`)

//! `SkBlitter::Choose` and `SkBlitter::UseLegacyBlitter`: which blitter draws a paint into a
//! device.
//!
//! [`choose`] returns a legacy blitter ([`Argb32Blitter`], [`Argb32OpaqueBlitter`],
//! [`Argb32BlackBlitter`], [`Argb32ShaderBlitter`] or [`A8CoverageBlitter`]) when it can, a null
//! blitter when nothing can be drawn, and otherwise the raster pipeline blitter
//! ([`create_raster_pipeline_blitter`]).
//!
//! skia-rust: the global `gSkForceRasterPipelineBlitter` (set by `dm --forceRasterPipeline`) is the
//! `force_raster_pipeline_blitter` parameter.

use std::borrow::Cow;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blend_mode_priv::{BlendFastPath, check_fast_path};
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::paint_priv::{remove_color_filter, should_dither};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{ContextRec, MatrixRec};
use skia_rust_core::surface_props::SurfaceProps;

use crate::blitter::{Blitter, NullBlitter};
use crate::blitter_a8::A8CoverageBlitter;
use crate::core_blitters::{
    Argb32BlackBlitter, Argb32Blitter, Argb32OpaqueBlitter, Argb32ShaderBlitter,
};
use crate::oracle_n32::is_n32;
use crate::raster_pipeline_blitter::{RasterPipelineBlitter, create_raster_pipeline_blitter};

/// Whether the legacy blitters (`SkARGB32_*_Blitter`) can draw `paint` into `device`
/// (`SkBlitter::UseLegacyBlitter`). They cannot handle dithering, unpremultiplied devices, blend
/// modes other than source-over, 3D masks, color spaces beyond sRGB, or any color type but
/// `kN32`.
///
/// `force_raster_pipeline_blitter` stands for `gSkForceRasterPipelineBlitter`. `_matrix` is
/// unused, as in Skia.
// Port of: src/core/SkBlitter.cpp#L612-L648 (chrome/m156)
#[doc(alias = "UseLegacyBlitter")]
#[must_use]
pub fn use_legacy_blitter(
    device: &Pixmap<'_>,
    paint: &Paint,
    _matrix: &Matrix,
    force_raster_pipeline_blitter: bool,
) -> bool {
    if force_raster_pipeline_blitter {
        return false;
    }

    if paint.is_dither() {
        return false;
    }

    let mf = paint.mask_filter();

    // The legacy blitters cannot handle any of these "complex" features (anymore).
    if device.alpha_type() == AlphaType::Unpremul
        || !paint.is_src_over()
        || mf.is_some_and(|mf| mf.as_base().format() == MaskFormat::ThreeD)
    {
        return false;
    }

    // We check (indirectly via makeContext()) later on if the shader can handle the colorspace in
    // legacy mode, so here we just focus on if a single color needs raster-pipeline.
    if let Some(cs) = device.color_space()
        && paint.shader().is_none()
        && (!paint.color4f().fits_in_bytes() || !cs.is_srgb())
    {
        return false;
    }

    // Only kN32 is handled by legacy blitters now
    is_n32(device.color_type())
}

/// The blitter `SkBlitter::Choose` made, by kind (the C++ returns an `SkBlitter*`).
pub(crate) enum Chosen<'a> {
    /// `SkNullBlitter`: nothing is drawn.
    Null(NullBlitter),
    /// `SkA8_Coverage_Blitter`.
    A8Coverage(A8CoverageBlitter<'a>),
    /// `SkARGB32_Shader_Blitter`.
    Shader(Argb32ShaderBlitter<'a>),
    /// `SkARGB32_Black_Blitter`.
    Black(Argb32BlackBlitter<'a>),
    /// `SkARGB32_Opaque_Blitter`.
    Opaque(Argb32OpaqueBlitter<'a>),
    /// `SkARGB32_Blitter`.
    Argb32(Argb32Blitter<'a>),
    /// The blitter of `SkCreateRasterPipelineBlitter`.
    RasterPipeline(Box<RasterPipelineBlitter<'a>>),
}

impl<'a> Chosen<'a> {
    pub(crate) fn into_blitter(self) -> Box<dyn Blitter + 'a> {
        match self {
            Chosen::Null(b) => Box::new(b),
            Chosen::A8Coverage(b) => Box::new(b),
            Chosen::Shader(b) => Box::new(b),
            Chosen::Black(b) => Box::new(b),
            Chosen::Opaque(b) => Box::new(b),
            Chosen::Argb32(b) => Box::new(b),
            Chosen::RasterPipeline(b) => b,
        }
    }
}

/// `SkBlitter::Choose`: the blitter that draws `orig_paint` with `ctm` into `device`.
///
/// The raster pipeline blitter (where the legacy ones cannot draw the paint) allocates its
/// contexts in `alloc`, which must outlive the blitter, as Skia's `SkArenaAlloc*` must.
/// `force_raster_pipeline_blitter` stands for `gSkForceRasterPipelineBlitter`.
#[must_use]
#[allow(clippy::too_many_arguments)] // Skia's signature, plus the force flag
pub fn choose<'a>(
    device: Pixmap<'a>,
    ctm: &Matrix,
    orig_paint: &Paint,
    alloc: &'a ArenaAlloc,
    draw_coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    props: &SurfaceProps,
    dev_bounds: &Rect,
    force_raster_pipeline_blitter: bool,
) -> Box<dyn Blitter + 'a> {
    choose_kind(
        device,
        ctm,
        orig_paint,
        alloc,
        draw_coverage,
        clip_shader,
        props,
        dev_bounds,
        force_raster_pipeline_blitter,
    )
    .into_blitter()
}

// Port of: src/core/SkBlitter.cpp#L650-L760 (chrome/m156)
#[allow(clippy::too_many_arguments)] // Skia's signature, plus the force flag
pub(crate) fn choose_kind<'a>(
    device: Pixmap<'a>,
    ctm: &Matrix,
    orig_paint: &Paint,
    alloc: &'a ArenaAlloc,
    draw_coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    props: &SurfaceProps,
    dev_bounds: &Rect,
    force_raster_pipeline_blitter: bool,
) -> Chosen<'a> {
    if ColorType::Unknown == device.color_type() {
        return Chosen::Null(NullBlitter::new());
    }

    // We may tweak the original paint as we go.
    let mut paint: Cow<'_, Paint> = Cow::Borrowed(orig_paint);

    if let Some(mode) = paint.as_blend_mode() {
        // We have the most fast-paths for SrcOver, so see if we can act like SrcOver.
        if mode != BlendMode::SrcOver {
            match check_fast_path(&paint, device.color_type().is_always_opaque()) {
                BlendFastPath::SrcOver => {
                    paint.to_mut().set_blend_mode(BlendMode::SrcOver);
                }
                BlendFastPath::SkipDrawing => return Chosen::Null(NullBlitter::new()),
                BlendFastPath::Normal => {}
            }
        }

        // A Clear blend mode will ignore the entire color pipeline, as if Src mode with
        // 0x00000000.
        if mode == BlendMode::Clear {
            let p = paint.to_mut();
            p.set_shader(None);
            p.set_color_filter(None);
            p.set_blend_mode(BlendMode::Src);
            p.set_color(Color::TRANSPARENT);
        }
    }

    if paint.color_filter().is_some() {
        remove_color_filter(paint.to_mut(), device.color_space().as_ref());
    }
    debug_assert!(paint.color_filter().is_none());

    if draw_coverage == DrawCoverage::Yes {
        if device.color_type() == ColorType::Alpha8 {
            debug_assert!(paint.shader().is_none());
            debug_assert!(paint.is_src_over());
            return Chosen::A8Coverage(A8CoverageBlitter::new(device, &paint));
        }
        return Chosen::Null(NullBlitter::new());
    }

    if paint.is_dither() && !should_dither(&paint, device.color_type()) {
        paint.to_mut().set_dither(false);
    }

    let create_rp_blitter = |device: Pixmap<'a>, paint: &Paint| -> Chosen<'a> {
        match create_raster_pipeline_blitter(
            device,
            paint,
            ctm,
            alloc,
            clip_shader,
            props,
            dev_bounds,
        ) {
            Some(blitter) => Chosen::RasterPipeline(Box::new(blitter)),
            None => Chosen::Null(NullBlitter::new()),
        }
    };

    // We'll end here for many interesting cases: color spaces, color filters, most color types.
    if clip_shader.is_some()
        || !use_legacy_blitter(&device, &paint, ctm, force_raster_pipeline_blitter)
    {
        return create_rp_blitter(device, &paint);
    }

    // Everything but legacy kN32_SkColorType should already be handled.
    debug_assert!(is_n32(device.color_type()));

    // And we should be blending with SrcOver
    debug_assert_eq!(paint.as_blend_mode(), Some(BlendMode::SrcOver));

    // Legacy blitters keep their shader state on a shader context.
    let mut shader_context = None;
    if let Some(shader) = paint.shader() {
        let rec = ContextRec::new(
            paint.alpha(),
            MatrixRec::new(ctm),
            device.color_type(),
            device.color_space(),
        );
        shader_context = shader.as_base().make_context(&rec);

        // Creating the context isn't always possible... try fallbacks before giving up.
        if shader_context.is_none() {
            return create_rp_blitter(device, &paint);
        }
    }

    if let Some(shader_context) = shader_context {
        Chosen::Shader(Argb32ShaderBlitter::new(device, &paint, shader_context))
    } else if paint.color() == Color::BLACK {
        Chosen::Black(Argb32BlackBlitter::new(device, &paint))
    } else if paint.alpha() == 0xFF {
        Chosen::Opaque(Argb32OpaqueBlitter::new(device, &paint))
    } else {
        Chosen::Argb32(Argb32Blitter::new(device, &paint))
    }
}

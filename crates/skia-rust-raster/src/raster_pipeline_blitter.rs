// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipelineBlitter.cpp

//! `SkRasterPipelineBlitter`: the blitter that draws everything through a raster pipeline.
//!
//! [`create_raster_pipeline_blitter`] (`SkCreateRasterPipelineBlitter`) builds the pipeline for a
//! paint, a destination and a clip shader; [`RasterPipelineBlitter`] then compiles one pipeline
//! per kind of blit (rect, antialiased run, A8, LCD16 and 3D mask) the first time it is needed
//! and runs it over the pixels, with a `memset` fast path for solid fills.
//!
//! # How it differs from the C++
//! - **Memory.** Pipeline stages name memory by [`MemSlot`] and the blitter binds the memory per
//!   run: slot 0 is the destination (Skia's `fDstPtr`, always the top left of `fDst`), slot 1 the
//!   mask plane (`fMaskPtr`, set by every `blitMask`), slots 2 and 3 the 3D mask's `mul` and `add`
//!   planes (`fEmbossCtx`) and slot 4 the clip shader's alpha buffer (`fClipShaderBuffer`, owned
//!   by the blitter instead of the arena; `fAlloc`, which only feeds `SkRasterPipeline`'s
//!   constructor, is not kept), slot 5 the sprite source ([`SOURCE`]) and slot 6 the scratch
//!   memory the shaders reserved in the arena (`SHADER_SCRATCH`, `ArenaAlloc::alloc_scratch`),
//!   which the blitter owns. `fCurrentCoverage` is a `Cell<f32>` in the arena, which
//!   `scale_1_float`/`lerp_1_float` read and `blit_anti_h` writes between runs.
//! - **Lifetime.** The blitter does not live in the arena; it is a value that borrows the arena
//!   and the destination pixels for `'a`.
//! - **Memset.** Skia's `rect_memset16/32/64` take typed pointers; the destination here is
//!   bytes, so the fill writes the same pixel bytes row by row (a memset has the same result
//!   however it is executed).
//! - **Not ported.** The viewer-only `SkRasterPipelineVisualizer::CreateBlitter`.

use std::cell::Cell;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blend_mode_priv::should_pre_scale_coverage;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::{Alpha, Color4f, colors};
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::effect_priv::{SHADER_SCRATCH, StageRec};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::raster_pipeline::{
    CompiledPipeline, MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
    contexts::{EmbossCtx, MAX_STRIDE},
};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::shader::Shader;
use skia_rust_core::surface_props::SurfaceProps;

use crate::blitter::{BlitMemory, Blitter, DirectBlit, blit_mask_default};

/// `fDstPtr`: the destination pixels.
const DST: MemSlot = MemSlot(0);
/// `fMaskPtr`: the mask plane of the current `blitMask`.
const MASK: MemSlot = MemSlot(1);
/// `fEmbossCtx.mul`.
const EMBOSS_MUL: MemSlot = MemSlot(2);
/// `fEmbossCtx.add`.
const EMBOSS_ADD: MemSlot = MemSlot(3);
/// `fClipShaderBuffer`.
const CLIP: MemSlot = MemSlot(4);

/// The source pixels of a pre-baked shader pipeline that loads from memory, bound by
/// [`RasterPipelineBlitter::blit_rect_with_source`]: the sprite blitter's `fSrcPtr`.
pub const SOURCE: MemSlot = MemSlot(5);

/// Bytes of the clip shader's alpha buffer: `kMaxStride` floats (large enough for highp floats
/// or lowp `U16`s).
const CLIP_BUFFER_BYTES: usize = MAX_STRIDE * 4;

/// The pipelines a [`RasterPipelineBlitter`] builds lazily (`fBlitRect`, `fBlitAntiH`,
/// `fBlitMaskA8`, `fBlitMaskLCD16`, `fBlitMask3D`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlitKind {
    /// `fBlitRect`: `blit_rect`/`blit_h`.
    Rect,
    /// `fBlitAntiH`: `blit_anti_h`.
    AntiH,
    /// `fBlitMaskA8`.
    MaskA8,
    /// `fBlitMaskLCD16`.
    MaskLcd16,
    /// `fBlitMask3D`.
    Mask3D,
}

impl BlitKind {
    /// Every kind, in `fBlit*` declaration order.
    pub const ALL: [BlitKind; 5] = [
        BlitKind::Rect,
        BlitKind::AntiH,
        BlitKind::MaskA8,
        BlitKind::MaskLcd16,
        BlitKind::Mask3D,
    ];
}

// Port of: src/core/SkRasterPipelineBlitter.cpp#L54-L70 (chrome/m156)
fn can_direct_blit(paint: &Paint) -> bool {
    if paint.shader().is_some() || paint.color_filter().is_some() || paint.is_dither() {
        return false;
    }
    if let Some(mode) = paint.as_blend_mode() {
        match mode {
            BlendMode::Clear | BlendMode::Src => return true,
            #[allow(clippy::float_cmp)] // mirrors `paint.getAlphaf() == 1`
            BlendMode::SrcOver => return paint.alpha_f() == 1.0,
            _ => {}
        }
    }
    false
}

/// `SkRasterPipelineBlitter`. See the module documentation.
// Port of: src/core/SkRasterPipelineBlitter.cpp#L72-L147 (chrome/m156)
#[doc(alias = "SkRasterPipelineBlitter")]
#[derive(Debug)]
pub struct RasterPipelineBlitter<'a> {
    /// `fDst`.
    dst: Pixmap<'a>,
    /// `fColorPipeline`.
    color_pipeline: RasterPipeline<'a>,
    /// `fBlendPipeline`.
    blend_pipeline: RasterPipeline<'a>,
    /// If the blender is a blend-mode, we retain that information for late-stage optimizations
    /// (`fBlendMode`).
    blend_mode: Option<BlendMode>,
    /// The clip shader's alpha storage (`fClipShaderBuffer` and the `Storage` it points into);
    /// empty without a clip shader.
    clip_buffer: Vec<u8>,
    /// The memory the shaders of the pipeline reserved in the arena (`ArenaAlloc::alloc_scratch`),
    /// bound to `SHADER_SCRATCH`.
    scratch: Vec<u8>,
    /// `fClipShaderBuffer != nullptr`.
    has_clip_shader: bool,

    /// `fCanDirectBlit`.
    can_direct_blit: bool,
    /// `fDirectBlitPaintColor`.
    direct_blit_paint_color: Color4f,
    /// `fDirectBlitValue`.
    direct_blit_value: Option<u64>,

    /// `fMemset2D`, as the shift per pixel of the fill (0 to 3); `None` when blits cannot memset.
    memset_shift: Option<usize>,
    /// `fMemsetColor`: the pixel to memset, as its bytes ("big enough for largest memsettable dst
    /// format, F16").
    memset_color: [u8; 8],

    /// `fBlitRect`, `fBlitAntiH`, `fBlitMaskA8`, `fBlitMaskLCD16`, `fBlitMask3D`, built lazily on
    /// first use (indexed by [`BlitKind`]).
    blit: [Option<CompiledPipeline<'a>>; 5],

    /// `SkBlitter::fBlitMemory`.
    blit_memory: BlitMemory,

    /// `fCurrentCoverage`, pointed to by the blit pipelines, which allows us to adjust it from
    /// call to call.
    current_coverage: &'a Cell<f32>,
    /// `fDitherRate`.
    dither_rate: f32,
}

// Port of: src/core/SkRasterPipelineBlitter.cpp#L149-L154 (chrome/m156)
fn paint_color_to_dst(paint: &Paint, dst: &Pixmap<'_>) -> Color4f {
    let paint_color = paint.color4f();
    let dst_cs = dst.color_space();
    let mut vec = paint_color.as_array();
    ColorSpaceXformSteps::new(
        Some(srgb_singleton()),
        AlphaType::Unpremul,
        dst_cs.as_ref(),
        AlphaType::Unpremul,
    )
    .apply(&mut vec);
    Color4f::new(vec[0], vec[1], vec[2], vec[3])
}

/// What [`create_pipeline_for_blitter`] works out besides the pipeline.
struct ShaderInfo {
    /// `dstPaintColor`.
    dst_paint_color: Color4f,
    /// `is_opaque`.
    is_opaque: bool,
    /// `is_constant`.
    is_constant: bool,
}

// Port of: src/core/SkRasterPipelineBlitter.cpp#L156-L193 (chrome/m156)
fn create_pipeline_for_blitter<'a>(
    dst: &Pixmap<'_>,
    paint: &Paint,
    ctm: &Matrix,
    alloc: &'a ArenaAlloc,
    props: &SurfaceProps,
    shader_pipeline: &mut RasterPipeline<'a>,
    dev_bounds: &Rect,
) -> Option<ShaderInfo> {
    let dst_paint_color = paint_color_to_dst(paint, dst);

    let Some(shader) = paint.shader() else {
        // Having no shader makes things nice and easy... just use the paint color
        shader_pipeline.append_constant_color(alloc, &dst_paint_color.premul().as_array());
        #[allow(clippy::float_cmp)] // mirrors `fA == 1.0f`
        let is_opaque = dst_paint_color.a == 1.0;
        return Some(ShaderInfo {
            dst_paint_color,
            is_opaque,
            is_constant: true,
        });
    };

    let color_space = dst.color_space();
    #[allow(clippy::float_cmp)] // mirrors `fA == 1.0f`
    let is_opaque = shader.as_base().is_opaque() && dst_paint_color.a == 1.0;
    let is_constant = shader.as_base().is_constant().is_some();
    let ok = shader.as_base().append_root_stages(
        &mut StageRec {
            pipeline: shader_pipeline,
            alloc,
            dst_color_type: dst.color_type(),
            dst_cs: color_space.as_ref(),
            paint_color: dst_paint_color,
            surface_props: *props,
            dst_bounds: *dev_bounds,
        },
        ctm,
    );
    if ok {
        #[allow(clippy::float_cmp)] // mirrors `fA != 1.0f`
        if dst_paint_color.a != 1.0 {
            shader_pipeline.append(Stage::Scale1Float(alloc.make(Cell::new(dst_paint_color.a))));
        }
        return Some(ShaderInfo {
            dst_paint_color,
            is_opaque,
            is_constant,
        });
    }
    // The shader can't draw with SkRasterPipeline.
    None
}

/// Creates the blitter that draws `paint` (with its shader, color filter and blender) through a
/// raster pipeline into `dst`, or `None` if the shader, color filter or blender cannot be
/// appended to a pipeline (`SkCreateRasterPipelineBlitter`).
///
/// `ctm` positions the shader, `dev_bounds` is the device-space bounds of the geometry (an empty
/// rect when expensive to compute) and `clip_shader`, if any, scales the coverage of every blit.
///
/// skia-rust: Skia allocates the blitter in `alloc`; here it is returned by value and borrows
/// `alloc` and the pixels of `dst`.
// Port of: src/core/SkRasterPipelineBlitter.cpp#L195-L226 (chrome/m156)
#[doc(alias = "SkCreateRasterPipelineBlitter")]
#[must_use]
pub fn create_raster_pipeline_blitter<'a>(
    dst: Pixmap<'a>,
    paint: &Paint,
    ctm: &Matrix,
    alloc: &'a ArenaAlloc,
    clip_shader: Option<&Shader>,
    props: &SurfaceProps,
    dev_bounds: &Rect,
) -> Option<RasterPipelineBlitter<'a>> {
    let mut shader_pipeline = RasterPipeline::new();
    let info = create_pipeline_for_blitter(
        &dst,
        paint,
        ctm,
        alloc,
        props,
        &mut shader_pipeline,
        dev_bounds,
    )?;

    RasterPipelineBlitter::create(
        dst,
        paint,
        info.dst_paint_color,
        alloc,
        &shader_pipeline,
        info.is_opaque,
        info.is_constant,
        clip_shader,
    )
}

/// [`create_raster_pipeline_blitter`] for a caller that has built the shader's stages itself
/// (sprites, vertices, atlases): `shader_pipeline` produces the source color and `is_opaque` says
/// whether it is always opaque (`SkCreateRasterPipelineBlitter` with a pipeline).
// Port of: src/core/SkRasterPipelineBlitter.cpp#L289-L299 (chrome/m156)
#[doc(alias = "SkCreateRasterPipelineBlitter")]
#[must_use]
pub fn create_raster_pipeline_blitter_with_pipeline<'a>(
    dst: Pixmap<'a>,
    paint: &Paint,
    shader_pipeline: &RasterPipeline<'a>,
    is_opaque: bool,
    alloc: &'a ArenaAlloc,
    clip_shader: Option<&Shader>,
) -> Option<RasterPipelineBlitter<'a>> {
    let is_constant = false; // If this were the case, it'd be better to just set a paint color.
    let dst_paint_color = paint_color_to_dst(paint, &dst);
    RasterPipelineBlitter::create(
        dst,
        paint,
        dst_paint_color,
        alloc,
        shader_pipeline,
        is_opaque,
        is_constant,
        clip_shader,
    )
}

// Port of: src/core/SkRasterPipelineBlitter.cpp#L501-L506 (chrome/m156)
fn append_load_dst(info: &ImageInfo, p: &mut RasterPipeline<'_>) {
    p.append_load_dst(info.color_type(), MemoryCtx::new(DST));
    if info.alpha_type() == AlphaType::Unpremul {
        p.append(Stage::PremulDst);
    }
}

// Port of: src/core/SkRasterPipelineBlitter.cpp#L508-L513 (chrome/m156)
fn append_store(info: &ImageInfo, p: &mut RasterPipeline<'_>) {
    if info.alpha_type() == AlphaType::Unpremul {
        p.append(Stage::Unpremul);
    }
    p.append_store(info.color_type(), MemoryCtx::new(DST));
}

/// `usize` of a blit coordinate (Skia converts the `int`s to `size_t`).
fn to_usize(v: i32) -> usize {
    usize::try_from(v).expect("blit coordinates and sizes are not negative")
}

impl<'a> RasterPipelineBlitter<'a> {
    // Port of: src/core/SkRasterPipelineBlitter.cpp#L84-L93 (chrome/m156)
    fn new(dst: Pixmap<'a>, paint: &Paint, alloc: &'a ArenaAlloc) -> Self {
        RasterPipelineBlitter {
            dst,
            color_pipeline: RasterPipeline::new(),
            blend_pipeline: RasterPipeline::new(),
            blend_mode: None,
            clip_buffer: Vec::new(),
            scratch: Vec::new(),
            has_clip_shader: false,
            can_direct_blit: can_direct_blit(paint),
            direct_blit_paint_color: paint.color4f(),
            direct_blit_value: None,
            memset_shift: None,
            memset_color: [0; 8],
            blit: [None, None, None, None, None],
            blit_memory: BlitMemory::default(),
            current_coverage: alloc.make(Cell::new(0.0)),
            dither_rate: 0.0,
        }
    }

    /// Our common entrypoint for creating the blitter once we've sorted out shaders.
    // Port of: src/core/SkRasterPipelineBlitter.cpp#L301-L499 (chrome/m156)
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // mirrors Create
    fn create(
        dst: Pixmap<'a>,
        paint: &Paint,
        dst_paint_color: Color4f,
        alloc: &'a ArenaAlloc,
        shader_pipeline: &RasterPipeline<'a>,
        mut is_opaque: bool,
        mut is_constant: bool,
        clip_shader: Option<&Shader>,
    ) -> Option<Self> {
        let mut blitter = RasterPipelineBlitter::new(dst, paint, alloc);

        // Our job in this factory is to fill out the blitter's color and blend pipelines.
        // The color pipeline is the common front of the full blit pipeline. The blend pipeline is
        // just the portion that does the actual blending math (and assumes that src and dst are
        // already loaded).
        //
        // The full blit pipelines are each constructed lazily on first use, and include the color
        // pipeline, reading the dst, the blend pipeline, coverage, dithering, and writing the dst.

        // Start with the color pipeline
        if let Some(clip_shader) = clip_shader {
            let clip_ct = ColorType::RGBA8888;
            let mut rec = StageRec {
                pipeline: &mut blitter.color_pipeline,
                alloc,
                dst_color_type: clip_ct,
                dst_cs: None, // clipCS
                paint_color: colors::BLACK,
                surface_props: SurfaceProps::default(), // default OK; no text here
                dst_bounds: Rect::new_empty(),
            };
            if clip_shader
                .as_base()
                .append_root_stages(&mut rec, Matrix::i())
            {
                // large enough for highp (float) or lowp(U16)
                blitter.clip_buffer = vec![0; CLIP_BUFFER_BYTES];
                blitter
                    .color_pipeline
                    .append(Stage::StoreSrcA(MemPtr::new(CLIP, 0)));
                blitter.has_clip_shader = true;
                is_constant = false;
            } else {
                return None;
            }
        }

        // Let's get the shader in first.
        blitter.color_pipeline.extend(shader_pipeline);

        // If there's a color filter it comes next.
        if let Some(color_filter) = paint.color_filter() {
            let dst_cs = blitter.dst.color_space();
            let mut rec = StageRec {
                pipeline: &mut blitter.color_pipeline,
                alloc,
                dst_color_type: blitter.dst.color_type(),
                dst_cs: dst_cs.as_ref(),
                paint_color: dst_paint_color,
                surface_props: SurfaceProps::default(), // default OK; no text here
                dst_bounds: Rect::new_empty(),
            };
            if !color_filter.as_base().append_stages(&mut rec, is_opaque) {
                return None;
            }
            is_opaque = is_opaque && color_filter.is_alpha_unchanged();
        }

        // Not all formats make sense to dither (think, F16).  We set their dither rate
        // to zero.  We only dither non-constant shaders, so is_constant won't change here.
        if paint.is_dither() && !is_constant {
            blitter.dither_rate = match blitter.dst.info().color_type() {
                ColorType::ARGB4444 => 1.0 / 15.0,
                ColorType::RGB565 => 1.0 / 63.0,
                ColorType::Gray8
                | ColorType::RGB888x
                | ColorType::RGBA8888
                | ColorType::BGRA8888
                | ColorType::SRGBA8888
                | ColorType::R8UNorm => 1.0 / 255.0,
                ColorType::RGB101010x
                | ColorType::RGBA1010102
                | ColorType::BGR101010x
                | ColorType::BGRA1010102
                | ColorType::BGRA10101010XR
                | ColorType::RGBA10x6 => 1.0 / 1023.0,
                ColorType::Unknown
                | ColorType::Alpha8
                | ColorType::BGR101010xXR
                | ColorType::RGBAF16
                | ColorType::RGBF16F16F16x
                | ColorType::RGBAF16Norm
                | ColorType::RGBAF32
                | ColorType::R8G8UNorm
                | ColorType::A16Float
                | ColorType::A16UNorm
                | ColorType::R16G16Float
                | ColorType::R16UNorm
                | ColorType::R16Float
                | ColorType::R16G16UNorm
                | ColorType::R16G16B16A16UNorm => 0.0,
            };
            if blitter.dither_rate > 0.0 {
                let rate = blitter.dither_rate;
                blitter.color_pipeline.append(Stage::Dither(rate));
            }
        }

        // Optimization: A pipeline that's still constant here can collapse back into a constant
        // color.
        if is_constant {
            // We could remove this clamp entirely, but if the destination is 8888, doing the
            // clamp here allows the color pipeline to still run in lowp (we'll use uniform_color,
            // rather than unbounded_uniform_color).
            blitter
                .color_pipeline
                .append_clamp_if_normalized(blitter.dst.info());
            blitter
                .color_pipeline
                .append(Stage::StoreF32(MemoryCtx::new(DST)));
            let mut constant_color = [0u8; 16];
            blitter.color_pipeline.run(
                0,
                0,
                1,
                1,
                &mut MemoryBindings::new().with(DST, MemView::write(&mut constant_color)),
            );
            blitter.color_pipeline.reset();
            let component = |i: usize| {
                f32::from_ne_bytes([
                    constant_color[4 * i],
                    constant_color[4 * i + 1],
                    constant_color[4 * i + 2],
                    constant_color[4 * i + 3],
                ])
            };
            let constant_color =
                Color4f::new(component(0), component(1), component(2), component(3));
            blitter
                .color_pipeline
                .append_constant_color4f(alloc, &constant_color);

            #[allow(clippy::float_cmp)] // mirrors `fA == 1.0f`
            {
                is_opaque = constant_color.a == 1.0;
            }
        }

        // Now we'll build the blend pipeline
        let mut blender = paint
            .blender()
            .unwrap_or_else(|| Blender::mode(BlendMode::SrcOver));

        // We can strength-reduce SrcOver into Src when opaque.
        if is_opaque && blender.as_base().as_blend_mode() == Some(BlendMode::SrcOver) {
            blender = Blender::mode(BlendMode::Src);
        }

        // When we're drawing a constant color in Src mode, we can sometimes just memset.
        // (The previous two optimizations help find more opportunities for this one.)
        if is_constant
            && blender.as_base().as_blend_mode() == Some(BlendMode::Src)
            && blitter.dst.info().bytes_per_pixel() <= size_of::<u64>()
        {
            // Run our color pipeline all the way through to produce what we'd memset when we can.
            // Not all blits can memset, so we need to keep colorPipeline too.
            let mut p = RasterPipeline::new();
            p.extend(&blitter.color_pipeline);
            append_store(blitter.dst.info(), &mut p);
            p.run(
                0,
                0,
                1,
                1,
                &mut MemoryBindings::new().with(DST, MemView::write(&mut blitter.memset_color)),
            );

            // TODO(F32)? (a shift of 4 has no fill)
            let shift = blitter.dst.shift_per_pixel();
            if shift <= 3 {
                blitter.memset_shift = Some(shift);
            }
        }

        {
            let dst_cs = blitter.dst.color_space();
            let mut rec = StageRec {
                pipeline: &mut blitter.blend_pipeline,
                alloc,
                dst_color_type: blitter.dst.color_type(),
                dst_cs: dst_cs.as_ref(),
                paint_color: dst_paint_color,
                surface_props: SurfaceProps::default(), // default OK; no text here
                dst_bounds: Rect::new_empty(),
            };
            if !blender.as_base().append_stages(&mut rec) {
                return None;
            }
            blitter.blend_mode = blender.as_base().as_blend_mode();
        }

        // The memory the shaders reserved in the arena (blend shaders' stored colors).
        blitter.scratch = vec![0; alloc.scratch_bytes()];

        Some(blitter)
    }

    /// `appendClipScale`: these check internally, and only append if there was a native
    /// clipShader.
    // Port of: src/core/SkRasterPipelineBlitter.cpp#L515-L519 (chrome/m156)
    fn append_clip_scale(&self, p: &mut RasterPipeline<'a>) {
        if self.has_clip_shader {
            p.append(Stage::ScaleNative(MemPtr::new(CLIP, 0)));
        }
    }

    /// `appendClipLerp`.
    // Port of: src/core/SkRasterPipelineBlitter.cpp#L521-L525 (chrome/m156)
    fn append_clip_lerp(&self, p: &mut RasterPipeline<'a>) {
        if self.has_clip_shader {
            p.append(Stage::LerpNative(MemPtr::new(CLIP, 0)));
        }
    }

    /// `fBlendMode.has_value() && SkBlendMode_ShouldPreScaleCoverage(*fBlendMode, rgb_coverage)`.
    fn pre_scale_coverage(&self, rgb_coverage: bool) -> bool {
        self.blend_mode
            .is_some_and(|mode| should_pre_scale_coverage(mode, rgb_coverage))
    }

    /// The A8 pipeline's coverage and blend steps, which the 3D pipeline shares ("Now onward just
    /// as kA8").
    fn append_a8_coverage(&self, p: &mut RasterPipeline<'a>) {
        p.append_clamp_if_normalized(self.dst.info());
        if self.pre_scale_coverage(false) {
            p.append(Stage::ScaleU8(MemoryCtx::new(MASK)));
            self.append_clip_scale(p);
            append_load_dst(self.dst.info(), p);
            p.extend(&self.blend_pipeline);
        } else {
            append_load_dst(self.dst.info(), p);
            p.extend(&self.blend_pipeline);
            p.append(Stage::LerpU8(MemoryCtx::new(MASK)));
            self.append_clip_lerp(p);
        }
        append_store(self.dst.info(), p);
    }

    /// The pipeline the blitter compiles for `kind` the first time it is needed: the color
    /// pipeline, the destination load, the blend pipeline, coverage and the store.
    ///
    /// skia-rust: Skia's lambdas are private; this exposes the stage lists (the tests compare
    /// them with the oracle's).
    // Port of: src/core/SkRasterPipelineBlitter.cpp#L537-L563, #L569-L588, #L661-L722 (chrome/m156)
    #[must_use]
    pub fn pipeline_for(&self, kind: BlitKind) -> RasterPipeline<'a> {
        let mut p = RasterPipeline::new();
        p.extend(&self.color_pipeline);
        let info = self.dst.info();
        match kind {
            BlitKind::Rect => {
                p.append_clamp_if_normalized(info);
                if self.blend_mode == Some(BlendMode::SrcOver)
                    && (info.color_type() == ColorType::RGBA8888
                        || info.color_type() == ColorType::BGRA8888)
                    && self.dst.color_space().is_none()
                    && info.alpha_type() != AlphaType::Unpremul
                    && self.dither_rate == 0.0
                {
                    if info.color_type() == ColorType::BGRA8888 {
                        p.append(Stage::SwapRb);
                    }
                    self.append_clip_scale(&mut p);
                    p.append(Stage::SrcoverRgba8888(MemoryCtx::new(DST)));
                } else {
                    if self.blend_mode != Some(BlendMode::Src) {
                        append_load_dst(info, &mut p);
                        p.extend(&self.blend_pipeline);
                        self.append_clip_lerp(&mut p);
                    } else if self.has_clip_shader {
                        append_load_dst(info, &mut p);
                        self.append_clip_lerp(&mut p);
                    }
                    append_store(info, &mut p);
                }
            }
            BlitKind::AntiH => {
                p.append_clamp_if_normalized(info);
                if self.pre_scale_coverage(false) {
                    p.append(Stage::Scale1Float(self.current_coverage));
                    self.append_clip_scale(&mut p);
                    append_load_dst(info, &mut p);
                    p.extend(&self.blend_pipeline);
                } else {
                    append_load_dst(info, &mut p);
                    p.extend(&self.blend_pipeline);
                    p.append(Stage::Lerp1Float(self.current_coverage));
                    self.append_clip_lerp(&mut p);
                }
                append_store(info, &mut p);
            }
            BlitKind::MaskA8 => self.append_a8_coverage(&mut p),
            BlitKind::MaskLcd16 => {
                p.append_clamp_if_normalized(info);
                if self.pre_scale_coverage(true) {
                    // Somewhat unusually, scale_565 needs dst loaded first.
                    append_load_dst(info, &mut p);
                    p.append(Stage::Scale565(MemoryCtx::new(MASK)));
                    self.append_clip_scale(&mut p);
                    p.extend(&self.blend_pipeline);
                } else {
                    append_load_dst(info, &mut p);
                    p.extend(&self.blend_pipeline);
                    p.append(Stage::Lerp565(MemoryCtx::new(MASK)));
                    self.append_clip_lerp(&mut p);
                }
                append_store(info, &mut p);
            }
            BlitKind::Mask3D => {
                // This bit is where we differ from kA8_Format:
                p.append(Stage::Emboss(EmbossCtx {
                    mul: MemoryCtx::new(EMBOSS_MUL),
                    add: MemoryCtx::new(EMBOSS_ADD),
                }));
                // Now onward just as kA8.
                self.append_a8_coverage(&mut p);
            }
        }
        p
    }

    /// The destination (`fDst`).
    ///
    /// skia-rust: Skia's blitter lets the caller keep reading the pixels it handed in; here the
    /// blitter holds the borrow, so this is how to see them between blits.
    #[must_use]
    pub fn dst(&self) -> &Pixmap<'a> {
        &self.dst
    }

    /// `blit_rect` with `source` bound to [`SOURCE`], for a blitter made by
    /// [`create_raster_pipeline_blitter_with_pipeline`] from a shader pipeline that loads its
    /// pixels through `MemoryCtx::new(SOURCE)`. This is the part of
    /// `SkRasterPipelineSpriteBlitter::blitRect` after it points `fSrcPtr` at the sprite
    /// (`fBlitter->blitRect(x, y, width, height)`); the view's origin is Skia's "fake base"
    /// pointer, so device pixel `(x, y)` reads the source's pixel `(x - left, y - top)`.
    ///
    /// skia-rust: Skia's blitter reads `fSrcPtr` through the pipeline's `MemoryCtx`; here the
    /// pixels are bound for the run, like every other plane.
    ///
    /// # Panics
    /// If the blitter has a `memset` fill (it only has one when the color pipeline is constant,
    /// which a pipeline that loads pixels is not).
    pub fn blit_rect_with_source(
        &mut self,
        (x, y, w, h): (i32, i32, i32, i32),
        source: MemView<'_>,
    ) {
        assert!(
            self.memset_shift.is_none(),
            "a shader pipeline that loads pixels is not constant"
        );
        self.run_blit(BlitKind::Rect, (x, y, w, h), vec![(SOURCE, source)]);
    }

    /// Whether the pipeline of `kind` has been built yet (`fBlit* != nullptr`).
    #[must_use]
    pub fn is_compiled(&self, kind: BlitKind) -> bool {
        self.blit[kind as usize].is_some()
    }

    /// Compiles the pipeline of `kind` if this is its first use.
    fn ensure_compiled(&mut self, kind: BlitKind) {
        let index = kind as usize;
        if self.blit[index].is_none() {
            let p = self.pipeline_for(kind);
            self.blit[index] = Some(p.compile());
        }
    }

    /// Runs the compiled pipeline of `kind` over the `w x h` pixels at `(x, y)`, with the mask
    /// planes (`MASK`, `EMBOSS_MUL`, `EMBOSS_ADD`) bound from `mask_planes`.
    fn run_blit(
        &mut self,
        kind: BlitKind,
        (x, y, w, h): (i32, i32, i32, i32),
        mask_planes: Vec<(MemSlot, MemView<'_>)>,
    ) {
        self.ensure_compiled(kind);
        let stride = isize::try_from(self.dst.row_bytes_as_pixels())
            .expect("row bytes fit in the address space");
        let dst_bytes = self
            .dst
            .bytes_mut()
            .expect("the destination of a raster pipeline blitter must be writable");
        let mut mem = MemoryBindings::new();
        mem.bind(DST, MemView::write(dst_bytes).with_stride(stride));
        if self.has_clip_shader {
            mem.bind(CLIP, MemView::write(&mut self.clip_buffer));
        }
        if !self.scratch.is_empty() {
            mem.bind(SHADER_SCRATCH, MemView::write(&mut self.scratch));
        }
        for (slot, view) in mask_planes {
            mem.bind(slot, view);
        }
        self.blit[kind as usize]
            .as_mut()
            .expect("compiled above")
            .run(to_usize(x), to_usize(y), to_usize(w), to_usize(h), &mut mem);
    }

    /// The fill `fMemset2D` does: `w x h` pixels at `(x, y)` set to `fMemsetColor`.
    // Port of: src/core/SkRasterPipelineBlitter.cpp#L453-L475 (chrome/m156)
    fn memset_2d(&mut self, shift: usize, (x, y, w, h): (i32, i32, i32, i32)) {
        let bpp = 1usize << shift;
        let row_bytes = self.dst.row_bytes();
        let pixel = self.memset_color;
        let bytes = self
            .dst
            .bytes_mut()
            .expect("the destination of a raster pipeline blitter must be writable");
        let mut offset = to_usize(y) * row_bytes + (to_usize(x) << shift);
        let row_len = to_usize(w) << shift;
        let mut h = h;
        while h > 0 {
            h -= 1;
            let row = &mut bytes[offset..offset + row_len];
            if shift == 0 {
                row.fill(pixel[0]);
            } else {
                for px in row.chunks_exact_mut(bpp) {
                    px.copy_from_slice(&pixel[..bpp]);
                }
            }
            offset += row_bytes;
        }
    }
}

// Port of: src/core/SkRasterPipelineBlitter.cpp#L527-L767 (chrome/m156)
impl Blitter for RasterPipelineBlitter<'_> {
    // Port of: src/core/SkRasterPipelineBlitter.cpp#L527-L529 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, w: i32) {
        self.blit_rect(x, y, w, 1);
    }

    // Port of: src/core/SkRasterPipelineBlitter.cpp#L531-L567 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if let Some(shift) = self.memset_shift {
            self.memset_2d(shift, (x, y, w, h));
            return;
        }
        self.run_blit(BlitKind::Rect, (x, y, w, h), Vec::new());
    }

    // Port of: src/core/SkRasterPipelineBlitter.cpp#L569-L603 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &mut [Alpha], runs: &mut [i16]) {
        self.ensure_compiled(BlitKind::AntiH);

        let mut x = x;
        let mut i = 0usize;
        loop {
            let run = runs[i];
            if run <= 0 {
                break;
            }
            let run_len = usize::try_from(run).expect("positive");
            match aa[i] {
                0x00 => {}
                0xff => self.blit_rect(x, y, i32::from(run), 1),
                a => {
                    self.current_coverage.set(f32::from(a) * (1.0 / 255.0));
                    self.run_blit(BlitKind::AntiH, (x, y, i32::from(run), 1), Vec::new());
                }
            }
            x += i32::from(run);
            i += run_len;
        }
    }

    // Port of: src/core/SkRasterPipelineBlitter.cpp#L605-L610 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // mirrors `(uint8_t)a0`
    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let clip = IRect::new(x, y, x + 2, y + 1);
        let coverage = [a0 as u8, a1 as u8];
        let mask = Mask::new(&coverage, clip, 2, MaskFormat::A8);
        self.blit_mask(&mask, &clip);
    }

    // Port of: src/core/SkRasterPipelineBlitter.cpp#L612-L617 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // mirrors `(uint8_t)a0`
    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let clip = IRect::new(x, y, x + 1, y + 2);
        let coverage = [a0 as u8, a1 as u8];
        let mask = Mask::new(&coverage, clip, 1, MaskFormat::A8);
        self.blit_mask(&mask, &clip);
    }

    // Port of: src/core/SkRasterPipelineBlitter.cpp#L619-L625 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        let clip = IRect::new(x, y, x + 1, y + height);
        let alpha = [alpha];
        let mask = Mask::new(
            &alpha,
            clip,
            0, // so we reuse the 1 "row" for all of height
            MaskFormat::A8,
        );
        self.blit_mask(&mask, &clip);
    }

    // Port of: src/core/SkRasterPipelineBlitter.cpp#L627-L736 (chrome/m156)
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        if mask.format == MaskFormat::BW {
            // TODO: native BW masks?
            blit_mask_default(self, mask, clip);
            return;
        }

        // ARGB and SDF masks shouldn't make it here.
        debug_assert!(matches!(
            mask.format,
            MaskFormat::A8 | MaskFormat::Lcd16 | MaskFormat::ThreeD
        ));

        // Selects the right mask plane. Usually plane == 0 and this is just mask.fImage. The view
        // is "into" this current mask, but lined up with the destination at (0,0): its origin is
        // the (possibly out of range) position of pixel (0, 0).
        let extract_mask_plane = |plane: usize| -> MemView<'_> {
            // LCD is 16-bit per pixel; A8 and 3D are 8-bit per pixel.
            let bpp: usize = if mask.format == MaskFormat::Lcd16 {
                2
            } else {
                1
            };
            let to_isize = |v: usize| isize::try_from(v).expect("mask sizes fit in isize");
            let row_bytes = to_isize(mask.row_bytes as usize);
            let from_i32 = |v: i32| isize::try_from(v).expect("i32 fits in isize");
            let origin = to_isize(plane * mask.compute_image_size())
                - from_i32(mask.bounds.left) * to_isize(bpp)
                - from_i32(mask.bounds.top) * row_bytes;
            MemView::read(mask.image)
                .with_stride(row_bytes / to_isize(bpp))
                .with_origin(origin)
        };

        let mut planes = vec![(MASK, extract_mask_plane(0))];
        if mask.format == MaskFormat::ThreeD {
            planes.push((EMBOSS_MUL, extract_mask_plane(1)));
            planes.push((EMBOSS_ADD, extract_mask_plane(2)));
        }

        // Lazily build whichever pipeline we need, specialized for each mask format.
        let kind = match mask.format {
            MaskFormat::A8 => BlitKind::MaskA8,
            MaskFormat::Lcd16 => BlitKind::MaskLcd16,
            MaskFormat::ThreeD => BlitKind::Mask3D,
            _ => {
                debug_assert!(false);
                return;
            }
        };
        self.run_blit(
            kind,
            (clip.left, clip.top, clip.width(), clip.height()),
            planes,
        );
    }

    // Port of: src/core/SkRasterPipelineBlitter.cpp#L738-L767 (chrome/m156)
    fn can_direct_blit(&mut self) -> Option<DirectBlit<'_>> {
        if self.can_direct_blit {
            if self.direct_blit_value.is_none() {
                // want maximum alignment (8) but legal punning for smaller int types
                let mut dst_buffer = [0u8; 8];
                let dst_info = ImageInfo::new(
                    (1, 1),
                    self.dst.info().color_type(),
                    self.dst.info().alpha_type(),
                    None,
                );
                let src_info =
                    ImageInfo::new((1, 1), ColorType::RGBAF32, AlphaType::Unpremul, None);
                let mut src = [0u8; 16];
                for (i, c) in self.direct_blit_paint_color.as_array().iter().enumerate() {
                    src[4 * i..4 * i + 4].copy_from_slice(&c.to_ne_bytes());
                }
                let converted = convert_pixels(
                    &dst_info,
                    &mut dst_buffer,
                    size_of::<[u8; 8]>(),
                    &src_info,
                    &src,
                    size_of::<[u8; 16]>(),
                );
                let bpp = dst_info.bytes_per_pixel();
                if converted && matches!(bpp, 1 | 2 | 4 | 8) {
                    let mut value = [0u8; 8];
                    value[..bpp].copy_from_slice(&dst_buffer[..bpp]);
                    self.direct_blit_value = Some(u64::from_le_bytes(value));
                } else {
                    self.can_direct_blit = false;
                    return None;
                }
            }
            let value = self.direct_blit_value.expect("computed above");
            return Some(DirectBlit {
                pm: self.dst.reborrow_mut(),
                value,
            });
        }
        self.can_direct_blit = false;
        None
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.blit_memory
    }
}

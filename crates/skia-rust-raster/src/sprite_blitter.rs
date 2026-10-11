// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkSpriteBlitter.h, src/core/SkBlitter_Sprite.cpp,
// src/core/SkSpriteBlitter_ARGB32.cpp

//! Sprite blitters: blitters that move large rectangles of pixels around (`SkSpriteBlitter`).
//!
//! Because of this use, the main primitive shifts from `blit_h`-style things to the more
//! efficient `blit_rect`; `blit_h`, `blit_anti_h`, `blit_v` and `blit_mask` are never called on a
//! sprite blitter. [`choose_sprite`] picks one for a (source pixmap, destination pixmap) pair:
//! a plain copy, the source-over `kN32` blitter, or a
//! [raster pipeline blitter](crate::raster_pipeline_blitter) that loads the sprite's pixels.

use core::cell::Cell;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Alpha, Color4f};
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info_priv::color_type_is_alpha_only;
use skia_rust_core::mask::Mask;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::raster_pipeline::{MemView, MemoryCtx, RasterPipeline, Stage};
use skia_rust_core::rect::IRect;
use skia_rust_core::shader::Shader;

use crate::blit_row::{GLOBAL_ALPHA_FLAG32, Proc32, SRC_PIXEL_ALPHA_FLAG32, factory32};
use crate::blitter::{BlitMemory, Blitter, blit_mask_default, blit_v_default};
use crate::oracle_n32::is_n32;
use crate::pixel_rows::{bytes_mut, load_u32s, with_span32};
use crate::raster_pipeline_blitter::{
    RasterPipelineBlitter, SOURCE, create_raster_pipeline_blitter_with_pipeline,
};

/// Which sprite blitter `ChooseSprite` made (the C++ returns an `SkBlitter*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpriteKind {
    /// `SkSpriteBlitter_Memcpy`.
    Memcpy,
    /// `Sprite_D32_S32`.
    D32S32,
    /// `SkRasterPipelineSpriteBlitter`.
    RasterPipeline,
}

/// A [`Blitter`] that draws a source pixmap into a destination one (`SkSpriteBlitter`'s
/// virtuals).
trait SpriteBlitter<'a>: Blitter {
    /// Which blitter this is.
    fn kind(&self) -> SpriteKind;

    /// Sets the destination, and where the source's pixel `(0, 0)` lands in it (`setup`); false
    /// if the blitter cannot draw `paint`.
    // Port of: src/core/SkBlitter_Sprite.cpp#L39-L46 (chrome/m156)
    fn setup(&mut self, dst: Pixmap<'a>, left: i32, top: i32, paint: &Paint) -> bool;
}

/// The state every sprite blitter has (`SkSpriteBlitter`'s data members).
///
/// skia-rust: `fPaint` is not kept: nothing reads it after `setup`.
// Port of: src/core/SkSpriteBlitter.h#L35-L39 (chrome/m156)
#[derive(Debug)]
struct SpriteBase<'a> {
    dst: Pixmap<'a>,
    source: Pixmap<'a>,
    left: i32,
    top: i32,
    memory: BlitMemory,
}

impl<'a> SpriteBase<'a> {
    // Port of: src/core/SkBlitter_Sprite.cpp#L35-L37 (chrome/m156)
    fn new(source: Pixmap<'a>) -> Self {
        SpriteBase {
            dst: Pixmap::default(),
            source,
            left: 0,
            top: 0,
            memory: BlitMemory::default(),
        }
    }

    // Port of: src/core/SkBlitter_Sprite.cpp#L39-L45 (chrome/m156)
    fn setup(&mut self, dst: Pixmap<'a>, left: i32, top: i32) -> bool {
        self.dst = dst;
        self.left = left;
        self.top = top;
        true
    }
}

// The methods of `SkSpriteBlitter` that should not be called, as `Blitter` methods of a type
// whose `SpriteBase` is `self.base`.
macro_rules! sprite_blitter_fallbacks {
    () => {
        // Port of: src/core/SkBlitter_Sprite.cpp#L48-L53 (chrome/m156)
        fn blit_h(&mut self, x: i32, y: i32, width: i32) {
            #[allow(clippy::assertions_on_constants)] // SkDEBUGFAIL: fails in debug builds only
            {
                assert!(!cfg!(debug_assertions), "how did we get here?");
            }

            // Fallback to blitRect.
            self.blit_rect(x, y, width, 1);
        }

        // Port of: src/core/SkBlitter_Sprite.cpp#L55-L59 (chrome/m156)
        fn blit_anti_h(&mut self, _x: i32, _y: i32, _antialias: &mut [Alpha], _runs: &mut [i16]) {
            #[allow(clippy::assertions_on_constants)] // SkDEBUGFAIL: fails in debug builds only
            {
                assert!(!cfg!(debug_assertions), "how did we get here?");
            }

            // No fallback strategy.
        }

        // Port of: src/core/SkBlitter_Sprite.cpp#L61-L66 (chrome/m156)
        fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
            #[allow(clippy::assertions_on_constants)] // SkDEBUGFAIL: fails in debug builds only
            {
                assert!(!cfg!(debug_assertions), "how did we get here?");
            }

            // Fall back to superclass if the code gets here in release mode.
            blit_v_default(self, x, y, height, alpha);
        }

        // Port of: src/core/SkBlitter_Sprite.cpp#L68-L73 (chrome/m156)
        fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
            #[allow(clippy::assertions_on_constants)] // SkDEBUGFAIL: fails in debug builds only
            {
                assert!(!cfg!(debug_assertions), "how did we get here?");
            }

            // Fall back to superclass if the code gets here in release mode.
            blit_mask_default(self, mask, clip);
        }

        fn blit_memory(&mut self) -> &mut BlitMemory {
            &mut self.base.memory
        }
    };
}

// The byte offset of pixel `(x, y)` of `pm`, of any color type.
fn offset_of(pm: &Pixmap<'_>, x: i32, y: i32) -> usize {
    debug_assert!(x >= 0 && y >= 0, "({x}, {y}) is outside the pixmap");
    let x = usize::try_from(x).expect("pixel x is not negative");
    let y = usize::try_from(y).expect("pixel y is not negative");
    y * pm.row_bytes() + x * pm.info().bytes_per_pixel()
}

// A non-negative `int` as a length.
fn to_len(n: i32) -> usize {
    usize::try_from(n).expect("blit sizes are not negative")
}

/// Copies the source into the destination (`SkSpriteBlitter_Memcpy`).
// Port of: src/core/SkBlitter_Sprite.cpp#L77-L113 (chrome/m156)
#[derive(Debug)]
struct SpriteMemcpy<'a> {
    base: SpriteBase<'a>,
}

impl<'a> SpriteMemcpy<'a> {
    // Port of: src/core/SkBlitter_Sprite.cpp#L79-L93 (chrome/m156)
    fn supports(dst: &Pixmap<'_>, src: &Pixmap<'_>, paint: &Paint) -> bool {
        // the caller has already inspected the colorspace on src and dst
        debug_assert_eq!(0, xform_flags_mask(src, dst));

        if dst.color_type() != src.color_type() {
            return false;
        }
        if paint.mask_filter().is_some()
            || paint.color_filter().is_some()
            || paint.image_filter().is_some()
        {
            return false;
        }
        if 0xFF != paint.alpha() {
            return false;
        }
        let mode = paint.as_blend_mode();
        mode == Some(BlendMode::Src) || (mode == Some(BlendMode::SrcOver) && src.is_opaque())
    }

    fn new(src: Pixmap<'a>) -> Self {
        SpriteMemcpy {
            base: SpriteBase::new(src),
        }
    }
}

impl<'a> SpriteBlitter<'a> for SpriteMemcpy<'a> {
    fn kind(&self) -> SpriteKind {
        SpriteKind::Memcpy
    }

    fn setup(&mut self, dst: Pixmap<'a>, left: i32, top: i32, _paint: &Paint) -> bool {
        self.base.setup(dst, left, top)
    }
}

impl Blitter for SpriteMemcpy<'_> {
    sprite_blitter_fallbacks!();

    // Port of: src/core/SkBlitter_Sprite.cpp#L97-L110 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let base = &mut self.base;
        debug_assert_eq!(base.dst.color_type(), base.source.color_type());
        debug_assert!(width > 0 && height > 0);

        let mut dst = offset_of(&base.dst, x, y);
        let mut src = offset_of(&base.source, x - base.left, y - base.top);
        let dst_rb = base.dst.row_bytes();
        let src_rb = base.source.row_bytes();
        let bytes_to_copy = to_len(width) << base.source.shift_per_pixel();
        let src_bytes = base.source.bytes().expect("the source has pixels");
        let dst_bytes = bytes_mut(&mut base.dst);

        let mut height = height;
        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            dst_bytes[dst..dst + bytes_to_copy]
                .copy_from_slice(&src_bytes[src..src + bytes_to_copy]);
            dst += dst_rb;
            src += src_rb;
        }
    }
}

// `SkColorSpaceXformSteps(src, dst).fFlags.mask()`: what converting `src`'s pixels to `dst`'s
// color space and alpha type needs.
fn xform_flags_mask(src: &Pixmap<'_>, dst: &Pixmap<'_>) -> u32 {
    ColorSpaceXformSteps::new(
        src.color_space().as_ref(),
        src.alpha_type(),
        dst.color_space().as_ref(),
        dst.alpha_type(),
    )
    .flags
    .mask()
}

/// A sprite blitter for a raster pipeline blitter (`SkRasterPipelineSpriteBlitter`).
///
/// skia-rust: the blitter the pipeline is given to is held as `fBlitter`, and the source pixels
/// are bound to the pipeline's [`SOURCE`] slot for each `blit_rect` (`fSrcPtr`).
// Port of: src/core/SkBlitter_Sprite.cpp#L115-L178 (chrome/m156)
struct RasterPipelineSpriteBlitter<'a> {
    base: SpriteBase<'a>,
    alloc: &'a ArenaAlloc,
    blitter: Option<RasterPipelineBlitter<'a>>,
    clip_shader: Option<Shader>,
}

impl<'a> RasterPipelineSpriteBlitter<'a> {
    // Port of: src/core/SkBlitter_Sprite.cpp#L117-L124 (chrome/m156)
    fn new(src: Pixmap<'a>, alloc: &'a ArenaAlloc, clip_shader: Option<Shader>) -> Self {
        RasterPipelineSpriteBlitter {
            base: SpriteBase::new(src),
            alloc,
            blitter: None,
            clip_shader,
        }
    }
}

impl<'a> SpriteBlitter<'a> for RasterPipelineSpriteBlitter<'a> {
    fn kind(&self) -> SpriteKind {
        SpriteKind::RasterPipeline
    }

    // Port of: src/core/SkBlitter_Sprite.cpp#L126-L158 (chrome/m156)
    fn setup(&mut self, dst: Pixmap<'a>, left: i32, top: i32, paint: &Paint) -> bool {
        self.base.left = left;
        self.base.top = top;
        let paint_color: Color4f = paint.color4f();

        let mut p = RasterPipeline::new();
        let source = &self.base.source;
        p.append_load(source.color_type(), MemoryCtx::new(SOURCE));

        let alpha_only = color_type_is_alpha_only(source.color_type());
        if alpha_only {
            // The color for A8 images comes from the (sRGB) paint color.
            p.append_set_rgb_color4f(self.alloc, &paint_color);
            p.append(Stage::Premul);
        }
        if let Some(dst_cs) = dst.color_space() {
            let src_cs = source.color_space();
            // We treat untagged images as sRGB. Alpha-only images get their r,g,b from the paint
            // color, so they're also sRGB.
            let src_cs = match (&src_cs, alpha_only) {
                (Some(cs), false) => cs,
                _ => srgb_singleton(),
            };
            let src_at = if source.is_opaque() {
                AlphaType::Opaque
            } else {
                AlphaType::Premul
            };
            ColorSpaceXformSteps::new(Some(src_cs), src_at, Some(&dst_cs), AlphaType::Premul)
                .apply_to_pipeline(&mut p, self.alloc);
        }
        #[allow(clippy::float_cmp)] // Skia compares the alpha with 1 exactly
        if paint_color.a != 1.0 {
            p.append(Stage::Scale1Float(
                self.alloc.make(Cell::new(paint_color.a)),
            ));
        }

        #[allow(clippy::float_cmp)] // Skia compares the alpha with 1 exactly
        let is_opaque = source.is_opaque() && paint_color.a == 1.0;
        self.blitter = create_raster_pipeline_blitter_with_pipeline(
            dst,
            paint,
            &p,
            is_opaque,
            self.alloc,
            self.clip_shader.as_ref(),
        );
        self.blitter.is_some()
    }
}

impl Blitter for RasterPipelineSpriteBlitter<'_> {
    sprite_blitter_fallbacks!();

    // Port of: src/core/SkBlitter_Sprite.cpp#L160-L171 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let base = &self.base;
        let stride = isize::try_from(base.source.row_bytes_as_pixels())
            .expect("the source's stride fits in isize");

        // We really want fSrcPtr.pixels = fSource.addr(-fLeft, -fTop) here, but that asserts.
        // The memory view takes the origin (the address of the source's pixel for device pixel
        // (0, 0)) as an offset into the pixels, which may be outside them.
        let bpp = isize::try_from(base.source.info().bytes_per_pixel())
            .expect("bytes per pixel fits in isize");
        let row_bytes = isize::try_from(base.source.row_bytes()).expect("row bytes fit in isize");
        let origin = -(bpp * isize::try_from(base.left).expect("left fits in isize"))
            - row_bytes * isize::try_from(base.top).expect("top fits in isize");
        let source = MemView::read(base.source.bytes().expect("the source has pixels"))
            .with_stride(stride)
            .with_origin(origin);

        self.blitter
            .as_mut()
            .expect("setup succeeded")
            .blit_rect_with_source((x, y, width, height), source);
    }
}

/// Source-over of `kN32` pixels onto `kN32` pixels, with a global alpha
/// (`Sprite_D32_S32`).
// Port of: src/core/SkSpriteBlitter_ARGB32.cpp#L21-L61 (chrome/m156)
#[derive(Debug)]
struct SpriteD32S32<'a> {
    base: SpriteBase<'a>,
    proc32: Proc32,
    alpha: u32,
    scratch: Vec<u32>,
    src_row: Vec<u32>,
}

impl<'a> SpriteD32S32<'a> {
    // Port of: src/core/SkSpriteBlitter_ARGB32.cpp#L23-L35 (chrome/m156)
    fn new(src: Pixmap<'a>, alpha: u32) -> Self {
        debug_assert_eq!(src.color_type(), ColorType::N32);

        let mut flags32 = 0;
        if 255 != alpha {
            flags32 |= GLOBAL_ALPHA_FLAG32;
        }
        if !src.is_opaque() {
            flags32 |= SRC_PIXEL_ALPHA_FLAG32;
        }

        SpriteD32S32 {
            base: SpriteBase::new(src),
            proc32: factory32(flags32),
            alpha,
            scratch: Vec::new(),
            src_row: Vec::new(),
        }
    }
}

impl<'a> SpriteBlitter<'a> for SpriteD32S32<'a> {
    fn kind(&self) -> SpriteKind {
        SpriteKind::D32S32
    }

    fn setup(&mut self, dst: Pixmap<'a>, left: i32, top: i32, _paint: &Paint) -> bool {
        self.base.setup(dst, left, top)
    }
}

impl Blitter for SpriteD32S32<'_> {
    sprite_blitter_fallbacks!();

    // Port of: src/core/SkSpriteBlitter_ARGB32.cpp#L37-L53 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        debug_assert!(width > 0 && height > 0);
        let base = &mut self.base;
        let src_rb = base.source.row_bytes();
        let proc32 = self.proc32;
        let alpha = self.alpha;
        let width = to_len(width);
        let mut y = y;
        let mut src_off = offset_of(&base.source, x - base.left, y - base.top);
        let src_bytes = base.source.bytes().expect("the source has pixels");

        let mut height = height;
        loop {
            load_u32s(src_bytes, src_off, width, &mut self.src_row);
            let src_row = &self.src_row;
            with_span32(&mut base.dst, &mut self.scratch, x, y, width, |dst| {
                proc32(dst, src_row, alpha);
            });
            y += 1;
            src_off += src_rb;
            height -= 1;
            if height == 0 {
                break;
            }
        }
    }
}

/// Chooses a source-over `kN32` sprite blitter for `source` and `paint` (`ChooseL32`), or gives
/// `source` back if there is none: the paint has a color filter or mask filter, the source is
/// not `kN32`, or the blend mode is not source-over.
///
/// Source-over blitters handle a paint alpha but not other blend modes.
// Port of: src/core/SkSpriteBlitter_ARGB32.cpp#L63-L79 (chrome/m156)
#[doc(alias = "ChooseL32")]
fn choose_l32<'a>(
    source: Pixmap<'a>,
    paint: &Paint,
) -> Result<Box<dyn SpriteBlitter<'a> + 'a>, Pixmap<'a>> {
    if paint.color_filter().is_some() {
        return Err(source);
    }
    if paint.mask_filter().is_some() {
        return Err(source);
    }
    if is_n32(source.color_type()) && paint.is_src_over() {
        // this can handle alpha, but not xfermode
        return Ok(Box::new(SpriteD32S32::new(
            source,
            u32::from(paint.alpha()),
        )));
    }
    Err(source)
}

/// `SkBlitter::ChooseSprite`: a blitter that draws `source`, with its top left corner at
/// `(left, top)`, onto `dst` according to `paint`. Returns `None` if the caller must use
/// [`choose`](crate::blitter_choose::choose) with the source wrapped in a shader.
///
/// Antialiasing and the filter quality are ignored: a sprite has no scale, and soft edges would
/// need the fractional `left` and `top`. A raster pipeline blitter (where the legacy ones cannot
/// draw the paint) allocates its contexts in `alloc`, which must outlive the blitter.
///
/// `force_raster_pipeline_blitter` stands for `gSkForceRasterPipelineBlitter`.
///
/// # Panics
/// Never in practice; the `expect`s hold invariants of this function.
// Port of: src/core/SkBlitter_Sprite.cpp#L180-L241 (chrome/m156)
#[doc(alias = "ChooseSprite")]
#[must_use]
#[allow(clippy::too_many_arguments)] // Skia's signature, plus the force flag
pub fn choose_sprite<'a>(
    dst: Pixmap<'a>,
    paint: &Paint,
    source: Pixmap<'a>,
    left: i32,
    top: i32,
    alloc: &'a ArenaAlloc,
    clip_shader: Option<&Shader>,
    force_raster_pipeline_blitter: bool,
) -> Option<Box<dyn Blitter + 'a>> {
    choose_sprite_kind(
        dst,
        paint,
        source,
        left,
        top,
        alloc,
        clip_shader,
        force_raster_pipeline_blitter,
    )
    .map(|(_, blitter)| blitter)
}

/// [`choose_sprite`], also telling which sprite blitter it made.
#[allow(clippy::too_many_arguments)] // Skia's signature, plus the force flag
pub(crate) fn choose_sprite_kind<'a>(
    dst: Pixmap<'a>,
    paint: &Paint,
    source: Pixmap<'a>,
    left: i32,
    top: i32,
    alloc: &'a ArenaAlloc,
    clip_shader: Option<&Shader>,
    force_raster_pipeline_blitter: bool,
) -> Option<(SpriteKind, Box<dyn Blitter + 'a>)> {
    // We currently ignore antialiasing and filtertype, meaning we will take our special blitters
    // regardless of these settings. Ignoring filtertype seems fine since by definition there is
    // no scale in the matrix. Ignoring antialiasing is a bit of a hack, since we "could" pass in
    // the fractional left/top for the bitmap, and respect that by blending the edges of the
    // bitmap against the device. To support this we could either add more special blitters here,
    // or detect antialiasing in the paint and return null if it is set, forcing the client to
    // take the slow shader case (which does respect soft edges).

    // TODO: in principle SkRasterPipelineSpriteBlitter could be made to handle this.
    if source.alpha_type() == AlphaType::Unpremul {
        return None;
    }

    let mut blitter: Option<Box<dyn SpriteBlitter<'a> + 'a>> = None;
    let mut source = Some(source);

    if force_raster_pipeline_blitter {
        // Do not use any of these optimized memory blitters
    } else if 0 == xform_flags_mask(source.as_ref().expect("source"), &dst) && clip_shader.is_none()
    {
        if blitter.is_none()
            && SpriteMemcpy::supports(&dst, source.as_ref().expect("source"), paint)
        {
            blitter = Some(Box::new(SpriteMemcpy::new(source.take().expect("source"))));
        }
        if blitter.is_none() {
            // The C++ switch on the color type has only `kN32`.
            if is_n32(dst.color_type()) {
                match choose_l32(source.take().expect("source"), paint) {
                    Ok(b) => blitter = Some(b),
                    Err(s) => source = Some(s),
                }
            }
        }
    }
    if blitter.is_none() && paint.mask_filter().is_none() {
        blitter = Some(Box::new(RasterPipelineSpriteBlitter::new(
            source.take().expect("source"),
            alloc,
            clip_shader.cloned(),
        )));
    }

    let mut blitter = blitter?;
    let kind = blitter.kind();
    if blitter.setup(dst, left, top, paint) {
        let b: Box<dyn Blitter + 'a> = blitter;
        return Some((kind, b));
    }

    None
}

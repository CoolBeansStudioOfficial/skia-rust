// Copyright 2006 The Android Open Source Project
// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlitter_A8.h, src/core/SkBlitter_A8.cpp

//! The A8 blitters: [`A8CoverageBlitter`] (draws coverage) and `SkA8_Blitter` (draws a solid
//! alpha into an `kAlpha_8` device with source-over or source), and the functions that choose
//! between them ([`choose_a8_blitter`], [`a8_blitter_choose`]).

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Alpha;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::shader::Shader;

use crate::blitter::{BlitMemory, Blitter, blit_mask_default};
use crate::pixel_rows::bytes_mut;

// The byte offset of the 8-bit pixel `(x, y)`.
fn offset8(pm: &Pixmap<'_>, x: i32, y: i32) -> usize {
    debug_assert!(x >= 0 && y >= 0, "({x}, {y}) is outside the pixmap");
    let x = usize::try_from(x).expect("pixel x is not negative");
    let y = usize::try_from(y).expect("pixel y is not negative");
    y * pm.row_bytes() + x
}

// A non-negative `int` as a length.
fn to_len(n: i32) -> usize {
    usize::try_from(n).expect("blit sizes are not negative")
}

/// A blitter for `kAlpha_8` devices that draws coverage (`SkA8_Coverage_Blitter`): what it is
/// asked to blit is stored, not blended.
// Port of: src/core/SkBlitter_A8.h#L28-L39 (chrome/m156)
#[doc(alias = "SkA8_Coverage_Blitter")]
#[derive(Debug)]
pub struct A8CoverageBlitter<'a> {
    device: Pixmap<'a>,
    memory: BlitMemory,
}

impl<'a> A8CoverageBlitter<'a> {
    /// A coverage blitter for `device`; `paint` must have no shader and no color filter.
    // Port of: src/core/SkBlitter_A8.cpp#L25-L30 (chrome/m156)
    #[must_use]
    pub fn new(device: Pixmap<'a>, paint: &Paint) -> Self {
        debug_assert!(paint.shader().is_none());
        debug_assert!(paint.color_filter().is_none());
        A8CoverageBlitter {
            device,
            memory: BlitMemory::default(),
        }
    }
}

impl Blitter for A8CoverageBlitter<'_> {
    // Port of: src/core/SkBlitter_A8.cpp#L53-L55 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        let off = offset8(&self.device, x, y);
        bytes_mut(&mut self.device)[off..off + to_len(width)].fill(0xFF);
    }

    // Port of: src/core/SkBlitter_A8.cpp#L32-L51 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        let mut off = offset8(&self.device, x, y);
        let bytes = bytes_mut(&mut self.device);
        let mut i = 0usize;

        loop {
            let count = i32::from(runs[i]);
            debug_assert!(count >= 0);
            if count == 0 {
                return;
            }
            let count = to_len(count);
            if antialias[i] != 0 {
                bytes[off..off + count].fill(antialias[i]);
            }
            i += count;
            off += count;
        }
    }

    // Port of: src/core/SkBlitter_A8.cpp#L57-L68 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        if 0 == alpha {
            return;
        }

        let mut dst = offset8(&self.device, x, y);
        let dst_rb = self.device.row_bytes();
        let bytes = bytes_mut(&mut self.device);
        let mut height = height;
        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            bytes[dst] = alpha;
            dst += dst_rb;
        }
    }

    // Port of: src/core/SkBlitter_A8.cpp#L70-L78 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let mut dst = offset8(&self.device, x, y);
        let dst_rb = self.device.row_bytes();
        let bytes = bytes_mut(&mut self.device);
        let mut height = height;
        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            bytes[dst..dst + to_len(width)].fill(0xFF);
            dst += dst_rb;
        }
    }

    // Port of: src/core/SkBlitter_A8.cpp#L80-L102 (chrome/m156)
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        if MaskFormat::A8 != mask.format {
            blit_mask_default(self, mask, clip);
            return;
        }

        let x = clip.left;
        let y = clip.top;
        let width = to_len(clip.width());
        let mut height = clip.height();

        let mut dst = offset8(&self.device, x, y);
        let src = mask.get_addr8(x, y);
        let mut src_off = 0usize;
        let src_rb = mask.row_bytes as usize;
        let dst_rb = self.device.row_bytes();
        let bytes = bytes_mut(&mut self.device);

        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            bytes[dst..dst + width].copy_from_slice(&src[src_off..src_off + width]);
            dst += dst_rb;
            src_off += src_rb;
        }
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}

// The blend modes the A8 blitter supports (`gA8_RowBlitPairs`), with their procs.
// Port of: src/core/SkBlitter_A8.cpp#L106-L158 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum A8Mode {
    /// `srcover_p`, which can fold the AA into the source.
    SrcOver,
    /// `src_p`, which cannot.
    Src,
}

impl A8Mode {
    // `find_a8_rowproc_pair`.
    fn find(bm: BlendMode) -> Option<A8Mode> {
        match bm {
            BlendMode::SrcOver => Some(A8Mode::SrcOver),
            BlendMode::Src => Some(A8Mode::Src),
            _ => None,
        }
    }

    // `oneProc`.
    #[allow(clippy::cast_possible_truncation)] // the C++ returns the sum as uint8_t
    fn one(self, src: u8, dst: u8) -> u8 {
        match self {
            // static uint8_t srcover_p(uint8_t src, uint8_t dst) { return src + div255((255 - src) * dst); }
            A8Mode::SrcOver => {
                (u32::from(src) + div255((255 - u32::from(src)) * u32::from(dst))) as u8
            }
            A8Mode::Src => src,
        }
    }

    // `bwProc`: `A8_row_bw`.
    fn row_bw(self, dst: &mut [u8], src: u8) {
        for d in dst {
            *d = self.one(src, *d);
        }
    }

    // `aaProc`: `A8_row_aa`, with `canFoldAA` true for source-over only.
    #[allow(clippy::cast_possible_truncation)] // the C++ stores the lerp as uint8_t
    fn row_aa(self, dst: &mut [u8], src: u8, aa: u8) {
        let can_fold_aa = self == A8Mode::SrcOver;
        if can_fold_aa {
            let src = div255(u32::from(src) * u32::from(aa)) as u8;
            for d in dst {
                *d = self.one(src, *d);
            }
        } else {
            for d in dst {
                *d = u8_lerp(*d, self.one(src, *d), aa) as u8;
            }
        }
    }
}

// Port of: src/core/SkBlitter_A8.cpp#L104-L112 (chrome/m156)
fn div255(prod: u32) -> u32 {
    debug_assert!(prod <= 255 * 255);
    ((prod + 128) * 257) >> 16
}

// Port of: src/core/SkBlitter_A8.cpp#L114-L116 (chrome/m156)
fn u8_lerp(a: u8, b: u8, t: u8) -> u32 {
    div255((255 - u32::from(t)) * u32::from(a) + u32::from(t) * u32::from(b))
}

/// A blitter for `kAlpha_8` devices that draws a solid alpha with source or source-over
/// (`SkA8_Blitter`).
// Port of: src/core/SkBlitter_A8.cpp#L160-L180 (chrome/m156)
#[doc(alias = "SkA8_Blitter")]
#[derive(Debug)]
struct A8Blitter<'a> {
    device: Pixmap<'a>,
    mode: A8Mode,
    src: Alpha,
    memory: BlitMemory,
}

impl<'a> A8Blitter<'a> {
    // Port of: src/core/SkBlitter_A8.cpp#L182-L194 (chrome/m156)
    fn new(device: Pixmap<'a>, paint: &Paint) -> Self {
        debug_assert!(paint.shader().is_none());
        debug_assert!(paint.color_filter().is_none());
        let mode = paint.as_blend_mode().expect("the paint has a blend mode");
        let mode = A8Mode::find(mode).expect("the blend mode is supported");

        A8Blitter {
            device,
            mode,
            src: paint.alpha(),
            memory: BlitMemory::default(),
        }
    }
}

impl Blitter for A8Blitter<'_> {
    // Port of: src/core/SkBlitter_A8.cpp#L284-L313 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        let off = offset8(&self.device, x, y);
        let (mode, src) = (self.mode, self.src);
        mode.row_bw(
            &mut bytes_mut(&mut self.device)[off..off + to_len(width)],
            src,
        );
    }

    // Port of: src/core/SkBlitter_A8.cpp#L196-L222 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        let mut off = offset8(&self.device, x, y);
        let (mode, src) = (self.mode, self.src);
        let bytes = bytes_mut(&mut self.device);
        let mut i = 0usize;

        loop {
            let count = i32::from(runs[i]);
            debug_assert!(count >= 0);
            if count == 0 {
                return;
            }
            let count = to_len(count);

            if antialias[i] == 0xFF {
                mode.row_bw(&mut bytes[off..off + count], src);
            } else if antialias[i] != 0 {
                mode.row_aa(&mut bytes[off..off + count], src, antialias[i]);
            }

            i += count;
            off += count;
        }
    }

    // Port of: src/core/SkBlitter_A8.cpp#L228-L246 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, aa: Alpha) {
        let mut device = offset8(&self.device, x, y);
        let dst_rb = self.device.row_bytes();
        let (mode, src) = (self.mode, self.src);
        let bytes = bytes_mut(&mut self.device);
        let mut height = height;

        if aa == 0xFF {
            loop {
                height -= 1;
                if height < 0 {
                    break;
                }
                bytes[device] = mode.one(src, bytes[device]);
                device += dst_rb;
            }
        } else if aa != 0 {
            loop {
                height -= 1;
                if height < 0 {
                    break;
                }
                mode.row_aa(&mut bytes[device..=device], src, aa);
                device += dst_rb;
            }
        }
    }

    // Port of: src/core/SkBlitter_A8.cpp#L248-L257 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let mut device = offset8(&self.device, x, y);
        let dst_rb = self.device.row_bytes();
        let (mode, src) = (self.mode, self.src);
        let bytes = bytes_mut(&mut self.device);
        let mut height = height;

        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            mode.row_bw(&mut bytes[device..device + to_len(width)], src);
            device += dst_rb;
        }
    }

    // Port of: src/core/SkBlitter_A8.cpp#L259-L282 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // the C++ stores the lerp as uint8_t
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        if MaskFormat::A8 != mask.format {
            blit_mask_default(self, mask, clip);
            return;
        }

        let x = clip.left;
        let y = clip.top;
        let width = to_len(clip.width());
        let mut height = clip.height();

        let mut dst = offset8(&self.device, x, y);
        let src = mask.get_addr8(x, y);
        let mut src_off = 0usize;
        let src_rb = mask.row_bytes as usize;
        let dst_rb = self.device.row_bytes();
        let (mode, fsrc) = (self.mode, self.src);
        let bytes = bytes_mut(&mut self.device);

        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            for i in 0..width {
                bytes[dst + i] = u8_lerp(
                    bytes[dst + i],
                    mode.one(fsrc, bytes[dst + i]),
                    src[src_off + i],
                ) as u8;
            }
            dst += dst_rb;
            src_off += src_rb;
        }
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}

/// Chooses the A8 blitter for `dst` and `paint`, or `None` if `dst` is not `kAlpha_8` or the
/// paint cannot be drawn by one (`SkChooseA8Blitter`). Draws coverage with `draw_coverage`;
/// otherwise only the blend modes source and source-over are supported.
// Port of: src/core/SkBlitter_A8.cpp#L292-L315 (chrome/m156)
#[doc(alias = "SkChooseA8Blitter")]
#[must_use]
pub fn choose_a8_blitter<'a>(
    dst: Pixmap<'a>,
    _ctm: &Matrix,
    paint: &Paint,
    draw_coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
) -> Option<Box<dyn Blitter + 'a>> {
    if dst.color_type() != ColorType::Alpha8 {
        return None;
    }
    if paint.shader().is_some() || paint.color_filter().is_some() {
        return None;
    }
    if clip_shader.is_some() {
        return None; // would not be hard to support ...?
    }

    if draw_coverage == DrawCoverage::Yes {
        Some(Box::new(A8CoverageBlitter::new(dst, paint)))
    } else {
        // we only support certain blendmodes...
        let mode = paint.as_blend_mode();
        if mode.and_then(A8Mode::find).is_some() {
            return Some(Box::new(A8Blitter::new(dst, paint)));
        }
        None
    }
}

/// [`choose_a8_blitter`] with the signature of [`Blitter` choosers](crate::blitter_choose::choose)
/// (`SkA8Blitter_Choose`): the surface properties and device bounds are ignored.
// Port of: src/core/SkBlitter_A8.cpp#L317-L324 (chrome/m156)
#[doc(alias = "SkA8Blitter_Choose")]
#[must_use]
pub fn a8_blitter_choose<'a>(
    dst: Pixmap<'a>,
    ctm: &Matrix,
    paint: &Paint,
    coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    _dev_bounds: &Rect,
) -> Option<Box<dyn Blitter + 'a>> {
    choose_a8_blitter(dst, ctm, paint, coverage, clip_shader)
}

// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCoreBlitters.h, src/core/SkBlitter_ARGB32.cpp,
// src/core/SkBlitBWMaskTemplate.h, SkBlitter.cpp (`SkShaderBlitter`)

//! The legacy 32-bit blitters: [`Argb32Blitter`] (translucent solid color),
//! [`Argb32OpaqueBlitter`], [`Argb32BlackBlitter`] and [`Argb32ShaderBlitter`] (a legacy shader
//! context).
//!
//! `SkBlitter::Choose` picks them for `kN32` devices drawing source-over with no dither (see
//! [`blitter_choose`](crate::blitter_choose)). Their pixel work is done by the `SkBlitRow` procs
//! and `SkOpts` kernels ([`blit_row`](crate::blit_row), `skia-rust-simd`), so it follows the
//! current CPU tier.
//!
//! # LCD16 masks
//! The three `blit_row_lcd16*` functions are `SkBlitter_ARGB32.cpp`'s portable per-pixel code.
//! Skia replaces them with SSE2/NEON/LASX versions chosen at compile time. They compute the same
//! color channels; the alpha channel differs in the SIMD versions when the destination is not
//! opaque (the SSE2 version compares the 16-bit-replicated source alpha with the destination's
//! lowest byte, so it always takes the max coverage, and which pixels it handles depends on the
//! destination's 16-byte alignment; the NEON version takes the min when `srcA <= dstA` where the
//! portable code takes it when `srcA < dstA`). LCD blitting is only meant for opaque
//! destinations, where the alpha stays `0xFF` in every version, so the portable code is the
//! port.

use skia_rust_core::color::{Alpha, Color, PMColor, pre_multiply_color};
use skia_rust_core::color_data::{
    B16_BITS, G16_BITS, R16_BITS, blend_argb32, fast_four_byte_interp, four_byte_interp,
    get_packed_b16, get_packed_g16, get_packed_r16,
};
use skia_rust_core::color_priv::{
    A32_MASK, A32_SHIFT, alpha_255_to_256, alpha_mul_q, get_packed_a32, get_packed_b32,
    get_packed_g32, get_packed_r32, pack_argb32,
};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::IRect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{OPAQUE_ALPHA_FLAG, ShaderContext};
use skia_rust_simd::blit_mask::blit_mask_d32_a8;
use skia_rust_simd::memset::{memset32, rect_memset32};

use crate::blit_row::{GLOBAL_ALPHA_FLAG32, Proc32, SRC_PIXEL_ALPHA_FLAG32, color32, factory32};
use crate::blitter::{BlitMemory, Blitter, DirectBlit, blit_mask_default};
use crate::pixel_rows::{
    bytes_mut, load_u32s, offset32, read32, with_rows32, with_span32, write32,
};

// Port of: src/core/SkBlitter_ARGB32.cpp#L33-L36 (chrome/m156)
fn upscale_31_to_32(value: i32) -> i32 {
    debug_assert!((0..=31).contains(&value));
    value + (value >> 4)
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L38-L42 (chrome/m156)
fn blend_32(src: i32, dst: i32, scale: i32) -> i32 {
    debug_assert!((0..=0xFF).contains(&src));
    debug_assert!((0..=0xFF).contains(&dst));
    debug_assert!((0..=32).contains(&scale));
    dst + (((src - dst) * scale) >> 5)
}

// Converts a blend result, which is in `0..=255`, to a color component.
#[allow(clippy::cast_sign_loss)] // blend_32 results are non-negative
fn component(v: i32) -> u32 {
    debug_assert!((0..=0xFF).contains(&v));
    v as u32
}

// The components of a packed pixel as `int`s.
#[allow(clippy::cast_possible_wrap)] // components are at most 255
fn unpack(c: PMColor) -> (i32, i32, i32, i32) {
    (
        get_packed_a32(c) as i32,
        get_packed_r32(c) as i32,
        get_packed_g32(c) as i32,
        get_packed_b32(c) as i32,
    )
}

// The 5-bit LCD coverages of `mask`, upscaled to `0..=32`.
#[allow(clippy::cast_possible_wrap)] // 5-bit values
fn lcd_masks_5(mask: u16) -> (i32, i32, i32) {
    let mask = u32::from(mask);
    // We want all of these in 5bits, hence the shifts in case one of them (green) is 6bits.
    let mask_r = (get_packed_r16(mask) >> (R16_BITS - 5)) as i32;
    let mask_g = (get_packed_g16(mask) >> (G16_BITS - 5)) as i32;
    let mask_b = (get_packed_b16(mask) >> (B16_BITS - 5)) as i32;

    // Now upscale them to 0..32, so we can use blend32
    (
        upscale_31_to_32(mask_r),
        upscale_31_to_32(mask_g),
        upscale_31_to_32(mask_b),
    )
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L44-L85 (chrome/m156)
fn blend_lcd16(src_a: i32, src_r: i32, src_g: i32, src_b: i32, dst: PMColor, mask: u16) -> PMColor {
    if mask == 0 {
        return dst;
    }

    let (mut mask_r, mut mask_g, mut mask_b) = lcd_masks_5(mask);

    // srcA has been upscaled to 256 before passed into this function
    mask_r = (mask_r * src_a) >> 8;
    mask_g = (mask_g * src_a) >> 8;
    mask_b = (mask_b * src_a) >> 8;

    let (dst_a, dst_r, dst_g, dst_b) = unpack(dst);

    // Subtract 1 from srcA to bring it back to [0-255] to compare against dstA, alpha needs to
    // use either the min or the max of the LCD coverages. See https:/skbug.com/40037823
    let mask_a = if (src_a - 1) < dst_a {
        mask_r.min(mask_g.min(mask_b))
    } else {
        mask_r.max(mask_g.max(mask_b))
    };

    pack_argb32(
        component(blend_32(0xFF, dst_a, mask_a)),
        component(blend_32(src_r, dst_r, mask_r)),
        component(blend_32(src_g, dst_g, mask_g)),
        component(blend_32(src_b, dst_b, mask_b)),
    )
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L87-L124 (chrome/m156)
fn blend_lcd16_opaque(
    src_r: i32,
    src_g: i32,
    src_b: i32,
    dst: PMColor,
    mask: u16,
    opaque_dst: PMColor,
) -> PMColor {
    if mask == 0 {
        return dst;
    }

    if 0xFFFF == mask {
        return opaque_dst;
    }

    let (mask_r, mask_g, mask_b) = lcd_masks_5(mask);

    let (dst_a, dst_r, dst_g, dst_b) = unpack(dst);

    // Opaque src alpha always uses the max of the LCD coverages.
    let mask_a = mask_r.max(mask_g.max(mask_b));

    // LCD blitting is only supported if the dst is known/required to be opaque
    pack_argb32(
        component(blend_32(0xFF, dst_a, mask_a)),
        component(blend_32(src_r, dst_r, mask_r)),
        component(blend_32(src_g, dst_g, mask_g)),
        component(blend_32(src_b, dst_b, mask_b)),
    )
}

// The 16-bit mask value at index `i` of a row of LCD16 mask bytes.
fn lcd_at(mask: &[u8], i: usize) -> u16 {
    u16::from_ne_bytes([mask[2 * i], mask[2 * i + 1]])
}

// The components of an `SkColor` as `int`s.
fn color_components(src: Color) -> (i32, i32, i32, i32) {
    (
        i32::from(src.a()),
        i32::from(src.r()),
        i32::from(src.g()),
        i32::from(src.b()),
    )
}

/// `blit_row_lcd16` (the portable version): blends `src` through the LCD16 `mask` row onto
/// `dst`. `opaque_dst` is ignored.
// Port of: src/core/SkBlitter_ARGB32.cpp#L1356-L1368 (chrome/m156)
fn blit_row_lcd16(dst: &mut [u32], mask: &[u8], src: Color, _opaque_dst: PMColor) {
    let (src_a, src_r, src_g, src_b) = color_components(src);

    let src_a = alpha_255_to_256_i32(src_a);

    for (i, d) in dst.iter_mut().enumerate() {
        *d = blend_lcd16(src_a, src_r, src_g, src_b, *d, lcd_at(mask, i));
    }
}

// `SkAlpha255To256` of a value that is known to be a byte.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // a byte
fn alpha_255_to_256_i32(a: i32) -> i32 {
    alpha_255_to_256(a as u32) as i32
}

/// `blit_row_lcd16_opaque` (the portable version): the same for an opaque `src`.
// Port of: src/core/SkBlitter_ARGB32.cpp#L1370-L1380 (chrome/m156)
fn blit_row_lcd16_opaque(dst: &mut [u32], mask: &[u8], src: Color, opaque_dst: PMColor) {
    let (_, src_r, src_g, src_b) = color_components(src);

    for (i, d) in dst.iter_mut().enumerate() {
        *d = blend_lcd16_opaque(src_r, src_g, src_b, *d, lcd_at(mask, i), opaque_dst);
    }
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1384-L1420 (chrome/m156)
fn blit_color(
    device: &mut Pixmap<'_>,
    scratch: &mut Vec<u32>,
    mask: &Mask<'_>,
    clip: &IRect,
    color: Color,
) -> bool {
    let x = clip.left;
    let y = clip.top;
    let width = to_len(clip.width());
    let height = to_len(clip.height());

    if device.color_type() == ColorType::N32 && mask.format == MaskFormat::A8 {
        let mask_bytes = mask.get_addr(x, y);
        with_rows32(device, scratch, x, y, width, height, |dst, dst_rb| {
            blit_mask_d32_a8(
                dst,
                dst_rb,
                mask_bytes,
                mask.row_bytes as usize,
                color.into(),
                width,
                height,
            );
        });
        return true;
    }

    if device.color_type() == ColorType::N32 && mask.format == MaskFormat::Lcd16 {
        let mask_bytes = mask.get_addr(x, y);

        let mut blit_row: fn(&mut [u32], &[u8], Color, PMColor) = blit_row_lcd16;
        let mut opaque_dst: PMColor = 0; // ignored unless opaque

        if 0xff == color.a() {
            blit_row = blit_row_lcd16_opaque;
            opaque_dst = pre_multiply_color(color);
        }

        for row in 0..height {
            let mask_row = &mask_bytes[row * mask.row_bytes as usize..];
            let row_y = y + i32::try_from(row).expect("the clip height fits in i32");
            with_span32(device, scratch, x, row_y, width, |dst| {
                blit_row(dst, mask_row, color, opaque_dst);
            });
        }
        return true;
    }

    false
}

// A non-negative `int` as a length (a width or height of a non-empty clip).
fn to_len(n: i32) -> usize {
    usize::try_from(n).expect("a clip's width and height are not negative")
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1424-L1446 (chrome/m156)
fn argb32_blit32(
    device: &mut Pixmap<'_>,
    scratch: &mut Vec<u32>,
    mask: &Mask<'_>,
    clip: &IRect,
    src_color: PMColor,
) {
    let alpha = get_packed_a32(src_color);
    let mut flags = SRC_PIXEL_ALPHA_FLAG32;
    if alpha != 255 {
        flags |= GLOBAL_ALPHA_FLAG32;
    }
    let proc: Proc32 = factory32(flags);

    let x = clip.left;
    let mut y = clip.top;
    let width = to_len(clip.width());
    let mut height = clip.height();

    let src = mask.get_addr32(x, y);
    let mut src_row = Vec::new();
    let mut row = 0usize;

    loop {
        load_u32s(src, row * mask.row_bytes as usize, width, &mut src_row);
        with_span32(device, scratch, x, y, width, |dst| {
            proc(dst, &src_row, alpha);
        });
        row += 1;
        y += 1;
        height -= 1;
        if height <= 0 {
            break;
        }
    }
}

/// `SkBlitBWMaskTemplate.h`: blits a 1-bit mask by calling `blit8(mask, device, x, y)` for each
/// byte of mask bits, where `x` is the device column of the byte's first bit (it can be left of
/// the clip: bits outside the clip are masked off). The C++ is a macro that is instantiated per
/// device pixel type and `BLIT8` body.
// Port of: src/core/SkBlitBWMaskTemplate.h#L33-L140 (chrome/m156)
fn blit_bw_mask(
    device: &mut Pixmap<'_>,
    src_mask: &Mask<'_>,
    clip: &IRect,
    mut blit8: impl FnMut(&mut Pixmap<'_>, u32, i32, i32),
) {
    debug_assert!(clip.right <= src_mask.bounds.right);

    let cx = clip.left;
    let mut y = clip.top;
    let mask_left = src_mask.bounds.left;
    let mask_row_bytes = src_mask.row_bytes as usize;
    #[allow(clippy::cast_sign_loss)] // the clip is not empty
    let mut height = clip.height() as u32;

    debug_assert!(mask_row_bytes != 0);
    debug_assert!(device.row_bytes() != 0);
    debug_assert!(height != 0);

    let bits = src_mask.get_addr1(cx, y);
    // The index of the next byte of mask bits.
    let mut b = 0usize;

    if cx == mask_left && clip.right == src_mask.bounds.right {
        loop {
            let mut dst_x = cx;
            #[allow(clippy::cast_possible_truncation)] // mirrors `unsigned mask_rowBytes`
            let mut rb = mask_row_bytes as u32;
            loop {
                let mask = u32::from(bits[b]);
                b += 1;
                blit8(device, mask, dst_x, y);
                dst_x += 8;
                rb -= 1;
                if rb == 0 {
                    break;
                }
            }
            y += 1;
            height -= 1;
            if height == 0 {
                break;
            }
        }
    } else {
        let left_edge = cx - mask_left;
        debug_assert!(left_edge >= 0);
        let rite_edge = clip.right - mask_left;
        debug_assert!(rite_edge > left_edge);

        let mut left_mask = 0xFFi32 >> (left_edge & 7);
        let mut rite_mask = 0xFFi32 << (8 - (rite_edge & 7));
        rite_mask &= 0xFF; // only want low-8 bits of mask
        let mut full_runs = (rite_edge >> 3) - ((left_edge + 7) >> 3);

        // check for empty right mask, so we don't read off the end (or go slower than we need to)
        if rite_mask == 0 {
            debug_assert!(full_runs >= 0);
            full_runs -= 1;
            rite_mask = 0xFF;
        }
        if left_mask == 0xFF {
            full_runs -= 1;
        }

        // back up manually so we can keep in sync with our byte-aligned src and not trigger an
        // assert from the getAddr## function
        let device_x = cx - (left_edge & 7);

        // The index of the first byte of the current row.
        let mut row = 0usize;
        #[allow(clippy::cast_sign_loss)] // masks are in 0..=0xFF
        if full_runs < 0 {
            left_mask &= rite_mask;
            debug_assert!(left_mask != 0);
            loop {
                let mask = u32::from(bits[row]) & left_mask as u32;
                blit8(device, mask, device_x, y);
                row += mask_row_bytes;
                y += 1;
                height -= 1;
                if height == 0 {
                    break;
                }
            }
        } else {
            loop {
                let mut runs = full_runs;
                let mut dst_x = device_x;
                let mut b = row;

                let mut mask = u32::from(bits[b]) & left_mask as u32;
                b += 1;
                blit8(device, mask, dst_x, y);
                dst_x += 8;

                loop {
                    runs -= 1;
                    if runs < 0 {
                        break;
                    }
                    mask = u32::from(bits[b]);
                    b += 1;
                    blit8(device, mask, dst_x, y);
                    dst_x += 8;
                }

                mask = u32::from(bits[b]) & rite_mask as u32;
                blit8(device, mask, dst_x, y);

                row += mask_row_bytes;
                y += 1;
                height -= 1;
                if height == 0 {
                    break;
                }
            }
        }
    }
}

// The pixel `i` (0 is the high bit) of the 8 that `mask` covers, if its bit is set.
fn bit_pixels(mask: u32, x: i32) -> impl Iterator<Item = i32> {
    (0..8).filter_map(move |i| (mask & (0x80 >> i) != 0).then_some(x + i))
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1515-L1532 (chrome/m156)
fn argb32_blit_bw(device: &mut Pixmap<'_>, src_mask: &Mask<'_>, clip: &IRect, color: PMColor) {
    blit_bw_mask(device, src_mask, clip, |dev, mask, x, y| {
        // solid_8_pixels
        for px in bit_pixels(mask, x) {
            dev.set_addr32(px, y, color);
        }
    });
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1534-L1551 (chrome/m156)
fn argb32_blend_bw(
    device: &mut Pixmap<'_>,
    src_mask: &Mask<'_>,
    clip: &IRect,
    sc: u32,
    dst_scale: u32,
) {
    blit_bw_mask(device, src_mask, clip, |dev, mask, x, y| {
        // blend_8_pixels
        for px in bit_pixels(mask, x) {
            let d = dev.addr32(px, y);
            dev.set_addr32(px, y, sc.wrapping_add(alpha_mul_q(d, dst_scale)));
        }
    });
}

// Runs `f` over the runs of a `blitAntiH` call: `f(device_x, count, alpha)` for each run with
// `count > 0` (the C++ loops stop at the first run of 0, or less).
fn for_each_run(x: i32, antialias: &[Alpha], runs: &[i16], mut f: impl FnMut(i32, usize, Alpha)) {
    let mut x = x;
    let mut i = 0usize;
    loop {
        let count = i32::from(runs[i]);
        debug_assert!(count >= 0);
        if count <= 0 {
            return;
        }
        f(x, to_len(count), antialias[i]);
        i += to_len(count);
        x += count;
    }
}

/// A solid color source-over `kN32` blitter, for translucent colors (`SkARGB32_Blitter`).
// Port of: src/core/SkCoreBlitters.h#L58-L72 (chrome/m156)
#[doc(alias = "SkARGB32_Blitter")]
#[derive(Debug)]
pub struct Argb32Blitter<'a> {
    device: Pixmap<'a>,
    color: Color,
    pm_color: PMColor,
    src_a: Alpha,
    scratch: Vec<u32>,
    memory: BlitMemory,
}

impl<'a> Argb32Blitter<'a> {
    /// A blitter that draws `paint`'s color onto `device` (`kN32` pixels).
    // Port of: src/core/SkBlitter_ARGB32.cpp#L1450-L1456 (chrome/m156)
    #[must_use]
    pub fn new(device: Pixmap<'a>, paint: &Paint) -> Self {
        let color = paint.color();
        Argb32Blitter {
            device,
            color,
            pm_color: pre_multiply_color(color),
            src_a: color.a(),
            scratch: Vec::new(),
            memory: BlitMemory::default(),
        }
    }

    // The span `(x, y)..(x + count, y)` of the device, as `u32`s.
    fn with_span(&mut self, x: i32, y: i32, count: usize, f: impl FnOnce(&mut [u32])) {
        with_span32(&mut self.device, &mut self.scratch, x, y, count, f);
    }
}

impl Blitter for Argb32Blitter<'_> {
    // Port of: src/core/SkBlitter_ARGB32.cpp#L1463-L1468 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        debug_assert!(x >= 0 && y >= 0 && x + width <= self.device.width());

        let pm_color = self.pm_color;
        self.with_span(x, y, to_len(width), |device| color32(device, pm_color));
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1470-L1494 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        debug_assert!(self.src_a != 0xFF); // There is an opaque specialization
        if self.src_a == 0 {
            return;
        }

        let pm_color = self.pm_color;
        for_each_run(x, antialias, runs, |x, count, aa| {
            if aa != 0 {
                let sc = alpha_mul_q(pm_color, alpha_255_to_256(u32::from(aa)));
                self.with_span(x, y, count, |device| color32(device, sc));
            }
        });
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1647-L1665 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        if alpha == 0 || self.src_a == 0 {
            return;
        }

        let mut color = self.pm_color;

        if alpha != 255 {
            color = alpha_mul_q(color, alpha_255_to_256(u32::from(alpha)));
        }

        let dst_scale = alpha_255_to_256(255 - get_packed_a32(color));
        let mut y = y;
        let mut height = height;
        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            let device = &mut self.device;
            let off = offset32(device, x, y);
            let bytes = bytes_mut(device);
            let d = read32(bytes, off);
            write32(bytes, off, color.wrapping_add(alpha_mul_q(d, dst_scale)));
            y += 1;
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1667-L1685 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        debug_assert!(
            x >= 0
                && y >= 0
                && x + width <= self.device.width()
                && y + height <= self.device.height()
        );

        if self.src_a == 0 {
            return;
        }

        let pm_color = self.pm_color;
        let (w, h) = (to_len(width), to_len(height));
        if get_packed_a32(pm_color) == 0xFF {
            with_rows32(
                &mut self.device,
                &mut self.scratch,
                x,
                y,
                w,
                h,
                |device, row_bytes| rect_memset32(device, pm_color, w, row_bytes, h),
            );
        } else {
            for row in 0..h {
                let row_y = y + i32::try_from(row).expect("the height fits in i32");
                self.with_span(x, row_y, w, |device| color32(device, pm_color));
            }
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1553-L1575 (chrome/m156)
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        debug_assert!(contains_clip(mask, clip));
        debug_assert!(self.src_a != 0xFF);

        if self.src_a == 0 {
            return;
        }

        if blit_color(&mut self.device, &mut self.scratch, mask, clip, self.color) {
            return;
        }

        match mask.format {
            MaskFormat::BW => argb32_blend_bw(
                &mut self.device,
                mask,
                clip,
                self.pm_color,
                alpha_255_to_256(255 - u32::from(self.src_a)),
            ),
            MaskFormat::Argb32 => {
                argb32_blit32(
                    &mut self.device,
                    &mut self.scratch,
                    mask,
                    clip,
                    self.pm_color,
                );
            }
            _ => panic!("Mask format not handled."),
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1496-L1502 (chrome/m156)
    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let pm_color = self.pm_color;
        let device = &mut self.device;
        let off = offset32(device, x, y);
        let bytes = bytes_mut(device);
        let (d0, d1) = (read32(bytes, off), read32(bytes, off + 4));

        write32(bytes, off, blend_argb32(pm_color, d0, a0));
        write32(bytes, off + 4, blend_argb32(pm_color, d1, a1));
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1504-L1511 (chrome/m156)
    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let pm_color = self.pm_color;
        let device = &mut self.device;
        let (off0, off1) = (offset32(device, x, y), offset32(device, x, y + 1));
        let bytes = bytes_mut(device);

        let d0 = read32(bytes, off0);
        write32(bytes, off0, blend_argb32(pm_color, d0, a0));
        let d1 = read32(bytes, off1);
        write32(bytes, off1, blend_argb32(pm_color, d1, a1));
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}

// `SkASSERT(mask.fBounds.contains(clip))`.
fn contains_clip(mask: &Mask<'_>, clip: &IRect) -> bool {
    mask.bounds.left <= clip.left
        && mask.bounds.top <= clip.top
        && clip.right <= mask.bounds.right
        && clip.bottom <= mask.bounds.bottom
}

/// A solid, opaque color source-over `kN32` blitter (`SkARGB32_Opaque_Blitter`).
// Port of: src/core/SkCoreBlitters.h#L74-L86 (chrome/m156)
#[doc(alias = "SkARGB32_Opaque_Blitter")]
#[derive(Debug)]
pub struct Argb32OpaqueBlitter<'a> {
    base: Argb32Blitter<'a>,
}

impl<'a> Argb32OpaqueBlitter<'a> {
    /// A blitter for `device` and a `paint` whose alpha is `0xFF`.
    // Port of: src/core/SkCoreBlitters.h#L76-L80 (chrome/m156)
    #[must_use]
    pub fn new(device: Pixmap<'a>, paint: &Paint) -> Self {
        debug_assert_eq!(paint.alpha(), 0xFF);
        Argb32OpaqueBlitter {
            base: Argb32Blitter::new(device, paint),
        }
    }
}

impl Blitter for Argb32OpaqueBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.base.blit_h(x, y, width);
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1598-L1622 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        debug_assert_eq!(self.base.src_a, 0xFF);

        let pm_color = self.base.pm_color;
        for_each_run(x, antialias, runs, |x, count, aa| {
            if aa == 255 {
                self.base
                    .with_span(x, y, count, |device| memset32(device, pm_color, count));
            } else if aa > 0 {
                let sc = alpha_mul_q(pm_color, alpha_255_to_256(u32::from(aa)));
                self.base
                    .with_span(x, y, count, |device| color32(device, sc));
            }
        });
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        self.base.blit_v(x, y, height, alpha);
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.base.blit_rect(x, y, width, height);
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1577-L1596 (chrome/m156)
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        debug_assert_eq!(self.base.src_a, 0xFF);
        debug_assert!(contains_clip(mask, clip));

        let base = &mut self.base;
        if blit_color(&mut base.device, &mut base.scratch, mask, clip, base.color) {
            return;
        }

        match mask.format {
            MaskFormat::BW => argb32_blit_bw(&mut base.device, mask, clip, base.pm_color),
            MaskFormat::Argb32 => {
                argb32_blit32(
                    &mut base.device,
                    &mut base.scratch,
                    mask,
                    clip,
                    base.pm_color,
                );
            }
            _ => panic!("Mask format not handled."),
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1624-L1630 (chrome/m156)
    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let pm_color = self.base.pm_color;
        let device = &mut self.base.device;
        let off = offset32(device, x, y);
        let bytes = bytes_mut(device);
        let (d0, d1) = (read32(bytes, off), read32(bytes, off + 4));

        write32(bytes, off, fast_four_byte_interp(pm_color, d0, a0));
        write32(bytes, off + 4, fast_four_byte_interp(pm_color, d1, a1));
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1632-L1639 (chrome/m156)
    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let pm_color = self.base.pm_color;
        let device = &mut self.base.device;
        let (off0, off1) = (offset32(device, x, y), offset32(device, x, y + 1));
        let bytes = bytes_mut(device);

        let d0 = read32(bytes, off0);
        write32(bytes, off0, fast_four_byte_interp(pm_color, d0, a0));
        let d1 = read32(bytes, off1);
        write32(bytes, off1, fast_four_byte_interp(pm_color, d1, a1));
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1641-L1643 (chrome/m156)
    fn can_direct_blit(&mut self) -> Option<DirectBlit<'_>> {
        let value = u64::from(self.base.pm_color);
        Some(DirectBlit {
            pm: self.base.device.reborrow_mut(),
            value,
        })
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        self.base.blit_memory()
    }
}

/// A solid, opaque black source-over `kN32` blitter (`SkARGB32_Black_Blitter`).
// Port of: src/core/SkCoreBlitters.h#L88-L98 (chrome/m156)
#[doc(alias = "SkARGB32_Black_Blitter")]
#[derive(Debug)]
pub struct Argb32BlackBlitter<'a> {
    base: Argb32OpaqueBlitter<'a>,
}

impl<'a> Argb32BlackBlitter<'a> {
    /// A blitter for `device` and a `paint` whose color is `SK_ColorBLACK`.
    // Port of: src/core/SkCoreBlitters.h#L90-L94 (chrome/m156)
    #[must_use]
    pub fn new(device: Pixmap<'a>, paint: &Paint) -> Self {
        debug_assert_eq!(paint.color(), Color::BLACK);
        Argb32BlackBlitter {
            base: Argb32OpaqueBlitter::new(device, paint),
        }
    }
}

impl Blitter for Argb32BlackBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.base.blit_h(x, y, width);
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1693-L1722 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        const BLACK: PMColor = A32_MASK << A32_SHIFT;

        let base = &mut self.base.base;
        for_each_run(x, antialias, runs, |x, count, aa| {
            let aa = u32::from(aa);
            if aa != 0 {
                if aa == 255 {
                    base.with_span(x, y, count, |device| memset32(device, BLACK, count));
                } else {
                    let src: PMColor = aa << A32_SHIFT;
                    let dst_scale = alpha_255_to_256(255 - aa);
                    base.with_span(x, y, count, |device| {
                        for d in device.iter_mut().rev() {
                            *d = src.wrapping_add(alpha_mul_q(*d, dst_scale));
                        }
                    });
                }
            }
        });
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        self.base.blit_v(x, y, height, alpha);
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.base.blit_rect(x, y, width, height);
    }

    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        self.base.blit_mask(mask, clip);
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1724-L1730 (chrome/m156)
    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let device = &mut self.base.base.device;
        let off = offset32(device, x, y);
        let bytes = bytes_mut(device);
        let (d0, d1) = (read32(bytes, off), read32(bytes, off + 4));

        write32(
            bytes,
            off,
            (a0 << A32_SHIFT).wrapping_add(alpha_mul_q(d0, 256 - a0)),
        );
        write32(
            bytes,
            off + 4,
            (a1 << A32_SHIFT).wrapping_add(alpha_mul_q(d1, 256 - a1)),
        );
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1732-L1739 (chrome/m156)
    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let device = &mut self.base.base.device;
        let (off0, off1) = (offset32(device, x, y), offset32(device, x, y + 1));
        let bytes = bytes_mut(device);

        let d0 = read32(bytes, off0);
        write32(
            bytes,
            off0,
            (a0 << A32_SHIFT).wrapping_add(alpha_mul_q(d0, 256 - a0)),
        );
        let d1 = read32(bytes, off1);
        write32(
            bytes,
            off1,
            (a1 << A32_SHIFT).wrapping_add(alpha_mul_q(d1, 256 - a1)),
        );
    }

    fn can_direct_blit(&mut self) -> Option<DirectBlit<'_>> {
        self.base.can_direct_blit()
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        self.base.blit_memory()
    }
}

/// `SkShaderBlitter`: what the shader blitters share, a shader and the legacy context that
/// shades spans for it. (`SkRasterBlitter`'s device is held by the blitter.)
// Port of: src/core/SkCoreBlitters.h#L30-L55 (chrome/m156)
#[doc(alias = "SkShaderBlitter")]
#[derive(Debug)]
pub struct ShaderBlitter<'a> {
    device: Pixmap<'a>,
    #[allow(dead_code)] // kept alive for the context, as `fShader` is in C++
    shader: Shader,
    shader_context: Box<dyn ShaderContext>,
    scratch: Vec<u32>,
    memory: BlitMemory,
}

impl<'a> ShaderBlitter<'a> {
    /// `SkShaderBlitter::SkShaderBlitter`: `paint` must have a shader, and `shader_context` is the
    /// legacy context made for it.
    ///
    /// # Panics
    /// If `paint` has no shader.
    // Port of: src/core/SkBlitter.cpp#L713-L720 (chrome/m156)
    #[must_use]
    pub fn new(device: Pixmap<'a>, paint: &Paint, shader_context: Box<dyn ShaderContext>) -> Self {
        let shader = paint
            .shader()
            .expect("a shader blitter needs a paint with a shader");
        ShaderBlitter {
            device,
            shader,
            shader_context,
            scratch: Vec::new(),
            memory: BlitMemory::default(),
        }
    }
}

/// A source-over `kN32` blitter that gets its colors from a legacy shader context
/// (`SkARGB32_Shader_Blitter`).
///
/// skia-rust: Skia's `SkShaderBase::makeContext` returns null in the pinned builds
/// ([`ENABLE_LEGACY_SHADER_CONTEXT`](skia_rust_core::shaders::ENABLE_LEGACY_SHADER_CONTEXT)), so
/// `SkBlitter::Choose` never makes one; it is ported (it is compiled in Skia) and tested with a
/// test shader context.
// Port of: src/core/SkCoreBlitters.h#L100-L117 (chrome/m156)
#[doc(alias = "SkARGB32_Shader_Blitter")]
#[derive(Debug)]
pub struct Argb32ShaderBlitter<'a> {
    base: ShaderBlitter<'a>,
    buffer: Vec<u32>,
    proc32: Proc32,
    proc32_blend: Proc32,
    shade_directly_into_device: bool,
}

impl<'a> Argb32ShaderBlitter<'a> {
    /// `SkARGB32_Shader_Blitter::SkARGB32_Shader_Blitter`.
    ///
    /// # Panics
    /// If `paint` has no shader.
    // Port of: src/core/SkBlitter_ARGB32.cpp#L1743-L1762 (chrome/m156)
    #[must_use]
    pub fn new(device: Pixmap<'a>, paint: &Paint, shader_context: Box<dyn ShaderContext>) -> Self {
        let buffer = vec![0u32; to_len(device.width())];

        debug_assert!(paint.is_src_over());

        let mut flags = 0;
        if shader_context.flags() & OPAQUE_ALPHA_FLAG == 0 {
            flags |= SRC_PIXEL_ALPHA_FLAG32;
        }
        // we call this on the output from the shader
        let proc32 = factory32(flags);
        // we call this on the output from the shader + alpha from the aa buffer
        let proc32_blend = factory32(flags | GLOBAL_ALPHA_FLAG32);

        let shade_directly_into_device = shader_context.flags() & OPAQUE_ALPHA_FLAG != 0;

        Argb32ShaderBlitter {
            base: ShaderBlitter::new(device, paint, shader_context),
            buffer,
            proc32,
            proc32_blend,
            shade_directly_into_device,
        }
    }
}

// Shades `count` pixels at `(x, y)` right into the device.
fn shade_into_device(base: &mut ShaderBlitter<'_>, x: i32, y: i32, count: usize) {
    let ShaderBlitter {
        device,
        shader_context,
        scratch,
        ..
    } = base;
    with_span32(device, scratch, x, y, count, |dst| {
        shader_context.shade_span(x, y, dst);
    });
}

// Shades `count` pixels at `(x, y)` into `span`, then blends them onto the device with `proc`.
fn shade_and_blend(
    base: &mut ShaderBlitter<'_>,
    span: &mut [u32],
    x: i32,
    y: i32,
    count: usize,
    proc: Proc32,
    alpha: u32,
) {
    let ShaderBlitter {
        device,
        shader_context,
        scratch,
        ..
    } = base;
    shader_context.shade_span(x, y, &mut span[..count]);
    with_span32(device, scratch, x, y, count, |dst| {
        proc(dst, &span[..count], alpha);
    });
}

impl Blitter for Argb32ShaderBlitter<'_> {
    // Port of: src/core/SkBlitter_ARGB32.cpp#L1768-L1780 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        debug_assert!(x >= 0 && y >= 0 && x + width <= self.base.device.width());
        let width = to_len(width);

        if self.shade_directly_into_device {
            shade_into_device(&mut self.base, x, y, width);
        } else {
            let (proc32, span) = (self.proc32, &mut self.buffer);
            shade_and_blend(&mut self.base, span, x, y, width, proc32, 255);
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L2022-L2054 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        debug_assert!(x >= 0 && y >= 0 && y + height <= self.base.device.height());

        let mut y = y;
        let mut height = height;
        if self.shade_directly_into_device {
            if 255 == alpha {
                loop {
                    shade_into_device(&mut self.base, x, y, 1);
                    y += 1;
                    height -= 1;
                    if height <= 0 {
                        break;
                    }
                }
            } else {
                loop {
                    let mut c = [0u32];
                    self.base.shader_context.shade_span(x, y, &mut c);
                    let device = &mut self.base.device;
                    let off = offset32(device, x, y);
                    let bytes = bytes_mut(device);
                    let d = read32(bytes, off);
                    write32(bytes, off, four_byte_interp(c[0], d, u32::from(alpha)));
                    y += 1;
                    height -= 1;
                    if height <= 0 {
                        break;
                    }
                }
            }
        } else {
            let proc = if 255 == alpha {
                self.proc32
            } else {
                self.proc32_blend
            };
            loop {
                let span = &mut self.buffer;
                shade_and_blend(&mut self.base, span, x, y, 1, proc, u32::from(alpha));
                y += 1;
                height -= 1;
                if height <= 0 {
                    break;
                }
            }
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1782-L1806 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        debug_assert!(
            x >= 0
                && y >= 0
                && x + width <= self.base.device.width()
                && y + height <= self.base.device.height()
        );
        let width = to_len(width);

        let mut y = y;
        let mut height = height;
        if self.shade_directly_into_device {
            loop {
                shade_into_device(&mut self.base, x, y, width);
                y += 1;
                height -= 1;
                if height <= 0 {
                    break;
                }
            }
        } else {
            let proc = self.proc32;
            loop {
                let span = &mut self.buffer;
                shade_and_blend(&mut self.base, span, x, y, width, proc, 255);
                y += 1;
                height -= 1;
                if height <= 0 {
                    break;
                }
            }
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1808-L1856 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        let opaque = self.base.shader_context.flags() & OPAQUE_ALPHA_FLAG != 0;
        if self.shade_directly_into_device || opaque {
            for_each_run(x, antialias, runs, |x, count, aa| {
                if aa != 0 {
                    if aa == 255 {
                        // cool, have the shader draw right into the device
                        shade_into_device(&mut self.base, x, y, count);
                    } else {
                        let (proc, span) = (self.proc32_blend, &mut self.buffer);
                        shade_and_blend(&mut self.base, span, x, y, count, proc, u32::from(aa));
                    }
                }
            });
        } else {
            for_each_run(x, antialias, runs, |x, count, aa| {
                if aa != 0 {
                    let proc = if aa == 255 {
                        self.proc32
                    } else {
                        self.proc32_blend
                    };
                    let alpha = if aa == 255 { 255 } else { u32::from(aa) };
                    let span = &mut self.buffer;
                    shade_and_blend(&mut self.base, span, x, y, count, proc, alpha);
                }
            });
        }
    }

    // Port of: src/core/SkBlitter_ARGB32.cpp#L1981-L2020 (chrome/m156)
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        debug_assert!(contains_clip(mask, clip));

        let opaque = self.base.shader_context.flags() & OPAQUE_ALPHA_FLAG != 0;

        let blend_row: BlendRow = match (mask.format, opaque) {
            (MaskFormat::A8, true) => blend_row_a8_opaque,
            (MaskFormat::A8, false) => blend_row_a8,
            (MaskFormat::Lcd16, true) => blend_row_lcd16_opaque,
            (MaskFormat::Lcd16, false) => blend_row_lcd16,
            _ => {
                // this->SkShaderBlitter::blitMask(mask, clip)
                blit_mask_default(self, mask, clip);
                return;
            }
        };

        let x = clip.left;
        let width = to_len(clip.width());
        let mut y = clip.top;
        let mut height = clip.height();

        let mask_row_bytes = mask.row_bytes as usize;
        let mask_rows = mask.get_addr(x, y);
        let mut row = 0usize;

        loop {
            let span = &mut self.buffer;
            let ShaderBlitter {
                device,
                shader_context,
                scratch,
                ..
            } = &mut self.base;
            shader_context.shade_span(x, y, &mut span[..width]);
            let mask_row = &mask_rows[row..];
            with_span32(device, scratch, x, y, width, |dst| {
                blend_row(dst, mask_row, &span[..width]);
            });
            row += mask_row_bytes;
            y += 1;
            height -= 1;
            if height <= 0 {
                break;
            }
        }
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.base.memory
    }
}

/// A row blender: blends the shaded `src` pixels onto `dst` through a row of mask bytes.
type BlendRow = fn(dst: &mut [u32], mask: &[u8], src: &[u32]);

// `skvx::approx_scale` on one byte: `(x*y + x) / 256`.
#[allow(clippy::cast_possible_truncation)] // cast<uint8_t>: (x*y + x) / 256 < 256
fn approx_scale(x: u8, y: u8) -> u8 {
    ((u16::from(x) * u16::from(y) + u16::from(x)) / 256) as u8
}

// `skvx::div255` on one value that is at most 255*255: `(x + 127) / 255`.
#[allow(clippy::cast_possible_truncation)] // cast<uint8_t>: the quotient is at most 255
fn div255(x: u16) -> u8 {
    ((x + 127) / 255) as u8
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1886-L1893 (chrome/m156)
fn blend_row_a8(dst: &mut [u32], mask: &[u8], src: &[u32]) {
    for (i, d) in dst.iter_mut().enumerate() {
        let (s, db) = (src[i].to_le_bytes(), d.to_le_bytes());
        let c = mask[i];
        let s_aa = s.map(|s| approx_scale(s, c));
        let alpha = s_aa[3]; // shuffle<3,3,3,3, ...>
        let mut out = [0u8; 4];
        for k in 0..4 {
            out[k] = s_aa[k].wrapping_add(approx_scale(db[k], 255 - alpha));
        }
        *d = u32::from_le_bytes(out);
    }
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1895-L1901 (chrome/m156)
fn blend_row_a8_opaque(dst: &mut [u32], mask: &[u8], src: &[u32]) {
    for (i, d) in dst.iter_mut().enumerate() {
        let (s, db) = (src[i].to_le_bytes(), d.to_le_bytes());
        let c = mask[i];
        let mut out = [0u8; 4];
        for k in 0..4 {
            out[k] = div255(u16::from(s[k]) * u16::from(c) + u16::from(db[k]) * u16::from(255 - c));
        }
        *d = u32::from_le_bytes(out);
    }
}

// `SkAlphaMul(value, alpha256)` on `int`s.
fn alpha_mul_i(value: i32, alpha256: i32) -> i32 {
    (value * alpha256) >> 8
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1903-L1945 (chrome/m156)
fn blend_row_lcd16(dst: &mut [u32], vmask: &[u8], src: &[u32]) {
    let src_alpha_blend =
        |s: i32, d: i32, sa: i32, m: i32| d + alpha_mul_i(s - alpha_mul_i(sa, d), m);

    let upscale_31_to_255 = |v: i32| (v << 3) | (v >> 2);

    for (i, d_out) in dst.iter_mut().enumerate() {
        let m = lcd_at(vmask, i);
        if 0 == m {
            continue;
        }

        let s = src[i];
        let d = *d_out;

        let (mut src_a, src_r, src_g, src_b) = unpack(s);

        src_a += src_a >> 7;

        // We're ignoring the least significant bit of the green coverage channel here.
        let m = u32::from(m);
        #[allow(clippy::cast_possible_wrap)] // 5-bit values
        let (mask_r, mask_g, mask_b) = (
            (get_packed_r16(m) >> (R16_BITS - 5)) as i32,
            (get_packed_g16(m) >> (G16_BITS - 5)) as i32,
            (get_packed_b16(m) >> (B16_BITS - 5)) as i32,
        );

        // Scale up to 8-bit coverage to work with SkAlphaMul() in src_alpha_blend().
        let (mask_r, mask_g, mask_b) = (
            upscale_31_to_255(mask_r),
            upscale_31_to_255(mask_g),
            upscale_31_to_255(mask_b),
        );

        // This LCD blit routine only works if the destination is opaque.
        let (_, d_r, d_g, d_b) = unpack(d);
        *d_out = pack_argb32(
            0xFF,
            component(src_alpha_blend(src_r, d_r, src_a, mask_r)),
            component(src_alpha_blend(src_g, d_g, src_a, mask_g)),
            component(src_alpha_blend(src_b, d_b, src_a, mask_b)),
        );
    }
}

// Port of: src/core/SkBlitter_ARGB32.cpp#L1947-L1979 (chrome/m156)
fn blend_row_lcd16_opaque(dst: &mut [u32], vmask: &[u8], src: &[u32]) {
    for (i, d_out) in dst.iter_mut().enumerate() {
        let m = lcd_at(vmask, i);
        if 0 == m {
            continue;
        }

        let s = src[i];
        let d = *d_out;

        let (_, src_r, src_g, src_b) = unpack(s);

        // We're ignoring the least significant bit of the green coverage channel here.
        // Now upscale them to 0..32, so we can use blend_32.
        let (mask_r, mask_g, mask_b) = lcd_masks_5(m);

        // This LCD blit routine only works if the destination is opaque.
        let (_, d_r, d_g, d_b) = unpack(d);
        *d_out = pack_argb32(
            0xFF,
            component(blend_32(src_r, d_r, mask_r)),
            component(blend_32(src_g, d_g, mask_g)),
            component(blend_32(src_b, d_b, mask_b)),
        );
    }
}

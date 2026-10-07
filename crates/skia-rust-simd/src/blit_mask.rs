// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkBlitMask_opts.h, src/core/Sk4px.h

//! `SkOpts::blit_mask_d32_a8` (design §1.8, task B7): blits a constant color through an A8 mask
//! onto 32-bit premultiplied pixels.
//!
//! Two formulas, as in Skia:
//!
//! | Tier | Skia | Arithmetic |
//! |---|---|---|
//! | `Sse2`, `Sse41`, `Ml3`, `Ml4`, `Scalar` | the portable `Sk4px` code | `approx_scale` (`(x*y + x) / 256`) per byte |
//! | `Neon` | `blit_mask_d32_a8_neon` | `(x * scale) >> 8` per byte, 8 pixels at a time, with `SkAlphaMulQ` on the row's tail (**different results**) |
//!
//! The portable `Sk4px` code is exact in any implementation (Skia compiles it with `skvx`, whose
//! SIMD paths agree with the scalar ones), so the non-Neon tiers share one implementation, which
//! is also the scalar twin ([`blit_mask_d32_a8_scalar`]). `Neon` has the intrinsics
//! implementation on aarch64 and a per-byte model everywhere (the twin of the native code).
//!
//! `SkPMColor`'s byte order follows [`crate::color_util`]: BGRA on Windows, RGBA elsewhere.

use crate::color_util::{
    A32_SHIFT, COLOR_BLACK, alpha_255_to_256, alpha_mul_q, pre_multiply_color,
};
use crate::tier::{Backend, Selection, Tier, selection};

/// `SkOpts::blit_mask_d32_a8(dst, dstRB, mask, maskRB, color, w, h)` on the current
/// [`selection`].
///
/// `dst_rb` is in bytes and must be a multiple of 4; `mask_rb` may be 0 (nine patch blits one
/// mask row repeatedly). `color` is an `SkColor` (`0xAARRGGBB`, not premultiplied).
///
/// # Panics
/// If `dst` or `mask` is too short for `h` rows of `w` pixels, or `dst_rb % 4 != 0`.
#[allow(clippy::too_many_arguments)] // Skia's signature
// Port of: src/opts/SkBlitMask_opts.h#L401-L412 (chrome/m156)
pub fn blit_mask_d32_a8(
    dst: &mut [u32],
    dst_rb: usize,
    mask: &[u8],
    mask_rb: usize,
    color: u32,
    w: usize,
    h: usize,
) {
    blit_mask_d32_a8_with(selection(), dst, dst_rb, mask, mask_rb, color, w, h);
}

/// [`blit_mask_d32_a8`] on an explicit [`Selection`].
///
/// # Panics
/// As [`blit_mask_d32_a8`]; also if `sel` is a native tier this host cannot run.
#[allow(clippy::too_many_arguments)] // Skia's signature
pub fn blit_mask_d32_a8_with(
    sel: Selection,
    dst: &mut [u32],
    dst_rb: usize,
    mask: &[u8],
    mask_rb: usize,
    color: u32,
    w: usize,
    h: usize,
) {
    match (sel.tier, sel.backend) {
        (Tier::Neon, Backend::Native) => {
            #[cfg(target_arch = "aarch64")]
            {
                let token = crate::cpu::NeonToken::get()
                    .unwrap_or_else(|| panic!("{sel}: the host cannot run this tier"));
                neon::blit(token, dst, dst_rb, mask, mask_rb, color, w, h);
            }
            #[cfg(not(target_arch = "aarch64"))]
            panic!("{sel}: the host cannot run this tier");
        }
        (Tier::Neon, Backend::Model(_)) => {
            blit_neon_model(dst, dst_rb, mask, mask_rb, color, w, h);
        }
        (_, Backend::Native) if !sel.tier.is_native() => {
            panic!("{sel}: the host cannot run this tier")
        }
        _ => blit_mask_d32_a8_scalar(dst, dst_rb, mask, mask_rb, color, w, h),
    }
}

/// The scalar twin of [`blit_mask_d32_a8`]: the portable `Sk4px` code, one pixel at a time
/// (every tier but `Neon`).
///
/// # Panics
/// As [`blit_mask_d32_a8`].
#[allow(clippy::too_many_arguments)] // Skia's signature
// Port of: src/opts/SkBlitMask_opts.h#L316-L384 (chrome/m156)
pub fn blit_mask_d32_a8_scalar(
    dst: &mut [u32],
    dst_rb: usize,
    mask: &[u8],
    mask_rb: usize,
    color: u32,
    w: usize,
    h: usize,
) {
    if color == COLOR_BLACK {
        for_each_row(dst, dst_rb, mask, mask_rb, w, h, |d, m| {
            for (d, aa) in d.iter_mut().zip(m) {
                *d = sk4px_black(*d, *aa);
            }
        });
    } else if color >> 24 == 0xFF {
        let s = pre_multiply_color(color);
        for_each_row(dst, dst_rb, mask, mask_rb, w, h, |d, m| {
            for (d, aa) in d.iter_mut().zip(m) {
                *d = sk4px_opaque(s, *d, *aa);
            }
        });
    } else {
        let s = pre_multiply_color(color);
        for_each_row(dst, dst_rb, mask, mask_rb, w, h, |d, m| {
            for (d, aa) in d.iter_mut().zip(m) {
                *d = sk4px_general(s, *d, *aa);
            }
        });
    }
}

#[allow(clippy::many_single_char_names)] // Skia's `w`, `h`, `d`, `m`
/// Calls `f(row_of_dst, row_of_mask)` for each of the `h` rows: `w` pixels of `dst`, `w` mask
/// bytes; `dst` advances by `dst_rb` bytes and `mask` by `mask_rb` bytes per row.
#[allow(clippy::too_many_arguments)] // Skia's signature
fn for_each_row(
    dst: &mut [u32],
    dst_rb: usize,
    mask: &[u8],
    mask_rb: usize,
    w: usize,
    h: usize,
    mut f: impl FnMut(&mut [u32], &[u8]),
) {
    assert!(dst_rb.is_multiple_of(4), "dst_rb must be a multiple of 4");
    let stride = dst_rb / 4;
    for y in 0..h {
        let d = &mut dst[y * stride..y * stride + w];
        let m = &mask[y * mask_rb..y * mask_rb + w];
        f(d, m);
    }
}

/// `Sk4px::approxMulDiv255` on one byte: `approx_scale(x, y)`.
// Port of: src/core/SkVx.h#L823-L831 (chrome/m156)
#[inline]
#[allow(clippy::cast_possible_truncation)] // cast<uint8_t>: (x*y + x) / 256 < 256
fn approx(x: u8, y: u8) -> u8 {
    ((u16::from(x) * u16::from(y) + u16::from(x)) / 256) as u8
}

/// The general case of `blit_mask_d32_a8_general`'s `fn` on one pixel.
// Port of: src/opts/SkBlitMask_opts.h#L316-L333 (chrome/m156)
fn sk4px_general(s: u32, d: u32, aa: u8) -> u32 {
    //  = (s + d(1-sa))aa + d(1-aa)
    //  = s*aa + d(1-sa*aa)
    let s = s.to_le_bytes();
    let d = d.to_le_bytes();
    let left = s.map(|s| approx(s, aa));
    let inv_left_alpha = 255 - left[3]; // left.alphas().inv()
    let mut out = [0u8; 4];
    for i in 0..4 {
        let right = approx(d[i], inv_left_alpha);
        out[i] = left[i].wrapping_add(right); // This does not overflow (exhaustively checked).
    }
    u32::from_le_bytes(out)
}

/// `blit_mask_d32_a8_opaque`'s `fn` on one pixel.
// Port of: src/opts/SkBlitMask_opts.h#L336-L352 (chrome/m156)
fn sk4px_opaque(s: u32, d: u32, aa: u8) -> u32 {
    //  = s*aa + d(1-aa)
    let s = s.to_le_bytes();
    let d = d.to_le_bytes();
    let mut out = [0u8; 4];
    for i in 0..4 {
        out[i] = approx(s[i], aa).wrapping_add(approx(d[i], 255 - aa));
    }
    u32::from_le_bytes(out)
}

/// `blit_mask_d32_a8_black`'s `fn` on one pixel.
// Port of: src/opts/SkBlitMask_opts.h#L355-L371 (chrome/m156)
fn sk4px_black(d: u32, aa: u8) -> u32 {
    // a = 1*aa + d(1-1*aa)
    // c = 0*aa + d(1-1*aa) =      d(1-aa)
    let d = d.to_le_bytes();
    let mut out = [0u8; 4];
    for i in 0..4 {
        // `aa & byte16{0,0,0,255, ...}`: alpha is byte 3 (`SK_A32_SHIFT == 24`).
        let a = if i == 3 { aa } else { 0 };
        out[i] = a.wrapping_add(approx(d[i], 255 - aa));
    }
    u32::from_le_bytes(out)
}

// ---------------------------------------------------------------------------------------------
// The Neon formula.

/// `SkAlphaMul_neon8` on one byte: `(color * scale) >> 8`, narrowing (`vshrn_n_u16`).
// Port of: src/opts/SkBlitMask_opts.h#L29-L31 (chrome/m156)
#[inline]
#[allow(clippy::cast_possible_truncation)] // vshrn_n_u16 narrows; the product is <= 255 * 256
fn alpha_mul8(color: u8, scale: u16) -> u8 {
    ((u16::from(color) * scale) >> 8) as u8
}

/// What a Neon row does for each of the three colors (`isTranslucent` or not, or black).
#[derive(Clone, Copy)]
enum Neon {
    /// `blit_mask_d32_a8_neon<true>`.
    General,
    /// `blit_mask_d32_a8_neon<false>`.
    Opaque,
    /// `blit_mask_d32_a8_black`.
    Black,
}

/// One Neon vector chunk (8 pixels), per byte.
// Port of: src/opts/SkBlitMask_opts.h#L62-L85, #L135-L150 (chrome/m156)
fn neon_chunk(kind: Neon, pmc: u32, d: &mut [u32; 8], m: [u8; 8]) {
    let pmc = pmc.to_le_bytes();
    let color_alpha = pmc[3];
    for (px, vmask) in d.iter_mut().zip(m) {
        let vmask256 = u16::from(vmask) + 1; // SkAlpha255To256_neon8
        let dev = px.to_le_bytes();
        let mut out = [0u8; 4];
        match kind {
            Neon::General | Neon::Opaque => {
                let vscale = if matches!(kind, Neon::General) {
                    256 - u16::from(alpha_mul8(color_alpha, vmask256))
                } else {
                    256 - u16::from(vmask)
                };
                for i in 0..4 {
                    out[i] = alpha_mul8(pmc[i], vmask256).wrapping_add(alpha_mul8(dev[i], vscale));
                }
            }
            Neon::Black => {
                let vscale = 256 - u16::from(vmask);
                for i in 0..4 {
                    out[i] = alpha_mul8(dev[i], vscale);
                }
                out[3] = out[3].wrapping_add(vmask);
            }
        }
        *px = u32::from_le_bytes(out);
    }
}

/// One Neon scalar tail pixel (`SkAlphaMulQ`).
// Port of: src/opts/SkBlitMask_opts.h#L87-L101, #L153-L159 (chrome/m156)
fn neon_tail(kind: Neon, pmc: u32, d: &mut u32, m: u8) {
    let color_alpha = pmc >> A32_SHIFT;
    let vmask = u32::from(m);
    match kind {
        Neon::General | Neon::Opaque => {
            let vmask256 = alpha_255_to_256(vmask);
            let vscale = if matches!(kind, Neon::General) {
                256 - alpha_mul_q(color_alpha, vmask256)
            } else {
                256 - vmask
            };
            *d = alpha_mul_q(pmc, vmask256).wrapping_add(alpha_mul_q(*d, vscale));
        }
        Neon::Black => {
            let vscale = 256 - vmask;
            *d = alpha_mul_q(*d, vscale).wrapping_add(vmask << A32_SHIFT);
        }
    }
}

/// Which Neon row kind `color` selects, and the premultiplied color it blits.
fn neon_kind(color: u32) -> (Neon, u32) {
    if color == COLOR_BLACK {
        (Neon::Black, pre_multiply_color(color))
    } else if color >> 24 == 0xFF {
        (Neon::Opaque, pre_multiply_color(color))
    } else {
        (Neon::General, pre_multiply_color(color))
    }
}

/// The `Neon` tier's model: the same arithmetic as the intrinsics, per byte. Like Skia's code,
/// it needs `h >= 1` (the C++ loops are `do { … } while (--height != 0)`), but `h == 0` is a
/// no-op here.
// Port of: src/opts/SkBlitMask_opts.h#L33-L184 (chrome/m156)
fn blit_neon_model(
    dst: &mut [u32],
    dst_rb: usize,
    mask: &[u8],
    mask_rb: usize,
    color: u32,
    w: usize,
    h: usize,
) {
    let (kind, pmc) = neon_kind(color);
    for_each_row(dst, dst_rb, mask, mask_rb, w, h, |d, m| {
        let (d8, d_tail) = d.as_chunks_mut::<8>();
        let (m8, m_tail) = m.as_chunks::<8>();
        for (d, m) in d8.iter_mut().zip(m8) {
            neon_chunk(kind, pmc, d, *m);
        }
        for (d, m) in d_tail.iter_mut().zip(m_tail) {
            neon_tail(kind, pmc, d, *m);
        }
    });
}

#[cfg(target_arch = "aarch64")]
mod neon {
    //! `blit_mask_d32_a8_neon<…>` and `blit_mask_d32_a8_black` with intrinsics.

    use core::arch::aarch64::{
        uint8x8_t, uint8x8x4_t, uint16x8_t, vaddw_u8, vdup_n_u8, vdupq_n_u16, vld1_u8, vld4_u8,
        vmovl_u8, vmulq_u16, vshrn_n_u16, vst4_u8, vsubw_u8,
    };

    use super::{Neon, for_each_row, neon_kind, neon_tail};
    use crate::cpu::NeonToken;

    /// Eight pixels, deinterleaved into planes.
    #[target_feature(enable = "neon")]
    #[inline]
    fn load_px(v: &[u32; 8]) -> uint8x8x4_t {
        // SAFETY: `v` is a live `[u32; 8]`, valid for 32 bytes of reads; `vld4_u8` needs only
        // byte alignment.
        unsafe { vld4_u8(v.as_ptr().cast()) }
    }

    /// Eight pixels, interleaved from planes.
    #[target_feature(enable = "neon")]
    #[inline]
    fn store_px(v: &mut [u32; 8], r: uint8x8x4_t) {
        // SAFETY: `v` is a live `[u32; 8]`, valid for 32 bytes of writes; `vst4_u8` needs only
        // byte alignment.
        unsafe { vst4_u8(v.as_mut_ptr().cast(), r) };
    }

    /// Eight mask bytes.
    #[target_feature(enable = "neon")]
    #[inline]
    fn load_mask(m: [u8; 8]) -> uint8x8_t {
        // SAFETY: `m` is a live `[u8; 8]`, valid for 8 bytes of reads; `vld1_u8` needs only byte
        // alignment.
        unsafe { vld1_u8(m.as_ptr()) }
    }

    // Port of: src/opts/SkBlitMask_opts.h#L29-L31 (chrome/m156)
    #[target_feature(enable = "neon")]
    #[inline]
    fn alpha_mul_neon8(color: uint8x8_t, scale: uint16x8_t) -> uint8x8_t {
        vshrn_n_u16::<8>(vmulq_u16(vmovl_u8(color), scale))
    }

    /// One 8-pixel chunk; `pmc` are the premultiplied color's planes.
    #[target_feature(enable = "neon")]
    #[inline]
    fn chunk(kind: Neon, pmc: u32, d: &mut [u32; 8], m: [u8; 8]) {
        let vmask = load_mask(m);
        let mut vdev = load_px(d);
        if matches!(kind, Neon::Black) {
            let vscale = vsubw_u8(vdupq_n_u16(256), vmask);
            vdev = uint8x8x4_t(
                alpha_mul_neon8(vdev.0, vscale),
                alpha_mul_neon8(vdev.1, vscale),
                alpha_mul_neon8(vdev.2, vscale),
                alpha_mul_neon8(vdev.3, vscale),
            );
            vdev.3 = core::arch::aarch64::vadd_u8(vdev.3, vmask);
        } else {
            let p = pmc.to_le_bytes();
            let vpmc = [
                vdup_n_u8(p[0]),
                vdup_n_u8(p[1]),
                vdup_n_u8(p[2]),
                vdup_n_u8(p[3]),
            ];
            let vmask256 = vaddw_u8(vdupq_n_u16(1), vmask);
            let vscale = if matches!(kind, Neon::General) {
                vsubw_u8(vdupq_n_u16(256), alpha_mul_neon8(vpmc[3], vmask256))
            } else {
                vsubw_u8(vdupq_n_u16(256), vmask)
            };
            let mix = |i: usize, dev: uint8x8_t| {
                core::arch::aarch64::vadd_u8(
                    alpha_mul_neon8(vpmc[i], vmask256),
                    alpha_mul_neon8(dev, vscale),
                )
            };
            vdev = uint8x8x4_t(
                mix(0, vdev.0),
                mix(1, vdev.1),
                mix(2, vdev.2),
                mix(3, vdev.3),
            );
        }
        store_px(d, vdev);
    }

    #[target_feature(enable = "neon")]
    #[allow(clippy::too_many_arguments)] // Skia's signature
    fn blit_neon(
        dst: &mut [u32],
        dst_rb: usize,
        mask: &[u8],
        mask_rb: usize,
        color: u32,
        w: usize,
        h: usize,
    ) {
        let (kind, pmc) = neon_kind(color);
        for_each_row(dst, dst_rb, mask, mask_rb, w, h, |d, m| {
            let (d8, d_tail) = d.as_chunks_mut::<8>();
            let (m8, m_tail) = m.as_chunks::<8>();
            for (d, m) in d8.iter_mut().zip(m8) {
                chunk(kind, pmc, d, *m);
            }
            for (d, m) in d_tail.iter_mut().zip(m_tail) {
                neon_tail(kind, pmc, d, *m);
            }
        });
    }

    #[allow(clippy::too_many_arguments)] // Skia's signature
    pub(super) fn blit(
        _token: NeonToken,
        dst: &mut [u32],
        dst_rb: usize,
        mask: &[u8],
        mask_rb: usize,
        color: u32,
        w: usize,
        h: usize,
    ) {
        // SAFETY: `blit_neon` enables `neon`, which `_token` proves the host supports (NEON is
        // also the aarch64 baseline).
        unsafe { blit_neon(dst, dst_rb, mask, mask_rb, color, w, h) };
    }
}

#[cfg(test)]
mod tests;

// Copyright 2011 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan_Antihair.cpp (antialiased rects)

//! Antialiased rectangle fills and frames (`SkScan::AntiFillRect`, `AntiFillXRect`,
//! `AntiFrameRect` from `SkScan_Antihair.cpp`).
//!
//! skia-rust: only the rectangle half of `SkScan_Antihair.cpp` is ported here; the antialiased
//! hairlines belong to task C4 and will join this module. Only the `SkRegion*` overloads are
//! ported; the `SkRasterClip` overloads (which wrap AA clips) come with the raster clip (C5).

use skia_rust_core::color::Alpha;
use skia_rust_core::color_priv::alpha_mul;
use skia_rust_core::fixed::Fixed;
use skia_rust_core::math::mul_div_255_round;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::region::{Cliperator, Region};

use crate::blitter::{Blitter, BlitterClipper};
use crate::scan::{XRect, xrect_from_irect, xrect_from_rect, xrect_round_out};
use crate::scan_priv::float_to_int;

// Port of: src/core/SkScan_Antihair.cpp#L29 (chrome/m156)
const HLINE_STACK_BUFFER: usize = 100;
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // 100 fits
const HLINE_STACK_BUFFER_I32: i32 = HLINE_STACK_BUFFER as i32;

// `SkToU8` / the implicit int -> SkAlpha conversion (checked only in debug builds in Skia).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_alpha(v: i32) -> Alpha {
    v as Alpha
}

/// Blits `count` pixels of `alpha` with `blitAntiH`, in chunks of at most 100.
// Port of: src/core/SkScan_Antihair.cpp#L43-L67 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // n is in [1, 100]
pub(crate) fn call_hline_blitter(
    blitter: &mut dyn Blitter,
    mut x: i32,
    y: i32,
    mut count: i32,
    alpha: u32,
) {
    debug_assert!(count > 0);

    let mut runs = [0i16; HLINE_STACK_BUFFER + 1];
    let mut aa = [0 as Alpha; HLINE_STACK_BUFFER];

    loop {
        // In theory, we should be able to just do this once (outside of the loop),
        // since aa[] and runs[] are "supposed" to be const when we call the blitter.
        // In reality, some wrapper-blitters (e.g. SkRgnClipBlitter) cast away that
        // constness, and modify the buffers in-place. Hence the need to be defensive
        // here and reseed the aa value.
        aa[0] = alpha as Alpha;

        let mut n = count;
        if n > HLINE_STACK_BUFFER_I32 {
            n = HLINE_STACK_BUFFER_I32;
        }
        runs[0] = n as i16;
        runs[n as usize] = 0;
        blitter.blit_anti_h(x, y, &mut aa, &mut runs);
        x += n;
        count -= n;
        if count <= 0 {
            break;
        }
    }
}

/// 24.8 integer fixed point.
// Port of: src/core/SkScan_Antihair.cpp#L625 (chrome/m156)
type FDot8 = i32;

// Port of: src/core/SkScan_Antihair.cpp#L627-L629 (chrome/m156)
fn fixed_to_fdot8(x: Fixed) -> FDot8 {
    x.wrapping_add(0x80) >> 8
}

// `SkAlphaMul(alpha, scale)` with the unsigned `U8CPU alpha`.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the unsigned arithmetic
fn alpha_mul_u(alpha: u32, scale: i32) -> Alpha {
    to_alpha(alpha_mul(alpha as i32, scale))
}

// Port of: src/core/SkScan_Antihair.cpp#L631-L655 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
fn do_scanline(l: FDot8, top: i32, r: FDot8, alpha: u32, blitter: &mut dyn Blitter) {
    debug_assert!(l < r);

    if (l >> 8) == ((r - 1) >> 8) {
        // 1x1 pixel
        blitter.blit_v(l >> 8, top, 1, alpha_mul_u(alpha, r - l));
        return;
    }

    let mut left = l >> 8;

    if l & 0xFF != 0 {
        blitter.blit_v(left, top, 1, alpha_mul_u(alpha, 256 - (l & 0xFF)));
        left += 1;
    }

    let rite = r >> 8;
    let width = rite - left;
    if width > 0 {
        call_hline_blitter(blitter, left, top, width, alpha);
    }
    if r & 0xFF != 0 {
        blitter.blit_v(rite, top, 1, alpha_mul_u(alpha, r & 0xFF));
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L657-L699 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
#[allow(clippy::cast_sign_loss)] // the alphas are positive
fn antifilldot8(
    l: FDot8,
    t: FDot8,
    r: FDot8,
    b: FDot8,
    blitter: &mut dyn Blitter,
    fill_inner: bool,
) {
    // check for empty now that we're in our reduced precision space
    if l >= r || t >= b {
        return;
    }
    let mut top = t >> 8;
    if top == ((b - 1) >> 8) {
        // just one scanline high
        do_scanline(l, top, r, (b - t - 1) as u32, blitter);
        return;
    }

    if t & 0xFF != 0 {
        do_scanline(l, top, r, (256 - (t & 0xFF)) as u32, blitter);
        top += 1;
    }

    let bot = b >> 8;
    let height = bot - top;
    if height > 0 {
        let mut left = l >> 8;
        if left == ((r - 1) >> 8) {
            // just 1-pixel wide
            blitter.blit_v(left, top, height, to_alpha(r - l - 1));
        } else {
            if l & 0xFF != 0 {
                blitter.blit_v(left, top, height, to_alpha(256 - (l & 0xFF)));
                left += 1;
            }
            let rite = r >> 8;
            let width = rite - left;
            if width > 0 && fill_inner {
                blitter.blit_rect(left, top, width, height);
            }
            if r & 0xFF != 0 {
                blitter.blit_v(rite, top, height, to_alpha(r & 0xFF));
            }
        }
    }

    if b & 0xFF != 0 {
        do_scanline(l, bot, r, (b & 0xFF) as u32, blitter);
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L701-L705 (chrome/m156)
fn antifillrect_x(xr: &XRect, blitter: &mut dyn Blitter) {
    antifilldot8(
        fixed_to_fdot8(xr.left),
        fixed_to_fdot8(xr.top),
        fixed_to_fdot8(xr.right),
        fixed_to_fdot8(xr.bottom),
        blitter,
        true,
    );
}

/// Fills the fixed-point rect `xr` with antialiasing, clipped to `clip` (`SkScan::AntiFillXRect(
/// const SkXRect&, const SkRegion*, SkBlitter*)`).
// Port of: src/core/SkScan_Antihair.cpp#L709-L746 (chrome/m156)
#[doc(alias = "AntiFillXRect")]
pub fn anti_fill_x_rect(xr: &XRect, clip: Option<&Region>, blitter: &mut dyn Blitter) {
    let Some(clip) = clip else {
        antifillrect_x(xr, blitter);
        return;
    };
    let outer_bounds = xrect_round_out(xr);

    if clip.is_rect() {
        let clip_bounds = clip.bounds();

        if clip_bounds.contains(&outer_bounds) {
            antifillrect_x(xr, blitter);
        } else {
            // this keeps our original edges fractional
            let tmp_r = xrect_from_irect(clip_bounds);
            if let Some(tmp_r) = IRect::intersect(&tmp_r, xr) {
                antifillrect_x(&tmp_r, blitter);
            }
        }
    } else {
        for rr in Cliperator::new(clip, outer_bounds) {
            // this keeps our original edges fractional
            let tmp_r = xrect_from_irect(&rr);
            if let Some(tmp_r) = IRect::intersect(&tmp_r, xr) {
                antifillrect_x(&tmp_r, blitter);
            }
        }
    }
}

// This takes a float-rect, but with the key improvement that it has already been clipped, so we
// know that it is safe to convert it into a XRect (fixedpoint), as it won't overflow.
// Port of: src/core/SkScan_Antihair.cpp#L765-L774 (chrome/m156)
fn antifillrect(r: &Rect, blitter: &mut dyn Blitter) {
    let xr = xrect_from_rect(r);
    antifillrect_x(&xr, blitter);
}

/// Fills `orig_r` with antialiasing, clipped to `clip` (`SkScan::AntiFillRect(const SkRect&,
/// const SkRegion*, SkBlitter*)`).
///
/// We repeat the clipping logic of [`anti_fill_x_rect`] because the float rect might overflow if
/// we blindly converted it to an [`XRect`]. We clip `r` (as needed) into one or more (smaller)
/// float rects, and then pass those to our version of antifillrect, which converts it into an
/// [`XRect`] and then calls the blit.
// Port of: src/core/SkScan_Antihair.cpp#L776-L810 (chrome/m156)
#[doc(alias = "AntiFillRect")]
pub fn anti_fill_rect(orig_r: &Rect, clip: Option<&Region>, blitter: &mut dyn Blitter) {
    if let Some(clip) = clip {
        let mut new_r = Rect::from(*clip.bounds());
        if !new_r.intersect(orig_r) {
            return;
        }

        let outer_bounds: IRect = new_r.round_out();

        if clip.is_rect() {
            antifillrect(&new_r, blitter);
        } else {
            for cr in Cliperator::new(clip, outer_bounds) {
                new_r = Rect::from(cr);
                if new_r.intersect(orig_r) {
                    antifillrect(&new_r, blitter);
                }
            }
        }
    } else {
        antifillrect(orig_r, blitter);
    }
}

// calls blitRect() if the rectangle is non-empty
// Port of: src/core/SkScan_Antihair.cpp#L826-L831 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
fn fillcheckrect(l: i32, t: i32, r: i32, b: i32, blitter: &mut dyn Blitter) {
    if l < r && t < b {
        blitter.blit_rect(l, t, r - l, b - t);
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L833-L835 (chrome/m156)
fn scalar_to_fdot8(x: f32) -> FDot8 {
    float_to_int(x * 256.0)
}

// Port of: src/core/SkScan_Antihair.cpp#L837-L839 (chrome/m156)
fn fdot8_floor(x: FDot8) -> i32 {
    x >> 8
}

// Port of: src/core/SkScan_Antihair.cpp#L841-L843 (chrome/m156)
fn fdot8_ceil(x: FDot8) -> i32 {
    x.wrapping_add(0xFF) >> 8
}

// 1 - (1 - a)*(1 - b)
// Port of: src/core/SkScan_Antihair.cpp#L845-L850 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors SkToU8
fn inv_alpha_mul(a: u32, b: u32) -> Alpha {
    // need precise rounding (not just SkAlphaMul) so that values like
    // a=228, b=252 don't overflow the result
    a.wrapping_add(b).wrapping_sub(mul_div_255_round(a, b)) as Alpha
}

// Port of: src/core/SkScan_Antihair.cpp#L852-L880 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
#[allow(clippy::cast_sign_loss)] // the values are masked to [0, 255] or positive widths
fn inner_scanline(l: FDot8, top: i32, r: FDot8, alpha: u32, blitter: &mut dyn Blitter) {
    debug_assert!(l < r);

    if (l >> 8) == ((r - 1) >> 8) {
        // 1x1 pixel
        let mut wid_clamp = r - l;
        // border case clamp 256 to 255 instead of going through call_hline_blitter
        // see skbug/4406
        wid_clamp -= wid_clamp >> 8;
        blitter.blit_v(l >> 8, top, 1, inv_alpha_mul(alpha, wid_clamp as u32));
        return;
    }

    let mut left = l >> 8;
    if l & 0xFF != 0 {
        blitter.blit_v(left, top, 1, inv_alpha_mul(alpha, (l & 0xFF) as u32));
        left += 1;
    }

    let rite = r >> 8;
    let width = rite - left;
    if width > 0 {
        call_hline_blitter(blitter, left, top, width, alpha);
    }

    if r & 0xFF != 0 {
        blitter.blit_v(rite, top, 1, inv_alpha_mul(alpha, (!r & 0xFF) as u32));
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L882-L915 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
#[allow(clippy::cast_sign_loss)] // the values are masked to [0, 255] or positive
fn innerstrokedot8(l: FDot8, t: FDot8, r: FDot8, b: FDot8, blitter: &mut dyn Blitter) {
    debug_assert!(l < r && t < b);

    let mut top = t >> 8;
    if top == ((b - 1) >> 8) {
        // just one scanline high
        // We want the inverse of B-T, since we're the inner-stroke
        let alpha = 256 - (b - t);
        if alpha != 0 {
            inner_scanline(l, top, r, alpha as u32, blitter);
        }
        return;
    }

    if t & 0xFF != 0 {
        inner_scanline(l, top, r, (t & 0xFF) as u32, blitter);
        top += 1;
    }

    let bot = b >> 8;
    let height = bot - top;
    if height > 0 {
        if l & 0xFF != 0 {
            blitter.blit_v(l >> 8, top, height, to_alpha(l & 0xFF));
        }
        if r & 0xFF != 0 {
            blitter.blit_v(r >> 8, top, height, to_alpha(!r & 0xFF));
        }
    }

    if b & 0xFF != 0 {
        inner_scanline(l, bot, r, (!b & 0xFF) as u32, blitter);
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L917-L924 (chrome/m156)
fn align_thin_stroke(edge1: &mut FDot8, edge2: &mut FDot8) {
    debug_assert!(*edge1 <= *edge2);

    if fdot8_floor(*edge1) == fdot8_floor(*edge2) {
        *edge2 -= *edge1 & 0xFF;
        *edge1 &= !0xFF;
    }
}

/// Strokes the frame of `r` with antialiasing, `stroke_size` wide, clipped to `clip`
/// (`SkScan::AntiFrameRect(const SkRect&, const SkPoint&, const SkRegion*, SkBlitter*)`).
// Port of: src/core/SkScan_Antihair.cpp#L926-L1015 (chrome/m156)
#[doc(alias = "AntiFrameRect")]
pub fn anti_frame_rect(
    r: &Rect,
    stroke_size: Point,
    clip: Option<&Region>,
    blitter: &mut dyn Blitter,
) {
    debug_assert!(stroke_size.x >= 0.0 && stroke_size.y >= 0.0);

    let mut rx = stroke_size.x / 2.0;
    let mut ry = stroke_size.y / 2.0;

    // If we're empty on either axis, we remove the outset amount, to be sure
    // we stroke the same way a polygon would (i.e. it would just see a "line"
    // and not extend it for the miter join).
    if r.width() == 0.0 {
        ry = 0.0;
    }
    if r.height() == 0.0 {
        rx = 0.0;
    }

    // outset by the radius
    let mut outer_l = scalar_to_fdot8(r.left - rx);
    let mut outer_t = scalar_to_fdot8(r.top - ry);
    let mut outer_r = scalar_to_fdot8(r.right + rx);
    let mut outer_b = scalar_to_fdot8(r.bottom + ry);

    // set outer to the outer rect of the outer section
    let mut outer = IRect::new(
        fdot8_floor(outer_l),
        fdot8_floor(outer_t),
        fdot8_ceil(outer_r),
        fdot8_ceil(outer_b),
    );

    let mut clipper = BlitterClipper::new();
    let blitter: &mut dyn Blitter = match clip {
        Some(clip) => {
            if clip.quick_reject_rect(outer) {
                return;
            }
            if clip.contains_rect(outer) {
                blitter
            } else {
                clipper.apply(blitter, Some(clip), Some(&outer))
            }
            // now we can ignore clip for the rest of the function
        }
        None => blitter,
    };

    // in case we lost a bit with diameter/2
    rx = stroke_size.x - rx;
    ry = stroke_size.y - ry;

    // inset by the radius
    let mut inner_l = scalar_to_fdot8(r.left + rx);
    let mut inner_t = scalar_to_fdot8(r.top + ry);
    let mut inner_r = scalar_to_fdot8(r.right - rx);
    let mut inner_b = scalar_to_fdot8(r.bottom - ry);

    // For sub-unit strokes, tweak the hulls such that one of the edges coincides with the pixel
    // edge. This ensures that the general rect stroking logic below
    //   a) doesn't blit the same scanline twice
    //   b) computes the correct coverage when both edges fall within the same pixel
    if stroke_size.x < 1.0 || stroke_size.y < 1.0 {
        align_thin_stroke(&mut outer_l, &mut inner_l);
        align_thin_stroke(&mut outer_t, &mut inner_t);
        align_thin_stroke(&mut inner_r, &mut outer_r);
        align_thin_stroke(&mut inner_b, &mut outer_b);
    }

    // stroke the outer hull
    antifilldot8(outer_l, outer_t, outer_r, outer_b, blitter, false);

    // set outer to the outer rect of the middle section
    outer = IRect::new(
        fdot8_ceil(outer_l),
        fdot8_ceil(outer_t),
        fdot8_floor(outer_r),
        fdot8_floor(outer_b),
    );

    if inner_l >= inner_r || inner_t >= inner_b {
        fillcheckrect(outer.left, outer.top, outer.right, outer.bottom, blitter);
    } else {
        // set inner to the inner rect of the middle section
        let inner = IRect::new(
            fdot8_floor(inner_l),
            fdot8_floor(inner_t),
            fdot8_ceil(inner_r),
            fdot8_ceil(inner_b),
        );

        // draw the frame in 4 pieces
        fillcheckrect(outer.left, outer.top, outer.right, inner.top, blitter);
        fillcheckrect(outer.left, inner.top, inner.left, inner.bottom, blitter);
        fillcheckrect(inner.right, inner.top, outer.right, inner.bottom, blitter);
        fillcheckrect(outer.left, inner.bottom, outer.right, outer.bottom, blitter);

        // now stroke the inner rect, which is similar to antifilldot8() except that
        // it treats the fractional coordinates with the inverse bias (since its
        // inner).
        innerstrokedot8(inner_l, inner_t, inner_r, inner_b, blitter);
    }
}

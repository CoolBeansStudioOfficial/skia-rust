// Copyright 2011 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan_Antihair.cpp

//! Antialiased hairlines and the antialiased rect fills/frames (`SkScan_Antihair.cpp`).
//!
//! Skia names are kept as snake case: `SkScan::AntiHairLineRgn` is [`anti_hair_line_rgn`],
//! `SkScan::AntiFillRect` is [`anti_fill_rect`] (region) / [`anti_fill_rect_clip`] (raster clip),
//! and so on. A C++ `const SkRegion*` that may be null is an `Option<&Region>`.

use skia_rust_core::color_priv::alpha_mul;
use skia_rust_core::fdot6::{
    FDOT6_HALF, FDOT6_ONE, Fdot6, fdot6_ceil, fdot6_floor, fdot6_to_fixed, float_to_fdot6,
    int_to_fdot6,
};
use skia_rust_core::fixed::{FIXED_1, FIXED_HALF, Fixed, fixed_ceil_to_int, fixed_floor_to_int};
use skia_rust_core::line_clipper::intersect_line;
use skia_rust_core::math::{U8CPU, left_shift, mul_div_255_round};
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::region::{Cliperator, Region};

use crate::blitter::{Blitter, BlitterClipper, RectClipBlitter};
use crate::raster_clip::{AAClipBlitterWrapper, RasterClip};
use crate::scan::{XRect, xrect_round_out, xrect_set_irect, xrect_set_rect};

const HLINE_STACK_BUFFER: usize = 100;

// `SkToU8` (checked only in debug builds in Skia).
#[allow(clippy::cast_possible_truncation)] // mirrors SkToU8, which is only asserted
fn to_u8(v: u32) -> u8 {
    debug_assert!(v <= 0xFF);
    v as u8
}

// `SkToS16`.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // mirrors SkToS16, which is only asserted
fn to_s16(v: usize) -> i16 {
    debug_assert!(i16::try_from(v).is_ok());
    v as i16
}

// Port of: src/core/SkScan_Antihair.cpp#L31-L35 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // coverage is asserted to be in 0..=64
fn scale_alpha_by_coverage(value: U8CPU, coverage: Fdot6) -> U8CPU {
    debug_assert!(value <= 255);
    debug_assert!((0..=FDOT6_ONE).contains(&coverage));
    (value * coverage as u32) >> 6
}

// Extracts the high 8 bits of the fractional part of a 16.16 fixed-point number, returning an
// 8-bit alpha value.
// Port of: src/core/SkScan_Antihair.cpp#L37-L41 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // masked to 8 bits
fn fixed_to_alpha(f: Fixed) -> U8CPU {
    ((f >> 8) & 0xFF) as u32
}

// Port of: src/core/SkScan_Antihair.cpp#L43-L68 (chrome/m156)
fn call_hline_blitter(blitter: &mut dyn Blitter, x: i32, y: i32, count: i32, alpha: U8CPU) {
    debug_assert!(count > 0);

    let mut runs = [0i16; HLINE_STACK_BUFFER + 1];
    let mut aa = [0u8; HLINE_STACK_BUFFER];

    let mut x = x;
    let mut count = count;
    loop {
        // In theory, we should be able to just do this once (outside of the loop),
        // since aa[] and runs[] are "supposed" to be const when we call the blitter.
        // In reality, some wrapper-blitters (e.g. SkRgnClipBlitter) cast away that
        // constness, and modify the buffers in-place. Hence the need to be defensive
        // here and reseed the aa value.
        aa[0] = to_u8(alpha);

        let mut n = usize::try_from(count).expect("count > 0");
        if n > HLINE_STACK_BUFFER {
            n = HLINE_STACK_BUFFER;
        }
        runs[0] = to_s16(n);
        runs[n] = 0;
        blitter.blit_anti_h(x, y, &mut aa, &mut runs);
        let n = i32::try_from(n).expect("n <= 100");
        x += n;
        count -= n;
        if count <= 0 {
            break;
        }
    }
}

/// The four concrete `SkAntiHairBlitter` classes of the C++ (an abstract class with `drawCap`
/// and `drawLine`), which draw one hairline orientation each. `do_anti_hairline` chooses one
/// based on the line's slope. For the vertical kinds the "x" parameters of the methods are rows
/// and the "fy" parameters are x positions, as in the C++.
// Port of: src/core/SkScan_Antihair.cpp#L70-L246 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AntiHairKind {
    /// `HLine_SkAntiHairBlitter`: perfectly horizontal hairlines (over two rows).
    HLine,
    /// `Horish_SkAntiHairBlitter`: slope between -1 and 1.
    Horish,
    /// `VLine_SkAntiHairBlitter`: perfectly vertical hairlines.
    VLine,
    /// `Vertish_SkAntiHairBlitter`: slope greater than 1 or less than -1.
    Vertish,
}

impl AntiHairKind {
    // `drawCap`
    fn draw_cap(
        self,
        blitter: &mut dyn Blitter,
        x: i32,
        fy: Fixed,
        slope: Fixed,
        coverage: Fdot6,
    ) -> Fixed {
        match self {
            // Port of: src/core/SkScan_Antihair.cpp#L97-L120 (chrome/m156)
            AntiHairKind::HLine => {
                let fy = fy.wrapping_add(FIXED_HALF);

                let y = fixed_floor_to_int(fy);
                // Compute an alpha value based on the fractional part of fy
                // 0 means fy was at NN.5 and we'll only be drawing the upper line.
                // 128 means fy was at NN.0 and we'll be coloring both lines approximately
                // the same opacity.
                let a = fixed_to_alpha(fy);

                // lower line
                let mut ma = scale_alpha_by_coverage(a, coverage);
                if ma != 0 {
                    call_hline_blitter(blitter, x, y, 1, ma);
                }

                // upper line
                ma = scale_alpha_by_coverage(255 - a, coverage);
                if ma != 0 {
                    call_hline_blitter(blitter, x, y - 1, 1, ma);
                }

                fy.wrapping_sub(FIXED_HALF)
            }
            // Port of: src/core/SkScan_Antihair.cpp#L149-L159 (chrome/m156)
            AntiHairKind::Horish => {
                let fy = fy.wrapping_add(FIXED_HALF);

                let lower_y = fixed_floor_to_int(fy);
                let a = fixed_to_alpha(fy);
                let a0 = scale_alpha_by_coverage(255 - a, coverage);
                let a1 = scale_alpha_by_coverage(a, coverage);
                blitter.blit_anti_v2(x, lower_y - 1, a0, a1);

                fy.wrapping_add(slope).wrapping_sub(FIXED_HALF)
            }
            // Port of: src/core/SkScan_Antihair.cpp#L180-L197 (chrome/m156)
            AntiHairKind::VLine => {
                let (y, fx, dx) = (x, fy, slope);
                debug_assert_eq!(0, dx);
                let fx = fx.wrapping_add(FIXED_HALF);

                let x = fixed_floor_to_int(fx);
                let a = fixed_to_alpha(fx);

                let mut ma = scale_alpha_by_coverage(a, coverage);
                if ma != 0 {
                    blitter.blit_v(x, y, 1, to_u8(ma));
                }
                ma = scale_alpha_by_coverage(255 - a, coverage);
                if ma != 0 {
                    blitter.blit_v(x - 1, y, 1, to_u8(ma));
                }

                fx.wrapping_sub(FIXED_HALF)
            }
            // Port of: src/core/SkScan_Antihair.cpp#L223-L232 (chrome/m156)
            AntiHairKind::Vertish => {
                let (y, fx, dx) = (x, fy, slope);
                let fx = fx.wrapping_add(FIXED_HALF);

                let x = fixed_floor_to_int(fx);
                let a = fixed_to_alpha(fx);
                blitter.blit_anti_h2(
                    x - 1,
                    y,
                    scale_alpha_by_coverage(255 - a, coverage),
                    scale_alpha_by_coverage(a, coverage),
                );

                fx.wrapping_add(dx).wrapping_sub(FIXED_HALF)
            }
        }
    }

    // `drawLine`
    fn draw_line(
        self,
        blitter: &mut dyn Blitter,
        x: i32,
        end: i32,
        fy: Fixed,
        slope: Fixed,
    ) -> Fixed {
        match self {
            // Port of: src/core/SkScan_Antihair.cpp#L122-L142 (chrome/m156)
            AntiHairKind::HLine => {
                debug_assert!(x < end);
                let count = end - x;
                let fy = fy.wrapping_add(FIXED_HALF);

                let y = fixed_floor_to_int(fy);
                let mut a = fixed_to_alpha(fy);

                // lower line
                if a != 0 {
                    call_hline_blitter(blitter, x, y, count, a);
                }

                // upper line
                a = 255 - a;
                if a != 0 {
                    call_hline_blitter(blitter, x, y - 1, count, a);
                }

                fy.wrapping_sub(FIXED_HALF)
            }
            // Port of: src/core/SkScan_Antihair.cpp#L161-L174 (chrome/m156)
            AntiHairKind::Horish => {
                debug_assert!(x < end);

                let mut fy = fy.wrapping_add(FIXED_HALF);
                let mut x = x;
                loop {
                    let lower_y = fixed_floor_to_int(fy);
                    let a = fixed_to_alpha(fy);
                    blitter.blit_anti_v2(x, lower_y - 1, 255 - a, a);
                    fy = fy.wrapping_add(slope);
                    x += 1;
                    if x >= end {
                        break;
                    }
                }

                fy.wrapping_sub(FIXED_HALF)
            }
            // Port of: src/core/SkScan_Antihair.cpp#L199-L216 (chrome/m156)
            AntiHairKind::VLine => {
                let (y, stopy, fx, dx) = (x, end, fy, slope);
                debug_assert!(y < stopy);
                debug_assert_eq!(0, dx);
                let fx = fx.wrapping_add(FIXED_HALF);

                let x = fixed_floor_to_int(fx);
                let mut a = fixed_to_alpha(fx);

                if a != 0 {
                    blitter.blit_v(x, y, stopy - y, to_u8(a));
                }
                a = 255 - a;
                if a != 0 {
                    blitter.blit_v(x - 1, y, stopy - y, to_u8(a));
                }

                fx.wrapping_sub(FIXED_HALF)
            }
            // Port of: src/core/SkScan_Antihair.cpp#L234-L245 (chrome/m156)
            AntiHairKind::Vertish => {
                let (mut y, stopy, mut fx, dx) = (x, end, fy, slope);
                debug_assert!(y < stopy);
                fx = fx.wrapping_add(FIXED_HALF);
                loop {
                    let x = fixed_floor_to_int(fx);
                    let a = fixed_to_alpha(fx);
                    blitter.blit_anti_h2(x - 1, y, 255 - a, a);
                    fx = fx.wrapping_add(dx);
                    y += 1;
                    if y >= stopy {
                        break;
                    }
                }

                fx.wrapping_sub(FIXED_HALF)
            }
        }
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L248-L252 (chrome/m156)
fn fastfixdiv(a: Fdot6, b: Fdot6) -> Fixed {
    debug_assert!(b != 0);
    left_shift(a, 16).wrapping_div(b)
}

// returns high-bit set iff x==0x8000...
// Port of: src/core/SkScan_Antihair.cpp#L258-L260 (chrome/m156)
#[allow(clippy::manual_isolate_lowest_one)] // mirrors the C++ `x & -x`
fn bad_int(x: i32) -> i32 {
    x & x.wrapping_neg()
}

// Port of: src/core/SkScan_Antihair.cpp#L262-L264 (chrome/m156)
fn any_bad_ints(a: i32, b: i32, c: i32, d: i32) -> i32 {
    (bad_int(a) | bad_int(b) | bad_int(c) | bad_int(d)) >> (i32::BITS - 1)
}

// Returns the fractional part of the passed in number.
// e.g. 2.75 -> 0.75
// Port of: src/core/SkScan_Antihair.cpp#L284-L286 (chrome/m156)
fn fd6_frac(x: Fdot6) -> Fdot6 {
    x & (FDOT6_ONE - 1)
}

// We want the fractional part of x or y, but we want multiples of 64 to return 64, not 0, so we
// can't just take the fractional component.
// Port of: src/core/SkScan_Antihair.cpp#L294-L305 (chrome/m156)
fn partial_pixel_coverage(pos: Fdot6) -> Fdot6 {
    let result = fd6_frac(pos.wrapping_sub(1)) + 1;
    debug_assert!(result > 0 && result <= FDOT6_ONE);
    result
}

// Port of: src/core/SkScan_Antihair.cpp#L307-L531 (chrome/m156)
#[allow(clippy::too_many_lines)] // one function in C++
fn do_anti_hairline(
    x0: Fdot6,
    y0: Fdot6,
    x1: Fdot6,
    y1: Fdot6,
    clip: Option<&IRect>,
    blitter: &mut dyn Blitter,
) {
    let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
    let mut clip = clip;

    // check for integer NaN (0x80000000) which we can't handle (can't negate it)
    // It appears typically from a huge float (inf or nan) being converted to int.
    // If we see it, just don't draw.
    if any_bad_ints(x0, y0, x1, y1) != 0 {
        return;
    }

    // The caller must clip the line to [-32767.0 ... 32767.0] ahead of time (in dot6 format)

    if x1.wrapping_sub(x0).wrapping_abs() > int_to_fdot6(511)
        || y1.wrapping_sub(y0).wrapping_abs() > int_to_fdot6(511)
    {
        /*  instead of (x0 + x1) >> 1, we shift each separately. This is less
           precise, but avoids overflowing the intermediate result if the
           values are huge. A better fix might be to clip the original pts
           directly (i.e. do the divide), so we don't spend time subdividing
           huge lines at all.
        */
        let hx = (x0 >> 1) + (x1 >> 1);
        let hy = (y0 >> 1) + (y1 >> 1);
        do_anti_hairline(x0, y0, hx, hy, clip, blitter);
        do_anti_hairline(hx, hy, x1, y1, clip, blitter);
        return;
    }

    let mut start_coverage: i32;
    let mut stop_coverage: i32;
    let mut istart: i32;
    let mut istop: i32;
    let mut fstart: Fixed;
    let slope: Fixed;

    let hair_blitter: AntiHairKind;

    if x1.wrapping_sub(x0).wrapping_abs() > y1.wrapping_sub(y0).wrapping_abs() {
        // mostly horizontal
        if x0 > x1 {
            // we want to go left-to-right
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }

        istart = fdot6_floor(x0);
        istop = fdot6_ceil(x1);
        if y0 == y1 {
            // completely horizontal, take fast case
            slope = 0;
            hair_blitter = AntiHairKind::HLine;
            fstart = fdot6_to_fixed(y0);
        } else {
            slope = fastfixdiv(y1 - y0, x1 - x0);
            debug_assert!(
                (-FIXED_1..=FIXED_1).contains(&slope),
                "should be vertical or mostly vertical"
            );
            // Adjust fstart to be the Y-intercept at the center of the first pixel.
            let dx_to_center: Fdot6 = FDOT6_HALF - fd6_frac(x0);
            fstart = fdot6_to_fixed(y0)
                .wrapping_add((slope.wrapping_mul(dx_to_center).wrapping_add(FDOT6_HALF)) >> 6);
            hair_blitter = AntiHairKind::Horish;
        }

        debug_assert!(istop > istart);
        if istop - istart == 1 {
            // we are within a single pixel
            start_coverage = x1 - x0;
            debug_assert!((0..=FDOT6_ONE).contains(&start_coverage));
            stop_coverage = 0;
        } else {
            start_coverage = FDOT6_ONE - fd6_frac(x0);
            stop_coverage = fd6_frac(x1);
        }

        if let Some(c) = clip {
            if istart >= c.right || istop <= c.left {
                return;
            }
            if istart < c.left {
                fstart = fstart.wrapping_add(slope.wrapping_mul(c.left - istart));
                istart = c.left;
                start_coverage = FDOT6_ONE;
                if istop - istart == 1 {
                    // we are within a single pixel
                    start_coverage = partial_pixel_coverage(x1);
                    stop_coverage = 0;
                }
            }
            if istop > c.right {
                istop = c.right;
                stop_coverage = 0; // so we don't draw this last column
            }

            debug_assert!(istart <= istop);
            if istart == istop {
                return;
            }
            // now test if our Y values are completely inside the clip
            let (mut top, mut bottom);
            if slope >= 0 {
                // T2B
                top = fixed_floor_to_int(fstart.wrapping_sub(FIXED_HALF));
                bottom = fixed_ceil_to_int(
                    fstart
                        .wrapping_add((istop - istart - 1).wrapping_mul(slope))
                        .wrapping_add(FIXED_HALF),
                );
            } else {
                // B2T
                bottom = fixed_ceil_to_int(fstart.wrapping_add(FIXED_HALF));
                top = fixed_floor_to_int(
                    fstart
                        .wrapping_add((istop - istart - 1).wrapping_mul(slope))
                        .wrapping_sub(FIXED_HALF),
                );
            }
            // Expand outset to work around possible numerical calculation bug that lead to
            // overflow
            top -= 1;
            bottom += 1;

            if top >= c.bottom || bottom <= c.top {
                return;
            }
            if c.top <= top && c.bottom >= bottom {
                clip = None;
            }
        }
    } else {
        // mostly vertical
        if y0 > y1 {
            // we want to go top-to-bottom
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }

        istart = fdot6_floor(y0);
        istop = fdot6_ceil(y1);
        if x0 == x1 {
            if y0 == y1 {
                // are we zero length?
                return; // nothing to do
            }
            slope = 0;
            hair_blitter = AntiHairKind::VLine;
            fstart = fdot6_to_fixed(x0);
        } else {
            slope = fastfixdiv(x1 - x0, y1 - y0);
            debug_assert!(
                (-FIXED_1..=FIXED_1).contains(&slope),
                "should be horizontal or mostly horizontal"
            );
            // Adjust fstart to be the X-intercept at the center of the first pixel row.
            let dy_to_center: Fdot6 = FDOT6_HALF - fd6_frac(y0);
            fstart = fdot6_to_fixed(x0)
                .wrapping_add((slope.wrapping_mul(dy_to_center).wrapping_add(FDOT6_HALF)) >> 6);
            hair_blitter = AntiHairKind::Vertish;
        }

        debug_assert!(istop > istart);
        if istop - istart == 1 {
            // we are within a single pixel
            start_coverage = y1 - y0;
            debug_assert!((0..=FDOT6_ONE).contains(&start_coverage));
            stop_coverage = 0;
        } else {
            start_coverage = FDOT6_ONE - fd6_frac(y0);
            stop_coverage = fd6_frac(y1);
        }

        if let Some(c) = clip {
            if istart >= c.bottom || istop <= c.top {
                return;
            }
            if istart < c.top {
                fstart = fstart.wrapping_add(slope.wrapping_mul(c.top - istart));
                istart = c.top;
                start_coverage = FDOT6_ONE;
                if istop - istart == 1 {
                    // we are within a single pixel
                    start_coverage = partial_pixel_coverage(y1);
                    stop_coverage = 0;
                }
            }
            if istop > c.bottom {
                istop = c.bottom;
                stop_coverage = 0; // so we don't draw this last row
            }

            debug_assert!(istart <= istop);
            if istart == istop {
                return;
            }

            // now test if our X values are completely inside the clip
            let (mut left, mut right);
            if slope >= 0 {
                // L2R
                left = fixed_floor_to_int(fstart.wrapping_sub(FIXED_HALF));
                right = fixed_ceil_to_int(
                    fstart
                        .wrapping_add((istop - istart - 1).wrapping_mul(slope))
                        .wrapping_add(FIXED_HALF),
                );
            } else {
                // R2L
                right = fixed_ceil_to_int(fstart.wrapping_add(FIXED_HALF));
                left = fixed_floor_to_int(
                    fstart
                        .wrapping_add((istop - istart - 1).wrapping_mul(slope))
                        .wrapping_sub(FIXED_HALF),
                );
            }
            // Expand outset to work around possible numerical calculation bug that lead to
            // overflow
            left -= 1;
            right += 1;

            if left >= c.right || right <= c.left {
                return;
            }
            if c.left <= left && c.right >= right {
                clip = None;
            }
        }
    }

    let mut rect_clipper;
    let blitter: &mut dyn Blitter = if let Some(c) = clip {
        rect_clipper = RectClipBlitter::new(blitter, *c);
        &mut rect_clipper
    } else {
        blitter
    };

    // be sure we don't draw twice in the same pixel
    debug_assert!(!(start_coverage > 0 && stop_coverage > 0) || istart < istop - 1);

    fstart = hair_blitter.draw_cap(blitter, istart, fstart, slope, start_coverage);
    istart += 1;
    let full_spans = istop - istart - i32::from(stop_coverage > 0);
    if full_spans > 0 {
        fstart = hair_blitter.draw_line(blitter, istart, istart + full_spans, fstart, slope);
    }
    if stop_coverage > 0 {
        hair_blitter.draw_cap(blitter, istop - 1, fstart, slope, stop_coverage);
    }
}

/// Draws `src.len() - 1` antialiased hairline segments, clipped to `clip` (`None` for no clip).
///
/// `SkScan::AntiHairLineRgn`.
// Port of: src/core/SkScan_Antihair.cpp#L533-L609 (chrome/m156)
#[doc(alias = "AntiHairLineRgn")]
pub fn anti_hair_line_rgn(src: &[Point], clip: Option<&Region>, blitter: &mut dyn Blitter) {
    if src.is_empty() || clip.is_some_and(Region::is_empty) {
        return;
    }

    debug_assert!(clip.is_none_or(|c| !c.bounds().is_empty()));

    let max = 32767.0f32;
    let fixed_bounds = Rect::new(-max, -max, max, max);

    let mut clip_bounds = Rect::default();
    if let Some(clip) = clip {
        clip_bounds = Rect::from(*clip.bounds());
        /*  We perform integral clipping later on, but we do a scalar clip first
        to ensure that our coordinates are expressible in fixed/integers.

        antialiased hairlines can draw up to 1/2 of a pixel outside of
        their bounds, so we need to outset the clip before calling the
        clipper. To make the numerics safer, we outset by a whole pixel,
        since the 1/2 pixel boundary is important to the antihair blitter,
        we don't want to risk numerical fate by chopping on that edge.
        */
        clip_bounds.outset(Point::new(1.0, 1.0));
    }

    for i in 0..src.len() - 1 {
        // We have to pre-clip the line to fit in a SkFixed, so we just chop
        // the line. TODO find a way to actually draw beyond that range.
        let Some(mut pts) = intersect_line(&[src[i], src[i + 1]], &fixed_bounds) else {
            continue;
        };

        if clip.is_some() {
            let Some(p) = intersect_line(&pts, &clip_bounds) else {
                continue;
            };
            pts = p;
        }

        let x0 = float_to_fdot6(pts[0].x);
        let y0 = float_to_fdot6(pts[0].y);
        let x1 = float_to_fdot6(pts[1].x);
        let y1 = float_to_fdot6(pts[1].y);

        if let Some(clip) = clip {
            let left = x0.min(x1);
            let top = y0.min(y1);
            let right = x0.max(x1);
            let bottom = y0.max(y1);

            let ir = IRect::new(
                fdot6_floor(left) - 1,
                fdot6_floor(top) - 1,
                fdot6_ceil(right) + 1,
                fdot6_ceil(bottom) + 1,
            );

            if clip.quick_reject_rect(ir) {
                continue;
            }
            if !clip.quick_contains(ir) {
                let mut iter = Cliperator::new(clip, ir);
                while !iter.is_done() {
                    let r = *iter.rect();
                    do_anti_hairline(x0, y0, x1, y1, Some(&r), blitter);
                    iter.next();
                }
                continue;
            }
            // fall through to no-clip case
        }
        do_anti_hairline(x0, y0, x1, y1, None, blitter);
    }
}

/// Draws the outline of `rect` as antialiased hairlines (`SkScan::AntiHairRect`).
// Port of: src/core/SkScan_Antihair.cpp#L611-L621 (chrome/m156)
#[doc(alias = "AntiHairRect")]
pub fn anti_hair_rect(rect: &Rect, clip: &RasterClip, blitter: &mut dyn Blitter) {
    let pts = [
        Point::new(rect.left, rect.top),
        Point::new(rect.right, rect.top),
        Point::new(rect.right, rect.bottom),
        Point::new(rect.left, rect.bottom),
        Point::new(rect.left, rect.top),
    ];
    anti_hair_line(&pts, clip, blitter);
}

/// `SkScan::AntiHairLine`: antialiased hairline segments through `pts`, clipped to a raster clip.
// Port of: src/core/SkScan_Hairline.cpp#L851-L867 (chrome/m156)
#[doc(alias = "AntiHairLine")]
pub fn anti_hair_line(pts: &[Point], clip: &RasterClip, blitter: &mut dyn Blitter) {
    if clip.is_bw() {
        anti_hair_line_rgn(pts, Some(clip.bw_rgn()), blitter);
    } else {
        let r = Rect::bounds_or_empty(pts);

        let rounded: IRect = r.round_out();
        if clip.quick_contains(&rounded.with_outset(IPoint::new(1, 1))) {
            anti_hair_line_rgn(pts, None, blitter);
        } else {
            let mut wrapper = AAClipBlitterWrapper::new(clip, blitter);
            let (rgn, b) = wrapper.parts();
            anti_hair_line_rgn(pts, Some(rgn), b);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

/// 24.8 integer fixed point.
// Port of: src/core/SkScan_Antihair.cpp#L625 (chrome/m156)
type FDot8 = i32;

// Port of: src/core/SkScan_Antihair.cpp#L627-L629 (chrome/m156)
fn fixed_to_fdot8(x: Fixed) -> FDot8 {
    x.wrapping_add(0x80) >> 8
}

// Port of: src/core/SkScan_Antihair.cpp#L631-L655 (chrome/m156)
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // alphas are in 0..=256
fn do_scanline(l: FDot8, top: i32, r: FDot8, alpha: U8CPU, blitter: &mut dyn Blitter) {
    debug_assert!(l < r);

    let alpha_i = alpha as i32;

    if (l >> 8) == ((r - 1) >> 8) {
        // 1x1 pixel
        blitter.blit_v(l >> 8, top, 1, to_u8(alpha_mul(alpha_i, r - l) as u32));
        return;
    }

    let mut left = l >> 8;

    if l & 0xFF != 0 {
        blitter.blit_v(
            left,
            top,
            1,
            to_u8(alpha_mul(alpha_i, 256 - (l & 0xFF)) as u32),
        );
        left += 1;
    }

    let rite = r >> 8;
    let width = rite - left;
    if width > 0 {
        call_hline_blitter(blitter, left, top, width, alpha);
    }
    if r & 0xFF != 0 {
        blitter.blit_v(rite, top, 1, to_u8(alpha_mul(alpha_i, r & 0xFF) as u32));
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L657-L699 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // the alphas are in 0..=256 by construction
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
            blitter.blit_v(left, top, height, to_u8((r - l - 1) as u32));
        } else {
            if l & 0xFF != 0 {
                blitter.blit_v(left, top, height, to_u8((256 - (l & 0xFF)) as u32));
                left += 1;
            }
            let rite = r >> 8;
            let width = rite - left;
            if width > 0 && fill_inner {
                blitter.blit_rect(left, top, width, height);
            }
            if r & 0xFF != 0 {
                blitter.blit_v(rite, top, height, to_u8((r & 0xFF) as u32));
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

///////////////////////////////////////////////////////////////////////////////

/// Fills the fixed-point rect `xr` with antialiasing, clipped to `clip` (`None`: no clip)
/// (`SkScan::AntiFillXRect` with a `const SkRegion*`).
// Port of: src/core/SkScan_Antihair.cpp#L709-L746 (chrome/m156)
#[doc(alias = "AntiFillXRect")]
pub fn anti_fill_x_rect(xr: &XRect, clip: Option<&Region>, blitter: &mut dyn Blitter) {
    let Some(clip) = clip else {
        antifillrect_x(xr, blitter);
        return;
    };
    let outer_bounds = xrect_round_out(xr);

    if clip.is_rect() {
        let clip_bounds = *clip.bounds();

        if clip_bounds.contains(outer_bounds) {
            antifillrect_x(xr, blitter);
        } else {
            // this keeps our original edges fractional
            let tmp_r = xrect_set_irect(&clip_bounds);
            if let Some(tmp_r) = IRect::intersect(&tmp_r, xr) {
                antifillrect_x(&tmp_r, blitter);
            }
        }
    } else {
        let mut clipper = Cliperator::new(clip, outer_bounds);

        while !clipper.is_done() {
            // this keeps our original edges fractional
            let tmp_r = xrect_set_irect(clipper.rect());
            if let Some(tmp_r) = IRect::intersect(&tmp_r, xr) {
                antifillrect_x(&tmp_r, blitter);
            }
            clipper.next();
        }
    }
}

/// `SkScan::AntiFillXRect` with a raster clip.
// Port of: src/core/SkScan_Antihair.cpp#L748-L763 (chrome/m156)
#[doc(alias = "AntiFillXRect")]
pub fn anti_fill_x_rect_clip(xr: &XRect, clip: &RasterClip, blitter: &mut dyn Blitter) {
    if clip.is_bw() {
        anti_fill_x_rect(xr, Some(clip.bw_rgn()), blitter);
    } else {
        let outer_bounds = xrect_round_out(xr);

        if clip.quick_contains(&outer_bounds) {
            anti_fill_x_rect(xr, None, blitter);
        } else {
            let mut wrapper = AAClipBlitterWrapper::new(clip, blitter);
            let (rgn, b) = wrapper.parts();
            anti_fill_x_rect(xr, Some(rgn), b);
        }
    }
}

/*  This takes a float-rect, but with the key improvement that it has
    already been clipped, so we know that it is safe to convert it into a
    XRect (fixedpoint), as it won't overflow.
*/
// Port of: src/core/SkScan_Antihair.cpp#L769-L774 (chrome/m156)
fn antifillrect(r: &Rect, blitter: &mut dyn Blitter) {
    let xr = xrect_set_rect(r);
    antifillrect_x(&xr, blitter);
}

/*  We repeat the clipping logic of AntiFillXRect because the float rect might
    overflow if we blindly converted it to an XRect. This sucks that we have to
    repeat the clipping logic, but I don't see how to share the code/logic.

    We clip r (as needed) into one or more (smaller) float rects, and then pass
    those to our version of antifillrect, which converts it into an XRect and
    then calls the blit.
*/
/// `SkScan::AntiFillRect` with a `const SkRegion*`.
// Port of: src/core/SkScan_Antihair.cpp#L784-L810 (chrome/m156)
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
            let mut clipper = Cliperator::new(clip, outer_bounds);
            while !clipper.is_done() {
                new_r = Rect::from(*clipper.rect());
                if new_r.intersect(orig_r) {
                    antifillrect(&new_r, blitter);
                }
                clipper.next();
            }
        }
    } else {
        antifillrect(orig_r, blitter);
    }
}

/// `SkScan::AntiFillRect` with a raster clip.
// Port of: src/core/SkScan_Antihair.cpp#L812-L820 (chrome/m156)
#[doc(alias = "AntiFillRect")]
pub fn anti_fill_rect_clip(r: &Rect, clip: &RasterClip, blitter: &mut dyn Blitter) {
    if clip.is_bw() {
        anti_fill_rect(r, Some(clip.bw_rgn()), blitter);
    } else {
        let mut wrapper = AAClipBlitterWrapper::new(clip, blitter);
        let (rgn, b) = wrapper.parts();
        anti_fill_rect(r, Some(rgn), b);
    }
}

///////////////////////////////////////////////////////////////////////////////

// calls blitRect() if the rectangle is non-empty
// Port of: src/core/SkScan_Antihair.cpp#L827-L831 (chrome/m156)
fn fillcheckrect(l: i32, t: i32, r: i32, b: i32, blitter: &mut dyn Blitter) {
    if l < r && t < b {
        blitter.blit_rect(l, t, r - l, b - t);
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L833-L835 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the (int) cast, range-checked below
fn scalar_to_fdot8(x: f32) -> FDot8 {
    // skia-rust: the C++ cast is undefined for NaN/out of range; mirror the x86 result
    // (`i32::MIN`), as `float_to_fdot6` does.
    let v = x * 256.0;
    if v >= 2_147_483_648.0_f32 || v < -2_147_483_648.0_f32 || v.is_nan() {
        i32::MIN
    } else {
        v as i32
    }
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
// Port of: src/core/SkScan_Antihair.cpp#L846-L850 (chrome/m156)
fn inv_alpha_mul(a: U8CPU, b: U8CPU) -> U8CPU {
    // need precise rounding (not just SkAlphaMul) so that values like
    // a=228, b=252 don't overflow the result
    a + b - mul_div_255_round(a, b)
}

// Port of: src/core/SkScan_Antihair.cpp#L852-L880 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // all values are in 0..=256 here
fn inner_scanline(l: FDot8, top: i32, r: FDot8, alpha: U8CPU, blitter: &mut dyn Blitter) {
    debug_assert!(l < r);

    if (l >> 8) == ((r - 1) >> 8) {
        // 1x1 pixel
        let mut wid_clamp: FDot8 = r - l;
        // border case clamp 256 to 255 instead of going through call_hline_blitter
        // see skbug/4406
        wid_clamp -= wid_clamp >> 8;
        blitter.blit_v(
            l >> 8,
            top,
            1,
            to_u8(inv_alpha_mul(alpha, wid_clamp as u32)),
        );
        return;
    }

    let mut left = l >> 8;
    if l & 0xFF != 0 {
        blitter.blit_v(left, top, 1, to_u8(inv_alpha_mul(alpha, (l & 0xFF) as u32)));
        left += 1;
    }

    let rite = r >> 8;
    let width = rite - left;
    if width > 0 {
        call_hline_blitter(blitter, left, top, width, alpha);
    }

    if r & 0xFF != 0 {
        blitter.blit_v(
            rite,
            top,
            1,
            to_u8(inv_alpha_mul(alpha, (!r & 0xFF) as u32)),
        );
    }
}

// Port of: src/core/SkScan_Antihair.cpp#L882-L915 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // all values are in 0..=256 here
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
            blitter.blit_v(l >> 8, top, height, to_u8((l & 0xFF) as u32));
        }
        if r & 0xFF != 0 {
            blitter.blit_v(r >> 8, top, height, to_u8((!r & 0xFF) as u32));
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

/// Strokes the frame of `r` with antialiasing (`SkScan::AntiFrameRect` with a
/// `const SkRegion*`).
// Port of: src/core/SkScan_Antihair.cpp#L926-L1015 (chrome/m156)
#[doc(alias = "AntiFrameRect")]
pub fn anti_frame_rect(
    r: &Rect,
    stroke_size: &Point,
    clip: Option<&Region>,
    blitter: &mut dyn Blitter,
) {
    debug_assert!(stroke_size.x >= 0.0 && stroke_size.y >= 0.0);

    let mut rx = stroke_size.x / 2.0f32;
    let mut ry = stroke_size.y / 2.0f32;

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
    let outer = IRect::new(
        fdot8_floor(outer_l),
        fdot8_floor(outer_t),
        fdot8_ceil(outer_r),
        fdot8_ceil(outer_b),
    );

    let mut clipper = BlitterClipper::new();
    let blitter: &mut dyn Blitter = if let Some(clip) = clip {
        if clip.quick_reject_rect(outer) {
            return;
        }
        // now we can ignore clip for the rest of the function
        if clip.contains_rect(outer) {
            blitter
        } else {
            clipper.apply(blitter, Some(clip), Some(&outer))
        }
    } else {
        blitter
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
    let outer = IRect::new(
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

/// `SkScan::AntiFrameRect` with a raster clip.
// Port of: src/core/SkScan_Antihair.cpp#L1017-L1025 (chrome/m156)
#[doc(alias = "AntiFrameRect")]
pub fn anti_frame_rect_clip(
    r: &Rect,
    stroke_size: &Point,
    clip: &RasterClip,
    blitter: &mut dyn Blitter,
) {
    if clip.is_bw() {
        anti_frame_rect(r, stroke_size, Some(clip.bw_rgn()), blitter);
    } else {
        let mut wrapper = AAClipBlitterWrapper::new(clip, blitter);
        let (rgn, b) = wrapper.parts();
        anti_frame_rect(r, stroke_size, Some(rgn), b);
    }
}

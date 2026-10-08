// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMaskBlurFilter.h, src/core/SkMaskBlurFilter.cpp

//! `SkMaskBlurFilter`: a single channel Gaussian blur of a [`Mask`]. The specifics of the
//! implementation are taken from <https://drafts.fxtf.org/filters/#feGaussianBlurElement>.
//!
//! skia-rust: Skia's small-sigma path (`small_blur`) uses `skvx::Vec<8, uint16_t>` for fixed
//! point 8.8 arithmetic. Every operation it uses (wrapping adds, `mulhi`, shifts, lane
//! selection) is exact integer arithmetic, identical on every CPU tier, so the lanes are plain
//! `[u16; 8]` arrays here and there is no SIMD kernel (and no scalar twin) to keep in sync.

use crate::color_data::{packed16_to_b32, packed16_to_g32, packed16_to_r32};
use crate::color_priv::get_packed_a32;
use crate::gauss_filter::{GAUSS_ARRAY_MAX, GaussFilter};
use crate::mask::{Mask, MaskBuilder, MaskFormat};
use crate::point::IPoint;
use crate::t_pin::t_pin;

// ---------------------------------------------------------------------------------------------
// PlanGauss

// Port of: src/core/SkMaskBlurFilter.cpp#L28-L162 (chrome/m156)
struct PlanGauss {
    weight: u64,
    border: i32,
    sliding_window: i32,
    pass0_size: usize,
    pass1_size: usize,
    pass2_size: usize,
}

// Port of: src/core/SkMaskBlurFilter.cpp#L30-L95 (chrome/m156)
impl PlanGauss {
    // SK_DoublePI (3.14159265358979323846264338327950288 is the same double)
    const DOUBLE_PI: f64 = std::f64::consts::PI;

    #[allow(
        clippy::cast_possible_truncation, // mirrors the (int) and (uint64_t) conversions
        clippy::cast_sign_loss,
        clippy::cast_precision_loss // mirrors the (1ull << 32) to double conversion (exact)
    )]
    fn new(sigma: f64) -> PlanGauss {
        let possible_window =
            (sigma * 3.0 * (2.0 * Self::DOUBLE_PI).sqrt() / 4.0 + 0.5).floor() as i32;
        let window = std::cmp::max(1, possible_window);

        let pass0_size = (window - 1) as usize;
        let pass1_size = (window - 1) as usize;
        let pass2_size = (if (window & 1) == 1 {
            window - 1
        } else {
            window
        }) as usize;

        // Calculating the border is tricky. See the long comment in Skia. For odd windows the
        // border is 3*((window - 1)/2), for even windows 3 * (window/2) - 1.
        let border = if (window & 1) == 1 {
            3 * ((window - 1) / 2)
        } else {
            3 * (window / 2) - 1
        };
        let sliding_window = 2 * border + 1;

        // If the window is odd then the divisor is just window ^ 3 otherwise,
        // it is window * window * (window + 1) = window ^ 2 + window ^ 3;
        let window2 = window.wrapping_mul(window);
        let window3 = window2.wrapping_mul(window);
        let divisor = if (window & 1) == 1 {
            window3
        } else {
            window3.wrapping_add(window2)
        };

        let weight = (1.0 / f64::from(divisor) * (1u64 << 32) as f64).round() as u64;

        PlanGauss {
            weight,
            border,
            sliding_window,
            pass0_size,
            pass1_size,
            pass2_size,
        }
    }

    fn buffer_size(&self) -> usize {
        self.pass0_size + self.pass1_size + self.pass2_size
    }

    fn border(&self) -> i32 {
        self.border
    }

    // Port of: src/core/SkMaskBlurFilter.cpp#L134-L150 (chrome/m156)
    fn make_blur_scan(&self, width: i32) -> Scan {
        let no_change_count = if self.sliding_window > width {
            self.sliding_window - width
        } else {
            0
        };
        Scan {
            weight: self.weight,
            no_change_count: usize::try_from(no_change_count).expect("non-negative"),
            sizes: [self.pass0_size, self.pass1_size, self.pass2_size],
        }
    }
}

/// The source of a scan: the alpha values of a row (or column) of a mask.
pub(crate) struct Alphas<'a> {
    pub(crate) data: &'a [u8],
    pub(crate) format: MaskFormat,
}

impl Alphas<'_> {
    // The `operator*` of `SkMask::AlphaIter<F>` at element `i` of the row.
    // Port of: src/core/SkMask.h#L136-L215 (chrome/m156)
    pub(crate) fn at(&self, i: usize) -> u8 {
        match self.format {
            MaskFormat::BW => {
                if (self.data[i >> 3] >> (7 - (i & 7))) & 1 != 0 {
                    0xFF
                } else {
                    0
                }
            }
            MaskFormat::A8 => self.data[i],
            MaskFormat::Argb32 => {
                let p = u32::from_ne_bytes([
                    self.data[4 * i],
                    self.data[4 * i + 1],
                    self.data[4 * i + 2],
                    self.data[4 * i + 3],
                ]);
                #[allow(clippy::cast_possible_truncation)] // an alpha is a byte
                {
                    get_packed_a32(p) as u8
                }
            }
            MaskFormat::Lcd16 => {
                let packed =
                    u32::from(u16::from_ne_bytes([self.data[2 * i], self.data[2 * i + 1]]));
                lcd_alpha(packed)
            }
            _ => unreachable!("Unhandled format."),
        }
    }
}

// The alpha of an LCD16 value: `(r + g + b) / 3`.
fn lcd_alpha(packed: u32) -> u8 {
    let r = packed16_to_r32(packed);
    let g = packed16_to_g32(packed);
    let b = packed16_to_b32(packed);
    #[allow(clippy::cast_possible_truncation)] // (r + g + b) / 3 <= 255
    {
        ((r + g + b) / 3) as u8
    }
}

// Port of: src/core/SkMaskBlurFilter.cpp#L64-L132 (chrome/m156)
struct Scan {
    weight: u64,
    no_change_count: usize,
    sizes: [usize; 3],
}

// The three circular buffers of a scan and their cursors.
struct Rings<'a> {
    bufs: [&'a mut [u32]; 3],
    cursors: [usize; 3],
}

impl Rings<'_> {
    // Replaces the oldest element of ring `k` with `value` and returns the old one. A ring of
    // size 0 (a window of 1, which Skia's callers never use) holds nothing.
    fn replace(&mut self, k: usize, value: u32) -> u32 {
        let buf = &mut self.bufs[k];
        if buf.is_empty() {
            return 0;
        }
        let c = self.cursors[k];
        let old = buf[c];
        buf[c] = value;
        self.cursors[k] = if c + 1 < buf.len() { c + 1 } else { 0 };
        old
    }
}

impl Scan {
    const HALF: u64 = 1u64 << 31;

    // Port of: src/core/SkMaskBlurFilter.cpp#L117-L123 (chrome/m156)
    fn final_scale(&self, sum: u32) -> u8 {
        let v = (self
            .weight
            .wrapping_mul(u64::from(sum))
            .wrapping_add(Self::HALF))
            >> 32;
        debug_assert!(v <= 255);
        #[allow(clippy::cast_possible_truncation)] // mirrors SkTo<uint8_t>, which asserts it fits
        {
            v as u8
        }
    }

    // One step of the scan: consumes `leading_edge` and returns the pixel it produces.
    fn step(&self, rings: &mut Rings<'_>, sums: &mut [u32; 3], leading_edge: u32) -> u8 {
        sums[0] = sums[0].wrapping_add(leading_edge);
        sums[1] = sums[1].wrapping_add(sums[0]);
        sums[2] = sums[2].wrapping_add(sums[1]);

        let out = self.final_scale(sums[2]);

        sums[2] = sums[2].wrapping_sub(rings.replace(2, sums[1]));
        sums[1] = sums[1].wrapping_sub(rings.replace(1, sums[0]));
        sums[0] = sums[0].wrapping_sub(rings.replace(0, leading_edge));
        out
    }

    /// Blurs the `src_len` alphas of `src` into `dst`, starting at index `dst_start` and
    /// advancing by `dst_stride`; `dst_end` is the index one past the last element.
    // Port of: src/core/SkMaskBlurFilter.cpp#L76-L115 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // the C++ passes the buffers and cursors through members
    fn blur(
        &self,
        buffer: &mut [u32],
        src: &Alphas<'_>,
        src_len: usize,
        dst: &mut [u8],
        dst_start: usize,
        dst_stride: usize,
        dst_end: usize,
    ) {
        let (b0, rest) = buffer.split_at_mut(self.sizes[0]);
        let (b1, rest) = rest.split_at_mut(self.sizes[1]);
        let b2 = &mut rest[..self.sizes[2]];
        let mut rings = Rings {
            bufs: [b0, b1, b2],
            cursors: [0; 3],
        };

        for ring in &mut rings.bufs {
            ring.fill(0);
        }

        let mut sums = [0u32; 3];

        // Consume the source generating pixels.
        let mut dst_idx = dst_start;
        for i in 0..src_len {
            let leading_edge = u32::from(src.at(i));
            dst[dst_idx] = self.step(&mut rings, &mut sums, leading_edge);
            dst_idx += dst_stride;
        }

        // The leading edge is off the right side of the mask.
        for _ in 0..self.no_change_count {
            dst[dst_idx] = self.step(&mut rings, &mut sums, 0);
            dst_idx += dst_stride;
        }

        // Starting from the right, fill in the rest of the buffer.
        for ring in &mut rings.bufs {
            ring.fill(0);
        }

        sums = [0; 3];

        let mut dst_cursor = dst_end;
        let mut src_idx = src_len;
        while dst_cursor > dst_idx {
            dst_cursor -= dst_stride;
            src_idx -= 1;
            let leading_edge = u32::from(src.at(src_idx));
            dst[dst_cursor] = self.step(&mut rings, &mut sums, leading_edge);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// small_blur

// 8-wide fixed point 8.8.
type Fp88 = [u16; 8];

const HALF: u16 = 0x80;

// `skvx::mulhi` on uint16 lanes: the high 16 bits of the product.
fn mulhi(a: &Fp88, b: u16) -> Fp88 {
    let mut out = [0u16; 8];
    for (o, &x) in out.iter_mut().zip(a) {
        #[allow(clippy::cast_possible_truncation)] // the high half of a 32-bit product
        {
            *o = ((u32::from(x) * u32::from(b)) >> 16) as u16;
        }
    }
    out
}

fn add(a: &Fp88, b: &Fp88) -> Fp88 {
    let mut out = [0u16; 8];
    for i in 0..8 {
        out[i] = a[i].wrapping_add(b[i]);
    }
    out
}

fn add_scalar(a: &Fp88, b: u16) -> Fp88 {
    a.map(|x| x.wrapping_add(b))
}

// Loads `width` (<= 8) values of the row at byte offset `off` of `src` as A8 and converts them
// to 8.8 fixed point; the rest are zero.
// Port of: src/core/SkMaskBlurFilter.cpp#L283-L315 (chrome/m156)
fn load_column(src: &Mask<'_>, off: usize, width: usize) -> Fp88 {
    debug_assert!(width <= 8);
    let mut tmp = [0u8; 8];
    match src.format {
        MaskFormat::BW => {
            // bw_to_a8
            let masks = src.image[off];
            for (i, t) in tmp.iter_mut().enumerate().take(width) {
                *t = if (masks >> (7 - i)) & 1 != 0 {
                    0xFF
                } else {
                    0x00
                };
            }
        }
        MaskFormat::A8 => {
            tmp[..width].copy_from_slice(&src.image[off..off + width]);
        }
        MaskFormat::Argb32 => {
            // argb32_to_a8
            for (i, t) in tmp.iter_mut().enumerate().take(width) {
                let p = u32::from_ne_bytes([
                    src.image[off + 4 * i],
                    src.image[off + 4 * i + 1],
                    src.image[off + 4 * i + 2],
                    src.image[off + 4 * i + 3],
                ]);
                #[allow(clippy::cast_possible_truncation)] // an alpha is a byte
                {
                    *t = get_packed_a32(p) as u8;
                }
            }
        }
        MaskFormat::Lcd16 => {
            // lcd_to_a8
            for (i, t) in tmp.iter_mut().enumerate().take(width) {
                let rgb = u32::from(u16::from_ne_bytes([
                    src.image[off + 2 * i],
                    src.image[off + 2 * i + 1],
                ]));
                *t = lcd_alpha(rgb);
            }
        }
        _ => unreachable!("Unhandled format."),
    }

    tmp.map(|b| u16::from(b) << 8)
}

// Stores the first `width` values of `v` as bytes at `dst[at..]`.
// Port of: src/core/SkMaskBlurFilter.cpp#L317-L328 (chrome/m156)
fn store(dst: &mut [u8], at: usize, v: &Fp88, width: usize) {
    for (i, &x) in v.iter().enumerate().take(width) {
        #[allow(clippy::cast_possible_truncation)] // mirrors skvx::cast<uint8_t>(v >> 8)
        {
            dst[at + i] = (x >> 8) as u8;
        }
    }
}

// blur_x_radius_1..4: adds the contribution of the 8 source values `s0` to `d0` and `d8`.
// The value `S[n]` contributes `G[|radius - s|] * S[n]` to `D[n + s]` for `s` in `0..=2*radius`.
// Port of: src/core/SkMaskBlurFilter.cpp#L396-L544 (chrome/m156)
fn blur_x_radius(
    radius: usize,
    s0: &Fp88,
    g: &[u16; GAUSS_ARRAY_MAX],
    d0: &mut Fp88,
    d8: &mut Fp88,
) {
    let mut v = [[0u16; 8]; 5];
    for (k, vk) in v.iter_mut().enumerate().take(radius + 1) {
        *vk = mulhi(s0, g[k]);
    }

    for s in 0..=2 * radius {
        let k = radius.abs_diff(s);
        for (i, &value) in v[k].iter().enumerate() {
            let p = i + s;
            if p < 8 {
                d0[p] = d0[p].wrapping_add(value);
            } else {
                d8[p - 8] = d8[p - 8].wrapping_add(value);
            }
        }
    }
}

// Port of: src/core/SkMaskBlurFilter.cpp#L549-L582 (chrome/m156)
fn blur_row(
    radius: usize,
    g: &[u16; GAUSS_ARRAY_MAX],
    src: &[u8],
    src_w: usize,
    dst: &mut [u8],
    dst_w: usize,
) {
    // Clear the buffer to handle summing wider than source.
    let mut d0: Fp88 = [HALF; 8];
    let mut d8: Fp88 = [HALF; 8];

    // load(src, 8, nullptr) / load(src, tail, nullptr) from an A8 row
    let load = |x: usize, width: usize| -> Fp88 {
        let mut tmp = [0u8; 8];
        tmp[..width].copy_from_slice(&src[x..x + width]);
        tmp.map(|b| u16::from(b) << 8)
    };

    // Go by multiples of 8 in src.
    let mut x = 0;
    while x + 8 <= src_w {
        blur_x_radius(radius, &load(x, 8), g, &mut d0, &mut d8);

        store(dst, x, &d0, 8);

        d0 = d8;
        d8 = [HALF; 8];

        x += 8;
    }

    // There are src values left, but the remainder of src values is not a multiple of 8.
    let src_tail = src_w - x;
    if src_tail > 0 {
        blur_x_radius(radius, &load(x, src_tail), g, &mut d0, &mut d8);

        let dst_tail = std::cmp::min(8, dst_w - x);
        store(dst, x, &d0, dst_tail);

        d0 = d8;
        x += dst_tail;
    }

    // There are dst mask values to complete.
    let dst_tail = dst_w - x;
    if dst_tail > 0 {
        store(dst, x, &d0, dst_tail);
    }
}

// blur_y_radius_1..4: one step of the vertical blur. `d` holds the pending sums
// (`d01, d12, ..., d78`); returns the finished row.
//   answer = d[0] + S*G[radius]; d[t-1] = d[t] + S*G[|radius - t|]; d[2r-1] = S*G[radius] + kHalf
// Port of: src/core/SkMaskBlurFilter.cpp#L664-L736 (chrome/m156)
fn blur_y_radius(radius: usize, s0: &Fp88, g: &[u16; GAUSS_ARRAY_MAX], d: &mut [Fp88; 8]) -> Fp88 {
    let mut v = [[0u16; 8]; 5];
    for (k, vk) in v.iter_mut().enumerate().take(radius + 1) {
        *vk = mulhi(s0, g[k]);
    }

    let answer = add(&d[0], &v[radius]);
    for t in 1..2 * radius {
        let k = radius.abs_diff(t);
        d[t - 1] = add(&d[t], &v[k]);
    }
    d[2 * radius - 1] = add_scalar(&v[radius], HALF);

    answer
}

// Port of: src/core/SkMaskBlurFilter.cpp#L741-L788 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn blur_column(
    radius: usize,
    width: usize,
    g: &[u16; GAUSS_ARRAY_MAX],
    src: &Mask<'_>,
    src_off: usize,
    src_h: i32,
    dst: &mut [u8],
    dst_off: usize,
    dst_rb: usize,
) {
    let mut d: [Fp88; 8] = [[HALF; 8]; 8];
    let src_rb = src.row_bytes as usize;

    let mut dst_at = dst_off;
    for y in 0..usize::try_from(src_h).expect("non-negative") {
        let s = load_column(src, src_off + y * src_rb, width);
        let b = blur_y_radius(radius, &s, g, &mut d);
        store(dst, dst_at, &b, width);
        dst_at += dst_rb;
    }

    // flush
    for pair in 0..radius {
        store(dst, dst_at, &d[2 * pair], width);
        dst_at += dst_rb;
        store(dst, dst_at, &d[2 * pair + 1], width);
        dst_at += dst_rb;
    }
}

// Port of: src/core/SkMaskBlurFilter.cpp#L790-L848 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn blur_y_rect(
    stride_of_8: usize,
    radius: usize,
    g: &[u16; GAUSS_ARRAY_MAX],
    src: &Mask<'_>,
    src_w: i32,
    src_h: i32,
    dst: &mut [u8],
    dst_base: usize,
    dst_rb: usize,
) {
    let src_w = usize::try_from(src_w).expect("non-negative");
    let mut x = 0;
    let mut src_off = 0;
    let mut dst_off = dst_base;
    while x + 8 <= src_w {
        blur_column(radius, 8, g, src, src_off, src_h, dst, dst_off, dst_rb);
        src_off += stride_of_8;
        dst_off += 8;
        x += 8;
    }

    let x_tail = src_w - x;
    if x_tail > 0 {
        blur_column(radius, x_tail, g, src, src_off, src_h, dst, dst_off, dst_rb);
    }
}

// Port of: src/core/SkMaskBlurFilter.cpp#L870-L956 (chrome/m156)
fn small_blur(sigma_x: f64, sigma_y: f64, src: &Mask<'_>, dst: &mut MaskBuilder) -> IPoint {
    debug_assert_eq!(sigma_x.to_bits(), sigma_y.to_bits()); // TODO
    debug_assert!((0.01..2.0).contains(&sigma_x));
    debug_assert!((0.01..2.0).contains(&sigma_y));

    let filter_x = GaussFilter::new(sigma_x);
    let filter_y = GaussFilter::new(sigma_y);

    let radius_x = filter_x.radius();
    let radius_y = filter_y.radius();

    debug_assert!(radius_x <= 4 && radius_y <= 4);

    let prepare_gauss = |filter: &GaussFilter| -> [u16; GAUSS_ARRAY_MAX] {
        let mut factors = [0u16; GAUSS_ARRAY_MAX];
        for (i, &d) in filter.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            // mirrors static_cast<uint16_t>(round(d * (1 << 16)))
            {
                factors[i] = (d * f64::from(1 << 16)).round() as u16;
            }
        }
        factors
    };

    let gauss_factors_x = prepare_gauss(&filter_x);
    let gauss_factors_y = prepare_gauss(&filter_y);

    *dst = MaskBuilder::prepare_destination(radius_x, radius_y, src);
    if src.image.is_empty() {
        return IPoint::new(radius_x, radius_y);
    }
    if dst.image.is_empty() {
        dst.bounds.set_empty();
        return IPoint::new(0, 0);
    }

    let src_w = src.bounds.width();
    let src_h = src.bounds.height();

    let dst_w = usize::try_from(dst.bounds.width()).expect("non-negative");
    let dst_h = usize::try_from(dst.bounds.height()).expect("non-negative");

    let dst_rb = dst.row_bytes as usize;
    let rx = usize::try_from(radius_x).expect("non-negative");
    let ry = usize::try_from(radius_y).expect("non-negative");

    // TODO: handle bluring in only one direction.

    // Blur vertically and copy to destination.
    let stride_of_8 = match src.format {
        MaskFormat::BW => 1,
        MaskFormat::A8 => 8,
        MaskFormat::Argb32 => 32,
        MaskFormat::Lcd16 => 16,
        _ => unreachable!("Unhandled format."),
    };
    blur_y_rect(
        stride_of_8,
        ry,
        &gauss_factors_y,
        src,
        src_w,
        src_h,
        &mut dst.image,
        rx,
        dst_rb,
    );

    // Blur horizontally in place.
    let src_w = usize::try_from(src_w).expect("non-negative");
    for y in 0..dst_h {
        let row = y * dst_rb;
        let src_row: Vec<u8> = dst.image[row + rx..row + rx + src_w].to_vec();
        blur_row(
            rx,
            &gauss_factors_x,
            &src_row,
            src_w,
            &mut dst.image[row..row + dst_w],
            dst_w,
        );
    }

    IPoint::new(radius_x, radius_y)
}

// ---------------------------------------------------------------------------------------------

/// Implements a single channel Gaussian blur of masks.
// Port of: src/core/SkMaskBlurFilter.h#L18-L34 (chrome/m156)
#[doc(alias = "SkMaskBlurFilter")]
#[derive(Copy, Clone, Debug)]
pub struct MaskBlurFilter {
    sigma_w: f64,
    sigma_h: f64,
}

impl MaskBlurFilter {
    /// Creates an object suitable for filtering a [`Mask`] using a filter with width `sigma_w`
    /// and height `sigma_h`.
    // NB 135 is the largest sigma that will not cause a buffer full of 255 mask values to
    // overflow using the Gauss filter. It also limits the size of buffers used hold intermediate
    // values.
    // Port of: src/core/SkMaskBlurFilter.cpp#L180-L186 (chrome/m156)
    #[must_use]
    pub fn new(sigma_w: f64, sigma_h: f64) -> MaskBlurFilter {
        debug_assert!(sigma_w >= 0.0);
        debug_assert!(sigma_h >= 0.0);
        MaskBlurFilter {
            sigma_w: t_pin(sigma_w, 0.0, 135.0),
            sigma_h: t_pin(sigma_h, 0.0, 135.0),
        }
    }

    /// Returns true iff the sigmas will result in an identity mask (no blurring).
    // If the sigma value is less than a certain amount, the window will be 0 which means there is
    // effectively no blur. Using Wolfram alpha to solve the equation used for possibleWindow
    // above shows that the threshold is (2 * sqrt(2/pi))/3. However, historically we used 1/3 as
    // the cutoff.
    // Port of: src/core/SkMaskBlurFilter.cpp#L188-L201 (chrome/m156)
    #[doc(alias = "hasNoBlur")]
    #[must_use]
    pub fn has_no_blur(&self) -> bool {
        const NO_WINDOW_SIGMA: f64 = 1. / 3.;
        self.sigma_w < NO_WINDOW_SIGMA && self.sigma_h <= NO_WINDOW_SIGMA
    }

    /// Given a `src` mask, generates `dst`, returning the border width and height.
    ///
    /// # Panics
    /// If a mask dimension or `src`'s image is inconsistent with its bounds and row bytes.
    // Port of: src/core/SkMaskBlurFilter.cpp#L958-L1065 (chrome/m156)
    #[must_use]
    pub fn blur(&self, src: &Mask<'_>, dst: &mut MaskBuilder) -> IPoint {
        if self.sigma_w < 2.0 && self.sigma_h < 2.0 {
            return small_blur(self.sigma_w, self.sigma_h, src, dst);
        }

        let plan_w = PlanGauss::new(self.sigma_w);
        let plan_h = PlanGauss::new(self.sigma_h);

        let border_w = plan_w.border();
        let border_h = plan_h.border();
        debug_assert!(border_h >= 0 && border_w >= 0);

        *dst = MaskBuilder::prepare_destination(border_w, border_h, src);
        if src.image.is_empty() {
            return IPoint::new(border_w, border_h);
        }
        if dst.image.is_empty() {
            dst.bounds.set_empty();
            return IPoint::new(0, 0);
        }

        let src_w = src.bounds.width();
        let src_h = src.bounds.height();
        let dst_w = dst.bounds.width();
        let dst_h = dst.bounds.height();
        debug_assert!(src_w >= 0 && src_h >= 0 && dst_w >= 0 && dst_h >= 0);

        let buffer_size = std::cmp::max(plan_w.buffer_size(), plan_h.buffer_size());
        let mut buffer = vec![0u32; buffer_size];

        // Blur both directions.
        let tmp_w = src_h;
        let tmp_h = dst_w;

        // Make sure not to overflow the multiply for the tmp buffer size.
        if tmp_w != 0 && tmp_h > i32::MAX / tmp_w {
            return IPoint::new(0, 0);
        }
        let to_usize = |v: i32| usize::try_from(v).expect("non-negative");
        let src_cols = to_usize(src_w);
        let src_rows = to_usize(src_h);
        let tmp_cols = to_usize(tmp_w);
        let tmp_rows = to_usize(tmp_h);
        let mut tmp = vec![0u8; tmp_cols * tmp_rows];

        // Blur horizontally, and transpose.
        let scan_w = plan_w.make_blur_scan(src_w);
        let src_rb = src.row_bytes as usize;
        for y in 0..src_rows {
            let alphas = Alphas {
                data: &src.image[y * src_rb..],
                format: src.format,
            };
            scan_w.blur(
                &mut buffer,
                &alphas,
                src_cols,
                &mut tmp,
                y,
                tmp_cols,
                y + tmp_cols * tmp_rows,
            );
        }

        // Blur vertically (scan in memory order because of the transposition),
        // and transpose back to the original orientation.
        let scan_h = plan_h.make_blur_scan(tmp_w);
        let dst_rb = dst.row_bytes as usize;
        let dst_rows = to_usize(dst_h);
        for y in 0..tmp_rows {
            let row = &tmp[y * tmp_cols..(y + 1) * tmp_cols];
            let alphas = Alphas {
                data: row,
                format: MaskFormat::A8,
            };
            scan_h.blur(
                &mut buffer,
                &alphas,
                tmp_cols,
                &mut dst.image,
                y,
                dst_rb,
                y + dst_rb * dst_rows,
            );
        }

        IPoint::new(border_w, border_h)
    }
}

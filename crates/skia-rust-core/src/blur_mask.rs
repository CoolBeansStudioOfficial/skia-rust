// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlurMask.h, src/core/SkBlurMask.cpp

//! `SkBlurMask`: blurs of masks: the box (gaussian approximating) blur, the analytic blur of a
//! rectangle and the "ground truth" gaussian convolution.
//!
//! skia-rust: `SkBlurMask::BlurRRect` is declared in Skia's header but never defined, so it is
//! not ported.

use crate::blur_types::BlurStyle;
use crate::color_priv::alpha_255_to_256;
use crate::color_priv::alpha_mul;
use crate::mask::{AllocType, Mask, MaskBuilder, MaskFormat};
use crate::mask_blur_filter::{Alphas, MaskBlurFilter};
use crate::math::mul_div_255_round;
use crate::math_priv::clamp_pos;
use crate::point::IPoint;
use crate::rect::{IRect, Rect};
use crate::safe32::abs32;
use crate::scalar::{
    scalar, scalar_ceil_to_int, scalar_exp, scalar_floor_to_int, scalar_round_to_int,
};
use crate::t_pin::t_pin;

/// `SkIVector` is an `IPoint`.
type IVector = IPoint;

// This constant approximates the scaling done in the software path's
// "high quality" mode, in SkBlurMask::Blur() (1 / sqrt(3)).
// IMHO, it actually should be 1:  we blur "less" than we should do
// according to the CSS and canvas specs, simply because Safari does the same.
// Firefox used to do the same too, until 4.0 where they fixed it.  So at some
// point we should probably get rid of these scaling constants and rebaseline
// all the blur tests.
// Port of: src/core/SkBlurMask.cpp#L33 (chrome/m156)
const BLUR_SIGMA_SCALE: scalar = 0.57735;

// The alpha of the pixel `x` of the row `y` of `src`.
fn src_alpha(src: &Mask<'_>, x: usize, y: usize) -> u32 {
    let rb = src.row_bytes as usize;
    u32::from(
        Alphas {
            data: &src.image[y * rb..],
            format: src.format,
        }
        .at(x),
    )
}

// Port of: src/core/SkBlurMask.cpp#L45-L64 (chrome/m156)
// `dst` starts at the first pixel to write; `blur` at the first pixel of the blur.
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn merge_src_with_blur(
    dst: &mut [u8],
    dst_rb: usize,
    src: &Mask<'_>,
    blur: &[u8],
    blur_rb: usize,
    sw: i32,
    sh: i32,
) {
    let sw = usize::try_from(sw).unwrap_or(0);
    let sh = usize::try_from(sh).unwrap_or(0);
    for y in 0..sh {
        for x in 0..sw {
            let s = i32::try_from(src_alpha(src, x, y)).expect("a byte");
            let b = i32::from(blur[y * blur_rb + x]);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // SkToU8
            {
                dst[y * dst_rb + x] = alpha_mul(
                    b,
                    i32::try_from(alpha_255_to_256(s as u32)).expect("<= 256"),
                ) as u8;
            }
        }
    }
}

// Port of: src/core/SkBlurMask.cpp#L66-L82 (chrome/m156)
fn clamp_solid_with_orig(dst: &mut [u8], dst_rb: usize, src: &Mask<'_>, sw: i32, sh: i32) {
    let sw = usize::try_from(sw).unwrap_or(0);
    let sh = usize::try_from(sh).unwrap_or(0);
    for y in 0..sh {
        for x in 0..sw {
            let s = src_alpha(src, x, y);
            let d = u32::from(dst[y * dst_rb + x]);
            #[allow(clippy::cast_possible_truncation)] // SkToU8
            {
                dst[y * dst_rb + x] = (s + d - mul_div_255_round(s, d)) as u8;
            }
        }
    }
}

// Port of: src/core/SkBlurMask.cpp#L84-L102 (chrome/m156)
fn clamp_outer_with_orig(dst: &mut [u8], dst_rb: usize, src: &Mask<'_>, sw: i32, sh: i32) {
    let sw = usize::try_from(sw).unwrap_or(0);
    let sh = usize::try_from(sh).unwrap_or(0);
    for y in 0..sh {
        for x in 0..sw {
            let src_value = src_alpha(src, x, y);
            if src_value != 0 {
                let d = i32::from(dst[y * dst_rb + x]);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // SkToU8
                {
                    dst[y * dst_rb + x] = alpha_mul(
                        d,
                        i32::try_from(alpha_255_to_256(255 - src_value)).expect("<= 256"),
                    ) as u8;
                }
            }
        }
    }
}

// Convolving a box with itself three times results in a piecewise quadratic function; the
// profile curve of the blurred step function at the rectangle edge is its (piecewise cubic)
// indefinite integral (see Skia's comment).
// Port of: src/core/SkBlurMask.cpp#L283-L301 (chrome/m156)
fn gaussian_integral(x: f32) -> f32 {
    if x > 1.5 {
        return 0.0;
    }
    if x < -1.5 {
        return 1.0;
    }

    let x2 = x * x;
    let x3 = x2 * x;

    if x > 0.5 {
        return 0.5625 - (x3 / 6.0 - 3.0 * x2 * 0.25 + 1.125 * x);
    }
    if x > -0.5 {
        return 0.5 - (0.75 * x - x3 / 3.0);
    }
    0.4375 + (-x3 / 6.0 - 3.0 * x2 * 0.25 - 1.125 * x)
}

/// The static functions of `SkBlurMask`.
// Port of: src/core/SkBlurMask.h#L22-L93 (chrome/m156)
#[doc(alias = "SkBlurMask")]
#[derive(Copy, Clone, Debug)]
pub struct BlurMask;

impl BlurMask {
    /// If `radius > 0`, returns the corresponding sigma, else 0.
    // Port of: src/core/SkBlurMask.cpp#L35-L37 (chrome/m156)
    #[doc(alias = "ConvertRadiusToSigma")]
    #[must_use]
    pub fn convert_radius_to_sigma(radius: scalar) -> scalar {
        if radius > 0.0 {
            BLUR_SIGMA_SCALE * radius + 0.5
        } else {
            0.0
        }
    }

    /// If `sigma > 0.5`, returns the corresponding radius, else 0.
    // Port of: src/core/SkBlurMask.cpp#L39-L41 (chrome/m156)
    #[doc(alias = "ConvertSigmaToRadius")]
    #[must_use]
    pub fn convert_sigma_to_radius(sigma: scalar) -> scalar {
        if sigma > 0.5 {
            (sigma - 0.5) / BLUR_SIGMA_SCALE
        } else {
            0.0
        }
    }

    /// Blurs `src` into `dst` with a box-blur approximation of a gaussian of `sigma`. If
    /// `src` has no image, only the border (`margin`) is calculated. On failure with an image
    /// `dst.image` is empty and this returns false.
    ///
    /// skia-rust: Skia's `BoxBlur` reports a failure to blur with an image by leaving
    /// `dst->fImage` null and returning false.
    // Port of: src/core/SkBlurMask.cpp#L104-L282 (chrome/m156)
    #[doc(alias = "BoxBlur")]
    #[must_use]
    pub fn box_blur(
        dst: &mut MaskBuilder,
        src: &Mask<'_>,
        sigma: scalar,
        style: BlurStyle,
        margin: Option<&mut IVector>,
    ) -> bool {
        if src.format != MaskFormat::BW
            && src.format != MaskFormat::A8
            && src.format != MaskFormat::Argb32
            && src.format != MaskFormat::Lcd16
        {
            return false;
        }

        let blur_filter = MaskBlurFilter::new(f64::from(sigma), f64::from(sigma));
        if blur_filter.has_no_blur() {
            // If there is no effective blur most styles will just produce the original mask.
            // However, kOuter_SkBlurStyle will produce an empty mask.
            if style == BlurStyle::Outer {
                dst.image = Vec::new();
                dst.bounds = IRect::new(0, 0, 0, 0);
                #[allow(clippy::cast_sign_loss)] // the width of an empty rect is 0
                {
                    dst.row_bytes = dst.bounds.width() as u32;
                }
                dst.format = MaskFormat::A8;
                if let Some(margin) = margin {
                    // This filter will disregard the src.fImage completely.
                    // The margin is actually {-(src.fBounds.width() / 2), -(src.fBounds.height()
                    // / 2)} but it is not clear if callers will fall over with negative margins.
                    *margin = IVector::new(0, 0);
                }
                return true;
            }
            return false;
        }
        let border = blur_filter.blur(src, dst);

        if !src.image.is_empty() && dst.image.is_empty() {
            // The call to blur() failed to set our destination image up (e.g. an overflow).
            // Note that if src.fImage was null, dst->fImage will also be null and that's
            // *not* an error case - the code should continue to calculate the border.
            return false;
        }

        if let Some(margin) = margin {
            *margin = border;
        }

        if src.image.is_empty() {
            if style == BlurStyle::Inner {
                dst.bounds = src.bounds; // restore trimmed bounds
                #[allow(clippy::cast_sign_loss)] // the width is non-negative
                {
                    dst.row_bytes = dst.bounds.width() as u32;
                }
            }
            return true;
        }

        let dst_rb = dst.row_bytes as usize;
        #[allow(clippy::cast_sign_loss)] // the border is non-negative
        let dst_start = border.x as usize + border.y as usize * dst_rb;

        match style {
            BlurStyle::Normal => {}
            BlurStyle::Solid => {
                clamp_solid_with_orig(
                    &mut dst.image[dst_start..],
                    dst_rb,
                    src,
                    src.bounds.width(),
                    src.bounds.height(),
                );
            }
            BlurStyle::Outer => {
                clamp_outer_with_orig(
                    &mut dst.image[dst_start..],
                    dst_rb,
                    src,
                    src.bounds.width(),
                    src.bounds.height(),
                );
            }
            BlurStyle::Inner => {
                // now we allocate the "real" dst, mirror the size of src
                #[allow(clippy::cast_sign_loss)] // the width is non-negative
                let new_dst = MaskBuilder::new(
                    Vec::new(),
                    src.bounds,
                    src.bounds.width() as u32,
                    dst.format,
                );
                let blur = std::mem::replace(dst, new_dst);

                let dst_size = dst.compute_image_size();
                if 0 == dst_size {
                    return false; // too big to allocate, abort
                }
                dst.image = MaskBuilder::alloc_image(dst_size, AllocType::Uninit);

                let blur_rb = blur.row_bytes as usize;
                #[allow(clippy::cast_sign_loss)] // the border is non-negative
                let blur_start = border.x as usize + border.y as usize * blur_rb;
                let new_rb = dst.row_bytes as usize;
                merge_src_with_blur(
                    &mut dst.image,
                    new_rb,
                    src,
                    &blur.image[blur_start..],
                    blur_rb,
                    src.bounds.width(),
                    src.bounds.height(),
                );
            }
        }

        true
    }

    /// Fills `profile` (of `size` bytes) with the profile signature of a blurred half-plane
    /// with the given sigma. Since we're going to be doing screened multiplications (i.e.,
    /// `1 - (1-x)(1-y)`) all the time, we actually fill in the profile pre-inverted (already
    /// done `255-x`).
    // Port of: src/core/SkBlurMask.cpp#L303-L318 (chrome/m156)
    #[doc(alias = "ComputeBlurProfile")]
    pub fn compute_blur_profile(profile: &mut [u8], size: i32, sigma: scalar) {
        debug_assert_eq!(scalar_ceil_to_int(6.0 * sigma), size);

        let center = size >> 1;

        let invr = 1.0 / (2.0 * sigma);

        profile[0] = 255;
        for x in 1..size {
            #[allow(clippy::cast_precision_loss)] // mirrors the int -> float conversion
            let scaled_x = ((center - x) as f32 - 0.5) * invr;
            let gi = gaussian_integral(scaled_x);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // (uint8_t) cast
            {
                profile[x as usize] = 255 - (255.0 * gi) as u8;
            }
        }
    }

    /// Looks up the intensity of the (one dimensional) blurred half-plane.
    ///
    /// Implementation adapted from Michael Herf's approach: <http://stereopsis.com/shadowrect/>
    // Port of: src/core/SkBlurMask.cpp#L320-L333 (chrome/m156)
    #[doc(alias = "ProfileLookup")]
    #[must_use]
    pub fn profile_lookup(profile: &[u8], loc: i32, blurred_width: i32, sharp_width: i32) -> u8 {
        // how far are we from the original edge?
        let dx = abs32(((loc << 1) + 1) - blurred_width) - sharp_width;
        let mut ox = dx >> 1;
        if ox < 0 {
            ox = 0;
        }

        #[allow(clippy::cast_sign_loss)] // ox is non-negative
        {
            profile[ox as usize]
        }
    }

    /// Computes an entire scanline of a blurred step function.
    // Port of: src/core/SkBlurMask.cpp#L335-L357 (chrome/m156)
    #[doc(alias = "ComputeBlurredScanline")]
    #[allow(
        clippy::cast_possible_truncation, // mirrors C++ unsigned/int/(uint8_t) conversions
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn compute_blurred_scanline(pixels: &mut [u8], profile: &[u8], width: u32, sigma: scalar) {
        let profile_size = scalar_ceil_to_int(6.0 * sigma) as u32;

        let sw: u32 = width.wrapping_sub(profile_size);
        // nearest odd number less than the profile size represents the center
        // of the (2x scaled) profile
        let center: i32 = ((profile_size & !1).wrapping_sub(1)) as i32;

        let w: i32 = sw.wrapping_sub(center as u32) as i32;

        for x in 0..width {
            if profile_size <= sw {
                pixels[x as usize] = Self::profile_lookup(profile, x as i32, width as i32, w);
            } else {
                let span = (sw as f32) / (2.0 * sigma);
                let gi_x = 1.5 - (x as f32 + 0.5) / (2.0 * sigma);
                pixels[x as usize] =
                    (255.0 * (gaussian_integral(gi_x) - gaussian_integral(gi_x + span))) as u8;
            }
        }
    }

    /// Analytic blur of the rectangle `src` (see Skia's comment on `BlurRect`).
    // Port of: src/core/SkBlurMask.cpp#L359-L463 (chrome/m156)
    #[doc(alias = "BlurRect")]
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation, // mirrors C++ conversions
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::too_many_lines
    )]
    pub fn blur_rect(
        sigma: scalar,
        dst: &mut MaskBuilder,
        src: &Rect,
        style: BlurStyle,
        margin: Option<&mut IVector>,
        create_mode: crate::mask::CreateMode,
    ) -> bool {
        let profile_size = scalar_ceil_to_int(6.0 * sigma);
        if profile_size <= 0 {
            return false; // no blur to compute
        }

        let pad = profile_size / 2;
        if let Some(margin) = margin {
            margin.set(pad, pad);
        }

        let pad_f = pad as scalar;
        dst.bounds.set_ltrb(
            scalar_round_to_int(src.left - pad_f),
            scalar_round_to_int(src.top - pad_f),
            scalar_round_to_int(src.right + pad_f),
            scalar_round_to_int(src.bottom + pad_f),
        );

        dst.row_bytes = dst.bounds.width() as u32;
        dst.format = MaskFormat::A8;
        dst.image = Vec::new();

        let sw = scalar_floor_to_int(src.width());
        let sh = scalar_floor_to_int(src.height());

        if create_mode == crate::mask::CreateMode::JustComputeBounds {
            if style == BlurStyle::Inner {
                dst.bounds = src.round(); // restore trimmed bounds
                dst.row_bytes = sw as u32;
            }
            return true;
        }

        let mut profile = vec![0u8; profile_size as usize];

        Self::compute_blur_profile(&mut profile, profile_size, sigma);

        let dst_size = dst.compute_image_size();
        if 0 == dst_size {
            return false; // too big to allocate, abort
        }

        let mut dp = MaskBuilder::alloc_image(dst_size, AllocType::Uninit);

        let dst_height = dst.bounds.height();
        let dst_width = dst.bounds.width();

        let mut horizontal_scanline = vec![0u8; dst_width as usize];
        let mut vertical_scanline = vec![0u8; dst_height as usize];

        Self::compute_blurred_scanline(&mut horizontal_scanline, &profile, dst_width as u32, sigma);
        Self::compute_blurred_scanline(&mut vertical_scanline, &profile, dst_height as u32, sigma);

        let mut outptr = 0;
        for &vertical in &vertical_scanline {
            for &horizontal in &horizontal_scanline {
                let maskval = mul_div_255_round(u32::from(horizontal), u32::from(vertical));
                dp[outptr] = maskval as u8;
                outptr += 1;
            }
        }

        // memset(dst_scanline, value, sw), where the C++ would run into the next row if it were
        // longer than the row (the image is contiguous).
        let fill_row = |dp: &mut Vec<u8>, y: i32, value: u8| {
            let start = (y * dst_width + pad) as usize;
            let end = std::cmp::min(start + sw as usize, dp.len());
            dp[start..end].fill(value);
        };

        if style == BlurStyle::Inner {
            // now we allocate the "real" dst, mirror the size of src
            let src_size = (src.width() * src.height()) as usize;
            if 0 == src_size {
                return false; // too big to allocate, abort
            }
            let mut image = MaskBuilder::alloc_image(src_size, AllocType::Uninit);
            for y in 0..sh {
                let blur_scanline = ((y + pad) * dst_width + pad) as usize;
                let inner_scanline = (y * sw) as usize;
                image[inner_scanline..inner_scanline + sw as usize]
                    .copy_from_slice(&dp[blur_scanline..blur_scanline + sw as usize]);
            }
            dst.image = image;

            dst.bounds = src.round(); // restore trimmed bounds
            dst.row_bytes = sw as u32;
        } else {
            if style == BlurStyle::Outer {
                for y in pad..dst_height - pad {
                    fill_row(&mut dp, y, 0);
                }
            } else if style == BlurStyle::Solid {
                for y in pad..dst_height - pad {
                    fill_row(&mut dp, y, 0xff);
                }
            }
            // normal and solid styles are the same for analytic rect blurs, so don't
            // need to handle solid specially.
            dst.image = dp;
        }

        true
    }

    /// The "ground truth" blur does a gaussian convolution; it's slow but useful for comparison
    /// purposes. The "simple" blur is a direct implementation of separable convolution with a
    /// discrete gaussian kernel.
    // Port of: src/core/SkBlurMask.cpp#L465-L664 (chrome/m156)
    #[doc(alias = "BlurGroundTruth")]
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation, // mirrors C++ conversions
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::too_many_lines
    )]
    pub fn blur_ground_truth(
        sigma: scalar,
        dst: &mut MaskBuilder,
        src: &Mask<'_>,
        style: BlurStyle,
        margin: Option<&mut IVector>,
    ) -> bool {
        if src.format != MaskFormat::A8 {
            return false;
        }

        let variance = sigma * sigma;

        let mut window_size = scalar_ceil_to_int(sigma * 6.0);
        // round window size up to nearest odd number
        window_size |= 1;

        let mut gauss_window = vec![0.0f32; window_size as usize];

        let half_window = window_size >> 1;

        gauss_window[half_window as usize] = 1.0;

        let mut window_sum = 1.0f32;
        for x in 1..=half_window {
            let gaussian = scalar_exp(((-x * x) as f32) / (2.0 * variance));
            gauss_window[(half_window + x) as usize] = gaussian;
            gauss_window[(half_window - x) as usize] = gaussian;
            window_sum += 2.0 * gaussian;
        }

        // leave the filter un-normalized for now; we will divide by the normalization
        // sum later;

        let pad = half_window;
        if let Some(margin) = margin {
            margin.set(pad, pad);
        }

        dst.bounds = src.bounds;
        dst.bounds.outset((pad, pad));

        dst.row_bytes = dst.bounds.width() as u32;
        dst.format = MaskFormat::A8;
        dst.image = Vec::new();

        if !src.image.is_empty() {
            let dst_size = dst.compute_image_size();
            if 0 == dst_size {
                return false; // too big to allocate, abort
            }

            let src_width = src.bounds.width();
            let src_height = src.bounds.height();
            let dst_width = dst.bounds.width();

            let src_pixels = src.image;
            let mut dst_pixels = MaskBuilder::alloc_image(dst_size, AllocType::Uninit);

            // do the actual blur.  First, make a padded copy of the source.
            // use double pad so we never have to check if we're outside anything

            let pad_width = src_width + 4 * pad;
            let pad_height = src_height;
            let pad_size = pad_width * pad_height;

            let mut pad_pixels = vec![0u8; pad_size as usize];

            for y in 0..src_height {
                let padptr = (y * pad_width + 2 * pad) as usize;
                let srcptr = (y * src_width) as usize;
                pad_pixels[padptr..padptr + src_width as usize]
                    .copy_from_slice(&src_pixels[srcptr..srcptr + src_width as usize]);
            }

            // blur in X, transposing the result into a temporary floating point buffer.
            // also double-pad the intermediate result so that the second blur doesn't
            // have to do extra conditionals.

            let tmp_width = pad_height + 4 * pad;
            let tmp_height = pad_width - 2 * pad;
            let tmp_size = tmp_width * tmp_height;

            let mut tmp_image = vec![0.0f32; tmp_size as usize];

            for y in 0..pad_height {
                let src_scanline = (y * pad_width) as usize;
                for x in pad..pad_width - pad {
                    let out_pixel = ((x - pad) * tmp_width + y + 2 * pad) as usize; // transposed output
                    let window_center = src_scanline + x as usize;
                    for i in -pad..=pad {
                        tmp_image[out_pixel] += gauss_window[(pad + i) as usize]
                            * f32::from(pad_pixels[(window_center as i32 + i) as usize]);
                    }
                    tmp_image[out_pixel] /= window_sum;
                }
            }

            // blur in Y; now filling in the actual desired destination.  We have to do
            // the transpose again; these transposes guarantee that we read memory in
            // linear order.

            for y in 0..tmp_height {
                let src_scanline = (y * tmp_width) as usize;
                for x in pad..tmp_width - pad {
                    let window_center = src_scanline + x as usize;
                    let mut final_value = 0.0f32;
                    for i in -pad..=pad {
                        final_value += gauss_window[(pad + i) as usize]
                            * tmp_image[(window_center as i32 + i) as usize];
                    }
                    final_value /= window_sum;
                    let out_pixel = ((x - pad) * dst_width + y) as usize; // transposed output
                    let integer_pixel = (final_value + 0.5) as i32;
                    dst_pixels[out_pixel] = t_pin(clamp_pos(integer_pixel), 0, 255) as u8;
                }
            }

            let dst_rb = dst.row_bytes as usize;
            let blur_start = pad as usize * dst_rb + pad as usize;
            match style {
                BlurStyle::Normal => {
                    dst.image = dst_pixels;
                }
                BlurStyle::Solid => {
                    clamp_solid_with_orig(
                        &mut dst_pixels[blur_start..],
                        dst_rb,
                        src,
                        src_width,
                        src_height,
                    );
                    dst.image = dst_pixels;
                }
                BlurStyle::Outer => {
                    clamp_outer_with_orig(
                        &mut dst_pixels[blur_start..],
                        dst_rb,
                        src,
                        src_width,
                        src_height,
                    );
                    dst.image = dst_pixels;
                }
                BlurStyle::Inner => {
                    // now we allocate the "real" dst, mirror the size of src
                    let src_size = src.compute_image_size();
                    if 0 == src_size {
                        return false; // too big to allocate, abort
                    }
                    let mut image = MaskBuilder::alloc_image(src_size, AllocType::Uninit);
                    merge_src_with_blur(
                        &mut image,
                        src.row_bytes as usize,
                        src,
                        &dst_pixels[blur_start..],
                        dst_rb,
                        src_width,
                        src_height,
                    );
                    dst.image = image;
                }
            }
        }

        if style == BlurStyle::Inner {
            dst.bounds = src.bounds; // restore trimmed bounds
            dst.row_bytes = src.row_bytes;
        }

        true
    }
}

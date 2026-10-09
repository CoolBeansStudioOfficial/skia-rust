// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/GradientBitmap.h, src/gpu/GradientBitmap.cpp

//! `skgpu::EncodeGradientStopToHalf` and `skgpu::CreateGradientColorAndOffsetBitmap`: the F16
//! texture that many-stop gradients upload, with each stop offset stored as a
//! (mantissa, exponent) pair that survives half precision.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::half::{float_to_half, half_to_float};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

/// `std::frexp` for `f32`: splits `x` into a mantissa in `[0.5, 1)` (with the sign of `x`) and
/// an exponent, so that `x == mantissa * 2^exponent`. Zero, infinities and NaN return `(x, 0)`.
fn frexp(x: f32) -> (f32, i32) {
    if x == 0.0 || !x.is_finite() {
        return (x, 0);
    }
    // Subnormals are scaled into the normal range first (an exact multiplication by 2^23).
    let (x, subnormal_shift) = if x.abs() < f32::MIN_POSITIVE {
        (x * 8_388_608.0, -23)
    } else {
        (x, 0)
    };
    let bits = x.to_bits();
    let biased_exp = ((bits >> 23) & 0xff).cast_signed();
    // Replace the exponent field with the one for [0.5, 1).
    let mantissa_bits = (bits & !(0xff << 23)) | (126 << 23);
    (
        f32::from_bits(mantissa_bits),
        biased_exp - 126 + subnormal_shift,
    )
}

/// Encodes a stop offset as a (mantissa, exponent) pair, both of which are exact in F16.
///
/// Returns `None` if the exponent does not round-trip through F16, in which case the caller must
/// not use the texture encoding.
// Port of: src/gpu/GradientBitmap.cpp#L15-L30 (chrome/m156)
#[doc(alias = "EncodeGradientStopToHalf")]
#[must_use]
pub fn encode_gradient_stop_to_half(offset: f32) -> Option<(f32, f32)> {
    let (mant, exp) = frexp(offset);
    // The exponent is a small integer; it is converted to float exactly as in C++.
    #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(exp)
    let exp_f = exp as f32;
    let half_e = float_to_half(exp_f);
    // mirrors static_cast<int>(SkHalfToFloat(halfE)) != exp
    #[allow(clippy::cast_possible_truncation)]
    if half_to_float(half_e) as i32 != exp {
        return None;
    }
    #[cfg(debug_assertions)]
    {
        let half_m = half_to_float(float_to_half(mant));
        let restored = f64::from(half_m) * 2f64.powi(exp);
        debug_assert!((restored - f64::from(offset)).abs() < 0.001);
    }
    Some((mant, exp_f))
}

/// Builds the F16 texture for a many-stop gradient: `num_stops` wide and 2 tall. Row 0 holds the
/// premultiplied colors (stored unpremultiplied, as in C++); row 1 holds each stop offset as
/// `(mantissa, exponent, 0, 1)`. If `offsets` is `None`, the stops are evenly spaced.
///
/// Returns an empty bitmap if allocation or offset encoding fails.
// Port of: src/gpu/GradientBitmap.cpp#L32-L70 (chrome/m156)
#[doc(alias = "CreateGradientColorAndOffsetBitmap")]
#[must_use]
// The integer indices and stop counts mirror the C++ `int` loop and its `SkIntToFloat(i)`.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::manual_range_contains
)]
pub fn create_gradient_color_and_offset_bitmap(
    num_stops: i32,
    colors: &[PMColor4f],
    offsets: Option<&[f32]>,
) -> Bitmap {
    let info = ImageInfo::new(
        ISize::new(num_stops, 2),
        ColorType::RGBAF16,
        AlphaType::Premul,
        None,
    );
    let mut colors_and_offsets = Bitmap::new();
    if !colors_and_offsets.try_alloc_pixels_info(&info, None) {
        return Bitmap::new();
    }

    for i in 0..num_stops {
        let idx = i as usize;
        // TODO in C++: there should be a way to directly set a premul pixel in a bitmap with a
        // premul color.
        let unpremul_color: Color4f = colors[idx].unpremul();
        colors_and_offsets.erase_4f(unpremul_color, IRect::from_xywh(i, 0, 1, 1));

        let offset = match offsets {
            Some(o) => o[idx],
            // SkIntToFloat(i) / (numStops - 1)
            None => (i as f32) / ((num_stops - 1) as f32),
        };
        debug_assert!((0.0..=1.0).contains(&offset));
        let Some((mantissa, exponent)) = encode_gradient_stop_to_half(offset) else {
            return Bitmap::new();
        };
        // TODO in C++: we're only using 2 of the f16s here. This encoding yields < 0.001f error
        // for 2^20 evenly spaced stops.
        colors_and_offsets.erase_4f(
            Color4f::new(mantissa, exponent, 0.0, 1.0),
            IRect::from_xywh(i, 1, 1, 1),
        );
    }
    colors_and_offsets
}

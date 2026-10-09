// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/BlurUtils.h, src/gpu/BlurUtils.cpp
//
// Ported: the blur-sigma helpers that wrap `SkBlurEngine` (reusing core's), and the CPU profiles
// `ComputeIntegralTableWidth`, `CreateCircleProfile` and `CreateHalfPlaneProfile`.
//
// Not ported yet:
// - the kernel wrappers (`BlurKernelWidth`, `Compute1DBlurKernel`, `Compute2DBlurOffsets`, ...)
//   and `GetBlur2DEffect`/`GetLinearBlur1DEffect`: they forward to `SkShaderBlurAlgorithm` and
//   `SkRuntimeEffect`, neither of which is ported;
// - `CreateIntegralTable` and `CreateRRectBlurMask`: they call `std::erf`, which the standard
//   library does not provide and which must not be approximated (no tolerances).

// The integer casts mirror the C++ conversions in the original (`(int)std::ceil`, `int` to
// `float` in the profile arithmetic).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_engine::{is_effectively_identity, sigma_to_radius};
use skia_rust_core::color_priv::unit_scalar_clamp_to_byte;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::math_priv::next_pow2;
use skia_rust_core::scalar::{scalar_ceil_to_int, scalar_floor_to_int};
use skia_rust_core::size::ISize;

// Port of: src/gpu/BlurUtils.h#L24-L27 (chrome/m156)
/// `BlurIsEffectivelyIdentity`: any sigma at or below this is effectively no blur.
#[doc(alias = "BlurIsEffectivelyIdentity")]
#[must_use]
pub fn blur_is_effectively_identity(sigma: f32) -> bool {
    is_effectively_identity(sigma)
}

// Port of: src/gpu/BlurUtils.h#L28 (chrome/m156)
/// `BlurSigmaRadius`: the pixel radius that covers a blur of `sigma`.
#[doc(alias = "BlurSigmaRadius")]
#[must_use]
pub fn blur_sigma_radius(sigma: f32) -> i32 {
    sigma_to_radius(sigma)
}

// Port of: src/gpu/BlurUtils.cpp#L86-L108 (chrome/m156)
/// Returns the width of the integral table for a blur over `six_sigma`: two texels per pixel,
/// rounded to a power of two with a minimum of 32.
#[doc(alias = "ComputeIntegralTableWidth")]
#[must_use]
pub fn compute_integral_table_width(six_sigma: f32) -> i32 {
    // Check for NaN/infinity
    if !six_sigma.is_finite() {
        return 0;
    }

    // Avoid overflow, covers both multiplying by 2 and finding next power of 2:
    // 2*((2^31-1)/4 + 1) = 2*(2^29-1) + 2 = 2^30 and SkNextPow2(2^30) = 2^30
    if six_sigma > (i32::MAX / 4 + 1) as f32 {
        return 0;
    }

    // The texture we're producing represents the integral of a normal distribution over a
    // six-sigma range centered at zero. We want enough resolution so that the linear
    // interpolation done in texture lookup doesn't introduce noticeable artifacts. We
    // conservatively choose to have 2 texels for each dst pixel.
    let min_width = 2 * (six_sigma.ceil() as i32);

    // Bin by powers of 2 with a minimum so we get good profile reuse.
    next_pow2(min_width).max(32)
}

// `kAllocLimit` in `CreateCircleProfile`: `INT32_MAX >> 2`.
const K_ALLOC_LIMIT: i32 = i32::MAX >> 2;

// Computes an unnormalized half kernel (right side). Returns the summation of all the half
// kernel values.
// Port of: src/gpu/BlurUtils.cpp#L127-L139 (chrome/m156)
fn make_unnormalized_half_kernel(half_kernel: &mut [f32], sigma: f32) -> f32 {
    let inv_sigma = 1.0f32 / sigma;
    let b = -0.5f32 * inv_sigma * inv_sigma;
    let mut tot = 0.0f32;
    // Compute half kernel values at half pixel steps out from the center.
    let mut t = 0.5f32;
    for slot in half_kernel.iter_mut() {
        let value = (t * t * b).exp();
        tot += value;
        *slot = value;
        t += 1.0f32;
    }
    tot
}

// Create a Gaussian half-kernel (right side) and a summed area table given a sigma and number
// of discrete steps. The half kernel is normalized to sum to 0.5.
// Port of: src/gpu/BlurUtils.cpp#L142-L155 (chrome/m156)
fn make_half_kernel_and_summed_table(
    half_kernel: &mut [f32],
    summed_half_kernel: &mut [f32],
    sigma: f32,
) {
    // The half kernel should sum to 0.5 not 1.0.
    let tot = 2.0f32 * make_unnormalized_half_kernel(half_kernel, sigma);
    let mut sum = 0.0f32;
    for (i, slot) in half_kernel.iter_mut().enumerate() {
        *slot /= tot;
        sum += *slot;
        summed_half_kernel[i] = sum;
    }
}

// Applies the 1D half kernel vertically at points along the x axis to a circle centered at the
// origin with radius circleR.
// Port of: src/gpu/BlurUtils.cpp#L158-L183 (chrome/m156)
fn apply_kernel_in_y(
    results: &mut [f32],
    num_steps: usize,
    first_x: f32,
    circle_r: f32,
    half_kernel_size: i32,
    summed_half_kernel_table: &[f32],
) {
    let mut x = first_x;
    for result in results.iter_mut().take(num_steps) {
        if x < -circle_r || x > circle_r {
            *result = 0.0;
            x += 1.0f32;
            continue;
        }

        let mut y = (circle_r * circle_r - x * x).sqrt();
        // In the column at x we exit the circle at +y and -y
        // The summed table entry j is actually reflects an offset of j + 0.5.
        y -= 0.5f32;
        let y_int = scalar_floor_to_int(y);
        debug_assert!(y_int >= -1);
        if y < 0.0 {
            *result = (y + 0.5f32) * summed_half_kernel_table[0];
        } else if y_int >= half_kernel_size - 1 {
            *result = 0.5f32;
        } else {
            let y_int = y_int as usize;
            let y_frac = y - y_int as f32;
            *result = (1.0f32 - y_frac) * summed_half_kernel_table[y_int]
                + y_frac * summed_half_kernel_table[y_int + 1];
        }
        x += 1.0f32;
    }
}

// Apply a Gaussian at point (evalX, 0) to a circle centered at the origin with radius circleR.
// This relies on having a half kernel computed for the Gaussian and a table of applications of
// the half kernel in y to columns at (evalX - halfKernel, evalX - halfKernel + 1, ..., evalX +
// halfKernel) passed in as yKernelEvaluations.
// Port of: src/gpu/BlurUtils.cpp#L186-L209 (chrome/m156)
fn eval_at(
    eval_x: f32,
    circle_r: f32,
    half_kernel: &[f32],
    half_kernel_size: usize,
    y_kernel_evaluations: &[f32],
) -> u8 {
    let mut acc = 0.0f32;
    let mut x = eval_x - half_kernel_size as f32;
    for i in 0..half_kernel_size {
        if !(x < -circle_r || x > circle_r) {
            let vertical_eval = y_kernel_evaluations[i];
            acc += vertical_eval * half_kernel[half_kernel_size - i - 1];
        }
        x += 1.0f32;
    }
    for i in 0..half_kernel_size {
        if !(x < -circle_r || x > circle_r) {
            let vertical_eval = y_kernel_evaluations[i + half_kernel_size];
            acc += vertical_eval * half_kernel[i];
        }
        x += 1.0f32;
    }
    // Since we applied a half kernel in y we multiply acc by 2 (the circle is symmetric about
    // the x axis).
    clamp_to_byte(2.0f32 * acc)
}

/// Creates a profile of a blurred circle of `radius`, `profile_width` samples wide, blurred by
/// `sigma`. Returns an empty bitmap if the allocation would overflow or fail.
///
/// The profile is computed by building a kernel for half the Gaussian and a matching summed
/// area table, then applying the half kernel vertically to the circle at each x step.
// Port of: src/gpu/BlurUtils.cpp#L211-L263 (chrome/m156)
#[doc(alias = "CreateCircleProfile")]
#[must_use]
pub fn create_circle_profile(sigma: f32, radius: f32, profile_width: i32) -> Bitmap {
    let num_steps = profile_width;

    // The full kernel is 6 sigmas wide. SkScalarCeilToInt saturates to a number large enough to
    // still detect overflow in the kernel size calculations.
    let mut half_kernel_size = scalar_ceil_to_int(6.0f32 * sigma);

    // Round up to next multiple of 2 and then divide by 2.
    half_kernel_size = ((half_kernel_size + 1) & !1) >> 1;

    // The full internal allocations will be numSteps + 4*halfKernelSize, so if that would
    // overflow then return an empty bitmap.
    if num_steps > K_ALLOC_LIMIT || half_kernel_size > (K_ALLOC_LIMIT - num_steps) {
        return Bitmap::new();
    }
    let half_kernel_size_i32 = half_kernel_size;
    let half_kernel_size = half_kernel_size as usize;
    let num_steps = num_steps as usize;

    // Number of x steps at which to apply kernel in y to cover all the profile samples in x.
    let num_y_steps = num_steps + 2 * half_kernel_size;

    let mut half_kernel = vec![0.0f32; half_kernel_size];
    let mut summed_kernel = vec![0.0f32; half_kernel_size];
    let mut y_evals = vec![0.0f32; num_y_steps];

    make_half_kernel_and_summed_table(&mut half_kernel, &mut summed_kernel, sigma);

    let first_x = -half_kernel_size_i32 as f32 + 0.5f32;
    apply_kernel_in_y(
        &mut y_evals,
        num_y_steps,
        first_x,
        radius,
        half_kernel_size_i32,
        &summed_kernel,
    );

    let mut profile = vec![0u8; num_steps];
    for (i, out) in profile.iter_mut().enumerate().take(num_steps - 1) {
        let eval_x = i as f32 + 0.5f32;
        *out = eval_at(
            eval_x,
            radius,
            &half_kernel,
            half_kernel_size,
            &y_evals[i..],
        );
    }
    // Ensure the tail of the Gaussian goes to zero.
    profile[num_steps - 1] = 0;

    install_a8_profile(profile, profile_width)
}

/// Creates a half-plane approximation profile of a blurred circle, `profile_width` samples wide
/// (which must be even). Returns an empty bitmap if the allocation fails.
// Port of: src/gpu/BlurUtils.cpp#L265-L300 (chrome/m156)
#[doc(alias = "CreateHalfPlaneProfile")]
#[must_use]
pub fn create_half_plane_profile(profile_width: i32) -> Bitmap {
    debug_assert_eq!(profile_width & 0x1, 0);
    let width = profile_width as usize;
    let mut profile = vec![0u8; width];

    // The full kernel is 6 sigmas wide.
    let sigma = profile_width as f32 / 6.0f32;
    let half_kernel_size = (profile_width / 2) as usize;

    let mut half_kernel = vec![0.0f32; half_kernel_size];

    // The half kernel should sum to 0.5.
    let tot = 2.0f32 * make_unnormalized_half_kernel(&mut half_kernel, sigma);

    let mut sum = 0.0f32;

    // Populate the profile from the right edge to the middle.
    for i in 0..half_kernel_size {
        let k = half_kernel_size - i - 1;
        half_kernel[k] /= tot;
        sum += half_kernel[k];
        profile[width - i - 1] = clamp_to_byte(sum);
    }

    // Populate the profile from the middle to the left edge (by flipping the half kernel and
    // continuing the summation).
    for i in 0..half_kernel_size {
        sum += half_kernel[i];
        profile[half_kernel_size - i - 1] = clamp_to_byte(sum);
    }

    // Ensure the tail of the Gaussian goes to zero.
    profile[width - 1] = 0;

    install_a8_profile(profile, profile_width)
}

// SkUnitScalarClampToByte, as the `u8` the profiles store (the core function returns U8CPU).
fn clamp_to_byte(x: f32) -> u8 {
    // The value is in [0, 255] by construction.
    unit_scalar_clamp_to_byte(x) as u8
}

// Wraps `profile` as an immutable `A8` bitmap of `profile_width` x 1 (`tryAllocPixels` +
// `setImmutable`). Returns an empty bitmap if the pixels cannot be installed.
fn install_a8_profile(profile: Vec<u8>, profile_width: i32) -> Bitmap {
    let info = ImageInfo::new(
        ISize::new(profile_width, 1),
        ColorType::Alpha8,
        AlphaType::Premul,
        None,
    );
    let mut bitmap = Bitmap::new();
    let row_bytes = profile_width as usize;
    if !bitmap.install_pixels(&info, profile, row_bytes) {
        return Bitmap::new();
    }
    bitmap.set_immutable();
    bitmap
}

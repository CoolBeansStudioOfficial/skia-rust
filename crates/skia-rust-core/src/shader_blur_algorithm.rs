// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlurEngine.h, src/core/SkBlurEngine.cpp (`SkShaderBlurAlgorithm`)

//! `SkShaderBlurAlgorithm`: a blur drawn with the blur runtime effects
//! (`SkKnownRuntimeEffects` `k1DBlur*`/`k2DBlur*`) into devices of the backend. Graphite's image
//! filter backend uses it for every color type (`docs/design/gpu.md` §5.5).
//!
//! skia-rust: the raster blur engine keeps its CPU passes only (`RasterShaderBlurAlgorithm`, which
//! Skia uses for the color types the CPU passes do not handle, is not wired up), so the raster
//! path does not change.

use crate::alpha_type::AlphaType;
use crate::blend_mode::BlendMode;
use crate::blur_engine::sigma_to_radius;
use crate::clip_op::ClipOp;
use crate::device::Device;
use crate::image_info::ImageInfo;
use crate::known_runtime_effects::{StableKey, get_known_runtime_effect, stable_key_from_u32};
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::rect::{Contains, IRect, Rect};
use crate::runtime_effect::{RuntimeEffect, RuntimeEffectBuilder};
use crate::sampling_options::{FilterMode, SamplingOptions};
use crate::size::{ISize, Size};
use crate::special_image::SpecialImage;
use crate::tile_mode::TileMode;

/// `SkShaderBlurAlgorithm::kMaxSamples`: the maximum number of texture samples (and kernel
/// weights) a blur effect makes in one pass.
// Port of: src/core/SkBlurEngine.h#L182-L188 (chrome/m156)
#[doc(alias = "kMaxSamples")]
pub const MAX_SAMPLES: i32 = 28;

/// `MAX_SAMPLES` as an array length.
const N: usize = 28;

/// `SkShaderBlurAlgorithm::kMaxLinearSigma`.
// Port of: src/core/SkBlurEngine.h#L190-L194 (chrome/m156)
#[doc(alias = "kMaxLinearSigma")]
pub const MAX_LINEAR_SIGMA: f32 = 4.0; // -> radius = 27 -> linear kernel width = 28

/// `KernelWidth(radius)`: the kernel width of a Gaussian blur of the given pixel radius, when all
/// pixels are sampled.
// Port of: src/core/SkBlurEngine.h#L176 (chrome/m156)
#[doc(alias = "KernelWidth")]
#[must_use]
pub const fn kernel_width(radius: i32) -> i32 {
    2 * radius + 1
}

/// `LinearKernelWidth(radius)`: the kernel width of a Gaussian blur of the given pixel radius,
/// that relies on HW bilinear filtering to combine adjacent pixels.
// Port of: src/core/SkBlurEngine.h#L180 (chrome/m156)
#[doc(alias = "LinearKernelWidth")]
#[must_use]
pub const fn linear_kernel_width(radius: i32) -> i32 {
    radius + 1
}

/// Converts a count that the callers keep non-negative to an index.
fn to_index(v: i32) -> usize {
    usize::try_from(v).expect("a non-negative count")
}

/// `Compute2DBlurKernel(sigma, radius, kernel)`: the normalized 2D Gaussian kernel of `radius`
/// into the first `KernelWidth(radius.width) * KernelWidth(radius.height)` values of `kernel`,
/// zeroing the rest.
// Port of: src/core/SkBlurEngine.cpp#L1324-L1371 (chrome/m156)
// skia-rust: libm -- `f32::exp` is the platform's expf, as `std::exp` is in Skia.
#[doc(alias = "Compute2DBlurKernel")]
#[allow(
    clippy::cast_precision_loss, // static_cast<float> of small radii: exact
    clippy::similar_names // the C++ names (sigmaXDenom/sigmaYDenom, xTerm/xyTerm)
)]
pub fn compute_2d_blur_kernel(sigma: Size, radius: ISize, kernel: &mut [f32]) {
    // Callers likely had to calculate the radius prior to filling out the kernel value, which is
    // why it's provided; but make sure it's consistent with expectations.
    debug_assert!(
        sigma_to_radius(sigma.width) == radius.width
            && sigma_to_radius(sigma.height) == radius.height
    );

    // Callers are responsible for downscaling large sigmas to values that can be processed by the
    // effects, so ensure the radius won't overflow 'kernel'
    let width = kernel_width(radius.width);
    let height = kernel_width(radius.height);
    let kernel_size = to_index(width) * to_index(height);
    debug_assert!(kernel_size <= kernel.len());

    // And the definition of an identity blur should be sufficient that 2sigma^2 isn't near zero
    // when there's a non-trivial radius.
    let two_sigma_sqrd_x = 2.0f32 * sigma.width * sigma.width;
    let two_sigma_sqrd_y = 2.0f32 * sigma.height * sigma.height;

    // Setting the denominator to 1 when the radius is 0 automatically converts the remaining math
    // to the 1D Gaussian distribution. When both radii are 0, it correctly computes a weight of 1.0
    let sigma_x_denom = if radius.width > 0 {
        1.0f32 / two_sigma_sqrd_x
    } else {
        1.0
    };
    let sigma_y_denom = if radius.height > 0 {
        1.0f32 / two_sigma_sqrd_y
    } else {
        1.0
    };

    let mut sum = 0.0f32;
    for x in 0..width {
        let mut x_term = (x - radius.width) as f32;
        x_term = x_term * x_term * sigma_x_denom;
        for y in 0..height {
            let y_term = (y - radius.height) as f32;
            let xy_term = (-(x_term + y_term * y_term * sigma_y_denom)).exp();
            // Note that the constant term (1/(sqrt(2*pi*sigma^2)) of the Gaussian
            // is dropped here, since we renormalize the kernel below.
            kernel[to_index(y * width + x)] = xy_term;
            sum += xy_term;
        }
    }
    // Normalize the kernel
    let scale = 1.0f32 / sum;
    for k in &mut kernel[..kernel_size] {
        *k *= scale;
    }
    // Zero remainder of the array
    kernel[kernel_size..].fill(0.0);
}

/// `Compute1DBlurKernel(sigma, radius, kernel)`: the 2D kernel with a zero height radius.
// Port of: src/core/SkBlurEngine.h#L215-L218 (chrome/m156)
#[doc(alias = "Compute1DBlurKernel")]
pub fn compute_1d_blur_kernel(sigma: f32, radius: i32, kernel: &mut [f32]) {
    compute_2d_blur_kernel(Size::new(sigma, 0.0), ISize::new(radius, 0), kernel);
}

/// `Compute2DBlurOffsets(radius, offsets)`: the `(x, y)` offset of each kernel sample, the last
/// valid offset repeated up to `kMaxSamples`.
// Port of: src/core/SkBlurEngine.cpp#L1382-L1403 (chrome/m156)
#[doc(alias = "Compute2DBlurOffsets")]
#[allow(clippy::cast_precision_loss)] // small integer offsets: exact
pub fn compute_2d_blur_offsets(radius: ISize, offsets: &mut [f32; 2 * N]) {
    let kernel_area = kernel_width(radius.width) * kernel_width(radius.height);
    debug_assert!(kernel_area <= MAX_SAMPLES);

    let mut i = 0usize;
    for y in -radius.height..=radius.height {
        for x in -radius.width..=radius.width {
            offsets[2 * i] = x as f32;
            offsets[2 * i + 1] = y as f32;
            i += 1;
        }
    }
    debug_assert_eq!(i, to_index(kernel_area));
    let last_valid_offset = 2 * (to_index(kernel_area) - 1);
    while i < N {
        offsets[2 * i] = offsets[last_valid_offset];
        offsets[2 * i + 1] = offsets[last_valid_offset + 1];
        i += 1;
    }
}

/// `Compute1DBlurLinearKernel(sigma, radius, offsetsAndKernel)`: a 1D kernel that pairs adjacent
/// Gaussian taps into one bilinear sample, interleaved as `{offset, weight, offset, weight}`.
// Port of: src/core/SkBlurEngine.cpp#L1405-L1488 (chrome/m156)
#[doc(alias = "Compute1DBlurLinearKernel")]
#[allow(clippy::cast_precision_loss)] // small integer offsets: exact
pub fn compute_1d_blur_linear_kernel(
    sigma: f32,
    radius: i32,
    offsets_and_kernel: &mut [f32; 2 * N],
) {
    // The maximum blur radius that can be passed to this function is (kMaxBlurSamples-1), so
    // make an array large enough to hold the full kernel width.
    const MAX_KERNEL_WIDTH: usize = 2 * (N - 1) + 1;

    debug_assert!(sigma <= MAX_LINEAR_SIGMA);
    debug_assert_eq!(radius, sigma_to_radius(sigma));
    debug_assert!(linear_kernel_width(radius) <= MAX_SAMPLES);

    // Given 2 adjacent gaussian points, they are blended as: Wi * Ci + Wj * Cj.
    // The GPU will mix Ci and Cj as Ci * (1 - x) + Cj * x during sampling.
    // Compute W', x such that W' * (Ci * (1 - x) + Cj * x) = Wi * Ci + Wj * Cj.
    // Solving W' * x = Wj, W' * (1 - x) = Wi:
    // W' = Wi + Wj
    // x = Wj / (Wi + Wj)
    let get_new_weight = |wi: f32, wj: f32| -> (f32, f32) { (wi + wj, wj / (wi + wj)) };

    // Create a temporary standard kernel.
    debug_assert!(to_index(kernel_width(radius)) <= MAX_KERNEL_WIDTH);
    let mut full_kernel = [0.0f32; MAX_KERNEL_WIDTH];
    compute_1d_blur_kernel(
        sigma,
        radius,
        &mut full_kernel[..to_index(kernel_width(radius))],
    );

    // (Every value is written below; the C++ arrays start uninitialized.)
    let mut kernel = [0.0f32; N];
    let mut offsets = [0.0f32; N];
    // Note that halfsize isn't just size / 2, but radius + 1. This is the size of the output
    // array.
    let half_size = linear_kernel_width(radius);
    let half_radius = half_size / 2;
    let mut low_index = half_radius - 1;

    // Compute1DGaussianKernel produces a full 2N + 1 kernel. Since the kernel can be mirrored,
    // compute only the upper half and mirror to the lower half.

    let mut index = radius;
    let hr = to_index(half_radius);
    if radius & 1 != 0 {
        // If N is odd, then use two samples.
        // The centre texel gets sampled twice, so halve its influence for each sample.
        // We essentially sample like this:
        // Texel edges
        // v    v    v    v
        // |    |    |    |
        // \-----^---/ Lower sample
        //      \---^-----/ Upper sample
        (kernel[hr], offsets[hr]) = get_new_weight(
            full_kernel[to_index(index)] * 0.5f32,
            full_kernel[to_index(index + 1)],
        );
        kernel[to_index(low_index)] = kernel[hr];
        offsets[to_index(low_index)] = -offsets[hr];
        index += 1;
        low_index -= 1;
    } else {
        // If N is even, then there are an even number of texels on either side of the centre
        // texel. Sample the centre texel directly.
        kernel[hr] = full_kernel[to_index(index)];
        offsets[hr] = 0.0;
    }
    index += 1;

    // Every other pair gets one sample.
    let mut i = half_radius + 1;
    while i < half_size {
        let iu = to_index(i);
        (kernel[iu], offsets[iu]) = get_new_weight(
            full_kernel[to_index(index)],
            full_kernel[to_index(index + 1)],
        );
        offsets[iu] += (index - radius) as f32;

        // Mirror to lower half.
        kernel[to_index(low_index)] = kernel[iu];
        offsets[to_index(low_index)] = -offsets[iu];

        index += 2;
        i += 1;
        low_index -= 1;
    }

    // Zero out remaining values in the kernel
    let hs = to_index(half_size);
    kernel[hs..].fill(0.0);
    // But copy the last valid offset into the remaining offsets, to increase the chance that
    // over-iteration in a fragment shader will have a cache hit.
    let last = offsets[hs - 1];
    offsets[hs..].fill(last);

    // Interleave into the output array to match the 1D SkSL effect
    for i in 0..N / 2 {
        offsets_and_kernel[4 * i] = offsets[2 * i];
        offsets_and_kernel[4 * i + 1] = kernel[2 * i];
        offsets_and_kernel[4 * i + 2] = offsets[2 * i + 1];
        offsets_and_kernel[4 * i + 3] = kernel[2 * i + 1];
    }
}

/// `to_stablekey(kernelWidth, baseKey)`: the blur effect that has room for `kernel_width`
/// samples (batched on multiples of 4, then 8).
// Port of: src/core/SkBlurEngine.cpp#L1490-L1526 (chrome/m156)
fn to_stablekey(kernel_width: i32, base_key: StableKey) -> StableKey {
    debug_assert!((2..=MAX_SAMPLES).contains(&kernel_width));
    let offset = match kernel_width {
        // Batch on multiples of 4 (skipping width=1, since that can't happen)
        2..=4 => 0,
        5..=8 => 1,
        9..=12 => 2,
        13..=16 => 3,
        // With larger kernels, batch on multiples of eight so up to 7 wasted samples.
        17..=20 => 4,
        21..=28 => 5,
        _ => unreachable!("a blur kernel width is in 2..=28"),
    };
    stable_key_from_u32(base_key as u32 + offset).expect("a blur stable key")
}

/// `GetLinearBlur1DEffect(radius)`.
///
/// # Panics
///
/// If `LinearKernelWidth(radius)` is not in `2..=kMaxSamples` (`SkUNREACHABLE`).
// Port of: src/core/SkBlurEngine.cpp#L1528-L1532 (chrome/m156)
#[doc(alias = "GetLinearBlur1DEffect")]
#[must_use]
pub fn get_linear_blur_1d_effect(radius: i32) -> &'static RuntimeEffect {
    get_known_runtime_effect(to_stablekey(
        linear_kernel_width(radius),
        StableKey::ONE_D_BLUR_BASE,
    ))
    .expect("the 1D blur effects compile")
}

/// `GetBlur2DEffect(radii)`.
///
/// # Panics
///
/// If the kernel area is not in `2..=kMaxSamples` (`SkUNREACHABLE`).
// Port of: src/core/SkBlurEngine.cpp#L1534-L1539 (chrome/m156)
#[doc(alias = "GetBlur2DEffect")]
#[must_use]
pub fn get_blur_2d_effect(radii: ISize) -> &'static RuntimeEffect {
    let kernel_area = kernel_width(radii.width) * kernel_width(radii.height);
    get_known_runtime_effect(to_stablekey(kernel_area, StableKey::TWO_D_BLUR_BASE))
        .expect("the 2D blur effects compile")
}

/// `builder.child("child") = shader`: a null shader leaves the child unset, as in C++.
fn set_child(builder: &mut RuntimeEffectBuilder, shader: Option<crate::shader::Shader>) {
    let mut child = builder.child("child");
    match shader {
        Some(shader) => child.assign(shader),
        None => child.assign_null(),
    };
}

/// `SkShaderBlurAlgorithm`: implementors provide [`make_device`](Self::make_device) and forward
/// their `SkBlurEngine::Algorithm` (`crate::blur_engine::BlurAlgorithm`) to
/// [`shader_blur`](Self::shader_blur); its `maxSigma` is [`MAX_LINEAR_SIGMA`] and it supports
/// every tile mode.
// Port of: src/core/SkBlurEngine.h#L134-L168 (chrome/m156)
#[doc(alias = "SkShaderBlurAlgorithm")]
pub trait ShaderBlurAlgorithm {
    /// Create a new surface, which can be approx-fit and have undefined contents (`makeDevice`).
    #[doc(alias = "makeDevice")]
    fn make_device(&self, image_info: &ImageInfo) -> Option<Box<dyn Device>>;

    /// `renderBlur(blurEffectBuilder, filter, radii, input, srcRect, tileMode, dstRect)`.
    // Port of: src/core/SkBlurEngine.cpp#L1541-L1629 (chrome/m156)
    #[doc(alias = "renderBlur")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn render_blur(
        &self,
        blur_effect_builder: &mut RuntimeEffectBuilder,
        filter: FilterMode,
        radii: ISize,
        input: &SpecialImage,
        src_rect: &IRect,
        tile_mode: TileMode,
        dst_rect: &IRect,
    ) -> Option<SpecialImage> {
        let out_ii = ImageInfo::new(
            (dst_rect.width(), dst_rect.height()),
            input.color_info().color_type(),
            AlphaType::Premul,
            input.color_info().color_space(),
        );
        let mut device = self.make_device(&out_ii)?;

        let subset = IRect::from_size(dst_rect.size());
        device.clip_rect(
            &Rect::from_irect(subset),
            ClipOp::Intersect,
            /*aa=*/ false,
        );
        #[allow(clippy::cast_precision_loss)] // mirrors SkM44::Translate of int coordinates
        device.state_mut().set_local_to_device(&M44::translate(
            -dst_rect.left() as f32,
            -dst_rect.top() as f32,
            0.0,
        ));

        // renderBlur() will either mix multiple fast and strict draws to cover dstRect, or will
        // issue a single strict draw. While the SkShader object changes (really just strict
        // mode), the rest of the SkPaint remains the same.
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);

        let safe_src_rect = src_rect.with_inset((radii.width, radii.height));
        let mut fast_dst_rect = *dst_rect;

        // Only consider the safeSrcRect for shader-based tiling if the original srcRect is
        // different from the backing store dimensions; when they match the full image we can use
        // HW tiling.
        if *src_rect != IRect::from_size(input.backing_store_dimensions()) {
            if let Some(intersection) = IRect::intersect(&fast_dst_rect, &safe_src_rect) {
                fast_dst_rect = intersection;
                // If the area of the non-clamping shader is small, it's better to just issue a
                // single draw that performs shader tiling over the whole dst.
                if fast_dst_rect != *dst_rect
                    && fast_dst_rect.width() * fast_dst_rect.height() < 128 * 128
                {
                    fast_dst_rect.set_empty();
                }
            } else {
                fast_dst_rect.set_empty();
            }
        }

        if !fast_dst_rect.is_empty() {
            // Fill as much as possible without adding shader tiling logic to each blur sample,
            // switching to clamp tiling if we aren't in this block due to HW tiling.
            let untiled_src_rect = src_rect.with_inset((1, 1));
            let fast_tile_mode = if untiled_src_rect.contains(&fast_dst_rect) {
                TileMode::Clamp
            } else {
                tile_mode
            };
            let child = input.as_shader(
                fast_tile_mode,
                SamplingOptions::from(filter),
                &Matrix::new_identity(),
                /*strict=*/ false,
            );
            set_child(blur_effect_builder, child);
            paint.set_shader(blur_effect_builder.make_shader(None));
            device.draw_rect(&Rect::from_irect(fast_dst_rect), &paint);
        }

        // Switch to a strict shader if there are remaining pixels to fill
        if fast_dst_rect != *dst_rect {
            #[allow(clippy::cast_precision_loss)] // mirrors SkMatrix::Translate of int coordinates
            let translate = Matrix::translate((src_rect.left() as f32, src_rect.top() as f32));
            let child = input.make_subset(src_rect)?.as_shader(
                tile_mode,
                SamplingOptions::from(filter),
                &translate,
                /*strict=*/ true,
            );
            set_child(blur_effect_builder, child);
            paint.set_shader(blur_effect_builder.make_shader(None));
        }

        if fast_dst_rect.is_empty() {
            // Fill the entire dst with the strict shader
            device.draw_rect(&Rect::from_irect(dst_rect), &paint);
        } else if fast_dst_rect != *dst_rect {
            // There will be up to four additional strict draws to fill in the border. The left
            // and right sides will span the full height of the dst rect. The top and bottom will
            // span the just the width of the fast interior. Strict border draws with zero
            // width/height are skipped.
            let mut draw_border = |r: IRect| {
                if !r.is_empty() {
                    device.draw_rect(&Rect::from_irect(r), &paint);
                }
            };

            draw_border(IRect::new(
                dst_rect.left(),
                dst_rect.top(),
                fast_dst_rect.left(),
                dst_rect.bottom(),
            )); // Left, spanning full height
            draw_border(IRect::new(
                fast_dst_rect.right(),
                dst_rect.top(),
                dst_rect.right(),
                dst_rect.bottom(),
            )); // Right, spanning full height
            draw_border(IRect::new(
                fast_dst_rect.left(),
                dst_rect.top(),
                fast_dst_rect.right(),
                fast_dst_rect.top(),
            )); // Top, spanning inner width
            draw_border(IRect::new(
                fast_dst_rect.left(),
                fast_dst_rect.bottom(),
                fast_dst_rect.right(),
                dst_rect.bottom(),
            )); // Bottom, spanning inner width
        }

        device.set_immutable();
        device.snap_special(&subset, false)
    }

    /// `evalBlur2D(sigma, radii, input, srcRect, tileMode, dstRect)`.
    // Port of: src/core/SkBlurEngine.cpp#L1631-L1649 (chrome/m156)
    #[doc(alias = "evalBlur2D")]
    fn eval_blur_2d(
        &self,
        sigma: Size,
        radii: ISize,
        input: &SpecialImage,
        src_rect: &IRect,
        tile_mode: TileMode,
        dst_rect: &IRect,
    ) -> Option<SpecialImage> {
        let mut kernel = [0.0f32; N];
        let mut offsets = [0.0f32; 2 * N];
        compute_2d_blur_kernel(sigma, radii, &mut kernel);
        compute_2d_blur_offsets(radii, &mut offsets);

        let mut builder = RuntimeEffectBuilder::new(get_blur_2d_effect(radii).clone());
        builder.uniform("kernel").set_f32(&kernel);
        builder.uniform("offsets").set_f32(&offsets);
        // NOTE: renderBlur() will configure the "child" shader as needed. The 2D blur effect only
        // requires nearest-neighbor filtering.
        self.render_blur(
            &mut builder,
            FilterMode::Nearest,
            radii,
            input,
            src_rect,
            tile_mode,
            dst_rect,
        )
    }

    /// `evalBlur1D(sigma, radius, dir, input, srcRect, tileMode, dstRect)`.
    // Port of: src/core/SkBlurEngine.cpp#L1651-L1669 (chrome/m156)
    #[doc(alias = "evalBlur1D")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn eval_blur_1d(
        &self,
        sigma: f32,
        radius: i32,
        dir: [f32; 2],
        input: &SpecialImage,
        src_rect: &IRect,
        tile_mode: TileMode,
        dst_rect: &IRect,
    ) -> Option<SpecialImage> {
        let mut offsets_and_kernel = [0.0f32; 2 * N];
        compute_1d_blur_linear_kernel(sigma, radius, &mut offsets_and_kernel);

        let mut builder = RuntimeEffectBuilder::new(get_linear_blur_1d_effect(radius).clone());
        builder
            .uniform("offsetsAndKernel")
            .set_f32(&offsets_and_kernel);
        builder.uniform("dir").set_f32(&dir);
        // NOTE: renderBlur() will configure the "child" shader as needed. The 1D blur effect
        // requires linear filtering. Reconstruct the appropriate "2D" radii inset value from
        // 'dir'.
        let radii = ISize::new(
            if dir[0] == 0.0 { 0 } else { radius },
            if dir[1] == 0.0 { 0 } else { radius },
        );
        self.render_blur(
            &mut builder,
            FilterMode::Linear,
            radii,
            input,
            src_rect,
            tile_mode,
            dst_rect,
        )
    }

    /// `SkShaderBlurAlgorithm::blur(sigma, src, srcRect, tileMode, dstRect)`: one 2D pass when
    /// the kernel fits, else a 1D pass per axis.
    // Port of: src/core/SkBlurEngine.cpp#L1671-L1745 (chrome/m156)
    fn shader_blur(
        &self,
        sigma: Size,
        src: &SpecialImage,
        src_rect: IRect,
        tile_mode: TileMode,
        dst_rect: IRect,
    ) -> Option<SpecialImage> {
        debug_assert!(sigma.width <= MAX_LINEAR_SIGMA && sigma.height <= MAX_LINEAR_SIGMA);

        let radius_x = sigma_to_radius(sigma.width);
        let radius_y = sigma_to_radius(sigma.height);
        let kernel_area = kernel_width(radius_x) * kernel_width(radius_y);
        if kernel_area <= MAX_SAMPLES && radius_x > 0 && radius_y > 0 {
            // Use a single-pass 2D kernel if it fits and isn't just 1D already
            return self.eval_blur_2d(
                sigma,
                ISize::new(radius_x, radius_y),
                src,
                &src_rect,
                tile_mode,
                &dst_rect,
            );
        }
        // Use two passes of a 1D kernel (one per axis).
        let mut intermediate: Option<SpecialImage> = None;
        let mut intermediate_src_rect = src_rect;
        let mut intermediate_dst_rect = dst_rect;
        if radius_x > 0 {
            if radius_y > 0 {
                // May need to maintain extra rows above and below 'dstRect' for the follow-up
                // pass.
                if tile_mode == TileMode::Repeat || tile_mode == TileMode::Mirror {
                    // If the srcRect and dstRect are aligned, then we don't need extra rows since
                    // the periodic tiling on srcRect is the same for the intermediate. If they
                    // are not aligned, then outset by the Y radius.
                    let period =
                        src_rect.height() * if tile_mode == TileMode::Mirror { 2 } else { 1 };
                    if (dst_rect.top() - src_rect.top()).abs() % period != 0
                        || dst_rect.height() != src_rect.height()
                    {
                        intermediate_dst_rect.outset((0, radius_y));
                    }
                } else {
                    // For clamp and decal tiling, we outset by the Y radius up to what's
                    // available from the srcRect. Anything beyond that is identical to tiling
                    // the intermediate dst image directly.
                    intermediate_dst_rect.outset((0, radius_y));
                    intermediate_dst_rect.top = intermediate_dst_rect.top.max(src_rect.top);
                    intermediate_dst_rect.bottom =
                        intermediate_dst_rect.bottom.min(src_rect.bottom);
                    if intermediate_dst_rect.top >= intermediate_dst_rect.bottom {
                        return None;
                    }
                }
            }

            let blurred = self.eval_blur_1d(
                sigma.width,
                radius_x,
                /*dir=*/ [1.0, 0.0],
                src,
                &src_rect,
                tile_mode,
                &intermediate_dst_rect,
            )?;
            intermediate_src_rect = IRect::from_wh(blurred.width(), blurred.height());
            intermediate_dst_rect =
                dst_rect.with_offset((-intermediate_dst_rect.left(), -intermediate_dst_rect.top()));
            intermediate = Some(blurred);
        }

        if radius_y > 0 {
            return self.eval_blur_1d(
                sigma.height,
                radius_y,
                /*dir=*/ [0.0, 1.0],
                intermediate.as_ref().unwrap_or(src),
                &intermediate_src_rect,
                tile_mode,
                &intermediate_dst_rect,
            );
        }

        // `return src;`: the first pass's result (or the input when neither axis blurs).
        Some(intermediate.unwrap_or_else(|| src.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The kernels sum to one and are symmetric (`Compute1DBlurLinearKernel` mirrors the upper
    // half).
    #[test]
    fn linear_kernel_is_mirrored() {
        let mut ok = [0.0f32; 2 * N];
        let sigma = 2.0;
        let radius = sigma_to_radius(sigma);
        compute_1d_blur_linear_kernel(sigma, radius, &mut ok);
        let half = to_index(linear_kernel_width(radius));
        let offsets: Vec<f32> = (0..N).map(|i| ok[2 * i]).collect();
        let weights: Vec<f32> = (0..N).map(|i| ok[2 * i + 1]).collect();
        for i in 0..half {
            assert_eq!(offsets[i], -offsets[half - 1 - i]);
            assert_eq!(weights[i], weights[half - 1 - i]);
        }
        assert!(weights[half..].iter().all(|w| *w == 0.0));
    }

    #[test]
    fn stable_keys_batch_by_four_then_eight() {
        assert_eq!(
            to_stablekey(2, StableKey::ONE_D_BLUR_BASE),
            StableKey::OneDBlur4
        );
        assert_eq!(
            to_stablekey(9, StableKey::ONE_D_BLUR_BASE),
            StableKey::OneDBlur12
        );
        assert_eq!(
            to_stablekey(20, StableKey::TWO_D_BLUR_BASE),
            StableKey::TwoDBlur20
        );
        assert_eq!(
            to_stablekey(21, StableKey::TWO_D_BLUR_BASE),
            StableKey::TwoDBlur28
        );
    }
}

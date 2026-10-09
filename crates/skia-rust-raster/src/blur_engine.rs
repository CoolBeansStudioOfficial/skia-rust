// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlurEngine.cpp (the CPU passes and RasterBlurEngine)

//! The CPU blur engine (`SkBlurEngine::GetRasterBlurEngine`): separable blurs made of passes
//! over 8-bit channels. Gaussian passes handle small sigmas; larger ones use a three-box
//! approximation (32-bit pixels), a tent filter (for very large 32-bit sigmas) or the A8 box
//! approximation (alpha-only images).
//!
//! Not ported: the shader-based algorithm (`SkShaderBlurAlgorithm`, `SkSL` runtime effects) that
//! Skia uses for every other color type. `find_algorithm` returns `None` for those, so a blur of
//! them produces no output.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_engine::{BlurAlgorithm, BlurEngine, box_blur_window, sigma_to_radius};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::floating_point::DOUBLE_PI;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_simd::vx::ScaledDividerU32;

/// One pass of a separable blur over values of `N` bytes (`Pass`). `blur_segment` consumes `n`
/// values: a source value (`src`, with its stride) is optional, as is a destination (`dst`).
// Port of: src/core/SkBlurEngine.cpp#L62-L139 (chrome/m156)
trait Pass<const N: usize> {
    /// The distance in values between the first destination value and the first source value.
    fn border(&self) -> i32;

    fn start_blur(&mut self);

    fn blur_segment(
        &mut self,
        n: usize,
        src: Option<(&[[u8; N]], usize)>,
        dst: Option<(&mut [[u8; N]], usize)>,
    );
}

/// Converts a non-negative count (guaranteed by the callers) to a `usize`.
fn to_usize(v: i32) -> usize {
    usize::try_from(v).unwrap_or(0)
}

/// A count of values (a pass length or an index into one) as `i32`: such lengths are far below
/// `i32::MAX`.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // blur lines are small
fn count_i32(n: usize) -> i32 {
    n as i32
}

/// Truncates a value in `0..=255` to a byte (the `uint8_t` conversion of Skia).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // callers clamp to 0..=255
fn trunc_u8(v: f32) -> u8 {
    v as u8
}

/// `Pass::blur`: runs `pass` over one line. `src` and `dst` start at the first value of the line,
/// with `src_left`, `src_right` and `dst_right` relative to `dst`'s first value.
// Port of: src/core/SkBlurEngine.cpp#L69-L134 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors Pass::blur
fn pass_blur<const N: usize>(
    pass: &mut dyn Pass<N>,
    src_left: i32,
    src_right: i32,
    dst_right: i32,
    src: &[[u8; N]],
    src_stride: usize,
    dst: &mut [[u8; N]],
    dst_stride: usize,
) {
    pass.start_blur();
    let border = pass.border();

    let src_start = src_left - border;
    let src_end = src_right - border;
    let dst_end = dst_right;
    let mut src_idx = src_start;
    let mut dst_idx = 0;
    let mut src_pos = 0usize;
    let mut dst_pos = 0usize;

    if dst_idx < src_idx {
        // The destination pixels are not effected by the src pixels, change to zero.
        let common_end = src_idx.min(dst_end);
        while dst_idx < common_end {
            dst[dst_pos] = [0; N];
            dst_pos += dst_stride;
            dst_idx += 1;
        }
    } else if src_idx < dst_idx {
        // The edge of the source is before the edge of the destination. Calculate the sums for
        // the pixels before the start of the destination.
        let common_end = dst_idx.min(src_end);
        if src_idx < common_end {
            // Preload the blur with values from src before dst is entered.
            let n = to_usize(common_end - src_idx);
            pass.blur_segment(n, Some((&src[src_pos..], src_stride)), None);
            src_idx += count_i32(n);
            src_pos += n * src_stride;
        }
        if src_idx < dst_idx {
            // The weird case where src is out of pixels before dst is even started.
            let n = to_usize(dst_idx - src_idx);
            pass.blur_segment(n, None, None);
        }
    }

    let common_end = dst_end.min(src_end);
    if dst_idx < common_end {
        // Both srcIdx and dstIdx are in sync now, and can run in a 1:1 fashion.
        let n = to_usize(common_end - dst_idx);
        pass.blur_segment(
            n,
            Some((&src[src_pos..], src_stride)),
            Some((&mut dst[dst_pos..], dst_stride)),
        );
        dst_pos += n * dst_stride;
        dst_idx += count_i32(n);
    }

    // Drain the remaining blur values into dst assuming 0's for the leading edge.
    if dst_idx < dst_end {
        let n = to_usize(dst_end - dst_idx);
        pass.blur_segment(n, None, Some((&mut dst[dst_pos..], dst_stride)));
    }
}

/// `GaussianPass`: a true 1D Gaussian kernel (for sigma < 2).
// Port of: src/core/SkBlurEngine.cpp#L274-L378 (chrome/m156)
struct GaussianPass<const N: usize> {
    radius: i32,
    window: usize,
    kernel: Vec<f32>,
    src_buffer: Vec<[f32; N]>,
    src_buffer_base: usize,
}

impl<const N: usize> GaussianPass<N> {
    /// `GaussianPass::Make`.
    // Port of: src/core/SkBlurEngine.cpp#L301-L311 (chrome/m156)
    fn new(sigma: f32) -> Self {
        let radius = sigma_to_radius(sigma);
        let kernel_width = to_usize(2 * radius + 1);
        let mut kernel = vec![0.0f32; kernel_width];
        compute_1d_blur_kernel(sigma, radius, &mut kernel);
        GaussianPass {
            radius,
            window: kernel_width,
            kernel,
            src_buffer: vec![[0.0; N]; kernel_width],
            src_buffer_base: 0,
        }
    }

    /// `convolve(srcBase)`: one blurred value per lane.
    // Port of: src/core/SkBlurEngine.cpp#L339-L346 (chrome/m156)
    fn convolve(&self, src_base: usize) -> [u8; N] {
        let mut sum = [0.0f32; N];
        for i in 0..self.window {
            let s = (i + src_base) % self.window;
            for (lane, sum_lane) in sum.iter_mut().enumerate() {
                *sum_lane += self.src_buffer[s][lane] * self.kernel[i];
            }
        }
        let mut out = [0u8; N];
        for (lane, out_lane) in out.iter_mut().enumerate() {
            // skvx::pin(sum * 255 + 0.5, 0, 255), then a truncating cast to uint8_t.
            let v = sum[lane] * 255.0 + 0.5;
            let v = if v > 255.0 { 255.0 } else { v };
            let v = if v < 0.0 { 0.0 } else { v };
            *out_lane = trunc_u8(v);
        }
        out
    }
}

impl<const N: usize> Pass<N> for GaussianPass<N> {
    fn border(&self) -> i32 {
        self.radius
    }

    // Port of: src/core/SkBlurEngine.cpp#L352-L355 (chrome/m156)
    fn start_blur(&mut self) {
        // Zero out the source buffer to ensure a clean state.
        self.src_buffer.fill([0.0; N]);
        // Reset the circular buffer's starting position.
        self.src_buffer_base = 0;
    }

    // Port of: src/core/SkBlurEngine.cpp#L357-L378 (chrome/m156)
    fn blur_segment(
        &mut self,
        n: usize,
        src: Option<(&[[u8; N]], usize)>,
        mut dst: Option<(&mut [[u8; N]], usize)>,
    ) {
        // Load the state from the last run.
        let mut base = self.src_buffer_base;
        let mut src_idx = 0usize;
        let mut dst_idx = 0usize;
        for _ in 0..n {
            // Load the new leading edge into the circular buffer.
            let mut leading_edge = [0.0f32; N];
            if let Some((s, _)) = src {
                let px = s.get(src_idx).copied().unwrap_or([0; N]);
                for (lane, edge) in leading_edge.iter_mut().enumerate() {
                    // skvx::cast<float>(uint8) * (1 / 255.0f)
                    *edge = f32::from(px[lane]) * (1.0 / 255.0);
                }
            }
            self.src_buffer[(base + self.window - 1) % self.window] = leading_edge;

            // Perform the convolution and store the result.
            if let Some((d, stride)) = dst.as_mut() {
                let out = self.convolve(base);
                if let Some(slot) = d.get_mut(dst_idx) {
                    *slot = out;
                }
                dst_idx += *stride;
            }

            // Advance the source pointer (if it exists) and the circular buffer base.
            if let Some((_, stride)) = src {
                src_idx += stride;
            }
            base = (base + 1) % self.window;
        }
        self.src_buffer_base = base;
    }
}

/// `ThreeBoxApproxPass`: three box blurs combined into one pass, for 32-bit pixels with
/// sigma >= 2 (window < 255).
// Port of: src/core/SkBlurEngine.cpp#L381-L560 (chrome/m156)
struct ThreeBoxApproxPass {
    border: i32,
    buffer: Vec<[u32; 4]>,
    /// Start of the second pass's circular buffer (`fBuffer1`).
    buffer1_start: usize,
    /// Start of the third pass's circular buffer (`fBuffer2`).
    buffer2_start: usize,
    /// End of the third circular buffer (`fBuffersEnd`).
    buffers_end: usize,
    divider: ScaledDividerU32,
    sum0: [u32; 4],
    sum1: [u32; 4],
    sum2: [u32; 4],
    cursor0: usize,
    cursor1: usize,
    cursor2: usize,
}

impl ThreeBoxApproxPass {
    /// `ThreeBoxApproxPass::Make`.
    // Port of: src/core/SkBlurEngine.cpp#L436-L488 (chrome/m156)
    fn new(window: i32) -> Self {
        // We don't need to store the trailing edge pixel in the buffer.
        let pass_size = to_usize(window - 1);
        let buffer1_start = pass_size;
        let buffer2_start = 2 * pass_size;
        // If the window is odd just one buffer is needed, but if it's even, then there is one
        // more element on that pass.
        let buffers_end = buffer2_start
            + if (window & 1) == 1 {
                pass_size
            } else {
                pass_size + 1
            };
        let border = if (window & 1) == 1 {
            3 * ((window - 1) / 2)
        } else {
            3 * (window / 2) - 1
        };
        // If the window is odd then the divisor is just window ^ 3 otherwise, it is
        // window * window * (window + 1) = window ^ 3 + window ^ 2.
        let window2 = window * window;
        let window3 = window2 * window;
        let divisor = if (window & 1) == 1 {
            window3
        } else {
            window3 + window2
        };
        ThreeBoxApproxPass {
            border,
            buffer: vec![[0; 4]; buffers_end],
            buffer1_start,
            buffer2_start,
            buffers_end,
            divider: ScaledDividerU32::new(u32::try_from(divisor).unwrap_or(0)),
            sum0: [0; 4],
            sum1: [0; 4],
            sum2: [0; 4],
            cursor0: 0,
            cursor1: buffer1_start,
            cursor2: buffer2_start,
        }
    }

    /// The `MakeMaker` check: windows of 255 or more are too large for the 8-bit sums.
    // Port of: src/core/SkBlurEngine.cpp#L404-L409 (chrome/m156)
    fn max_window_ok(sigma: f32) -> Option<i32> {
        let window = box_blur_window(sigma);
        if 255 <= window { None } else { Some(window) }
    }
}

impl Pass<4> for ThreeBoxApproxPass {
    fn border(&self) -> i32 {
        self.border
    }

    // Port of: src/core/SkBlurEngine.cpp#L557-L564 (chrome/m156)
    fn start_blur(&mut self) {
        self.sum0 = [0; 4];
        self.sum1 = [0; 4];
        let half = self.divider.half();
        self.sum2 = [half; 4];
        self.buffer.fill([0; 4]);
        self.cursor0 = 0;
        self.cursor1 = self.buffer1_start;
        self.cursor2 = self.buffer2_start;
    }

    // Port of: src/core/SkBlurEngine.cpp#L619-L688 (chrome/m156), the scalar branch
    fn blur_segment(
        &mut self,
        n: usize,
        src: Option<(&[[u8; 4]], usize)>,
        mut dst: Option<(&mut [[u8; 4]], usize)>,
    ) {
        let mut src_idx = 0usize;
        let mut dst_idx = 0usize;
        for _ in 0..n {
            let leading = match src {
                Some((s, _)) => s.get(src_idx).copied().unwrap_or([0; 4]),
                None => [0; 4],
            };
            // Given an expanded input pixel, move the window ahead using the leadingEdge value.
            let blurred = self.process_value(leading);
            if let Some((d, stride)) = dst.as_mut() {
                if let Some(slot) = d.get_mut(dst_idx) {
                    *slot = blurred;
                }
                dst_idx += *stride;
            }
            if let Some((_, stride)) = src {
                src_idx += stride;
            }
        }
    }
}

impl ThreeBoxApproxPass {
    /// `processValue`, for the non-LSX path: the per-lane arithmetic is `uint32` with wrapping
    /// sums and the divider's `(n * factor) >> 32`.
    // Port of: src/core/SkBlurEngine.cpp#L647-L663 (chrome/m156)
    #[allow(clippy::needless_range_loop)] // four parallel lanes, as the SIMD code
    fn process_value(&mut self, leading: [u8; 4]) -> [u8; 4] {
        let leading: [u32; 4] = leading.map(u32::from);
        for lane in 0..4 {
            self.sum0[lane] = self.sum0[lane].wrapping_add(leading[lane]);
            self.sum1[lane] = self.sum1[lane].wrapping_add(self.sum0[lane]);
            self.sum2[lane] = self.sum2[lane].wrapping_add(self.sum1[lane]);
        }
        // fDivider.divide(sum2): cast<uint32_t>((cast<uint64_t>(numerator) * factor) >> 32)
        let factor = u64::from(self.divider.divisor_factor());
        let mut blurred = [0u8; 4];
        for lane in 0..4 {
            let q = ((u64::from(self.sum2[lane]) * factor) >> 32) as u32;
            // skvx::cast<uint8_t>: a truncating conversion.
            blurred[lane] = q as u8;
        }

        let c2 = self.cursor2;
        for lane in 0..4 {
            self.sum2[lane] = self.sum2[lane].wrapping_sub(self.buffer[c2][lane]);
            self.buffer[c2][lane] = self.sum1[lane];
        }
        self.cursor2 = if c2 + 1 < self.buffers_end {
            c2 + 1
        } else {
            self.buffer2_start
        };

        let c1 = self.cursor1;
        for lane in 0..4 {
            self.sum1[lane] = self.sum1[lane].wrapping_sub(self.buffer[c1][lane]);
            self.buffer[c1][lane] = self.sum0[lane];
        }
        self.cursor1 = if c1 + 1 < self.buffer2_start {
            c1 + 1
        } else {
            self.buffer1_start
        };

        let c0 = self.cursor0;
        for lane in 0..4 {
            self.sum0[lane] = self.sum0[lane].wrapping_sub(self.buffer[c0][lane]);
            self.buffer[c0][lane] = leading[lane];
        }
        self.cursor0 = if c0 + 1 < self.buffer1_start {
            c0 + 1
        } else {
            0
        };

        blurred
    }
}

/// `TentPass`: two box passes with a tent kernel, for very large 32-bit sigmas.
// Port of: src/core/SkBlurEngine.cpp#L716-L946 (chrome/m156)
struct TentPass {
    border: i32,
    buffer: Vec<[u32; 4]>,
    buffer1_start: usize,
    buffers_end: usize,
    divider: ScaledDividerU32,
    sum0: [u32; 4],
    sum1: [u32; 4],
    cursor0: usize,
    cursor1: usize,
}

impl TentPass {
    /// `TentPass::Make`.
    // Port of: src/core/SkBlurEngine.cpp#L762-L795 (chrome/m156)
    fn new(window: i32) -> Self {
        // We don't need to store the trailing edge pixel in the buffer.
        let pass_size = to_usize(window - 1);
        let buffer1_start = pass_size;
        let buffers_end = buffer1_start + pass_size;
        let border = window - 1;
        let divisor = window * window;
        TentPass {
            border,
            buffer: vec![[0; 4]; buffers_end],
            buffer1_start,
            buffers_end,
            divider: ScaledDividerU32::new(u32::try_from(divisor).unwrap_or(0)),
            sum0: [0; 4],
            sum1: [0; 4],
            cursor0: 0,
            cursor1: buffer1_start,
        }
    }
}

impl Pass<4> for TentPass {
    fn border(&self) -> i32 {
        self.border
    }

    // Port of: src/core/SkBlurEngine.cpp#L828-L840 (chrome/m156)
    fn start_blur(&mut self) {
        self.sum0 = [0; 4];
        let half = self.divider.half();
        self.sum1 = [half; 4];
        self.buffer.fill([0; 4]);
        self.cursor0 = 0;
        self.cursor1 = self.buffer1_start;
    }

    // Port of: src/core/SkBlurEngine.cpp#L842-L906 (chrome/m156)
    fn blur_segment(
        &mut self,
        n: usize,
        src: Option<(&[[u8; 4]], usize)>,
        mut dst: Option<(&mut [[u8; 4]], usize)>,
    ) {
        let mut src_idx = 0usize;
        let mut dst_idx = 0usize;
        for _ in 0..n {
            let leading = match src {
                Some((s, _)) => s.get(src_idx).copied().unwrap_or([0; 4]),
                None => [0; 4],
            };
            let blurred = self.process_value(leading);
            if let Some((d, stride)) = dst.as_mut() {
                if let Some(slot) = d.get_mut(dst_idx) {
                    *slot = blurred;
                }
                dst_idx += *stride;
            }
            if let Some((_, stride)) = src {
                src_idx += stride;
            }
        }
    }
}

impl TentPass {
    // Port of: src/core/SkBlurEngine.cpp#L880-L905 (chrome/m156), the scalar processValue
    #[allow(clippy::needless_range_loop)] // four parallel lanes, as the SIMD code
    fn process_value(&mut self, leading: [u8; 4]) -> [u8; 4] {
        let leading: [u32; 4] = leading.map(u32::from);
        for lane in 0..4 {
            self.sum0[lane] = self.sum0[lane].wrapping_add(leading[lane]);
            self.sum1[lane] = self.sum1[lane].wrapping_add(self.sum0[lane]);
        }
        let factor = u64::from(self.divider.divisor_factor());
        let mut blurred = [0u8; 4];
        for lane in 0..4 {
            let q = ((u64::from(self.sum1[lane]) * factor) >> 32) as u32;
            blurred[lane] = q as u8;
        }

        let c1 = self.cursor1;
        for lane in 0..4 {
            self.sum1[lane] = self.sum1[lane].wrapping_sub(self.buffer[c1][lane]);
            self.buffer[c1][lane] = self.sum0[lane];
        }
        self.cursor1 = if c1 + 1 < self.buffers_end {
            c1 + 1
        } else {
            self.buffer1_start
        };

        let c0 = self.cursor0;
        for lane in 0..4 {
            self.sum0[lane] = self.sum0[lane].wrapping_sub(self.buffer[c0][lane]);
            self.buffer[c0][lane] = leading[lane];
        }
        self.cursor0 = if c0 + 1 < self.buffer1_start {
            c0 + 1
        } else {
            0
        };

        blurred
    }
}

/// `A8Pass`: three box blurs over 8-bit values with a fixed-point reciprocal of the divisor.
// Port of: src/core/SkBlurEngine.cpp#L948-L1152 (chrome/m156)
struct A8Pass {
    weight: u64,
    border: i32,
    buffer: Vec<u32>,
    buffer0_end: usize,
    buffer1_start: usize,
    buffer1_end: usize,
    buffer2_start: usize,
    buffer2_end: usize,
    cursor0: usize,
    cursor1: usize,
    cursor2: usize,
    sum0: u32,
    sum1: u32,
    sum2: u32,
}

impl A8Pass {
    /// `A8Pass::MakeMaker`'s window: `floor(sigma * 3 * sqrt(2 * pi) / 4 + 0.5)` in double, at
    /// least 1.
    // Port of: src/core/SkBlurEngine.cpp#L954-L957 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // floor of a small window, as static_cast<int>
    fn window_for(sigma: f32) -> i32 {
        let possible_window =
            (f64::from(sigma * 3.0) * (2.0 * DOUBLE_PI).sqrt() / 4.0 + 0.5).floor() as i32;
        possible_window.max(1)
    }

    /// `A8Pass::Make`.
    // Port of: src/core/SkBlurEngine.cpp#L976-L1000 (chrome/m156)
    fn new(window: i32) -> Self {
        let pass0_size = to_usize(window - 1);
        let pass1_size = to_usize(window - 1);
        let pass2_size = if (window & 1) == 1 {
            to_usize(window - 1)
        } else {
            to_usize(window)
        };
        let buffer0_end = pass0_size;
        let buffer1_start = buffer0_end;
        let buffer1_end = buffer1_start + pass1_size;
        let buffer2_start = buffer1_end;
        let buffer2_end = buffer2_start + pass2_size;

        let border = if (window & 1) == 1 {
            3 * ((window - 1) / 2)
        } else {
            3 * (window / 2) - 1
        };
        let window2 = window * window;
        let window3 = window2 * window;
        let divisor = if (window & 1) == 1 {
            window3
        } else {
            window3 + window2
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // (uint64_t)round(..)
        let weight = ((1.0 / f64::from(divisor)) * 4_294_967_296.0).round() as u64;
        A8Pass {
            weight,
            border,
            buffer: vec![0; buffer2_end],
            buffer0_end,
            buffer1_start,
            buffer1_end,
            buffer2_start,
            buffer2_end,
            cursor0: 0,
            cursor1: buffer1_start,
            cursor2: buffer2_start,
            sum0: 0,
            sum1: 0,
            sum2: 0,
        }
    }

    /// `finalScale`: `(weight * sum + 2^31) >> 32`, truncated to 8 bits.
    // Port of: src/core/SkBlurEngine.cpp#L1130-L1132 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // SkTo<uint8_t>: a truncating conversion
    fn final_scale(&self, sum: u32) -> u8 {
        let half: u64 = 1 << 31;
        (self.weight.wrapping_mul(u64::from(sum)).wrapping_add(half) >> 32) as u8
    }

    /// `processValue`.
    // Port of: src/core/SkBlurEngine.cpp#L1074-L1098 (chrome/m156)
    fn process_value(&mut self, leading_edge: u32) -> u8 {
        self.sum0 = self.sum0.wrapping_add(leading_edge);
        self.sum1 = self.sum1.wrapping_add(self.sum0);
        self.sum2 = self.sum2.wrapping_add(self.sum1);
        let blurred = self.final_scale(self.sum2);

        let c2 = self.cursor2;
        self.sum2 = self.sum2.wrapping_sub(self.buffer[c2]);
        self.buffer[c2] = self.sum1;
        self.cursor2 = if c2 + 1 < self.buffer2_end {
            c2 + 1
        } else {
            self.buffer2_start
        };

        let c1 = self.cursor1;
        self.sum1 = self.sum1.wrapping_sub(self.buffer[c1]);
        self.buffer[c1] = self.sum0;
        self.cursor1 = if c1 + 1 < self.buffer1_end {
            c1 + 1
        } else {
            self.buffer1_start
        };

        let c0 = self.cursor0;
        self.sum0 = self.sum0.wrapping_sub(self.buffer[c0]);
        self.buffer[c0] = leading_edge;
        self.cursor0 = if c0 + 1 < self.buffer0_end { c0 + 1 } else { 0 };

        blurred
    }
}

impl Pass<1> for A8Pass {
    fn border(&self) -> i32 {
        self.border
    }

    // Port of: src/core/SkBlurEngine.cpp#L1009-L1017 (chrome/m156)
    fn start_blur(&mut self) {
        self.sum0 = 0;
        self.sum1 = 0;
        self.sum2 = 0;
        self.buffer.fill(0);
        self.cursor0 = 0;
        self.cursor1 = self.buffer1_start;
        self.cursor2 = self.buffer2_start;
    }

    // Port of: src/core/SkBlurEngine.cpp#L1019-L1063 (chrome/m156)
    fn blur_segment(
        &mut self,
        n: usize,
        src: Option<(&[[u8; 1]], usize)>,
        mut dst: Option<(&mut [[u8; 1]], usize)>,
    ) {
        if n == 0 {
            return;
        }
        let mut src_idx = 0usize;
        let mut dst_idx = 0usize;
        for _ in 0..n {
            let leading = match src {
                Some((s, _)) => u32::from(s.get(src_idx).map_or(0, |p| p[0])),
                None => 0,
            };
            let blurred = self.process_value(leading);
            if let Some((d, stride)) = dst.as_mut() {
                if let Some(slot) = d.get_mut(dst_idx) {
                    *slot = [blurred];
                }
                dst_idx += *stride;
            }
            if let Some((_, stride)) = src {
                src_idx += stride;
            }
        }
    }
}

/// `Compute1DBlurKernel`: the normalized 1D Gaussian kernel of `radius` (`2 * radius + 1` taps).
// Port of: src/core/SkBlurEngine.cpp#L1324-L1371 (chrome/m156), with radius.height == 0
// skia-rust: libm -- `std::exp` is the platform's expf, as in Skia.
#[allow(clippy::cast_precision_loss)] // radii are small: exact in f32
fn compute_1d_blur_kernel(sigma: f32, radius: i32, kernel: &mut [f32]) {
    let width = to_usize(2 * radius + 1);
    // And the definition of an identity blur should be sufficient that 2sigma^2 isn't near zero
    // when there's a non-trivial radius.
    let two_sigma_sqrd_x = 2.0f32 * sigma * sigma;
    // Setting the denominator to 1 when the radius is 0 automatically converts the remaining math
    // to the 1D Gaussian distribution.
    let sigma_x_denom = if radius > 0 {
        1.0f32 / two_sigma_sqrd_x
    } else {
        1.0f32
    };
    let mut sum = 0.0f32;
    for (x, weight) in kernel.iter_mut().enumerate().take(width) {
        // static_cast<float>(x - radius)
        let mut x_term = count_i32(x) as f32 - radius as f32;
        x_term = x_term * x_term * sigma_x_denom;
        // The height is 1 (radius 0), so yTerm is 0.
        let term = skia_rust_core::libm::expf(-x_term);
        // Note that the constant term (1/(sqrt(2*pi*sigma^2)) of the Gaussian is dropped here,
        // since we renormalize the kernel below.
        *weight = term;
        sum += term;
    }
    // Normalize the kernel
    let scale = 1.0f32 / sum;
    for weight in kernel.iter_mut().take(width) {
        *weight *= scale;
    }
}

/// A `PassMaker`: the window and sigma of one pass, and the pass it makes
/// (`GaussianPass`, `ThreeBoxApproxPass`, `TentPass` or `A8Pass`).
// Port of: src/core/SkBlurEngine.cpp#L141-L156 (chrome/m156)
#[derive(Clone, Copy, Debug)]
enum PassMaker {
    Gaussian { window: i32, sigma: f32 },
    ThreeBox { window: i32, sigma: f32 },
    Tent { window: i32, sigma: f32 },
    A8 { window: i32, sigma: f32 },
}

impl PassMaker {
    fn window(self) -> i32 {
        match self {
            PassMaker::Gaussian { window, .. }
            | PassMaker::ThreeBox { window, .. }
            | PassMaker::Tent { window, .. }
            | PassMaker::A8 { window, .. } => window,
        }
    }

    fn sigma(self) -> f32 {
        match self {
            PassMaker::Gaussian { sigma, .. }
            | PassMaker::ThreeBox { sigma, .. }
            | PassMaker::Tent { sigma, .. }
            | PassMaker::A8 { sigma, .. } => sigma,
        }
    }

    /// The pass for 8-bit values (`GaussianPass<uint8_t>` and `A8Pass`).
    fn make_u8(self) -> Option<Box<dyn Pass<1>>> {
        match self {
            PassMaker::Gaussian { sigma, .. } => Some(Box::new(GaussianPass::<1>::new(sigma))),
            PassMaker::A8 { window, .. } => Some(Box::new(A8Pass::new(window))),
            PassMaker::ThreeBox { .. } | PassMaker::Tent { .. } => None,
        }
    }

    /// The pass for 32-bit pixels (`GaussianPass<uint32_t>`, `ThreeBoxApproxPass`, `TentPass`).
    fn make_u32(self) -> Option<Box<dyn Pass<4>>> {
        match self {
            PassMaker::Gaussian { sigma, .. } => Some(Box::new(GaussianPass::<4>::new(sigma))),
            PassMaker::ThreeBox { window, .. } => Some(Box::new(ThreeBoxApproxPass::new(window))),
            PassMaker::Tent { window, .. } => Some(Box::new(TentPass::new(window))),
            PassMaker::A8 { .. } => None,
        }
    }
}

/// `GaussianPass::MakeMaker` (sigma below 2) for `N`-byte values.
// Port of: src/core/SkBlurEngine.cpp#L290-L305 (chrome/m156)
fn gaussian_maker(sigma: f32) -> Option<PassMaker> {
    if sigma >= 2.0 {
        return None;
    }
    Some(PassMaker::Gaussian {
        window: 2 * sigma_to_radius(sigma) + 1,
        sigma,
    })
}

/// The `makeMaker` of `RasterA8BlurAlgorithm`: Gaussian for small sigmas, else the A8 box blur.
// Port of: src/core/SkBlurEngine.cpp#L1182-L1189 (chrome/m156)
fn a8_maker(sigma: f32) -> PassMaker {
    if let Some(maker) = gaussian_maker(sigma) {
        return maker;
    }
    PassMaker::A8 {
        window: A8Pass::window_for(sigma),
        sigma,
    }
}

/// The `makeMaker` of `Raster8888BlurAlgorithm`: Gaussian, then three box blurs, then the tent
/// filter for the largest sigmas.
// Port of: src/core/SkBlurEngine.cpp#L1229-L1245 (chrome/m156)
fn rgba8_maker(sigma: f32) -> Option<PassMaker> {
    if let Some(maker) = gaussian_maker(sigma) {
        return Some(maker);
    }
    if let Some(window) = ThreeBoxApproxPass::max_window_ok(sigma) {
        return Some(PassMaker::ThreeBox { window, sigma });
    }
    // TentPass::MakeMaker
    let gaussian_window = box_blur_window(sigma);
    let tent_window = 3 * gaussian_window / 2;
    if tent_window >= 4104 {
        return None;
    }
    Some(PassMaker::Tent {
        window: tent_window,
        sigma,
    })
}

/// `eval_blur_passes`: the X pass from `src` into `dst` (with the rows the Y pass needs), then
/// the Y pass in place. `src` is a `src_width`-wide image of `N`-byte pixels. Returns the
/// destination pixels, their size, and the destination bounds relative to them.
// Port of: src/core/SkBlurEngine.cpp#L159-L255 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors eval_blur_passes
fn eval_blur_passes<const N: usize>(
    maker_x: PassMaker,
    maker_y: PassMaker,
    make: fn(PassMaker) -> Option<Box<dyn Pass<N>>>,
    src: Vec<[u8; N]>,
    src_width: usize,
    original_src_bounds: IRect,
    original_dst_bounds: IRect,
) -> Option<(Vec<[u8; N]>, i32, i32, IRect)> {
    let mut src_bounds = original_src_bounds;
    let mut dst_bounds = original_dst_bounds;
    if maker_x.window() > 1 {
        // Inflate the dst by the window required for the Y pass so that the X pass can prepare
        // it. The Y pass will be offset to only write to the original rows in dstBounds.
        dst_bounds.outset((0, sigma_to_radius(maker_y.sigma())));
    }

    let dst_origin = dst_bounds.top_left();
    let dst_w = dst_bounds.width();
    let dst_h = dst_bounds.height();
    if dst_w <= 0 || dst_h <= 0 {
        return None;
    }
    let dst_stride = to_usize(dst_w);
    let mut dst = vec![[0u8; N]; dst_stride * to_usize(dst_h)];

    let mut src_buf = src;
    let mut src_stride = src_width;

    // Initialize these assuming the Y-only case
    let mut loop_start = src_bounds.left.max(dst_bounds.left);
    let mut loop_end = src_bounds.right.min(dst_bounds.right);
    let mut dst_y_offset = 0;

    if maker_x.window() > 1 {
        // First an X-only blur from src into dst, including the extra rows that will become
        // input for the second Y pass, which will then be performed in place.
        loop_start = src_bounds.top.max(dst_bounds.top);
        loop_end = src_bounds.bottom.min(dst_bounds.bottom);

        if loop_start < loop_end {
            let mut pass = make(maker_x)?;
            for y in loop_start..loop_end {
                let src_row = to_usize(y - src_bounds.top) * src_stride;
                let dst_row = to_usize(y - dst_bounds.top) * dst_stride;
                pass_blur(
                    pass.as_mut(),
                    src_bounds.left - dst_bounds.left,
                    src_bounds.right - dst_bounds.left,
                    dst_bounds.width(),
                    &src_buf[src_row..],
                    1,
                    &mut dst[dst_row..],
                    1,
                );
            }
        }

        // Set up the Y pass to blur from the full dst into the non-outset portion of dst. The
        // Y pass reads each value before it writes the value that replaces it, so reading from a
        // copy of dst gives the same result as Skia's in-place blur.
        src_buf.clone_from(&dst);
        src_stride = dst_stride;
        loop_start = original_dst_bounds.left;
        loop_end = original_dst_bounds.right;
        // The new 'dst' is equal to dst.extractSubset(originalDstBounds.offset(-dstOrigin)), but
        // by construction only the Y offset has an interesting value so this is a little more
        // efficient.
        dst_y_offset = original_dst_bounds.top - dst_bounds.top;

        src_bounds = dst_bounds;
        dst_bounds = original_dst_bounds;
    }

    // Iterate over each column to calculate 1D blur along Y. This is either blurring from src
    // into dst for a 1D blur; or it's blurring from dst into dst for the second pass of a 2D blur.
    if maker_y.window() > 1 && loop_start < loop_end {
        let mut pass = make(maker_y)?;
        for x in loop_start..loop_end {
            let src_col = to_usize(x - src_bounds.left);
            let dst_col = to_usize(x - dst_bounds.left) + to_usize(dst_y_offset) * dst_stride;
            pass_blur(
                pass.as_mut(),
                src_bounds.top - dst_bounds.top,
                src_bounds.bottom - dst_bounds.top,
                dst_bounds.height(),
                &src_buf[src_col..],
                src_stride,
                &mut dst[dst_col..],
                dst_stride,
            );
        }
    }

    // dstBounds = originalDstBounds.makeOffset(-dstOrigin): relative to dst's pixels.
    let relative = original_dst_bounds.with_offset((-dst_origin.x, -dst_origin.y));
    Some((dst, dst_w, dst_h, relative))
}

/// Reads the `N`-byte pixels of `bm`, row by row, into a `width`-wide buffer.
fn read_pixels<const N: usize>(bm: &Bitmap) -> Option<Vec<[u8; N]>> {
    let pixmap = bm.peek_pixels()?;
    let bytes = pixmap.addr()?;
    let row_bytes = pixmap.row_bytes();
    let width = to_usize(bm.width());
    let height = to_usize(bm.height());
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let off = y * row_bytes + x * N;
            let mut px = [0u8; N];
            px.copy_from_slice(bytes.get(off..off + N)?);
            pixels.push(px);
        }
    }
    Some(pixels)
}

/// Makes the raster bitmap of `pixels` (`width` x `height`, `N`-byte pixels of `bm`'s format) and
/// returns it as a special image of `bounds` (`SkSpecialImages::MakeFromRaster`).
fn make_special<const N: usize>(
    bm: &Bitmap,
    pixels: &[[u8; N]],
    width: i32,
    height: i32,
    bounds: &IRect,
) -> Option<SpecialImage> {
    let info = ImageInfo::new(
        ISize::new(width, height),
        bm.color_type(),
        bm.alpha_type(),
        bm.color_space(),
    );
    let bytes: Vec<u8> = pixels.iter().flat_map(|px| px.iter().copied()).collect();
    let mut dst = Bitmap::new();
    if !dst.install_pixels(&info, bytes, to_usize(width) * N) {
        return None;
    }
    SpecialImage::make_from_raster(bounds, &dst, &SurfaceProps::default())
}

/// `RasterA8BlurAlgorithm`.
// Port of: src/core/SkBlurEngine.cpp#L1154-L1203 (chrome/m156)
#[derive(Debug, Default)]
pub struct RasterA8BlurAlgorithm;

impl BlurAlgorithm for RasterA8BlurAlgorithm {
    // See analysis in description of GaussPass for the max supported sigma.
    fn max_sigma(&self) -> f32 {
        135.0
    }

    // The tiling is applied via the CropImageFilter and carried as metadata on the FilterResult.
    fn supports_only_decal_tiling(&self) -> bool {
        true
    }

    fn blur(
        &self,
        sigma: Size,
        input: &SpecialImage,
        original_src_bounds: IRect,
        tile_mode: TileMode,
        original_dst_bounds: IRect,
    ) -> Option<SpecialImage> {
        if tile_mode != TileMode::Decal {
            return None;
        }
        let bm = input.as_bitmap()?;
        // The blur engine should not have picked this algorithm for a non-8-bit color type.
        if bm.color_type() != ColorType::Alpha8 {
            return None;
        }
        let maker_x = a8_maker(sigma.width);
        let maker_y = a8_maker(sigma.height);
        let pixels = read_pixels::<1>(&bm)?;
        let width = bm.width();
        let (dst, dst_w, dst_h, bounds) = eval_blur_passes::<1>(
            maker_x,
            maker_y,
            PassMaker::make_u8,
            pixels,
            to_usize(width),
            original_src_bounds,
            original_dst_bounds,
        )?;
        make_special(&bm, &dst, dst_w, dst_h, &bounds)
    }
}

/// `Raster8888BlurAlgorithm`.
// Port of: src/core/SkBlurEngine.cpp#L1205-L1249 (chrome/m156)
#[derive(Debug, Default)]
pub struct Raster8888BlurAlgorithm;

impl BlurAlgorithm for Raster8888BlurAlgorithm {
    // See analysis in description of TentPass for the max supported sigma.
    fn max_sigma(&self) -> f32 {
        135.0
    }

    fn supports_only_decal_tiling(&self) -> bool {
        true
    }

    fn blur(
        &self,
        sigma: Size,
        input: &SpecialImage,
        original_src_bounds: IRect,
        tile_mode: TileMode,
        original_dst_bounds: IRect,
    ) -> Option<SpecialImage> {
        if tile_mode != TileMode::Decal {
            return None;
        }
        let bm = input.as_bitmap()?;
        // The blur engine should not have picked this algorithm for a non-32-bit color type.
        if !matches!(bm.color_type(), ColorType::RGBA8888 | ColorType::BGRA8888) {
            return None;
        }
        let maker_x = rgba8_maker(sigma.width)?;
        let maker_y = rgba8_maker(sigma.height)?;
        let pixels = read_pixels::<4>(&bm)?;
        let width = bm.width();
        let (dst, dst_w, dst_h, bounds) = eval_blur_passes::<4>(
            maker_x,
            maker_y,
            PassMaker::make_u32,
            pixels,
            to_usize(width),
            original_src_bounds,
            original_dst_bounds,
        )?;
        make_special(&bm, &dst, dst_w, dst_h, &bounds)
    }
}

/// `RasterBlurEngine`: the CPU blur engine. Non-8-bit color types have no algorithm here (their
/// shader-based blur needs `SkSL`, which is not ported).
// Port of: src/core/SkBlurEngine.cpp#L1284-L1314 (chrome/m156)
#[derive(Debug, Default)]
pub struct RasterBlurEngine {
    a8: RasterA8BlurAlgorithm,
    rgba8: Raster8888BlurAlgorithm,
}

impl BlurEngine for RasterBlurEngine {
    // Port of: src/core/SkBlurEngine.cpp#L1285-L1302 (chrome/m156)
    fn find_algorithm(&self, _sigma: Size, color_type: ColorType) -> Option<&dyn BlurAlgorithm> {
        match color_type {
            ColorType::Alpha8 => Some(&self.a8),
            ColorType::RGBA8888 | ColorType::BGRA8888 => Some(&self.rgba8),
            _ => None,
        }
    }
}

/// The raster blur engine (`SkBlurEngine::GetRasterBlurEngine`).
// Port of: src/core/SkBlurEngine.cpp#L1316-L1321 (chrome/m156)
#[must_use]
pub fn raster_blur_engine() -> &'static dyn BlurEngine {
    static ENGINE: RasterBlurEngine = RasterBlurEngine {
        a8: RasterA8BlurAlgorithm,
        rgba8: Raster8888BlurAlgorithm,
    };
    &ENGINE
}

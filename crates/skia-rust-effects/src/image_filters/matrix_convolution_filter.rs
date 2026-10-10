// Copyright 2012 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkMatrixConvolutionImageFilter.{h,cpp}

//! `SkMatrixConvolutionImageFilter`: convolves its input with a kernel applied to layer-space
//! pixels. Small kernels are uploaded as uniforms; larger ones are stored in an A8 bitmap that the
//! texture-based known effects sample.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{Context, Mapping, irect_intersect_in_place};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{IRect, Rect, rect_priv};
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeShaderBuilder};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, scalar_round_to_int};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::crop_filter::crop;

/// `kLargeKernelSize`: the largest kernel size, in values. `SkSL` balks at 2048 values, so kernels
/// are capped well below that.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.h#L20-L21 (chrome/m156)
const K_LARGE_KERNEL_SIZE: i32 = 256;

/// `kSmallKernelSize`: the size of the small texture-based kernel variant.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.h#L26 (chrome/m156)
const K_SMALL_KERNEL_SIZE: i32 = 64;

/// `kMaxUniformKernelSize`: the number of values the uniform-based kernel shader stores.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.h#L31 (chrome/m156)
const K_MAX_UNIFORM_KERNEL_SIZE: i32 = 28;

/// `quantize_by_kernel_size(kernelSize)`: the padded kernel size and the known effect that
/// convolves a kernel of `kernel_size` values.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L79-L87 (chrome/m156)
fn quantize_by_kernel_size(kernel_size: i32) -> (i32, StableKey) {
    if kernel_size < K_MAX_UNIFORM_KERNEL_SIZE {
        (K_MAX_UNIFORM_KERNEL_SIZE, StableKey::MatrixConvUniforms)
    } else if kernel_size <= K_SMALL_KERNEL_SIZE {
        (K_SMALL_KERNEL_SIZE, StableKey::MatrixConvTexSm)
    } else {
        (K_LARGE_KERNEL_SIZE, StableKey::MatrixConvTexLg)
    }
}

/// `create_kernel_bitmap(kernelSize, kernel, innerGain, innerBias)`: encodes a large kernel as an
/// A8 image. Returns an empty bitmap for the uniform-based variant, which needs none.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L89-L140 (chrome/m156)
fn create_kernel_bitmap(
    kernel_size: ISize,
    kernel: &[f32],
    inner_gain: &mut f32,
    inner_bias: &mut f32,
) -> Bitmap {
    let length = kernel_size.width * kernel_size.height;
    let (quantized_kernel_size, key) = quantize_by_kernel_size(length);
    if key == StableKey::MatrixConvUniforms {
        // No bitmap is needed to store the kernel on the GPU
        *inner_gain = 1.0;
        *inner_bias = 0.0;
        return Bitmap::default();
    }
    let length = usize::try_from(length).expect("a positive kernel length");

    // The convolution kernel is "big". The SVG spec has no upper limit on what's supported so
    // store the kernel in a SkBitmap that will be uploaded to a data texture.
    //
    // We store the data in A8 for universal support, but this requires normalizing the values
    // and adding an extra inner bias operation to the shader.
    let mut min = kernel[0];
    let mut max = kernel[0];
    for &value in &kernel[1..length] {
        if value < min {
            min = value;
        }
        if value > max {
            max = value;
        }
    }
    *inner_gain = max - min;
    *inner_bias = min;

    // Treat a near-0 gain (i.e. box blur) as 1 and let innerBias move everything to final value.
    if inner_gain.abs() <= SCALAR_NEARLY_ZERO {
        *inner_gain = 1.0;
    }

    let mut kernel_bm = Bitmap::new();
    if !kernel_bm.try_alloc_pixels_info(
        &ImageInfo::new(
            (quantized_kernel_size, 1),
            ColorType::Alpha8,
            AlphaType::Premul,
            None,
        ),
        None,
    ) {
        // OOM so return an empty bitmap, which will be detected later on in onFilterImage().
        return Bitmap::default();
    }
    for (i, &value) in (0_i32..).zip(kernel[..length].iter()) {
        // `*kernelBM.getAddr8(i, 0) = SkScalarRoundToInt(255 * (kernel[i] - min) / *innerGain)`:
        // the C++ int is narrowed to a byte, which `as u8` does too.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ narrows to u8
        let byte = scalar_round_to_int(255.0 * (value - min) / *inner_gain) as u8;
        kernel_bm.set_addr8(i, 0, byte);
    }
    for i in i32::try_from(length).expect("a kernel length fits in i32")..quantized_kernel_size {
        kernel_bm.set_addr8(i, 0, 0);
    }
    kernel_bm.set_immutable();
    kernel_bm
}

/// `adjust(rect, dl, dt, dr, db)`: an `SkIRect::adjust` of `rect`.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L65-L72 (chrome/m156)
fn adjust(rect: &IRect, dl: i32, dt: i32, dr: i32, db: i32) -> IRect {
    let mut adjusted = *rect;
    adjusted.adjust(dl, dt, dr, db);
    adjusted
}

/// The matrix convolution image filter (`SkMatrixConvolutionImageFilter`).
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L142-L178 (chrome/m156)
#[doc(alias = "SkMatrixConvolutionImageFilter")]
#[derive(Debug)]
pub struct MatrixConvolutionImageFilter {
    common: ImageFilterCommon,
    /// Original kernel data, preserved for serialization even if it was encoded into the bitmap.
    kernel: Vec<f32>,
    /// `fKernelSize`: the kernel size, in layer space.
    kernel_size: ISize,
    /// `fKernelOffset`: the kernel offset, in layer space.
    kernel_offset: IPoint,
    gain: f32,
    /// NOTE: This is assumed to be in [0-255] for historical reasons.
    bias: f32,
    convolve_alpha: bool,
    /// Derived from `kernel` when larger than what is uploaded as uniforms (`fKernelBitmap`).
    kernel_bitmap: Bitmap,
    /// `fInnerBias`: reconstructs the coefficient from unorm8 data as `(a + innerBias) * innerGain`.
    inner_bias: f32,
    /// `fInnerGain`.
    inner_gain: f32,
}

impl MatrixConvolutionImageFilter {
    /// `SkMatrixConvolutionImageFilter(kernelSize, kernel, gain, bias, kernelOffset,
    /// convolveAlpha, input)`.
    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L142-L161 (chrome/m156)
    #[must_use]
    fn new(
        kernel_size: ISize,
        kernel: &[f32],
        gain: f32,
        bias: f32,
        kernel_offset: IPoint,
        convolve_alpha: bool,
        input: Option<ImageFilter>,
    ) -> Self {
        let mut inner_gain = 0.0;
        let mut inner_bias = 0.0;
        // Does nothing for small kernels, otherwise encodes kernel into an A8 image.
        let kernel_bitmap =
            create_kernel_bitmap(kernel_size, kernel, &mut inner_gain, &mut inner_bias);
        MatrixConvolutionImageFilter {
            common: ImageFilterCommon::new(vec![input], None),
            kernel: kernel.to_vec(),
            kernel_size,
            kernel_offset,
            gain,
            bias,
            convolve_alpha,
            kernel_bitmap,
            inner_bias,
            inner_gain,
        }
    }

    /// `boundsSampledByKernel(bounds)`: the bounds the kernel samples to cover `bounds`.
    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L378-L384 (chrome/m156)
    fn bounds_sampled_by_kernel(&self, bounds: IRect) -> IRect {
        adjust(
            &bounds,
            -self.kernel_offset.x,
            -self.kernel_offset.y,
            self.kernel_size.width - self.kernel_offset.x - 1,
            self.kernel_size.height - self.kernel_offset.y - 1,
        )
    }

    /// `boundsAffectedByKernel(bounds)`: the bounds the kernel can produce values for from `bounds`.
    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L386-L392 (chrome/m156)
    fn bounds_affected_by_kernel(&self, bounds: IRect) -> IRect {
        adjust(
            &bounds,
            self.kernel_offset.x - self.kernel_size.width + 1,
            self.kernel_offset.y - self.kernel_size.height + 1,
            self.kernel_offset.x,
            self.kernel_offset.y,
        )
    }

    /// `createShader(ctx, input)`: the convolution shader over `input`.
    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L394-L428 (chrome/m156)
    fn create_shader(&self, ctx: &Context<'_>, input: Option<Shader>) -> Option<Shader> {
        let kernel_length = self.kernel_size.width * self.kernel_size.height;
        let (_, key) = quantize_by_kernel_size(kernel_length);
        let use_texture_shader = key != StableKey::MatrixConvUniforms;
        if use_texture_shader && self.kernel_bitmap.is_empty() {
            return None; // No actual kernel data to work with from a prior OOM
        }

        let effect = get_known_runtime_effect(key)?;
        let mut builder = RuntimeShaderBuilder::new(effect.clone());
        builder
            .child("child")
            .assign(input.map_or(ChildPtr::Empty, ChildPtr::Shader));
        if use_texture_shader {
            let cached_kernel = ctx.backend().get_cached_bitmap(&self.kernel_bitmap)?;
            let kernel_shader = cached_kernel.to_raw_shader(
                None::<(TileMode, TileMode)>,
                SamplingOptions::from(FilterMode::Nearest),
                None::<&Matrix>,
            )?;
            builder
                .child("kernel")
                .assign(ChildPtr::Shader(kernel_shader));
            builder
                .uniform("innerGainAndBias")
                .set_f32(&[self.inner_gain, self.inner_bias]);
        } else {
            // The uniform array is padded with zeros to its full size.
            let mut padded_kernel = [0.0_f32; 28];
            assert!(
                kernel_length <= K_MAX_UNIFORM_KERNEL_SIZE,
                "the uniform kernel holds at most {K_MAX_UNIFORM_KERNEL_SIZE} values"
            );
            padded_kernel[..self.kernel.len()].copy_from_slice(&self.kernel);
            builder.uniform("kernel").set_f32(&padded_kernel);
        }
        builder
            .uniform("size")
            .set_i32(&[self.kernel_size.width, self.kernel_size.height]);
        builder
            .uniform("offset")
            .set_i32(&[self.kernel_offset.x, self.kernel_offset.y]);
        // Scale the user-provided bias by 1/255 to match the [0,1] color channel range
        builder
            .uniform("gainAndBias")
            .set_f32(&[self.gain, self.bias / 255.0]);
        builder
            .uniform("convolveAlpha")
            .set_i32(&[i32::from(self.convolve_alpha)]);
        builder.make_shader(None::<&Matrix>)
    }
}

impl ImageFilterBase for MatrixConvolutionImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L110-L123 (chrome/m156)
    //
    // affectsTransparentBlack() is conflated with "canComputeFastBounds" and MatrixConvolution is
    // unique in that it might not produce unbounded output, but we can't calculate the fast bounds
    // because the kernel is applied in device space and no transform is provided with that API.
    fn on_affects_transparent_black(&self) -> bool {
        true
    }

    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L430-L461 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors `fConvolveAlpha && fBias != 0.f`
    fn on_filter_image(&self, context: &Context<'_>) -> FilterResult {
        let required_input = self.bounds_sampled_by_kernel(context.desired_output());
        let child_output =
            self.get_child_output(0, &context.with_new_desired_output(required_input));

        let output_bounds = if self.convolve_alpha && self.bias != 0.0 {
            // The convolution will produce a non-trivial value for every pixel so fill desired
            // output.
            context.desired_output()
        } else {
            // Calculate the possible extent of the convolution given what was actually produced by
            // the child filter and then intersect that with the desired output.
            let mut bounds = self.bounds_affected_by_kernel(child_output.layer_bounds());
            if !irect_intersect_in_place(&mut bounds, &context.desired_output()) {
                return FilterResult::default();
            }
            bounds
        };

        let mut builder = Builder::new(context);
        builder.add(
            child_output,
            Some(self.bounds_sampled_by_kernel(output_bounds)),
            ShaderFlags::SAMPLED_REPEATEDLY,
            default_sampling(),
        );
        builder.eval(
            |inputs| self.create_shader(context, inputs[0].clone()),
            Some(output_bounds),
            false,
        )
    }

    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L463-L472 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        // Adjust the desired output bounds by the kernel size to avoid evaluating edge conditions,
        // and then recurse to the child filter.
        let required_input = self.bounds_sampled_by_kernel(desired_output);
        self.get_child_input_layer_bounds(0, mapping, required_input, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L474-L494 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors `fConvolveAlpha && fBias != 0.f`
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        if self.convolve_alpha && self.bias != 0.0 {
            // Applying the kernel as a convolution to fully transparent black will result in 0 for
            // each channel, unless the bias itself shifts this "zero-point".
            return None;
        }
        // Otherwise apply the kernel to the output bounds of the child filter.
        self.get_child_output_layer_bounds(0, mapping, content_bounds)
            .map(|bounds| self.bounds_affected_by_kernel(bounds))
    }

    // Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L496-L502 (chrome/m156)
    fn compute_fast_bounds(&self, _bounds: &Rect) -> Rect {
        // See onAffectsTransparentBlack(), but without knowing the local-to-device transform, we
        // don't know how many pixels will be sampled by the kernel. Return unbounded to match the
        // expectations of an image filter that "affects" transparent black.
        rect_priv::make_large_s32()
    }
}

/// `SkImageFilters::MatrixConvolution(kernelSize, kernel, gain, bias, kernelOffset, tileMode,
/// convolveAlpha, input, cropRect)`. The kernel must hold `kernelSize.width * kernelSize.height`
/// values (skia-safe asserts this too).
///
/// # Panics
/// If `kernel` does not hold `kernel_size.width * kernel_size.height` values.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.cpp#L313-L354 (chrome/m156)
#[doc(alias = "MatrixConvolution")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors the C++ factory's parameter list
pub fn matrix_convolution(
    kernel_size: impl Into<ISize>,
    kernel: &[f32],
    gain: f32,
    bias: f32,
    kernel_offset: impl Into<IPoint>,
    tile_mode: TileMode,
    convolve_alpha: bool,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    let kernel_size = kernel_size.into();
    let kernel_offset = kernel_offset.into();
    if kernel_size.width < 1 || kernel_size.height < 1 {
        return None;
    }
    if i64::from(kernel_size.width) * i64::from(kernel_size.height) > i64::from(K_LARGE_KERNEL_SIZE)
    {
        return None;
    }
    assert_eq!(
        usize::try_from(kernel_size.width * kernel_size.height).expect("a positive size"),
        kernel.len(),
        "the kernel must hold width * height values"
    );
    if kernel_offset.x < 0
        || kernel_offset.x >= kernel_size.width
        || kernel_offset.y < 0
        || kernel_offset.y >= kernel_size.height
    {
        return None;
    }

    // The 'tileMode' behavior is not well-defined if there is no crop, so we only apply it if
    // there is a provided 'cropRect'.
    let mut filter = input;
    if let Some(rect) = crop_rect
        && tile_mode != TileMode::Decal
    {
        // Historically the input image was restricted to the cropRect when tiling was not kDecal
        // so that the kernel evaluated the tiled edge conditions, while a kDecal crop only affected
        // the output.
        filter = crop(&rect, tile_mode, filter);
    }
    filter = Some(ImageFilter::from_base(MatrixConvolutionImageFilter::new(
        kernel_size,
        kernel,
        gain,
        bias,
        kernel_offset,
        convolve_alpha,
        filter,
    )));
    if let Some(rect) = crop_rect {
        // But regardless of the tileMode, the output is decal cropped.
        filter = crop(&rect, TileMode::Decal, filter);
    }
    filter
}

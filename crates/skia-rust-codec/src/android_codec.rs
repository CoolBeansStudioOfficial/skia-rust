// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkAndroidCodec.cpp (the output rules, the constructors and the sampling and
// subset API), and include/codec/SkAndroidCodec.h (the same methods) (chrome/m156)
// Port of: src/codec/SkCodecColorProfile.cpp#L23-L51 (cicp_get_android_sk_color_space) and
// #L146-L168 (ColorProfile::getAndroidOutputColorSpace), the parts the output colour space uses.
// Ported from: src/codec/SkAndroidCodec.cpp, include/codec/SkAndroidCodec.h,
// src/codec/SkCodecColorProfile.cpp
//
// The sampled path lives in `sampled_codec` (SkSampledCodec) and `android_codec_adapter`
// (SkAndroidCodecAdapter). The gainmap accessors are not ported.

//! The Android codec: a codec with the output colour type, alpha type and colour space rules that
//! Android's image decoder uses.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::{ColorSpace, named_primaries, named_transfer_fn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::Stream;
use skia_rust_skcms::{IccProfile, TransferFunction};

use crate::android_codec_adapter;
use crate::codec::{Codec, Options, Result};
use crate::codec_priv::{get_sampled_dimension, is_valid_subset};
use crate::codecs;
use crate::sampled_codec;

/// The options of an Android decode. Port of `SkAndroidCodec::AndroidOptions`, which extends
/// `SkCodec::Options` with a sample size.
#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(alias = "SkAndroidCodec::AndroidOptions")]
pub struct AndroidOptions {
    /// The `SkCodec::Options` base: the subset, zero-initialization and frame index.
    pub base: Options,
    /// Port of `fSampleSize`: the integer downscale factor. The default of 1 is no downscaling.
    pub sample_size: i32,
}

impl Default for AndroidOptions {
    // Port of: include/codec/SkAndroidCodec.h#L206-L210 (the AndroidOptions constructor)
    fn default() -> Self {
        Self {
            base: Options::default(),
            sample_size: 1,
        }
    }
}

// Which `SkAndroidCodec` subclass the codec is: the sampled one, or the adapter for codecs that
// scale internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    // Port of `SkSampledCodec`.
    Sampled,
    // Port of `SkAndroidCodecAdapter`.
    Adapter,
}

/// A codec with Android's output rules. Port of `SkAndroidCodec`.
#[derive(Debug)]
#[doc(alias = "SkAndroidCodec")]
pub struct AndroidCodec<'a> {
    // Port of `fInfo`: the codec's natural info, taken when the codec was made.
    info: ImageInfo,
    // Port of `fCodec`.
    pub(crate) codec: Codec<'a>,
    // The subclass, which decides how sampling and subsetting are done.
    kind: Kind,
}

// Port of `is_valid_sample_size` in SkAndroidCodec.cpp. Skia's FIXME notes there is no maximum.
// Port of: src/codec/SkAndroidCodec.cpp#L23-L26 (chrome/m156)
fn is_valid_sample_size(sample_size: i32) -> bool {
    sample_size > 0
}

// Port of `smaller_than` in SkAndroidCodec.cpp: either dimension of `a` is below that of `b`.
// Port of: src/codec/SkAndroidCodec.cpp#L244-L247 (chrome/m156)
fn smaller_than(a: ISize, b: ISize) -> bool {
    a.width < b.width || a.height < b.height
}

// Port of `strictly_bigger_than` in SkAndroidCodec.cpp: both dimensions of `a` are above `b`'s.
// Port of: src/codec/SkAndroidCodec.cpp#L249-L252 (chrome/m156)
fn strictly_bigger_than(a: ISize, b: ISize) -> bool {
    a.width > b.width && a.height > b.height
}

impl<'a> AndroidCodec<'a> {
    /// Port of `SkAndroidCodec::MakeFromCodec`: wraps a codec for the formats Android decodes. The
    /// formats Android does not support (PKM, KTX, ASTC, JPEG XL) return `None`.
    // Port of: src/codec/SkAndroidCodec.cpp#L43-L72 (chrome/m156)
    #[doc(alias = "SkAndroidCodec::MakeFromCodec")]
    #[must_use]
    pub fn make_from_codec(codec: Codec<'a>) -> Option<Self> {
        let kind = match codec.encoded_format() {
            EncodedImageFormat::PNG
            | EncodedImageFormat::ICO
            | EncodedImageFormat::JPEG
            | EncodedImageFormat::BMP
            | EncodedImageFormat::WBMP => Kind::Sampled,
            EncodedImageFormat::GIF
            | EncodedImageFormat::WEBP
            | EncodedImageFormat::DNG
            // On the Android framework, both HEIF and AVIF are handled by SkCrabbyAvifCodec, which
            // scales internally.
            | EncodedImageFormat::AVIF
            | EncodedImageFormat::HEIF => Kind::Adapter,
            EncodedImageFormat::PKM
            | EncodedImageFormat::KTX
            | EncodedImageFormat::ASTC
            | EncodedImageFormat::JPEGXL => return None,
        };
        Some(Self {
            info: codec.info(),
            codec,
            kind,
        })
    }

    /// Port of `SkAndroidCodec::MakeFromStream` (with no PNG chunk reader): makes the codec with the
    /// registered decoders, then wraps it.
    // Port of: src/codec/SkAndroidCodec.cpp#L35-L39 (chrome/m156)
    #[doc(alias = "SkAndroidCodec::MakeFromStream")]
    #[must_use]
    pub fn make_from_stream(stream: Box<dyn Stream + Send + 'a>) -> Option<Self> {
        let codec = codecs::make_codec_from_stream(stream).ok()?;
        Self::make_from_codec(codec)
    }

    /// Port of `SkAndroidCodec::getInfo`: the codec's natural info.
    #[must_use]
    #[doc(alias = "getInfo")]
    pub fn info(&self) -> ImageInfo {
        self.info.clone()
    }

    /// The codec this wraps. Port of `SkAndroidCodec::codec`.
    #[must_use]
    pub fn codec(&self) -> &Codec<'a> {
        &self.codec
    }

    /// Port of `SkAndroidCodec::getSampledDimensions`: the size of the output for a sample size.
    /// The codec may round up or down to the size it decodes most efficiently. Never zero: a sample
    /// size larger than a dimension gives one.
    // Port of: src/codec/SkAndroidCodec.cpp#L195-L205 (chrome/m156)
    #[must_use]
    #[doc(alias = "getSampledDimensions")]
    pub fn get_sampled_dimensions(&self, sample_size: i32) -> ISize {
        if !is_valid_sample_size(sample_size) {
            return ISize::new(0, 0);
        }
        // Fast path for when we are not scaling.
        if sample_size == 1 {
            return self.codec.dimensions();
        }
        self.on_get_sampled_dimensions(sample_size)
    }

    /// Port of `SkAndroidCodec::getSupportedSubset`: adjusts `desired_subset` to a subset this codec
    /// decodes. Returns false if the subset is not inside the image, or the codec cannot decode it.
    // Port of: src/codec/SkAndroidCodec.cpp#L213-L219 (chrome/m156)
    #[must_use]
    #[doc(alias = "getSupportedSubset")]
    pub fn get_supported_subset(&self, desired_subset: &mut IRect) -> bool {
        if !is_valid_subset(*desired_subset, self.codec.dimensions()) {
            return false;
        }
        match self.kind {
            Kind::Sampled => true,
            Kind::Adapter => self.codec.get_valid_subset(desired_subset),
        }
    }

    /// Port of `SkAndroidCodec::getSampledSubsetDimensions`: the output size for a sample size and a
    /// subset that `get_supported_subset` accepts unchanged. Zero for any other subset.
    // Port of: src/codec/SkAndroidCodec.cpp#L221-L243 (chrome/m156)
    #[must_use]
    #[doc(alias = "getSampledSubsetDimensions")]
    pub fn get_sampled_subset_dimensions(&self, sample_size: i32, subset: IRect) -> ISize {
        if !is_valid_sample_size(sample_size) {
            return ISize::new(0, 0);
        }
        // The subset must be one that is supported by the codec.
        let mut copy_subset = subset;
        if !self.get_supported_subset(&mut copy_subset) || copy_subset != subset {
            return ISize::new(0, 0);
        }
        // If the subset is the entire image, for consistency, use get_sampled_dimensions().
        if self.codec.dimensions() == subset.size() {
            return self.get_sampled_dimensions(sample_size);
        }
        // Both subclasses want the same implementation here.
        ISize::new(
            get_sampled_dimension(subset.width(), sample_size),
            get_sampled_dimension(subset.height(), sample_size),
        )
    }

    /// Port of `SkAndroidCodec::computeSampleSize`: the sample size that decodes to the size nearest
    /// `desired_size`. On return `desired_size` holds the size that sample size produces. Returns 1,
    /// and the original size, when the image cannot be downscaled to a size of that kind.
    // Port of: src/codec/SkAndroidCodec.cpp#L254-L330 (chrome/m156)
    #[must_use]
    #[doc(alias = "computeSampleSize")]
    pub fn compute_sample_size(&self, desired_size: &mut ISize) -> i32 {
        let orig_dims = self.codec.dimensions();
        if *desired_size == orig_dims {
            return 1;
        }
        if smaller_than(orig_dims, *desired_size) {
            *desired_size = orig_dims;
            return 1;
        }
        // Handle bad input.
        if desired_size.width < 1 || desired_size.height < 1 {
            *desired_size = ISize::new(desired_size.width.max(1), desired_size.height.max(1));
        }
        // Skia returns 1 for a WebP here, and keeps the original size when the WebP is animated
        // (`getFrameCount() > 1`). WebP is not ported, so no codec reaches this branch yet, and the
        // frame-count check lands with it.
        if self.codec.encoded_format() == EncodedImageFormat::WEBP {
            return 1;
        }

        let mut sample_size =
            (orig_dims.width / desired_size.width).min(orig_dims.height / desired_size.height);
        let mut computed_size = self.get_sampled_dimensions(sample_size);
        if computed_size == *desired_size {
            return sample_size;
        }
        if computed_size == orig_dims || sample_size == 1 {
            // Cannot downscale.
            *desired_size = computed_size;
            return 1;
        }

        if strictly_bigger_than(computed_size, *desired_size) {
            // See if there is a tighter fit.
            loop {
                let smaller = self.get_sampled_dimensions(sample_size + 1);
                if smaller == *desired_size {
                    return sample_size + 1;
                }
                if smaller == computed_size || smaller_than(smaller, *desired_size) {
                    // Cannot get any smaller without being smaller than desired.
                    *desired_size = computed_size;
                    return sample_size;
                }
                sample_size += 1;
                computed_size = smaller;
            }
        }

        if !smaller_than(computed_size, *desired_size) {
            // One of the computed dimensions is equal to desired, and the other is bigger. This is
            // as close as we can get.
            *desired_size = computed_size;
            return sample_size;
        }

        // computed_size is too small. Make it larger.
        while sample_size > 2 {
            let bigger = self.get_sampled_dimensions(sample_size - 1);
            if bigger == *desired_size || !smaller_than(bigger, *desired_size) {
                *desired_size = bigger;
                return sample_size - 1;
            }
            sample_size -= 1;
        }

        *desired_size = orig_dims;
        1
    }

    /// Port of `SkAndroidCodec::getAndroidPixels`: decodes the image, or a subset of it, scaled by
    /// `options.sample_size`, into `pixels` (`row_bytes` apart). `None` options decode the whole image
    /// unscaled.
    // Port of: src/codec/SkAndroidCodec.cpp#L332-L382 (chrome/m156), without the frame-callback
    // recursion, which only animated codecs need (none is ported).
    #[doc(alias = "getAndroidPixels")]
    pub fn get_android_pixels(
        &mut self,
        request_info: &ImageInfo,
        pixels: &mut [u8],
        row_bytes: usize,
        options: Option<&AndroidOptions>,
    ) -> Result {
        if pixels.is_empty() {
            return Result::InvalidParameters;
        }
        if row_bytes < request_info.min_row_bytes() {
            return Result::InvalidParameters;
        }

        let mut options = options.cloned().unwrap_or_default();
        if let Some(subset) = options.base.subset {
            if !is_valid_subset(subset, self.codec.dimensions()) {
                return Result::InvalidParameters;
            }
            if IRect::from_size(self.codec.dimensions()) == subset {
                // The caller wants the whole thing, rather than a subset.
                options.base.subset = None;
            }
        }

        let frame_result = self.codec.handle_frame_index(request_info, &options.base);
        if frame_result != Result::Success {
            return frame_result;
        }

        self.on_get_android_pixels(request_info, pixels, row_bytes, &options)
    }

    // Port of the virtual `SkAndroidCodec::onGetSampledDimensions`, dispatched on the subclass.
    fn on_get_sampled_dimensions(&self, sample_size: i32) -> ISize {
        match self.kind {
            Kind::Sampled => sampled_codec::on_get_sampled_dimensions(&self.codec, sample_size),
            Kind::Adapter => {
                android_codec_adapter::on_get_sampled_dimensions(&self.codec, sample_size)
            }
        }
    }

    // Port of the virtual `SkAndroidCodec::onGetAndroidPixels`, dispatched on the subclass.
    fn on_get_android_pixels(
        &mut self,
        info: &ImageInfo,
        pixels: &mut [u8],
        row_bytes: usize,
        options: &AndroidOptions,
    ) -> Result {
        match self.kind {
            Kind::Sampled => {
                sampled_codec::on_get_android_pixels(self, info, pixels, row_bytes, options)
            }
            Kind::Adapter => android_codec_adapter::on_get_android_pixels(
                &mut self.codec,
                info,
                pixels,
                row_bytes,
                options,
            ),
        }
    }

    /// Port of `SkAndroidCodec::getEncodedFormat` (through the wrapped codec).
    #[must_use]
    pub fn encoded_format(&self) -> EncodedImageFormat {
        self.codec.encoded_format()
    }

    /// Port of `SkAndroidCodec::computeOutputColorType`: the colour type the codec decodes to for a
    /// requested one. High-precision files decode to F16 and 10-bit ones to 1010102 by default.
    // Port of: src/codec/SkAndroidCodec.cpp#L74-L113 (chrome/m156)
    #[must_use]
    #[doc(alias = "computeOutputColorType")]
    pub fn compute_output_color_type(&self, requested_color_type: ColorType) -> ColorType {
        let high_precision = self.codec.encoded_info().bits_per_component() > 8;
        let color_depth = self.codec.encoded_info().color_depth();
        match requested_color_type {
            ColorType::ARGB4444 => return ColorType::N32,
            // Before Gray8 existed, clients requested Alpha8 for a grayscale decode, so Alpha8 falls
            // through to Gray8.
            ColorType::Alpha8 | ColorType::Gray8 => {
                if self.info.color_type() == ColorType::Gray8 {
                    return ColorType::Gray8;
                }
            }
            ColorType::RGB565 => {
                if self.info.alpha_type() == AlphaType::Opaque {
                    return ColorType::RGB565;
                }
            }
            ColorType::RGBA1010102 => {
                if color_depth == 10 {
                    return ColorType::RGBA1010102;
                }
            }
            ColorType::RGBAF16 => return ColorType::RGBAF16,
            _ => {}
        }

        // F16 is the Android default for high-precision images.
        if high_precision {
            ColorType::RGBAF16
        } else if color_depth == 10 {
            ColorType::RGBA1010102
        } else {
            ColorType::N32
        }
    }

    /// Port of `SkAndroidCodec::computeOutputAlphaType`: opaque images stay opaque; otherwise the
    /// requested unpremultiplied or premultiplied alpha.
    // Port of: src/codec/SkAndroidCodec.cpp#L115-L120 (chrome/m156)
    #[must_use]
    #[doc(alias = "computeOutputAlphaType")]
    pub fn compute_output_alpha_type(&self, requested_unpremul: bool) -> AlphaType {
        if self.info.alpha_type() == AlphaType::Opaque {
            return AlphaType::Opaque;
        }
        if requested_unpremul {
            AlphaType::Unpremul
        } else {
            AlphaType::Premul
        }
    }

    /// Port of `SkAndroidCodec::computeOutputColorSpace`: the colour space a decode to
    /// `output_color_type` produces. A preferred space wins; then the image's own profile; then sRGB.
    /// Colour types that Android does not colour-correct have none.
    // Port of: src/codec/SkAndroidCodec.cpp#L122-L157 (chrome/m156)
    #[must_use]
    #[doc(alias = "computeOutputColorSpace")]
    pub fn compute_output_color_space(
        &self,
        output_color_type: ColorType,
        pref_color_space: Option<ColorSpace>,
    ) -> Option<ColorSpace> {
        match output_color_type {
            ColorType::RGBAF16
            | ColorType::RGB565
            | ColorType::RGBA8888
            | ColorType::BGRA8888
            | ColorType::RGBA1010102 => {
                // If a preferred space is supplied, choose it.
                if pref_color_space.is_some() {
                    return pref_color_space;
                }
                if let Some(profile) = self.codec.encoded_info().profile() {
                    return get_android_output_color_space(profile);
                }
                Some(ColorSpace::new_srgb())
            }
            // Colour correction is not supported for gray.
            _ => None,
        }
    }
}

/// Port of `ColorProfile::getAndroidOutputColorSpace`: a CICP description when the profile has one,
/// else the profile's own colour space, else its primaries with the sRGB curve, else sRGB.
// Port of: src/codec/SkCodecColorProfile.cpp#L146-L168 (chrome/m156)
#[must_use]
pub fn get_android_output_color_space(profile: &IccProfile) -> Option<ColorSpace> {
    // Prefer CICP information if it exists.
    if profile.has_cicp {
        let cicp = &profile.cicp;
        if let Some(cicp_space) = cicp_get_android_sk_color_space(
            cicp.color_primaries,
            cicp.transfer_characteristics,
            cicp.matrix_coefficients,
            cicp.video_full_range_flag,
        ) {
            return Some(cicp_space);
        }
    }
    // Leave the pixels in the encoded colour space. Colour space conversion is done after decode.
    if let Some(encoded_space) = ColorSpace::make(profile) {
        return Some(encoded_space);
    }
    if profile.has_to_xyzd50 {
        return ColorSpace::new_rgb(&named_transfer_fn::SRGB, &profile.to_xyzd50);
    }
    Some(ColorSpace::new_srgb())
}

// The constants are written at the precision Skia uses; `f32` rounds them to the same values.
/// Port of `cicp_get_android_sk_color_space`: the colour space of a CICP description, with the
/// Android adjustments to PQ and HLG that put SDR white at 203 nits. Only full-range, non-matrix
/// descriptions are handled.
// Port of: src/codec/SkCodecColorProfile.cpp#L23-L51 (chrome/m156)
#[must_use]
#[allow(clippy::excessive_precision)] // constants as written in Skia (see the note above)
fn cicp_get_android_sk_color_space(
    color_primaries: u8,
    transfer_characteristics: u8,
    matrix_coefficients: u8,
    full_range_flag: u8,
) -> Option<ColorSpace> {
    if matrix_coefficients != 0 {
        return None;
    }
    if full_range_flag != 1 {
        return None;
    }
    let primaries = named_primaries::get_cicp(named_primaries::CicpId::from_u8(color_primaries)?)?;
    let mut trfn: TransferFunction = named_transfer_fn::get_cicp(
        named_transfer_fn::CicpId::from_u8(transfer_characteristics)?,
    )?;
    match transfer_characteristics {
        16 => {
            // Android expects PQ to match 203 nits to SDR white.
            trfn = TransferFunction {
                g: -2.0,
                a: -1.555_222_978_32,
                b: 1.860_453_656_31,
                c: 32.0 / 2523.0,
                d: 2413.0 / 128.0,
                e: -2392.0 / 128.0,
                f: 8192.0 / 1305.0,
            };
        }
        18 => {
            // Android expects HLG to match 203 nits to SDR white.
            trfn = TransferFunction::make_scaled_hlgish(
                0.314_509_843,
                2.0,
                2.0,
                1.0 / 0.178_832_77,
                0.284_668_92,
                0.559_910_73,
            );
        }
        _ => {}
    }
    ColorSpace::new_rgb(&trfn, &primaries)
}

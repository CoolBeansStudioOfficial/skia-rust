// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkSampledCodec.cpp#L1-L355 and src/codec/SkSampledCodec.h (chrome/m156)
// Ported from: src/codec/SkSampledCodec.cpp, src/codec/SkSampledCodec.h
//
// SkSampledCodec is the AndroidCodec subclass for the codecs that cannot scale natively. The C++
// class holds only the codec; its methods are free functions here that take the AndroidCodec, which
// holds the codec (`fCodec`) and the subclass decision.

//! Android decodes for codecs that scale by sampling: the codec decodes at its own size (or a size
//! it supports natively), and rows and columns are then sampled as they are decoded.

use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

use crate::android_codec::{AndroidCodec, AndroidOptions};
use crate::codec::{Codec, Result};
use crate::codec_priv::{
    get_dst_coord, get_sampled_dimension, get_scale_from_sample_size, get_start_coord,
    is_coord_necessary,
};
use crate::sampler;

// The row offset of a destination row, in bytes. The row numbers are never negative.
// Port of the `rowBytes * SkCodecPriv::GetDstCoord(...)` pointer arithmetic in SkSampledCodec.cpp.
fn row_offset(row: i32, row_bytes: usize) -> usize {
    usize::try_from(row).unwrap_or(0) * row_bytes
}

/// Port of `SkSampledCodec::onGetSampledDimensions`.
// Port of: src/codec/SkSampledCodec.cpp#L44-L48 (chrome/m156)
#[must_use]
pub(crate) fn on_get_sampled_dimensions(codec: &Codec<'_>, sample_size: i32) -> ISize {
    let mut sample_size = sample_size;
    let size = account_for_native_scaling(codec, &mut sample_size, None);
    ISize::new(
        get_sampled_dimension(size.width, sample_size),
        get_sampled_dimension(size.height, sample_size),
    )
}

/// Port of `SkSampledCodec::accountForNativeScaling`: the size the codec should scale to, and the
/// sample size that still has to be applied to that size. `native_sample_size`, when given, is set
/// to the factor the codec scales by natively (1 when it does not).
// Port of: src/codec/SkSampledCodec.cpp#L16-L60 (chrome/m156)
#[must_use]
pub(crate) fn account_for_native_scaling(
    codec: &Codec<'_>,
    sample_size: &mut i32,
    native_sample_size: Option<&mut i32>,
) -> ISize {
    let mut pre_sampled_size = codec.dimensions();
    let requested = *sample_size;
    debug_assert!(requested > 1);

    let mut native = 1;

    // Only JPEG supports native downsampling.
    if codec.encoded_format() == EncodedImageFormat::JPEG {
        // See if libjpeg supports this scale directly.
        if matches!(requested, 2 | 4 | 8) {
            // This class does not need to do any sampling.
            *sample_size = 1;
            if let Some(native_out) = native_sample_size {
                *native_out = 1;
            }
            return codec.get_scaled_dimensions(get_scale_from_sample_size(requested));
        }

        // Check if sample_size is a multiple of something libjpeg can support.
        for supported_sample_size in [8, 4, 2] {
            let actual_sample_size = requested / supported_sample_size;
            let remainder = requested % supported_sample_size;
            if remainder == 0 {
                let scale = get_scale_from_sample_size(supported_sample_size);

                // The codec will scale to this size.
                pre_sampled_size = codec.get_scaled_dimensions(scale);

                // And then this class will sample it.
                *sample_size = actual_sample_size;
                native = supported_sample_size;
                break;
            }
        }
    }

    if let Some(native_out) = native_sample_size {
        *native_out = native;
    }
    pre_sampled_size
}

/// Port of `SkSampledCodec::onGetAndroidPixels`: decodes the whole image, or a subset of it, at the
/// requested sample size, by native scaling where the codec supports it and by sampling otherwise.
// Port of: src/codec/SkSampledCodec.cpp#L62-L159 (chrome/m156)
pub(crate) fn on_get_android_pixels(
    this: &mut AndroidCodec<'_>,
    info: &ImageInfo,
    pixels: &mut [u8],
    row_bytes: usize,
    options: &AndroidOptions,
) -> Result {
    let dims = this.codec.dimensions();
    match options.base.subset {
        Some(subset) if subset.size() != dims => {}
        _ => {
            if this.codec.dimensions_supported(info.dimensions()) {
                return this
                    .codec
                    .get_pixels(info, pixels, row_bytes, Some(&options.base));
            }
            // If the native codec does not support the requested scale, scale by sampling.
            return sampled_decode(this, info, pixels, row_bytes, options);
        }
    }
    // Subset is `Some` and not the full image: we are performing a subset decode.
    let Some(subset) = options.base.subset else {
        return Result::InternalError;
    };

    let sample_size = options.sample_size;
    let scaled_size = this.get_sampled_dimensions(sample_size);
    if !this.codec.dimensions_supported(scaled_size) {
        // If the native codec does not support the requested scale, scale by sampling.
        return sampled_decode(this, info, pixels, row_bytes, options);
    }

    // Calculate the scaled subset bounds.
    let scaled_subset_x = subset.x() / sample_size;
    let scaled_subset_y = subset.y() / sample_size;
    let scaled_subset_width = info.width();
    let scaled_subset_height = info.height();

    let scaled_info = info.with_dimensions(scaled_size);

    // Copy so we can use a different subset.
    let mut subset_options = options.clone();
    {
        // Although start_scanline_decode expects the bottom and top to match the ImageInfo,
        // start_incremental_decode uses them to determine which rows to decode.
        let incremental_subset = IRect::from_xywh(
            scaled_subset_x,
            scaled_subset_y,
            scaled_subset_width,
            scaled_subset_height,
        );
        subset_options.base.subset = Some(incremental_subset);
        match this.codec.start_incremental_decode(
            &scaled_info,
            pixels,
            row_bytes,
            Some(&subset_options.base),
        ) {
            Result::Success => {
                let (inc_result, rows_decoded) = this.codec.incremental_decode_rows(pixels);
                if inc_result == Result::Success {
                    return Result::Success;
                }
                if inc_result == Result::IncompleteInput || inc_result == Result::ErrorInInput {
                    this.codec.fill_incomplete_image(
                        &scaled_info,
                        pixels,
                        row_bytes,
                        options.base.zero_initialized,
                        scaled_subset_height,
                        rows_decoded,
                    );
                }
                return inc_result;
            }
            Result::Unimplemented => {
                // Otherwise fall down to use the old scanline decoder. subset_options.subset is
                // reset below, so it will not continue to point to the object that is gone.
            }
            start_result => return start_result,
        }
    }

    // Start the scanline decode.
    let scanline_subset =
        IRect::from_xywh(scaled_subset_x, 0, scaled_subset_width, scaled_size.height);
    subset_options.base.subset = Some(scanline_subset);

    let result = this
        .codec
        .start_scanline_decode(&scaled_info, Some(&subset_options.base));
    if result != Result::Success {
        return result;
    }

    // At this point, we are only concerned with subsetting. Either no scale was requested, or the
    // codec is handling the scale. Subsetting is only supported for top-down images, so this code
    // is not reached for other orders.
    if !this.codec.skip_scanlines(scaled_subset_y) {
        this.codec.fill_incomplete_image(
            info,
            pixels,
            row_bytes,
            options.base.zero_initialized,
            scaled_subset_height,
            0,
        );
        return Result::IncompleteInput;
    }

    let decoded_lines = this
        .codec
        .get_scanlines(pixels, scaled_subset_height, row_bytes);
    if decoded_lines != scaled_subset_height {
        return Result::IncompleteInput;
    }
    Result::Success
}

/// Port of `SkSampledCodec::sampledDecode`: decodes at the requested sample size by sampling the
/// codec's native output. Same contract as `on_get_android_pixels`.
// Port of: src/codec/SkSampledCodec.cpp#L161-L355 (chrome/m156)
// The C++ `sampleY` loops and the `GetDstCoord` pointer arithmetic are kept as written.
#[allow(clippy::too_many_lines)] // one arm per scanline order, as in the C++ switch
fn sampled_decode(
    this: &mut AndroidCodec<'_>,
    info: &ImageInfo,
    pixels: &mut [u8],
    row_bytes: usize,
    options: &AndroidOptions,
) -> Result {
    // We should only call this function when sampling.
    debug_assert!(options.sample_size > 1);

    // This was already called by on_get_android_pixels. The native sample size is needed here.
    let mut sample_size = options.sample_size;
    let mut native_sample_size = 1;
    let native_size =
        account_for_native_scaling(&this.codec, &mut sample_size, Some(&mut native_sample_size));

    // Check if there is a subset.
    let mut subset = IRect::from_wh(0, 0);
    let mut subset_y = 0;
    let mut subset_width = native_size.width;
    let mut subset_height = native_size.height;
    if let Some(subset_ptr) = options.base.subset {
        // We will need to know about subsetting in the y-dimension in order to use the scanline
        // decoder. Update the subset to account for scaling done by the codec.
        //
        // Do the divide ourselves, instead of calling get_sampled_dimension. If X and Y are 0, they
        // should remain 0, rather than being upgraded to 1 due to being smaller than the sample size.
        let subset_x = subset_ptr.x() / native_sample_size;
        subset_y = subset_ptr.y() / native_sample_size;

        subset_width = get_sampled_dimension(subset_ptr.width(), native_sample_size);
        subset_height = get_sampled_dimension(subset_ptr.height(), native_sample_size);

        // The scanline decoder only needs to be aware of subsetting in the x-dimension.
        subset = IRect::from_xywh(subset_x, 0, subset_width, native_size.height);
    }

    // Since we guarantee that output dimensions are always at least one (even if the sample size is
    // greater than a given dimension), the input sample size is not always the sample size that we
    // use in practice.
    let sample_x = subset_width / info.width();
    let sample_y = subset_height / info.height();

    let sampling_offset_y = get_start_coord(sample_y);
    let start_y = sampling_offset_y + subset_y;
    let dst_height = info.height();

    let native_info = info.with_dimensions(native_size);

    {
        // Although start_scanline_decode expects the bottom and top to match the ImageInfo,
        // start_incremental_decode uses them to determine which rows to decode.
        let mut incremental_options = options.clone();
        if options.base.subset.is_some() {
            incremental_options.base.subset = Some(IRect::from_ltrb(
                subset.left(),
                subset_y,
                subset.right(),
                subset_y + subset_height,
            ));
        }
        match this.codec.start_incremental_decode(
            &native_info,
            pixels,
            row_bytes,
            Some(&incremental_options.base),
        ) {
            Result::Success => {
                let Some(sampler_ref) = this.codec.get_sampler(true) else {
                    return Result::Unimplemented;
                };
                if sampler_ref.set_sample_x(sample_x) != info.width() {
                    return Result::InvalidScale;
                }
                if get_sampled_dimension(subset_height, sample_y) != info.height() {
                    return Result::InvalidScale;
                }
                sampler_ref.set_sample_y(sample_y);

                let (inc_result, rows_decoded) = this.codec.incremental_decode_rows(pixels);
                if inc_result == Result::Success {
                    return Result::Success;
                }
                if inc_result == Result::IncompleteInput || inc_result == Result::ErrorInInput {
                    debug_assert!(rows_decoded <= info.height());
                    this.codec.fill_incomplete_image(
                        info,
                        pixels,
                        row_bytes,
                        options.base.zero_initialized,
                        info.height(),
                        rows_decoded,
                    );
                }
                return inc_result;
            }
            Result::IncompleteInput | Result::ErrorInInput => {
                return Result::InvalidInput;
            }
            // For Unimplemented we fall back to the scanline decoder.
            Result::Unimplemented => {}
            start_result => return start_result,
        }
    }

    // Start the scanline decode.
    let mut sampled_options = options.clone();
    if options.base.subset.is_some() {
        sampled_options.base.subset = Some(subset);
    }
    let result = this
        .codec
        .start_scanline_decode(&native_info, Some(&sampled_options.base));
    if result == Result::IncompleteInput || result == Result::ErrorInInput {
        return Result::InvalidInput;
    }
    if result != Result::Success {
        return result;
    }

    let Some(sampler_ref) = this.codec.get_sampler(true) else {
        return Result::InternalError;
    };
    if sampler_ref.set_sample_x(sample_x) != info.width() {
        return Result::InvalidScale;
    }
    if get_sampled_dimension(subset_height, sample_y) != info.height() {
        return Result::InvalidScale;
    }

    match this.codec.scanline_order() {
        crate::codec::ScanlineOrder::TopDown => {
            if !this.codec.skip_scanlines(start_y) {
                this.codec.fill_incomplete_image(
                    info,
                    pixels,
                    row_bytes,
                    options.base.zero_initialized,
                    dst_height,
                    0,
                );
                return Result::IncompleteInput;
            }
            let mut offset = 0;
            for y in 0..dst_height {
                if this
                    .codec
                    .get_scanlines(&mut pixels[offset..], 1, row_bytes)
                    != 1
                {
                    this.codec.fill_incomplete_image(
                        info,
                        pixels,
                        row_bytes,
                        options.base.zero_initialized,
                        dst_height,
                        y + 1,
                    );
                    return Result::IncompleteInput;
                }
                if y + 1 < dst_height && !this.codec.skip_scanlines(sample_y - 1) {
                    this.codec.fill_incomplete_image(
                        info,
                        pixels,
                        row_bytes,
                        options.base.zero_initialized,
                        dst_height,
                        y + 1,
                    );
                    return Result::IncompleteInput;
                }
                offset += row_bytes;
            }
            Result::Success
        }
        crate::codec::ScanlineOrder::BottomUp => {
            // Note that these modes do not support subsetting.
            debug_assert!(subset_y == 0 && native_size.height == subset_height);
            let mut y = 0;
            while y < native_size.height {
                let src_y = this.codec.next_scanline();
                if is_coord_necessary(src_y, sample_y, dst_height) {
                    let offset = row_offset(get_dst_coord(src_y, sample_y), row_bytes);
                    if this
                        .codec
                        .get_scanlines(&mut pixels[offset..], 1, row_bytes)
                        != 1
                    {
                        break;
                    }
                } else if !this.codec.skip_scanlines(1) {
                    break;
                }
                y += 1;
            }

            if native_size.height == y {
                return Result::Success;
            }

            // We handle filling uninitialized memory here instead of using the codec. The codec does
            // not know that we are sampling.
            let fill_info = info.with_wh(info.width(), 1);
            while y < native_size.height {
                let src_y = this.codec.output_scanline(y);
                if is_coord_necessary(src_y, sample_y, dst_height) {
                    let offset = row_offset(get_dst_coord(src_y, sample_y), row_bytes);
                    sampler::fill(
                        &fill_info,
                        &mut pixels[offset..],
                        row_bytes,
                        options.base.zero_initialized,
                    );
                }
                y += 1;
            }
            Result::IncompleteInput
        }
    }
}

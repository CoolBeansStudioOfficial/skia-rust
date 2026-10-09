// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/WebpTest.cpp (chrome/m156).

#![cfg(test)]

use skia_rust_codec::{Options, Result, codecs};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::{def_test, errorf, reporter_assert};

/// `ToolUtils::equal_pixels` for two buffers of the same image info: the rows are compared over the
/// bytes of each pixel row.
fn equal_pixels(info: &ImageInfo, row_bytes: usize, a: &[u8], b: &[u8]) -> bool {
    let row_len = info.bytes_per_pixel() * usize::try_from(info.width()).unwrap_or(0);
    (0..usize::try_from(info.height()).unwrap_or(0)).all(|y| {
        let start = y * row_bytes;
        a[start..start + row_len] == b[start..start + row_len]
    })
}

// Port of: tests/WebpTest.cpp#L18-L64 (WebpCodecBlend, chrome/m156)
def_test!(WebpCodecBlend, |r| {
    let path = "images/blendBG.webp";
    let Some(data) = get_resource_as_data(path) else {
        errorf!(r, "Failed to open/decode {}", path);
        return;
    };
    let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        errorf!(r, "Failed to open/decode {}", path);
        return;
    };

    // Previously, a bug in SkWebpCodec resulted in different output depending on whether kPremul
    // or kOpaque SkAlphaType was passed to getPixels(). Decode each frame twice, once with kPremul
    // and once with kOpaque if the frame is opaque, and verify they look the same.
    let premul_info = codec.info().with_alpha_type(AlphaType::Premul);
    let row_bytes = premul_info.min_row_bytes();
    let size = premul_info.compute_byte_size(row_bytes);
    // The SkBitmap's SkAlphaType is unrelated to the bug.
    let mut premul_bm = vec![0u8; size];
    let mut change_bm = vec![0u8; size];

    for i in 0..codec.get_frame_count() {
        let options = Options {
            frame_index: i,
            ..Options::default()
        };
        let result = codec.get_pixels(&premul_info, &mut premul_bm, row_bytes, Some(&options));
        if result != Result::Success {
            errorf!(
                r,
                "Failed to decode {} frame {} (premul) - error {}",
                path,
                i,
                result.as_str()
            );
            return;
        }

        let Some(frame_info) = codec.get_frame_info(i) else {
            errorf!(r, "Failed to getFrameInfo for {} frame {}", path, i);
            return;
        };

        let alpha_type = if frame_info.alpha_type == AlphaType::Opaque {
            AlphaType::Opaque
        } else {
            AlphaType::Premul
        };
        let result = codec.get_pixels(
            &premul_info.with_alpha_type(alpha_type),
            &mut change_bm,
            row_bytes,
            Some(&options),
        );
        if result != Result::Success {
            errorf!(
                r,
                "Failed to decode {} frame {} (change) - error {}",
                path,
                i,
                result.as_str()
            );
            return;
        }

        reporter_assert!(
            r,
            equal_pixels(&premul_info, row_bytes, &premul_bm, &change_bm),
            "{} frame {} does not match with mismatched AlphaType",
            path,
            i
        );
    }
});

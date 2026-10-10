// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/EncodedInfoTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_codec::codec::Result as CodecResult;
use skia_rust_codec::codecs::make_codec_from_stream;
use skia_rust_codec::encode::png_encoder::{self, Options};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::tools::tool_utils::equal_pixels;
use crate::{def_test, reporter_assert, skip_missing_resource};

// Port of: tests/EncodedInfoTest.cpp#L22-L46 (chrome/m156)
def_test!(AlphaEncodedInfo, |r| {
    let path = "images/grayscale.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(mut codec) = make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, codec.info().color_type() == ColorType::Gray8);

    // SkBitmap bm; bm.allocPixels(codec->getInfo().makeColorType(kAlpha_8).makeColorSpace(nullptr));
    let alpha_info = codec
        .info()
        .with_color_type(ColorType::Alpha8)
        .with_color_space(None);
    let row_bytes = alpha_info.min_row_bytes();
    let mut bm = vec![0u8; alpha_info.compute_byte_size(row_bytes)];
    // codec->getPixels(codec->getInfo(), bm.getPixels(), bm.rowBytes())
    let result = codec.get_pixels(&codec.info(), &mut bm, row_bytes, None);
    reporter_assert!(r, result == CodecResult::Success);

    // sk_sp<SkData> data = SkPngEncoder::Encode(bm.pixmap(), {});
    let Some(pm) = Pixmap::new_readonly(&alpha_info, &bm, row_bytes) else {
        reporter_assert!(r, false);
        return;
    };
    let Some(encoded) = png_encoder::encode_pixmap(&pm, &Options::default()) else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, encoded.size() > 0);

    // codec = SkCodec::MakeFromData(data);
    let Ok(mut codec) = make_codec_from_stream(MemoryStream::make_copy(encoded.as_bytes())) else {
        reporter_assert!(r, false);
        return;
    };
    // TODO (C++): Make SkEncodedInfo public and compare to its version of kAlpha_8.
    reporter_assert!(r, codec.info().color_type() == ColorType::Alpha8);

    // SkBitmap bm2; bm2.allocPixels(codec->getInfo().makeColorSpace(nullptr));
    let bm2_info = codec.info().with_color_space(None);
    let row_bytes2 = bm2_info.min_row_bytes();
    let mut bm2 = vec![0u8; bm2_info.compute_byte_size(row_bytes2)];
    // result = codec->getPixels(bm2.pixmap());
    let result = codec.get_pixels(&bm2_info, &mut bm2, row_bytes2, None);
    reporter_assert!(r, result == CodecResult::Success);
    let Some(pm2) = Pixmap::new_readonly(&bm2_info, &bm2, row_bytes2) else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, equal_pixels(&pm, &pm2));
});

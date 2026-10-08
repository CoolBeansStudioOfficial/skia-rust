// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecTest.cpp (chrome/m156), the cases that only need the codec base and the WBMP
// decoder. The rest of the file needs the Android codec, the image generator and the other
// decoders, and is ported with them.

use skia_rust_codec::{Codec, Result, decoders};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, reporter_assert, skip_missing_resource};

// Port of: tests/CodecTest.cpp#L122-L135 (test_info, without the digest comparison that these
// cases do not use)
fn test_info(reporter: &mut Reporter, codec: &mut Codec<'_>, info: &ImageInfo, expected: Result) {
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(info, &mut pixels, row_bytes, None);
    reporter_assert!(reporter, result == expected);
}

// Port of: tests/CodecTest.cpp#L1122-L1146 (chrome/m156)
def_test!(Codec_wbmp_restrictive, |r| {
    let path = "images/mandrill.wbmp";
    let mut data = skip_missing_resource!(get_resource_as_data(path), path);

    // Modify the stream to contain a second byte with some bits set.
    data[1] = !0x9F_u8;

    // SkCodec should support this.
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(r, false);
        return;
    };
    let info = codec.info();
    test_info(r, &mut codec, &info, Result::Success);
});

// wbmp images have a header that can be arbitrarily large, depending on the size of the image. We
// cap the size at 65535, meaning we only need to look at 8 bytes to determine whether we can read
// the image. This is important because SkCodec only passes a limited number of bytes to
// SkWbmpCodec to determine whether the image is a wbmp.
// Port of: tests/CodecTest.cpp#L1148-L1172 (chrome/m156)
def_test!(Codec_wbmp_max_size, |r| {
    let max_size_wbmp: [u8; 8] = [
        0x00, 0x00, // Header
        0x83, 0xFF, 0x7F, // W: 65535
        0x83, 0xFF, 0x7F, // H: 65535
    ];
    let Ok(codec) = Codec::make_from_stream(MemoryStream::make_copy(&max_size_wbmp), decoders())
    else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, codec.info().width() == 65535);
    reporter_assert!(r, codec.info().height() == 65535);

    // Now test an image which is too big. Any image with a larger header (i.e. has bigger
    // width/height) is also too big.
    let too_big_wbmp: [u8; 8] = [
        0x00, 0x00, // Header
        0x84, 0x80, 0x00, // W: 65536
        0x84, 0x80, 0x00, // H: 65536
    ];
    let codec = Codec::make_from_stream(MemoryStream::make_copy(&too_big_wbmp), decoders());
    reporter_assert!(r, codec.is_err());
});

// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkBase64Test.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_resources::base64::{decode, encode, encoded_size};

use crate::{def_test, errorf, reporter_assert};

/// The bytes up to the first NUL, as `strcmp` compares them.
fn c_str(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

// Port of: tests/SkBase64Test.cpp#L18-L62 (chrome/m156) (`DEF_TEST(SkBase64)`)
def_test!(SkBase64, |reporter| {
    // The bytes 1..=255, then NUL, as in the C++ array.
    let mut all = [0_u8; 256];
    for (index, byte) in all.iter_mut().take(255).enumerate() {
        #[allow(clippy::cast_possible_truncation)] // 1..=255 fit in a byte, as in the C++ array
        let value = (index + 1) as u8;
        *byte = value;
    }
    all[255] = 0;

    for offset in 0..6 {
        let length = 256 - offset;
        let source = &all[offset..offset + length];

        // Encode. The C++ measures the output with a null destination first; the size it
        // predicts is the size of the output.
        let predicted_encode_length = encoded_size(length);
        let mut encoded = vec![0_u8; predicted_encode_length];
        let actual_encode_length = encode(source, &mut encoded);
        reporter_assert!(
            reporter,
            actual_encode_length == predicted_encode_length,
            "input size {} output size {} != {}",
            length,
            actual_encode_length,
            predicted_encode_length
        );

        // Decode. The C++ NUL-terminates the text, and decodes its first `actual` bytes.
        let text = &encoded[..actual_encode_length];
        let decode_length = match decode(text, None) {
            Ok(decode_length) => decode_length,
            Err(error) => {
                errorf!(reporter, "SkBase64::Decode failed: {:?}", error);
                continue;
            }
        };
        reporter_assert!(reporter, decode_length == length);

        let mut decoded = vec![0_u8; decode_length];
        match decode(text, Some(&mut decoded)) {
            Ok(written) => reporter_assert!(reporter, written == length),
            Err(error) => {
                errorf!(reporter, "SkBase64::Decode failed: {:?}", error);
                continue;
            }
        }
        reporter_assert!(
            reporter,
            c_str(&all[offset..]) == c_str(&decoded),
            "decoded text differs at offset {}",
            offset
        );
    }
});

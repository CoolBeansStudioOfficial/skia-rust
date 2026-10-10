// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkBase64Test.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::base64::{decode, encode, encoded_size};

use crate::{def_test, errorf, reporter_assert};

// `strcmp(a, b) == 0` for byte strings: compares up to the first NUL of either.
fn c_str_eq(a: &[u8], b: &[u8]) -> bool {
    let mut i = 0;
    loop {
        let ca = a.get(i).copied().unwrap_or(0);
        let cb = b.get(i).copied().unwrap_or(0);
        if ca != cb {
            return false;
        }
        if ca == 0 {
            return true;
        }
        i += 1;
    }
}

// Port of: tests/SkBase64Test.cpp#L18-L62 (chrome/m156)
def_test!(SkBase64, |reporter| {
    let mut all = [0u8; 256];
    for (index, byte) in all.iter_mut().take(255).enumerate() {
        *byte = u8::try_from(index + 1).expect("1..=255 fits in a byte");
    }
    all[255] = 0;

    for offset in 0..6 {
        let length = 256 - offset;

        // Encode
        let predicted_encode_length = encoded_size(length);
        let actual_encode_length = encode(&all[offset..offset + length], None, None);

        reporter_assert!(
            reporter,
            actual_encode_length == predicted_encode_length,
            "input size {length}; output size {actual_encode_length} != {predicted_encode_length}"
        );
        let mut src = vec![0u8; actual_encode_length + 1];
        let n = encode(&all[offset..offset + length], Some(&mut src), None);
        reporter_assert!(reporter, n == predicted_encode_length);

        src[actual_encode_length] = 0;

        // Decode
        let Ok(decode_length) = decode(&src[..actual_encode_length], None) else {
            errorf!(reporter, "SkBase64::Decode failed!");
            continue;
        };
        reporter_assert!(reporter, decode_length == length);

        let mut dst = vec![0u8; decode_length];
        let Ok(decode_length) = decode(&src[..actual_encode_length], Some(&mut dst)) else {
            errorf!(reporter, "SkBase64::Decode failed!");
            continue;
        };
        reporter_assert!(reporter, decode_length == length);

        reporter_assert!(reporter, c_str_eq(&all[offset..], &dst));
    }
});

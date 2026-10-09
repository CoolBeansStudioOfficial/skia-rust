// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MD5Test.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::md5::{Digest, Md5};
use skia_rust_core::stream::WStream;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/MD5Test.cpp#L13-L20 (chrome/m156)
fn digests_equal(expected_digest: &Digest, computed_digest: &Digest) -> bool {
    for (expected, computed) in expected_digest.data.iter().zip(&computed_digest.data) {
        if expected != computed {
            return false;
        }
    }
    true
}

// Port of: tests/MD5Test.cpp#L22-L44 (chrome/m156)
fn md5_test(string: &str, expected_digest: &Digest, reporter: &mut Reporter) {
    // All at once
    {
        let mut context = Md5::new();
        context.write(string.as_bytes());
        let digest = context.finish();

        reporter_assert!(reporter, digests_equal(expected_digest, &digest));
    }

    // One byte at a time.
    {
        let mut context = Md5::new();
        for byte in string.as_bytes() {
            context.write(std::slice::from_ref(byte));
        }
        let digest = context.finish();

        reporter_assert!(reporter, digests_equal(expected_digest, &digest));
    }
}

// Port of: tests/MD5Test.cpp#L46-L65 (chrome/m156)
struct MD5Test {
    message: &'static str,
    digest: Digest,
}

// Port of: tests/MD5Test.cpp#L46-L65 (chrome/m156)
const MD5_TESTS: [MD5Test; 7] = [
    // Reference tests from RFC1321 Section A.5 ( http://www.ietf.org/rfc/rfc1321.txt )
    MD5Test {
        message: "",
        digest: Digest {
            data: [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e,
            ],
        },
    },
    MD5Test {
        message: "a",
        digest: Digest {
            data: [
                0x0c, 0xc1, 0x75, 0xb9, 0xc0, 0xf1, 0xb6, 0xa8, 0x31, 0xc3, 0x99, 0xe2, 0x69, 0x77,
                0x26, 0x61,
            ],
        },
    },
    MD5Test {
        message: "abc",
        digest: Digest {
            data: [
                0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
                0x7f, 0x72,
            ],
        },
    },
    MD5Test {
        message: "message digest",
        digest: Digest {
            data: [
                0xf9, 0x6b, 0x69, 0x7d, 0x7c, 0xb7, 0x93, 0x8d, 0x52, 0x5a, 0x2f, 0x31, 0xaa, 0xf1,
                0x61, 0xd0,
            ],
        },
    },
    MD5Test {
        message: "abcdefghijklmnopqrstuvwxyz",
        digest: Digest {
            data: [
                0xc3, 0xfc, 0xd3, 0xd7, 0x61, 0x92, 0xe4, 0x00, 0x7d, 0xfb, 0x49, 0x6c, 0xca, 0x67,
                0xe1, 0x3b,
            ],
        },
    },
    MD5Test {
        message: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
        digest: Digest {
            data: [
                0xd1, 0x74, 0xab, 0x98, 0xd2, 0x77, 0xd9, 0xf5, 0xa5, 0x61, 0x1c, 0x2c, 0x9f, 0x41,
                0x9d, 0x9f,
            ],
        },
    },
    MD5Test {
        message: "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
        digest: Digest {
            data: [
                0x57, 0xed, 0xf4, 0xa2, 0x2b, 0xe3, 0xc9, 0x55, 0xac, 0x49, 0xda, 0x2e, 0x21, 0x07,
                0xb6, 0x7a,
            ],
        },
    },
];

// Port of: tests/MD5Test.cpp#L67-L71 (chrome/m156)
def_test!(MD5, |reporter| {
    for test in &MD5_TESTS {
        md5_test(test.message, &test.digest, reporter);
    }
});

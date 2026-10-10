// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/UtilsTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};

// Port of: tests/UtilsTest.cpp#L615-L646 (chrome/m156)
def_test!(UtilsPreserveBitPatterns, |r| {
    // Various kinds of floating point bit patterns. We round trip each one through float using
    // utility functions. If any of them ever do any real FP operation (including loading it into
    // the x87 FPU on x86 builds), they might change. (In practice, signaling NaN is the only one
    // that's likely to break -- it can be converted to a quiet NaN).
    let bit_patterns: [u32; 6] = [
        0x0040_0000, // Denormal value
        0x8000_0000, // -0.0f
        0x3f80_0000, // 1.0f (arbitrary normal float)
        0x7f80_0000, // Infinity
        0x7fa0_0000, // Signaling NaN
        0x7fe0_0000, // Quiet NaN
    ];

    for src_bits in bit_patterns {
        {
            // sk_unaligned_load<float>(&srcBits), then sk_unaligned_load<uint32_t>(&floatVal)
            let float_val = f32::from_bits(src_bits);
            let dst_bits = float_val.to_bits();
            reporter_assert!(r, dst_bits == src_bits);
        }

        {
            // sk_unaligned_store(&floatVal, srcBits), then sk_unaligned_store(&dstBits, floatVal)
            let float_val = f32::from_bits(src_bits);
            let dst_bits = float_val.to_bits();
            reporter_assert!(r, dst_bits == src_bits);
        }

        // sk_bit_cast<uint32_t>(sk_bit_cast<float>(srcBits))
        reporter_assert!(r, f32::from_bits(src_bits).to_bits() == src_bits);
    }
});

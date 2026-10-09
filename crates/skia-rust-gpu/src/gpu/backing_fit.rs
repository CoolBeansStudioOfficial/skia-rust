// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/SkBackingFit.h, src/gpu/SkBackingFit.cpp

//! `SkBackingFit` and `skgpu::GetApproxSize`.

use skia_rust_core::size::ISize;

/// Indicates whether a backing store needs to be an exact match or can be larger than is
/// strictly necessary.
// Port of: src/gpu/SkBackingFit.h#L15 (chrome/m156)
#[doc(alias = "SkBackingFit")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackingFit {
    /// Can be larger than strictly necessary.
    #[doc(alias = "kApprox")]
    Approx,
    /// An exact match.
    #[doc(alias = "kExact")]
    Exact,
}

/// `GetApproxSize`: maps dimensions to larger powers of 2. Above a certain tolerance, dimensions
/// can also map to the midpoints between powers of 2.
// Port of: src/gpu/SkBackingFit.cpp#L15-L40 (chrome/m156)
#[doc(alias = "GetApproxSize")]
#[must_use]
pub fn get_approx_size(size: ISize) -> ISize {
    // Map 'value' to a larger multiple of 2. Values <= 'kMagicTol' will pop up to
    // the next power of 2. Those above 'kMagicTol' will only go up half the floor power of 2.
    let adjust = |mut value: i32| -> i32 {
        const MIN_APPROX_SIZE: i32 = 16;
        const MAGIC_TOL: i32 = 1024;

        value = MIN_APPROX_SIZE.max(value);
        if value.count_ones() == 1 {
            return value;
        }

        let ceil_pow2 =
            i32::try_from(value.cast_unsigned().next_power_of_two()).unwrap_or(i32::MIN);
        if value <= MAGIC_TOL {
            return ceil_pow2;
        }

        let floor_pow2 = ceil_pow2 >> 1;
        let mid = floor_pow2 + (floor_pow2 >> 1);
        if value <= mid {
            return mid;
        }
        ceil_pow2
    };

    ISize::new(adjust(size.width), adjust(size.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approx_size() {
        assert_eq!(get_approx_size(ISize::new(1, 15)), ISize::new(16, 16));
        assert_eq!(get_approx_size(ISize::new(17, 33)), ISize::new(32, 64));
        assert_eq!(
            get_approx_size(ISize::new(1024, 1000)),
            ISize::new(1024, 1024)
        );
        // Above the tolerance: the midpoint of 1024 and 2048 is 1536.
        assert_eq!(
            get_approx_size(ISize::new(1025, 1536)),
            ISize::new(1536, 1536)
        );
        assert_eq!(
            get_approx_size(ISize::new(1537, 2048)),
            ISize::new(2048, 2048)
        );
    }
}

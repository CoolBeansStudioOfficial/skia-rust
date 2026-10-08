// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/SkEmbossMask.h, src/effects/SkEmbossMask.cpp

//! `SkEmbossMask`: computes the multiply and additive planes of a 3D emboss mask.

use skia_rust_core::fixed::{Fixed, scalar_to_fixed};
use skia_rust_core::mask::{MaskBuilder, MaskFormat};
use skia_rust_core::math_priv::sqrt32;

use crate::emboss_mask_filter::Light;

// Port of: src/effects/SkEmbossMask.cpp#L22-L29 (chrome/m156)
fn nonzero_to_one(x: i32) -> i32 {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (unsigned) cast
    {
        ((x | x.wrapping_neg()) as u32 >> 31) as i32
    }
}

// Port of: src/effects/SkEmbossMask.cpp#L31-L38 (chrome/m156)
fn neq_to_one(x: i32, max: i32) -> i32 {
    debug_assert!(x >= 0 && x <= max);
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (unsigned) cast
    {
        ((x - max) as u32 >> 31) as i32
    }
}

// Port of: src/effects/SkEmbossMask.cpp#L40-L47 (chrome/m156)
fn neq_to_mask(x: i32, max: i32) -> i32 {
    debug_assert!(x >= 0 && x <= max);
    (x - max) >> 31
}

// Port of: src/effects/SkEmbossMask.cpp#L49-L52 (chrome/m156)
fn div255(x: u32) -> u32 {
    debug_assert!(x <= (255 * 255));
    (x * ((1 << 24) / 255)) >> 24
}

// small enough to show off angle differences
// Port of: src/effects/SkEmbossMask.cpp#L54 (chrome/m156)
const DELTA: i32 = 32;

/// Computes the multiply and additive planes of the 3D `mask` (whose alpha plane is filled in)
/// for `light` (`SkEmbossMask::Emboss`).
///
/// # Panics
/// If `mask` is not a [`MaskFormat::ThreeD`] mask with room for its three planes.
// Port of: src/effects/SkEmbossMask.cpp#L56-L121 (chrome/m156)
#[doc(alias = "SkEmbossMask::Emboss")]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
// mirrors the C++ int/unsigned/SkToU8 conversions of the (bounded) mask arithmetic
pub fn emboss(mask: &mut MaskBuilder, light: &Light) {
    assert_eq!(mask.format, MaskFormat::ThreeD);

    let specular = i32::from(light.specular);
    let ambient = i32::from(light.ambient);
    let lx: Fixed = scalar_to_fixed(light.direction[0]);
    let ly: Fixed = scalar_to_fixed(light.direction[1]);
    let lz: Fixed = scalar_to_fixed(light.direction[2]);
    let lz_dot_nz: Fixed = lz * DELTA;
    let lz_dot8: i32 = lz >> 8;

    let plane_size = mask.compute_image_size();
    let row_bytes = mask.row_bytes as i32;
    let max_y = mask.bounds.height() - 1;
    let max_x = mask.bounds.width() - 1;

    // the planes: alpha, multiply, additive
    let (alpha_plane, rest) = mask.image.split_at_mut(plane_size);
    let (multiply_plane, additive_plane) = rest.split_at_mut(plane_size);

    let mut prev_row: i32 = 0;
    for y in 0..=max_y {
        let next_row: i32 = neq_to_mask(y, max_y) & row_bytes;
        let base = y * row_bytes;

        for x in 0..=max_x {
            let at = |offset: i32| i32::from(alpha_plane[(base + offset) as usize]);
            let nx: i32 = at(x + neq_to_one(x, max_x)) - at(x - nonzero_to_one(x));
            let ny: i32 = at(x + next_row) - at(x - prev_row);

            let numer: Fixed = lx * nx + ly * ny + lz_dot_nz;
            let mut mul: i32 = ambient;
            let mut add: i32 = 0;

            if numer > 0 {
                // preflight when numer/denom will be <= 0
                let denom: i32 = sqrt32(nx * nx + ny * ny + DELTA * DELTA);
                let mut dot: Fixed = numer / denom;
                dot >>= 8; // now dot is 2^8 instead of 2^16
                mul = std::cmp::min(mul + dot, 255);

                // now for the reflection

                //  R = 2 (Light * Normal) Normal - Light
                //  hilite = R * Eye(0, 0, 1)

                let mut hilite: i32 = ((2 * dot - lz_dot8) * lz_dot8) >> 8;
                if hilite > 0 {
                    // pin hilite to 255, since our fast math is also a little sloppy
                    hilite = std::cmp::min(hilite, 255);

                    // specular is 4.4
                    // would really like to compute the fractional part of this
                    // and then possibly cache a 256 table for a given specular
                    // value in the light, and just pass that in to this function.
                    add = hilite;
                    let mut i = specular >> 4;
                    while i > 0 {
                        add = div255((add * hilite) as u32) as i32;
                        i -= 1;
                    }
                }
            }
            multiply_plane[(base + x) as usize] = mul as u8;
            additive_plane[(base + x) as usize] = add as u8;
        }
        prev_row = row_bytes;
    }
}

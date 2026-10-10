// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/DistanceFieldAdjustTable.h, src/text/gpu/DistanceFieldAdjustTable.cpp

//! [`DistanceFieldAdjustTable`]: distance field text needs this table to compute a value for use
//! in the fragment shader.

use std::sync::OnceLock;

use skia_rust_core::scalar::{SCALAR_1, scalar};
use skia_rust_core::scaler_context::{get_gamma_lut_data, get_gamma_lut_size};

/// `SK_GAMMA_CONTRAST` (`SkTypes.h`).
const GAMMA_CONTRAST: scalar = 0.5;
/// `SK_GAMMA_EXPONENT` (`SkTypes.h`): 0 is sRGB.
const GAMMA_EXPONENT: scalar = 0.0;

/// `kExpectedDistanceAdjustTableSize`.
const EXPECTED_DISTANCE_ADJUST_TABLE_SIZE: usize = 8;

/// `kDistanceAdjustLumShift`.
const DISTANCE_ADJUST_LUM_SHIFT: u32 = 5;

/// `build_distance_adjust_table(deviceGamma)`.
///
/// This is used for an approximation of the mask gamma hack, used by raster and bitmap text. The
/// mask gamma hack is based off of guessing what the blend color is going to be, and adjusting the
/// mask so that when run through the linear blend will produce the value closest to the desired
/// result. However, in practice this means that the 'adjusted' mask is just increasing or
/// decreasing the coverage of the mask depending on what it is thought it will blit against. For
/// black (on assumed white) this means that coverages are decreased (on a curve). For white (on
/// assumed black) this means that coverages are increased (on a a curve). At middle (perceptual)
/// gray (which could be blit against anything) the coverages remain the same.
///
/// The idea here is that instead of determining the initial (real) coverage and then adjusting
/// that coverage, we determine an adjusted coverage directly by essentially manipulating the
/// geometry (in this case, the distance to the glyph edge). So for black (on assumed white) this
/// thins a bit; for white (on assumed black) this fake bolds the geometry a bit.
///
/// The distance adjustment is calculated by determining the actual coverage value which when fed
/// into the mask gamma table gives us an 'adjusted coverage' value of 0.5. This actual coverage
/// value (assuming it's between 0 and 1) corresponds to a distance from the actual edge. So by
/// subtracting this distance adjustment and computing without the the coverage adjustment we
/// should get 0.5 coverage at the same point.
// Port of: src/text/gpu/DistanceFieldAdjustTable.cpp#L23-L110 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // small table indices and byte values, as the C++ ints
fn build_distance_adjust_table(device_gamma: scalar) -> Vec<scalar> {
    let contrast = GAMMA_CONTRAST;

    let (width, height, _size) = get_gamma_lut_size();
    debug_assert_eq!(EXPECTED_DISTANCE_ADJUST_TABLE_SIZE, height);
    // The C++ leaves rows without a crossing uninitialized; here they are 0.
    let mut table = vec![0.0; height];

    let Some(data) = get_gamma_lut_data(contrast, device_gamma) else {
        // if no valid data is available simply do no adjustment
        return table;
    };

    // find the inverse points where we cross 0.5
    // binsearch might be better, but we only need to do this once on creation
    for (row, entry) in table.iter_mut().enumerate() {
        let row_data = &data[row * width..];
        for col in 0..width - 1 {
            if row_data[col] <= 127 && row_data[col + 1] >= 128 {
                // compute point where a mask value will give us a result of 0.5
                let a = i32::from(row_data[col]);
                let b = i32::from(row_data[col + 1]);
                let interp = (127.5 - a as f32) / (b - a) as f32;
                let border_alpha = (col as f32 + interp) / 255.0;

                // compute t value for that alpha
                // this is an approximate inverse for smoothstep()
                let t = border_alpha * (border_alpha * (4.0 * border_alpha - 6.0) + 5.0) / 3.0;

                // compute distance which gives us that t value
                const DISTANCE_FIELD_AA_FACTOR: f32 = 0.65; // should match SK_DistanceFieldAAFactor
                let d = 2.0 * DISTANCE_FIELD_AA_FACTOR * t - DISTANCE_FIELD_AA_FACTOR;

                *entry = d;
                break;
            }
        }
    }

    table
}

/// Distance field text needs this table to compute a value for use in the fragment shader
/// (`sktext::gpu::DistanceFieldAdjustTable`).
// Port of: src/text/gpu/DistanceFieldAdjustTable.h#L18-L41 (chrome/m156)
#[doc(alias = "sktext::gpu::DistanceFieldAdjustTable")]
#[derive(Debug)]
pub struct DistanceFieldAdjustTable {
    /// `fTable`.
    table: Vec<scalar>,
    /// `fGammaCorrectTable`.
    gamma_correct_table: Vec<scalar>,
}

impl DistanceFieldAdjustTable {
    /// `DistanceFieldAdjustTable::Get()`: the one table, built on first use.
    // Port of: src/text/gpu/DistanceFieldAdjustTable.cpp#L112-L115 (chrome/m156)
    #[must_use]
    pub fn get() -> &'static DistanceFieldAdjustTable {
        static TABLE: OnceLock<DistanceFieldAdjustTable> = OnceLock::new();
        TABLE.get_or_init(DistanceFieldAdjustTable::new)
    }

    // Port of: src/text/gpu/DistanceFieldAdjustTable.cpp#L117-L120 (chrome/m156)
    fn new() -> Self {
        Self {
            table: build_distance_adjust_table(GAMMA_EXPONENT),
            gamma_correct_table: build_distance_adjust_table(SCALAR_1),
        }
    }

    /// `getAdjustment(lum, useGammaCorrectTable)`.
    // Port of: src/text/gpu/DistanceFieldAdjustTable.h#L28-L31 (chrome/m156)
    #[must_use]
    pub fn get_adjustment(&self, lum: i32, use_gamma_correct_table: bool) -> scalar {
        let lum = (lum >> DISTANCE_ADJUST_LUM_SHIFT) as usize;
        if use_gamma_correct_table {
            self.gamma_correct_table[lum]
        } else {
            self.table[lum]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_one_adjustment_per_luminance_bucket() {
        let table = DistanceFieldAdjustTable::get();
        // The distance is subtracted from the shader's: dark text (low luminance) is thinned
        // (positive adjustment), light text fattened (negative).
        let dark = table.get_adjustment(0, false);
        let light = table.get_adjustment(255, false);
        assert!(dark > 0.0 && light < 0.0, "dark {dark} light {light}");
        // Linear blending needs no adjustment (the linear gamma has no tables).
        assert_eq!(table.get_adjustment(0, true), 0.0);
        assert_eq!(table.get_adjustment(255, true), 0.0);
    }
}

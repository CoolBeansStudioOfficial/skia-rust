// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: fuzz/FuzzCommon.h, fuzz/FuzzCommon.cpp

//! Helpers shared by the fuzzers (`FuzzCommon.h`). Ported as the fuzzers that use them are:
//! `FuzzNicePath`, `FuzzNiceRRect`, `FuzzNiceMatrix` and `FuzzCreateValidInputsForRuntimeEffect`
//! come with the `FuzzCanvas` slice (docs/design/fuzz.md §6).

use skia_rust_core::path::{Path, Verb};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::IRect;
use skia_rust_core::region::{Op as RegionOp, Region};
use skia_rust_core::scalar::scalar;

use crate::Fuzz;

/// `valid_weight`: a conic weight that is positive (1 for anything else, NaN included).
// Port of: fuzz/FuzzCommon.cpp#L34-L39 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // `!(f > 0)`, true for NaN as in C++
fn valid_weight(f: f32) -> f32 {
    if !(f > 0.0) {
        return 1.0;
    }
    f
}

/// A path built from the input until it runs out or picks `kDone_Verb`, with any float values
/// for its points (`FuzzEvilPath`). `last_verb` bounds the verbs it picks.
// Port of: fuzz/FuzzCommon.cpp#L223-L268 (chrome/m156)
#[doc(alias = "FuzzEvilPath")]
#[must_use]
#[allow(clippy::many_single_char_names)] // names follow the C++
pub fn fuzz_evil_path(fuzz: &mut Fuzz<'_>, last_verb: Verb) -> Path {
    let mut builder = PathBuilder::new();
    while !fuzz.exhausted() {
        // Use a uint8_t to conserve bytes.  This makes our "fuzzed bytes footprint"
        // smaller, which leads to more efficient fuzzing.
        let operation: u8 = fuzz.next();

        match i32::from(operation) % (last_verb as i32 + 1) {
            v if v == Verb::Move as i32 => {
                let (a, b): (scalar, scalar) = (fuzz.next(), fuzz.next());
                builder.move_to((a, b));
            }
            v if v == Verb::Line as i32 => {
                let (a, b): (scalar, scalar) = (fuzz.next(), fuzz.next());
                builder.line_to((a, b));
            }
            v if v == Verb::Quad as i32 => {
                let (a, b, c, d): (scalar, scalar, scalar, scalar) =
                    (fuzz.next(), fuzz.next(), fuzz.next(), fuzz.next());
                builder.quad_to((a, b), (c, d));
            }
            v if v == Verb::Conic as i32 => {
                let (a, b, c, d, e): (scalar, scalar, scalar, scalar, scalar) = (
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                );
                builder.conic_to((a, b), (c, d), valid_weight(e));
            }
            v if v == Verb::Cubic as i32 => {
                let (a, b, c, d, e, f): (scalar, scalar, scalar, scalar, scalar, scalar) = (
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                );
                builder.cubic_to((a, b), (c, d), (e, f));
            }
            v if v == Verb::Close as i32 => {
                builder.close();
            }
            v if v == Verb::Done as i32 => {
                // In this case, simply exit.
                return builder.detach();
            }
            _ => {}
        }
    }
    builder.detach()
}

/// `(SkRegion::Op)v` for a value `nextEnum` already clamped to `[0, kLastOp]`.
fn region_op(v: i32) -> RegionOp {
    match v {
        0 => RegionOp::Difference,
        1 => RegionOp::Intersect,
        2 => RegionOp::Union,
        3 => RegionOp::XOR,
        4 => RegionOp::ReverseDifference,
        _ => RegionOp::Replace,
    }
}

/// Ops up to `max_n` fuzzed rects into `region`, stopping at the first op that fails
/// (`FuzzNiceRegion`).
///
/// `SkRegion::Op` is an unscoped enum without a fixed type; its underlying type is `int` on the
/// oracle's (Windows) ABI, so `nextEnum` reads an `i32`.
// Port of: fuzz/FuzzCommon.cpp#L332-L349 (chrome/m156)
#[doc(alias = "FuzzNiceRegion")]
pub fn fuzz_nice_region(fuzz: &mut Fuzz<'_>, region: &mut Region, max_n: i32) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // `(uint8_t)maxN` in
    // `nextRange`: RegionOp's 300 becomes 44, as in C++
    let n: u8 = fuzz.next_range(0, max_n as u8);
    for _ in 0..n {
        // Avoid the sentinel value used by Region.
        let left: i32 = fuzz.next_range(-2_147_483_646, 2_147_483_646);
        let top: i32 = fuzz.next_range(-2_147_483_646, 2_147_483_646);
        let right: i32 = fuzz.next_range(-2_147_483_646, 2_147_483_646);
        let bottom: i32 = fuzz.next_range(-2_147_483_646, 2_147_483_646);
        let mut r = IRect::new(left, top, right, bottom);
        r.sort();
        let op = region_op(fuzz.next_enum(RegionOp::Replace as i32));
        if !region.op_rect(r, op) {
            return;
        }
    }
}

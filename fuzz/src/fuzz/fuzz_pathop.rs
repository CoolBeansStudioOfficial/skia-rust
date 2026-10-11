// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzPathop.cpp (chrome/m156)
//
// `SkPathOp` is an unscoped enum without a fixed type: `nextRange` reads its underlying type,
// which is `int` on the oracle's (Windows) ABI and `unsigned` on the Itanium ABI. We read an
// `i32`, as the oracle does; both read 4 bytes, so only out-of-range values clamp differently.

use skia_rust_core::path::{Path, Verb};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::scalar::scalar;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_pathops::{OpBuilder, as_winding, op, simplify, tight_bounds};

use crate::common::fuzz_evil_path;
use crate::{Fuzz, def_fuzz};

// Port of: fuzz/FuzzPathop.cpp#L17 (chrome/m156)
const MAX_OPS: u8 = 20;

/// `(SkPathOp)v` for `v` in `[0, kReverseDifference_SkPathOp]`.
fn path_op(v: i32) -> PathOp {
    match v {
        0 => PathOp::Difference,
        1 => PathOp::Intersect,
        2 => PathOp::Union,
        3 => PathOp::Xor,
        _ => PathOp::ReverseDifference,
    }
}

/// `nextRange(&ft, 0, kInverseEvenOdd)` on an `SkPathFillType` (underlying `uint8_t`).
fn next_fill_type(fuzz: &mut Fuzz<'_>) -> PathFillType {
    let raw: u8 = fuzz.next_range(0, PathFillType::InverseEvenOdd as u8);
    PathFillType::from_bits(u32::from(raw))
}

/// `nextRange(&op, 0, kReverseDifference_SkPathOp)`.
fn next_path_op(fuzz: &mut Fuzz<'_>) -> PathOp {
    path_op(fuzz.next_range(0, PathOp::ReverseDifference as i32))
}

// Port of: fuzz/FuzzPathop.cpp#L19-L106 (chrome/m156)
def_fuzz!(Pathop, fuzz_Pathop, |fuzz| {
    let choice: u8 = fuzz.next_range(0, 4);
    match choice {
        0 => {
            let ops: u8 = fuzz.next_range(0, MAX_OPS);
            let mut builder = OpBuilder::new();
            let mut i: u8 = 0;
            while i < ops && !fuzz.exhausted() {
                let mut path = fuzz_evil_path(fuzz, Verb::Done);
                let ft = next_fill_type(fuzz);
                path.set_fill_type(ft);

                let op = next_path_op(fuzz);
                builder.add(&path, op);
                i += 1;
            }

            let _result = builder.resolve();
        }
        1 => {
            let path = fuzz_evil_path(fuzz, Verb::Done);
            let ft = next_fill_type(fuzz);
            let _ = simplify(&path.make_fill_type(ft));
        }
        2 => {
            let mut path = fuzz_evil_path(fuzz, Verb::Done);
            let ft = next_fill_type(fuzz);
            path.set_fill_type(ft);

            let path2 = fuzz_evil_path(fuzz, Verb::Done);
            let ft = next_fill_type(fuzz);
            // As in C++, the second fill type also goes to `path`.
            path.set_fill_type(ft);

            let op = next_path_op(fuzz);

            let mut result = Path::new();
            let pick_output: u8 = fuzz.next_range(0, 2);
            if pick_output == 1 {
                result = path.clone();
            } else if pick_output == 2 {
                result = path2.clone();
            }
            if let Some(res) = self::op(&path, &path2, op) {
                result = res;
            }
            drop(result);
        }
        3 => {
            let mut path = fuzz_evil_path(fuzz, Verb::Done);
            let ft = next_fill_type(fuzz);
            path.set_fill_type(ft);

            let mut result = Path::new();
            let is_same = fuzz.next_bool();
            if is_same {
                result = path.clone();
            }
            drop(result);
            let _ = as_winding(&path);
        }
        4 => {
            let mut path = fuzz_evil_path(fuzz, Verb::Done);
            let ft = next_fill_type(fuzz);
            path.set_fill_type(ft);

            let _result = tight_bounds(&path);
        }
        _ => {
            debug_assert!(false);
        }
    }
});

// Port of: fuzz/FuzzPathop.cpp#L109 (chrome/m156)
const LAST_OP: i32 = PathOp::ReverseDifference as i32;

/// `BuildPath`: [`fuzz_evil_path`] that stops (depleting the input) when too few bytes remain for
/// a verb's points.
// Port of: fuzz/FuzzPathop.cpp#L111-L179 (chrome/m156)
#[doc(alias = "BuildPath")]
#[must_use]
#[allow(clippy::many_single_char_names)] // names follow the C++
#[allow(clippy::neg_cmp_op_on_partial_ord)] // `!(f > 0)`, true for NaN as in C++
pub fn build_path(fuzz: &mut Fuzz<'_>) -> Path {
    const SCALAR: usize = size_of::<scalar>();
    let mut builder = PathBuilder::new();
    while !fuzz.exhausted() {
        // Use a uint8_t to conserve bytes.  This makes our "fuzzed bytes footprint"
        // smaller, which leads to more efficient fuzzing.
        let operation: u8 = fuzz.next();

        match i32::from(operation) % (Verb::Done as i32 + 1) {
            v if v == Verb::Move as i32 => {
                if fuzz.remaining_size() < 2 * SCALAR {
                    fuzz.deplete();
                    return builder.detach();
                }
                let (a, b): (scalar, scalar) = (fuzz.next(), fuzz.next());
                builder.move_to((a, b));
            }
            v if v == Verb::Line as i32 => {
                if fuzz.remaining_size() < 2 * SCALAR {
                    fuzz.deplete();
                    return builder.detach();
                }
                let (a, b): (scalar, scalar) = (fuzz.next(), fuzz.next());
                builder.line_to((a, b));
            }
            v if v == Verb::Quad as i32 => {
                if fuzz.remaining_size() < 4 * SCALAR {
                    fuzz.deplete();
                    return builder.detach();
                }
                let (a, b, c, d): (scalar, scalar, scalar, scalar) =
                    (fuzz.next(), fuzz.next(), fuzz.next(), fuzz.next());
                builder.quad_to((a, b), (c, d));
            }
            v if v == Verb::Conic as i32 => {
                if fuzz.remaining_size() < 5 * SCALAR {
                    fuzz.deplete();
                    return builder.detach();
                }
                let (a, b, c, d, mut e): (scalar, scalar, scalar, scalar, scalar) = (
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                    fuzz.next(),
                );
                if !(e > 0.0) {
                    e = 1.0;
                }
                builder.conic_to((a, b), (c, d), e);
            }
            v if v == Verb::Cubic as i32 => {
                if fuzz.remaining_size() < 6 * SCALAR {
                    fuzz.deplete();
                    return builder.detach();
                }
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

// Port of: fuzz/FuzzPathop.cpp#L181-L197 (chrome/m156)
def_fuzz!(LegacyChromiumPathop, fuzz_LegacyChromiumPathop, |fuzz| {
    // See https://cs.chromium.org/chromium/src/testing/libfuzzer/fuzzers/skia_pathop_fuzzer.cc
    let mut builder = OpBuilder::new();
    while !fuzz.exhausted() {
        let op: u8 = fuzz.next();
        if fuzz.exhausted() {
            break;
        }

        let path = build_path(fuzz);
        builder.add(&path, path_op(i32::from(op) % (LAST_OP + 1)));
    }

    let _result = builder.resolve();
});

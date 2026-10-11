// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: fuzz/FuzzCommon.h, fuzz/FuzzCommon.cpp

//! Helpers shared by the fuzzers (`FuzzCommon.h`). Ported as the fuzzers that use them are:
//! `FuzzNicePath`, `FuzzNiceRRect`, `FuzzNiceMatrix` and `FuzzCreateValidInputsForRuntimeEffect`
//! come with the `FuzzCanvas` slice (docs/design/fuzz.md §6).

// The reader's clamped values are converted with `as` as the C++ casts are, and cannot wrap.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::{AddPathMode, Path, Verb};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv::reverse_add_path;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Vector;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op as RegionOp, Region};
use skia_rust_core::rrect::RRect;
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

/// `fuzz_nice_float`: a float that is not NaN and lies within `±1e35`, else `0` (one float read
/// per call).
// Port of: fuzz/FuzzCommon.cpp#L18-L23 (chrome/m156)
fn nice_float(fuzz: &mut Fuzz<'_>) -> f32 {
    /// `kLimit`: `1.0e35f` (the C++ comment says `FLT_MAX?`).
    const LIMIT: f32 = 1.0e35;
    let v: f32 = fuzz.next();
    if !v.is_nan() && (-LIMIT..=LIMIT).contains(&v) {
        v
    } else {
        0.0
    }
}

/// `fuzz_nice_float(fuzz, &a, &b, …)`: `N` nice floats, read in order.
fn nice_floats<const N: usize>(fuzz: &mut Fuzz<'_>) -> [f32; N] {
    std::array::from_fn(|_| nice_float(fuzz))
}

/// `fuzz_nice_rect`: four nice floats (left, top, right, bottom), sorted.
// Port of: fuzz/FuzzCommon.cpp#L31-L34 (chrome/m156)
fn nice_rect(fuzz: &mut Fuzz<'_>) -> Rect {
    let [l, t, r, b] = nice_floats::<4>(fuzz);
    let mut rect = Rect::new(l, t, r, b);
    rect.sort();
    rect
}

/// `SkPathFillType` from the `uint8_t` `FuzzNicePath` reads (`0..=3`).
fn path_fill_type(v: u8) -> PathFillType {
    match v {
        0 => PathFillType::Winding,
        1 => PathFillType::EvenOdd,
        2 => PathFillType::InverseWinding,
        _ => PathFillType::InverseEvenOdd,
    }
}

/// `SkPathDirection` from the `unsigned` value `FuzzNicePath` reads (`0` or `1`).
fn path_direction(v: u32) -> PathDirection {
    if v == 0 {
        PathDirection::CW
    } else {
        PathDirection::CCW
    }
}

/// `SkPath::AddPathMode` from the `unsigned` value `FuzzNicePath` reads (`0` or `1`).
fn add_path_mode(v: u32) -> AddPathMode {
    if v == 0 {
        AddPathMode::Append
    } else {
        AddPathMode::Extend
    }
}

/// `FuzzNicePath`: a path of at most `max_ops` operations, built until the input runs out or the
/// path is too big. Allows some float values for path points.
// Port of: fuzz/FuzzCommon.cpp#L44-L246 (chrome/m156)
#[doc(alias = "FuzzNicePath")]
#[must_use]
// One arm per C++ `case`, as in the port; the names `a` to `f` are the C++ ones.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
pub fn fuzz_nice_path(fuzz: &mut Fuzz<'_>, max_ops: i32) -> Path {
    /// How many items in the switch statement below.
    const MAX_PATH_OPERATION: u8 = 31;
    let mut builder = PathBuilder::new();
    if max_ops <= 0 || fuzz.exhausted() {
        return Path::new();
    }
    let fill_type: u8 = fuzz.next_range(0, 3);
    builder.set_fill_type(path_fill_type(fill_type));
    let num_ops: u8 = fuzz.next_range(0, max_ops as u8);
    for _ in 0..num_ops {
        // When we start adding the path to itself, the fuzzer can make an exponentially long
        // path, which causes timeouts.
        if builder.count_points() > 100_000 {
            return Path::new();
        }
        let op: u8 = fuzz.next_range(0, MAX_PATH_OPERATION);
        match op {
            0 => {
                let [a, b] = nice_floats(fuzz);
                builder.move_to((a, b));
            }
            1 => {
                let [a, b] = nice_floats(fuzz);
                builder.r_move_to((a, b));
            }
            2 => {
                let [a, b] = nice_floats(fuzz);
                builder.line_to((a, b));
            }
            3 => {
                let [a, b] = nice_floats(fuzz);
                builder.r_line_to((a, b));
            }
            4 => {
                let [a, b, c, d] = nice_floats(fuzz);
                builder.quad_to((a, b), (c, d));
            }
            5 => {
                let [a, b, c, d] = nice_floats(fuzz);
                builder.r_quad_to((a, b), (c, d));
            }
            6 => {
                let [a, b, c, d, e] = nice_floats(fuzz);
                builder.conic_to((a, b), (c, d), valid_weight(e));
            }
            7 => {
                let [a, b, c, d, e] = nice_floats(fuzz);
                builder.r_conic_to((a, b), (c, d), valid_weight(e));
            }
            8 => {
                let [a, b, c, d, e, f] = nice_floats(fuzz);
                builder.cubic_to((a, b), (c, d), (e, f));
            }
            9 => {
                let [a, b, c, d, e, f] = nice_floats(fuzz);
                builder.r_cubic_to((a, b), (c, d), (e, f));
            }
            10 => {
                let [a, b, c, d, e] = nice_floats(fuzz);
                builder.arc_to_tangent((a, b), (c, d), e);
            }
            11 => {
                let [a, b] = nice_floats(fuzz);
                let r = nice_rect(fuzz);
                let test = fuzz.next_bool();
                builder.arc_to(r, a, b, test);
            }
            12 => {
                builder.close();
            }
            13 => {
                let r = nice_rect(fuzz);
                let ui: u32 = fuzz.next_range(0, 1);
                builder.add_rect(r, path_direction(ui), None);
            }
            14 => {
                let ui: u32 = fuzz.next_range(0, 1);
                let dir = path_direction(ui);
                let r = nice_rect(fuzz);
                let start: u32 = fuzz.next();
                builder.add_rect(r, dir, Some(start as usize));
            }
            15 => {
                let ui: u32 = fuzz.next_range(0, 1);
                let r = nice_rect(fuzz);
                builder.add_oval(r, path_direction(ui), None);
            }
            16 => {
                let ui: u32 = fuzz.next_range(0, 1);
                let dir = path_direction(ui);
                let r = nice_rect(fuzz);
                let start: u32 = fuzz.next();
                builder.add_oval(r, dir, Some(start as usize));
            }
            17 => {
                let ui: u32 = fuzz.next_range(0, 1);
                let [a, b, c] = nice_floats(fuzz);
                builder.add_circle((a, b), c, path_direction(ui));
            }
            18 => {
                let r = nice_rect(fuzz);
                let [a, b] = nice_floats(fuzz);
                builder.add_arc(r, a, b);
            }
            19 => {
                let [a, b] = nice_floats(fuzz);
                let r = nice_rect(fuzz);
                let ui: u32 = fuzz.next_range(0, 1);
                builder.add_rrect(RRect::new_rect_xy(r, a, b), path_direction(ui), None);
            }
            20 => {
                let rr = nice_rrect(fuzz);
                let ui: u32 = fuzz.next_range(0, 1);
                builder.add_rrect(rr, path_direction(ui), None);
            }
            21 => {
                let ui: u32 = fuzz.next_range(0, 1);
                let dir = path_direction(ui);
                let rr = nice_rrect(fuzz);
                builder.add_rrect(rr, dir, Some(ui as usize));
            }
            22 => {
                let ui: u32 = fuzz.next_range(0, 1);
                let mode = add_path_mode(ui);
                let m = nice_matrix(fuzz);
                let p = fuzz_nice_path(fuzz, max_ops - 1);
                builder.add_path_with_transform(&p, &m, mode);
            }
            23 => {
                let ui: u32 = fuzz.next_range(0, 1);
                let mode = add_path_mode(ui);
                let m = nice_matrix(fuzz);
                let snapshot = builder.snapshot();
                builder.add_path_with_transform(&snapshot, &m, mode);
            }
            24 => {
                let p = fuzz_nice_path(fuzz, max_ops - 1);
                reverse_add_path(&mut builder, &p);
            }
            25 => {
                let snapshot = builder.snapshot();
                builder.add_path(&snapshot, None);
            }
            26 => {
                let snapshot = builder.snapshot();
                reverse_add_path(&mut builder, &snapshot);
            }
            27 => {
                let [a, b] = nice_floats(fuzz);
                builder.offset((a, b));
            }
            28 => {
                let p = fuzz_nice_path(fuzz, max_ops - 1);
                let [a, b] = nice_floats(fuzz);
                builder.assign_path(&p);
                builder.offset((a, b));
            }
            29 => {
                let m = nice_matrix(fuzz);
                builder.transform(&m);
            }
            30 => {
                let p = fuzz_nice_path(fuzz, max_ops - 1);
                // transform can explode path sizes so skip this op if p too big
                if p.count_points() <= 100_000 {
                    let m = nice_matrix(fuzz);
                    builder.assign_path(&p);
                    builder.transform(&m);
                }
            }
            31 => {
                let [a, b] = nice_floats(fuzz);
                builder.set_last_point((a, b));
            }
            // `default: SkASSERT(false)`: unreachable, `op` is clamped to `0..=31`.
            _ => {}
        }
    }
    let rv = builder.detach();
    if rv.is_valid() {
        return rv;
    }
    Path::new()
}

/// `FuzzNiceRRect`: a rect of nice floats with four radii, each scaled into half the rect.
// Port of: fuzz/FuzzCommon.cpp#L295-L308 (chrome/m156)
#[doc(alias = "FuzzNiceRRect")]
#[must_use]
pub fn nice_rrect(fuzz: &mut Fuzz<'_>) -> RRect {
    let r = nice_rect(fuzz);
    let mut radii = [Vector::new(0.0, 0.0); 4];
    for vec in &mut radii {
        vec.x = fuzz.next_range_f32(0.0, 1.0);
        vec.x *= 0.5 * r.width();
        vec.y = fuzz.next_range_f32(0.0, 1.0);
        vec.y *= 0.5 * r.height();
    }
    let mut rr = RRect::new();
    rr.set_rect_radii(r, &radii);
    // `SkASSERT(rr->isValid())`: debug-only in Skia, so not checked here.
    rr
}

/// `FuzzNiceMatrix`: an identity, a translation, a scale with a translation, an affine matrix or
/// a perspective matrix, with bounded values for the first three.
// Port of: fuzz/FuzzCommon.cpp#L310-L344 (chrome/m156)
#[doc(alias = "FuzzNiceMatrix")]
#[must_use]
pub fn nice_matrix(fuzz: &mut Fuzz<'_>) -> Matrix {
    let matrix_type: i32 = fuzz.next_range(0, 4);
    let mut buffer = [0.0f32; 9];
    match matrix_type {
        // identity
        0 => Matrix::new_identity(),
        // translate
        1 => {
            buffer[0] = fuzz.next_range_f32(-4000.0, 4000.0);
            buffer[1] = fuzz.next_range_f32(-4000.0, 4000.0);
            Matrix::translate((buffer[0], buffer[1]))
        }
        // translate + scale
        2 => {
            buffer[0] = fuzz.next_range_f32(-400.0, 400.0);
            buffer[1] = fuzz.next_range_f32(-400.0, 400.0);
            buffer[2] = fuzz.next_range_f32(-4000.0, 4000.0);
            buffer[3] = fuzz.next_range_f32(-4000.0, 4000.0);
            let mut m = Matrix::scale((buffer[0], buffer[1]));
            m.post_translate((buffer[2], buffer[3]));
            m
        }
        // affine
        3 => {
            let six: [f32; 6] = std::array::from_fn(|_| fuzz.next());
            Matrix::from_affine(&six)
        }
        // perspective
        _ => {
            for v in &mut buffer {
                *v = fuzz.next();
            }
            let mut m = Matrix::new_identity();
            m.set_all(
                buffer[0], buffer[1], buffer[2], buffer[3], buffer[4], buffer[5], buffer[6],
                buffer[7], buffer[8],
            );
            m
        }
    }
}

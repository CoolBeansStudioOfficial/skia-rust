// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsBuilderConicTest.cpp (chrome/m156)

// The float literals below are Skia's test inputs (SkBits2Float bit patterns and decimal values),
// copied as written. `DEFINE_int` flags are fixed at their defaults (processOffset 0,
// processCount 1, trialRuns 100).
#![allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    // `0.707107f` is Skia's literal for 1/sqrt(2) at 6 digits; it is kept as written.
    clippy::approx_constant,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::similar_names,
    clippy::many_single_char_names
)]
#![cfg(test)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op as RegionOp, Region};
use skia_rust_pathops::op;
use skia_rust_pathops::op_builder::OpBuilder;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_raster::region_path::RegionExt;

use crate::unit::path_ops_extended_test::{compare_paths, test_simplify};
use crate::{Reporter, def_test};

/// `struct OvalSet`.
// Port of: tests/PathOpsBuilderConicTest.cpp#L24-L31 (chrome/m156)
struct OvalSet {
    bounds: Rect,
    columns: i32,
    rows: i32,
    rotations: i32,
    x_spacing: f32,
    y_spacing: f32,
}

/// Which accumulator `testOvalSet` writes to: an `SkOpBuilder`, an `SkRegion`, or `Op` calls.
enum OvalSink<'a> {
    Builder(&'a mut OpBuilder),
    Region(&'a mut Region),
    Op,
}

// Port of: tests/PathOpsBuilderConicTest.cpp#L33-L65 (chrome/m156)
fn test_oval_set(set: &OvalSet, oval: &Path, mut sink: OvalSink<'_>, result: &mut Path) {
    for x in 0..set.columns {
        for y in 0..set.rows {
            // `for (SkScalar r = 0; r < 360; r += 360.f / set.fRotations)`
            let step = 360.0f32 / set.rotations as f32;
            let mut r = 0.0f32;
            while r < 360.0 {
                let mut matrix = Matrix::default();
                matrix.reset();
                matrix.post_rotate(r, Point::new(0.0, 0.0));
                matrix.post_translate((x as f32 * set.x_spacing, y as f32 * set.y_spacing));
                let rotated = oval.make_transform(&matrix);
                match &mut sink {
                    OvalSink::Builder(builder) => builder.add(&rotated, PathOp::Union),
                    OvalSink::Region(region) => {
                        let open_clip = Region::from_rect(IRect::new(-16000, -16000, 16000, 16000));
                        let mut rgn_b = Region::new();
                        rgn_b.set_path(&rotated, &open_clip);
                        region.op_region(&rgn_b, RegionOp::Union);
                    }
                    OvalSink::Op => {
                        if let Some(res) = op(result, &rotated, PathOp::Union) {
                            *result = res;
                        }
                    }
                }
                r += step;
            }
        }
    }
    match sink {
        OvalSink::Builder(builder) => {
            if let Some(res) = builder.resolve() {
                *result = res;
            }
        }
        OvalSink::Region(region) => {
            *result = region.boundary_path();
        }
        OvalSink::Op => {}
    }
}

// Port of: tests/PathOpsBuilderConicTest.cpp#L67-L78 (chrome/m156)
fn test_one(reporter: &mut Reporter, set: &OvalSet) {
    let mut region_result = PathBuilder::new().detach();
    let mut builder_result = PathBuilder::new().detach();
    let mut op_result = PathBuilder::new().detach();
    let oval = Path::oval(set.bounds, None);
    let mut builder = OpBuilder::new();
    let mut region = Region::new();
    test_oval_set(
        set,
        &oval,
        OvalSink::Region(&mut region),
        &mut region_result,
    );
    test_oval_set(
        set,
        &oval,
        OvalSink::Builder(&mut builder),
        &mut builder_result,
    );
    test_oval_set(set, &oval, OvalSink::Op, &mut op_result);
    // comparePaths' result is not checked by the C++ test.
    let _ = compare_paths(&region_result, &builder_result);
    let _ = compare_paths(&region_result, &op_result);
    let _ = reporter;
}

/// `struct OvalSetOneOff`.
// Port of: tests/PathOpsBuilderConicTest.cpp#L80-L83 (chrome/m156)
struct OvalSetOneOff {
    col: i32,
    row: i32,
    rot: i32,
    trial: i32,
}

// Port of: tests/PathOpsBuilderConicTest.cpp#L85-L88 (chrome/m156)
const ONE_OFFS: [OvalSetOneOff; 2] = [
    OvalSetOneOff {
        col: 2,
        row: 2,
        rot: 9,
        trial: 73,
    },
    OvalSetOneOff {
        col: 1,
        row: 2,
        rot: 7,
        trial: 93,
    },
];

// Port of: tests/PathOpsBuilderConicTest.cpp#L90-L100 (chrome/m156)
fn setup_one(reporter: &mut Reporter, col: i32, row: i32, rot: i32, trial: i32) {
    let scale = 10;
    let mut r = Random::default();
    r.set_seed((col * 100_000_000 + row * 10_000_000 + rot * 1_000_000 + trial).cast_unsigned());
    let x_offset = r.next_range_scalar(1.0, 40.0) * scale as f32;
    let y_offset = r.next_range_scalar(1.0, 100.0) * scale as f32;
    let mut set = OvalSet {
        bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        columns: col,
        rows: row,
        rotations: rot,
        x_spacing: x_offset,
        y_spacing: y_offset,
    };
    // Arguments are evaluated left to right here; the C++ order is unspecified.
    let left = 5.0;
    let top = 5.0;
    let width = r.next_range_scalar(5.0, 50.0) * scale as f32;
    let height = r.next_range_scalar(50.0, 90.0) * scale as f32;
    set.bounds = Rect::new(left, top, left + width, top + height);
    test_one(reporter, &set);
}

// Port of: tests/PathOpsBuilderConicTest.cpp#L110-L132 (chrome/m156)
// `processOffset` 0, `processCount` 1, `trialRuns` 100 (the DEFINE_int defaults).
def_test!(SixtyOvals, |reporter| {
    let skip_one_offs = false;
    let mut trial_runs: i32 = 100;
    for col in 1..=2 {
        for row in 1..=3 {
            for rot in 2..=9 {
                // `for (int trial = processOffset * trialRuns; --trialRuns >= 0; ++trial)`: the
                // condition decrements the shared `trialRuns`, so only the first (col, row, rot)
                // combination runs its trials, as in Skia.
                let mut trial = 0;
                loop {
                    trial_runs -= 1;
                    if trial_runs < 0 {
                        break;
                    }
                    if skip_one_offs
                        && ONE_OFFS.iter().any(|o| {
                            col == o.col && row == o.row && rot == o.rot && trial == o.trial
                        })
                    {
                        trial += 1;
                        continue;
                    }
                    setup_one(reporter, col, row, rot, trial);
                    trial += 1;
                }
            }
        }
    }
});

// Port of: tests/PathOpsBuilderConicTest.cpp#L134-L138 (chrome/m156)
def_test!(SixtyOvalsOneOff, |reporter| {
    for one_off in &ONE_OFFS {
        setup_one(
            reporter,
            one_off.col,
            one_off.row,
            one_off.rot,
            one_off.trial,
        );
    }
});

// Port of: tests/PathOpsBuilderConicTest.cpp#L140-L361 (chrome/m156)
def_test!(SixtyOvals_2_2_9_73, |reporter| {
    let path = PathBuilder::new()
        .move_to((f32::from_bits(0x434d53ca), f32::from_bits(0x43ad6ab0)))
        .conic_to(
            (f32::from_bits(0x434d53ca), f32::from_bits(0x40a00000)),
            (f32::from_bits(0x42d253ca), f32::from_bits(0x40a00000)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40a00000), f32::from_bits(0x40a00000)),
            (f32::from_bits(0x40a00000), f32::from_bits(0x43ad6ab0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40a00000), f32::from_bits(0x442c2ab0)),
            (f32::from_bits(0x42d253ca), f32::from_bits(0x442c2ab0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x434d53ca), f32::from_bits(0x442c2ab0)),
            (f32::from_bits(0x434d53ca), f32::from_bits(0x43ad6ab0)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc2834d04), f32::from_bits(0x43c6d5fb)))
        .conic_to(
            (f32::from_bits(0x431a136e), f32::from_bits(0x4307cfe3)),
            (f32::from_bits(0x429ab133), f32::from_bits(0x428edb31)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x3f1dc4d0), f32::from_bits(0x40e169c2)),
            (f32::from_bits(0xc35b1c2c), f32::from_bits(0x438673b0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3db6b0e), f32::from_bits(0x4404b0dc)),
            (f32::from_bits(0xc3b50da4), f32::from_bits(0x4414c96f)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc38eb03a), f32::from_bits(0x4424e202)),
            (f32::from_bits(0xc2834d04), f32::from_bits(0x43c6d5fb)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc398f46d), f32::from_bits(0x438337ac)))
        .conic_to(
            (f32::from_bits(0x41f5d870), f32::from_bits(0x434b137f)),
            (f32::from_bits(0x41556629), f32::from_bits(0x42d0de52)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc081c918), f32::from_bits(0x40b95a5c)),
            (f32::from_bits(0xc3aa5918), f32::from_bits(0x42824d58)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc4295587), f32::from_bits(0x42f9050a)),
            (f32::from_bits(0xc424fc5c), f32::from_bits(0x435f26db)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc420a331), f32::from_bits(0x43a0e598)),
            (f32::from_bits(0xc398f46d), f32::from_bits(0x438337ac)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc3c983e0), f32::from_bits(0x408cdc40)))
        .conic_to(
            (f32::from_bits(0xc2d5fcd2), f32::from_bits(0x432f5193)),
            (f32::from_bits(0xc263a5d9), f32::from_bits(0x42b12617)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc0da9066), f32::from_bits(0x3fea4196)),
            (f32::from_bits(0xc3976eed), f32::from_bits(0xc329162e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc415b9cc), f32::from_bits(0xc3aa006f)),
            (f32::from_bits(0xc4223f09), f32::from_bits(0xc37d4256)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc42ec446), f32::from_bits(0xc32683cf)),
            (f32::from_bits(0xc3c983e0), f32::from_bits(0x408cdc40)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc39bc8c8), f32::from_bits(0xc37fb0d7)))
        .conic_to(
            (f32::from_bits(0xc342a797), f32::from_bits(0x42830e25)),
            (f32::from_bits(0xc2c9102e), f32::from_bits(0x41fa2834)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc0cd12f5), f32::from_bits(0xc03f4152)),
            (f32::from_bits(0xc2f6a523), f32::from_bits(0xc3a21a77)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3703c8a), f32::from_bits(0xc4215b37)),
            (f32::from_bits(0xc3a72e05), f32::from_bits(0xc418cab4)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3d63dc5), f32::from_bits(0xc4103a31)),
            (f32::from_bits(0xc39bc8c8), f32::from_bits(0xc37fb0d7)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc294a419), f32::from_bits(0xc3c6124c)))
        .conic_to(
            (f32::from_bits(0xc33f3c05), f32::from_bits(0xc295d95d)),
            (f32::from_bits(0xc2c2390a), f32::from_bits(0xc222aa8c)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc03f4154), f32::from_bits(0xc0cd12f4)),
            (f32::from_bits(0x42e3d9e6), f32::from_bits(0xc3a3d041)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4366d6ec), f32::from_bits(0xc422361b)),
            (f32::from_bits(0x4308b76c), f32::from_bits(0xc42ac69e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x422a5fb0), f32::from_bits(0xc4335721)),
            (f32::from_bits(0xc294a419), f32::from_bits(0xc3c6124c)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x4345b3f8), f32::from_bits(0xc3af9e21)))
        .conic_to(
            (f32::from_bits(0xc2c4aac2), f32::from_bits(0xc3345194)),
            (f32::from_bits(0xc24101bb), f32::from_bits(0xc2bb2617)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x3fea41a0), f32::from_bits(0xc0da9066)),
            (f32::from_bits(0x4394eeee), f32::from_bits(0xc331bf31)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x441479cd), f32::from_bits(0xc3ae54f0)),
            (f32::from_bits(0x4407f490), f32::from_bits(0xc3d9b434)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43f6dea8), f32::from_bits(0xc40289bc)),
            (f32::from_bits(0x4345b3f8), f32::from_bits(0xc3af9e21)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43bc9c08), f32::from_bits(0xc30dfb1e)))
        .conic_to(
            (f32::from_bits(0x422250a2), f32::from_bits(0xc34956f5)),
            (f32::from_bits(0x41b97bee), f32::from_bits(0xc2cd653e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40b95a5b), f32::from_bits(0xc081c919)),
            (f32::from_bits(0x43ab375e), f32::from_bits(0x425d363a)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4429c4a9), f32::from_bits(0x42e552cb)),
            (f32::from_bits(0x442e1dd4), f32::from_bits(0x4180287c)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x443276ff), f32::from_bits(0xc2a53e8d)),
            (f32::from_bits(0x43bc9c08), f32::from_bits(0xc30dfb1e)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43be1d75), f32::from_bits(0x4305b53c)))
        .conic_to(
            (f32::from_bits(0x432080f6), f32::from_bits(0xc30026d3)),
            (f32::from_bits(0x42a78c44), f32::from_bits(0xc27f121c)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40e169c3), f32::from_bits(0x3f1dc4b8)),
            (f32::from_bits(0x4362c542), f32::from_bits(0x43833cea)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43df3f9c), f32::from_bits(0x44031579)),
            (f32::from_bits(0x4402ce83), f32::from_bits(0x43e5f9cc)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4415fd38), f32::from_bits(0x43c5c8a6)),
            (f32::from_bits(0x43be1d75), f32::from_bits(0x4305b53c)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x434d53ca), f32::from_bits(0x44487cfb)))
        .conic_to(
            (f32::from_bits(0x434d53ca), f32::from_bits(0x43e60f46)),
            (f32::from_bits(0x42d253ca), f32::from_bits(0x43e60f46)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40a00000), f32::from_bits(0x43e60f46)),
            (f32::from_bits(0x40a00000), f32::from_bits(0x44487cfb)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40a00000), f32::from_bits(0x448ef92a)),
            (f32::from_bits(0x42d253ca), f32::from_bits(0x448ef92a)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x434d53ca), f32::from_bits(0x448ef92a)),
            (f32::from_bits(0x434d53ca), f32::from_bits(0x44487cfb)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc2834d04), f32::from_bits(0x445532a0)))
        .conic_to(
            (f32::from_bits(0x431a136e), f32::from_bits(0x4413bb9c)),
            (f32::from_bits(0x429ab133), f32::from_bits(0x4403a309)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x3f1dc4d0), f32::from_bits(0x43e714ed)),
            (f32::from_bits(0xc35b1c2c), f32::from_bits(0x4435017b)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3db6b0e), f32::from_bits(0x4476787f)),
            (f32::from_bits(0xc3b50da4), f32::from_bits(0x44834889)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc38eb03a), f32::from_bits(0x448b54d2)),
            (f32::from_bits(0xc2834d04), f32::from_bits(0x445532a0)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc398f46d), f32::from_bits(0x44336379)))
        .conic_to(
            (f32::from_bits(0x41f5d870), f32::from_bits(0x44248c83)),
            (f32::from_bits(0x41556629), f32::from_bits(0x440be36d)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc081c918), f32::from_bits(0x43e674af)),
            (f32::from_bits(0xc3aa5918), f32::from_bits(0x4402114e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc4295587), f32::from_bits(0x4410e844)),
            (f32::from_bits(0xc424fc5c), f32::from_bits(0x4429915a)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc420a331), f32::from_bits(0x44423a6f)),
            (f32::from_bits(0xc398f46d), f32::from_bits(0x44336379)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc3c983e0), f32::from_bits(0x43e5c2b7)))
        .conic_to(
            (f32::from_bits(0xc2d5fcd2), f32::from_bits(0x441d9c08)),
            (f32::from_bits(0xc263a5d9), f32::from_bits(0x4407ec66)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc0da9066), f32::from_bits(0x43e47988)),
            (f32::from_bits(0xc3976eed), f32::from_bits(0x438f042f)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc415b9cc), f32::from_bits(0x42e63b5c)),
            (f32::from_bits(0xc4223f09), f32::from_bits(0x4349dc36)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc42ec446), f32::from_bits(0x43904d5e)),
            (f32::from_bits(0xc3c983e0), f32::from_bits(0x43e5c2b7)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc39bc8c8), f32::from_bits(0x43476db5)))
        .conic_to(
            (f32::from_bits(0xc342a797), f32::from_bits(0x44022968)),
            (f32::from_bits(0xc2c9102e), f32::from_bits(0x43f331c9)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc0cd12f5), f32::from_bits(0x43e210c3)),
            (f32::from_bits(0xc2f6a523), f32::from_bits(0x4302e99e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3703c8a), f32::from_bits(0xc33e4e50)),
            (f32::from_bits(0xc3a72e05), f32::from_bits(0xc31c0c44)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3d63dc5), f32::from_bits(0xc2f39470)),
            (f32::from_bits(0xc39bc8c8), f32::from_bits(0x43476db5)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc294a419), f32::from_bits(0x426be7d0)))
        .conic_to(
            (f32::from_bits(0xc33f3c05), f32::from_bits(0x43be18ef)),
            (f32::from_bits(0xc2c2390a), f32::from_bits(0x43cf39f4)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc03f4154), f32::from_bits(0x43e05afa)),
            (f32::from_bits(0x42e3d9e6), f32::from_bits(0x42fefc14)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4366d6ec), f32::from_bits(0xc341b9e0)),
            (f32::from_bits(0x4308b76c), f32::from_bits(0xc363fbec)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x422a5fb0), f32::from_bits(0xc3831efc)),
            (f32::from_bits(0xc294a419), f32::from_bits(0x426be7d0)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x4345b3f8), f32::from_bits(0x42cfc494)))
        .conic_to(
            (f32::from_bits(0xc2c4aac2), f32::from_bits(0x4389667c)),
            (f32::from_bits(0xc24101bb), f32::from_bits(0x43b4c5c0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x3fea41a0), f32::from_bits(0x43e02504)),
            (f32::from_bits(0x4394eeee), f32::from_bits(0x438aafae)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x441479cd), f32::from_bits(0x42d4e958)),
            (f32::from_bits(0x4407f490), f32::from_bits(0x419db120)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43f6dea8), f32::from_bits(0xc28610c8)),
            (f32::from_bits(0x4345b3f8), f32::from_bits(0x42cfc494)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43bc9c08), f32::from_bits(0x439c91b7)))
        .conic_to(
            (f32::from_bits(0x422250a2), f32::from_bits(0x437dc797)),
            (f32::from_bits(0x41b97bee), f32::from_bits(0x43b035f6)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40b95a5b), f32::from_bits(0x43e18822)),
            (f32::from_bits(0x43ab375e), f32::from_bits(0x43ff360d)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4429c4a9), f32::from_bits(0x440e71fc)),
            (f32::from_bits(0x442e1dd4), f32::from_bits(0x43eb91ce)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x443276ff), f32::from_bits(0x43ba3fa3)),
            (f32::from_bits(0x43bc9c08), f32::from_bits(0x439c91b7)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43be1d75), f32::from_bits(0x441334f2)))
        .conic_to(
            (f32::from_bits(0x432080f6), f32::from_bits(0x43a37bdc)),
            (f32::from_bits(0x42a78c44), f32::from_bits(0x43c3ad02)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x40e169c3), f32::from_bits(0x43e3de28)),
            (f32::from_bits(0x4362c542), f32::from_bits(0x44336618)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43df3f9c), f32::from_bits(0x4474dd1c)),
            (f32::from_bits(0x4402ce83), f32::from_bits(0x4464c489)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4415fd38), f32::from_bits(0x4454abf6)),
            (f32::from_bits(0x43be1d75), f32::from_bits(0x441334f2)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43bb9978), f32::from_bits(0x43ad6ab0)))
        .conic_to(
            (f32::from_bits(0x43bb9978), f32::from_bits(0x40a00000)),
            (f32::from_bits(0x43898486), f32::from_bits(0x40a00000)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432edf26), f32::from_bits(0x40a00000)),
            (f32::from_bits(0x432edf26), f32::from_bits(0x43ad6ab0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432edf26), f32::from_bits(0x442c2ab0)),
            (f32::from_bits(0x43898486), f32::from_bits(0x442c2ab0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43bb9978), f32::from_bits(0x442c2ab0)),
            (f32::from_bits(0x43bb9978), f32::from_bits(0x43ad6ab0)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x42d07148), f32::from_bits(0x43c6d5fb)))
        .conic_to(
            (f32::from_bits(0x43a1f94a), f32::from_bits(0x4307cfe3)),
            (f32::from_bits(0x437737c0), f32::from_bits(0x428edb31)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432a7ceb), f32::from_bits(0x40e169c2)),
            (f32::from_bits(0xc244f418), f32::from_bits(0x438673b0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3867b7b), f32::from_bits(0x4404b0dc)),
            (f32::from_bits(0xc3403c22), f32::from_bits(0x4414c96f)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc2e7029c), f32::from_bits(0x4424e202)),
            (f32::from_bits(0x42d07148), f32::from_bits(0x43c6d5fb)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc30809b4), f32::from_bits(0x438337ac)))
        .conic_to(
            (f32::from_bits(0x43489a34), f32::from_bits(0x434b137f)),
            (f32::from_bits(0x43373589), f32::from_bits(0x42d0de52)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4325d0dd), f32::from_bits(0x40b95a5c)),
            (f32::from_bits(0xc32ad30a), f32::from_bits(0x42824d58)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3fdbb7b), f32::from_bits(0x42f9050a)),
            (f32::from_bits(0xc3f50925), f32::from_bits(0x435f26db)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3ec56cf), f32::from_bits(0x43a0e598)),
            (f32::from_bits(0xc30809b4), f32::from_bits(0x438337ac)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc369289a), f32::from_bits(0x408cdc40)))
        .conic_to(
            (f32::from_bits(0x427b82f4), f32::from_bits(0x432f5193)),
            (f32::from_bits(0x42e1eb60), f32::from_bits(0x42b12617)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43230aa3), f32::from_bits(0x3fea4196)),
            (f32::from_bits(0xc304feb4), f32::from_bits(0xc329162e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3d68405), f32::from_bits(0xc3aa006f)),
            (f32::from_bits(0xc3ef8e7f), f32::from_bits(0xc37d4256)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc4044c7c), f32::from_bits(0xc32683cf)),
            (f32::from_bits(0xc369289a), f32::from_bits(0x408cdc40)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc30db26a), f32::from_bits(0xc37fb0d7)))
        .conic_to(
            (f32::from_bits(0xc1c64388), f32::from_bits(0x42830e25)),
            (f32::from_bits(0x428aae1e), f32::from_bits(0x41fa2834)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4323768e), f32::from_bits(0xc03f4152)),
            (f32::from_bits(0x423a3252), f32::from_bits(0xc3a21a77)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc28cbac8), f32::from_bits(0xc4215b37)),
            (f32::from_bits(0xc3247ce4), f32::from_bits(0xc418cab4)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3814e32), f32::from_bits(0xc4103a31)),
            (f32::from_bits(0xc30db26a), f32::from_bits(0xc37fb0d7)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x42bf1a33), f32::from_bits(0xc3c6124c)))
        .conic_to(
            (f32::from_bits(0xc1aae6f8), f32::from_bits(0xc295d95d)),
            (f32::from_bits(0x42918542), f32::from_bits(0xc222aa8c)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4326e221), f32::from_bits(0xc0cd12f4)),
            (f32::from_bits(0x438de60c), f32::from_bits(0xc3a3d041)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43c85b09), f32::from_bits(0xc422361b)),
            (f32::from_bits(0x43994b49), f32::from_bits(0xc42ac69e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43547712), f32::from_bits(0xc4335721)),
            (f32::from_bits(0x42bf1a33), f32::from_bits(0xc3c6124c)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43b7c98f), f32::from_bits(0xc3af9e21)))
        .conic_to(
            (f32::from_bits(0x428f138a), f32::from_bits(0xc3345194)),
            (f32::from_bits(0x42f33d6e), f32::from_bits(0xc2bb2617)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432bb3a9), f32::from_bits(0xc0da9066)),
            (f32::from_bits(0x43e9de81), f32::from_bits(0xc331bf31)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x443ef196), f32::from_bits(0xc3ae54f0)),
            (f32::from_bits(0x44326c5a), f32::from_bits(0xc3d9b434)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4425e71e), f32::from_bits(0xc40289bc)),
            (f32::from_bits(0x43b7c98f), f32::from_bits(0xc3af9e21)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x4408c5ce), f32::from_bits(0xc30dfb1e)))
        .conic_to(
            (f32::from_bits(0x4352734e), f32::from_bits(0xc34956f5)),
            (f32::from_bits(0x43410ea4), f32::from_bits(0xc2cd653e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432fa9f9), f32::from_bits(0xc081c919)),
            (f32::from_bits(0x44001378), f32::from_bits(0x425d363a)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x44543c72), f32::from_bits(0x42e552cb)),
            (f32::from_bits(0x4458959e), f32::from_bits(0x4180287c)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x445ceec8), f32::from_bits(0xc2a53e8d)),
            (f32::from_bits(0x4408c5ce), f32::from_bits(0xc30dfb1e)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x44098684), f32::from_bits(0x4305b53c)))
        .conic_to(
            (f32::from_bits(0x43a5300e), f32::from_bits(0xc30026d3)),
            (f32::from_bits(0x437da548), f32::from_bits(0xc27f121c)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4330ea74), f32::from_bits(0x3f1dc4b8)),
            (f32::from_bits(0x43c65234), f32::from_bits(0x43833cea)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x441a1798), f32::from_bits(0x44031579)),
            (f32::from_bits(0x442d464c), f32::from_bits(0x43e5f9cc)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x44407502), f32::from_bits(0x43c5c8a6)),
            (f32::from_bits(0x44098684), f32::from_bits(0x4305b53c)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43bb9978), f32::from_bits(0x44487cfb)))
        .conic_to(
            (f32::from_bits(0x43bb9978), f32::from_bits(0x43e60f46)),
            (f32::from_bits(0x43898486), f32::from_bits(0x43e60f46)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432edf26), f32::from_bits(0x43e60f46)),
            (f32::from_bits(0x432edf26), f32::from_bits(0x44487cfb)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432edf26), f32::from_bits(0x448ef92a)),
            (f32::from_bits(0x43898486), f32::from_bits(0x448ef92a)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43bb9978), f32::from_bits(0x448ef92a)),
            (f32::from_bits(0x43bb9978), f32::from_bits(0x44487cfb)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x42d07148), f32::from_bits(0x445532a0)))
        .conic_to(
            (f32::from_bits(0x43a1f94a), f32::from_bits(0x4413bb9c)),
            (f32::from_bits(0x437737c0), f32::from_bits(0x4403a309)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432a7ceb), f32::from_bits(0x43e714ed)),
            (f32::from_bits(0xc244f418), f32::from_bits(0x4435017b)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3867b7b), f32::from_bits(0x4476787f)),
            (f32::from_bits(0xc3403c22), f32::from_bits(0x44834889)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc2e7029c), f32::from_bits(0x448b54d2)),
            (f32::from_bits(0x42d07148), f32::from_bits(0x445532a0)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc30809b4), f32::from_bits(0x44336379)))
        .conic_to(
            (f32::from_bits(0x43489a34), f32::from_bits(0x44248c83)),
            (f32::from_bits(0x43373589), f32::from_bits(0x440be36d)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4325d0dd), f32::from_bits(0x43e674af)),
            (f32::from_bits(0xc32ad30a), f32::from_bits(0x4402114e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3fdbb7b), f32::from_bits(0x4410e844)),
            (f32::from_bits(0xc3f50925), f32::from_bits(0x4429915a)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3ec56cf), f32::from_bits(0x44423a6f)),
            (f32::from_bits(0xc30809b4), f32::from_bits(0x44336379)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc369289a), f32::from_bits(0x43e5c2b7)))
        .conic_to(
            (f32::from_bits(0x427b82f4), f32::from_bits(0x441d9c08)),
            (f32::from_bits(0x42e1eb60), f32::from_bits(0x4407ec66)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43230aa3), f32::from_bits(0x43e47988)),
            (f32::from_bits(0xc304feb4), f32::from_bits(0x438f042f)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3d68405), f32::from_bits(0x42e63b5c)),
            (f32::from_bits(0xc3ef8e7f), f32::from_bits(0x4349dc36)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc4044c7c), f32::from_bits(0x43904d5e)),
            (f32::from_bits(0xc369289a), f32::from_bits(0x43e5c2b7)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0xc30db26a), f32::from_bits(0x43476db5)))
        .conic_to(
            (f32::from_bits(0xc1c64388), f32::from_bits(0x44022968)),
            (f32::from_bits(0x428aae1e), f32::from_bits(0x43f331c9)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4323768e), f32::from_bits(0x43e210c3)),
            (f32::from_bits(0x423a3252), f32::from_bits(0x4302e99e)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc28cbac8), f32::from_bits(0xc33e4e50)),
            (f32::from_bits(0xc3247ce4), f32::from_bits(0xc31c0c44)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc3814e32), f32::from_bits(0xc2f39470)),
            (f32::from_bits(0xc30db26a), f32::from_bits(0x43476db5)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x42bf1a33), f32::from_bits(0x426be7d0)))
        .conic_to(
            (f32::from_bits(0xc1aae6f8), f32::from_bits(0x43be18ef)),
            (f32::from_bits(0x42918542), f32::from_bits(0x43cf39f4)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4326e221), f32::from_bits(0x43e05afa)),
            (f32::from_bits(0x438de60c), f32::from_bits(0x42fefc14)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43c85b09), f32::from_bits(0xc341b9e0)),
            (f32::from_bits(0x43994b49), f32::from_bits(0xc363fbec)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x43547712), f32::from_bits(0xc3831efc)),
            (f32::from_bits(0x42bf1a33), f32::from_bits(0x426be7d0)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x43b7c98f), f32::from_bits(0x42cfc494)))
        .conic_to(
            (f32::from_bits(0x428f138a), f32::from_bits(0x4389667c)),
            (f32::from_bits(0x42f33d6e), f32::from_bits(0x43b4c5c0)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432bb3a9), f32::from_bits(0x43e02504)),
            (f32::from_bits(0x43e9de81), f32::from_bits(0x438aafae)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x443ef196), f32::from_bits(0x42d4e958)),
            (f32::from_bits(0x44326c5a), f32::from_bits(0x419db120)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4425e71e), f32::from_bits(0xc28610c8)),
            (f32::from_bits(0x43b7c98f), f32::from_bits(0x42cfc494)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x4408c5ce), f32::from_bits(0x439c91b7)))
        .conic_to(
            (f32::from_bits(0x4352734e), f32::from_bits(0x437dc797)),
            (f32::from_bits(0x43410ea4), f32::from_bits(0x43b035f6)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x432fa9f9), f32::from_bits(0x43e18822)),
            (f32::from_bits(0x44001378), f32::from_bits(0x43ff360d)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x44543c72), f32::from_bits(0x440e71fc)),
            (f32::from_bits(0x4458959e), f32::from_bits(0x43eb91ce)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x445ceec8), f32::from_bits(0x43ba3fa3)),
            (f32::from_bits(0x4408c5ce), f32::from_bits(0x439c91b7)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .move_to((f32::from_bits(0x44098684), f32::from_bits(0x441334f2)))
        .conic_to(
            (f32::from_bits(0x43a5300e), f32::from_bits(0x43a37bdc)),
            (f32::from_bits(0x437da548), f32::from_bits(0x43c3ad02)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x4330ea74), f32::from_bits(0x43e3de28)),
            (f32::from_bits(0x43c65234), f32::from_bits(0x44336618)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x441a1798), f32::from_bits(0x4474dd1c)),
            (f32::from_bits(0x442d464c), f32::from_bits(0x4464c489)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x44407502), f32::from_bits(0x4454abf6)),
            (f32::from_bits(0x44098684), f32::from_bits(0x441334f2)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .detach();

    test_simplify(reporter, &path, "SixtyOvals_2_2_9_73");
});
// Port of: tests/PathOpsBuilderConicTest.cpp#L363-L375 (chrome/m156)
def_test!(SixtyOvals_2_2_9_73_reduced, |reporter| {
    let path = PathBuilder::new()
        .move_to((377.219f32, -141.981f32))
        .conic_to(
            (40.5787f32, -201.34f32),
            (23.1855f32, -102.698f32),
            0.707107f32,
        )
        .line_to((377.219f32, -141.981f32))
        .close()
        .move_to((306.588f32, -227.984f32))
        .conic_to(
            (212.465f32, -262.242f32),
            (95.5512f32, 58.9764f32),
            0.707107f32,
        )
        .line_to((306.588f32, -227.984f32))
        .close()
        .detach();
    test_simplify(reporter, &path, "SixtyOvals_2_2_9_73_reduced");
});
// Port of: tests/PathOpsBuilderConicTest.cpp#L377-L416 (chrome/m156)
def_test!(SixtyOvalsA, |_reporter| {
    let mut path = PathBuilder::new_with_fill_type(PathFillType::EvenOdd)
        .move_to((11.1722f32, -8.10398f32))
        .conic_to(
            (22.9143f32, -10.3787f32),
            (23.7764f32, -7.72542f32),
            1.00863f32,
        )
        .conic_to(
            (24.6671f32, -4.98406f32),
            (13.8147f32, 0.0166066f32),
            0.973016f32,
        )
        .conic_to(
            (24.6378f32, 5.07425f32),
            (23.7764f32, 7.72542f32),
            1.00888f32,
        )
        .conic_to(
            (22.8777f32, 10.4915f32),
            (11.1648f32, 8.13034f32),
            0.960143f32,
        )
        .conic_to(
            (16.9503f32, 18.5866f32),
            (14.6946f32, 20.2254f32),
            1.00881f32,
        )
        .conic_to(
            (12.4417f32, 21.8623f32),
            (4.29722f32, 13.1468f32),
            1.0092f32,
        )
        .conic_to((2.92708f32, 25.0), (0.0, 25.0), 0.955692f32)
        .conic_to((-2.79361f32, 25.0), (-4.258f32, 13.1048f32), 1.00818f32)
        .conic_to(
            (-4.27813f32, 13.1264f32),
            (-4.29822f32, 13.1479f32),
            1.03158f32,
        )
        .conic_to(
            (-12.44f32, 21.8635f32),
            (-14.6946f32, 20.2254f32),
            1.00811f32,
        )
        .conic_to(
            (-16.9933f32, 18.5554f32),
            (-11.1722f32, 8.10398f32),
            0.989875f32,
        )
        .conic_to(
            (-22.9143f32, 10.3787f32),
            (-23.7764f32, 7.72542f32),
            1.00863f32,
        )
        .conic_to(
            (-24.6671f32, 4.98406f32),
            (-13.8147f32, -0.0166066f32),
            0.973016f32,
        )
        .conic_to(
            (-24.6378f32, -5.07425f32),
            (-23.7764f32, -7.72542f32),
            1.00888f32,
        )
        .conic_to(
            (-22.8777f32, -10.4915f32),
            (-11.1648f32, -8.13034f32),
            0.960143f32,
        )
        .conic_to(
            (-16.9503f32, -18.5866f32),
            (-14.6946f32, -20.2254f32),
            1.00881f32,
        )
        .conic_to(
            (-12.4417f32, -21.8623f32),
            (-4.29722f32, -13.1468f32),
            1.0092f32,
        )
        .conic_to((-2.92708f32, -25.0), (0.0, -25.0), 0.955692f32)
        .conic_to((2.79361f32, -25.0), (4.258f32, -13.1048f32), 1.00818f32)
        .conic_to(
            (4.27813f32, -13.1264f32),
            (4.29822f32, -13.1479f32),
            1.03158f32,
        )
        .conic_to(
            (12.44f32, -21.8635f32),
            (14.6946f32, -20.2254f32),
            1.00811f32,
        )
        .conic_to(
            (16.9933f32, -18.5554f32),
            (11.1722f32, -8.10398f32),
            0.989875f32,
        )
        .close()
        .detach();
    let one = path.clone();

    path = PathBuilder::new_with_fill_type(PathFillType::Winding)
        .move_to((-1.54509f32, -4.75528f32))
        .conic_to(
            (22.2313f32, -12.4807f32),
            (23.7764f32, -7.72543f32),
            0.707107f32,
        )
        .conic_to(
            (25.3215f32, -2.97014f32),
            (1.54509f32, 4.75528f32),
            0.707107f32,
        )
        .conic_to(
            (-22.2313f32, 12.4807f32),
            (-23.7764f32, 7.72543f32),
            0.707107f32,
        )
        .conic_to(
            (-25.3215f32, 2.97014f32),
            (-1.54509f32, -4.75528f32),
            0.707107f32,
        )
        .close()
        .detach();
    let two = path.clone();
    let _ = op(&one, &two, PathOp::Union);
});
// Port of: tests/PathOpsBuilderConicTest.cpp#L418-L459 (chrome/m156)
def_test!(SixtyOvalsAX, |_reporter| {
    let mut path = PathBuilder::new_with_fill_type(PathFillType::EvenOdd)
        .move_to((f32::from_bits(0x4132c174), f32::from_bits(0xc101a9e5)))
        .conic_to(
            (f32::from_bits(0x41b7508a), f32::from_bits(0xc1260efe)),
            (f32::from_bits(0x41be3618), f32::from_bits(0xc0f736ad)),
            f32::from_bits(0x3f811abd),
        )
        .conic_to(
            (f32::from_bits(0x41c5564b), f32::from_bits(0xc09f7d6d)),
            (f32::from_bits(0x415d0934), f32::from_bits(0x3c880a93)),
            f32::from_bits(0x3f79179a),
        )
        .conic_to(
            (f32::from_bits(0x41c51a48), f32::from_bits(0x40a2603c)),
            (f32::from_bits(0x41be3618), f32::from_bits(0x40f736ac)),
            f32::from_bits(0x3f8122f3),
        )
        .conic_to(
            (f32::from_bits(0x41b7056f), f32::from_bits(0x4127dd49)),
            (f32::from_bits(0x4132a328), f32::from_bits(0x410215e1)),
            f32::from_bits(0x3f75cbec),
        )
        .conic_to(
            (f32::from_bits(0x41879a3b), f32::from_bits(0x4194b151)),
            (f32::from_bits(0x416b1d34), f32::from_bits(0x41a1cdac)),
            f32::from_bits(0x3f8120d4),
        )
        .conic_to(
            (f32::from_bits(0x41471107), f32::from_bits(0x41aee601)),
            (f32::from_bits(0x408982d1), f32::from_bits(0x41525939)),
            f32::from_bits(0x3f812d7f),
        )
        .conic_to(
            (f32::from_bits(0x403b5543), f32::from_bits(0x41c80000)),
            (f32::from_bits(0x00000000), f32::from_bits(0x41c80000)),
            f32::from_bits(0x3f74a837),
        )
        .conic_to(
            (f32::from_bits(0xc032ca93), f32::from_bits(0x41c80000)),
            (f32::from_bits(0xc088418e), f32::from_bits(0x4151ad32)),
            f32::from_bits(0x3f810c2d),
        )
        .conic_to(
            (f32::from_bits(0xc088e66c), f32::from_bits(0x4152058a)),
            (f32::from_bits(0xc0898afc), f32::from_bits(0x41525d9e)),
            f32::from_bits(0x3f840adb),
        )
        .conic_to(
            (f32::from_bits(0xc1470a56), f32::from_bits(0x41aee870)),
            (f32::from_bits(0xc16b1d36), f32::from_bits(0x41a1cdac)),
            f32::from_bits(0x3f81099f),
        )
        .conic_to(
            (f32::from_bits(0xc187f23a), f32::from_bits(0x41947162)),
            (f32::from_bits(0xc132c174), f32::from_bits(0x4101a9e5)),
            f32::from_bits(0x3f7d6873),
        )
        .conic_to(
            (f32::from_bits(0xc1b7508a), f32::from_bits(0x41260efe)),
            (f32::from_bits(0xc1be3618), f32::from_bits(0x40f736ad)),
            f32::from_bits(0x3f811abd),
        )
        .conic_to(
            (f32::from_bits(0xc1c5564b), f32::from_bits(0x409f7d6d)),
            (f32::from_bits(0xc15d0934), f32::from_bits(0xbc880a93)),
            f32::from_bits(0x3f79179a),
        )
        .conic_to(
            (f32::from_bits(0xc1c51a48), f32::from_bits(0xc0a2603c)),
            (f32::from_bits(0xc1be3618), f32::from_bits(0xc0f736ac)),
            f32::from_bits(0x3f8122f3),
        )
        .conic_to(
            (f32::from_bits(0xc1b7056f), f32::from_bits(0xc127dd49)),
            (f32::from_bits(0xc132a328), f32::from_bits(0xc10215e1)),
            f32::from_bits(0x3f75cbec),
        )
        .conic_to(
            (f32::from_bits(0xc1879a3b), f32::from_bits(0xc194b151)),
            (f32::from_bits(0xc16b1d34), f32::from_bits(0xc1a1cdac)),
            f32::from_bits(0x3f8120d4),
        )
        .conic_to(
            (f32::from_bits(0xc1471107), f32::from_bits(0xc1aee601)),
            (f32::from_bits(0xc08982d1), f32::from_bits(0xc1525939)),
            f32::from_bits(0x3f812d7f),
        )
        .conic_to(
            (f32::from_bits(0xc03b5543), f32::from_bits(0xc1c80000)),
            (f32::from_bits(0x00000000), f32::from_bits(0xc1c80000)),
            f32::from_bits(0x3f74a837),
        )
        .conic_to(
            (f32::from_bits(0x4032ca93), f32::from_bits(0xc1c80000)),
            (f32::from_bits(0x4088418e), f32::from_bits(0xc151ad32)),
            f32::from_bits(0x3f810c2d),
        )
        .conic_to(
            (f32::from_bits(0x4088e66c), f32::from_bits(0xc152058a)),
            (f32::from_bits(0x40898afc), f32::from_bits(0xc1525d9e)),
            f32::from_bits(0x3f840adb),
        )
        .conic_to(
            (f32::from_bits(0x41470a56), f32::from_bits(0xc1aee870)),
            (f32::from_bits(0x416b1d36), f32::from_bits(0xc1a1cdac)),
            f32::from_bits(0x3f81099f),
        )
        .conic_to(
            (f32::from_bits(0x4187f23a), f32::from_bits(0xc1947162)),
            (f32::from_bits(0x4132c174), f32::from_bits(0xc101a9e5)),
            f32::from_bits(0x3f7d6873),
        )
        .close()
        .close()
        .detach();
    let one = path.clone();

    path = PathBuilder::new_with_fill_type(PathFillType::Winding)
        .move_to((f32::from_bits(0xbfc5c55c), f32::from_bits(0xc0982b46)))
        .conic_to(
            (f32::from_bits(0x41b1d9c2), f32::from_bits(0xc147b0fc)),
            (f32::from_bits(0x41be3618), f32::from_bits(0xc0f736b3)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0x41ca926e), f32::from_bits(0xc03e16da)),
            (f32::from_bits(0x3fc5c55c), f32::from_bits(0x40982b46)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc1b1d9c2), f32::from_bits(0x4147b0fc)),
            (f32::from_bits(0xc1be3618), f32::from_bits(0x40f736b3)),
            f32::from_bits(0x3f3504f3),
        )
        .conic_to(
            (f32::from_bits(0xc1ca926e), f32::from_bits(0x403e16da)),
            (f32::from_bits(0xbfc5c55c), f32::from_bits(0xc0982b46)),
            f32::from_bits(0x3f3504f3),
        )
        .close()
        .detach();
    let two = path.clone();

    let _ = op(&one, &two, PathOp::Union);
});

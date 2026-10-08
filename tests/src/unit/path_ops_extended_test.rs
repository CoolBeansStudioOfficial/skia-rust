// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsExtendedTest.cpp (chrome/m156): the helpers of the PathOps tests,
// `inner_simplify`, `testSimplify`, `testSimplifyFail` and `comparePaths`.

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_pathops::simplify;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, reporter_assert};

/// `enum class ExpectSuccess`.
// Port of: tests/PathOpsExtendedTest.cpp#L38-L42 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ExpectSuccess {
    No,
    Yes,
}

/// `enum class ExpectMatch`.
// Port of: tests/PathOpsExtendedTest.cpp#L51-L55 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ExpectMatch {
    No,
    Yes,
}

/// `kBitWidth` and `kBitHeight` of `PathOpsExtendedTest.cpp`.
const K_BIT_WIDTH: i32 = 64;
const K_BIT_HEIGHT: i32 = 64;

/// `MAX_ERRORS` of `comparePaths`: this many mismatched 2x2 blocks are still a match.
const MAX_ERRORS: i32 = 9;

/// `scaleMatrix(one, two, scale)`: scales the union of the two bounds to fit the comparison grid.
// Port of: tests/PathOpsExtendedTest.cpp#L63-L84 (chrome/m156)
fn scale_matrix(one: &Path, two: &Path) -> Matrix {
    let (b1, b2) = (*one.bounds(), *two.bounds());
    // SkRect::join.
    let mut larger = Rect::new(
        b1.left.min(b2.left),
        b1.top.min(b2.top),
        b1.right.max(b2.right),
        b1.bottom.max(b2.bottom),
    );
    let mut largest_width = larger.right - larger.left;
    if largest_width < 4.0 {
        largest_width = 4.0;
    }
    let mut largest_height = larger.bottom - larger.top;
    if largest_height < 4.0 {
        largest_height = 4.0;
    }
    let h_scale = (K_BIT_WIDTH - 2) as f32 / largest_width;
    let v_scale = (K_BIT_HEIGHT - 2) as f32 / largest_height;
    larger.left *= h_scale;
    larger.right *= h_scale;
    larger.top *= v_scale;
    larger.bottom *= v_scale;
    let dx = if -16000.0 > larger.left {
        -16000.0 - larger.left
    } else if 16000.0 < larger.right {
        16000.0 - larger.right
    } else {
        0.0
    };
    let dy = if -16000.0 > larger.top {
        -16000.0 - larger.top
    } else if 16000.0 < larger.bottom {
        16000.0 - larger.bottom
    } else {
        0.0
    };
    // scale.reset(); scale.preScale(hScale, vScale); scale.postTranslate(dx, dy);
    Matrix::scale_translate((h_scale, v_scale), (dx, dy))
}

/// `pathsDrawTheSame(bits, scaledOne, scaledTwo, error2x2)`: draws both paths side by side into
/// a fresh bitmap, and counts the mismatched pixels, and the mismatched 2x2 blocks.
// Port of: tests/PathOpsExtendedTest.cpp#L100-L131 (chrome/m156)
fn paths_draw_the_same(scaled_one: &Path, scaled_two: &Path) -> (i32, i32) {
    let mut bits = Bitmap::new();
    bits.alloc_n32_pixels((K_BIT_WIDTH * 2, K_BIT_HEIGHT), None);
    {
        let canvas = Canvas::from_bitmap(&mut bits, None).expect("canvas over the comparison bitmap");
        canvas.clear(Color::WHITE);
        // SkPaint's defaults: no anti-aliasing.
        let paint = Paint::default();
        let bounds1 = *scaled_one.bounds();
        canvas.save();
        canvas.translate((-bounds1.left + 1.0, -bounds1.top + 1.0));
        canvas.draw_path(scaled_one, &paint);
        canvas.restore();
        canvas.save();
        canvas.translate((-bounds1.left + 1.0 + K_BIT_WIDTH as f32, -bounds1.top + 1.0));
        canvas.draw_path(scaled_two, &paint);
        canvas.restore();
    }
    let px = |x: i32, y: i32| bits.get_addr32(x, y);
    let mut errors2 = 0;
    let mut errors = 0;
    for y in 0..K_BIT_HEIGHT - 1 {
        for x in 0..K_BIT_WIDTH - 1 {
            let err = px(x, y) != px(K_BIT_WIDTH + x, y);
            if err {
                errors2 += i32::from(
                    px(x + 1, y) != px(K_BIT_WIDTH + x + 1, y)
                        && px(x, y + 1) != px(K_BIT_WIDTH + x, y + 1)
                        && px(x + 1, y + 1) != px(K_BIT_WIDTH + x + 1, y + 1),
                );
                errors += 1;
            }
        }
    }
    (errors2, errors)
}

/// `comparePaths(reporter, filename, one, two, bitmap)`: whether the two paths draw the same,
/// allowing up to `MAX_ERRORS` mismatched 2x2 blocks, as Skia's comparison does. Returns true on a
/// match.
// Port of: tests/PathOpsExtendedTest.cpp#L209-L221 (chrome/m156)
pub(crate) fn compare_paths(one: &Path, two: &Path) -> bool {
    let scale = scale_matrix(one, two);
    let scaled_one = one.make_transform(&scale);
    let scaled_two = two.make_transform(&scale);
    let (errors2x2, _) = paths_draw_the_same(&scaled_one, &scaled_two);
    errors2x2 <= MAX_ERRORS
}

/// `inner_simplify(reporter, path, filename, expectSuccess, skipAssert, expectMatch)`.
// Port of: tests/PathOpsExtendedTest.cpp#L357-L400 (chrome/m156)
fn inner_simplify(
    reporter: &mut Reporter,
    path: &Path,
    filename: &str,
    expect_success: ExpectSuccess,
    expect_match: ExpectMatch,
) -> bool {
    let Some(out) = simplify(path) else {
        if expect_success == ExpectSuccess::Yes {
            reporter_assert!(reporter, false, "Simplify failed for {}", filename);
        }
        return false;
    };
    if expect_success == ExpectSuccess::No {
        reporter_assert!(reporter, false, "Simplify unexpected success for {}", filename);
    }
    let matches = compare_paths(path, &out);
    if expect_match == ExpectMatch::No {
        if matches {
            reporter_assert!(reporter, false, "Failing Simplify test {} now succeeds", filename);
            return false;
        }
    } else if expect_match == ExpectMatch::Yes && !matches {
        reporter_assert!(reporter, false, "Simplify mismatch for {}", filename);
    }
    reporter.bump_test_count();
    matches
}

/// `testSimplify(reporter, path, filename)`.
// Port of: tests/PathOpsExtendedTest.cpp#L402-L405 (chrome/m156)
pub(crate) fn test_simplify(reporter: &mut Reporter, path: &Path, filename: &str) -> bool {
    inner_simplify(reporter, path, filename, ExpectSuccess::Yes, ExpectMatch::Yes)
}

/// `testSimplifyFail(reporter, path, filename)`.
// Port of: tests/PathOpsExtendedTest.cpp#L416-L419 (chrome/m156)
pub(crate) fn test_simplify_fail(reporter: &mut Reporter, path: &Path, filename: &str) -> bool {
    inner_simplify(reporter, path, filename, ExpectSuccess::No, ExpectMatch::No)
}

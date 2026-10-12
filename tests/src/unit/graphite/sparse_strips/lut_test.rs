// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/LUTTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::many_single_char_names
)]

use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_gpu::graphite::sparse_strips::msaa_lut::{MsaaLut, generate_msaa_lut};

use crate::{Reporter, def_test, errorf, infof};

// Port of: tests/graphite/sparse_strips/LUTTest.cpp#L22-L47 (chrome/m156)
fn compute_naive_mask(p0: Point, p1: Point) -> u8 {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return 0;
    }
    let mut nx = dy;
    let mut ny = -dx;
    // `std::hypot`: the test only compares the masks within a tolerance of a few subsamples.
    let len = nx.hypot(ny);
    nx /= len;
    ny /= len;

    if nx < 0.0 {
        nx = -nx;
        ny = -ny;
    }
    let c = nx * p0.x + ny * p0.y;

    let mut mask: u8 = 0;
    for i in 0..8usize {
        let sx = (f32::from(MsaaLut::<u8>::K_PATTERN[i]) + 0.5) / 8.0;
        let sy = (i as f32 + 0.5) / 8.0;
        if nx * sx + ny * sy - c > 0.0 {
            mask |= 1 << i;
        }
    }
    mask
}

// Port of: tests/graphite/sparse_strips/LUTTest.cpp#L49-L83 (chrome/m156)
fn lookup_lut(p0: Point, p1: Point, lut: &[u8]) -> u8 {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;

    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return 0;
    }

    let mut nx = dy;
    let mut ny = -dx;

    // Match the Naive compute's normal orientation
    if nx < 0.0 {
        nx = -nx;
        ny = -ny;
    }
    let c = nx * p0.x + ny * p0.y;

    let is_pos = ny <= 0.0;
    let d = nx + ny.abs();

    // Squash the slope
    let s = ny.abs() / d;
    let t = ((if is_pos { nx } else { d }) - c) / d;

    const K_WIDTH: i32 = MsaaLut::<u8>::K_WIDTH;
    const K_HEIGHT: i32 = MsaaLut::<u8>::K_HEIGHT;
    const HALF_HEIGHT: i32 = K_HEIGHT / 2;

    let u = ((t * K_WIDTH as f32).floor() as i32).clamp(0, K_WIDTH - 1);
    let v_mod = ((s * HALF_HEIGHT as f32).floor() as i32).clamp(0, HALF_HEIGHT - 1);
    let v = if is_pos { v_mod + HALF_HEIGHT } else { v_mod };

    lut[(v * K_WIDTH + u) as usize]
}

// Note: Although it is possible for a line segment to end within a pixel, we do not test that case
// here. The LUT's quantization relies solely on a parameterized slope and intercept, effectively
// treating the segment as an infinite line that crosses the entire pixel. Downstream subsample
// masking logic independently handles the specific case of endpoints falling inside a pixel.
// Port of: tests/graphite/sparse_strips/LUTTest.cpp#L89-L102 (chrome/m156)
fn pick_random_square_point(rand: &mut Random) -> Point {
    let edge = rand.next_u() % 4;
    let t = rand.next_f();
    match edge {
        0 => Point::new(t, 0.0),
        1 => Point::new(1.0, t),
        2 => Point::new(t, 1.0),
        _ => Point::new(0.0, t),
    }
}

// Port of: tests/graphite/sparse_strips/LUTTest.cpp#L104-L154 (chrome/m156)
fn test_lut(
    reporter: &mut Reporter,
    lut_name: &str,
    lut: &[u8],
    seed: u32,
    lookup_func: impl Fn(Point, Point, &[u8]) -> u8,
) {
    let mut rand = Random::new(seed);

    const K_ITERATIONS: i32 = 10000;

    // Track and report the number of 1,2, and 3 sample errors. These are not reported as errors as
    // some noise is expected due to quantization to the LUT.
    const K_ERROR_LIMIT: u32 = 3;
    let mut minor_error_count = [0u32; K_ERROR_LIMIT as usize];

    for i in 0..K_ITERATIONS {
        let p0 = pick_random_square_point(&mut rand);
        let p1 = pick_random_square_point(&mut rand);

        let expected = compute_naive_mask(p0, p1);
        let actual = lookup_func(p0, p1, lut);

        let diff = expected ^ actual;
        let bit_errors = diff.count_ones();

        if bit_errors > K_ERROR_LIMIT {
            errorf!(
                reporter,
                "[{}] CRITICAL Fail at iter {} (Seed: {}). Expected 0x{:02x}, Got 0x{:02x} ({} \
                 bits diff)\nP0: {:.6}, {:.6} (0x{:08x}, 0x{:08x})\nP1: {:.6}, {:.6} (0x{:08x}, \
                 0x{:08x})",
                lut_name,
                i,
                seed,
                expected,
                actual,
                bit_errors,
                p0.x,
                p0.y,
                p0.x.to_bits(),
                p0.y.to_bits(),
                p1.x,
                p1.y,
                p1.x.to_bits(),
                p1.y.to_bits()
            );
        } else if bit_errors > 0 {
            minor_error_count[bit_errors as usize - 1] += 1;
        }
    }

    infof!(
        reporter,
        "[{}] Minor Error Summary (Seed: {}): 1-bit: {}, 2-bit: {}, 3-bit: {}",
        lut_name,
        seed,
        minor_error_count[0],
        minor_error_count[1],
        minor_error_count[2]
    );
}

def_test!(SparseStrips_LUTTest, |reporter| {
    // Port of: tests/graphite/sparse_strips/LUTTest.cpp#L158-L164 (chrome/m156)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seed = now.as_nanos() as u32;

    let msaa_lut = generate_msaa_lut::<u8>();
    test_lut(reporter, "SlopeBasedLUT", &msaa_lut, seed, lookup_lut);
});

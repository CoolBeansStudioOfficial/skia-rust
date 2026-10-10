//! The corpus of the lossy differential references (`oracle/codec-diff/libwebp/enc/encode_lossy.c`
//! and `yuv_import.c`): the image generators of `encode_lossless.c` (`make_image`) as RGBA bytes,
//! the case list in the order of `main`, and the FNV-1a hash the references print.

// The generators mirror the C reference's `uint8_t` and `int` arithmetic, so their casts wrap as the
// C conversions do; the single-letter names are the C names (x, y, r, g, b, a).
#![allow(clippy::cast_possible_truncation, clippy::many_single_char_names)]

/// One corpus image: name, generator kind, width, height.
pub const IMAGES: [(&str, u32, usize, usize); 12] = [
    ("gradient_37x23", 0, 37, 23),
    ("solid_64x64", 1, 64, 64),
    ("five_colours_50x40", 2, 50, 40),
    ("noise_33x17", 3, 33, 17),
    ("noise_alpha_40x30", 4, 40, 30),
    ("colours200_120x120", 5, 120, 120),
    ("smooth_96x80", 6, 96, 80),
    ("alpha_ramp_64x8", 7, 64, 8),
    ("pixel_1x1", 0, 1, 1),
    ("row_2x1", 0, 2, 1),
    ("column_1x5", 2, 1, 5),
    ("gradient_300x200", 6, 300, 200),
];

/// The generator's LCG (`lcg_next` in the C reference): `seed` advances, the 15-bit high part is
/// returned.
fn lcg_next(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
    (*seed >> 16) & 0x7fff
}

/// Port of `make_image` (`encode_lossless.c`): `w * h` RGBA pixels, row-major.
#[must_use]
pub fn make_rgba(kind: u32, w: usize, h: usize) -> Vec<u8> {
    let mut seed: u32 = 12345u32.wrapping_add(kind);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut a) = (0u8, 0u8, 0u8, 255u8);
            match kind {
                0 => {
                    r = (x * 7) as u8;
                    g = (y * 11) as u8;
                    b = ((x + y) * 3) as u8;
                }
                1 => b = 255,
                2 => {
                    const PAL: [[u8; 3]; 5] = [
                        [255, 0, 0],
                        [0, 255, 0],
                        [0, 0, 255],
                        [10, 20, 30],
                        [200, 200, 0],
                    ];
                    let idx = ((x / 5) + (y / 3)) % 5;
                    [r, g, b] = PAL[idx];
                }
                3 => {
                    r = lcg_next(&mut seed) as u8;
                    g = lcg_next(&mut seed) as u8;
                    b = lcg_next(&mut seed) as u8;
                }
                4 => {
                    r = lcg_next(&mut seed) as u8;
                    g = lcg_next(&mut seed) as u8;
                    b = lcg_next(&mut seed) as u8;
                    a = lcg_next(&mut seed) as u8;
                }
                5 => {
                    let idx = (x * x + y * 3) % 200;
                    r = (idx * 37) as u8;
                    g = (idx * 91 + 7) as u8;
                    b = (idx * 13 + 100) as u8;
                }
                6 => {
                    r = ((x * 255) / if w > 1 { w - 1 } else { 1 }) as u8;
                    g = ((y * 255) / if h > 1 { h - 1 } else { 1 }) as u8;
                    b = (((x ^ y) as u32 + (lcg_next(&mut seed) & 3)) & 255) as u8;
                }
                7 => {
                    r = 40;
                    g = 90;
                    b = 200;
                    a = ((x * 255) / if w > 1 { w - 1 } else { 1 }) as u8;
                }
                _ => {}
            }
            let p = 4 * (y * w + x);
            out[p] = r;
            out[p + 1] = g;
            out[p + 2] = b;
            out[p + 3] = a;
        }
    }
    out
}

/// Port of the reference's `fnv1a64`.
#[must_use]
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash: u64 = 1_469_598_103_934_665_603;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

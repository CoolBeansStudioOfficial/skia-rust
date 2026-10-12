// Copyright 2009 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: third_party/etc1/etc1.cpp (the encoder half: `etc1_encode_block` and
// `etc1_encode_image`; the decoder and PKM helpers are not used by any ported test or GM).

//! The ETC1 encoder from Skia's `third_party/etc1` (a fork of the AOSP ETC1 codec), used by the
//! compressed-texture GMs to build ETC2 RGB8 data.

// Port of: third_party/etc1/etc1.cpp#L118-L126 (chrome/m156), `kModifierTable`
const MODIFIER_TABLE: [i32; 32] = [
    2, 8, -2, -8, //
    5, 17, -5, -17, //
    9, 29, -9, -29, //
    13, 42, -13, -42, //
    18, 60, -18, -60, //
    24, 80, -24, -80, //
    33, 106, -33, -106, //
    47, 183, -47, -183,
];

// Port of: third_party/etc1/etc1.cpp#L130-L132 (chrome/m156), `clamp`
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 0..=255
fn clamp(x: i32) -> u8 {
    x.clamp(0, 255) as u8
}

// Port of: third_party/etc1/etc1.cpp#L134-L138 (chrome/m156), `convert4To8`
fn convert_4_to_8(b: i32) -> i32 {
    let c = b & 0xf;
    (c << 4) | c
}

// Port of: third_party/etc1/etc1.cpp#L140-L144 (chrome/m156), `convert5To8`
fn convert_5_to_8(b: i32) -> i32 {
    let c = b & 0x1f;
    (c << 3) | (c >> 2)
}

// Port of: third_party/etc1/etc1.cpp#L146-L150 (chrome/m156), `convert6To8`
fn convert_6_to_8(b: i32) -> i32 {
    let c = b & 0x3f;
    (c << 2) | (c >> 4)
}

// Port of: third_party/etc1/etc1.cpp#L152-L155 (chrome/m156), `divideBy255`
fn divide_by_255(d: i32) -> i32 {
    (d + 128 + (d >> 8)) >> 8
}

// Port of: third_party/etc1/etc1.cpp#L157-L161 (chrome/m156), `convert8To4`
fn convert_8_to_4(b: i32) -> i32 {
    let c = b & 0xff;
    divide_by_255(c * 15)
}

// Port of: third_party/etc1/etc1.cpp#L163-L167 (chrome/m156), `convert8To5`
fn convert_8_to_5(b: i32) -> i32 {
    let c = b & 0xff;
    divide_by_255(c * 31)
}

// Port of: third_party/etc1/etc1.cpp#L241-L245 (chrome/m156), `etc_compressed`
#[derive(Clone, Copy, Default)]
struct EtcCompressed {
    high: u32,
    low: u32,
    score: u32, // Lower is more accurate
}

// Port of: third_party/etc1/etc1.cpp#L247-L252 (chrome/m156), `take_best`
fn take_best(a: &mut EtcCompressed, b: &EtcCompressed) {
    if a.score > b.score {
        *a = *b;
    }
}

// Port of: third_party/etc1/etc1.cpp#L254-L299 (chrome/m156), `etc_average_colors_subblock`
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names, // r, g, b are the C++ names
)]
fn etc_average_colors_subblock(
    p_in: &[u8],
    in_mask: u32,
    p_colors: &mut [u8],
    flipped: bool,
    second: bool,
) {
    let mut r: i32 = 0;
    let mut g: i32 = 0;
    let mut b: i32 = 0;

    if flipped {
        let by = if second { 2 } else { 0 };
        for y in 0..2 {
            let yy = by + y;
            for x in 0..4 {
                let i = x + 4 * yy;
                if in_mask & (1 << i) != 0 {
                    let p = i as usize * 3;
                    r += i32::from(p_in[p]);
                    g += i32::from(p_in[p + 1]);
                    b += i32::from(p_in[p + 2]);
                }
            }
        }
    } else {
        let bx = if second { 2 } else { 0 };
        for y in 0..4 {
            for x in 0..2 {
                let xx = bx + x;
                let i = xx + 4 * y;
                if in_mask & (1 << i) != 0 {
                    let p = i as usize * 3;
                    r += i32::from(p_in[p]);
                    g += i32::from(p_in[p + 1]);
                    b += i32::from(p_in[p + 2]);
                }
            }
        }
    }
    p_colors[0] = ((r + 4) >> 3) as u8;
    p_colors[1] = ((g + 4) >> 3) as u8;
    p_colors[2] = ((b + 4) >> 3) as u8;
}

// Port of: third_party/etc1/etc1.cpp#L301-L304 (chrome/m156), `square`
fn square(x: i32) -> i32 {
    x * x
}

// Port of: third_party/etc1/etc1.cpp#L306-L340 (chrome/m156), `chooseModifier`
#[allow(
    clippy::cast_sign_loss, // every score term is a square times a constant: non-negative
    clippy::cast_possible_truncation, // the modifier index is 0..=3
    clippy::needless_range_loop, // the loop index selects the modifier, as in the C++
    clippy::many_single_char_names,
)]
fn choose_modifier(
    p_base_colors: &[u8],
    p_in: &[u8],
    p_low: &mut u32,
    bit_index: u32,
    p_modifier_table: &[i32],
) -> u32 {
    let mut best_score: u32 = !0;
    let mut best_index: i32 = 0;
    let pixel_r = i32::from(p_in[0]);
    let pixel_g = i32::from(p_in[1]);
    let pixel_b = i32::from(p_in[2]);
    let r = i32::from(p_base_colors[0]);
    let g = i32::from(p_base_colors[1]);
    let b = i32::from(p_base_colors[2]);
    for i in 0..4 {
        let modifier = p_modifier_table[i];
        let decoded_g = i32::from(clamp(g + modifier));
        let mut score = (6 * square(decoded_g - pixel_g)) as u32;
        if score >= best_score {
            continue;
        }
        let decoded_r = i32::from(clamp(r + modifier));
        score += (3 * square(decoded_r - pixel_r)) as u32;
        if score >= best_score {
            continue;
        }
        let decoded_b = i32::from(clamp(b + modifier));
        score += square(decoded_b - pixel_b) as u32;
        if score < best_score {
            best_score = score;
            best_index = i32::try_from(i).expect("the index is 0..=3");
        }
    }
    // `(bestIndex >> 1) << 16` is at most 0x20000 and `bitIndex` at most 15, so the mask fits.
    let low_mask = ((((best_index >> 1) << 16) | (best_index & 1)) as u32) << bit_index;
    *p_low |= low_mask;
    best_score
}

// Port of: third_party/etc1/etc1.cpp#L342-L379 (chrome/m156), `etc_encode_subblock_helper`
#[allow(clippy::cast_sign_loss)] // indices are non-negative
fn etc_encode_subblock_helper(
    p_in: &[u8],
    in_mask: u32,
    p_compressed: &mut EtcCompressed,
    flipped: bool,
    second: bool,
    p_base_colors: &[u8],
    p_modifier_table: &[i32],
) {
    let mut score = p_compressed.score;
    if flipped {
        let by = if second { 2 } else { 0 };
        for y in 0..2 {
            let yy = by + y;
            for x in 0..4 {
                let i = x + 4 * yy;
                if in_mask & (1 << i) != 0 {
                    let p = i as usize * 3;
                    score += choose_modifier(
                        p_base_colors,
                        &p_in[p..],
                        &mut p_compressed.low,
                        (yy + x * 4) as u32,
                        p_modifier_table,
                    );
                }
            }
        }
    } else {
        let bx = if second { 2 } else { 0 };
        for y in 0..4 {
            for x in 0..2 {
                let xx = bx + x;
                let i = xx + 4 * y;
                if in_mask & (1 << i) != 0 {
                    let p = i as usize * 3;
                    score += choose_modifier(
                        p_base_colors,
                        &p_in[p..],
                        &mut p_compressed.low,
                        (y + xx * 4) as u32,
                        p_modifier_table,
                    );
                }
            }
        }
    }
    p_compressed.score = score;
}

// Port of: third_party/etc1/etc1.cpp#L381-L383 (chrome/m156), `inRange4bitSigned`
fn in_range_4bit_signed(color: i32) -> bool {
    (-4..=3).contains(&color)
}

// Port of: third_party/etc1/etc1.cpp#L385-L435 (chrome/m156), `etc_encodeBaseColors`
#[allow(
    clippy::cast_sign_loss, // the packed fields are in 0..=31 or masked to 0..=7 (see below)
    clippy::cast_possible_truncation, // the channels are in 0..=255
    clippy::many_single_char_names,
    clippy::similar_names, // the names are the C++ ones
)]
fn etc_encode_base_colors(
    p_base_colors: &mut [u8; 6],
    p_colors: &[u8],
    p_compressed: &mut EtcCompressed,
) {
    let r51 = convert_8_to_5(i32::from(p_colors[0]));
    let g51 = convert_8_to_5(i32::from(p_colors[1]));
    let b51 = convert_8_to_5(i32::from(p_colors[2]));
    let r52 = convert_8_to_5(i32::from(p_colors[3]));
    let g52 = convert_8_to_5(i32::from(p_colors[4]));
    let b52 = convert_8_to_5(i32::from(p_colors[5]));

    let dr = r52 - r51;
    let dg = g52 - g51;
    let db = b52 - b51;

    let (r1, g1, b1, r2, g2, b2);
    if in_range_4bit_signed(dr) && in_range_4bit_signed(dg) && in_range_4bit_signed(db) {
        r1 = convert_5_to_8(r51);
        g1 = convert_5_to_8(g51);
        b1 = convert_5_to_8(b51);
        r2 = convert_5_to_8(r51 + dr);
        g2 = convert_5_to_8(g51 + dg);
        b2 = convert_5_to_8(b51 + db);
        p_compressed.high |= ((r51 as u32) << 27)
            | (((7 & dr) as u32) << 24)
            | ((g51 as u32) << 19)
            | (((7 & dg) as u32) << 16)
            | ((b51 as u32) << 11)
            | (((7 & db) as u32) << 8)
            | 2;
    } else {
        let r41 = convert_8_to_4(i32::from(p_colors[0]));
        let g41 = convert_8_to_4(i32::from(p_colors[1]));
        let b41 = convert_8_to_4(i32::from(p_colors[2]));
        let r42 = convert_8_to_4(i32::from(p_colors[3]));
        let g42 = convert_8_to_4(i32::from(p_colors[4]));
        let b42 = convert_8_to_4(i32::from(p_colors[5]));
        r1 = convert_4_to_8(r41);
        g1 = convert_4_to_8(g41);
        b1 = convert_4_to_8(b41);
        r2 = convert_4_to_8(r42);
        g2 = convert_4_to_8(g42);
        b2 = convert_4_to_8(b42);
        p_compressed.high |= ((r41 as u32) << 28)
            | ((r42 as u32) << 24)
            | ((g41 as u32) << 20)
            | ((g42 as u32) << 16)
            | ((b41 as u32) << 12)
            | ((b42 as u32) << 8);
    }
    // All of these are in 0..=255 (`convert*To8` of a masked value).
    p_base_colors[0] = r1 as u8;
    p_base_colors[1] = g1 as u8;
    p_base_colors[2] = b1 as u8;
    p_base_colors[3] = r2 as u8;
    p_base_colors[4] = g2 as u8;
    p_base_colors[5] = b2 as u8;
}

// Port of: third_party/etc1/etc1.cpp#L437-L475 (chrome/m156), `etc_encode_block_helper`
#[allow(clippy::cast_possible_truncation)] // the subblock index is 0..=7
fn etc_encode_block_helper(
    p_in: &[u8],
    in_mask: u32,
    p_colors: &[u8],
    p_compressed: &mut EtcCompressed,
    flipped: bool,
) {
    p_compressed.score = !0;
    p_compressed.high = u32::from(flipped);
    p_compressed.low = 0;

    let mut p_base_colors = [0u8; 6];

    etc_encode_base_colors(&mut p_base_colors, p_colors, p_compressed);

    let original_high = p_compressed.high;

    for i in 0..8 {
        let mut temp = EtcCompressed {
            score: 0,
            high: original_high | ((i as u32) << 5),
            low: 0,
        };
        etc_encode_subblock_helper(
            p_in,
            in_mask,
            &mut temp,
            flipped,
            false,
            &p_base_colors,
            &MODIFIER_TABLE[i * 4..i * 4 + 4],
        );
        take_best(p_compressed, &temp);
    }
    let first_half = *p_compressed;
    for i in 0..8 {
        let mut temp = EtcCompressed {
            score: first_half.score,
            high: first_half.high | ((i as u32) << 2),
            low: first_half.low,
        };
        etc_encode_subblock_helper(
            p_in,
            in_mask,
            &mut temp,
            flipped,
            true,
            &p_base_colors[3..],
            &MODIFIER_TABLE[i * 4..i * 4 + 4],
        );
        if i == 0 {
            *p_compressed = temp;
        } else {
            take_best(p_compressed, &temp);
        }
    }
}

// Port of: third_party/etc1/etc1.cpp#L477-L482 (chrome/m156), `writeBigEndian`
fn write_big_endian(p_out: &mut [u8], d: u32) {
    p_out[0] = (d >> 24) as u8;
    p_out[1] = ((d >> 16) & 0xFF) as u8;
    p_out[2] = ((d >> 8) & 0xFF) as u8;
    p_out[3] = (d & 0xFF) as u8;
}

// Port of: third_party/etc1/etc1.cpp#L484-L504 (chrome/m156), `etc1_encode_block`
//
// `p_in` is a 4 x 4 square of 3-byte pixels in form R, G, B. `in_mask` is a 16-bit mask where
// bit `1 << (x + y * 4)` tells whether the pixel is valid; invalid pixels are ignored.
pub fn etc1_encode_block(p_in: &[u8], in_mask: u32, p_out: &mut [u8]) {
    let mut colors = [0u8; 6];
    let mut flipped_colors = [0u8; 6];
    etc_average_colors_subblock(p_in, in_mask, &mut colors[0..3], false, false);
    etc_average_colors_subblock(p_in, in_mask, &mut colors[3..6], false, true);
    etc_average_colors_subblock(p_in, in_mask, &mut flipped_colors[0..3], true, false);
    etc_average_colors_subblock(p_in, in_mask, &mut flipped_colors[3..6], true, true);

    let mut a = EtcCompressed::default();
    let mut b = EtcCompressed::default();
    etc_encode_block_helper(p_in, in_mask, &colors, &mut a, false);
    etc_encode_block_helper(p_in, in_mask, &flipped_colors, &mut b, true);
    take_best(&mut a, &b);
    write_big_endian(&mut p_out[0..4], a.high);
    write_big_endian(&mut p_out[4..8], a.low);
}

// Port of: third_party/etc1/etc1.cpp#L522-L524 (chrome/m156), `kYMask` and `kXMask`
const Y_MASK: [u32; 5] = [0x0, 0xf, 0xff, 0xfff, 0xffff];
const X_MASK: [u32; 5] = [0x0, 0x1111, 0x3333, 0x7777, 0xffff];

// Port of: third_party/etc1/etc1.cpp#L517-L564 (chrome/m156), `etc1_encode_image`
//
// `p_in` holds the image with the red component of pixel (x, y) at
// `p_in[pixel_size * x + stride * y]`; `pixel_size` is 2 (RGB565) or 3 (RGB888). The encoded
// blocks are written to `p_out`, which must hold the whole encoded image. Returns 0 on success.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap, // pixel values are at most 16 bits
)]
pub fn etc1_encode_image(
    p_in: &[u8],
    width: u32,
    height: u32,
    pixel_size: u32,
    stride: u32,
    p_out: &mut [u8],
) -> i32 {
    if !(2..=3).contains(&pixel_size) {
        return -1;
    }
    let mut block = [0u8; 48];
    let mut encoded = [0u8; 8];

    let encoded_width = (width + 3) & !3;
    let encoded_height = (height + 3) & !3;

    let mut out = 0usize;
    for y in (0..encoded_height).step_by(4) {
        let mut y_end = height - y;
        if y_end > 4 {
            y_end = 4;
        }
        let ymask = Y_MASK[y_end as usize];
        for x in (0..encoded_width).step_by(4) {
            let mut x_end = width - x;
            if x_end > 4 {
                x_end = 4;
            }
            let mask = ymask & X_MASK[x_end as usize];
            for cy in 0..y_end {
                let mut q = (cy * 4) as usize * 3;
                let mut p = (pixel_size * x + stride * (y + cy)) as usize;
                if pixel_size == 3 {
                    let n = x_end as usize * 3;
                    block[q..q + n].copy_from_slice(&p_in[p..p + n]);
                } else {
                    for _ in 0..x_end {
                        let pixel = (u32::from(p_in[p + 1]) << 8) | u32::from(p_in[p]);
                        // `convert5To8`/`convert6To8` mask their input, so the shifts are exact.
                        block[q] = convert_5_to_8((pixel >> 11).cast_signed()) as u8;
                        block[q + 1] = convert_6_to_8((pixel >> 5).cast_signed()) as u8;
                        block[q + 2] = convert_5_to_8(pixel.cast_signed()) as u8;
                        q += 3;
                        p += pixel_size as usize;
                    }
                }
            }
            etc1_encode_block(&block, mask, &mut encoded);
            p_out[out..out + encoded.len()].copy_from_slice(&encoded);
            out += encoded.len();
        }
    }
    0
}

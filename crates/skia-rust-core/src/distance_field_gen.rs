// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDistanceFieldGen.h, src/core/SkDistanceFieldGen.cpp

//! Signed distance fields of glyph masks (`SkDistanceFieldGen`), for GPU distance field text.
//!
//! The distance field is made with Danielsson's 8SSEDT over the mask's edge texels, whose
//! distances come from Gustavson's gradient-based estimate (2011).

use crate::color_data::{packed16_to_b32, packed16_to_g32, packed16_to_r32};
use crate::point::Point;
use crate::point::point_priv;
use crate::scalar::{SCALAR_SQRT2, Scalar, scalar_round_to_int, scalar_sqrt};
use crate::t_pin::t_pin;

/// `SK_DistanceFieldMagnitude`: the max magnitude for the distance field. Distance values are
/// limited to the range `(-MAGNITUDE, MAGNITUDE]`.
// Port of: src/core/SkDistanceFieldGen.h#L20 (chrome/m156)
pub const DISTANCE_FIELD_MAGNITUDE: i32 = 4;

/// `SK_DistanceFieldPad`: the padding around the original glyph that allows the maximum
/// distance of [`DISTANCE_FIELD_MAGNITUDE`] texels away from any edge.
// Port of: src/core/SkDistanceFieldGen.h#L23 (chrome/m156)
pub const DISTANCE_FIELD_PAD: i32 = 4;

/// `SK_DistanceFieldInset`: the rect we render with is inset from the distance field glyph size
/// to allow for bilerp.
// Port of: src/core/SkDistanceFieldGen.h#L25 (chrome/m156)
pub const DISTANCE_FIELD_INSET: i32 = 2;

/// `SkComputeDistanceFieldSize(w, h)`: the size in bytes of the distance field of a `w` x `h`
/// image.
// Port of: src/core/SkDistanceFieldGen.h#L80-L82 (chrome/m156)
#[doc(alias = "SkComputeDistanceFieldSize")]
#[must_use]
pub fn compute_distance_field_size(w: i32, h: i32) -> usize {
    usize::try_from((w + 2 * DISTANCE_FIELD_PAD) * (h + 2 * DISTANCE_FIELD_PAD))
        .expect("non-negative size")
}

/// `DFData`: the working data of one texel.
#[derive(Copy, Clone, Default)]
struct DfData {
    /// `fAlpha`: alpha value of the source texel.
    alpha: f32,
    /// `fDistSq`: distance squared to the nearest (so far) edge texel.
    dist_sq: f32,
    /// `fDistVector`: distance vector to the nearest (so far) edge texel.
    dist_vector: Point,
}

// `NeighborFlags`.
const LEFT_NEIGHBOR_FLAG: i32 = 0x01;
const RIGHT_NEIGHBOR_FLAG: i32 = 0x02;
const TOP_LEFT_NEIGHBOR_FLAG: i32 = 0x04;
const TOP_NEIGHBOR_FLAG: i32 = 0x08;
const TOP_RIGHT_NEIGHBOR_FLAG: i32 = 0x10;
const BOTTOM_LEFT_NEIGHBOR_FLAG: i32 = 0x20;
const BOTTOM_NEIGHBOR_FLAG: i32 = 0x40;
const BOTTOM_RIGHT_NEIGHBOR_FLAG: i32 = 0x80;
const ALL_NEIGHBOR_FLAGS: i32 = 0xff;

/// `found_edge`: we treat an "edge" as a place where we cross from >=128 to <128, or vice versa,
/// or where we have two non-zero pixels that are <128. `neighbor_flags` limits the directions in
/// which we test to avoid indexing outside of the image.
// Port of: src/core/SkDistanceFieldGen.cpp#L52-L80 (chrome/m156)
fn found_edge(image: &[u8], pos: usize, width: isize, neighbor_flags: i32) -> bool {
    // the order of these should match the neighbor flags above
    let offsets: [isize; 8] = [
        -1,
        1,
        -width - 1,
        -width,
        -width + 1,
        width - 1,
        width,
        width + 1,
    ];

    // search for an edge
    let curr_val = image[pos];
    let curr_check = curr_val >> 7;
    for (i, offset) in offsets.iter().enumerate() {
        let neighbor_val = if (1 << i) & neighbor_flags != 0 {
            image[pos.wrapping_add_signed(*offset)]
        } else {
            0
        };
        let neighbor_check = neighbor_val >> 7;
        // if sharp transition
        if curr_check != neighbor_check
            // or both <128 and >0
            || (curr_check == 0 && neighbor_check == 0 && curr_val != 0 && neighbor_val != 0)
        {
            return true;
        }
    }
    false
}

/// `init_glyph_data`.
// Port of: src/core/SkDistanceFieldGen.cpp#L82-L123 (chrome/m156)
fn init_glyph_data(
    data: &mut [DfData],
    edges: &mut [u8],
    image: &[u8],
    data_width: usize,
    image_width: usize,
    image_height: usize,
    pad: usize,
) {
    let mut data_pos = pad * data_width + pad;
    let mut edge_pos = data_pos;
    let mut image_pos = 0usize;

    for j in 0..image_height {
        for i in 0..image_width {
            data[data_pos].alpha = if image[image_pos] == 255 {
                1.0
            } else {
                f32::from(image[image_pos]) * 0.003_921_568_6 // 1/255
            };
            let mut check_mask = ALL_NEIGHBOR_FLAGS;
            if i == 0 {
                check_mask &=
                    !(LEFT_NEIGHBOR_FLAG | TOP_LEFT_NEIGHBOR_FLAG | BOTTOM_LEFT_NEIGHBOR_FLAG);
            }
            if i == image_width - 1 {
                check_mask &=
                    !(RIGHT_NEIGHBOR_FLAG | TOP_RIGHT_NEIGHBOR_FLAG | BOTTOM_RIGHT_NEIGHBOR_FLAG);
            }
            if j == 0 {
                check_mask &=
                    !(TOP_LEFT_NEIGHBOR_FLAG | TOP_NEIGHBOR_FLAG | TOP_RIGHT_NEIGHBOR_FLAG);
            }
            if j == image_height - 1 {
                check_mask &= !(BOTTOM_LEFT_NEIGHBOR_FLAG
                    | BOTTOM_NEIGHBOR_FLAG
                    | BOTTOM_RIGHT_NEIGHBOR_FLAG);
            }
            if found_edge(image, image_pos, image_width as isize, check_mask) {
                edges[edge_pos] = 255; // using 255 makes for convenient debug rendering
            }
            data_pos += 1;
            image_pos += 1;
            edge_pos += 1;
        }
        data_pos += 2 * pad;
        edge_pos += 2 * pad;
    }
}

/// `edge_distance` (from Gustavson (2011)): the distance to an edge given an edge normal vector
/// and a pixel's alpha value. Assumes that `direction` has been pre-normalized.
// Port of: src/core/SkDistanceFieldGen.cpp#L125-L165 (chrome/m156)
fn edge_distance(direction: Point, alpha: f32) -> f32 {
    let mut dx = direction.x;
    let mut dy = direction.y;
    if dx.nearly_zero(None) || dy.nearly_zero(None) {
        0.5 - alpha
    } else {
        // this is easier if we treat the direction as being in the first octant
        // (other octants are symmetrical)
        dx = dx.abs();
        dy = dy.abs();
        if dx < dy {
            std::mem::swap(&mut dx, &mut dy);
        }

        // a1 = 0.5*dy/dx is the smaller fractional area chopped off by the edge
        // to avoid the divide, we just consider the numerator
        let a1num = 0.5 * dy;

        // we now compute the approximate distance, depending where the alpha falls
        // relative to the edge fractional area
        if alpha * dx < a1num {
            // if 0 <= alpha < a1
            0.5 * (dx + dy) - scalar_sqrt(2.0 * dx * dy * alpha)
        } else if alpha * dx < (dx - a1num) {
            // if a1 <= alpha <= 1 - a1
            (0.5 - alpha) * dx
        } else {
            // if 1 - a1 < alpha <= 1
            -0.5 * (dx + dy) + scalar_sqrt(2.0 * dx * dy * (1.0 - alpha))
        }
    }
}

/// `init_distances`.
// Port of: src/core/SkDistanceFieldGen.cpp#L167-L212 (chrome/m156)
fn init_distances(data: &mut [DfData], edges: &[u8], width: usize, height: usize) {
    let mut curr = 0usize;
    for _j in 0..height {
        for _i in 0..width {
            if edges[curr] != 0 {
                // we should not be in the one-pixel outside band
                // gradient will point from low to high
                // +y is down in this case
                // i.e., if you're outside, gradient points towards edge
                // if you're inside, gradient points away from edge
                let prev = curr - width;
                let next = curr + width;
                let mut curr_grad = Point::new(
                    data[prev + 1].alpha - data[prev - 1].alpha
                        + SCALAR_SQRT2 * data[curr + 1].alpha
                        - SCALAR_SQRT2 * data[curr - 1].alpha
                        + data[next + 1].alpha
                        - data[next - 1].alpha,
                    data[next - 1].alpha - data[prev - 1].alpha + SCALAR_SQRT2 * data[next].alpha
                        - SCALAR_SQRT2 * data[prev].alpha
                        + data[next + 1].alpha
                        - data[prev + 1].alpha,
                );
                point_priv::set_length_fast(&mut curr_grad, 1.0);

                // init squared distance to edge and distance vector
                let dist = edge_distance(curr_grad, data[curr].alpha);
                data[curr].dist_vector = Point::new(curr_grad.x * dist, curr_grad.y * dist);
                data[curr].dist_sq = dist * dist;
            } else {
                // init distance to "far away"
                data[curr].dist_sq = 2_000_000.0;
                data[curr].dist_vector = Point::new(1000.0, 1000.0);
            }
            curr += 1;
        }
    }
}

// Danielsson's 8SSEDT

/// First stage forward pass (forward in Y, forward in X).
// Port of: src/core/SkDistanceFieldGen.cpp#L216-L257 (chrome/m156)
fn f1(data: &mut [DfData], curr: usize, width: usize) {
    // upper left
    let mut check = curr - width - 1;
    let mut dist_vec = data[check].dist_vector;
    let mut dist_sq = data[check].dist_sq - 2.0 * (dist_vec.x + dist_vec.y - 1.0);
    if dist_sq < data[curr].dist_sq {
        dist_vec.x -= 1.0;
        dist_vec.y -= 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }

    // up
    check = curr - width;
    dist_vec = data[check].dist_vector;
    dist_sq = data[check].dist_sq - 2.0 * dist_vec.y + 1.0;
    if dist_sq < data[curr].dist_sq {
        dist_vec.y -= 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }

    // upper right
    check = curr - width + 1;
    dist_vec = data[check].dist_vector;
    dist_sq = data[check].dist_sq + 2.0 * (dist_vec.x - dist_vec.y + 1.0);
    if dist_sq < data[curr].dist_sq {
        dist_vec.x += 1.0;
        dist_vec.y -= 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }

    // left
    check = curr - 1;
    dist_vec = data[check].dist_vector;
    dist_sq = data[check].dist_sq - 2.0 * dist_vec.x + 1.0;
    if dist_sq < data[curr].dist_sq {
        dist_vec.x -= 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }
}

/// Second stage forward pass (forward in Y, backward in X).
// Port of: src/core/SkDistanceFieldGen.cpp#L259-L271 (chrome/m156)
fn f2(data: &mut [DfData], curr: usize) {
    // right
    let check = curr + 1;
    let mut dist_vec = data[check].dist_vector;
    let dist_sq = data[check].dist_sq + 2.0 * dist_vec.x + 1.0;
    if dist_sq < data[curr].dist_sq {
        dist_vec.x += 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }
}

/// First stage backward pass (backward in Y, forward in X).
// Port of: src/core/SkDistanceFieldGen.cpp#L273-L285 (chrome/m156)
fn b1(data: &mut [DfData], curr: usize) {
    // left
    let check = curr - 1;
    let mut dist_vec = data[check].dist_vector;
    let dist_sq = data[check].dist_sq - 2.0 * dist_vec.x + 1.0;
    if dist_sq < data[curr].dist_sq {
        dist_vec.x -= 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }
}

/// Second stage backward pass (backward in Y, backwards in X).
// Port of: src/core/SkDistanceFieldGen.cpp#L287-L327 (chrome/m156)
fn b2(data: &mut [DfData], curr: usize, width: usize) {
    // right
    let mut check = curr + 1;
    let mut dist_vec = data[check].dist_vector;
    let mut dist_sq = data[check].dist_sq + 2.0 * dist_vec.x + 1.0;
    if dist_sq < data[curr].dist_sq {
        dist_vec.x += 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }

    // bottom left
    check = curr + width - 1;
    dist_vec = data[check].dist_vector;
    dist_sq = data[check].dist_sq - 2.0 * (dist_vec.x - dist_vec.y - 1.0);
    if dist_sq < data[curr].dist_sq {
        dist_vec.x -= 1.0;
        dist_vec.y += 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }

    // bottom
    check = curr + width;
    dist_vec = data[check].dist_vector;
    dist_sq = data[check].dist_sq + 2.0 * dist_vec.y + 1.0;
    if dist_sq < data[curr].dist_sq {
        dist_vec.y += 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }

    // bottom right
    check = curr + width + 1;
    dist_vec = data[check].dist_vector;
    dist_sq = data[check].dist_sq + 2.0 * (dist_vec.x + dist_vec.y + 1.0);
    if dist_sq < data[curr].dist_sq {
        dist_vec.x += 1.0;
        dist_vec.y += 1.0;
        data[curr].dist_sq = dist_sq;
        data[curr].dist_vector = dist_vec;
    }
}

/// `pack_distance_field_val<distanceMagnitude>`.
// Port of: src/core/SkDistanceFieldGen.cpp#L335-L349 (chrome/m156)
// The final cast mirrors the C++ `(unsigned char)` cast of the rounded int.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pack_distance_field_val(distance_magnitude: i32, dist: f32) -> u8 {
    #[allow(clippy::cast_precision_loss)] // the magnitude is 4
    let magnitude = distance_magnitude as f32;
    // The distance field is constructed as unsigned char values, so that the zero value is at
    // 128, Beside 128, we have 128 values in range [0, 128), but only 127 values in range
    // (128, 255]. So we multiply distanceMagnitude by 127/128 at the latter range to avoid
    // overflow.
    let mut dist: f32 = t_pin(-dist, -magnitude, magnitude * 127.0 / 128.0);

    // Scale into the positive range for unsigned distance.
    dist += magnitude;

    // Scale into unsigned char range. Round to place negative and positive values as equally
    // as possible around 128 (which represents zero).
    #[allow(clippy::cast_precision_loss)] // the magnitude is 4
    let two_magnitude = (2 * distance_magnitude) as f32;
    scalar_round_to_int(dist / two_magnitude * 256.0) as u8
}

/// `generate_distance_field_from_image`: assumes a padded 8-bit image and distance field. `width`
/// and `height` are the original width and height of the image.
// Port of: src/core/SkDistanceFieldGen.cpp#L351-L486 (chrome/m156)
fn generate_distance_field_from_image(
    distance_field: &mut [u8],
    copy: &[u8],
    width: usize,
    height: usize,
) -> bool {
    // we expand our temp data by one more on each side to simplify
    // the scanning code -- will always be treated as infinitely far away
    let pad = DISTANCE_FIELD_PAD as usize + 1;

    // set params for distance field data
    let data_width = width + 2 * pad;
    let data_height = height + 2 * pad;

    // create zeroed temp DFData+edge storage
    let count = data_width * data_height;
    let mut data = vec![DfData::default(); count];
    let mut edges = vec![0u8; count];

    // copy glyph into distance field storage
    init_glyph_data(
        &mut data,
        &mut edges,
        copy,
        data_width,
        width + 2,
        height + 2,
        DISTANCE_FIELD_PAD as usize,
    );

    // create initial distance data, particularly at edges
    init_distances(&mut data, &edges, data_width, data_height);

    // now perform Euclidean distance transform to propagate distances
    // (`curr` is signed because the backward pass ends one row before the start of the data).

    // forwards in y
    let dw = data_width as isize;
    let mut curr = dw + 1; // skip outer buffer
    for _j in 1..data_height - 1 {
        // forwards in x
        for _i in 1..data_width - 1 {
            // don't need to calculate distance for edge pixels
            if edges[curr as usize] == 0 {
                f1(&mut data, curr as usize, data_width);
            }
            curr += 1;
        }

        // backwards in x
        curr -= 1; // reset to end
        for _i in 1..data_width - 1 {
            // don't need to calculate distance for edge pixels
            if edges[curr as usize] == 0 {
                f2(&mut data, curr as usize);
            }
            curr -= 1;
        }

        curr += dw + 1;
    }

    // backwards in y
    curr = dw * (data_height as isize - 2) - 1; // skip outer buffer
    for _j in 1..data_height - 1 {
        // forwards in x
        for _i in 1..data_width - 1 {
            // don't need to calculate distance for edge pixels
            if edges[curr as usize] == 0 {
                b1(&mut data, curr as usize);
            }
            curr += 1;
        }

        // backwards in x
        curr -= 1; // reset to end
        for _i in 1..data_width - 1 {
            // don't need to calculate distance for edge pixels
            if edges[curr as usize] == 0 {
                b2(&mut data, curr as usize, data_width);
            }
            curr -= 1;
        }

        curr -= dw - 1;
    }

    // copy results to final distance field data
    let mut curr = data_width + 1;
    let mut df_pos = 0usize;
    for _j in 1..data_height - 1 {
        for _i in 1..data_width - 1 {
            let dist = if data[curr].alpha > 0.5 {
                -scalar_sqrt(data[curr].dist_sq)
            } else {
                scalar_sqrt(data[curr].dist_sq)
            };
            distance_field[df_pos] = pack_distance_field_val(DISTANCE_FIELD_MAGNITUDE, dist);
            df_pos += 1;
            curr += 1;
        }
        curr += 2;
    }

    true
}

/// `SkGenerateDistanceFieldFromA8Image`: assumes an 8-bit image and distance field.
// Port of: src/core/SkDistanceFieldGen.cpp#L488-L512 (chrome/m156)
#[doc(alias = "SkGenerateDistanceFieldFromA8Image")]
pub fn generate_distance_field_from_a8_image(
    distance_field: &mut [u8],
    image: &[u8],
    width: usize,
    height: usize,
    row_bytes: usize,
) -> bool {
    // we copy our source image into a padded copy to ensure we catch edge transitions
    // around the outside
    let mut copy = vec![0u8; (width + 2) * (height + 2)];
    let mut dest = width + 2;
    for i in 0..height {
        dest += 1;
        copy[dest..dest + width].copy_from_slice(&image[i * row_bytes..i * row_bytes + width]);
        dest += width;
        dest += 1;
    }
    generate_distance_field_from_image(distance_field, &copy, width, height)
}

/// `SkGenerateDistanceFieldFromLCD16Mask`: assumes a 16-bit lcd mask and 8-bit distance field.
// Port of: src/core/SkDistanceFieldGen.cpp#L514-L541 (chrome/m156)
#[doc(alias = "SkGenerateDistanceFieldFromLCD16Mask")]
pub fn generate_distance_field_from_lcd16_mask(
    distance_field: &mut [u8],
    image: &[u8],
    w: usize,
    h: usize,
    row_bytes: usize,
) -> bool {
    // we copy our source image into a padded copy to ensure we catch edge transitions
    // around the outside
    let mut copy = vec![0u8; (w + 2) * (h + 2)];
    let mut dest = w + 2;
    for i in 0..h {
        dest += 1;
        let row = &image[i * row_bytes..];
        for x in 0..w {
            let packed = u32::from(u16::from_ne_bytes([row[2 * x], row[2 * x + 1]]));
            let r = packed16_to_r32(packed);
            let g = packed16_to_g32(packed);
            let b = packed16_to_b32(packed);
            // `(r + g + b) / 3` is at most 255.
            copy[dest] = u8::try_from((r + g + b) / 3).unwrap_or(u8::MAX);
            dest += 1;
        }
        dest += 1;
    }
    generate_distance_field_from_image(distance_field, &copy, w, h)
}

/// `SkGenerateDistanceFieldFromBWImage`: assumes a 1-bit image and 8-bit distance field.
// Port of: src/core/SkDistanceFieldGen.cpp#L543-L570 (chrome/m156)
#[doc(alias = "SkGenerateDistanceFieldFromBWImage")]
pub fn generate_distance_field_from_bw_image(
    distance_field: &mut [u8],
    image: &[u8],
    width: usize,
    height: usize,
    row_bytes: usize,
) -> bool {
    // we copy our source image into a padded copy to ensure we catch edge transitions
    // around the outside
    let mut copy = vec![0u8; (width + 2) * (height + 2)];
    let mut dest = width + 2;
    for i in 0..height {
        dest += 1;

        let mut row_writes_left = width;
        let mut mask_pos = i * row_bytes;
        while row_writes_left > 0 {
            let mask = image[mask_pos];
            mask_pos += 1;
            let mut j = 7;
            while j >= 0 && row_writes_left != 0 {
                copy[dest] = if mask & (1 << j) != 0 { 0xff } else { 0 };
                dest += 1;
                j -= 1;
                row_writes_left -= 1;
            }
        }

        dest += 1;
    }
    generate_distance_field_from_image(distance_field, &copy, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_includes_padding() {
        assert_eq!(compute_distance_field_size(10, 20), 18 * 28);
    }

    #[test]
    fn solid_square_is_negative_inside_and_positive_outside() {
        // A filled 8x8 A8 square: the field is padded by SK_DistanceFieldPad on each side.
        let (w, h) = (8usize, 8usize);
        let image = vec![255u8; w * h];
        let mut df = vec![0u8; compute_distance_field_size(8, 8)];
        assert!(generate_distance_field_from_a8_image(
            &mut df, &image, w, h, w
        ));
        let dfw = w + 2 * DISTANCE_FIELD_PAD as usize;
        let at = |x: usize, y: usize| df[y * dfw + x];
        // Inside the shape the packed value is above 128 (the zero), outside below.
        assert!(at(dfw / 2, dfw / 2) > 128);
        assert!(at(0, 0) < 128);
    }
}

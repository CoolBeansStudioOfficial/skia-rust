// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the YUV branch of `WebPCleanupTransparentArea` (`src/enc/picture_tools_enc.c`): the
//! luma and chroma of fully transparent 8x8 blocks are flattened to the average of the visible
//! luma around them, so that the lossy encoder spends no bits on invisible pixels. `WebPEncode`
//! runs it for lossy pictures unless `exact` is set.
//!
//! The ARGB branch (`pic->use_argb`) is not reached by the lossy path and is not ported.

// Module-level clippy allows, as in the neighbouring encoder modules.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_arguments,
    clippy::many_single_char_names
)]

use super::picture::YuvPicture;

/// Port of `SIZE` (the block size of the cleanup).
const SIZE: usize = 8;
/// Port of `SIZE2` (the chroma block size).
const SIZE2: usize = SIZE / 2;

/// Port of `Flatten`: sets `size` rows of `size` bytes to `v`.
fn flatten(plane: &mut [u8], offset: usize, v: u8, stride: usize, size: usize) {
    for y in 0..size {
        let start = offset + y * stride;
        for b in &mut plane[start..start + size] {
            *b = v;
        }
    }
}

/// Port of `SmoothenBlock`: fills the luma of the transparent pixels (alpha 0) of a `width` x
/// `height` block with the average luma of its visible pixels. Returns whether the block is
/// entirely transparent (`count == 0`).
fn smoothen_block(
    a: &[u8],
    a_off: usize,
    a_stride: usize,
    y: &mut [u8],
    y_off: usize,
    y_stride: usize,
    width: usize,
    height: usize,
) -> bool {
    let mut sum: i32 = 0;
    let mut count: i32 = 0;
    for row in 0..height {
        for x in 0..width {
            if a[a_off + row * a_stride + x] != 0 {
                count += 1;
                sum += i32::from(y[y_off + row * y_stride + x]);
            }
        }
    }
    if count > 0 && (count as usize) < width * height {
        let avg_u8 = (sum / count) as u8;
        for row in 0..height {
            for x in 0..width {
                if a[a_off + row * a_stride + x] == 0 {
                    y[y_off + row * y_stride + x] = avg_u8;
                }
            }
        }
    }
    count == 0
}

/// Port of `WebPCleanupTransparentArea` for a YUV picture with an alpha plane. Does nothing when
/// the picture has no alpha plane.
pub fn cleanup_transparent_area(pic: &mut YuvPicture) {
    let Some(a) = pic.a.as_ref() else {
        return;
    };
    let width = pic.width;
    let height = pic.height;
    let y_stride = width;
    let uv_stride = width.div_ceil(2);
    let a_stride = width;
    let a = a.clone();
    let y = &mut pic.y;
    let u = &mut pic.u;
    let v = &mut pic.v;
    let mut a_ptr = 0usize;
    let mut y_ptr = 0usize;
    let mut u_ptr = 0usize;
    let mut v_ptr = 0usize;
    let mut yy = 0usize;
    while yy + SIZE <= height {
        let mut need_reset = true;
        let mut values = [0u8; 3];
        let mut x = 0usize;
        while x + SIZE <= width {
            if smoothen_block(&a, a_ptr + x, a_stride, y, y_ptr + x, y_stride, SIZE, SIZE) {
                if need_reset {
                    values[0] = y[y_ptr + x];
                    values[1] = u[u_ptr + (x >> 1)];
                    values[2] = v[v_ptr + (x >> 1)];
                    need_reset = false;
                }
                flatten(y, y_ptr + x, values[0], y_stride, SIZE);
                flatten(u, u_ptr + (x >> 1), values[1], uv_stride, SIZE2);
                flatten(v, v_ptr + (x >> 1), values[2], uv_stride, SIZE2);
            } else {
                need_reset = true;
            }
            x += SIZE;
        }
        if x < width {
            smoothen_block(
                &a,
                a_ptr + x,
                a_stride,
                y,
                y_ptr + x,
                y_stride,
                width - x,
                SIZE,
            );
        }
        a_ptr += SIZE * a_stride;
        y_ptr += SIZE * y_stride;
        u_ptr += SIZE2 * uv_stride;
        v_ptr += SIZE2 * uv_stride;
        yy += SIZE;
    }
    if yy < height {
        let sub_height = height - yy;
        let mut x = 0usize;
        while x + SIZE <= width {
            smoothen_block(
                &a,
                a_ptr + x,
                a_stride,
                y,
                y_ptr + x,
                y_stride,
                SIZE,
                sub_height,
            );
            x += SIZE;
        }
        if x < width {
            smoothen_block(
                &a,
                a_ptr + x,
                a_stride,
                y,
                y_ptr + x,
                y_stride,
                width - x,
                sub_height,
            );
        }
    }
}

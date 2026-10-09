// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/RectanizerPow2.h, src/gpu/RectanizerPow2.cpp

//! `skgpu::RectanizerPow2`: quantizes rects to powers of two, one open shelf per power.

use crate::gpu::rectanizer::{IPoint16, Rectanizer};
use skia_rust_core::math_priv::{clz, next_pow2};

// Port of: src/gpu/RectanizerPow2.h#L49 (chrome/m156)
const K_MIN_HEIGHT_POW2: i32 = 2;
// Port of: src/gpu/RectanizerPow2.h#L50 (chrome/m156)
const K_MAX_EXPONENT: usize = 16;

// Port of: src/gpu/RectanizerPow2.h#L52-L61 (chrome/m156)
#[derive(Clone, Copy, Debug, Default)]
struct Row {
    loc: IPoint16,
    // fRowHeight is actually known by this struct's position in fRows, but it is used to signal
    // if there exists an open row of this height.
    row_height: i32,
}

impl Row {
    // Port of: src/gpu/RectanizerPow2.h#L56-L59 (chrome/m156)
    fn can_add_width(self, width: i32, container_width: i32) -> bool {
        i32::from(self.loc.x) + width <= container_width
    }
}

// This Rectanizer quantizes the incoming rects to powers of 2. Each power of two can have, at
// most, one active row/shelf. Once a row/shelf for a particular power of two gets full its
// fRows entry is recycled to point to a new row. The skyline algorithm almost always provides a
// better packing.
// Port of: src/gpu/RectanizerPow2.h#L26-L84 (chrome/m156)
#[doc(alias = "skgpu::RectanizerPow2")]
#[derive(Debug)]
pub struct RectanizerPow2 {
    width: i32,
    height: i32,
    // 0-th entry will be unused.
    rows: [Row; K_MAX_EXPONENT],
    next_strip_y: i32,
    area_so_far: i32,
}

impl RectanizerPow2 {
    // Port of: src/gpu/RectanizerPow2.h#L29-L33 (chrome/m156)
    /// `RectanizerPow2(w, h)`.
    #[must_use]
    pub fn new(w: i32, h: i32) -> RectanizerPow2 {
        debug_assert!(w >= 0);
        debug_assert!(h >= 0);
        let mut r = RectanizerPow2 {
            width: w,
            height: h,
            rows: [Row::default(); K_MAX_EXPONENT],
            next_strip_y: 0,
            area_so_far: 0,
        };
        r.reset();
        r
    }

    // Port of: src/gpu/RectanizerPow2.h#L68-L73 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // clz is at most 32 and the index is below 16 (asserted)
    fn height_to_row_index(height: i32) -> usize {
        debug_assert!(height >= K_MIN_HEIGHT_POW2);
        // SkCLZ returns 32 for zero; height - 1 is at least 1 here.
        let index = 32 - clz((height - 1) as u32);
        debug_assert!((index as usize) < K_MAX_EXPONENT);
        index as usize
    }

    // Port of: src/gpu/RectanizerPow2.h#L75-L77 (chrome/m156)
    fn can_add_strip(&self, height: i32) -> bool {
        self.next_strip_y + height <= self.height
    }

    // Port of: src/gpu/RectanizerPow2.h#L79-L83 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // the y position is below the atlas height
    fn init_row(&mut self, row_index: usize, row_height: i32) {
        let next_strip_y = self.next_strip_y;
        let row = &mut self.rows[row_index];
        // The y position always fits in an i16: it is below the atlas height.
        row.loc.set(0, next_strip_y as i16);
        row.row_height = row_height;
        self.next_strip_y += row_height;
    }
}

impl Rectanizer for RectanizerPow2 {
    // Port of: src/gpu/RectanizerPow2.h#L36-L41 (chrome/m156)
    fn reset(&mut self) {
        self.next_strip_y = 0;
        self.area_so_far = 0;
        self.rows = [Row::default(); K_MAX_EXPONENT];
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    // Port of: src/gpu/RectanizerPow2.cpp#L12-L60 (chrome/m156)
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // unsigned bound checks; the rect fits in the row, so x fits in an int16
    fn add_rect(&mut self, width: i32, mut height: i32) -> Option<IPoint16> {
        if width as u32 > self.width as u32 || height as u32 > self.height as u32 {
            return None;
        }

        // Computed here since height will be modified. C++ leaves overflow undefined; wrapping
        // keeps the release behaviour.
        let area = width.wrapping_mul(height);

        // SkNextPow2 is undefined for inputs <= 0. If small values happen to creep in here, round
        // them all up to the minimum power of 2.
        if height < K_MIN_HEIGHT_POW2 {
            height = K_MIN_HEIGHT_POW2;
        } else {
            height = next_pow2(height);
        }

        let row_index = Self::height_to_row_index(height);
        debug_assert!(
            self.rows[row_index].row_height == 0 || self.rows[row_index].row_height == height
        );
        if self.rows[row_index].row_height == 0 {
            if !self.can_add_strip(height) {
                return None;
            }
            self.init_row(row_index, height);
        } else if !self.rows[row_index].can_add_width(width, self.width) {
            if !self.can_add_strip(height) {
                return None;
            }
            // that row is now "full", so retarget our Row record for another one
            self.init_row(row_index, height);
        }

        let row = &mut self.rows[row_index];
        debug_assert_eq!(row.row_height, height);
        debug_assert!(row.can_add_width(width, self.width));
        let loc = row.loc;
        // Only reached when the rect fits, so the sum stays within the atlas width.
        row.loc.x += width as i16;
        debug_assert!(i32::from(row.loc.x) <= self.width);
        debug_assert!(i32::from(row.loc.y) <= self.height);
        debug_assert!(self.next_strip_y <= self.height);
        self.area_so_far = self.area_so_far.wrapping_add(area);
        Some(loc)
    }

    // Port of: src/gpu/RectanizerPow2.h#L44-L46 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the (float) casts in SkRectanizer
    fn percent_full(&self) -> f32 {
        self.area_so_far as f32 / (self.width as f32 * self.height as f32)
    }
}

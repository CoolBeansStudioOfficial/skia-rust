// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/RectanizerSkyline.h, src/gpu/RectanizerSkyline.cpp

//! `skgpu::RectanizerSkyline`: packs rects by tracking the silhouette of the packed area.
//! Based, in part, on Jukka Jylanki's work at <http://clb.demon.fi>.

use crate::gpu::rectanizer::{IPoint16, Rectanizer};

// Port of: src/gpu/RectanizerSkyline.h#L49-L53 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct SkylineSegment {
    x: i32,
    y: i32,
    width: i32,
}

/// Packs rectangles and tracks the current silhouette.
// Port of: src/gpu/RectanizerSkyline.h#L24-L63 (chrome/m156)
#[doc(alias = "skgpu::RectanizerSkyline")]
#[derive(Debug)]
pub struct RectanizerSkyline {
    width: i32,
    height: i32,
    skyline: Vec<SkylineSegment>,
    area_so_far: i32,
}

impl RectanizerSkyline {
    // Port of: src/gpu/RectanizerSkyline.h#L27-L30 (chrome/m156)
    /// `RectanizerSkyline(w, h)`.
    #[must_use]
    pub fn new(w: i32, h: i32) -> RectanizerSkyline {
        debug_assert!(w >= 0);
        debug_assert!(h >= 0);
        let mut r = RectanizerSkyline {
            width: w,
            height: h,
            skyline: Vec::new(),
            area_so_far: 0,
        };
        r.reset();
        r
    }

    // Port of: src/gpu/RectanizerSkyline.cpp#L63-L77 (chrome/m156)
    // Can a width x height rectangle fit in the free space represented by the skyline segments
    // >= 'skyline_index'? If so, returns the y-location at which it fits (the x location is pulled
    // from 'skyline_index's segment).
    fn rectangle_fits(&self, skyline_index: usize, width: i32, height: i32) -> Option<i32> {
        let x = self.skyline[skyline_index].x;
        if x + width > self.width {
            return None;
        }
        let mut width_left = width;
        let mut i = skyline_index;
        let mut y = self.skyline[skyline_index].y;
        while width_left > 0 {
            y = y.max(self.skyline[i].y);
            if y + height > self.height {
                return None;
            }
            width_left -= self.skyline[i].width;
            i += 1;
            debug_assert!(i < self.skyline.len() || width_left <= 0);
        }
        Some(y)
    }

    // Port of: src/gpu/RectanizerSkyline.cpp#L79-L121 (chrome/m156)
    // Updates the skyline structure to include a width x height rect located at x,y.
    fn add_skyline_level(&mut self, skyline_index: usize, x: i32, y: i32, width: i32, height: i32) {
        let new_segment = SkylineSegment {
            x,
            y: y + height,
            width,
        };
        self.skyline.insert(skyline_index, new_segment);

        debug_assert!(new_segment.x + new_segment.width <= self.width);
        debug_assert!(new_segment.y <= self.height);

        // delete width of the new skyline segment from following ones
        let i = skyline_index + 1;
        while i < self.skyline.len() {
            // The new segment subsumes all or part of fSkyline[i]
            debug_assert!(self.skyline[i - 1].x <= self.skyline[i].x);
            if self.skyline[i].x >= self.skyline[i - 1].x + self.skyline[i - 1].width {
                break;
            }
            let shrink = self.skyline[i - 1].x + self.skyline[i - 1].width - self.skyline[i].x;
            self.skyline[i].x += shrink;
            self.skyline[i].width -= shrink;
            if self.skyline[i].width > 0 {
                // only partially consumed
                break;
            }
            // fully consumed: C++ `remove(i); --i;` followed by `++i` retries the same index.
            self.skyline.remove(i);
        }

        // merge fSkylines
        let mut i = 0;
        while i + 1 < self.skyline.len() {
            if self.skyline[i].y == self.skyline[i + 1].y {
                self.skyline[i].width += self.skyline[i + 1].width;
                self.skyline.remove(i + 1);
                // C++ `--i` then `++i`: retry the same index.
            } else {
                i += 1;
            }
        }
    }
}

impl Rectanizer for RectanizerSkyline {
    // Port of: src/gpu/RectanizerSkyline.h#L32-L36 (chrome/m156)
    fn reset(&mut self) {
        self.area_so_far = 0;
        self.skyline.clear();
        self.skyline.push(SkylineSegment {
            x: 0,
            y: 0,
            width: self.width,
        });
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    // Port of: src/gpu/RectanizerSkyline.cpp#L17-L54 (chrome/m156)
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // unsigned bound checks; int16 store of a position inside the atlas
    fn add_rect(&mut self, width: i32, height: i32) -> Option<IPoint16> {
        if width as u32 > self.width as u32 || height as u32 > self.height as u32 {
            return None;
        }

        // find position for new rectangle
        let mut best_width = self.width + 1;
        let mut best_x = 0;
        let mut best_y = self.height + 1;
        let mut best_index: Option<usize> = None;
        for i in 0..self.skyline.len() {
            if let Some(y) = self.rectangle_fits(i, width, height) {
                // minimize y position first, then width of skyline
                if y < best_y || (y == best_y && self.skyline[i].width < best_width) {
                    best_index = Some(i);
                    best_width = self.skyline[i].width;
                    best_x = self.skyline[i].x;
                    best_y = y;
                }
            }
        }

        // add rectangle to skyline
        if let Some(best_index) = best_index {
            self.add_skyline_level(best_index, best_x, best_y, width, height);
            self.area_so_far = self.area_so_far.wrapping_add(width.wrapping_mul(height));
            // The position comes from a skyline segment, which lies inside the atlas.
            return Some(IPoint16::new(best_x as i16, best_y as i16));
        }
        None
    }

    // Port of: src/gpu/RectanizerSkyline.h#L40-L42 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the (float) casts in SkRectanizer
    fn percent_full(&self) -> f32 {
        self.area_so_far as f32 / (self.width as f32 * self.height as f32)
    }
}

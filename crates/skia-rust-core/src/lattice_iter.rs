// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkLatticeIter.{h,cpp}, include/core/SkCanvas.h (`SkCanvas::Lattice`)

//! `SkCanvas::Lattice` and `SkLatticeIter`: dividing an image into a grid of patches that are
//! drawn fixed or stretched (`drawImageNine`, `drawImageLattice`).

use crate::color::Color;
use crate::matrix::Matrix;
use crate::rect::{Contains, IRect, Rect};
use crate::scalar::scalar;

/// Optional setting per rectangular grid entry to make it transparent, or to fill the grid entry
/// with a color (`SkCanvas::Lattice::RectType`).
// Port of: include/core/SkCanvas.h#L1546-L1550 (chrome/m156)
#[doc(alias = "SkCanvas::Lattice::RectType")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum RectType {
    /// Draws the image into the lattice rectangle (`kDefault`).
    #[default]
    Default = 0,
    /// Skips the lattice rectangle by making it transparent (`kTransparent`).
    Transparent,
    /// Draws one of `colors` into the lattice rectangle (`kFixedColor`).
    FixedColor,
}

/// Divides an image into a rectangular grid (`SkCanvas::Lattice`). Grid entries on even columns
/// and even rows are fixed; these entries are always drawn at their original size if the
/// destination is large enough. If the destination side is too small to hold the fixed entries,
/// all fixed entries are proportionately scaled down to fit. The grid entries not on even
/// columns and rows are scaled to fit the remaining space, if any.
// Port of: include/core/SkCanvas.h#L1536-L1583 (chrome/m156)
#[doc(alias = "SkCanvas::Lattice")]
#[derive(Clone, Debug)]
pub struct Lattice<'a> {
    /// x-axis values dividing the bitmap (`fXDivs`, `fXCount`).
    pub x_divs: &'a [i32],
    /// y-axis values dividing the bitmap (`fYDivs`, `fYCount`).
    pub y_divs: &'a [i32],
    /// Array of fill types (`fRectTypes`).
    pub rect_types: Option<&'a [RectType]>,
    /// Source bounds to draw from (`fBounds`).
    pub bounds: Option<IRect>,
    /// Array of colors (`fColors`).
    pub colors: Option<&'a [Color]>,
}

/// One patch of a lattice: the source and destination rects, and the color if the patch is
/// filled with a fixed color.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LatticePatch {
    /// The source rect, in the image.
    pub src: IRect,
    /// The destination rect.
    pub dst: Rect,
    /// `Some` if the patch is filled with a fixed color (`isFixedColor`, `fixedColor`).
    pub fixed_color: Option<Color>,
}

/// Iterates over the patches of a lattice (`SkLatticeIter`).
// Port of: src/core/SkLatticeIter.h#L19-L68 (chrome/m156)
#[doc(alias = "SkLatticeIter")]
#[derive(Clone, Debug)]
pub struct LatticeIter {
    src_x: Vec<i32>,
    src_y: Vec<i32>,
    dst_x: Vec<scalar>,
    dst_y: Vec<scalar>,
    rect_types: Vec<RectType>,
    colors: Vec<Color>,

    curr_x: usize,
    curr_y: usize,
    num_rects_in_lattice: usize,
    num_rects_to_draw: usize,
}

/// Divs must be in increasing order with no duplicates.
// Port of: src/core/SkLatticeIter.cpp#L18-L28 (chrome/m156)
fn valid_divs(divs: &[i32], start: i32, end: i32) -> bool {
    let mut prev = start - 1;
    for &div in divs {
        if prev >= div || div > end {
            return false;
        }
        prev = div;
    }

    true
}

/// Count the number of pixels that are in "scalable" patches.
// Port of: src/core/SkLatticeIter.cpp#L62-L87 (chrome/m156)
fn count_scalable_pixels(divs: &[i32], first_is_scalable: bool, start: i32, end: i32) -> i32 {
    if divs.is_empty() {
        return if first_is_scalable { end - start } else { 0 };
    }

    let mut i;
    let mut count;
    if first_is_scalable {
        count = divs[0] - start;
        i = 1;
    } else {
        count = 0;
        i = 0;
    }

    while i < divs.len() {
        // Alternatively, we could use |top| and |bottom| as variable names, instead of
        // |left| and |right|.
        let left = divs[i];
        let right = if i + 1 < divs.len() { divs[i + 1] } else { end };
        count += right - left;
        i += 2;
    }

    count
}

/// Set points for the src and dst rects on subsequent draw calls.
// Port of: src/core/SkLatticeIter.cpp#L89-L127 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::cast_precision_loss)] // mirrors the (float) casts
fn set_points(
    dst: &mut [scalar],
    src: &mut [i32],
    divs: &[i32],
    src_fixed: i32,
    src_scalable: i32,
    src_start: i32,
    src_end: i32,
    dst_start: scalar,
    dst_end: scalar,
    mut is_scalable: bool,
) {
    let dst_len = dst_end - dst_start;
    let scale = if src_fixed as f32 <= dst_len {
        // This is the "normal" case, where we scale the "scalable" patches and leave
        // the other patches fixed.
        (dst_len - (src_fixed as f32)) / (src_scalable as f32)
    } else {
        // In this case, we eliminate the "scalable" patches and scale the "fixed" patches.
        dst_len / (src_fixed as f32)
    };

    src[0] = src_start;
    dst[0] = dst_start;
    for (i, &div) in divs.iter().enumerate() {
        src[i + 1] = div;
        let src_delta = src[i + 1] - src[i];
        let dst_delta = if src_fixed as f32 <= dst_len {
            if is_scalable {
                scale * src_delta as f32
            } else {
                src_delta as f32
            }
        } else if is_scalable {
            0.0f32
        } else {
            scale * src_delta as f32
        };
        dst[i + 1] = dst[i] + dst_delta;

        // Alternate between "scalable" and "fixed" patches.
        is_scalable = !is_scalable;
    }

    src[divs.len() + 1] = src_end;
    dst[divs.len() + 1] = dst_end;
}

impl LatticeIter {
    /// Whether `lattice` is valid for an image of the given size (`Valid(width, height,
    /// lattice)`). `lattice.bounds` must be set.
    // Port of: src/core/SkLatticeIter.cpp#L30-L52 (chrome/m156)
    #[must_use]
    #[allow(clippy::similar_names)] // Skia's `zero_x_divs`/`zero_y_divs`
    pub fn valid(width: i32, height: i32, lattice: &Lattice<'_>) -> bool {
        let total_bounds = IRect::from_wh(width, height);
        let lattice_bounds = lattice.bounds.expect("the lattice has bounds");
        if !total_bounds.contains(&lattice_bounds) {
            return false;
        }

        let zero_x_divs = lattice.x_divs.is_empty()
            || (1 == lattice.x_divs.len() && lattice_bounds.left == lattice.x_divs[0]);
        let zero_y_divs = lattice.y_divs.is_empty()
            || (1 == lattice.y_divs.len() && lattice_bounds.top == lattice.y_divs[0]);
        if zero_x_divs && zero_y_divs {
            return false;
        }

        valid_divs(lattice.x_divs, lattice_bounds.left, lattice_bounds.right)
            && valid_divs(lattice.y_divs, lattice_bounds.top, lattice_bounds.bottom)
    }

    /// Whether the nine-patch `center` is valid for an image of the given size
    /// (`Valid(width, height, center)`).
    // Port of: src/core/SkLatticeIter.cpp#L209-L211 (chrome/m156)
    #[must_use]
    pub fn valid_center(width: i32, height: i32, center: &IRect) -> bool {
        !center.is_empty() && IRect::from_wh(width, height).contains(center)
    }

    /// An iterator over the patches of `lattice` stretched to `dst`
    /// (`SkLatticeIter(lattice, dst)`). `lattice.bounds` must be set.
    // Port of: src/core/SkLatticeIter.cpp#L129-L207 (chrome/m156)
    #[must_use]
    #[allow(clippy::similar_names)] // Skia's `orig_x_count`/`orig_y_count`
    pub fn new(lattice: &Lattice<'_>, dst: &Rect) -> LatticeIter {
        let mut x_divs = lattice.x_divs;
        let orig_x_count = lattice.x_divs.len();
        let mut y_divs = lattice.y_divs;
        let orig_y_count = lattice.y_divs.len();
        let src = lattice.bounds.expect("the lattice has bounds");

        // In the x-dimension, the first rectangle always starts at x = 0 and is "scalable".
        // If xDiv[0] is 0, it indicates that the first rectangle is degenerate, so the
        // first real rectangle "scalable" in the x-direction.
        //
        // The same interpretation applies to the y-dimension.
        //
        // As we move left to right across the image, alternating patches will be "fixed" or
        // "scalable" in the x-direction.  Similarly, as move top to bottom, alternating
        // patches will be "fixed" or "scalable" in the y-direction.
        let x_is_scalable = !x_divs.is_empty() && src.left == x_divs[0];
        if x_is_scalable {
            // Once we've decided that the first patch is "scalable", we don't need the
            // xDiv.  It is always implied that we start at the edge of the bounds.
            x_divs = &x_divs[1..];
        }
        let y_is_scalable = !y_divs.is_empty() && src.top == y_divs[0];
        if y_is_scalable {
            // Once we've decided that the first patch is "scalable", we don't need the
            // yDiv.  It is always implied that we start at the edge of the bounds.
            y_divs = &y_divs[1..];
        }
        let x_count = x_divs.len();
        let y_count = y_divs.len();

        // Count "scalable" and "fixed" pixels in each dimension.
        let x_count_scalable = count_scalable_pixels(x_divs, x_is_scalable, src.left, src.right);
        let x_count_fixed = src.width() - x_count_scalable;
        let y_count_scalable = count_scalable_pixels(y_divs, y_is_scalable, src.top, src.bottom);
        let y_count_fixed = src.height() - y_count_scalable;

        let mut src_x = vec![0; x_count + 2];
        let mut dst_x = vec![0.0; x_count + 2];
        set_points(
            &mut dst_x,
            &mut src_x,
            x_divs,
            x_count_fixed,
            x_count_scalable,
            src.left,
            src.right,
            dst.left,
            dst.right,
            x_is_scalable,
        );

        let mut src_y = vec![0; y_count + 2];
        let mut dst_y = vec![0.0; y_count + 2];
        set_points(
            &mut dst_y,
            &mut src_y,
            y_divs,
            y_count_fixed,
            y_count_scalable,
            src.top,
            src.bottom,
            dst.top,
            dst.bottom,
            y_is_scalable,
        );

        let num_rects_in_lattice = (x_count + 1) * (y_count + 1);
        let mut iter = LatticeIter {
            src_x,
            src_y,
            dst_x,
            dst_y,
            rect_types: Vec::new(),
            colors: Vec::new(),
            curr_x: 0,
            curr_y: 0,
            num_rects_in_lattice,
            num_rects_to_draw: num_rects_in_lattice,
        };

        if let Some(rect_types) = lattice.rect_types {
            let colors = lattice
                .colors
                .expect("a lattice with rect types has colors");
            iter.rect_types = vec![RectType::Default; num_rects_in_lattice];
            iter.colors = vec![Color::new(0); num_rects_in_lattice];

            let mut flags = 0;
            let has_pad_row = y_count != orig_y_count;
            let has_pad_col = x_count != orig_x_count;
            if has_pad_row {
                // The first row of rects are all empty, skip the first row of flags.
                flags += orig_x_count + 1;
            }

            let mut i = 0;
            for _y in 0..=y_count {
                for x in 0..=orig_x_count {
                    if 0 == x && has_pad_col {
                        // The first column of rects are all empty.  Skip a rect.
                        flags += 1;
                        continue;
                    }

                    iter.rect_types[i] = rect_types[flags];
                    iter.colors[i] = if RectType::FixedColor == rect_types[flags] {
                        colors[flags]
                    } else {
                        Color::new(0)
                    };
                    flags += 1;
                    i += 1;
                }
            }

            for ty in &iter.rect_types {
                if RectType::Transparent == *ty {
                    iter.num_rects_to_draw -= 1;
                }
            }
        }
        iter
    }

    /// An iterator over the nine patches of an image of `w` x `h` with the center `c`
    /// stretched to `dst` (`SkLatticeIter(w, h, c, dst)`).
    // Port of: src/core/SkLatticeIter.cpp#L213-L255 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar and the int * float math
    pub fn new_center(w: i32, h: i32, c: &IRect, dst: &Rect) -> LatticeIter {
        debug_assert!(IRect::from_wh(w, h).contains(c));

        let src_x = vec![0, c.left, c.right, w];
        let src_y = vec![0, c.top, c.bottom, h];

        let mut dst_x = vec![
            dst.left,
            dst.left + c.left as f32,
            dst.right - (w - c.right) as f32,
            dst.right,
        ];
        let mut dst_y = vec![
            dst.top,
            dst.top + c.top as f32,
            dst.bottom - (h - c.bottom) as f32,
            dst.bottom,
        ];

        if dst_x[1] > dst_x[2] {
            dst_x[1] = dst_x[0] + (dst_x[3] - dst_x[0]) * c.left as f32 / (w - c.width()) as f32;
            dst_x[2] = dst_x[1];
        }

        if dst_y[1] > dst_y[2] {
            dst_y[1] = dst_y[0] + (dst_y[3] - dst_y[0]) * c.top as f32 / (h - c.height()) as f32;
            dst_y[2] = dst_y[1];
        }

        LatticeIter {
            src_x,
            src_y,
            dst_x,
            dst_y,
            rect_types: Vec::new(),
            colors: Vec::new(),
            curr_x: 0,
            curr_y: 0,
            num_rects_in_lattice: 9,
            num_rects_to_draw: 9,
        }
    }

    /// The next patch to draw, or `None` when done (`next`). Transparent patches are skipped.
    // Port of: src/core/SkLatticeIter.cpp#L257-L291 (chrome/m156)
    pub fn next_patch(&mut self) -> Option<LatticePatch> {
        loop {
            let curr_rect = self.curr_x + self.curr_y * (self.src_x.len() - 1);
            if curr_rect == self.num_rects_in_lattice {
                return None;
            }

            let x = self.curr_x;
            let y = self.curr_y;
            debug_assert!(x < self.src_x.len() - 1);
            debug_assert!(y < self.src_y.len() - 1);

            self.curr_x += 1;
            if self.src_x.len() - 1 == self.curr_x {
                self.curr_x = 0;
                self.curr_y += 1;
            }

            if !self.rect_types.is_empty() && RectType::Transparent == self.rect_types[curr_rect] {
                continue;
            }

            let src = IRect::new(
                self.src_x[x],
                self.src_y[y],
                self.src_x[x + 1],
                self.src_y[y + 1],
            );
            let dst = Rect::new(
                self.dst_x[x],
                self.dst_y[y],
                self.dst_x[x + 1],
                self.dst_y[y + 1],
            );
            let fixed_color = (!self.rect_types.is_empty()
                && RectType::FixedColor == self.rect_types[curr_rect])
                .then(|| self.colors[curr_rect]);
            return Some(LatticePatch {
                src,
                dst,
                fixed_color,
            });
        }
    }

    /// Applies a scale+translate matrix to the dst points (`mapDstScaleTranslate`).
    // Port of: src/core/SkLatticeIter.cpp#L293-L307 (chrome/m156)
    #[doc(alias = "mapDstScaleTranslate")]
    pub fn map_dst_scale_translate(&mut self, matrix: &Matrix) {
        debug_assert!(matrix.is_scale_translate());
        let tx = matrix.translate_x();
        let sx = matrix.scale_x();
        for v in &mut self.dst_x {
            *v = *v * sx + tx;
        }

        let ty = matrix.translate_y();
        let sy = matrix.scale_y();
        for v in &mut self.dst_y {
            *v = *v * sy + ty;
        }
    }

    /// The number of rects that will actually be drawn (`numRectsToDraw`).
    #[doc(alias = "numRectsToDraw")]
    #[must_use]
    pub fn num_rects_to_draw(&self) -> usize {
        self.num_rects_to_draw
    }
}

impl Iterator for LatticeIter {
    type Item = LatticePatch;

    fn next(&mut self) -> Option<LatticePatch> {
        self.next_patch()
    }
}

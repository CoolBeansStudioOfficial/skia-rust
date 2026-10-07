// Copyright 2016 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan_AAAPath.cpp (AdditiveBlitter and its subclasses)

//! The additive blitters of the analytic AA scan converter: they accumulate coverage before
//! handing it to the real blitter.

use skia_rust_core::color::Alpha;
use skia_rust_core::fixed::{Fixed, fixed_floor_to_int};
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::rect::IRect;

use crate::alpha_runs::AlphaRuns;
use crate::blitter::{BlitMemory, Blitter};

// `SkAlphaRuns::CatchOverflow` without its debug check: Skia's release builds (the oracle) wrap
// silently if the sum exceeds 256.
#[allow(clippy::cast_possible_truncation)] // mirrors the implicit int -> SkAlpha conversion
#[allow(clippy::cast_sign_loss)] // mirrors the implicit int -> SkAlpha conversion
fn catch_overflow(alpha: i32) -> Alpha {
    (alpha - (alpha >> 8)) as Alpha
}

// Port of: src/core/SkScan_AAAPath.cpp#L91-L94 (chrome/m156)
pub(super) fn add_alpha(alpha: &mut Alpha, delta: Alpha) {
    debug_assert!(i32::from(*alpha) + i32::from(delta) <= 256);
    *alpha = catch_overflow(i32::from(*alpha) + i32::from(delta));
}

// Port of: src/core/SkScan_AAAPath.cpp#L96-L98 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // the value is in [0, 255]
pub(super) fn safely_add_alpha(alpha: &mut Alpha, delta: Alpha) {
    *alpha = 0xFF.min(i32::from(*alpha) + i32::from(delta)) as Alpha;
}

/// A row of a [`MaskAdditiveBlitter`]'s mask: `storage[row + x]` is the alpha of `(x, y)` (the
/// C++ `SkAlpha* maskRow`, a pointer with `maskRow[x]` the alpha of `(x, y)`).
pub type MaskRow = isize;

/// A blitter that accumulates (adds) coverage before handing it to the real blitter
/// (`AdditiveBlitter`).
///
/// skia-rust: the `SkBlitter` overrides of the C++ class that only `SkDEBUGFAIL` are not part of
/// the trait; [`Self::real_blitter`] returns the blitter to call them on.
// Port of: src/core/SkScan_AAAPath.cpp#L100-L136 (chrome/m156)
pub trait AdditiveBlitter {
    /// `getRealBlitter()`: the mask blitter returns itself (so that rectangles are blitted into
    /// the mask), the run-based ones the wrapped blitter.
    ///
    /// skia-rust: the `forceRealBlitter` parameter is not ported; `SkScan_AAAPath.cpp` never
    /// passes true.
    #[doc(alias = "getRealBlitter")]
    fn real_blitter(&mut self) -> &mut dyn Blitter;

    /// `blitAntiH(x, y, antialias[], len)`: adds `antialias` to `len` pixels.
    fn blit_anti_h_alphas(&mut self, x: i32, y: i32, antialias: &[Alpha]);
    /// `blitAntiH(x, y, alpha)`: adds `alpha` to one pixel.
    fn blit_anti_h_alpha(&mut self, x: i32, y: i32, alpha: Alpha);
    /// `blitAntiH(x, y, width, alpha)`: adds `alpha` to `width` pixels.
    fn blit_anti_h_width(&mut self, x: i32, y: i32, width: i32, alpha: Alpha);

    /// `getWidth()`.
    #[doc(alias = "getWidth")]
    fn width(&self) -> i32;

    /// Flush the additive alpha cache if `floor(y)` and `floor(next_y)` are different (i.e.,
    /// we'll start working on a new pixel row).
    fn flush_if_y_changed(&mut self, y: Fixed, next_y: Fixed);

    /// `static_cast<MaskAdditiveBlitter*>(this)->getRow(y)`: only valid on a
    /// [`MaskAdditiveBlitter`].
    fn get_row(&mut self, _y: i32) -> MaskRow {
        unreachable!("only the mask blitter has rows")
    }

    /// `maskRow[x]`: only valid on a [`MaskAdditiveBlitter`].
    fn mask_byte(&mut self, _row: MaskRow, _x: i32) -> &mut Alpha {
        unreachable!("only the mask blitter has rows")
    }
}

/// So we don't try to do very wide things, where the RLE blitter would be faster.
// Port of: src/core/SkScan_AAAPath.cpp#L194-L196 (chrome/m156)
const MASK_MAX_WIDTH: i32 = 32;
const MASK_MAX_STORAGE: i32 = 1024;
// `uint32_t fStorage[(kMAX_STORAGE >> 2) + 2]`, in bytes: we add 2 because we can write 1 extra
// byte at either end due to precision error.
#[allow(clippy::cast_sign_loss)] // a positive constant
const MASK_STORAGE_BYTES: usize = (((MASK_MAX_STORAGE >> 2) + 2) * 4) as usize;

/// Accumulates coverage directly in an A8 mask, then blits the mask once (when dropped). We need
/// this mask blitter because it significantly accelerates small path filling.
// Port of: src/core/SkScan_AAAPath.cpp#L138-L280 (chrome/m156)
#[derive(Debug)]
pub struct MaskAdditiveBlitter<'a> {
    real_blitter: &'a mut dyn Blitter,
    // fMask: its image is `storage[1..]`.
    mask_bounds: IRect,
    row_bytes: i32,
    clip_rect: IRect,
    storage: [u8; MASK_STORAGE_BYTES],
    blit_memory: BlitMemory,
}

impl<'a> MaskAdditiveBlitter<'a> {
    // Port of: src/core/SkScan_AAAPath.cpp#L208-L227 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // the mask size is checked by can_handle_rect
    pub(super) fn new(
        real_blitter: &'a mut dyn Blitter,
        ir: &IRect,
        clip_bounds: &IRect,
        is_inverse: bool,
    ) -> Self {
        debug_assert!(Self::can_handle_rect(ir));
        debug_assert!(!is_inverse);

        let clip_rect = IRect::intersect(ir, clip_bounds).unwrap_or_else(|| {
            debug_assert!(false);
            IRect::new_empty()
        });

        // memset(fStorage, 0, fMask.fBounds.height() * fMask.fRowBytes + 2): the rest of the
        // storage is never read, so all of it is zeroed here.
        MaskAdditiveBlitter {
            real_blitter,
            mask_bounds: *ir,
            row_bytes: ir.width(),
            clip_rect,
            storage: [0; MASK_STORAGE_BYTES],
            blit_memory: BlitMemory::default(),
        }
    }

    /// `CanHandleRect`: whether the mask of `bounds` is small enough for this blitter.
    // Port of: src/core/SkScan_AAAPath.cpp#L171-L182 (chrome/m156)
    #[doc(alias = "CanHandleRect")]
    #[must_use]
    pub fn can_handle_rect(bounds: &IRect) -> bool {
        let width = bounds.width();
        if width > MASK_MAX_WIDTH {
            return false;
        }
        let rb = i64::from(skia_rust_core::align::align4(width));
        // use 64bits to detect overflow
        let storage = rb * i64::from(bounds.height());

        (width <= MASK_MAX_WIDTH) && (storage <= i64::from(MASK_MAX_STORAGE))
    }

    // `getRow(y)` (without its cache): `storage[row + x]` is `fMask.image()[(y - fTop) *
    // fRowBytes + x - fLeft]`.
    // Port of: src/core/SkScan_AAAPath.cpp#L184-L191 (chrome/m156)
    fn row(&self, y: i32) -> MaskRow {
        1 + isize::try_from((y - self.mask_bounds.top) * self.row_bytes - self.mask_bounds.left)
            .expect("fits")
    }

    #[allow(clippy::cast_sign_loss)] // asserted non-negative
    fn byte(&mut self, row: MaskRow, x: i32) -> &mut Alpha {
        let i = row + isize::try_from(x).expect("fits");
        debug_assert!(i >= 0);
        &mut self.storage[i as usize]
    }
}

impl Drop for MaskAdditiveBlitter<'_> {
    // Port of: src/core/SkScan_AAAPath.cpp#L145 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // row_bytes is a positive width
    fn drop(&mut self) {
        let mask = Mask::new(
            &self.storage[1..],
            self.mask_bounds,
            self.row_bytes as u32,
            MaskFormat::A8,
        );
        self.real_blitter.blit_mask(&mask, &self.clip_rect);
    }
}

impl AdditiveBlitter for MaskAdditiveBlitter<'_> {
    // Most of the time, we still consider this mask blitter as the real blitter so we can
    // accelerate blitRect and others.
    // Port of: src/core/SkScan_AAAPath.cpp#L147-L152 (chrome/m156)
    fn real_blitter(&mut self) -> &mut dyn Blitter {
        self
    }

    // Virtual function is slow. So don't use this. Directly add alpha to the mask instead.
    // Port of: src/core/SkScan_AAAPath.cpp#L229-L231 (chrome/m156)
    fn blit_anti_h_alphas(&mut self, _x: i32, _y: i32, _antialias: &[Alpha]) {
        panic!("Don't use this; directly add alphas to the mask.");
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L233-L236 (chrome/m156)
    fn blit_anti_h_alpha(&mut self, x: i32, y: i32, alpha: Alpha) {
        debug_assert!(x >= self.mask_bounds.left - 1);
        let row = self.row(y);
        add_alpha(self.byte(row, x), alpha);
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L238-L244 (chrome/m156)
    fn blit_anti_h_width(&mut self, x: i32, y: i32, width: i32, alpha: Alpha) {
        debug_assert!(x >= self.mask_bounds.left - 1);
        let row = self.row(y);
        for i in 0..width {
            add_alpha(self.byte(row, x + i), alpha);
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L169 (chrome/m156)
    fn width(&self) -> i32 {
        self.clip_rect.width()
    }

    // The flush is only needed for RLE (RunBasedAdditiveBlitter)
    // Port of: src/core/SkScan_AAAPath.cpp#L167 (chrome/m156)
    fn flush_if_y_changed(&mut self, _y: Fixed, _next_y: Fixed) {}

    fn get_row(&mut self, y: i32) -> MaskRow {
        self.row(y)
    }

    fn mask_byte(&mut self, row: MaskRow, x: i32) -> &mut Alpha {
        self.byte(row, x)
    }
}

impl Blitter for MaskAdditiveBlitter<'_> {
    // Port of: src/core/SkScan_AAAPath.cpp#L118-L120 (chrome/m156)
    fn blit_h(&mut self, _x: i32, _y: i32, _width: i32) {
        debug_assert!(false, "Please call real blitter's blitH instead.");
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L110-L112 (chrome/m156)
    fn blit_anti_h(&mut self, _x: i32, _y: i32, _antialias: &mut [Alpha], _runs: &mut [i16]) {
        debug_assert!(false, "Please call real blitter's blitAntiH instead.");
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L246-L258 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        if alpha == 0 {
            return;
        }
        debug_assert!(x >= self.mask_bounds.left - 1);
        // This must be called as if this is a real blitter.
        // So we directly set alpha rather than adding it.
        let mut row = self.row(y);
        for _ in 0..height {
            *self.byte(row, x) = alpha;
            row += isize::try_from(self.row_bytes).expect("fits");
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L260-L269 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        debug_assert!(x >= self.mask_bounds.left - 1);
        // This must be called as if this is a real blitter.
        // So we directly set alpha rather than adding it.
        let mut row = self.row(y);
        for _ in 0..height {
            for i in 0..width {
                *self.byte(row, x + i) = 0xFF;
            }
            row += isize::try_from(self.row_bytes).expect("fits");
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L271-L280 (chrome/m156)
    fn blit_anti_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    ) {
        self.blit_v(x, y, height, left_alpha);
        self.blit_v(x + 1 + width, y, height, right_alpha);
        self.blit_rect(x + 1, y, width, height);
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.blit_memory
    }
}

/// Accumulates coverage of one row in [`AlphaRuns`] and blits the row to the real blitter when
/// the scan converter moves to another row (`RunBasedAdditiveBlitter`), or, with `safe` set,
/// clamps the accumulated alpha to 255 (`SafeRLEAdditiveBlitter`, which exists specifically for
/// concave path filling: in those cases, we can easily accumulate alpha greater than 0xFF).
///
/// skia-rust: Skia keeps `requestRowsPreserved()` rows of runs in a circular buffer carved out of
/// the real blitter's `allocBlitMemory`, so a blitter may hold on to earlier rows. Rust blitters
/// cannot keep borrows across calls, so one [`AlphaRuns`] owned by this blitter is reset after
/// each row instead; the calls the real blitter sees are the same.
// Port of: src/core/SkScan_AAAPath.cpp#L282-L532 (chrome/m156)
#[doc(alias = "SafeRLEAdditiveBlitter")]
#[derive(Debug)]
pub struct RunBasedAdditiveBlitter<'a> {
    real_blitter: &'a mut dyn Blitter,
    safe: bool,

    curr_y: i32, // Current y coordinate.
    width: i32,  // Widest row of region to be blitted
    left: i32,   // Leftmost x coordinate in any row
    top: i32,    // Initial y coordinate (top of bounds)

    runs: AlphaRuns,

    offset_x: i32,
}

impl<'a> RunBasedAdditiveBlitter<'a> {
    /// `RunBasedAdditiveBlitter(realBlitter, ir, clipBounds, isInverse)`, or with `safe` set
    /// `SafeRLEAdditiveBlitter(...)`.
    // Port of: src/core/SkScan_AAAPath.cpp#L369-L401 (chrome/m156)
    // Port of: src/core/SkScan_AAAPath.cpp#L462-L466 (chrome/m156)
    pub(super) fn new(
        real_blitter: &'a mut dyn Blitter,
        ir: &IRect,
        clip_bounds: &IRect,
        is_inverse: bool,
        safe: bool,
    ) -> Self {
        let sect_bounds = if is_inverse {
            // We use the clip bounds instead of the ir, since we may be asked to
            // draw outside of the rect when we're a inverse filltype
            *clip_bounds
        } else {
            IRect::intersect(ir, clip_bounds).unwrap_or_else(IRect::new_empty)
        };

        let left = sect_bounds.left;
        let right = sect_bounds.right;

        let width = right - left;
        let top = sect_bounds.top;
        // advanceRuns()
        let runs = AlphaRuns::new(width);

        RunBasedAdditiveBlitter {
            real_blitter,
            safe,
            curr_y: top - 1,
            width,
            left,
            top,
            runs,
            offset_x: 0,
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L324 (chrome/m156)
    fn check(&self, x: i32, width: i32) -> bool {
        x >= 0 && x + width <= self.width
    }

    // Updates the runs to point to the next buffer space (here: the same one) and resets it to
    // an empty scanline.
    // Port of: src/core/SkScan_AAAPath.cpp#L329-L339 (chrome/m156)
    fn advance_runs(&mut self) {
        self.runs.reset(self.width);
    }

    // Blitting 0xFF and 0 is much faster so we snap alphas close to them
    // Port of: src/core/SkScan_AAAPath.cpp#L341-L342 (chrome/m156)
    fn snap_alpha(alpha: Alpha) -> Alpha {
        if alpha > 247 {
            0xFF
        } else if alpha < 8 {
            0x00
        } else {
            alpha
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L344-L359 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // run lengths are positive
    fn flush(&mut self) {
        if self.curr_y >= self.top {
            let mut x = 0usize;
            while self.runs.runs[x] != 0 {
                // It seems that blitting 255 or 0 is much faster than blitting 254 or 1
                self.runs.alpha[x] = Self::snap_alpha(self.runs.alpha[x]);
                x += self.runs.runs[x] as usize;
            }
            if !self.runs.is_empty() {
                self.real_blitter.blit_anti_h(
                    self.left,
                    self.curr_y,
                    &mut self.runs.alpha,
                    &mut self.runs.runs,
                );
                self.advance_runs();
                self.offset_x = 0;
            }
            self.curr_y = self.top - 1;
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L361-L366 (chrome/m156)
    fn check_y(&mut self, y: i32) {
        if y != self.curr_y {
            self.flush();
            self.curr_y = y;
        }
    }
}

impl Drop for RunBasedAdditiveBlitter<'_> {
    // Port of: src/core/SkScan_AAAPath.cpp#L289 (chrome/m156)
    fn drop(&mut self) {
        self.flush();
    }
}

impl AdditiveBlitter for RunBasedAdditiveBlitter<'_> {
    // Port of: src/core/SkScan_AAAPath.cpp#L291 (chrome/m156)
    fn real_blitter(&mut self) -> &mut dyn Blitter {
        &mut *self.real_blitter
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L403-L430 (chrome/m156)
    // Port of: src/core/SkScan_AAAPath.cpp#L473-L500 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // indices are checked non-negative
    fn blit_anti_h_alphas(&mut self, x: i32, y: i32, antialias: &[Alpha]) {
        self.check_y(y);
        let mut x = x - self.left;
        let mut antialias = antialias;
        let mut len = i32::try_from(antialias.len()).expect("fits");

        if x < 0 {
            len += x;
            antialias = &antialias[(-x) as usize..];
            x = 0;
        }
        len = len.min(self.width - x);
        debug_assert!(self.check(x, len));

        if x < self.offset_x {
            self.offset_x = 0;
        }

        self.offset_x = self.runs.add(x, 0, len, 0, 0, self.offset_x); // Break the run
        let xu = x as usize;
        let mut i = 0;
        while i < len {
            let iu = i as usize;
            let n = self.runs.runs[xu + iu] as usize;
            for j in 1..n {
                self.runs.runs[xu + iu + j] = 1;
                self.runs.alpha[xu + iu + j] = self.runs.alpha[xu + iu];
            }
            self.runs.runs[xu + iu] = 1;
            i += i32::from(self.runs.runs[xu + iu]);
        }
        let dst = &mut self.runs.alpha[xu..];
        for (d, &a) in dst
            .iter_mut()
            .zip(antialias)
            .take(usize::try_from(len).unwrap_or(0))
        {
            if self.safe {
                safely_add_alpha(d, a);
            } else {
                add_alpha(d, a);
            }
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L432-L443 (chrome/m156)
    // Port of: src/core/SkScan_AAAPath.cpp#L502-L515 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // x is checked
    fn blit_anti_h_alpha(&mut self, x: i32, y: i32, alpha: Alpha) {
        self.check_y(y);
        let x = x - self.left;

        if x < self.offset_x {
            self.offset_x = 0;
        }

        if self.check(x, 1) {
            if self.safe {
                // Break the run
                self.offset_x = self.runs.add(x, 0, 1, 0, 0, self.offset_x);
                safely_add_alpha(&mut self.runs.alpha[x as usize], alpha);
            } else {
                self.offset_x = self.runs.add(x, 0, 1, 0, u32::from(alpha), self.offset_x);
            }
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L445-L456 (chrome/m156)
    // Port of: src/core/SkScan_AAAPath.cpp#L517-L532 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // x is checked
    fn blit_anti_h_width(&mut self, x: i32, y: i32, width: i32, alpha: Alpha) {
        self.check_y(y);
        let x = x - self.left;

        if x < self.offset_x {
            self.offset_x = 0;
        }

        if self.check(x, width) {
            if self.safe {
                // Break the run
                self.offset_x = self.runs.add(x, 0, width, 0, 0, self.offset_x);
                let mut i = x;
                while i < x + width {
                    safely_add_alpha(&mut self.runs.alpha[i as usize], alpha);
                    i += i32::from(self.runs.runs[i as usize]);
                }
            } else {
                self.offset_x = self
                    .runs
                    .add(x, 0, width, 0, u32::from(alpha), self.offset_x);
            }
        }
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L297 (chrome/m156)
    fn width(&self) -> i32 {
        self.width
    }

    // Port of: src/core/SkScan_AAAPath.cpp#L299-L303 (chrome/m156)
    fn flush_if_y_changed(&mut self, y: Fixed, next_y: Fixed) {
        if fixed_floor_to_int(y) != fixed_floor_to_int(next_y) {
            self.flush();
        }
    }
}

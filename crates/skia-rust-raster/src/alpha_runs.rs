// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkAlphaRuns.h, src/core/SkAlphaRuns.cpp

//! Sparse run-length-encoded alpha (supersampling coverage) values (`SkAlphaRuns`).
//!
//! Sparseness allows several paths to be composed independently into the same [`AlphaRuns`]
//! buffer. The encoding is the one documented on `Blitter::blit_anti_h`: `runs[i]` is the number
//! of pixels (`np`) starting at pixel `i` that share the alpha `alpha[i]`, and the next valid
//! entry is at `runs[i + np]`; the array is terminated by a zero entry.
//!
//! Skia's `SkAlphaRuns` points into caller-provided buffers; this port owns two `Vec`s of
//! `width + 1` entries instead (`runs` and `alpha` stay public, like `fRuns` and `fAlpha`, so they
//! can be handed to `Blitter::blit_anti_h`). Pointer arithmetic became indices.

// The functions here panic where Skia asserts (or would corrupt memory): out-of-range widths and
// offsets, or run arrays that are not valid encodings.
#![allow(clippy::missing_panics_doc)]

/// An 8-bit alpha value (`SkAlpha`).
pub use skia_rust_core::color::Alpha;

// `SkToU8`: checked in debug builds only, like the C++ `SkASSERT`.
#[allow(clippy::cast_possible_truncation)] // mirrors SkToU8, which is only asserted
fn to_u8(v: u32) -> u8 {
    debug_assert!(v <= 0xFF);
    v as u8
}

// `SkToS16`: checked in debug builds only, like the C++ `SkASSERT`.
#[allow(clippy::cast_possible_truncation)] // mirrors SkToS16, which is only asserted
fn to_s16(v: i32) -> i16 {
    debug_assert!(i16::try_from(v).is_ok());
    v as i16
}

// Run lengths are non-negative `int16_t` values used as indices.
#[allow(clippy::cast_sign_loss)] // asserted non-negative by the callers
fn idx(n: i16) -> usize {
    debug_assert!(n >= 0);
    n as usize
}

/// Sparse array of run-length-encoded alpha (supersampling coverage) values.
// Port of: src/core/SkAlphaRuns.h#L24-L173 (chrome/m156)
#[doc(alias = "SkAlphaRuns")]
#[derive(Clone, Debug)]
pub struct AlphaRuns {
    /// `fRuns`: `width + 1` entries once [`Self::reset`] has been called.
    pub runs: Vec<i16>,
    /// `fAlpha`: parallel to `runs`.
    pub alpha: Vec<Alpha>,
    // `fWidth` (SK_DEBUG-only in Skia, always kept here).
    width: i32,
}

impl AlphaRuns {
    /// Creates runs able to hold scanlines up to `max_width` pixels wide, already
    /// [`reset`](Self::reset) to `max_width`.
    #[must_use]
    pub fn new(max_width: i32) -> Self {
        assert!(max_width > 0);
        let n = usize::try_from(max_width).expect("positive") + 1;
        let mut r = AlphaRuns {
            runs: vec![0; n],
            alpha: vec![0; n],
            width: 0,
        };
        r.reset(max_width);
        r
    }

    /// Return 0-255 given 0-256.
    // Port of: src/core/SkAlphaRuns.h#L31-L34 (chrome/m156)
    #[doc(alias = "CatchOverflow")]
    #[must_use]
    pub fn catch_overflow(alpha: i32) -> Alpha {
        debug_assert!((0..=256).contains(&alpha));
        #[allow(clippy::cast_sign_loss)] // asserted in [0, 256]
        to_u8((alpha - (alpha >> 8)) as u32)
    }

    /// Returns true if the scanline contains only a single run, of alpha value 0.
    // Port of: src/core/SkAlphaRuns.h#L38-L41 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        debug_assert!(self.runs[0] > 0);
        self.alpha[0] == 0 && self.runs[idx(self.runs[0])] == 0
    }

    /// Reinitialize for a new scanline.
    // Port of: src/core/SkAlphaRuns.cpp#L12-L25 (chrome/m156)
    pub fn reset(&mut self, width: i32) {
        assert!(width > 0);
        let w = usize::try_from(width).expect("positive");
        assert!(
            w < self.runs.len() && w < self.alpha.len(),
            "width exceeds the capacity"
        );

        if cfg!(debug_assertions) {
            // SkOpts::memset16(fRuns, -42, width) under SK_DEBUG
            self.runs[..w].fill(-42);
        }
        self.runs[0] = to_s16(width);
        self.runs[w] = 0;
        self.alpha[0] = 0;

        self.width = width;
        self.validate();
    }

    /// Insert into the buffer a run starting at `(x - offset_x)`:
    ///
    /// * if `start_alpha > 0`, one pixel with value `+= start_alpha`, max 255;
    /// * if `middle_count > 0`, `middle_count` pixels with value `+= max_value`;
    /// * if `stop_alpha > 0`, one pixel with value `+= stop_alpha`.
    ///
    /// Returns the `offset_x` value that should be passed on the next call, assuming we're on the
    /// same scanline. If the caller is switching scanlines, then `offset_x` should be 0 when this
    /// is called.
    // Port of: src/core/SkAlphaRuns.h#L58-L113 (chrome/m156)
    pub fn add(
        &mut self,
        x: i32,
        start_alpha: u32,
        middle_count: i32,
        stop_alpha: u32,
        max_value: u32,
        offset_x: i32,
    ) -> i32 {
        let mut middle_count = middle_count;
        debug_assert!(middle_count >= 0);
        debug_assert!(
            x >= 0
                && x + i32::from(start_alpha != 0) + middle_count + i32::from(stop_alpha != 0)
                    <= self.width
        );
        debug_assert!(self.runs[usize::try_from(offset_x).expect("offset_x >= 0")] >= 0);

        // `runs` and `alpha` always advance in lockstep, so one index (relative to the start of
        // the buffers) stands for both pointers.
        let mut off = usize::try_from(offset_x).expect("offset_x >= 0");
        let mut last_alpha = off;
        let mut x = x - offset_x;

        if start_alpha != 0 {
            AlphaRuns::break_runs(&mut self.runs[off..], &mut self.alpha[off..], x, 1);
            // I should be able to just add alpha[x] + startAlpha. However, if the trailing edge of
            // the previous span and the leading edge of the current span round to the same
            // super-sampled x value, I might overflow to 256 with this add, hence the funny
            // subtract (crud).
            let xi = usize::try_from(x).expect("x >= 0");
            let tmp = u32::from(self.alpha[off + xi]) + start_alpha;
            debug_assert!(tmp <= 256);
            // was (tmp >> 7), but that seems wrong if we're trying to catch 256
            self.alpha[off + xi] = to_u8(tmp - (tmp >> 8));

            off += xi + 1;
            x = 0;
            self.validate();
        }

        if middle_count != 0 {
            AlphaRuns::break_runs(
                &mut self.runs[off..],
                &mut self.alpha[off..],
                x,
                middle_count,
            );
            off += usize::try_from(x).expect("x >= 0");
            x = 0;
            loop {
                let sum = i32::from(self.alpha[off]) + i32::try_from(max_value).expect("<= 255");
                self.alpha[off] = AlphaRuns::catch_overflow(sum);
                let n = i32::from(self.runs[off]);
                debug_assert!(n <= middle_count);
                off += usize::try_from(n).expect("run lengths are positive");
                middle_count -= n;
                if middle_count <= 0 {
                    break;
                }
            }
            self.validate();
            last_alpha = off;
        }

        if stop_alpha != 0 {
            AlphaRuns::break_runs(&mut self.runs[off..], &mut self.alpha[off..], x, 1);
            off += usize::try_from(x).expect("x >= 0");
            self.alpha[off] = to_u8(u32::from(self.alpha[off]) + stop_alpha);
            self.validate();
            last_alpha = off;
        }

        i32::try_from(last_alpha).expect("fits in i32") // new offsetX
    }

    /// Asserts (in debug builds) that every alpha is at most `(y + 1) * max_step - (y ==
    /// max_step - 1)`.
    // Port of: src/core/SkAlphaRuns.cpp#L28-L38 (chrome/m156)
    #[doc(alias = "assertValid")]
    pub fn assert_valid(&self, y: i32, max_step: i32) {
        let max = (y + 1) * max_step - i32::from(y == max_step - 1);

        let mut i = 0usize;
        while self.runs[i] != 0 {
            debug_assert!(i32::from(self.alpha[i]) <= max);
            i += idx(self.runs[i]);
        }
    }

    /// Formats the runs like Skia's `SkAlphaRuns::dump` (which prints them): `Runs`, then for each
    /// run ` %02x` of the alpha and `,%d` of the length when it exceeds 1.
    // Port of: src/core/SkAlphaRuns.cpp#L40-L54 (chrome/m156)
    #[must_use]
    pub fn dump(&self) -> String {
        use std::fmt::Write;

        let mut s = String::from("Runs");
        let mut i = 0usize;
        while self.runs[i] != 0 {
            let n = self.runs[i];
            let _ = write!(s, " {:02x}", self.alpha[i]);
            if n > 1 {
                let _ = write!(s, ",{n}");
            }
            i += idx(n);
        }
        s.push('\n');
        s
    }

    // Port of: src/core/SkAlphaRuns.cpp#L56-L69 (chrome/m156)
    fn validate(&self) {
        if !cfg!(debug_assertions) {
            return;
        }
        debug_assert!(self.width > 0);

        let mut count = 0;
        let mut i = 0usize;
        while self.runs[i] != 0 {
            debug_assert!(self.runs[i] > 0);
            count += i32::from(self.runs[i]);
            debug_assert!(count <= self.width);
            i += idx(self.runs[i]);
        }
        debug_assert_eq!(count, self.width);
    }

    /// Break the runs in the buffer at offsets `x` and `x + count`, properly updating the runs to
    /// the right and left. I.e. from the state `AAAABBBB`, run-length encoded as `A4B4`,
    /// `break_runs(..., 2, 5)` would produce `AAAABBBB` rle as `A2A2B3B1`. Allows
    /// [`Self::add`] to sum another run to some of the new sub-runs; i.e. adding `..CCCCC.` would
    /// produce `AADDEEEB`, rle as `A2D2E3B1`.
    // Port of: src/core/SkAlphaRuns.h#L124-L163 (chrome/m156)
    #[doc(alias = "Break")]
    pub fn break_runs(runs: &mut [i16], alpha: &mut [Alpha], x: i32, count: i32) {
        debug_assert!(count > 0 && x >= 0);

        // `runs` / `alpha` advance in lockstep: one index for both pointers.
        let next = usize::try_from(x).expect("x >= 0");
        let mut x = x;
        let mut i = 0usize;

        while x > 0 {
            let n = i32::from(runs[i]);
            debug_assert!(n > 0);

            if x < n {
                let xu = usize::try_from(x).expect("positive");
                alpha[i + xu] = alpha[i];
                runs[i] = to_s16(x);
                runs[i + xu] = to_s16(n - x);
                break;
            }
            let nu = usize::try_from(n).expect("positive");
            i += nu;
            x -= n;
        }

        i = next;
        x = count;

        loop {
            let n = i32::from(runs[i]);
            debug_assert!(n > 0);

            if x < n {
                let xu = usize::try_from(x).expect("positive");
                alpha[i + xu] = alpha[i];
                runs[i] = to_s16(x);
                runs[i + xu] = to_s16(n - x);
                break;
            }
            x -= n;
            if x <= 0 {
                break;
            }
            i += usize::try_from(n).expect("positive");
        }
    }

    /// Cut (at offset `x` in the buffer) a run into two shorter runs with matching alpha values.
    /// Used by the rect clip blitter to trim a RLE encoding to match the clipping rectangle.
    // Port of: src/core/SkAlphaRuns.h#L171-L186 (chrome/m156)
    #[doc(alias = "BreakAt")]
    pub fn break_at(runs: &mut [i16], alpha: &mut [Alpha], x: i32) {
        let mut x = x;
        let mut i = 0usize;
        while x > 0 {
            let n = i32::from(runs[i]);
            debug_assert!(n > 0);

            if x < n {
                let xu = usize::try_from(x).expect("positive");
                alpha[i + xu] = alpha[i];
                runs[i] = to_s16(x);
                runs[i + xu] = to_s16(n - x);
                break;
            }
            i += usize::try_from(n).expect("positive");
            x -= n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expands (runs, alpha) into per-pixel alpha.
    fn expand(ar: &AlphaRuns) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while ar.runs[i] != 0 {
            for _ in 0..ar.runs[i] {
                out.push(ar.alpha[i]);
            }
            i += usize::try_from(ar.runs[i]).unwrap();
        }
        out
    }

    #[test]
    fn reset_and_empty() {
        let mut ar = AlphaRuns::new(8);
        assert!(ar.is_empty());
        assert_eq!(ar.runs[0], 8);
        assert_eq!(ar.runs[8], 0);
        assert_eq!(ar.dump(), "Runs 00,8\n");
        ar.add(2, 0, 3, 0, 10, 0);
        assert!(!ar.is_empty());
        ar.reset(8);
        assert!(ar.is_empty());
    }

    // The doc example of `Break`: A4B4 broken at (2, 5) is A2A2B3B1.
    #[test]
    fn break_example() {
        let mut runs = [4i16, 0, 0, 0, 4, 0, 0, 0, 0];
        let mut alpha = [0xAAu8, 0, 0, 0, 0xBB, 0, 0, 0, 0];
        AlphaRuns::break_runs(&mut runs, &mut alpha, 2, 5);
        assert_eq!(&runs[..8], &[2, 0, 2, 0, 3, 0, 0, 1]);
        assert_eq!(alpha[0], 0xAA);
        assert_eq!(alpha[2], 0xAA);
        assert_eq!(alpha[4], 0xBB);
        assert_eq!(alpha[7], 0xBB);
    }

    #[test]
    fn break_at_splits_one_run() {
        let mut runs = [6i16, 0, 0, 0, 0, 0, 0];
        let mut alpha = [0x40u8, 0, 0, 0, 0, 0, 0];
        AlphaRuns::break_at(&mut runs, &mut alpha, 2);
        assert_eq!(&runs[..6], &[2, 0, 4, 0, 0, 0]);
        assert_eq!(alpha[0], 0x40);
        assert_eq!(alpha[2], 0x40);
        // x == 0 and x beyond the first run leave the first run alone.
        AlphaRuns::break_at(&mut runs, &mut alpha, 0);
        assert_eq!(runs[0], 2);
    }

    #[test]
    fn add_start_middle_stop() {
        let mut ar = AlphaRuns::new(10);
        // 1 pixel at x=1 (+0x20), 3 pixels (+0x80), 1 pixel (+0x10).
        let off = ar.add(1, 0x20, 3, 0x10, 0x80, 0);
        assert_eq!(expand(&ar), [0, 0x20, 0x80, 0x80, 0x80, 0x10, 0, 0, 0, 0]);
        // The last touched run starts at pixel 5.
        assert_eq!(off, 5);
        // Adding again on the same scanline, continuing from `off`, accumulates.
        let off2 = ar.add(5, 0x01, 0, 0, 0, off);
        assert_eq!(expand(&ar)[5], 0x11);
        assert_eq!(off2, 5);
        ar.assert_valid(0, 0x100);
    }

    #[test]
    fn add_overflow_is_caught() {
        let mut ar = AlphaRuns::new(4);
        ar.add(0, 0, 4, 0, 0x80, 0);
        ar.add(0, 0, 4, 0, 0x80, 0);
        // 0x80 + 0x80 = 256 -> 255 by CatchOverflow.
        assert_eq!(expand(&ar), [255; 4]);
        assert_eq!(AlphaRuns::catch_overflow(256), 255);
        assert_eq!(AlphaRuns::catch_overflow(255), 255);
        assert_eq!(AlphaRuns::catch_overflow(0), 0);

        let mut ar = AlphaRuns::new(3);
        ar.add(1, 0x80, 0, 0, 0, 0);
        ar.add(1, 0x80, 0, 0, 0, 0);
        // start alpha: 0x100 - (0x100 >> 8) = 0xFF
        assert_eq!(expand(&ar), [0, 255, 0]);
    }

    #[test]
    fn add_composes_overlapping_runs() {
        let mut ar = AlphaRuns::new(6);
        ar.add(0, 0, 4, 0, 0x10, 0);
        ar.add(2, 0, 4, 0, 0x20, 0);
        assert_eq!(expand(&ar), [0x10, 0x10, 0x30, 0x30, 0x20, 0x20]);
        assert_eq!(ar.dump(), "Runs 10,2 30,2 20,2\n");
    }
}

// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkMemset_opts.h, src/core/SkMemset.h

//! `SkOpts::memset16/32/64` and `SkOpts::rect_memset16/32/64` (design §1.8, task B7).
//!
//! The result of a memset does not depend on how it is executed, so every tier is exact; what
//! differs is the store width. Skia's `memsetT` stores `skvx::Vec<VecSize, T>` blocks, with
//! `VecSize = 32 / sizeof(T)` when `SK_CPU_X64_LEVEL >= AVX` and `16 / sizeof(T)` otherwise,
//! then a scalar tail. Here the block is a `[T; VecSize]` array store (no `unsafe`: the
//! compiler turns the block fill into vector stores), with `VecSize` chosen by tier: 32 bytes
//! for `Ml3`/`Ml4` (AVX), 16 bytes for the rest. Skia's `rep stos` (ERMS) path for large fills
//! and its AVX file produce the same bytes and are not separate code here.
//!
//! The scalar twins ([`memset16_scalar`], …) are plain loops. Counts and row lengths are
//! `usize` instead of Skia's `int`.

use crate::tier::{Selection, Tier, selection};

/// Bytes per store block: `32` on AVX tiers, `16` otherwise (`memsetT`'s `VecSize * sizeof(T)`).
// Port of: src/opts/SkMemset_opts.h#L16-L21 (chrome/m156)
const fn block_bytes(tier: Tier) -> usize {
    match tier {
        Tier::Ml3 | Tier::Ml4 => 32,
        _ => 16,
    }
}

/// `memsetT<T>` with `N == VecSize` elements per block.
// Port of: src/opts/SkMemset_opts.h#L14-L36 (chrome/m156)
fn memset_t<T: Copy, const N: usize>(buffer: &mut [T], value: T, count: usize) {
    let (blocks, tail) = buffer[..count].as_chunks_mut::<N>();
    // Copy the value into the destination buffer (VecSize elements at a time).
    for block in blocks {
        *block = [value; N];
    }
    // If count was not an even multiple of VecSize, take care of the last few.
    tail.fill(value);
}

/// `rect_memsetT<T>`.
// Port of: src/opts/SkMemset_opts.h#L50-L56 (chrome/m156)
fn rect_memset_t<T: Copy, const N: usize>(
    buffer: &mut [T],
    value: T,
    count: usize,
    row_bytes: usize,
    height: usize,
) {
    assert!(
        row_bytes.is_multiple_of(size_of::<T>()),
        "row_bytes must be a multiple of the element size"
    );
    let stride = row_bytes / size_of::<T>();
    for y in 0..height {
        memset_t::<T, N>(&mut buffer[y * stride..], value, count);
    }
}

macro_rules! memset_fns {
    ($t:ty, $memset:ident, $memset_with:ident, $scalar:ident,
     $rect:ident, $rect_with:ident, $rect_scalar:ident, $alias:literal, $rect_alias:literal) => {
        #[doc = concat!("`SkOpts::", $alias, "(buffer, value, count)`: sets `buffer[..count]` to `value`,")]
        #[doc = "on the current [`selection`]."]
        ///
        /// # Panics
        /// If `count > buffer.len()`.
        // Port of: src/opts/SkMemset_opts.h#L38-L48 (chrome/m156)
        pub fn $memset(buffer: &mut [$t], value: $t, count: usize) {
            $memset_with(selection(), buffer, value, count);
        }

        #[doc = concat!("[`", stringify!($memset), "`] on an explicit [`Selection`] (only its tier matters).")]
        ///
        /// # Panics
        /// If `count > buffer.len()`.
        pub fn $memset_with(sel: Selection, buffer: &mut [$t], value: $t, count: usize) {
            if block_bytes(sel.tier) == 32 {
                memset_t::<$t, { 32 / size_of::<$t>() }>(buffer, value, count);
            } else {
                memset_t::<$t, { 16 / size_of::<$t>() }>(buffer, value, count);
            }
        }

        #[doc = concat!("The scalar twin of [`", stringify!($memset), "`]: a plain loop.")]
        ///
        /// # Panics
        /// If `count > buffer.len()`.
        pub fn $scalar(buffer: &mut [$t], value: $t, count: usize) {
            for x in &mut buffer[..count] {
                *x = value;
            }
        }

        #[doc = concat!("`SkOpts::", $rect_alias, "(buffer, value, count, rowBytes, height)`: sets `count` elements of each of")]
        #[doc = "`height` rows, rows `row_bytes` bytes apart, on the current [`selection`]."]
        ///
        /// # Panics
        /// If `row_bytes` is not a multiple of the element size, or `buffer` is too short.
        // Port of: src/opts/SkMemset_opts.h#L58-L74 (chrome/m156)
        pub fn $rect(buffer: &mut [$t], value: $t, count: usize, row_bytes: usize, height: usize) {
            $rect_with(selection(), buffer, value, count, row_bytes, height);
        }

        #[doc = concat!("[`", stringify!($rect), "`] on an explicit [`Selection`] (only its tier matters).")]
        ///
        /// # Panics
        /// As the unsuffixed function.
        pub fn $rect_with(
            sel: Selection,
            buffer: &mut [$t],
            value: $t,
            count: usize,
            row_bytes: usize,
            height: usize,
        ) {
            if block_bytes(sel.tier) == 32 {
                rect_memset_t::<$t, { 32 / size_of::<$t>() }>(
                    buffer, value, count, row_bytes, height,
                );
            } else {
                rect_memset_t::<$t, { 16 / size_of::<$t>() }>(
                    buffer, value, count, row_bytes, height,
                );
            }
        }

        #[doc = concat!("The scalar twin of [`", stringify!($rect), "`]: plain loops.")]
        ///
        /// # Panics
        /// As the unsuffixed function.
        pub fn $rect_scalar(
            buffer: &mut [$t],
            value: $t,
            count: usize,
            row_bytes: usize,
            height: usize,
        ) {
            assert!(
                row_bytes.is_multiple_of(size_of::<$t>()),
                "row_bytes must be a multiple of the element size"
            );
            let stride = row_bytes / size_of::<$t>();
            for y in 0..height {
                for x in &mut buffer[y * stride..][..count] {
                    *x = value;
                }
            }
        }
    };
}

memset_fns!(
    u16,
    memset16,
    memset16_with,
    memset16_scalar,
    rect_memset16,
    rect_memset16_with,
    rect_memset16_scalar,
    "memset16",
    "rect_memset16"
);
memset_fns!(
    u32,
    memset32,
    memset32_with,
    memset32_scalar,
    rect_memset32,
    rect_memset32_with,
    rect_memset32_scalar,
    "memset32",
    "rect_memset32"
);
memset_fns!(
    u64,
    memset64,
    memset64_with,
    memset64_scalar,
    rect_memset64,
    rect_memset64_with,
    rect_memset64_scalar,
    "memset64",
    "rect_memset64"
);

#[cfg(test)]
mod tests;

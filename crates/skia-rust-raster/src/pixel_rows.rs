// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Safe access to 32-bit pixels of a byte-backed [`Pixmap`] for the legacy blitters.
//!
//! Skia's blitters get a `uint32_t*` into the pixel memory and pass it to the `SkOpts` kernels,
//! which take `&mut [u32]` here (`skia-rust-simd`). A [`Pixmap`] holds bytes, which a safe
//! program cannot reinterpret as `u32`s, so each call loads the affected pixels into a scratch
//! `Vec<u32>`, runs the kernel on it and stores the result back. The pixels are the same, and
//! so is the arithmetic.

use skia_rust_core::pixmap::Pixmap;

/// The byte offset of the 32-bit pixel `(x, y)`.
pub(crate) fn offset32(pm: &Pixmap<'_>, x: i32, y: i32) -> usize {
    debug_assert!(x >= 0 && y >= 0, "({x}, {y}) is outside the pixmap");
    let x = usize::try_from(x).expect("pixel x is not negative");
    let y = usize::try_from(y).expect("pixel y is not negative");
    y * pm.row_bytes() + x * 4
}

/// The 32-bit pixel at byte offset `off` of `bytes`.
pub(crate) fn read32(bytes: &[u8], off: usize) -> u32 {
    u32::from_ne_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

/// Stores the 32-bit pixel `value` at byte offset `off` of `bytes`.
pub(crate) fn write32(bytes: &mut [u8], off: usize, value: u32) {
    bytes[off..off + 4].copy_from_slice(&value.to_ne_bytes());
}

/// The writable bytes of `pm` (a blitter's device always has writable pixels).
pub(crate) fn bytes_mut<'p>(pm: &'p mut Pixmap<'_>) -> &'p mut [u8] {
    pm.bytes_mut()
        .expect("a blitter's device has writable pixels")
}

/// Loads `count` 32-bit pixels starting at byte `off` of `bytes` into `out`.
pub(crate) fn load_u32s(bytes: &[u8], off: usize, count: usize, out: &mut Vec<u32>) {
    out.clear();
    out.extend(
        bytes[off..off + count * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_ne_bytes(*c)),
    );
}

/// Runs `f` on the `count` pixels of the row starting at `(x, y)` as a `u32` slice, then stores
/// them back.
pub(crate) fn with_span32<R>(
    pm: &mut Pixmap<'_>,
    scratch: &mut Vec<u32>,
    x: i32,
    y: i32,
    count: usize,
    f: impl FnOnce(&mut [u32]) -> R,
) -> R {
    let off = offset32(pm, x, y);
    let bytes = bytes_mut(pm);
    load_u32s(bytes, off, count, scratch);
    let r = f(scratch);
    for (c, v) in bytes[off..off + count * 4]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(scratch.iter())
    {
        *c = v.to_ne_bytes();
    }
    r
}

/// Runs `f` on a `u32` slice that starts at pixel `(x, y)` and covers `height` rows of `width`
/// pixels (`f` gets the row bytes to step between them), then stores it back. The slice also
/// covers the pixels between the rows, which are stored back unchanged.
pub(crate) fn with_rows32<R>(
    pm: &mut Pixmap<'_>,
    scratch: &mut Vec<u32>,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    f: impl FnOnce(&mut [u32], usize) -> R,
) -> R {
    let off = offset32(pm, x, y);
    let row_bytes = pm.row_bytes();
    debug_assert_eq!(0, row_bytes % 4);
    let count = if height == 0 {
        0
    } else {
        (height - 1) * (row_bytes / 4) + width
    };
    let bytes = bytes_mut(pm);
    load_u32s(bytes, off, count, scratch);
    let r = f(scratch, row_bytes);
    for (c, v) in bytes[off..off + count * 4]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(scratch.iter())
    {
        *c = v.to_ne_bytes();
    }
    r
}

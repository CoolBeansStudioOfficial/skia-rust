// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan_AntiPath.cpp

//! The antialiased path fill entry point (`SkScan::AntiFillPath`).
//!
//! In m156 there is no supersampling scan converter any more: after clipping, every antialiased
//! fill goes through the analytic one ([`aaa_fill_path_raw`]).
//!
//! skia-rust: the C++ overloads are [`anti_fill_path_region`] (`SkRegion`, with `forceRLE`) and
//! [`anti_fill_path_clip`] (`SkRasterClip`, which wraps AA clips in an `SkAAClipBlitter`).

use skia_rust_core::math::{MAX_S32, left_shift};
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::rect::{IRect, RoundOut};
use skia_rust_core::region::{Op, Region};

use crate::aa_clip::AAClipBlitter;
use crate::blitter::Blitter;
use crate::raster_clip::RasterClip;
use crate::scan::fill_path;
use crate::scan_aaa_path::aaa_fill_path_raw;
use crate::scan_priv::{SUPERSAMPLE_SHIFT, ScanClipper, blit_above, blit_below};

// Port of: src/core/SkScan_AntiPath.cpp#L23-L34 (chrome/m156)
fn safe_round_out(src: &skia_rust_core::rect::Rect) -> IRect {
    // roundOut will pin huge floats to max/min int
    let mut dst: IRect = src.round_out();

    // intersect with a smaller huge rect, so the rect will not be considered empty for being
    // too large. e.g. { -SK_MaxS32 ... SK_MaxS32 } is considered empty because its width
    // exceeds signed 32bit.
    let limit = MAX_S32 >> SUPERSAMPLE_SHIFT;
    if let Some(r) = IRect::intersect(&dst, &IRect::new(-limit, -limit, limit, limit)) {
        dst = r;
    }

    dst
}

// Port of: src/core/SkScan_AntiPath.cpp#L36-L39 (chrome/m156)
const fn overflows_short_shift(value: i32, shift: i32) -> i32 {
    let s = 16 + shift;
    (left_shift(value, s) >> s).wrapping_sub(value)
}

const _: () = assert!(overflows_short_shift(8191, SUPERSAMPLE_SHIFT) == 0);
const _: () = assert!(overflows_short_shift(8192, SUPERSAMPLE_SHIFT) != 0);
const _: () = assert!(overflows_short_shift(32767, 0) == 0);
const _: () = assert!(overflows_short_shift(32768, 0) != 0);

/// Would any of the coordinates of this rectangle not fit in a short, when left-shifted by
/// shift?
// Port of: src/core/SkScan_AntiPath.cpp#L46-L57 (chrome/m156)
fn rect_overflows_short_shift(rect: IRect, shift: i32) -> i32 {
    // Since we expect these to succeed, we bit-or together
    // for a tiny extra bit of speed.
    overflows_short_shift(rect.left, shift)
        | overflows_short_shift(rect.right, shift)
        | overflows_short_shift(rect.top, shift)
        | overflows_short_shift(rect.bottom, shift)
}

/// Fills `path` with antialiasing, clipped to `orig_clip` (`SkScan::AntiFillPath(const
/// SkPathRaw&, const SkRegion&, SkBlitter*, bool forceRLE)`). `force_rle` is set by `SkAAClip`.
///
/// When the clipped bounds are too large to antialias (beyond ±8191 pixels), this falls back to
/// the non-AA [`fill_path`].
// Port of: src/core/SkScan_AntiPath.cpp#L59-L135 (chrome/m156)
#[doc(alias = "AntiFillPath")]
pub fn anti_fill_path_region(
    path: &PathRaw<'_>,
    orig_clip: &Region,
    blitter: &mut dyn Blitter,
    force_rle: bool,
) {
    if orig_clip.is_empty() {
        return;
    }

    let is_inverse = path.is_inverse_fill_type();
    let ir = safe_round_out(&path.bounds());
    if ir.is_empty() {
        if is_inverse {
            blitter.blit_region(orig_clip);
        }
        return;
    }

    // If the intersection of the path bounds and the clip bounds
    // will overflow 32767 when << by SHIFT, we can't supersample,
    // so draw without antialiasing.
    let clipped_ir = if is_inverse {
        // If the path is an inverse fill, it's going to fill the entire
        // clip, and we care whether the entire clip exceeds our limits.
        *orig_clip.bounds()
    } else {
        match IRect::intersect(&ir, orig_clip.bounds()) {
            Some(r) => r,
            None => return,
        }
    };
    if rect_overflows_short_shift(clipped_ir, SUPERSAMPLE_SHIFT) != 0 {
        fill_path(path, orig_clip, blitter);
        return;
    }

    // Our antialiasing can't handle a clip larger than 32767, so we restrict
    // the clip to that limit here. (the runs[] uses int16_t for its index).
    //
    // A more general solution (one that could also eliminate the need to
    // disable aa based on ir bounds (see overflows_short_shift) would be
    // to tile the clip/target...
    let mut tmp_clip_storage = Region::new();
    let clip_rgn: &Region = {
        const MAX_CLIP_COORD: i32 = 32767;
        let bounds = orig_clip.bounds();
        if bounds.right > MAX_CLIP_COORD || bounds.bottom > MAX_CLIP_COORD {
            let limit = IRect::new(0, 0, MAX_CLIP_COORD, MAX_CLIP_COORD);
            tmp_clip_storage.op_region_rect(orig_clip, limit, Op::Intersect);
            &tmp_clip_storage
        } else {
            orig_clip
        }
    };
    // for here down, use clipRgn, not origClip

    let mut clipper = ScanClipper::new(blitter, clip_rgn, &ir, false, false);

    debug_assert!(clipper.clip_rect().is_none_or(|r| *r == *clip_rgn.bounds()));

    let Some(blitter) = clipper.blitter() else {
        // clipped out
        if is_inverse && let Some(blitter) = clipper.clipped_out_blitter() {
            blitter.blit_region(clip_rgn);
        }
        return;
    };

    // now use the (possibly wrapped) blitter

    if is_inverse {
        blit_above(blitter, &ir, clip_rgn);
    }

    aaa_fill_path_raw(path, blitter, &ir, clip_rgn.bounds(), force_rle);

    if is_inverse {
        blit_below(blitter, &ir, clip_rgn);
    }
}

///////////////////////////////////////////////////////////////////////////////

/// Fills `raw` with antialiasing, clipped to a raster clip (`SkScan::AntiFillPath(const
/// SkPathRaw&, const SkRasterClip&, SkBlitter*)`).
// Port of: src/core/SkScan_AntiPath.cpp#L139-L154 (chrome/m156)
#[doc(alias = "AntiFillPath")]
pub fn anti_fill_path_clip(raw: &PathRaw<'_>, clip: &RasterClip, blitter: &mut dyn Blitter) {
    debug_assert!(raw.bounds().is_finite());
    if clip.is_empty() {
        return;
    }

    if clip.is_bw() {
        anti_fill_path_region(raw, clip.bw_rgn(), blitter, false);
    } else {
        let mut tmp = Region::new();

        tmp.set_rect(*clip.bounds());
        let mut aa_blitter = AAClipBlitter::new(blitter, clip.aa_rgn());
        // SkAAClipBlitter can blitMask, why forceRLE?
        anti_fill_path_region(raw, &tmp, &mut aa_blitter, true);
    }
}

// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan.h, src/core/SkRasterClip.h (the parts the scan converters use)

//! The clip abstraction the scan converters draw through, and the fixed-point rect helpers of
//! `SkScan.h`.
//!
//! `SkRasterClip` (a BW [`Region`] or an `SkAAClip`) is ported by the clip task (C5). Until then
//! the scan converters take a [`ScanClip`], which exposes exactly the `SkRasterClip` queries they
//! use plus the `SkAAClipBlitterWrapper` hook; [`Region`] implements it as a BW clip. C5 makes
//! `RasterClip` implement it too.

use skia_rust_core::fixed::{
    fixed_ceil_to_int, fixed_floor_to_int, fixed_round_to_int, int_to_fixed, scalar_to_fixed,
};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::Region;

use crate::blitter::Blitter;

/// The `SkRasterClip` interface used by the scan converters.
// Port of: src/core/SkRasterClip.h#L22-L130 (chrome/m156), restricted to what SkScan uses
pub trait ScanClip {
    /// `isBW()`: true if the clip is a plain [`Region`].
    fn is_bw(&self) -> bool;

    /// `isRect()`.
    fn is_rect(&self) -> bool;

    /// `getBounds()`.
    fn bounds(&self) -> IRect;

    /// `quickContains(rect)`.
    fn quick_contains(&self, rect: &IRect) -> bool;

    /// `quickReject(rect)`: `!IRect::Intersects(getBounds(), rect)`.
    fn quick_reject(&self, rect: &IRect) -> bool {
        !IRect::intersects(&self.bounds(), rect)
    }

    /// `bwRgn()`. Only called when [`Self::is_bw`] is true.
    fn bw_rgn(&self) -> &Region;

    /// `SkAAClipBlitterWrapper(clip, blitter)`: calls `f` with the wrapper's region and blitter.
    /// For a BW clip the region is the clip and the blitter is `blitter` itself.
    fn with_aa_wrapper(
        &self,
        blitter: &mut dyn Blitter,
        f: &mut dyn FnMut(&Region, &mut dyn Blitter),
    );
}

/// A plain region is a BW raster clip.
impl ScanClip for Region {
    fn is_bw(&self) -> bool {
        true
    }

    fn is_rect(&self) -> bool {
        Region::is_rect(self)
    }

    fn bounds(&self) -> IRect {
        *Region::bounds(self)
    }

    fn quick_contains(&self, rect: &IRect) -> bool {
        Region::quick_contains(self, rect)
    }

    fn bw_rgn(&self) -> &Region {
        self
    }

    fn with_aa_wrapper(
        &self,
        blitter: &mut dyn Blitter,
        f: &mut dyn FnMut(&Region, &mut dyn Blitter),
    ) {
        f(self, blitter);
    }
}

/// A fixed-point (16.16) rectangle, identical to [`IRect`] but with `Fixed` coordinates
/// (`SkXRect`).
// Port of: src/core/SkScan.h#L25 (chrome/m156)
#[doc(alias = "SkXRect")]
pub type XRect = IRect;

/// `XRect_set(SkXRect*, const SkIRect&)`: promotes int coordinates to fixed.
// Port of: src/core/SkScan.h#L98-L103 (chrome/m156)
#[must_use]
pub fn xrect_from_irect(src: &IRect) -> XRect {
    XRect::new(
        int_to_fixed(src.left),
        int_to_fixed(src.top),
        int_to_fixed(src.right),
        int_to_fixed(src.bottom),
    )
}

/// `XRect_set(SkXRect*, const SkRect&)`: converts scalar coordinates to fixed.
// Port of: src/core/SkScan.h#L109-L114 (chrome/m156)
#[must_use]
pub fn xrect_from_rect(src: &Rect) -> XRect {
    XRect::new(
        scalar_to_fixed(src.left),
        scalar_to_fixed(src.top),
        scalar_to_fixed(src.right),
        scalar_to_fixed(src.bottom),
    )
}

/// `XRect_round`.
// Port of: src/core/SkScan.h#L118-L123 (chrome/m156)
#[must_use]
pub fn xrect_round(xr: &XRect) -> IRect {
    IRect::new(
        fixed_round_to_int(xr.left),
        fixed_round_to_int(xr.top),
        fixed_round_to_int(xr.right),
        fixed_round_to_int(xr.bottom),
    )
}

/// `XRect_roundOut`.
// Port of: src/core/SkScan.h#L128-L133 (chrome/m156)
#[must_use]
pub fn xrect_round_out(xr: &XRect) -> IRect {
    IRect::new(
        fixed_floor_to_int(xr.left),
        fixed_floor_to_int(xr.top),
        fixed_ceil_to_int(xr.right),
        fixed_ceil_to_int(xr.bottom),
    )
}

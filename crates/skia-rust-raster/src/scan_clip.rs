// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan.h, src/core/SkRasterClip.h (the parts the scan converters use)

//! The clip abstraction the scan converters draw through (the `SkXRect` helpers of `SkScan.h`
//! are in [`crate::scan`]).
//!
//! `SkRasterClip` (a BW [`Region`] or an `SkAAClip`) is ported by the clip task (C5). Until then
//! the scan converters take a [`ScanClip`], which exposes exactly the `SkRasterClip` queries they
//! use plus the `SkAAClipBlitterWrapper` hook; [`Region`] implements it as a BW clip. C5 makes
//! `RasterClip` implement it too.

use skia_rust_core::rect::IRect;
use skia_rust_core::region::Region;

use crate::blitter::Blitter;

/// The `SkRasterClip` interface used by the scan converters.
// Port of: src/core/SkRasterClip.h#L22-L130 (chrome/m156), restricted to what SkScan uses
pub trait ScanClip {
    /// `isBW()`: true if the clip is a plain [`Region`].
    fn is_bw(&self) -> bool;

    /// `isEmpty()`.
    fn is_empty(&self) -> bool;

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

    fn is_empty(&self) -> bool {
        Region::is_empty(self)
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

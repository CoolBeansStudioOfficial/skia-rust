// Copyright 2010 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterClip.h, src/core/SkRasterClip.cpp

//! [`RasterClip`]: a clip that is either a BW [`Region`] or an antialiased [`AAClip`], and
//! [`AAClipBlitterWrapper`], which hands the scan converters the region and blitter to draw with.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Region, region_priv};
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::{scalar, scalar_floor_to_scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;

use crate::aa_clip::{AAClip, AAClipBlitter};
use crate::blitter::Blitter;
use crate::region_path::RegionExt;

/// Wraps a [`Region`] and an [`AAClip`], so there is a single object that can represent either
/// BW or antialiased clips (`SkRasterClip`).
///
// Port of: src/core/SkRasterClip.h#L28-L127 (chrome/m156)
#[doc(alias = "SkRasterClip")]
#[derive(Clone, Debug)]
pub struct RasterClip {
    bw: Region,
    aa: AAClip,
    is_bw: bool,
    // these 2 are caches based on querying the right obj based on is_bw
    is_empty: bool,
    is_rect: bool,
    // if present, this augments the clip, not replaces it
    shader: Option<Shader>,
}

impl Default for RasterClip {
    fn default() -> Self {
        Self::new()
    }
}

impl RasterClip {
    /// An empty BW clip (`SkRasterClip()`).
    // Port of: src/core/SkRasterClip.cpp#L73-L78 (chrome/m156)
    #[must_use]
    pub fn new() -> RasterClip {
        let rc = RasterClip {
            bw: Region::new(),
            aa: AAClip::new(),
            is_bw: true,
            is_empty: true,
            is_rect: false,
            shader: None,
        };
        rc.validate();
        rc
    }

    /// A BW clip of `bounds` (`SkRasterClip(const SkIRect&)`).
    // Port of: src/core/SkRasterClip.cpp#L66-L71 (chrome/m156)
    #[must_use]
    pub fn from_rect(bounds: &IRect) -> RasterClip {
        let mut rc = RasterClip {
            bw: Region::from_rect(bounds),
            aa: AAClip::new(),
            is_bw: true,
            is_empty: true,
            is_rect: false,
            shader: None,
        };
        rc.is_empty = rc.compute_is_empty(); // bounds might be empty, so compute
        rc.is_rect = !rc.is_empty;
        rc.validate();
        rc
    }

    /// A BW clip of `rgn` (`SkRasterClip(const SkRegion&)`).
    // Port of: src/core/SkRasterClip.cpp#L59-L64 (chrome/m156)
    #[must_use]
    pub fn from_region(rgn: &Region) -> RasterClip {
        let mut rc = RasterClip {
            bw: rgn.clone(),
            aa: AAClip::new(),
            is_bw: true,
            is_empty: true,
            is_rect: false,
            shader: None,
        };
        rc.is_empty = rc.compute_is_empty(); // bounds might be empty, so compute
        rc.is_rect = !rc.is_empty;
        rc.validate();
        rc
    }

    /// The part of `path` inside `bounds`, antialiased if `do_aa`
    /// (`SkRasterClip(const SkPath&, const SkIRect&, bool)`).
    // Port of: src/core/SkRasterClip.cpp#L80-L92 (chrome/m156)
    #[must_use]
    pub fn from_path(path: &Path, bounds: &IRect, do_aa: bool) -> RasterClip {
        let mut rc = RasterClip {
            bw: Region::new(),
            aa: AAClip::new(),
            is_bw: true,
            is_empty: true,
            is_rect: false,
            shader: None,
        };
        if do_aa {
            rc.is_bw = false;
            rc.aa.set_path(path, bounds, true);
        } else {
            rc.is_bw = true;
            rc.bw.set_path(path, &Region::from_rect(bounds));
        }
        rc.is_empty = rc.compute_is_empty(); // bounds might be empty, so compute
        rc.is_rect = rc.compute_is_rect();
        rc.validate();
        rc
    }

    /// True if the clip is a plain region (`isBW`).
    #[doc(alias = "isBW")]
    #[must_use]
    pub fn is_bw(&self) -> bool {
        self.is_bw
    }

    /// True if the clip is antialiased (`isAA`).
    #[doc(alias = "isAA")]
    #[must_use]
    pub fn is_aa(&self) -> bool {
        !self.is_bw
    }

    /// The region of a BW clip (`bwRgn`).
    #[doc(alias = "bwRgn")]
    #[must_use]
    pub fn bw_rgn(&self) -> &Region {
        debug_assert!(self.is_bw);
        &self.bw
    }

    /// The antialiased clip of an AA clip (`aaRgn`).
    #[doc(alias = "aaRgn")]
    #[must_use]
    pub fn aa_rgn(&self) -> &AAClip {
        debug_assert!(!self.is_bw);
        &self.aa
    }

    /// True if the clip is empty (`isEmpty`).
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        debug_assert_eq!(self.compute_is_empty(), self.is_empty);
        self.is_empty
    }

    /// True if the clip is a single hard-edged rectangle (`isRect`).
    #[doc(alias = "isRect")]
    #[must_use]
    pub fn is_rect(&self) -> bool {
        // skia-rust: Skia asserts `computeIsRect() == fIsRect` here (and in `validate`), but
        // `SkRasterClip(const SkRegion&)` caches `!isEmpty()` even for a complex region, so a
        // release Skia (the oracle) returns a cached value the assertion would reject.
        self.is_rect
    }

    /// True if the clip is more than a rectangle (`isComplex`).
    #[doc(alias = "isComplex")]
    #[must_use]
    pub fn is_complex(&self) -> bool {
        if self.is_bw {
            self.bw.is_complex()
        } else {
            !self.aa.is_empty()
        }
    }

    /// The bounds of the clip (`getBounds`).
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> &IRect {
        if self.is_bw {
            self.bw.bounds()
        } else {
            self.aa.bounds()
        }
    }

    /// Makes the clip empty; always returns false (`setEmpty`).
    // Port of: src/core/SkRasterClip.cpp#L98-L107 (chrome/m156)
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) -> bool {
        self.validate();

        self.is_bw = true;
        self.bw.set_empty();
        self.aa.set_empty();
        self.is_empty = true;
        self.is_rect = false;
        false
    }

    /// Makes the clip the BW rectangle `rect`; returns true if it is not empty (`setRect`).
    // Port of: src/core/SkRasterClip.cpp#L109-L117 (chrome/m156)
    #[doc(alias = "setRect")]
    pub fn set_rect(&mut self, rect: &IRect) -> bool {
        self.validate();

        self.is_bw = true;
        self.aa.set_empty();
        self.is_rect = self.bw.set_rect(rect);
        self.is_empty = !self.is_rect;
        self.is_rect
    }

    /// Combines the clip with the pixel rect `rect`; returns true if the result is not empty
    /// (`op(const SkIRect&, SkClipOp)`).
    // Port of: src/core/SkRasterClip.cpp#L121-L130 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_irect(&mut self, rect: &IRect, op: ClipOp) -> bool {
        self.validate();

        if self.is_bw {
            self.bw.op_rect(rect, op.into());
        } else {
            self.aa.op_irect(rect, op);
        }
        self.update_cache_and_return_non_empty(true)
    }

    /// Combines the clip with `rgn`; returns true if the result is not empty
    /// (`op(const SkRegion&, SkClipOp)`).
    // Port of: src/core/SkRasterClip.cpp#L132-L144 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_region(&mut self, rgn: &Region, op: ClipOp) -> bool {
        self.validate();

        if self.is_bw {
            self.bw.op_region(rgn, op.into());
        } else {
            let mut tmp = AAClip::new();
            tmp.set_region(rgn);
            self.aa.op_aa_clip(&tmp, op);
        }
        self.update_cache_and_return_non_empty(true)
    }

    /// Combines the clip with `local_rect` mapped by `matrix`; returns true if the result is
    /// not empty (`op(const SkRect&, const SkMatrix&, SkClipOp, bool)`).
    // Port of: src/core/SkRasterClip.cpp#L158-L190 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_rect(&mut self, local_rect: &Rect, matrix: &Matrix, op: ClipOp, do_aa: bool) -> bool {
        self.validate();

        let mut do_aa = do_aa;
        let is_scale_trans = matrix.is_scale_translate();
        if !is_scale_trans {
            return self.op_path(&Path::rect(local_rect, None), matrix, op, do_aa);
        }

        let (dev_rect, _) = matrix.map_rect(local_rect);
        if self.is_bw
            && do_aa
            // check that the rect really needs aa, or is it close enought to
            // integer boundaries that we can just treat it as a BW rect?
            && nearly_integral(dev_rect.left)
            && nearly_integral(dev_rect.top)
            && nearly_integral(dev_rect.right)
            && nearly_integral(dev_rect.bottom)
        {
            do_aa = false;
        }

        if self.is_bw && !do_aa {
            self.bw.op_rect(dev_rect.round(), op.into());
        } else {
            if self.is_bw {
                self.convert_to_aa();
            }
            self.aa.op_rect(&dev_rect, op, do_aa);
        }
        self.update_cache_and_return_non_empty(true)
    }

    /// Combines the clip with `rrect` mapped by `matrix`; returns true if the result is not
    /// empty (`op(const SkRRect&, const SkMatrix&, SkClipOp, bool)`).
    // Port of: src/core/SkRasterClip.cpp#L192-L194 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_rrect(&mut self, rrect: &RRect, matrix: &Matrix, op: ClipOp, do_aa: bool) -> bool {
        self.op_path(&Path::rrect(rrect, None), matrix, op, do_aa)
    }

    /// Combines the clip with `path` mapped by `matrix`; returns true if the result is not
    /// empty (`op(const SkPath&, const SkMatrix&, SkClipOp, bool)`).
    // Port of: src/core/SkRasterClip.cpp#L196-L222 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_path(&mut self, path: &Path, matrix: &Matrix, op: ClipOp, do_aa: bool) -> bool {
        self.validate();

        let dev_path = path.make_transform(matrix);

        // Since op is either intersect or difference, the clip is always shrinking; that means we
        // can always use our current bounds as the limiting factor for region/aaclip operations.
        if self.is_rect() && op == ClipOp::Intersect {
            // However, in the relatively common case of intersecting a new path with a
            // rectangular clip, it's faster to convert the path into a region/aa-mask in place
            // than evaluate the actual intersection. See skbug.com/40043482
            if do_aa && self.is_bw {
                self.convert_to_aa();
            }
            let bounds = *self.bounds();
            if self.is_bw {
                self.bw.set_path(&dev_path, &Region::from_rect(bounds));
            } else {
                self.aa.set_path(&dev_path, &bounds, do_aa);
            }
            self.update_cache_and_return_non_empty(true)
        } else {
            let clip = RasterClip::from_path(&dev_path, self.bounds(), do_aa);
            self.op_raster_clip(&clip, op)
        }
    }

    /// Adds `shader` to the clip: it augments the clip rather than replacing it. Returns true if
    /// the clip is not empty (`op(sk_sp<SkShader>)`).
    ///
    /// When the clip already has a shader, the two are combined with
    /// `SkShaders::Blend(SkBlendMode::kSrcIn, sh, fShader)` (a [`BlendShader`]).
    // Port of: src/core/SkRasterClip.cpp#L210-L220 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_shader(&mut self, sh: Shader) -> bool {
        self.validate();

        self.shader = Some(match self.shader.take() {
            None => sh,
            Some(existing) => shaders::blend(BlendMode::SrcIn, sh, existing),
        });
        !self.is_empty()
    }

    /// Offsets the clip by `(dx, dy)` into `dst` (`translate`).
    // Port of: src/core/SkRasterClip.cpp#L259-L285 (chrome/m156)
    pub fn translate(&self, dx: i32, dy: i32, dst: &mut RasterClip) {
        self.validate();

        if self.is_empty() {
            dst.set_empty();
            return;
        }
        if 0 == (dx | dy) {
            *dst = self.clone();
            return;
        }

        dst.is_bw = self.is_bw;
        if self.is_bw {
            self.bw.translate_to(dx, dy, &mut dst.bw);
            dst.aa.set_empty();
        } else {
            self.aa.translate(dx, dy, &mut dst.aa);
            dst.bw.set_empty();
        }
        dst.update_cache_and_return_non_empty(true);
    }

    /// True if every pixel of `rect` is inside the clip and fully covered (`quickContains`).
    #[doc(alias = "quickContains")]
    #[must_use]
    pub fn quick_contains(&self, rect: &IRect) -> bool {
        if self.is_bw {
            self.bw.quick_contains(rect)
        } else {
            self.aa.quick_contains(rect)
        }
    }

    /// Return true if this region is empty, or if the specified rectangle does not intersect the
    /// region. Returning false is not a guarantee that they intersect, but returning true is a
    /// guarantee that they do not (`quickReject`).
    // Port of: src/core/SkRasterClip.h#L93-L95 (chrome/m156)
    #[doc(alias = "quickReject")]
    #[must_use]
    pub fn quick_reject(&self, rect: &IRect) -> bool {
        !IRect::intersects(self.bounds(), rect)
    }

    /// The clip shader, if any (`clipShader`).
    #[doc(alias = "clipShader")]
    #[must_use]
    pub fn clip_shader(&self) -> Option<&Shader> {
        self.shader.as_ref()
    }

    /// Checks the invariants of the clip (`validate`, debug builds only).
    // Port of: src/core/SkRasterClip.cpp#L304-L315 (chrome/m156)
    pub fn validate(&self) {
        if !cfg!(debug_assertions) {
            return;
        }
        // can't ever assert that fBW is empty, since we may have called forceGetBW
        if self.is_bw {
            debug_assert!(self.aa.is_empty());
        }

        region_priv::validate(&self.bw);
        self.aa.validate();

        debug_assert_eq!(self.compute_is_empty(), self.is_empty);
    }

    // Port of: src/core/SkRasterClip.h#L136-L142 (chrome/m156)
    fn compute_is_empty(&self) -> bool {
        if self.is_bw {
            self.bw.is_empty()
        } else {
            self.aa.is_empty()
        }
    }

    // Port of: src/core/SkRasterClip.h#L144-L146 (chrome/m156)
    fn compute_is_rect(&self) -> bool {
        if self.is_bw {
            self.bw.is_rect()
        } else {
            self.aa.is_rect()
        }
    }

    // Port of: src/core/SkRasterClip.h#L148-L160 (chrome/m156)
    fn update_cache_and_return_non_empty(&mut self, detect_aa_rect: bool) -> bool {
        self.is_empty = self.compute_is_empty();

        // detect that our computed AA is really just a (hard-edged) rect
        if detect_aa_rect && !self.is_empty && !self.is_bw && self.aa.is_rect() {
            self.bw.set_rect(*self.aa.bounds());
            self.aa.set_empty(); // don't need this anymore
            self.is_bw = true;
        }

        self.is_rect = self.compute_is_rect();
        !self.is_empty
    }

    // Port of: src/core/SkRasterClip.cpp#L287-L296 (chrome/m156)
    fn convert_to_aa(&mut self) {
        self.validate();

        debug_assert!(self.is_bw);
        self.aa.set_region(&self.bw);
        self.is_bw = false;

        // since we are being explicitly asked to convert-to-aa, we pass false so we don't
        // "optimize" ourselves back to BW.
        self.update_cache_and_return_non_empty(false);
    }

    // Port of: src/core/SkRasterClip.cpp#L235-L257 (chrome/m156)
    fn op_raster_clip(&mut self, clip: &RasterClip, op: ClipOp) -> bool {
        self.validate();
        clip.validate();

        if self.is_bw() && clip.is_bw() {
            self.bw.op_region(&clip.bw, op.into());
        } else {
            let mut tmp = AAClip::new();
            if self.is_bw() {
                self.convert_to_aa();
            }
            let other: &AAClip = if clip.is_bw() {
                tmp.set_region(clip.bw_rgn());
                &tmp
            } else {
                clip.aa_rgn()
            };
            self.aa.op_aa_clip(other, op);
        }
        self.update_cache_and_return_non_empty(true)
    }
}

/// Our antialiasing currently has a granularity of 1/4 of a pixel along each axis. Thus we can
/// treat an axis coordinate as an integer if it differs from its nearest int by < half of that
/// value (1/8 in this case).
// Port of: src/core/SkRasterClip.cpp#L146-L156 (chrome/m156)
fn nearly_integral(x: scalar) -> bool {
    const DOMAIN: scalar = 1.0 / 4.0;
    const HALF_DOMAIN: scalar = DOMAIN / 2.0;

    let x = x + HALF_DOMAIN;
    x - scalar_floor_to_scalar(x) < DOMAIN
}

///////////////////////////////////////////////////////////////////////////////

enum Wrapped<'a> {
    /// A BW clip: the clip's region and the original blitter.
    Bw {
        rgn: &'a Region,
        blitter: &'a mut dyn Blitter,
    },
    /// An AA clip: the bounds as a region, and a blitter that applies the coverage.
    Aa {
        bw_rgn: Region,
        blitter: AAClipBlitter<'a>,
    },
}

/// Encapsulates the logic of deciding if we need to change/wrap the blitter for aaclipping. If
/// so, [`Self::rgn`] and [`Self::blitter`] return modified values. If not, they return the raw
/// blitter and (bw) clip region (`SkAAClipBlitterWrapper`).
///
/// skia-rust: use [`Self::parts`] to hold the region and the blitter at the same time. Skia's
/// default constructor plus `init` is [`Self::new`].
// Port of: src/core/SkRasterClip.h#L151-L182 (chrome/m156)
#[doc(alias = "SkAAClipBlitterWrapper")]
pub struct AAClipBlitterWrapper<'a> {
    wrapped: Wrapped<'a>,
}

impl std::fmt::Debug for AAClipBlitterWrapper<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AAClipBlitterWrapper")
            .field("rgn", self.rgn())
            .finish_non_exhaustive()
    }
}

impl<'a> AAClipBlitterWrapper<'a> {
    /// `SkAAClipBlitterWrapper(const SkRasterClip&, SkBlitter*)` / `init`.
    // Port of: src/core/SkRasterClip.cpp#L331-L346 (chrome/m156)
    #[must_use]
    pub fn new(clip: &'a RasterClip, blitter: &'a mut dyn Blitter) -> Self {
        if clip.is_bw() {
            AAClipBlitterWrapper {
                wrapped: Wrapped::Bw {
                    rgn: clip.bw_rgn(),
                    blitter,
                },
            }
        } else {
            Self::from_aa_clip(clip.aa_rgn(), blitter)
        }
    }

    /// `SkAAClipBlitterWrapper(const SkAAClip*, SkBlitter*)`.
    // Port of: src/core/SkRasterClip.cpp#L321-L330 (chrome/m156)
    #[must_use]
    pub fn from_aa_clip(aaclip: &'a AAClip, blitter: &'a mut dyn Blitter) -> Self {
        let mut bw_rgn = Region::new();
        bw_rgn.set_rect(*aaclip.bounds());
        AAClipBlitterWrapper {
            wrapped: Wrapped::Aa {
                bw_rgn,
                blitter: AAClipBlitter::new(blitter, aaclip),
            },
        }
    }

    /// The bounds of the region to draw in (`getBounds`).
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> &IRect {
        self.rgn().bounds()
    }

    /// The region to draw in (`getRgn`).
    #[doc(alias = "getRgn")]
    #[must_use]
    pub fn rgn(&self) -> &Region {
        match &self.wrapped {
            Wrapped::Bw { rgn, .. } => rgn,
            Wrapped::Aa { bw_rgn, .. } => bw_rgn,
        }
    }

    /// The blitter to draw with (`getBlitter`).
    #[doc(alias = "getBlitter")]
    pub fn blitter(&mut self) -> &mut dyn Blitter {
        self.parts().1
    }

    /// The region and the blitter, together (`getRgn` and `getBlitter`).
    pub fn parts(&mut self) -> (&Region, &mut dyn Blitter) {
        match &mut self.wrapped {
            Wrapped::Bw { rgn, blitter } => (*rgn, &mut **blitter),
            Wrapped::Aa { bw_rgn, blitter } => (&*bw_rgn, blitter),
        }
    }
}

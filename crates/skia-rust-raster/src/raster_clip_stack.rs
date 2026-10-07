// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterClipStack.h

//! [`RasterClipStack`]: the save/restore stack of [`RasterClip`]s behind a bitmap device.

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::Region;
use skia_rust_core::rrect::RRect;
use skia_rust_core::shader::Shader;

use crate::raster_clip::RasterClip;
use crate::scan::path_requires_tiling;

#[derive(Clone, Debug)]
struct Rec {
    rc: RasterClip,
    // 0 for a "normal" entry
    deferred_count: i32,
}

impl Rec {
    fn new(rc: RasterClip) -> Rec {
        Rec {
            rc,
            deferred_count: 0,
        }
    }
}

/// A stack of raster clips with Skia's deferred copy: `save` only counts, and the top clip is
/// copied when it is first modified (`SkRasterClipStack`).
// Port of: src/core/SkRasterClipStack.h#L16-L123 (chrome/m156)
#[doc(alias = "SkRasterClipStack")]
#[derive(Clone, Debug)]
pub struct RasterClipStack {
    stack: Vec<Rec>,
    root_bounds: IRect,
    disable_aa: bool,
}

impl RasterClipStack {
    /// A stack with one clip covering `(0, 0, width, height)`.
    // Port of: src/core/SkRasterClipStack.h#L18-L24 (chrome/m156)
    #[must_use]
    pub fn new(width: i32, height: i32) -> RasterClipStack {
        let root_bounds = IRect::from_wh(width, height);
        RasterClipStack {
            stack: vec![Rec::new(RasterClip::from_rect(&root_bounds))],
            root_bounds,
            disable_aa: path_requires_tiling(&root_bounds),
        }
    }

    /// Resizes the stack's root clip (`setNewSize`); the stack must be at its root.
    // Port of: src/core/SkRasterClipStack.h#L26-L33 (chrome/m156)
    #[doc(alias = "setNewSize")]
    #[allow(clippy::missing_panics_doc)] // the stack always holds the root clip
    pub fn set_new_size(&mut self, w: i32, h: i32) {
        self.root_bounds.set_ltrb(0, 0, w, h);

        debug_assert_eq!(self.stack.len(), 1);
        let root_bounds = self.root_bounds;
        let rec = self.stack.last_mut().expect("the stack is never empty");
        debug_assert_eq!(rec.deferred_count, 0);
        rec.rc.set_rect(&root_bounds);
    }

    /// The current clip (`rc`).
    // Port of: src/core/SkRasterClipStack.h#L35 (chrome/m156)
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // the stack always holds the root clip
    pub fn rc(&self) -> &RasterClip {
        &self.stack.last().expect("the stack is never empty").rc
    }

    /// Saves the clip (`save`).
    // Port of: src/core/SkRasterClipStack.h#L37-L41 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // the stack always holds the root clip
    pub fn save(&mut self) {
        let top = self.stack.last_mut().expect("the stack is never empty");
        debug_assert!(top.deferred_count >= 0);
        top.deferred_count += 1;
    }

    /// Restores the clip saved by the matching [`save`](Self::save) (`restore`).
    // Port of: src/core/SkRasterClipStack.h#L43-L53 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // the stack always holds the root clip
    pub fn restore(&mut self) {
        let top = self.stack.last_mut().expect("the stack is never empty");
        top.deferred_count -= 1;
        if top.deferred_count < 0 {
            debug_assert_eq!(top.deferred_count, -1);
            debug_assert!(self.stack.len() > 1);
            self.stack.pop();
        }
    }

    /// Clips to `rect` under `ctm` (`clipRect`).
    // Port of: src/core/SkRasterClipStack.h#L55-L58 (chrome/m156)
    #[doc(alias = "clipRect")]
    pub fn clip_rect(&mut self, ctm: &Matrix, rect: &Rect, op: ClipOp, aa: bool) {
        let aa = self.final_aa(aa);
        self.writable_rc().op_rect(rect, ctm, op, aa);
        self.validate();
    }

    /// Clips to `rrect` under `ctm` (`clipRRect`).
    // Port of: src/core/SkRasterClipStack.h#L60-L63 (chrome/m156)
    #[doc(alias = "clipRRect")]
    pub fn clip_rrect(&mut self, ctm: &Matrix, rrect: &RRect, op: ClipOp, aa: bool) {
        let aa = self.final_aa(aa);
        self.writable_rc().op_rrect(rrect, ctm, op, aa);
        self.validate();
    }

    /// Clips to `path` under `ctm` (`clipPath`).
    // Port of: src/core/SkRasterClipStack.h#L65-L68 (chrome/m156)
    #[doc(alias = "clipPath")]
    pub fn clip_path(&mut self, ctm: &Matrix, path: &Path, op: ClipOp, aa: bool) {
        let aa = self.final_aa(aa);
        self.writable_rc().op_path(path, ctm, op, aa);
        self.validate();
    }

    /// Adds a clip shader (`clipShader`).
    // Port of: src/core/SkRasterClipStack.h#L70-L73 (chrome/m156)
    #[doc(alias = "clipShader")]
    pub fn clip_shader(&mut self, sh: Shader) {
        self.writable_rc().op_shader(sh);
        self.validate();
    }

    /// Clips to `rgn` (`clipRegion`).
    // Port of: src/core/SkRasterClipStack.h#L75-L78 (chrome/m156)
    #[doc(alias = "clipRegion")]
    pub fn clip_region(&mut self, rgn: &Region, op: ClipOp) {
        self.writable_rc().op_region(rgn, op);
        self.validate();
    }

    /// Replaces the clip with `rect` clipped to the root bounds (`replaceClip`).
    // Port of: src/core/SkRasterClipStack.h#L80-L87 (chrome/m156)
    #[doc(alias = "replaceClip")]
    pub fn replace_clip(&mut self, rect: &IRect) {
        let root_bounds = self.root_bounds;
        match IRect::intersect(rect, &root_bounds) {
            None => {
                self.writable_rc().set_empty();
            }
            Some(dev_rect) => {
                self.writable_rc().set_rect(&dev_rect);
            }
        }
    }

    // Port of: src/core/SkRasterClipStack.h#L89-L99 (chrome/m156)
    fn validate(&self) {
        #[cfg(debug_assertions)]
        {
            use skia_rust_core::rect::Contains;
            let clip = self.rc();
            if self.root_bounds.is_empty() {
                debug_assert!(clip.is_empty());
            } else if !clip.is_empty() {
                debug_assert!(self.root_bounds.contains(clip.bounds()));
            }
        }
    }

    // Port of: src/core/SkRasterClipStack.h#L109-L117 (chrome/m156)
    fn writable_rc(&mut self) -> &mut RasterClip {
        let top = self.stack.last_mut().expect("the stack is never empty");
        debug_assert!(top.deferred_count >= 0);
        if top.deferred_count > 0 {
            top.deferred_count -= 1;
            let copy = Rec::new(top.rc.clone());
            self.stack.push(copy);
        }
        &mut self.stack.last_mut().expect("the stack is never empty").rc
    }

    // Port of: src/core/SkRasterClipStack.h#L119 (chrome/m156)
    fn final_aa(&self, aa: bool) -> bool {
        aa && !self.disable_aa
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_saves_copy_on_first_write() {
        let mut s = RasterClipStack::new(20, 10);
        assert_eq!(*s.rc().bounds(), IRect::from_wh(20, 10));
        s.save();
        s.save();
        // Nothing is copied yet.
        assert_eq!(s.stack.len(), 1);
        s.clip_rect(
            &Matrix::new_identity(),
            &Rect::new(2.0, 2.0, 8.0, 8.0),
            ClipOp::Intersect,
            false,
        );
        assert_eq!(s.stack.len(), 2);
        assert_eq!(*s.rc().bounds(), IRect::new(2, 2, 8, 8));
        s.clip_rect(
            &Matrix::new_identity(),
            &Rect::new(4.0, 4.0, 6.0, 6.0),
            ClipOp::Intersect,
            true,
        );
        // The second save was used up by the first clip; the top clip is now modified in place.
        assert_eq!(s.stack.len(), 2);
        assert_eq!(*s.rc().bounds(), IRect::new(4, 4, 6, 6));
        s.restore();
        assert_eq!(*s.rc().bounds(), IRect::from_wh(20, 10));
        assert_eq!(s.stack.len(), 1);
        s.restore();
        assert_eq!(*s.rc().bounds(), IRect::from_wh(20, 10));
    }

    #[test]
    fn replace_clip_is_limited_to_the_root() {
        let mut s = RasterClipStack::new(20, 10);
        s.replace_clip(&IRect::new(-5, -5, 5, 30));
        assert_eq!(*s.rc().bounds(), IRect::new(0, 0, 5, 10));
        s.replace_clip(&IRect::new(40, 40, 50, 50));
        assert!(s.rc().is_empty());
    }

    #[test]
    fn huge_devices_disable_aa_clips() {
        let mut s = RasterClipStack::new(40_000, 10);
        s.clip_rect(
            &Matrix::new_identity(),
            &Rect::new(0.5, 0.5, 100.5, 5.5),
            ClipOp::Intersect,
            true,
        );
        assert!(s.rc().is_bw());
    }
}

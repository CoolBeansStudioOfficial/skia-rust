// Copyright 2017 Google LLC
// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/core/SkClipStackDevice.{h,cpp}, src/utils/SkClipStackUtils.cpp (chrome/m156)

//! The clip handling of `SkClipStackDevice`, the base class of the PDF device, and
//! `SkClipStack_AsPath`.
//!
//! skia-rust: a device is a [`Device`](skia_rust_core::device::Device) over a
//! [`DeviceState`], so the base class is this struct, which the device embeds. Its methods take
//! the `DeviceState` where Skia reads `this->localToDevice()` or the image info.

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::clip_stack::{
    B2TIter, BoundsType, ClipStack, DeviceSpaceType, Iter, IterStart,
};
use skia_rust_core::device::DeviceState;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_priv::map_rect;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::region::{Op as RegionOp, Region};
use skia_rust_core::rrect::RRect;
use skia_rust_core::shader::Shader;
use skia_rust_raster::region_path::RegionExt;

use skia_rust_pathops::path_op::PathOp;

/// The clip stack of a device and the clip methods of `SkClipStackDevice`.
// Port of: src/core/SkClipStackDevice.h#L21-L46 (chrome/m156)
#[doc(alias = "SkClipStackDevice")]
#[derive(Debug, Default)]
pub struct ClipStackDevice {
    clip_stack: ClipStack,
}

impl ClipStackDevice {
    /// A device with a wide-open clip.
    #[must_use]
    pub fn new() -> Self {
        Self {
            clip_stack: ClipStack::new(),
        }
    }

    /// `cs()`.
    #[must_use]
    pub fn cs(&self) -> &ClipStack {
        &self.clip_stack
    }

    /// `cs()`, mutable.
    pub fn cs_mut(&mut self) -> &mut ClipStack {
        &mut self.clip_stack
    }

    /// `devClipBounds`.
    // Port of: src/core/SkClipStackDevice.cpp#L20-L27 (chrome/m156)
    #[doc(alias = "devClipBounds")]
    #[must_use]
    pub fn dev_clip_bounds(&self, state: &DeviceState) -> IRect {
        let r: IRect = self
            .clip_stack
            .bounds(&state.image_info().bounds())
            .round_out();
        if !r.is_empty() {
            debug_assert!(state.image_info().bounds().contains_no_empty_check(&r));
        }
        r
    }

    /// `pushClipStack`.
    // Port of: src/core/SkClipStackDevice.cpp#L31-L33 (chrome/m156)
    pub fn push_clip_stack(&mut self) {
        self.clip_stack.save();
    }

    /// `popClipStack`.
    // Port of: src/core/SkClipStackDevice.cpp#L35-L37 (chrome/m156)
    pub fn pop_clip_stack(&mut self) {
        self.clip_stack.restore();
    }

    /// `clipRect`.
    // Port of: src/core/SkClipStackDevice.cpp#L39-L41 (chrome/m156)
    pub fn clip_rect(&mut self, state: &DeviceState, rect: &Rect, op: ClipOp, aa: bool) {
        self.clip_stack
            .clip_rect(rect, state.local_to_device(), op, aa);
    }

    /// `clipRRect`.
    // Port of: src/core/SkClipStackDevice.cpp#L43-L45 (chrome/m156)
    pub fn clip_rrect(&mut self, state: &DeviceState, rrect: &RRect, op: ClipOp, aa: bool) {
        self.clip_stack
            .clip_rrect(rrect, state.local_to_device(), op, aa);
    }

    /// `clipPath`.
    // Port of: src/core/SkClipStackDevice.cpp#L47-L49 (chrome/m156)
    pub fn clip_path(&mut self, state: &DeviceState, path: &Path, op: ClipOp, aa: bool) {
        self.clip_stack
            .clip_path(path, state.local_to_device(), op, aa);
    }

    /// `onClipShader`.
    // Port of: src/core/SkClipStackDevice.cpp#L51-L53 (chrome/m156)
    pub fn on_clip_shader(&mut self, shader: Shader) {
        self.clip_stack.clip_shader(shader);
    }

    /// `clipRegion`.
    // Port of: src/core/SkClipStackDevice.cpp#L55-L63 (chrome/m156)
    pub fn clip_region(&mut self, state: &DeviceState, rgn: &Region, op: ClipOp) {
        let origin = state.origin();
        let mut builder = PathBuilder::new();
        let _ = rgn.add_boundary_path(&mut builder);
        builder.transform(&Matrix::translate((-(origin.x as f32), -(origin.y as f32))));
        self.clip_stack
            .clip_path(&builder.detach(), &Matrix::new_identity(), op, false);
    }

    /// `replaceClip`.
    // Port of: src/core/SkClipStackDevice.cpp#L65-L68 (chrome/m156)
    pub fn replace_clip(&mut self, state: &DeviceState, rect: &IRect) {
        let device_rect = map_rect(state.global_to_device(), &Rect::from_irect(rect));
        self.clip_stack.replace_clip(&device_rect, false);
    }

    /// `isClipAntiAliased`.
    // Port of: src/core/SkClipStackDevice.cpp#L70-L81 (chrome/m156)
    #[must_use]
    pub fn is_clip_anti_aliased(&self) -> bool {
        let mut iter = B2TIter::new(&self.clip_stack);
        while let Some(element) = iter.next() {
            if element.is_aa() {
                return true;
            }
        }
        false
    }

    /// `isClipWideOpen`.
    // Port of: src/core/SkClipStackDevice.cpp#L83-L85 (chrome/m156)
    #[must_use]
    pub fn is_clip_wide_open(&self, state: &DeviceState) -> bool {
        self.clip_stack
            .quick_contains_rect(&Rect::from_iwh(state.width(), state.height()))
    }

    /// `isClipEmpty`.
    // Port of: src/core/SkClipStackDevice.cpp#L87-L89 (chrome/m156)
    #[must_use]
    pub fn is_clip_empty(&self, state: &DeviceState) -> bool {
        self.clip_stack
            .is_empty(&IRect::from_wh(state.width(), state.height()))
    }

    /// `isClipRect`.
    // Port of: src/core/SkClipStackDevice.cpp#L91-L104 (chrome/m156)
    #[must_use]
    pub fn is_clip_rect(&self, state: &DeviceState) -> bool {
        if self.is_clip_wide_open(state) {
            return true;
        } else if self.is_clip_empty(state) {
            return false;
        }
        let (_, bound_type, is_intersection_of_rects) = self.clip_stack.get_bounds();
        is_intersection_of_rects && bound_type == BoundsType::Normal
    }

    /// `android_utils_clipAsRgn`.
    // Port of: src/core/SkClipStackDevice.cpp#L106-L135 (chrome/m156)
    pub fn android_utils_clip_as_rgn(&self, state: &DeviceState, rgn: &mut Region) {
        let (bounds, bound_type, is_intersection_of_rects) = self.clip_stack.get_bounds();
        if is_intersection_of_rects && BoundsType::Normal == bound_type {
            rgn.set_rect(bounds.round());
        } else {
            let mut bounds_rgn = Region::new();
            bounds_rgn.set_rect(IRect::new(0, 0, state.width(), state.height()));

            *rgn = bounds_rgn.clone();
            let mut iter = B2TIter::new(&self.clip_stack);
            while let Some(elem) = iter.next() {
                let tmp_path = elem.as_device_space_path();
                let mut tmp_rgn = Region::new();
                tmp_rgn.set_path(&tmp_path, &bounds_rgn);
                if elem.is_replace_op() {
                    // All replace elements are rectangles
                    rgn.set_rect(elem.device_space_rect().round());
                } else {
                    // `static_cast<SkRegion::Op>(elem->getOp())`: Difference == 0, Intersect == 1.
                    let region_op = match elem.op() {
                        ClipOp::Difference => RegionOp::Difference,
                        ClipOp::Intersect => RegionOp::Intersect,
                    };
                    rgn.op_region(&tmp_rgn, region_op);
                }
            }
        }
    }
}

/// `SkClipStack_AsPath`: the clip stack as one path, in device space.
// Port of: src/utils/SkClipStackUtils.cpp#L14-L45 (chrome/m156)
#[doc(alias = "SkClipStack_AsPath")]
#[must_use]
pub fn clip_stack_as_path(cs: &ClipStack) -> Path {
    let mut path = Path::new();
    path = path.with_fill_type(PathFillType::InverseEvenOdd);

    let mut iter = Iter::new(cs, IterStart::Bottom);
    while let Some(element) = iter.next() {
        if element.device_space_type() == DeviceSpaceType::Shader {
            // TODO: Handle DeviceSpaceType::kShader somehow; it can't be turned into an SkPath
            // but perhaps the pdf backend can apply shaders in another way.
            continue;
        }
        let operand = if element.device_space_type() == DeviceSpaceType::Empty {
            Path::new()
        } else {
            element.as_device_space_path()
        };

        let element_op = element.op();
        if element.is_replace_op() {
            path = operand;
            // TODO: Once expanding clip ops are removed, we can switch the iterator to be top
            // to bottom, which allows us to break here on encountering a replace op.
        } else {
            let path_op = match element_op {
                ClipOp::Difference => PathOp::Difference,
                ClipOp::Intersect => PathOp::Intersect,
            };
            if let Some(result) = skia_rust_pathops::op(&path, &operand, path_op) {
                path = result;
            }
        }
    }
    path
}

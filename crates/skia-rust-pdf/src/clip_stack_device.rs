// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/utils/SkClipStackUtils.cpp (chrome/m156)

//! `SkClipStack_AsPath`. `SkClipStackDevice` itself lives in `skia_rust_raster::clip_stack_device`
//! (the SVG device shares it) and is re-exported here.

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::clip_stack::{ClipStack, DeviceSpaceType, Iter, IterStart};
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_pathops::path_op::PathOp;

pub use skia_rust_raster::clip_stack_device::ClipStackDevice;

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

// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Checks the `PathOpsExt` methods against the free functions they forward to (no Skia
// counterpart; see docs/API_MAPPING.md).

use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::rect::Rect;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_pathops::{PathOpsExt, as_winding, op, simplify, tight_bounds};

#[test]
fn path_ops_ext_forwards_to_free_functions() {
    let one = Path::rect(Rect::new(0.0, 0.0, 6.0, 6.0), PathDirection::CW);
    let two = Path::rect(Rect::new(3.0, 3.0, 9.0, 9.0), PathDirection::CW);
    assert_eq!(
        one.op(&two, PathOp::Intersect),
        op(&one, &two, PathOp::Intersect)
    );
    assert_eq!(one.simplify(), simplify(&one));
    assert_eq!(one.tight_bounds(), tight_bounds(&one));
    assert_eq!(one.as_winding(), as_winding(&one));
}

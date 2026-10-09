// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkMatrix22.{h,cpp}

//! The Givens rotation that splits a 2x2 matrix into a rotation and an upper-triangular part.

use crate::matrix::Matrix;
use crate::point::Point;
use crate::scalar::{scalar, scalar_copy_sign};

/// `SkComputeGivensRotation`: the rotation `G` that maps `h` onto the x axis, so that `G * A`
/// has no skew in its first column. `h` is where the matrix maps the x axis.
// Port of: src/utils/SkMatrix22.cpp#L14-L40 (chrome/m156)
#[doc(alias = "SkComputeGivensRotation")]
#[must_use]
#[allow(clippy::many_single_char_names)] // mirrors the names in SkComputeGivensRotation
pub fn compute_givens_rotation(h: Point) -> Matrix {
    let a: scalar = h.x;
    let b: scalar = h.y;
    let c: scalar;
    let s: scalar;
    if 0.0 == b {
        c = scalar_copy_sign(1.0, a);
        s = 0.0;
    } else if 0.0 == a {
        c = 0.0;
        s = -scalar_copy_sign(1.0, b);
    } else if b.abs() > a.abs() {
        let t = a / b;
        let u = scalar_copy_sign((1.0 + t * t).sqrt(), b);
        s = -1.0 / u;
        c = -s * t;
    } else {
        let t = b / a;
        let u = scalar_copy_sign((1.0 + t * t).sqrt(), a);
        c = 1.0 / u;
        s = -c * t;
    }
    let mut g = Matrix::new_identity();
    g.set_sin_cos((s, c), None);
    g
}

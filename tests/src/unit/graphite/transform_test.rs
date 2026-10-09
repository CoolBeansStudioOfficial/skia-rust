// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/TransformTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::m44::M44;
use skia_rust_gpu::graphite::geom::transform::{Transform, Type};

use crate::{def_test, reporter_assert};

def_test!(TransformTest, |r| {
    // Port of: tests/graphite/TransformTest.cpp#L12-L16 (chrome/m156)
    let t = Transform::new(M44::new_identity());
    reporter_assert!(r, t.type_() == Type::Identity);
});

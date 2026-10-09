// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/ShapeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_gpu::graphite::geom::shape::{Shape, Type};

use crate::{def_test, reporter_assert};

def_test!(ShapeTest, |r| {
    // Port of: tests/graphite/ShapeTest.cpp#L12-L16 (chrome/m156)
    let s = Shape::default();
    reporter_assert!(r, s.type_() == Type::Empty);
});

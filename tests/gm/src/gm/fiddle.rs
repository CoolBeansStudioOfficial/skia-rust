// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fiddle.cpp (chrome/m156)

use crate::prelude::*;

// "Paste your fiddle.skia.org code over this stub."
// Port of: gm/fiddle.cpp#L15 (chrome/m156)
fn draw(_canvas: &Canvas) {}

// Port of: gm/fiddle.cpp#L12 (chrome/m156)
crate::def_simple_gm!(fiddle, canvas, 256, 256, { draw(canvas) });

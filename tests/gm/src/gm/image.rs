// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/image.cpp (chrome/m156)
//
// Only `new_texture_image` is ported here (the text-module entry of this file). The other GMs in
// gm/image.cpp have their own manifest entries.
//
// Skip-matching only (docs/design/text.md §1.2): this GM is GPU-only. `isGPU` is false on a raster
// sink, so it reports `kErrorMsg_DrawSkippedGpuOnly` and skips before it draws.

use crate::prelude::*;

// Port of: gm/image.cpp#L374-L385 (chrome/m156), new_texture_image (the GPU-only skip)
crate::def_simple_gm_can_fail!(new_texture_image, canvas, error_msg, 280, 115, {
    // No GPU context or Graphite recorder on a raster sink, so `isGPU` is false.
    error_msg.clear();
    error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
    DrawResult::Skip
});

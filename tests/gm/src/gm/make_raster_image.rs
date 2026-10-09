// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/make_raster_image.cpp (chrome/m156)

use crate::tool_utils::get_resource_as_image;

// Port of: gm/make_raster_image.cpp#L15-L19 (chrome/m156)
crate::def_simple_gm!(makeRasterImage, canvas, 128, 128, {
    if let Some(img) = get_resource_as_image("images/color_wheel.png") {
        // A null image (no raster copy) draws nothing, as `drawImage(nullptr)` does.
        if let Some(raster) = img.to_raster_image() {
            canvas.draw_image(&raster, (0.0, 0.0), None);
        }
    }
});

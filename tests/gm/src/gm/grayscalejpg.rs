// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/grayscalejpg.cpp (chrome/m156)

use crate::tool_utils::get_resource_as_image;

// Port of: gm/grayscalejpg.cpp#L20-L29 (chrome/m156)
// Test decoding grayscale JPEG (crbug.com/436079).
crate::def_simple_gm!(grayscalejpg, canvas, 128, 128, {
    let resource = "images/grayscale.jpg";
    if let Some(image) = get_resource_as_image(resource) {
        canvas.draw_image(&image, (0.0, 0.0), None);
    } else {
        eprintln!("\nCould not decode file '{resource}'. Did you forget to set the resourcePath?");
    }
});

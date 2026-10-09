// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_8664.cpp (chrome/m156)

use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;

// Port of: gm/skbug_8664.cpp#L18-L65 (chrome/m156)
crate::def_simple_gm!(skbug_8664, canvas, 830, 550, {
    // (x scale, y scale, x translate, y translate)
    let xforms = [
        (1.0_f32, 1.0_f32, 0.0_f32, 0.0_f32),
        (0.5, 0.5, 530.0, 0.0),
        (0.25, 0.25, 530.0, 275.0),
        (0.125, 0.125, 530.0, 420.0),
    ];

    // Must be at least medium to require mipmaps when we downscale the image
    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);
    let image = get_resource_as_image("images/mandrill_512.png");
    let mut overlay_paint = Paint::default();
    overlay_paint.set_color(Color::new(0x80FF_FFFF));

    // Make the overlay visible even when the downscaled images fail to render
    canvas.clear(Color::new(0xFF88_8888));
    canvas.translate((20.0, 20.0));
    for (sx, sy, tx, ty) in xforms {
        let saved = canvas.save();
        canvas.translate((tx, ty));
        canvas.scale((sx, sy));
        // Draw an image, possibly down sampled, which forces us to generate mipmaps inline
        // on the second iteration.
        if let Some(image) = &image {
            canvas.draw_image_with_sampling_options(image, (0.0, 0.0), sampling, None);
        }

        // Draw an overlay that requires the scissor test for its clipping.
        let inner = Rect::from_ltrb(32.0, 32.0, 480.0, 480.0);
        let outer = inner.with_outset((16.0, 16.0));

        // Clip to smaller rectangle
        let saved_inner = canvas.save();
        canvas.clip_rect(inner, None, None);
        // Then apply a rotation and draw a larger rectangle to ensure the clip cannot be dropped
        canvas.rotate(20.0, None);
        canvas.draw_rect(outer, &overlay_paint);
        canvas.restore_to_count(saved_inner);
        canvas.restore_to_count(saved);
    }
});

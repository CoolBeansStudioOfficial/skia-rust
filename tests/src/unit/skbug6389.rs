// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/Skbug6389.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_effects::image_filters;
use skia_rust_raster::surfaces;

use crate::def_test;
use crate::resources::get_resource_as_image;

// Port of: tests/Skbug6389.cpp#L20-L29 (chrome/m156)
def_test!(skbug_6389, |_reporter| {
    let mut s = surfaces::raster_n32_premul((100, 100)).expect("surface");
    let mut p = Paint::default();
    p.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 5.0, None));
    p.set_image_filter(image_filters::image(
        get_resource_as_image("images/mandrill_512.png"),
        Rect::new(0.0, 0.0, 0.0, 0.0),
        Rect::new(0.0, 0.0, 0.0, 0.0),
        SamplingOptions::default(),
    ));
    s.canvas().draw_paint(&p);
});

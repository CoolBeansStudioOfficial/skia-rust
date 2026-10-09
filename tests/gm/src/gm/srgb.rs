// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/srgb.cpp (chrome/m156)

use crate::tool_utils::get_resource_as_image;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::SamplingOptions;

// Port of: gm/srgb.cpp#L13-L43 (chrome/m156), srgb_colorfilter
crate::def_simple_gm!(srgb_colorfilter, canvas, 512, 256 * 3, {
    let Some(img) = get_resource_as_image("images/mandrill_256.png") else {
        return;
    };
    #[rustfmt::skip]
    let array = ColorMatrix::new(
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0, 0.0,
        -1.0, 0.0, 0.0, 1.0, 0.0,
    );
    let cf0: Option<ColorFilter> = color_filters::matrix(&array, Clamp::Yes);
    let cf1: Option<ColorFilter> = Some(color_filters::linear_to_srgb_gamma());
    let cf2: Option<ColorFilter> = Some(color_filters::srgb_to_linear_gamma());
    let sampling = SamplingOptions::default();
    let mut p = Paint::default();
    p.set_color_filter(cf0.clone());
    canvas.draw_image(&img, (0.0, 0.0), None);
    canvas.draw_image_with_sampling_options(&img, (256.0, 0.0), sampling, Some(&p));
    p.set_color_filter(cf1.clone());
    canvas.draw_image_with_sampling_options(&img, (0.0, 256.0), sampling, Some(&p));
    p.set_color_filter(compose(cf1.as_ref(), cf0.as_ref()));
    canvas.draw_image_with_sampling_options(&img, (256.0, 256.0), sampling, Some(&p));
    p.set_color_filter(cf2.clone());
    canvas.draw_image_with_sampling_options(&img, (0.0, 512.0), sampling, Some(&p));
    p.set_color_filter(compose(cf2.as_ref(), cf0.as_ref()));
    canvas.draw_image_with_sampling_options(&img, (256.0, 512.0), sampling, Some(&p));
});

// `outer->makeComposed(inner)`: `outer` applied after `inner`.
fn compose(outer: Option<&ColorFilter>, inner: Option<&ColorFilter>) -> Option<ColorFilter> {
    color_filters::compose(outer, inner.cloned())
}

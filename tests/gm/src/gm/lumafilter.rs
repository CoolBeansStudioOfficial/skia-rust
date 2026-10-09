// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lumafilter.cpp (chrome/m156)
//
// Not ported: `LumaFilterGM`, which draws its labels with text (SkFont and drawSimpleText).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::{named_gamut, named_transfer_fn};
use skia_rust_core::data::Data;
use skia_rust_core::paint::Paint;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::working_format_color_filter::with_working_format;
use skia_rust_effects::luma_color_filter;

// Port of: gm/lumafilter.cpp#L168-L198 (chrome/m156), DEF_SIMPLE_GM(AlternateLuma)
crate::def_simple_gm!(AlternateLuma, canvas, 384, 128, {
    let Some(img) = crate::tool_utils::get_resource_as_image("images/mandrill_128.png") else {
        return;
    };

    // Normal luma colorfilter on the left.
    let mut paint = Paint::default();
    paint.set_color_filter(luma_color_filter::make());
    canvas.draw_image_with_sampling_options(
        &img,
        (0.0, 0.0),
        SamplingOptions::from(FilterMode::Nearest),
        Some(&paint),
    );
    canvas.translate((128.0, 0.0));

    // Original image in the middle for reference.
    canvas.draw_image(&img, (0.0, 0.0), None);
    canvas.translate((128.0, 0.0));

    // Here, RGB holds CIE XYZ. Splatting the G (Y) channel should result in (near) greyscale.
    let effect = RuntimeEffect::make_for_color_filter(
        "half4 main(half4 inColor) { return inColor.ggga; }",
        None,
    )
    .expect("the effect compiles");

    let filter = effect
        .make_color_filter(Data::new_empty(), &[])
        .expect("a colour filter");
    let unpremul = AlphaType::Unpremul;
    paint.set_color_filter(with_working_format(
        Some(filter),
        Some(&named_transfer_fn::LINEAR),
        Some(&named_gamut::XYZ),
        Some(&unpremul),
    ));
    canvas.draw_image_with_sampling_options(
        &img,
        (0.0, 0.0),
        SamplingOptions::from(FilterMode::Nearest),
        Some(&paint),
    );
});

// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/complexclip.cpp (chrome/m156)
//
// Only `clip_shader_layer` is ported here: `clip_shader` needs SkShaders::Blend (combining clip
// shaders, not ported), `ComplexClipGM` labels its draws with text
// (`drawSimpleText`), `clip_shader_nested` and `clip_shader_persp` draw text banners, and
// `clip_shader_difference` has a text quadrant. Text is not ported yet.

use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;

use crate::tool_utils::{get_resource_as_image, int_to_scalar};

// Port of: gm/complexclip.cpp#L254-L268 (chrome/m156)
crate::def_simple_gm!(clip_shader_layer, canvas, 430, 320, {
    let img = get_resource_as_image("images/yellow_rose.png")
        .expect("images/yellow_rose.png (set SKIA_RESOURCES)");
    let sh = img
        .to_shader(None, SamplingOptions::default(), None)
        .expect("shader");

    let r = Rect::from_wh(int_to_scalar(img.width()), int_to_scalar(img.height()));

    canvas.translate((10.0, 10.0));
    // now add the cool clip
    canvas.clip_rect(r, None, None);
    canvas.clip_shader(sh, None);
    // now draw a layer with the same image, and watch it get restored w/ the clip
    canvas.save_layer(&SaveLayerRec::default().bounds(&r));
    canvas.draw_color(Color::new(0xFFFF_0000), None);
    canvas.restore();
});

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/destcolor.cpp (chrome/m156)

use skia_rust_core::data::Data;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::RuntimeEffect;

// Port of: gm/destcolor.cpp#L17-L31 (chrome/m156), DEF_SIMPLE_GM(destcolor)
crate::def_simple_gm!(destcolor, canvas, 640, 640, {
    // Draw the mandrill.
    canvas.draw_image(
        crate::tool_utils::get_resource_as_image("images/mandrill_512.png")
            .expect("images/mandrill_512.png"),
        (0.0, 0.0),
        None,
    );
    // Now let's add our test effect on top. It reads back the original image and inverts it.
    let effect = RuntimeEffect::make_for_blender(
        r"
        half4 main(half4 src, half4 dst) {
            return (half4(1) - dst).rgb1;
        }
    ",
        None,
    )
    .expect("the effect compiles");
    let mut invert_paint = Paint::default();
    invert_paint.set_anti_alias(true);
    invert_paint.set_blender(effect.make_blender(Data::new_empty(), &[]));
    canvas.draw_oval(Rect::from_ltrb(128.0, 128.0, 640.0, 640.0), &invert_paint);
});

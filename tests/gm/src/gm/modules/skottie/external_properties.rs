// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/gm/ExternalProperties.cpp (chrome/m156)
//
// `onAnimate` is not run: the golden is the first frame, as for the other Skottie GMs.

use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::rect::Rect;
use skia_rust_core::typeface::Typeface;
use skia_rust_resources::ResourceProvider;
use skia_rust_shaper::best_available;
use skia_rust_skottie::utils::CustomPropertyManager;
use skia_rust_skottie::{Animation, Builder};
use skia_rust_tools::font_tool_utils::{create_typeface_from_resource, test_font_mgr};
use skia_rust_tools::resources::get_resource_as_stream;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_data;

const WEB_FONT_RESOURCE: &str = "fonts/Roboto-Regular.ttf";
const SKOTTIE_RESOURCE: &str = "skottie/skottie_sample_webfont.json";

// Mock web font loader which serves a single local font (checked in under resources/).
// Port of: modules/skottie/gm/ExternalProperties.cpp#L29-L41 (FakeWebFontProvider)
struct FakeWebFontProvider {
    typeface: Option<Typeface>,
}

impl FakeWebFontProvider {
    fn new() -> Self {
        Self {
            typeface: create_typeface_from_resource(get_resource_as_stream(WEB_FONT_RESOURCE), 0),
        }
    }
}

impl ResourceProvider for FakeWebFontProvider {
    // Port of: modules/skottie/gm/ExternalProperties.cpp#L34-L36 (loadTypeface)
    fn load_typeface(&self, _name: &str, _url: &str) -> Option<Typeface> {
        self.typeface.clone()
    }
}

/// `kSize`, as the `int` the size is made from.
const SIZE: i32 = 800;

/// `kSize`, as the `SkScalar` the destination rectangles are made from.
const SIZE_SCALAR: f32 = 800.0;

// Port of: modules/skottie/gm/ExternalProperties.cpp#L44-L140 (SkottieExternalPropsGM)
struct SkottieExternalPropsGm {
    animation: Option<Animation>,
    prop_manager: Option<CustomPropertyManager>,
}

impl SkottieExternalPropsGm {
    fn new() -> Self {
        Self {
            animation: None,
            prop_manager: None,
        }
    }

    // Port of: modules/skottie/gm/ExternalProperties.cpp#L92-L127 (update_props)
    #[allow(clippy::items_after_statements)] // the function-local table of C++
    fn update_props(&self, i: usize) {
        debug_assert!(i < 4);
        if i == 0 {
            return;
        }

        struct Test {
            txt_string: &'static str,
            txt_color: Color,
            solid_color: Color,
            transform_scale: f32,
        }
        let tests = [
            Test {
                txt_string: "update #1",
                txt_color: Color::RED,
                solid_color: Color::YELLOW,
                transform_scale: 100.0,
            },
            Test {
                txt_string: "update #2",
                txt_color: Color::GREEN,
                solid_color: Color::BLUE,
                transform_scale: 50.0,
            },
            Test {
                txt_string: "update #3",
                txt_color: Color::MAGENTA,
                solid_color: Color::CYAN,
                transform_scale: 150.0,
            },
        ];

        debug_assert!(i - 1 < tests.len());
        let tst = &tests[i - 1];

        let Some(prop_manager) = &self.prop_manager else {
            return;
        };

        for prop in prop_manager.get_color_props() {
            // SkAssertResult
            assert!(prop_manager.set_color(&prop, tst.solid_color));
        }

        for prop in prop_manager.get_transform_props() {
            let mut t = prop_manager.get_transform(&prop);
            t.scale = (tst.transform_scale, tst.transform_scale).into();
            // SkAssertResult
            assert!(prop_manager.set_transform(&prop, &t));
        }

        for prop in prop_manager.get_text_props() {
            let mut txt = prop_manager.get_text(&prop);
            tst.txt_string.clone_into(&mut txt.text);
            txt.fill_color = tst.txt_color;
            // SkAssertResult
            assert!(prop_manager.set_text(&prop, &txt));
        }
    }
}

impl GM for SkottieExternalPropsGm {
    // Port of: modules/skottie/gm/ExternalProperties.cpp#L48 (getName)
    fn name(&self) -> String {
        "skottie_external_props".to_owned()
    }

    // Port of: modules/skottie/gm/ExternalProperties.cpp#L50 (getISize)
    fn size(&mut self) -> ISize {
        ISize::new(SIZE, SIZE)
    }

    // Port of: modules/skottie/gm/ExternalProperties.cpp#L52-L61 (onOnceBeforeDraw)
    fn on_once_before_draw(&mut self) {
        if let Some(data) = get_resource_as_data(SKOTTIE_RESOURCE) {
            let prop_manager = CustomPropertyManager::default();
            let mut builder = Builder::new();
            builder
                .set_font_manager(test_font_mgr())
                .set_resource_provider(Rc::new(FakeWebFontProvider::new()))
                .set_property_observer(prop_manager.get_property_observer())
                .set_text_shaping_factory(best_available());
            self.animation = builder.make(&data);
            self.prop_manager = Some(prop_manager);
        }
    }

    // Port of: modules/skottie/gm/ExternalProperties.cpp#L63-L77 (onDraw)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        if self.animation.is_none() {
            "No animation".clone_into(error_msg);
            return DrawResult::Fail;
        }

        for i in 0..4_usize {
            self.update_props(i);
            #[allow(clippy::cast_precision_loss)] // i is in 0..4
            let dest = Rect::from_wh(SIZE_SCALAR / 2.0, SIZE_SCALAR / 2.0).with_offset((
                SIZE_SCALAR * ((i & 1) as f32) / 2.0,
                SIZE_SCALAR * ((i & 2) as f32) / 4.0,
            ));
            if let Some(animation) = &self.animation {
                animation.render(canvas, Some(&dest));
            }
        }
        DrawResult::Ok
    }
}

crate::def_gm!(SkottieExternalPropsGM, SkottieExternalPropsGm::new());

// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/gm/SkottieGM.cpp (chrome/m156). `onAnimate` is not run: the golden is
// the first frame, as for the animated GIF GM. `SkottieColorizeGM::onChar` (the `c` key cycling
// the colors) is not run either: the harness sends no key events, and the golden is drawn with
// the default colors.

use std::rc::Rc;

use skia_rust_codec::wuffs_codec;
use skia_rust_core::rect::Rect;
use skia_rust_core::stream::MemoryStream;
use skia_rust_core::typeface::Typeface;
use skia_rust_resources::{
    ImageAsset, ImageDecodeStrategy, MultiFrameImageAsset, ResourceProvider,
};
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
// Port of: modules/skottie/gm/SkottieGM.cpp#L32-L44 (FakeWebFontProvider)
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
    // Port of: modules/skottie/gm/SkottieGM.cpp#L37-L39 (loadTypeface)
    fn load_typeface(&self, _name: &str, _url: &str) -> Option<Typeface> {
        self.typeface.clone()
    }
}

/// `kSize` of the GMs, as the `int` the size is made from.
// Port of: modules/skottie/gm/SkottieGM.cpp#L168 (kSize, in SkottieColorizeGM)
const SIZE: i32 = 800;

/// `kSize` as the `SkScalar` the destination rectangle is made from.
const SIZE_SCALAR: f32 = 800.0;

// Port of: modules/skottie/gm/SkottieGM.cpp#L52-L100 (SkottieWebFontGM)
struct SkottieWebFontGm {
    animation: Option<Animation>,
}

impl SkottieWebFontGm {
    fn new() -> Self {
        Self { animation: None }
    }
}

impl GM for SkottieWebFontGm {
    // Port of: modules/skottie/gm/SkottieGM.cpp#L56 (getName)
    fn name(&self) -> String {
        "skottie_webfont".to_owned()
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L58 (getISize)
    fn size(&mut self) -> ISize {
        ISize::new(SIZE, SIZE)
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L60-L68 (onOnceBeforeDraw)
    fn on_once_before_draw(&mut self) {
        if let Some(data) = get_resource_as_data(SKOTTIE_RESOURCE) {
            let mut builder = Builder::new();
            builder
                .set_font_manager(test_font_mgr())
                .set_resource_provider(Rc::new(FakeWebFontProvider::new()))
                .set_text_shaping_factory(best_available());
            self.animation = builder.make(&data);
        }
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L70-L79 (onDraw)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(animation) = &self.animation else {
            "No animation".clone_into(error_msg);
            return DrawResult::Fail;
        };

        let dest = Rect::from_wh(SIZE_SCALAR, SIZE_SCALAR);
        animation.render(canvas, Some(&dest));
        DrawResult::Ok
    }
}

crate::def_gm!(SkottieWebFontGM, SkottieWebFontGm::new());

// Port of: modules/skottie/gm/SkottieGM.cpp#L223-L233 (SkottieMultiFrameGM::MultiFrameResourceProvider)
struct MultiFrameResourceProvider;

impl ResourceProvider for MultiFrameResourceProvider {
    // Port of: modules/skottie/gm/SkottieGM.cpp#L225-L232 (loadImageAsset)
    fn load_image_asset(&self, _path: &str, _name: &str, _id: &str) -> Option<Rc<dyn ImageAsset>> {
        let data = get_resource_as_data("images/flightAnim.gif")?;
        // SkGifDecoder::Decode(data, nullptr)
        let codec = wuffs_codec::make_from_stream(MemoryStream::make_copy(&data)).ok()?;
        let asset: Rc<dyn ImageAsset> =
            MultiFrameImageAsset::make_from_codec(codec, ImageDecodeStrategy::LazyDecode);
        Some(asset)
    }
}

// Port of: modules/skottie/gm/SkottieGM.cpp#L185 (SkottieMultiFrameGM)
struct SkottieMultiFrameGm {
    animation: Option<Animation>,
}

impl SkottieMultiFrameGm {
    fn new() -> Self {
        Self { animation: None }
    }
}

impl GM for SkottieMultiFrameGm {
    // Port of: modules/skottie/gm/SkottieGM.cpp#L188 (getName)
    fn name(&self) -> String {
        "skottie_multiframe".to_owned()
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L190 (getISize)
    fn size(&mut self) -> ISize {
        ISize::new(SIZE, SIZE)
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L192 (onOnceBeforeDraw)
    fn on_once_before_draw(&mut self) {
        let Some(data) = get_resource_as_data("skottie/skottie_sample_multiframe.json") else {
            return;
        };
        let mut builder = Builder::new();
        builder.set_resource_provider(Rc::new(MultiFrameResourceProvider));
        self.animation = builder.make(&data);
        if let Some(animation) = &self.animation {
            animation.seek(0.0);
        }
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L201 (onDraw)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(animation) = &self.animation else {
            "No animation".clone_into(error_msg);
            return DrawResult::Fail;
        };
        let dest = Rect::from_wh(SIZE_SCALAR, SIZE_SCALAR);
        animation.render(canvas, Some(&dest));
        DrawResult::Ok
    }
}

crate::def_gm!(SkottieMultiFrameGM, SkottieMultiFrameGm::new());

// Port of: modules/skottie/gm/SkottieGM.cpp#L96 (SkottieColorizeGM, without `onChar` and the
// property keys it uses)
struct SkottieColorizeGm {
    name: &'static str,
    resource: &'static str,
    animation: Option<Animation>,
}

impl SkottieColorizeGm {
    fn new(name: &'static str, resource: &'static str) -> Self {
        Self {
            name,
            resource,
            animation: None,
        }
    }
}

impl GM for SkottieColorizeGm {
    // Port of: modules/skottie/gm/SkottieGM.cpp#L104 (getName)
    fn name(&self) -> String {
        format!("skottie_colorize_{}", self.name)
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L106 (getISize)
    fn size(&mut self) -> ISize {
        ISize::new(SIZE, SIZE)
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L108 (onOnceBeforeDraw)
    fn on_once_before_draw(&mut self) {
        let Some(data) = get_resource_as_data(self.resource) else {
            return;
        };
        // The manager only collects the handles for `onChar`, which the GM trait has no hook
        // for, so it is not kept: the observer is attached to the builder as in C++.
        let manager = CustomPropertyManager::default();
        let mut builder = Builder::new();
        builder
            .set_font_manager(test_font_mgr())
            .set_property_observer(manager.get_property_observer())
            .set_text_shaping_factory(best_available());
        self.animation = builder.make(&data);
    }

    // Port of: modules/skottie/gm/SkottieGM.cpp#L121 (onDraw)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(animation) = &self.animation else {
            "No animation".clone_into(error_msg);
            return DrawResult::Fail;
        };
        let dest = Rect::from_wh(SIZE_SCALAR, SIZE_SCALAR);
        animation.render(canvas, Some(&dest));
        DrawResult::Ok
    }
}

crate::def_gm!(
    SkottieColorizeGM_color =
        "SkottieColorizeGM(\"color\", \"skottie/skottie_sample_search.json\")",
    SkottieColorizeGm::new("color", "skottie/skottie_sample_search.json")
);
crate::def_gm!(
    SkottieColorizeGM_text =
        "SkottieColorizeGM(\"text\" , \"skottie/skottie-text-animator-5.json\")",
    SkottieColorizeGm::new("text", "skottie/skottie-text-animator-5.json")
);

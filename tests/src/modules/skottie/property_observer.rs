// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/PropertyObserver.cpp (chrome/m156)

#![cfg(test)]

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_mgr::{FontMgr, FontMgrBase, FontStyleSet};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::paint::Join;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::Scalar;
use skia_rust_core::stream::{MemoryStream, StreamAsset};
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;
use skia_rust_core::utils::text_utils::Align;
use skia_rust_skottie::skottie_property::{LazyHandle, TextPaintOrder};
use skia_rust_skottie::text_shaper::{
    Capitalization, Direction, LinebreakPolicy, ResizePolicy, VAlign,
};
use skia_rust_skottie::{
    Builder, ColorPropertyHandle, ColorPropertyValue, NodeType, OpacityPropertyHandle,
    OpacityPropertyValue, PropertyObserver, TextPropertyHandle, TextPropertyValue,
    TransformPropertyHandle, TransformPropertyValue,
};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

use crate::{def_test, reporter_assert};

// Port of: modules/skottie/tests/PropertyObserver.cpp#L22-L29 (chrome/m156)
fn make_text_prop(str_: &str) -> TextPropertyValue {
    TextPropertyValue {
        typeface: Some(default_portable_typeface()),
        text: str_.to_owned(),
        ..TextPropertyValue::default()
    }
}

// Port of: modules/skottie/tests/PropertyObserver.cpp#L31-L141 (chrome/m156)
// (`MockPropertyObserver`)
struct ColorInfo {
    node_name: String,
    handle: Box<ColorPropertyHandle>,
}

struct OpacityInfo {
    node_name: String,
    handle: Box<OpacityPropertyHandle>,
}

struct TextInfo {
    node_name: String,
    handle: Box<TextPropertyHandle>,
}

struct TransformInfo {
    node_name: String,
    handle: Box<TransformPropertyHandle>,
}

struct MockPropertyObserver {
    override_props: bool,
    colors: RefCell<Vec<ColorInfo>>,
    opacities: RefCell<Vec<OpacityInfo>>,
    texts: RefCell<Vec<TextInfo>>,
    transforms: RefCell<Vec<TransformInfo>>,
    current_node: RefCell<String>,
    colors_with_full_keypath: RefCell<Vec<ColorInfo>>,
}

impl MockPropertyObserver {
    fn new(override_props: bool) -> Self {
        Self {
            override_props,
            colors: RefCell::new(Vec::new()),
            opacities: RefCell::new(Vec::new()),
            texts: RefCell::new(Vec::new()),
            transforms: RefCell::new(Vec::new()),
            current_node: RefCell::new(String::new()),
            colors_with_full_keypath: RefCell::new(Vec::new()),
        }
    }
}

impl PropertyObserver for MockPropertyObserver {
    fn on_color_property(&self, node_name: Option<&str>, lh: LazyHandle<'_, ColorPropertyHandle>) {
        let prop_handle = lh();
        if self.override_props {
            const OVERRIDE_COLOR: ColorPropertyValue = Color::new(0xffff_ff00);
            prop_handle.set(OVERRIDE_COLOR);
        }
        self.colors.borrow_mut().push(ColorInfo {
            node_name: node_name.unwrap_or_default().to_owned(),
            handle: prop_handle,
        });
        self.colors_with_full_keypath.borrow_mut().push(ColorInfo {
            node_name: self.current_node.borrow().clone(),
            handle: lh(),
        });
    }

    fn on_opacity_property(
        &self,
        node_name: Option<&str>,
        lh: LazyHandle<'_, OpacityPropertyHandle>,
    ) {
        let prop_handle = lh();
        if self.override_props {
            const OVERRIDE_OPACITY: OpacityPropertyValue = 0.75;
            prop_handle.set(OVERRIDE_OPACITY);
        }
        self.opacities.borrow_mut().push(OpacityInfo {
            node_name: node_name.unwrap_or_default().to_owned(),
            handle: prop_handle,
        });
    }

    fn on_text_property(&self, node_name: Option<&str>, lh: LazyHandle<'_, TextPropertyHandle>) {
        let prop_handle = lh();
        if self.override_props {
            let override_text = make_text_prop("foo");
            prop_handle.set(&override_text);
        }
        self.texts.borrow_mut().push(TextInfo {
            node_name: node_name.unwrap_or_default().to_owned(),
            handle: prop_handle,
        });
    }

    fn on_transform_property(
        &self,
        node_name: Option<&str>,
        lh: LazyHandle<'_, TransformPropertyHandle>,
    ) {
        let prop_handle = lh();
        if self.override_props {
            let override_transform = TransformPropertyValue {
                anchor_point: Point::new(100.0, 100.0),
                position: Point::new(200.0, 200.0),
                scale: Vector::new(2.0, 2.0),
                rotation: 45.0,
                skew: 0.0,
                skew_axis: 0.0,
            };
            prop_handle.set(&override_transform);
        }
        self.transforms.borrow_mut().push(TransformInfo {
            node_name: node_name.unwrap_or_default().to_owned(),
            handle: prop_handle,
        });
    }

    fn on_enter_node(&self, node_name: Option<&str>, _node_type: NodeType) {
        let Some(node_name) = node_name else {
            return;
        };
        let mut current = self.current_node.borrow_mut();
        *current = if current.is_empty() {
            node_name.to_owned()
        } else {
            format!("{current}.{node_name}")
        };
    }

    fn on_leaving_node(&self, node_name: Option<&str>, _node_type: NodeType) {
        let Some(node_name) = node_name else {
            return;
        };
        let length = node_name.len();
        let mut current = self.current_node.borrow_mut();
        *current = if current.len() > length {
            current[..current.len() - node_name.len() - 1].to_owned()
        } else {
            String::new()
        };
    }
}

// Returns a single specified typeface for all requests.
// Port of: modules/skottie/tests/PropertyObserver.cpp#L143-L175 (chrome/m156) (`MockFontMgr`)
#[derive(Debug)]
struct MockFontMgr {
    test_font: Typeface,
}

impl FontMgrBase for MockFontMgr {
    fn on_count_families(&self) -> usize {
        1
    }
    fn on_get_family_name(&self, _index: usize) -> String {
        String::new()
    }
    fn on_create_style_set(&self, _index: usize) -> Option<FontStyleSet> {
        None
    }
    fn on_match_family(&self, _family_name: Option<&str>) -> Option<FontStyleSet> {
        None
    }
    fn on_match_family_style(
        &self,
        _family_name: Option<&str>,
        _font_style: &FontStyle,
    ) -> Option<Typeface> {
        None
    }
    fn on_match_family_style_character(
        &self,
        _family_name: Option<&str>,
        _style: &FontStyle,
        _bcp47: &[&str],
        _character: Unichar,
    ) -> Option<Typeface> {
        None
    }
    fn on_make_from_data(&self, _data: &Data, _tt_index: i32) -> Option<Typeface> {
        Some(self.test_font.clone())
    }
    fn on_make_from_stream_index(
        &self,
        _stream: Box<dyn StreamAsset>,
        _tt_index: i32,
    ) -> Option<Typeface> {
        Some(self.test_font.clone())
    }
    fn on_make_from_stream_args(
        &self,
        _stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        Some(self.test_font.clone())
    }
    fn on_make_from_file(&self, _path: &str, _tt_index: i32) -> Option<Typeface> {
        Some(self.test_font.clone())
    }
    fn on_legacy_make_typeface(
        &self,
        _family_name: Option<&str>,
        _style: FontStyle,
    ) -> Option<Typeface> {
        Some(self.test_font.clone())
    }
}

// Port of: modules/skottie/tests/PropertyObserver.cpp#L177-L268 (chrome/m156)
const TEST_JSON: &str = r#"{
                                 "v": "5.2.1",
                                 "w": 100,
                                 "h": 100,
                                 "fr": 1,
                                 "ip": 0,
                                 "op": 1,
                                 "fonts": {
                                   "list": [
                                     {
                                       "fName": "test_font",
                                       "fFamily": "test-family",
                                       "fStyle": "TestFontStyle"
                                     }
                                   ]
                                 },
                                 "layers": [
                                   {
                                     "ty": 4,
                                     "nm": "layer_0",
                                     "ind": 0,
                                     "ip": 0,
                                     "op": 1,
                                     "ks": {
                                       "o": { "a": 0, "k": 50 }
                                     },
                                     "ef": [{
                                       "ef": [
                                         {},
                                         {},
                                         { "v": { "a": 0, "k": [ 0, 1, 0 ] }},
                                         {},
                                         {},
                                         {},
                                         { "v": { "a": 0, "k": 1 }}
                                       ],
                                       "nm": "fill_effect_0",
                                       "mn": "ADBE Fill",
                                       "ty": 21
                                     }],
                                     "shapes": [
                                       {
                                         "ty": "el",
                                         "nm": "geometry_0",
                                         "p": { "a": 0, "k": [ 50, 50 ] },
                                         "s": { "a": 0, "k": [ 50, 50 ] }
                                       },
                                       {
                                         "ty": "fl",
                                         "nm": "fill_0",
                                         "c": { "a": 0, "k": [ 1, 0, 0] }
                                       },
                                       {
                                         "ty": "tr",
                                         "nm": "shape_transform_0",
                                         "o": { "a": 0, "k": 100 },
                                         "s": { "a": 0, "k": [ 50, 50 ] }
                                       }
                                     ]
                                   },
                                   {
                                     "ty": 5,
                                     "nm": "layer_1",
                                     "ip": 0,
                                     "op": 1,
                                     "ks": {
                                       "p": { "a": 0, "k": [25, 25] }
                                     },
                                     "t": {
                                       "d": {
                                         "k": [
                                            {
                                              "t": 0,
                                              "s": {
                                                "f": "test_font",
                                                "s": 100,
                                                "t": "inline_text",
                                                "lh": 120,
                                                "ls": 12
                                              }
                                            }
                                         ]
                                       }
                                     }
                                   }
                                 ]
                               }"#;

// Port of: modules/skottie/tests/PropertyObserver.cpp#L270-L370 (chrome/m156)
def_test!(Skottie_Props, |reporter| {
    let test_typeface = default_portable_typeface();
    // REPORTER_ASSERT(reporter, test_typeface): skia-rust: not expressible in Rust: a typeface
    // is never null.
    let test_font_manager = FontMgr::new(std::sync::Arc::new(MockFontMgr {
        test_font: test_typeface.clone(),
    }));

    let mut stream = MemoryStream::make_copy(TEST_JSON.as_bytes());
    let observer = Rc::new(MockPropertyObserver::new(false));

    let animation = Builder::new()
        .set_property_observer(observer.clone())
        .set_font_manager(test_font_manager)
        .make_from_stream(&mut *stream);

    reporter_assert!(reporter, animation.is_some());

    let colors = observer.colors.borrow();
    reporter_assert!(reporter, colors.len() == 2);
    reporter_assert!(reporter, colors[0].node_name == "fill_0");
    reporter_assert!(reporter, colors[0].handle.get() == Color::new(0xffff_0000));
    reporter_assert!(reporter, colors[1].node_name == "fill_effect_0");
    reporter_assert!(reporter, colors[1].handle.get() == Color::new(0xff00_ff00));

    let colors_with_full_keypath = observer.colors_with_full_keypath.borrow();
    reporter_assert!(reporter, colors_with_full_keypath.len() == 2);
    reporter_assert!(
        reporter,
        colors_with_full_keypath[0].node_name == "layer_0.fill_0"
    );
    reporter_assert!(
        reporter,
        colors_with_full_keypath[0].handle.get() == Color::new(0xffff_0000)
    );
    reporter_assert!(
        reporter,
        colors_with_full_keypath[1].node_name == "layer_0.fill_effect_0"
    );
    reporter_assert!(
        reporter,
        colors_with_full_keypath[1].handle.get() == Color::new(0xff00_ff00)
    );

    let opacities = observer.opacities.borrow();
    reporter_assert!(reporter, opacities.len() == 3);
    reporter_assert!(reporter, opacities[0].node_name == "shape_transform_0");
    reporter_assert!(
        reporter,
        f32::nearly_equal(opacities[0].handle.get(), 100.0, None)
    );
    reporter_assert!(reporter, opacities[1].node_name == "layer_0");
    reporter_assert!(
        reporter,
        f32::nearly_equal(opacities[1].handle.get(), 50.0, None)
    );

    let transforms = observer.transforms.borrow();
    reporter_assert!(reporter, transforms.len() == 3);
    reporter_assert!(reporter, transforms[0].node_name == "layer_0");
    reporter_assert!(
        reporter,
        transforms[0].handle.get()
            == TransformPropertyValue {
                anchor_point: Point::new(0.0, 0.0),
                position: Point::new(0.0, 0.0),
                scale: Vector::new(100.0, 100.0),
                rotation: 0.0,
                skew: 0.0,
                skew_axis: 0.0,
            }
    );
    reporter_assert!(reporter, transforms[1].node_name == "layer_1");
    reporter_assert!(
        reporter,
        transforms[1].handle.get()
            == TransformPropertyValue {
                anchor_point: Point::new(0.0, 0.0),
                position: Point::new(25.0, 25.0),
                scale: Vector::new(100.0, 100.0),
                rotation: 0.0,
                skew: 0.0,
                skew_axis: 0.0,
            }
    );
    reporter_assert!(reporter, transforms[2].node_name == "shape_transform_0");
    reporter_assert!(
        reporter,
        transforms[2].handle.get()
            == TransformPropertyValue {
                anchor_point: Point::new(0.0, 0.0),
                position: Point::new(0.0, 0.0),
                scale: Vector::new(50.0, 50.0),
                rotation: 0.0,
                skew: 0.0,
                skew_axis: 0.0,
            }
    );

    let texts = observer.texts.borrow();
    reporter_assert!(reporter, texts.len() == 1);
    reporter_assert!(reporter, texts[0].node_name == "layer_1");
    let mut text_prop = TextPropertyValue {
        typeface: Some(test_typeface),
        text: "inline_text".to_owned(),
        text_size: 100.0,
        min_text_size: 0.0,
        max_text_size: 100.0,
        stroke_width: 0.0,
        line_height: 120.0,
        line_shift: 12.0,
        ascent: 0.0,
        max_lines: 0,
        h_align: Align::Left,
        v_align: VAlign::TopBaseline,
        resize: ResizePolicy::None,
        line_break: LinebreakPolicy::Explicit,
        direction: Direction::LTR,
        capitalization: Capitalization::None,
        box_: Rect::new_empty(),
        fill_color: Color::TRANSPARENT,
        stroke_color: Color::TRANSPARENT,
        paint_order: TextPaintOrder::FillStroke,
        stroke_join: Join::Miter,
        has_fill: false,
        has_stroke: false,
        decorator: None,
        locale: String::new(),
        font_family: "test-family".to_owned(),
        text_tracking: 0.0,
    };
    reporter_assert!(reporter, texts[0].handle.get() == text_prop);
    "custom_lc".clone_into(&mut text_prop.locale);
    texts[0].handle.set(&text_prop);
    reporter_assert!(reporter, texts[0].handle.get() == text_prop);
});

// Port of: modules/skottie/tests/PropertyObserver.cpp#L372-L414 (chrome/m156)
def_test!(Skottie_Props_Revalidation, |reporter| {
    let test_typeface = default_portable_typeface();
    // REPORTER_ASSERT(reporter, test_typeface): skia-rust: not expressible in Rust: a typeface
    // is never null.
    let test_font_manager = FontMgr::new(std::sync::Arc::new(MockFontMgr {
        test_font: test_typeface,
    }));

    let mut stream = MemoryStream::make_copy(TEST_JSON.as_bytes());
    let observer = Rc::new(MockPropertyObserver::new(true));

    let animation = Builder::new()
        .set_property_observer(observer.clone())
        .set_font_manager(test_font_manager)
        .make_from_stream(&mut *stream);

    reporter_assert!(reporter, animation.is_some());
    let Some(animation) = animation else {
        return;
    };

    let mut recorder = PictureRecorder::new();
    // Rendering without seek() should not trigger revalidation asserts.
    animation.render(
        recorder.begin_recording(Rect::from_wh(100.0, 100.0), false),
        None,
    );

    // Mutate all props.
    for c in observer.colors.borrow().iter() {
        c.handle.set(Color::GRAY);
    }
    for o in observer.opacities.borrow().iter() {
        o.handle.set(0.25);
    }
    for t in observer.texts.borrow().iter() {
        t.handle.set(&make_text_prop("bar"));
    }
    for t in observer.transforms.borrow().iter() {
        t.handle.set(&TransformPropertyValue {
            anchor_point: Point::new(1000.0, 1000.0),
            position: Point::new(2000.0, 2000.0),
            scale: Vector::new(20.0, 20.0),
            rotation: 5.0,
            skew: 0.0,
            skew_axis: 0.0,
        });
    }

    // Rendering without seek() after property mutation should not trigger revalidation asserts.
    animation.render(
        recorder.begin_recording(Rect::from_wh(100.0, 100.0), false),
        None,
    );
});

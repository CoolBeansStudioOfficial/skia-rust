// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/Text.cpp (chrome/m156)

#![cfg(test)]
// The local classes of the tests stay local, as in C++.
#![allow(clippy::items_after_statements, clippy::similar_names)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use skia_rust_core::data::Data;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_mgr::{FontMgr, FontMgrBase, FontStyleSet};
use skia_rust_core::font_style::{FontStyle, Slant, Weight};
use skia_rust_core::stream::{MemoryStream, StreamAsset};
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;
use skia_rust_resources::ResourceProvider;
use skia_rust_shaper::best_available;
use skia_rust_skottie::skottie_property::LazyHandle;
use skia_rust_skottie::{Builder, Logger, LoggerLevel, PropertyObserver, TextPropertyHandle};
use skia_rust_tools::font_tool_utils::create_portable_typeface;

use crate::{def_test, reporter_assert};

#[derive(Debug, Default)]
struct RecordMatchFamilyStyleFontMgr {
    style_requested_when_matching_family: Mutex<HashMap<String, FontStyle>>,
}

impl RecordMatchFamilyStyleFontMgr {
    fn style_requested_when_matching_family(&self, family: &str) -> Option<FontStyle> {
        self.style_requested_when_matching_family
            .lock()
            .unwrap()
            .get(family)
            .copied()
    }
}

impl FontMgrBase for RecordMatchFamilyStyleFontMgr {
    fn on_count_families(&self) -> usize {
        0
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

    fn on_match_family_style(&self, family: Option<&str>, style: &FontStyle) -> Option<Typeface> {
        // `fStyleRequestedWhenMatchingFamily[family] = style`; a null family is undefined
        // behavior in C++ (`std::string(nullptr)`).
        self.style_requested_when_matching_family
            .lock()
            .unwrap()
            .insert(family.expect("a family name").to_owned(), *style);
        None
    }
    fn on_match_family_style_character(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
        _bcp47: &[&str],
        _character: Unichar,
    ) -> Option<Typeface> {
        self.style_requested_when_matching_family
            .lock()
            .unwrap()
            .insert(family_name.expect("a family name").to_owned(), *style);
        None
    }

    fn on_make_from_data(&self, _data: &Data, _tt_index: i32) -> Option<Typeface> {
        None
    }
    fn on_make_from_stream_index(
        &self,
        _stream: Box<dyn StreamAsset>,
        _tt_index: i32,
    ) -> Option<Typeface> {
        None
    }
    fn on_make_from_stream_args(
        &self,
        _stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        None
    }
    fn on_make_from_file(&self, _path: &str, _tt_index: i32) -> Option<Typeface> {
        None
    }

    fn on_legacy_make_typeface(
        &self,
        _family_name: Option<&str>,
        _style: FontStyle,
    ) -> Option<Typeface> {
        None
    }
}

// This test relies on Skottie internals/implementation details, and may need to
// be updated in the future, if Skottie font resolution changes.
// Port of: modules/skottie/tests/Text.cpp#L77-L149 (chrome/m156)
def_test!(Skottie_Text_Style, |r| {
    let json = r#"{
             "v": "5.2.1",
             "w": 100,
             "h": 100,
             "fr": 10,
             "ip": 0,
             "op": 100,
             "fonts": {
               "list": [
                 { "fName"  : "f1", "fFamily": "f1", "fStyle" : "Regular"   },
                 { "fName"  : "f2", "fFamily": "f2", "fStyle" : "Medium"    },
                 { "fName"  : "f3", "fFamily": "f3", "fStyle" : "Bold"      },
                 { "fName"  : "f4", "fFamily": "f4", "fStyle" : "Light"     },
                 { "fName"  : "f5", "fFamily": "f5", "fStyle" : "Extra"     },
                 { "fName"  : "f6", "fFamily": "f6", "fStyle" : "ExtraBold" },

                 { "fName"  : "f7" , "fFamily": "f7" , "fStyle" : "Regular Italic"    },
                 { "fName"  : "f8" , "fFamily": "f8" , "fStyle" : "Medium Italic"     },
                 { "fName"  : "f9" , "fFamily": "f9" , "fStyle" : "Bold Italic"       },
                 { "fName"  : "f10", "fFamily": "f10", "fStyle" : "Light Oblique"     },
                 { "fName"  : "f11", "fFamily": "f11", "fStyle" : "Extra Oblique"     },
                 { "fName"  : "f12", "fFamily": "f12", "fStyle" : "Extrabold Oblique" },

                 { "fName"  : "f13", "fFamily": "f13", "fStyle" : "Italic"  },
                 { "fName"  : "f14", "fFamily": "f14", "fStyle" : "Oblique" },
                 { "fName"  : "f15", "fFamily": "f15", "fStyle" : ""        }
               ]
             }
           }"#;

    let mut stream = MemoryStream::make_copy(json.as_bytes());
    let recorder = Arc::new(RecordMatchFamilyStyleFontMgr::default());
    let fmgr = FontMgr::new(recorder.clone());

    let anim = Builder::new()
        .set_font_manager(fmgr)
        .set_text_shaping_factory(best_available())
        .make_from_stream(&mut *stream);

    reporter_assert!(r, anim.is_some());

    struct Expected {
        family: &'static str,
        weight: Weight,
        slant: Slant,
    }
    let expected = [
        Expected {
            family: "f1",
            weight: Weight::NORMAL,
            slant: Slant::Upright,
        },
        Expected {
            family: "f2",
            weight: Weight::MEDIUM,
            slant: Slant::Upright,
        },
        Expected {
            family: "f3",
            weight: Weight::BOLD,
            slant: Slant::Upright,
        },
        Expected {
            family: "f4",
            weight: Weight::LIGHT,
            slant: Slant::Upright,
        },
        Expected {
            family: "f5",
            weight: Weight::EXTRA_BOLD,
            slant: Slant::Upright,
        },
        Expected {
            family: "f6",
            weight: Weight::EXTRA_BOLD,
            slant: Slant::Upright,
        },
        Expected {
            family: "f7",
            weight: Weight::NORMAL,
            slant: Slant::Italic,
        },
        Expected {
            family: "f8",
            weight: Weight::MEDIUM,
            slant: Slant::Italic,
        },
        Expected {
            family: "f9",
            weight: Weight::BOLD,
            slant: Slant::Italic,
        },
        Expected {
            family: "f10",
            weight: Weight::LIGHT,
            slant: Slant::Oblique,
        },
        Expected {
            family: "f11",
            weight: Weight::EXTRA_BOLD,
            slant: Slant::Oblique,
        },
        Expected {
            family: "f12",
            weight: Weight::EXTRA_BOLD,
            slant: Slant::Oblique,
        },
        Expected {
            family: "f13",
            weight: Weight::NORMAL,
            slant: Slant::Italic,
        },
        Expected {
            family: "f14",
            weight: Weight::NORMAL,
            slant: Slant::Oblique,
        },
        Expected {
            family: "f15",
            weight: Weight::NORMAL,
            slant: Slant::Upright,
        },
    ];

    for exp in &expected {
        let style = recorder.style_requested_when_matching_family(exp.family);
        reporter_assert!(r, style.is_some());
        if let Some(style) = style {
            reporter_assert!(r, style.weight() == exp.weight);
            reporter_assert!(r, style.slant() == exp.slant);
        }
    }
});

// Port of: modules/skottie/tests/Text.cpp#L151-L229 (chrome/m156)
def_test!(Skottie_Text_LayoutError, |r| {
    // Text node properties:
    //   - scale to fit
    //   - box width: 100
    //   - min font size: 70
    //   - string: Foo Bar Baz
    //
    // Layout should fail with these unsatisfiable constraints.
    let json = r#"{
             "v": "5.2.1",
             "w": 100,
             "h": 100,
             "fr": 10,
             "ip": 0,
             "op": 100,
             "fonts": {
               "list": [{
                 "fFamily": "Arial",
                 "fName": "Arial",
                 "fStyle": "Bold"
               }]
             },
             "layers": [{
               "ty": 5,
               "t": {
                 "d": {
                   "k": [{
                     "t": 0,
                     "s": {
                       "f": "Arial",
                       "t": "Foo Bar Baz",
                       "s": 24,
                       "fc": [1,1,1,1],
                       "lh": 70,
                       "ps": [0, 0],
                       "sz": [100, 100],
                       "mf": 70,
                       "rs": 1
                     }
                   }]
                 }
               }
             }]
           }"#;

    #[derive(Default)]
    struct TestLogger {
        errors: RefCell<Vec<String>>,
    }

    impl TestLogger {
        fn errors(&self) -> Vec<String> {
            self.errors.borrow().clone()
        }
    }

    impl Logger for TestLogger {
        fn log(&self, lvl: LoggerLevel, message: &str, _json: Option<&str>) {
            if lvl == LoggerLevel::Error {
                self.errors.borrow_mut().push(message.to_owned());
            }
        }
    }

    struct PortableRp;

    impl ResourceProvider for PortableRp {
        fn load_typeface(&self, _name: &str, _url: &str) -> Option<Typeface> {
            Some(create_portable_typeface(
                Some("Serif"),
                FontStyle::default(),
            ))
        }
    }

    let mut stream = MemoryStream::make_copy(json.as_bytes());
    let logger = Rc::new(TestLogger::default());

    let anim = Builder::new()
        .set_logger(logger.clone())
        .set_resource_provider(Rc::new(PortableRp))
        .make_from_stream(&mut *stream);

    reporter_assert!(r, anim.is_some());
    reporter_assert!(r, logger.errors().len() == 1);
    reporter_assert!(
        r,
        logger
            .errors()
            .first()
            .is_some_and(|error| error.starts_with("Text layout failed"))
    );
});

// Port of: modules/skottie/tests/Text.cpp#L231-L307 (chrome/m156)
def_test!(Skottie_Text_FontFamily, |r| {
    let json = r#"{
             "v": "5.2.1",
             "w": 100,
             "h": 100,
             "fr": 10,
             "ip": 0,
             "op": 100,
             "fonts": {
               "list": [{
                 "fFamily": "family_1",
                 "fName": "Font1",
                 "fStyle": "Bold"
               }]
             },
             "layers": [{
               "ty": 5,
               "t": {
                 "d": {
                   "k": [{
                     "t": 0,
                     "s": {
                       "f": "Font1",
                       "t": "Foo Bar Baz",
                       "s": 24,
                       "fc": [1,1,1,1],
                       "lh": 70,
                       "ps": [0, 0],
                       "sz": [100, 100],
                       "mf": 70,
                       "rs": 0
                     }
                   }]
                 }
               }
             }]
           }"#;

    #[derive(Default)]
    struct TextObserver {
        text: RefCell<Option<Box<TextPropertyHandle>>>,
    }

    impl PropertyObserver for TextObserver {
        fn on_text_property(
            &self,
            _node_name: Option<&str>,
            lh: LazyHandle<'_, TextPropertyHandle>,
        ) {
            debug_assert!(self.text.borrow().is_none()); // only one text prop in test
            *self.text.borrow_mut() = Some(lh());
        }
    }

    let recorder = Arc::new(RecordMatchFamilyStyleFontMgr::default());
    let fmgr = FontMgr::new(recorder.clone());
    let prop_observer = Rc::new(TextObserver::default());

    let mut stream = MemoryStream::make_copy(json.as_bytes());
    let anim = Builder::new()
        .set_font_manager(fmgr)
        .set_property_observer(prop_observer.clone())
        .set_text_shaping_factory(best_available())
        .make_from_stream(&mut *stream);
    reporter_assert!(r, anim.is_some());

    // Original family name was passed to fontmgr.
    let style1 = recorder.style_requested_when_matching_family("family_1");
    reporter_assert!(r, style1.is_some());

    let text = prop_observer.text.borrow();
    reporter_assert!(r, text.is_some());
    let Some(text_handle) = text.as_ref() else {
        return;
    };
    let mut txt = text_handle.get();
    txt.font_family = "family_2".to_owned();
    text_handle.set(&txt);

    // Updated family name was passed to fontmgr.
    let style2 = recorder.style_requested_when_matching_family("family_2");
    reporter_assert!(r, style2.is_some());
});

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/Expression.cpp (chrome/m156)
//
// `Skottie_ExpressionText` needs the text layers of M22, and is not ported yet.

#![cfg(test)]

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_core::color::Color4f;
use skia_rust_core::scalar::Scalar;
use skia_rust_core::stream::MemoryStream;
use skia_rust_skottie::skottie_property::LazyHandle;
use skia_rust_skottie::{
    Builder, ColorPropertyHandle, ExpressionEvaluator, ExpressionManager, OpacityPropertyHandle,
    PropertyObserver, TransformPropertyHandle,
};

use crate::{def_test, reporter_assert};

struct FakeScalarExpressionEvaluator;

impl ExpressionEvaluator<f32> for FakeScalarExpressionEvaluator {
    fn evaluate(&self, _t: f32) -> f32 {
        7.0
    }
}

struct FakeVectorExpressionEvaluator;

impl ExpressionEvaluator<Vec<f32>> for FakeVectorExpressionEvaluator {
    fn evaluate(&self, _t: f32) -> Vec<f32> {
        vec![0.1, 0.2, 0.3, 1.0]
    }
}

struct FakeStringExpressionEvaluator;

impl ExpressionEvaluator<String> for FakeStringExpressionEvaluator {
    fn evaluate(&self, _t: f32) -> String {
        "Hello, world!".to_string()
    }
}

struct FakeExpressionManager;

impl ExpressionManager for FakeExpressionManager {
    fn create_number_expression_evaluator(
        &self,
        _expression: &str,
    ) -> Option<Rc<dyn ExpressionEvaluator<f32>>> {
        Some(Rc::new(FakeScalarExpressionEvaluator))
    }

    fn create_string_expression_evaluator(
        &self,
        _expression: &str,
    ) -> Option<Rc<dyn ExpressionEvaluator<String>>> {
        Some(Rc::new(FakeStringExpressionEvaluator))
    }

    fn create_array_expression_evaluator(
        &self,
        _expression: &str,
    ) -> Option<Rc<dyn ExpressionEvaluator<Vec<f32>>>> {
        Some(Rc::new(FakeVectorExpressionEvaluator))
    }
}

#[derive(Default)]
struct FakePropertyObserver {
    opacity: RefCell<Option<Box<OpacityPropertyHandle>>>,
    transform: RefCell<Option<Box<TransformPropertyHandle>>>,
    color: RefCell<Option<Box<ColorPropertyHandle>>>,
}

impl PropertyObserver for FakePropertyObserver {
    fn on_opacity_property(
        &self,
        _node_name: Option<&str>,
        opacity_handle: LazyHandle<'_, OpacityPropertyHandle>,
    ) {
        *self.opacity.borrow_mut() = Some(opacity_handle());
    }

    fn on_transform_property(
        &self,
        _node_name: Option<&str>,
        transform_handle: LazyHandle<'_, TransformPropertyHandle>,
    ) {
        *self.transform.borrow_mut() = Some(transform_handle());
    }

    fn on_color_property(
        &self,
        _node_name: Option<&str>,
        color_handle: LazyHandle<'_, ColorPropertyHandle>,
    ) {
        *self.color.borrow_mut() = Some(color_handle());
    }
}

// Port of: modules/skottie/tests/Expression.cpp#L82-L267 (chrome/m156)
def_test!(Skottie_Expression, |r| {
    let json = r##"{
             "v": "5.2.1",
             "w": 100,
             "h": 100,
             "fr": 10,
             "ip": 0,
             "op": 100,
             "layers": [
               {
                 "ip": 0,
                 "op": 100,
                 "ty": 1,
                 "nm": "My Layer",
                 "sr": 1,
                 "ks": {
                   "o": {
                     "a": 0,
                     "k": 100,
                     "ix": 11,
                     "x": "fake; return value is specified by the FakeScalarExpressionEvaluator."
                   },
                    "r": {
                        "a": 0,
                        "k": 0,
                        "ix": 10
                    },
                    "p": {
                        "a": 0,
                        "k": [
                            50,
                            50,
                            0
                        ],
                        "ix": 2,
                        "l": 2
                    },
                    "a": {
                        "a": 0,
                        "k": [
                            50,
                            50,
                            0
                        ],
                        "ix": 1,
                        "l": 2,
                        "x": "fake; return value is specified by the FakeArrayExpressionEvaluator."
                    },
                    "s": {
                        "a": 0,
                        "k": [
                            100,
                            100,
                            100
                        ],
                        "ix": 6,
                        "l": 2
                    }
                },
                "ef": [
                {
                    "ty": 21,
                    "nm": "Fill",
                    "np": 9,
                    "mn": "ADBE Fill",
                    "ix": 1,
                    "en": 1,
                    "ef": [
                        {
                            "ty": 10,
                            "nm": "Fill Mask",
                            "mn": "ADBE Fill-0001",
                            "ix": 1,
                            "v": {
                                "a": 0,
                                "k": 0,
                                "ix": 1
                            }
                        },
                        {
                            "ty": 7,
                            "nm": "All Masks",
                            "mn": "ADBE Fill-0007",
                            "ix": 2,
                            "v": {
                                "a": 0,
                                "k": 0,
                                "ix": 2
                            }
                        },
                        {
                            "ty": 2,
                            "nm": "Color",
                            "mn": "ADBE Fill-0002",
                            "ix": 3,
                            "v": {
                                "a": 0,
                                "k": [
                                    1,
                                    0,
                                    0,
                                    1
                                ],
                                "ix": 3,
                                "x": "fake; return value is specified by the FakeArrayExpressionEvaluator."
                            }
                        },
                        {
                            "ty": 7,
                            "nm": "Invert",
                            "mn": "ADBE Fill-0006",
                            "ix": 4,
                            "v": {
                                "a": 0,
                                "k": 0,
                                "ix": 4
                            }
                        },
                        {
                            "ty": 0,
                            "nm": "Horizontal Feather",
                            "mn": "ADBE Fill-0003",
                            "ix": 5,
                            "v": {
                                "a": 0,
                                "k": 0,
                                "ix": 5
                            }
                        },
                        {
                            "ty": 0,
                            "nm": "Vertical Feather",
                            "mn": "ADBE Fill-0004",
                            "ix": 6,
                            "v": {
                                "a": 0,
                                "k": 0,
                                "ix": 6
                            }
                        },
                        {
                            "ty": 0,
                            "nm": "Opacity",
                            "mn": "ADBE Fill-0005",
                            "ix": 7,
                            "v": {
                                "a": 0,
                                "k": 1,
                                "ix": 7
                            }
                        }
                    ]
                }
                ],
                "ao": 0,
                "sw": 100,
                "sh": 100,
                "sc": "#000000",
                "st": 0,
                "bm": 0
               }
             ]
           }"##;

    let mut stream = MemoryStream::make_copy(json.as_bytes());

    let em = Rc::new(FakeExpressionManager);
    let observer = Rc::new(FakePropertyObserver::default());

    let anim = Builder::new()
        .set_expression_manager(em)
        .set_property_observer(observer.clone())
        .make_from_stream(&mut *stream);

    reporter_assert!(r, anim.is_some());
    let Some(anim) = anim else {
        return;
    };

    anim.seek_frame_time(0.0);

    reporter_assert!(
        r,
        f32::nearly_equal(
            observer
                .opacity
                .borrow()
                .as_ref()
                .expect("an opacity handle")
                .get(),
            7.0,
            None
        )
    );
    let anchor_point = observer
        .transform
        .borrow()
        .as_ref()
        .expect("a transform handle")
        .get()
        .anchor_point;
    reporter_assert!(r, f32::nearly_equal(anchor_point.x, 0.1, None));
    reporter_assert!(r, f32::nearly_equal(anchor_point.y, 0.2, None));
    reporter_assert!(
        r,
        observer
            .color
            .borrow()
            .as_ref()
            .expect("a color handle")
            .get()
            == Color4f {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0
            }
            .to_color()
    );
});

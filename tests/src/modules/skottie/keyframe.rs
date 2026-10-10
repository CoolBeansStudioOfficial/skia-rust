// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/Keyframe.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::neg_cmp_op_on_partial_ord)] // REPORTER_ASSERT negates the condition
#![allow(clippy::float_cmp)] // the C++ compares the scalars with ==

use std::rc::{Rc, Weak};

use skia_rust_core::scalar::Scalar;
use skia_rust_skottie::internal::animator::{
    AnimatablePropertyContainer, Animator, Bindable, Prop, PropertyContainer,
};
use skia_rust_skottie::internal::skottie_priv::AnimationBuilder;
use skia_rust_skottie::json::DOM;
use skia_rust_skottie::{BuilderFlags, ScalarValue, Vec2Value};

use crate::{def_test, reporter_assert};

/// Port of `MockProperty<T>`.
// Port of: modules/skottie/tests/Keyframe.cpp#L22-L45 (chrome/m156)
struct MockProperty<T: Bindable + Default + Clone> {
    container: PropertyContainer,
    value: Prop<T>,
    did_bind: bool,
}

impl<T: Bindable + Default + Clone> MockProperty<T> {
    fn new(jprop: &str) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let container = PropertyContainer::new(weak.clone());
            let abuilder = AnimationBuilder::new(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                (100.0, 100.0).into(),
                10.0,
                1.0,
                BuilderFlags::empty(),
            );
            let json_dom = DOM::new(jprop.as_bytes());

            let value = Prop::new(T::default());
            let did_bind = container.bind(&abuilder, json_dom.root(), &value);

            Self {
                container,
                value,
                did_bind,
            }
        })
    }

    /// `explicit operator bool`.
    fn is_bound(&self) -> bool {
        self.did_bind
    }

    /// `operator()(float t)`: seeks, and returns the value.
    fn at(&self, t: f32) -> T {
        self.seek(t);
        self.value.get()
    }
}

impl<T: Bindable + Default + Clone> AnimatablePropertyContainer for MockProperty<T> {
    fn container(&self) -> &PropertyContainer {
        &self.container
    }

    fn on_sync(&self) {}
}

impl<T: Bindable + Default + Clone> Animator for MockProperty<T> {
    fn seek(&self, t: f32) -> bool {
        self.container.seek_container(t, &|| self.on_sync())
    }
}

/// `SkScalarNearlyEqual(a, b)`.
fn nearly_equal(a: f32, b: f32) -> bool {
    f32::nearly_equal(a, b, None)
}

// Port of: modules/skottie/tests/Keyframe.cpp#L47-L227 (chrome/m156)
def_test!(Skottie_Keyframe, |reporter| {
    {
        let prop = MockProperty::<ScalarValue>::new(r"{}");
        reporter_assert!(reporter, !prop.is_bound());
    }
    {
        let prop = MockProperty::<ScalarValue>::new(r#"{ "a": 1, "k": [] }"#);
        reporter_assert!(reporter, !prop.is_bound());
    }
    {
        // New style
        let prop = MockProperty::<ScalarValue>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t":  1, "s": 1 },
                   { "t":  2, "s": 2 },
                   { "t":  3, "s": 4 }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, !prop.is_static());
        reporter_assert!(reporter, nearly_equal(prop.at(-1.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(0.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.5), 1.5));
        reporter_assert!(reporter, nearly_equal(prop.at(2.0), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.5), 3.0));
        reporter_assert!(reporter, nearly_equal(prop.at(3.0), 4.0));
        reporter_assert!(reporter, nearly_equal(prop.at(4.0), 4.0));
    }
    {
        // New style hold (hard stops)
        let prop = MockProperty::<ScalarValue>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t":  1, "s": 1, "h": true },
                   { "t":  2, "s": 2, "h": true },
                   { "t":  3, "s": 4, "h": true }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, !prop.is_static());
        reporter_assert!(reporter, nearly_equal(prop.at(0.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.5), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.0_f32.next_down()), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.0), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.5), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(3.0_f32.next_down()), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(3.0), 4.0));
        reporter_assert!(reporter, nearly_equal(prop.at(4.0), 4.0));
    }
    {
        // Legacy style
        let prop = MockProperty::<ScalarValue>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t":  1, "s": 1, "e": 2 },
                   { "t":  2, "s": 2, "e": 4 },
                   { "t":  3 }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, !prop.is_static());
        reporter_assert!(reporter, nearly_equal(prop.at(-1.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(0.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.5), 1.5));
        reporter_assert!(reporter, nearly_equal(prop.at(2.0), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.5), 3.0));
        reporter_assert!(reporter, nearly_equal(prop.at(3.0), 4.0));
        reporter_assert!(reporter, nearly_equal(prop.at(4.0), 4.0));
    }
    {
        // Legacy style hold (hard stops)
        let prop = MockProperty::<ScalarValue>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t":  1, "s": 1, "e": 2, "h": true },
                   { "t":  2, "s": 2, "e": 4, "h": true },
                   { "t":  3 }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, !prop.is_static());
        reporter_assert!(reporter, nearly_equal(prop.at(0.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.0), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(1.5), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.0_f32.next_down()), 1.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.0), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(2.5), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(3.0_f32.next_down()), 2.0));
        reporter_assert!(reporter, nearly_equal(prop.at(3.0), 4.0));
        reporter_assert!(reporter, nearly_equal(prop.at(4.0), 4.0));
    }
    {
        // Static scalar prop (all equal keyframes, using float kf Value)
        let prop = MockProperty::<ScalarValue>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t":  1, "s": 42, "e": 42 },
                   { "t":  2, "s": 42, "e": 42 },
                   { "t":  3 }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, prop.is_static());
        reporter_assert!(reporter, nearly_equal(prop.at(0.0), 42.0));
    }
    {
        // Static vector prop (all equal keyframes, using uint32 kf Value)
        let prop = MockProperty::<Vec2Value>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t":  1, "s": [4,2], "e": [4,2] },
                   { "t":  2, "s": [4,2], "e": [4,2] },
                   { "t":  3 }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, prop.is_static());
        reporter_assert!(reporter, nearly_equal(prop.at(0.0).x, 4.0));
        reporter_assert!(reporter, nearly_equal(prop.at(0.0).y, 2.0));
    }
    {
        // Spatial interpolation [100,100]->[200,200], with supernormal easing:
        // https://cubic-bezier.com/#.5,-0.5,.5,1.5
        let prop = MockProperty::<Vec2Value>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t": 0, "s": [100,100],
                     "o":{"x":[0.5], "y":[-0.5]}, "i":{"x":[0.5], "y":[1.5]},
                    "to": [10,15], "ti": [-10,-5]
                   },
                   { "t": 1, "s": [200,200]
                   }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, !prop.is_static());

        // Not linear.
        reporter_assert!(reporter, !nearly_equal(prop.at(0.5).x, 150.0));
        reporter_assert!(reporter, !nearly_equal(prop.at(0.5).y, 150.0));

        // Subnormal region triggers extrapolation.
        reporter_assert!(reporter, prop.at(0.15).x < 100.0);
        reporter_assert!(reporter, prop.at(0.15).y < 100.0);

        // Supernormal region triggers extrapolation.
        reporter_assert!(reporter, prop.at(0.85).x > 200.0);
        reporter_assert!(reporter, prop.at(0.85).y > 200.0);
    }
    {
        // Coincident keyframes (t == 1)
        //
        // Effective interpolation intervals:
        //   [0 .. 1) -> [100 .. 200)
        //   [1 .. 2) -> [300 .. 400)
        //
        // When more than 2 concident keyframes are present, only the first and last one count.
        let prop = MockProperty::<ScalarValue>::new(
            r#"{
                 "a": 1,
                 "k": [
                   { "t": 0, "s": [100]  },
                   { "t": 1, "s": [200]  },
                   { "t": 1, "s": [1000] },
                   { "t": 1, "s": [300]  },
                   { "t": 2, "s": [400]  }
                 ]
               }"#,
        );
        reporter_assert!(reporter, prop.is_bound());
        reporter_assert!(reporter, !prop.is_static());

        reporter_assert!(reporter, prop.at(0.9999) > 100.0);
        reporter_assert!(reporter, prop.at(0.9999) < 200.0);
        // REPORTER_ASSERT(reporter, prop(1) == 300): exact comparison.
        let at_one = prop.at(1.0);
        reporter_assert!(reporter, at_one.to_bits() == 300.0_f32.to_bits());
        reporter_assert!(reporter, prop.at(1.0001) > 300.0);
        reporter_assert!(reporter, prop.at(1.0001) < 400.0);
    }
});

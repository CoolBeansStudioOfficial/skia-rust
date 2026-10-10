// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/SkottieTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::float_cmp)] // the C++ compares the scalars with ==

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_core::image::Image;
use skia_rust_core::scalar::Scalar;
use skia_rust_core::stream::MemoryStream;
use skia_rust_raster::surfaces;
use skia_rust_resources::{ImageAsset, ResourceProvider};
use skia_rust_skottie::{Animation, Builder, BuilderFlags, MarkerObserver};

use crate::{def_test, reporter_assert};

// Port of: modules/skottie/tests/SkottieTest.cpp#L24-L35 (chrome/m156)
def_test!(Skottie_OssFuzz8956, |_reporter| {
    let json = "{\"v\":\" \",\"fr\":3,\"w\":4,\"h\":3,\"layers\":[{\"ty\": 1, \"sw\": 10, \"sh\": 10,\
                \"sc\":\"#ffffff\", \"ks\":{\"o\":{\"a\": true, \"k\":\
                [{\"t\": 0, \"s\": 0, \"e\": 1, \"i\": {\"x\":[]}}]}}}]}";

    let mut stream = MemoryStream::make_copy(json.as_bytes());

    // Passes if parsing doesn't crash.
    let _animation = Animation::from_stream(&mut *stream);
});

// Port of: modules/skottie/tests/SkottieTest.cpp#L37-L98 (chrome/m156)
def_test!(Skottie_Annotations, |reporter| {
    struct TestMarkerObserver {
        markers: RefCell<Vec<(String, f32, f32)>>,
    }

    impl MarkerObserver for TestMarkerObserver {
        fn on_marker(&self, name: &str, t0: f32, t1: f32) {
            self.markers.borrow_mut().push((name.to_string(), t0, t1));
        }
    }

    let json = r##"{
                     "v": "5.2.1",
                     "w": 100,
                     "h": 100,
                     "fr": 10,
                     "ip": 0,
                     "op": 100,
                     "layers": [
                       {
                         "ty": 1,
                         "ind": 0,
                         "ip": 0,
                         "op": 1,
                         "ks": {
                           "o": { "a": 0, "k": 50 }
                         },
                         "sw": 100,
                         "sh": 100,
                         "sc": "#ffffff"
                       }
                     ],
                     "markers": [
                       {
                           "cm": "marker_1",
                           "dr": 25,
                           "tm": 25
                       },
                       {
                           "cm": "marker_2",
                           "dr": 0,
                           "tm": 75
                       }
                     ]
                   }"##;

    let mut stream = MemoryStream::make_copy(json.as_bytes());
    let observer = Rc::new(TestMarkerObserver {
        markers: RefCell::new(Vec::new()),
    });

    let animation = Builder::new()
        .set_marker_observer(observer.clone())
        .make_from_stream(&mut *stream);

    reporter_assert!(reporter, animation.is_some());
    let Some(animation) = animation else {
        return;
    };
    reporter_assert!(reporter, animation.duration() == 10.0);
    reporter_assert!(reporter, animation.in_point() == 0.0);
    reporter_assert!(reporter, animation.out_point() == 100.0);

    let markers = observer.markers.borrow();
    reporter_assert!(reporter, markers.len() == 2);
    reporter_assert!(reporter, markers[0].0 == "marker_1");
    reporter_assert!(reporter, markers[0].1 == 0.25);
    reporter_assert!(reporter, markers[0].2 == 0.50);
    reporter_assert!(reporter, markers[1].0 == "marker_2");
    reporter_assert!(reporter, markers[1].1 == 0.75);
    reporter_assert!(reporter, markers[1].2 == 0.75);
});

// Port of: modules/skottie/tests/SkottieTest.cpp#L100-L238 (chrome/m156)
def_test!(Skottie_Image_Loading, |reporter| {
    struct TestResourceProvider {
        single_frame_asset: Rc<dyn ImageAsset>,
        multi_frame_asset: Rc<dyn ImageAsset>,
    }

    impl ResourceProvider for TestResourceProvider {
        fn load_image_asset(
            &self,
            _path: &str,
            _name: &str,
            id: &str,
        ) -> Option<Rc<dyn ImageAsset>> {
            Some(if id == "single_frame" {
                Rc::clone(&self.single_frame_asset)
            } else {
                Rc::clone(&self.multi_frame_asset)
            })
        }
    }

    struct TestAsset {
        multi_frame: bool,
        requested_frames: RefCell<Vec<f32>>,
    }

    impl TestAsset {
        fn new(multi_frame: bool) -> Rc<Self> {
            Rc::new(Self {
                multi_frame,
                requested_frames: RefCell::new(Vec::new()),
            })
        }

        fn requested_frames(&self) -> Vec<f32> {
            self.requested_frames.borrow().clone()
        }
    }

    impl ImageAsset for TestAsset {
        fn is_multi_frame(&self) -> bool {
            self.multi_frame
        }

        fn get_frame(&self, t: f32) -> Option<Image> {
            self.requested_frames.borrow_mut().push(t);

            surfaces::raster_n32_premul((10, 10))
                .expect("a 10x10 surface")
                .image_snapshot()
        }
    }

    let make_animation = |reporter: &mut crate::Reporter,
                          single_asset: Rc<dyn ImageAsset>,
                          multi_asset: Rc<dyn ImageAsset>,
                          deferred_image_loading: bool|
     -> Option<Animation> {
        let json = r#"{
                         "v": "5.2.1",
                         "w": 100,
                         "h": 100,
                         "fr": 10,
                         "ip": 0,
                         "op": 100,
                         "assets": [
                           {
                             "id": "single_frame",
                             "p" : "single_frame.png",
                             "u" : "images/",
                             "w" : 500,
                             "h" : 500
                           },
                           {
                             "id": "multi_frame",
                             "p" : "multi_frame.png",
                             "u" : "images/",
                             "w" : 500,
                             "h" : 500
                           }
                         ],
                         "layers": [
                           {
                             "ty": 2,
                             "refId": "single_frame",
                             "ind": 0,
                             "ip": 0,
                             "op": 100,
                             "ks": {}
                           },
                           {
                             "ty": 2,
                             "refId": "multi_frame",
                             "ind": 1,
                             "ip": 0,
                             "op": 100,
                             "ks": {}
                           }
                         ]
                       }"#;

        let mut stream = MemoryStream::make_copy(json.as_bytes());

        let flags = if deferred_image_loading {
            BuilderFlags::DEFER_IMAGE_LOADING
        } else {
            BuilderFlags::empty()
        };
        let animation = Builder::with_flags(flags)
            .set_resource_provider(Rc::new(TestResourceProvider {
                single_frame_asset: single_asset,
                multi_frame_asset: multi_asset,
            }))
            .make_from_stream(&mut *stream);

        reporter_assert!(reporter, animation.is_some());

        animation
    };

    {
        let single_asset = TestAsset::new(false);
        let multi_asset = TestAsset::new(true);

        // Default image loading: single-frame images are loaded upfront, multi-frame images are
        // loaded on-demand.
        let animation = make_animation(reporter, single_asset.clone(), multi_asset.clone(), false);
        let Some(animation) = animation else {
            return;
        };

        reporter_assert!(reporter, single_asset.requested_frames().len() == 1);
        reporter_assert!(reporter, multi_asset.requested_frames().is_empty());
        reporter_assert!(
            reporter,
            single_asset.requested_frames()[0].nearly_zero(None)
        );

        animation.seek_frame_time(1.0);
        reporter_assert!(reporter, single_asset.requested_frames().len() == 1);
        reporter_assert!(reporter, multi_asset.requested_frames().len() == 1);
        reporter_assert!(
            reporter,
            f32::nearly_equal(multi_asset.requested_frames()[0], 1.0, None)
        );

        animation.seek_frame_time(2.0);
        reporter_assert!(reporter, single_asset.requested_frames().len() == 1);
        reporter_assert!(reporter, multi_asset.requested_frames().len() == 2);
        reporter_assert!(
            reporter,
            f32::nearly_equal(multi_asset.requested_frames()[1], 2.0, None)
        );
    }

    {
        let single_asset = TestAsset::new(false);
        let multi_asset = TestAsset::new(true);

        // Deferred image loading: both single-frame and multi-frame images are loaded on-demand.
        let animation = make_animation(reporter, single_asset.clone(), multi_asset.clone(), true);
        let Some(animation) = animation else {
            return;
        };

        reporter_assert!(reporter, single_asset.requested_frames().is_empty());
        reporter_assert!(reporter, multi_asset.requested_frames().is_empty());

        animation.seek_frame_time(1.0);
        reporter_assert!(reporter, single_asset.requested_frames().len() == 1);
        reporter_assert!(reporter, multi_asset.requested_frames().len() == 1);
        reporter_assert!(
            reporter,
            f32::nearly_equal(single_asset.requested_frames()[0], 1.0, None)
        );
        reporter_assert!(
            reporter,
            f32::nearly_equal(multi_asset.requested_frames()[0], 1.0, None)
        );

        animation.seek_frame_time(2.0);
        reporter_assert!(reporter, single_asset.requested_frames().len() == 1);
        reporter_assert!(reporter, multi_asset.requested_frames().len() == 2);
        reporter_assert!(
            reporter,
            f32::nearly_equal(multi_asset.requested_frames()[1], 2.0, None)
        );
    }
});

// Port of: modules/skottie/tests/SkottieTest.cpp#L240-L266 (chrome/m156)
def_test!(Skottie_Layer_NoType, |r| {
    let json = r#"{
             "v": "5.2.1",
             "w": 100,
             "h": 100,
             "fr": 10,
             "ip": 0,
             "op": 100,
             "layers": [
               {
                 "ind": 0,
                 "ip": 0,
                 "op": 100,
                 "ks": {}
               }
             ]
           }"#;

    let mut stream = MemoryStream::make_copy(json.as_bytes());
    let anim = Animation::from_stream(&mut *stream);

    // passes if we don't crash
    reporter_assert!(r, anim.is_some());
});

// Port of: modules/skottie/tests/SkottieTest.cpp#L268-L322 (chrome/m156)
def_test!(Skottie_Gradient_InvalidCount, |r| {
    let json = r#"{
      "v": "5.12.0",
      "fr": 60,
      "ip": 0,
      "op": 1,
      "w": 64,
      "h": 64,
      "layers": [
        {
          "ind": 1,
          "ty": 4,
          "sr": 1,
          "ks": {},
          "shapes": [
            {
              "ty": "rc",
              "d": 1,
              "p": { "a": 0, "k": [0, 0] },
              "s": { "a": 0, "k": [32, 32] },
              "r": { "a": 0, "k": 0 }
            },
            {
              "ty": "gf",
              "t": 1,
              "r": 1,
              "s": { "a": 0, "k": [-16, 0] },
              "e": { "a": 0, "k": [16, 0] },
              "o": { "a": 0, "k": 100 },
              "g": {
                "p": 1073741824,
                "k": {
                  "a": 0,
                  "k": [0.0, 0.5]
                }
              }
            }
          ],
          "ip": 0,
          "op": 1,
          "st": 0,
          "bm": 0
        }
      ]
    }"#;

    let mut stream = MemoryStream::make_copy(json.as_bytes());

    // The test passes if we don't crash.
    reporter_assert!(r, Animation::from_stream(&mut *stream).is_some());
});

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/Image.cpp (chrome/m156)

#![cfg(test)]

use std::cell::Cell;
use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::size::Size;
use skia_rust_core::stream::MemoryStream;
use skia_rust_raster::surfaces;
use skia_rust_resources::{FrameData, ImageAsset, ResourceProvider, SizeFit};
use skia_rust_skottie::Builder;

use crate::{def_test, reporter_assert};

/// One frame of the test: the time the asset is asked for, the matrix it answers with, and the
/// expected color samples at center/L/T/R/B.
// Port of: modules/skottie/tests/Image.cpp#L48-L52 (chrome/m156) (`TestData`)
struct TestData {
    t: f32,
    m: Matrix,
    c: [Color; 5],
}

// Port of: modules/skottie/tests/Image.cpp#L16-L135 (chrome/m156)
def_test!(Skottie_Image_CustomTransform, |r| {
    struct TestImageAsset {
        image: Option<Image>,
        tests: Rc<Vec<TestData>>,
        /// The index of the next `TestData` (`fTest`).
        test: Cell<usize>,
        /// `REPORTER_ASSERT(fReporter, t == fTest->t)`: the number of unexpected times.
        unexpected_times: Rc<Cell<usize>>,
    }

    impl TestImageAsset {
        fn new(tests: Rc<Vec<TestData>>, unexpected_times: Rc<Cell<usize>>) -> Self {
            let mut surf = surfaces::raster_n32_premul((200, 100)).expect("a 200x100 surface");
            surf.canvas().clear(Color::new(0xffff_0000));
            Self {
                image: surf.image_snapshot(),
                tests,
                test: Cell::new(0),
                unexpected_times,
            }
        }
    }

    impl ImageAsset for TestImageAsset {
        fn is_multi_frame(&self) -> bool {
            true
        }

        #[allow(clippy::float_cmp)] // t == fTest->t
        fn get_frame_data(&self, t: f32) -> FrameData {
            let test = &self.tests[self.test.get()];
            if t != test.t {
                self.unexpected_times.set(self.unexpected_times.get() + 1);
            }
            self.test.set(self.test.get() + 1);

            FrameData {
                image: self.image.clone(),
                sampling: SamplingOptions::default(),
                matrix: test.m.clone(),
                scaling: SizeFit::Center,
            }
        }
    }

    struct TestResourceProvider {
        tests: Rc<Vec<TestData>>,
        unexpected_times: Rc<Cell<usize>>,
    }

    impl ResourceProvider for TestResourceProvider {
        fn load_image_asset(&self, _: &str, _: &str, _: &str) -> Option<Rc<dyn ImageAsset>> {
            Some(Rc::new(TestImageAsset::new(
                Rc::clone(&self.tests),
                Rc::clone(&self.unexpected_times),
            )))
        }
    }

    let json = r#"{
             "v": "5.2.1",
             "w": 100,
             "h": 100,
             "fr": 10,
             "ip": 0,
             "op": 100,
             "assets": [{
               "id": "img_0",
               "p" : "img_0.png",
               "u" : "images/",
               "w" : 100,
               "h" :  50
             }],
             "layers": [
               {
                 "ip": 0,
                 "op": 100,
                 "ty": 2,
                 "refId": "img_0",
                 "ks": {
                   "p": { "a": 0, "k": [0,25] }
                 }
               }
             ]
           }"#;

    let mut stream = MemoryStream::make_copy(json.as_bytes());

    let red = Color::new(0xffff_0000);
    let green = Color::new(0xff00_ff00);
    let tests: Rc<Vec<TestData>> = Rc::new(vec![
        TestData {
            t: 0.0,
            m: Matrix::new_identity(),
            c: [red, red, green, red, green],
        },
        TestData {
            t: 1.0,
            m: &(&Matrix::translate((50.0, 25.0)) * &Matrix::scale((0.5, 0.5)))
                * &Matrix::translate((-50.0, -25.0)),
            c: [red, green, green, green, green],
        },
        TestData {
            t: 2.0,
            m: Matrix::translate((-50.0, 0.0)),
            c: [green, red, green, green, green],
        },
        TestData {
            t: 3.0,
            m: Matrix::translate((0.0, -25.0)),
            c: [green, green, red, green, green],
        },
        TestData {
            t: 4.0,
            m: Matrix::translate((50.0, 0.0)),
            c: [red, green, green, red, green],
        },
        TestData {
            t: 5.0,
            m: Matrix::translate((0.0, 25.0)),
            c: [red, red, green, red, red],
        },
    ]);

    let unexpected_times = Rc::new(Cell::new(0));
    let anim = Builder::new()
        .set_resource_provider(Rc::new(TestResourceProvider {
            tests: Rc::clone(&tests),
            unexpected_times: Rc::clone(&unexpected_times),
        }))
        .make_from_stream(&mut *stream);

    reporter_assert!(r, anim.is_some());
    let Some(anim) = anim else {
        return;
    };

    let render_size = Size::new(100.0, 100.0);
    let mut surf = surfaces::raster_n32_premul((100, 100)).expect("a 100x100 surface");
    let rect = Rect::from_wh(render_size.width, render_size.height);

    for tst in tests.iter() {
        surf.canvas().clear(green);
        anim.seek_frame_time(f64::from(tst.t));
        anim.render(surf.canvas(), Some(&rect));

        let peeked = surf.peek_pixels().expect("the surface has pixels");
        let pmap = peeked.pixmap();
        // 100 / 2, 100 - 1: integer arithmetic on the render size.
        reporter_assert!(r, tst.c[0] == pmap.get_color((50, 50)));
        reporter_assert!(r, tst.c[1] == pmap.get_color((1, 50)));
        reporter_assert!(r, tst.c[2] == pmap.get_color((50, 1)));
        reporter_assert!(r, tst.c[3] == pmap.get_color((99, 50)));
        reporter_assert!(r, tst.c[4] == pmap.get_color((50, 99)));
    }

    reporter_assert!(r, unexpected_times.get() == 0);
});

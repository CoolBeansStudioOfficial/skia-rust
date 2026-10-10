// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/AudioLayer.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::float_cmp)] // the C++ compares the scalars with ==

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use skia_rust_core::stream::MemoryStream;
use skia_rust_resources::{ExternalTrackAsset, ResourceProvider};
use skia_rust_skottie::Builder;

use crate::{def_test, reporter_assert};

// Port of: modules/skottie/tests/AudioLayer.cpp#L15-L134 (chrome/m156)
def_test!(Skottie_AudioLayer, |r| {
    struct MockTracker {
        current_time: Cell<f32>,
    }

    impl MockTracker {
        fn is_playing(&self) -> bool {
            self.current_time.get() >= 0.0
        }

        fn current_time(&self) -> f32 {
            self.current_time.get()
        }
    }

    impl ExternalTrackAsset for MockTracker {
        fn seek(&self, t: f32) {
            self.current_time.set(t);
        }
    }

    struct MockResourceProvider {
        /// `REPORTER_ASSERT`s of the provider: the number of unexpected arguments.
        unexpected_args: Cell<usize>,
        tracks: RefCell<Vec<Rc<MockTracker>>>,
    }

    impl MockResourceProvider {
        fn tracks(&self) -> Vec<Rc<MockTracker>> {
            self.tracks.borrow().clone()
        }
    }

    impl ResourceProvider for MockResourceProvider {
        fn load_audio_asset(
            &self,
            path: &str,
            name: &str,
            id: &str,
        ) -> Option<Rc<dyn ExternalTrackAsset>> {
            if path != "assets/" || name != "audio.mp3" || id != "audio_0" {
                self.unexpected_args.set(self.unexpected_args.get() + 1);
            }

            let track = Rc::new(MockTracker {
                current_time: Cell::new(0.0),
            });
            self.tracks.borrow_mut().push(Rc::clone(&track));

            Some(track)
        }
    }

    let json = r##"{
             "v": "5.2.1",
             "w": 100,
             "h": 100,
             "fr": 10,
             "ip": 0,
             "op": 100,
             "assets": [
               {
                 "id": "audio_0",
                 "p" : "audio.mp3",
                 "u" : "assets/"
               }
             ],
             "layers": [
               {
                 "ty"   : 6,
                 "ind"  : 0,
                 "ip"   : 20,
                 "op"   : 70,
                 "refId": "audio_0"
               },
               {
                 "ty"   : 6,
                 "ind"  : 0,
                 "ip"   : 50,
                 "op"   : 80,
                 "refId": "audio_0"
               },
               {
                 "ty": 1,
                 "ip": 0,
                 "op": 100,
                 "sw": 100,
                 "sh": 100,
                 "sc": "#ffffff"
               }
             ]
           }"##;

    let mut stream = MemoryStream::make_copy(json.as_bytes());
    let rp = Rc::new(MockResourceProvider {
        unexpected_args: Cell::new(0),
        tracks: RefCell::new(Vec::new()),
    });

    let skottie = Builder::new()
        .set_resource_provider(rp.clone())
        .make_from_stream(&mut *stream);

    let tracks = rp.tracks();

    // REPORTER_ASSERT(fReporter, !strcmp(path, "assets/")) and friends, in loadAudioAsset.
    reporter_assert!(r, rp.unexpected_args.get() == 0);
    reporter_assert!(r, skottie.is_some());
    reporter_assert!(r, tracks.len() == 2);
    let Some(skottie) = skottie else {
        return;
    };

    skottie.seek_frame(0.0);
    reporter_assert!(r, !tracks[0].is_playing());
    reporter_assert!(r, !tracks[1].is_playing());

    skottie.seek_frame(20.0);
    reporter_assert!(r, tracks[0].is_playing());
    reporter_assert!(r, !tracks[1].is_playing());
    reporter_assert!(r, tracks[0].current_time() == 0.0);

    skottie.seek_frame(50.0);
    reporter_assert!(r, tracks[0].is_playing());
    reporter_assert!(r, tracks[1].is_playing());
    reporter_assert!(r, tracks[0].current_time() == 3.0);
    reporter_assert!(r, tracks[1].current_time() == 0.0);

    skottie.seek_frame(70.0);
    reporter_assert!(r, tracks[0].is_playing());
    reporter_assert!(r, tracks[1].is_playing());
    reporter_assert!(r, tracks[0].current_time() == 5.0);
    reporter_assert!(r, tracks[1].current_time() == 2.0);

    skottie.seek_frame(80.0);
    reporter_assert!(r, !tracks[0].is_playing());
    reporter_assert!(r, tracks[1].is_playing());
    reporter_assert!(r, tracks[1].current_time() == 3.0);

    skottie.seek_frame(100.0);
    reporter_assert!(r, !tracks[0].is_playing());
    reporter_assert!(r, !tracks[1].is_playing());
});

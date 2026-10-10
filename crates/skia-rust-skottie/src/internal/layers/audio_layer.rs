// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/AudioLayer.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_resources::ExternalTrackAsset;
use skia_rust_sksg::RenderNode;

use crate::internal::animator::{Animator, StateChanged};
use crate::internal::skottie_priv::{AnimationBuilder, ScopedAssetRef};
use crate::json::ObjectValue;
use crate::skottie::{LayerInfo, LoggerLevel};
use crate::skottie_json::{ValueExt, string_text};

/// Forwards the animation time to an audio track.
// Port of: modules/skottie/src/layers/AudioLayer.cpp#L21-L47 (chrome/m156) (`ForwardingPlaybackController`)
struct ForwardingPlaybackController {
    track: Rc<dyn ExternalTrackAsset>,
    in_point: f32,
    out_point: f32,
    fps: f32,
}

impl Animator for ForwardingPlaybackController {
    fn seek(&self, t: f32) -> StateChanged {
        // Adjust t relative to the track time (s).
        let t = if t < self.in_point || t > self.out_point {
            -1.0
        } else {
            (t - self.in_point) / self.fps
        };

        self.track.seek(t);

        // does not interact with the render tree.
        false
    }
}

impl<'j> AnimationBuilder<'j> {
    /// Attaches an audio layer: it has no render node, its playback is controlled from the
    /// animator tree.
    // Port of: modules/skottie/src/layers/AudioLayer.cpp#L51-L84 (chrome/m156) (`attachAudioLayer`)
    #[doc(alias = "attachAudioLayer")]
    #[must_use]
    pub fn attach_audio_layer(
        &self,
        jlayer: &ObjectValue,
        layer_info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        let audio_asset = ScopedAssetRef::new(self, jlayer);

        if let Some(jaudio) = audio_asset.asset() {
            let name = jaudio.get("p").as_string();
            let path = jaudio.get("u").as_string();
            let id = jaudio.get("id").as_string();

            if let (Some(name), Some(path), Some(id)) = (name, path, id) {
                let track = self.resource_provider().and_then(|rp| {
                    rp.load_audio_asset(
                        &string_text(path),
                        &string_text(name),
                        &string_text(id),
                    )
                });
                if let Some(track) = track {
                    self.push_animator(Rc::new(ForwardingPlaybackController {
                        track,
                        in_point: layer_info.in_point,
                        out_point: layer_info.out_point,
                        fps: self.frame_rate(),
                    }));
                } else {
                    self.log(
                        LoggerLevel::Warning,
                        &format!("Could not load audio asset '{}'.", string_text(name)),
                    );
                }
            }
        }

        // no render node, playback is controlled from the Animator tree.
        None
    }
}

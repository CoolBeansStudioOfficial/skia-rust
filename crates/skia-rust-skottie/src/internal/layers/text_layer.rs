// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/TextLayer.cpp (chrome/m156)
//
// The text layer boundary. Text layers are M22: they need the shaper (M5/M6), the font table and
// the text animators. Until then a text layer builds no content, and says so in the log.

use std::rc::Rc;

use skia_rust_sksg::RenderNode;

use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::ObjectValue;
use crate::skottie::{LayerInfo, LoggerLevel};

impl AnimationBuilder<'_> {
    /// Attaches a text layer (`attachTextLayer`). Not ported yet (M22): the layer has no content.
    // Port of: modules/skottie/src/layers/TextLayer.cpp#L378-L402 (chrome/m156)
    #[doc(alias = "attachTextLayer")]
    #[must_use]
    pub fn attach_text_layer(
        &self,
        layer: &ObjectValue,
        _info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        self.log_json(
            LoggerLevel::Warning,
            layer,
            "Text layers are not supported yet.",
        );
        None
    }
}

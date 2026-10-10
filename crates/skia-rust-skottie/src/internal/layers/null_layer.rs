// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/NullLayer.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_sksg::RenderNode;

use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::ObjectValue;
use crate::skottie::LayerInfo;

impl AnimationBuilder<'_> {
    /// Attaches a null layer: no render node.
    // Port of: modules/skottie/src/layers/NullLayer.cpp#L17-L22 (chrome/m156) (`attachNullLayer`)
    #[doc(alias = "attachNullLayer")]
    #[must_use]
    pub fn attach_null_layer(
        &self,
        _layer: &ObjectValue,
        _info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        // Null layers are used solely to drive dependent transforms,
        // but we use free-floating sksg::Matrices for that purpose.
        None
    }
}

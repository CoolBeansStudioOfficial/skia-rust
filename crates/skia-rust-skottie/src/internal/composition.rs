// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/Composition.h, modules/skottie/src/Composition.cpp
// (chrome/m156)

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use skia_rust_core::size::Size;
use skia_rust_sksg::{Group, RenderNode, Transform};

use crate::json::ObjectValue;
use crate::skottie::LoggerLevel;
use crate::skottie_json::{ValueExt, parse_default};

use super::camera::default_camera_transform;
use super::layer::LayerBuilder;
use super::skottie_priv::AnimationBuilder;

/// Builds the layers of a composition.
// Port of: modules/skottie/src/Composition.h#L27-L52 (chrome/m156) (`class CompositionBuilder`)
#[doc(alias = "skottie::internal::CompositionBuilder")]
pub struct CompositionBuilder<'j> {
    size: Size,
    layer_builders: Vec<LayerBuilder<'j>>,
    /// Maps layer "ind" to layer builder index.
    layer_index_map: HashMap<i32, usize>,
    camera_transform: RefCell<Option<Rc<dyn Transform>>>,
}

impl<'j> CompositionBuilder<'j> {
    /// The builder of the composition `jcomp` of the given size.
    // Port of: modules/skottie/src/Composition.cpp#L51-L91 (chrome/m156)
    #[must_use]
    pub fn new(abuilder: &AnimationBuilder<'j>, size: Size, jcomp: &'j ObjectValue) -> Self {
        let mut camera_builder_index: Option<usize> = None;
        let mut layer_builders = Vec::new();
        let mut layer_index_map = HashMap::new();

        // Prepare layer builders.
        if let Some(jlayers) = jcomp.get("layers").as_array() {
            layer_builders.reserve(jlayers.size());
            for i in 0..jlayers.size() {
                let Some(jlayer) = jlayers[i].as_object() else {
                    continue;
                };

                let lbuilder_index = layer_builders.len();
                layer_builders.push(LayerBuilder::new(jlayer, size));
                let lbuilder = &layer_builders[lbuilder_index];

                layer_index_map.insert(lbuilder.index(), lbuilder_index);

                // Keep track of the camera builder.
                if lbuilder.is_camera() {
                    // We only support one (first) camera for now.
                    if camera_builder_index.is_none() {
                        camera_builder_index = Some(lbuilder_index);
                    } else {
                        abuilder.log_json(
                            LoggerLevel::Warning,
                            jlayer,
                            "Ignoring duplicate camera layer.",
                        );
                    }
                }
            }
        }

        let this = Self {
            size,
            layer_builders,
            layer_index_map,
            camera_transform: RefCell::new(None),
        };

        // Attach a camera transform upfront, if needed (required to build
        // all other 3D transform chains).
        if let Some(camera_builder_index) = camera_builder_index {
            // Explicit camera.
            let camera = this.layer_builders[camera_builder_index].build_transform(abuilder, &this);
            *this.camera_transform.borrow_mut() = camera;
        } else if parse_default::<i32>(jcomp.get("ddd"), 0) != 0 && !this.size.is_empty() {
            // Default/implicit camera when 3D layers are present.
            *this.camera_transform.borrow_mut() = Some(default_camera_transform(this.size));
        }

        this
    }

    /// The size of the composition.
    #[must_use]
    pub fn size(&self) -> Size {
        self.size
    }

    /// The camera transform, if there is one (`getCameraTransform`).
    #[must_use]
    pub(crate) fn camera_transform(&self) -> Option<Rc<dyn Transform>> {
        self.camera_transform.borrow().clone()
    }

    /// The builder of the layer with the index `layer_index` ("ind"), if there is one.
    // Port of: modules/skottie/src/Composition.cpp#L95-L105 (chrome/m156) (`layerBuilder`)
    #[doc(alias = "layerBuilder")]
    #[must_use]
    pub fn layer_builder(&self, layer_index: i32) -> Option<&LayerBuilder<'j>> {
        if layer_index < 0 {
            return None;
        }

        self.layer_index_map
            .get(&layer_index)
            .map(|idx| &self.layer_builders[*idx])
    }

    /// The content tree of the layer with the index `layer_index`.
    // Port of: modules/skottie/src/Composition.cpp#L107-L114 (chrome/m156) (`layerContent`)
    #[doc(alias = "layerContent")]
    #[must_use]
    pub fn layer_content(
        &self,
        abuilder: &AnimationBuilder<'j>,
        layer_index: i32,
    ) -> Option<Rc<dyn RenderNode>> {
        if let Some(lbuilder) = self.layer_builder(layer_index) {
            return lbuilder.get_content_tree(abuilder, self);
        }

        None
    }

    /// Builds the render tree of the composition.
    // Port of: modules/skottie/src/Composition.cpp#L116-L146 (chrome/m156)
    #[must_use]
    pub fn build(&self, abuilder: &AnimationBuilder<'j>) -> Option<Rc<dyn RenderNode>> {
        // First pass - transitively attach layer transform chains.
        for lbuilder in &self.layer_builders {
            let _ = lbuilder.build_transform(abuilder, self);
        }

        // Second pass - attach actual layer contents and finalize the layer render tree.
        let mut layers: Vec<Rc<dyn RenderNode>> = Vec::with_capacity(self.layer_builders.len());

        let mut prev_layer_index = -1;
        for lbuilder in &self.layer_builders {
            if let Some(layer) = lbuilder.build_render_tree(abuilder, self, prev_layer_index) {
                layers.push(layer);
            }
            prev_layer_index = lbuilder.index();
        }

        if layers.is_empty() {
            return None;
        }

        if layers.len() == 1 {
            return layers.pop();
        }

        // Layers are painted in bottom->top order.
        layers.reverse();
        layers.shrink_to_fit();

        Some(Group::make_with_children(&layers) as Rc<dyn RenderNode>)
    }
}

opaque_debug!(CompositionBuilder<'j>);

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/Path.cpp (chrome/m156)

use std::rc::{Rc, Weak};

use skia_rust_sksg::Path as SgPath;

use crate::impl_container_animator;
use crate::json::Value;
use crate::skottie_value::ShapeValue;

use super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, IntoJsonProp, Prop, PropertyContainer,
};
use super::skottie_priv::AnimationBuilder;

/// Drives the path of a path node from a shape property.
// Port of: modules/skottie/src/Path.cpp#L21-L47 (chrome/m156) (`class PathAdapter`)
struct PathAdapter {
    base: DiscardableAdapterBase<SgPath>,
    shape: Prop<ShapeValue>,
}

impl PathAdapter {
    fn make<'a>(jpath: impl IntoJsonProp<'a>, abuilder: &AnimationBuilder<'_>) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), SgPath::make_empty());
            let shape = Prop::new(ShapeValue::new());
            base.container().bind(abuilder, jpath, &shape);
            Self { base, shape }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }
}

impl AnimatablePropertyContainer for PathAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/Path.cpp#L30-L41 (chrome/m156) (`onSync`)
    fn on_sync(&self) {
        let path_node = self.base.node();

        let mut path = self.shape.borrow().to_path();

        // FillType is tracked in the SG node, not in keyframes -- make sure we preserve it.
        path.set_fill_type(path_node.fill_type());
        path.set_is_volatile(!self.is_static());

        path_node.set_path(path);
    }
}

impl_container_animator!(PathAdapter);

impl AnimationBuilder<'_> {
    /// Attaches a path node driven by the shape property `jpath`.
    // Port of: modules/skottie/src/Path.cpp#L51-L53 (chrome/m156) (`attachPath`)
    #[doc(alias = "attachPath")]
    #[must_use]
    pub fn attach_path(&self, jpath: &Value) -> Option<Rc<SgPath>> {
        let adapter = PathAdapter::make(jpath, self);
        let node = Rc::clone(adapter.base.node());
        self.attach_discardable_adapter(&adapter);
        Some(node)
    }
}

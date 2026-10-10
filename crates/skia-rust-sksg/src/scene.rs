// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGScene.h, modules/sksg/src/SkSGScene.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;

use crate::invalidation_controller::InvalidationController;
use crate::render_node::{RenderNode, node_at};

/// The root of a render tree, with its entry points: render, revalidate and hit-test (`Scene`).
// Port of: modules/sksg/include/SkSGScene.h#L13-L33 (chrome/m156) (`class Scene`)
#[doc(alias = "sksg::Scene")]
#[derive(Debug)]
pub struct Scene {
    root: Rc<dyn RenderNode>,
}

impl Scene {
    /// `Scene::Make(root)`: `None` if there is no root.
    // Port of: modules/sksg/src/SkSGScene.cpp#L16-L18 (chrome/m156) (`Scene::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(root: Option<Rc<dyn RenderNode>>) -> Option<Self> {
        root.map(|root| Self { root })
    }

    /// Renders the scene (`render`).
    // Port of: modules/sksg/src/SkSGScene.cpp#L24-L26 (chrome/m156) (`Scene::render`)
    pub fn render(&self, canvas: &Canvas) {
        self.root.render(canvas, None);
    }

    /// Revalidates the scene, reporting damage to `ic` (`revalidate`).
    // Port of: modules/sksg/src/SkSGScene.cpp#L28-L30 (chrome/m156) (`Scene::revalidate`)
    pub fn revalidate(&self, ic: Option<&mut InvalidationController>) {
        self.root.revalidate(ic, &Matrix::new_identity());
    }

    /// The front-most render node at `p` (`nodeAt`).
    // Port of: modules/sksg/src/SkSGScene.cpp#L32-L34 (chrome/m156) (`Scene::nodeAt`)
    #[doc(alias = "nodeAt")]
    #[must_use]
    pub fn node_at(&self, p: Point) -> Option<Rc<dyn RenderNode>> {
        node_at(&self.root, p)
    }
}

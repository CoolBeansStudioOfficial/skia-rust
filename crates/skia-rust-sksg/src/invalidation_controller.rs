// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGInvalidationController.h,
// modules/sksg/src/SkSGInvalidationController.cpp (chrome/m156)

use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;

/// Collects the damage rects of a revalidation pass.
// Port of: modules/sksg/include/SkSGInvalidationController.h#L14-L40 (chrome/m156)
#[doc(alias = "sksg::InvalidationController")]
#[derive(Debug, Clone, Default)]
pub struct InvalidationController {
    rects: Vec<Rect>,
    bounds: Rect,
}

impl InvalidationController {
    /// Port of `InvalidationController::InvalidationController()`.
    // Port of: modules/sksg/src/SkSGInvalidationController.cpp#L13-L13 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            rects: Vec::new(),
            bounds: Rect::new_empty(),
        }
    }

    /// Adds `r`, mapped by `ctm`, as damage. Empty rects are dropped.
    // Port of: modules/sksg/src/SkSGInvalidationController.cpp#L15-L23 (chrome/m156)
    pub fn inval(&mut self, r: Rect, ctm: &Matrix) {
        if r.is_empty() {
            return;
        }
        let (rect, _) = ctm.map_rect(r);
        self.rects.push(rect);
        self.bounds.join(rect);
    }

    /// The union of all damage rects.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// The damage rects, in the order they were added.
    #[must_use]
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Clears the damage.
    // Port of: modules/sksg/src/SkSGInvalidationController.cpp#L25-L28 (chrome/m156)
    pub fn reset(&mut self) {
        self.rects.clear();
        self.bounds.set_empty();
    }
}

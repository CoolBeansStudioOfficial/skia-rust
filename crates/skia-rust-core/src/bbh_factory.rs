// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkBBHFactory.h, src/core/SkBBHFactory.cpp

//! `SkBBoxHierarchy` and `SkBBHFactory`: the bounding box hierarchy a picture can carry to skip
//! the ops outside the clip, and the factories that make one.

use std::sync::Arc;

use crate::r_tree::RTree;
use crate::rect::Rect;

/// What is known about the op a bounding box belongs to (`SkBBoxHierarchy::Metadata`).
// Port of: include/core/SkBBHFactory.h#L21-L23 (chrome/m156)
#[doc(alias = "SkBBoxHierarchy::Metadata")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Hash)]
pub struct Metadata {
    /// The corresponding rect bounds a draw command, not a pure state change.
    pub is_draw: bool,
}

/// A bounding box hierarchy (`SkBBoxHierarchy`).
///
/// skia-rust: Skia shares a hierarchy with `sk_sp` (the recorder, the picture and sometimes the
/// caller hold it), so the handle is an `Arc<dyn BBoxHierarchy>` and [`insert`](Self::insert)
/// takes `&self`: an implementation keeps its state behind interior mutability.
// Port of: include/core/SkBBHFactory.h#L19-L48 (chrome/m156)
#[doc(alias = "SkBBoxHierarchy")]
pub trait BBoxHierarchy: Send + Sync {
    /// Inserts N bounding boxes into the hierarchy (`insert(const SkRect[], int N)`).
    fn insert(&self, rects: &[Rect]);

    /// Inserts N bounding boxes with their metadata (`insert(const SkRect[], const Metadata[], int
    /// N)`); the default ignores the metadata.
    // Port of: src/core/SkBBHFactory.cpp#L16-L19 (chrome/m156)
    fn insert_with_metadata(&self, rects: &[Rect], _meta: &[Metadata]) {
        // Ignore Metadata.
        self.insert(rects);
    }

    /// Appends to `results` the indices of the bounding boxes intersecting `query`.
    fn search(&self, query: &Rect, results: &mut Vec<usize>);

    /// Returns the approximate size in memory of `self`.
    #[doc(alias = "bytesUsed")]
    fn bytes_used(&self) -> usize;
}

/// Makes bounding box hierarchies (`SkBBHFactory`).
// Port of: include/core/SkBBHFactory.h#L50-L61 (chrome/m156)
#[doc(alias = "SkBBHFactory")]
pub trait BBHFactory {
    /// Allocates a new hierarchy; `None` on failure (`operator()`).
    fn make(&self) -> Option<Arc<dyn BBoxHierarchy>>;
}

/// The factory of R-trees (`SkRTreeFactory`).
// Port of: include/core/SkBBHFactory.h#L63-L65 (chrome/m156)
#[doc(alias = "SkRTreeFactory")]
#[derive(Copy, Clone, Debug, Default)]
pub struct RTreeFactory;

impl BBHFactory for RTreeFactory {
    // Port of: src/core/SkBBHFactory.cpp#L12-L14 (chrome/m156)
    fn make(&self) -> Option<Arc<dyn BBoxHierarchy>> {
        Some(Arc::new(RTree::new()))
    }
}

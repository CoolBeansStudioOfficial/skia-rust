// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGIDMapper.h

//! The id to node map of an SVG document (`SkSVGIDMapper`).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::node::Node;

/// `SkSVGIDMapper`: `THashMap<SkString, sk_sp<SkSVGNode>>`.
///
/// Entries are temporarily emptied while a node is borrowed (see
/// [`BorrowedNode`](crate::render_context::BorrowedNode)), which is why they are behind a lock.
// Port of: modules/svg/include/SkSVGIDMapper.h#L16-L17 (chrome/m156)
#[doc(alias = "SkSVGIDMapper")]
#[derive(Debug, Default)]
pub struct IdMapper {
    map: HashMap<String, Mutex<Option<Node>>>,
}

impl IdMapper {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Associates `id` with `node`, replacing an earlier association.
    pub fn set(&mut self, id: String, node: Node) {
        self.map.insert(id, Mutex::new(Some(node)));
    }

    /// The slot of `id`.
    #[must_use]
    pub fn find_slot(&self, id: &str) -> Option<&Mutex<Option<Node>>> {
        self.map.get(id)
    }

    /// A reference to the node with the given id, if there is one that is not currently
    /// borrowed.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<Node> {
        self.map
            .get(id)
            .and_then(|slot| slot.lock().ok().and_then(|n| n.clone()))
    }
}

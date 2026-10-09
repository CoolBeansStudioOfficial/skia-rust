// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GraphiteResourceKey.h, src/gpu/graphite/GraphiteResourceKey.cpp

//! `GraphiteResourceKey`: a [`ResourceKey`] whose domain is a Graphite [`ResourceType`].
//!
//! Graphite does not use different kinds of keys to manage reusability or sharing like Ganesh. A
//! key encodes the configuration of the resource; whether it can be shared is decided by the
//! `ResourceCache` and `ResourceProvider`.

use std::hash::{Hash, Hasher};
use std::ops::{Deref, Index, IndexMut};
use std::sync::atomic::{AtomicI32, Ordering};

use crate::gpu::resource_key::{ResourceKey, ResourceKeyBuilder};
use crate::graphite::resource_types::ResourceType;

/// `ResourceKey::kInvalidDomain`.
const INVALID_DOMAIN: i32 = 0;

// Port of: src/gpu/graphite/GraphiteResourceKey.cpp#L13 (chrome/m156)
static NEXT_TYPE: AtomicI32 = AtomicI32::new(INVALID_DOMAIN + 1);

/// A Graphite resource cache key.
// Port of: src/gpu/graphite/GraphiteResourceKey.h#L17-L52 (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphiteResourceKey")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphiteResourceKey {
    key: ResourceKey,
}

impl GraphiteResourceKey {
    /// `GraphiteResourceKey()`: an invalid key. It must be initialized with a
    /// [`GraphiteResourceKeyBuilder`] before use.
    // Port of: src/gpu/graphite/GraphiteResourceKey.h#L28 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `GenerateResourceType()`: a resource type no other call has returned.
    ///
    /// # Panics
    /// If more than `u16::MAX` types are generated (Skia aborts).
    // Port of: src/gpu/graphite/GraphiteResourceKey.cpp#L12-L21 (chrome/m156)
    #[doc(alias = "GenerateResourceType")]
    #[must_use]
    pub fn generate_resource_type() -> ResourceType {
        let ty = NEXT_TYPE.fetch_add(1, Ordering::Relaxed);
        assert!(
            ty <= i32::from(u16::MAX),
            "Too many Graphite Resource Types"
        );
        ResourceType::try_from(ty).expect("resource types are positive")
    }

    /// `resourceType()`: the key's domain.
    // Port of: src/gpu/graphite/GraphiteResourceKey.h#L32 (chrome/m156)
    #[doc(alias = "resourceType")]
    #[must_use]
    pub fn resource_type(&self) -> ResourceType {
        ResourceType::from(self.key.domain())
    }

    /// The underlying [`ResourceKey`].
    #[must_use]
    pub fn as_resource_key(&self) -> &ResourceKey {
        &self.key
    }
}

impl Deref for GraphiteResourceKey {
    type Target = ResourceKey;

    fn deref(&self) -> &ResourceKey {
        &self.key
    }
}

impl Hash for GraphiteResourceKey {
    // The cache hashes keys with `key.hash()` (ResourceCache::MapTraits::Hash).
    // Port of: src/gpu/graphite/ResourceCache.h#L171-L176 (chrome/m156)
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(if self.key.is_valid() {
            self.key.hash()
        } else {
            0
        });
    }
}

/// `GraphiteResourceKey::Builder`: fills in the key's data; the hash is computed when the builder
/// is dropped or [`finish`](Self::finish)ed.
// Port of: src/gpu/graphite/GraphiteResourceKey.h#L46-L50 (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphiteResourceKey::Builder")]
#[derive(Debug)]
pub struct GraphiteResourceKeyBuilder<'a>(ResourceKeyBuilder<'a>);

impl<'a> GraphiteResourceKeyBuilder<'a> {
    /// Starts building `key` for `ty` with `data32_count` data words.
    // Port of: src/gpu/graphite/GraphiteResourceKey.h#L48-L49 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // ResourceType -> uint16_t domain, as in C++
    #[must_use]
    pub fn new(key: &'a mut GraphiteResourceKey, ty: ResourceType, data32_count: u16) -> Self {
        Self(ResourceKeyBuilder::new(
            &mut key.key,
            ty as u16,
            data32_count,
        ))
    }

    /// `finish()`.
    pub fn finish(&mut self) {
        self.0.finish();
    }
}

impl Index<usize> for GraphiteResourceKeyBuilder<'_> {
    type Output = u32;

    fn index(&self, data_idx: usize) -> &u32 {
        &self.0[data_idx]
    }
}

impl IndexMut<usize> for GraphiteResourceKeyBuilder<'_> {
    fn index_mut(&mut self, data_idx: usize) -> &mut u32 {
        &mut self.0[data_idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_compare_by_type_and_data() {
        let t1 = GraphiteResourceKey::generate_resource_type();
        let t2 = GraphiteResourceKey::generate_resource_type();
        assert_ne!(t1, t2);

        let make = |ty, v| {
            let mut key = GraphiteResourceKey::new();
            {
                let mut b = GraphiteResourceKeyBuilder::new(&mut key, ty, 1);
                b[0] = v;
            }
            key
        };
        let a = make(t1, 7);
        assert!(a.is_valid());
        assert_eq!(a.resource_type(), t1);
        assert_eq!(a, make(t1, 7));
        assert_ne!(a, make(t1, 8));
        assert_ne!(a, make(t2, 7));
        assert!(!GraphiteResourceKey::new().is_valid());
    }
}

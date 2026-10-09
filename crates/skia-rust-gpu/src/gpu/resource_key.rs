// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/ResourceKey.h, src/gpu/ResourceKey.cpp

//! `skgpu::ResourceKey`, `ScratchKey`, `UniqueKey` and `FixedSizeKey`: the keys the GPU resource
//! cache indexes its resources with.
//!
//! A [`ResourceKey`] is a `Vec<u32>` laid out exactly as in Skia: two metadata words (the hash,
//! then the domain in the low 16 bits and the data length in `u32`s in the high 16 bits)
//! followed by the data. The hash covers everything after itself, so [`ResourceKey::hash`]
//! and the equality of two keys mean the same as in C++.
//!
//! Process-wide domain and resource-type allocation (`GenerateDomain`, `GenerateResourceType`)
//! keeps Skia's atomic counters. Skia's `SKGPU_DEFINE_STATIC_UNIQUE_KEY` macro becomes a
//! `LazyLock<UniqueKey>` built by [`init_static_unique_key`].

use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::sync::atomic::{AtomicU32, Ordering};

use skia_rust_core::checksum::hash32;
use skia_rust_core::data::Data;

/// `SK_InvalidUniqueID` (`include/core/SkTypes.h`).
// Port of: include/core/SkTypes.h#L191 (chrome/m156)
pub const SK_INVALID_UNIQUE_ID: u32 = 0;

/// `ResourceKeyHash`: hashes the given words as their in-memory bytes.
///
/// Skia hashes `size` bytes starting at `data`; the bytes of each `u32` are taken in native
/// order, which is little-endian on every target this crate supports.
// Port of: src/gpu/ResourceKey.cpp#L35-L37 (chrome/m156)
#[doc(alias = "ResourceKeyHash")]
#[must_use]
pub fn resource_key_hash(data: &[u32]) -> u32 {
    let bytes: Vec<u8> = data.iter().flat_map(|w| w.to_ne_bytes()).collect();
    hash32(&bytes, 0)
}

/// Index of the hash word.
const HASH_META_DATA_IDX: usize = 0;
/// Index of the packed domain and size.
const DOMAIN_AND_SIZE_META_DATA_IDX: usize = 1;
/// Number of metadata words in front of the data.
const META_DATA_CNT: usize = DOMAIN_AND_SIZE_META_DATA_IDX + 1;

/// Base class for all GPU resource cache keys.
///
/// The layout is `[hash, domain | (data32Count << 16), data...]`.
// Port of: src/gpu/ResourceKey.h#L27-L146 (chrome/m156)
#[doc(alias = "skgpu::ResourceKey")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceKey {
    key: Vec<u32>,
}

/// The domain of an invalid key.
const INVALID_DOMAIN: u16 = 0;

impl Default for ResourceKey {
    // Port of: src/gpu/ResourceKey.h#L53 (chrome/m156)
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceKey {
    /// `ResourceKey()`: an invalid key.
    // Port of: src/gpu/ResourceKey.h#L53 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        let mut key = Self { key: Vec::new() };
        key.reset();
        key
    }

    // Port of: src/gpu/ResourceKey.h#L32-L37 (chrome/m156)
    /// `hash()`.
    #[must_use]
    pub fn hash(&self) -> u32 {
        self.validate();
        self.key[HASH_META_DATA_IDX]
    }

    // Port of: src/gpu/ResourceKey.h#L38-L43 (chrome/m156)
    /// `size()`: the total size in bytes, including metadata.
    #[must_use]
    pub fn size(&self) -> usize {
        self.validate();
        debug_assert!(self.is_valid());
        self.internal_size()
    }

    // Port of: src/gpu/ResourceKey.h#L45-L51 (chrome/m156)
    /// `reset()`: makes the key invalid.
    pub fn reset(&mut self) {
        self.key.clear();
        self.key.resize(META_DATA_CNT, 0);
        self.key[HASH_META_DATA_IDX] = 0;
        self.key[DOMAIN_AND_SIZE_META_DATA_IDX] = u32::from(INVALID_DOMAIN);
    }

    // Port of: src/gpu/ResourceKey.h#L57 (chrome/m156)
    /// `isValid()`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        INVALID_DOMAIN != self.domain()
    }

    // Port of: src/gpu/ResourceKey.h#L80 (chrome/m156), protected in C++
    /// The domain, from the low 16 bits of the packed word.
    #[must_use]
    pub(crate) fn domain(&self) -> u16 {
        (self.key[DOMAIN_AND_SIZE_META_DATA_IDX] & 0xffff) as u16
    }

    // Port of: src/gpu/ResourceKey.h#L82-L83 (chrome/m156), protected in C++
    /// Size of the key data in bytes, excluding metadata.
    #[must_use]
    pub(crate) fn data_size(&self) -> usize {
        (self.key[DOMAIN_AND_SIZE_META_DATA_IDX] >> 16) as usize * std::mem::size_of::<u32>()
    }

    // Port of: src/gpu/ResourceKey.h#L85-L90 (chrome/m156), protected in C++
    /// The key data, excluding metadata.
    #[must_use]
    pub(crate) fn data(&self) -> &[u32] {
        self.validate();
        &self.key[META_DATA_CNT..]
    }

    // Port of: src/gpu/ResourceKey.h#L185-L186 (chrome/m156)
    // Total size in bytes, including metadata.
    fn internal_size(&self) -> usize {
        self.data_size() + std::mem::size_of::<u32>() * META_DATA_CNT
    }

    // Port of: src/gpu/ResourceKey.h#L188-L194 (chrome/m156)
    fn validate(&self) {
        debug_assert!(self.is_valid());
        debug_assert_eq!(
            self.key[HASH_META_DATA_IDX],
            resource_key_hash(&self.key[HASH_META_DATA_IDX + 1..])
        );
        debug_assert_eq!(self.internal_size() % 4, 0);
    }
}

/// `ResourceKey::Builder`: fills in a key's data, and its hash when dropped or finished.
// Port of: src/gpu/ResourceKey.h#L92-L129 (chrome/m156)
#[doc(alias = "skgpu::ResourceKey::Builder")]
#[derive(Debug)]
pub(crate) struct ResourceKeyBuilder<'a> {
    key: Option<&'a mut ResourceKey>,
}

impl<'a> ResourceKeyBuilder<'a> {
    // Port of: src/gpu/ResourceKey.h#L120-L127 (chrome/m156)
    pub(crate) fn new(key: &'a mut ResourceKey, domain: u16, data32_count: u16) -> Self {
        debug_assert_ne!(domain, INVALID_DOMAIN);
        key.key.clear();
        key.key.resize(META_DATA_CNT + usize::from(data32_count), 0);
        key.key[DOMAIN_AND_SIZE_META_DATA_IDX] =
            u32::from(domain) | (u32::from(data32_count) << 16);
        Self { key: Some(key) }
    }

    // Port of: src/gpu/ResourceKey.h#L97-L104 (chrome/m156)
    /// `finish()`: computes the hash. Calling it again does nothing.
    pub(crate) fn finish(&mut self) {
        if let Some(key) = self.key.take() {
            let hash = resource_key_hash(&key.key[HASH_META_DATA_IDX + 1..]);
            key.key[HASH_META_DATA_IDX] = hash;
            key.validate();
        }
    }
}

impl Drop for ResourceKeyBuilder<'_> {
    // Port of: src/gpu/ResourceKey.h#L95 (chrome/m156)
    fn drop(&mut self) {
        self.finish();
    }
}

impl Index<usize> for ResourceKeyBuilder<'_> {
    type Output = u32;

    // Port of: src/gpu/ResourceKey.h#L105-L112 (chrome/m156)
    fn index(&self, data_idx: usize) -> &u32 {
        let key = self
            .key
            .as_ref()
            .expect("ResourceKey::Builder used after finish");
        &key.key[META_DATA_CNT + data_idx]
    }
}

impl IndexMut<usize> for ResourceKeyBuilder<'_> {
    // Port of: src/gpu/ResourceKey.h#L105-L112 (chrome/m156)
    /// `operator[]`: the `data_idx`-th data word.
    fn index_mut(&mut self, data_idx: usize) -> &mut u32 {
        let key = self
            .key
            .as_mut()
            .expect("ResourceKey::Builder used after finish");
        debug_assert!(data_idx < key.key.len() - META_DATA_CNT);
        &mut key.key[META_DATA_CNT + data_idx]
    }
}

// Port of: src/gpu/ResourceKey.cpp#L14-L25 (chrome/m156)
static NEXT_RESOURCE_TYPE: AtomicU32 = AtomicU32::new(INVALID_DOMAIN as u32 + 1);

// Port of: src/gpu/ResourceKey.cpp#L26-L35 (chrome/m156)
static NEXT_DOMAIN: AtomicU32 = AtomicU32::new(INVALID_DOMAIN as u32 + 1);

/// A key used for scratch resources: any number of resources can share one, and a resource
/// has at most one.
// Port of: src/gpu/ResourceKey.h#L143-L177 (chrome/m156)
#[doc(alias = "skgpu::ScratchKey")]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ScratchKey {
    key: ResourceKey,
}

/// Uniquely identifies the type of resource that is cached as scratch.
#[doc(alias = "skgpu::ScratchKey::ResourceType")]
pub type ScratchResourceType = u16;

impl ScratchKey {
    /// `ScratchKey()`: an invalid scratch key. It must be initialized with a [`ScratchKeyBuilder`].
    // Port of: src/gpu/ResourceKey.h#L154 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `GenerateResourceType()`: a resource type no other call has returned.
    ///
    /// # Panics
    /// If more than `u16::MAX` types are generated (Skia aborts).
    // Port of: src/gpu/ResourceKey.cpp#L14-L25 (chrome/m156)
    #[doc(alias = "GenerateResourceType")]
    #[must_use]
    pub fn generate_resource_type() -> ScratchResourceType {
        let t = NEXT_RESOURCE_TYPE.fetch_add(1, Ordering::Relaxed);
        u16::try_from(t).expect("Too many Resource Types")
    }

    // Port of: src/gpu/ResourceKey.h#L160 (chrome/m156)
    /// `resourceType()`: the key's domain.
    #[must_use]
    pub fn resource_type(&self) -> ScratchResourceType {
        self.key.domain()
    }
}

impl Deref for ScratchKey {
    type Target = ResourceKey;

    fn deref(&self) -> &ResourceKey {
        &self.key
    }
}

/// `ScratchKey::Builder`.
// Port of: src/gpu/ResourceKey.h#L168-L175 (chrome/m156)
#[doc(alias = "skgpu::ScratchKey::Builder")]
#[derive(Debug)]
pub struct ScratchKeyBuilder<'a>(ResourceKeyBuilder<'a>);

impl<'a> ScratchKeyBuilder<'a> {
    /// Starts building `key` for resource type `ty` with `data32_count` data words.
    // Port of: src/gpu/ResourceKey.h#L171-L173 (chrome/m156)
    #[must_use]
    pub fn new(key: &'a mut ScratchKey, ty: ScratchResourceType, data32_count: u16) -> Self {
        Self(ResourceKeyBuilder::new(&mut key.key, ty, data32_count))
    }

    /// `finish()`.
    pub fn finish(&mut self) {
        self.0.finish();
    }
}

impl Index<usize> for ScratchKeyBuilder<'_> {
    type Output = u32;

    fn index(&self, data_idx: usize) -> &u32 {
        &self.0[data_idx]
    }
}

impl IndexMut<usize> for ScratchKeyBuilder<'_> {
    fn index_mut(&mut self, data_idx: usize) -> &mut u32 {
        &mut self.0[data_idx]
    }
}

/// A key that allows exclusive use of a resource for a use case (its "domain"). Only one
/// resource can have a given unique key at a time, and a resource has at most one.
// Port of: src/gpu/ResourceKey.h#L186-L243 (chrome/m156)
#[doc(alias = "skgpu::UniqueKey")]
#[derive(Clone, Debug, Default)]
pub struct UniqueKey {
    key: ResourceKey,
    data: Option<Data>,
    tag: Option<&'static str>,
}

/// The domain of a [`UniqueKey`].
#[doc(alias = "skgpu::UniqueKey::Domain")]
pub type UniqueKeyDomain = u16;

impl UniqueKey {
    /// `UniqueKey()`: an invalid unique key. It must be initialized with a [`UniqueKeyBuilder`].
    // Port of: src/gpu/ResourceKey.h#L196 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `GenerateDomain()`: a domain no other call has returned.
    ///
    /// # Panics
    /// If more than `u16::MAX` domains are generated (Skia aborts).
    // Port of: src/gpu/ResourceKey.cpp#L27-L38 (chrome/m156)
    #[doc(alias = "GenerateDomain")]
    #[must_use]
    pub fn generate_domain() -> UniqueKeyDomain {
        let d = NEXT_DOMAIN.fetch_add(1, Ordering::Relaxed);
        u16::try_from(d).expect("Too many skgpu::UniqueKey Domains")
    }

    // Port of: src/gpu/ResourceKey.h#L201-L206 (chrome/m156)
    /// `setCustomData()`.
    pub fn set_custom_data(&mut self, data: Option<Data>) {
        self.data = data;
    }

    // Port of: src/gpu/ResourceKey.h#L207 (chrome/m156)
    /// `getCustomData()`.
    #[must_use]
    pub fn custom_data(&self) -> Option<&Data> {
        self.data.as_ref()
    }

    // Port of: src/gpu/ResourceKey.h#L208 (chrome/m156)
    /// `refCustomData()`: a new reference to the custom data.
    #[must_use]
    pub fn ref_custom_data(&self) -> Option<Data> {
        self.data.clone()
    }

    // Port of: src/gpu/ResourceKey.h#L209 (chrome/m156)
    /// `tag()`.
    #[must_use]
    pub fn tag(&self) -> Option<&'static str> {
        self.tag
    }

    // Port of: src/gpu/ResourceKey.h#L210 (chrome/m156)
    /// `data()`: the key's data words.
    #[must_use]
    pub fn data(&self) -> &[u32] {
        self.key.data()
    }
}

impl Deref for UniqueKey {
    type Target = ResourceKey;

    fn deref(&self) -> &ResourceKey {
        &self.key
    }
}

impl DerefMut for UniqueKey {
    fn deref_mut(&mut self) -> &mut ResourceKey {
        &mut self.key
    }
}

impl PartialEq for UniqueKey {
    // Port of: src/gpu/ResourceKey.h#L199 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for UniqueKey {}

/// `UniqueKey::Builder`.
// Port of: src/gpu/ResourceKey.h#L213-L224 (chrome/m156)
#[doc(alias = "skgpu::UniqueKey::Builder")]
#[derive(Debug)]
pub struct UniqueKeyBuilder<'a> {
    base: ResourceKeyBuilder<'a>,
}

impl<'a> UniqueKeyBuilder<'a> {
    /// Starts building `key` for `domain` with `data32_count` data words and an optional tag.
    // Port of: src/gpu/ResourceKey.h#L215-L217 (chrome/m156)
    #[must_use]
    pub fn new(
        key: &'a mut UniqueKey,
        domain: UniqueKeyDomain,
        data32_count: u16,
        tag: Option<&'static str>,
    ) -> Self {
        key.tag = tag;
        Self {
            base: ResourceKeyBuilder::new(&mut key.key, domain, data32_count),
        }
    }

    /// Builds a key that wraps `inner_key` and adds `extra_data32_cnt` words of its own. The
    /// inner key is stored after those words: its domain, then its data.
    // Port of: src/gpu/ResourceKey.h#L219-L232 (chrome/m156)
    #[must_use]
    pub fn new_wrapping(
        key: &'a mut UniqueKey,
        inner_key: &UniqueKey,
        domain: UniqueKeyDomain,
        extra_data32_cnt: u16,
        tag: Option<&'static str>,
    ) -> Self {
        let count = data32_cnt_for_inner_key(inner_key, extra_data32_cnt);
        let mut base = ResourceKeyBuilder::new(&mut key.key, domain, count);
        // Add the inner key to the end of the key so that op[] can be indexed normally.
        let extra = usize::from(extra_data32_cnt);
        base[extra] = u32::from(inner_key.domain());
        for (i, w) in inner_key.data().iter().enumerate() {
            base[extra + 1 + i] = *w;
        }
        key.tag = tag;
        Self { base }
    }

    /// `finish()`.
    pub fn finish(&mut self) {
        self.base.finish();
    }
}

impl Index<usize> for UniqueKeyBuilder<'_> {
    type Output = u32;

    fn index(&self, data_idx: usize) -> &u32 {
        &self.base[data_idx]
    }
}

impl IndexMut<usize> for UniqueKeyBuilder<'_> {
    fn index_mut(&mut self, data_idx: usize) -> &mut u32 {
        &mut self.base[data_idx]
    }
}

// Port of: src/gpu/ResourceKey.h#L226-L233 (chrome/m156)
fn data32_cnt_for_inner_key(inner_key: &UniqueKey, extra_data32_cnt: u16) -> u16 {
    // key data + domain + extraData32Cnt needs to fit into a uint16_t.
    let inner_data32_cnt = inner_key.data_size() >> 2;
    let total =
        u32::from(extra_data32_cnt) + u32::try_from(inner_data32_cnt).unwrap_or(u32::MAX) + 1;
    // The Builder API doesn't have a way to return a failure, so if this is somehow exceeded,
    // then we have no way to recover (SkASSERT_RELEASE).
    u16::try_from(total).expect("ResourceKey data does not fit in 16 bits")
}

/// `skgpu_init_static_unique_key_once`: builds a key with a fresh domain and no data.
///
/// Use it to initialize a `static` in place of `SKGPU_DEFINE_STATIC_UNIQUE_KEY`:
/// `static KEY: LazyLock<UniqueKey> = LazyLock::new(init_static_unique_key);`
// Port of: src/gpu/ResourceKey.h#L252-L256 (chrome/m156)
#[must_use]
pub fn init_static_unique_key() -> UniqueKey {
    let mut key = UniqueKey::new();
    {
        let _builder = UniqueKeyBuilder::new(&mut key, UniqueKey::generate_domain(), 0, None);
    }
    key
}

/// The message the cache posts when a unique key is invalidated (`UniqueKeyInvalidatedMessage`).
// Port of: src/gpu/ResourceKey.h#L258-L290 (chrome/m156)
#[doc(alias = "skgpu::UniqueKeyInvalidatedMessage")]
#[derive(Clone, Debug)]
pub struct UniqueKeyInvalidatedMessage {
    key: UniqueKey,
    context_id: u32,
    in_thread_safe_cache: bool,
}

impl Default for UniqueKeyInvalidatedMessage {
    // Port of: src/gpu/ResourceKey.h#L265 (chrome/m156)
    fn default() -> Self {
        Self {
            key: UniqueKey::new(),
            context_id: SK_INVALID_UNIQUE_ID,
            in_thread_safe_cache: false,
        }
    }
}

impl UniqueKeyInvalidatedMessage {
    // Port of: src/gpu/ResourceKey.h#L267-L272 (chrome/m156)
    /// Creates the message for `key` in the context `context_unique_id`.
    #[must_use]
    pub fn new(key: &UniqueKey, context_unique_id: u32, in_thread_safe_cache: bool) -> Self {
        debug_assert_ne!(SK_INVALID_UNIQUE_ID, context_unique_id);
        Self {
            key: key.clone(),
            context_id: context_unique_id,
            in_thread_safe_cache,
        }
    }

    /// `key()`.
    #[must_use]
    pub fn key(&self) -> &UniqueKey {
        &self.key
    }

    /// `contextID()`.
    #[must_use]
    pub fn context_id(&self) -> u32 {
        self.context_id
    }

    /// `inThreadSafeCache()`.
    #[must_use]
    pub fn in_thread_safe_cache(&self) -> bool {
        self.in_thread_safe_cache
    }
}

/// `SkShouldPostMessageToBus` for [`UniqueKeyInvalidatedMessage`].
// Port of: src/gpu/ResourceKey.h#L292-L295 (chrome/m156)
#[must_use]
pub fn should_post_message_to_bus(
    msg: &UniqueKeyInvalidatedMessage,
    msg_bus_unique_id: u32,
) -> bool {
    msg.context_id() == msg_bus_unique_id
}

/// The Graphite variant of [`UniqueKeyInvalidatedMessage`] (`UniqueKeyInvalidatedMsg_Graphite`).
// Port of: src/gpu/ResourceKey.h#L297-L325 (chrome/m156)
#[doc(alias = "skgpu::UniqueKeyInvalidatedMsg_Graphite")]
#[derive(Clone, Debug)]
pub struct UniqueKeyInvalidatedMsgGraphite {
    key: UniqueKey,
    recorder_id: u32,
}

impl Default for UniqueKeyInvalidatedMsgGraphite {
    // Port of: src/gpu/ResourceKey.h#L302 (chrome/m156)
    fn default() -> Self {
        Self {
            key: UniqueKey::new(),
            recorder_id: SK_INVALID_UNIQUE_ID,
        }
    }
}

impl UniqueKeyInvalidatedMsgGraphite {
    // Port of: src/gpu/ResourceKey.h#L304-L307 (chrome/m156)
    /// Creates the message for `key` in the recorder `recorder_id`.
    #[must_use]
    pub fn new(key: &UniqueKey, recorder_id: u32) -> Self {
        debug_assert_ne!(SK_INVALID_UNIQUE_ID, recorder_id);
        Self {
            key: key.clone(),
            recorder_id,
        }
    }

    /// `key()`.
    #[must_use]
    pub fn key(&self) -> &UniqueKey {
        &self.key
    }

    /// `recorderID()`.
    #[must_use]
    pub fn recorder_id(&self) -> u32 {
        self.recorder_id
    }
}

/// `SkShouldPostMessageToBus` for [`UniqueKeyInvalidatedMsgGraphite`].
// Port of: src/gpu/ResourceKey.h#L317-L320 (chrome/m156)
#[must_use]
pub fn should_post_message_to_bus_graphite(
    msg: &UniqueKeyInvalidatedMsgGraphite,
    msg_bus_unique_id: u32,
) -> bool {
    msg.recorder_id() == msg_bus_unique_id
}

/// A key that has a compile-time size and no domain; it can only be used in a dedicated cache.
///
/// Unlike [`UniqueKey`] and [`ScratchKey`], it needs no dynamic allocation.
// Port of: src/gpu/ResourceKey.h#L327-L385 (chrome/m156)
#[doc(alias = "skgpu::FixedSizeKey")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedSizeKey<const SIZE_IN_UINT32: usize> {
    hash: u32,
    packed_data: [u32; SIZE_IN_UINT32],
}

impl<const SIZE_IN_UINT32: usize> Default for FixedSizeKey<SIZE_IN_UINT32> {
    // Port of: src/gpu/ResourceKey.h#L372-L373 (chrome/m156)
    fn default() -> Self {
        Self {
            hash: 0,
            packed_data: [0; SIZE_IN_UINT32],
        }
    }
}

impl<const SIZE_IN_UINT32: usize> FixedSizeKey<SIZE_IN_UINT32> {
    /// `hash()`.
    #[must_use]
    pub fn hash(&self) -> u32 {
        self.hash
    }
}

impl<const SIZE_IN_UINT32: usize> std::hash::Hash for FixedSizeKey<SIZE_IN_UINT32> {
    // Port of: src/gpu/ResourceKey.h#L380 (chrome/m156), the `Hash` functor
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.hash);
    }
}

/// `FixedSizeKey::Builder`: fills the key's words, then its hash on [`finish`](Self::finish).
// Port of: src/gpu/ResourceKey.h#L339-L359 (chrome/m156)
#[doc(alias = "skgpu::FixedSizeKey::Builder")]
#[derive(Debug)]
pub struct FixedSizeKeyBuilder<'a, const SIZE_IN_UINT32: usize> {
    key: Option<&'a mut FixedSizeKey<SIZE_IN_UINT32>>,
}

impl<'a, const SIZE_IN_UINT32: usize> FixedSizeKeyBuilder<'a, SIZE_IN_UINT32> {
    /// Starts building `key`.
    // Port of: src/gpu/ResourceKey.h#L341 (chrome/m156)
    #[must_use]
    pub fn new(key: &'a mut FixedSizeKey<SIZE_IN_UINT32>) -> Self {
        Self { key: Some(key) }
    }

    /// `finish()`: computes the hash of the packed words.
    // Port of: src/gpu/ResourceKey.h#L342-L347 (chrome/m156)
    pub fn finish(&mut self) {
        if let Some(key) = self.key.take() {
            key.hash = resource_key_hash(&key.packed_data);
        }
    }
}

impl<const SIZE_IN_UINT32: usize> Index<usize> for FixedSizeKeyBuilder<'_, SIZE_IN_UINT32> {
    type Output = u32;

    fn index(&self, data_idx: usize) -> &u32 {
        let key = self
            .key
            .as_ref()
            .expect("FixedSizeKey::Builder used after finish");
        &key.packed_data[data_idx]
    }
}

impl<const SIZE_IN_UINT32: usize> IndexMut<usize> for FixedSizeKeyBuilder<'_, SIZE_IN_UINT32> {
    // Port of: src/gpu/ResourceKey.h#L348-L353 (chrome/m156)
    fn index_mut(&mut self, data_idx: usize) -> &mut u32 {
        let key = self
            .key
            .as_mut()
            .expect("FixedSizeKey::Builder used after finish");
        debug_assert!(data_idx < SIZE_IN_UINT32);
        &mut key.packed_data[data_idx]
    }
}

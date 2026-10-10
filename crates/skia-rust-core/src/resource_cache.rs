// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkResourceCache.{h,cpp}, src/core/SkMessageBus (purge-shared-ID
// messages only), src/core/SkSynchronizedResourceCache.h (chrome/m156)

//! A byte-budgeted, least-recently-used cache of records (`SkResourceCache`).
//!
//! Each record ([`Rec`]) carries a [`Key`]. A record is purged when the cache is over its budget
//! and the record reports that it can be purged. A cache built with a [`DiscardableFactory`]
//! instead has no byte budget. It has a count budget, and its data lives in discardable memory.
//!
//! Skia keeps a process-wide cache behind [`ResourceCache::global`]. That cache is a `Mutex`,
//! where Skia's `SkSynchronizedResourceCache` locks.
//!
//! Not ported: the `Rec` payload of `postAddInstall(void*)`, which only the bitmap cache uses,
//! and the memory dump and debug-print facilities.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::cached_data::CachedData;
use crate::checksum::hash32;
use crate::discardable_memory::{DiscardableFactory, DiscardableMemory};
use crate::strike_cache::lock_unpoisoned;

/// The byte limit of the global cache, unless the build overrides it
/// (`SK_DEFAULT_IMAGE_CACHE_LIMIT`).
#[doc(alias = "SK_DEFAULT_IMAGE_CACHE_LIMIT")]
pub const DEFAULT_IMAGE_CACHE_LIMIT: usize = 32 * 1024 * 1024;

/// The count limit of a cache with a discardable factory (`SK_DISCARDABLEMEMORY_SCALEDIMAGECACHE_COUNT_LIMIT`).
#[doc(alias = "SK_DISCARDABLEMEMORY_SCALEDIMAGECACHE_COUNT_LIMIT")]
const DISCARDABLE_COUNT_LIMIT: i32 = 1024;

/// The key of a record (`SkResourceCache::Key`).
///
/// A key is a namespace (unique per kind of record), a shared ID (0 means none, and it allows
/// group purging), and the record's own key data as 32-bit words. Two keys are equal when all of
/// those are equal.
#[doc(alias = "SkResourceCache::Key")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    hash: u32,
    shared_id: u64,
    namespace: usize,
    contents: Vec<u32>,
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The stored hash identifies the key; equal keys have equal hashes.
        state.write_u32(self.hash);
    }
}

impl Key {
    /// Creates a key (`SkResourceCache::Key::init`).
    ///
    /// `namespace` must be unique per kind of record. `shared_id == 0` means the key does not
    /// support group purging. The hash covers the shared ID, the namespace and the contents, in
    /// memory order, as Skia does.
    #[doc(alias = "init")]
    #[must_use]
    pub fn new(namespace: usize, shared_id: u64, contents: &[u32]) -> Self {
        // Port of: SkResourceCache::Key::init (src/core/SkResourceCache.cpp)
        let mut bytes = Vec::with_capacity(8 + std::mem::size_of::<usize>() + contents.len() * 4);
        // fSharedID_lo, fSharedID_hi
        let lo = u32::try_from(shared_id & 0xFFFF_FFFF).unwrap_or_default();
        let hi = u32::try_from(shared_id >> 32).unwrap_or_default();
        bytes.extend_from_slice(&lo.to_ne_bytes());
        bytes.extend_from_slice(&hi.to_ne_bytes());
        // fNamespace
        bytes.extend_from_slice(&namespace.to_ne_bytes());
        for word in contents {
            bytes.extend_from_slice(&word.to_ne_bytes());
        }
        Self {
            hash: hash32(&bytes, 0),
            shared_id,
            namespace,
            contents: contents.to_vec(),
        }
    }

    /// Returns the size of the key in bytes (`size`).
    #[must_use]
    pub fn size(&self) -> usize {
        // The local words (count, hash, shared ID, namespace) plus the contents.
        (6 + self.contents.len()) * 4
    }

    /// Returns the namespace (`getNamespace`).
    #[must_use]
    pub fn namespace(&self) -> usize {
        self.namespace
    }

    /// Returns the shared ID (`getSharedID`).
    #[must_use]
    pub fn shared_id(&self) -> u64 {
        self.shared_id
    }

    /// Returns the hash (`hash`).
    #[must_use]
    pub fn hash(&self) -> u32 {
        self.hash
    }

    /// Returns the key's contents words.
    #[must_use]
    pub fn contents(&self) -> &[u32] {
        &self.contents
    }
}

/// A record in the cache (`SkResourceCache::Rec`).
///
/// The record is owned by the cache from the moment it is added, and is dropped when it is purged.
#[doc(alias = "SkResourceCache::Rec")]
pub trait Rec: Send {
    /// Returns the record's key (`getKey`).
    fn key(&self) -> &Key;

    /// Returns the bytes the record counts against the budget (`bytesUsed`).
    fn bytes_used(&self) -> usize;

    /// Returns whether the cache may purge the record now (`canBePurged`).
    fn can_be_purged(&mut self) -> bool {
        true
    }

    /// Called after the record is installed, or when an identical record is already present and
    /// kept instead (`postAddInstall`).
    fn post_add_install(&mut self) {}

    /// Returns the category name, for diagnostics (`getCategory`).
    fn category(&self) -> &'static str;
}

/// The visitor of [`ResourceCache::find`]. It returns `true` if the record is valid, and `false`
/// if the record is stale and must be purged (`FindVisitor`).
pub type FindVisitor<'a> = dyn FnOnce(&dyn Rec) -> bool + 'a;

/// A record's place in the cache's list.
struct Slot {
    rec: Box<dyn Rec>,
    prev: Option<usize>,
    next: Option<usize>,
}

/// The inboxes of the purge-shared-ID messages, all of which receive every message. Each cache
/// holds one; a dropped cache's inbox is pruned on the next post.
static PURGE_BUS: Mutex<Vec<Weak<Mutex<Vec<u64>>>>> = Mutex::new(Vec::new());

/// The process-wide cache.
static GLOBAL_CACHE: OnceLock<Mutex<ResourceCache>> = OnceLock::new();

/// Posts a message to purge every record with `shared_id`, to all caches
/// (`SkResourceCache::PostPurgeSharedID`).
#[doc(alias = "SkResourceCache::PostPurgeSharedID")]
pub fn post_purge_shared_id(shared_id: u64) {
    if shared_id == 0 {
        return;
    }
    let mut bus = lock_unpoisoned(&PURGE_BUS);
    bus.retain(|inbox| inbox.strong_count() > 0);
    for inbox in bus.iter().filter_map(Weak::upgrade) {
        lock_unpoisoned(&inbox).push(shared_id);
    }
}

/// A cache of records with a byte budget, or with a discardable factory (`SkResourceCache`).
#[doc(alias = "SkResourceCache")]
pub struct ResourceCache {
    slots: Vec<Option<Slot>>,
    free: Vec<usize>,
    /// The most recently used record.
    head: Option<usize>,
    /// The least recently used record.
    tail: Option<usize>,
    hash: HashMap<Key, usize>,
    discardable_factory: Option<DiscardableFactory>,
    total_bytes_used: usize,
    total_byte_limit: usize,
    single_allocation_byte_limit: usize,
    count: i32,
    inbox: Arc<Mutex<Vec<u64>>>,
}

impl std::fmt::Debug for ResourceCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceCache")
            .field("count", &self.count)
            .field("total_bytes_used", &self.total_bytes_used)
            .field("total_byte_limit", &self.total_byte_limit)
            .finish_non_exhaustive()
    }
}

impl ResourceCache {
    /// Creates a cache with a byte budget, purging automatically when an added record pushes the
    /// total over `byte_limit` (`SkResourceCache(size_t)`).
    #[doc(alias = "SkResourceCache")]
    #[must_use]
    pub fn new(byte_limit: usize) -> Self {
        Self::with_parts(None, byte_limit)
    }

    /// Creates a cache whose data is allocated with `factory`. It has no byte budget, so
    /// [`ResourceCache::total_byte_limit`] is 0 and [`ResourceCache::set_total_byte_limit`]
    /// ignores its argument (`SkResourceCache(DiscardableFactory)`).
    #[doc(alias = "SkResourceCache")]
    #[must_use]
    pub fn with_discardable_factory(factory: DiscardableFactory) -> Self {
        Self::with_parts(Some(factory), 0)
    }

    fn with_parts(discardable_factory: Option<DiscardableFactory>, byte_limit: usize) -> Self {
        let inbox = Arc::new(Mutex::new(Vec::new()));
        lock_unpoisoned(&PURGE_BUS).push(Arc::downgrade(&inbox));
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            head: None,
            tail: None,
            hash: HashMap::new(),
            discardable_factory,
            total_bytes_used: 0,
            total_byte_limit: byte_limit,
            single_allocation_byte_limit: 0,
            count: 0,
            inbox,
        }
    }

    /// Returns the process-wide cache (the static `SkResourceCache` methods). Lock it to use it.
    #[must_use]
    pub fn global() -> &'static Mutex<Self> {
        GLOBAL_CACHE.get_or_init(|| Mutex::new(Self::new(DEFAULT_IMAGE_CACHE_LIMIT)))
    }

    /// Returns the total bytes of the records (`getTotalBytesUsed`).
    #[must_use]
    pub fn total_bytes_used(&self) -> usize {
        self.total_bytes_used
    }

    /// Returns the byte budget (`getTotalByteLimit`).
    #[must_use]
    pub fn total_byte_limit(&self) -> usize {
        self.total_byte_limit
    }

    /// Sets the byte budget, purging to fit a smaller one, and returns the previous budget
    /// (`setTotalByteLimit`).
    pub fn set_total_byte_limit(&mut self, new_limit: usize) -> usize {
        let prev_limit = self.total_byte_limit;
        self.total_byte_limit = new_limit;
        if new_limit < prev_limit {
            self.purge_as_needed(false);
        }
        prev_limit
    }

    /// Sets the largest single allocation the cache accepts, and returns the previous one. 0 means
    /// no maximum (`setSingleAllocationByteLimit`).
    pub fn set_single_allocation_byte_limit(&mut self, maximum_allocation_size: usize) -> usize {
        std::mem::replace(
            &mut self.single_allocation_byte_limit,
            maximum_allocation_size,
        )
    }

    /// Returns the single allocation limit (`getSingleAllocationByteLimit`).
    #[must_use]
    pub fn single_allocation_byte_limit(&self) -> usize {
        self.single_allocation_byte_limit
    }

    /// Returns the single allocation limit that applies, capped to the budget for a byte-budgeted
    /// cache (`getEffectiveSingleAllocationByteLimit`).
    #[must_use]
    pub fn effective_single_allocation_byte_limit(&self) -> usize {
        // 0 means the default: the budget.
        let limit = self.single_allocation_byte_limit;
        if self.discardable_factory.is_none() {
            if limit == 0 {
                return self.total_byte_limit;
            }
            return limit.min(self.total_byte_limit);
        }
        limit
    }

    /// Returns the discardable factory, if the cache has one (`discardableFactory`).
    #[must_use]
    pub fn discardable_factory(&self) -> Option<DiscardableFactory> {
        self.discardable_factory
    }

    /// Creates data of `bytes` for this cache (`newCachedData`). With a discardable factory, the
    /// data is backed by discardable memory, and `None` means the factory failed.
    #[must_use]
    pub fn new_cached_data(&mut self, bytes: usize) -> Option<Arc<CachedData>> {
        self.check_messages();
        match self.discardable_factory {
            Some(factory) => {
                let dm: Box<dyn DiscardableMemory> = factory(bytes)?;
                Some(CachedData::new_discardable(bytes, dm))
            }
            None => Some(CachedData::new_malloc(vec![0_u8; bytes])),
        }
    }

    /// Looks up `key`. If a record matches, calls `visitor` with it. Returns the visitor's result;
    /// `false` also when no record matches. A `false` from the visitor purges the record
    /// (`find`).
    ///
    /// # Panics
    ///
    /// Only if the cache's internal links are broken, which no call of this API does.
    pub fn find<F>(&mut self, key: &Key, visitor: F) -> bool
    where
        F: FnOnce(&dyn Rec) -> bool,
    {
        self.check_messages();
        let Some(&idx) = self.hash.get(key) else {
            return false;
        };
        let valid = {
            let slot = self.slots[idx].as_ref().expect("hashed slot is live");
            visitor(&*slot.rec)
        };
        if valid {
            self.move_to_head(idx); // for our LRU
            true
        } else {
            self.remove(idx); // stale
            false
        }
    }

    /// Adds a record (`add`). If a record with an equal key is already present, that record is
    /// replaced when it can be purged; otherwise it is kept, and the new record is dropped.
    ///
    /// # Panics
    ///
    /// Only if the cache's internal links are broken, which no call of this API does.
    pub fn add(&mut self, rec: Box<dyn Rec>) {
        self.check_messages();
        if let Some(&preexisting) = self.hash.get(rec.key()) {
            let slot = self.slots[preexisting].as_mut().expect("hashed slot is live");
            if slot.rec.can_be_purged() {
                // If it can be purged, the install may fail, so we have to remove it.
                self.remove(preexisting);
            } else {
                // If it cannot be purged, we reuse it and delete the new one.
                slot.rec.post_add_install();
                return;
            }
        }
        let key = rec.key().clone();
        let bytes = rec.bytes_used();
        let idx = self.add_to_head(rec, bytes);
        self.hash.insert(key, idx);
        self.slots[idx]
            .as_mut()
            .expect("slot was just added")
            .rec
            .post_add_install();
        // Since the new rec may push us over budget, we perform a purge check now.
        self.purge_as_needed(false);
    }

    /// Calls `visitor` on every record, from the least recently used (`visitAll`).
    ///
    /// # Panics
    ///
    /// Only if the cache's internal links are broken, which no call of this API does.
    pub fn visit_all(&self, mut visitor: impl FnMut(&dyn Rec)) {
        let mut cur = self.tail;
        while let Some(idx) = cur {
            let slot = self.slots[idx].as_ref().expect("listed slot is live");
            visitor(&*slot.rec);
            cur = slot.prev;
        }
    }

    /// Purges every record that can be purged, as a forced purge (`purgeAll`).
    pub fn purge_all(&mut self) {
        self.purge_as_needed(true);
    }

    /// Purges every record with `shared_id` that can be purged (`purgeSharedID`).
    ///
    /// # Panics
    ///
    /// Only if the cache's internal links are broken, which no call of this API does.
    pub fn purge_shared_id(&mut self, shared_id: u64) {
        if shared_id == 0 {
            return;
        }
        // Go backwards, as purge_as_needed does; either direction is correct.
        let mut cur = self.tail;
        while let Some(idx) = cur {
            let slot = self.slots[idx].as_mut().expect("listed slot is live");
            let prev = slot.prev;
            if slot.rec.key().shared_id() == shared_id && slot.rec.can_be_purged() {
                // Even though the source is now dead, caches could still be in flight, so we
                // have to check whether it can be removed.
                self.remove(idx);
            }
            cur = prev;
        }
    }

    /// Processes the purge-shared-ID messages that are waiting (`checkMessages`).
    fn check_messages(&mut self) {
        let msgs = std::mem::take(&mut *lock_unpoisoned(&self.inbox));
        for shared_id in msgs {
            self.purge_shared_id(shared_id);
        }
    }

    /// Purges from the tail while the cache is over budget, or always when `force_purge`
    /// (`purgeAsNeeded`).
    fn purge_as_needed(&mut self, force_purge: bool) {
        let (byte_limit, count_limit) = if self.discardable_factory.is_some() {
            // No limit based on bytes.
            (u32::MAX as usize, DISCARDABLE_COUNT_LIMIT)
        } else {
            // No limit based on count.
            (self.total_byte_limit, i32::MAX)
        };
        let mut cur = self.tail;
        while let Some(idx) = cur {
            if !force_purge && self.total_bytes_used < byte_limit && self.count < count_limit {
                break;
            }
            let slot = self.slots[idx].as_mut().expect("listed slot is live");
            let prev = slot.prev;
            if slot.rec.can_be_purged() {
                self.remove(idx);
            }
            cur = prev;
        }
    }

    /// Takes `idx` off the list (`release`).
    fn release(&mut self, idx: usize) {
        let (prev, next) = {
            let slot = self.slots[idx].as_ref().expect("released slot is live");
            (slot.prev, slot.next)
        };
        match prev {
            None => {
                debug_assert_eq!(self.head, Some(idx));
                self.head = next;
            }
            Some(prev) => self.slots[prev].as_mut().expect("linked slot is live").next = next,
        }
        match next {
            None => self.tail = prev,
            Some(next) => self.slots[next].as_mut().expect("linked slot is live").prev = prev,
        }
        let slot = self.slots[idx].as_mut().expect("released slot is live");
        slot.prev = None;
        slot.next = None;
    }

    /// Moves `idx` to the head of the list (`moveToHead`).
    fn move_to_head(&mut self, idx: usize) {
        if self.head == Some(idx) {
            return;
        }
        self.release(idx);
        let old_head = self.head.expect("a list with a record other than idx has a head");
        self.slots[old_head].as_mut().expect("head is live").prev = Some(idx);
        let slot = self.slots[idx].as_mut().expect("moved slot is live");
        slot.next = Some(old_head);
        self.head = Some(idx);
    }

    /// Adds `rec` at the head of the list, and counts it (`addToHead`).
    fn add_to_head(&mut self, rec: Box<dyn Rec>, bytes: usize) -> usize {
        let slot = Slot {
            rec,
            prev: None,
            next: self.head,
        };
        let idx = if let Some(idx) = self.free.pop() {
            self.slots[idx] = Some(slot);
            idx
        } else {
            self.slots.push(Some(slot));
            self.slots.len() - 1
        };
        if let Some(old_head) = self.head {
            self.slots[old_head].as_mut().expect("head is live").prev = Some(idx);
        }
        self.head = Some(idx);
        if self.tail.is_none() {
            self.tail = Some(idx);
        }
        self.total_bytes_used += bytes;
        self.count += 1;
        idx
    }

    /// Removes `idx` from the cache and drops the record (`remove`).
    fn remove(&mut self, idx: usize) {
        let used = self.slots[idx].as_ref().expect("removed slot is live").rec.bytes_used();
        debug_assert!(used <= self.total_bytes_used);
        self.release(idx);
        let slot = self.slots[idx].take().expect("removed slot is live");
        self.hash.remove(slot.rec.key());
        self.free.push(idx);
        self.total_bytes_used -= used;
        self.count -= 1;
        // The record is dropped here (`delete rec`).
        drop(slot);
    }
}


// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkLRUCache.h (chrome/m156)

//! A generic LRU cache with an optional purge callback.
//!
//! Entries are kept in most-recently-used order. [`LruCache::find`] and the insert operations move
//! an entry to the head; when an insert takes the count above the limit, the tail (the least
//! recently used entry) is removed. The purge callback runs for every entry removed by
//! [`LruCache::remove`] and by eviction, but not by [`LruCache::reset`], and not when the cache is
//! dropped, as in Skia.

use std::collections::HashMap;
use std::hash::Hash;

/// The purge callback: `(context, key, value)`, run before an entry is removed.
pub type PurgeCallback<C, K, V> = fn(&mut C, &K, &mut V);

struct Node<K, V> {
    key: K,
    value: V,
    prev: Option<usize>,
    next: Option<usize>,
}

/// A generic LRU cache (`SkLRUCache<K, V, HashK, PurgeCB>`).
///
/// `C` is the context passed to the purge callback (Skia's `void* context`). It is returned by
/// [`LruCache::context`], so a cache can keep the statistics its purge callback updates.
// Port of: src/core/SkLRUCache.h#L19-L121 (chrome/m156)
#[doc(alias = "SkLRUCache")]
pub struct LruCache<K, V, C = ()> {
    max_count: usize,
    /// Key to slot index.
    map: HashMap<K, usize>,
    /// Slots of the intrusive doubly-linked list; `None` marks a free slot.
    slots: Vec<Option<Node<K, V>>>,
    /// Indices of free slots, reused before the vector grows.
    free: Vec<usize>,
    /// Most recently used slot.
    head: Option<usize>,
    /// Least recently used slot.
    tail: Option<usize>,
    context: C,
    purge: Option<PurgeCallback<C, K, V>>,
}

impl<K, V, C> std::fmt::Debug for LruCache<K, V, C>
where
    K: Clone + Eq + Hash,
    C: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LruCache")
            .field("max_count", &self.max_count)
            .field("count", &self.count())
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl<K, V> LruCache<K, V, ()>
where
    K: Clone + Eq + Hash,
{
    /// `SkLRUCache(maxCount)` with no purge callback.
    #[must_use]
    pub fn new(max_count: usize) -> Self {
        Self::with_purge(max_count, (), None)
    }
}

impl<K, V, C> LruCache<K, V, C>
where
    K: Clone + Eq + Hash,
{
    /// `SkLRUCache(maxCount, context)` with a purge callback.
    ///
    /// # Panics
    /// If `max_count` is zero (Skia's cache would evict the entry it has just inserted).
    // Port of: src/core/SkLRUCache.h#L35-L37 (chrome/m156)
    #[must_use]
    pub fn with_purge(max_count: usize, context: C, purge: Option<PurgeCallback<C, K, V>>) -> Self {
        assert!(max_count > 0, "an LRU cache needs room for one entry");
        Self {
            max_count,
            map: HashMap::new(),
            slots: Vec::new(),
            free: Vec::new(),
            head: None,
            tail: None,
            context,
            purge,
        }
    }

    /// The context the purge callback receives.
    #[must_use]
    pub fn context(&self) -> &C {
        &self.context
    }

    /// The context the purge callback receives, mutably.
    pub fn context_mut(&mut self) -> &mut C {
        &mut self.context
    }

    /// `find(key)`: the value for `key`, which becomes the most recently used entry.
    // Port of: src/core/SkLRUCache.h#L51-L62 (chrome/m156)
    pub fn find(&mut self, key: &K) -> Option<&mut V> {
        let index = *self.map.get(key)?;
        self.move_to_head(index);
        self.slots[index].as_mut().map(|node| &mut node.value)
    }

    /// `insert(key, value)`: adds an entry that must not be present, as the most recently used,
    /// and evicts from the tail while the count exceeds the limit.
    // Port of: src/core/SkLRUCache.h#L64-L78 (chrome/m156)
    pub fn insert(&mut self, key: K, value: V) -> &mut V {
        debug_assert!(
            !self.map.contains_key(&key),
            "the key is already in the cache"
        );
        let index = self.alloc(Node {
            key: key.clone(),
            value,
            prev: None,
            next: None,
        });
        self.map.insert(key, index);
        self.push_head(index);
        while self.map.len() > self.max_count {
            let tail_key = self.slots[self.tail.expect("a non-empty cache has a tail")]
                .as_ref()
                .expect("the tail is occupied")
                .key
                .clone();
            self.remove(&tail_key);
        }
        &mut self.slots[index]
            .as_mut()
            .expect("the new entry is occupied")
            .value
    }

    /// `insert_or_update(key, value)`: replaces the value of an existing entry (which becomes the
    /// most recently used), or inserts a new one.
    // Port of: src/core/SkLRUCache.h#L80-L86 (chrome/m156)
    pub fn insert_or_update(&mut self, key: K, value: V) -> &mut V {
        if let Some(found) = self.find(&key) {
            *found = value;
            // `find` moved the entry to the head; the borrow ends before the return below.
            let index = self.map[&key];
            return &mut self.slots[index]
                .as_mut()
                .expect("the entry is occupied")
                .value;
        }
        self.insert(key, value)
    }

    /// `count()`: the number of entries.
    #[must_use]
    pub fn count(&self) -> usize {
        self.map.len()
    }

    /// `foreach(fn)`: calls `f` for every entry, from the most to the least recently used.
    // Port of: src/core/SkLRUCache.h#L100-L107 (chrome/m156)
    pub fn foreach(&mut self, mut f: impl FnMut(&K, &mut V)) {
        let mut next = self.head;
        while let Some(index) = next {
            let node = self.slots[index]
                .as_mut()
                .expect("linked slots are occupied");
            f(&node.key, &mut node.value);
            next = node.next;
        }
    }

    /// `reset()`: removes every entry without running the purge callback.
    // Port of: src/core/SkLRUCache.h#L109-L115 (chrome/m156)
    pub fn reset(&mut self) {
        self.map.clear();
        self.slots.clear();
        self.free.clear();
        self.head = None;
        self.tail = None;
    }

    /// `remove(key)`: runs the purge callback for the entry, then removes it.
    ///
    /// # Panics
    /// In debug builds, if `key` is not in the cache (Skia asserts the same).
    // Port of: src/core/SkLRUCache.h#L117-L128 (chrome/m156)
    pub fn remove(&mut self, key: &K) {
        let Some(index) = self.map.get(key).copied() else {
            debug_assert!(false, "removing a key that is not in the cache");
            return;
        };
        if let Some(purge) = self.purge {
            let node = self.slots[index]
                .as_mut()
                .expect("mapped slots are occupied");
            purge(&mut self.context, &node.key, &mut node.value);
        }
        self.map.remove(key);
        self.unlink(index);
        self.slots[index] = None;
        self.free.push(index);
    }

    fn alloc(&mut self, node: Node<K, V>) -> usize {
        if let Some(index) = self.free.pop() {
            self.slots[index] = Some(node);
            index
        } else {
            self.slots.push(Some(node));
            self.slots.len() - 1
        }
    }

    fn node(&mut self, index: usize) -> &mut Node<K, V> {
        self.slots[index]
            .as_mut()
            .expect("linked slots are occupied")
    }

    /// Detaches a linked slot from the list, leaving its own links stale.
    fn unlink(&mut self, index: usize) {
        let (prev, next) = {
            let node = self.node(index);
            (node.prev, node.next)
        };
        match prev {
            Some(p) => self.node(p).next = next,
            None => self.head = next,
        }
        match next {
            Some(n) => self.node(n).prev = prev,
            None => self.tail = prev,
        }
        let node = self.node(index);
        node.prev = None;
        node.next = None;
    }

    fn push_head(&mut self, index: usize) {
        let old_head = self.head;
        {
            let node = self.node(index);
            node.prev = None;
            node.next = old_head;
        }
        match old_head {
            Some(h) => self.node(h).prev = Some(index),
            None => self.tail = Some(index),
        }
        self.head = Some(index);
    }

    fn move_to_head(&mut self, index: usize) {
        if self.head != Some(index) {
            self.unlink(index);
            self.push_head(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LruCache;
    use std::cell::RefCell;

    thread_local! {
        static PURGED: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
    }

    // The purge callback takes its key by reference, as Skia's does, whatever the key's size.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn record_purge(context: &mut u32, key: &u32, value: &mut u32) {
        *context += 1;
        PURGED.with(|p| p.borrow_mut().push((*key, *value)));
    }

    fn order(cache: &mut LruCache<u32, u32, u32>) -> Vec<u32> {
        let mut keys = Vec::new();
        cache.foreach(|k, _| keys.push(*k));
        keys
    }

    #[test]
    fn evicts_least_recently_used_and_purges_in_order() {
        PURGED.with(|p| p.borrow_mut().clear());
        let mut cache: LruCache<u32, u32, u32> = LruCache::with_purge(3, 0, Some(record_purge));
        cache.insert(1, 10);
        cache.insert(2, 20);
        cache.insert(3, 30);
        assert_eq!(order(&mut cache), vec![3, 2, 1]);

        // Touching 1 makes 2 the least recently used.
        assert_eq!(cache.find(&1).copied(), Some(10));
        assert_eq!(order(&mut cache), vec![1, 3, 2]);

        cache.insert(4, 40);
        assert_eq!(cache.count(), 3);
        assert_eq!(order(&mut cache), vec![4, 1, 3]);
        assert_eq!(*cache.context(), 1);
        PURGED.with(|p| assert_eq!(*p.borrow(), vec![(2, 20)]));

        cache.insert(5, 50);
        cache.insert(6, 60);
        PURGED.with(|p| assert_eq!(*p.borrow(), vec![(2, 20), (3, 30), (1, 10)]));
        assert_eq!(order(&mut cache), vec![6, 5, 4]);
        assert_eq!(*cache.context(), 3);
    }

    #[test]
    fn remove_runs_purge_and_reset_does_not() {
        PURGED.with(|p| p.borrow_mut().clear());
        let mut cache: LruCache<u32, u32, u32> = LruCache::with_purge(4, 0, Some(record_purge));
        cache.insert(1, 10);
        cache.insert(2, 20);
        cache.insert(3, 30);
        cache.remove(&2);
        assert_eq!(order(&mut cache), vec![3, 1]);
        PURGED.with(|p| assert_eq!(*p.borrow(), vec![(2, 20)]));

        cache.reset();
        assert_eq!(cache.count(), 0);
        assert_eq!(*cache.context(), 1);
        PURGED.with(|p| assert_eq!(*p.borrow(), vec![(2, 20)]));
    }

    #[test]
    fn freed_slots_are_reused_and_lists_stay_consistent() {
        let mut cache: LruCache<u32, u32> = LruCache::new(2);
        for round in 0..100u32 {
            cache.insert(round * 2, round);
            cache.insert(round * 2 + 1, round);
            assert_eq!(cache.count(), 2);
        }
        let mut keys = Vec::new();
        cache.foreach(|k, _| keys.push(*k));
        assert_eq!(keys, vec![199, 198]);
        assert!(cache.slots.len() <= 3);
    }

    #[test]
    fn insert_or_update_replaces_and_promotes() {
        let mut cache: LruCache<u32, u32> = LruCache::new(3);
        cache.insert(1, 10);
        cache.insert(2, 20);
        assert_eq!(*cache.insert_or_update(1, 11), 11);
        assert_eq!(*cache.insert_or_update(3, 30), 30);
        let mut keys = Vec::new();
        cache.foreach(|k, v| keys.push((*k, *v)));
        assert_eq!(keys, vec![(3, 30), (1, 11), (2, 20)]);
    }

    #[test]
    fn find_misses_do_not_reorder() {
        let mut cache: LruCache<u32, u32> = LruCache::new(2);
        cache.insert(1, 10);
        cache.insert(2, 20);
        assert!(cache.find(&9).is_none());
        let mut keys = Vec::new();
        cache.foreach(|k, _| keys.push(*k));
        assert_eq!(keys, vec![2, 1]);
    }

    #[test]
    fn dropping_the_cache_does_not_purge() {
        PURGED.with(|p| p.borrow_mut().clear());
        {
            let mut cache: LruCache<u32, u32, u32> = LruCache::with_purge(2, 0, Some(record_purge));
            cache.insert(7, 70);
        }
        PURGED.with(|p| assert!(p.borrow().is_empty()));
    }
}

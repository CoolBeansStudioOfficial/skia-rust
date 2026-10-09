// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTHash.h

//! `skia_private::THashMap`: Skia's open-addressing hash map.
//!
//! Most of the port uses `std::collections::HashMap`. This map exists for the places where Skia's
//! *iteration order* is observable: `foreach` visits the slots of a linear-probing table (probing
//! backwards, power-of-two capacity, grown at 3/4 load and shrunk at 1/4), so the order depends on
//! the hashes and on the history of insertions and removals. Graphite's `ProxyCache`, for example,
//! frees proxies in `foreach` order, which decides the order resources return to the resource
//! cache and therefore which ones it purges first.
//!
//! The hash is supplied by the caller (Skia's `Traits::Hash`), so the slot layout is the same as
//! in C++ for the same hash values.

/// One occupied slot: the stored (non-zero) hash, the key and the value.
// Port of: src/core/SkTHash.h#L418-L504 (chrome/m156)
#[derive(Clone, Debug)]
struct Slot<K, V> {
    hash: u32,
    key: K,
    value: V,
}

/// `skia_private::THashMap<K, V, HashK>`, backed by a port of `THashTable`.
// Port of: src/core/SkTHash.h#L36-L505 (chrome/m156), THashMap at #L510-L625
#[doc(alias = "THashTable")]
#[derive(Clone, Debug)]
pub struct THashMap<K, V> {
    count: usize,
    slots: Vec<Option<Slot<K, V>>>,
    hash_fn: fn(&K) -> u32,
}

impl<K: PartialEq, V> THashMap<K, V> {
    /// An empty map that hashes keys with `hash_fn` (Skia's `HashK`).
    #[must_use]
    pub fn new(hash_fn: fn(&K) -> u32) -> Self {
        Self {
            count: 0,
            slots: Vec::new(),
            hash_fn,
        }
    }

    /// `count()`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// `capacity()`.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// True if the map holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// `reset()`: clears the table (capacity back to 0).
    // Port of: src/core/SkTHash.h#L67-L68 (chrome/m156)
    pub fn reset(&mut self) {
        self.count = 0;
        self.slots = Vec::new();
    }

    // Port of: src/core/SkTHash.h#L403-L406 (chrome/m156)
    fn hash(&self, key: &K) -> u32 {
        let hash = (self.hash_fn)(key);
        // We reserve hash 0 to mark empty.
        if hash == 0 { 1 } else { hash }
    }

    // Port of: src/core/SkTHash.h#L397-L401 (chrome/m156)
    fn next(&self, index: usize) -> usize {
        if index == 0 {
            self.slots.len() - 1
        } else {
            index - 1
        }
    }

    // `hash & (fCapacity-1)`; only called with a non-zero capacity.
    fn home(&self, hash: u32) -> usize {
        (hash as usize) & (self.slots.len() - 1)
    }

    /// `set(key, value)`: inserts or overwrites, returning the stored value.
    ///
    /// # Panics
    /// Never in practice: the table always grows before it is full.
    // Port of: src/core/SkTHash.h#L104-L116 (chrome/m156), THashMap::set at #L560-L564
    pub fn set(&mut self, key: K, value: V) -> &mut V {
        if 4 * self.count >= 3 * self.slots.len() {
            let capacity = if self.slots.is_empty() {
                4
            } else {
                self.slots.len() * 2
            };
            self.resize(capacity);
        }
        let index = self.unchecked_set(key, value);
        &mut self.slots[index].as_mut().expect("slot just set").value
    }

    // Returns the slot index the entry was written to.
    // Port of: src/core/SkTHash.h#L333-L356 (chrome/m156)
    fn unchecked_set(&mut self, key: K, value: V) -> usize {
        let hash = self.hash(&key);
        let mut index = self.home(hash);
        for _ in 0..self.slots.len() {
            match &self.slots[index] {
                None => {
                    // New entry.
                    self.slots[index] = Some(Slot { hash, key, value });
                    self.count += 1;
                    return index;
                }
                Some(s) if s.hash == hash && s.key == key => {
                    // Overwrite previous entry.
                    self.slots[index] = Some(Slot { hash, key, value });
                    return index;
                }
                Some(_) => {}
            }
            index = self.next(index);
        }
        unreachable!("THashTable is full");
    }

    // Port of: src/core/SkTHash.h#L118-L134 (chrome/m156)
    fn find_index(&self, key: &K) -> Option<usize> {
        if self.slots.is_empty() {
            return None;
        }
        let hash = self.hash(key);
        let mut index = self.home(hash);
        for _ in 0..self.slots.len() {
            match &self.slots[index] {
                None => return None,
                Some(s) if s.hash == hash && s.key == *key => return Some(index),
                Some(_) => {}
            }
            index = self.next(index);
        }
        debug_assert_eq!(self.slots.len(), self.count);
        None
    }

    /// `find(key)`.
    ///
    /// # Panics
    /// Never in practice (the found slot is occupied).
    // Port of: src/core/SkTHash.h#L118-L134 (chrome/m156)
    #[must_use]
    pub fn find(&self, key: &K) -> Option<&V> {
        self.find_index(key)
            .map(|i| &self.slots[i].as_ref().expect("found slot").value)
    }

    /// `find(key)`, mutable.
    ///
    /// # Panics
    /// Never in practice (the found slot is occupied).
    pub fn find_mut(&mut self, key: &K) -> Option<&mut V> {
        self.find_index(key)
            .map(|i| &mut self.slots[i].as_mut().expect("found slot").value)
    }

    /// `removeIfExists(key)`: removes the entry and returns its value, if present.
    // Port of: src/core/SkTHash.h#L145-L172 (chrome/m156)
    #[doc(alias = "removeIfExists")]
    pub fn remove_if_exists(&mut self, key: &K) -> Option<V> {
        let index = self.find_index(key)?;
        let removed = self.remove_slot(index);
        if self.slots.len() > 4 && 4 * self.count <= self.slots.len() {
            self.resize(self.slots.len() / 2);
        }
        Some(removed)
    }

    /// `remove(key)`: asserts the key is present.
    // Port of: src/core/SkTHash.h#L175-L178 (chrome/m156)
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let removed = self.remove_if_exists(key);
        debug_assert!(removed.is_some());
        removed
    }

    // Port of: src/core/SkTHash.h#L192-L214 (chrome/m156)
    fn resize(&mut self, capacity: usize) {
        debug_assert!(capacity >= self.count);
        debug_assert!(capacity.is_power_of_two());
        let old_slots = std::mem::take(&mut self.slots);
        self.count = 0;
        self.slots.resize_with(capacity, || None);
        for s in old_slots.into_iter().flatten() {
            self.unchecked_set(s.key, s.value);
        }
    }

    // Removes the entry at `index` and restores the linear-probing invariants.
    // Port of: src/core/SkTHash.h#L358-L395 (chrome/m156)
    fn remove_slot(&mut self, mut index: usize) -> V {
        self.count -= 1;
        let removed = self.slots[index].take().expect("removing an empty slot");

        // Rearrange elements to restore the invariants for linear probing.
        loop {
            let empty_index = index;
            let mut original_index;
            // Look for an element that can be moved into the empty slot.
            // If the empty slot is in between where an element landed, and its native slot, then
            // move it to the empty slot. Don't move it if its native slot is in between where
            // the element landed and the empty slot.
            // [native] <= [empty] < [candidate] == GOOD, can move candidate to empty slot
            // [empty] < [native] < [candidate] == BAD, need to leave candidate where it is
            loop {
                index = self.next(index);
                match &self.slots[index] {
                    // We're done shuffling elements around.
                    None => return removed.value,
                    Some(s) => original_index = self.home(s.hash),
                }
                #[allow(clippy::nonminimal_bool)] // Skia's three cases, kept as written
                let keep_looking = (index <= original_index && original_index < empty_index)
                    || (original_index < empty_index && empty_index < index)
                    || (empty_index < index && index <= original_index);
                if !keep_looking {
                    break;
                }
            }
            // Move the element to the empty slot.
            self.slots[empty_index] = self.slots[index].take();
        }
    }

    /// `foreach(fn(key, value))`, in slot order.
    // Port of: src/core/SkTHash.h#L239-L247 (chrome/m156)
    pub fn foreach(&self, mut f: impl FnMut(&K, &V)) {
        for s in self.slots.iter().flatten() {
            f(&s.key, &s.value);
        }
    }

    /// Iterates the entries in slot order (Skia's `begin()`/`end()`).
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.slots.iter().flatten().map(|s| (&s.key, &s.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::trivially_copy_pass_by_ref)] // the map's hash takes &K
    fn id_hash(k: &u32) -> u32 {
        *k
    }

    #[test]
    fn grows_and_keeps_slot_order() {
        let mut m = THashMap::new(id_hash);
        for k in [1u32, 2, 3] {
            m.set(k, k * 10);
        }
        assert_eq!(m.capacity(), 4);
        // Fourth insert grows to 8 (3 * 4 >= 3 * 4).
        m.set(5, 50);
        assert_eq!(m.capacity(), 8);
        let keys: Vec<u32> = m.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, [1, 2, 3, 5]);
        assert_eq!(m.find(&5), Some(&50));
        assert_eq!(m.find(&4), None);
    }

    #[test]
    fn colliding_keys_probe_backwards_and_shift_on_remove() {
        // Capacity 4: hashes 1, 5 and 9 all land on slot 1 and probe down to 0, then 3.
        let mut m = THashMap::new(id_hash);
        m.set(1u32, 'a');
        m.set(5, 'b');
        m.set(9, 'c');
        let keys: Vec<u32> = m.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, [5, 1, 9]);
        assert_eq!(m.remove(&1), Some('a'));
        // 5 moves into slot 1, 9 into slot 0.
        let keys: Vec<u32> = m.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, [9, 5]);
        assert_eq!(m.find(&9), Some(&'c'));
    }

    #[test]
    fn zero_hash_is_remapped() {
        let mut m = THashMap::new(|_: &u32| 0);
        m.set(7, ());
        m.set(8, ());
        assert_eq!(m.count(), 2);
        assert!(m.find(&7).is_some());
        assert!(m.find(&8).is_some());
    }
}

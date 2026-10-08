// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTHash.h (`THashTable`, `THashMap`, `THashSet`) and the
// `SkGoodHash` of src/core/SkChecksum.h.

//! Skia's hash containers, ported exactly, including their slot order.
//!
//! Iteration order is observable in `SkSL`'s output: ``findPreexistingImmutableData`` iterates a
//! `THashSet<Slot>` and the first match decides which immutable slots are reused
//! (`docs/design/sksl.md` §5). So this is a faithful open-addressing table, not a `std` map:
//! linear probing towards lower indices, `SkGoodHash` (`Mix`) for 4-byte keys, reserved hash
//! value 0 for empty slots, the grow and shrink thresholds, and backward-shift deletion.
//!
//! Only the default load-factor policy is ported (`ShouldGrow`/`ShouldShrink` traits are not used
//! by any ported code). Only the 4-byte [`GoodHash`] keys are provided: other key types need their
//! own `SkGoodHash` port first.

use std::marker::PhantomData;

use crate::base_helpers::{checksum_mix, next_pow2};

/// `SkGoodHash` for the key types this crate uses: `Mix` of the four bytes of a 4-byte key.
#[doc(alias = "SkGoodHash")]
pub trait GoodHash {
    /// `SkGoodHash::operator()`.
    fn good_hash(&self) -> u32;
}

impl GoodHash for i32 {
    fn good_hash(&self) -> u32 {
        // `*(const uint32_t*)&k`: the bits of the key, not its value.
        checksum_mix(u32::from_ne_bytes(self.to_ne_bytes()))
    }
}

impl GoodHash for u32 {
    fn good_hash(&self) -> u32 {
        checksum_mix(*self)
    }
}

/// The static traits of a table: Skia's `Traits` (`GetKey` and `Hash`).
pub trait TableTraits<T> {
    /// The key type. Keys are compared with `==`, as Skia's `key == Traits::GetKey(*s)`.
    type Key: PartialEq;
    /// `Traits::GetKey`.
    fn key(item: &T) -> &Self::Key;
    /// `Traits::Hash`.
    fn hash(key: &Self::Key) -> u32;
}

/// The hash of a key as the table stores it: `Traits::Hash`, with 0 reserved for empty slots.
fn table_hash<T, Tr: TableTraits<T>>(key: &Tr::Key) -> u32 {
    let hash = Tr::hash(key);
    if hash == 0 { 1 } else { hash }
}

#[derive(Clone)]
struct Entry<T> {
    hash: u32,
    value: T,
}

/// `skia_private::THashTable<T, K, Traits>`: the slot array behind [`THashSet`] and [`THashMap`].
pub struct THashTable<T, Tr> {
    count: i32,
    capacity: i32,
    /// `fSlots`: `capacity` slots, `None` when empty. A populated slot's hash is never 0.
    slots: Vec<Option<Entry<T>>>,
    traits: PhantomData<fn() -> Tr>,
}

impl<T, Tr> Default for THashTable<T, Tr> {
    fn default() -> Self {
        Self {
            count: 0,
            capacity: 0,
            slots: Vec::new(),
            traits: PhantomData,
        }
    }
}

impl<T: Clone, Tr> Clone for THashTable<T, Tr> {
    fn clone(&self) -> Self {
        Self {
            count: self.count,
            capacity: self.capacity,
            slots: self.slots.clone(),
            traits: PhantomData,
        }
    }
}

impl<T: std::fmt::Debug, Tr> std::fmt::Debug for THashTable<T, Tr> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.slots.iter().flatten().map(|e| &e.value))
            .finish()
    }
}

impl<T, Tr: TableTraits<T>> THashTable<T, Tr> {
    /// The number of entries.
    #[must_use]
    pub fn count(&self) -> i32 {
        self.count
    }

    /// The number of slots, which is 0 or a power of two.
    #[must_use]
    pub fn capacity(&self) -> i32 {
        self.capacity
    }

    /// Clears the table (`reset()`).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `set`: inserts `val`, replacing an entry with the same key. Grows the table first when
    /// `4 * count >= 3 * capacity`. Returns the stored value.
    pub fn set(&mut self, val: T) -> &mut T {
        if 4 * self.count >= 3 * self.capacity {
            let new_capacity = if self.capacity > 0 {
                self.capacity * 2
            } else {
                4
            };
            self.resize(new_capacity);
        }
        let index = self.unchecked_set(val);
        self.entry_mut(index)
    }

    /// `find`: the entry with this key, if any.
    #[must_use]
    pub fn find(&self, key: &Tr::Key) -> Option<&T> {
        let index = self.find_index(key)?;
        self.slots[index].as_ref().map(|e| &e.value)
    }

    /// `find` with mutable access. Changing the entry's key breaks the table (as in Skia).
    pub fn find_mut(&mut self, key: &Tr::Key) -> Option<&mut T> {
        let index = self.find_index(key)?;
        Some(self.entry_mut(index))
    }

    /// `removeIfExists`: removes the entry with this key. Shrinks the table when
    /// `capacity > 4` and `4 * count <= capacity`. Returns whether an entry was removed.
    pub fn remove_if_exists(&mut self, key: &Tr::Key) -> bool {
        let Some(index) = self.find_index(key) else {
            return false;
        };
        self.remove_slot(index);
        if self.capacity > 4 && 4 * self.count <= self.capacity {
            self.resize(self.capacity / 2);
        }
        true
    }

    /// `remove`: removes an entry that must exist.
    pub fn remove(&mut self, key: &Tr::Key) {
        let removed = self.remove_if_exists(key);
        debug_assert!(removed, "remove() of a key that is not in the table");
    }

    /// `reserve(n)`: makes room for `n` entries without growing again.
    pub fn reserve(&mut self, n: i32) {
        debug_assert!(n > 0, "reserve() takes a positive count");
        let mut new_capacity =
            i32::try_from(next_pow2(u32::try_from(n).unwrap_or(1))).unwrap_or(i32::MAX);
        if n * 4 > new_capacity * 3 {
            new_capacity *= 2;
        }
        if new_capacity > self.capacity {
            self.resize(new_capacity);
        }
    }

    /// Visits every entry in slot order (`foreach`, and the iterator of `THashSet`/`THashMap`).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().flatten().map(|e| &e.value)
    }

    /// `resize(capacity)`: rehashes every entry into a table of `capacity` slots, in the order of
    /// the old slots.
    fn resize(&mut self, capacity: i32) {
        debug_assert!(capacity >= self.count);
        debug_assert_eq!(
            capacity & (capacity - 1),
            0,
            "capacity must be a power of two"
        );
        let old_slots = std::mem::replace(
            &mut self.slots,
            std::iter::repeat_with(|| None)
                .take(usize::try_from(capacity).unwrap_or(0))
                .collect(),
        );
        self.count = 0;
        self.capacity = capacity;
        for entry in old_slots.into_iter().flatten() {
            self.unchecked_set(entry.value);
        }
    }

    /// The mask that turns a hash into a slot index (`fCapacity - 1`), as an unsigned value.
    fn mask(&self) -> u32 {
        // Only called on a non-empty table; `capacity` is a positive power of two there.
        u32::try_from(self.capacity - 1).unwrap_or(0)
    }

    /// The slot before `index`, wrapping around (`next`).
    fn next(&self, index: usize) -> usize {
        if index == 0 {
            usize::try_from(self.capacity - 1).unwrap_or(0)
        } else {
            index - 1
        }
    }

    fn entry_mut(&mut self, index: usize) -> &mut T {
        self.slots[index]
            .as_mut()
            .map(|e| &mut e.value)
            .expect("the slot was just filled")
    }

    /// `find`'s probe loop: the slot index holding this key.
    fn find_index(&self, key: &Tr::Key) -> Option<usize> {
        if self.capacity == 0 {
            return None;
        }
        let hash = table_hash::<T, Tr>(key);
        let mut index = (hash & self.mask()) as usize;
        for _ in 0..self.capacity {
            match &self.slots[index] {
                None => return None,
                Some(s) if s.hash == hash && Tr::key(&s.value) == key => return Some(index),
                Some(_) => {}
            }
            index = self.next(index);
        }
        None
    }

    /// `uncheckedSet`: inserts or replaces without the grow check. Returns the slot index.
    fn unchecked_set(&mut self, val: T) -> usize {
        let hash = table_hash::<T, Tr>(Tr::key(&val));
        let mut index = (hash & self.mask()) as usize;
        for _ in 0..self.capacity {
            enum Probe {
                Empty,
                Same,
                Other,
            }
            let probe = match &self.slots[index] {
                None => Probe::Empty,
                Some(s) if s.hash == hash && Tr::key(&s.value) == Tr::key(&val) => Probe::Same,
                Some(_) => Probe::Other,
            };
            match probe {
                Probe::Empty => {
                    self.slots[index] = Some(Entry { hash, value: val });
                    self.count += 1;
                    return index;
                }
                Probe::Same => {
                    self.slots[index] = Some(Entry { hash, value: val });
                    return index;
                }
                Probe::Other => index = self.next(index),
            }
        }
        unreachable!("the table always has an empty slot: it grows at 3/4 load");
    }

    /// `removeSlot`: empties `index` and moves later entries of the probe run back into the
    /// hole, so that every remaining key is still reachable from its home slot.
    fn remove_slot(&mut self, mut index: usize) {
        self.count -= 1;
        loop {
            let empty_index = index;
            loop {
                index = self.next(index);
                let original_index = match &self.slots[index] {
                    None => {
                        self.slots[empty_index] = None;
                        return;
                    }
                    Some(s) => (s.hash & self.mask()) as usize,
                };
                // Kept as Skia's three-clause condition, verbatim: clippy's rewrite is not
                // equivalent to it.
                #[allow(clippy::nonminimal_bool)]
                let wraps_past_hole = (index <= original_index && original_index < empty_index)
                    || (original_index < empty_index && empty_index < index)
                    || (empty_index < index && index <= original_index);
                if !wraps_past_hole {
                    break;
                }
            }
            // `emptySlot = std::move(moveFrom)`: the hole moves to where the entry was.
            self.slots[empty_index] = self.slots[index].take();
        }
    }
}

/// `SkTHash`'s traits for a set: the item is its own key, hashed with [`GoodHash`].
struct SetTraits<T>(PhantomData<fn() -> T>);

impl<T: GoodHash + PartialEq> TableTraits<T> for SetTraits<T> {
    type Key = T;

    fn key(item: &T) -> &T {
        item
    }

    fn hash(key: &T) -> u32 {
        key.good_hash()
    }
}

/// `skia_private::THashSet<T>`. Iteration is in slot order, the order Skia's code observes.
#[derive(Debug)]
pub struct THashSet<T> {
    table: THashTable<T, SetTraits<T>>,
}

impl<T> Default for THashSet<T> {
    fn default() -> Self {
        Self {
            table: THashTable::default(),
        }
    }
}

impl<T: Clone> Clone for THashSet<T> {
    fn clone(&self) -> Self {
        Self {
            table: self.table.clone(),
        }
    }
}

impl<T: GoodHash + PartialEq> THashSet<T> {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of items.
    #[must_use]
    pub fn count(&self) -> i32 {
        self.table.count()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.count() == 0
    }

    /// Removes every item.
    pub fn reset(&mut self) {
        self.table.reset();
    }

    /// Makes room for `n` items.
    pub fn reserve(&mut self, n: i32) {
        self.table.reserve(n);
    }

    /// `add`: inserts `item`, replacing an equal one.
    pub fn add(&mut self, item: T) {
        self.table.set(item);
    }

    /// Whether an equal item is in the set.
    #[must_use]
    pub fn contains(&self, item: &T) -> bool {
        self.find(item).is_some()
    }

    /// The stored item equal to `item`, if any.
    #[must_use]
    pub fn find(&self, item: &T) -> Option<&T> {
        self.table.find(item)
    }

    /// Removes an item that must be in the set.
    pub fn remove(&mut self, item: &T) {
        self.table.remove(item);
    }

    /// Visits the items in slot order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.table.iter()
    }
}

/// `SkTHash`'s traits for a map: the pair's first element is the key.
struct MapTraits<K, V>(PhantomData<fn() -> (K, V)>);

impl<K: GoodHash + PartialEq, V> TableTraits<(K, V)> for MapTraits<K, V> {
    type Key = K;

    fn key(pair: &(K, V)) -> &K {
        &pair.0
    }

    fn hash(key: &K) -> u32 {
        key.good_hash()
    }
}

/// `skia_private::THashMap<K, V>`. Iteration is in slot order.
#[derive(Debug)]
pub struct THashMap<K, V> {
    table: THashTable<(K, V), MapTraits<K, V>>,
}

impl<K, V> Default for THashMap<K, V> {
    fn default() -> Self {
        Self {
            table: THashTable::default(),
        }
    }
}

impl<K: Clone, V: Clone> Clone for THashMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            table: self.table.clone(),
        }
    }
}

impl<K: GoodHash + PartialEq, V> THashMap<K, V> {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of entries.
    #[must_use]
    pub fn count(&self) -> i32 {
        self.table.count()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.count() == 0
    }

    /// Removes every entry.
    pub fn reset(&mut self) {
        self.table.reset();
    }

    /// Makes room for `n` entries.
    pub fn reserve(&mut self, n: i32) {
        self.table.reserve(n);
    }

    /// `set`: stores `val` under `key`, replacing an existing value. Returns the stored value.
    pub fn set(&mut self, key: K, val: V) -> &mut V {
        &mut self.table.set((key, val)).1
    }

    /// The value for `key`, if any.
    #[must_use]
    pub fn find(&self, key: &K) -> Option<&V> {
        self.table.find(key).map(|pair| &pair.1)
    }

    /// The value for `key`, mutably, if any.
    pub fn find_mut(&mut self, key: &K) -> Option<&mut V> {
        self.table.find_mut(key).map(|pair| &mut pair.1)
    }

    /// `operator[]`: the value for `key`, inserting `V::default()` when it is missing.
    ///
    /// # Panics
    ///
    /// Never: the key is inserted before it is looked up again.
    pub fn get_or_insert_default(&mut self, key: &K) -> &mut V
    where
        K: Clone,
        V: Default,
    {
        if self.table.find(key).is_none() {
            self.set(key.clone(), V::default());
        }
        self.find_mut(key).expect("the key was just inserted")
    }

    /// Removes the entry for `key`. The key must be present.
    pub fn remove(&mut self, key: &K) {
        self.table.remove(key);
    }

    /// Removes the entry for `key` if it is present; returns whether it was.
    pub fn remove_if_exists(&mut self, key: &K) -> bool {
        self.table.remove_if_exists(key)
    }

    /// Visits the entries in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.table.iter().map(|(k, v)| (k, v))
    }
}

#[cfg(test)]
mod tests {
    use super::{THashMap, THashSet};

    #[test]
    fn set_finds_and_removes() {
        let mut set = THashSet::new();
        for v in [5_i32, -1, 9, 5] {
            set.add(v);
        }
        assert_eq!(set.count(), 3);
        assert!(set.contains(&-1));
        assert!(!set.contains(&7));
        set.remove(&9);
        assert!(!set.contains(&9));
        assert_eq!(set.count(), 2);
    }

    #[test]
    fn map_sets_and_defaults() {
        let mut map: THashMap<u32, i32> = THashMap::new();
        map.set(3, 30);
        *map.get_or_insert_default(&4) += 7;
        assert_eq!(map.find(&3), Some(&30));
        assert_eq!(map.find(&4), Some(&7));
        assert!(map.remove_if_exists(&3));
        assert!(!map.remove_if_exists(&3));
        assert_eq!(map.count(), 1);
    }

    #[test]
    fn grows_at_three_quarters() {
        let mut set = THashSet::new();
        for v in 0..3_u32 {
            set.add(v);
        }
        assert_eq!(set.table.capacity(), 4);
        // The fourth insert finds 3 entries in 4 slots: 4 * 3 >= 3 * 4, so it doubles first.
        set.add(3);
        assert_eq!(set.table.capacity(), 8);
    }

    /// Every key stays reachable across removals that shift probe runs back into holes.
    #[test]
    fn removal_keeps_probe_runs_intact() {
        let mut set = THashSet::new();
        for v in 0..200_u32 {
            set.add(v * 7);
        }
        for v in (0..200_u32).step_by(3) {
            set.remove(&(v * 7));
        }
        for v in 0..200_u32 {
            assert_eq!(set.contains(&(v * 7)), v % 3 != 0, "key {}", v * 7);
        }
    }

    fn order_i32(set: &THashSet<i32>) -> Vec<i32> {
        set.iter().copied().collect()
    }

    /// Slot order of the real `SkTHash.h` (`src/core/SkTHash.h` at the pin, compiled with a
    /// harness that stubs only `SkTypes.h`, `SkChecksum.h` and `SkMathPriv.h`): the orders below
    /// are what Skia's `THashSet<int>` iterates to.
    #[test]
    fn slot_order_matches_skia() {
        let mut a = THashSet::new();
        for i in 0..20 {
            a.add(i);
        }
        assert_eq!(
            order_i32(&a),
            [
                17, 0, 15, 11, 7, 4, 2, 3, 6, 8, 5, 14, 10, 13, 12, 9, 19, 18, 1, 16
            ]
        );

        let mut b = THashSet::new();
        for i in 0..30 {
            b.add((i * 37) % 101 - 50);
        }
        assert_eq!(
            order_i32(&b),
            [
                0, 40, 7, -17, 30, -44, 34, 37, 10, -13, 13, 44, -10, 20, 3, -7, -50, -34, -47, 17,
                27, -27, 47, -3, -37, -30, 24, -40, -24, -20
            ]
        );
        for i in (0..30).step_by(3) {
            b.remove(&((i * 37) % 101 - 50));
        }
        assert_eq!(
            order_i32(&b),
            [
                7, -17, -44, 34, 37, -13, 13, 44, -7, 3, -34, -47, 17, 27, -27, 47, -3, -37, 24,
                -24
            ]
        );

        let mut e = THashSet::new();
        for i in 0..64 {
            e.add(-i);
        }
        for i in (0..64).step_by(5) {
            e.remove(&-i);
        }
        assert_eq!(
            order_i32(&e),
            [
                -12, -2, -43, -16, -8, -44, -13, -48, -23, -22, -14, -42, -9, -61, -57, -56, -53,
                -26, -39, -36, -37, -52, -18, -1, -24, -21, -4, -17, -62, -49, -6, -46, -63, -54,
                -19, -51, -32, -11, -41, -7, -38, -33, -34, -47, -27, -29, -3, -31, -58, -28, -59
            ]
        );
    }
}

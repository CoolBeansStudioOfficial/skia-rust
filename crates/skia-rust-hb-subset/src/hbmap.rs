// Copyright © 2018  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-map.hh (`hb_hashmap_t`) (harfbuzz 9cb1fee5)

//! `hb_hashmap_t<unsigned, unsigned>` with `HarfBuzz`'s open addressing, so that iteration visits the
//! entries in the same order. The repacker's results depend on that order (the order in which
//! subgraphs are duplicated).

const PRIME_MOD: [u32; 32] = [
    1, 2, 3, 7, 13, 31, 61, 127, 251, 509, 1021, 2039, 4093, 8191, 16381, 32749, 65521, 131_071, 262_139,
    524_287, 1_048_573, 2_097_143, 4_194_301, 8_388_593, 16_777_213, 33_554_393, 67_108_859, 134_217_689,
    268_435_399, 536_870_909, 1_073_741_789, 2_147_483_647,
];

/// `hb_hash` of an `unsigned` (Knuth's multiplicative hash).
fn hb_hash(v: u32) -> u32 {
    v.wrapping_mul(2_654_435_761)
}

#[derive(Clone, Copy, Default, Debug)]
struct Item {
    key: u32,
    value: u32,
    hash: u32,
    is_real: bool,
    is_used: bool,
}

/// The map, with the default value of `get` for a missing key.
#[derive(Clone, Debug, Default)]
pub(crate) struct HbMap {
    items: Vec<Item>,
    max_chain_length: u32,
    population: u32,
    occupancy: u32,
    mask: u32,
    prime: u32,
    /// The value `get` returns for a missing key: `Null (V)` (0), or `minus_1` for `hb_map_t`.
    default_value: u32,
}

impl HbMap {
    /// A `hb_hashmap_t<unsigned, unsigned>`: `get` of a missing key is 0.
    pub(crate) fn new() -> HbMap {
        HbMap::default()
    }

    /// A `hb_map_t`: `get` of a missing key is `HB_MAP_VALUE_INVALID`.
    pub(crate) fn new_minus_one() -> HbMap {
        HbMap { default_value: u32::MAX, ..HbMap::default() }
    }

    pub(crate) fn len(&self) -> usize {
        self.population as usize
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.population == 0
    }

    fn size(&self) -> u32 {
        if self.mask != 0 { self.mask + 1 } else { 0 }
    }

    fn alloc(&mut self, new_population: u32) {
        if new_population != 0 && (new_population + new_population / 2) < self.mask {
            return;
        }
        let power = crate::bytes::bit_storage((self.population.max(new_population) * 2).max(4));
        let new_size = 1u32 << power;
        let old_items = std::mem::replace(&mut self.items, vec![Item::default(); new_size as usize]);
        self.population = 0;
        self.occupancy = 0;
        self.mask = new_size - 1;
        self.prime = PRIME_MOD[(power as usize).min(PRIME_MOD.len() - 1)];
        self.max_chain_length = power * 2;
        for item in old_items {
            if item.is_real {
                self.set_with_hash(item.key, item.hash, item.value, true);
            }
        }
    }

    fn set_with_hash(&mut self, key: u32, hash: u32, value: u32, overwrite: bool) -> bool {
        if (self.occupancy + self.occupancy / 2) >= self.mask {
            self.alloc(0);
        }
        let hash = hash & 0x3FFF_FFFF;
        let mut tombstone = usize::MAX;
        let mut i = (hash % self.prime) as usize;
        let mut length = 0u32;
        let mut step = 0usize;
        while self.items[i].is_used {
            if self.items[i].key == key {
                if !overwrite {
                    return false;
                }
                break;
            }
            if !self.items[i].is_real && tombstone == usize::MAX {
                tombstone = i;
            }
            step += 1;
            i = (i + step) & self.mask as usize;
            length += 1;
        }
        let slot = if tombstone == usize::MAX { i } else { tombstone };
        if self.items[slot].is_used {
            self.occupancy -= 1;
            self.population -= u32::from(self.items[slot].is_real);
        }
        self.items[slot] = Item { key, value, hash, is_real: true, is_used: true };
        self.occupancy += 1;
        self.population += 1;
        if length > self.max_chain_length && self.occupancy * 8 > self.mask {
            self.alloc(self.mask - 8);
        }
        true
    }

    /// `set (key, value)`.
    pub(crate) fn set(&mut self, key: u32, value: u32) {
        self.set_with_hash(key, hb_hash(key), value, true);
    }

    fn fetch_item(&self, key: u32) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let hash = hb_hash(key) & 0x3FFF_FFFF;
        let mut i = (hash % self.prime) as usize;
        let mut step = 0usize;
        while self.items[i].is_used {
            if self.items[i].key == key {
                return if self.items[i].is_real { Some(i) } else { None };
            }
            step += 1;
            i = (i + step) & self.mask as usize;
        }
        None
    }

    /// `has (key, &value)`.
    pub(crate) fn has(&self, key: u32) -> Option<u32> {
        self.fetch_item(key).map(|i| self.items[i].value)
    }

    pub(crate) fn contains(&self, key: u32) -> bool {
        self.fetch_item(key).is_some()
    }

    /// `get (key)`.
    pub(crate) fn get(&self, key: u32) -> u32 {
        self.has(key).unwrap_or(self.default_value)
    }

    /// The value of the key, mutably (`has (key, &v)` followed by a write through `v`).
    pub(crate) fn get_mut(&mut self, key: u32) -> Option<&mut u32> {
        let i = self.fetch_item(key)?;
        Some(&mut self.items[i].value)
    }

    /// `del (key)`.
    pub(crate) fn del(&mut self, key: u32) {
        if let Some(i) = self.fetch_item(key) {
            self.items[i].is_real = false;
            self.population -= 1;
        }
    }

    /// `reset ()`: every entry removed, the storage kept.
    pub(crate) fn reset(&mut self) {
        for item in &mut self.items {
            *item = Item::default();
        }
        self.population = 0;
        self.occupancy = 0;
    }

    /// The `(key, value)` pairs in table order (`iter ()`).
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.items.iter().filter(|i| i.is_real).map(|i| (i.key, i.value))
    }

    /// The keys in table order (`keys ()`).
    pub(crate) fn keys(&self) -> impl Iterator<Item = u32> + '_ {
        self.iter().map(|p| p.0)
    }

    #[allow(dead_code)] // part of the `hb_hashmap_t` interface
    pub(crate) fn size_for_tests(&self) -> u32 {
        self.size()
    }
}

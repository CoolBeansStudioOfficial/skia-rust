// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/lazy/SkDiscardableMemoryPool.{h,cpp} (chrome/m156)

//! A pool of discardable memory that purges unlocked memory once the unlocked total is over a
//! budget.
//!
//! Skia keeps the pool's blocks on an intrusive list, most recently used first. Here the list is a
//! `VecDeque` of ids with the head at the front, and each block's state lives in the pool, so the
//! pool can purge a block that its owner does not currently hold. A purged block keeps its owner's
//! allocation until the owner next calls [`DiscardableMemory::lock`] (which fails), so the bytes are
//! released by the owner rather than at the moment of the purge. Nothing observable depends on
//! that timing.
//!
//! The lock statistics that `SK_LAZY_CACHE_STATS` enables are not ported.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use crate::discardable_memory::DiscardableMemory;
use crate::strike_cache::lock_unpoisoned;

/// The budget of the global pool, in bytes (`SK_DEFAULT_GLOBAL_DISCARDABLE_MEMORY_POOL_SIZE`).
#[doc(alias = "SK_DEFAULT_GLOBAL_DISCARDABLE_MEMORY_POOL_SIZE")]
pub const DEFAULT_GLOBAL_POOL_SIZE: usize = 128 * 1024 * 1024;

#[derive(Debug)]
struct Entry {
    bytes: usize,
    locked: bool,
    purged: bool,
}

#[derive(Debug)]
struct PoolState {
    budget: usize,
    used: usize,
    next_id: u64,
    /// The list of blocks that are not purged; the front is the head (most recently used).
    order: VecDeque<u64>,
    entries: HashMap<u64, Entry>,
}

/// A pool of discardable memory (`SkDiscardableMemoryPool`).
#[doc(alias = "SkDiscardableMemoryPool")]
#[derive(Debug)]
pub struct DiscardableMemoryPool {
    state: Mutex<PoolState>,
}

/// The global pool, with the default budget.
static GLOBAL_POOL: OnceLock<Arc<DiscardableMemoryPool>> = OnceLock::new();

/// Returns the global pool (`SkGetGlobalDiscardableMemoryPool`).
#[doc(alias = "SkGetGlobalDiscardableMemoryPool")]
pub fn global_pool() -> &'static Arc<DiscardableMemoryPool> {
    GLOBAL_POOL.get_or_init(|| DiscardableMemoryPool::new(DEFAULT_GLOBAL_POOL_SIZE))
}

/// Moves `id` out of `order`, if it is there.
fn remove_from_order(order: &mut VecDeque<u64>, id: u64) {
    if let Some(pos) = order.iter().position(|&x| x == id) {
        order.remove(pos);
    }
}

/// Purges unlocked blocks from the tail until the total is within `budget`
/// (`DiscardableMemoryPool::dumpDownTo`).
fn dump_down_to(state: &mut PoolState, budget: usize) {
    if state.used <= budget {
        return;
    }
    let mut i = state.order.len();
    while state.used > budget && i > 0 {
        i -= 1;
        let id = state.order[i];
        let Some(entry) = state.entries.get_mut(&id) else {
            continue;
        };
        if !entry.locked {
            // Purged blocks are taken off the list, and are not deleted.
            entry.purged = true;
            state.used -= entry.bytes;
            state.order.remove(i);
        }
    }
}

impl DiscardableMemoryPool {
    /// Creates a pool with the given budget in bytes (`SkDiscardableMemoryPool::Make`).
    #[doc(alias = "SkDiscardableMemoryPool::Make")]
    #[must_use]
    pub fn new(budget: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(PoolState {
                budget,
                used: 0,
                next_id: 0,
                order: VecDeque::new(),
                entries: HashMap::new(),
            }),
        })
    }

    /// Creates a locked block of `bytes` in the pool (`DiscardableMemoryPool::create`).
    #[must_use]
    pub fn create(self: &Arc<Self>, bytes: usize) -> Option<Box<dyn DiscardableMemory>> {
        // Port of: DiscardableMemoryPool::make (src/lazy/SkDiscardableMemoryPool.cpp)
        let memory = vec![0_u8; bytes].into_boxed_slice();
        let id = {
            let mut state = lock_unpoisoned(&self.state);
            let id = state.next_id;
            state.next_id += 1;
            state.entries.insert(
                id,
                Entry {
                    bytes,
                    locked: true,
                    purged: false,
                },
            );
            state.order.push_front(id);
            state.used += bytes;
            let budget = state.budget;
            dump_down_to(&mut state, budget);
            id
        };
        Some(Box::new(PoolDiscardableMemory {
            pool: Arc::clone(self),
            id,
            memory: Some(memory),
        }))
    }

    /// Returns the bytes held by blocks that are not purged (`getRAMUsed`).
    #[must_use]
    pub fn get_ram_used(&self) -> usize {
        lock_unpoisoned(&self.state).used
    }

    /// Sets the budget and purges down to it (`setRAMBudget`).
    pub fn set_ram_budget(&self, budget: usize) {
        let mut state = lock_unpoisoned(&self.state);
        state.budget = budget;
        dump_down_to(&mut state, budget);
    }

    /// Returns the budget in bytes (`getRAMBudget`).
    #[must_use]
    pub fn get_ram_budget(&self) -> usize {
        lock_unpoisoned(&self.state).budget
    }

    /// Purges every unlocked block (`dumpPool`).
    pub fn dump_pool(&self) {
        dump_down_to(&mut lock_unpoisoned(&self.state), 0);
    }

    /// Locks block `id`; fails if it was purged (`DiscardableMemoryPool::lock`).
    fn lock(&self, id: u64) -> bool {
        let mut state = lock_unpoisoned(&self.state);
        let PoolState { order, entries, .. } = &mut *state;
        let Some(entry) = entries.get_mut(&id) else {
            return false;
        };
        if entry.purged {
            // May have been purged while waiting for the lock.
            return false;
        }
        entry.locked = true;
        remove_from_order(order, id);
        order.push_front(id);
        true
    }

    /// Unlocks block `id` and purges down to the budget (`DiscardableMemoryPool::unlock`).
    fn unlock(&self, id: u64) {
        let mut state = lock_unpoisoned(&self.state);
        if let Some(entry) = state.entries.get_mut(&id) {
            entry.locked = false;
        }
        let budget = state.budget;
        dump_down_to(&mut state, budget);
    }

    fn is_purged(&self, id: u64) -> bool {
        lock_unpoisoned(&self.state)
            .entries
            .get(&id)
            .is_none_or(|entry| entry.purged)
    }

    /// Called when a block is destroyed (`DiscardableMemoryPool::removeFromPool`).
    fn remove_from_pool(&self, id: u64) {
        let mut state = lock_unpoisoned(&self.state);
        // A purged block is already out of the list and the total.
        if let Some(entry) = state.entries.remove(&id).filter(|entry| !entry.purged) {
            state.used -= entry.bytes;
            remove_from_order(&mut state.order, id);
        }
    }
}

/// A block of memory that is counted in a [`DiscardableMemoryPool`].
struct PoolDiscardableMemory {
    pool: Arc<DiscardableMemoryPool>,
    id: u64,
    memory: Option<Box<[u8]>>,
}

impl DiscardableMemory for PoolDiscardableMemory {
    fn lock(&mut self) -> bool {
        if self.pool.lock(self.id) {
            true
        } else {
            // Purged: the contents are gone, so release the allocation now.
            self.memory = None;
            false
        }
    }

    fn data(&self) -> Option<&[u8]> {
        if self.pool.is_purged(self.id) {
            return None;
        }
        self.memory.as_deref()
    }

    fn data_mut(&mut self) -> Option<&mut [u8]> {
        if self.pool.is_purged(self.id) {
            return None;
        }
        self.memory.as_deref_mut()
    }

    fn unlock(&mut self) {
        self.pool.unlock(self.id);
    }
}

impl Drop for PoolDiscardableMemory {
    fn drop(&mut self) {
        self.pool.remove_from_pool(self.id);
    }
}

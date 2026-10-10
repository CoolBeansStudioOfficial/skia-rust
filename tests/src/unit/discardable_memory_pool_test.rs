// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DiscardableMemoryPoolTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::discardable_memory_pool::DiscardableMemoryPool;

use crate::{def_test, reporter_assert};

// Port of: tests/DiscardableMemoryPoolTest.cpp#L15-L38 (chrome/m156)
def_test!(DiscardableMemoryPool, |reporter| {
    let pool = DiscardableMemoryPool::new(1);
    pool.set_ram_budget(3);
    reporter_assert!(reporter, pool.get_ram_used() == 0);

    let mut dm1 = pool.create(100).expect("a discardable memory of 100 bytes");
    reporter_assert!(reporter, dm1.data().is_some());
    reporter_assert!(reporter, pool.get_ram_used() == 100);
    dm1.unlock();
    reporter_assert!(reporter, pool.get_ram_used() == 0);
    reporter_assert!(reporter, !dm1.lock());

    let mut dm2 = pool.create(200).expect("a discardable memory of 200 bytes");
    reporter_assert!(reporter, pool.get_ram_used() == 200);
    pool.set_ram_budget(400);
    dm2.unlock();
    reporter_assert!(reporter, pool.get_ram_used() == 200);
    reporter_assert!(reporter, dm2.lock());
    dm2.unlock();
    pool.dump_pool();
    reporter_assert!(reporter, !dm2.lock());
    reporter_assert!(reporter, pool.get_ram_used() == 0);
});

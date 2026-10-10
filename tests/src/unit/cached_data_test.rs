// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CachedDataTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::cached_data::CachedData;
use skia_rust_core::discardable_memory_pool::DiscardableMemoryPool;

use crate::{Reporter, def_test, reporter_assert};

#[derive(PartialEq, Eq)]
enum LockedState {
    Unlocked,
    Locked,
}

#[derive(PartialEq, Eq)]
enum CachedState {
    NotInCache,
    InCache,
}

// Port of: tests/CachedDataTest.cpp#L24-L33 (chrome/m156), `check_data`
fn check_data(
    reporter: &mut Reporter,
    data: &CachedData,
    refcnt: i32,
    cache_state: CachedState,
    locked_state: LockedState,
) {
    reporter_assert!(reporter, data.testing_only_get_ref_cnt() == refcnt);
    reporter_assert!(
        reporter,
        data.testing_only_is_in_cache() == (cache_state == CachedState::InCache)
    );
    reporter_assert!(
        reporter,
        data.testing_only_is_locked() == (locked_state == LockedState::Locked)
    );
}

// Port of: tests/CachedDataTest.cpp#L35-L44 (chrome/m156), `make_data`
fn make_data(size: usize, pool: Option<&Arc<DiscardableMemoryPool>>) -> Arc<CachedData> {
    match pool {
        Some(pool) => {
            // The pool "can" return null, but it shouldn't in these controlled conditions.
            let dm = pool
                .create(size)
                .expect("the pool does not fail in these controlled conditions");
            CachedData::new_discardable(size, dm)
        }
        None => CachedData::new_malloc(vec![0_u8; size]),
    }
}

// Port of: tests/CachedDataTest.cpp#L46-L69 (chrome/m156), `test_locking`
// returns with the data locked by client and cache
fn test_locking(
    reporter: &mut Reporter,
    size: usize,
    pool: Option<&Arc<DiscardableMemoryPool>>,
) -> Arc<CachedData> {
    let data = make_data(size, pool);

    if let Some(mut bytes) = data.writable_data() {
        bytes.fill(0x80); // just to use writable_data()
    }

    check_data(
        reporter,
        &data,
        1,
        CachedState::NotInCache,
        LockedState::Locked,
    );

    data.add_ref();
    check_data(
        reporter,
        &data,
        2,
        CachedState::NotInCache,
        LockedState::Locked,
    );
    data.unref();
    check_data(
        reporter,
        &data,
        1,
        CachedState::NotInCache,
        LockedState::Locked,
    );

    data.attach_to_cache_and_ref();
    check_data(
        reporter,
        &data,
        2,
        CachedState::InCache,
        LockedState::Locked,
    );

    data.unref();
    check_data(
        reporter,
        &data,
        1,
        CachedState::InCache,
        LockedState::Unlocked,
    );

    data.add_ref();
    check_data(
        reporter,
        &data,
        2,
        CachedState::InCache,
        LockedState::Locked,
    );

    data
}

// Port of: tests/CachedDataTest.cpp#L81-L101 (chrome/m156)
//
// SkCachedData behaves differently (regarding its locked/unlocked state) depending on when it is in
// the cache or not. Being in the cache is signaled by calling attachToCacheAndRef() instead of
// ref() (and balanced by detachFromCacheAndUnref).
//
// Thus, among other things, we test the end-of-life behavior when the client is the last owner and
// when the cache is.
def_test!(CachedData, |reporter| {
    let pool = DiscardableMemoryPool::new(1000);

    for use_discardable in [false, true] {
        let size = 100;
        let pool_arg = use_discardable.then_some(&pool);

        // test with client as last owner
        let data = test_locking(reporter, size, pool_arg);
        check_data(
            reporter,
            &data,
            2,
            CachedState::InCache,
            LockedState::Locked,
        );
        data.detach_from_cache_and_unref();
        check_data(
            reporter,
            &data,
            1,
            CachedState::NotInCache,
            LockedState::Locked,
        );
        data.unref();

        // test with cache as last owner
        let data = test_locking(reporter, size, pool_arg);
        check_data(
            reporter,
            &data,
            2,
            CachedState::InCache,
            LockedState::Locked,
        );
        data.unref();
        check_data(
            reporter,
            &data,
            1,
            CachedState::InCache,
            LockedState::Unlocked,
        );
        data.detach_from_cache_and_unref();
    }
});

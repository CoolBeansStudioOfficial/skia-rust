// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkResourceCacheTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use skia_rust_core::discardable_memory::create as create_discardable;
use skia_rust_core::resource_cache::{Key, Rec, ResourceCache};

use crate::{Reporter, def_test, reporter_assert};

// Only the address of this static is used, as the namespace of the test keys.
static TEST_NAMESPACE: u8 = 0;

// Port of: tests/SkResourceCacheTest.cpp#L262-L264 (chrome/m156), `TestRec::kDidInstall`
const DID_INSTALL: u32 = 1 << 0;

// Port of: tests/SkResourceCacheTest.cpp#L266-L273 (chrome/m156), `TestKey`
fn test_key(shared_id: u64, data: i32) -> Key {
    Key::new(
        std::ptr::addr_of!(TEST_NAMESPACE) as usize,
        shared_id,
        &[data.cast_unsigned()],
    )
}

// Port of: tests/SkResourceCacheTest.cpp#L275-L291 (chrome/m156), `TestRec`
//
// The flags and `fCanBePurged` are shared with the test, which keeps them after the cache takes
// ownership of the record.
struct TestRec {
    key: Key,
    flags: Arc<AtomicU32>,
    can_be_purged: Arc<AtomicBool>,
}

impl TestRec {
    fn new(
        shared_id: u64,
        data: i32,
        flags: Arc<AtomicU32>,
        can_be_purged: Arc<AtomicBool>,
    ) -> Box<Self> {
        Box::new(Self {
            key: test_key(shared_id, data),
            flags,
            can_be_purged,
        })
    }
}

impl Rec for TestRec {
    fn key(&self) -> &Key {
        &self.key
    }

    fn bytes_used(&self) -> usize {
        1024 // just need a value
    }

    fn can_be_purged(&mut self) -> bool {
        self.can_be_purged.load(Ordering::Relaxed)
    }

    fn post_add_install(&mut self) {
        self.flags.fetch_or(DID_INSTALL, Ordering::Relaxed);
    }

    fn category(&self) -> &'static str {
        "test-category"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Port of: tests/SkResourceCacheTest.cpp#L259-L295 (chrome/m156), `test_duplicate_add`
fn test_duplicate_add(cache: &mut ResourceCache, reporter: &mut Reporter, purgable: bool) {
    let shared_id = 1;
    let data = 0;

    let flags0 = Arc::new(AtomicU32::new(0));
    let flags1 = Arc::new(AtomicU32::new(0));

    let can_be_purged0 = Arc::new(AtomicBool::new(purgable));
    let rec0 = TestRec::new(shared_id, data, flags0.clone(), can_be_purged0.clone());
    let rec1 = TestRec::new(
        shared_id,
        data,
        flags1.clone(),
        Arc::new(AtomicBool::new(false)),
    );
    // SkASSERT(rec0->getKey() == rec1->getKey())
    debug_assert!(rec0.key() == rec1.key());

    reporter_assert!(reporter, flags0.load(Ordering::Relaxed) & DID_INSTALL == 0);
    reporter_assert!(reporter, flags1.load(Ordering::Relaxed) & DID_INSTALL == 0);

    cache.add(rec0);
    reporter_assert!(reporter, flags0.load(Ordering::Relaxed) & DID_INSTALL != 0);
    reporter_assert!(reporter, flags1.load(Ordering::Relaxed) & DID_INSTALL == 0);
    flags0.store(0, Ordering::Relaxed); // reset the flag

    cache.add(rec1);
    if purgable {
        // we purged rec0, and did install rec1
        reporter_assert!(reporter, flags0.load(Ordering::Relaxed) & DID_INSTALL == 0);
        reporter_assert!(reporter, flags1.load(Ordering::Relaxed) & DID_INSTALL != 0);
    } else {
        // we re-used rec0 and did not install rec1
        reporter_assert!(reporter, flags0.load(Ordering::Relaxed) & DID_INSTALL != 0);
        reporter_assert!(reporter, flags1.load(Ordering::Relaxed) & DID_INSTALL == 0);
        can_be_purged0.store(true, Ordering::Relaxed); // so we can cleanup the cache
    }
}

// Port of: tests/SkResourceCacheTest.cpp#L297-L308 (chrome/m156)
//
// Test behavior when the same key is added more than once.
def_test!(ResourceCache_purge, |reporter| {
    for purgable in [false, true] {
        {
            let mut cache = ResourceCache::new(1024 * 1024);
            test_duplicate_add(&mut cache, reporter, purgable);
        }
        {
            let mut cache = ResourceCache::with_discardable_factory(create_discardable);
            test_duplicate_add(&mut cache, reporter, purgable);
        }
    }
});

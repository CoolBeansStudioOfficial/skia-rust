// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DiscardableMemoryTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::discardable_memory::{DiscardableMemory, create};
use skia_rust_core::discardable_memory_pool::DiscardableMemoryPool;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/DiscardableMemoryTest.cpp#L13-L16 (chrome/m156). The C string includes its NUL.
const TEST_STRING: &[u8] = b"HELLO, WORLD!\0";

// Port of: tests/DiscardableMemoryTest.cpp#L21-L49 (chrome/m156), `test_dm`
fn test_dm<D: DiscardableMemory + ?Sized>(
    reporter: &mut Reporter,
    dm: Option<&mut D>,
    assert_relock: bool,
) {
    reporter_assert!(reporter, dm.is_some());
    let Some(dm) = dm else {
        return;
    };
    let Some(memory) = dm.data_mut() else {
        reporter_assert!(reporter, false);
        return;
    };
    memory[..TEST_STRING.len()].copy_from_slice(TEST_STRING);
    dm.unlock();
    let relock_success = dm.lock();
    if assert_relock {
        reporter_assert!(reporter, relock_success);
    }
    if !relock_success {
        return;
    }
    let Some(memory) = dm.data() else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(reporter, memory[..TEST_STRING.len()] == *TEST_STRING);
    dm.unlock();
}

// Port of: tests/DiscardableMemoryTest.cpp#L51-L56 (chrome/m156)
def_test!(DiscardableMemory_global, |reporter| {
    let mut dm = create(TEST_STRING.len());
    // lock() test is allowed to fail, since other threads could be using global pool.
    test_dm(reporter, dm.as_deref_mut(), false);
});

// Port of: tests/DiscardableMemoryTest.cpp#L58-L63 (chrome/m156)
def_test!(DiscardableMemory_nonglobal, |reporter| {
    let pool = DiscardableMemoryPool::new(1024);
    let mut dm = pool.create(TEST_STRING.len());
    test_dm(reporter, dm.as_deref_mut(), true);
});

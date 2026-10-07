// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ChecksumTest.cpp (chrome/m156)

use skia_rust_core::checksum::{self, GoodHash};
use skia_rust_core::random::Random;

use crate::{def_test, reporter_assert};

// Hashes the native-endian bytes of `words`, as `SkChecksum::Hash32(data, kBytes)` does.
fn hash_words(words: &[u32]) -> u32 {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_ne_bytes()).collect();
    checksum::hash32(&bytes, 0)
}

// Port of: tests/ChecksumTest.cpp#L21-L43 (chrome/m156)
def_test!(Checksum, |r| {
    // Put 128 random bytes into two identical buffers.  Any multiple of 4 will do.
    const K_BYTES: usize = 128; // SkAlign4(128)
    let mut rand = Random::default();
    let mut data = [0u32; K_BYTES / 4];
    let mut tweaked = [0u32; K_BYTES / 4];
    for i in 0..tweaked.len() {
        let v = rand.next_u();
        data[i] = v;
        tweaked[i] = v;
    }

    let hash = hash_words(&data);
    // Should be deterministic.
    reporter_assert!(r, hash == hash_words(&data));

    // Changing any single element should change the hash.
    for j in 0..tweaked.len() {
        let saved = tweaked[j];
        tweaked[j] = rand.next_u();
        let tweaked_hash = hash_words(&tweaked);
        reporter_assert!(r, tweaked_hash != hash);
        reporter_assert!(r, tweaked_hash == hash_words(&tweaked));
        tweaked[j] = saved;
    }
});

// Port of: tests/ChecksumTest.cpp#L45-L49 (chrome/m156)
def_test!(GoodHash, |r| {
    // 4 bytes --> hits SkChecksum::Mix fast path.
    reporter_assert!(r, 4_i32.good_hash() == 614_249_093);
    reporter_assert!(r, 4_u32.good_hash() == 614_249_093);
});

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_ne_bytes()).collect()
}

fn f64_bytes(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_ne_bytes()).collect()
}

// Port of: tests/ChecksumTest.cpp#L51-L77 (chrome/m156)
def_test!(ChecksumCollisions, |r| {
    // We noticed a few workloads that would cause hash collisions due to the way
    // our old optimized hashes split into three concurrent hashes and merged those hashes together.
    //
    // One of these two workloads ought to cause an unintentional hash collision on very similar
    // data in those old algorithms, the float version on 32-bit x86 and double elsewhere.
    {
        let a: [f32; 9] = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b: [f32; 9] = [1.0, 2.0, 0.0, 4.0, 5.0, 3.0, 7.0, 8.0, 6.0];

        reporter_assert!(
            r,
            checksum::hash32(&f32_bytes(&a), 0) != checksum::hash32(&f32_bytes(&b), 0)
        );
    }
    {
        let a: [f64; 9] = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b: [f64; 9] = [1.0, 2.0, 0.0, 4.0, 5.0, 3.0, 7.0, 8.0, 6.0];

        reporter_assert!(
            r,
            checksum::hash32(&f64_bytes(&a), 0) != checksum::hash32(&f64_bytes(&b), 0)
        );
    }
});

// Port of: tests/ChecksumTest.cpp#L79-L96 (chrome/m156)
def_test!(ChecksumConsistent, |r| {
    // We don't guarantee that SkChecksum::Hash32 will return consistent results, but it does today.
    // Spot check a few:
    let mut bytes = [0u8; 256];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::try_from(i).unwrap();
    }
    let hash_bytes = |n: usize| checksum::hash32(&bytes[..n], 0);
    reporter_assert!(r, hash_bytes(0) == 0xe2bd_e459, "{:08x}", hash_bytes(0));
    reporter_assert!(r, hash_bytes(1) == 0xe5f8_bd85, "{:08x}", hash_bytes(1));
    reporter_assert!(r, hash_bytes(2) == 0x77ac_d42a, "{:08x}", hash_bytes(2));
    reporter_assert!(r, hash_bytes(7) == 0x78d0_861f, "{:08x}", hash_bytes(7));
    reporter_assert!(r, hash_bytes(32) == 0x4e73_df6d, "{:08x}", hash_bytes(32));
    reporter_assert!(r, hash_bytes(63) == 0x5e66_a3f4, "{:08x}", hash_bytes(63));
    reporter_assert!(r, hash_bytes(64) == 0x962d_6746, "{:08x}", hash_bytes(64));
    reporter_assert!(r, hash_bytes(99) == 0x79e0_9416, "{:08x}", hash_bytes(99));
    reporter_assert!(r, hash_bytes(255) == 0x85f8_37f0, "{:08x}", hash_bytes(255));
});

// skia-rust: ChecksumStrings is not ported: it exercises SkGoodHash on SkString, which is not
// ported (manifest status stays `todo`).

// Port of: tests/ChecksumTest.cpp#L98-L105 (chrome/m156)
def_test!(ChecksumStrings, |r| {
    const K_MESSAGE: &str = "Checksums are supported for SkString, string, and string_view.";
    let expected_hash = checksum::hash32(K_MESSAGE.as_bytes(), 0);

    // skia-rust: SkString is not ported (Rust uses `String`); `String` and `&str` stand in for
    // SkString, std::string and std::string_view.
    reporter_assert!(r, expected_hash == String::from(K_MESSAGE).good_hash());
    reporter_assert!(r, expected_hash == K_MESSAGE.to_owned().good_hash());
    reporter_assert!(r, expected_hash == K_MESSAGE.good_hash());
});

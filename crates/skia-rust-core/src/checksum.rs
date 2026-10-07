// Copyright 2012 Google Inc.
// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkChecksum.h, src/core/SkChecksum.cpp

//! Hash functions: the Murmur3 finalizer mixes and a 64-bit wyhash, bit-identical to Skia's.

// wyhash, a fast and good hash function, from https://github.com/wangyi-fudan/wyhash

// 128bit multiply function
// Port of: src/core/SkChecksum.cpp#L15-L34 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors (uint64_t)r
fn wymum(a: &mut u64, b: &mut u64) {
    let mut r = u128::from(*a);
    r = r.wrapping_mul(u128::from(*b));
    *a = r as u64;
    *b = (r >> 64) as u64;
}

// multiply and xor mix function, aka MUM
// Port of: src/core/SkChecksum.cpp#L36-L40 (chrome/m156)
fn wymix(mut a: u64, mut b: u64) -> u64 {
    wymum(&mut a, &mut b);
    a ^ b
}

// read functions (native endian, as `memcpy` in C++)
// Port of: src/core/SkChecksum.cpp#L42-L57 (chrome/m156)
fn wyr8(p: &[u8]) -> u64 {
    u64::from_ne_bytes(p[..8].try_into().expect("8 bytes"))
}

fn wyr4(p: &[u8]) -> u64 {
    u64::from(u32::from_ne_bytes(p[..4].try_into().expect("4 bytes")))
}

fn wyr3(p: &[u8], k: usize) -> u64 {
    (u64::from(p[0]) << 16) | (u64::from(p[k >> 1]) << 8) | u64::from(p[k - 1])
}

// wyhash main function
// Port of: src/core/SkChecksum.cpp#L59-L98 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors wyhash's see1/see2
fn wyhash(key: &[u8], mut seed: u64, secret: &[u64; 4]) -> u64 {
    let len = key.len();
    let mut p = 0usize;
    seed ^= wymix(seed ^ secret[0], secret[1]);
    let (mut a, mut b);
    if len <= 16 {
        if len >= 4 {
            a = (wyr4(key) << 32) | wyr4(&key[((len >> 3) << 2)..]);
            b = (wyr4(&key[len - 4..]) << 32) | wyr4(&key[len - 4 - ((len >> 3) << 2)..]);
        } else if len > 0 {
            a = wyr3(key, len);
            b = 0;
        } else {
            a = 0;
            b = 0;
        }
    } else {
        let mut i = len;
        if i > 48 {
            let mut see1 = seed;
            let mut see2 = seed;
            loop {
                seed = wymix(wyr8(&key[p..]) ^ secret[1], wyr8(&key[p + 8..]) ^ seed);
                see1 = wymix(
                    wyr8(&key[p + 16..]) ^ secret[2],
                    wyr8(&key[p + 24..]) ^ see1,
                );
                see2 = wymix(
                    wyr8(&key[p + 32..]) ^ secret[3],
                    wyr8(&key[p + 40..]) ^ see2,
                );
                p += 48;
                i -= 48;
                if i <= 48 {
                    break;
                }
            }
            seed ^= see1 ^ see2;
        }
        while i > 16 {
            seed = wymix(wyr8(&key[p..]) ^ secret[1], wyr8(&key[p + 8..]) ^ seed);
            i -= 16;
            p += 16;
        }
        a = wyr8(&key[p + i - 16..]);
        b = wyr8(&key[p + i - 8..]);
    }
    a ^= secret[1];
    b ^= seed;
    wymum(&mut a, &mut b);
    wymix(a ^ secret[0] ^ (len as u64), b ^ secret[1])
}

// the default secret parameters
// Port of: src/core/SkChecksum.cpp#L100-L102 (chrome/m156)
const WYP: [u64; 4] = [
    0xa076_1d64_78bd_642f,
    0xe703_7ed1_a0b4_28db,
    0x8ebc_6af0_9c88_c6e3,
    0x5899_65cc_7537_4cc3,
];

/// `uint32_t -> uint32_t` hash, useful for when you're about to truncate this hash but you
/// suspect its low bits aren't well mixed. This is the Murmur3 finalizer.
// Port of: src/core/SkChecksum.h#L30-L37 (chrome/m156)
#[doc(alias = "SkChecksum::Mix")]
#[doc(alias = "Mix")]
#[must_use]
pub fn mix(mut hash: u32) -> u32 {
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^= hash >> 16;
    hash
}

/// `uint32_t -> uint32_t` hash, useful for when you're about to truncate this hash but you
/// suspect its low bits aren't well mixed. This version is 2-lines cheaper than [`mix`], but
/// seems to be sufficient for the font cache.
// Port of: src/core/SkChecksum.h#L45-L50 (chrome/m156)
#[doc(alias = "SkChecksum::CheapMix")]
#[doc(alias = "CheapMix")]
#[must_use]
pub fn cheap_mix(mut hash: u32) -> u32 {
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 16;
    hash
}

/// A fast, high-quality 32-bit hash. No guarantees are made about this remaining stable over
/// time, or being consistent across devices.
///
/// For now, this is a 64-bit wyhash, truncated to 32 bits.
// Port of: src/core/SkChecksum.cpp#L106-L108 (chrome/m156)
#[doc(alias = "SkChecksum::Hash32")]
#[doc(alias = "Hash32")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // mirrors static_cast<uint32_t>
pub fn hash32(data: &[u8], seed: u32) -> u32 {
    wyhash(data, u64::from(seed), &WYP) as u32
}

/// A fast, high-quality 64-bit hash. No guarantees are made about this remaining stable over
/// time, or being consistent across devices.
///
/// For now, this is a 64-bit wyhash.
// Port of: src/core/SkChecksum.cpp#L110-L112 (chrome/m156)
#[doc(alias = "SkChecksum::Hash64")]
#[doc(alias = "Hash64")]
#[must_use]
pub fn hash64(data: &[u8], seed: u64) -> u64 {
    wyhash(data, seed, &WYP)
}

/// Port of `SkGoodHash`: usually the first choice in hashing data. Keys whose object
/// representation is 4 bytes use [`mix`]; other keys use [`hash32`] over their bytes.
// Port of: src/core/SkChecksum.h#L72-L98 (chrome/m156)
#[doc(alias = "SkGoodHash")]
pub trait GoodHash {
    /// `SkGoodHash()(self)`.
    #[must_use]
    fn good_hash(&self) -> u32;
}

macro_rules! impl_good_hash_4_bytes {
    ($($t:ty),*) => {$(
        impl GoodHash for $t {
            #[allow(clippy::cast_sign_loss)] // mirrors *(const uint32_t*)&k
            fn good_hash(&self) -> u32 {
                mix(*self as u32)
            }
        }
    )*};
}
impl_good_hash_4_bytes!(u32, i32);

macro_rules! impl_good_hash_bytes {
    ($($t:ty),*) => {$(
        impl GoodHash for $t {
            fn good_hash(&self) -> u32 {
                hash32(&self.to_ne_bytes(), 0)
            }
        }
    )*};
}
impl_good_hash_bytes!(u8, i8, u16, i16, u64, i64, u128, i128);

impl GoodHash for [u8] {
    fn good_hash(&self) -> u32 {
        hash32(self, 0)
    }
}

impl GoodHash for str {
    fn good_hash(&self) -> u32 {
        hash32(self.as_bytes(), 0)
    }
}

impl GoodHash for String {
    fn good_hash(&self) -> u32 {
        hash32(self.as_bytes(), 0)
    }
}

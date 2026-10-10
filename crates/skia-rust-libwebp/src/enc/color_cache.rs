// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of libwebp `src/utils/color_cache_utils.{c,h}`: the colour cache of the VP8L encoder
//! (`VP8LColorCache`), a direct-mapped table of recently used colours.

use super::palette::hash_pix;

/// Port of `VP8LColorCache`.
#[derive(Clone, Debug, Default)]
pub struct ColorCache {
    colors: Vec<u32>,
    hash_shift: u32,
    hash_bits: u32,
}

impl ColorCache {
    /// Port of `VP8LColorCacheInit`: a cache of `1 << hash_bits` zeroed entries.
    #[must_use]
    pub fn new(hash_bits: u32) -> Self {
        Self {
            colors: vec![0; 1usize << hash_bits],
            hash_shift: 32 - hash_bits,
            hash_bits,
        }
    }

    /// The `hash_bits` the cache was created with (`hash_bits_`).
    #[must_use]
    pub fn hash_bits(&self) -> u32 {
        self.hash_bits
    }

    /// Port of `VP8LColorCacheGetIndex`: the key of `argb` in this cache.
    #[must_use]
    pub fn get_index(&self, argb: u32) -> usize {
        hash_pix(argb, self.hash_shift)
    }

    /// Port of `VP8LColorCacheLookup`: the colour stored under `key`.
    #[must_use]
    pub fn lookup(&self, key: usize) -> u32 {
        self.colors[key]
    }

    /// Port of `VP8LColorCacheSet`.
    pub fn set(&mut self, key: usize, argb: u32) {
        self.colors[key] = argb;
    }

    /// Port of `VP8LColorCacheContains`: the key holding `argb`, or `None`.
    #[must_use]
    pub fn contains(&self, argb: u32) -> Option<usize> {
        let key = self.get_index(argb);
        if self.colors[key] == argb {
            Some(key)
        } else {
            None
        }
    }

    /// Port of `VP8LColorCacheInsert`.
    pub fn insert(&mut self, argb: u32) {
        let key = self.get_index(argb);
        self.colors[key] = argb;
    }
}

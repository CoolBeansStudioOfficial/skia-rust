// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/KeyBuilder.h

//! `skgpu::KeyBuilder` and `skgpu::StringKeyBuilder`: packs bit fields into a `u32` key.
//!
//! Skia's `KeyBuilder` has a virtual `addBits`, overridden by `StringKeyBuilder` so that every
//! field is also described in text. Here the base builder is a plain struct and the string
//! builder wraps it, re-implementing the methods that dispatch through `addBits` in C++.

use std::fmt::Write as _;

/// Packs bit fields into a `u32` array, least significant bit first.
// Port of: src/gpu/KeyBuilder.h#L18-L84 (chrome/m156)
#[doc(alias = "skgpu::KeyBuilder")]
#[derive(Debug)]
pub struct KeyBuilder<'a> {
    data: &'a mut Vec<u32>,
    cur_value: u32,
    bits_used: u32, // ... in current value
}

impl<'a> KeyBuilder<'a> {
    // Port of: src/gpu/KeyBuilder.h#L20 (chrome/m156)
    /// Creates a builder that appends to `data`.
    #[must_use]
    pub fn new(data: &'a mut Vec<u32>) -> Self {
        Self {
            data,
            cur_value: 0,
            bits_used: 0,
        }
    }

    // Port of: src/gpu/KeyBuilder.h#L26-L41 (chrome/m156)
    /// `addBits`: appends the low `num_bits` bits of `val`. `label` is only used by
    /// [`StringKeyBuilder`].
    #[doc(alias = "addBits")]
    pub fn add_bits(&mut self, num_bits: u32, val: u32, _label: &str) {
        debug_assert!(num_bits > 0 && num_bits <= 32);
        debug_assert!(num_bits == 32 || (val < (1u32 << num_bits)));
        self.cur_value |= val << self.bits_used;
        self.bits_used += num_bits;
        if self.bits_used >= 32 {
            // Overflow, start a new working value
            self.data.push(self.cur_value);
            let excess = self.bits_used - 32;
            self.cur_value = if excess != 0 {
                val >> (num_bits - excess)
            } else {
                0
            };
            self.bits_used = excess;
        }
        debug_assert!(self.cur_value < (1u32 << self.bits_used));
    }

    // Port of: src/gpu/KeyBuilder.h#L43-L48 (chrome/m156)
    /// `addBytes`: appends each byte of `bytes`, one 8-bit field at a time.
    #[doc(alias = "addBytes")]
    pub fn add_bytes(&mut self, bytes: &[u8], label: &str) {
        for &b in bytes {
            self.add_bits(8, u32::from(b), label);
        }
    }

    // Port of: src/gpu/KeyBuilder.h#L50-L52 (chrome/m156)
    /// `addBool`.
    #[doc(alias = "addBool")]
    pub fn add_bool(&mut self, b: bool, label: &str) {
        self.add_bits(1, u32::from(b), label);
    }

    // Port of: src/gpu/KeyBuilder.h#L54-L56 (chrome/m156)
    /// `add32`.
    pub fn add32(&mut self, v: u32, label: &str) {
        self.add_bits(32, v, label);
    }

    // Port of: src/gpu/KeyBuilder.h#L58 (chrome/m156)
    /// `appendComment`: a no-op for the plain builder.
    #[doc(alias = "appendComment")]
    pub fn append_comment(&mut self, _comment: &str) {}

    // Port of: src/gpu/KeyBuilder.h#L62-L68 (chrome/m156)
    /// `flush`: introduces a word boundary in the key. Must be called before the key is used
    /// with any cache.
    pub fn flush(&mut self) {
        if self.bits_used != 0 {
            self.data.push(self.cur_value);
            self.cur_value = 0;
            self.bits_used = 0;
        }
    }
}

impl Drop for KeyBuilder<'_> {
    // Port of: src/gpu/KeyBuilder.h#L24-L27 (chrome/m156)
    fn drop(&mut self) {
        // Ensure that flush was called before we went out of scope
        debug_assert_eq!(self.bits_used, 0);
    }
}

/// A [`KeyBuilder`] that also records a text description of every field.
// Port of: src/gpu/KeyBuilder.h#L86-L99 (chrome/m156)
#[doc(alias = "skgpu::StringKeyBuilder")]
#[derive(Debug)]
pub struct StringKeyBuilder<'a> {
    base: KeyBuilder<'a>,
    description: String,
}

impl<'a> StringKeyBuilder<'a> {
    // Port of: src/gpu/KeyBuilder.h#L88 (chrome/m156)
    /// Creates a string builder that appends to `data`.
    #[must_use]
    pub fn new(data: &'a mut Vec<u32>) -> Self {
        Self {
            base: KeyBuilder::new(data),
            description: String::new(),
        }
    }

    // Port of: src/gpu/KeyBuilder.h#L90-L93 (chrome/m156)
    /// `addBits`, overridden: packs the field and appends `label: val` to the description.
    pub fn add_bits(&mut self, num_bits: u32, val: u32, label: &str) {
        self.base.add_bits(num_bits, val, label);
        // appendf("%.*s: %u\n", label.size(), label.data(), val)
        let _ = writeln!(self.description, "{label}: {val}");
    }

    // Port of: src/gpu/KeyBuilder.h#L43-L48 (chrome/m156)
    /// `addBytes`, dispatched through this builder's own `add_bits`.
    pub fn add_bytes(&mut self, bytes: &[u8], label: &str) {
        for &b in bytes {
            self.add_bits(8, u32::from(b), label);
        }
    }

    // Port of: src/gpu/KeyBuilder.h#L50-L52 (chrome/m156)
    /// `addBool`, dispatched through this builder's own `add_bits`.
    pub fn add_bool(&mut self, b: bool, label: &str) {
        self.add_bits(1, u32::from(b), label);
    }

    // Port of: src/gpu/KeyBuilder.h#L54-L56 (chrome/m156)
    /// `add32`, dispatched through this builder's own `add_bits`.
    pub fn add32(&mut self, v: u32, label: &str) {
        self.add_bits(32, v, label);
    }

    // Port of: src/gpu/KeyBuilder.h#L94-L96 (chrome/m156)
    /// `appendComment`, overridden: appends `comment` and a newline to the description.
    pub fn append_comment(&mut self, comment: &str) {
        let _ = writeln!(self.description, "{comment}");
    }

    // Port of: src/gpu/KeyBuilder.h#L62-L68 (chrome/m156)
    /// `flush`, forwarded to the packed key.
    pub fn flush(&mut self) {
        self.base.flush();
    }

    // Port of: src/gpu/KeyBuilder.h#L97 (chrome/m156)
    /// `description()`.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

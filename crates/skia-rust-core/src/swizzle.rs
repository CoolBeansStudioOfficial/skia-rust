// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/Swizzle.h, src/gpu/Swizzle.cpp

//! `skgpu::Swizzle`: an rgba swizzle, ported minimally (construction, key, string form and
//! [`Swizzle::apply`] to a raster pipeline).

use crate::raster_pipeline::{RasterPipeline, Stage};

/// Represents a rgba swizzle. It can be converted either into a string or a sixteen bit int.
// Port of: src/gpu/Swizzle.h#L27-L99 (chrome/m156)
#[doc(alias = "skgpu::Swizzle")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swizzle {
    key: u16,
}

impl Default for Swizzle {
    /// Equivalent to "rgba".
    fn default() -> Self {
        Swizzle { key: 0x3210 }
    }
}

impl Swizzle {
    // Port of: src/gpu/Swizzle.h#L101-L103 (chrome/m156)
    /// `Swizzle(char r, char g, char b, char a)`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // mirrors the static_cast<uint16_t>; indices are < 6
    pub const fn from_chars(r: char, g: char, b: char, a: char) -> Swizzle {
        Swizzle {
            key: (c_to_i(r) | (c_to_i(g) << 4) | (c_to_i(b) << 8) | (c_to_i(a) << 12)) as u16,
        }
    }

    // Port of: src/gpu/Swizzle.h#L33 (chrome/m156)
    /// `Swizzle(const char c[4])`.
    ///
    /// # Panics
    /// If `c` is not four characters from `r`, `g`, `b`, `a`, `0`, `1`.
    #[must_use]
    pub const fn new(c: &str) -> Swizzle {
        let b = c.as_bytes();
        assert!(b.len() == 4);
        Swizzle::from_chars(b[0] as char, b[1] as char, b[2] as char, b[3] as char)
    }

    // Port of: src/gpu/Swizzle.h#L46 (chrome/m156)
    /// `asKey()`: compact representation of the swizzle suitable for a key.
    #[must_use]
    pub const fn as_key(&self) -> u16 {
        self.key
    }

    // Port of: src/gpu/Swizzle.cpp#L54-L63 (chrome/m156)
    /// `asString()`: 4 char string consisting only of chars 'r', 'g', 'b', 'a', '0', and '1'.
    #[must_use]
    pub fn as_string(&self) -> String {
        let mut key = self.key;
        let mut s = String::with_capacity(4);
        for _ in 0..4 {
            s.push(i_to_c(u32::from(key & 0xf)));
            key >>= 4;
        }
        s
    }

    // Port of: src/gpu/Swizzle.cpp#L17-L52 (chrome/m156)
    /// `apply(SkRasterPipeline*)`.
    pub fn apply(&self, pipeline: &mut RasterPipeline<'_>) {
        match self.key {
            k if k == Swizzle::new("rgba").as_key() => {}
            k if k == Swizzle::new("bgra").as_key() => pipeline.append(Stage::SwapRb),
            k if k == Swizzle::new("aaa1").as_key() => pipeline.append(Stage::AlphaToGray),
            k if k == Swizzle::new("rgb1").as_key() => pipeline.append(Stage::ForceOpaque),
            k if k == Swizzle::new("bgr1").as_key() => {
                pipeline.append(Stage::SwapRb);
                pipeline.append(Stage::ForceOpaque);
            }
            k if k == Swizzle::new("a001").as_key() => pipeline.append(Stage::AlphaToRed),
            _ => {
                // The 4 control bytes are the context: map from packed 16 bits (4 bits/channel
                // holding index values in [0,5]) to 4 bytes holding characters.
                let key = u32::from(self.key);
                let chars = [
                    i_to_c(key & 0xf),
                    i_to_c((key >> 4) & 0xf),
                    i_to_c((key >> 8) & 0xf),
                    i_to_c((key >> 12) & 0xf),
                ]
                .map(|c| c as u8);
                pipeline.append(Stage::Swizzle(chars));
            }
        }
    }

    // Port of: src/gpu/Swizzle.h#L80-L81 (chrome/m156)
    /// `RGBA()`: equivalent to `"rgba"`.
    #[must_use]
    #[doc(alias = "RGBA")]
    pub const fn rgba() -> Swizzle {
        Swizzle::new("rgba")
    }

    // Port of: src/gpu/Swizzle.h#L76 (chrome/m156)
    /// `BGRA()`: equivalent to `"bgra"`.
    #[must_use]
    #[doc(alias = "BGRA")]
    pub const fn bgra() -> Swizzle {
        Swizzle::new("bgra")
    }

    // Port of: src/gpu/Swizzle.h#L77 (chrome/m156)
    /// `RRRA()`: equivalent to `"rrra"`.
    #[must_use]
    #[doc(alias = "RRRA")]
    pub const fn rrra() -> Swizzle {
        Swizzle::new("rrra")
    }

    // Port of: src/gpu/Swizzle.h#L78 (chrome/m156)
    /// `RGB1()`: equivalent to `"rgb1"`.
    #[must_use]
    #[doc(alias = "RGB1")]
    pub const fn rgb1() -> Swizzle {
        Swizzle::new("rgb1")
    }

    // Port of: src/gpu/Swizzle.h#L38 (chrome/m156)
    /// `Concat(a, b)`: the swizzle that applies `b` to the output of `a`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)] // mirrors the uint16_t key (`u32::from` is not const)
    #[doc(alias = "Concat")]
    pub const fn concat(a: &Swizzle, b: &Swizzle) -> Swizzle {
        let mut key: u32 = 0;
        let mut i: u32 = 0;
        while i < 4 {
            let mut idx = ((b.key as u32) >> (4 * i)) & 0xf;
            if idx != c_to_i('0') && idx != c_to_i('1') {
                // Get the index value stored in a at location idx.
                idx = ((a.key as u32) >> (4 * idx)) & 0xf;
            }
            key |= idx << (4 * i);
            i += 1;
        }
        Swizzle { key: key as u16 }
    }

    // Port of: src/gpu/Swizzle.h#L53-L55 (chrome/m156)
    /// `selectChannelInR(i)`: moves the component in index `i` to index 0 and sets all other
    /// channels to 0, i.e. `s[i]000`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // mirrors the static_cast<uint16_t>
    #[doc(alias = "selectChannelInR")]
    pub const fn select_channel_in_r(&self, i: usize) -> Swizzle {
        Swizzle {
            key: (self.channel_index(i)
                | (c_to_i('0') << 4)
                | (c_to_i('0') << 8)
                | (c_to_i('0') << 12)) as u16,
        }
    }

    // Port of: src/gpu/Swizzle.h#L57-L61 (chrome/m156)
    /// `invert()`: as close to an inverse of this swizzle as possible. If the swizzle is
    /// one-to-one, the inverse is exact. Repeated channel values map to the earliest encountered
    /// channel. Channels not present use their default value (0 for RGB and 1 for A).
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)] // mirrors the uint16_t key (`u32::from` is not const)
    pub const fn invert(&self) -> Swizzle {
        // Our starting value will be "0001", but with a blank mask so everything can be
        // overridden by a swizzle component reference.
        let mut key: u32 = Swizzle::new("0001").as_key() as u32;
        let mut mask: u32 = 0;
        let mut i: u32 = 0;
        while i < 4 {
            // This swizzle maps the sampled channel 'idx' to the final channel 'i'.
            let idx = ((self.key as u32) >> (4 * i)) & 0xf;
            // The inverse is to store 'i' at 'idx' in key, if 'idx' is r,g,b,a (in [0,3]).
            if idx <= 3 {
                // Set the 4 bits of the idx channel, unless idx has already been written to
                // (blocked by mask).
                let channel_mask = (0xf << (4 * idx)) & !mask;
                key = (key & !channel_mask) | ((i << (4 * idx)) & channel_mask);
                mask |= 0xf << (4 * idx); // update mask to block future writes
            } else {
                // Push the '0' or '1' constant value into channel i if it hasn't been set yet,
                // which preserves non-default constant values. We don't update the mask so future
                // channel references could still overwrite it with an actual swizzle.
                let channel_mask = (0xf << (4 * i)) & !mask;
                key = (key & !channel_mask) | ((idx << (4 * i)) & channel_mask);
            }
            i += 1;
        }
        Swizzle { key: key as u16 }
    }

    // Port of: src/gpu/Swizzle.h#L67-L71 (chrome/m156)
    /// `applyTo(color)`: applies this swizzle to the input color.
    #[must_use]
    #[doc(alias = "applyTo")]
    pub fn apply_to(&self, color: [f32; 4]) -> [f32; 4] {
        let mut key = u32::from(self.key);
        // Index of the input color that should be mapped to output r.
        let out_r = component_index_to_float(color, (key & 15) as usize);
        key >>= 4;
        let out_g = component_index_to_float(color, (key & 15) as usize);
        key >>= 4;
        let out_b = component_index_to_float(color, (key & 15) as usize);
        key >>= 4;
        let out_a = component_index_to_float(color, (key & 15) as usize);
        [out_r, out_g, out_b, out_a]
    }

    // Port of: src/gpu/Swizzle.h#L118-L120 (chrome/m156)
    /// The index of channel `i` (`fKey` nibble `i`).
    #[allow(clippy::cast_lossless)] // u32::from is not const, and this fn must be const
    const fn channel_index(self, i: usize) -> u32 {
        assert!(i < 4);
        ((self.key >> (4 * i)) & 0xf) as u32 // u32::from is not const
    }
}

// Port of: src/gpu/Swizzle.h#L126-L138 (chrome/m156)
fn component_index_to_float(color: [f32; 4], idx: usize) -> f32 {
    if idx <= 3 {
        return color[idx];
    }
    if idx == c_to_i('1') as usize {
        return 1.0;
    }
    if idx == c_to_i('0') as usize {
        return 0.0;
    }
    unreachable!("invalid swizzle component index {idx}")
}

// Port of: src/gpu/Swizzle.h#L140-L151 (chrome/m156)
const fn c_to_i(c: char) -> u32 {
    match c {
        'r' => 0,
        'g' => 1,
        'b' => 2,
        'a' => 3,
        '0' => 4,
        '1' => 5,
        _ => panic!("invalid swizzle character"),
    }
}

// Port of: src/gpu/Swizzle.h#L153-L163 (chrome/m156)
const fn i_to_c(idx: u32) -> char {
    match idx {
        0 => 'r',
        1 => 'g',
        2 => 'b',
        3 => 'a',
        4 => '0',
        5 => '1',
        _ => panic!("invalid swizzle index"),
    }
}

/// Swizzles the byte order of 32-bit pixels, swapping R and B (RGBA <-> BGRA): `SkSwapRB`
/// from `include/core/SkSwizzle.h`, on the `RGBA_to_BGRA` kernel.
///
/// - `dest` destination pixels
/// - `src` source pixels
///
/// # Panics
/// If `dest` and `src` have different lengths.
// Port of: src/core/SkSwizzle.cpp#L12-L14 (chrome/m156)
#[doc(alias = "SkSwapRB")]
pub fn swap_rb(dest: &mut [u32], src: &[u32]) {
    assert_eq!(dest.len(), src.len());
    skia_rust_simd::swizzle::rgba_to_bgra_words(dest, src);
}

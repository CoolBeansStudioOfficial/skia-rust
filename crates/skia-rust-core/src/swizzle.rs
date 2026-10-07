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

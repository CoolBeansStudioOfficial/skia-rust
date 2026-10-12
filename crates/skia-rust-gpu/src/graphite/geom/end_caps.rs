// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/EndCaps.h

//! [`EndCaps`]: the antialiased boundary runs produced by sparse strips rendering.

use std::sync::Arc;

use crate::graphite::sparse_strips::config::SparseStripConfig;
use crate::graphite::texture_proxy::TextureProxy;

/// `EndCaps::EndCap`: the antialiased portion of the path rendering. Spans a variable non-zero
/// number of tiles.
// Port of: src/gpu/graphite/geom/EndCaps.h#L27-L42 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndCap {
    /// `fX`: top left coordinate of the `EndCap`.
    pub x: u16,
    /// `fY`: top left coordinate of the `EndCap`.
    pub y: u16,
    /// `fWidth`: the width of the `EndCap` in pixels.
    pub width: u16,
    /// `fTexPage`: the associated page in the `EndCap`'s backing texture.
    pub tex_page: u16,
    /// `fAlphaIndex`: the `EndCap`'s offset into the alpha buffer.
    pub alpha_index: i32,
    /// `fPadding`: pads to vec4 alignment.
    pub padding: u32,
}

impl EndCap {
    /// `EndCap(x, y, width, alphaIndex, texPage)`.
    // Port of: src/gpu/graphite/geom/EndCaps.h#L29-L31 (chrome/m156)
    #[must_use]
    pub fn new(x: u16, y: u16, width: u16, alpha_index: i32, tex_page: u16) -> Self {
        Self {
            x,
            y,
            width,
            tex_page,
            alpha_index,
            padding: 0xffff_ffff,
        }
    }
}

// Port of: src/gpu/graphite/geom/EndCaps.h#L43 (chrome/m156)
const _: () = assert!(size_of::<EndCap>() == 16);

/// Number of alpha texture pages (`SparseStripConfig::kMaxTexturePages`) as an array length.
const K_PAGES: usize = SparseStripConfig::K_MAX_TEXTURE_PAGES as usize;

/// Container holding the "`EndCaps`" produced by sparse strips rendering. An `EndCap` stores the
/// antialiased portion of the path rendering, and spans a variable non-zero number of tiles.
// Port of: src/gpu/graphite/geom/EndCaps.h#L25-L98 (chrome/m156)
#[doc(alias = "skgpu::graphite::EndCaps")]
#[derive(Clone, Debug)]
pub struct EndCaps {
    caps: Vec<EndCap>,
    proxies: [Option<Arc<TextureProxy>>; K_PAGES],
    draw_start_index: i32,
    first_null_cap_index: i32,
}

impl Default for EndCaps {
    fn default() -> Self {
        Self {
            caps: Vec::new(),
            proxies: [None, None],
            draw_start_index: 0,
            first_null_cap_index: Self::K_INVALID_INDEX,
        }
    }
}

impl EndCaps {
    /// `kInvalidIndex`.
    const K_INVALID_INDEX: i32 = -1;

    /// `EndCaps()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `addCap(x, y, width, alphaIndex, texPage)`.
    // Port of: src/gpu/graphite/geom/EndCaps.h#L47-L54 (chrome/m156)
    #[doc(alias = "addCap")]
    pub fn add_cap(&mut self, x: u16, y: u16, width: u16, alpha_index: i32, tex_page: u16) {
        self.caps
            .push(EndCap::new(x, y, width, alpha_index, tex_page));
    }

    /// `markFirstNullCap()`: records the index of the next cap as the first one that has no
    /// texture, unless a first one is already recorded.
    // Port of: src/gpu/graphite/geom/EndCaps.h#L56-L60 (chrome/m156)
    #[doc(alias = "markFirstNullCap")]
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)] // mirrors the C++ `static_cast<int32_t>(fCaps.size())`
    pub fn mark_first_null_cap(&mut self) {
        if self.first_null_cap_index == Self::K_INVALID_INDEX {
            self.first_null_cap_index = self.caps.len() as i32;
        }
    }

    /// `setProxy(slot, proxy)`.
    // Port of: src/gpu/graphite/geom/EndCaps.h#L62-L65 (chrome/m156)
    #[doc(alias = "setProxy")]
    pub fn set_proxy(&mut self, slot: usize, proxy: Option<Arc<TextureProxy>>) {
        debug_assert!(slot < K_PAGES);
        self.proxies[slot] = proxy;
    }

    /// `caps()`.
    #[must_use]
    pub fn caps(&self) -> &[EndCap] {
        &self.caps
    }

    /// `caps()`, to change the caps.
    #[must_use]
    pub fn caps_mut(&mut self) -> &mut Vec<EndCap> {
        &mut self.caps
    }

    /// `proxies()`.
    #[must_use]
    pub fn proxies(&self) -> &[Option<Arc<TextureProxy>>; K_PAGES] {
        &self.proxies
    }

    /// `empty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// `size()`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.caps.len()
    }

    /// `hasNullCaps()`.
    #[must_use]
    pub fn has_null_caps(&self) -> bool {
        self.first_null_cap_index != Self::K_INVALID_INDEX
    }

    /// `firstNullCapIndex()`: -1 if there is none.
    #[must_use]
    pub fn first_null_cap_index(&self) -> i32 {
        self.first_null_cap_index
    }

    /// `setFirstNullCapIndex(index)`.
    pub fn set_first_null_cap_index(&mut self, index: i32) {
        self.first_null_cap_index = index;
    }

    /// `clearNullCaps()`.
    pub fn clear_null_caps(&mut self) {
        self.first_null_cap_index = Self::K_INVALID_INDEX;
    }

    /// `drawStartIndex()`.
    #[must_use]
    pub fn draw_start_index(&self) -> i32 {
        self.draw_start_index
    }

    /// `drawEndIndex()`.
    #[must_use]
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)] // mirrors the C++ `static_cast<int32_t>(fCaps.size())`
    pub fn draw_end_index(&self) -> i32 {
        if self.has_null_caps() {
            self.first_null_cap_index
        } else {
            self.caps.len() as i32
        }
    }

    /// `setDrawStartIndex(index)`.
    pub fn set_draw_start_index(&mut self, index: i32) {
        self.draw_start_index = index;
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.caps.clear();
        for proxy in &mut self.proxies {
            *proxy = None;
        }
        self.draw_start_index = 0;
        self.first_null_cap_index = Self::K_INVALID_INDEX;
    }
}

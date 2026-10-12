// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/WideTiles.h

//! [`WideTiles`]: the solid inner fill of a path produced by sparse strips rendering.

/// `WideTiles::WideTile`: the inner fill of a path, which requires no antialiasing. Spans a
/// variable non-zero number of tiles.
// Port of: src/gpu/graphite/geom/WideTiles.h#L21-L26 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideTile {
    /// `fX`: top left coordinate of the `WideTile`.
    pub x: u16,
    /// `fY`: top left coordinate of the `WideTile`.
    pub y: u16,
    /// `fWidth`: the width of the `WideTile` in pixels.
    pub width: u16,
    /// `fPadding`: pads to vec2 alignment.
    pub padding: u16,
}

impl WideTile {
    /// `WideTile(x, y, width)`.
    // Port of: src/gpu/graphite/geom/WideTiles.h#L20-L21 (chrome/m156)
    #[must_use]
    pub fn new(x: u16, y: u16, width: u16) -> Self {
        Self {
            x,
            y,
            width,
            padding: 0xffff,
        }
    }
}

/// Container holding the "`WideTiles`" produced by sparse strips rendering. A `WideTile` stores
/// the inner fill of a path which requires no antialiasing. Also spans a variable non-zero number
/// of tiles.
// Port of: src/gpu/graphite/geom/WideTiles.h#L19-L44 (chrome/m156)
#[doc(alias = "skgpu::graphite::WideTiles")]
#[derive(Clone, Debug, Default)]
pub struct WideTiles {
    tiles: Vec<WideTile>,
}

impl WideTiles {
    /// `WideTiles()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `addTile(x, y, width)`.
    // Port of: src/gpu/graphite/geom/WideTiles.h#L29-L31 (chrome/m156)
    #[doc(alias = "addTile")]
    pub fn add_tile(&mut self, x: u16, y: u16, width: u16) {
        self.tiles.push(WideTile::new(x, y, width));
    }

    /// `tiles()`.
    #[must_use]
    pub fn tiles(&self) -> &[WideTile] {
        &self.tiles
    }

    /// `tiles()`, to change the tiles.
    #[must_use]
    pub fn tiles_mut(&mut self) -> &mut Vec<WideTile> {
        &mut self.tiles
    }

    /// `empty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// `size()`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.tiles.clear();
    }
}

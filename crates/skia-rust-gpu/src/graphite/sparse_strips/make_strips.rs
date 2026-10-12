// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/MakeStrips.h

//! [`MakeStrips`]: turns the sorted tiles of a path into `EndCaps` and `WideTiles`.
//!
//! At this point in the sparse strips pipeline, the path has been stroked, flattened into a
//! polyline, tiled, and sorted. Now, the tiles are consumed by `MakeStrips::*` to produce:
//!
//!  1) `EndCaps`: Runs of rasterized boundary tiles containing fractional per-pixel coverage
//!     masks, which are stored in the alpha atlas.
//!
//!  2) `WideTiles`: Contiguous interior regions with solid 100% full coverage, which are rendered
//!     directly as solid fill rectangles without sampling the atlas textures.
//!
//! Two things are required here:
//!
//!  1) Coverage Resolution: Multiple line segments often intersect the exact same spatial tile.
//!     Because the input tiles are generated per-line-segment, the individual winding
//!     contributions must be combined to produce the final coverage mask for that location.
//!
//!  2) Geometry Generation: Because the incoming tiles are sorted by y, then x, runs of contiguous
//!     tiles identify boundary edge tiles (`EndCaps`), while gaps between runs are checked against
//!     the winding fill rule to identify solid interior fill spans (`WideTiles`).
//!
//! The pipeline emits two geometric primitives directly into their respective render buffers:
//!
//!  1) `EndCap` (x, y, width, alphaIndex, texPage):
//!     - Coordinates (x, y): The top-left pixel coordinate of the first tile in a contiguous run
//!       of boundary/edge tiles.
//!     - Width (width): The total horizontal span in pixels (contiguous tiles * kTileWidth).
//!     - Alpha Index (alphaIndex): Offset into the alpha atlas page.
//!     - Texture Page (texPage): Associated texture page index in the atlas manager.
//!
//!  2) `WideTile` (x, y, width):
//!     - Coordinates (x, y): The top-left coordinate of the solid fill rectangle.
//!     - Width (width): The total horizontal span in pixels. Rendered with 100% alpha without
//!       atlas sampling.
//!
//! The casts between `u16` and `i32` below mirror the C++ integer promotions: tile coordinates are
//! `uint16_t`, their products with the tile size are `int`, and they are converted back to
//! `uint16_t` where they are passed on.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;

use crate::graphite::geom::end_caps::EndCaps;
use crate::graphite::geom::wide_tiles::WideTiles;
use crate::graphite::recorder::Recorder;
use crate::graphite::sparse_strips::alpha_atlas_manager::AlphaAtlasManager;
use crate::graphite::sparse_strips::polyline::Polyline;
use crate::graphite::sparse_strips::strip_processor_scalar::StripProcessorScalar;
use crate::graphite::sparse_strips::strip_processor_simd::StripProcessorSimd;
use crate::graphite::sparse_strips::tiler::{Tile, Tiles};
use crate::graphite::sparse_strips::types::MsaaExactMaskObserver;

/// What `MakeStrips::TraverseCPU` needs of a strip processor (`StripProcessorScalar` and
/// `StripProcessorSimd` in the C++, which the template resolves by duck typing).
pub trait StripProcessor {
    /// `resolveWindingToAlpha(tileAlphaBase)`: converts the subsample windings to the alphas of
    /// the tile.
    fn resolve_winding_to_alpha(&mut self, tile_alpha_base: &mut [u8]);
    /// `clearWithCoarseWinding()`.
    fn clear_with_coarse_winding(&mut self);
    /// `clearWindingForNewRow()`.
    fn clear_winding_for_new_row(&mut self);
    /// `ShouldFill(w)`: whether the winding number is inside the fill rule.
    fn should_fill(w: i32) -> bool;
    /// `coarseWinding()`.
    fn coarse_winding(&self) -> i32;
    /// `setCoarseWinding(val)`.
    fn set_coarse_winding(&mut self, val: i32);
    /// `rasterizeLineToTile(tile, tileBounds)`.
    fn rasterize_line_to_tile(&mut self, tile: &Tile, tile_bounds: [Point; 2]);
}

/// `MakeStrips`.
// Port of: src/gpu/graphite/sparse_strips/MakeStrips.h#L117-L425 (chrome/m156)
#[doc(alias = "skgpu::graphite::MakeStrips")]
#[derive(Clone, Copy, Debug)]
pub struct MakeStrips;

impl MakeStrips {
    /// `MsaaScalar<kTileWidth, kTileHeight>(...)`: the strips with the scalar processor. Returns
    /// whether the atlas had space for all the alphas.
    // Port of: src/gpu/graphite/sparse_strips/MakeStrips.h#L119-L155 (chrome/m156)
    #[doc(alias = "MsaaScalar")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn msaa_scalar<'a, const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
        tile_container: &Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>,
        wides: &mut WideTiles,
        ends: &mut EndCaps,
        atlas_manager: &mut AlphaAtlasManager,
        recorder: &Recorder,
        fill_type: PathFillType,
        polyline: &'a Polyline,
        mask_lut: &'a [u8],
        viewport_width: u16,
        viewport_height: u16,
        observer: Option<MsaaExactMaskObserver<'a>>,
    ) -> bool {
        let (is_winding, is_inverse) = Self::dispatch(fill_type);
        if is_winding {
            let mut processor = StripProcessorScalar::<K_TILE_WIDTH, K_TILE_HEIGHT, true>::new(
                is_inverse, polyline, mask_lut, observer,
            );
            Self::traverse_cpu(
                tile_container,
                wides,
                ends,
                atlas_manager,
                recorder,
                viewport_width,
                viewport_height,
                is_inverse,
                &mut processor,
            )
        } else {
            let mut processor = StripProcessorScalar::<K_TILE_WIDTH, K_TILE_HEIGHT, false>::new(
                is_inverse, polyline, mask_lut, observer,
            );
            Self::traverse_cpu(
                tile_container,
                wides,
                ends,
                atlas_manager,
                recorder,
                viewport_width,
                viewport_height,
                is_inverse,
                &mut processor,
            )
        }
    }

    /// `MsaaSimd<kTileWidth, kTileHeight>(...)`: the strips with the SWAR processor. Returns
    /// whether the atlas had space for all the alphas.
    // Port of: src/gpu/graphite/sparse_strips/MakeStrips.h#L157-L193 (chrome/m156)
    #[doc(alias = "MsaaSimd")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn msaa_simd<'a, const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
        tile_container: &Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>,
        wides: &mut WideTiles,
        ends: &mut EndCaps,
        atlas_manager: &mut AlphaAtlasManager,
        recorder: &Recorder,
        fill_type: PathFillType,
        polyline: &'a Polyline,
        mask_lut: &'a [u8],
        viewport_width: u16,
        viewport_height: u16,
        observer: Option<MsaaExactMaskObserver<'a>>,
    ) -> bool {
        let (is_winding, is_inverse) = Self::dispatch(fill_type);
        if is_winding {
            let mut processor = StripProcessorSimd::<K_TILE_WIDTH, K_TILE_HEIGHT, true>::new(
                is_inverse, polyline, mask_lut, observer,
            );
            Self::traverse_cpu(
                tile_container,
                wides,
                ends,
                atlas_manager,
                recorder,
                viewport_width,
                viewport_height,
                is_inverse,
                &mut processor,
            )
        } else {
            let mut processor = StripProcessorSimd::<K_TILE_WIDTH, K_TILE_HEIGHT, false>::new(
                is_inverse, polyline, mask_lut, observer,
            );
            Self::traverse_cpu(
                tile_container,
                wides,
                ends,
                atlas_manager,
                recorder,
                viewport_width,
                viewport_height,
                is_inverse,
                &mut processor,
            )
        }
    }

    /// `Dispatch(fillType, f)`: the `(isWinding, isInverse)` pair of the fill type, which the C++
    /// passes to `f` as a compile time constant and a runtime flag.
    // Port of: src/gpu/graphite/sparse_strips/MakeStrips.h#L196-L212 (chrome/m156)
    fn dispatch(fill_type: PathFillType) -> (bool, bool) {
        match fill_type {
            PathFillType::Winding => (true, false),
            PathFillType::InverseWinding => (true, true),
            PathFillType::EvenOdd => (false, false),
            PathFillType::InverseEvenOdd => (false, true),
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/MakeStrips.h#L214-L222 (chrome/m156)
    fn emit_background<const K_TILE_HEIGHT: u16>(
        wides: &mut WideTiles,
        start: u16,
        end: u16,
        width: u16,
    ) {
        let mut row = start;
        while row < end {
            wides.add_tile(0, row, width);
            // `row += kTileHeight` on a `uint16_t` wraps.
            row = row.wrapping_add(K_TILE_HEIGHT);
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/MakeStrips.h#L224-L246 (chrome/m156)
    fn finalize_run<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
        run_start_x: u16,
        prev_tile: Tile,
        atlas_manager: &mut AlphaAtlasManager,
        recorder: &Recorder,
        ends: &mut EndCaps,
    ) -> bool {
        let end_cap_x = (i32::from(run_start_x) * i32::from(K_TILE_WIDTH)) as u16;
        let end_cap_width = ((i32::from(prev_tile.x) - i32::from(run_start_x) + 1)
            * i32::from(K_TILE_WIDTH)) as u16;

        let Some((alpha_index, tex_page)) = atlas_manager.finalize_run(recorder) else {
            return false;
        };
        if tex_page == AlphaAtlasManager::K_NULL_SLOT {
            ends.mark_first_null_cap();
        }
        ends.add_cap(
            end_cap_x,
            (i32::from(prev_tile.y) * i32::from(K_TILE_HEIGHT)) as u16,
            end_cap_width,
            alpha_index,
            tex_page,
        );
        true
    }

    /// While the underlying implementation may be scalar or SIMD, the core traversal across the
    /// tiles is identical. To reiterate, the goal of `MakeStrips` is twofold:
    /// 1) Combine polyline segments at the same spatial tile to produce the final coverage.
    /// 2) Generate `EndCaps` for antialiased boundary runs and `WideTiles` for solid interior
    ///    fills.
    ///
    /// To do this in a single pass, the traversal treats the sorted tile stream as a state
    /// machine governed by three transition events:
    ///
    /// 1) Tile Start (`tileStart`):
    ///    Triggered when the current tile's x or y differs from the previous tile.
    ///    Action: All overlapping segments at the previous spatial coordinate have been
    ///    processed. The accumulated coverage is resolved into pixel alpha and pushed to the
    ///    atlas buffer. If the new tile is on the same row, it is seeded with the carried coarse
    ///    winding.
    ///
    /// 2) Segment Start (`segStart`):
    ///    Triggered by a `rowStart`, OR when the current tile's x coordinate skips forward by more
    ///    than 1 (a non-contiguous gap in the same row).
    ///    Action:
    ///    a) Finalizes the preceding contiguous boundary run (`finalizeRun`), committing its
    ///    alpha buffer in the atlas manager and emitting an `EndCap`.
    ///    b) If the coarse winding indicates an interior fill, emits a solid `WideTile` covering
    ///    the gap up to the current tile.
    ///    c) If `rowStart`, closes out the previous row (emitting trailing inverse fills if
    ///    needed), resets the coarse winding to 0, emits any inverse background rows, and
    ///    begins the new row.
    // Port of: src/gpu/graphite/sparse_strips/MakeStrips.h#L273-L424 (chrome/m156)
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // mirrors the C++ signature and body
    fn traverse_cpu<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16, P: StripProcessor>(
        tile_container: &Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>,
        wides: &mut WideTiles,
        ends: &mut EndCaps,
        atlas_manager: &mut AlphaAtlasManager,
        recorder: &Recorder,
        viewport_width: u16,
        viewport_height: u16,
        is_inverse: bool,
        processor: &mut P,
    ) -> bool {
        let tile_pixel_count = i32::from(K_TILE_WIDTH) * i32::from(K_TILE_HEIGHT);
        let tile_w = i32::from(K_TILE_WIDTH);
        let tile_h = i32::from(K_TILE_HEIGHT);

        let tiles = tile_container.get_tiles();
        if tiles.is_empty() {
            if is_inverse {
                Self::emit_background::<K_TILE_HEIGHT>(wides, 0, viewport_height, viewport_width);
            }
            return true;
        }

        let total_count = tiles.len();
        let mut prev_tile = tiles[0];

        let mut run_start_x = prev_tile.x;

        if is_inverse {
            Self::emit_background::<K_TILE_HEIGHT>(
                wides,
                0,
                (i32::from(prev_tile.y) * tile_h) as u16,
                viewport_width,
            );
            if prev_tile.x > 0 {
                wides.add_tile(
                    0,
                    (i32::from(prev_tile.y) * tile_h) as u16,
                    (i32::from(prev_tile.x) * tile_w) as u16,
                );
            }
        }

        let prev_x = (i32::from(prev_tile.x) * tile_w) as f32;
        let prev_y = (i32::from(prev_tile.y) * tile_h) as f32;
        let mut tile_bounds = [
            Point::new(prev_x, prev_y),
            Point::new(
                prev_x + f32::from(K_TILE_WIDTH),
                prev_y + f32::from(K_TILE_HEIGHT),
            ),
        ];

        for &tile in tiles.iter().take(total_count) {
            // Determine tile traversal events
            let row_start = tile.y != prev_tile.y;
            let tile_start = tile.x != prev_tile.x || row_start;
            let seg_start =
                tile_start && (row_start || (i32::from(tile.x) != i32::from(prev_tile.x) + 1));

            if tile_start {
                // Moving to a new tile implies that all previous tile's coverage has been
                // combined, resolve the coverage mask winding to alpha, then clear it.
                let Some(dst) = atlas_manager.request_alpha_space(recorder, tile_pixel_count)
                else {
                    return false;
                };
                processor.resolve_winding_to_alpha(dst);
                if !row_start {
                    // If we're not a row start, carry the scanline winding by seeding the
                    // coverage mask with the coarse winding.
                    processor.clear_with_coarse_winding();
                }
            }

            if seg_start {
                // 1. Finalize the contiguous EndCap run
                if !Self::finalize_run::<K_TILE_WIDTH, K_TILE_HEIGHT>(
                    run_start_x,
                    prev_tile,
                    atlas_manager,
                    recorder,
                    ends,
                ) {
                    return false;
                }

                let run_end_x = ((i32::from(prev_tile.x) + 1) * tile_w) as u16;

                // 2. If winding is inside, emit the solid WideTile interior span
                let should_fill = P::should_fill(processor.coarse_winding()) ^ is_inverse;
                if should_fill && !row_start {
                    let wide_end_x = (i32::from(tile.x) * tile_w) as u16;
                    if wide_end_x > run_end_x {
                        wides.add_tile(
                            run_end_x,
                            (i32::from(prev_tile.y) * tile_h) as u16,
                            wide_end_x - run_end_x,
                        );
                    }
                }

                // 3. Handle Row Breaks
                if row_start {
                    if should_fill && is_inverse && viewport_width > run_end_x {
                        wides.add_tile(
                            run_end_x,
                            (i32::from(prev_tile.y) * tile_h) as u16,
                            viewport_width - run_end_x,
                        );
                    }

                    // Reset coarse winding for the new row
                    processor.set_coarse_winding(0);
                    processor.clear_winding_for_new_row();

                    if is_inverse {
                        Self::emit_background::<K_TILE_HEIGHT>(
                            wides,
                            ((i32::from(prev_tile.y) + 1) * tile_h) as u16,
                            (i32::from(tile.y) * tile_h) as u16,
                            viewport_width,
                        );
                        if tile.x > 0 {
                            wides.add_tile(
                                0,
                                (i32::from(tile.y) * tile_h) as u16,
                                (i32::from(tile.x) * tile_w) as u16,
                            );
                        }
                    }
                }

                // 4. Start a new contiguous alpha run
                run_start_x = tile.x;
            }

            prev_tile = tile;

            // Lazily recalculate tile bounds only if we have moved to a new tile
            if tile_start {
                let x = (i32::from(tile.x) * tile_w) as f32;
                let y = (i32::from(tile.y) * tile_h) as f32;
                tile_bounds = [
                    Point::new(x, y),
                    Point::new(x + f32::from(K_TILE_WIDTH), y + f32::from(K_TILE_HEIGHT)),
                ];
            }

            processor.rasterize_line_to_tile(&tile, tile_bounds);
        }

        // Process the last tile and finalize
        let Some(dst) = atlas_manager.request_alpha_space(recorder, tile_pixel_count) else {
            return false;
        };
        processor.resolve_winding_to_alpha(dst);
        if !Self::finalize_run::<K_TILE_WIDTH, K_TILE_HEIGHT>(
            run_start_x,
            prev_tile,
            atlas_manager,
            recorder,
            ends,
        ) {
            return false;
        }

        let should_fill = P::should_fill(processor.coarse_winding()) ^ is_inverse;
        if is_inverse {
            if should_fill {
                let run_end_x = ((i32::from(prev_tile.x) + 1) * tile_w) as u16;
                if viewport_width > run_end_x {
                    wides.add_tile(
                        run_end_x,
                        (i32::from(prev_tile.y) * tile_h) as u16,
                        viewport_width - run_end_x,
                    );
                }
            }
            Self::emit_background::<K_TILE_HEIGHT>(
                wides,
                ((i32::from(prev_tile.y) + 1) * tile_h) as u16,
                viewport_height,
                viewport_width,
            );
        }
        true
    }
}

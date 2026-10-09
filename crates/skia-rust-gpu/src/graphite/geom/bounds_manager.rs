// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/BoundsManager.h

//! `skgpu::graphite::BoundsManager`: an acceleration structure for device-space pixel bounds
//! queries, tracking the highest `CompressedPaintersOrder` drawn over each area.
//!
//! The C++ class is a virtual base with four subclasses. Here it is a trait with the four
//! implementations; `HybridBoundsManager` picks between its brute-force and grid members with an
//! enum, as the C++ `fCurrentManager` pointer does.

use skia_rust_core::scalar::scalar_ceil_to_int;
use skia_rust_core::size::ISize;
use skia_rust_simd::vx::{Float4, Int4};

use crate::graphite::draw_order::CompressedPaintersOrder;
use crate::graphite::geom::rect::Rect;

// TODO: Select one most-effective BoundsManager implementation, make it the only option, and remove
// virtual-ness. For now, this seems useful for correctness testing by comparing against trivial
// implementations and for identifying how much "smarts" are actually worthwhile.

/// Acceleration structure for device-space pixel bounds queries.
// Port of: src/gpu/graphite/geom/BoundsManager.h#L37-L46 (chrome/m156)
#[doc(alias = "skgpu::graphite::BoundsManager")]
pub trait BoundsManager {
    /// `getMostRecentDraw(bounds)`: the highest order of any recorded draw intersecting `bounds`.
    fn get_most_recent_draw(&self, bounds: Rect) -> CompressedPaintersOrder;

    /// `recordDraw(bounds, order)`.
    fn record_draw(&mut self, bounds: Rect, order: CompressedPaintersOrder);

    /// `reset()`.
    fn reset(&mut self);
}

// A BoundsManager that produces exact painter's order and assumes nothing is occluded.
// Port of: src/gpu/graphite/geom/BoundsManager.h#L53-L74 (chrome/m156)
/// A `BoundsManager` that produces exact painter's order and assumes nothing is occluded.
#[doc(alias = "skgpu::graphite::NaiveBoundsManager")]
#[derive(Debug)]
pub struct NaiveBoundsManager {
    latest_draw: CompressedPaintersOrder,
}

impl NaiveBoundsManager {
    /// Creates a manager with no draws recorded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            latest_draw: CompressedPaintersOrder::first(),
        }
    }
}

impl Default for NaiveBoundsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BoundsManager for NaiveBoundsManager {
    fn get_most_recent_draw(&self, _bounds: Rect) -> CompressedPaintersOrder {
        self.latest_draw
    }

    fn record_draw(&mut self, _bounds: Rect, order: CompressedPaintersOrder) {
        if self.latest_draw < order {
            self.latest_draw = order;
        }
    }

    fn reset(&mut self) {
        self.latest_draw = CompressedPaintersOrder::first();
    }
}

// A BoundsManager that tracks every draw and can exactly determine all queries using a brute force
// search.
// Port of: src/gpu/graphite/geom/BoundsManager.h#L78-L122 (chrome/m156)
/// A `BoundsManager` that tracks every draw and can exactly determine all queries using a brute
/// force search.
#[doc(alias = "skgpu::graphite::BruteForceBoundsManager")]
#[derive(Debug, Default)]
pub struct BruteForceBoundsManager {
    // fRects and fOrders are parallel, but kept separate as in the C++ (SkTBlockList).
    rects: Vec<Rect>,
    orders: Vec<CompressedPaintersOrder>,
}

impl BruteForceBoundsManager {
    /// Creates a manager with no draws recorded.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `count()`: the number of recorded draws.
    #[must_use]
    pub fn count(&self) -> usize {
        self.rects.len()
    }

    /// `replayDraws(manager)`: records every draw into `manager`, in order.
    pub fn replay_draws(&self, manager: &mut dyn BoundsManager) {
        for (r, order) in self.rects.iter().zip(&self.orders) {
            manager.record_draw(*r, *order);
        }
    }
}

impl BoundsManager for BruteForceBoundsManager {
    fn get_most_recent_draw(&self, bounds: Rect) -> CompressedPaintersOrder {
        debug_assert_eq!(self.rects.len(), self.orders.len());

        let bounds_complement = crate::graphite::geom::rect::ComplementRect::new(bounds);
        let mut max = CompressedPaintersOrder::first();
        for (r, order) in self.rects.iter().zip(&self.orders) {
            if r.intersects_complement(bounds_complement) && max < *order {
                max = *order;
            }
        }
        max
    }

    fn record_draw(&mut self, bounds: Rect, order: CompressedPaintersOrder) {
        self.rects.push(bounds);
        self.orders.push(order);
    }

    fn reset(&mut self) {
        self.rects.clear();
        self.orders.clear();
    }
}

// A BoundsManager that tracks highest CompressedPaintersOrder over a uniform spatial grid.
// Port of: src/gpu/graphite/geom/BoundsManager.h#L124-L240 (chrome/m156)
/// A `BoundsManager` that tracks highest `CompressedPaintersOrder` over a uniform spatial grid.
#[doc(alias = "skgpu::graphite::GridBoundsManager")]
#[derive(Debug)]
pub struct GridBoundsManager {
    scale_x: f32,
    scale_y: f32,

    grid_width: i32,
    grid_height: i32,

    nodes: Vec<CompressedPaintersOrder>,
}

impl GridBoundsManager {
    /// `Make(deviceSize, gridSize)`: `grid_size` is the number of cells in the X and Y directions,
    /// splitting the pixels from `[0,0]` to `device_size` into uniformly-sized cells.
    #[must_use]
    pub fn make(device_size: ISize, grid_size: ISize) -> Box<Self> {
        debug_assert!(device_size.width > 0 && device_size.height > 0);
        debug_assert!(grid_size.width >= 1 && grid_size.height >= 1);

        Box::new(Self::new(device_size, grid_size))
    }

    /// `Make(deviceSize, int gridSize)`: a square grid.
    #[must_use]
    pub fn make_square(device_size: ISize, grid_size: i32) -> Box<Self> {
        Self::make(
            device_size,
            ISize {
                width: grid_size,
                height: grid_size,
            },
        )
    }

    /// `MakeRes(deviceSize, gridCellSize, maxGridSize)`.
    #[must_use]
    pub fn make_res(mut device_size: ISize, grid_cell_size: i32, max_grid_size: i32) -> Box<Self> {
        debug_assert!(device_size.width > 0 && device_size.height > 0);
        debug_assert!(grid_cell_size >= 1);

        let mut grid_width = scalar_ceil_to_int(device_size.width as f32 / grid_cell_size as f32);
        if max_grid_size > 0 && grid_width > max_grid_size {
            // We'd have too many sizes so clamp the grid resolution, leave the device size alone
            // since the grid cell size can't be preserved anyways.
            grid_width = max_grid_size;
        } else {
            // Pad out the device size to keep cell size the same
            device_size.width = grid_width * grid_cell_size;
        }

        let mut grid_height = scalar_ceil_to_int(device_size.height as f32 / grid_cell_size as f32);
        if max_grid_size > 0 && grid_height > max_grid_size {
            grid_height = max_grid_size;
        } else {
            device_size.height = grid_height * grid_cell_size;
        }
        Self::make(
            device_size,
            ISize {
                width: grid_width,
                height: grid_height,
            },
        )
    }

    // Port of: src/gpu/graphite/geom/BoundsManager.h#L215-L223 (chrome/m156)
    fn new(device_size: ISize, grid_size: ISize) -> Self {
        let mut grid = Self {
            scale_x: grid_size.width as f32 / device_size.width as f32,
            scale_y: grid_size.height as f32 / device_size.height as f32,
            grid_width: grid_size.width,
            grid_height: grid_size.height,
            nodes: vec![
                CompressedPaintersOrder::first();
                grid_size.width as usize * grid_size.height as usize
            ],
        };
        // Reset is needed to zero-out the uninitialized fNodes values.
        grid.reset();
        grid
    }

    // Port of: src/gpu/graphite/geom/BoundsManager.h#L225-L231 (chrome/m156)
    fn get_grid_coords(&self, bounds: Rect) -> Int4 {
        // Normalize bounds by 1/wh of device bounds, then scale up to number of cells per side.
        // fScaleXY includes both 1/wh and the grid dimension scaling, then clamp to [0, gridDim-1].
        let scaled = bounds.ltrb() * Float4::new(
            self.scale_x,
            self.scale_y,
            self.scale_x,
            self.scale_y,
        );
        scaled
            .cast::<i32>()
            .pin(
                Int4::splat(0),
                Int4::new(
                    self.grid_width - 1,
                    self.grid_height - 1,
                    self.grid_width - 1,
                    self.grid_height - 1,
                ),
            )
    }

    // Linear index of the cell at `(x, y)` in `nodes`.
    fn cell_index(&self, x: i32, y: i32) -> usize {
        (y * self.grid_width + x) as usize
    }
}

impl BoundsManager for GridBoundsManager {
    fn get_most_recent_draw(&self, bounds: Rect) -> CompressedPaintersOrder {
        debug_assert!(!bounds.is_empty_negative_or_nan());

        let ltrb = self.get_grid_coords(bounds);
        let base = self.cell_index(ltrb[0], ltrb[1]);
        let h = ltrb[3] - ltrb[1];
        let w = ltrb[2] - ltrb[0];

        let mut max = CompressedPaintersOrder::first();
        let mut p = base;
        for _y in 0..=h {
            for x in 0..=w {
                let v = self.nodes[p + x as usize];
                if v > max {
                    max = v;
                }
            }
            p += self.grid_width as usize;
        }

        max
    }

    fn record_draw(&mut self, bounds: Rect, order: CompressedPaintersOrder) {
        debug_assert!(!bounds.is_empty_negative_or_nan());

        let ltrb = self.get_grid_coords(bounds);
        let base = self.cell_index(ltrb[0], ltrb[1]);
        let h = ltrb[3] - ltrb[1];
        let w = ltrb[2] - ltrb[0];

        let mut p = base;
        for _y in 0..=h {
            for x in 0..=w {
                let v = &mut self.nodes[p + x as usize];
                if order > *v {
                    *v = order;
                }
            }
            p += self.grid_width as usize;
        }
    }

    fn reset(&mut self) {
        // memset(fNodes, 0, ...): the all-zero bit pattern is CompressedPaintersOrder::First().
        for node in &mut self.nodes {
            *node = CompressedPaintersOrder::first();
        }
    }
}

// A BoundsManager that first relies on BruteForceBoundsManager for N draw calls, and then switches
// to the GridBoundsManager if it exceeds its limit. For low N, the brute force approach is
// surprisingly efficient, has the highest accuracy, and very low memory overhead. Once the draw
// call count is large enough, the grid's lower performance complexity outweigh its memory cost and
// reduced accuracy.
// Port of: src/gpu/graphite/geom/BoundsManager.h#L242-L326 (chrome/m156)
/// Relies on `BruteForceBoundsManager` for `max_brute_force_n` draws, then switches to a
/// `GridBoundsManager`.
#[doc(alias = "skgpu::graphite::HybridBoundsManager")]
#[derive(Debug)]
pub struct HybridBoundsManager {
    device_size: ISize,
    grid_cell_size: i32,
    max_brute_force_n: i32,
    max_grid_size: i32,

    brute_force_manager: BruteForceBoundsManager,

    // The grid manager starts out null and is created the first time we exceed max_brute_force_n.
    // However, even if we reset back to the brute force manager, we keep the grid around under the
    // assumption that the owning Device will have similar frame-to-frame draw counts and will need
    // to upgrade to the grid manager again.
    grid_manager: Option<Box<GridBoundsManager>>,

    // Which member the C++ `fCurrentManager` pointer refers to.
    current: CurrentManager,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentManager {
    BruteForce,
    Grid,
}

impl HybridBoundsManager {
    /// Creates a hybrid manager; `max_grid_size` of 0 means unlimited.
    #[must_use]
    pub fn new(
        device_size: ISize,
        grid_cell_size: i32,
        max_brute_force_n: i32,
        max_grid_size: i32,
    ) -> Self {
        debug_assert!(
            device_size.width >= 1
                && device_size.height >= 1
                && grid_cell_size >= 1
                && max_brute_force_n >= 1
        );
        Self {
            device_size,
            grid_cell_size,
            max_brute_force_n,
            max_grid_size,
            brute_force_manager: BruteForceBoundsManager::new(),
            grid_manager: None,
            current: CurrentManager::BruteForce,
        }
    }

    // Port of: src/gpu/graphite/geom/BoundsManager.h#L309-L325 (chrome/m156)
    fn update_current_manager_if_needed(&mut self) {
        if self.current == CurrentManager::Grid
            || (self.brute_force_manager.count() as i64) < i64::from(self.max_brute_force_n)
        {
            // Already using the grid or the about-to-be-recorded draw will not cause us to exceed
            // the brute force limit, so no need to change the current manager implementation.
            return;
        }
        // Else we need to switch from the brute force manager to the grid manager
        let (device_size, cell, max_grid) = (self.device_size, self.grid_cell_size, self.max_grid_size);
        let grid = self
            .grid_manager
            .get_or_insert_with(|| GridBoundsManager::make_res(device_size, cell, max_grid));
        self.current = CurrentManager::Grid;

        // Fill out the grid manager with the recorded draws in the brute force manager
        self.brute_force_manager.replay_draws(grid.as_mut());
        self.brute_force_manager.reset();
    }
}

impl BoundsManager for HybridBoundsManager {
    fn get_most_recent_draw(&self, bounds: Rect) -> CompressedPaintersOrder {
        match self.current {
            CurrentManager::BruteForce => self.brute_force_manager.get_most_recent_draw(bounds),
            CurrentManager::Grid => self
                .grid_manager
                .as_ref()
                .expect("the grid exists while it is the current manager")
                .get_most_recent_draw(bounds),
        }
    }

    fn record_draw(&mut self, bounds: Rect, order: CompressedPaintersOrder) {
        self.update_current_manager_if_needed();
        match self.current {
            CurrentManager::BruteForce => self.brute_force_manager.record_draw(bounds, order),
            CurrentManager::Grid => self
                .grid_manager
                .as_mut()
                .expect("the grid exists while it is the current manager")
                .record_draw(bounds, order),
        }
    }

    fn reset(&mut self) {
        let used_grid = self.current == CurrentManager::Grid;
        if used_grid {
            // Reset the grid manager so it's ready to use next frame, but don't delete it.
            if let Some(grid) = self.grid_manager.as_mut() {
                grid.reset();
            }
            // Assume brute force manager was reset when we swapped to the grid originally
            self.current = CurrentManager::BruteForce;
        } else {
            // Clean up the grid manager that was created over a frame ago without being used.
            // This could lead to re-allocating the grid every-other frame, but it's a simple way
            // to ensure we don't hold onto the grid in perpetuity if it's not needed.
            self.grid_manager = None;
            self.brute_force_manager.reset();
            debug_assert!(self.current == CurrentManager::BruteForce);
        }
    }
}

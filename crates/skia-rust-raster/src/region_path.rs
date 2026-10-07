// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRegion_path.cpp

//! `SkRegion::setPath`: converts a path to a [`Region`] with the non-antialiased scan converter.
//!
//! skia-rust: `Region` lives in `skia-rust-core`, which the scan converter depends on, so
//! `setPath` cannot be an inherent method of `Region`. It is the method of the extension trait
//! [`RegionExt`] (import the trait to call `region.set_path(&path, &clip)`). The rest of
//! `SkRegion_path.cpp` (`addBoundaryPath`, `getBoundaryPath`) does not need the scan converter
//! and is in `skia_rust_core::region_path`.

use skia_rust_core::path::{Iter, Path, Verb};
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, RoundOut};
use skia_rust_core::region::{Op, Region, region_priv};
use skia_rust_core::safe_math::SafeMath;
use skia_rust_core::scalar::scalar_round_to_int;

use crate::blitter::{BlitMemory, Blitter};
use crate::scan;

/// `SkRegion::Scanline` storage of the builder: `[LastY, XCount, [L R]..., one slot for the
/// x-sentinel]`. Offsets below are indices into [`RgnBuilder::storage`].
// Port of: src/core/SkRegion_path.cpp#L41-L162 (chrome/m156)
#[doc(alias = "SkRgnBuilder")]
struct RgnBuilder {
    // skia-rust: grown on demand instead of one up-front allocation of `fStorageCount` values
    // (Skia uses `sk_malloc_canfail`, which can fail; Rust's allocation failure aborts, and the
    // storage is never touched beyond what the scan converter writes).
    storage: Vec<i32>,
    storage_count: usize,
    // offset of the scanline being built (`fCurrScanline`), or None before the first blit
    curr_scanline: Option<usize>,
    // `fPrevScanline`, None for the first scanline
    prev_scanline: Option<usize>,
    // points at next available x[] in the current scanline (`fCurrXPtr`)
    curr_x: usize,
    // first Y value (`fTop`)
    top: i32,
    blit_memory: BlitMemory,
}

impl RgnBuilder {
    fn new() -> Self {
        RgnBuilder {
            storage: Vec::new(),
            storage_count: 0,
            curr_scanline: None,
            prev_scanline: None,
            curr_x: 0,
            top: 0,
            blit_memory: BlitMemory::default(),
        }
    }

    // returns true if it could allocate the working storage needed
    // Port of: src/core/SkRegion_path.cpp#L132-L166 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // mirrors the implicit int -> size_t conversions
    fn init(&mut self, max_height: i32, max_transitions: i32, path_is_inverse: bool) -> bool {
        let mut max_transitions = max_transitions;
        if (max_height | max_transitions) < 0 {
            return false;
        }

        let mut safe = SafeMath::new();

        if path_is_inverse {
            // allow for additional X transitions to "invert" each scanline
            // [ L' ... normal transitions ... R' ]
            //
            max_transitions = safe.add_int(max_transitions, 2);
        }

        // compute the count with +1 and +3 slop for the working buffer
        let height_plus_1 = safe.add_int(max_height, 1);
        let transitions_plus_3 = safe.add_int(3, max_transitions);
        let mut count = safe.mul(height_plus_1 as usize, transitions_plus_3 as usize);

        if path_is_inverse {
            // allow for two "empty" rows for the top and bottom
            //      [ Y, 1, L, R, S] == 5 (*2 for top and bottom)
            count = safe.add(count, 10);
        }

        if !safe.ok() || i32::try_from(count).is_err() {
            return false;
        }
        self.storage_count = count;

        self.curr_scanline = None; // signal empty collection
        self.prev_scanline = None; // signal first scanline
        true
    }

    // Makes sure `storage[..=index]` exists.
    fn reserve(&mut self, index: usize) {
        debug_assert!(index < self.storage_count + 8);
        if index >= self.storage.len() {
            let new_len = (index + 1).max(self.storage.len() * 2).max(64);
            self.storage.resize(new_len, 0);
        }
    }

    // `Scanline::nextScanline`: add final +1 for the x-sentinel
    #[allow(clippy::cast_sign_loss)] // fXCount is non-negative
    fn next_scanline(&self, line: usize) -> usize {
        line + 2 + self.storage[line + 1] as usize + 1
    }

    // Port of: src/core/SkRegion_path.cpp#L58-L63 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // fits RunType
    fn done(&mut self) {
        if let Some(curr) = self.curr_scanline {
            self.storage[curr + 1] = (self.curr_x - (curr + 2)) as i32;
            if !self.collaps_with_prev() {
                // flush the last line
                self.curr_scanline = Some(self.next_scanline(curr));
            }
        }
    }

    // Port of: src/core/SkRegion_path.cpp#L120-L130 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // fXCount is non-negative
    fn collaps_with_prev(&mut self) -> bool {
        let (Some(prev), Some(curr)) = (self.prev_scanline, self.curr_scanline) else {
            return false;
        };
        let count = self.storage[curr + 1] as usize;
        if self.storage[prev] + 1 == self.storage[curr]
            && self.storage[prev + 1] == self.storage[curr + 1]
            && self.storage[prev + 2..prev + 2 + count] == self.storage[curr + 2..curr + 2 + count]
        {
            // update the height of fPrevScanline
            self.storage[prev] = self.storage[curr];
            return true;
        }
        false
    }

    // Port of: src/core/SkRegion_path.cpp#L168-L205 (chrome/m156)
    fn compute_run_count(&self) -> usize {
        match self.curr_scanline {
            None => 0,
            Some(curr) => 2 + curr,
        }
    }

    // Port of: src/core/SkRegion_path.cpp#L207-L216 (chrome/m156)
    fn copy_to_rect(&self) -> IRect {
        let curr = self.curr_scanline.expect("not empty");
        // A rect's scanline is [bottom intervals left right sentinel] == 5
        debug_assert_eq!(curr, 5);

        debug_assert_eq!(self.storage[1], 2);

        IRect::new(
            self.storage[2],
            self.top,
            self.storage[3],
            self.storage[0] + 1,
        )
    }

    // Port of: src/core/SkRegion_path.cpp#L218-L238 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // fXCount is non-negative
    fn copy_to_rgn(&self, count: usize) -> Vec<i32> {
        let stop = self.curr_scanline.expect("not empty");
        debug_assert!(stop > 4);

        let mut runs = Vec::with_capacity(count);
        let mut line = 0;
        runs.push(self.top);
        loop {
            runs.push(self.storage[line] + 1);
            let x_count = self.storage[line + 1] as usize;
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // fits RunType
            runs.push((x_count >> 1) as i32); // intervalCount
            if x_count != 0 {
                runs.extend_from_slice(&self.storage[line + 2..line + 2 + x_count]);
            }
            runs.push(region_priv::RUN_TYPE_SENTINEL);
            line = self.next_scanline(line);
            if line >= stop {
                break;
            }
        }
        debug_assert_eq!(line, stop);
        runs.push(region_priv::RUN_TYPE_SENTINEL);
        runs
    }
}

impl Blitter for RgnBuilder {
    // Port of: src/core/SkRegion_path.cpp#L168-L205 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        match self.curr_scanline {
            None => {
                // first time
                self.top = y;
                self.curr_scanline = Some(0);
                self.reserve(1);
                self.storage[0] = y;
                self.curr_x = 2;
            }
            Some(mut curr) => {
                debug_assert!(y >= self.storage[curr]);

                if y > self.storage[curr] {
                    // if we get here, we're done with fCurrScanline
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    // fits RunType
                    {
                        self.storage[curr + 1] = (self.curr_x - (curr + 2)) as i32;
                    }

                    let prev_last_y = self.storage[curr];
                    if !self.collaps_with_prev() {
                        self.prev_scanline = Some(curr);
                        curr = self.next_scanline(curr);
                        self.curr_scanline = Some(curr);
                    }
                    if y - 1 > prev_last_y {
                        // insert empty run
                        self.reserve(curr + 2);
                        self.storage[curr] = y - 1;
                        self.storage[curr + 1] = 0;
                        curr = self.next_scanline(curr);
                        self.curr_scanline = Some(curr);
                    }
                    // setup for the new curr line
                    self.reserve(curr + 1);
                    self.storage[curr] = y;
                    self.curr_x = curr + 2;
                }
            }
        }
        let curr = self.curr_scanline.expect("set above");
        //  check if we should extend the current run, or add a new one
        self.reserve(self.curr_x + 1);
        if self.curr_x > curr + 2 && self.storage[self.curr_x - 1] == x {
            self.storage[self.curr_x - 1] = x.wrapping_add(width);
        } else {
            self.storage[self.curr_x] = x;
            self.storage[self.curr_x + 1] = x.wrapping_add(width);
            self.curr_x += 2;
        }
        debug_assert!(self.curr_x < self.storage_count);
    }

    fn blit_anti_h(&mut self, _x: i32, _y: i32, _antialias: &mut [u8], _runs: &mut [i16]) {
        debug_assert!(false, "blitAntiH not implemented");
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.blit_memory
    }
}

// Port of: src/core/SkRegion_path.cpp#L240-L252 (chrome/m156)
fn verb_to_initial_last_index(verb: Verb) -> usize {
    match verb {
        Verb::Move | Verb::Close | Verb::Done => 0,
        Verb::Line => 1,
        Verb::Quad | Verb::Conic => 2,
        Verb::Cubic => 3,
    }
}

// Port of: src/core/SkRegion_path.cpp#L254-L266 (chrome/m156)
fn verb_to_max_edges(verb: Verb) -> i32 {
    match verb {
        Verb::Move | Verb::Close | Verb::Done => 0,
        Verb::Line => 1,
        Verb::Quad | Verb::Conic => 2,
        Verb::Cubic => 3,
    }
}

// If returns 0, ignore itop and ibot
// Port of: src/core/SkRegion_path.cpp#L268-L303 (chrome/m156)
fn count_path_runtype_values(path: &Path) -> (i32, i32, i32) {
    let mut max_edges = 0;
    let mut top = f32::from(i16::MAX);
    let mut bot = f32::from(i16::MIN);

    for (verb, pts) in Iter::new(path, true) {
        max_edges += verb_to_max_edges(verb);

        let last_index = verb_to_initial_last_index(verb);
        if last_index > 0 {
            for pt in pts.iter().take(last_index + 1).skip(1) {
                if top > pt.y {
                    top = pt.y;
                } else if bot < pt.y {
                    bot = pt.y;
                }
            }
        } else if verb == Verb::Move {
            if top > pts[0].y {
                top = pts[0].y;
            } else if bot < pts[0].y {
                bot = pts[0].y;
            }
        }
    }
    if 0 == max_edges {
        return (0, 0, 0); // we have only moves+closes
    }

    debug_assert!(top <= bot);
    (
        max_edges,
        scalar_round_to_int(top),
        scalar_round_to_int(bot),
    )
}

// Port of: src/core/SkRegion_path.cpp#L305-L311 (chrome/m156)
fn check_inverse_on_empty_return(dst: &mut Region, path: &Path, clip: &Region) -> bool {
    if path.is_inverse_fill_type() {
        dst.set(clip)
    } else {
        dst.set_empty()
    }
}

/// `SkRegion::setPath`, as a method of [`Region`] (see the module documentation).
pub trait RegionExt {
    /// Sets the region to the area of `path` that is inside `clip`. Returns true if the
    /// resulting region is not empty (or, on failure, false and an empty region).
    ///
    /// An inverse-filled path results in `clip` minus the path.
    #[doc(alias = "setPath")]
    fn set_path(&mut self, path: &Path, clip: &Region) -> bool;
}

impl RegionExt for Region {
    // Port of: src/core/SkRegion_path.cpp#L313-L456 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    // mirrors the int64 -> int and int64 -> float conversions of the tiling code
    fn set_path(&mut self, path: &Path, clip: &Region) -> bool {
        region_priv::validate(self);

        let raw = path_priv::raw(path, ResolveConvexity::Yes);

        let Some(raw) = raw.filter(|_| !clip.is_empty() && !path.is_empty()) else {
            // This treats non-finite paths (no raw) as empty as well, so this returns empty or
            // 'clip' if it's inverse-filled. If clip is also empty, path's fill type doesn't
            // really matter and this region ends up empty.
            return check_inverse_on_empty_return(self, path, clip);
        };

        // Our builder is very fragile, and can't be called with spans/rects out of Y->X order.
        // To ensure this, we only "fill" clipped to a rect (the clip's bounds), and if the
        // clip is more complex than that, we just post-intersect the result with the clip.
        let clip_bounds = *clip.bounds();
        if clip.is_complex() {
            if !self.set_path(path, &Region::from_rect(clip_bounds)) {
                return false;
            }
            return self.op_region(clip, Op::Intersect);
        }

        // SkScan::FillPath has limits on the coordinate range of the clipping SkRegion. If it's
        // too big, tile the clip bounds and union the pieces back together.
        if scan::path_requires_tiling(&clip_bounds) {
            const TILE_SIZE: i32 = 32767 >> 1; // Limit so coords can fit into SkFixed (16.16)
            const TILE_LIMIT: i32 = 1000; // Max size is about 500k x 500k
            let path_bounds: IRect = path.bounds().round_out();

            self.set_empty();

            let Some(intersection) = IRect::intersect(&path_bounds, &clip_bounds) else {
                return false;
            };

            let mut safe = SafeMath::new();
            let width = safe.sub_int(intersection.right, intersection.left);
            let height = safe.sub_int(intersection.bottom, intersection.top);

            let width_rounded = safe.add_int(width, TILE_SIZE - 1);
            let tiles_x = safe.div_int(width_rounded, TILE_SIZE);
            let height_rounded = safe.add_int(height, TILE_SIZE - 1);
            let tiles_y = safe.div_int(height_rounded, TILE_SIZE);

            let total_tiles = safe.mul_int(tiles_x, tiles_y);

            // Limit the total number of tiles to prevent large coordinate spans from timing out.
            if !safe.ok() || total_tiles > TILE_LIMIT {
                return false;
            }

            // Note: With large integers some intermediate calculations can overflow, but the
            // end results will still be in integer range. Using i64 for the intermediate
            // values will handle this situation.
            let mut top = i64::from(intersection.top);
            while top < i64::from(intersection.bottom) {
                let bot = (top + i64::from(TILE_SIZE)).min(i64::from(intersection.bottom));
                let mut left = i64::from(intersection.left);
                while left < i64::from(intersection.right) {
                    let right = (left + i64::from(TILE_SIZE)).min(i64::from(intersection.right));

                    let mut tile_clip_bounds =
                        IRect::new(left as i32, top as i32, right as i32, bot as i32);
                    debug_assert!(IRect::intersects(&path_bounds, &tile_clip_bounds));

                    // Shift coordinates so the top left is (0,0) during scan conversion and then
                    // translate the SkRegion afterwards.
                    tile_clip_bounds.offset(((-left) as i32, (-top) as i32));
                    debug_assert!(!scan::path_requires_tiling(&tile_clip_bounds));
                    let mut tile = Region::new();
                    if let Some(newpath) =
                        path.try_make_offset(Point::new(-(left as f32), -(top as f32)))
                    {
                        tile.set_path(&newpath, &Region::from_rect(tile_clip_bounds));
                        tile.translate((left as i32, top as i32));
                        self.op_region(&tile, Op::Union);
                    } else {
                        return false;
                    }
                    left += i64::from(TILE_SIZE);
                }
                top += i64::from(TILE_SIZE);
            }
            // During tiling we only applied the bounds of the tile, now that we have a full
            // SkRegion, apply the original clip.
            return self.op_region(clip, Op::Intersect);
        }

        //  compute worst-case rgn-size for the path
        let (path_transitions, path_top, path_bot) = count_path_runtype_values(path);
        if 0 == path_transitions {
            return check_inverse_on_empty_return(self, path, clip);
        }

        let (clip_transitions, clip_top, clip_bot) = region_priv::count_runtype_values(clip);

        let top = path_top.max(clip_top);
        let bot = path_bot.min(clip_bot);
        if top >= bot {
            return check_inverse_on_empty_return(self, path, clip);
        }

        let mut builder = RgnBuilder::new();

        if !builder.init(
            bot - top,
            path_transitions.max(clip_transitions),
            path.is_inverse_fill_type(),
        ) {
            // can't allocate working space, so return false
            return self.set_empty();
        }

        scan::fill_path(&raw, clip, &mut builder);
        builder.done();

        let count = builder.compute_run_count();
        if count == 0 {
            return self.set_empty();
        } else if count == region_priv::RECT_REGION_RUNS {
            let bounds = builder.copy_to_rect();
            self.set_rect(bounds);
        } else {
            let mut tmp = region_priv::make_complex(builder.copy_to_rgn(count));
            self.swap(&mut tmp);
        }
        region_priv::validate(self);
        true
    }
}

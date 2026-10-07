// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkAutoBlitterChoose.h

//! [`auto_blitter_choose`]: runs a closure with the blitter the draw's chooser picks for a paint
//! (`SkAutoBlitterChoose`).
//!
//! skia-rust: `SkAutoBlitterChoose` owns a `SkBlitterSizedArena` (a 2,736-byte stack arena that
//! grows on the heap) and the blitter allocated in it, which a Rust value cannot do because the
//! blitter borrows the arena. The arena lives for the duration of the call instead and the
//! blitter is handed to the closure; both are dropped when it returns, as the destructor of the
//! C++ object does at the end of the draw call.

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::surface_props::SurfaceProps;

use crate::blitter::Blitter;
use crate::draw::Draw;

/// Chooses the blitter for `paint` with `draw`'s blitter chooser (`draw.fBlitterChooser`) and
/// calls `f` with it. `ctm` is the matrix to choose for; `None` is `draw.fCTM`. `dev_bounds`
/// are the device-space bounds of the geometry (`SkAutoBlitterChoose::choose`).
// Port of: src/core/SkAutoBlitterChoose.h#L35-L61 (chrome/m156)
#[doc(alias = "SkAutoBlitterChoose")]
pub fn auto_blitter_choose<R>(
    draw: &mut Draw<'_>,
    ctm: Option<&Matrix>,
    paint: &Paint,
    dev_bounds: &Rect,
    draw_coverage: DrawCoverage,
    f: impl FnOnce(&mut dyn Blitter) -> R,
) -> R {
    let alloc = ArenaAlloc::new();
    let ctm = ctm.unwrap_or(draw.ctm);
    let rc = draw.rc;
    let default_props = SurfaceProps::default();
    let props = draw.props.unwrap_or(&default_props);
    let chooser = draw.blitter_chooser;
    let mut blitter = chooser(
        draw.dst.reborrow_mut(),
        ctm,
        paint,
        &alloc,
        draw_coverage,
        rc.clip_shader(),
        props,
        dev_bounds,
    );
    f(&mut *blitter)
}

// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkTrimPathEffect.h, src/effects/SkTrimPathEffect.cpp,
// src/effects/SkTrimPE.h

//! `SkTrimPathEffect`: returns the subset of a path between a start and a stop `t` value in
//! `0..=1` (or its complement, in [`Mode::Inverted`]).
//!
//! skia-rust: flattening (`CreateProc`, `flatten`) is not ported.

use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PathEffectBase};
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

/// Whether to return the trimmed subset or the complement (`SkTrimPathEffect::Mode`).
#[doc(alias = "SkTrimPathEffect::Mode")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Returns the subset between the start and stop `t` values (`kNormal`).
    Normal = 0,
    /// Returns the complement: `stop..1` followed by `0..start` (`kInverted`).
    Inverted = 1,
}

// Returns the number of contours iterated to satisfy the request.
// Port of: src/effects/SkTrimPathEffect.cpp#L26-L53 (chrome/m156), add_segments
fn add_segments(
    src: &Path,
    start: scalar,
    stop: scalar,
    dst: &mut PathBuilder,
    requires_moveto: bool,
) -> usize {
    debug_assert!(start < stop);

    let mut measure = PathMeasure::new(src, false, None);

    let mut current_segment_offset: scalar = 0.0;
    let mut contour_count: usize = 1;

    loop {
        let next_offset = current_segment_offset + measure.length();

        if start < next_offset {
            measure.get_segment(
                start - current_segment_offset,
                stop - current_segment_offset,
                dst,
                requires_moveto,
            );

            if stop <= next_offset {
                break;
            }
        }

        contour_count += 1;
        current_segment_offset = next_offset;
        if !measure.next_contour() {
            break;
        }
    }

    contour_count
}

// Port of: src/effects/SkTrimPE.h#L17-L40 and src/effects/SkTrimPathEffect.cpp#L55-L113 (chrome/m156)
#[derive(Clone, Debug)]
struct TrimPE {
    start_t: scalar,
    stop_t: scalar,
    mode: Mode,
}

/// `SkTrimPE::CreateProc`: the start, the stop, and the mode in bit 0.
// Port of: src/effects/SkTrimPathEffect.cpp#L123-L130 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<PathEffect> {
    let start = buffer.read_scalar();
    let stop = buffer.read_scalar();
    let mode = if buffer.read_uint() & 1 == 1 {
        Mode::Inverted
    } else {
        Mode::Normal
    };
    new(start, stop, mode)
}

impl PathEffectBase for TrimPE {
    // Port of: src/effects/SkTrimPE.h#L24 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkTrimPE"
    }

    // Port of: src/effects/SkTrimPathEffect.cpp#L117-L121 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_scalar(self.start_t);
        buffer.write_scalar(self.stop_t);
        buffer.write_uint(self.mode as u32);
    }

    // Port of: src/effects/SkTrimPathEffect.cpp#L61-L109 (chrome/m156), SkTrimPE::onFilterPath
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        _rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        _ctm: &Matrix,
    ) -> bool {
        if self.start_t >= self.stop_t {
            debug_assert_eq!(self.mode, Mode::Normal);
            return true;
        }

        // First pass: compute the total len.
        let mut len: scalar = 0.0;
        let mut meas = PathMeasure::new(src, false, None);
        loop {
            len += meas.length();
            if !meas.next_contour() {
                break;
            }
        }

        let arc_start = len * self.start_t;
        let arc_stop = len * self.stop_t;

        // Second pass: actually add segments.
        if self.mode == Mode::Normal {
            // Normal mode -> one span.
            if arc_start < arc_stop {
                add_segments(src, arc_start, arc_stop, dst, true);
            }
        } else {
            // Inverted mode -> one logical span which wraps around at the end -> two actual
            // spans. In order to preserve closed path continuity:
            //
            //   1) add the second/tail span first
            //
            //   2) skip the head span move-to for single-closed-contour paths
            let mut requires_moveto = true;
            if arc_stop < len {
                // since we're adding the "tail" first, this is the total number of contours
                let contour_count = add_segments(src, arc_stop, len, dst, true);

                // if the path consists of a single closed contour, we don't want to disconnect
                // the two parts with a moveto.
                if contour_count == 1 && src.is_last_contour_closed() {
                    requires_moveto = false;
                }
            }
            if 0.0 < arc_start {
                add_segments(src, 0.0, arc_start, dst, requires_moveto);
            }
        }

        true
    }

    // Port of: src/effects/SkTrimPE.h#L27-L31 (chrome/m156)
    // Trimming a path returns a subset of the input path, so `bounds` is left unmodified.
    fn compute_fast_bounds(&self, _bounds: Option<&mut Rect>) -> bool {
        true
    }
}

/// Takes start and stop `t` values (`0..=1`) and returns the path that is that subset of the
/// original path (`SkTrimPathEffect::Make`).
///
/// e.g. `new(0.5, 1.0, Mode::Normal)` returns the second half of the path, and
/// `new(0.33333, 0.66667, Mode::Normal)` returns the middle third. The trim values apply to the
/// whole path, so all of its contours count in the calculation.
///
/// `Mode::Normal` returns one (logical) segment, even when it spans several contours.
/// `Mode::Inverted` returns two logical segments, `stop_t..1` and then `0..start_t`.
///
/// `start_t` and `stop_t` are pinned to `0..=1`. Returns `None` if either is not finite, if
/// `Mode::Normal` would trim nothing (`start_t <= 0` and `stop_t >= 1`), or if `Mode::Inverted`
/// would keep nothing (`start_t >= stop_t`).
// Port of: src/effects/SkTrimPathEffect.cpp#L115-L136 (chrome/m156)
#[doc(alias = "SkTrimPathEffect::Make")]
#[must_use]
pub fn new(start_t: scalar, stop_t: scalar, mode: impl Into<Option<Mode>>) -> Option<PathEffect> {
    let mode = mode.into().unwrap_or(Mode::Normal);
    if !is_finite(start_t) || !is_finite(stop_t) {
        return None;
    }

    if start_t <= 0.0 && stop_t >= 1.0 && mode == Mode::Normal {
        return None;
    }

    let start_t = t_pin(start_t, 0.0, 1.0);
    let stop_t = t_pin(stop_t, 0.0, 1.0);

    if start_t >= stop_t && mode == Mode::Inverted {
        return None;
    }

    Some(PathEffect::from_base(TrimPE {
        start_t,
        stop_t,
        mode,
    }))
}

/// Provides `PathEffect::trim`, as `skia-safe` has it as an inherent method (Rust does not allow
/// inherent impls outside the defining crate).
pub trait TrimPathEffectExt {
    /// Returns the subset of the path between `start_t` and `stop_t`; see [`new`].
    #[doc(alias = "SkTrimPathEffect::Make")]
    fn trim(start_t: scalar, stop_t: scalar, mode: impl Into<Option<Mode>>) -> Option<PathEffect>;
}

impl TrimPathEffectExt for PathEffect {
    fn trim(start_t: scalar, stop_t: scalar, mode: impl Into<Option<Mode>>) -> Option<PathEffect> {
        new(start_t, stop_t, mode)
    }
}

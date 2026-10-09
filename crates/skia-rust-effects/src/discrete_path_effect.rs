// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkDiscretePathEffect.h, src/effects/SkDiscretePathEffect.cpp

//! `SkDiscretePathEffect`: chops a path into discrete segments and randomly displaces their
//! endpoints.
//!
//! skia-rust: flattening (`CreateProc`, `flatten`) is not ported.

use skia_rust_core::fixed::{Fixed, fixed_to_scalar};
use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PathEffectBase};
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::point::{Point, Vector, point_priv};
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, scalar, scalar_abs, scalar_round_to_int};
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

/// Utility that implements pseudo random 32-bit numbers with a fast linear equation. Unlike
/// `rand()`, it holds its own seed, so that several instances can be used without side effects
/// (`LCGRandom`).
// Port of: src/effects/SkDiscretePathEffect.cpp#L90 (chrome/m156), `kMaxReasonableIterations`
const K_MAX_REASONABLE_ITERATIONS: i32 = 100_000;

// Port of: src/effects/SkDiscretePathEffect.cpp#L24-L65 (chrome/m156)
#[derive(Debug)]
struct LcgRandom {
    seed: u32,
}

impl LcgRandom {
    // See "Numerical Recipes in C", 1992 page 284 for these constants
    // Port of: src/effects/SkDiscretePathEffect.cpp#L56-L59 (chrome/m156)
    const MUL: u32 = 1_664_525;
    const ADD: u32 = 1_013_904_223;

    // Port of: src/effects/SkDiscretePathEffect.cpp#L38-L39 (chrome/m156)
    fn new(seed: u32) -> Self {
        Self { seed }
    }

    // Return the next pseudo random number as an unsigned 32-bit value.
    // Port of: src/effects/SkDiscretePathEffect.cpp#L46-L50 (chrome/m156)
    fn next_u(&mut self) -> u32 {
        let r = self.seed.wrapping_mul(Self::MUL).wrapping_add(Self::ADD);
        self.seed = r;
        r
    }

    // Return the next pseudo random number as a signed 32-bit value.
    // Port of: src/effects/SkDiscretePathEffect.cpp#L52-L54 (chrome/m156)
    #[allow(clippy::cast_possible_wrap)] // mirrors the C++ `(int32_t)` cast
    fn next_s(&mut self) -> i32 {
        self.next_u() as i32
    }

    // Return the next pseudo random number as a signed `Fixed` in `[-1, 1)`.
    // Port of: src/effects/SkDiscretePathEffect.cpp#L59-L62 (chrome/m156)
    fn next_s_fixed1(&mut self) -> Fixed {
        self.next_s() >> 15
    }

    // Return the next pseudo random number expressed as a `scalar` in `[-1, 1)`.
    // Port of: src/effects/SkDiscretePathEffect.cpp#L33-L35 (chrome/m156)
    fn next_s_scalar1(&mut self) -> scalar {
        fixed_to_scalar(self.next_s_fixed1())
    }
}

// Port of: src/effects/SkDiscretePathEffect.cpp#L67-L72 (chrome/m156)
fn perterb(p: &mut Point, tangent: Vector, scale: scalar) {
    let mut normal = tangent;
    point_priv::rotate_ccw_in_place(&mut normal);
    normal.set_length(scale);
    *p += normal;
}

// Port of: src/effects/SkDiscretePathEffect.cpp#L74-L131 (chrome/m156)
#[derive(Clone, Debug)]
struct DiscretePathEffectImpl {
    seg_length: scalar,
    perterb: scalar,
    // Caller-supplied 32-bit seed assist.
    seed_assist: u32,
}

/// `SkDiscretePathEffect::CreateProc`: the segment length, the perturbation and the seed.
// Port of: src/effects/SkDiscretePathEffect.cpp#L153-L158 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<PathEffect> {
    let seg_length = buffer.read_scalar();
    let perterb = buffer.read_scalar();
    let seed = buffer.read_uint();
    new(seg_length, perterb, seed)
}

impl PathEffectBase for DiscretePathEffectImpl {
    // Port of: src/effects/SkDiscretePathEffect.cpp#L167 (chrome/m156)
    fn type_name(&self) -> &'static str {
        "SkDiscretePathEffect"
    }

    // Port of: src/effects/SkDiscretePathEffect.cpp#L160-L164 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_scalar(self.seg_length);
        buffer.write_scalar(self.perterb);
        buffer.write_uint(self.seed_assist);
    }

    // Port of: src/effects/SkDiscretePathEffect.cpp#L85-L127 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the C++ `int` -> `SkScalar` conversion
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        _ctm: &Matrix,
    ) -> bool {
        let do_fill = rec.is_fill_style();

        let mut meas = PathMeasure::new(src, do_fill, None);

        // Caller may supply their own seed assist, which by default is 0
        // `int` -> `uint32_t` in C++ keeps the bits.
        #[allow(clippy::cast_sign_loss)] // mirrors the C++ int -> uint32_t conversion
        let round_length = scalar_round_to_int(meas.length()) as u32;
        let seed = self.seed_assist ^ round_length;

        // `(seed << 16) | (seed >> 16)` is a rotate by 16.
        let mut rand = LcgRandom::new(seed ^ seed.rotate_left(16));
        let scale = self.perterb;

        loop {
            let length = meas.length();

            if self.seg_length * (2.0 + f32::from(u8::from(do_fill))) > length {
                meas.get_segment(0.0, length, dst, true); // to short for us to mangle
            } else {
                let mut n = scalar_round_to_int(length / self.seg_length);
                n = n.min(K_MAX_REASONABLE_ITERATIONS);
                let delta = length / (n as scalar);
                let mut distance: scalar = 0.0;

                if meas.is_closed() {
                    n -= 1;
                    distance += delta / 2.0;
                }

                let mut p = Point::default();
                let mut v = Vector::default();
                if meas.get_pos_tan(distance, Some(&mut p), Some(&mut v)) {
                    perterb(&mut p, v, rand.next_s_scalar1() * scale);
                    dst.move_to(p);
                }
                // C++: `while (--n >= 0)`
                n -= 1;
                while n >= 0 {
                    distance += delta;
                    if meas.get_pos_tan(distance, Some(&mut p), Some(&mut v)) {
                        perterb(&mut p, v, rand.next_s_scalar1() * scale);
                        dst.line_to(p);
                    }
                    n -= 1;
                }
                if meas.is_closed() {
                    dst.close();
                }
            }

            if !meas.next_contour() {
                break;
            }
        }
        true
    }

    // Port of: src/effects/SkDiscretePathEffect.cpp#L129-L135 (chrome/m156)
    fn compute_fast_bounds(&self, bounds: Option<&mut Rect>) -> bool {
        if let Some(bounds) = bounds {
            let max_outset = scalar_abs(self.perterb);
            bounds.outset((max_outset, max_outset));
        }
        true
    }
}

/// Chops a path into discrete segments, and randomly displaces them
/// (`SkDiscretePathEffect::Make`).
///
/// - `seg_length`: break the path into segments of this length
/// - `dev`: randomly move the endpoints away from the original path by a maximum of this
///   deviation
/// - `seed_assist`: a caller-supplied seed that modifies the seed value used to randomize the
///   path segments' endpoints. If `None`, it defaults to 0, in which case filtering a path
///   multiple times gives the same set of segments (useful for testing).
///
/// Returns `None` if `seg_length` or `dev` is not finite, or if `seg_length` is nearly zero.
/// Works on filled or framed paths.
// Port of: src/effects/SkDiscretePathEffect.cpp#L143-L153 (chrome/m156)
#[doc(alias = "SkDiscretePathEffect::Make")]
#[must_use]
pub fn new(
    seg_length: scalar,
    dev: scalar,
    seed_assist: impl Into<Option<u32>>,
) -> Option<PathEffect> {
    if !is_finite(seg_length) || !is_finite(dev) {
        return None;
    }
    if seg_length <= SCALAR_NEARLY_ZERO {
        return None;
    }
    Some(PathEffect::from_base(DiscretePathEffectImpl {
        seg_length,
        perterb: dev,
        seed_assist: seed_assist.into().unwrap_or(0),
    }))
}

/// Provides `PathEffect::discrete`, as `skia-safe` has it as an inherent method (Rust does not
/// allow inherent impls outside the defining crate).
pub trait DiscretePathEffectExt {
    /// Chops a path into discrete segments, and randomly displaces them; see [`new`].
    #[doc(alias = "SkDiscretePathEffect::Make")]
    fn discrete(
        seg_length: scalar,
        dev: scalar,
        seed_assist: impl Into<Option<u32>>,
    ) -> Option<PathEffect>;
}

impl DiscretePathEffectExt for PathEffect {
    fn discrete(
        seg_length: scalar,
        dev: scalar,
        seed_assist: impl Into<Option<u32>>,
    ) -> Option<PathEffect> {
        new(seg_length, dev, seed_assist)
    }
}

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/animator/KeyframeAnimator.h, KeyframeAnimator.cpp
// (chrome/m156)

use std::cell::Cell;
use std::rc::Rc;

use skia_rust_core::cubic_map::CubicMap;
use skia_rust_core::point::Point;

use crate::ExpressionManager;
use crate::json::{ArrayValue, ObjectValue, Value};
use crate::skottie_json::{ValueExt, parse_default, parse_value};

use super::Animator;
use crate::internal::skottie_priv::AnimationBuilder;

/// How the value of a keyframe is stored.
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L38-L40 (chrome/m156) (`Keyframe::Value::Type`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyframeValueType {
    /// An index into external storage.
    Index,
    /// A scalar, stored inline.
    Scalar,
}

/// The value of a keyframe: scalar values are stored inline, other types are stored externally
/// and tracked by index. Skia's `union { uint32_t idx; float flt; }`.
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L36-L55 (chrome/m156) (`Keyframe::Value`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyframeValue(u32);

impl KeyframeValue {
    /// A value that is an index.
    #[must_use]
    pub const fn from_idx(idx: u32) -> Self {
        Self(idx)
    }

    /// A value that is a scalar.
    #[must_use]
    pub fn from_flt(flt: f32) -> Self {
        Self(flt.to_bits())
    }

    /// The index.
    #[must_use]
    pub const fn idx(self) -> u32 {
        self.0
    }

    /// The scalar.
    #[must_use]
    pub fn flt(self) -> f32 {
        f32::from_bits(self.0)
    }

    /// Value equality for the given storage type: indices compare as integers, scalars as
    /// floats (`-0.0 == 0.0`, `NaN != NaN`).
    // Port of: modules/skottie/src/animator/KeyframeAnimator.h#L48-L52 (chrome/m156) (`Keyframe::Value::equals`)
    #[must_use]
    #[allow(clippy::float_cmp)] // exact float comparison, as in Skia
    pub fn equals(self, other: Self, ty: KeyframeValueType) -> bool {
        match ty {
            KeyframeValueType::Index => self.idx() == other.idx(),
            KeyframeValueType::Scalar => self.flt() == other.flt(),
        }
    }
}

/// One AE/Lottie keyframe.
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L33-L72 (chrome/m156) (`struct Keyframe`)
#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    /// The time.
    pub t: f32,
    /// The value.
    pub v: KeyframeValue,
    /// Encodes the value interpolation in `[KFRec_n .. KFRec_n+1)`:
    ///   0 -> constant
    ///   1 -> linear
    ///   n -> cubic: cubic_mappers[n-2]
    pub mapping: u32,
}

impl Keyframe {
    /// `kConstantMapping`.
    pub const CONSTANT_MAPPING: u32 = 0;
    /// `kLinearMapping`.
    pub const LINEAR_MAPPING: u32 = 1;
    /// `kCubicIndexOffset`.
    pub const CUBIC_INDEX_OFFSET: u32 = 2;
}

/// The interpolation inputs for one time: a weight and the two values it blends.
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L89-L93 (chrome/m156) (`KeyframeAnimator::LERPInfo`)
#[derive(Debug, Clone, Copy)]
pub struct LerpInfo {
    /// The `vrec0`/`vrec1` weight, in `[0 .. 1]` (supernormal for eased overshoot).
    pub weight: f32,
    /// The first value.
    pub vrec0: KeyframeValue,
    /// The second value.
    pub vrec1: KeyframeValue,
}

/// The keyframes of a property, and the cubic mappers of their easing (the data members of
/// `KeyframeAnimator`).
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L74-L120 (chrome/m156) (`class KeyframeAnimator`)
#[derive(Debug)]
pub struct KeyframeData {
    /// Keyframe records, one per AE/Lottie keyframe.
    kfs: Vec<Keyframe>,
    /// Optional cubic mappers (Bezier interpolation).
    cms: Vec<CubicMap>,
    /// Cached segment: the index of the first of its two keyframes.
    current_segment: Cell<Option<usize>>,
}

impl KeyframeData {
    /// Keyframe data for `kfs`, with the cubic mappers `cms`.
    #[must_use]
    pub fn new(kfs: Vec<Keyframe>, cms: Vec<CubicMap>) -> Self {
        Self {
            kfs,
            cms,
            current_segment: Cell::new(None),
        }
    }

    /// True if all the keyframes hold the same value. `parse_keyframes` keeps a single frame for
    /// constant properties.
    // Port of: modules/skottie/src/animator/KeyframeAnimator.h#L65-L70 (chrome/m156) (`isConstant`)
    #[must_use]
    pub fn is_constant(&self) -> bool {
        debug_assert!(!self.kfs.is_empty());
        self.kfs.len() == 1
    }

    /// The number of keyframes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.kfs.len()
    }

    /// True if there are no keyframes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kfs.is_empty()
    }

    /// Two sequential keyframes determine how the value varies within `[kf0 .. kf1)`.
    // Port of: modules/skottie/src/animator/KeyframeAnimator.h#L99-L108 (chrome/m156) (`KFSegment::contains`)
    fn segment_contains(&self, kf0: Option<usize>, t: f32) -> bool {
        match kf0 {
            Some(i) => self.kfs[i].t <= t && t < self.kfs[i + 1].t,
            None => false,
        }
    }

    /// Main entry point: `t` -> the interpolation info.
    // Port of: modules/skottie/src/animator/KeyframeAnimator.cpp#L22-L49 (chrome/m156) (`getLERPInfo`)
    #[must_use]
    pub fn get_lerp_info(&self, t: f32) -> LerpInfo {
        debug_assert!(!self.kfs.is_empty());
        let front = self.kfs[0];
        let back = self.kfs[self.kfs.len() - 1];

        if t <= front.t {
            // Constant/clamped segment.
            return LerpInfo {
                weight: 0.0,
                vrec0: front.v,
                vrec1: front.v,
            };
        }
        if t >= back.t {
            // Constant/clamped segment.
            return LerpInfo {
                weight: 0.0,
                vrec0: back.v,
                vrec1: back.v,
            };
        }

        // Cache the current segment (most queries have good locality).
        if !self.segment_contains(self.current_segment.get(), t) {
            self.current_segment.set(Some(self.find_segment(t)));
        }
        let kf0_index = self
            .current_segment
            .get()
            .expect("the current segment contains t");
        debug_assert!(self.segment_contains(Some(kf0_index), t));
        let kf0 = self.kfs[kf0_index];
        let kf1 = self.kfs[kf0_index + 1];

        if kf0.mapping == Keyframe::CONSTANT_MAPPING {
            // Constant/hold segment.
            return LerpInfo {
                weight: 0.0,
                vrec0: kf0.v,
                vrec1: kf0.v,
            };
        }

        LerpInfo {
            weight: self.compute_weight(kf0_index, t),
            vrec0: kf0.v,
            vrec1: kf1.v,
        }
    }

    /// Finds the segment containing `t`: the index of its first keyframe.
    // Port of: modules/skottie/src/animator/KeyframeAnimator.cpp#L51-L72 (chrome/m156) (`find_segment`)
    fn find_segment(&self, t: f32) -> usize {
        debug_assert!(self.kfs.len() > 1);
        debug_assert!(t > self.kfs[0].t);
        debug_assert!(t < self.kfs[self.kfs.len() - 1].t);

        let mut kf0 = 0_usize;
        let mut kf1 = self.kfs.len() - 1;

        // Binary-search, until we reduce to sequential keyframes.
        while kf0 + 1 != kf1 {
            debug_assert!(kf0 < kf1);
            debug_assert!(self.kfs[kf0].t <= t && t < self.kfs[kf1].t);

            let mid_kf = kf0 + (kf1 - kf0) / 2;

            if t >= self.kfs[mid_kf].t {
                kf0 = mid_kf;
            } else {
                kf1 = mid_kf;
            }
        }

        kf0
    }

    /// Given a `t` and a containing segment, computes the local interpolation weight.
    // Port of: modules/skottie/src/animator/KeyframeAnimator.cpp#L74-L86 (chrome/m156) (`compute_weight`)
    fn compute_weight(&self, kf0_index: usize, t: f32) -> f32 {
        debug_assert!(self.segment_contains(Some(kf0_index), t));
        let kf0 = self.kfs[kf0_index];
        let kf1 = self.kfs[kf0_index + 1];

        // Linear weight.
        let mut w = (t - kf0.t) / (kf1.t - kf0.t);

        // Optional cubic mapper.
        if kf0.mapping >= Keyframe::CUBIC_INDEX_OFFSET {
            let mapper_index = (kf0.mapping - Keyframe::CUBIC_INDEX_OFFSET) as usize;
            w = self.cms[mapper_index].compute_y_from_x(w);
        }

        w
    }
}

/// A keyframe animator: it is constant when all its keyframes hold the same value.
pub trait KeyframeAnimator: Animator {
    /// True if all the keyframes hold the same value (`isConstant`).
    #[doc(alias = "isConstant")]
    fn is_constant(&self) -> bool;
}

/// The state every animator builder shares (the data members of `AnimatorBuilder`).
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L122-L156 (chrome/m156) (`class AnimatorBuilder`)
#[derive(Debug)]
pub struct AnimatorBuilderBase {
    /// Keyframe records, one per AE/Lottie keyframe.
    pub kfs: Vec<Keyframe>,
    /// Optional cubic mappers (Bezier interpolation).
    pub cms: Vec<CubicMap>,
    keyframe_type: KeyframeValueType,
    // Track previous cubic map parameters (for deduping).
    prev_c0: Point,
    prev_c1: Point,
}

impl AnimatorBuilderBase {
    /// The state for a builder whose keyframes store values of the type `ty`.
    #[must_use]
    pub fn new(ty: KeyframeValueType) -> Self {
        Self {
            kfs: Vec::new(),
            cms: Vec::new(),
            keyframe_type: ty,
            prev_c0: Point { x: 0.0, y: 0.0 },
            prev_c1: Point { x: 0.0, y: 0.0 },
        }
    }

    // Port of: modules/skottie/src/animator/KeyframeAnimator.cpp#L166-L190 (chrome/m156) (`AnimatorBuilder::parseMapping`)
    fn parse_mapping(&mut self, jkf: &ObjectValue) -> u32 {
        if parse_default::<bool>(jkf.get("h"), false) {
            return Keyframe::CONSTANT_MAPPING;
        }

        let c0 = parse_value::<Point>(jkf.get("o"));
        let c1 = parse_value::<Point>(jkf.get("i"));
        let (Some(c0), Some(c1)) = (c0, c1) else {
            return Keyframe::LINEAR_MAPPING;
        };
        if CubicMap::is_linear(c0, c1) {
            return Keyframe::LINEAR_MAPPING;
        }

        // De-dupe sequential cubic mappers.
        if c0 != self.prev_c0 || c1 != self.prev_c1 || self.cms.is_empty() {
            self.cms.push(CubicMap::new(c0, c1));
            self.prev_c0 = c0;
            self.prev_c1 = c1;
        }

        debug_assert!(!self.cms.is_empty());
        let count = u32::try_from(self.cms.len()).expect("fewer than 2^32 cubic mappers");
        count - 1 + Keyframe::CUBIC_INDEX_OFFSET
    }
}

/// Builds the animator of one property type from its JSON.
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L122-L156 (chrome/m156) (`class AnimatorBuilder`)
pub trait AnimatorBuilder {
    /// The shared state.
    fn base(&mut self) -> &mut AnimatorBuilderBase;

    /// Builds the keyframe animator for the keyframe array (`makeFromKeyframes`).
    fn make_from_keyframes(
        &mut self,
        abuilder: &AnimationBuilder<'_>,
        jkfs: &ArrayValue,
    ) -> Option<Rc<dyn KeyframeAnimator>>;

    /// Builds the animator that evaluates an expression (`makeFromExpression`).
    fn make_from_expression(
        &mut self,
        expression_manager: &dyn ExpressionManager,
        expression: &str,
    ) -> Option<Rc<dyn Animator>>;

    /// Parses a static value into the target (`parseValue`).
    fn parse_value(&self, abuilder: &AnimationBuilder<'_>, jv: &Value) -> bool;

    /// Parses the value of one keyframe (`parseKFValue`).
    fn parse_kf_value(
        &mut self,
        abuilder: &AnimationBuilder<'_>,
        jkf: &ObjectValue,
        jv: &Value,
        v: &mut KeyframeValue,
    ) -> bool;
}

/// Parses the keyframe array into the builder's base state (`AnimatorBuilder::parseKeyframes`).
///
/// Keyframe format:
///
/// ```text
/// [                        // array of
///   {
///     "t": <float>         // keyframe time
///     "s": <T>             // keyframe value
///     "h": <bool>          // optional constant/hold keyframe marker
///     "i": [<float,float>] // optional "in" Bezier control point
///     "o": [<float,float>] // optional "out" Bezier control point
///   },
///   ...
/// ]
/// ```
///
/// Legacy keyframe format:
///
/// ```text
/// [                        // array of
///   {
///     "t": <float>         // keyframe time
///     "s": <T>             // keyframe start value
///     "e": <T>             // keyframe end value
///     "h": <bool>          // optional constant/hold keyframe marker (constant mapping)
///     "i": [<float,float>] // optional "in" Bezier control point (cubic mapping)
///     "o": [<float,float>] // optional "out" Bezier control point (cubic mapping)
///   },
///   ...
///   {
///     "t": <float>         // last keyframe only specifies a t
///                          // the value is prev. keyframe end value
///   }
/// ]
/// ```
///
/// Note: the legacy format contains duplicates, as normal frames are contiguous:
///       frame(n).e == frame(n+1).s
// Port of: modules/skottie/src/animator/KeyframeAnimator.cpp#L91-L164 (chrome/m156)
pub fn parse_keyframes(
    builder: &mut dyn AnimatorBuilder,
    abuilder: &AnimationBuilder<'_>,
    jkfs: &ArrayValue,
) -> bool {
    let keyframe_type = builder.base().keyframe_type;

    let parse_value = |builder: &mut dyn AnimatorBuilder,
                           jkf: &ObjectValue,
                           i: usize,
                           v: &mut KeyframeValue| {
        let mut parsed = builder.parse_kf_value(abuilder, jkf, jkf.get("s"), v);

        // A missing value is only OK for the last legacy KF
        // (where it is pulled from prev KF 'end' value).
        if !parsed && i > 0 && i == jkfs.size() - 1 {
            let prev_kf = jkfs[i - 1]
                .as_object()
                .expect("the previous keyframe was parsed as an object");
            parsed = builder.parse_kf_value(abuilder, jkf, prev_kf.get("e"), v);
        }

        parsed
    };

    let mut constant_value = true;

    builder.base().kfs.reserve(jkfs.size());

    for i in 0..jkfs.size() {
        let Some(jkf) = jkfs[i].as_object() else {
            return false;
        };

        let Some(t) = parse_value_f32(jkf.get("t")) else {
            return false;
        };

        let mut v = KeyframeValue::default();
        if !parse_value(builder, jkf, i, &mut v) {
            return false;
        }

        if i > 0 {
            let base = builder.base();
            let prev_kf = base.kfs.last_mut().expect("i > 0 has a previous keyframe");

            // Ts must be monotonic.
            if t < prev_kf.t {
                return false;
            }

            // We can power-reduce the mapping of repeated values (implicitly constant).
            if v.equals(prev_kf.v, keyframe_type) {
                prev_kf.mapping = Keyframe::CONSTANT_MAPPING;
            }
        }

        let base = builder.base();
        let mapping = base.parse_mapping(jkf);
        base.kfs.push(Keyframe { t, v, mapping });

        constant_value = constant_value && v.equals(base.kfs[0].v, keyframe_type);
    }

    let base = builder.base();
    debug_assert!(base.kfs.len() == jkfs.size());
    base.cms.shrink_to_fit();

    if constant_value {
        // When all keyframes hold the same value, we can discard all but one
        // (interpolation has no effect).
        base.kfs.truncate(1);
    }

    true
}

fn parse_value_f32(v: &Value) -> Option<f32> {
    parse_value::<f32>(v)
}

/// `Lerp(a, b, t)`: `a + (b - a) * t`, for scalars.
// Port of: modules/skottie/src/animator/KeyframeAnimator.h#L158-L159 (chrome/m156) (`Lerp`)
#[must_use]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

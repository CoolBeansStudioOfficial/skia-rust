// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::contour_measure::{ContourMeasure, ContourMeasureIter};
use skia_rust_core::floating_point::float_radians_to_degrees;
use skia_rust_core::m44::V2;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{Scalar, scalar_atan2};

use crate::ExpressionEvaluator;
use crate::ExpressionManager;
use crate::json::{ArrayValue, ObjectValue, Value};
use crate::skottie_json::{Parse, parse_default, parse_slot_id, string_text};

use super::keyframe_animator::{
    AnimatorBuilder, AnimatorBuilderBase, KeyframeAnimator, KeyframeData, KeyframeValue,
    KeyframeValueType, lerp, parse_keyframes,
};
use super::{Animator, Prop, PropertyContainer, ScalarTarget, StateChanged};
use crate::internal::skottie_priv::AnimationBuilder;

/// A spatial keyframe value: a point, and the optional contour of the motion path to the next
/// keyframe.
// Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L35-L38 (chrome/m156) (`Vec2KeyframeAnimator::SpatialValue`)
#[derive(Debug, Clone, Default)]
struct SpatialValue {
    v2: V2,
    cmeasure: Option<Rc<ContourMeasure>>,
}

/// `std::max(a, b)`: `(a < b) ? b : a`.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// Spatial 2D specialization: stores `SkV2`s and optional contour interpolators externally.
// Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L32-L120 (chrome/m156) (`Vec2KeyframeAnimator`)
struct Vec2KeyframeAnimator {
    data: KeyframeData,
    values: Vec<SpatialValue>,
    vec_target: Prop<V2>,
    rot_target: Option<Prop<f32>>,
}

impl Vec2KeyframeAnimator {
    // Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L50-L62 (chrome/m156) (`update`)
    fn update(&self, new_vec_value: V2, new_tan_value: V2) -> StateChanged {
        let mut changed = new_vec_value != self.vec_target.get();
        self.vec_target.set(new_vec_value);

        if let Some(rot_target) = &self.rot_target {
            // skia-rust: libm (std::atan2)
            let new_rot_value =
                float_radians_to_degrees(scalar_atan2(new_tan_value.y, new_tan_value.x));
            #[allow(clippy::float_cmp)] // exact comparison, as in Skia
            let rot_changed = new_rot_value != rot_target.get();
            changed |= rot_changed;
            rot_target.set(new_rot_value);
        }

        changed
    }
}

impl Animator for Vec2KeyframeAnimator {
    // Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L64-L118 (chrome/m156) (`onSeek`)
    fn seek(&self, t: f32) -> StateChanged {
        let get_lerp_info = |t: f32| {
            let mut lerp_info = self.data.get_lerp_info(t);

            // When tracking rotation/orientation, the last keyframe requires special handling:
            // it doesn't store any spatial information but it is expected to maintain the
            // previous orientation (per AE semantics).
            //
            // The easiest way to achieve this is to actually swap with the previous keyframe,
            // with an adjusted weight of 1.
            let vidx = lerp_info.vrec0.idx();
            if self.rot_target.is_some() && vidx as usize == self.values.len() - 1 && vidx > 0 {
                debug_assert!(self.values[vidx as usize].cmeasure.is_none());
                debug_assert_eq!(lerp_info.vrec1.idx(), vidx);

                // Change LERPInfo{0, SIZE - 1, SIZE - 1}
                // to     LERPInfo{1, SIZE - 2, SIZE - 1}
                lerp_info.weight = 1.0;
                lerp_info.vrec0 = KeyframeValue::from_idx(vidx - 1);

                // This yields equivalent lerp results because keyframed values are contiguous
                // i.e frame[n-1].end_val == frame[n].start_val.
            }

            lerp_info
        };

        let lerp_info = get_lerp_info(t);

        let v0 = &self.values[lerp_info.vrec0.idx() as usize];
        if let Some(cmeasure) = &v0.cmeasure {
            // Spatial keyframe: the computed weight is relative to the interpolation path
            // arc length.
            let len = cmeasure.length();
            let distance = len * lerp_info.weight;
            if let Some((mut pos, tan)) = cmeasure.pos_tan(distance) {
                // Easing can yield a sub/super normal weight, which in turn can cause the
                // interpolation position to become negative or larger than the path length.
                // In those cases the expectation is to extrapolate using the endpoint tangent.
                if distance < 0.0 || distance > len {
                    let overshoot = std_max(-distance, distance - len).copysign(distance);
                    pos += tan * overshoot;
                }

                return self.update(V2::new(pos.x, pos.y), V2::new(tan.x, tan.y));
            }
        }

        let v1 = &self.values[lerp_info.vrec1.idx() as usize];
        let tan = v1.v2 - v0.v2;

        self.update(
            V2::new(
                lerp(v0.v2.x, v1.v2.x, lerp_info.weight),
                lerp(v0.v2.y, v1.v2.y, lerp_info.weight),
            ),
            tan,
        )
    }
}

impl KeyframeAnimator for Vec2KeyframeAnimator {
    fn is_constant(&self) -> bool {
        self.data.is_constant()
    }
}

// Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L120-L142 (chrome/m156) (`Vec2ExpressionAnimator`)
struct Vec2ExpressionAnimator {
    expression_evaluator: Rc<dyn ExpressionEvaluator<Vec<f32>>>,
    target: Prop<V2>,
}

impl Animator for Vec2ExpressionAnimator {
    fn seek(&self, t: f32) -> StateChanged {
        let old_value = self.target.get();

        let result = self.expression_evaluator.evaluate(t);
        let new_value = V2::new(
            if result.is_empty() { 0.0 } else { result[0] },
            if result.len() > 1 { result[1] } else { 0.0 },
        );
        self.target.set(new_value);

        new_value != old_value
    }
}

// Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L144-L252 (chrome/m156) (`Vec2AnimatorBuilder`)
struct Vec2AnimatorBuilder {
    base: AnimatorBuilderBase,
    values: Vec<SpatialValue>,
    vec_target: Prop<V2>,          // required
    rot_target: Option<Prop<f32>>, // optional
    ti: V2,
    to: V2,
    pending_spatial: bool,
}

impl Vec2AnimatorBuilder {
    // Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L184-L229 (chrome/m156) (`backfill_spatial`)
    fn backfill_spatial(&mut self, val: &SpatialValue) {
        debug_assert!(!self.values.is_empty());
        let (ti, to) = (self.ti, self.to);
        let prev_val = self
            .values
            .last_mut()
            .expect("backfill_spatial has a previous value");
        debug_assert!(prev_val.cmeasure.is_none());

        if val.v2 == prev_val.v2 {
            // spatial interpolation only make sense for noncoincident values
            return;
        }

        // Check whether v0 and v1 have the same direction AND ||v0||>=||v1||
        let check_vecs = |v0: V2, v1: V2| {
            let v0_len2 = v0.length_squared();
            let v1_len2 = v1.length_squared();

            // check magnitude
            if v0_len2 < v1_len2 {
                return false;
            }

            // v0, v1 have the same direction iff dot(v0,v1) = ||v0||*||v1||
            // <=>    dot(v0,v1)^2 = ||v0||^2 * ||v1||^2
            let dot = v0.dot(v1);
            f32::nearly_equal(dot * dot, v0_len2 * v1_len2, None)
        };

        if check_vecs(val.v2 - prev_val.v2, to) && check_vecs(prev_val.v2 - val.v2, ti) {
            // Both control points lie on the [prev_val..val] segment
            //   => we can power-reduce the Bezier "curve" to a straight line.
            return;
        }

        // Finally, this looks like a legitimate spatial keyframe.
        let mut p = PathBuilder::new();
        p.move_to(Point {
            x: prev_val.v2.x,
            y: prev_val.v2.y,
        });
        p.cubic_to(
            Point {
                x: prev_val.v2.x + to.x,
                y: prev_val.v2.y + to.y,
            },
            Point {
                x: val.v2.x + ti.x,
                y: val.v2.y + ti.y,
            },
            Point {
                x: val.v2.x,
                y: val.v2.y,
            },
        );
        prev_val.cmeasure = ContourMeasureIter::new(&p.detach(), false, None)
            .next()
            .map(Rc::new);
    }
}

impl AnimatorBuilder for Vec2AnimatorBuilder {
    fn base(&mut self) -> &mut AnimatorBuilderBase {
        &mut self.base
    }

    fn make_from_keyframes(
        &mut self,
        abuilder: &AnimationBuilder<'_>,
        jkfs: &ArrayValue,
    ) -> Option<Rc<dyn KeyframeAnimator>> {
        debug_assert!(jkfs.size() > 0);

        self.values.reserve(jkfs.size());
        if !parse_keyframes(self, abuilder, jkfs) {
            return None;
        }
        self.values.shrink_to_fit();

        let kfs = std::mem::take(&mut self.base.kfs);
        let cms = std::mem::take(&mut self.base.cms);
        Some(Rc::new(Vec2KeyframeAnimator {
            data: KeyframeData::new(kfs, cms),
            values: std::mem::take(&mut self.values),
            vec_target: self.vec_target.clone(),
            rot_target: self.rot_target.clone(),
        }))
    }

    fn make_from_expression(
        &mut self,
        em: &dyn ExpressionManager,
        expr: &str,
    ) -> Option<Rc<dyn Animator>> {
        // skia-rust: Skia wraps a null evaluator and crashes on the first seek.
        let expression_evaluator = em.create_array_expression_evaluator(expr)?;
        Some(Rc::new(Vec2ExpressionAnimator {
            expression_evaluator,
            target: self.vec_target.clone(),
        }))
    }

    fn parse_value(&self, _abuilder: &AnimationBuilder<'_>, jv: &Value) -> bool {
        V2::parse(jv, &mut self.vec_target.borrow_mut())
    }

    // Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L231-L255 (chrome/m156) (`parseKFValue`)
    fn parse_kf_value(
        &mut self,
        _abuilder: &AnimationBuilder<'_>,
        jkf: &ObjectValue,
        jv: &Value,
        v: &mut KeyframeValue,
    ) -> bool {
        let mut val = SpatialValue::default();
        if !V2::parse(jv, &mut val.v2) {
            return false;
        }

        if self.pending_spatial {
            self.backfill_spatial(&val);
        }

        // Track the last keyframe spatial tangents (checked on next parseValue).
        self.ti = parse_default::<V2>(jkf.get("ti"), V2::new(0.0, 0.0));
        self.to = parse_default::<V2>(jkf.get("to"), V2::new(0.0, 0.0));
        self.pending_spatial = self.ti != V2::new(0.0, 0.0) || self.to != V2::new(0.0, 0.0);

        if self.values.is_empty()
            || val.v2 != self.values[self.values.len() - 1].v2
            || self.pending_spatial
        {
            self.values.push(val);
        }

        *v = KeyframeValue::from_idx(
            u32::try_from(self.values.len() - 1).expect("fewer than 2^32 keyframe values"),
        );

        true
    }
}

/// Binds a 2D vector, with an optional orientation target (`bindAutoOrientable`).
// Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L257-L284 (chrome/m156)
pub(crate) fn bind_auto_orientable(
    container: &PropertyContainer,
    abuilder: &AnimationBuilder<'_>,
    jprop: Option<&ObjectValue>,
    v: &Prop<V2>,
    orientation: Option<&Prop<f32>>,
) -> bool {
    let Some(jprop) = jprop else {
        return false;
    };

    if let Some(sid) = parse_slot_id(Some(jprop)) {
        container.set_has_slot_id();
        abuilder
            .slot_manager()
            .track_vec2_value(&string_text(sid), v.clone(), container.this());
    }

    if !parse_default::<bool>(jprop.get("s"), false) {
        // Regular (static or keyframed) 2D value.
        let mut builder = Vec2AnimatorBuilder {
            base: AnimatorBuilderBase::new(KeyframeValueType::Index),
            values: Vec::new(),
            vec_target: v.clone(),
            rot_target: orientation.cloned(),
            ti: V2::new(0.0, 0.0),
            to: V2::new(0.0, 0.0),
            pending_spatial: false,
        };
        return container.bind_impl(abuilder, Some(jprop), &mut builder);
    }

    // Separate-dimensions vector value: each component is animated independently.
    let bound_x =
        container.bind_scalar_target(abuilder, jprop.get("x"), ScalarTarget::Vec2X(v.clone()));
    let bound_y =
        container.bind_scalar_target(abuilder, jprop.get("y"), ScalarTarget::Vec2Y(v.clone()));
    bound_x || bound_y
}

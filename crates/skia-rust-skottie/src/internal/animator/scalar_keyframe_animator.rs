// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/animator/ScalarKeyframeAnimator.cpp (chrome/m156)

use std::rc::Rc;

use crate::ExpressionEvaluator;
use crate::ExpressionManager;
use crate::json::{ArrayValue, ObjectValue, Value};
use crate::skottie_json::{Parse, parse_slot_id, string_text};

use super::keyframe_animator::{
    AnimatorBuilder, AnimatorBuilderBase, KeyframeAnimator, KeyframeData, KeyframeValue,
    KeyframeValueType, lerp, parse_keyframes,
};
use super::{Animator, PropertyContainer, ScalarTarget, StateChanged};
use crate::internal::skottie_priv::AnimationBuilder;

/// Scalar specialization: stores scalar values (floats) inline in keyframes.
// Port of: modules/skottie/src/animator/ScalarKeyframeAnimator.cpp#L28-L50 (chrome/m156) (`ScalarKeyframeAnimator`)
struct ScalarKeyframeAnimator {
    data: KeyframeData,
    target: ScalarTarget,
}

impl Animator for ScalarKeyframeAnimator {
    fn seek(&self, t: f32) -> StateChanged {
        let lerp_info = self.data.get_lerp_info(t);
        let old_value = self.target.get();

        let new_value = lerp(
            lerp_info.vrec0.flt(),
            lerp_info.vrec1.flt(),
            lerp_info.weight,
        );
        self.target.set(new_value);

        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        let changed = new_value != old_value;
        changed
    }
}

impl KeyframeAnimator for ScalarKeyframeAnimator {
    fn is_constant(&self) -> bool {
        self.data.is_constant()
    }
}

/// Evaluates a scalar expression.
// Port of: modules/skottie/src/animator/ScalarKeyframeAnimator.cpp#L52-L72 (chrome/m156) (`ScalarExpressionAnimator`)
struct ScalarExpressionAnimator {
    expression_evaluator: Rc<dyn ExpressionEvaluator<f32>>,
    target: ScalarTarget,
}

impl Animator for ScalarExpressionAnimator {
    fn seek(&self, t: f32) -> StateChanged {
        let old_value = self.target.get();

        let new_value = self.expression_evaluator.evaluate(t);
        self.target.set(new_value);

        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        let changed = new_value != old_value;
        changed
    }
}

// Port of: modules/skottie/src/animator/ScalarKeyframeAnimator.cpp#L74-L102 (chrome/m156) (`ScalarAnimatorBuilder`)
struct ScalarAnimatorBuilder {
    base: AnimatorBuilderBase,
    target: ScalarTarget,
}

impl AnimatorBuilder for ScalarAnimatorBuilder {
    fn base(&mut self) -> &mut AnimatorBuilderBase {
        &mut self.base
    }

    fn make_from_keyframes(
        &mut self,
        abuilder: &AnimationBuilder<'_>,
        jkfs: &ArrayValue,
    ) -> Option<Rc<dyn KeyframeAnimator>> {
        debug_assert!(jkfs.size() > 0);
        if !parse_keyframes(self, abuilder, jkfs) {
            return None;
        }

        let kfs = std::mem::take(&mut self.base.kfs);
        let cms = std::mem::take(&mut self.base.cms);
        Some(Rc::new(ScalarKeyframeAnimator {
            data: KeyframeData::new(kfs, cms),
            target: self.target.clone(),
        }))
    }

    fn make_from_expression(
        &mut self,
        em: &dyn ExpressionManager,
        expr: &str,
    ) -> Option<Rc<dyn Animator>> {
        // skia-rust: Skia wraps a null evaluator and crashes on the first seek; without an
        // evaluator there is nothing to animate.
        let expression_evaluator = em.create_number_expression_evaluator(expr)?;
        Some(Rc::new(ScalarExpressionAnimator {
            expression_evaluator,
            target: self.target.clone(),
        }))
    }

    fn parse_value(&self, _abuilder: &AnimationBuilder<'_>, jv: &Value) -> bool {
        let mut value = self.target.get();
        // Skia parses straight into the target: a failed parse leaves it unchanged.
        let parsed = f32::parse(jv, &mut value);
        self.target.set(value);
        parsed
    }

    fn parse_kf_value(
        &mut self,
        _abuilder: &AnimationBuilder<'_>,
        _jkf: &ObjectValue,
        jv: &Value,
        v: &mut KeyframeValue,
    ) -> bool {
        let mut flt = v.flt();
        let parsed = f32::parse(jv, &mut flt);
        *v = KeyframeValue::from_flt(flt);
        parsed
    }
}

/// Binds a scalar target (`bind<ScalarValue>`).
// Port of: modules/skottie/src/animator/ScalarKeyframeAnimator.cpp#L106-L117 (chrome/m156)
pub(crate) fn bind(
    container: &PropertyContainer,
    abuilder: &AnimationBuilder<'_>,
    jprop: Option<&ObjectValue>,
    target: ScalarTarget,
) -> bool {
    if let Some(sid) = parse_slot_id(jprop) {
        container.set_has_slot_id();
        abuilder.slot_manager().track_scalar_value(
            &string_text(sid),
            target.clone(),
            container.this(),
        );
    }
    let mut builder = ScalarAnimatorBuilder {
        base: AnimatorBuilderBase::new(KeyframeValueType::Scalar),
        target,
    };

    container.bind_impl(abuilder, jprop, &mut builder)
}

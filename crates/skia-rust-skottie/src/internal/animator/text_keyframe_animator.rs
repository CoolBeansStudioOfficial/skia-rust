// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/animator/TextKeyframeAnimator.cpp (chrome/m156)

use std::rc::Rc;

use crate::ExpressionEvaluator;
use crate::ExpressionManager;
use crate::internal::text::text_value::{TextValue, parse};
use crate::json::{ArrayValue, ObjectValue, Value};

use super::keyframe_animator::{
    AnimatorBuilder, AnimatorBuilderBase, KeyframeAnimator, KeyframeData, KeyframeValue,
    KeyframeValueType, parse_keyframes,
};
use super::{Animator, Prop, PropertyContainer, StateChanged};
use crate::internal::skottie_priv::AnimationBuilder;

// Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L27-L51 (chrome/m156) (`TextKeyframeAnimator`)
struct TextKeyframeAnimator {
    data: KeyframeData,
    values: Vec<TextValue>,
    target: Prop<TextValue>,
}

impl Animator for TextKeyframeAnimator {
    // Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L35-L46 (chrome/m156)
    fn seek(&self, t: f32) -> StateChanged {
        let lerp_info = self.data.get_lerp_info(t);

        // Text value keyframes are treated as selectors, not as interpolated values.
        let value = &self.values[lerp_info.vrec0.idx() as usize];
        if *self.target.borrow() != *value {
            self.target.set(value.clone());
            return true;
        }

        false
    }
}

impl KeyframeAnimator for TextKeyframeAnimator {
    fn is_constant(&self) -> bool {
        self.data.is_constant()
    }
}

// Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L54-L73 (chrome/m156) (`TextExpressionAnimator`)
struct TextExpressionAnimator {
    expression_evaluator: Rc<dyn ExpressionEvaluator<String>>,
    target: Prop<TextValue>,
}

impl Animator for TextExpressionAnimator {
    // Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L62-L68 (chrome/m156)
    fn seek(&self, t: f32) -> StateChanged {
        let old_value = self.target.borrow().text.clone();

        let new_value = self.expression_evaluator.evaluate(t);
        let changed = new_value != old_value;
        self.target.borrow_mut().text = new_value;

        changed
    }
}

// Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L75-L127 (chrome/m156) (`TextAnimatorBuilder`)
struct TextAnimatorBuilder {
    base: AnimatorBuilderBase,
    values: Vec<TextValue>,
    target: Prop<TextValue>,
}

impl TextAnimatorBuilder {
    // Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L77-L79 (chrome/m156)
    fn new(target: &Prop<TextValue>) -> Self {
        Self {
            base: AnimatorBuilderBase::new(KeyframeValueType::Index),
            values: Vec::new(),
            target: target.clone(),
        }
    }
}

impl AnimatorBuilder for TextAnimatorBuilder {
    fn base(&mut self) -> &mut AnimatorBuilderBase {
        &mut self.base
    }

    // Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L81-L97 (chrome/m156)
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
        Some(Rc::new(TextKeyframeAnimator {
            data: KeyframeData::new(kfs, cms),
            values: std::mem::take(&mut self.values),
            target: self.target.clone(),
        }))
    }

    // Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L99-L103 (chrome/m156)
    fn make_from_expression(
        &mut self,
        em: &dyn ExpressionManager,
        expr: &str,
    ) -> Option<Rc<dyn Animator>> {
        // skia-rust: Skia wraps a null evaluator and crashes on the first seek; without an
        // evaluator there is nothing to animate.
        let expression_evaluator = em.create_string_expression_evaluator(expr)?;
        Some(Rc::new(TextExpressionAnimator {
            expression_evaluator,
            target: self.target.clone(),
        }))
    }

    // Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L105-L107 (chrome/m156)
    fn parse_value(&self, abuilder: &AnimationBuilder<'_>, jv: &Value) -> bool {
        // Skia parses straight into the target: a failed parse leaves what it wrote.
        let mut value = self.target.get();
        let parsed = parse(jv, abuilder, &mut value);
        self.target.set(value);
        parsed
    }

    // Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L109-L124 (chrome/m156)
    fn parse_kf_value(
        &mut self,
        abuilder: &AnimationBuilder<'_>,
        _jkf: &ObjectValue,
        jv: &Value,
        v: &mut KeyframeValue,
    ) -> bool {
        let mut val = TextValue::default();
        if !parse(jv, abuilder, &mut val) {
            return false;
        }

        // TODO: full deduping?
        if self.values.last().is_none_or(|last| val != *last) {
            self.values.push(val);
        }

        *v = KeyframeValue::from_idx(
            u32::try_from(self.values.len() - 1).expect("fewer than 2^32 values"),
        );

        true
    }
}

/// Binds a text target (`bind<TextValue>`).
// Port of: modules/skottie/src/animator/TextKeyframeAnimator.cpp#L131-L137 (chrome/m156)
pub(crate) fn bind(
    container: &PropertyContainer,
    abuilder: &AnimationBuilder<'_>,
    jprop: Option<&ObjectValue>,
    target: &Prop<TextValue>,
) -> bool {
    let mut builder = TextAnimatorBuilder::new(target);
    container.bind_impl(abuilder, jprop, &mut builder)
}

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/animator/VectorKeyframeAnimator.h,
// VectorKeyframeAnimator.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::scalar::Scalar;

use crate::ExpressionEvaluator;
use crate::ExpressionManager;
use crate::json::{ArrayValue, ObjectValue, Value};
use crate::skottie_json::{Parse, ValueExt, parse_default};
use crate::skottie_value::{ColorValue, ShapeValue, VectorValue};

use super::keyframe_animator::{
    AnimatorBuilder, AnimatorBuilderBase, KeyframeAnimator, KeyframeData, KeyframeValue,
    KeyframeValueType, lerp, parse_keyframes,
};
use super::{Animator, Prop, PropertyContainer, StateChanged};
use crate::internal::skottie_priv::AnimationBuilder;

/// A property value that is a vector of floats (`std::vector<float>`): the target type of the
/// vector animators.
pub trait FloatVec: Default + 'static {
    /// The floats.
    fn floats(&self) -> &Vec<f32>;
    /// The floats, mutable.
    fn floats_mut(&mut self) -> &mut Vec<f32>;
}

macro_rules! float_vec {
    ($ty:ty) => {
        impl FloatVec for $ty {
            fn floats(&self) -> &Vec<f32> {
                self
            }

            fn floats_mut(&mut self) -> &mut Vec<f32> {
                self
            }
        }
    };
}

float_vec!(VectorValue);
float_vec!(ColorValue);
float_vec!(ShapeValue);

/// Parses the length of a vector value (`VectorLenParser`).
// Port of: modules/skottie/src/animator/VectorKeyframeAnimator.h#L30 (chrome/m156)
pub type VectorLenParser = fn(&Value, &mut usize) -> bool;

/// Parses the data of a vector value of the given length into `data` (`VectorDataParser`).
// Port of: modules/skottie/src/animator/VectorKeyframeAnimator.h#L31 (chrome/m156)
pub type VectorDataParser = fn(&Value, usize, &mut [f32]) -> bool;

/// Parses an array of exact size.
// Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L33-L46 (chrome/m156) (`parse_array`)
fn parse_array(ja: Option<&ArrayValue>, a: &mut [f32], count: usize) -> bool {
    let Some(ja) = ja else {
        return false;
    };
    if ja.size() != count {
        return false;
    }

    for i in 0..count {
        if !f32::parse(&ja[i], &mut a[i]) {
            return false;
        }
    }

    true
}

/// Vector specialization - stores float vector values (of same length) in consolidated/contiguous
/// storage. Keyframe records hold the storage offset for each value:
///
/// ```text
/// storage: [     vec0     ][     vec1     ] ... [     vecN     ]
///           <-  vec_len ->  <-  vec_len ->       <-  vec_len ->
///
///          ^               ^                    ^
/// kfs[]: .idx            .idx       ...       .idx
/// ```
// Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L91-L158 (chrome/m156) (`VectorKeyframeAnimator`)
struct VectorKeyframeAnimator<T: FloatVec> {
    data: KeyframeData,
    storage: Vec<f32>,
    vec_len: usize,
    target: Prop<T>,
}

impl<T: FloatVec> VectorKeyframeAnimator<T> {
    fn new(data: KeyframeData, storage: Vec<f32>, vec_len: usize, target: Prop<T>) -> Self {
        // Resize the target value appropriately.
        target.borrow_mut().floats_mut().resize(vec_len, 0.0);
        Self {
            data,
            storage,
            vec_len,
            target,
        }
    }
}

impl<T: FloatVec> Animator for VectorKeyframeAnimator<T> {
    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L108-L150 (chrome/m156) (`onSeek`)
    fn seek(&self, t: f32) -> StateChanged {
        let lerp_info = self.data.get_lerp_info(t);

        let o0 = lerp_info.vrec0.idx() as usize;
        let o1 = lerp_info.vrec1.idx() as usize;
        debug_assert!(o0 + self.vec_len <= self.storage.len());
        debug_assert!(o1 + self.vec_len <= self.storage.len());

        let v0 = &self.storage[o0..o0 + self.vec_len];
        let v1 = &self.storage[o1..o1 + self.vec_len];
        let mut target = self.target.borrow_mut();
        let dst = target.floats_mut();
        debug_assert!(dst.len() == self.vec_len);

        let is_constant = lerp_info.vrec0.equals(lerp_info.vrec1, KeyframeValueType::Index);
        if is_constant {
            // memcmp: bitwise comparison.
            if dst.iter().zip(v0).any(|(d, s)| d.to_bits() != s.to_bits()) {
                dst.copy_from_slice(v0);
                return true;
            }
            return false;
        }

        // Skia processes the vector in float4 chunks and a scalar tail; the arithmetic is
        // lane-wise, so a single loop produces the same values.
        let mut changed = false;
        for ((d, a), b) in dst.iter_mut().zip(v0).zip(v1) {
            let new_val = lerp(*a, *b, lerp_info.weight);

            #[allow(clippy::float_cmp)] // exact comparison, as in Skia
            let value_changed = new_val != *d;
            changed |= value_changed;
            *d = new_val;
        }

        changed
    }
}

impl<T: FloatVec> KeyframeAnimator for VectorKeyframeAnimator<T> {
    fn is_constant(&self) -> bool {
        self.data.is_constant()
    }
}

// Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L160-L187 (chrome/m156) (`VectorExpressionAnimator`)
struct VectorExpressionAnimator<T: FloatVec> {
    expression_evaluator: Rc<dyn ExpressionEvaluator<Vec<f32>>>,
    target: Prop<T>,
}

impl<T: FloatVec> Animator for VectorExpressionAnimator<T> {
    fn seek(&self, t: f32) -> StateChanged {
        let result = self.expression_evaluator.evaluate(t);
        let mut changed = false;
        let mut target = self.target.borrow_mut();
        let dst = target.floats_mut();
        for (i, slot) in dst.iter_mut().enumerate() {
            // Use 0 as a default if the result is too small.
            let val = if i >= result.len() { 0.0 } else { result[i] };
            if !f32::nearly_equal(val, *slot, None) {
                changed = true;
            }
            *slot = val;
        }

        changed
    }
}

/// The animator builder of the float vector types.
// Port of: modules/skottie/src/animator/VectorKeyframeAnimator.h#L28-L58 (chrome/m156) (`VectorAnimatorBuilder`)
pub struct VectorAnimatorBuilder<T: FloatVec> {
    base: AnimatorBuilderBase,
    parse_len: VectorLenParser,
    parse_data: VectorDataParser,
    storage: Vec<f32>,
    /// Size of individual vector values we store.
    vec_len: usize,
    /// Vector value index being parsed (the corresponding storage offset is
    /// `current_vec * vec_len`).
    current_vec: usize,
    target: Prop<T>,
}

impl<T: FloatVec> VectorAnimatorBuilder<T> {
    /// A builder for `target`.
    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L189-L196 (chrome/m156)
    #[must_use]
    pub fn new(target: Prop<T>, parse_len: VectorLenParser, parse_data: VectorDataParser) -> Self {
        Self {
            base: AnimatorBuilderBase::new(KeyframeValueType::Index),
            parse_len,
            parse_data,
            storage: Vec::new(),
            vec_len: 0,
            current_vec: 0,
            target,
        }
    }
}

impl<T: FloatVec> AnimatorBuilder for VectorAnimatorBuilder<T> {
    fn base(&mut self) -> &mut AnimatorBuilderBase {
        &mut self.base
    }

    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L198-L234 (chrome/m156)
    fn make_from_keyframes(
        &mut self,
        abuilder: &AnimationBuilder<'_>,
        jkfs: &ArrayValue,
    ) -> Option<Rc<dyn KeyframeAnimator>> {
        debug_assert!(jkfs.size() > 0);

        // peek at the first keyframe value to find our vector length
        let jkf0 = jkfs[0].as_object()?;
        if !(self.parse_len)(jkf0.get("s"), &mut self.vec_len) {
            return None;
        }

        // total elements: vector length x number vectors
        let total_size = self.vec_len.checked_mul(jkfs.size())?;

        // we must be able to store all offsets in Keyframe::Value::idx (uint32_t)
        if u32::try_from(total_size).is_err() {
            return None;
        }
        self.storage.resize(total_size, 0.0);

        if !parse_keyframes(self, abuilder, jkfs) {
            return None;
        }

        // parseKFValue() might have stored fewer vectors thanks to tail-deduping.
        debug_assert!(self.current_vec <= jkfs.size());
        self.storage.resize(self.current_vec * self.vec_len, 0.0);
        self.storage.shrink_to_fit();

        let kfs = std::mem::take(&mut self.base.kfs);
        let cms = std::mem::take(&mut self.base.cms);
        Some(Rc::new(VectorKeyframeAnimator::new(
            KeyframeData::new(kfs, cms),
            std::mem::take(&mut self.storage),
            self.vec_len,
            self.target.clone(),
        )))
    }

    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L236-L242 (chrome/m156)
    fn make_from_expression(
        &mut self,
        em: &dyn ExpressionManager,
        expr: &str,
    ) -> Option<Rc<dyn Animator>> {
        // skia-rust: Skia wraps a null evaluator and crashes on the first seek.
        let expression_evaluator = em.create_array_expression_evaluator(expr)?;
        Some(Rc::new(VectorExpressionAnimator {
            expression_evaluator,
            target: self.target.clone(),
        }))
    }

    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L244-L254 (chrome/m156)
    fn parse_value(&self, _abuilder: &AnimationBuilder<'_>, jv: &Value) -> bool {
        let mut vec_len = 0;
        if !(self.parse_len)(jv, &mut vec_len) {
            return false;
        }

        let mut target = self.target.borrow_mut();
        let floats = target.floats_mut();
        floats.resize(vec_len, 0.0);
        (self.parse_data)(jv, vec_len, floats)
    }

    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L256-L284 (chrome/m156)
    fn parse_kf_value(
        &mut self,
        _abuilder: &AnimationBuilder<'_>,
        _jkf: &ObjectValue,
        jv: &Value,
        kfv: &mut KeyframeValue,
    ) -> bool {
        let mut offset = self.current_vec * self.vec_len;
        debug_assert!(offset + self.vec_len <= self.storage.len());

        if !(self.parse_data)(jv, self.vec_len, &mut self.storage[offset..offset + self.vec_len]) {
            return false;
        }

        debug_assert!(self.current_vec == 0 || offset >= self.vec_len);
        // compare with previous vector value
        if self.current_vec > 0
            && self.storage[offset..offset + self.vec_len]
                .iter()
                .zip(&self.storage[offset - self.vec_len..offset])
                .all(|(a, b)| a.to_bits() == b.to_bits())
        {
            // repeating value -> use prev offset (dedupe)
            offset -= self.vec_len;
        } else {
            // new value -> advance the current index
            self.current_vec += 1;
        }

        // Keyframes record the storage-offset for a given vector value.
        *kfv = KeyframeValue::from_idx(
            u32::try_from(offset).expect("storage offsets fit in 32 bits (checked upfront)"),
        );

        true
    }
}

/// Binds a float vector (`bind<VectorValue>`).
// Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L286-L324 (chrome/m156)
pub(crate) fn bind_vector<T: VectorBindTarget>(
    container: &PropertyContainer,
    abuilder: &AnimationBuilder<'_>,
    jprop: Option<&ObjectValue>,
    v: &Prop<T>,
) -> bool {
    let Some(jprop) = jprop else {
        return false;
    };

    if !parse_default::<bool>(jprop.get("s"), false) {
        // Regular (static or keyframed) vector value.
        let mut builder = VectorAnimatorBuilder::new(
            v.clone(),
            // Len parser.
            |jv: &Value, len: &mut usize| -> bool {
                if let Some(ja) = jv.as_array() {
                    *len = ja.size();
                    return true;
                }
                false
            },
            // Data parser.
            |jv: &Value, len: usize, data: &mut [f32]| parse_array(jv.as_array(), data, len),
        );

        return container.bind_impl(abuilder, Some(jprop), &mut builder);
    }

    // Separate-dimensions vector value: each component is animated independently.
    T::bind_separate_dimensions(container, abuilder, jprop, v)
}

/// The vector types that bind as vectors, with the separate-dimensions form.
pub trait VectorBindTarget: FloatVec {
    /// Binds the `x`/`y`/`z` components of a separate-dimensions vector.
    fn bind_separate_dimensions(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: &ObjectValue,
        v: &Prop<Self>,
    ) -> bool;
}

impl VectorBindTarget for VectorValue {
    fn bind_separate_dimensions(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: &ObjectValue,
        v: &Prop<Self>,
    ) -> bool {
        use super::ScalarTarget;

        *v.borrow_mut() = VectorValue::from_slice(&[0.0, 0.0, 0.0]);
        let bound_x = container.bind_scalar_target(
            abuilder,
            jprop.get("x"),
            ScalarTarget::VectorAt(v.clone(), 0),
        );
        let bound_y = container.bind_scalar_target(
            abuilder,
            jprop.get("y"),
            ScalarTarget::VectorAt(v.clone(), 1),
        );
        let bound_z = container.bind_scalar_target(
            abuilder,
            jprop.get("z"),
            ScalarTarget::VectorAt(v.clone(), 2),
        );
        bound_x || bound_y || bound_z
    }
}

impl VectorBindTarget for ColorValue {
    fn bind_separate_dimensions(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: &ObjectValue,
        v: &Prop<Self>,
    ) -> bool {
        use super::ScalarTarget;

        // Skia converts the ColorValue* to a VectorValue* and binds the components of the same
        // storage.
        *v.borrow_mut() = ColorValue::from_slice(&[0.0, 0.0, 0.0]);
        let bound_x = container.bind_scalar_target(
            abuilder,
            jprop.get("x"),
            ScalarTarget::ColorAt(v.clone(), 0),
        );
        let bound_y = container.bind_scalar_target(
            abuilder,
            jprop.get("y"),
            ScalarTarget::ColorAt(v.clone(), 1),
        );
        let bound_z = container.bind_scalar_target(
            abuilder,
            jprop.get("z"),
            ScalarTarget::ColorAt(v.clone(), 2),
        );
        bound_x || bound_y || bound_z
    }
}

impl VectorBindTarget for ShapeValue {
    fn bind_separate_dimensions(
        _container: &PropertyContainer,
        _abuilder: &AnimationBuilder<'_>,
        _jprop: &ObjectValue,
        _v: &Prop<Self>,
    ) -> bool {
        // Shapes bind through their own builder and never reach the vector binding.
        false
    }
}

impl<T: FloatVec> std::fmt::Debug for VectorAnimatorBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorAnimatorBuilder").finish_non_exhaustive()
    }
}

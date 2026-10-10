// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/animator/Animator.h, Animator.cpp, modules/skottie/src/Adapter.h
// (chrome/m156)
//
// Property binding. Skia's animators write into raw `T*` targets that point at fields of the
// adapter that owns them. Here the targets are shared cells ([`Prop`]): the adapter keeps one
// handle and the animator another. A scalar target may also be one component of a vector
// property ([`ScalarTarget`]), which is what separate-dimension properties bind to.
//
// An adapter embeds a [`PropertyContainer`] (Skia's `AnimatablePropertyContainer` base) and is
// built with `Rc::new_cyclic`, so the container knows its own `Weak` handle when a slot ID asks
// the slot manager to track the adapter.

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

use skia_rust_core::m44::V2;

use crate::json::{ObjectValue, Value};
use crate::skottie::LoggerLevel;
use crate::skottie_json::{ValueExt, parse_default, parse_slot_id, string_text};
use crate::skottie_value::{ColorValue, ShapeValue, VectorValue};

use super::skottie_priv::AnimationBuilder;

pub mod keyframe_animator;
pub mod scalar_keyframe_animator;
pub mod shape_keyframe_animator;
pub mod vec2_keyframe_animator;
pub mod vector_keyframe_animator;

use keyframe_animator::AnimatorBuilder;
use vector_keyframe_animator::VectorAnimatorBuilder;

/// Whether an animator changed the state of the scene graph.
// Port of: modules/skottie/src/animator/Animator.h#L33 (chrome/m156) (`Animator::StateChanged`)
pub type StateChanged = bool;

/// Something that updates its targets for a given animation time.
// Port of: modules/skottie/src/animator/Animator.h#L31-L44 (chrome/m156) (`class Animator`)
#[doc(alias = "skottie::internal::Animator")]
pub trait Animator {
    /// Updates the animated state for the time `t` (`seek`, `onSeek`).
    #[doc(alias = "onSeek")]
    fn seek(&self, t: f32) -> StateChanged;
}

/// A shared, mutable property value: the animator writes it, and the adapter reads it.
#[doc(alias = "T*")]
#[derive(Debug)]
pub struct Prop<T>(Rc<RefCell<T>>);

impl<T> Clone for Prop<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Default> Default for Prop<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Prop<T> {
    /// A property holding `value`.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self(Rc::new(RefCell::new(value)))
    }

    /// Borrows the value.
    #[must_use]
    pub fn borrow(&self) -> Ref<'_, T> {
        self.0.borrow()
    }

    /// Mutably borrows the value.
    #[must_use]
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }

    /// Replaces the value.
    pub fn set(&self, value: T) {
        *self.0.borrow_mut() = value;
    }

    /// True if both handles refer to the same value.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: Clone> Prop<T> {
    /// A copy of the value.
    #[must_use]
    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }
}

/// A scalar animation target: a scalar property, or one component of a vector property (a
/// `ScalarValue*` that points inside a `Vec2Value` or `VectorValue`).
// Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L274-L282 (chrome/m156) (`&v->x`)
#[derive(Debug, Clone)]
pub enum ScalarTarget {
    /// A whole scalar property.
    Whole(Prop<f32>),
    /// The `x` component of a 2D vector.
    Vec2X(Prop<V2>),
    /// The `y` component of a 2D vector.
    Vec2Y(Prop<V2>),
    /// The component at an index of a vector.
    VectorAt(Prop<VectorValue>, usize),
    /// The component at an index of a color vector.
    ColorAt(Prop<ColorValue>, usize),
}

impl ScalarTarget {
    /// The current value.
    #[must_use]
    pub fn get(&self) -> f32 {
        match self {
            Self::Whole(p) => p.get(),
            Self::Vec2X(p) => p.borrow().x,
            Self::Vec2Y(p) => p.borrow().y,
            Self::VectorAt(p, i) => p.borrow()[*i],
            Self::ColorAt(p, i) => p.borrow()[*i],
        }
    }

    /// Writes the value.
    pub fn set(&self, value: f32) {
        match self {
            Self::Whole(p) => p.set(value),
            Self::Vec2X(p) => p.borrow_mut().x = value,
            Self::Vec2Y(p) => p.borrow_mut().y = value,
            Self::VectorAt(p, i) => p.borrow_mut()[*i] = value,
            Self::ColorAt(p, i) => p.borrow_mut()[*i] = value,
        }
    }
}

impl From<&Prop<f32>> for ScalarTarget {
    fn from(p: &Prop<f32>) -> Self {
        Self::Whole(p.clone())
    }
}

/// The state every animator-with-properties carries (the data members of
/// `AnimatablePropertyContainer`).
// Port of: modules/skottie/src/animator/Animator.h#L46-L81 (chrome/m156) (`class AnimatablePropertyContainer`)
pub struct PropertyContainer {
    animators: RefCell<Vec<Rc<dyn Animator>>>,
    has_synced: Cell<bool>,
    has_slot_id: Cell<bool>,
    this: Weak<dyn AnimatablePropertyContainer>,
}

impl std::fmt::Debug for PropertyContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyContainer")
            .field("animators", &self.animators.borrow().len())
            .field("has_synced", &self.has_synced.get())
            .field("has_slot_id", &self.has_slot_id.get())
            .finish()
    }
}

/// An animator that binds properties and syncs its scene graph nodes when they change.
// Port of: modules/skottie/src/animator/Animator.h#L46-L81 (chrome/m156) (`class AnimatablePropertyContainer`)
#[doc(alias = "skottie::internal::AnimatablePropertyContainer")]
pub trait AnimatablePropertyContainer: Animator {
    /// The container state.
    fn container(&self) -> &PropertyContainer;

    /// Pushes the bound values to the scene graph (`onSync`).
    #[doc(alias = "onSync")]
    fn on_sync(&self);

    /// True if no property is animated, and no slot can change one (`isStatic`).
    // Port of: modules/skottie/src/animator/Animator.h#L69 (chrome/m156)
    #[doc(alias = "isStatic")]
    fn is_static(&self) -> bool {
        self.container().is_static()
    }
}

/// Implements [`Animator`] for an [`AnimatablePropertyContainer`]: the final `onSeek`.
// Port of: modules/skottie/src/animator/Animator.cpp#L19-L34 (chrome/m156) (`AnimatablePropertyContainer::onSeek`)
#[macro_export]
#[doc(hidden)]
macro_rules! impl_container_animator {
    ($ty:ty) => {
        impl $crate::internal::animator::Animator for $ty {
            fn seek(&self, t: f32) -> bool {
                $crate::internal::animator::AnimatablePropertyContainer::container(self)
                    .seek_container(t, &|| {
                        $crate::internal::animator::AnimatablePropertyContainer::on_sync(self);
                    })
            }
        }
    };
}

impl PropertyContainer {
    /// A container for the adapter `this` (in `Rc::new_cyclic`).
    #[must_use]
    pub fn new(this: Weak<dyn AnimatablePropertyContainer>) -> Self {
        Self {
            animators: RefCell::new(Vec::new()),
            has_synced: Cell::new(false),
            has_slot_id: Cell::new(false),
            this,
        }
    }

    /// True if no property is animated, and no slot can change one (`isStatic`).
    #[must_use]
    pub fn is_static(&self) -> bool {
        self.animators.borrow().is_empty() && !self.has_slot_id.get()
    }

    /// The adapter this container belongs to.
    #[must_use]
    pub fn this(&self) -> Weak<dyn AnimatablePropertyContainer> {
        self.this.clone()
    }

    /// Marks the container as bound to a slot ID.
    pub(crate) fn set_has_slot_id(&self) {
        self.has_slot_id.set(true);
    }

    /// Adds an animator to this container's scope.
    pub fn push_animator(&self, animator: Rc<dyn Animator>) {
        self.animators.borrow_mut().push(animator);
    }

    /// `AnimatablePropertyContainer::onSeek`: seeks all child animators and syncs on the first
    /// seek, and whenever anything changed.
    // Port of: modules/skottie/src/animator/Animator.cpp#L19-L34 (chrome/m156)
    pub fn seek_container(&self, t: f32, on_sync: &dyn Fn()) -> StateChanged {
        // The very first seek must trigger a sync, to ensure proper SG setup.
        let mut changed = !self.has_synced.get();

        for animator in self.animators.borrow().iter() {
            changed |= animator.seek(t);
        }

        if changed {
            on_sync();
            self.has_synced.set(true);
        }

        changed
    }

    /// `AnimatablePropertyContainer::attachDiscardableAdapter`: a static child is synced once and
    /// dropped, an animated one is attached to this container.
    // Port of: modules/skottie/src/animator/Animator.cpp#L36-L47 (chrome/m156)
    pub fn attach_discardable_adapter(&self, child: Option<Rc<dyn AnimatablePropertyContainer>>) {
        let Some(child) = child else {
            return;
        };

        if child.is_static() {
            child.seek(0.0);
            return;
        }

        self.animators.borrow_mut().push(child);
    }

    /// `AnimatablePropertyContainer::shrink_to_fit`.
    // Port of: modules/skottie/src/animator/Animator.cpp#L49-L51 (chrome/m156)
    pub fn shrink_to_fit(&self) {
        self.animators.borrow_mut().shrink_to_fit();
    }

    /// The workhorse for property binding: depending on whether the property is animated, it
    /// applies the value immediately or instantiates and attaches a keyframe animator, scoped to
    /// this container.
    // Port of: modules/skottie/src/animator/Animator.h#L55-L62 (chrome/m156) (`bind`)
    pub fn bind<'a, T: Bindable>(
        &self,
        abuilder: &AnimationBuilder<'_>,
        jprop: impl IntoJsonProp<'a>,
        target: &Prop<T>,
    ) -> bool {
        T::bind_to(self, abuilder, jprop.into_prop(), target)
    }

    /// A flavor of bind for 2D vectors which drives an additional/optional orientation target
    /// (rotation in degrees), when bound to a motion path property.
    // Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L254-L284 (chrome/m156)
    #[doc(alias = "bindAutoOrientable")]
    pub fn bind_auto_orientable<'a>(
        &self,
        abuilder: &AnimationBuilder<'_>,
        jprop: impl IntoJsonProp<'a>,
        v: &Prop<V2>,
        orientation: Option<&Prop<f32>>,
    ) -> bool {
        vec2_keyframe_animator::bind_auto_orientable(
            self,
            abuilder,
            jprop.into_prop(),
            v,
            orientation,
        )
    }

    /// Binds a scalar target, which can be a component of a vector property.
    pub fn bind_scalar_target<'a>(
        &self,
        abuilder: &AnimationBuilder<'_>,
        jprop: impl IntoJsonProp<'a>,
        target: ScalarTarget,
    ) -> bool {
        scalar_keyframe_animator::bind(self, abuilder, jprop.into_prop(), target)
    }

    /// Binds a property with an animator builder (`bindImpl`).
    // Port of: modules/skottie/src/animator/Animator.cpp#L53-L131 (chrome/m156) (`bindImpl`)
    pub(crate) fn bind_impl(
        &self,
        abuilder: &AnimationBuilder<'_>,
        jprop: Option<&ObjectValue>,
        builder: &mut dyn AnimatorBuilder,
    ) -> bool {
        let Some(mut jprop) = jprop else {
            return false;
        };

        if let Some(jprop_slot_id) = jprop.get("sid").as_string() {
            match abuilder.get_slots_root() {
                None => {
                    abuilder.log_json(LoggerLevel::Warning, jprop, "Slotid found but no slots were found in the json. Using default values.");
                }
                Some(slots_root) => {
                    let slot = slots_root.get(&string_text(jprop_slot_id)).as_object();
                    if let Some(slot) = slot {
                        // A slot without a property object is unusable (Skia dereferences null).
                        match slot.get("p").as_object() {
                            Some(p) => jprop = p,
                            None => return false,
                        }
                    } else {
                        abuilder.log_json(LoggerLevel::Warning, jprop, "Specified slotID not found in 'slots'. Using default values.");
                    }
                }
            }
        }

        let jprop_a = jprop.get("a");
        let jprop_k = jprop.get("k");

        // Handle expressions on the property.
        if let Some(expr) = jprop.get("x").as_string() {
            if let Some(expression_manager) = abuilder.expression_manager() {
                builder.parse_value(abuilder, jprop_k);
                if let Some(expression_animator) =
                    builder.make_from_expression(expression_manager.as_ref(), &string_text(expr))
                {
                    self.animators.borrow_mut().push(expression_animator);
                    return true;
                }
            } else {
                abuilder.log_json(LoggerLevel::Warning, jprop, "Expression encountered but ExpressionManager not provided.");
            }
        }

        // Older Json versions don't have an "a" animation marker.
        // For those, we attempt to parse both ways.
        if !parse_default::<bool>(jprop_a, false) {
            if builder.parse_value(abuilder, jprop_k) {
                // Static property.
                return true;
            }

            if !matches!(jprop_a, Value::Null(_)) {
                abuilder.log_json(LoggerLevel::Error, jprop, "Could not parse (explicit) static property.");
                return false;
            }
        }

        // Keyframed property.
        let mut animator = None;
        if let Some(jkfs) = jprop_k.as_array() {
            if jkfs.size() > 0 {
                animator = builder.make_from_keyframes(abuilder, jkfs);
            }
        }

        let Some(animator) = animator else {
            abuilder.log_json(LoggerLevel::Error, jprop, "Could not parse keyframed property.");
            return false;
        };

        if animator.is_constant() {
            // If all keyframes are constant, there is no reason to treat this
            // as an animated property - apply immediately and discard the animator.
            animator.seek(0.0);
        } else {
            self.animators.borrow_mut().push(animator);
        }

        true
    }
}

/// A JSON property node: Skia's `const skjson::ObjectValue*`, null when absent or of another kind.
pub trait IntoJsonProp<'a> {
    /// The object, if there is one.
    fn into_prop(self) -> Option<&'a ObjectValue>;
}

impl<'a> IntoJsonProp<'a> for &'a Value {
    fn into_prop(self) -> Option<&'a ObjectValue> {
        self.as_object()
    }
}

impl<'a> IntoJsonProp<'a> for &'a ObjectValue {
    fn into_prop(self) -> Option<&'a ObjectValue> {
        Some(self)
    }
}

impl<'a> IntoJsonProp<'a> for Option<&'a ObjectValue> {
    fn into_prop(self) -> Option<&'a ObjectValue> {
        self
    }
}

/// A property value type that [`PropertyContainer::bind`] supports.
pub trait Bindable: Sized + 'static {
    /// Binds `target` to the property `jprop`.
    fn bind_to(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: Option<&ObjectValue>,
        target: &Prop<Self>,
    ) -> bool;
}

impl Bindable for f32 {
    // Port of: modules/skottie/src/animator/ScalarKeyframeAnimator.cpp#L106-L117 (chrome/m156)
    fn bind_to(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: Option<&ObjectValue>,
        target: &Prop<Self>,
    ) -> bool {
        scalar_keyframe_animator::bind(container, abuilder, jprop, ScalarTarget::Whole(target.clone()))
    }
}

impl Bindable for V2 {
    // Port of: modules/skottie/src/animator/Vec2KeyframeAnimator.cpp#L286-L292 (chrome/m156)
    fn bind_to(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: Option<&ObjectValue>,
        target: &Prop<Self>,
    ) -> bool {
        vec2_keyframe_animator::bind_auto_orientable(container, abuilder, jprop, target, None)
    }
}

impl Bindable for VectorValue {
    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L249-L291 (chrome/m156)
    fn bind_to(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: Option<&ObjectValue>,
        target: &Prop<Self>,
    ) -> bool {
        vector_keyframe_animator::bind_vector(container, abuilder, jprop, target)
    }
}

impl Bindable for ColorValue {
    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L293-L303 (chrome/m156)
    fn bind_to(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: Option<&ObjectValue>,
        target: &Prop<Self>,
    ) -> bool {
        if let Some(sid) = parse_slot_id(jprop) {
            container.set_has_slot_id();
            abuilder
                .slot_manager()
                .track_color_value(&string_text(sid), target.clone(), container.this());
        }
        vector_keyframe_animator::bind_vector(container, abuilder, jprop, target)
    }
}

impl Bindable for ShapeValue {
    // Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L160-L168 (chrome/m156)
    fn bind_to(
        container: &PropertyContainer,
        abuilder: &AnimationBuilder<'_>,
        jprop: Option<&ObjectValue>,
        target: &Prop<Self>,
    ) -> bool {
        let mut builder = VectorAnimatorBuilder::new(
            target.clone(),
            shape_keyframe_animator::parse_encoding_len,
            shape_keyframe_animator::parse_encoding_data,
        );

        container.bind_impl(abuilder, jprop, &mut builder)
    }
}

/// The base of the adapters that drive a single scene graph node (`DiscardableAdapterBase`).
// Port of: modules/skottie/src/Adapter.h#L18-L42 (chrome/m156) (`DiscardableAdapterBase`)
pub struct DiscardableAdapterBase<T: ?Sized> {
    container: PropertyContainer,
    node: Rc<T>,
}

impl<T: ?Sized> DiscardableAdapterBase<T> {
    /// A base for an adapter that drives `node`.
    #[must_use]
    pub fn new(this: Weak<dyn AnimatablePropertyContainer>, node: Rc<T>) -> Self {
        Self {
            container: PropertyContainer::new(this),
            node,
        }
    }

    /// The container state.
    #[must_use]
    pub fn container(&self) -> &PropertyContainer {
        &self.container
    }

    /// The node this adapter drives.
    #[must_use]
    pub fn node(&self) -> &Rc<T> {
        &self.node
    }
}

impl<T: ?Sized> std::fmt::Debug for DiscardableAdapterBase<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscardableAdapterBase")
            .field("container", &self.container)
            .finish_non_exhaustive()
    }
}

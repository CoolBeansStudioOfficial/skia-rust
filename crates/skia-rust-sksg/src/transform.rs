// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGTransform.h, modules/sksg/src/SkSGTransform.cpp,
// modules/sksg/src/SkSGTransformPriv.h (chrome/m156)
//
// Skia templates the transform nodes on `SkMatrix` and `SkM44`. `TransformValue` is the Rust form
// of those two instantiations, and `Matrix<T>`, `Concat<T>` and `Inverse<T>` are generic over it.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::m44::M44 as SkM44;
use skia_rust_core::matrix::Matrix as SkMatrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::effect_node::{effect_on_node_at, effect_on_render, effect_on_revalidate};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore, inval_traits};
use crate::render_node::{Hit, RenderContext, RenderNode};

/// A transform node: a matrix, or a composition or inverse of transforms.
// Port of: modules/sksg/include/SkSGTransform.h#L14-L33 (chrome/m156) (`class Transform`)
#[doc(alias = "sksg::Transform")]
pub trait Transform: Node {
    /// True if the transform is a 4x4 matrix (`is44`).
    fn is44(&self) -> bool;
    /// The transform as a 3x3 matrix (`asMatrix`).
    fn as_matrix(&self) -> SkMatrix;
    /// The transform as a 4x4 matrix (`asM44`).
    fn as_m44(&self) -> SkM44;
}

/// The two matrix types of the transform nodes (Skia's `T` of `Matrix<T>`, `Concat<T>` and
/// `Inverse<T>`), and their `TransformPriv` conversions.
// Port of: modules/sksg/src/SkSGTransform.cpp#L11-L33 (chrome/m156) (`AsSkMatrix`, `AsSkM44`)
pub trait TransformValue: Clone + 'static {
    /// `std::is_same<T, SkM44>`.
    const IS44: bool;
    /// `TransformPriv::As<T>(t)`.
    fn from_transform(t: &dyn Transform) -> Self;
    /// `AsSkMatrix(m)`.
    fn to_sk_matrix(self) -> SkMatrix;
    /// `AsSkM44(m)`.
    fn to_sk_m44(self) -> SkM44;
    /// `setConcat(a, b)`: `a x b`.
    fn concat(a: Self, b: Self) -> Self;
    /// `invert(&out)`: `None` if not invertible.
    fn invert(self) -> Option<Self>;
    /// The identity.
    fn identity() -> Self;
}

impl TransformValue for SkMatrix {
    const IS44: bool = false;

    fn from_transform(t: &dyn Transform) -> Self {
        t.as_matrix()
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L15-L15 (chrome/m156)
    fn to_sk_matrix(self) -> SkMatrix {
        self
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L20-L20 (chrome/m156)
    fn to_sk_m44(self) -> SkM44 {
        SkM44::from(self)
    }

    fn concat(a: Self, b: Self) -> Self {
        SkMatrix::concat(&a, &b)
    }

    fn invert(self) -> Option<Self> {
        SkMatrix::invert(&self)
    }

    fn identity() -> Self {
        SkMatrix::new_identity()
    }
}

impl TransformValue for SkM44 {
    const IS44: bool = true;

    fn from_transform(t: &dyn Transform) -> Self {
        t.as_m44()
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L17-L17 (chrome/m156)
    fn to_sk_matrix(self) -> SkMatrix {
        self.to_m33()
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L22-L22 (chrome/m156)
    fn to_sk_m44(self) -> SkM44 {
        self
    }

    fn concat(a: Self, b: Self) -> Self {
        SkM44::concat(&a, &b)
    }

    fn invert(self) -> Option<Self> {
        SkM44::invert(&self)
    }

    fn identity() -> Self {
        SkM44::new_identity()
    }
}

/// A constant matrix transform (`sksg::Matrix<T>`).
// Port of: modules/sksg/include/SkSGTransform.h#L35-L58 (chrome/m156) (`class Matrix`)
#[doc(alias = "sksg::Matrix")]
#[derive(Debug)]
pub struct Matrix<T: TransformValue + std::fmt::Debug> {
    core: NodeCore,
    matrix: RefCell<T>,
}

impl<T: TransformValue + std::fmt::Debug> Matrix<T> {
    /// A constant transform (`Matrix<T>::Make`).
    // Port of: modules/sksg/include/SkSGTransform.h#L40-L42 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(m: T) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(inval_traits::BUBBLE_DAMAGE, weak.clone()),
            matrix: RefCell::new(m),
        })
    }

    /// The matrix.
    #[doc(alias = "getMatrix")]
    #[must_use]
    pub fn matrix(&self) -> T {
        self.matrix.borrow().clone()
    }

    /// Sets the matrix, invalidating the node if it changed (`setMatrix`).
    #[doc(alias = "setMatrix")]
    pub fn set_matrix(&self, m: T)
    where
        T: PartialEq,
    {
        if self.matrix.borrow().clone() == m {
            return;
        }
        *self.matrix.borrow_mut() = m;
        self.invalidate();
    }
}

impl<T: TransformValue + std::fmt::Debug> Node for Matrix<T> {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/include/SkSGTransform.h#L48-L50 (chrome/m156)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &SkMatrix) -> Rect {
        Rect::new_empty()
    }
}

impl<T: TransformValue + std::fmt::Debug> Transform for Matrix<T> {
    fn is44(&self) -> bool {
        T::IS44
    }

    fn as_matrix(&self) -> SkMatrix {
        self.matrix.borrow().clone().to_sk_matrix()
    }

    fn as_m44(&self) -> SkM44 {
        self.matrix.borrow().clone().to_sk_m44()
    }
}

/// The composition `A x B` of two transforms (`Concat<T>`).
// Port of: modules/sksg/src/SkSGTransform.cpp#L35-L66 (chrome/m156) (`Concat`)
#[derive(Debug)]
pub struct Concat<T: TransformValue + std::fmt::Debug> {
    core: NodeCore,
    a: Rc<dyn Transform>,
    b: Rc<dyn Transform>,
    composed: RefCell<T>,
}

impl<T: TransformValue + std::fmt::Debug> Concat<T> {
    fn make(a: &Rc<dyn Transform>, b: &Rc<dyn Transform>) -> Rc<Self> {
        let concat = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(inval_traits::BUBBLE_DAMAGE, weak.clone()),
            a: Rc::clone(a),
            b: Rc::clone(b),
            composed: RefCell::new(T::identity()),
        });
        concat.observe_inval(a.as_ref());
        concat.observe_inval(b.as_ref());
        concat
    }
}

impl<T: TransformValue + std::fmt::Debug> Drop for Concat<T> {
    fn drop(&mut self) {
        self.unobserve_inval(self.a.as_ref());
        self.unobserve_inval(self.b.as_ref());
    }
}

impl<T: TransformValue + std::fmt::Debug> Node for Concat<T> {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L43-L50 (chrome/m156) (`Concat::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &SkMatrix) -> Rect {
        self.a.revalidate(ic.as_deref_mut(), ctm);
        self.b.revalidate(ic, ctm);
        let composed = T::concat(
            T::from_transform(self.a.as_ref()),
            T::from_transform(self.b.as_ref()),
        );
        *self.composed.borrow_mut() = composed;
        Rect::new_empty()
    }
}

impl<T: TransformValue + std::fmt::Debug> Transform for Concat<T> {
    fn is44(&self) -> bool {
        T::IS44
    }

    fn as_matrix(&self) -> SkMatrix {
        debug_assert!(!self.core.has_inval());
        self.composed.borrow().clone().to_sk_matrix()
    }

    fn as_m44(&self) -> SkM44 {
        debug_assert!(!self.core.has_inval());
        self.composed.borrow().clone().to_sk_m44()
    }
}

/// The inverse of a transform (`Inverse<T>`). A singular transform inverts to the identity.
// Port of: modules/sksg/src/SkSGTransform.cpp#L68-L96 (chrome/m156) (`Inverse`)
#[derive(Debug)]
pub struct Inverse<T: TransformValue + std::fmt::Debug> {
    core: NodeCore,
    t: Rc<dyn Transform>,
    inverted: RefCell<T>,
}

impl<T: TransformValue + std::fmt::Debug> Inverse<T> {
    fn make(t: &Rc<dyn Transform>) -> Rc<Self> {
        let inverse = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(inval_traits::BUBBLE_DAMAGE, weak.clone()),
            t: Rc::clone(t),
            inverted: RefCell::new(T::identity()),
        });
        inverse.observe_inval(t.as_ref());
        inverse
    }
}

impl<T: TransformValue + std::fmt::Debug> Drop for Inverse<T> {
    fn drop(&mut self) {
        self.unobserve_inval(self.t.as_ref());
    }
}

impl<T: TransformValue + std::fmt::Debug> Node for Inverse<T> {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L77-L84 (chrome/m156) (`Inverse::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &SkMatrix) -> Rect {
        self.t.revalidate(ic, ctm);
        *self.inverted.borrow_mut() = T::from_transform(self.t.as_ref())
            .invert()
            .unwrap_or_else(T::identity);
        Rect::new_empty()
    }
}

impl<T: TransformValue + std::fmt::Debug> Transform for Inverse<T> {
    fn is44(&self) -> bool {
        T::IS44
    }

    fn as_matrix(&self) -> SkMatrix {
        debug_assert!(!self.core.has_inval());
        self.inverted.borrow().clone().to_sk_matrix()
    }

    fn as_m44(&self) -> SkM44 {
        debug_assert!(!self.core.has_inval());
        self.inverted.borrow().clone().to_sk_m44()
    }
}

/// `Transform::MakeConcat`: `A x B`. A missing operand is returned as the other one.
// Port of: modules/sksg/src/SkSGTransform.cpp#L98-L108 (chrome/m156) (`Transform::MakeConcat`)
#[doc(alias = "MakeConcat")]
#[must_use]
pub fn make_concat(
    a: Option<Rc<dyn Transform>>,
    b: Option<Rc<dyn Transform>>,
) -> Option<Rc<dyn Transform>> {
    let (a, b) = match (a, b) {
        (None, b) => return b,
        (a, None) => return a,
        (Some(a), Some(b)) => (a, b),
    };
    if a.is44() || b.is44() {
        Some(Concat::<SkM44>::make(&a, &b))
    } else {
        Some(Concat::<SkMatrix>::make(&a, &b))
    }
}

/// `Transform::MakeInverse`: `Inv(T)`, or `None` for a missing operand.
// Port of: modules/sksg/src/SkSGTransform.cpp#L110-L118 (chrome/m156) (`Transform::MakeInverse`)
#[doc(alias = "MakeInverse")]
#[must_use]
pub fn make_inverse(t: Option<Rc<dyn Transform>>) -> Option<Rc<dyn Transform>> {
    let t = t?;
    if t.is44() {
        Some(Inverse::<SkM44>::make(&t))
    } else {
        Some(Inverse::<SkMatrix>::make(&t))
    }
}

/// Applies a transform to its child (`TransformEffect`).
// Port of: modules/sksg/include/SkSGTransform.h#L60-L84 (chrome/m156) (`class TransformEffect`)
#[doc(alias = "sksg::TransformEffect")]
#[derive(Debug)]
pub struct TransformEffect {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    transform: Rc<dyn Transform>,
}

impl TransformEffect {
    /// `TransformEffect::Make(child, transform)`: `None` if either is missing.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Option<Rc<dyn RenderNode>>,
        transform: Option<Rc<dyn Transform>>,
    ) -> Option<Rc<Self>> {
        let (child, transform) = (child?, transform?);
        let effect = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            transform: Rc::clone(&transform),
        });
        // Port of: modules/sksg/src/SkSGTransform.cpp#L152-L157 (chrome/m156) (`TransformEffect::TransformEffect`)
        effect.observe_inval(effect.child.as_ref());
        effect.observe_inval(effect.transform.as_ref());
        Some(effect)
    }

    /// `TransformEffect::Make(child, matrix)`, with a 3x3 matrix.
    #[must_use]
    pub fn make_with_matrix(child: Option<Rc<dyn RenderNode>>, m: SkMatrix) -> Option<Rc<Self>> {
        Self::make(child, Some(Matrix::<SkMatrix>::make(m)))
    }

    /// The transform.
    #[doc(alias = "getTransform")]
    #[must_use]
    pub fn transform(&self) -> Rc<dyn Transform> {
        Rc::clone(&self.transform)
    }
}

impl Drop for TransformEffect {
    // Port of: modules/sksg/src/SkSGTransform.cpp#L159-L161 (chrome/m156) (`TransformEffect::~TransformEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.transform.as_ref());
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for TransformEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L186-L200 (chrome/m156) (`TransformEffect::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &SkMatrix) -> Rect {
        debug_assert!(self.core.has_inval());
        // We don't care about matrix reval results.
        self.transform.revalidate(ic.as_deref_mut(), ctm);
        // TODO: need to update all the reval plumbing for m44.
        let m = self.transform.as_matrix();
        let mut bounds = effect_on_revalidate(&self.child, ic, &SkMatrix::concat(ctm, &m));
        bounds = m.map_rect(bounds).0;
        bounds
    }
}

impl RenderNode for TransformEffect {
    // Port of: modules/sksg/src/SkSGTransform.cpp#L163-L170 (chrome/m156) (`TransformEffect::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let save_count = canvas.save();
        canvas.concat_44(&self.transform.as_m44());
        effect_on_render(&self.child, canvas, ctx);
        canvas.restore_to_count(save_count);
    }

    // Port of: modules/sksg/src/SkSGTransform.cpp#L172-L177 (chrome/m156) (`TransformEffect::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        let p4 = self.transform.as_m44().map(p.x, p.y, 0.0, 0.0);
        effect_on_node_at(&self.child, Point { x: p4.x, y: p4.y })
    }
}

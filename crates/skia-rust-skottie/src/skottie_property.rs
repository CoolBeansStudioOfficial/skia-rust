// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/include/SkottieProperty.h, modules/skottie/src/SkottieProperty.cpp
// (chrome/m156)
//
// Property observers and handles. The text property (`TextPropertyValue`, `TextPropertyHandle`,
// `PropertyObserver::onTextProperty`) belongs to the text layers of M22 and is not here yet.

use std::marker::PhantomData;
use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::point::{Point, Vector};
use skia_rust_sksg::{Color as SgColor, OpacityEffect};

use crate::internal::skottie_priv::SceneGraphRevalidator;
use crate::internal::transform::TransformAdapter2D;

/// The value of a color property.
// Port of: modules/skottie/include/SkottieProperty.h#L45 (chrome/m156) (`ColorPropertyValue`)
pub type ColorPropertyValue = Color;

/// The value of an opacity property: a percentage.
// Port of: modules/skottie/include/SkottieProperty.h#L46 (chrome/m156) (`OpacityPropertyValue`)
pub type OpacityPropertyValue = f32;

/// The value of a transform property.
// Port of: modules/skottie/include/SkottieProperty.h#L121-L132 (chrome/m156) (`TransformPropertyValue`)
#[doc(alias = "skottie::TransformPropertyValue")]
#[derive(Debug, Clone, Copy)]
pub struct TransformPropertyValue {
    /// The anchor point (`fAnchorPoint`).
    pub anchor_point: Point,
    /// The position (`fPosition`).
    pub position: Point,
    /// The scale, in percent (`fScale`).
    pub scale: Vector,
    /// The rotation, in degrees (`fRotation`).
    pub rotation: f32,
    /// The skew (`fSkew`).
    pub skew: f32,
    /// The skew axis (`fSkewAxis`).
    pub skew_axis: f32,
}

impl PartialEq for TransformPropertyValue {
    /// Like Skia's `operator==`, this does not compare the rotation.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L50-L57 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.anchor_point == other.anchor_point
            && self.position == other.position
            && self.scale == other.scale
            && self.skew == other.skew
            && self.skew_axis == other.skew_axis
    }
}

/// A handle that gets and sets the value of a property of the scene graph. Setting a value
/// revalidates the scene graph (when the handle came from an animation).
// Port of: modules/skottie/include/SkottieProperty.h#L134-L152 (chrome/m156) (`class PropertyHandle`)
#[doc(alias = "skottie::PropertyHandle")]
pub struct PropertyHandle<V, N> {
    node: Rc<N>,
    revalidator: Option<Rc<SceneGraphRevalidator>>,
    value: PhantomData<V>,
}

impl<V, N> Clone for PropertyHandle<V, N> {
    fn clone(&self) -> Self {
        Self {
            node: Rc::clone(&self.node),
            revalidator: self.revalidator.clone(),
            value: PhantomData,
        }
    }
}

impl<V, N> std::fmt::Debug for PropertyHandle<V, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyHandle").finish_non_exhaustive()
    }
}

impl<V, N> PropertyHandle<V, N> {
    /// A handle that does not revalidate the scene graph on set.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L60-L61 (chrome/m156) (`PropertyHandle(sk_sp<NodeT>)`)
    #[must_use]
    pub fn new(node: Rc<N>) -> Self {
        Self {
            node,
            revalidator: None,
            value: PhantomData,
        }
    }

    /// A handle that revalidates the scene graph on set.
    // Port of: modules/skottie/include/SkottieProperty.h#L141-L144 (chrome/m156)
    #[must_use]
    pub(crate) fn with_revalidator(node: Rc<N>, revalidator: Rc<SceneGraphRevalidator>) -> Self {
        Self {
            node,
            revalidator: Some(revalidator),
            value: PhantomData,
        }
    }

    fn revalidate(&self) {
        if let Some(revalidator) = &self.revalidator {
            revalidator.revalidate();
        }
    }
}

/// A handle to a color property.
// Port of: modules/skottie/include/SkottieProperty.h#L165-L166 (chrome/m156) (`ColorPropertyHandle`)
pub type ColorPropertyHandle = PropertyHandle<ColorPropertyValue, SgColor>;
/// A handle to an opacity property.
// Port of: modules/skottie/include/SkottieProperty.h#L167-L168 (chrome/m156) (`OpacityPropertyHandle`)
pub type OpacityPropertyHandle = PropertyHandle<OpacityPropertyValue, OpacityEffect>;
/// A handle to a transform property.
// Port of: modules/skottie/include/SkottieProperty.h#L171-L172 (chrome/m156) (`TransformPropertyHandle`)
pub type TransformPropertyHandle = PropertyHandle<TransformPropertyValue, TransformAdapter2D>;

impl ColorPropertyHandle {
    /// The color.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L79-L82 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> ColorPropertyValue {
        self.node.color()
    }

    /// Sets the color.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L84-L90 (chrome/m156)
    pub fn set(&self, c: ColorPropertyValue) {
        self.node.set_color(c);
        self.revalidate();
    }
}

impl OpacityPropertyHandle {
    /// The opacity, in percent.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L105-L108 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> OpacityPropertyValue {
        self.node.opacity() * 100.0
    }

    /// Sets the opacity, in percent.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L110-L116 (chrome/m156)
    pub fn set(&self, o: OpacityPropertyValue) {
        self.node.set_opacity(o / 100.0);
        self.revalidate();
    }
}

impl TransformPropertyHandle {
    /// The transform properties.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L151-L160 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> TransformPropertyValue {
        TransformPropertyValue {
            anchor_point: self.node.anchor_point(),
            position: self.node.position(),
            scale: self.node.scale(),
            rotation: self.node.rotation(),
            skew: self.node.skew(),
            skew_axis: self.node.skew_axis(),
        }
    }

    /// Sets the transform properties.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L162-L172 (chrome/m156)
    pub fn set(&self, t: &TransformPropertyValue) {
        self.node.set_anchor_point(t.anchor_point);
        self.node.set_position(t.position);
        self.node.set_scale(t.scale);
        self.node.set_rotation(t.rotation);
        self.node.set_skew(t.skew);
        self.node.set_skew_axis(t.skew_axis);

        self.revalidate();
    }
}

/// The kinds of nodes a [`PropertyObserver`] is told about.
// Port of: modules/skottie/include/SkottieProperty.h#L174-L175 (chrome/m156) (`PropertyObserver::NodeType`)
#[doc(alias = "skottie::PropertyObserver::NodeType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// A composition.
    Composition,
    /// A layer.
    Layer,
    /// An effect.
    Effect,
    /// Anything else.
    Other,
}

/// A lazily created property handle: the observer calls it if it wants the handle
/// (`LazyHandle`). Calling it marks the property as dispatched.
// Port of: modules/skottie/include/SkottieProperty.h#L177-L178 (chrome/m156) (`PropertyObserver::LazyHandle`)
pub type LazyHandle<'a, T> = &'a dyn Fn() -> Box<T>;

/// Receives callbacks during animation parsing. The default implementations ignore everything.
/// Node names are `None` for unnamed nodes.
// Port of: modules/skottie/include/SkottieProperty.h#L174-L205 (chrome/m156) (`class PropertyObserver`)
#[doc(alias = "skottie::PropertyObserver")]
pub trait PropertyObserver {
    /// A color property (`onColorProperty`).
    #[doc(alias = "onColorProperty")]
    fn on_color_property(&self, _node_name: Option<&str>, _handle: LazyHandle<'_, ColorPropertyHandle>) {}

    /// An opacity property (`onOpacityProperty`).
    #[doc(alias = "onOpacityProperty")]
    fn on_opacity_property(
        &self,
        _node_name: Option<&str>,
        _handle: LazyHandle<'_, OpacityPropertyHandle>,
    ) {
    }

    /// A transform property (`onTransformProperty`).
    #[doc(alias = "onTransformProperty")]
    fn on_transform_property(
        &self,
        _node_name: Option<&str>,
        _handle: LazyHandle<'_, TransformPropertyHandle>,
    ) {
    }

    /// Entering a node (`onEnterNode`).
    #[doc(alias = "onEnterNode")]
    fn on_enter_node(&self, _node_name: Option<&str>, _node_type: NodeType) {}

    /// Leaving a node (`onLeavingNode`).
    #[doc(alias = "onLeavingNode")]
    fn on_leaving_node(&self, _node_name: Option<&str>, _node_type: NodeType) {}
}

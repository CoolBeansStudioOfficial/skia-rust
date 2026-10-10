// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/Transform.h, modules/skottie/src/Transform.cpp
// (chrome/m156)

use std::rc::{Rc, Weak};

use skia_rust_core::floating_point::float_degrees_to_radians;
use skia_rust_core::m44::{M44, V2, V3};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar_tan;
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::Transform;
use skia_rust_sksg::transform::{Matrix as SgMatrix, make_concat};

use crate::impl_container_animator;
use crate::json::ObjectValue;
use crate::skottie_value::VectorValue;

use super::animator::{
    AnimatablePropertyContainer, Animator, DiscardableAdapterBase, IntoJsonProp, Prop,
    PropertyContainer,
};
use super::camera::CameraState;
use super::skottie_priv::AnimationBuilder;

/// The skew limit of AE, in degrees.
const MAX_SKEW_ANGLE: f32 = 85.0;

/// The 2D transform of a layer or shape group.
// Port of: modules/skottie/src/Transform.h#L29-L68 (chrome/m156) (`class TransformAdapter2D`)
#[doc(alias = "skottie::internal::TransformAdapter2D")]
pub struct TransformAdapter2D {
    base: DiscardableAdapterBase<SgMatrix<Matrix>>,
    anchor_point: Prop<V2>,
    position: Prop<V2>,
    scale: Prop<V2>,
    rotation: Prop<f32>,
    skew: Prop<f32>,
    skew_axis: Prop<f32>,
    /// Additional rotation component controlled by auto-orient.
    orientation: Prop<f32>,
}

impl TransformAdapter2D {
    /// Binds the transform properties (`TransformAdapter2D::TransformAdapter2D`).
    // Port of: modules/skottie/src/Transform.cpp#L22-L40 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ constructor
    pub fn make<'a>(
        abuilder: &AnimationBuilder<'_>,
        janchor_point: impl IntoJsonProp<'a>,
        jposition: impl IntoJsonProp<'a>,
        jscale: impl IntoJsonProp<'a>,
        jrotation: impl IntoJsonProp<'a>,
        jskew: impl IntoJsonProp<'a>,
        jskew_axis: impl IntoJsonProp<'a>,
        auto_orient: bool,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                SgMatrix::<Matrix>::make(Matrix::new_identity()),
            );
            let anchor_point = Prop::new(V2::new(0.0, 0.0));
            let position = Prop::new(V2::new(0.0, 0.0));
            let scale = Prop::new(V2::new(100.0, 100.0));
            let rotation = Prop::new(0.0);
            let skew = Prop::new(0.0);
            let skew_axis = Prop::new(0.0);
            let orientation = Prop::new(0.0);

            let c = base.container();
            c.bind(abuilder, janchor_point, &anchor_point);
            c.bind(abuilder, jscale, &scale);
            c.bind(abuilder, jrotation, &rotation);
            c.bind(abuilder, jskew, &skew);
            c.bind(abuilder, jskew_axis, &skew_axis);

            c.bind_auto_orientable(
                abuilder,
                jposition,
                &position,
                if auto_orient {
                    Some(&orientation)
                } else {
                    None
                },
            );

            Self {
                base,
                anchor_point,
                position,
                scale,
                rotation,
                skew,
                skew_axis,
                orientation,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    /// The node this adapter drives.
    #[must_use]
    pub fn node(&self) -> &Rc<SgMatrix<Matrix>> {
        self.base.node()
    }

    /// The anchor point (`getAnchorPoint`).
    #[must_use]
    pub fn anchor_point(&self) -> Point {
        let v = self.anchor_point.get();
        Point { x: v.x, y: v.y }
    }

    /// Sets the anchor point (`setAnchorPoint`).
    pub fn set_anchor_point(&self, ap: Point) {
        self.anchor_point.set(V2::new(ap.x, ap.y));
        self.on_sync();
    }

    /// The position (`getPosition`).
    #[must_use]
    pub fn position(&self) -> Point {
        let v = self.position.get();
        Point { x: v.x, y: v.y }
    }

    /// Sets the position (`setPosition`).
    pub fn set_position(&self, p: Point) {
        self.position.set(V2::new(p.x, p.y));
        self.on_sync();
    }

    /// The scale, in percent (`getScale`).
    #[must_use]
    pub fn scale(&self) -> Point {
        let v = self.scale.get();
        Point { x: v.x, y: v.y }
    }

    /// Sets the scale (`setScale`).
    pub fn set_scale(&self, s: Point) {
        self.scale.set(V2::new(s.x, s.y));
        self.on_sync();
    }

    /// The rotation, in degrees (`getRotation`).
    #[must_use]
    pub fn rotation(&self) -> f32 {
        self.rotation.get()
    }

    /// Sets the rotation (`setRotation`).
    pub fn set_rotation(&self, r: f32) {
        self.rotation.set(r);
        self.on_sync();
    }

    /// The skew (`getSkew`).
    #[must_use]
    pub fn skew(&self) -> f32 {
        self.skew.get()
    }

    /// Sets the skew (`setSkew`).
    pub fn set_skew(&self, sk: f32) {
        self.skew.set(sk);
        self.on_sync();
    }

    /// The skew axis (`getSkewAxis`).
    #[must_use]
    pub fn skew_axis(&self) -> f32 {
        self.skew_axis.get()
    }

    /// Sets the skew axis (`setSkewAxis`).
    pub fn set_skew_axis(&self, sa: f32) {
        self.skew_axis.set(sa);
        self.on_sync();
    }

    /// The total matrix of the transform (`totalMatrix`).
    // Port of: modules/skottie/src/Transform.cpp#L46-L68 (chrome/m156)
    #[must_use]
    pub fn total_matrix(&self) -> Matrix {
        let skew_matrix = |mut sk: f32, mut sa: f32| {
            if sk == 0.0 {
                return Matrix::new_identity();
            }

            // AE control limit.
            sk = -float_degrees_to_radians(t_pin(sk, -MAX_SKEW_ANGLE, MAX_SKEW_ANGLE));
            sa = float_degrees_to_radians(sa);

            // Similar to CSS/SVG SkewX [1] with an explicit rotation.
            // [1] https://www.w3.org/TR/css-transforms-1/#SkewXDefined
            // skia-rust: libm (std::tan)
            &(&Matrix::rotate_rad(sa) * &Matrix::skew((scalar_tan(sk), 0.0)))
                * &Matrix::rotate_rad(-sa)
        };

        let position = self.position.get();
        let scale = self.scale.get();
        let anchor_point = self.anchor_point.get();

        let m = &Matrix::translate((position.x, position.y))
            * &Matrix::rotate_deg(self.rotation.get() + self.orientation.get());
        let m = &m * &skew_matrix(self.skew.get(), self.skew_axis.get());
        let m = &m * &Matrix::scale((scale.x / 100.0, scale.y / 100.0)); // 100% based
        &m * &Matrix::translate((-anchor_point.x, -anchor_point.y))
    }
}

impl AnimatablePropertyContainer for TransformAdapter2D {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/Transform.cpp#L42-L44 (chrome/m156) (`onSync`)
    fn on_sync(&self) {
        self.node().set_matrix(self.total_matrix());
    }
}

impl_container_animator!(TransformAdapter2D);

/// The 3D transform of a layer, or the camera.
// Port of: modules/skottie/src/Transform.h#L70-L96 (chrome/m156) (`class TransformAdapter3D`)
#[doc(alias = "skottie::internal::TransformAdapter3D")]
pub struct TransformAdapter3D {
    base: DiscardableAdapterBase<SgMatrix<M44>>,
    anchor_point: Prop<VectorValue>,
    position: Prop<VectorValue>,
    orientation: Prop<VectorValue>,
    scale: Prop<VectorValue>,
    rx: Prop<f32>,
    ry: Prop<f32>,
    rz: Prop<f32>,
    /// The camera state, when this adapter is a `CameraAdaper`.
    camera: Option<CameraState>,
}

impl TransformAdapter3D {
    /// Binds the transform properties (`TransformAdapter3D::TransformAdapter3D`); `camera` makes
    /// the adapter a camera: `bind_camera` is called after the transform properties.
    // Port of: modules/skottie/src/Transform.cpp#L128-L142 (chrome/m156)
    pub(crate) fn make_with(
        jtransform: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        make_camera: impl FnOnce(&PropertyContainer) -> Option<CameraState>,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base =
                DiscardableAdapterBase::new(weak.clone(), SgMatrix::<M44>::make(M44::default()));
            let anchor_point = Prop::new(VectorValue::new());
            let position = Prop::new(VectorValue::new());
            let orientation = Prop::new(VectorValue::new());
            let scale = Prop::new(VectorValue::from_slice(&[100.0, 100.0, 100.0]));
            let rx = Prop::new(0.0);
            let ry = Prop::new(0.0);
            let rz = Prop::new(0.0);

            let c = base.container();
            c.bind(abuilder, jtransform.get("a"), &anchor_point);
            c.bind(abuilder, jtransform.get("p"), &position);
            c.bind(abuilder, jtransform.get("s"), &scale);

            // Axis-wise rotation and orientation are mapped to the same rotation property (3D
            // rotation). The difference is in how they get interpolated (scalar/decomposed vs.
            // vector).
            c.bind(abuilder, jtransform.get("rx"), &rx);
            c.bind(abuilder, jtransform.get("ry"), &ry);
            c.bind(abuilder, jtransform.get("rz"), &rz);
            c.bind(abuilder, jtransform.get("or"), &orientation);

            let camera = make_camera(c);

            Self {
                base,
                anchor_point,
                position,
                orientation,
                scale,
                rx,
                ry,
                rz,
                camera,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    /// Binds a plain 3D transform.
    #[must_use]
    pub fn make(jtransform: &ObjectValue, abuilder: &AnimationBuilder<'_>) -> Rc<Self> {
        Self::make_with(jtransform, abuilder, |_| None)
    }

    /// The node this adapter drives.
    #[must_use]
    pub fn node(&self) -> &Rc<SgMatrix<M44>> {
        self.base.node()
    }

    /// The anchor point (`anchor_point`).
    #[must_use]
    pub fn anchor_point(&self) -> V3 {
        self.anchor_point.borrow().to_v3()
    }

    /// The position (`position`).
    #[must_use]
    pub fn position(&self) -> V3 {
        self.position.borrow().to_v3()
    }

    /// The rotation (`rotation`): orientation and axis-wise rotation map onto the same property.
    // Port of: modules/skottie/src/Transform.cpp#L154-L157 (chrome/m156)
    #[must_use]
    pub fn rotation(&self) -> V3 {
        self.orientation.borrow().to_v3()
            + V3 {
                x: self.rx.get(),
                y: self.ry.get(),
                z: self.rz.get(),
            }
    }

    /// The total matrix of the transform (`totalMatrix`, virtual: cameras override it).
    // Port of: modules/skottie/src/Transform.cpp#L159-L172 (chrome/m156)
    #[must_use]
    pub fn total_matrix(&self) -> M44 {
        if let Some(camera) = &self.camera {
            return camera.total_matrix(self);
        }

        let anchor_point = self.anchor_point();
        let position = self.position();
        let scale = self.scale.borrow().to_v3();
        let rotation = self.rotation();

        let m = &M44::translate(position.x, position.y, position.z)
            * &M44::rotate(
                V3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                float_degrees_to_radians(rotation.x),
            );
        let m = &m
            * &M44::rotate(
                V3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                float_degrees_to_radians(rotation.y),
            );
        let m = &m
            * &M44::rotate(
                V3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                float_degrees_to_radians(rotation.z),
            );
        let m = &m * &M44::scale(scale.x / 100.0, scale.y / 100.0, scale.z / 100.0);
        &m * &M44::translate(-anchor_point.x, -anchor_point.y, -anchor_point.z)
    }
}

impl AnimatablePropertyContainer for TransformAdapter3D {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/Transform.cpp#L146-L148 (chrome/m156) (`onSync`)
    fn on_sync(&self) {
        self.node().set_matrix(self.total_matrix());
    }
}

impl_container_animator!(TransformAdapter3D);

impl AnimationBuilder<'_> {
    /// Attaches the 2D transform of `jtransform` on top of `parent`.
    // Port of: modules/skottie/src/Transform.cpp#L70-L106 (chrome/m156) (`attachMatrix2D`)
    #[doc(alias = "attachMatrix2D")]
    #[must_use]
    pub fn attach_matrix_2d(
        &self,
        jtransform: &ObjectValue,
        parent: Option<Rc<dyn Transform>>,
        auto_orient: bool,
    ) -> Option<Rc<dyn Transform>> {
        let mut jrotation = jtransform.get("r");
        if matches!(jrotation, crate::json::Value::Null(_)) {
            // Some 2D rotations are disguised as 3D...
            jrotation = jtransform.get("rz");
        }

        let adapter = TransformAdapter2D::make(
            self,
            jtransform.get("a"),
            jtransform.get("p"),
            jtransform.get("s"),
            jrotation,
            jtransform.get("sk"),
            jtransform.get("sa"),
            auto_orient,
        );

        let dispatched = self.dispatch_transform_property(&adapter);

        if adapter.is_static() {
            if !dispatched && adapter.total_matrix().is_identity() {
                // The transform has no observable effects - we can discard.
                return parent;
            }
            adapter.seek(0.0);
        } else {
            self.push_animator(adapter.clone());
        }

        make_concat(parent, Some(Rc::clone(adapter.node()) as Rc<dyn Transform>))
    }

    /// Attaches the 3D transform of `jtransform` on top of `parent`.
    // Port of: modules/skottie/src/Transform.cpp#L174-L196 (chrome/m156) (`attachMatrix3D`)
    #[doc(alias = "attachMatrix3D")]
    #[must_use]
    pub fn attach_matrix_3d(
        &self,
        jtransform: &ObjectValue,
        parent: Option<Rc<dyn Transform>>,
        _auto_orient: bool, // TODO: auto_orient
    ) -> Option<Rc<dyn Transform>> {
        let adapter = TransformAdapter3D::make(jtransform, self);

        if adapter.is_static() {
            // TODO: SkM44::isIdentity?
            if adapter.total_matrix() == M44::default() {
                // The transform has no observable effects - we can discard.
                return parent;
            }
            adapter.seek(0.0);
        } else {
            self.push_animator(adapter.clone());
        }

        make_concat(parent, Some(Rc::clone(adapter.node()) as Rc<dyn Transform>))
    }
}

opaque_debug!(TransformAdapter2D, TransformAdapter3D);

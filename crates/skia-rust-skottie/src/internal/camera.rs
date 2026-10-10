// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/Camera.h, modules/skottie/src/Camera.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::floating_point::{float_degrees_to_radians, ieee_float_divide};
use skia_rust_core::m44::{M44, V3};
use skia_rust_core::point::Point;
use skia_rust_core::size::Size;
use skia_rust_sksg::Transform;
use skia_rust_sksg::transform::{Matrix as SgMatrix, make_concat};

use crate::json::{ObjectValue, Value};

use super::animator::{AnimatablePropertyContainer, Animator, Prop};
use super::skottie_priv::AnimationBuilder;
use super::transform::TransformAdapter3D;

/// The camera types of AE.
// Port of: modules/skottie/src/Camera.h#L38-L41 (chrome/m156) (`CameraAdaper::CameraType`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CameraType {
    /// Implicitly facing forward (decreasing z), does not auto-orient.
    OneNode,
    /// Explicitly facing a POI (the anchor point), auto-orients.
    TwoNode,
}

// Port of: modules/skottie/src/Camera.cpp#L22-L58 (chrome/m156) (`ComputeCameraMatrix`)
fn compute_camera_matrix(
    position: V3,
    poi: V3,
    rotation: V3,
    viewport_size: Size,
    zoom: f32,
) -> M44 {
    // Initial camera vector.
    let cam_t = &M44::rotate(
        V3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        float_degrees_to_radians(-rotation.z),
    ) * &M44::rotate(
        V3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        float_degrees_to_radians(rotation.y),
    );
    let cam_t = &cam_t
        * &M44::rotate(
            V3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            float_degrees_to_radians(rotation.x),
        );
    let cam_t = &cam_t
        * &M44::look_at(
            &V3 {
                x: position.x,
                y: position.y,
                z: -position.z,
            },
            &V3 {
                x: poi.x,
                y: poi.y,
                z: poi.z,
            },
            &V3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        );
    let cam_t = &cam_t * &M44::scale(1.0, 1.0, -1.0);

    // View parameters:
    //
    //   * size     -> composition size (TODO: AE seems to base it on width only?)
    //   * distance -> "zoom" camera attribute
    //
    // std::max(a, b) is `(a < b) ? b : a`.
    let view_size = if viewport_size.width < viewport_size.height {
        viewport_size.height
    } else {
        viewport_size.width
    };
    let view_distance = zoom;
    // skia-rust: libm (std::atan)
    let view_angle = ieee_float_divide(view_size * 0.5, view_distance).atan();

    let persp_t = &M44::scale(view_size * 0.5, view_size * 0.5, 1.0)
        * &M44::perspective(0.0, view_distance, 2.0 * view_angle);

    let m = &M44::translate(viewport_size.width * 0.5, viewport_size.height * 0.5, 0.0) * &persp_t;
    &m * &cam_t
}

/// The camera-specific state of a 3D transform adapter (`CameraAdaper`).
// Port of: modules/skottie/src/Camera.h#L28-L56 (chrome/m156) (`class CameraAdaper`)
#[doc(alias = "skottie::internal::CameraAdaper")]
pub(crate) struct CameraState {
    viewport_size: Size,
    camera_type: CameraType,
    zoom: Prop<f32>,
}

impl CameraState {
    /// Binds the camera properties of `jlayer` (`CameraAdaper::CameraAdaper`).
    // Port of: modules/skottie/src/Camera.cpp#L60-L73 (chrome/m156)
    pub(crate) fn new(
        jlayer: &ObjectValue,
        jtransform: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        viewport_size: Size,
        container: &super::animator::PropertyContainer,
    ) -> Self {
        let zoom = Prop::new(0.0);
        // 'pe' (perspective?) corresponds to AE's "zoom" camera property.
        container.bind(abuilder, jlayer.get("pe"), &zoom);

        Self {
            viewport_size,
            // The presence of an anchor point property ('a') differentiates
            // one-node vs. two-node cameras.
            camera_type: if matches!(jtransform.get("a"), Value::Null(_)) {
                CameraType::OneNode
            } else {
                CameraType::TwoNode
            },
            zoom,
        }
    }

    /// The camera matrix (`CameraAdaper::totalMatrix`).
    // Port of: modules/skottie/src/Camera.cpp#L75-L90 (chrome/m156)
    pub(crate) fn total_matrix(&self, adapter: &TransformAdapter3D) -> M44 {
        // Camera parameters:
        //
        //   * location          -> position attribute
        //   * point of interest -> anchor point attribute (two-node camera only)
        //   * orientation       -> rotation attribute
        //
        let position = adapter.position();

        compute_camera_matrix(
            position,
            self.poi(adapter, position),
            adapter.rotation(),
            self.viewport_size,
            self.zoom.get(),
        )
    }

    // Port of: modules/skottie/src/Camera.cpp#L92-L109 (chrome/m156) (`CameraAdaper::poi`)
    fn poi(&self, adapter: &TransformAdapter3D, pos: V3) -> V3 {
        // AE supports two camera types:
        //
        //   - one-node camera: does not auto-orient, and starts off perpendicular
        //     to the z = 0 plane, facing "forward" (decreasing z).
        //
        //   - two-node camera: has a point of interest (encoded as the anchor point),
        //     and auto-orients to point in its direction.

        if self.camera_type == CameraType::OneNode {
            return V3 {
                x: pos.x,
                y: pos.y,
                z: -pos.z - 1.0,
            };
        }

        let ap = adapter.anchor_point();

        V3 {
            x: ap.x,
            y: ap.y,
            z: -ap.z,
        }
    }
}

/// The transform of the camera used in the absence of an explicit camera layer
/// (`CameraAdaper::DefaultCameraTransform`).
// Port of: modules/skottie/src/Camera.cpp#L111-L125 (chrome/m156)
#[must_use]
pub fn default_camera_transform(viewport_size: Size) -> Rc<dyn Transform> {
    const DEFAULT_AE_ZOOM: f32 = 879.13;

    let center = Point {
        x: viewport_size.width * 0.5,
        y: viewport_size.height * 0.5,
    };

    let pos = V3 {
        x: center.x,
        y: center.y,
        z: -DEFAULT_AE_ZOOM,
    };
    let poi = V3 {
        x: pos.x,
        y: pos.y,
        z: -pos.z - 1.0,
    };
    let rot = V3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    SgMatrix::<M44>::make(compute_camera_matrix(
        pos,
        poi,
        rot,
        viewport_size,
        DEFAULT_AE_ZOOM,
    ))
}

impl AnimationBuilder<'_> {
    /// Attaches the camera of the layer `jlayer`: its transform precomposes with `parent`.
    // Port of: modules/skottie/src/Camera.cpp#L127-L139 (chrome/m156) (`attachCamera`)
    #[doc(alias = "attachCamera")]
    #[must_use]
    pub fn attach_camera(
        &self,
        jlayer: &ObjectValue,
        jtransform: &ObjectValue,
        parent: Option<Rc<dyn Transform>>,
        viewport_size: Size,
    ) -> Option<Rc<dyn Transform>> {
        let adapter = TransformAdapter3D::make_with(jtransform, self, |container| {
            Some(CameraState::new(
                jlayer,
                jtransform,
                self,
                viewport_size,
                container,
            ))
        });

        if adapter.is_static() {
            adapter.seek(0.0);
        } else {
            self.push_animator(adapter.clone());
        }

        make_concat(Some(Rc::clone(adapter.node()) as Rc<dyn Transform>), parent)
    }
}

// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/SkottieValue.h, and the conversions defined in
// modules/skottie/src/animator/VectorKeyframeAnimator.cpp and ShapeKeyframeAnimator.cpp
// (chrome/m156)

use std::ops::{Deref, DerefMut};

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::m44::{V2, V3};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::t_pin::t_pin;

/// A scalar property value.
// Port of: modules/skottie/src/SkottieValue.h#L20 (chrome/m156) (`ScalarValue`)
pub type ScalarValue = f32;

/// A 2D vector property value.
// Port of: modules/skottie/src/SkottieValue.h#L21 (chrome/m156) (`Vec2Value`)
pub type Vec2Value = V2;

/// A vector property value: an arbitrary number of floats.
// Port of: modules/skottie/src/SkottieValue.h#L23-L32 (chrome/m156) (`VectorValue`)
#[doc(alias = "skottie::VectorValue")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VectorValue(Vec<f32>);

impl VectorValue {
    /// An empty vector.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// The vector of the given floats.
    #[must_use]
    pub fn from_slice(values: &[f32]) -> Self {
        Self(values.to_vec())
    }

    /// Best effort conversion to a 3D vector (`operator SkV3`): missing components are 0.
    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L56-L62 (chrome/m156)
    #[must_use]
    pub fn to_v3(&self) -> V3 {
        V3 {
            x: if !self.is_empty() { self[0] } else { 0.0 },
            y: if self.len() > 1 { self[1] } else { 0.0 },
            z: if self.len() > 2 { self[2] } else { 0.0 },
        }
    }
}

impl Deref for VectorValue {
    type Target = Vec<f32>;

    fn deref(&self) -> &Vec<f32> {
        &self.0
    }
}

impl DerefMut for VectorValue {
    fn deref_mut(&mut self) -> &mut Vec<f32> {
        &mut self.0
    }
}

impl From<Vec<f32>> for VectorValue {
    fn from(values: Vec<f32>) -> Self {
        Self(values)
    }
}

/// A color property value: a vector of up to four floats (r, g, b, a).
// Port of: modules/skottie/src/SkottieValue.h#L34-L45 (chrome/m156) (`ColorValue`)
#[doc(alias = "skottie::ColorValue")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColorValue(VectorValue);

impl ColorValue {
    /// An empty color vector.
    #[must_use]
    pub const fn new() -> Self {
        Self(VectorValue::new())
    }

    /// The color vector of the given floats.
    #[must_use]
    pub fn from_slice(values: &[f32]) -> Self {
        Self(VectorValue::from_slice(values))
    }

    /// Best effort conversion of the vector into a color (`operator SkColor4f`).
    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L68-L77 (chrome/m156)
    #[must_use]
    pub fn to_color4f(&self) -> Color4f {
        let r = if !self.is_empty() {
            t_pin(self[0], 0.0, 1.0)
        } else {
            0.0
        };
        let g = if self.len() > 1 {
            t_pin(self[1], 0.0, 1.0)
        } else {
            0.0
        };
        let b = if self.len() > 2 {
            t_pin(self[2], 0.0, 1.0)
        } else {
            0.0
        };
        let a = if self.len() > 3 {
            t_pin(self[3], 0.0, 1.0)
        } else {
            1.0
        };

        Color4f { r, g, b, a }
    }

    /// The color as a packed color (`operator SkColor`).
    // Port of: modules/skottie/src/animator/VectorKeyframeAnimator.cpp#L64-L66 (chrome/m156)
    #[must_use]
    pub fn to_color(&self) -> Color {
        self.to_color4f().to_color()
    }
}

impl Deref for ColorValue {
    type Target = VectorValue;

    fn deref(&self) -> &VectorValue {
        &self.0
    }
}

impl DerefMut for ColorValue {
    fn deref_mut(&mut self) -> &mut VectorValue {
        &mut self.0
    }
}

// Shapes (paths) are encoded as a vector of floats. For each vertex, we store 6 floats:
//
//   - vertex point      (2 floats)
//   - in-tangent point  (2 floats)
//   - out-tangent point (2 floats)
//
// Additionally, we store one trailing "closed shape" flag - e.g.
//
//  [ v0.x, v0.y, v0_in.x, v0_in.y, v0_out.x, v0_out.y, ... , closed_flag ]
//
// Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L33-L44 (chrome/m156) (`ShapeEncodingInfo`)
pub(crate) const X_INDEX: usize = 0;
pub(crate) const Y_INDEX: usize = 1;
pub(crate) const IN_X_INDEX: usize = 2;
pub(crate) const IN_Y_INDEX: usize = 3;
pub(crate) const OUT_X_INDEX: usize = 4;
pub(crate) const OUT_Y_INDEX: usize = 5;
pub(crate) const FLOATS_PER_VERTEX: usize = 6;

/// A shape property value: a path, encoded as a vector of floats.
// Port of: modules/skottie/src/SkottieValue.h#L47-L52 (chrome/m156) (`ShapeValue`)
#[doc(alias = "skottie::ShapeValue")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShapeValue(Vec<f32>);

impl ShapeValue {
    /// An empty shape.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// The shape as a path (`operator SkPath`).
    // Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L111-L158 (chrome/m156)
    #[must_use]
    pub fn to_path(&self) -> Path {
        let vertex_count = self.len() / FLOATS_PER_VERTEX;

        let mut path = PathBuilder::new();

        if vertex_count != 0 {
            // conservatively assume all cubics
            let count = i32::try_from(vertex_count * 3).unwrap_or(i32::MAX);
            let reserve = count.saturating_add(1);
            path.inc_reserve(reserve, reserve, 0);

            // Move to first vertex.
            path.move_to(Point {
                x: self[X_INDEX],
                y: self[Y_INDEX],
            });
        }

        let add_cubic = |path: &mut PathBuilder, from_vertex: usize, to_vertex: usize| {
            let from_index = FLOATS_PER_VERTEX * from_vertex;
            let to_index = FLOATS_PER_VERTEX * to_vertex;

            let p0 = Point {
                x: self[from_index + X_INDEX],
                y: self[from_index + Y_INDEX],
            };
            let p1 = Point {
                x: self[to_index + X_INDEX],
                y: self[to_index + Y_INDEX],
            };
            let c0 = Point {
                x: self[from_index + OUT_X_INDEX],
                y: self[from_index + OUT_Y_INDEX],
            } + p0;
            let c1 = Point {
                x: self[to_index + IN_X_INDEX],
                y: self[to_index + IN_Y_INDEX],
            } + p1;

            if c0 == p0 && c1 == p1 {
                // If the control points are coincident, we can power-reduce to a straight line.
                // TODO: we could also do that when the controls are on the same line as the
                //       vertices, but it's unclear how common that case is.
                path.line_to(p1);
            } else {
                path.cubic_to(c0, c1, p1);
            }
        };

        for i in 1..vertex_count {
            add_cubic(&mut path, i - 1, i);
        }

        // Close the path with an extra cubic, if needed.
        if vertex_count != 0 && self.last().copied().unwrap_or(0.0) != 0.0 {
            add_cubic(&mut path, vertex_count - 1, 0);
            path.close();
        }

        path.detach()
    }
}

impl Deref for ShapeValue {
    type Target = Vec<f32>;

    fn deref(&self) -> &Vec<f32> {
        &self.0
    }
}

impl DerefMut for ShapeValue {
    fn deref_mut(&mut self) -> &mut Vec<f32> {
        &mut self.0
    }
}

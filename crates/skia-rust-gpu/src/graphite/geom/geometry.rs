// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/Geometry.h (the variants ported so far)

//! [`Geometry`]: what a draw covers, as seen by the `RenderStep`s.
//!
//! Only the variants whose payload types are ported are here: `Empty`, `Shape`, `EdgeAAQuad`,
//! `Vertices` and `Mesh`. The other variants of Skia's `Geometry` (`SubRun`, `CoverageMaskShape`,
//! `AnalyticBlur`, `AnalyticRRectBlur`, and the sparse-strip `WideTiles` and `EndCaps`) wait for
//! their payload types (G2, G7c, G10 and G17). Their `bounds()` cases are not written here, so no
//! draw can carry them yet.

use skia_rust_core::mesh::Mesh;
use skia_rust_core::vertices::Vertices;

use skia_rust_core::m44::M44;

use crate::graphite::geom::coverage_mask_shape::CoverageMaskShape;
use crate::graphite::geom::edge_aa_quad::EdgeAAQuad;
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;

/// The geometry of a draw (`skgpu::graphite::Geometry`), restricted to the ported variants.
// Port of: src/gpu/graphite/geom/Geometry.h#L26-L106 (chrome/m156), the ported variants
#[doc(alias = "skgpu::graphite::Geometry")]
#[derive(Clone, Debug, Default)]
pub enum Geometry {
    /// `Type::kEmpty`.
    #[default]
    Empty,
    /// `Type::kShape`.
    Shape(Shape),
    /// `Type::kEdgeAAQuad`.
    EdgeAAQuad(EdgeAAQuad),
    /// `Type::kVertices`.
    Vertices(Vertices),
    /// `Type::kMesh`.
    Mesh(Mesh),
    /// `Type::kCoverageMaskShape`.
    CoverageMaskShape(CoverageMaskShape),
}

impl Geometry {
    /// `isShape()`.
    // Port of: src/gpu/graphite/geom/Geometry.h#L97 (chrome/m156)
    #[must_use]
    pub const fn is_shape(&self) -> bool {
        matches!(self, Self::Shape(_))
    }

    /// `isVertices()`.
    // Port of: src/gpu/graphite/geom/Geometry.h#L100 (chrome/m156)
    #[must_use]
    pub const fn is_vertices(&self) -> bool {
        matches!(self, Self::Vertices(_))
    }

    /// `vertices()`. Skia asserts that the type is `kVertices`.
    ///
    /// # Panics
    /// If the geometry is not a vertices draw.
    // Port of: src/gpu/graphite/geom/Geometry.h#L119 (chrome/m156)
    #[must_use]
    pub fn vertices(&self) -> &Vertices {
        match self {
            Self::Vertices(vertices) => vertices,
            _ => panic!("Geometry::vertices() called on a non-vertices geometry"),
        }
    }

    /// `isMesh()`.
    // Port of: src/gpu/graphite/geom/Geometry.h#L164 (chrome/m156)
    #[must_use]
    pub const fn is_mesh(&self) -> bool {
        matches!(self, Self::Mesh(_))
    }

    /// `mesh()`. Skia asserts that the type is `kMesh`.
    ///
    /// # Panics
    /// If the geometry is not a mesh draw.
    // Port of: src/gpu/graphite/geom/Geometry.h#L207 (chrome/m156)
    #[must_use]
    pub fn mesh(&self) -> &Mesh {
        match self {
            Self::Mesh(mesh) => mesh,
            _ => panic!("Geometry::mesh() called on a non-mesh geometry"),
        }
    }

    /// `isEdgeAAQuad()`.
    // Port of: src/gpu/graphite/geom/Geometry.h#L103 (chrome/m156)
    #[must_use]
    pub const fn is_edge_aa_quad(&self) -> bool {
        matches!(self, Self::EdgeAAQuad(_))
    }

    /// `isEmpty()`: `kEmpty`, or a non-inverted shape that is empty.
    // Port of: src/gpu/graphite/geom/Geometry.h#L108-L113 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Shape(shape) => shape.is_empty() && !shape.inverted(),
            Self::EdgeAAQuad(_) | Self::Vertices(_) | Self::Mesh(_) => false,
            Self::CoverageMaskShape(_) => false,
        }
    }

    /// `shape()`. Skia asserts that the type is `kShape`.
    ///
    /// # Panics
    /// If the geometry is not a shape.
    // Port of: src/gpu/graphite/geom/Geometry.h#L115-L117 (chrome/m156)
    #[must_use]
    pub fn shape(&self) -> &Shape {
        match self {
            Self::Shape(shape) => shape,
            _ => panic!("Geometry::shape() called on a non-shape geometry"),
        }
    }

    /// `edgeAAQuad()`. Skia asserts that the type is `kEdgeAAQuad`.
    ///
    /// # Panics
    /// If the geometry is not an edge-AA quad.
    // Port of: src/gpu/graphite/geom/Geometry.h#L121 (chrome/m156)
    #[must_use]
    pub fn edge_aa_quad(&self) -> &EdgeAAQuad {
        match self {
            Self::EdgeAAQuad(quad) => quad,
            _ => panic!("Geometry::edge_aa_quad() called on a non-quad geometry"),
        }
    }

    /// `isCoverageMaskShape()`.
    // Port of: src/gpu/graphite/geom/Geometry.h#L167 (chrome/m156)
    #[must_use]
    pub const fn is_coverage_mask_shape(&self) -> bool {
        matches!(self, Self::CoverageMaskShape(_))
    }

    /// `coverageMaskShape()`. Skia asserts that the type is `kCoverageMaskShape`.
    ///
    /// # Panics
    /// If the geometry is not a coverage mask shape.
    // Port of: src/gpu/graphite/geom/Geometry.h#L185-L187 (chrome/m156)
    #[must_use]
    pub fn coverage_mask_shape(&self) -> &CoverageMaskShape {
        match self {
            Self::CoverageMaskShape(mask) => mask,
            _ => panic!("Geometry::coverage_mask_shape() called on a non-mask geometry"),
        }
    }

    /// `maskToDevice()`: the transform from the mask space to device space, for the geometry
    /// types that have a mask space (coverage masks).
    // Port of: src/gpu/graphite/geom/Geometry.h#L345-L349 (chrome/m156)
    #[must_use]
    pub fn mask_to_device(&self) -> Option<&M44> {
        match self {
            Self::CoverageMaskShape(mask) => Some(mask.mask_to_device()),
            _ => None,
        }
    }

    /// `setCoverageMaskShape(maskShape)`: the geometry becomes the coverage mask.
    // Port of: src/gpu/graphite/geom/Geometry.h#L256-L262 (chrome/m156)
    pub fn set_coverage_mask_shape(&mut self, mask: CoverageMaskShape) {
        *self = Self::CoverageMaskShape(mask);
    }

    /// `bounds()` for the ported variants.
    // Port of: src/gpu/graphite/geom/Geometry.h#L318-L335 (chrome/m156), the ported cases
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Empty => Rect::new(0.0, 0.0, 0.0, 0.0),
            Self::Shape(shape) => shape.bounds(),
            Self::EdgeAAQuad(quad) => quad.bounds(),
            Self::Vertices(vertices) => Rect::from_sk_rect(vertices.bounds()),
            Self::Mesh(mesh) => Rect::from_sk_rect(&mesh.bounds()),
            Self::CoverageMaskShape(mask) => mask.bounds(),
        }
    }
}

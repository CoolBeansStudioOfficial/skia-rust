// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkImageFilterTypes.h, src/core/SkImageFilterTypes.cpp (`Mapping`)

//! The part of `skif` (the image filter types) that `SkCanvas` needs to size and position a
//! layer: [`Mapping`], which splits the canvas's total matrix into a layer-to-device part and a
//! parameter-to-layer part.
//!
//! skia-rust: `LayerSpace`/`ParameterSpace`/`DeviceSpace` are plain `Rect`/`IRect` here. Image
//! filters (`FilterResult`, `Context`, `Backend`, ...) are Phase 3; a layer without filters always
//! asks for [`MatrixCapability::Complex`], where the layer space is the device space and the
//! layer matrix is the whole CTM. The `Translate`/`ScaleRotateTranslate` capabilities
//! (`decompose_transform`) are only requested by image filters and are not ported yet:
//! [`Mapping::decompose_ctm`] fails for the cases that need them.

use crate::floating_point::double_saturate2int;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::matrix_priv::is_scale_translate_as_m33;
use crate::point::Point;
use crate::rect::{IRect, Rect, RoundOut};

/// `kRoundEpsilon`: tolerates a near-integer CTM when rounding layer bounds.
// Port of: src/core/SkImageFilterTypes.cpp#L54 (chrome/m156)
pub const ROUND_EPSILON: f32 = 1e-3;

/// What kind of CTM an image filter can handle (`skif::MatrixCapability`).
// Port of: src/core/SkImageFilterTypes.h#L128-L141 (chrome/m156)
#[doc(alias = "skif::MatrixCapability")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum MatrixCapability {
    /// `kTranslate`.
    Translate,
    /// `kScaleTranslate`.
    ScaleTranslate,
    /// `kComplex`.
    Complex,
}

/// `skif::RoundOut`: rounds out, ignoring `kRoundEpsilon` of slop.
// Port of: src/core/SkImageFilterTypes.cpp#L186 (chrome/m156)
#[doc(alias = "RoundOut")]
#[must_use]
pub fn round_out(r: &Rect) -> IRect {
    r.with_inset((ROUND_EPSILON, ROUND_EPSILON)).round_out()
}

/// The decomposition of a total matrix into the transforms between parameter, layer and device
/// space (`skif::Mapping`).
// Port of: src/core/SkImageFilterTypes.h#L564-L663 (chrome/m156)
#[doc(alias = "skif::Mapping")]
#[derive(Clone, Debug, Default)]
pub struct Mapping {
    layer_to_dev: M44,
    param_to_layer: M44,
    dev_to_layer: M44,
}

impl Mapping {
    /// An identity mapping (`Mapping()`).
    #[must_use]
    pub fn new() -> Mapping {
        Mapping::default()
    }

    /// Equates device and layer space (`Mapping(paramToLayer)`).
    #[must_use]
    pub fn from_layer_matrix(param_to_layer: &M44) -> Mapping {
        Mapping {
            layer_to_dev: M44::new_identity(),
            param_to_layer: *param_to_layer,
            dev_to_layer: M44::new_identity(),
        }
    }

    /// An explicit decomposition; `dev_to_layer` is the inverse of `layer_to_dev`.
    #[must_use]
    pub fn from_parts(layer_to_dev: &M44, dev_to_layer: &M44, param_to_layer: &M44) -> Mapping {
        Mapping {
            layer_to_dev: *layer_to_dev,
            param_to_layer: *param_to_layer,
            dev_to_layer: *dev_to_layer,
        }
    }

    /// Sets this mapping to the default decomposition of the canvas's total transform, given what
    /// the filter can handle. False if the decomposition failed or would produce an invalid device
    /// matrix. Assumes `ctm` is invertible.
    // Port of: src/core/SkImageFilterTypes.cpp#L259-L298 (chrome/m156)
    #[doc(alias = "decomposeCTM")]
    #[must_use]
    pub fn decompose_ctm(
        &mut self,
        ctm: &M44,
        capability: MatrixCapability,
        _representative_pt: Point,
    ) -> bool {
        let remainder;
        let layer;
        if capability == MatrixCapability::Translate {
            // Apply the entire CTM post-filtering
            remainder = *ctm;
            layer = M44::new_identity();
        } else if is_scale_translate_as_m33(ctm) || capability == MatrixCapability::Complex {
            // Either layer space can be anything (kComplex) - or - it can be scale+translate, and
            // the ctm is. In both cases, the layer space can be equivalent to device space.
            remainder = M44::new_identity();
            layer = *ctm;
        } else {
            // This case implies some amount of sampling post-filtering, either due to skew or
            // rotation in the original matrix. TODO(Phase 3, image filters): `decompose_transform`.
            return false;
        }

        let Some(inv_remainder) = remainder.invert() else {
            // Under floating point arithmetic, it's possible to decompose an invertible matrix
            // into a scaling matrix and a remainder and have the remainder be non-invertible.
            // Generally when this happens the scale factors are so large and the matrix so
            // ill-conditioned that it's unlikely that any drawing would be reasonable, so failing
            // to make a layer is okay.
            return false;
        };
        self.param_to_layer = layer;
        self.layer_to_dev = remainder;
        self.dev_to_layer = inv_remainder;
        true
    }

    /// Changes the layer space so that neither the parameter nor the device space changes
    /// (`adjustLayerSpace`). False (and no change) if `layer` cannot be inverted.
    // Port of: src/core/SkImageFilterTypes.cpp#L309-L320 (chrome/m156)
    #[doc(alias = "adjustLayerSpace")]
    #[must_use]
    pub fn adjust_layer_space(&mut self, layer: &M44) -> bool {
        let Some(inv_layer) = layer.invert() else {
            return false;
        };
        self.param_to_layer.post_concat(layer);
        self.dev_to_layer.post_concat(layer);
        self.layer_to_dev.pre_concat(&inv_layer);
        true
    }

    /// The layer-to-device transform (`layerToDevice`).
    #[doc(alias = "layerToDevice")]
    #[must_use]
    pub fn layer_to_device(&self) -> &M44 {
        &self.layer_to_dev
    }

    /// The device-to-layer transform (`deviceToLayer`).
    #[doc(alias = "deviceToLayer")]
    #[must_use]
    pub fn device_to_layer_matrix(&self) -> &M44 {
        &self.dev_to_layer
    }

    /// The parameter-to-layer transform (`layerMatrix`).
    #[doc(alias = "layerMatrix")]
    #[must_use]
    pub fn layer_matrix(&self) -> &M44 {
        &self.param_to_layer
    }

    /// `Mapping::map<SkRect>`.
    // Port of: src/core/SkImageFilterTypes.cpp#L322-L325 (chrome/m156)
    #[must_use]
    pub fn map_rect(geom: &Rect, matrix: &Matrix) -> Rect {
        if geom.is_empty() {
            Rect::new_empty()
        } else {
            matrix.map_rect(geom).0
        }
    }

    /// `Mapping::map<SkIRect>`.
    // Port of: src/core/SkImageFilterTypes.cpp#L327-L349 (chrome/m156)
    #[must_use]
    pub fn map_irect(geom: &IRect, matrix: &Matrix) -> IRect {
        if geom.is_empty() {
            return IRect::new_empty();
        }
        // Unfortunately, there is a range of integer values such that we have 1px precision as an
        // int, but less precision as a float. This can lead to non-empty SkIRects becoming empty
        // simply because of float casting. If we're already dealing with a float rect or having a
        // float output, that's what we're stuck with; but if we are starting form an irect and
        // desiring an SkIRect output, we go through efforts to preserve the 1px precision for
        // simple transforms.
        if matrix.is_scale_translate() {
            let eps = f64::from(ROUND_EPSILON);
            let l = f64::from(matrix.scale_x()) * f64::from(geom.left)
                + f64::from(matrix.translate_x());
            let r = f64::from(matrix.scale_x()) * f64::from(geom.right)
                + f64::from(matrix.translate_x());
            let t =
                f64::from(matrix.scale_y()) * f64::from(geom.top) + f64::from(matrix.translate_y());
            let b = f64::from(matrix.scale_y()) * f64::from(geom.bottom)
                + f64::from(matrix.translate_y());
            IRect {
                left: double_saturate2int((l.min(r) + eps).floor()),
                top: double_saturate2int((t.min(b) + eps).floor()),
                right: double_saturate2int((l.max(r) - eps).ceil()),
                bottom: double_saturate2int((t.max(b) - eps).ceil()),
            }
        } else {
            round_out(&matrix.map_rect(Rect::from_irect(geom)).0)
        }
    }

    /// Maps a parameter-space rectangle to layer space (`paramToLayer`).
    #[doc(alias = "paramToLayer")]
    #[must_use]
    pub fn param_to_layer_rect(&self, param: &Rect) -> Rect {
        Mapping::map_rect(param, &self.param_to_layer.to_m33())
    }

    /// Maps a device-space integer rectangle to layer space (`deviceToLayer`); empty if the
    /// layer-to-device transform cannot be inverted in 3x3.
    // Port of: src/core/SkImageFilterTypes.h#L624-L636 (chrome/m156)
    #[doc(alias = "deviceToLayer")]
    #[must_use]
    pub fn device_to_layer(&self, dev: &IRect) -> IRect {
        if let Some(dev_to_layer33) = self.layer_to_dev.to_m33().invert() {
            return Mapping::map_irect(dev, &dev_to_layer33);
        }
        IRect::new_empty()
    }
}

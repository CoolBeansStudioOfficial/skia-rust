// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkImageFilterTypes.h, src/core/SkImageFilterTypes.cpp

//! The image filter types (`skif`): the coordinate-space [`Mapping`], the [`Backend`] that makes
//! devices and special images, the [`Stats`] and the [`Context`] that filters are evaluated in.
//!
//! skia-rust: `ParameterSpace<T>`, `DeviceSpace<T>` and `LayerSpace<T>` are not wrapper types
//! here. Each space is documented at the call site and the geometry is the plain `Rect`/`IRect`/
//! `Point`/`Matrix` (`LayerSpace<SkIRect>` is an `IRect`, and so on). `FilterResult` lives in
//! [`crate::image_filter_result`]. The shared `SkBlurEngine` and the image filter cache are not
//! ported yet, so [`Backend::blur_engine`] is absent and `Backend::cache` does not exist.

#![allow(
    // The float casts and exact float comparisons mirror the C++ arithmetic of the Skia source
    // (`SkScalar` and `int` conversions, `==` on scalars); the control flow keeps the C++ shape
    // so the port can be reviewed line by line.
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::manual_let_else
)]

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;

use crate::bitmap::Bitmap;
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::device::Device;
use crate::floating_point::double_saturate2int;
use crate::image::Image;
use crate::image_filter_result::FilterResult;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::matrix_priv::{differential_area_scale, is_scale_translate_as_m33};
use crate::point::{IPoint, Point, Vector};
use crate::rect::{IRect, Rect, RoundOut, rect_priv};
use crate::scalar::{SCALAR_NEARLY_ZERO, scalar, scalar_invert};
use crate::size::{ISize, Size};
use crate::special_image::SpecialImage;
use crate::surface_props::SurfaceProps;

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
// Port of: src/core/SkImageFilterTypes.cpp#L255 (chrome/m156)
#[doc(alias = "RoundOut")]
#[must_use]
pub fn round_out(r: &Rect) -> IRect {
    r.with_inset((ROUND_EPSILON, ROUND_EPSILON)).round_out()
}

/// `skif::RoundIn`: rounds in, ignoring `kRoundEpsilon` of slop.
// Port of: src/core/SkImageFilterTypes.cpp#L257 (chrome/m156)
#[doc(alias = "RoundIn")]
#[must_use]
pub fn round_in(r: &Rect) -> IRect {
    r.with_outset((ROUND_EPSILON, ROUND_EPSILON)).round_in()
}

/// `skif::Mapping::map<SkIRect>`'s float fallback and `LayerSpace<SkIRect>::inverseMapRect`'s
/// double-precision helper: `floor(min + eps)`, `ceil(max - eps)` as saturated ints.
fn saturated_bounds(l: f64, r: f64, t: f64, b: f64) -> IRect {
    let eps = f64::from(ROUND_EPSILON);
    IRect {
        left: double_saturate2int((l.min(r) + eps).floor()),
        top: double_saturate2int((t.min(b) + eps).floor()),
        right: double_saturate2int((l.max(r) - eps).ceil()),
        bottom: double_saturate2int((t.max(b) - eps).ceil()),
    }
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
    /// the filter can handle (`decomposeCTM(ctm, capability, pt)`). False if the decomposition
    /// failed or would produce an invalid device matrix.
    // Port of: src/core/SkImageFilterTypes.cpp#L259-L298 (chrome/m156)
    #[doc(alias = "decomposeCTM")]
    #[must_use]
    pub fn decompose_ctm(
        &mut self,
        ctm: &M44,
        capability: MatrixCapability,
        representative_pt: Point,
    ) -> bool {
        let (remainder, layer) = if capability == MatrixCapability::Translate {
            // Apply the entire CTM post-filtering
            (*ctm, M44::new_identity())
        } else if is_scale_translate_as_m33(ctm) || capability == MatrixCapability::Complex {
            // Either layer space can be anything (kComplex) - or - it can be scale+translate, and
            // the ctm is. In both cases, the layer space can be equivalent to device space.
            (M44::new_identity(), *ctm)
        } else {
            // This case implies some amount of sampling post-filtering, either due to skew or
            // rotation in the original matrix. Decompose the 3x3 into a scale (applied in layer
            // space) and a remainder (applied to the layer when drawn to the device).
            let (_, layer33) = decompose_transform(&ctm.to_m33(), representative_pt);
            let layer = M44::from(layer33);
            let mut remainder = *ctm;
            remainder.pre_scale(1.0 / layer.rc(0, 0), 1.0 / layer.rc(1, 1));
            (remainder, layer)
        };
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

    /// `applyOrigin`: shifts the layer space so that `origin` becomes the layer origin.
    // Port of: src/core/SkImageFilterTypes.h#L594-L597 (chrome/m156)
    #[doc(alias = "applyOrigin")]
    pub fn apply_origin(&mut self, origin: IPoint) {
        let adjusted = self.adjust_layer_space(&M44::translate(
            -scalar_from_i32(origin.x),
            -scalar_from_i32(origin.y),
            0.0,
        ));
        debug_assert!(adjusted);
    }

    /// Concatenates a local (parameter-space) matrix onto the parameter-to-layer transform
    /// (`concatLocal`).
    // Port of: src/core/SkImageFilterTypes.h#L558 (chrome/m156)
    #[doc(alias = "concatLocal")]
    pub fn concat_local(&mut self, local: &Matrix) {
        self.param_to_layer.pre_concat(&M44::from(local));
    }

    /// The layer-to-device transform (`layerToDevice`).
    #[doc(alias = "layerToDevice")]
    #[must_use]
    pub fn layer_to_device(&self) -> &M44 {
        &self.layer_to_dev
    }

    /// The device-to-layer transform (`deviceToLayer`).
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

    /// The total transform from parameter space to device space (`totalMatrix`).
    #[must_use]
    pub fn total_matrix(&self) -> M44 {
        M44::concat(&self.layer_to_dev, &self.param_to_layer)
    }

    /// `paramToLayer` of a rectangle (`Mapping::map<SkRect>` with the parameter-to-layer 3x3).
    // Port of: src/core/SkImageFilterTypes.cpp#L322-L325 (chrome/m156)
    #[doc(alias = "paramToLayer")]
    #[must_use]
    pub fn param_to_layer_rect(&self, param: &Rect) -> Rect {
        map_rect(param, &self.param_to_layer.to_m33())
    }

    /// `paramToLayer` of a matrix (`Mapping::map<SkMatrix>` with the parameter-to-layer 3x3).
    // Port of: src/core/SkImageFilterTypes.cpp#L395-L405 (chrome/m156)
    #[doc(alias = "paramToLayer")]
    #[must_use]
    pub fn param_to_layer_matrix(&self, param: &Matrix) -> Matrix {
        map_matrix(param, &self.param_to_layer.to_m33())
    }

    /// `paramToLayer` of a size (`Mapping::map<SkSize>` with the parameter-to-layer 3x3).
    // Port of: src/core/SkImageFilterTypes.cpp#L372-L386 (chrome/m156)
    #[doc(alias = "paramToLayer")]
    #[must_use]
    pub fn param_to_layer_size(&self, param: Size) -> Size {
        map_size(param, &self.param_to_layer.to_m33())
    }

    /// `paramToLayer` of a point.
    #[doc(alias = "paramToLayer")]
    #[must_use]
    pub fn param_to_layer_point(&self, param: Point) -> Point {
        map_point(param, &self.param_to_layer.to_m33())
    }

    /// `paramToLayer` of a vector (no translation).
    #[must_use]
    pub fn param_to_layer_vector(&self, param: Vector) -> Vector {
        map_vector(param, &self.param_to_layer.to_m33())
    }

    /// `deviceToLayer` of an integer rectangle; empty if the layer-to-device 3x3 cannot be
    /// inverted.
    // Port of: src/core/SkImageFilterTypes.h#L624-L636 (chrome/m156)
    #[doc(alias = "deviceToLayer")]
    #[must_use]
    pub fn device_to_layer(&self, dev: &IRect) -> IRect {
        if let Some(dev_to_layer33) = self.layer_to_dev.to_m33().invert() {
            return map_irect(dev, &dev_to_layer33);
        }
        IRect::new_empty()
    }

    /// `deviceToLayer` of a rectangle; empty if the layer-to-device 3x3 cannot be inverted.
    #[doc(alias = "deviceToLayer")]
    #[must_use]
    pub fn device_to_layer_rect(&self, dev: &Rect) -> Rect {
        if let Some(dev_to_layer33) = self.layer_to_dev.to_m33().invert() {
            return map_rect(dev, &dev_to_layer33);
        }
        Rect::new_empty()
    }

    /// `layerToDevice` of an integer rectangle.
    #[doc(alias = "layerToDevice")]
    #[must_use]
    pub fn layer_to_device_irect(&self, layer: &IRect) -> IRect {
        map_irect(layer, &self.layer_to_dev.to_m33())
    }

    /// `layerToDevice` of a rectangle.
    #[doc(alias = "layerToDevice")]
    #[must_use]
    pub fn layer_to_device_rect(&self, layer: &Rect) -> Rect {
        map_rect(layer, &self.layer_to_dev.to_m33())
    }
}

/// `decompose_transform`: splits `transform` into `postScaling * scaling`, where `scaling` is a
/// scale (approximating a non-scale-able matrix around `representative_point`). Returns
/// `(postScaling, scaling)`.
// Port of: src/core/SkImageFilterTypes.cpp#L93-L116 (chrome/m156)
pub(crate) fn decompose_transform(
    transform: &Matrix,
    representative_point: Point,
) -> (Matrix, Matrix) {
    let mut post = Matrix::new_identity();
    if let Some(scale) = transform.decompose_scale(Some(&mut post)) {
        return (post, Matrix::scale((scale.width, scale.height)));
    }
    let mut approx_scale = differential_area_scale(transform, representative_point);
    if approx_scale.is_finite() && approx_scale.abs() > SCALAR_NEARLY_ZERO {
        approx_scale = approx_scale.sqrt();
    } else {
        approx_scale = 1.0;
    }
    let mut post_scaling = transform.clone();
    let inv_scale = scalar_invert(approx_scale);
    post_scaling.pre_scale((inv_scale, inv_scale), None);
    (post_scaling, Matrix::scale((approx_scale, approx_scale)))
}

/// An integer coordinate as a float, as `SkIntToScalar` does.
fn scalar_from_i32(v: i32) -> scalar {
    v as scalar
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
        let sx = f64::from(matrix.scale_x());
        let sy = f64::from(matrix.scale_y());
        let tx = f64::from(matrix.translate_x());
        let ty = f64::from(matrix.translate_y());
        let l = sx * f64::from(geom.left) + tx;
        let r = sx * f64::from(geom.right) + tx;
        let t = sy * f64::from(geom.top) + ty;
        let b = sy * f64::from(geom.bottom) + ty;
        saturated_bounds(l, r, t, b)
    } else {
        round_out(&matrix.map_rect(Rect::from_irect(geom)).0)
    }
}

/// `Mapping::map<SkPoint>`.
// Port of: src/core/SkImageFilterTypes.cpp#L356-L359 (chrome/m156)
#[must_use]
pub fn map_point(geom: Point, matrix: &Matrix) -> Point {
    matrix.map_point(geom)
}

/// `Mapping::map<Vector>`.
// Port of: src/core/SkImageFilterTypes.cpp#L361-L364 (chrome/m156)
#[must_use]
pub fn map_vector(geom: Vector, matrix: &Matrix) -> Vector {
    matrix.map_vector(geom)
}

/// `Mapping::map<SkIPoint>`.
// Port of: src/core/SkImageFilterTypes.cpp#L350-L354 (chrome/m156)
#[must_use]
pub fn map_ipoint(geom: IPoint, matrix: &Matrix) -> IPoint {
    let p = matrix.map_point(Point::new(scalar_from_i32(geom.x), scalar_from_i32(geom.y)));
    IPoint::new(
        crate::scalar::scalar_round_to_int(p.x),
        crate::scalar::scalar_round_to_int(p.y),
    )
}

/// `Mapping::map<SkSize>`.
// Port of: src/core/SkImageFilterTypes.cpp#L375-L386 (chrome/m156)
#[must_use]
pub fn map_size(geom: Size, matrix: &Matrix) -> Size {
    if matrix.is_scale_translate() {
        let sizes = matrix.map_vector(Vector::new(geom.width, geom.height));
        return Size::new(sizes.x.abs(), sizes.y.abs());
    }
    let x_axis = matrix.map_vector(Vector::new(geom.width, 0.0));
    let y_axis = matrix.map_vector(Vector::new(0.0, geom.height));
    Size::new(x_axis.length(), y_axis.length())
}

/// `Mapping::map<SkISize>`.
// Port of: src/core/SkImageFilterTypes.cpp#L388-L393 (chrome/m156)
#[must_use]
pub fn map_isize(geom: ISize, matrix: &Matrix) -> ISize {
    let size = map_size(
        Size::new(scalar_from_i32(geom.width), scalar_from_i32(geom.height)),
        matrix,
    );
    ISize::new(
        crate::scalar::scalar_ceil_to_int(size.width - ROUND_EPSILON),
        crate::scalar::scalar_ceil_to_int(size.height - ROUND_EPSILON),
    )
}

/// `Mapping::map<SkMatrix>`: `inverse(matrix) * m * matrix`.
// Port of: src/core/SkImageFilterTypes.cpp#L395-L405 (chrome/m156)
#[must_use]
pub fn map_matrix(m: &Matrix, matrix: &Matrix) -> Matrix {
    let mut inv = matrix.invert().unwrap_or_else(Matrix::new_identity);
    inv.post_concat(m);
    inv.post_concat(matrix);
    inv
}

/// `LayerSpace<SkIRect>::relevantSubset`: the part of `src` that a tile mode of `tile_mode` needs
/// when drawing into `dst_rect`.
// Port of: src/core/SkImageFilterTypes.cpp#L408-L429 (chrome/m156)
#[doc(alias = "relevantSubset")]
#[must_use]
pub fn relevant_subset(
    src: IRect,
    dst_rect: IRect,
    tile_mode: crate::tile_mode::TileMode,
) -> IRect {
    use crate::tile_mode::TileMode;
    let mut fitted_src = src;
    if tile_mode == TileMode::Decal || tile_mode == TileMode::Clamp {
        if !irect_intersect_in_place(&mut fitted_src, &dst_rect) {
            if tile_mode == TileMode::Decal {
                fitted_src = IRect::new_empty();
            } else {
                fitted_src = rect_priv::closest_disjoint_edge(&src, &dst_rect);
            }
        }
    } // else assume the entire source is needed for periodic tile modes, so leave fittedSrc alone
    fitted_src
}

/// `SkIRect::intersect(r)`: intersects in place, leaving `dst` unchanged when the result is empty.
// Port of: include/core/SkRect.h (`SkIRect::intersect`) (chrome/m156)
pub fn irect_intersect_in_place(dst: &mut IRect, r: &IRect) -> bool {
    match IRect::intersect(dst, r) {
        Some(out) => {
            *dst = out;
            true
        }
        None => false,
    }
}

/// `LayerSpace<SkSize>::round`.
// Port of: src/core/SkImageFilterTypes.cpp#L432 (chrome/m156)
#[must_use]
pub fn size_round(s: Size) -> ISize {
    s.to_round()
}

/// `LayerSpace<SkSize>::ceil`.
// Port of: src/core/SkImageFilterTypes.cpp#L435-L438 (chrome/m156)
#[must_use]
pub fn size_ceil(s: Size) -> ISize {
    ISize::new(
        crate::scalar::scalar_ceil_to_int(s.width - ROUND_EPSILON),
        crate::scalar::scalar_ceil_to_int(s.height - ROUND_EPSILON),
    )
}

/// `LayerSpace<SkSize>::floor`.
// Port of: src/core/SkImageFilterTypes.cpp#L439-L442 (chrome/m156)
#[must_use]
pub fn size_floor(s: Size) -> ISize {
    ISize::new(
        crate::scalar::scalar_floor_to_int(s.width + ROUND_EPSILON),
        crate::scalar::scalar_floor_to_int(s.height + ROUND_EPSILON),
    )
}

/// `LayerSpace<SkMatrix>::inverseMapRect` for a rectangle. `None` if the matrix is not
/// invertible in the way `SkMatrixPriv::InverseMapRect` needs.
// Port of: src/core/SkImageFilterTypes.cpp#L466-L479 (chrome/m156)
#[must_use]
pub fn inverse_map_rect_f(m: &Matrix, r: &Rect) -> Option<Rect> {
    if r.is_empty() {
        return Some(Rect::new_empty());
    }
    crate::matrix_priv::inverse_map_rect(m, r)
}

/// `LayerSpace<SkMatrix>::inverseMapRect` for an integer rectangle.
// Port of: src/core/SkImageFilterTypes.cpp#L481-L512 (chrome/m156)
#[must_use]
pub fn inverse_map_irect(m: &Matrix, rect: &IRect) -> Option<IRect> {
    if rect.is_empty() {
        return Some(IRect::new_empty());
    }
    if m.is_scale_translate() {
        // Specialized inverse of 1px-preserving map<SkIRect>
        if m.scale_x() == 0.0 || m.scale_y() == 0.0 {
            return None;
        }
        let sx = f64::from(m.scale_x());
        let sy = f64::from(m.scale_y());
        let tx = f64::from(m.translate_x());
        let ty = f64::from(m.translate_y());
        let l = (f64::from(rect.left) - tx) / sx;
        let r = (f64::from(rect.right) - tx) / sx;
        let t = (f64::from(rect.top) - ty) / sy;
        let b = (f64::from(rect.bottom) - ty) / sy;
        Some(saturated_bounds(l, r, t, b))
    } else {
        let mapped = crate::matrix_priv::inverse_map_rect(m, &Rect::from_irect(rect))?;
        Some(round_out(&mapped))
    }
}

/// The `Stats` of an image filter evaluation (`skif::Stats`). Counters are cells so a `Context`
/// can record into a shared borrow.
// Port of: src/core/SkImageFilterTypes.h#L1141-L1150 (chrome/m156)
#[doc(alias = "skif::Stats")]
#[derive(Debug, Default)]
pub struct Stats {
    /// size of the filter dag
    pub num_visited_image_filters: Cell<i32>,
    /// amount of reuse within the dag
    pub num_cache_hits: Cell<i32>,
    /// difference to the # of visited filters shows deferred steps
    pub num_offscreen_surfaces: Cell<i32>,
    /// shader-emulated clamp is fairly cheap but HW tiling is best
    pub num_shader_clamped_draws: Cell<i32>,
    /// shader-emulated decal, mirror, repeat are expensive
    pub num_shader_based_tiling_draws: Cell<i32>,
}

/// The backend a filter evaluation makes its devices, images and shaders with
/// (`skif::Backend`). The raster backend is `skia_rust_raster::image_filter_backend`.
// Port of: src/core/SkImageFilterTypes.h#L1158-L1186 (chrome/m156)
#[doc(alias = "skif::Backend")]
pub trait Backend: Send + Sync {
    /// A device of `size` in premultiplied color type `color_type()` (`makeDevice`); `props`
    /// overrides the backend's surface properties.
    #[doc(alias = "makeDevice")]
    fn make_device(
        &self,
        size: ISize,
        color_space: Option<ColorSpace>,
        props: Option<&SurfaceProps>,
    ) -> Option<Box<dyn Device>>;

    /// A special image of `subset` of `image` (`makeImage`).
    #[doc(alias = "makeImage")]
    fn make_image(&self, subset: &IRect, image: &Image) -> Option<SpecialImage>;

    /// A raster image of `data`, sharing its pixels (`getCachedBitmap`).
    #[doc(alias = "getCachedBitmap")]
    fn get_cached_bitmap(&self, data: &Bitmap) -> Option<Image>;

    /// The surface properties devices of this backend get (`surfaceProps`).
    #[doc(alias = "surfaceProps")]
    fn surface_props(&self) -> &SurfaceProps;

    /// The color type of devices of this backend (`colorType`).
    #[doc(alias = "colorType")]
    fn color_type(&self) -> ColorType;

    /// The blur engine of this backend (`getBlurEngine`); `None` if it has none.
    #[doc(alias = "getBlurEngine")]
    fn blur_engine(&self) -> Option<&dyn crate::blur_engine::BlurEngine> {
        None
    }
}

/// The context a filter is evaluated in (`skif::Context`).
// Port of: src/core/SkImageFilterTypes.h#L1188-L1280 (chrome/m156)
#[doc(alias = "skif::Context")]
#[derive(Clone)]
pub struct Context<'a> {
    backend: Arc<dyn Backend>,
    mapping: Mapping,
    desired_output: IRect,
    source: FilterResult,
    color_space: Option<ColorSpace>,
    stats: Option<&'a Stats>,
}

impl fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("mapping", &self.mapping)
            .field("desired_output", &self.desired_output)
            .field("source", &self.source)
            .field("color_space", &self.color_space)
            .finish_non_exhaustive()
    }
}

impl<'a> Context<'a> {
    /// Creates a context (`Context(backend, mapping, desiredOutput, source, colorSpace, stats)`).
    #[must_use]
    pub fn new(
        backend: Arc<dyn Backend>,
        mapping: Mapping,
        desired_output: IRect,
        source: FilterResult,
        color_space: Option<ColorSpace>,
        stats: Option<&'a Stats>,
    ) -> Context<'a> {
        Context {
            backend,
            mapping,
            desired_output,
            source,
            color_space,
            stats,
        }
    }

    /// The backend (`backend`).
    #[must_use]
    pub fn backend(&self) -> &dyn Backend {
        &*self.backend
    }

    /// A shared handle to the backend (`sk_sp<Backend>`).
    #[must_use]
    pub fn backend_arc(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }

    /// The mapping of this evaluation (`mapping`).
    #[must_use]
    pub fn mapping(&self) -> &Mapping {
        &self.mapping
    }

    /// The layer-space rectangle the output must cover (`desiredOutput`).
    #[must_use]
    pub fn desired_output(&self) -> IRect {
        self.desired_output
    }

    /// The color space of intermediate results (`colorSpace`).
    #[must_use]
    pub fn color_space(&self) -> Option<&ColorSpace> {
        self.color_space.as_ref()
    }

    /// The source image the filter DAG reads when an input is null (`source`).
    #[must_use]
    pub fn source(&self) -> &FilterResult {
        &self.source
    }

    /// The same context with a new mapping (`withNewMapping`).
    #[must_use]
    pub fn with_new_mapping(&self, mapping: Mapping) -> Context<'a> {
        let mut c = self.clone();
        c.mapping = mapping;
        c
    }

    /// The same context with a new desired output (`withNewDesiredOutput`).
    #[must_use]
    pub fn with_new_desired_output(&self, desired_output: IRect) -> Context<'a> {
        let mut c = self.clone();
        c.desired_output = desired_output;
        c
    }

    /// The same context with a new color space (`withNewColorSpace`).
    #[must_use]
    pub fn with_new_color_space(&self, color_space: Option<ColorSpace>) -> Context<'a> {
        let mut c = self.clone();
        c.color_space = color_space;
        c
    }

    /// The same context with a new source (`withNewSource`).
    #[must_use]
    pub fn with_new_source(&self, source: FilterResult) -> Context<'a> {
        let mut c = self.clone();
        c.source = source;
        c
    }

    /// Records a visited filter (`markVisitedImageFilter`).
    pub fn mark_visited_image_filter(&self) {
        if let Some(s) = self.stats {
            s.num_visited_image_filters
                .set(s.num_visited_image_filters.get() + 1);
        }
    }

    /// Records a cache hit (`markCacheHit`).
    pub fn mark_cache_hit(&self) {
        if let Some(s) = self.stats {
            s.num_cache_hits.set(s.num_cache_hits.get() + 1);
        }
    }

    /// Records an offscreen surface (`markNewSurface`).
    pub fn mark_new_surface(&self) {
        if let Some(s) = self.stats {
            s.num_offscreen_surfaces
                .set(s.num_offscreen_surfaces.get() + 1);
        }
    }

    /// Records a draw that needs shader-based tiling (`markShaderBasedTilingRequired`).
    pub fn mark_shader_based_tiling_required(&self, tile_mode: crate::tile_mode::TileMode) {
        if let Some(s) = self.stats {
            if tile_mode == crate::tile_mode::TileMode::Clamp {
                s.num_shader_clamped_draws
                    .set(s.num_shader_clamped_draws.get() + 1);
            } else {
                s.num_shader_based_tiling_draws
                    .set(s.num_shader_based_tiling_draws.get() + 1);
            }
        }
    }
}

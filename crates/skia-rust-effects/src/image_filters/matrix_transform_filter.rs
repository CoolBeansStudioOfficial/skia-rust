// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp

//! `SkMatrixTransformImageFilter`: transforms its input by a matrix. `Offset` is a matrix transform
//! by a translation with nearest sampling.

use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter::{flatten_common, input_at, unflatten_common};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{
    Context, Mapping, MatrixCapability, inverse_map_irect, map_irect,
};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::picture_priv;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::sampling_priv::{LegacyFQ, MediumAs, from_fq};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::image_filters::crop_filter::crop;

/// The matrix transform filter (`SkMatrixTransformImageFilter`).
// Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L18-L44 (chrome/m156)
#[doc(alias = "SkMatrixTransformImageFilter")]
#[derive(Debug)]
pub struct MatrixTransformImageFilter {
    common: ImageFilterCommon,
    /// The parameter-space matrix (`fTransform`).
    transform: Matrix,
    sampling: SamplingOptions,
}

impl MatrixTransformImageFilter {
    /// `SkMatrixTransformImageFilter(transform, sampling, input)`.
    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L18-L27 (chrome/m156)
    #[must_use]
    pub fn new(transform: Matrix, sampling: SamplingOptions, input: Option<ImageFilter>) -> Self {
        MatrixTransformImageFilter {
            common: ImageFilterCommon::new(vec![input], None),
            transform,
            sampling,
        }
    }

    /// `requiredInput`: the input the transform needs to cover `desired_output`.
    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L125-L136 (chrome/m156)
    fn required_input(&self, mapping: &Mapping, desired_output: IRect) -> IRect {
        let layer_transform = mapping.param_to_layer_matrix(&self.transform);
        let Some(mut required_input) = inverse_map_irect(&layer_transform, &desired_output) else {
            return IRect::new_empty();
        };
        if self.sampling != SamplingOptions::default() {
            required_input.outset((1, 1));
        }
        required_input
    }
}

impl ImageFilterBase for MatrixTransformImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L52 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkMatrixTransformImageFilter"
    }

    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L190-L194 (chrome/m156), flatten
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        flatten_common(&self.common, buffer);
        buffer.write_matrix(&self.transform);
        buffer.write_sampling(&self.sampling);
    }

    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L34 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L163-L169 (chrome/m156)
    fn on_filter_image(&self, context: &Context<'_>) -> FilterResult {
        let required_input = self.required_input(context.mapping(), context.desired_output());
        let child_output =
            self.get_child_output(0, &context.with_new_desired_output(required_input));
        let transform = context.mapping().param_to_layer_matrix(&self.transform);
        child_output.apply_transform(context, &transform, self.sampling)
    }

    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L171-L176 (chrome/m156)
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        let bounds = match self.get_input(0) {
            Some(input) => input.compute_fast_bounds(src),
            None => *src,
        };
        self.transform.map_rect(bounds).0
    }

    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L178-L185 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        let required_input = self.required_input(mapping, desired_output);
        self.get_child_input_layer_bounds(0, mapping, required_input, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L187-L198 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        let child_output = self.get_child_output_layer_bounds(0, mapping, content_bounds)?;
        let layer_transform = mapping.param_to_layer_matrix(&self.transform);
        Some(map_irect(&child_output, &layer_transform))
    }
}

/// `SkImageFilters::MatrixTransform(transform, sampling, input)`: `None` if `transform` is not
/// invertible.
// Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L46-L56 (chrome/m156)
#[doc(alias = "MatrixTransform")]
#[must_use]
pub fn matrix_transform(
    transform: &Matrix,
    sampling: SamplingOptions,
    input: Option<ImageFilter>,
) -> Option<ImageFilter> {
    transform.invert()?;
    Some(ImageFilter::from_base(MatrixTransformImageFilter::new(
        transform.clone(),
        sampling,
        input,
    )))
}

/// `SkImageFilters::Offset(dx, dy, input, cropRect)`: translates `input` by `(dx, dy)`, then
/// crops it to `crop` if given.
// Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L58-L71 (chrome/m156)
#[doc(alias = "Offset")]
#[must_use]
pub fn offset(
    delta: (f32, f32),
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    let translate = Matrix::translate(delta);
    let mut offset = matrix_transform(
        &translate,
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
        input,
    );
    if let Some(crop_rect) = crop_rect {
        // `offset = SkImageFilters::Crop(*cropRect, std::move(offset))`: a null crop gives null.
        offset = crop(&crop_rect, TileMode::Decal, offset);
    }
    offset
}

/// `SkMatrixTransformImageFilter::CreateProc`.
// Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L154-L170 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    registry: &FlattenableRegistry,
) -> Option<ImageFilter> {
    let (inputs, _) = unflatten_common(buffer, 1, registry)?;
    let matrix = buffer.read_matrix();
    let sampling = if buffer.is_version_lt(picture_priv::VERSION_MATRIX_IMAGE_FILTER_SAMPLING) {
        from_fq(
            legacy_fq(buffer.read32_le(LEGACY_FQ_LAST)),
            MediumAs::Linear,
        )
    } else {
        buffer.read_sampling()
    };
    matrix_transform(&matrix, sampling, input_at(&inputs, 0))
}

/// `SkMatrixTransformImageFilter::LegacyOffsetCreateProc`: the offset of the old names.
// Port of: src/effects/imagefilters/SkMatrixTransformImageFilter.cpp#L142-L148 (chrome/m156)
pub fn legacy_offset_create_proc(
    buffer: &mut ReadBuffer<'_>,
    registry: &FlattenableRegistry,
) -> Option<ImageFilter> {
    let (inputs, crop_rect) = unflatten_common(buffer, 1, registry)?;
    let point = buffer.read_point();
    offset((point.x, point.y), input_at(&inputs, 0), crop_rect)
}

/// `kLast_SkLegacyFQ`.
// Port of: src/core/SkSamplingPriv.h#L28 (chrome/m156)
const LEGACY_FQ_LAST: u32 = 3;

/// The legacy filter quality of `value`, which the reader has checked is at most
/// [`LEGACY_FQ_LAST`].
fn legacy_fq(value: u32) -> LegacyFQ {
    match value {
        1 => LegacyFQ::Low,
        2 => LegacyFQ::Medium,
        3 => LegacyFQ::High,
        _ => LegacyFQ::None,
    }
}

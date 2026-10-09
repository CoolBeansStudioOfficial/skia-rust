// Copyright 2015 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkLocalMatrixImageFilter.cpp, src/core/SkLocalMatrixImageFilter.h

//! `SkLocalMatrixImageFilter`: wraps an image filter and a matrix, so that using the wrapper gives
//! the same result as using the wrapped filter with the matrix applied to its context. It is what
//! [`ImageFilter::with_local_matrix`] (`SkImageFilter::makeWithLocalMatrix`) builds.

use crate::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use crate::image_filter_result::FilterResult;
use crate::image_filter_types::{Context, Mapping, MatrixCapability};
use crate::matrix::Matrix;
use crate::rect::{IRect, Rect};

/// `SkLocalMatrixImageFilter`.
// Port of: src/core/SkLocalMatrixImageFilter.h#L21-L70 (chrome/m156)
#[doc(alias = "SkLocalMatrixImageFilter")]
#[derive(Debug)]
pub struct LocalMatrixImageFilter {
    common: ImageFilterCommon,
    /// `fLocalMatrix`.
    local_matrix: Matrix,
    /// `fInvLocalMatrix`.
    inv_local_matrix: Matrix,
}

impl LocalMatrixImageFilter {
    /// `SkLocalMatrixImageFilter(localMatrix, invLocalMatrix, input)`.
    // Port of: src/core/SkLocalMatrixImageFilter.h#L38-L43 (chrome/m156)
    fn new(local_matrix: Matrix, inv_local_matrix: Matrix, input: ImageFilter) -> Self {
        LocalMatrixImageFilter {
            common: ImageFilterCommon::new(vec![Some(input)], None),
            local_matrix,
            inv_local_matrix,
        }
    }

    /// `SkLocalMatrixImageFilter::localMapping`: the mapping of the child, with the local matrix
    /// applied to its parameter space.
    // Port of: src/core/SkLocalMatrixImageFilter.cpp#L51-L55 (chrome/m156)
    fn local_mapping(&self, mapping: &Mapping) -> Mapping {
        let mut local_mapping = mapping.clone();
        local_mapping.concat_local(&self.local_matrix);
        local_mapping
    }
}

impl ImageFilterBase for LocalMatrixImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/core/SkLocalMatrixImageFilter.h#L46 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/core/SkLocalMatrixImageFilter.cpp#L57-L61 (chrome/m156)
    fn on_filter_image(&self, context: &Context<'_>) -> FilterResult {
        let local_mapping = self.local_mapping(context.mapping());
        self.get_child_output(0, &context.with_new_mapping(local_mapping))
    }

    // Port of: src/core/SkLocalMatrixImageFilter.cpp#L63-L72 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        // The local matrix changes 'mapping' by adjusting the parameter space of the image
        // filter, but 'desired_output' and 'content_bounds' have already been transformed to the
        // consistent layer space. They remain unchanged with the new mapping.
        self.get_child_input_layer_bounds(
            0,
            &self.local_mapping(mapping),
            desired_output,
            content_bounds,
        )
    }

    // Port of: src/core/SkLocalMatrixImageFilter.cpp#L74-L77 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        self.get_child_output_layer_bounds(0, &self.local_mapping(mapping), content_bounds)
    }

    // Port of: src/core/SkLocalMatrixImageFilter.cpp#L79-L92 (chrome/m156)
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        // In on_get_[input|output]_layer_bounds there is a Mapping that the local matrix adjusts,
        // but compute_fast_bounds takes no matrix, so it behaves as if the mapping were the
        // identity. To match, map `src` by the inverse of the local matrix, pass that to the
        // child, and map the result by the local matrix.
        let (local_bounds, _) = self.inv_local_matrix.map_rect(src);
        let child_bounds = match self.get_input(0) {
            Some(input) => input.compute_fast_bounds(&local_bounds),
            None => local_bounds,
        };
        self.local_matrix.map_rect(child_bounds).0
    }
}

/// `SkLocalMatrixImageFilter::Make(localMatrix, input)`: `None` if `input` is `None` or if the
/// local matrix cannot be inverted or does not preserve the input's matrix capability.
// Port of: src/core/SkLocalMatrixImageFilter.cpp#L14-L34 (chrome/m156)
#[doc(alias = "SkLocalMatrixImageFilter::Make")]
#[must_use]
pub fn make_local_matrix_image_filter(
    local_matrix: &Matrix,
    input: Option<ImageFilter>,
) -> Option<ImageFilter> {
    let input = input?;
    if local_matrix.is_identity() {
        return Some(input);
    }

    let input_capability = input.as_base().get_ctm_capability();
    if (input_capability == MatrixCapability::Translate && !local_matrix.is_translate())
        || (input_capability == MatrixCapability::ScaleTranslate
            && !local_matrix.is_scale_translate())
    {
        // Nothing we can do at this point.
        return None;
    }

    let inv_local = local_matrix.invert()?;
    Some(ImageFilter::from_base(LocalMatrixImageFilter::new(
        local_matrix.clone(),
        inv_local,
        input,
    )))
}

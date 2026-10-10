// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/VertexFiller.h, src/text/gpu/VertexFiller.cpp

//! [`VertexFiller`]: the positions and bounds of the glyphs of an atlas sub run, and how they
//! map to the device for a position matrix that differs from the one they were created with.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_is_int;

use crate::gpu::mask_format::MaskFormat;

/// `sktext::gpu::FillerType`.
// Port of: src/text/gpu/VertexFiller.h#L32-L35 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillerType {
    /// `kIsDirect`.
    IsDirect,
    /// `kIsTransformed`.
    IsTransformed,
}

/// The `VertexFiller` assumes that all points, glyph atlas entries, and bounds are created with
/// respect to the creation matrix. This assumes that mapping any point, mask or bounds through
/// the creation matrix will result in the proper device position. In order to draw using an
/// arbitrary position matrix, calculate a
///
/// ```text
/// viewDifference = [PositionMatrix] * [CreationMatrix] ^ -1.
/// ```
///
/// The view difference is used to map all points, masks and bounds to position to the device
/// respecting the position matrix.
// Port of: src/text/gpu/VertexFiller.h#L37-L103 (chrome/m156)
#[doc(alias = "sktext::gpu::VertexFiller")]
#[derive(Clone, Debug)]
pub struct VertexFiller {
    /// `fMaskFormat`.
    mask_format: MaskFormat,
    /// `fCanDrawDirect`.
    can_draw_direct: bool,
    /// `fCreationMatrix`.
    creation_matrix: Matrix,
    /// `fCreationBounds`.
    creation_bounds: Rect,
    /// `fLeftTop`.
    left_top: Vec<Point>,
}

impl VertexFiller {
    /// `VertexFiller(maskFormat, creationMatrix, creationBounds, leftTop, canDrawDirect)`.
    // Port of: src/text/gpu/VertexFiller.cpp#L24-L35 (chrome/m156)
    #[must_use]
    pub fn new(
        mask_format: MaskFormat,
        creation_matrix: &Matrix,
        creation_bounds: Rect,
        left_top: &[Point],
        can_draw_direct: bool,
    ) -> Self {
        Self {
            mask_format,
            can_draw_direct,
            creation_matrix: creation_matrix.clone(),
            creation_bounds,
            left_top: left_top.to_vec(),
        }
    }

    /// `Make(maskType, creationMatrix, creationBounds, positions, alloc, fillerType)`.
    // Port of: src/text/gpu/VertexFiller.cpp#L37-L47 (chrome/m156)
    #[must_use]
    pub fn make(
        mask_type: MaskFormat,
        creation_matrix: &Matrix,
        creation_bounds: Rect,
        positions: &[Point],
        filler_type: FillerType,
    ) -> Self {
        Self::new(
            mask_type,
            creation_matrix,
            creation_bounds,
            positions,
            filler_type == FillerType::IsDirect,
        )
    }

    /// `unflattenSize()`.
    #[must_use]
    pub fn unflatten_size(&self) -> usize {
        self.left_top.len() * std::mem::size_of::<Point>()
    }

    /// `viewDifference(positionMatrix)`: `positionMatrix * inverse(creationMatrix)`, or the
    /// identity if the creation matrix is singular.
    // Port of: src/text/gpu/VertexFiller.cpp#L49-L54 (chrome/m156)
    #[must_use]
    pub fn view_difference(&self, position_matrix: &Matrix) -> Matrix {
        if let Some(inverse) = self.creation_matrix.invert() {
            return Matrix::concat(position_matrix, &inverse);
        }
        Matrix::i().clone()
    }

    /// `CanUseDirect(creationMatrix, positionMatrix)`: checks for an integer translate with the
    /// same 2x2 matrix. Returns true if the change from creation matrix to the position matrix
    /// supports using direct glyph masks, and the translation.
    // Port of: src/text/gpu/VertexFiller.cpp#L56-L74 (chrome/m156)
    // The scales and skews are compared exactly, as the C++ does.
    #[allow(clippy::float_cmp)]
    #[must_use]
    pub fn can_use_direct(creation_matrix: &Matrix, position_matrix: &Matrix) -> (bool, Vector) {
        // The existing direct glyph info can be used if the creationMatrix, and the
        // positionMatrix have the same 2x2, the translation between them is integer, and no
        // perspective is involved. Calculate the translation in source space to a translation in
        // device space by mapping (0, 0) through both the creationMatrix and the
        // positionMatrix; take the difference.
        let translation = position_matrix.map_origin() - creation_matrix.map_origin();
        (
            creation_matrix.scale_x() == position_matrix.scale_x()
                && creation_matrix.scale_y() == position_matrix.scale_y()
                && creation_matrix.skew_x() == position_matrix.skew_x()
                && creation_matrix.skew_y() == position_matrix.skew_y()
                && !position_matrix.has_perspective()
                && !creation_matrix.has_perspective()
                && scalar_is_int(translation.x)
                && scalar_is_int(translation.y),
            translation,
        )
    }

    /// `canUseDirect(positionMatrix)`.
    // Port of: src/text/gpu/VertexFiller.h#L76-L78 (chrome/m156)
    #[must_use]
    pub fn can_use_direct_with(&self, position_matrix: &Matrix) -> (bool, Vector) {
        Self::can_use_direct(&self.creation_matrix, position_matrix)
    }

    /// `isLCD()`.
    // Port of: src/text/gpu/VertexFiller.cpp#L76 (chrome/m156)
    #[must_use]
    pub fn is_lcd(&self) -> bool {
        self.mask_format == MaskFormat::A565
    }

    /// Returns true if the position matrix represents an integer translation, and the device
    /// bounding box of all the glyphs. If the bounding box is empty, then something went
    /// singular and this operation should be dropped.
    // Port of: src/text/gpu/VertexFiller.cpp#L78-L98 (chrome/m156)
    #[must_use]
    pub fn device_rect_and_check_transform(&self, position_matrix: &Matrix) -> (bool, Rect) {
        if self.can_draw_direct {
            let (direct_draw_compatible, offset) =
                Self::can_use_direct(&self.creation_matrix, position_matrix);

            if direct_draw_compatible {
                return (true, self.creation_bounds.with_offset(offset));
            }
        }

        if let Some(inverse) = self.creation_matrix.invert() {
            let view_difference = Matrix::concat(position_matrix, &inverse);
            return (false, view_difference.map_rect(self.creation_bounds).0);
        }

        // initialPositionMatrix is singular. Do nothing.
        (false, Rect::new_empty())
    }

    /// `boundsAndDeviceMatrix(localToDevice, drawOrigin)`: the bounds of the glyphs in the mask
    /// (creation) space, and the matrix from the mask space to the device.
    // Port of: src/text/gpu/VertexFiller.cpp#L100-L133 (chrome/m156)
    // The matrix entries are compared exactly, as the C++ does.
    #[allow(clippy::float_cmp)]
    #[must_use]
    pub fn bounds_and_device_matrix(
        &self,
        local_to_device: &Matrix,
        draw_origin: Point,
    ) -> (Rect, Matrix) {
        // The baked-in matrix differs from the current localToDevice by a translation if the
        // upper 2x2 remains the same, and there's no perspective. Since there's no projection,
        // Z is irrelevant, so it's okay that fCreationMatrix is an SkMatrix and has discarded the
        // 3rd row/col, and can ignore those values in localToDevice.
        let compatible_matrix = local_to_device.rc(0, 0) == self.creation_matrix.rc(0, 0)
            && local_to_device.rc(0, 1) == self.creation_matrix.rc(0, 1)
            && local_to_device.rc(1, 0) == self.creation_matrix.rc(1, 0)
            && local_to_device.rc(1, 1) == self.creation_matrix.rc(1, 1)
            && !local_to_device.has_perspective()
            && !self.creation_matrix.has_perspective();

        if compatible_matrix {
            let mapped_origin = local_to_device.map_point(draw_origin);
            let offset = mapped_origin
                - Point::new(
                    self.creation_matrix.translate_x(),
                    self.creation_matrix.translate_y(),
                );
            if scalar_is_int(offset.x) && scalar_is_int(offset.y) {
                // The offset is an integer (but make sure), which means the generated mask can
                // be accessed without changing how texels would be sampled.
                return (self.creation_bounds, Matrix::translate(offset));
            }
        }

        // Otherwise compute the relative transformation from fCreationMatrix to localToDevice,
        // with the drawOrigin applied. If fCreationMatrix or the concatenation is not invertible
        // the returned Transform is marked invalid and the draw will be automatically dropped.
        let mut position_matrix = local_to_device.clone();
        position_matrix.pre_translate((draw_origin.x, draw_origin.y));
        let view_difference = self.view_difference(&position_matrix);
        (self.creation_bounds, view_difference)
    }

    /// `maskFormat()`.
    // Port of: src/text/gpu/VertexFiller.h#L61 (chrome/m156)
    #[must_use]
    pub fn mask_format(&self) -> MaskFormat {
        self.mask_format
    }

    /// `count()`.
    // Port of: src/text/gpu/VertexFiller.h#L65 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.left_top.len()
    }

    /// `topLefts()`.
    // Port of: src/text/gpu/VertexFiller.h#L67 (chrome/m156)
    #[must_use]
    pub fn top_lefts(&self) -> &[Point] {
        &self.left_top
    }

    /// `canDrawDirect()`.
    // Port of: src/text/gpu/VertexFiller.h#L69 (chrome/m156)
    #[must_use]
    pub fn can_draw_direct(&self) -> bool {
        self.can_draw_direct
    }

    /// `creationMatrix()`.
    // Port of: src/text/gpu/VertexFiller.h#L71 (chrome/m156)
    #[must_use]
    pub fn creation_matrix(&self) -> &Matrix {
        &self.creation_matrix
    }

    /// `fCreationBounds`.
    #[must_use]
    pub fn creation_bounds(&self) -> Rect {
        self.creation_bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filler(creation: &Matrix, direct: bool) -> VertexFiller {
        VertexFiller::new(
            MaskFormat::A8,
            creation,
            Rect::from_ltrb(10.0, 10.0, 30.0, 20.0),
            &[Point::new(10.0, 10.0), Point::new(20.0, 10.0)],
            direct,
        )
    }

    #[test]
    fn integer_translations_keep_direct_masks() {
        let creation = Matrix::translate((3.0, 4.0));
        let vf = filler(&creation, true);
        let (ok, rect) = vf.device_rect_and_check_transform(&Matrix::translate((8.0, 1.0)));
        assert!(ok);
        assert_eq!(rect, Rect::from_ltrb(15.0, 7.0, 35.0, 17.0));
    }

    #[test]
    fn fractional_translations_or_scales_need_a_transform() {
        let creation = Matrix::i().clone();
        let vf = filler(&creation, true);
        let (ok, _) = vf.device_rect_and_check_transform(&Matrix::translate((0.5, 0.0)));
        assert!(!ok);
        let (ok, rect) = vf.device_rect_and_check_transform(&Matrix::scale((2.0, 2.0)));
        assert!(!ok);
        assert_eq!(rect, Rect::from_ltrb(20.0, 20.0, 60.0, 40.0));
    }

    #[test]
    fn transformed_fillers_never_draw_direct() {
        let creation = Matrix::i().clone();
        let vf = filler(&creation, false);
        let (ok, _) = vf.device_rect_and_check_transform(&Matrix::translate((8.0, 1.0)));
        assert!(!ok);
    }

    #[test]
    fn a_singular_creation_matrix_gives_an_empty_rect() {
        let vf = filler(&Matrix::scale((0.0, 1.0)), false);
        let (ok, rect) = vf.device_rect_and_check_transform(&Matrix::i().clone());
        assert!(!ok);
        assert!(rect.is_empty());
    }

    #[test]
    fn bounds_and_device_matrix_extracts_integer_translations() {
        let creation = Matrix::translate((3.0, 4.0));
        let vf = filler(&creation, true);
        let (bounds, m) =
            vf.bounds_and_device_matrix(&Matrix::translate((10.0, 20.0)), Point::new(0.0, 0.0));
        assert_eq!(bounds, vf.creation_bounds());
        assert_eq!(m, Matrix::translate((7.0, 16.0)));
    }

    #[test]
    fn bounds_and_device_matrix_falls_back_to_the_view_difference() {
        let vf = filler(&Matrix::i().clone(), true);
        let (_, m) = vf.bounds_and_device_matrix(&Matrix::scale((2.0, 2.0)), Point::new(1.0, 1.0));
        // positionMatrix = scale(2) preTranslate(1, 1); creation is the identity.
        let mut expected = Matrix::scale((2.0, 2.0));
        expected.pre_translate((1.0, 1.0));
        assert_eq!(m, expected);
    }
}

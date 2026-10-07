// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCanvasPriv.h, src/core/SkCanvasPriv.cpp

//! `SkCanvasPriv`: the private canvas helpers pictures use (`SkAutoCanvasMatrixPaint` and the
//! unroll limit of `drawPicture`).

use crate::canvas::{Canvas, SaveLayerRec};
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::rect::Rect;

/// This constant is trying to balance the speed of ref'ing a subpicture into a parent picture,
/// against the playback cost of recursing into the subpicture to get at its actual ops.
///
/// For now Skia picks a conservatively small value.
// Port of: src/core/SkCanvasPriv.h#L115-L115 (chrome/m156)
#[doc(alias = "kMaxPictureOpsToUnrollInsteadOfRef")]
pub const MAX_PICTURE_OPS_TO_UNROLL_INSTEAD_OF_REF: usize = 1;

/// Saves the canvas (with a layer if there is a paint), applies a matrix, and restores the canvas
/// to its save count when dropped (`SkAutoCanvasMatrixPaint`).
// Port of: src/core/SkCanvasPriv.h#L25-L33 (chrome/m156)
#[doc(alias = "SkAutoCanvasMatrixPaint")]
#[derive(Debug)]
#[must_use]
pub struct AutoCanvasMatrixPaint<'a> {
    canvas: &'a Canvas,
    save_count: usize,
}

impl<'a> AutoCanvasMatrixPaint<'a> {
    // Port of: src/core/SkCanvasPriv.cpp#L32-L49 (chrome/m156)
    pub fn new(
        canvas: &'a Canvas,
        matrix: Option<&Matrix>,
        paint: Option<&Paint>,
        bounds: &Rect,
    ) -> Self {
        let save_count = canvas.save_count();
        if let Some(paint) = paint {
            let mut new_bounds = *bounds;
            if let Some(matrix) = matrix {
                new_bounds = matrix.map_rect(new_bounds).0;
            }
            canvas.save_layer(&SaveLayerRec::default().bounds(&new_bounds).paint(paint));
        } else if matrix.is_some() {
            canvas.save();
        }

        if let Some(matrix) = matrix {
            canvas.concat(matrix);
        }
        AutoCanvasMatrixPaint { canvas, save_count }
    }
}

// Port of: src/core/SkCanvasPriv.cpp#L51-L53 (chrome/m156)
impl Drop for AutoCanvasMatrixPaint<'_> {
    fn drop(&mut self) {
        self.canvas.restore_to_count(self.save_count);
    }
}

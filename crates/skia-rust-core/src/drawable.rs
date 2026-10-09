// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkDrawable.h, src/core/SkDrawable.cpp

//! [`Drawable`]: an object that draws itself into a canvas and reports its bounds (`SkDrawable`).
//!
//! The per-type behavior is the [`DrawableBase`] trait (the `on*` virtuals). [`Drawable`] is the
//! shared handle, like `sk_sp<SkDrawable>`. The GPU snapshot and picture-snapshot virtuals are
//! not part of this port (they belong to the GPU and picture phases).

use std::fmt;
use std::sync::Arc;

use crate::canvas::Canvas;
use crate::matrix::Matrix;
use crate::point::Vector;
use crate::rect::Rect;
use crate::scalar::scalar;

/// The `on*` virtuals of `SkDrawable`.
// Port of: include/core/SkDrawable.h#L149-L153 (chrome/m156)
#[doc(alias = "SkDrawable")]
pub trait DrawableBase: Send + Sync + fmt::Debug {
    /// `SkDrawable::onGetBounds`: the bounds of everything the drawable draws.
    // Port of: include/core/SkDrawable.h#L151 (chrome/m156)
    fn on_get_bounds(&self) -> Rect;

    /// `SkDrawable::onDraw`: draws into `canvas`. The caller has saved the canvas state.
    // Port of: include/core/SkDrawable.h#L153 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas);

    /// `SkDrawable::onApproximateBytesUsed`: 0 unless the drawable overrides it.
    // Port of: src/core/SkDrawable.cpp#L78-L80 (chrome/m156)
    fn on_approximate_bytes_used(&self) -> usize {
        0
    }
}

/// A shared handle to a drawable (`sk_sp<SkDrawable>`). Cloning it shares the drawable.
// Port of: include/core/SkDrawable.h#L40-L140 (chrome/m156)
#[doc(alias = "SkDrawable")]
#[derive(Clone, Debug)]
pub struct Drawable(Arc<dyn DrawableBase>);

impl Drawable {
    /// Wraps a drawable implementation in a shared handle.
    #[must_use]
    pub fn new(base: Arc<dyn DrawableBase>) -> Self {
        Self(base)
    }

    /// `SkDrawable::draw(canvas, matrix)`: saves the canvas, concatenates `matrix` if any, draws,
    /// and restores the canvas to the state it had.
    // Port of: src/core/SkDrawable.cpp#L43-L53 (chrome/m156)
    pub fn draw(&self, canvas: &Canvas, matrix: Option<&Matrix>) {
        let save_count = canvas.save_count();
        canvas.save();
        if let Some(matrix) = matrix {
            canvas.concat(matrix);
        }
        self.0.on_draw(canvas);
        canvas.restore_to_count(save_count);
    }

    /// `SkDrawable::draw(canvas, x, y)`: draws translated by `(x, y)`.
    // Port of: src/core/SkDrawable.cpp#L55-L58 (chrome/m156)
    pub fn draw_at(&self, canvas: &Canvas, x: scalar, y: scalar) {
        let matrix = Matrix::translate(Vector::new(x, y));
        self.draw(canvas, Some(&matrix));
    }

    /// `SkDrawable::getBounds`.
    // Port of: src/core/SkDrawable.cpp#L71-L73 (chrome/m156)
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.0.on_get_bounds()
    }

    /// `SkDrawable::approximateBytesUsed`.
    // Port of: src/core/SkDrawable.cpp#L75-L77 (chrome/m156)
    #[doc(alias = "approximateBytesUsed")]
    #[must_use]
    pub fn approximate_bytes_used(&self) -> usize {
        self.0.on_approximate_bytes_used()
    }
}

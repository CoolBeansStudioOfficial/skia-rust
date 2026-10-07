// Copyright 2006 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkCanvas.h (`SkCanvas::PointMode`)

//! The small enums of `SkCanvas` that the draw layer ([`device`](crate::device), `SkDraw`) needs.
//!
//! skia-rust: D5 adds only [`PointMode`]. `Canvas` itself (task D6) will live next to it and
//! `skia-safe` spells the path `canvas::PointMode`.

/// How [`Device::draw_points`](crate::device::Device::draw_points) interprets its points
/// (`SkCanvas::PointMode`).
// Port of: include/core/SkCanvas.h#L1295-L1299 (chrome/m156)
#[doc(alias = "SkCanvas::PointMode")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
#[repr(i32)]
pub enum PointMode {
    /// Draws each point separately (`kPoints_PointMode`).
    Points = 0,
    /// Draws each pair of points as a line segment (`kLines_PointMode`).
    Lines = 1,
    /// Draws the array of points as a polyline (`kPolygon_PointMode`).
    Polygon = 2,
}

// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/fontations/src/verbs_points_pen.rs (chrome/m156). The pen writes into a
// `PathBuilder` rather than a verb and point array; the verbs it emits are the same.

//! The outline pen that turns a skrifa glyph into a path (`VerbsPointsPen`).

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skrifa::{
    GlyphId,
    outline::{DrawSettings, OutlinePen},
    prelude::Size,
};

use super::base::{BridgeNormalizedCoords, BridgeOutlineCollection};
use super::hinting::BridgeHintingInstance;

/// The scaler metrics that drawing a glyph reports (`BridgeScalerMetrics`).
// Port of: src/ports/fontations/src/ffi.rs (BridgeScalerMetrics, chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BridgeScalerMetrics {
    /// The glyph has overlapping contours or components.
    pub has_overlaps: bool,
    /// The scaler adjusted the advance width.
    pub has_adjusted_advance: bool,
    /// The adjusted advance width, when `has_adjusted_advance` is set.
    pub adjusted_advance: f32,
}

/// Records the outline of a glyph in a `PathBuilder`, with the y axis flipped (Skia's y points
/// down, the font's up). Consecutive points that repeat the current point are dropped.
// Port of: src/ports/fontations/src/verbs_points_pen.rs#L19-L68 (chrome/m156)
struct VerbsPointsPen<'a> {
    path: &'a mut PathBuilder,
    started: bool,
    current: Point,
}

impl<'a> VerbsPointsPen<'a> {
    // Port of: src/ports/fontations/src/verbs_points_pen.rs#L42-L54 (chrome/m156)
    fn new(path: &'a mut PathBuilder) -> Self {
        // The C++ pen clears the verb and point arrays here.
        path.reset();
        Self {
            path,
            started: false,
            current: Point::default(),
        }
    }

    // Port of: src/ports/fontations/src/verbs_points_pen.rs#L56-L63 (chrome/m156)
    fn going_to(&mut self, point: Point) {
        if !self.started {
            self.started = true;
            // The C++ pen pushes a MoveTo to the current point here.
            self.path.move_to(self.current);
        }
        self.current = point;
    }

    // Port of: src/ports/fontations/src/verbs_points_pen.rs#L65-L67 (chrome/m156)
    fn current_is_not(&self, point: Point) -> bool {
        self.current != point
    }
}

// Port of: src/ports/fontations/src/verbs_points_pen.rs#L70-L122 (chrome/m156)
impl OutlinePen for VerbsPointsPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let pt0 = Point::new(x, -y);
        if self.started {
            OutlinePen::close(self);
            self.started = false;
        }
        self.current = pt0;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let pt0 = Point::new(x, -y);
        if self.current_is_not(pt0) {
            self.going_to(pt0);
            self.path.line_to(pt0);
        }
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let pt0 = Point::new(cx0, -cy0);
        let pt1 = Point::new(x, -y);
        if self.current_is_not(pt0) || self.current_is_not(pt1) {
            self.going_to(pt1);
            self.path.quad_to(pt0, pt1);
        }
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let pt0 = Point::new(cx0, -cy0);
        let pt1 = Point::new(cx1, -cy1);
        let pt2 = Point::new(x, -y);
        if self.current_is_not(pt0) || self.current_is_not(pt1) || self.current_is_not(pt2) {
            self.going_to(pt2);
            self.path.cubic_to(pt0, pt1, pt2);
        }
    }

    fn close(&mut self) {
        // The C++ pen closes only after a verb that starts or continues a contour, and
        // `PathBuilder::close` does the same for the verbs this pen writes.
        self.path.close();
    }
}

/// Port of `get_path_verbs_points`: draws the glyph into `path` (after clearing it), and returns
/// the scaler metrics, or `None` when the glyph is missing or the draw fails.
///
/// The C++ version returns verb and point arrays that the caller builds into an `SkPath`; here
/// the caller's `PathBuilder` is the path.
// Port of: src/ports/fontations/src/verbs_points_pen.rs#L126-L161 (chrome/m156)
#[must_use]
pub fn get_path_verbs_points(
    outlines: &BridgeOutlineCollection<'_>,
    glyph_id: u16,
    size: f32,
    coords: &BridgeNormalizedCoords,
    hinting_instance: &BridgeHintingInstance,
    path: &mut PathBuilder,
) -> Option<BridgeScalerMetrics> {
    outlines.0.as_ref().and_then(|outlines| {
        let glyph = outlines.get(GlyphId::from(glyph_id))?;

        let draw_settings = match &hinting_instance.0 {
            Some(instance) => DrawSettings::hinted(instance, false),
            _ => DrawSettings::unhinted(Size::new(size), &coords.normalized_coords),
        };

        let mut verbs_points_pen = VerbsPointsPen::new(path);
        match glyph.draw(draw_settings, &mut verbs_points_pen) {
            Err(_) => None,
            Ok(metrics) => Some(BridgeScalerMetrics {
                has_overlaps: metrics.has_overlaps,
                has_adjusted_advance: metrics.advance_width.is_some(),
                adjusted_advance: metrics.advance_width.unwrap_or(0.0),
            }),
        }
    })
}

// Port of: src/ports/fontations/src/verbs_points_pen.rs#L163-L166 (chrome/m156)
// `shrink_verbs_points_if_needed` has no counterpart: it trimmed the reserved capacity of the
// arrays that the C++ bridge shared with `SkTypeface_Fontations`. The caller owns the
// `PathBuilder` here, so there is no shared buffer to trim.

#[cfg(test)]
mod tests {
    //! Draws a real glyph through the pen. The font is read from Skia's `resources/fonts`; the
    //! test skips when that directory is missing.

    use std::path::PathBuf;

    use skia_rust_core::path_builder::PathBuilder;
    use skia_rust_core::path_types::PathVerb;

    use super::get_path_verbs_points;
    use crate::ports::fontations::base::{
        get_outline_collection, lookup_glyph_or_zero, make_font_ref, make_mapping_index,
        resolve_into_normalized_coords,
    };
    use crate::ports::fontations::hinting::no_hinting_instance;

    #[test]
    fn draws_h_outline_with_flipped_y() {
        let dir = match std::env::var_os("SKIA_RESOURCES") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("third_party")
                .join("skia")
                .join("resources"),
        };
        let file = dir.join("fonts").join("Roboto-Regular.ttf");
        let Ok(bytes) = std::fs::read(&file) else {
            eprintln!("todo: skipping, missing Skia resource {}", file.display());
            return;
        };

        let font_ref = make_font_ref(&bytes, 0);
        let mapping = make_mapping_index(&font_ref).expect("valid font");
        let mut glyph = [0_u16; 1];
        lookup_glyph_or_zero(&font_ref, &mapping, &[u32::from('H')], &mut glyph);
        assert_ne!(glyph[0], 0, "Roboto has an H");

        let outlines = get_outline_collection(&font_ref);
        let coords = resolve_into_normalized_coords(&font_ref, &[]);
        let hinting = no_hinting_instance();
        let mut builder = PathBuilder::new();
        let metrics =
            get_path_verbs_points(&outlines, glyph[0], 2048.0, &coords, &hinting, &mut builder)
                .expect("the H draws");
        assert!(!metrics.has_overlaps);

        let verbs = builder.verbs();
        assert_eq!(verbs.first(), Some(&PathVerb::Move));
        assert_eq!(verbs.last(), Some(&PathVerb::Close));
        // The pen flips y: the glyph is above the baseline, so every point has y <= 0.
        assert!(builder.points().iter().all(|p| p.y <= 0.0));
        // An H is one contour of straight edges: no curves.
        assert!(!verbs.contains(&PathVerb::Cubic));
        assert!(!verbs.contains(&PathVerb::Quad));
    }
}

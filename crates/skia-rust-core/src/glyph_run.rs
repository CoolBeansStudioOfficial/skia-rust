// Copyright 2018 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/GlyphRun.{h,cpp} (chrome/m156), the text-to-runs half.
//
// Not ported yet: `GlyphRunList::{uniqueID, canCache, makeBlob, temporaryShuntBlobNotifyAddedToCache}`
// and `GlyphRunBuilder::blobToGlyphRunList`. They need `SkTextBlob` (T15a), which adds them.
//
// Layout differences from C++, with the same results: a `GlyphRun` owns its arrays instead of
// viewing spans of the builder's buffers, and the builder does not pool its buffers (canvas calls
// get a fresh builder). `GlyphRunList` borrows the builder's storage, or owns a single run.

//! `sktext::GlyphRun`, `GlyphRunList` and `GlyphRunBuilder`: the runs of glyphs that a text draw
//! turns into before the device paints them.

use std::borrow::Cow;

use crate::font::Font;
use crate::font_priv::get_font_bounds;
use crate::font_types::{GlyphId, TextEncoding};
use crate::glyph::Glyph;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::point::{Point, Vector};
use crate::rect::Rect;
use crate::rsxform::RSXform;
use crate::scalar::scalar;
use crate::scaler_context::ScalerContextBuildFlags;
use crate::strike_spec::{BulkGlyphMetrics, StrikeSpec};

/// One run of glyphs with one font: the glyph ids, their positions and, for `RSXform` runs, the
/// scale-rotation of each glyph (`sktext::GlyphRun`).
#[doc(alias = "sktext::GlyphRun")]
#[derive(Clone, Debug)]
pub struct GlyphRun {
    /// The glyph ids (`fSource`'s `get<0>`).
    glyph_ids: Vec<GlyphId>,
    /// The position of each glyph (`fSource`'s `get<1>`).
    positions: Vec<Point>,
    /// The original UTF-8 text when the run came from a text blob (empty otherwise).
    text: Vec<u8>,
    /// The original clusters when the run came from a text blob (empty otherwise).
    clusters: Vec<u32>,
    /// The scale and rotation of each glyph of an `RSXform` run (empty otherwise).
    scaled_rotations: Vec<Vector>,
    /// The font for this run, with glyph encoding and left alignment (`fFont`).
    font: Font,
}

impl GlyphRun {
    /// `GlyphRun(font, positions, glyphIDs, text, clusters, scaledRotations)`. The glyph and
    /// position arrays have equal length, as `SkMakeZip` requires.
    // Port of: src/text/GlyphRun.cpp#L26-L36 (chrome/m156)
    #[must_use]
    pub fn new(
        font: Font,
        positions: Vec<Point>,
        glyph_ids: Vec<GlyphId>,
        text: Vec<u8>,
        clusters: Vec<u32>,
        scaled_rotations: Vec<Vector>,
    ) -> Self {
        debug_assert_eq!(glyph_ids.len(), positions.len());
        Self {
            glyph_ids,
            positions,
            text,
            clusters,
            scaled_rotations,
            font,
        }
    }

    /// `GlyphRun(const GlyphRun& that, const SkFont& font)`: the same glyphs with another font.
    /// As in C++, the scaled rotations are not copied (`fScaledRotations` is left empty).
    // Port of: src/text/GlyphRun.cpp#L38-L43 (chrome/m156)
    #[must_use]
    pub fn with_font(&self, font: Font) -> Self {
        Self {
            glyph_ids: self.glyph_ids.clone(),
            positions: self.positions.clone(),
            text: self.text.clone(),
            clusters: self.clusters.clone(),
            scaled_rotations: Vec::new(),
            font,
        }
    }

    /// `runSize()`: the number of glyphs.
    // Port of: src/text/GlyphRun.h#L27 (chrome/m156)
    #[must_use]
    pub fn run_size(&self) -> usize {
        self.glyph_ids.len()
    }

    /// `glyphsIDs()`.
    // Port of: src/text/GlyphRun.h#L29 (chrome/m156)
    #[must_use]
    pub fn glyph_ids(&self) -> &[GlyphId] {
        &self.glyph_ids
    }

    /// `positions()`.
    // Port of: src/text/GlyphRun.h#L28 (chrome/m156)
    #[must_use]
    pub fn positions(&self) -> &[Point] {
        &self.positions
    }

    /// `source()`: the glyph id and position of each glyph (`SkZip`).
    // Port of: src/text/GlyphRun.h#L30 (chrome/m156)
    pub fn source(&self) -> impl Iterator<Item = (GlyphId, Point)> + '_ {
        self.glyph_ids
            .iter()
            .copied()
            .zip(self.positions.iter().copied())
    }

    /// `font()`.
    // Port of: src/text/GlyphRun.h#L31 (chrome/m156)
    #[must_use]
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// `clusters()`.
    // Port of: src/text/GlyphRun.h#L32 (chrome/m156)
    #[must_use]
    pub fn clusters(&self) -> &[u32] {
        &self.clusters
    }

    /// `text()`.
    // Port of: src/text/GlyphRun.h#L33 (chrome/m156)
    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.text
    }

    /// `scaledRotations()`.
    // Port of: src/text/GlyphRun.h#L34 (chrome/m156)
    #[must_use]
    pub fn scaled_rotations(&self) -> &[Vector] {
        &self.scaled_rotations
    }
}

/// The runs of one draw, with the bounds of their glyphs and the origin they are drawn at
/// (`sktext::GlyphRunList`).
#[doc(alias = "sktext::GlyphRunList")]
#[derive(Clone, Debug)]
pub struct GlyphRunList<'a> {
    /// `fGlyphRuns`: borrowed from a builder, or owned when made from a single run.
    runs: Cow<'a, [GlyphRun]>,
    /// `fSourceBounds`: the bounds of the glyphs, relative to the origin.
    source_bounds: Rect,
    /// `fOrigin`.
    origin: Point,
}

impl<'a> GlyphRunList<'a> {
    /// `GlyphRunList(blob, bounds, origin, glyphRunList, builder)` without a blob.
    // Port of: src/text/GlyphRun.cpp#L45-L54 (chrome/m156)
    #[must_use]
    pub fn new(runs: Cow<'a, [GlyphRun]>, source_bounds: Rect, origin: Point) -> Self {
        Self {
            runs,
            source_bounds,
            origin,
        }
    }

    /// `runCount()`.
    // Port of: src/text/GlyphRun.h#L62 (chrome/m156)
    #[must_use]
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// The runs, in order (`begin()`/`end()`).
    // Port of: src/text/GlyphRun.h#L95-L100 (chrome/m156)
    #[must_use]
    pub fn runs(&self) -> &[GlyphRun] {
        &self.runs
    }

    /// `size()`/`empty()`.
    // Port of: src/text/GlyphRun.h#L101-L102 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// `totalGlyphCount()`.
    // Port of: src/text/GlyphRun.h#L64-L70 (chrome/m156)
    #[must_use]
    pub fn total_glyph_count(&self) -> usize {
        self.runs.iter().map(GlyphRun::run_size).sum()
    }

    /// `maxGlyphRunSize()`.
    // Port of: src/text/GlyphRun.h#L71-L78 (chrome/m156)
    #[must_use]
    pub fn max_glyph_run_size(&self) -> usize {
        self.runs.iter().map(GlyphRun::run_size).max().unwrap_or(0)
    }

    /// `hasRSXForm()`: whether any run has scaled rotations.
    // Port of: src/text/GlyphRun.h#L79-L86 (chrome/m156)
    #[must_use]
    pub fn has_rsxform(&self) -> bool {
        self.runs
            .iter()
            .any(|run| !run.scaled_rotations().is_empty())
    }

    /// `anyRunsLCD()`: whether any run uses subpixel (LCD) edging.
    // Port of: src/text/GlyphRun.cpp#L71-L78 (chrome/m156)
    #[must_use]
    pub fn any_runs_lcd(&self) -> bool {
        self.runs
            .iter()
            .any(|run| run.font().edging() == crate::font::Edging::SubpixelAntiAlias)
    }

    /// `origin()`.
    // Port of: src/text/GlyphRun.h#L108 (chrome/m156)
    #[must_use]
    pub fn origin(&self) -> Point {
        self.origin
    }

    /// `sourceBounds()`: the glyph bounds, relative to the origin.
    // Port of: src/text/GlyphRun.h#L109 (chrome/m156)
    #[must_use]
    pub fn source_bounds(&self) -> Rect {
        self.source_bounds
    }

    /// `sourceBoundsWithOrigin()`: the glyph bounds in the canvas' coordinates.
    // Port of: src/text/GlyphRun.h#L110 (chrome/m156)
    #[must_use]
    pub fn source_bounds_with_origin(&self) -> Rect {
        let mut bounds = self.source_bounds;
        bounds.offset(self.origin);
        bounds
    }
}

/// Builds the glyph runs of a draw (`sktext::GlyphRunBuilder`).
#[doc(alias = "sktext::GlyphRunBuilder")]
#[derive(Debug, Default)]
pub struct GlyphRunBuilder {
    /// `fGlyphRunListStorage`: the runs of the last text draw.
    storage: Vec<GlyphRun>,
}

impl GlyphRunBuilder {
    /// A builder with no runs.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `makeGlyphRunList(run, paint, origin)`: a list holding one run.
    // Port of: src/text/GlyphRun.cpp#L187-L192 (chrome/m156)
    #[must_use]
    pub fn make_glyph_run_list(
        &self,
        run: GlyphRun,
        paint: &Paint,
        origin: Point,
    ) -> GlyphRunList<'static> {
        let bounds = glyphrun_source_bounds(
            run.font(),
            paint,
            run.glyph_ids(),
            run.positions(),
            run.scaled_rotations(),
        );
        GlyphRunList::new(Cow::Owned(vec![run]), bounds, origin)
    }

    /// `textToGlyphRunList(font, paint, bytes, byteLength, origin, encoding)`: the runs of
    /// `bytes` in `encoding`, with one run and glyphs at the advances of the font.
    // Port of: src/text/GlyphRun.cpp#L209-L229 (chrome/m156)
    #[must_use]
    pub fn text_to_glyph_run_list(
        &mut self,
        font: &Font,
        paint: &Paint,
        bytes: &[u8],
        origin: Point,
        encoding: TextEncoding,
    ) -> GlyphRunList<'_> {
        let glyph_ids = text_to_glyph_ids(font, bytes, encoding);
        self.prepare_buffers();
        let mut bounds = Rect::default();
        if !glyph_ids.is_empty() {
            let positions = draw_text_positions(font, &glyph_ids, Point::default());
            self.make_glyph_run(
                font,
                glyph_ids,
                positions,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            let run = &self.storage[0];
            bounds = glyphrun_source_bounds(
                run.font(),
                paint,
                run.glyph_ids(),
                run.positions(),
                run.scaled_rotations(),
            );
        }
        self.set_glyph_run_list(bounds, origin)
    }

    /// `convertRSXForm(xforms)`: the positions and scale-rotations of `RSXform`s (`(tx, ty)` and
    /// `(scos, ssin)`).
    // Port of: src/text/GlyphRun.cpp#L291-L303 (chrome/m156)
    #[must_use]
    pub fn convert_rsxform(xforms: &[RSXform]) -> (Vec<Point>, Vec<Vector>) {
        xforms
            .iter()
            .map(|x| (Point::new(x.tx, x.ty), Point::new(x.s_cos, x.s_sin)))
            .unzip()
    }

    /// `empty()`: whether the last draw made no runs.
    // Port of: src/text/GlyphRun.h#L168 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// `prepareBuffers`: forgets the runs of the previous draw.
    // Port of: src/text/GlyphRun.cpp#L320-L332 (chrome/m156), the storage reset
    fn prepare_buffers(&mut self) {
        self.storage.clear();
    }

    /// `makeGlyphRun`: adds a run unless it has no glyphs.
    // Port of: src/text/GlyphRun.cpp#L350-L368 (chrome/m156)
    fn make_glyph_run(
        &mut self,
        font: &Font,
        glyph_ids: Vec<GlyphId>,
        positions: Vec<Point>,
        text: Vec<u8>,
        clusters: Vec<u32>,
        scaled_rotations: Vec<Vector>,
    ) {
        if !glyph_ids.is_empty() {
            self.storage.push(GlyphRun::new(
                font.clone(),
                positions,
                glyph_ids,
                text,
                clusters,
                scaled_rotations,
            ));
        }
    }

    /// `setGlyphRunList(blob, bounds, origin)`, without a blob.
    // Port of: src/text/GlyphRun.cpp#L369-L373 (chrome/m156)
    fn set_glyph_run_list(&self, bounds: Rect, origin: Point) -> GlyphRunList<'_> {
        GlyphRunList::new(Cow::Borrowed(&self.storage), bounds, origin)
    }
}

/// `GlyphRunBuilder::textToGlyphIDs`: the glyphs of `bytes`. Glyph-id text is read as native
/// `u16`s, and a trailing odd byte is ignored, as C++ does.
// Port of: src/text/GlyphRun.cpp#L334-L348 (chrome/m156)
fn text_to_glyph_ids(font: &Font, bytes: &[u8], encoding: TextEncoding) -> Vec<GlyphId> {
    if encoding == TextEncoding::GlyphId {
        return bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| GlyphId::from_ne_bytes(*pair))
            .collect();
    }
    let count = font.count_text(bytes, encoding);
    if count == 0 {
        return Vec::new();
    }
    let mut glyph_ids = vec![0; count];
    font.text_to_glyphs(bytes, encoding, &mut glyph_ids);
    glyph_ids
}

/// `draw_text_positions`: each glyph's position, from the advances of the glyphs before it.
// Port of: src/text/GlyphRun.cpp#L194-L207 (chrome/m156)
fn draw_text_positions(font: &Font, glyph_ids: &[GlyphId], origin: Point) -> Vec<Point> {
    // MakeWithNoDevice without a paint has no effects, so it always has a descriptor.
    let spec = StrikeSpec::make_with_no_device(
        font,
        None,
        ScalerContextBuildFlags::FAKE_GAMMA_AND_BOOST_CONTRAST,
    )
    .expect("a spec without a paint has no effects, so it has a descriptor");
    let glyphs = BulkGlyphMetrics::new(&spec).glyphs(glyph_ids);
    let mut positions = Vec::with_capacity(glyph_ids.len());
    let mut end_of_last_glyph = origin;
    for glyph in &glyphs {
        positions.push(end_of_last_glyph);
        end_of_last_glyph += glyph.advance_vector();
    }
    positions
}

/// `glyphrun_source_bounds`: the bounds of the glyphs of a run, relative to its origin. With
/// empty font bounds the glyphs' own bounds are used, measured with the canonicalized strike.
///
/// Mirrors C++ exactly, including its `RSXform` branches, which build the transform as
/// `SkRSXform{pos.x(), pos.y(), scaleRotate.x(), scaleRotate.y()}`: the position fills
/// `fSCos`/`fSSin` and the scale-rotation fills `fTx`/`fTy`.
///
/// # Panics
///
/// If the paint has a path effect or mask filter (the strike descriptor for it is not ported).
// Port of: src/text/GlyphRun.cpp#L118-L185 (chrome/m156)
fn glyphrun_source_bounds(
    font: &Font,
    paint: &Paint,
    glyph_ids: &[GlyphId],
    positions: &[Point],
    scaled_rotations: &[Vector],
) -> Rect {
    debug_assert_ne!(glyph_ids.len(), 0);
    let font_bounds = get_font_bounds(font);
    if font_bounds.is_empty() {
        // Empty font bounds are likely a font bug. TightBounds has a better chance of producing
        // useful results in this case.
        let (strike_spec, strike_to_source_scale) =
            StrikeSpec::make_canonicalized(font, Some(paint)).expect(
                "a path effect or mask filter needs its descriptor entry, which is not ported yet",
            );
        let glyphs = BulkGlyphMetrics::new(&strike_spec).glyphs(glyph_ids);
        return tight_source_bounds(positions, scaled_rotations, &glyphs, strike_to_source_scale);
    }

    // Use conservative bounds. All glyph have a box of fontBounds size.
    if scaled_rotations.is_empty() {
        let mut bounds = Rect::bounds_or_empty(positions);
        bounds.left += font_bounds.left;
        bounds.top += font_bounds.top;
        bounds.right += font_bounds.right;
        bounds.bottom += font_bounds.bottom;
        bounds
    } else {
        // RSXForm case glyphs can be any scale or rotation.
        let mut bounds = Rect::default();
        for (pos, scale_rotate) in positions.iter().zip(scaled_rotations) {
            let mut xform = Matrix::default();
            xform.set_rsxform(&rsxform_as_cpp(*pos, *scale_rotate));
            bounds.join(xform.map_rect(font_bounds).0);
        }
        bounds
    }
}

/// The bounds of glyphs measured from their metrics (the empty-font-bounds branch of
/// `glyphrun_source_bounds`).
// Port of: src/text/GlyphRun.cpp#L129-L166 (chrome/m156)
fn tight_source_bounds(
    positions: &[Point],
    scaled_rotations: &[Vector],
    glyphs: &[Glyph],
    strike_to_source_scale: scalar,
) -> Rect {
    let mut bounds = Rect::default();
    if scaled_rotations.is_empty() {
        // No RSXForm data - glyphs x/y aligned.
        for (pos, glyph) in positions.iter().zip(glyphs) {
            let r = glyph.rect();
            if !r.is_empty() {
                let scale = strike_to_source_scale;
                bounds.join(Rect::from_ltrb(
                    r.left * scale + pos.x,
                    r.top * scale + pos.y,
                    r.right * scale + pos.x,
                    r.bottom * scale + pos.y,
                ));
            }
        }
    } else {
        // RSXForm - glyphs can be any scale or rotation.
        for ((pos, scale_rotate), glyph) in positions.iter().zip(scaled_rotations).zip(glyphs) {
            if !glyph.rect().is_empty() {
                let mut xform = Matrix::default();
                xform.set_rsxform(&rsxform_as_cpp(*pos, *scale_rotate));
                xform.pre_scale((strike_to_source_scale, strike_to_source_scale), None);
                bounds.join(xform.map_rect(glyph.rect()).0);
            }
        }
    }
    bounds
}

/// `SkRSXform{pos.x(), pos.y(), scaleRotate.x(), scaleRotate.y()}` as the bounds code writes it.
/// The field order is C++'s, not the `RSXform`'s meaning (see [`glyphrun_source_bounds`]).
// Port of: src/text/GlyphRun.cpp#L158 and #L180 (chrome/m156)
fn rsxform_as_cpp(pos: Point, scale_rotate: Vector) -> RSXform {
    RSXform::new(pos.x, pos.y, scale_rotate.x, scale_rotate.y)
}

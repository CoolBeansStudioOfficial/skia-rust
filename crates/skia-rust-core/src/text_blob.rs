// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTextBlob.cpp and src/core/SkTextBlobPriv.h (chrome/m156), the
// builder, the iterators and the `from_*` constructors. Not ported yet: `get_intercepts`
// (T15b), serialization (T15b) and the purge delegate (GPU only, Phase 6).
//
// Layout differences from C++, with the same results: a run owns its glyph, position, text and
// cluster arrays as `Vec`s instead of a byte arena. Runs stay in order in the builder, so
// `mergeRun` and the bounds code see the same run sequence as C++ does.

//! `SkTextBlob`: glyphs and positions, grouped into runs that share a font, ready to draw with
//! `Canvas::draw_text_blob`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::data::Data;
use crate::font::Font;
use crate::font_priv::get_font_bounds;
use crate::font_types::{GlyphId, TextEncoding};
use crate::glyph_intercepts::glyph_run_intercepts;
use crate::glyph_run::GlyphRunBuilder;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::point::Point;
use crate::read_buffer::ReadBuffer;
use crate::rect::Rect;
use crate::rsxform::RSXform;
use crate::scalar::scalar;
use crate::serial_procs::{DeserialProcs, SerialProcs};
use crate::typeface::Typeface;
use crate::write_buffer::BinaryWriteBuffer;

/// How the glyphs of a run are positioned (`SkTextBlob::GlyphPositioning`).
#[doc(alias = "SkTextBlob::GlyphPositioning")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphPositioning {
    /// The default advances: no positions are stored (`kDefault_Positioning`).
    Default,
    /// One x position per glyph, sharing the run's y (`kHorizontal_Positioning`).
    Horizontal,
    /// One point per glyph (`kFull_Positioning`).
    Full,
    /// One `RSXform` per glyph (`kRSXform_Positioning`).
    RSXform,
}

impl GlyphPositioning {
    /// `ScalarsPerGlyph(pos)`: the number of scalars a glyph takes in the position buffer.
    // Port of: src/core/SkTextBlob.cpp#L199-L208 (chrome/m156)
    #[must_use]
    pub fn scalars_per_glyph(self) -> usize {
        match self {
            Self::Default => 0,
            Self::Horizontal => 1,
            Self::Full => 2,
            Self::RSXform => 4,
        }
    }
}

/// The positions of a run, typed by its positioning. C++ keeps them in one scalar buffer; the
/// typed vectors hold the same numbers.
#[derive(Clone, Debug)]
enum Positions {
    /// `kDefault_Positioning`: no positions.
    None,
    /// `kHorizontal_Positioning`: one x per glyph.
    Horizontal(Vec<scalar>),
    /// `kFull_Positioning`: one point per glyph.
    Full(Vec<Point>),
    /// `kRSXform_Positioning`: one transform per glyph.
    RSXform(Vec<RSXform>),
}

impl Positions {
    /// An empty buffer of `positioning`.
    fn new(positioning: GlyphPositioning) -> Self {
        match positioning {
            GlyphPositioning::Default => Self::None,
            GlyphPositioning::Horizontal => Self::Horizontal(Vec::new()),
            GlyphPositioning::Full => Self::Full(Vec::new()),
            GlyphPositioning::RSXform => Self::RSXform(Vec::new()),
        }
    }

    /// The positioning this buffer holds.
    fn positioning(&self) -> GlyphPositioning {
        match self {
            Self::None => GlyphPositioning::Default,
            Self::Horizontal(_) => GlyphPositioning::Horizontal,
            Self::Full(_) => GlyphPositioning::Full,
            Self::RSXform(_) => GlyphPositioning::RSXform,
        }
    }

    /// Appends `count` zeroed positions (`RunRecord::grow`'s position part).
    fn grow(&mut self, count: usize) {
        match self {
            Self::None => {}
            Self::Horizontal(v) => v.resize(v.len() + count, 0.0),
            Self::Full(v) => v.resize(v.len() + count, Point::default()),
            Self::RSXform(v) => v.resize(v.len() + count, RSXform::default()),
        }
    }
}

/// One run of a text blob (`SkTextBlob::RunRecord`).
#[derive(Clone, Debug)]
struct RunRecord {
    /// The font of every glyph of the run (`fFont`).
    font: Font,
    /// The offset added to the positions of a non-default run (`fOffset`).
    offset: Point,
    /// The glyph ids (`glyphBuffer`).
    glyphs: Vec<GlyphId>,
    /// The positions, typed by the run's positioning (`posBuffer`).
    positions: Positions,
    /// The UTF-8 text of an extended run (`textBuffer`); empty when the run is not extended.
    text: Vec<u8>,
    /// The cluster of each glyph of an extended run (`clusterBuffer`).
    clusters: Vec<u32>,
}

impl RunRecord {
    /// A run of `count` glyphs with the given positioning, all glyph and position values zero
    /// (`RunRecord::RunRecord`).
    // Port of: src/core/SkTextBlob.cpp#L50-L70 (chrome/m156), the constructor and StorageSize
    fn new(
        font: Font,
        offset: Point,
        count: usize,
        text_size: usize,
        positioning: GlyphPositioning,
    ) -> Self {
        let mut positions = Positions::new(positioning);
        positions.grow(count);
        Self {
            font,
            offset,
            glyphs: vec![0; count],
            positions,
            text: vec![0; text_size],
            clusters: if text_size == 0 {
                Vec::new()
            } else {
                vec![0; count]
            },
        }
    }

    /// `glyphCount()`.
    // Port of: src/core/SkTextBlob.h (RunRecord::glyphCount, chrome/m156)
    fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// `positioning()`.
    fn positioning(&self) -> GlyphPositioning {
        self.positions.positioning()
    }

    /// `isExtended()`: the run carries UTF-8 text and clusters.
    // Port of: src/core/SkTextBlob.cpp#L92-L108 (chrome/m156), the isExtended test
    fn is_extended(&self) -> bool {
        !self.text.is_empty()
    }
}

/// The shared, immutable part of a text blob (`SkTextBlob`).
#[derive(Debug)]
struct TextBlobData {
    /// The bounds of the glyphs, relative to the blob origin (`fBounds`).
    bounds: Rect,
    /// `fUniqueID`.
    unique_id: u32,
    /// The runs, in draw order.
    runs: Vec<RunRecord>,
}

/// Process-wide source of unique ids (`next_id`); zero is never handed out.
// Port of: src/core/SkTextBlob.cpp#L146-L153 (chrome/m156)
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// `next_id()`: the next unique id, skipping the invalid id 0.
// Port of: src/core/SkTextBlob.cpp#L146-L153 (chrome/m156)
fn next_id() -> u32 {
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

/// An immutable text blob (`SkTextBlob`): runs of glyphs with their positions and bounds.
/// Cloning shares the blob.
#[doc(alias = "SkTextBlob")]
#[derive(Clone, Debug)]
pub struct TextBlob(Arc<TextBlobData>);

impl TextBlob {
    /// `bounds()`: the bounds of the glyphs, relative to the blob origin.
    // Port of: include/core/SkTextBlob.h (chrome/m156), bounds()
    #[must_use]
    pub fn bounds(&self) -> &Rect {
        &self.0.bounds
    }

    /// `uniqueID()`.
    // Port of: include/core/SkTextBlob.h (chrome/m156), uniqueID()
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.0.unique_id
    }

    /// `SkTextBlob::MakeFromString(str, font, kUTF8)`: a blob of the UTF-8 string `text`, or
    /// `None` when it has no glyphs.
    // Port of: include/core/SkTextBlob.h (chrome/m156), MakeFromString
    #[doc(alias = "MakeFromString")]
    #[must_use]
    pub fn from_str(text: impl AsRef<str>, font: &Font) -> Option<Self> {
        Self::from_text(text.as_ref().as_bytes(), TextEncoding::UTF8, font)
    }

    /// `SkTextBlob::MakeFromText(text, font, encoding)`: a fully positioned blob of the glyphs of
    /// `text`, or `None` when there are none. The positions are promoted to points here, as C++
    /// does, so the bounds are paid for once.
    // Port of: src/core/SkTextBlob.cpp#L794-L807 (chrome/m156)
    #[doc(alias = "MakeFromText")]
    #[must_use]
    pub fn from_text(text: &[u8], encoding: TextEncoding, font: &Font) -> Option<Self> {
        let count = font.count_text(text, encoding);
        if count == 0 {
            return None;
        }
        let mut builder = TextBlobBuilder::new();
        let (glyphs, points) = builder.alloc_run_pos(font, count, None);
        font.text_to_glyphs(text, encoding, glyphs);
        font.get_pos(glyphs, points, Point::new(0.0, 0.0));
        builder.make()
    }

    /// `SkTextBlob::MakeFromPosText(text, byteLength, pos, font, encoding)`: a blob with the
    /// glyphs of `text` at `pos`, or `None` when there are no glyphs or too few positions.
    // Port of: src/core/SkTextBlob.cpp#L809-L821 (chrome/m156)
    #[doc(alias = "MakeFromPosText")]
    #[must_use]
    pub fn from_pos_text(
        text: &[u8],
        encoding: TextEncoding,
        pos: &[Point],
        font: &Font,
    ) -> Option<Self> {
        let count = font.count_text(text, encoding);
        if count == 0 || pos.len() < count {
            return None;
        }
        let mut builder = TextBlobBuilder::new();
        let (glyphs, points) = builder.alloc_run_pos(font, count, None);
        font.text_to_glyphs(text, encoding, glyphs);
        points.copy_from_slice(&pos[..count]);
        builder.make()
    }

    /// `SkTextBlob::MakeFromPosTextH(text, byteLength, xpos, constY, font, encoding)`: a blob with
    /// the glyphs of `text` at the x positions `xpos` on the baseline `const_y`, or `None` when
    /// there are no glyphs or too few positions.
    // Port of: src/core/SkTextBlob.cpp#L823-L835 (chrome/m156)
    #[doc(alias = "MakeFromPosTextH")]
    #[must_use]
    pub fn from_pos_text_h(
        text: &[u8],
        encoding: TextEncoding,
        xpos: &[scalar],
        const_y: scalar,
        font: &Font,
    ) -> Option<Self> {
        let count = font.count_text(text, encoding);
        if count == 0 || xpos.len() < count {
            return None;
        }
        let mut builder = TextBlobBuilder::new();
        let (glyphs, pos) = builder.alloc_run_pos_h(font, count, const_y, None);
        font.text_to_glyphs(text, encoding, glyphs);
        pos.copy_from_slice(&xpos[..count]);
        builder.make()
    }

    /// `SkTextBlob::MakeFromPosHGlyphs(glyphs, xpos, constY, font)`: a blob of `glyphs` at the x
    /// positions `xpos` on the baseline `const_y`.
    // Port of: include/core/SkTextBlob.h#L162-L166 (chrome/m156)
    #[doc(alias = "MakeFromPosHGlyphs")]
    #[must_use]
    pub fn from_pos_h_glyphs(
        glyphs: &[GlyphId],
        xpos: &[scalar],
        const_y: scalar,
        font: &Font,
    ) -> Option<Self> {
        Self::from_pos_text_h(
            &glyph_bytes(glyphs),
            TextEncoding::GlyphId,
            xpos,
            const_y,
            font,
        )
    }

    /// `SkTextBlob::MakeFromPosGlyphs(glyphs, pos, font)`: a blob of `glyphs` at `pos`.
    // Port of: include/core/SkTextBlob.h#L168-L171 (chrome/m156)
    #[doc(alias = "MakeFromPosGlyphs")]
    #[must_use]
    pub fn from_pos_glyphs(glyphs: &[GlyphId], pos: &[Point], font: &Font) -> Option<Self> {
        Self::from_pos_text(&glyph_bytes(glyphs), TextEncoding::GlyphId, pos, font)
    }

    /// `SkTextBlob::MakeFromRSXform(text, byteLength, xform, font, encoding)`: a blob with the
    /// glyphs of `text`, each placed by its `RSXform`, or `None` when there are no glyphs or too
    /// few transforms.
    // Port of: src/core/SkTextBlob.cpp#L837-L849 (chrome/m156)
    #[doc(alias = "MakeFromRSXform")]
    #[must_use]
    pub fn from_rsxform(
        text: &[u8],
        encoding: TextEncoding,
        xform: &[RSXform],
        font: &Font,
    ) -> Option<Self> {
        let count = font.count_text(text, encoding);
        if count == 0 || xform.len() < count {
            return None;
        }
        let mut builder = TextBlobBuilder::new();
        let (glyphs, xforms) = builder.alloc_run_rsxform(font, count);
        font.text_to_glyphs(text, encoding, glyphs);
        xforms.copy_from_slice(&xform[..count]);
        builder.make()
    }

    /// The x intervals where the horizontal band `bounds` (its top and bottom y) crosses the
    /// outlines of the glyphs, as pairs `[start, end]`. `RSXform` runs are ignored. `paint` gives
    /// the stroke and path effect that change the outlines (`getIntercepts`).
    // Port of: src/core/SkTextBlob.cpp#L933-L954 (chrome/m156)
    #[doc(alias = "getIntercepts")]
    #[must_use]
    pub fn get_intercepts(&self, bounds: [scalar; 2], paint: Option<&Paint>) -> Vec<scalar> {
        let paint = paint.cloned().unwrap_or_default();
        let mut builder = GlyphRunBuilder::new();
        let list = builder.blob_to_glyph_run_list(self, Point::new(0.0, 0.0));
        let mut intervals = Vec::new();
        for run in list.runs() {
            // Ignore RSXForm runs.
            if run.scaled_rotations().is_empty() {
                glyph_run_intercepts(run, &paint, bounds, &mut intervals);
            }
        }
        intervals
    }

    /// `SkTextBlob::Iter`: the runs of the blob, as glyph indices and typefaces.
    // Port of: src/core/SkTextBlob.cpp#L979-L1003 (chrome/m156)
    #[doc(alias = "Iter")]
    #[must_use]
    #[allow(clippy::iter_without_into_iter)] // mirrors SkTextBlob::Iter, a separate iterator type
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            runs: self.0.runs.iter(),
        }
    }

    /// `SkTextBlobRunIterator`: the runs of the blob with their full data.
    // Port of: src/core/SkTextBlob.cpp#L222-L225 (chrome/m156), the SkTextBlobRunIterator
    #[must_use]
    pub fn run_iter(&self) -> RunIterator<'_> {
        RunIterator {
            runs: &self.0.runs,
            index: 0,
        }
    }
}

/// One run as the blob iterator reports it (`SkTextBlob::Iter::Run`).
#[derive(Clone, Copy, Debug)]
pub struct Run<'a> {
    /// The typeface of the run's font (`fTypeface`).
    typeface: &'a Typeface,
    /// The glyph ids (`fGlyphIndices`), with `fGlyphCount` entries.
    pub glyph_indices: &'a [GlyphId],
}

impl Run<'_> {
    /// `fGlyphCount`: the number of glyphs.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyph_indices.len()
    }

    /// `fTypeface`: the typeface of the run.
    #[must_use]
    pub fn typeface(&self) -> &Typeface {
        self.typeface
    }
}

/// Iterates the runs of a blob (`SkTextBlob::Iter`).
#[doc(alias = "SkTextBlob::Iter")]
#[derive(Clone, Debug)]
pub struct Iter<'a> {
    runs: std::slice::Iter<'a, RunRecord>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Run<'a>;

    // Port of: src/core/SkTextBlob.cpp#L983-L1003 (chrome/m156), Iter::next
    fn next(&mut self) -> Option<Run<'a>> {
        self.runs.next().map(|run| Run {
            typeface: run.font.typeface(),
            glyph_indices: &run.glyphs,
        })
    }
}

/// Iterates the runs of a blob with their positions, text and clusters
/// (`SkTextBlobRunIterator`). Starts on the first run; check [`RunIterator::done`].
#[doc(alias = "SkTextBlobRunIterator")]
#[derive(Clone, Debug)]
pub struct RunIterator<'a> {
    runs: &'a [RunRecord],
    index: usize,
}

impl<'a> RunIterator<'a> {
    /// `done()`: whether every run has been visited.
    // Port of: src/core/SkTextBlob.cpp#L222-L225 (chrome/m156), the done test of the iterator
    #[must_use]
    pub fn done(&self) -> bool {
        self.index >= self.runs.len()
    }

    /// `next()`: moves to the following run.
    // Port of: src/core/SkTextBlob.cpp#L227-L234 (chrome/m156)
    pub fn next(&mut self) {
        if !self.done() {
            self.index += 1;
        }
    }

    /// The current run, or `None` when the iterator is done.
    fn current(&self) -> Option<&'a RunRecord> {
        self.runs.get(self.index)
    }

    /// `positioning()`.
    // Port of: src/core/SkTextBlob.cpp#L236-L248 (chrome/m156)
    #[must_use]
    pub fn positioning(&self) -> GlyphPositioning {
        self.current()
            .map_or(GlyphPositioning::Default, RunRecord::positioning)
    }

    /// `scalarsPerGlyph()`.
    // Port of: src/core/SkTextBlob.cpp#L250-L252 (chrome/m156)
    #[must_use]
    pub fn scalars_per_glyph(&self) -> usize {
        self.positioning().scalars_per_glyph()
    }

    /// `isLCD()`: the run uses subpixel edging.
    // Port of: src/core/SkTextBlob.cpp#L254-L256 (chrome/m156)
    #[must_use]
    pub fn is_lcd(&self) -> bool {
        self.current()
            .is_some_and(|run| run.font.edging() == crate::font::Edging::SubpixelAntiAlias)
    }

    /// `font()`.
    #[must_use]
    pub fn font(&self) -> Option<&'a Font> {
        self.current().map(|run| &run.font)
    }

    /// `glyphCount()`.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.current().map_or(0, RunRecord::glyph_count)
    }

    /// `glyphs()`: the glyph ids of the run.
    #[must_use]
    pub fn glyphs(&self) -> &'a [GlyphId] {
        self.current().map_or(&[], |run| run.glyphs.as_slice())
    }

    /// `offset()`: the offset of the run within the blob.
    #[must_use]
    pub fn offset(&self) -> Point {
        self.current().map_or(Point::default(), |run| run.offset)
    }

    /// `pos()`: the x positions of a horizontal run (empty for other positionings).
    #[must_use]
    pub fn pos(&self) -> &'a [scalar] {
        match self.current().map(|run| &run.positions) {
            Some(Positions::Horizontal(x)) => x,
            _ => &[],
        }
    }

    /// `points()`: the points of a full-positioned run (empty for other positionings).
    #[must_use]
    pub fn points(&self) -> &'a [Point] {
        match self.current().map(|run| &run.positions) {
            Some(Positions::Full(p)) => p,
            _ => &[],
        }
    }

    /// `xforms()`: the transforms of an `RSXform` run (empty for other positionings).
    #[must_use]
    pub fn xforms(&self) -> &'a [RSXform] {
        match self.current().map(|run| &run.positions) {
            Some(Positions::RSXform(x)) => x,
            _ => &[],
        }
    }

    /// `textSize()`: the length of the UTF-8 text of an extended run, zero otherwise.
    #[must_use]
    pub fn text_size(&self) -> usize {
        self.current().map_or(0, |run| run.text.len())
    }

    /// `text()`: the UTF-8 text of an extended run (empty otherwise).
    #[must_use]
    pub fn text(&self) -> &'a [u8] {
        self.current().map_or(&[], |run| run.text.as_slice())
    }

    /// `clusters()`: the cluster of each glyph of an extended run (empty otherwise).
    #[must_use]
    pub fn clusters(&self) -> &'a [u32] {
        self.current().map_or(&[], |run| run.clusters.as_slice())
    }
}

/// The bytes of `glyphs` in the native byte order of the glyph encoding (`kGlyphID`).
fn glyph_bytes(glyphs: &[GlyphId]) -> Vec<u8> {
    glyphs
        .iter()
        .flat_map(|glyph| glyph.to_ne_bytes())
        .collect()
}

/// Builds a text blob one run at a time (`SkTextBlobBuilder`). Each `alloc_run*` returns the
/// buffers to fill, which borrow the builder until the next call.
#[doc(alias = "SkTextBlobBuilder")]
#[derive(Debug, Default)]
pub struct TextBlobBuilder {
    /// The runs so far, in order (`fStorage` with `fRunCount` records).
    runs: Vec<RunRecord>,
    /// The bounds of the runs that had explicit bounds, joined with the deferred ones made by
    /// `update_deferred_bounds` (`fBounds`).
    bounds: Rect,
    /// Whether the last run's bounds are still to be computed (`fDeferredBounds`).
    deferred_bounds: bool,
}

impl TextBlobBuilder {
    /// An empty builder (`SkTextBlobBuilder()`).
    // Port of: src/core/SkTextBlob.cpp#L258-L265 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The buffers of a run with glyphs and a font: `alloc_run(font, count, x, y, bounds)`.
    /// The positions are the offset `(x, y)`, applied to the whole run.
    // Port of: src/core/SkTextBlob.cpp#L539-L544 (chrome/m156)
    #[doc(alias = "allocRun")]
    pub fn alloc_run(
        &mut self,
        font: &Font,
        count: usize,
        x: scalar,
        y: scalar,
        bounds: Option<&Rect>,
    ) -> &mut [GlyphId] {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::Default,
            count,
            0,
            Point::new(x, y),
            bounds,
        ) else {
            return &mut [];
        };
        &mut self.last_run_mut().glyphs[start..]
    }

    /// The buffers of a horizontal-positioned run: `alloc_run_pos_h(font, count, y, bounds)`.
    // Port of: src/core/SkTextBlob.cpp#L546-L551 (chrome/m156)
    #[doc(alias = "allocRunPosH")]
    pub fn alloc_run_pos_h(
        &mut self,
        font: &Font,
        count: usize,
        y: scalar,
        bounds: Option<&Rect>,
    ) -> (&mut [GlyphId], &mut [scalar]) {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::Horizontal,
            count,
            0,
            Point::new(0.0, y),
            bounds,
        ) else {
            return (&mut [], &mut []);
        };
        let run = self.last_run_mut();
        let Positions::Horizontal(pos) = &mut run.positions else {
            unreachable!("a horizontal run has horizontal positions");
        };
        (&mut run.glyphs[start..], &mut pos[start..])
    }

    /// The buffers of a full-positioned run: `alloc_run_pos(font, count, bounds)`.
    // Port of: src/core/SkTextBlob.cpp#L553-L558 (chrome/m156)
    #[doc(alias = "allocRunPos")]
    pub fn alloc_run_pos(
        &mut self,
        font: &Font,
        count: usize,
        bounds: Option<&Rect>,
    ) -> (&mut [GlyphId], &mut [Point]) {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::Full,
            count,
            0,
            Point::new(0.0, 0.0),
            bounds,
        ) else {
            return (&mut [], &mut []);
        };
        let run = self.last_run_mut();
        let Positions::Full(pos) = &mut run.positions else {
            unreachable!("a full run has point positions");
        };
        (&mut run.glyphs[start..], &mut pos[start..])
    }

    /// The buffers of an `RSXform` run: `alloc_run_rsxform(font, count)`. Its bounds are always
    /// computed from the transforms.
    // Port of: src/core/SkTextBlob.cpp#L560-L564 (chrome/m156)
    #[doc(alias = "allocRunRSXform")]
    pub fn alloc_run_rsxform(
        &mut self,
        font: &Font,
        count: usize,
    ) -> (&mut [GlyphId], &mut [RSXform]) {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::RSXform,
            count,
            0,
            Point::new(0.0, 0.0),
            None,
        ) else {
            return (&mut [], &mut []);
        };
        let run = self.last_run_mut();
        let Positions::RSXform(xforms) = &mut run.positions else {
            unreachable!("an RSXform run has transforms");
        };
        (&mut run.glyphs[start..], &mut xforms[start..])
    }

    /// The buffers of an extended run with a default positioning:
    /// `alloc_run_text(font, count, x, y, text_byte_count, bounds)`. The glyphs, the UTF-8 text and
    /// the clusters, one cluster per glyph.
    // Port of: src/core/SkTextBlob.cpp#L565-L577 (chrome/m156)
    #[doc(alias = "allocRunText")]
    pub fn alloc_run_text(
        &mut self,
        font: &Font,
        count: usize,
        offset: impl Into<Point>,
        text_byte_count: usize,
        bounds: Option<&Rect>,
    ) -> (&mut [GlyphId], &mut [u8], &mut [u32]) {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::Default,
            count,
            text_byte_count,
            offset.into(),
            bounds,
        ) else {
            return (&mut [], &mut [], &mut []);
        };
        let run = self.last_run_mut();
        (
            &mut run.glyphs[start..],
            run.text.as_mut_slice(),
            run.clusters.as_mut_slice(),
        )
    }

    /// The buffers of an extended horizontal-positioned run:
    /// `alloc_run_text_pos_h(font, count, y, text_byte_count, bounds)`.
    // Port of: src/core/SkTextBlob.cpp#L578-L591 (chrome/m156)
    #[doc(alias = "allocRunTextPosH")]
    pub fn alloc_run_text_pos_h(
        &mut self,
        font: &Font,
        count: usize,
        y: scalar,
        text_byte_count: usize,
        bounds: Option<&Rect>,
    ) -> (&mut [GlyphId], &mut [scalar], &mut [u8], &mut [u32]) {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::Horizontal,
            count,
            text_byte_count,
            Point::new(0.0, y),
            bounds,
        ) else {
            return (&mut [], &mut [], &mut [], &mut []);
        };
        let run = self.last_run_mut();
        let Positions::Horizontal(pos) = &mut run.positions else {
            unreachable!("a horizontal run has horizontal positions");
        };
        (
            &mut run.glyphs[start..],
            &mut pos[start..],
            run.text.as_mut_slice(),
            run.clusters.as_mut_slice(),
        )
    }

    /// The buffers of an extended full-positioned run:
    /// `alloc_run_text_pos(font, count, text_byte_count, bounds)`.
    // Port of: src/core/SkTextBlob.cpp#L592-L603 (chrome/m156)
    #[doc(alias = "allocRunTextPos")]
    pub fn alloc_run_text_pos(
        &mut self,
        font: &Font,
        count: usize,
        text_byte_count: usize,
        bounds: Option<&Rect>,
    ) -> (&mut [GlyphId], &mut [Point], &mut [u8], &mut [u32]) {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::Full,
            count,
            text_byte_count,
            Point::new(0.0, 0.0),
            bounds,
        ) else {
            return (&mut [], &mut [], &mut [], &mut []);
        };
        let run = self.last_run_mut();
        let Positions::Full(pos) = &mut run.positions else {
            unreachable!("a full run has point positions");
        };
        (
            &mut run.glyphs[start..],
            &mut pos[start..],
            run.text.as_mut_slice(),
            run.clusters.as_mut_slice(),
        )
    }

    /// The buffers of an extended `RSXform` run:
    /// `alloc_run_text_rsxform(font, count, text_byte_count, bounds)`.
    // Port of: src/core/SkTextBlob.cpp#L604-L616 (chrome/m156)
    #[doc(alias = "allocRunTextRSXform")]
    pub fn alloc_run_text_rsxform(
        &mut self,
        font: &Font,
        count: usize,
        text_byte_count: usize,
        bounds: Option<&Rect>,
    ) -> (&mut [GlyphId], &mut [RSXform], &mut [u8], &mut [u32]) {
        let Some(start) = self.alloc_internal(
            font,
            GlyphPositioning::RSXform,
            count,
            text_byte_count,
            Point::new(0.0, 0.0),
            bounds,
        ) else {
            return (&mut [], &mut [], &mut [], &mut []);
        };
        let run = self.last_run_mut();
        let Positions::RSXform(xforms) = &mut run.positions else {
            unreachable!("an RSXform run has transforms");
        };
        (
            &mut run.glyphs[start..],
            &mut xforms[start..],
            run.text.as_mut_slice(),
            run.clusters.as_mut_slice(),
        )
    }

    /// `make()`: the blob of the runs so far, or `None` when there are none. The builder is
    /// emptied, so it can build the next blob.
    // Port of: src/core/SkTextBlob.cpp#L617-L661 (chrome/m156)
    pub fn make(&mut self) -> Option<TextBlob> {
        if self.runs.is_empty() {
            // We don't instantiate empty blobs.
            return None;
        }
        self.update_deferred_bounds();
        let bounds = std::mem::take(&mut self.bounds);
        self.deferred_bounds = false;
        let runs = std::mem::take(&mut self.runs);
        Some(TextBlob(Arc::new(TextBlobData {
            bounds,
            unique_id: next_id(),
            runs,
        })))
    }

    /// The last run, which the `alloc_*` calls just added to or made.
    fn last_run_mut(&mut self) -> &mut RunRecord {
        self.runs
            .last_mut()
            .expect("an alloc call leaves a run to fill")
    }

    /// `allocInternal`: adds a run, or extends the last one when it can absorb the new glyphs.
    /// Returns the index of the first new glyph in the run that holds it, or `None` when the
    /// request is empty (C++ returns null buffers).
    // Port of: src/core/SkTextBlob.cpp#L488-L537 (chrome/m156)
    fn alloc_internal(
        &mut self,
        font: &Font,
        positioning: GlyphPositioning,
        count: usize,
        text_size: usize,
        offset: Point,
        bounds: Option<&Rect>,
    ) -> Option<usize> {
        if count == 0 {
            return None;
        }
        let mut start = None;
        if text_size == 0 {
            start = self.merge_run(font, positioning, count, offset);
        }
        let start = start.unwrap_or_else(|| {
            self.update_deferred_bounds();
            self.runs.push(RunRecord::new(
                font.clone(),
                offset,
                count,
                text_size,
                positioning,
            ));
            0
        });
        if !self.deferred_bounds {
            match bounds {
                Some(bounds) => self.bounds.join(bounds),
                None => self.deferred_bounds = true,
            }
        }
        Some(start)
    }

    /// `mergeRun`: appends `count` glyphs to the last run when that run has the same font and
    /// positioning, and (for horizontal runs) the same y offset. Returns the index of the first
    /// appended glyph, or `None` when the glyphs cannot merge.
    // Port of: src/core/SkTextBlob.cpp#L428-L486 (chrome/m156)
    #[allow(clippy::float_cmp)] // C++ compares the y offsets exactly (`run->offset().y() != offset.y()`)
    fn merge_run(
        &mut self,
        font: &Font,
        positioning: GlyphPositioning,
        count: usize,
        offset: Point,
    ) -> Option<usize> {
        let run = self.runs.last_mut()?;
        if run.is_extended() {
            return None;
        }
        if run.positioning() != positioning
            || run.font != *font
            || run.glyph_count().checked_add(count).is_none()
        {
            return None;
        }
        // We can merge same-font/same-positioning runs in the following cases:
        //   * fully positioned run following another fully positioned run
        //   * horizontally positioned run following another horizontally positioned run with the
        //     same y-offset
        if positioning != GlyphPositioning::Full
            && (positioning != GlyphPositioning::Horizontal || run.offset.y != offset.y)
        {
            return None;
        }
        let pre_merge_count = run.glyph_count();
        run.glyphs.resize(pre_merge_count + count, 0);
        run.positions.grow(count);
        Some(pre_merge_count)
    }

    /// `updateDeferredBounds`: joins the bounds of the last run into the blob bounds, when its
    /// bounds were not given.
    // Port of: src/core/SkTextBlob.cpp#L385-L401 (chrome/m156)
    fn update_deferred_bounds(&mut self) {
        if !self.deferred_bounds {
            return;
        }
        let Some(run) = self.runs.last() else {
            return;
        };
        // FIXME (C++): conservative bounds would also serve kDefault_Positioning.
        let run_bounds = if run.positioning() == GlyphPositioning::Default {
            tight_run_bounds(run)
        } else {
            conservative_run_bounds(run)
        };
        self.bounds.join(run_bounds);
        self.deferred_bounds = false;
    }
}

/// `map_quad_to_rect(xform, rect)`: the bounds of `rect` under the transform `xform`.
// Port of: src/core/SkTextBlob.cpp#L275-L277 (chrome/m156)
fn map_quad_to_rect(xform: &RSXform, rect: &Rect) -> Rect {
    let mut m = Matrix::default();
    m.set_rsxform(xform);
    m.map_rect(rect).0
}

/// `TightRunBounds`: the bounds of the glyphs of `run` as they are drawn, from the glyph metrics.
// Port of: src/core/SkTextBlob.cpp#L279-L323 (chrome/m156)
fn tight_run_bounds(run: &RunRecord) -> Rect {
    let font = &run.font;
    if run.positioning() == GlyphPositioning::Default {
        let bytes: Vec<u8> = run.glyphs.iter().flat_map(|g| g.to_ne_bytes()).collect();
        let (_, mut bounds) = font.measure_text(&bytes, TextEncoding::GlyphId, None);
        bounds.offset(run.offset);
        return bounds;
    }

    let mut glyph_bounds = vec![Rect::default(); run.glyphs.len()];
    font.get_widths_bounds(&run.glyphs, &mut [], &mut glyph_bounds, None);
    let mut bounds = Rect::default();
    match &run.positions {
        Positions::RSXform(xforms) => {
            for (xform, glyph_bound) in xforms.iter().zip(&glyph_bounds) {
                bounds.join(map_quad_to_rect(xform, glyph_bound));
            }
        }
        Positions::Horizontal(xs) => {
            // kHorizontal_Positioning => [ x, x, x... ]; the constant y is the run offset's.
            for (x, glyph_bound) in xs.iter().zip(&glyph_bounds) {
                bounds.join(glyph_bound.with_offset((*x, 0.0)));
            }
        }
        Positions::Full(points) => {
            for (point, glyph_bound) in points.iter().zip(&glyph_bounds) {
                bounds.join(glyph_bound.with_offset(*point));
            }
        }
        Positions::None => {}
    }
    bounds.with_offset(run.offset)
}

/// `ConservativeRunBounds`: the bounds of the glyphs of `run` from the font's bounds, without
/// measuring glyphs. Empty font bounds fall back to [`tight_run_bounds`].
// Port of: src/core/SkTextBlob.cpp#L325-L383 (chrome/m156)
fn conservative_run_bounds(run: &RunRecord) -> Rect {
    let font_bounds = get_font_bounds(&run.font);
    if font_bounds.is_empty() {
        // Empty font bounds are likely a font bug. TightBounds has a better chance of producing
        // useful results in this case.
        return tight_run_bounds(run);
    }

    // Compute the glyph position bbox.
    let mut bounds = match &run.positions {
        Positions::Horizontal(xs) => {
            let mut min_x = xs[0];
            let mut max_x = xs[0];
            for &x in &xs[1..] {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
            }
            Rect::from_ltrb(min_x, 0.0, max_x, 0.0)
        }
        Positions::Full(points) => Rect::bounds_or_empty(points),
        Positions::RSXform(xforms) => {
            let mut bounds = Rect::default();
            for xform in xforms {
                bounds.join(map_quad_to_rect(xform, &font_bounds));
            }
            bounds
        }
        // Only non-default runs reach here: `update_deferred_bounds` picks tight bounds for them.
        Positions::None => return tight_run_bounds(run),
    };

    if run.positioning() != GlyphPositioning::RSXform {
        // Expand by typeface glyph bounds.
        bounds.left += font_bounds.left;
        bounds.top += font_bounds.top;
        bounds.right += font_bounds.right;
        bounds.bottom += font_bounds.bottom;
    }

    // Offset by run position.
    bounds.with_offset(run.offset)
}

impl GlyphPositioning {
    /// The byte that `PositioningAndExtended` stores for the positioning
    /// (`SkTextBlob::GlyphPositioning`'s values).
    // Port of: src/core/SkTextBlob.cpp#L179-L186 (chrome/m156), the enum values
    fn to_wire(self) -> u8 {
        match self {
            Self::Default => 0,
            Self::Horizontal => 1,
            Self::Full => 2,
            Self::RSXform => 3,
        }
    }

    /// The positioning of a stored byte, or `None` for a value past `kRSXform_Positioning`.
    // Port of: src/core/SkTextBlob.cpp#L179-L186 and #L708-L713 (chrome/m156)
    fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Default),
            1 => Some(Self::Horizontal),
            2 => Some(Self::Full),
            3 => Some(Self::RSXform),
            _ => None,
        }
    }
}

/// The native-endian bytes of the positions of a run (`it.pos()`).
fn positions_bytes(positions: &Positions) -> Vec<u8> {
    match positions {
        Positions::None => Vec::new(),
        Positions::Horizontal(values) => values.iter().flat_map(|v| v.to_ne_bytes()).collect(),
        Positions::Full(points) => points
            .iter()
            .flat_map(|p| p.x.to_ne_bytes().into_iter().chain(p.y.to_ne_bytes()))
            .collect(),
        Positions::RSXform(xforms) => xforms
            .iter()
            .flat_map(|x| {
                x.scos
                    .to_ne_bytes()
                    .into_iter()
                    .chain(x.ssin.to_ne_bytes())
                    .chain(x.tx.to_ne_bytes())
                    .chain(x.ty.to_ne_bytes())
            })
            .collect(),
    }
}

/// The native-endian `u32` bytes of the clusters of a run (`it.clusters()`).
fn cluster_bytes(clusters: &[u32]) -> Vec<u8> {
    clusters.iter().flat_map(|c| c.to_ne_bytes()).collect()
}

/// A count as the `int32_t` the buffer stores. A blob never has `i32::MAX` glyphs.
fn count_i32(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// `readByteArray` into a fresh buffer of `size` bytes, or `None` if the buffer rejects it.
fn read_byte_array_of(reader: &mut ReadBuffer<'_>, size: usize) -> Option<Vec<u8>> {
    let mut bytes = vec![0u8; size];
    reader.read_byte_array(&mut bytes).then_some(bytes)
}

/// The native-endian `f32` values of `bytes`, in order.
fn scalars_of(bytes: &[u8]) -> Vec<scalar> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| scalar::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The native-endian glyph ids of `bytes`, in order.
fn glyphs_of(bytes: &[u8]) -> Vec<GlyphId> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| GlyphId::from_ne_bytes([c[0], c[1]]))
        .collect()
}

/// The native-endian cluster values of `bytes`, in order.
fn clusters_of(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

impl TextBlob {
    /// `SkTextBlob::serialize(procs)`: the blob as bytes, with its typefaces written by
    /// `procs.typeface`.
    // Port of: src/core/SkTextBlob.cpp#L851-L859 (chrome/m156)
    #[must_use]
    pub fn serialize(&self, procs: &SerialProcs) -> Data {
        let mut buffer = BinaryWriteBuffer::with_serial_procs(procs.clone());
        self.flatten(&mut buffer);
        buffer.snapshot_as_data()
    }

    /// `SkTextBlob::serialize(procs, memory, size)`: writes the blob into `memory` and returns
    /// the bytes written, or 0 if the blob does not fit (C++ returns 0 when its buffer would
    /// have to leave the caller's storage). The blob is serialized first and then copied, which
    /// gives the same bytes.
    // Port of: src/core/SkTextBlob.cpp#L870-L874 (chrome/m156)
    #[doc(alias = "serialize")]
    pub fn serialize_into(&self, procs: &SerialProcs, memory: &mut [u8]) -> usize {
        let data = self.serialize(procs);
        let bytes = data.as_bytes();
        if bytes.len() > memory.len() {
            return 0;
        }
        memory[..bytes.len()].copy_from_slice(bytes);
        bytes.len()
    }

    /// `SkTextBlob::Deserialize(data, procs)`: the blob that `data` holds, or `None` if the data
    /// is malformed.
    // Port of: src/core/SkTextBlob.cpp#L861-L867 (chrome/m156)
    #[must_use]
    #[doc(alias = "Deserialize")]
    pub fn deserialize(data: &[u8], procs: &DeserialProcs) -> Option<Self> {
        let mut buffer = ReadBuffer::with_deserial_procs(data, procs.clone());
        make_from_buffer(&mut buffer)
    }

    /// `SkTextBlobPriv::Flatten`: the bounds, then each run (its glyph count, positioning and
    /// extended flag, text size, offset, font and arrays), then a zero glyph count.
    // Port of: src/core/SkTextBlob.cpp#L663-L702 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_rect(&self.0.bounds);
        for run in &self.0.runs {
            buffer.write_int(count_i32(run.glyph_count()));
            // `PositioningAndExtended`: the positioning in the low byte, the flag in the next.
            let text_size = run.text.len();
            let extended = text_size > 0;
            let pe = u32::from_le_bytes([run.positioning().to_wire(), u8::from(extended), 0, 0]);
            buffer.write_uint(pe);
            if extended {
                buffer.write_uint(u32::try_from(text_size).unwrap_or(u32::MAX));
            }
            buffer.write_point(run.offset);
            run.font.flatten(buffer);
            buffer.write_byte_array(&glyph_bytes(&run.glyphs));
            buffer.write_byte_array(&positions_bytes(&run.positions));
            if extended {
                buffer.write_byte_array(&cluster_bytes(&run.clusters));
                buffer.write_byte_array(&run.text);
            }
        }
        // Marker for the last run (0 is not a valid glyph count).
        buffer.write_int(0);
    }
}

/// `SkTextBlobPriv::MakeFromBuffer`: reads the runs that `flatten` wrote. Each run's arrays are
/// read and checked before its buffers are allocated, so a malformed stream gives `None`.
// Port of: src/core/SkTextBlob.cpp#L703-L777 (chrome/m156)
fn make_from_buffer(reader: &mut ReadBuffer<'_>) -> Option<TextBlob> {
    let bounds = reader.read_rect();
    let mut builder = TextBlobBuilder::new();
    loop {
        let glyph_count = reader.read_int();
        if glyph_count == 0 {
            // End-of-runs marker.
            break;
        }
        let pe = reader.read_uint();
        let [positioning_byte, extended_byte, _, _] = pe.to_le_bytes();
        let positioning = GlyphPositioning::from_wire(positioning_byte)?;
        let extended = extended_byte != 0;
        if glyph_count < 0 {
            return None;
        }
        let text_size = if extended { reader.read_int() } else { 0 };
        let text_size = usize::try_from(text_size).ok()?;
        let offset = reader.read_point();
        let mut font = Font::default();
        font.unflatten(reader);

        // The expected size of the arrays. Overflow is a malformed run (`SkSafeMath`).
        let count = usize::try_from(glyph_count).ok()?;
        let glyph_size = count.checked_mul(size_of::<GlyphId>())?;
        let pos_size = count
            .checked_mul(size_of::<scalar>())?
            .checked_mul(positioning.scalars_per_glyph())?;
        let cluster_size = if extended {
            count.checked_mul(size_of::<u32>())?
        } else {
            0
        };
        let total_size = glyph_size
            .checked_add(pos_size)?
            .checked_add(cluster_size.checked_add(text_size)?)?;
        if !reader.is_valid() || total_size > reader.available() {
            return None;
        }

        let glyph_data = read_byte_array_of(reader, glyph_size)?;
        let pos_data = read_byte_array_of(reader, pos_size)?;
        let (cluster_data, text_data) = if extended {
            let clusters = read_byte_array_of(reader, cluster_size)?;
            let text = read_byte_array_of(reader, text_size)?;
            (clusters, text)
        } else {
            (Vec::new(), Vec::new())
        };
        let glyph_values = glyphs_of(&glyph_data);
        let cluster_values = clusters_of(&cluster_data);
        if extended && cluster_values.iter().any(|&c| c as usize >= text_size) {
            return None;
        }
        let bounds_ref = Some(&bounds);
        match positioning {
            GlyphPositioning::Default => {
                let (glyphs, text, clusters) =
                    builder.alloc_run_text(&font, count, offset, text_size, bounds_ref);
                fill_glyphs(glyphs, &glyph_values)?;
                fill_bytes(text, &text_data);
                fill_clusters(clusters, &cluster_values, extended)?;
            }
            GlyphPositioning::Horizontal => {
                let (glyphs, pos, text, clusters) =
                    builder.alloc_run_text_pos_h(&font, count, offset.y, text_size, bounds_ref);
                fill_glyphs(glyphs, &glyph_values)?;
                fill_scalars(pos, &scalars_of(&pos_data));
                fill_bytes(text, &text_data);
                fill_clusters(clusters, &cluster_values, extended)?;
            }
            GlyphPositioning::Full => {
                let (glyphs, pos, text, clusters) =
                    builder.alloc_run_text_pos(&font, count, text_size, bounds_ref);
                fill_glyphs(glyphs, &glyph_values)?;
                fill_points(pos, &scalars_of(&pos_data));
                fill_bytes(text, &text_data);
                fill_clusters(clusters, &cluster_values, extended)?;
            }
            GlyphPositioning::RSXform => {
                let (glyphs, xforms, text, clusters) =
                    builder.alloc_run_text_rsxform(&font, count, text_size, bounds_ref);
                fill_glyphs(glyphs, &glyph_values)?;
                fill_rsxforms(xforms, &scalars_of(&pos_data));
                fill_bytes(text, &text_data);
                fill_clusters(clusters, &cluster_values, extended)?;
            }
        }
    }
    builder.make()
}

/// Copies the decoded glyphs into the allocated buffer. An empty buffer means the allocation
/// failed, which C++ sees as the `!buf->glyphs` check.
fn fill_glyphs(dst: &mut [GlyphId], src: &[GlyphId]) -> Option<()> {
    if dst.is_empty() {
        return None;
    }
    dst.iter_mut().zip(src).for_each(|(d, s)| *d = *s);
    Some(())
}

/// Copies the UTF-8 text of an extended run.
fn fill_bytes(dst: &mut [u8], src: &[u8]) {
    dst.iter_mut().zip(src).for_each(|(d, s)| *d = *s);
}

/// Copies the clusters of an extended run; an extended run must have them.
fn fill_clusters(dst: &mut [u32], src: &[u32], extended: bool) -> Option<()> {
    if extended && dst.is_empty() {
        return None;
    }
    dst.iter_mut().zip(src).for_each(|(d, s)| *d = *s);
    Some(())
}

fn fill_scalars(dst: &mut [scalar], src: &[scalar]) {
    dst.iter_mut().zip(src).for_each(|(d, s)| *d = *s);
}

fn fill_points(dst: &mut [Point], src: &[scalar]) {
    for (d, s) in dst.iter_mut().zip(src.as_chunks::<2>().0.iter()) {
        *d = Point::new(s[0], s[1]);
    }
}

fn fill_rsxforms(dst: &mut [RSXform], src: &[scalar]) {
    for (d, s) in dst.iter_mut().zip(src.as_chunks::<4>().0.iter()) {
        *d = RSXform {
            scos: s[0],
            ssin: s[1],
            tx: s[2],
            ty: s[3],
        };
    }
}

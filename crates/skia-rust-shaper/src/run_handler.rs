// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/include/SkShaper.h (RunHandler, its RunInfo and
// Buffer), modules/skshaper/src/SkShaper.cpp (SkTextBlobBuilderRunHandler).

//! The run handler: the callback interface that receives shaped runs.
//!
//! A shaper calls [`RunHandler::begin_line`], then for each run [`RunHandler::run_info`] (all of
//! them before [`RunHandler::commit_run_info`]), then [`RunHandler::run_buffer`] and
//! [`RunHandler::commit_run_buffer`], and finally [`RunHandler::commit_line`].

use std::ops::Range;

use skia_rust_core::font::Font;
use skia_rust_core::font_types::{FourByteTag, GlyphId};
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};

// The C++ `std::min` and `std::max` of SkScalars used in `runInfo` (SkShaper.cpp#L186-L193):
// `std::min(a, b)` is `b < a ? b : a` and `std::max(a, b)` is `a < b ? b : a`. The NaN
// behaviour follows from those comparisons.
fn scalar_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

fn scalar_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

// Port of: modules/skshaper/include/SkShaper.h#L329-L345 (chrome/m156), `RunHandler::RunInfo`.
/// `SkShaper::RunHandler::RunInfo`: one run of a line.
#[derive(Debug)]
#[doc(alias = "SkShaper::RunHandler::RunInfo")]
pub struct RunInfo<'a> {
    /// The font of the run.
    pub font: &'a Font,
    /// The bidi embedding level of the run.
    pub bidi_level: u8,
    /// The script of the run, as an ISO 15924 tag.
    pub script: FourByteTag,
    /// The language of the run, if any.
    pub language: Option<&'a str>,
    /// The advance of the run.
    pub advance: Vector,
    /// The number of glyphs of the run.
    pub glyph_count: usize,
    /// The UTF-8 bytes of the run.
    pub utf8_range: Range<usize>,
}

// Port of: modules/skshaper/include/SkShaper.h#L347-L355 (chrome/m156), `RunHandler::Buffer`.
/// `SkShaper::RunHandler::Buffer`: the arrays a shaper fills for one run.
#[derive(Debug)]
#[doc(alias = "SkShaper::RunHandler::Buffer")]
pub struct Buffer<'a> {
    /// The glyphs of the run. Required.
    pub glyphs: &'a mut [GlyphId],
    /// The positions of the glyphs. Required. If `offsets` is `None`, `glyphs[i]` goes at
    /// `positions[i]`.
    pub positions: &'a mut [Point],
    /// Optional offsets: if present, `glyphs[i]` goes at `positions[i] + offsets[i]`.
    pub offsets: Option<&'a mut [Point]>,
    /// Optional UTF-8 clusters: `clusters[i]` starts the run that produced `glyphs[i]`.
    pub clusters: Option<&'a mut [u32]>,
    /// The offset added to all positions.
    pub point: Point,
}

/// `SkShaper::RunHandler`: receives the shaped runs of each line.
#[doc(alias = "SkShaper::RunHandler")]
pub trait RunHandler {
    /// Called when beginning a line.
    #[doc(alias = "beginLine")]
    fn begin_line(&mut self);

    /// Called once for each run in a line, before [`RunHandler::commit_run_info`]. Can compute
    /// baselines and offsets.
    #[doc(alias = "runInfo")]
    fn run_info(&mut self, info: &RunInfo<'_>);

    /// Called after all [`RunHandler::run_info`] calls for a line.
    #[doc(alias = "commitRunInfo")]
    fn commit_run_info(&mut self);

    /// Called for each run in a line after [`RunHandler::commit_run_info`]. The buffer is filled
    /// in by the shaper.
    #[doc(alias = "runBuffer")]
    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_>;

    /// Called after each run's buffer is filled in.
    #[doc(alias = "commitRunBuffer")]
    fn commit_run_buffer(&mut self, info: &RunInfo<'_>);

    /// Called when ending a line.
    #[doc(alias = "commitLine")]
    fn commit_line(&mut self);
}

// Port of: modules/skshaper/src/SkShaper.cpp#L199-L204 (chrome/m156), the buffers that
// `SkTextBlobBuilderRunHandler::runBuffer` allocates in the builder. They are staged here
// because the builder allocates its run only when the run is committed, and the glyph and
// cluster arrays must be written before that.
#[derive(Debug)]
struct StagedRun {
    glyphs: Vec<GlyphId>,
    positions: Vec<Point>,
    clusters: Vec<u32>,
    /// `fClusterOffset`: the UTF-8 offset of the run, subtracted from its clusters.
    cluster_offset: usize,
}

/// `SkTextBlobBuilderRunHandler`: builds a [`TextBlob`] from the shaped runs.
#[derive(Debug)]
#[doc(alias = "SkTextBlobBuilderRunHandler")]
pub struct TextBlobBuilderRunHandler<'text> {
    builder: TextBlobBuilder,
    utf8_text: &'text str,
    staged: Option<StagedRun>,
    max_run_ascent: scalar,
    max_run_descent: scalar,
    max_run_leading: scalar,
    current_position: Point,
    offset: Point,
}

impl<'text> TextBlobBuilderRunHandler<'text> {
    // Port of: modules/skshaper/src/SkShaper.cpp#L174-L177 (chrome/m156), the constructor.
    /// `SkTextBlobBuilderRunHandler(const char* utf8Text, SkPoint offset)`.
    #[must_use]
    pub fn new(utf8_text: &'text str, offset: Point) -> Self {
        Self {
            builder: TextBlobBuilder::new(),
            utf8_text,
            staged: None,
            max_run_ascent: 0.0,
            max_run_descent: 0.0,
            max_run_leading: 0.0,
            current_position: Point::new(0.0, 0.0),
            offset,
        }
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L256-L258 (chrome/m156), `endPoint`.
    /// `endPoint()`: the offset at which the next line starts.
    #[must_use]
    pub fn end_point(&self) -> Point {
        self.offset
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L256-L258 (chrome/m156), `makeBlob`.
    /// `makeBlob()`: the blob of all the runs so far, or `None` if there are none.
    #[must_use]
    pub fn make_blob(&mut self) -> Option<TextBlob> {
        self.builder.make()
    }
}

impl RunHandler for TextBlobBuilderRunHandler<'_> {
    // Port of: modules/skshaper/src/SkShaper.cpp#L179-L184 (chrome/m156), `beginLine`.
    fn begin_line(&mut self) {
        self.current_position = self.offset;
        self.max_run_ascent = 0.0;
        self.max_run_descent = 0.0;
        self.max_run_leading = 0.0;
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L186-L193 (chrome/m156), `runInfo`.
    fn run_info(&mut self, info: &RunInfo<'_>) {
        let (_, metrics) = info.font.metrics();
        self.max_run_ascent = scalar_min(self.max_run_ascent, metrics.ascent);
        self.max_run_descent = scalar_max(self.max_run_descent, metrics.descent);
        self.max_run_leading = scalar_max(self.max_run_leading, metrics.leading);
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L195-L197 (chrome/m156), `commitRunInfo`.
    fn commit_run_info(&mut self) {
        self.current_position.y -= self.max_run_ascent;
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L199-L214 (chrome/m156), `runBuffer`. The
    // builder allocation happens in `commit_run_buffer`, where the staged arrays are copied.
    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
        let staged = self.staged.insert(StagedRun {
            glyphs: vec![0; info.glyph_count],
            positions: vec![Point::new(0.0, 0.0); info.glyph_count],
            clusters: vec![0; info.glyph_count],
            cluster_offset: info.utf8_range.start,
        });
        Buffer {
            glyphs: staged.glyphs.as_mut_slice(),
            positions: staged.positions.as_mut_slice(),
            offsets: None,
            clusters: Some(staged.clusters.as_mut_slice()),
            point: self.current_position,
        }
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L216-L225 (chrome/m156), `commitRunBuffer`.
    // The run is allocated in the builder here, with its UTF-8 bytes (`runBuffer` in Skia), and
    // its clusters are rebased to the run.
    fn commit_run_buffer(&mut self, info: &RunInfo<'_>) {
        let Some(staged) = self.staged.take() else {
            debug_assert!(false, "commitRunBuffer without runBuffer");
            return;
        };
        let glyph_count = staged.glyphs.len();
        let utf8_size = info.utf8_range.len();
        let (glyphs, points, utf8_text, clusters) =
            self.builder
                .alloc_run_text_pos(info.font, glyph_count, utf8_size, None);
        glyphs.copy_from_slice(&staged.glyphs);
        points.copy_from_slice(&staged.positions);
        // `if (runBuffer.utf8text && fUtf8Text) memcpy(...)`.
        let text = &self.utf8_text.as_bytes()[info.utf8_range.clone()];
        utf8_text.copy_from_slice(text);
        // `fClusters[i] -= fClusterOffset`: unsigned wrapping, as in C++.
        #[allow(clippy::cast_possible_truncation)] // mirrors `SkToU32` of the UTF-8 offset
        let cluster_offset = staged.cluster_offset as u32;
        for (dst, &cluster) in clusters.iter_mut().zip(&staged.clusters) {
            *dst = cluster.wrapping_sub(cluster_offset);
        }
        self.current_position.x += info.advance.x;
        self.current_position.y += info.advance.y;
    }

    // Port of: modules/skshaper/src/SkShaper.cpp#L227-L230 (chrome/m156), `commitLine`.
    fn commit_line(&mut self) {
        self.offset.y += self.max_run_descent + self.max_run_leading - self.max_run_ascent;
    }
}

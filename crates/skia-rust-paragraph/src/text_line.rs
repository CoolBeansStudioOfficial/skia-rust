// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/src/TextLine.h,
// modules/skparagraph/src/TextLine.cpp
//
// A `TextLine` holds no pointer to its paragraph; the methods that read the paragraph take it as
// an argument (`owner`). The lines are the paragraph's `lines`, so a method that writes the
// paragraph (`format`, `create_ellipsis`) is called on a line taken out of it.

//! `TextLine`: one line of a laid out paragraph.

use std::sync::{Mutex, PoisonError};

use bitflags::bitflags;
use skia_rust_core::font::Font;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    Scalar, scalar, scalar_abs, scalar_floor_to_int, scalar_floor_to_scalar, scalar_round_to_scalar,
};
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::typeface::Typeface;
use skia_rust_shaper::harfbuzz;
use skia_rust_shaper::run_handler::{Buffer, RunHandler, RunInfo};
use skia_rust_shaper::run_iterator::{make_font_mgr_run_iterator, make_std_language_run_iterator};
use skia_rust_shaper::unicode_shaper::unicode_bidi_run_iterator;

use crate::dart_types::{
    Affinity, EMPTY_INDEX, LineMetricStyle, PositionWithAffinity, RangeExtensions, RectHeightStyle,
    RectWidthStyle, TextAlign, TextBox, TextDirection,
};
use crate::decorations::Decorations;
use crate::metrics::{LineMetrics, StyleMetrics};
use crate::paragraph_impl::Paragraph;
use crate::paragraph_painter::{PaintOrId, ParagraphPainter};
use crate::run::{
    Cluster, ClusterIndex, ClusterRange, EMPTY_RUN, GlyphRange, InternalLineMetrics, Run, RunIndex,
};
use crate::text_style::{
    BlockRange, EMPTY_TEXT, StyleType, TextRange, TextStyle, nearly_equal_default,
    nearly_zero_default,
};

// Port of: modules/skparagraph/src/TextLine.cpp#L40-L46 (chrome/m156)
/// `intersected(a, b)`.
fn intersected(a: &TextRange, b: &TextRange) -> TextRange {
    if a.start == b.start && a.end == b.end {
        return a.clone();
    }
    let begin = a.start.max(b.start);
    let end = a.end.min(b.end);
    if end >= begin { begin..end } else { EMPTY_TEXT }
}

// Port of: modules/skparagraph/src/TextLine.cpp#L48-L51 (chrome/m156)
/// `littleRound(a)`: `SkScalarRoundToScalar(a * 100.0)/100.0`, evaluated in double.
fn little_round(a: scalar) -> scalar {
    (f64::from(scalar_round_to_scalar((f64::from(a) * 100.0) as scalar)) / 100.0) as scalar
}

// Port of: modules/skparagraph/src/TextLine.cpp#L53-L58 (chrome/m156)
/// `operator*(TextRange a, TextRange b)`: the intersection, or the empty range.
#[must_use]
pub fn range_intersection(a: &TextRange, b: &TextRange) -> TextRange {
    if a.start == b.start && a.end == b.end {
        return a.clone();
    }
    let begin = a.start.max(b.start);
    let end = a.end.min(b.end);
    if end > begin { begin..end } else { EMPTY_TEXT }
}

// Port of: modules/skparagraph/src/TextLine.cpp#L60-L84 (chrome/m156)
/// `compareRound(a, b, applyRoundingHack)`.
fn compare_round(a: scalar, b: scalar, apply_rounding_hack: bool) -> i32 {
    // There is a rounding error that gets bigger when maxWidth gets bigger
    // VERY long zalgo text (> 100000) on a VERY long line (> 10000)
    // Canvas scaling affects it
    // Letter spacing affects it
    // It has to be relative to be useful
    let base = scalar_abs(a).max(scalar_abs(b));
    let diff = scalar_abs(a - b);
    if nearly_zero_default(base) || diff / base < 0.001 {
        return 0;
    }

    let mut ra = a;
    let mut rb = b;

    if apply_rounding_hack {
        ra = little_round(a);
        rb = little_round(b);
    }
    if ra < rb { -1 } else { 1 }
}

// Port of: modules/skparagraph/src/TextLine.h#L33-L43 (chrome/m156)
/// `TextLine::ClipContext`: where a piece of a run is, and how it is clipped.
#[doc(alias = "TextLine::ClipContext")]
#[derive(Debug, Clone)]
pub struct ClipContext<'a> {
    /// `run`.
    pub run: &'a Run,
    /// `pos`.
    pub pos: usize,
    /// `size`.
    pub size: usize,
    /// `fTextShift`: shifts the text inside the run so it's placed at the right position.
    pub text_shift: scalar,
    /// `clip`.
    pub clip: Rect,
    /// `fExcludedTrailingSpaces`.
    pub excluded_trailing_spaces: scalar,
    /// `clippingNeeded`.
    pub clipping_needed: bool,
}

bitflags! {
    // Port of: modules/skparagraph/src/TextLine.h#L45-L50 (chrome/m156)
    /// `TextLine::TextAdjustment`.
    #[doc(alias = "TextLine::TextAdjustment")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct TextAdjustment: u32 {
        /// `GlyphCluster`: all text producing glyphs pointing to the same ClusterIndex.
        const GLYPH_CLUSTER = 0x01;
        /// `GlyphemeCluster`: base glyph + all attached diacritics.
        const GLYPHEME_CLUSTER = 0x02;
        /// `Grapheme`: text adjusted to graphemes.
        const GRAPHEME = 0x04;
        /// `GraphemeGluster`: GlyphCluster & Grapheme.
        const GRAPHEME_GLUSTER = 0x05;
    }
}

/// Which run a [`TextBlobRecord`] was made from: a run of the paragraph, or the ellipsis or the
/// hyphen of the line (`Run*` in C++).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RunRef {
    /// `fRuns[index]` of the paragraph.
    Run(usize),
    /// The ellipsis run of the line.
    Ellipsis,
    /// The hyphen run of the line.
    Hyphen,
}

// Port of: modules/skparagraph/src/TextLine.h#L200-L215 (chrome/m156)
/// `TextLine::TextBlobRecord`.
#[doc(alias = "TextLine::TextBlobRecord")]
#[derive(Debug, Clone)]
pub struct TextBlobRecord {
    /// `fBlob`.
    pub blob: Option<TextBlob>,
    /// `fOffset`.
    pub offset: Point,
    /// `fPaint`.
    pub paint: PaintOrId,
    /// `fBounds`.
    pub bounds: Rect,
    /// `fClippingNeeded`.
    pub clipping_needed: bool,
    /// `fClipRect`.
    pub clip_rect: Rect,
    /// `fVisitor_Run`.
    pub visitor_run: RunRef,
    /// `fVisitor_Pos`.
    pub visitor_pos: usize,
}

impl TextBlobRecord {
    // Port of: modules/skparagraph/src/TextLine.cpp#L365-L373 (chrome/m156)
    /// `paint(painter, x, y)`.
    pub fn paint(&self, painter: &mut dyn ParagraphPainter, x: scalar, y: scalar) {
        if self.clipping_needed {
            painter.save();
            painter.clip_rect(&self.clip_rect.with_offset((x, y)));
        }
        painter.draw_text_blob(
            self.blob.as_ref(),
            x + self.offset.x,
            y + self.offset.y,
            &self.paint,
        );
        if self.clipping_needed {
            painter.restore();
        }
    }
}

#[derive(Debug, Default)]
struct TextBlobCache {
    populated: bool,
    records: Vec<TextBlobRecord>,
}

// Port of: modules/skparagraph/src/TextLine.h#L28-L226 (chrome/m156)
/// `TextLine`: a line of a paragraph.
#[derive(Debug)]
pub struct TextLine {
    /// The index of the line in the paragraph's lines (`this - fOwner->lines().data()`).
    pub(crate) index: usize,
    pub(crate) block_range: BlockRange,
    pub(crate) text_excluding_spaces: TextRange,
    pub(crate) text: TextRange,
    pub(crate) text_including_newlines: TextRange,
    pub(crate) cluster_range: ClusterRange,
    pub(crate) ghost_cluster_range: ClusterRange,
    pub(crate) runs_in_visual_order: Vec<usize>,
    /// `fAdvance`: text size.
    pub(crate) advance: Vector,
    /// `fOffset`: text position.
    pub(crate) offset: Vector,
    /// `fShift`: left shift.
    pub(crate) shift: scalar,
    pub(crate) width_with_spaces: scalar,
    /// `fEllipsis`: in case the line ends with the ellipsis.
    pub(crate) ellipsis: Option<Box<Run>>,
    /// `fHyphen`: visible hyphen for soft hyphen line breaks.
    pub(crate) hyphen: Option<Box<Run>>,
    /// `fSizes`: line metrics as a max of all run metrics and struts.
    pub(crate) sizes: InternalLineMetrics,
    /// `fMaxRunMetrics`: no struts - need it for GetRectForRange(max height).
    pub(crate) max_run_metrics: InternalLineMetrics,
    pub(crate) has_background: bool,
    pub(crate) has_shadows: bool,
    pub(crate) has_decorations: bool,
    pub(crate) ascent_style: LineMetricStyle,
    pub(crate) descent_style: LineMetricStyle,
    blob_cache: Mutex<TextBlobCache>,
}

impl TextLine {
    // Port of: modules/skparagraph/src/TextLine.cpp#L86-L183 (chrome/m156)
    /// `TextLine(owner, offset, advance, blocks, textExcludingSpaces, text, textIncludingNewlines,
    /// clusters, clustersWithGhosts, widthWithSpaces, sizes)`: the line number is `index`.
    #[allow(clippy::too_many_arguments)] // mirrors the constructor of TextLine
    #[must_use]
    pub fn new(
        owner: &Paragraph,
        index: usize,
        offset: Vector,
        advance: Vector,
        blocks: BlockRange,
        text_excluding_spaces: TextRange,
        text: TextRange,
        text_including_newlines: TextRange,
        clusters: ClusterRange,
        clusters_with_ghosts: ClusterRange,
        width_with_spaces: scalar,
        sizes: InternalLineMetrics,
    ) -> Self {
        let mut line = TextLine {
            index,
            block_range: blocks,
            text_excluding_spaces,
            text,
            text_including_newlines,
            cluster_range: clusters,
            ghost_cluster_range: clusters_with_ghosts,
            runs_in_visual_order: Vec::new(),
            advance,
            offset,
            shift: 0.0,
            width_with_spaces,
            ellipsis: None,
            hyphen: None,
            sizes,
            max_run_metrics: InternalLineMetrics::default(),
            has_background: false,
            has_shadows: false,
            has_decorations: false,
            ascent_style: LineMetricStyle::CSS,
            descent_style: LineMetricStyle::CSS,
            blob_cache: Mutex::new(TextBlobCache::default()),
        };

        // Reorder visual runs
        let start_index = line.ghost_cluster_range.start;
        let end_index = line.ghost_cluster_range.end - 1;
        let start = owner.cluster(start_index);
        let end = owner.cluster(end_index);
        // `end.runIndex() - start.runIndex() + 1` in size_t arithmetic.
        let num_runs = end
            .run_index()
            .wrapping_sub(start.run_index())
            .wrapping_add(1);

        for index in line.block_range.clone() {
            let b = &owner.styles()[index];
            if b.style.has_background() {
                line.has_background = true;
            }
            if b.style.decoration_type() != crate::text_style::TextDecoration::NO_DECORATION {
                line.has_decorations = true;
            }
            if b.style.shadow_number() > 0 {
                line.has_shadows = true;
            }
        }

        // Get the logical order

        let mut run_levels: Vec<u8> = vec![0; num_runs];
        let mut placeholders_in_original_order: Vec<RunIndex> = Vec::new();
        let mut run_levels_index: usize = 0;
        // Placeholders must be laid out using the original order in which they were added
        // in the input. The API does not provide a way to indicate that a placeholder
        // position was moved due to bidi reordering.
        let mut run_index = start.run_index();
        while run_index <= end.run_index() {
            let run = owner.run(run_index);
            run_levels[run_levels_index] = run.bidi_level;
            run_levels_index += 1;
            line.max_run_metrics.add(InternalLineMetrics::from_adl(
                run.correct_ascent(),
                run.correct_descent(),
                run.font_metrics.leading,
            ));
            if run.is_placeholder() {
                placeholders_in_original_order.push(run_index);
            }
            run_index += 1;
        }
        debug_assert!(run_levels_index == num_runs);

        let mut logical_order: Vec<i32> = vec![0; num_runs];

        // TODO: hide all these logic in SkUnicode?
        owner
            .unicode()
            .reorder_visual(&run_levels, &mut logical_order);
        let first_run_index = start.run_index();
        let mut placeholder_iter = placeholders_in_original_order.iter();
        for &index in &logical_order {
            let run_index = first_run_index.wrapping_add_signed(index as isize);
            if owner.run(run_index).is_placeholder() {
                line.runs_in_visual_order
                    .push(*placeholder_iter.next().expect("a placeholder run"));
            } else {
                line.runs_in_visual_order.push(run_index);
            }
        }

        // TODO: This is the fix for flutter. Must be removed...
        let mut cluster_index = start_index;
        while cluster_index <= end_index {
            let cluster = owner.cluster(cluster_index);
            let run = cluster.run(owner.runs());
            if !run.is_placeholder() && !run.is_cursive_script() {
                line.shift += cluster.half_letter_spacing();
                break;
            }
            cluster_index += 1;
        }
        line
    }

    /// `trimmedText()`.
    #[must_use]
    #[doc(alias = "trimmedText")]
    pub fn trimmed_text(&self) -> TextRange {
        self.text_excluding_spaces.clone()
    }

    /// `textWithNewlines()`.
    #[must_use]
    #[doc(alias = "textWithNewlines")]
    pub fn text_with_newlines(&self) -> TextRange {
        self.text_including_newlines.clone()
    }

    /// `text()`.
    #[must_use]
    pub fn text(&self) -> TextRange {
        self.text.clone()
    }

    /// `clusters()`.
    #[must_use]
    pub fn clusters(&self) -> ClusterRange {
        self.cluster_range.clone()
    }

    /// `clustersWithSpaces()`.
    #[must_use]
    #[doc(alias = "clustersWithSpaces")]
    pub fn clusters_with_spaces(&self) -> ClusterRange {
        self.ghost_cluster_range.clone()
    }

    /// `ellipsis()`.
    #[must_use]
    pub fn ellipsis(&self) -> Option<&Run> {
        self.ellipsis.as_deref()
    }

    /// `hyphen()`.
    #[must_use]
    pub fn hyphen(&self) -> Option<&Run> {
        self.hyphen.as_deref()
    }

    /// `sizes()`.
    #[must_use]
    pub fn sizes(&self) -> InternalLineMetrics {
        self.sizes
    }

    /// `empty()`.
    #[must_use]
    pub fn empty(&self) -> bool {
        self.text_excluding_spaces.empty()
    }

    /// `spacesWidth()`.
    #[must_use]
    #[doc(alias = "spacesWidth")]
    pub fn spaces_width(&self) -> scalar {
        self.width_with_spaces - self.width()
    }

    /// `height()`.
    #[must_use]
    pub fn height(&self) -> scalar {
        self.advance.y
    }

    /// `width()`.
    #[must_use]
    pub fn width(&self) -> scalar {
        self.advance.x
            + (match &self.ellipsis {
                Some(ellipsis) => ellipsis.advance.x,
                None => 0.0,
            })
            + (match &self.hyphen {
                Some(hyphen) => hyphen.advance.x,
                None => 0.0,
            })
    }

    /// `widthWithoutEllipsis()`.
    #[must_use]
    #[doc(alias = "widthWithoutEllipsis")]
    pub fn width_without_ellipsis(&self) -> scalar {
        self.advance.x
    }

    /// `offset()`.
    #[must_use]
    pub fn offset(&self) -> Vector {
        // Port of: modules/skparagraph/src/TextLine.cpp#L1131-L1133 (chrome/m156)
        self.offset + Vector::new(self.shift, 0.0)
    }

    /// `alphabeticBaseline()`.
    #[must_use]
    #[doc(alias = "alphabeticBaseline")]
    pub fn alphabetic_baseline(&self) -> scalar {
        self.sizes.alphabetic_baseline()
    }

    /// `ideographicBaseline()`.
    #[must_use]
    #[doc(alias = "ideographicBaseline")]
    pub fn ideographic_baseline(&self) -> scalar {
        self.sizes.ideographic_baseline()
    }

    /// `baseline()`.
    #[must_use]
    pub fn baseline(&self) -> scalar {
        self.sizes.baseline()
    }

    /// The run of a [`RunRef`].
    #[must_use]
    pub fn run_of<'a>(&'a self, owner: &'a Paragraph, run_ref: RunRef) -> &'a Run {
        match run_ref {
            RunRef::Run(index) => owner.run(index),
            RunRef::Ellipsis => self.ellipsis.as_deref().expect("the line has an ellipsis"),
            RunRef::Hyphen => self.hyphen.as_deref().expect("the line has a hyphen"),
        }
    }

    /// The [`RunRef`] of a run of this line or of the paragraph (the pointer comparison of C++).
    fn run_ref(&self, run: &Run) -> RunRef {
        if let Some(ellipsis) = self.ellipsis.as_deref() {
            if std::ptr::eq(ellipsis, run) {
                return RunRef::Ellipsis;
            }
        }
        if let Some(hyphen) = self.hyphen.as_deref() {
            if std::ptr::eq(hyphen, run) {
                return RunRef::Hyphen;
            }
        }
        RunRef::Run(run.index)
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L185-L241 (chrome/m156)
    /// `paint(painter, x, y)`.
    pub fn paint(
        &self,
        owner: &Paragraph,
        painter: &mut dyn ParagraphPainter,
        x: scalar,
        y: scalar,
    ) {
        if self.has_background {
            self.iterate_through_visual_runs(
                owner,
                false,
                &mut |run: &Run,
                      run_offset_in_line,
                      text_range: TextRange,
                      run_width_in_line: &mut scalar| {
                    *run_width_in_line = self.iterate_through_single_run_by_styles(
                        owner,
                        TextAdjustment::GLYPH_CLUSTER,
                        run,
                        run_offset_in_line,
                        text_range,
                        StyleType::Background,
                        &mut |text_range: TextRange,
                              style: &TextStyle,
                              context: &ClipContext<'_>| {
                            self.paint_background(painter, x, y, text_range, style, context);
                        },
                    );
                    true
                },
            );
        }

        if self.has_shadows {
            self.iterate_through_visual_runs(
                owner,
                false,
                &mut |run: &Run,
                      run_offset_in_line,
                      text_range: TextRange,
                      run_width_in_line: &mut scalar| {
                    *run_width_in_line = self.iterate_through_single_run_by_styles(
                        owner,
                        TextAdjustment::GLYPH_CLUSTER,
                        run,
                        run_offset_in_line,
                        text_range,
                        StyleType::Shadow,
                        &mut |text_range: TextRange,
                              style: &TextStyle,
                              context: &ClipContext<'_>| {
                            self.paint_shadow(painter, x, y, text_range, style, context);
                        },
                    );
                    true
                },
            );
        }

        self.ensure_text_blob_cache_populated(owner);

        {
            let cache = self
                .blob_cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            for record in &cache.records {
                record.paint(painter, x, y);
            }
        }

        if self.has_decorations {
            self.iterate_through_visual_runs(
                owner,
                false,
                &mut |run: &Run,
                      run_offset_in_line,
                      text_range: TextRange,
                      run_width_in_line: &mut scalar| {
                    *run_width_in_line = self.iterate_through_single_run_by_styles(
                        owner,
                        TextAdjustment::GLYPH_CLUSTER,
                        run,
                        run_offset_in_line,
                        text_range,
                        StyleType::Decorations,
                        &mut |text_range: TextRange,
                              style: &TextStyle,
                              context: &ClipContext<'_>| {
                            self.paint_decorations(painter, x, y, text_range, style, context);
                        },
                    );
                    true
                },
            );
        }
    }

    /// The text blob records of the line, after [`TextLine::ensure_text_blob_cache_populated`]
    /// (`fTextBlobCache`).
    #[must_use]
    pub fn text_blob_cache(&self) -> Vec<TextBlobRecord> {
        self.blob_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .records
            .clone()
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L243-L312 (chrome/m156)
    /// `ensureTextBlobCachePopulated()`.
    #[doc(alias = "ensureTextBlobCachePopulated")]
    pub fn ensure_text_blob_cache_populated(&self, owner: &Paragraph) {
        if self
            .blob_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .populated
        {
            return;
        }
        let mut records: Vec<TextBlobRecord> = Vec::new();
        if self.block_range.width() == 1
            && self.runs_in_visual_order.len() == 1
            && self.ellipsis.is_none()
            && self.hyphen.is_none()
            && !owner.run(self.runs_in_visual_order[0]).is_placeholder()
        {
            if self.cluster_range.width() == 0 {
                return;
            }

            // Most common and most simple case
            let style = &owner.block(self.block_range.start).style;
            let run = owner.run(self.runs_in_visual_order[0]);
            let clip = Rect::from_xywh(
                0.0,
                self.sizes().run_top(run, self.ascent_style),
                self.advance.x,
                run.calculate_height(self.ascent_style, self.descent_style),
            );

            let start = owner.cluster(self.cluster_range.start);
            let end = owner.cluster(self.cluster_range.end - 1);
            debug_assert!(start.run_index() == end.run_index());
            let glyphs: GlyphRange = if run.left_to_right() {
                start.start_pos()..(if end.is_hard_break() {
                    end.start_pos()
                } else {
                    end.end_pos()
                })
            } else {
                end.start_pos()..(if start.is_hard_break() {
                    start.start_pos()
                } else {
                    start.end_pos()
                })
            };
            let context = ClipContext {
                run,
                pos: glyphs.start,
                size: glyphs.width(),
                // starting position
                text_shift: -run.position_x(glyphs.start),
                // entire line
                clip,
                // no need for that
                excluded_trailing_spaces: 0.0,
                // no need for that
                clipping_needed: false,
            };
            self.build_text_blob(
                &mut records,
                self.text_excluding_spaces.clone(),
                style,
                &context,
            );
        } else {
            self.iterate_through_visual_runs(
                owner,
                false,
                &mut |run: &Run,
                      run_offset_in_line,
                      text_range: TextRange,
                      run_width_in_line: &mut scalar| {
                    if run.is_placeholder() {
                        *run_width_in_line = run.advance().x;
                        return true;
                    }
                    *run_width_in_line = self.iterate_through_single_run_by_styles(
                        owner,
                        TextAdjustment::GLYPH_CLUSTER,
                        run,
                        run_offset_in_line,
                        text_range,
                        StyleType::Foreground,
                        &mut |text_range: TextRange,
                              style: &TextStyle,
                              context: &ClipContext<'_>| {
                            self.build_text_blob(&mut records, text_range, style, context);
                        },
                    );
                    true
                },
            );
        }
        let mut cache = self
            .blob_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        cache.records.extend(records);
        cache.populated = true;
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L314-L326 (chrome/m156)
    /// `format(align, maxWidth)`.
    pub fn format(&mut self, owner: &mut Paragraph, align: TextAlign, max_width: scalar) {
        let delta = max_width - self.width();
        if delta <= 0.0 {
            return;
        }

        // We do nothing for left align
        if align == TextAlign::Justify {
            if !self.ends_with_hard_line_break(owner) {
                self.justify(owner, max_width);
            } else if owner.paragraph_style().text_direction() == TextDirection::RTL {
                // Justify -> Right align
                self.shift = delta;
            }
        } else if align == TextAlign::Right {
            self.shift = delta;
        } else if align == TextAlign::Center {
            self.shift = delta / 2.0;
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L328-L346 (chrome/m156)
    /// `scanStyles(style, visitor)`.
    #[doc(alias = "scanStyles")]
    pub fn scan_styles(
        &self,
        owner: &Paragraph,
        style_type: StyleType,
        visitor: &mut dyn FnMut(TextRange, &TextStyle, &ClipContext<'_>),
    ) {
        if self.empty() {
            return;
        }

        self.iterate_through_visual_runs(
            owner,
            false,
            &mut |run: &Run, run_offset, text_range: TextRange, width: &mut scalar| {
                *width = self.iterate_through_single_run_by_styles(
                    owner,
                    TextAdjustment::GLYPH_CLUSTER,
                    run,
                    run_offset,
                    text_range,
                    style_type,
                    &mut |text_range: TextRange, style: &TextStyle, context: &ClipContext<'_>| {
                        visitor(text_range, style, context);
                    },
                );
                true
            },
        );
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L348-L352 (chrome/m156)
    /// `extendHeight(context)`.
    #[must_use]
    #[doc(alias = "extendHeight")]
    pub fn extend_height(&self, context: &ClipContext<'_>) -> Rect {
        let mut result = context.clip;
        // `std::max(a, 0.0f)` is `a < 0 ? 0 : a`.
        let extra = self.max_run_metrics.height() - self.height();
        result.bottom += if extra < 0.0 { 0.0 } else { extra };
        result
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L354-L401 (chrome/m156)
    /// `buildTextBlob(textRange, style, context)`: appends the record of the text to `records`.
    fn build_text_blob(
        &self,
        records: &mut Vec<TextBlobRecord>,
        _text_range: TextRange,
        style: &TextStyle,
        context: &ClipContext<'_>,
    ) {
        if context.run.is_placeholder() {
            return;
        }

        let mut record = TextBlobRecord {
            blob: None,
            offset: Point::new(0.0, 0.0),
            paint: PaintOrId::default(),
            bounds: Rect::from_xywh(0.0, 0.0, 0.0, 0.0),
            clipping_needed: false,
            clip_rect: Rect::from_xywh(0.0, 0.0, 0.0, 0.0),
            visitor_run: RunRef::Run(0),
            visitor_pos: 0,
        };

        if style.has_foreground() {
            record.paint = style.foreground_paint_or_id();
        } else {
            // `std::get<SkPaint>(record.fPaint).setColor(style.getColor())` on the default paint.
            let mut paint = skia_rust_core::paint::Paint::default();
            paint.set_color(style.color());
            record.paint = PaintOrId::Paint(paint);
        }
        record.visitor_run = self.run_ref(context.run);
        record.visitor_pos = context.pos;

        // TODO: This is the change for flutter, must be removed later
        let mut builder = TextBlobBuilder::new();
        context.run.copy_to(&mut builder, context.pos, context.size);
        record.clipping_needed = context.clipping_needed;
        if context.clipping_needed {
            record.clip_rect = self.extend_height(context).with_offset(self.offset());
        } else {
            record.clip_rect = context.clip.with_offset(self.offset());
        }

        debug_assert!(nearly_equal_default(
            context.run.baseline_shift(),
            style.baseline_shift()
        ));
        let corrected_baseline = scalar_floor_to_scalar(
            (f64::from(self.baseline() + style.baseline_shift()) + 0.5) as scalar,
        );
        record.blob = builder.make();
        if let Some(blob) = &record.blob {
            record.bounds.join_possibly_empty_rect(blob.bounds());
        }

        record.offset = Point::new(
            self.offset().x + context.text_shift,
            self.offset().y + corrected_baseline,
        );
        records.push(record);
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L375-L384 (chrome/m156)
    fn paint_background(
        &self,
        painter: &mut dyn ParagraphPainter,
        x: scalar,
        y: scalar,
        _text_range: TextRange,
        style: &TextStyle,
        context: &ClipContext<'_>,
    ) {
        if style.has_background() {
            painter.draw_rect(
                &context.clip.with_offset(self.offset() + Point::new(x, y)),
                &style.background_paint_or_id(),
            );
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L386-L421 (chrome/m156)
    fn paint_shadow(
        &self,
        painter: &mut dyn ParagraphPainter,
        x: scalar,
        y: scalar,
        _text_range: TextRange,
        style: &TextStyle,
        context: &ClipContext<'_>,
    ) {
        let corrected_baseline = scalar_floor_to_scalar(
            (f64::from(self.baseline() + style.baseline_shift()) + 0.5) as scalar,
        );

        for shadow in style.shadows() {
            if !shadow.has_shadow() {
                continue;
            }

            let mut builder = TextBlobBuilder::new();
            context.run.copy_to(&mut builder, context.pos, context.size);

            if context.clipping_needed {
                painter.save();
                let mut clip = self.extend_height(context);
                clip.offset((x, y));
                clip.offset(self.offset());
                painter.clip_rect(&clip);
            }
            let blob = builder.make();
            painter.draw_text_shadow(
                blob.as_ref(),
                x + self.offset().x + shadow.offset.x + context.text_shift,
                y + self.offset().y + shadow.offset.y + corrected_baseline,
                shadow.color,
                shadow.blur_sigma as scalar,
            );
            if context.clipping_needed {
                painter.restore();
            }
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L423-L433 (chrome/m156)
    fn paint_decorations(
        &self,
        painter: &mut dyn ParagraphPainter,
        x: scalar,
        y: scalar,
        _text_range: TextRange,
        style: &TextStyle,
        context: &ClipContext<'_>,
    ) {
        painter.save();
        painter.translate(
            x + self.offset().x,
            y + self.offset().y + style.baseline_shift(),
        );
        let mut decorations = Decorations::default();
        let corrected_baseline = scalar_floor_to_scalar(
            (f64::from(-self.sizes().raw_ascent() + style.baseline_shift()) + 0.5) as scalar,
        );
        decorations.paint(painter, style, context, corrected_baseline);
        painter.restore();
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L435-L541 (chrome/m156)
    fn justify(&mut self, owner: &mut Paragraph, max_width: scalar) {
        let mut whitespace_patches: i32 = 0;
        let mut text_len: scalar = 0.0;
        let mut whitespace_len: scalar = 0.0;
        let mut whitespace_patch = false;
        // Take leading whitespaces width but do not increment a whitespace patch number
        let mut leading_whitespaces = false;
        self.iterate_through_clusters_in_glyphs_order(
            owner,
            false,
            false,
            &mut |cluster: &Cluster, index: ClusterIndex, _ghost: bool| {
                if cluster.is_whitespace_break() {
                    if index == 0 {
                        leading_whitespaces = true;
                    } else if !whitespace_patch && !leading_whitespaces {
                        // We only count patches BETWEEN words, not before
                        whitespace_patches += 1;
                    }
                    whitespace_patch = !leading_whitespaces;
                    whitespace_len += cluster.width();
                } else if cluster.is_ideographic() {
                    // Whitespace break before and after
                    if !whitespace_patch && index != 0 {
                        // We only count patches BETWEEN words, not before
                        whitespace_patches += 1; // before
                    }
                    whitespace_patch = true;
                    leading_whitespaces = false;
                    whitespace_patches += 1; // after
                } else {
                    whitespace_patch = false;
                    leading_whitespaces = false;
                }
                text_len += cluster.width();
                true
            },
        );

        if whitespace_patch {
            // We only count patches BETWEEN words, not after
            whitespace_patches -= 1;
        }
        if whitespace_patches == 0 {
            if owner.paragraph_style().text_direction() == TextDirection::RTL {
                // Justify -> Right align
                self.shift = max_width - text_len;
            }
            return;
        }

        let step = (max_width - text_len + whitespace_len) / whitespace_patches as scalar;
        let mut shift: scalar = 0.0;
        let mut prev_shift: scalar = 0.0;

        // Deal with the ghost spaces
        let ghost_shift = max_width - self.advance.x;
        // Spread the extra whitespaces
        whitespace_patch = false;
        // Do not break on leading whitespaces
        leading_whitespaces = false;

        // The visit shifts the clusters of the paragraph's runs, so the order of the clusters is
        // collected first.
        let order = self.clusters_in_glyphs_order(owner, false, true);
        for (cluster_index, index, ghost) in order {
            let cluster = owner.cluster(cluster_index).clone();

            if ghost {
                if cluster.run(owner.runs()).left_to_right() {
                    Self::shift_cluster(owner, &cluster, ghost_shift, ghost_shift);
                }
                continue;
            }

            if cluster.is_whitespace_break() {
                if index == 0 {
                    leading_whitespaces = true;
                } else if !whitespace_patch && !leading_whitespaces {
                    shift += step;
                    whitespace_patch = true;
                    whitespace_patches -= 1;
                }
                shift -= cluster.width();
            } else if cluster.is_ideographic() {
                if !whitespace_patch && index != 0 {
                    shift += step;
                    whitespace_patches -= 1;
                }
                whitespace_patch = false;
                leading_whitespaces = false;
            } else {
                whitespace_patch = false;
                leading_whitespaces = false;
            }
            Self::shift_cluster(owner, &cluster, shift, prev_shift);
            prev_shift = shift;

            // We skip ideographic whitespaces
            if !cluster.is_whitespace_break() && cluster.is_ideographic() {
                shift += step;
                whitespace_patch = true;
                whitespace_patches -= 1;
            }
        }

        if whitespace_patch && whitespace_patches < 0 {
            whitespace_patches += 1;
            shift -= step;
        }

        // `SkAssertResult(nearlyEqual(shift, maxWidth - textLen))`: the assert result is kept in
        // release builds.
        let ok = nearly_equal_default(shift, max_width - text_len);
        assert!(ok);
        debug_assert!(whitespace_patches == 0);

        self.width_with_spaces += ghost_shift;
        self.advance.x = max_width;
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L543-L566 (chrome/m156)
    fn shift_cluster(owner: &mut Paragraph, cluster: &Cluster, shift: scalar, prev_shift: scalar) {
        let run = &mut owner.runs[cluster.run_index()];
        let start = cluster.start_pos();
        let mut end = cluster.end_pos();

        if end == run.size() {
            // Set the same shift for the fake last glyph (to avoid all extra checks)
            end += 1;
        }

        if run.justification_shifts.is_empty() {
            // Do not fill this array until needed
            let size = run.size() + 1;
            run.justification_shifts.resize(size, Point::new(0.0, 0.0));
        }

        for pos in start..end {
            run.justification_shifts[pos] = Point::new(shift, prev_shift);
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L568-L620 (chrome/m156)
    /// `createEllipsis(maxWidth, ellipsis, ltr)`: replaces some clusters with the ellipsis.
    #[doc(alias = "createEllipsis")]
    pub fn create_ellipsis(
        &mut self,
        owner: &Paragraph,
        max_width: scalar,
        ellipsis: &[u8],
        _ltr: bool,
    ) {
        // Go through the clusters in the reverse logical order
        // taking off cluster by cluster until the ellipsis fits
        let mut width = self.advance.x;
        let mut last_run: RunIndex = EMPTY_RUN;
        let mut ellipsis_run: Option<Run> = None;
        let mut cluster_index = self.ghost_cluster_range.end;
        while cluster_index > self.ghost_cluster_range.start {
            let cluster = owner.cluster(cluster_index - 1);

            // Shape the ellipsis if the run has changed
            if last_run != cluster.run_index() {
                let run = self.shape_ellipsis(owner, ellipsis, cluster, false);
                if run.advance().x > max_width {
                    // Ellipsis is bigger than the entire line; no way we can add it at all
                    // BUT! We can keep scanning in case the next run will give us better results
                    last_run = EMPTY_RUN;
                    ellipsis_run = Some(run);
                    cluster_index -= 1;
                    continue;
                }
                // We may need to continue
                last_run = cluster.run_index();
                ellipsis_run = Some(run);
            }

            // See if it fits
            let ellipsis_advance = ellipsis_run
                .as_ref()
                .expect("a shaped ellipsis")
                .advance()
                .x;
            if width + ellipsis_advance > max_width {
                width -= cluster.width();
                // Continue if the ellipsis does not fit
                cluster_index -= 1;
                continue;
            }

            // We found enough room for the ellipsis
            self.advance.x = width;
            let mut run = ellipsis_run.take().expect("a shaped ellipsis");
            // Let's update the line
            self.cluster_range.end = cluster_index;
            self.ghost_cluster_range.end = self.cluster_range.end;
            run.cluster_start = cluster.text_range().start;
            self.text.end = cluster.text_range().end;
            self.text_including_newlines.end = cluster.text_range().end;
            self.text_excluding_spaces.end = cluster.text_range().end;
            self.ellipsis = Some(Box::new(run));
            break;
        }

        if self.ellipsis.is_none() {
            // Weird situation: ellipsis does not fit; no ellipsis then
            self.cluster_range.end = self.cluster_range.start;
            self.ghost_cluster_range.end = self.cluster_range.start;
            self.text.end = self.text.start;
            self.text_including_newlines.end = self.text_including_newlines.start;
            self.text_excluding_spaces.end = self.text_excluding_spaces.start;
            self.advance.x = 0.0;
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L622-L640 (chrome/m156)
    /// `createSoftHyphen()`.
    #[doc(alias = "createSoftHyphen")]
    pub fn create_soft_hyphen(&mut self, owner: &Paragraph) {
        if self.ellipsis.is_some() {
            return;
        }
        if self.cluster_range.width() == 0 {
            return;
        }

        let last_cluster = owner.cluster(self.cluster_range.end - 1);
        if !last_cluster.is_soft_hyphen(owner.text()) {
            return;
        }

        let hyphen_str = b"-";
        let mut hyphen = self.shape_ellipsis(owner, hyphen_str, last_cluster, true);
        hyphen.cluster_start = last_cluster.text_range().start;
        self.hyphen = Some(Box::new(hyphen));
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L642-L812 (chrome/m156)
    fn shape_ellipsis(
        &self,
        owner: &Paragraph,
        ellipsis: &[u8],
        cluster: &Cluster,
        is_hyphen: bool,
    ) -> Run {
        let run = cluster.run(owner.runs());
        let mut text_style = owner.paragraph_style().text_style().clone();
        for i in self.block_range.clone() {
            let block = owner.block(i);
            if run.left_to_right() && cluster.text_range().end <= block.range.end {
                text_style = block.style.clone();
                break;
            } else if !run.left_to_right() && cluster.text_range().start <= block.range.end {
                text_style = block.style.clone();
                break;
            }
        }

        let shaped = |typeface: Option<Typeface>, fallback: Option<FontMgr>| -> Run {
            let mut handler = ShapeHandler {
                run: None,
                line_height: run.height_multiplier(),
                use_half_leading: run.use_half_leading(),
                baseline_shift: run.baseline_shift(),
                is_hyphen,
            };
            let mut font = Font::from_typeface(typeface);
            font.set_size(text_style.font_size());
            font.set_edging(text_style.font_edging());
            font.set_hinting(text_style.font_hinting());
            font.set_subpixel(text_style.subpixel());

            let fallback_mgr = fallback.unwrap_or_else(FontMgr::empty);
            let shaper = harfbuzz::shape_dont_wrap_or_reorder(
                owner.unicode().clone(),
                Some(fallback_mgr.clone()),
            );

            // `SkBidiIterator::kLTR`
            let default_level: u8 = 0;
            let utf8 = ellipsis;

            let mut bidi = unicode_bidi_run_iterator(&**owner.unicode(), utf8, default_level)
                .expect("a bidi run iterator for the ellipsis");
            let mut language = make_std_language_run_iterator(utf8.len());
            let mut script = harfbuzz::script_run_iterator(utf8);
            let mut font_runs = make_font_mgr_run_iterator(utf8, &font, Some(fallback_mgr));

            shaper.shape(
                utf8,
                &mut font_runs,
                &mut *bidi,
                &mut *script,
                &mut language,
                &[],
                &skia_rust_shaper::Options::default(),
                &mut handler,
            );
            let mut ellipsis_run = handler.run.take().expect("the shaper made a run");
            ellipsis_run.text_range = 0..ellipsis.len();
            ellipsis_run
        };

        // Check the current font
        let mut ellipsis_run = shaped(Some(run.font.typeface().clone()), None);
        if ellipsis_run.is_resolved() {
            return ellipsis_run;
        }

        // Check all allowed fonts
        let typefaces = owner.font_collection().find_typefaces_with_font_arguments(
            text_style.font_families(),
            text_style.font_style(),
            text_style.font_arguments(),
        );
        for typeface in typefaces {
            ellipsis_run = shaped(Some(typeface), None);
            if ellipsis_run.is_resolved() {
                return ellipsis_run;
            }
        }

        // Try the fallback
        if owner.font_collection().font_fallback_enabled() {
            let mut ch = ellipsis;
            let unicode = skia_rust_core::utf::next_utf8_with_replacement(&mut ch);
            // We do not expect emojis in ellipsis so if they appeat there
            // they will not be resolved with the pretiest color emoji font
            let typeface = owner.font_collection().default_fallback_char(
                unicode,
                text_style.font_families(),
                text_style.font_style(),
                text_style.locale(),
                text_style.font_arguments(),
            );
            if let Some(typeface) = typeface {
                ellipsis_run = shaped(Some(typeface), owner.font_collection().fallback_manager());
                if ellipsis_run.is_resolved() {
                    return ellipsis_run;
                }
            }
        }
        ellipsis_run
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L814-L973 (chrome/m156)
    /// `measureTextInsideOneRun(textRange, run, runOffsetInLine, textOffsetInRunInLine,
    /// includeGhostSpaces, textAdjustment)`.
    #[doc(alias = "measureTextInsideOneRun")]
    pub fn measure_text_inside_one_run<'a>(
        &self,
        owner: &Paragraph,
        text_range: TextRange,
        run: &'a Run,
        run_offset_in_line: scalar,
        text_offset_in_run_in_line: scalar,
        include_ghost_spaces: bool,
        text_adjustment: TextAdjustment,
    ) -> ClipContext<'a> {
        let mut result = ClipContext {
            run,
            pos: 0,
            size: run.size(),
            text_shift: 0.0,
            clip: Rect::from_xywh(0.0, 0.0, 0.0, 0.0),
            excluded_trailing_spaces: 0.0,
            clipping_needed: false,
        };

        if run.ellipsis {
            // Both ellipsis and placeholders can only be measured as one glyph
            result.text_shift = run_offset_in_line;
            result.clip = Rect::from_xywh(
                run_offset_in_line,
                self.sizes().run_top(run, self.ascent_style),
                run.advance().x,
                run.calculate_height(self.ascent_style, self.descent_style),
            );
            return result;
        } else if run.is_placeholder() {
            result.text_shift = run_offset_in_line;
            if skia_rust_core::floating_point::is_finite(run.font_metrics.ascent) {
                result.clip = Rect::from_xywh(
                    run_offset_in_line,
                    self.sizes().run_top(run, self.ascent_style),
                    run.advance().x,
                    run.calculate_height(self.ascent_style, self.descent_style),
                );
            } else {
                result.clip = Rect::from_xywh(
                    run_offset_in_line,
                    run.font_metrics.ascent,
                    run.advance().x,
                    0.0,
                );
            }
            return result;
        } else if text_range.empty() {
            return result;
        }

        let original_text_range = text_range.clone(); // We need it for proportional measurement
        let mut text_range = text_range;
        // Find [start:end] clusters for the text
        loop {
            // Update textRange by cluster edges (shift start up to the edge of the cluster)
            // TODO: remove this limitation?
            let (found, start, end) = Run::find_limiting_glyph_clusters(
                &owner.code_unit_properties,
                owner.text().len(),
                &text_range,
            );
            let mut updated_text_range: TextRange = start..end;
            if !found {
                return result;
            }

            if (text_adjustment.bits() & TextAdjustment::GRAPHEME.bits()) == 0 {
                text_range = updated_text_range;
                break;
            }

            // Update text range by grapheme edges (shift start up to the edge of the grapheme)
            let (_found, start, end) = Run::find_limiting_graphemes(
                &owner.code_unit_properties,
                owner.text().len(),
                &updated_text_range,
            );
            updated_text_range = start..end;
            if updated_text_range == text_range {
                break;
            }

            // Some clusters are inside graphemes and we need to adjust them
            text_range = updated_text_range;

            // Move the start until it's on the grapheme edge (and glypheme, too)
        }

        let mut start = owner.cluster(owner.cluster_index(text_range.start));
        let mut end = owner.cluster(
            owner.cluster_index(text_range.end - if text_range.width() == 0 { 0 } else { 1 }),
        );

        if !run.left_to_right() {
            std::mem::swap(&mut start, &mut end);
        }
        result.pos = start.start_pos();
        result.size = (if end.is_hard_break() {
            end.start_pos()
        } else {
            end.end_pos()
        }) - start.start_pos();
        let text_start_in_run = run.position_x(start.start_pos());
        let mut text_start_in_line = run_offset_in_line + text_offset_in_run_in_line;
        if !run.left_to_right() {
            std::mem::swap(&mut start, &mut end);
        }

        // Calculate the clipping rectangle for the text with cluster edges
        // There are 2 cases:
        // EOL (when we expect the last cluster clipped without any spaces)
        // Anything else (when we want the cluster width contain all the spaces -
        // coming from letter spacing or word spacing or justification)
        result.clip = Rect::from_xywh(
            0.0,
            self.sizes().run_top(run, self.ascent_style),
            run.calculate_width(result.pos, result.pos + result.size, false),
            run.calculate_height(self.ascent_style, self.descent_style),
        );

        // Correct the width in case the text edges don't match clusters
        // TODO: This is where we get smart about selecting a part of a cluster
        //  by shaping each grapheme separately and then use the result sizes
        //  to calculate the proportions
        let left_correction = start.size_to_char(original_text_range.start);
        let right_correction = end.size_from_char(original_text_range.end - 1);
        #[allow(clippy::float_cmp)] // mirrors `leftCorrection != 0 || rightCorrection != 0`
        {
            result.clipping_needed = left_correction != 0.0 || right_correction != 0.0;
        }
        if run.left_to_right() {
            result.clip.left += left_correction;
            result.clip.right -= right_correction;
            text_start_in_line -= left_correction;
        } else {
            result.clip.right -= left_correction;
            result.clip.left += right_correction;
            text_start_in_line -= right_correction;
        }

        result.clip.offset((text_start_in_line, 0.0));

        if compare_round(
            result.clip.right,
            self.advance.x,
            owner.get_apply_rounding_hack(),
        ) > 0
            && !include_ghost_spaces
        {
            // There are few cases when we need it.
            // The most important one: we measure the text with spaces at the end (or at the beginning in RTL)
            // and we should ignore these spaces
            if owner.paragraph_style().text_direction() == TextDirection::LTR {
                // We only use this member for LTR
                let excluded = result.clip.right - self.advance.x;
                result.excluded_trailing_spaces = if excluded < 0.0 { 0.0 } else { excluded };
                result.clipping_needed = true;
                result.clip.right = self.advance.x;
            }
        }

        if result.clip.width() < 0.0 {
            // Weird situation when glyph offsets move the glyph to the left
            // (happens with zalgo texts, for instance)
            result.clip.right = result.clip.left;
        }

        // The text must be aligned with the lineOffset
        result.text_shift = text_start_in_line - text_start_in_run;

        result
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L975-L1008 (chrome/m156)
    /// The clusters visited by `iterateThroughClustersInGlyphsOrder(reversed, includeGhosts, ..)`
    /// in order: the cluster index, the visitor's `index` and the ghost flag.
    fn clusters_in_glyphs_order(
        &self,
        owner: &Paragraph,
        reversed: bool,
        include_ghosts: bool,
    ) -> Vec<(ClusterIndex, ClusterIndex, bool)> {
        let mut order = Vec::new();
        // Walk through the clusters in the logical order (or reverse)
        let mut index: ClusterIndex = 0;
        let mut visit_run = |r: usize| {
            let run = owner.run(r);
            let trimmed_range = self.cluster_range.intersection(&run.cluster_range());
            let trailed_range = self.ghost_cluster_range.intersection(&run.cluster_range());
            debug_assert!(trimmed_range.start == trailed_range.start);

            // `fOwner->clusters(range)` asserts that the range is in the clusters.
            let forward = reversed == run.left_to_right();
            let mut visit_cluster = |cluster_index: usize| {
                let ghost = cluster_index >= trimmed_range.end;
                if !include_ghosts && ghost {
                    return;
                }
                order.push((cluster_index, index, ghost));
                index += 1;
            };
            // `directional_for_each(trailed, forwards = reversed != run.leftToRight(), ...)`
            if !forward {
                for cluster_index in trailed_range.clone() {
                    visit_cluster(cluster_index);
                }
            } else {
                for cluster_index in trailed_range.clone().rev() {
                    visit_cluster(cluster_index);
                }
            }
        };
        // `directional_for_each(runs, !reversed, ...)`
        if !reversed {
            for &r in &self.runs_in_visual_order {
                visit_run(r);
            }
        } else {
            for &r in self.runs_in_visual_order.iter().rev() {
                visit_run(r);
            }
        }
        order
    }

    /// `iterateThroughClustersInGlyphsOrder(reverse, includeGhosts, visitor)`: the visitor
    /// returns false to stop.
    #[doc(alias = "iterateThroughClustersInGlyphsOrder")]
    pub fn iterate_through_clusters_in_glyphs_order(
        &self,
        owner: &Paragraph,
        reversed: bool,
        include_ghosts: bool,
        visitor: &mut dyn FnMut(&Cluster, ClusterIndex, bool) -> bool,
    ) {
        for (cluster_index, index, ghost) in
            self.clusters_in_glyphs_order(owner, reversed, include_ghosts)
        {
            if !visitor(owner.cluster(cluster_index), index, ghost) {
                break;
            }
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1010-L1124 (chrome/m156)
    /// `iterateThroughSingleRunByStyles(textAdjustment, run, runOffset, textRange, styleType,
    /// visitor)`.
    #[doc(alias = "iterateThroughSingleRunByStyles")]
    pub fn iterate_through_single_run_by_styles(
        &self,
        owner: &Paragraph,
        text_adjustment: TextAdjustment,
        run: &Run,
        run_offset: scalar,
        text_range: TextRange,
        style_type: StyleType,
        visitor: &mut dyn FnMut(TextRange, &TextStyle, &ClipContext<'_>),
    ) -> scalar {
        let correct_context =
            |text_range: TextRange, text_offset_in_run: scalar| -> ClipContext<'_> {
                let mut result = self.measure_text_inside_one_run(
                    owner,
                    text_range,
                    run,
                    run_offset,
                    text_offset_in_run,
                    false,
                    text_adjustment,
                );
                if style_type == StyleType::Decorations {
                    // Decorations are drawn based on the real font metrics (regardless of styles and strut)
                    result.clip.top = self.sizes().run_top(run, LineMetricStyle::CSS);
                    result.clip.bottom = result.clip.top
                        + run.calculate_height(LineMetricStyle::CSS, LineMetricStyle::CSS);
                }
                result
            };

        if run.ellipsis {
            // Extra efforts to get the ellipsis text style
            let clip_context = correct_context(run.text_range(), 0.0);
            let test_range: TextRange =
                run.cluster_start..(run.cluster_start + run.text_range().width());
            for index in self.block_range.clone() {
                let block = &owner.styles()[index];
                let intersect = intersected(&block.range, &test_range);
                if intersect.width() > 0 {
                    visitor(test_range, &block.style, &clip_context);
                    return run.advance().x;
                }
            }
            debug_assert!(false);
        }

        if style_type == StyleType::None {
            let clip_context = correct_context(text_range.clone(), 0.0);
            // The placehoder can have height=0 or (exclusively) width=0 and still be a thing
            if clip_context.clip.height() > 0.0 || clip_context.clip.width() > 0.0 {
                visitor(text_range, &TextStyle::default(), &clip_context);
                return clip_context.clip.width();
            }
            return 0.0;
        }

        let mut start: usize = EMPTY_INDEX;
        let mut size: usize = 0;
        let mut prev_style: Option<&TextStyle> = None;
        let mut text_offset_in_run: scalar = 0.0;

        let block_range_size = self.block_range.end - self.block_range.start;
        let mut index: usize = 0;
        while index <= block_range_size {
            let mut intersect: TextRange = 0..0;
            let mut style: Option<&TextStyle> = None;
            if index < block_range_size {
                let block = &owner.styles()[if run.left_to_right() {
                    self.block_range.start + index
                } else {
                    self.block_range.end - index - 1
                }];

                // Get the text
                intersect = intersected(&block.range, &text_range);
                if intersect.width() == 0 {
                    if start == EMPTY_INDEX {
                        // This style is not applicable to the text yet
                        index += 1;
                        continue;
                    }
                    // We have found all the good styles already
                    // but we need to process the last one of them
                    intersect = start..(start + size);
                    index = self.block_range.end;
                } else {
                    // Get the style
                    style = Some(&block.style);
                    if start != EMPTY_INDEX
                        && block
                            .style
                            .match_one_attribute(style_type, prev_style.expect("a previous style"))
                    {
                        size += intersect.width();
                        // RTL text intervals move backward
                        start = intersect.start.min(start);
                        index += 1;
                        continue;
                    } else if start == EMPTY_INDEX {
                        // First time only
                        prev_style = style;
                        size = intersect.width();
                        start = intersect.start;
                        index += 1;
                        continue;
                    }
                }
            } else if prev_style.is_some() {
                // This is the last style
            } else {
                break;
            }

            // We have the style and the text
            let run_style_text_range: TextRange = start..(start + size);
            let clip_context = correct_context(run_style_text_range.clone(), text_offset_in_run);
            text_offset_in_run += clip_context.clip.width();
            if clip_context.clip.height() == 0.0 {
                index += 1;
                continue;
            }
            visitor(
                run_style_text_range,
                prev_style.expect("a previous style"),
                &clip_context,
            );

            // Start all over again
            prev_style = style;
            start = intersect.start;
            size = intersect.width();
            index += 1;
        }
        text_offset_in_run
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1126-L1189 (chrome/m156)
    /// `iterateThroughVisualRuns(includingGhostSpaces, visitor)`: the visitor returns false to
    /// stop.
    #[doc(alias = "iterateThroughVisualRuns")]
    pub fn iterate_through_visual_runs(
        &self,
        owner: &Paragraph,
        including_ghost_spaces: bool,
        visitor: &mut dyn FnMut(&Run, scalar, TextRange, &mut scalar) -> bool,
    ) {
        // Walk through all the runs that intersect with the line in visual order
        let mut width: scalar = 0.0;
        let mut run_offset: scalar = 0.0;
        let mut total_width: scalar = 0.0;
        let text_range = if including_ghost_spaces {
            self.text_with_newlines()
        } else {
            self.trimmed_text()
        };

        if let Some(ellipsis) = self.ellipsis() {
            if owner.paragraph_style().text_direction() == TextDirection::RTL {
                run_offset = ellipsis.offset().x;
                let _ = visitor(ellipsis, run_offset, ellipsis.text_range(), &mut width);
            }
        }

        if let Some(hyphen) = self.hyphen() {
            if owner.paragraph_style().text_direction() == TextDirection::RTL {
                run_offset = hyphen.offset().x;
                let _ = visitor(hyphen, run_offset, hyphen.text_range(), &mut width);
            }
        }

        for &run_index in &self.runs_in_visual_order {
            let run = owner.run(run_index);
            let line_intersection = intersected(&run.text_range(), &text_range);
            if line_intersection.width() == 0 && self.width() != 0.0 {
                // TODO: deal with empty runs in a better way
                continue;
            }
            if !run.left_to_right() && run_offset == 0.0 && including_ghost_spaces {
                // runOffset does not take in account a possibility
                // that RTL run could start before the line (trailing spaces)
                // so we need to do runOffset -= "trailing whitespaces length"
                let whitespaces = intersected(
                    &(self.text_excluding_spaces.end..self.text_including_newlines.end),
                    &run.text_range,
                );
                if whitespaces.width() > 0 {
                    let whitespaces_len = self
                        .measure_text_inside_one_run(
                            owner,
                            whitespaces,
                            run,
                            run_offset,
                            0.0,
                            true,
                            TextAdjustment::GLYPH_CLUSTER,
                        )
                        .clip
                        .width();
                    run_offset -= whitespaces_len;
                }
            }
            run_offset += width;
            total_width += width;
            if !visitor(run, run_offset, line_intersection, &mut width) {
                return;
            }
        }

        run_offset += width;
        total_width += width;

        if let Some(ellipsis) = self.ellipsis() {
            if owner.paragraph_style().text_direction() == TextDirection::LTR
                && visitor(ellipsis, run_offset, ellipsis.text_range(), &mut width)
            {
                total_width += width;
            }
        }

        if let Some(hyphen) = self.hyphen() {
            if owner.paragraph_style().text_direction() == TextDirection::LTR
                && visitor(hyphen, run_offset, hyphen.text_range(), &mut width)
            {
                total_width += width;
            }
        }

        if !including_ghost_spaces
            && compare_round(total_width, self.width(), owner.get_apply_rounding_hack()) != 0
        {
            // This is a very important assert!
            // It asserts that 2 different ways of calculation come with the same results
            // (`SkDEBUGFAILF`: a debug-only failure)
            debug_assert!(false, "ASSERT: {total_width} != {}", self.width());
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1135-L1180 (chrome/m156)
    /// `getMetrics()`.
    #[must_use]
    #[doc(alias = "getMetrics")]
    pub fn get_metrics<'a>(&self, owner: &'a Paragraph) -> LineMetrics<'a> {
        let mut result = LineMetrics::default();

        // Fill out the metrics
        owner.ensure_utf16_mapping();
        result.start_index = owner.get_utf16_index(self.text_excluding_spaces.start);
        result.end_excluding_whitespaces = owner.get_utf16_index(self.text_excluding_spaces.end);
        result.end_index = owner.get_utf16_index(self.text.end);
        result.end_including_newline = owner.get_utf16_index(self.text_including_newlines.end);
        result.hard_break = self.ends_with_hard_line_break(owner);
        result.ascent = f64::from(-self.max_run_metrics.ascent());
        result.descent = f64::from(self.max_run_metrics.descent());
        result.unscaled_ascent = f64::from(-self.max_run_metrics.ascent()); // TODO: implement
        result.height = f64::from(self.advance.y);
        result.width = f64::from(self.advance.x);
        if owner.get_apply_rounding_hack() {
            result.height = f64::from(little_round(result.height as scalar));
            result.width = f64::from(little_round(result.width as scalar));
        }
        result.left = f64::from(self.offset().x);
        // This is Flutter definition of a baseline
        result.baseline = f64::from(self.offset().y + self.height() - self.sizes().descent());
        result.line_number = self.index;

        // Fill out the style parts
        self.iterate_through_visual_runs(
            owner,
            false,
            &mut |run: &Run,
                  run_offset_in_line,
                  text_range: TextRange,
                  run_width_in_line: &mut scalar| {
                if run.is_placeholder() {
                    *run_width_in_line = run.advance().x;
                    return true;
                }
                *run_width_in_line = self.iterate_through_single_run_by_styles(
                    owner,
                    TextAdjustment::GLYPH_CLUSTER,
                    run,
                    run_offset_in_line,
                    text_range,
                    StyleType::Foreground,
                    &mut |text_range: TextRange, style: &TextStyle, _context: &ClipContext<'_>| {
                        let (_, font_metrics) = run.font.metrics();
                        // `style` points into the paragraph's blocks, which outlive `owner`'s
                        // borrow; find it again to give the metrics the paragraph's lifetime.
                        let style = owner
                            .styles()
                            .iter()
                            .map(|block| &block.style)
                            .find(|candidate| std::ptr::eq(*candidate, style))
                            .unwrap_or_else(|| owner.paragraph_style().text_style());
                        let style_metrics = StyleMetrics::new(style, font_metrics);
                        result.emplace_style_metrics(text_range.start, style_metrics);
                    },
                );
                true
            },
        );

        result
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1182-L1188 (chrome/m156)
    /// `isFirstLine()`.
    #[must_use]
    #[doc(alias = "isFirstLine")]
    pub fn is_first_line(&self) -> bool {
        self.index == 0
    }

    /// `isLastLine()`.
    #[must_use]
    #[doc(alias = "isLastLine")]
    pub fn is_last_line(&self, owner: &Paragraph) -> bool {
        self.index + 1 == owner.lines().len()
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1190-L1196 (chrome/m156)
    /// `endsWithHardLineBreak()`.
    #[must_use]
    #[doc(alias = "endsWithHardLineBreak")]
    pub fn ends_with_hard_line_break(&self, owner: &Paragraph) -> bool {
        // TODO: For some reason Flutter imagines a hard line break at the end of the last line.
        //  To be removed...
        (self.ghost_cluster_range.width() > 0
            && owner
                .cluster(self.ghost_cluster_range.end - 1)
                .is_hard_break())
            || self.ellipsis.is_some()
            || self.ghost_cluster_range.end == owner.clusters().len() - 1
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1198-L1382 (chrome/m156)
    /// `getRectsForRange(textRange0, rectHeightStyle, rectWidthStyle, boxes)`.
    #[doc(alias = "getRectsForRange")]
    pub fn get_rects_for_range(
        &self,
        owner: &Paragraph,
        text_range0: TextRange,
        rect_height_style: RectHeightStyle,
        rect_width_style: RectWidthStyle,
        boxes: &mut Vec<TextBox>,
    ) {
        let mut last_run: Option<RunRef> = None;
        let start_box = boxes.len();
        self.iterate_through_visual_runs(
            owner,
            true,
            &mut |run: &Run,
                  run_offset_in_line,
                  text_range: TextRange,
                  run_width_in_line: &mut scalar| {
                // The rendered soft hyphen is a synthetic glyph run that does not correspond
                // to any source text. Its synthetic textRange (offsets into the "-" string)
                // would falsely intersect caller-supplied source ranges and produce a
                // spurious selection rect at the line-end hyphen position. Skip it.
                if run.is_hyphen() {
                    return true;
                }
                *run_width_in_line = self.iterate_through_single_run_by_styles(
                    owner,
                    TextAdjustment::GRAPHEME_GLUSTER,
                    run,
                    run_offset_in_line,
                    text_range,
                    StyleType::None,
                    &mut |text_range: TextRange,
                          _style: &TextStyle,
                          line_context: &ClipContext<'_>| {
                        let intersect = range_intersection(&text_range, &text_range0);
                        if intersect.empty() {
                            return;
                        }

                        let paragraph_style = owner.paragraph_style();

                        // Found a run that intersects with the text
                        let context = self.measure_text_inside_one_run(
                            owner,
                            intersect.clone(),
                            run,
                            run_offset_in_line,
                            0.0,
                            true,
                            TextAdjustment::GRAPHEME_GLUSTER,
                        );
                        let mut clip = context.clip;
                        clip.offset((line_context.text_shift - context.text_shift, 0.0));

                        match rect_height_style {
                            RectHeightStyle::Max => {
                                // TODO: Change it once flutter rolls into google3
                                //  (probably will break things if changed before)
                                clip.bottom = self.height();
                                clip.top = self.sizes().delta();
                            }
                            RectHeightStyle::IncludeLineSpacingTop => {
                                clip.bottom = self.height();
                                clip.top = self.sizes().delta();
                                let vertical_shift =
                                    self.sizes().raw_ascent() - self.sizes().ascent();
                                if self.is_first_line() {
                                    clip.top += vertical_shift;
                                }
                            }
                            RectHeightStyle::IncludeLineSpacingMiddle => {
                                clip.bottom = self.height();
                                clip.top = self.sizes().delta();
                                let vertical_shift =
                                    self.sizes().raw_ascent() - self.sizes().ascent();
                                clip.offset((0.0, (f64::from(vertical_shift) / 2.0) as scalar));
                                if self.is_first_line() {
                                    clip.top += (f64::from(vertical_shift) / 2.0) as scalar;
                                }
                                if self.is_last_line(owner) {
                                    clip.bottom -= (f64::from(vertical_shift) / 2.0) as scalar;
                                }
                            }
                            RectHeightStyle::IncludeLineSpacingBottom => {
                                clip.bottom = self.height();
                                clip.top = self.sizes().delta();
                                let vertical_shift =
                                    self.sizes().raw_ascent() - self.sizes().ascent();
                                clip.offset((0.0, vertical_shift));
                                if self.is_last_line(owner) {
                                    clip.bottom -= vertical_shift;
                                }
                            }
                            RectHeightStyle::Strut => {
                                let strut_style = paragraph_style.strut_style();
                                if strut_style.strut_enabled() && strut_style.font_size() > 0.0 {
                                    let strut_metrics = owner.strut_metrics();
                                    let top = self.baseline();
                                    clip.top = top + strut_metrics.ascent();
                                    clip.bottom = top + strut_metrics.descent();
                                }
                            }
                            RectHeightStyle::Tight => {
                                if run.height_multiplier <= 0.0 {
                                    // break
                                } else {
                                    let effective_baseline = self.baseline() + self.sizes().delta();
                                    clip.top = effective_baseline + run.ascent();
                                    clip.bottom = effective_baseline + run.descent();
                                }
                            }
                        }

                        // Separate trailing spaces and move them in the default order of the paragraph
                        // in case the run order and the paragraph order don't match
                        let mut trailing_spaces = Rect::from_xywh(0.0, 0.0, 0.0, 0.0);
                        if self.trimmed_text().end < self.text_with_newlines().end // Line has trailing space
                            && self.text_with_newlines().end == intersect.end // Range is at the end of the line
                            && self.trimmed_text().end > intersect.start
                        // Range has more than just spaces
                        {
                            let delta = self.spaces_width();
                            trailing_spaces = Rect::from_xywh(0.0, 0.0, 0.0, 0.0);
                            // There are trailing spaces in this run
                            if paragraph_style.text_align() == TextAlign::Justify
                                && self.is_last_line(owner)
                            {
                                // TODO: this is just a patch. Make it right later (when it's clear what and how)
                                trailing_spaces = clip;
                                if run.left_to_right() {
                                    trailing_spaces.left = self.width();
                                    clip.right = self.width();
                                } else {
                                    trailing_spaces.right = 0.0;
                                    clip.left = 0.0;
                                }
                            } else if paragraph_style.text_direction() == TextDirection::RTL
                                && !run.left_to_right()
                            {
                                // Split
                                trailing_spaces = clip;
                                trailing_spaces.left = -delta;
                                trailing_spaces.right = 0.0;
                                clip.left += delta;
                            } else if paragraph_style.text_direction() == TextDirection::LTR
                                && run.left_to_right()
                            {
                                // Split
                                trailing_spaces = clip;
                                trailing_spaces.left = self.width();
                                trailing_spaces.right = trailing_spaces.left + delta;
                                clip.right -= delta;
                            }
                        }

                        clip.offset(self.offset());
                        if trailing_spaces.width() > 0.0 {
                            trailing_spaces.offset(self.offset());
                        }

                        // Check if we can merge two boxes instead of adding a new one
                        let mut merge = |clip: Rect, boxes: &mut Vec<TextBox>| -> bool {
                            let mut merged_boxes = false;
                            if !boxes.is_empty() {
                                if let Some(last_run_ref) = last_run {
                                    let last_run_value = self.run_of(owner, last_run_ref);
                                    if context.run.left_to_right() == last_run_value.left_to_right()
                                        && !last_run_value.is_placeholder()
                                        && !context.run.is_placeholder()
                                        && nearly_equal_default(
                                            last_run_value.height_multiplier(),
                                            context.run.height_multiplier(),
                                        )
                                        && last_run_value.font() == context.run.font()
                                    {
                                        let last_box = boxes.last_mut().expect("a box");
                                        if nearly_equal_default(last_box.rect.top, clip.top)
                                            && nearly_equal_default(
                                                last_box.rect.bottom,
                                                clip.bottom,
                                            )
                                            && (nearly_equal_default(
                                                last_box.rect.left,
                                                clip.right,
                                            ) || nearly_equal_default(
                                                last_box.rect.right,
                                                clip.left,
                                            ))
                                        {
                                            // `std::min(a, b)` is `b < a ? b : a`.
                                            last_box.rect.left = if clip.left < last_box.rect.left {
                                                clip.left
                                            } else {
                                                last_box.rect.left
                                            };
                                            // `std::max(a, b)` is `a < b ? b : a`.
                                            last_box.rect.right =
                                                if last_box.rect.right < clip.right {
                                                    clip.right
                                                } else {
                                                    last_box.rect.right
                                                };
                                            merged_boxes = true;
                                        }
                                    }
                                }
                            }
                            last_run = Some(self.run_ref(context.run));
                            merged_boxes
                        };

                        if !merge(clip, boxes) {
                            boxes.push(TextBox::new(clip, context.run.text_direction()));
                        }
                        if !nearly_zero_default(trailing_spaces.width())
                            && !merge(trailing_spaces, boxes)
                        {
                            boxes.push(TextBox::new(
                                trailing_spaces,
                                paragraph_style.text_direction(),
                            ));
                        }

                        if rect_width_style == RectWidthStyle::Max && !self.is_last_line(owner) {
                            // Align the very left/right box horizontally
                            let line_start = self.offset().x;
                            let line_end = self.offset().x + self.width();
                            let mut left = boxes[start_box];
                            let mut right = *boxes.last().expect("a box");
                            if left.rect.left > line_start && left.direct == TextDirection::RTL {
                                left.rect.right = left.rect.left;
                                left.rect.left = 0.0;
                                boxes.insert(start_box + 1, left);
                            }
                            if right.direct == TextDirection::LTR
                                && right.rect.right >= line_end
                                && right.rect.right < owner.width_with_trailing_spaces()
                            {
                                right.rect.left = right.rect.right;
                                right.rect.right = owner.width_with_trailing_spaces();
                                boxes.push(right);
                            }
                        }
                    },
                );
                true
            },
        );
        if owner.get_apply_rounding_hack() {
            for r in boxes.iter_mut() {
                r.rect.left = little_round(r.rect.left);
                r.rect.right = little_round(r.rect.right);
                r.rect.top = little_round(r.rect.top);
                r.rect.bottom = little_round(r.rect.bottom);
            }
        }
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1384-L1534 (chrome/m156)
    /// `getGlyphPositionAtCoordinate(dx)`.
    #[must_use]
    #[doc(alias = "getGlyphPositionAtCoordinate")]
    pub fn get_glyph_position_at_coordinate(
        &self,
        owner: &Paragraph,
        dx: scalar,
    ) -> PositionWithAffinity {
        if self.width().nearly_zero(None) && self.spaces_width().nearly_zero(None) {
            // TODO: this is one of the flutter changes that have to go away eventually
            //  Empty line is a special case in txtlib (but only when there are no spaces, too)
            let utf16_index = owner.get_utf16_index(self.text_excluding_spaces.end);
            return PositionWithAffinity {
                position: utf16_index as i32,
                affinity: Affinity::Downstream,
            };
        }

        let mut result = PositionWithAffinity {
            position: 0,
            affinity: Affinity::Downstream,
        };
        self.iterate_through_visual_runs(
            owner,
            true,
            &mut |run: &Run,
                  run_offset_in_line,
                  text_range: TextRange,
                  run_width_in_line: &mut scalar| {
                if run.is_ellipsis() {
                    let utf16_index = owner.get_utf16_index(self.text.end);
                    result = PositionWithAffinity {
                        position: utf16_index as i32,
                        affinity: Affinity::Downstream,
                    };
                    return false;
                }

                let mut keep_looking = true;
                *run_width_in_line = self.iterate_through_single_run_by_styles(
                    owner,
                    TextAdjustment::GRAPHEME_GLUSTER,
                    run,
                    run_offset_in_line,
                    text_range,
                    StyleType::None,
                    &mut |_text_range: TextRange,
                          _style: &TextStyle,
                          context0: &ClipContext<'_>| {
                        let offset_x = self.offset().x;
                        let mut context = context0.clone();

                        // Correct the clip size because libtxt counts trailing spaces
                        if run.left_to_right() {
                            context.clip.right += context.excluded_trailing_spaces; // extending clip to the right
                        } else {
                            // Clip starts from 0; we cannot extend it to the left from that
                        }
                        // However, we need to offset the clip
                        context.clip.offset((offset_x, 0.0));

                        // This patch will help us to avoid a floating point error
                        if scalar::nearly_equal(context.clip.right, dx, 0.01) {
                            context.clip.right = dx;
                        }

                        if dx <= context.clip.left {
                            // All the other runs are placed right of this one
                            let utf16_index = owner
                                .get_utf16_index(context.run.global_cluster_index(context.pos));
                            if run.left_to_right() {
                                result = PositionWithAffinity {
                                    position: utf16_index as i32,
                                    affinity: Affinity::Downstream,
                                };
                                keep_looking = false;
                            } else {
                                result = PositionWithAffinity {
                                    position: (utf16_index + 1) as i32,
                                    affinity: Affinity::Upstream,
                                };
                                // If we haven't reached the end of the run we need to keep looking
                                keep_looking = context.pos != 0;
                            }
                            // For RTL we go another way
                            return;
                        }

                        if dx >= context.clip.right {
                            // We have to keep looking ; just in case keep the last one as the closest
                            let utf16_index = owner.get_utf16_index(
                                context.run.global_cluster_index(context.pos + context.size),
                            );
                            if run.left_to_right() {
                                result = PositionWithAffinity {
                                    position: utf16_index as i32,
                                    affinity: Affinity::Upstream,
                                };
                            } else {
                                result = PositionWithAffinity {
                                    position: utf16_index as i32,
                                    affinity: Affinity::Downstream,
                                };
                            }
                            // For RTL we go another way
                            // (`return run->leftToRight()`: the value of a void visitor is ignored,
                            // but `keepLooking` is left unchanged)
                            return;
                        }

                        // So we found the run that contains our coordinates
                        // Find the glyph position in the run that is the closest left of our point
                        // TODO: binary search
                        let mut found = context.pos;
                        for index in context.pos..context.pos + context.size {
                            // TODO: this rounding is done to match Flutter tests. Must be removed..
                            let mut end =
                                context.run.position_x(index) + context.text_shift + offset_x;
                            if owner.get_apply_rounding_hack() {
                                end = little_round(end);
                            }
                            if end > dx {
                                break;
                            } else if end == dx && !context.run.left_to_right() {
                                // When we move RTL variable end points to the beginning of the code point which is included
                                found = index;
                                break;
                            }
                            found = index;
                        }

                        let glypheme_pos_left =
                            context.run.position_x(found) + context.text_shift + offset_x;
                        let glyphemes_width =
                            context.run.position_x(found + 1) - context.run.position_x(found);

                        // Find the grapheme range that contains the point
                        let cluster_index8 = context.run.global_cluster_index(found);
                        let cluster_end8 = context.run.global_cluster_index(found + 1);
                        let graphemes =
                            owner.count_surrounding_graphemes(cluster_index8..cluster_end8);

                        let center = glypheme_pos_left + glyphemes_width / 2.0;
                        if graphemes.len() > 1 {
                            // Calculate the position proportionally based on grapheme count
                            let average_grapheme_width =
                                glyphemes_width / graphemes.len() as scalar;
                            let delta = dx - glypheme_pos_left;
                            let grapheme_index: i32 = if average_grapheme_width.nearly_zero(None) {
                                0
                            } else {
                                scalar_floor_to_int(delta / average_grapheme_width)
                            };
                            let grapheme_center = glypheme_pos_left
                                + grapheme_index as scalar * average_grapheme_width
                                + average_grapheme_width / 2.0;
                            let grapheme_utf8_index = graphemes[grapheme_index as usize];
                            if (dx < grapheme_center) == context.run.left_to_right() {
                                let utf16_index = owner.get_utf16_index(grapheme_utf8_index);
                                result = PositionWithAffinity {
                                    position: utf16_index as i32,
                                    affinity: Affinity::Downstream,
                                };
                            } else {
                                let utf16_index = owner.get_utf16_index(grapheme_utf8_index + 1);
                                result = PositionWithAffinity {
                                    position: utf16_index as i32,
                                    affinity: Affinity::Upstream,
                                };
                            }
                            // Keep UTF16 index as is
                        } else if (dx < center) == context.run.left_to_right() {
                            let utf16_index = owner.get_utf16_index(cluster_index8);
                            result = PositionWithAffinity {
                                position: utf16_index as i32,
                                affinity: Affinity::Downstream,
                            };
                        } else {
                            let utf16_index = if context.run.left_to_right() {
                                owner.get_utf16_index(cluster_end8)
                            } else {
                                owner.get_utf16_index(cluster_index8) + 1
                            };
                            result = PositionWithAffinity {
                                position: utf16_index as i32,
                                affinity: Affinity::Upstream,
                            };
                        }

                        keep_looking = false;
                    },
                );
                keep_looking
            },
        );
        result
    }

    // Port of: modules/skparagraph/src/TextLine.cpp#L1536-L1570 (chrome/m156)
    /// `getRectsForPlaceholders(boxes)`.
    #[doc(alias = "getRectsForPlaceholders")]
    pub fn get_rects_for_placeholders(&self, owner: &Paragraph, boxes: &mut Vec<TextBox>) {
        self.iterate_through_visual_runs(
            owner,
            true,
            &mut |run: &Run, run_offset, text_range: TextRange, width: &mut scalar| {
                let context = self.measure_text_inside_one_run(
                    owner,
                    text_range.clone(),
                    run,
                    run_offset,
                    0.0,
                    true,
                    TextAdjustment::GRAPHEME_GLUSTER,
                );
                *width = context.clip.width();

                if text_range.width() == 0 {
                    return true;
                }
                if !run.is_placeholder() {
                    return true;
                }

                let mut clip = context.clip;
                clip.offset(self.offset());

                if owner.get_apply_rounding_hack() {
                    clip.left = little_round(clip.left);
                    clip.right = little_round(clip.right);
                    clip.top = little_round(clip.top);
                    clip.bottom = little_round(clip.bottom);
                }
                boxes.push(TextBox::new(clip, run.text_direction()));
                true
            },
        );
    }

    /// `shiftVertically(shift)`.
    #[doc(alias = "shiftVertically")]
    pub fn shift_vertically(&mut self, shift: scalar) {
        self.offset.y += shift;
    }

    /// `setMaxRunMetrics(metrics)`.
    #[doc(alias = "setMaxRunMetrics")]
    pub fn set_max_run_metrics(&mut self, metrics: InternalLineMetrics) {
        self.max_run_metrics = metrics;
    }

    /// `getMaxRunMetrics()`.
    #[must_use]
    #[doc(alias = "getMaxRunMetrics")]
    pub fn max_run_metrics(&self) -> InternalLineMetrics {
        self.max_run_metrics
    }

    /// `setAscentStyle(style)`.
    #[doc(alias = "setAscentStyle")]
    pub fn set_ascent_style(&mut self, style: LineMetricStyle) {
        self.ascent_style = style;
    }

    /// `setDescentStyle(style)`.
    #[doc(alias = "setDescentStyle")]
    pub fn set_descent_style(&mut self, style: LineMetricStyle) {
        self.descent_style = style;
    }

    /// The first block of the line (`fBlockRange`).
    #[must_use]
    pub fn block_range(&self) -> BlockRange {
        self.block_range.clone()
    }

    /// `ClusterIndex` is also the line's cluster type, re-exported for the visitors.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }
}

// Port of: modules/skparagraph/src/TextLine.cpp#L650-L700 (chrome/m156), the `ShapeHandler` class
// of `TextLine::shapeEllipsis`.
struct ShapeHandler {
    run: Option<Run>,
    line_height: scalar,
    use_half_leading: bool,
    baseline_shift: scalar,
    is_hyphen: bool,
}

impl RunHandler for ShapeHandler {
    fn begin_line(&mut self) {}

    fn run_info(&mut self, _info: &RunInfo<'_>) {}

    fn commit_run_info(&mut self) {}

    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
        debug_assert!(self.run.is_none());
        let run = self.run.insert(Run::new(
            info,
            0,
            self.line_height,
            self.use_half_leading,
            self.baseline_shift,
            0,
            0.0,
        ));
        Buffer {
            glyphs: run.glyphs.as_mut_slice(),
            positions: run.positions.as_mut_slice(),
            offsets: Some(run.offsets.as_mut_slice()),
            clusters: Some(run.cluster_indexes.as_mut_slice()),
            point: run.offset,
        }
    }

    fn commit_run_buffer(&mut self, info: &RunInfo<'_>) {
        let run = self.run.as_mut().expect("a run buffer was made");
        run.advance.x = info.advance.x;
        run.advance.y = run.advance().y;
        run.placeholder_index = usize::MAX;
        run.ellipsis = true;
        run.hyphen = self.is_hyphen;
    }

    fn commit_line(&mut self) {}
}

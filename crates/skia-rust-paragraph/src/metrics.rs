// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/Metrics.h

//! `StyleMetrics` and `LineMetrics`.

use std::ops::Range;

use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::scalar::{SCALAR_MAX, SCALAR_MIN};

use crate::text_style::TextStyle;

// Port of: modules/skparagraph/include/Metrics.h#L16-L41 (chrome/m156)
/// `StyleMetrics`: the font metrics of a text style used in a line.
#[derive(Debug, Clone)]
pub struct StyleMetrics<'a> {
    /// `text_style`.
    pub text_style: &'a TextStyle,
    /// `font_metrics`: the metrics of the font of the style. `SkFontMetrics` contains the
    /// following metrics:
    ///
    /// * Top                 distance to reserve above baseline
    /// * Ascent              distance to reserve below baseline
    /// * Descent             extent below baseline
    /// * Bottom              extent below baseline
    /// * Leading             distance to add between lines
    /// * AvgCharWidth        average character width
    /// * MaxCharWidth        maximum character width
    /// * XMin                minimum x
    /// * XMax                maximum x
    /// * XHeight             height of lower-case 'x'
    /// * CapHeight           height of an upper-case letter
    /// * UnderlineThickness  underline thickness
    /// * UnderlinePosition   underline position relative to baseline
    /// * StrikeoutThickness  strikeout thickness
    /// * StrikeoutPosition   strikeout position relative to baseline
    pub font_metrics: FontMetrics,
}

impl<'a> StyleMetrics<'a> {
    // Port of: modules/skparagraph/include/Metrics.h#L18-L21 (chrome/m156)
    /// `StyleMetrics(const TextStyle* style)` and `StyleMetrics(style, metrics)`.
    #[must_use]
    pub fn new(style: &'a TextStyle, metrics: impl Into<Option<FontMetrics>>) -> Self {
        Self {
            text_style: style,
            font_metrics: metrics.into().unwrap_or_default(),
        }
    }
}

/// An entry of the `std::map<size_t, StyleMetrics>` of a [`LineMetrics`], which is a sorted
/// vector here.
#[derive(Debug, Clone)]
pub struct IndexedStyleMetrics<'a> {
    /// The key: the text index where the style starts.
    pub index: usize,
    /// The value.
    pub metrics: StyleMetrics<'a>,
}

// Port of: modules/skparagraph/include/Metrics.h#L43-L98 (chrome/m156)
/// `LineMetrics`: the metrics of a laid out line.
#[derive(Debug, Clone)]
pub struct LineMetrics<'a> {
    // The following fields are used in the layout process itself.
    /// `fStartIndex`: the index in the text buffer the line begins.
    pub start_index: usize,
    /// `fEndIndex`: the index in the text buffer the line ends.
    pub end_index: usize,
    /// `fEndExcludingWhitespaces`.
    pub end_excluding_whitespaces: usize,
    /// `fEndIncludingNewline`.
    pub end_including_newline: usize,
    /// `fHardBreak`.
    pub hard_break: bool,

    // The following fields are tracked after or during layout to provide to the user as well as
    // for computing bounding boxes.
    /// `fAscent`: the final computed ascent and descent for the line. This can be impacted by
    /// the strut, height, scaling, as well as outlying runs that are very tall.
    ///
    /// The top edge is `baseline - ascent` and the bottom edge is `baseline + descent`. Ascent
    /// and descent are provided as positive numbers. Raw numbers for specific runs of text can
    /// be obtained in `style_metrics`. These values are the cumulative metrics for the entire
    /// line.
    pub ascent: f64,
    /// `fDescent`.
    pub descent: f64,
    /// `fUnscaledAscent`.
    pub unscaled_ascent: f64,
    /// `fHeight`: total height of the paragraph including the current line.
    ///
    /// The height of the current line is `round(ascent + descent)`.
    pub height: f64,
    /// `fWidth`: width of the line.
    pub width: f64,
    /// `fLeft`: the left edge of the line. The right edge can be obtained with `left + width`.
    pub left: f64,
    /// `fBaseline`: the y position of the baseline for this line from the top of the paragraph.
    pub baseline: f64,
    /// `fLineNumber`: zero indexed line number.
    pub line_number: usize,
    /// `fLineMetrics`: mapping between text index ranges and the font metrics associated with
    /// them. The first run will be keyed under start_index. The metrics here are before layout
    /// and are the base values we calculate from. Sorted by the index.
    pub style_metrics: Vec<IndexedStyleMetrics<'a>>,
}

impl Default for LineMetrics<'_> {
    // Port of: modules/skparagraph/include/Metrics.h#L45 and #L73-L98 (chrome/m156)
    fn default() -> Self {
        Self {
            start_index: 0,
            end_index: 0,
            end_excluding_whitespaces: 0,
            end_including_newline: 0,
            hard_break: false,
            ascent: f64::from(SCALAR_MAX),
            descent: f64::from(SCALAR_MIN),
            unscaled_ascent: f64::from(SCALAR_MAX),
            height: 0.0,
            width: 0.0,
            left: 0.0,
            baseline: 0.0,
            line_number: 0,
            style_metrics: Vec::new(),
        }
    }
}

impl<'a> LineMetrics<'a> {
    /// `fLineMetrics.emplace(index, metrics)`: inserts unless the index is already there, as
    /// `std::map::emplace` does.
    pub fn emplace_style_metrics(&mut self, index: usize, metrics: StyleMetrics<'a>) {
        let at = self.style_metrics.partition_point(|ism| ism.index < index);
        if self
            .style_metrics
            .get(at)
            .is_some_and(|ism| ism.index == index)
        {
            return;
        }
        self.style_metrics
            .insert(at, IndexedStyleMetrics { index, metrics });
    }

    /// The number of style metrics in the given index range.
    #[must_use]
    pub fn get_style_metrics_count(&self, range: Range<usize>) -> usize {
        let lower = self
            .style_metrics
            .partition_point(|ism| ism.index < range.start);
        let upper = self
            .style_metrics
            .partition_point(|ism| ism.index < range.end);
        upper - lower
    }

    /// The indices and style metrics in the given range.
    #[must_use]
    pub fn get_style_metrics(&self, range: Range<usize>) -> Vec<(usize, &StyleMetrics<'a>)> {
        let lower = self
            .style_metrics
            .partition_point(|ism| ism.index < range.start);
        let upper = self
            .style_metrics
            .partition_point(|ism| ism.index < range.end);
        self.style_metrics[lower..upper]
            .iter()
            .map(|ism| (ism.index, &ism.metrics))
            .collect()
    }
}

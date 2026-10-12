// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/src/Run.h, modules/skparagraph/src/Run.cpp,
// and `Cluster::Cluster` of modules/skparagraph/src/ParagraphImpl.cpp.
//
// Skia's `Run`, `Cluster` and `TextLine` hold a pointer to their `ParagraphImpl`. Here they hold
// none: the methods that read the paragraph (its text, runs, clusters or code unit properties)
// take what they read as arguments, and `ParagraphImpl` passes its own fields.

//! `Run`, `Cluster` and `InternalLineMetrics`: the shaped text of a paragraph.

use std::ops::Range;

use skia_rust_core::font::Font;
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_types::{FourByteTag, GlyphId, set_four_byte_tag};
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_MAX, SCALAR_MIN, Scalar, double_to_scalar, scalar};
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_shaper::run_handler::RunInfo;
use skia_rust_unicode::unicode::CodeUnitFlags;

use crate::dart_types::{
    EMPTY_INDEX, EMPTY_RANGE, LineMetricStyle, RangeExtensions, TextBaseline, TextDirection,
};
use crate::text_style::{
    EMPTY_TEXT, Placeholder, PlaceholderAlignment, PlaceholderStyle, TextIndex, TextRange,
};

// Port of: modules/skparagraph/src/Run.h#L30 (chrome/m156)
/// `RunIndex`.
pub type RunIndex = usize;
// Port of: modules/skparagraph/src/Run.h#L31 (chrome/m156)
/// `EMPTY_RUN`.
pub const EMPTY_RUN: usize = EMPTY_INDEX;
// Port of: modules/skparagraph/src/Run.h#L33 (chrome/m156)
/// `ClusterIndex`.
pub type ClusterIndex = usize;
// Port of: modules/skparagraph/src/Run.h#L34 (chrome/m156)
/// `ClusterRange`.
pub type ClusterRange = Range<usize>;
// Port of: modules/skparagraph/src/Run.h#L35 (chrome/m156)
/// `EMPTY_CLUSTER`.
pub const EMPTY_CLUSTER: usize = EMPTY_INDEX;
// Port of: modules/skparagraph/src/Run.h#L36 (chrome/m156)
/// `EMPTY_CLUSTERS`.
pub const EMPTY_CLUSTERS: Range<usize> = EMPTY_RANGE;
// Port of: modules/skparagraph/src/Run.h#L38 (chrome/m156)
/// `GraphemeIndex`.
pub type GraphemeIndex = usize;
// Port of: modules/skparagraph/src/Run.h#L39 (chrome/m156)
/// `GraphemeRange`.
pub type GraphemeRange = Range<usize>;
// Port of: modules/skparagraph/src/Run.h#L41 (chrome/m156)
/// `GlyphIndex`.
pub type GlyphIndex = usize;
// Port of: modules/skparagraph/src/Run.h#L42 (chrome/m156)
/// `GlyphRange`.
pub type GlyphRange = Range<usize>;

/// `codeUnitHasProperty(index, property)`: all the bits of `property` are set at `index`.
// Port of: modules/skparagraph/src/ParagraphImpl.h#L172-L174 (chrome/m156)
#[must_use]
pub fn code_unit_has_property(
    code_unit_properties: &[CodeUnitFlags],
    index: usize,
    property: CodeUnitFlags,
) -> bool {
    (code_unit_properties[index] & property) == property
}

// Port of: modules/skparagraph/src/Run.h#L53-L529 (chrome/m156)
/// `Run`: a sequence of glyphs shaped with one font, in one direction.
#[derive(Debug, Clone)]
pub struct Run {
    pub(crate) text_range: TextRange,
    pub(crate) cluster_range: ClusterRange,
    pub(crate) font: Font,
    pub(crate) placeholder_index: usize,
    pub(crate) index: usize,
    pub(crate) advance: Vector,
    pub(crate) offset: Vector,
    pub(crate) cluster_start: TextIndex,
    /// `fUtf8Range`: the begin and the end (`begin() + size()`) of the shaped UTF-8 range.
    pub(crate) utf8_range: Range<usize>,
    // `GlyphData`. C++ shares it between the copies of a run (`std::shared_ptr`); a copy owns
    // its own here. Nothing writes a run's glyph data after the paragraph is shaped, so no copy
    // can tell the difference.
    pub(crate) glyphs: Vec<GlyphId>,
    pub(crate) positions: Vec<Point>,
    pub(crate) offsets: Vec<Point>,
    pub(crate) cluster_indexes: Vec<u32>,
    /// `fJustificationShifts`: for justification (`x` is the shift of the glyph and `y` the
    /// shift of the previous glyph).
    pub(crate) justification_shifts: Vec<Point>,
    pub(crate) font_metrics: FontMetrics,
    pub(crate) height_multiplier: scalar,
    pub(crate) use_half_leading: bool,
    pub(crate) baseline_shift: scalar,
    pub(crate) correct_ascent: scalar,
    pub(crate) correct_descent: scalar,
    pub(crate) correct_leading: scalar,
    pub(crate) ellipsis: bool,
    pub(crate) hyphen: bool,
    pub(crate) bidi_level: u8,
    pub(crate) script: FourByteTag,
    pub(crate) language: String,
}

impl Run {
    // Port of: modules/skparagraph/src/Run.cpp#L21-L63 (chrome/m156)
    /// `Run(owner, info, firstChar, heightMultiplier, useHalfLeading, baselineShift, index,
    /// offsetX)`, without the owner.
    #[must_use]
    pub fn new(
        info: &RunInfo<'_>,
        first_char: usize,
        height_multiplier: scalar,
        use_half_leading: bool,
        baseline_shift: scalar,
        index: usize,
        offset_x: scalar,
    ) -> Self {
        let glyph_count = info.glyph_count;
        let offset = Vector::new(offset_x, 0.0);
        let (_, font_metrics) = info.font.metrics();
        let mut run = Run {
            text_range: (first_char + info.utf8_range.start)..(first_char + info.utf8_range.end),
            cluster_range: EMPTY_CLUSTERS,
            font: info.font.clone(),
            placeholder_index: usize::MAX,
            index,
            advance: info.advance,
            offset,
            cluster_start: first_char,
            utf8_range: info.utf8_range.clone(),
            glyphs: vec![0; glyph_count],
            positions: vec![Point::new(0.0, 0.0); glyph_count + 1],
            offsets: vec![Point::new(0.0, 0.0); glyph_count + 1],
            cluster_indexes: vec![0; glyph_count + 1],
            justification_shifts: Vec::new(),
            font_metrics,
            height_multiplier,
            use_half_leading,
            baseline_shift,
            correct_ascent: 0.0,
            correct_descent: 0.0,
            correct_leading: 0.0,
            ellipsis: false,
            hyphen: false,
            bidi_level: info.bidi_level,
            script: info.script,
            language: info.language.unwrap_or("").to_owned(),
        };
        run.calculate_metrics();

        // To make edge cases easier:
        run.positions[glyph_count] = run.offset + run.advance;
        run.offsets[glyph_count] = Point::new(0.0, 0.0);
        #[allow(clippy::cast_possible_truncation)] // mirrors the implicit size_t -> uint32_t
        {
            run.cluster_indexes[glyph_count] = if run.left_to_right() {
                info.utf8_range.end as u32
            } else {
                info.utf8_range.start as u32
            };
        }
        run
    }

    // Port of: modules/skparagraph/src/Run.cpp#L65-L86 (chrome/m156)
    /// `calculateMetrics()`.
    #[doc(alias = "calculateMetrics")]
    pub fn calculate_metrics(&mut self) {
        // `fFontMetrics.fLeading * 0.5` is a float times a double.
        self.correct_ascent = (f64::from(self.font_metrics.ascent)
            - f64::from(self.font_metrics.leading) * 0.5) as scalar;
        self.correct_descent = (f64::from(self.font_metrics.descent)
            + f64::from(self.font_metrics.leading) * 0.5) as scalar;
        self.correct_leading = 0.0;
        if self.height_multiplier.nearly_zero(None) {
            return;
        }
        let run_height = self.height_multiplier * self.font.size();
        let font_intrinsic_height = self.correct_descent - self.correct_ascent;
        if self.use_half_leading {
            let extra_leading = (run_height - font_intrinsic_height) / 2.0;
            self.correct_ascent -= extra_leading;
            self.correct_descent += extra_leading;
        } else {
            let multiplier = run_height / font_intrinsic_height;
            self.correct_ascent *= multiplier;
            self.correct_descent *= multiplier;
        }
        // If we shift the baseline we need to make sure the shifted text fits the line
        self.correct_ascent += self.baseline_shift;
        self.correct_descent += self.baseline_shift;
    }

    /// `posX(index)`.
    #[must_use]
    #[doc(alias = "posX")]
    pub fn pos_x(&self, index: usize) -> scalar {
        self.positions[index].x
    }

    /// `addX(index, shift)`.
    #[doc(alias = "addX")]
    pub fn add_x(&mut self, index: usize, shift: scalar) {
        self.positions[index].x += shift;
    }

    /// `posY(index)`.
    #[must_use]
    #[doc(alias = "posY")]
    pub fn pos_y(&self, index: usize) -> scalar {
        self.positions[index].y
    }

    /// `size()`: the number of glyphs.
    #[must_use]
    pub fn size(&self) -> usize {
        self.glyphs.len()
    }

    /// `setWidth(width)`.
    #[doc(alias = "setWidth")]
    pub fn set_width(&mut self, width: scalar) {
        self.advance.x = width;
    }

    /// `setHeight(height)`.
    #[doc(alias = "setHeight")]
    pub fn set_height(&mut self, height: scalar) {
        self.advance.y = height;
    }

    /// `shift(shiftX, shiftY)`: moves the run.
    #[doc(alias = "shift")]
    pub fn shift_run(&mut self, shift_x: scalar, shift_y: scalar) {
        self.offset.x += shift_x;
        self.offset.y += shift_y;
    }

    /// `advance()`.
    #[must_use]
    pub fn advance(&self) -> Vector {
        Vector::new(
            self.advance.x,
            self.font_metrics.descent - self.font_metrics.ascent + self.font_metrics.leading,
        )
    }

    /// `offset()`.
    #[must_use]
    pub fn offset(&self) -> Vector {
        self.offset
    }

    /// `ascent()`.
    #[must_use]
    pub fn ascent(&self) -> scalar {
        self.font_metrics.ascent + self.baseline_shift
    }

    /// `descent()`.
    #[must_use]
    pub fn descent(&self) -> scalar {
        self.font_metrics.descent + self.baseline_shift
    }

    /// `leading()`.
    #[must_use]
    pub fn leading(&self) -> scalar {
        self.font_metrics.leading
    }

    /// `correctAscent()`.
    #[must_use]
    #[doc(alias = "correctAscent")]
    pub fn correct_ascent(&self) -> scalar {
        self.correct_ascent + self.baseline_shift
    }

    /// `correctDescent()`.
    #[must_use]
    #[doc(alias = "correctDescent")]
    pub fn correct_descent(&self) -> scalar {
        self.correct_descent + self.baseline_shift
    }

    /// `correctLeading()`.
    #[must_use]
    #[doc(alias = "correctLeading")]
    pub fn correct_leading(&self) -> scalar {
        self.correct_leading
    }

    /// `font()`.
    #[must_use]
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// `leftToRight()`.
    #[must_use]
    #[doc(alias = "leftToRight")]
    pub fn left_to_right(&self) -> bool {
        self.bidi_level.is_multiple_of(2)
    }

    /// `getTextDirection()`.
    #[must_use]
    #[doc(alias = "getTextDirection")]
    pub fn text_direction(&self) -> TextDirection {
        if self.left_to_right() {
            TextDirection::LTR
        } else {
            TextDirection::RTL
        }
    }

    /// `index()`.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// `heightMultiplier()`.
    #[must_use]
    #[doc(alias = "heightMultiplier")]
    pub fn height_multiplier(&self) -> scalar {
        self.height_multiplier
    }

    /// `useHalfLeading()`.
    #[must_use]
    #[doc(alias = "useHalfLeading")]
    pub fn use_half_leading(&self) -> bool {
        self.use_half_leading
    }

    /// `baselineShift()`.
    #[must_use]
    #[doc(alias = "baselineShift")]
    pub fn baseline_shift(&self) -> scalar {
        self.baseline_shift
    }

    // Port of: modules/skparagraph/src/Run.cpp#L319-L325 (chrome/m156)
    /// `placeholderStyle()`: the style of the placeholder of this run, in the paragraph's
    /// `placeholders`.
    #[must_use]
    #[doc(alias = "placeholderStyle")]
    pub fn placeholder_style<'a>(
        &self,
        placeholders: &'a [Placeholder],
    ) -> Option<&'a PlaceholderStyle> {
        if self.is_placeholder() {
            Some(&placeholders[self.placeholder_index].style)
        } else {
            None
        }
    }

    /// `isPlaceholder()`.
    #[must_use]
    #[doc(alias = "isPlaceholder")]
    pub fn is_placeholder(&self) -> bool {
        self.placeholder_index != usize::MAX
    }

    /// `clusterIndex(pos)`.
    #[must_use]
    #[doc(alias = "clusterIndex")]
    pub fn cluster_index(&self, pos: usize) -> usize {
        self.cluster_indexes[pos] as usize
    }

    /// `globalClusterIndex(pos)`.
    #[must_use]
    #[doc(alias = "globalClusterIndex")]
    pub fn global_cluster_index(&self, pos: usize) -> usize {
        self.cluster_start + self.cluster_indexes[pos] as usize
    }

    // Port of: modules/skparagraph/src/Run.cpp#L316-L318 (chrome/m156)
    /// `positionX(pos)`.
    #[must_use]
    #[doc(alias = "positionX")]
    pub fn position_x(&self, pos: usize) -> scalar {
        self.pos_x(pos)
            + if self.justification_shifts.is_empty() {
                0.0
            } else {
                self.justification_shifts[pos].y
            }
    }

    /// `textRange()`.
    #[must_use]
    #[doc(alias = "textRange")]
    pub fn text_range(&self) -> TextRange {
        self.text_range.clone()
    }

    /// `clusterRange()`.
    #[must_use]
    #[doc(alias = "clusterRange")]
    pub fn cluster_range(&self) -> ClusterRange {
        self.cluster_range.clone()
    }

    /// `isEllipsis()`.
    #[must_use]
    #[doc(alias = "isEllipsis")]
    pub fn is_ellipsis(&self) -> bool {
        self.ellipsis
    }

    /// `isHyphen()`.
    #[must_use]
    #[doc(alias = "isHyphen")]
    pub fn is_hyphen(&self) -> bool {
        self.hyphen
    }

    /// `setClusterRange(from, to)`.
    #[doc(alias = "setClusterRange")]
    pub fn set_cluster_range(&mut self, from: usize, to: usize) {
        self.cluster_range = from..to;
    }

    /// `clip()`.
    #[must_use]
    pub fn clip(&self) -> Rect {
        Rect::from_xywh(self.offset.x, self.offset.y, self.advance.x, self.advance.y)
    }

    // Port of: modules/skparagraph/src/Run.cpp#L184-L197 (chrome/m156)
    /// `isCursiveScript()`.
    #[must_use]
    #[doc(alias = "isCursiveScript")]
    pub fn is_cursive_script(&self) -> bool {
        const ARAB: FourByteTag = set_four_byte_tag(b'A', b'r', b'a', b'b');
        const ROHG: FourByteTag = set_four_byte_tag(b'R', b'o', b'h', b'g');
        const MAND: FourByteTag = set_four_byte_tag(b'M', b'a', b'n', b'd');
        const MONG: FourByteTag = set_four_byte_tag(b'M', b'o', b'n', b'g');
        const NKOO: FourByteTag = set_four_byte_tag(b'N', b'k', b'o', b'o');
        const PHAG: FourByteTag = set_four_byte_tag(b'P', b'h', b'a', b'g');
        const SYRC: FourByteTag = set_four_byte_tag(b'S', b'y', b'r', b'c');
        matches!(self.script, ARAB | ROHG | MAND | MONG | NKOO | PHAG | SYRC)
    }

    // Port of: modules/skparagraph/src/Run.cpp#L150-L155 (chrome/m156)
    /// `addSpacesAtTheEnd(space, cluster)`.
    #[doc(alias = "addSpacesAtTheEnd")]
    pub fn add_spaces_at_the_end(&mut self, space: scalar, cluster: &mut Cluster) {
        // Increment the run width
        self.advance.x += space;
        // Increment the cluster width
        cluster.space(space);
    }

    // Port of: modules/skparagraph/src/Run.cpp#L156-L165 (chrome/m156)
    /// `addLetterSpacesEvenly(space)`.
    #[doc(alias = "addLetterSpacesEvenly")]
    pub fn add_letter_spaces_evenly(&mut self, space: scalar) -> scalar {
        let mut shift: scalar = 0.0;
        for i in 0..self.size() {
            self.positions[i].x += shift;
            shift += space;
        }
        let size = self.size();
        self.positions[size].x += shift;
        self.advance.x += shift;
        shift
    }

    // Port of: modules/skparagraph/src/Run.cpp#L166-L183 (chrome/m156)
    /// `addLetterSpacesEvenly(space, cluster)`.
    #[doc(alias = "addLetterSpacesEvenly")]
    pub fn add_letter_spaces_evenly_in_cluster(
        &mut self,
        space: scalar,
        cluster: &mut Cluster,
    ) -> scalar {
        // Offset all the glyphs in the cluster
        let mut shift: scalar = 0.0;
        for i in cluster.start_pos()..cluster.end_pos() {
            self.positions[i].x += shift;
            shift += space;
        }
        if self.size() == cluster.end_pos() {
            // To make calculations easier
            self.positions[cluster.end_pos()].x += shift;
        }
        // Increment the run width
        self.advance.x += shift;
        // Increment the cluster width
        cluster.space(shift);
        cluster.set_half_letter_spacing(space / 2.0);
        shift
    }

    // Port of: modules/skparagraph/src/Run.cpp#L198-L209 (chrome/m156)
    /// `shift(cluster, offset)`.
    #[doc(alias = "shift")]
    pub fn shift_cluster(&mut self, cluster: &Cluster, offset: scalar) {
        #[allow(clippy::float_cmp)] // mirrors `offset == 0`
        if offset == 0.0 {
            return;
        }
        for i in cluster.start_pos()..cluster.end_pos() {
            self.positions[i].x += offset;
        }
        if self.size() == cluster.end_pos() {
            // To make calculations easier
            self.positions[cluster.end_pos()].x += offset;
        }
    }

    // Port of: modules/skparagraph/src/Run.cpp#L210-L213 (chrome/m156)
    /// `extend(cluster, offset)`.
    pub fn extend(&mut self, cluster: &Cluster, offset: scalar) {
        // Extend the cluster at the end
        self.positions[cluster.end_pos()].x += offset;
    }

    // Port of: modules/skparagraph/src/Run.cpp#L214-L257 (chrome/m156)
    /// `updateMetrics(endlineMetrics)`, for a placeholder run with the style `placeholder_style`.
    #[doc(alias = "updateMetrics")]
    pub fn update_metrics(
        &mut self,
        placeholder_style: &PlaceholderStyle,
        endline_metrics: &mut InternalLineMetrics,
    ) {
        debug_assert!(self.is_placeholder());
        // Difference between the placeholder baseline and the line bottom
        let mut baseline_adjustment: scalar = 0.0;
        match placeholder_style.baseline {
            TextBaseline::Alphabetic => {}
            TextBaseline::Ideographic => {
                baseline_adjustment = endline_metrics.delta_baselines() / 2.0;
            }
        }

        let height = placeholder_style.height;
        let offset = placeholder_style.baseline_offset;

        self.font_metrics.leading = 0.0;
        match placeholder_style.alignment {
            PlaceholderAlignment::Baseline => {
                self.font_metrics.ascent = baseline_adjustment - offset;
                self.font_metrics.descent = baseline_adjustment + height - offset;
            }
            PlaceholderAlignment::AboveBaseline => {
                self.font_metrics.ascent = baseline_adjustment - height;
                self.font_metrics.descent = baseline_adjustment;
            }
            PlaceholderAlignment::BelowBaseline => {
                self.font_metrics.ascent = baseline_adjustment;
                self.font_metrics.descent = baseline_adjustment + height;
            }
            PlaceholderAlignment::Top => {
                self.font_metrics.descent = height + self.font_metrics.ascent;
            }
            PlaceholderAlignment::Bottom => {
                self.font_metrics.ascent = self.font_metrics.descent - height;
            }
            PlaceholderAlignment::Middle => {
                // `auto mid = (-fDescent - fAscent)/2.0` is a double.
                let mid = (f64::from(-self.font_metrics.descent)
                    - f64::from(self.font_metrics.ascent))
                    / 2.0;
                self.font_metrics.descent = (f64::from(height) / 2.0 - mid) as scalar;
                self.font_metrics.ascent = (-f64::from(height) / 2.0 - mid) as scalar;
            }
        }
        self.calculate_metrics();

        // Make sure the placeholder can fit the line
        endline_metrics.add_run(self);
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L379-L390 (chrome/m156)
    /// `calculateWidth(start, end, clip)`.
    #[must_use]
    #[doc(alias = "calculateWidth")]
    pub fn calculate_width(&self, start: usize, end: usize, _clip: bool) -> scalar {
        debug_assert!(start <= end);
        // clip |= end == size();  // Clip at the end of the run?
        let mut correction: scalar = 0.0;
        if end > start && !self.justification_shifts.is_empty() {
            // This is not a typo: we are using Point as a pair of SkScalars
            correction = self.justification_shifts[end - 1].x - self.justification_shifts[start].y;
        }
        self.pos_x(end) - self.pos_x(start) + correction
    }

    /// `calculateHeight(ascentStyle, descentStyle)`.
    #[must_use]
    #[doc(alias = "calculateHeight")]
    pub fn calculate_height(
        &self,
        ascent_style: LineMetricStyle,
        descent_style: LineMetricStyle,
    ) -> scalar {
        let ascent = if ascent_style == LineMetricStyle::Typographic {
            self.ascent()
        } else {
            self.correct_ascent()
        };
        let descent = if descent_style == LineMetricStyle::Typographic {
            self.descent()
        } else {
            self.correct_descent()
        };
        descent - ascent
    }

    // Port of: modules/skparagraph/src/Run.cpp#L83-L97 (chrome/m156)
    /// `copyTo(builder, pos, size)`.
    #[doc(alias = "copyTo")]
    pub fn copy_to(&self, builder: &mut TextBlobBuilder, pos: usize, size: usize) {
        debug_assert!(pos + size <= self.size());
        let (glyphs, points) = builder.alloc_run_pos(&self.font, size, None);
        glyphs.copy_from_slice(&self.glyphs[pos..pos + size]);
        for i in 0..size {
            let mut point = self.positions[i + pos];
            if !self.justification_shifts.is_empty() {
                point.x += self.justification_shifts[i + pos].x;
            }
            point += self.offsets[i + pos];
            points[i] = point;
        }
    }

    // Port of: modules/skparagraph/src/Run.cpp#L99-L129 (chrome/m156)
    /// `findLimitingClusters(text)`: a cluster range from a text range (within one run). The
    /// range is normalized (`start < end` regardless of the text direction). The boolean says
    /// whether the cluster range was found. `cluster_index_from_code_unit` is the paragraph's
    /// `fClustersIndexFromCodeUnit`.
    #[must_use]
    #[doc(alias = "findLimitingClusters")]
    pub fn find_limiting_clusters(
        &self,
        cluster_index_from_code_unit: &[usize],
        text: TextRange,
    ) -> (bool, ClusterIndex, ClusterIndex) {
        let cluster_index = |text_index: usize| {
            let cluster_index = cluster_index_from_code_unit[text_index];
            debug_assert!(cluster_index != EMPTY_INDEX);
            cluster_index
        };
        if text.width() == 0 {
            // Special Flutter case for "\n" and "...\n"
            if text.end > self.text_range.start {
                let index = cluster_index(text.end - 1);
                return (true, index, index);
            }
            return (false, 0, 0);
        }

        // `ClusterRange clusterRange;` is {0, 0}.
        let mut cluster_range: ClusterRange = 0..0;
        let found;
        // Deal with the case when either start or end are not align with glyph cluster edge
        // In such case we shift the text range to the right
        // (cutting from the left and adding to the right)
        if self.left_to_right() {
            // LTR: [start:end)
            found = cluster_range.start != self.cluster_range.end;
            cluster_range.start = cluster_index(text.start);
            cluster_range.end = cluster_index(text.end - 1);
        } else {
            // RTL: (start:end]
            cluster_range.start = cluster_index(text.end);
            cluster_range.end = cluster_index(text.start + 1);
            found = cluster_range.end != self.cluster_range.start;
        }
        (found, cluster_range.start, cluster_range.end)
    }

    // Port of: modules/skparagraph/src/Run.cpp#L130-L134 (chrome/m156)
    /// `findLimitingGlyphClusters(text)`.
    #[must_use]
    #[doc(alias = "findLimitingGlyphClusters")]
    pub fn find_limiting_glyph_clusters(
        code_unit_properties: &[CodeUnitFlags],
        text_size: usize,
        text: &TextRange,
    ) -> (bool, TextIndex, TextIndex) {
        let start = find_previous_glyph_cluster_boundary(code_unit_properties, text.start);
        let end = find_next_glyph_cluster_boundary(code_unit_properties, text_size, text.end);
        (true, start, end)
    }

    // Port of: modules/skparagraph/src/Run.cpp#L135-L142 (chrome/m156)
    /// `findLimitingGraphemes(text)`: adjusts the text to grapheme edges so the first grapheme
    /// start is in the text and the last grapheme start is in the text. It actually means that
    /// the first grapheme is entirely in the text and the last grapheme does not have to be.
    #[must_use]
    #[doc(alias = "findLimitingGraphemes")]
    pub fn find_limiting_graphemes(
        code_unit_properties: &[CodeUnitFlags],
        text_size: usize,
        text: &TextRange,
    ) -> (bool, TextIndex, TextIndex) {
        let start = find_previous_grapheme_boundary(code_unit_properties, text.start);
        let end = find_next_grapheme_boundary(code_unit_properties, text_size, text.end);
        (true, start, end)
    }

    // Port of: modules/skparagraph/src/Run.h#L168-L206 (chrome/m156)
    /// `iterateThroughClustersInTextOrder(visitor)`: calls `visitor(glyphStart, glyphEnd,
    /// charStart, charEnd, width, height)` for the clusters of the run, in the order of the text.
    #[doc(alias = "iterateThroughClustersInTextOrder")]
    pub fn iterate_through_clusters_in_text_order(
        &self,
        mut visitor: impl FnMut(usize, usize, usize, usize, scalar, scalar),
    ) {
        if self.left_to_right() {
            let mut start: usize = 0;
            let mut cluster = self.cluster_index(start);
            for glyph in 1..=self.size() {
                let next_cluster = self.cluster_index(glyph);
                if next_cluster <= cluster {
                    continue;
                }

                visitor(
                    start,
                    glyph,
                    self.cluster_start + cluster,
                    self.cluster_start + next_cluster,
                    self.calculate_width(start, glyph, glyph == self.size()),
                    self.calculate_height(LineMetricStyle::CSS, LineMetricStyle::CSS),
                );

                start = glyph;
                cluster = next_cluster;
            }
        } else {
            let mut glyph = self.size();
            let mut cluster = self.utf8_range.start;
            let mut start = self.size() as i32 - 1;
            while start >= 0 {
                let next_cluster = if start == 0 {
                    self.utf8_range.end
                } else {
                    self.cluster_index(start as usize - 1)
                };
                if next_cluster <= cluster {
                    start -= 1;
                    continue;
                }

                visitor(
                    start as usize,
                    glyph,
                    self.cluster_start + cluster,
                    self.cluster_start + next_cluster,
                    self.calculate_width(start as usize, glyph, glyph == 0),
                    self.calculate_height(LineMetricStyle::CSS, LineMetricStyle::CSS),
                );

                glyph = start as usize;
                cluster = next_cluster;
                start -= 1;
            }
        }
    }

    /// `glyphs()`.
    #[must_use]
    pub fn glyphs(&self) -> &[GlyphId] {
        &self.glyphs
    }

    /// `positions()`.
    #[must_use]
    pub fn positions(&self) -> &[Point] {
        &self.positions
    }

    /// `offsets()`.
    #[must_use]
    pub fn offsets(&self) -> &[Point] {
        &self.offsets
    }

    /// `clusterIndexes()`.
    #[must_use]
    #[doc(alias = "clusterIndexes")]
    pub fn cluster_indexes(&self) -> &[u32] {
        &self.cluster_indexes
    }

    /// `commit()`.
    pub fn commit(&self) {}

    /// `resetJustificationShifts()`.
    #[doc(alias = "resetJustificationShifts")]
    pub fn reset_justification_shifts(&mut self) {
        self.justification_shifts.clear();
    }

    // Port of: modules/skparagraph/src/Run.cpp#L326-L333 (chrome/m156)
    /// `isResolved()`.
    #[must_use]
    #[doc(alias = "isResolved")]
    pub fn is_resolved(&self) -> bool {
        for &glyph in &self.glyphs {
            if glyph == 0 {
                return false;
            }
        }
        true
    }
}

// Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1103-L1109 (chrome/m156)
/// `findPreviousGraphemeBoundary(utf8)`.
#[must_use]
pub fn find_previous_grapheme_boundary(
    code_unit_properties: &[CodeUnitFlags],
    mut utf8: TextIndex,
) -> TextIndex {
    while utf8 > 0 && (code_unit_properties[utf8] & CodeUnitFlags::GRAPHEME_START).is_empty() {
        utf8 -= 1;
    }
    utf8
}

// Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1111-L1117 (chrome/m156)
/// `findNextGraphemeBoundary(utf8)`.
#[must_use]
pub fn find_next_grapheme_boundary(
    code_unit_properties: &[CodeUnitFlags],
    text_size: usize,
    mut utf8: TextIndex,
) -> TextIndex {
    while utf8 < text_size
        && (code_unit_properties[utf8] & CodeUnitFlags::GRAPHEME_START).is_empty()
    {
        utf8 += 1;
    }
    utf8
}

// Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1119-L1125 (chrome/m156)
/// `findNextGlyphClusterBoundary(utf8)`.
#[must_use]
pub fn find_next_glyph_cluster_boundary(
    code_unit_properties: &[CodeUnitFlags],
    text_size: usize,
    mut utf8: TextIndex,
) -> TextIndex {
    while utf8 < text_size
        && (code_unit_properties[utf8] & CodeUnitFlags::GLYPH_CLUSTER_START).is_empty()
    {
        utf8 += 1;
    }
    utf8
}

// Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1127-L1133 (chrome/m156)
/// `findPreviousGlyphClusterBoundary(utf8)`.
#[must_use]
pub fn find_previous_glyph_cluster_boundary(
    code_unit_properties: &[CodeUnitFlags],
    mut utf8: TextIndex,
) -> TextIndex {
    while utf8 > 0 && (code_unit_properties[utf8] & CodeUnitFlags::GLYPH_CLUSTER_START).is_empty() {
        utf8 -= 1;
    }
    utf8
}

// Port of: modules/skparagraph/src/Run.h#L463-L477 (chrome/m156)
/// `BreakType` of `Cluster`.
#[doc(alias = "Cluster::BreakType")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BreakType {
    /// `None`.
    None,
    /// `GraphemeBreak`: calculated for all clusters (UBRK_CHARACTER).
    GraphemeBreak,
    /// `SoftLineBreak`: calculated for all clusters (UBRK_LINE & UBRK_CHARACTER).
    SoftLineBreak,
    /// `HardLineBreak`: calculated for all clusters (UBRK_LINE).
    HardLineBreak,
}

// Port of: modules/skparagraph/src/ParagraphImpl.cpp#L309-L335 (chrome/m156)
fn is_ascii_7bit_space(c: i32) -> bool {
    debug_assert!((0..=127).contains(&c));

    // Extracted from https://en.wikipedia.org/wiki/Whitespace_character
    //
    const K_HT: i32 = 9;
    const K_LF: i32 = 10;
    const K_VT: i32 = 11;
    const K_FF: i32 = 12;
    const K_CR: i32 = 13;
    const K_SP: i32 = 32; // too big to use as shift
    let m = |shift: i32| 1u32 << shift;
    let k_space_mask: u32 = m(K_HT) | m(K_LF) | m(K_VT) | m(K_FF) | m(K_CR);
    // we check for Space (32) explicitly, since it is too large to shift
    c == K_SP || (c <= 31 && (k_space_mask & m(c)) != 0)
}

// Port of: modules/skparagraph/src/Run.h#L479-L529 (chrome/m156)
/// `Cluster`: the glyphs of a run that shape the same text.
#[derive(Debug, Clone)]
pub struct Cluster {
    pub(crate) run_index: RunIndex,
    pub(crate) text_range: TextRange,
    pub grapheme_range: GraphemeRange,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) width: scalar,
    pub(crate) height: scalar,
    pub(crate) half_letter_spacing: scalar,
    pub(crate) is_white_space_break: bool,
    pub(crate) is_intra_word_break: bool,
    pub(crate) is_hard_break: bool,
    pub(crate) is_ideographic: bool,
}

impl Default for Cluster {
    // Port of: modules/skparagraph/src/Run.h#L204-L214 (chrome/m156). The break flags are not
    // initialized in C++; they are false here.
    fn default() -> Self {
        Self {
            run_index: EMPTY_RUN,
            text_range: EMPTY_TEXT,
            grapheme_range: EMPTY_RANGE,
            start: 0,
            end: 0,
            width: 0.0,
            height: 0.0,
            half_letter_spacing: 0.0,
            is_white_space_break: false,
            is_intra_word_break: false,
            is_hard_break: false,
            is_ideographic: false,
        }
    }
}

impl Cluster {
    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L337-L381 (chrome/m156)
    /// `Cluster(owner, runIndex, start, end, text, width, height)`: `text` is the range of the
    /// paragraph's text (`text`) that the cluster covers, and `code_unit_properties` the
    /// paragraph's `fCodeUnitProperties`.
    #[must_use]
    pub fn new(
        code_unit_properties: &[CodeUnitFlags],
        text: &[u8],
        run_index: RunIndex,
        start: usize,
        end: usize,
        text_range: TextRange,
        width: scalar,
        height: scalar,
    ) -> Self {
        let mut cluster = Cluster {
            run_index,
            text_range,
            grapheme_range: EMPTY_RANGE,
            start,
            end,
            width,
            height,
            half_letter_spacing: 0.0,
            is_white_space_break: false,
            is_intra_word_break: false,
            is_hard_break: false,
            is_ideographic: false,
        };
        let mut white_spaces_break_len: usize = 0;
        let mut intra_word_break_len: usize = 0;

        let ch = &text[cluster.text_range.start..cluster.text_range.end];
        if ch.len() == 1 && ch[0] <= 0x7F {
            // I am not even sure it's worth it if we do not save a unicode call
            if is_ascii_7bit_space(i32::from(ch[0])) {
                white_spaces_break_len += 1;
            }
        } else {
            for i in cluster.text_range.start..cluster.text_range.end {
                if code_unit_has_property(
                    code_unit_properties,
                    i,
                    CodeUnitFlags::PART_OF_WHITE_SPACE_BREAK,
                ) {
                    white_spaces_break_len += 1;
                }
                if code_unit_has_property(
                    code_unit_properties,
                    i,
                    CodeUnitFlags::PART_OF_INTRA_WORD_BREAK,
                ) {
                    intra_word_break_len += 1;
                }
                if code_unit_has_property(code_unit_properties, i, CodeUnitFlags::IDEOGRAPHIC) {
                    cluster.is_ideographic = true;
                }
            }
        }

        cluster.is_white_space_break = white_spaces_break_len == cluster.text_range.width();
        cluster.is_intra_word_break = intra_word_break_len == cluster.text_range.width();
        cluster.is_hard_break = code_unit_has_property(
            code_unit_properties,
            cluster.text_range.end,
            CodeUnitFlags::HARD_LINE_BREAK_BEFORE,
        );
        cluster
    }

    /// `Cluster(TextRange textRange)`.
    #[must_use]
    pub fn from_text_range(text_range: TextRange) -> Self {
        Self {
            text_range,
            grapheme_range: EMPTY_RANGE,
            ..Self::default()
        }
    }

    // Port of: modules/skparagraph/src/Run.cpp#L258-L265 (chrome/m156)
    /// `sizeToChar(ch)`.
    #[must_use]
    #[doc(alias = "sizeToChar")]
    pub fn size_to_char(&self, ch: TextIndex) -> scalar {
        if ch < self.text_range.start || ch >= self.text_range.end {
            return 0.0;
        }
        let shift = ch - self.text_range.start;
        let ratio = shift as f64 * 1.0 / self.text_range.width() as f64;
        double_to_scalar(f64::from(self.width) * ratio)
    }

    // Port of: modules/skparagraph/src/Run.cpp#L266-L273 (chrome/m156)
    /// `sizeFromChar(ch)`.
    #[must_use]
    #[doc(alias = "sizeFromChar")]
    pub fn size_from_char(&self, ch: TextIndex) -> scalar {
        if ch < self.text_range.start || ch >= self.text_range.end {
            return 0.0;
        }
        let shift = self.text_range.end - ch - 1;
        let ratio = shift as f64 * 1.0 / self.text_range.width() as f64;
        double_to_scalar(f64::from(self.width) * ratio)
    }

    // Port of: modules/skparagraph/src/Run.cpp#L274-L277 (chrome/m156)
    /// `roundPos(s)`.
    #[must_use]
    #[doc(alias = "roundPos")]
    pub fn round_pos(&self, s: scalar) -> usize {
        let ratio = f64::from(s) * 1.0 / f64::from(self.width);
        // `sk_double_floor2int`
        skia_rust_core::floating_point::double_floor2int(ratio * self.size() as f64) as usize
    }

    /// `space(shift)`.
    pub fn space(&mut self, shift: scalar) {
        self.width += shift;
    }

    /// `isWhitespaceBreak()`.
    #[must_use]
    #[doc(alias = "isWhitespaceBreak")]
    pub fn is_whitespace_break(&self) -> bool {
        self.is_white_space_break
    }

    /// `isIntraWordBreak()`.
    #[must_use]
    #[doc(alias = "isIntraWordBreak")]
    pub fn is_intra_word_break(&self) -> bool {
        self.is_intra_word_break
    }

    /// `isHardBreak()`.
    #[must_use]
    #[doc(alias = "isHardBreak")]
    pub fn is_hard_break(&self) -> bool {
        self.is_hard_break
    }

    /// `isIdeographic()`.
    #[must_use]
    #[doc(alias = "isIdeographic")]
    pub fn is_ideographic(&self) -> bool {
        self.is_ideographic
    }

    // Port of: modules/skparagraph/src/Run.cpp#L334-L337 (chrome/m156)
    /// `isSoftBreak()`.
    #[must_use]
    #[doc(alias = "isSoftBreak")]
    pub fn is_soft_break(&self, code_unit_properties: &[CodeUnitFlags]) -> bool {
        code_unit_has_property(
            code_unit_properties,
            self.text_range.end,
            CodeUnitFlags::SOFT_LINE_BREAK_BEFORE,
        )
    }

    // Port of: modules/skparagraph/src/Run.cpp#L338-L347 (chrome/m156)
    /// `isSoftHyphen()`.
    #[must_use]
    #[doc(alias = "isSoftHyphen")]
    pub fn is_soft_hyphen(&self, text: &[u8]) -> bool {
        // `p` runs from the start of the cluster's text to `end`.
        let mut p = &text[self.text_range.start..self.text_range.end];
        if p.is_empty() {
            return false;
        }
        let cp = skia_rust_core::utf::next_utf8(&mut p);
        cp == 0x00AD && p.is_empty()
    }

    // Port of: modules/skparagraph/src/Run.cpp#L348-L350 (chrome/m156)
    /// `isGraphemeBreak()`.
    #[must_use]
    #[doc(alias = "isGraphemeBreak")]
    pub fn is_grapheme_break(&self, code_unit_properties: &[CodeUnitFlags]) -> bool {
        code_unit_has_property(
            code_unit_properties,
            self.text_range.end,
            CodeUnitFlags::GRAPHEME_START,
        )
    }

    /// `canBreakLineAfter()`.
    #[must_use]
    #[doc(alias = "canBreakLineAfter")]
    pub fn can_break_line_after(&self, code_unit_properties: &[CodeUnitFlags]) -> bool {
        self.is_hard_break() || self.is_soft_break(code_unit_properties)
    }

    /// `startPos()`.
    #[must_use]
    #[doc(alias = "startPos")]
    pub fn start_pos(&self) -> usize {
        self.start
    }

    /// `endPos()`.
    #[must_use]
    #[doc(alias = "endPos")]
    pub fn end_pos(&self) -> usize {
        self.end
    }

    /// `width()`.
    #[must_use]
    pub fn width(&self) -> scalar {
        self.width
    }

    /// `height()`.
    #[must_use]
    pub fn height(&self) -> scalar {
        self.height
    }

    /// `size()`.
    #[must_use]
    pub fn size(&self) -> usize {
        self.end - self.start
    }

    /// `setHalfLetterSpacing(halfLetterSpacing)`.
    #[doc(alias = "setHalfLetterSpacing")]
    pub fn set_half_letter_spacing(&mut self, half_letter_spacing: scalar) {
        self.half_letter_spacing = half_letter_spacing;
    }

    /// `getHalfLetterSpacing()`.
    #[must_use]
    #[doc(alias = "getHalfLetterSpacing")]
    pub fn half_letter_spacing(&self) -> scalar {
        self.half_letter_spacing
    }

    /// `textRange()`.
    #[must_use]
    #[doc(alias = "textRange")]
    pub fn text_range(&self) -> TextRange {
        self.text_range.clone()
    }

    /// `runIndex()`.
    #[must_use]
    #[doc(alias = "runIndex")]
    pub fn run_index(&self) -> RunIndex {
        self.run_index
    }

    // Port of: modules/skparagraph/src/Run.cpp#L320-L325 (chrome/m156)
    /// `runOrNull()`.
    #[must_use]
    #[doc(alias = "runOrNull")]
    pub fn run_or_null<'a>(&self, runs: &'a [Run]) -> Option<&'a Run> {
        runs.get(self.run_index)
    }

    // Port of: modules/skparagraph/src/Run.cpp#L326-L329 (chrome/m156)
    /// `run()`.
    #[must_use]
    pub fn run<'a>(&self, runs: &'a [Run]) -> &'a Run {
        debug_assert!(self.run_index < runs.len());
        &runs[self.run_index]
    }

    /// `font()`.
    #[must_use]
    pub fn font(&self, runs: &[Run]) -> Font {
        debug_assert!(self.run_index < runs.len());
        runs[self.run_index].font().clone()
    }

    // Port of: modules/skparagraph/src/Run.cpp#L278-L283 (chrome/m156)
    /// `trimmedWidth(pos)`: the width until `pos`, at most the width of the cluster.
    #[must_use]
    #[doc(alias = "trimmedWidth")]
    pub fn trimmed_width(&self, runs: &[Run], pos: usize) -> scalar {
        // Find the width until the pos and return the min between trimmedWidth and the width(pos)
        // We don't have to take in account cluster shift since it's the same for 0 and for pos
        let run = &runs[self.run_index];
        let a = run.position_x(pos) - run.position_x(self.start);
        // `std::min(a, b)` is `b < a ? b : a`.
        if self.width < a { self.width } else { a }
    }

    /// `contains(ch)`.
    #[must_use]
    pub fn contains(&self, ch: TextIndex) -> bool {
        ch >= self.text_range.start && ch < self.text_range.end
    }

    /// `belongs(text)`.
    #[must_use]
    pub fn belongs(&self, text: &TextRange) -> bool {
        self.text_range.start >= text.start && self.text_range.end <= text.end
    }

    /// `startsIn(text)`.
    #[must_use]
    #[doc(alias = "startsIn")]
    pub fn starts_in(&self, text: &TextRange) -> bool {
        self.text_range.start >= text.start && self.text_range.start < text.end
    }
}

// Port of: modules/skparagraph/src/Run.h#L531-L674 (chrome/m156)
/// `InternalLineMetrics`: the ascent, descent and leading of a line.
#[derive(Debug, Copy, Clone)]
pub struct InternalLineMetrics {
    pub(crate) ascent: scalar,
    pub(crate) descent: scalar,
    pub(crate) leading: scalar,
    pub(crate) raw_ascent: scalar,
    pub(crate) raw_descent: scalar,
    pub(crate) raw_leading: scalar,
    pub(crate) force_strut: bool,
}

impl Default for InternalLineMetrics {
    // Port of: modules/skparagraph/src/Run.h#L541-L544 (chrome/m156)
    fn default() -> Self {
        Self::new(false)
    }
}

impl InternalLineMetrics {
    // Port of: modules/skparagraph/src/Run.h#L546-L549 (chrome/m156)
    /// `InternalLineMetrics(bool forceStrut)`.
    #[must_use]
    pub fn new(force_strut: bool) -> Self {
        let mut metrics = Self {
            ascent: 0.0,
            descent: 0.0,
            leading: 0.0,
            raw_ascent: 0.0,
            raw_descent: 0.0,
            raw_leading: 0.0,
            force_strut,
        };
        metrics.clean();
        metrics
    }

    // Port of: modules/skparagraph/src/Run.h#L551-L559 (chrome/m156)
    /// `InternalLineMetrics(a, d, l)`.
    #[must_use]
    pub fn from_adl(a: scalar, d: scalar, l: scalar) -> Self {
        Self {
            ascent: a,
            descent: d,
            leading: l,
            raw_ascent: a,
            raw_descent: d,
            raw_leading: l,
            force_strut: false,
        }
    }

    // Port of: modules/skparagraph/src/Run.h#L561-L569 (chrome/m156)
    /// `InternalLineMetrics(a, d, l, ra, rd, rl)`.
    #[must_use]
    pub fn from_adl_raw(
        a: scalar,
        d: scalar,
        l: scalar,
        ra: scalar,
        rd: scalar,
        rl: scalar,
    ) -> Self {
        Self {
            ascent: a,
            descent: d,
            leading: l,
            raw_ascent: ra,
            raw_descent: rd,
            raw_leading: rl,
            force_strut: false,
        }
    }

    // Port of: modules/skparagraph/src/Run.h#L571-L581 (chrome/m156)
    /// `InternalLineMetrics(font, forceStrut)`.
    #[must_use]
    pub fn from_font(font: &Font, force_strut: bool) -> Self {
        let (_, metrics) = font.metrics();
        Self {
            ascent: metrics.ascent,
            descent: metrics.descent,
            leading: metrics.leading,
            raw_ascent: metrics.ascent,
            raw_descent: metrics.descent,
            raw_leading: metrics.leading,
            force_strut,
        }
    }

    // Port of: modules/skparagraph/src/Run.h#L583-L593 (chrome/m156)
    /// `add(Run*)`.
    pub fn add_run(&mut self, run: &Run) {
        if self.force_strut {
            return;
        }
        // `std::min(a, b)` is `b < a ? b : a` and `std::max(a, b)` is `a < b ? b : a`.
        self.ascent = std_min(self.ascent, run.correct_ascent());
        self.descent = std_max(self.descent, run.correct_descent());
        self.leading = std_max(self.leading, run.correct_leading());

        self.raw_ascent = std_min(self.raw_ascent, run.ascent());
        self.raw_descent = std_max(self.raw_descent, run.descent());
        self.raw_leading = std_max(self.raw_leading, run.leading());
    }

    // Port of: modules/skparagraph/src/Run.h#L595-L603 (chrome/m156)
    /// `add(InternalLineMetrics other)`.
    pub fn add(&mut self, other: InternalLineMetrics) {
        self.ascent = std_min(self.ascent, other.ascent);
        self.descent = std_max(self.descent, other.descent);
        self.leading = std_max(self.leading, other.leading);
        self.raw_ascent = std_min(self.raw_ascent, other.raw_ascent);
        self.raw_descent = std_max(self.raw_descent, other.raw_descent);
        self.raw_leading = std_max(self.raw_leading, other.raw_leading);
    }

    // Port of: modules/skparagraph/src/Run.h#L605-L612 (chrome/m156)
    /// `clean()`.
    pub fn clean(&mut self) {
        self.ascent = SCALAR_MAX;
        self.descent = SCALAR_MIN;
        self.leading = 0.0;
        self.raw_ascent = SCALAR_MAX;
        self.raw_descent = SCALAR_MIN;
        self.raw_leading = 0.0;
    }

    // Port of: modules/skparagraph/src/Run.h#L614-L621 (chrome/m156)
    /// `isClean()`.
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors the exact comparisons of isClean
    pub fn is_clean(&self) -> bool {
        self.ascent == SCALAR_MAX
            && self.descent == SCALAR_MIN
            && self.leading == 0.0
            && self.raw_ascent == SCALAR_MAX
            && self.raw_descent == SCALAR_MIN
            && self.raw_leading == 0.0
    }

    // Port of: modules/skparagraph/src/Run.h#L623 (chrome/m156)
    /// `delta()`.
    #[must_use]
    pub fn delta(&self) -> scalar {
        self.height() - self.ideographic_baseline()
    }

    // Port of: modules/skparagraph/src/Run.h#L625-L639 (chrome/m156)
    /// `updateLineMetrics(metrics)`.
    pub fn update_line_metrics(&self, metrics: &mut InternalLineMetrics) {
        if metrics.force_strut {
            metrics.ascent = self.ascent;
            metrics.descent = self.descent;
            metrics.leading = self.leading;
            metrics.raw_ascent = self.raw_ascent;
            metrics.raw_descent = self.raw_descent;
            metrics.raw_leading = self.raw_leading;
        } else {
            // This is the Flutter way to calculate the leading
            metrics.ascent = std_min(metrics.ascent, self.ascent - self.leading / 2.0);
            metrics.descent = std_max(metrics.descent, self.descent + self.leading / 2.0);
            metrics.raw_ascent =
                std_min(metrics.raw_ascent, self.raw_ascent - self.raw_leading / 2.0);
            metrics.raw_descent = std_max(
                metrics.raw_descent,
                self.raw_descent + self.raw_leading / 2.0,
            );
        }
    }

    // Port of: modules/skparagraph/src/Run.h#L641-L644 (chrome/m156)
    /// `runTop(run, ascentStyle)`.
    #[must_use]
    #[doc(alias = "runTop")]
    pub fn run_top(&self, run: &Run, ascent_style: LineMetricStyle) -> scalar {
        self.leading / 2.0 - self.ascent
            + (if ascent_style == LineMetricStyle::Typographic {
                run.ascent()
            } else {
                run.correct_ascent()
            })
            + self.delta()
    }

    // Port of: modules/skparagraph/src/Run.h#L645-L647 (chrome/m156)
    /// `height()`: `::round((double)fDescent - fAscent + fLeading)`.
    #[must_use]
    pub fn height(&self) -> scalar {
        (f64::from(self.descent) - f64::from(self.ascent) + f64::from(self.leading)).round()
            as scalar
    }

    /// `update(a, d, l)`.
    pub fn update(&mut self, a: scalar, d: scalar, l: scalar) {
        self.ascent = a;
        self.descent = d;
        self.leading = l;
    }

    /// `updateRawData(ra, rd)`.
    #[doc(alias = "updateRawData")]
    pub fn update_raw_data(&mut self, ra: scalar, rd: scalar) {
        self.raw_ascent = ra;
        self.raw_descent = rd;
    }

    /// `alphabeticBaseline()`.
    #[must_use]
    #[doc(alias = "alphabeticBaseline")]
    pub fn alphabetic_baseline(&self) -> scalar {
        self.leading / 2.0 - self.ascent
    }

    /// `ideographicBaseline()`.
    #[must_use]
    #[doc(alias = "ideographicBaseline")]
    pub fn ideographic_baseline(&self) -> scalar {
        self.descent - self.ascent + self.leading
    }

    /// `deltaBaselines()`.
    #[must_use]
    #[doc(alias = "deltaBaselines")]
    pub fn delta_baselines(&self) -> scalar {
        self.leading / 2.0 + self.descent
    }

    /// `baseline()`.
    #[must_use]
    pub fn baseline(&self) -> scalar {
        self.leading / 2.0 - self.ascent
    }

    /// `ascent()`.
    #[must_use]
    pub fn ascent(&self) -> scalar {
        self.ascent
    }

    /// `descent()`.
    #[must_use]
    pub fn descent(&self) -> scalar {
        self.descent
    }

    /// `leading()`.
    #[must_use]
    pub fn leading(&self) -> scalar {
        self.leading
    }

    /// `rawAscent()`.
    #[must_use]
    #[doc(alias = "rawAscent")]
    pub fn raw_ascent(&self) -> scalar {
        self.raw_ascent
    }

    /// `rawDescent()`.
    #[must_use]
    #[doc(alias = "rawDescent")]
    pub fn raw_descent(&self) -> scalar {
        self.raw_descent
    }

    /// `setForceStrut(value)`.
    #[doc(alias = "setForceStrut")]
    pub fn set_force_strut(&mut self, value: bool) {
        self.force_strut = value;
    }

    /// `getForceStrut()`.
    #[must_use]
    #[doc(alias = "getForceStrut")]
    pub fn force_strut(&self) -> bool {
        self.force_strut
    }
}

/// C++ `std::min(a, b)` for floats: `b < a ? b : a`.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

/// C++ `std::max(a, b)` for floats: `a < b ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

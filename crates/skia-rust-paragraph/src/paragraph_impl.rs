// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/Paragraph.h,
// modules/skparagraph/src/ParagraphImpl.h, modules/skparagraph/src/ParagraphImpl.cpp
//
// Skia's `Paragraph` is abstract and `ParagraphImpl` is its only implementation, so one type,
// [`Paragraph`], is both (`skia_safe::textlayout::Paragraph` is the same single type), and
// [`ParagraphImpl`] is an alias of it for the tests that name the implementation.

//! `Paragraph`: a paragraph of text, shaped, broken into lines and painted.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use bitflags::bitflags;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{scalar, scalar_abs, scalar_floor_to_scalar, scalar_round_to_scalar};
use skia_rust_core::size::Size;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::utf::Unichar;
use skia_rust_unicode::unicode::{
    BidiRegion, CodeUnitFlags, TextDirection as UnicodeTextDirection, Unicode,
    convert_utf16_to_utf8, extract_utf_conversion_mapping,
};

use crate::dart_types::{
    Affinity, EMPTY_INDEX, EMPTY_RANGE, PositionWithAffinity, RangeExtensions, RectHeightStyle,
    RectWidthStyle, TextAlign, TextBox, TextDirection,
};
use crate::font_collection::FontCollection;
use crate::metrics::LineMetrics;
use crate::one_line_shaper::OneLineShaper;
use crate::paragraph_painter::{CanvasParagraphPainter, ParagraphPainter};
use crate::paragraph_style::ParagraphStyle;
use crate::run::{
    Cluster, ClusterIndex, ClusterRange, EMPTY_RUN, InternalLineMetrics, Run, RunIndex,
    find_next_glyph_cluster_boundary, find_next_grapheme_boundary,
    find_previous_glyph_cluster_boundary, find_previous_grapheme_boundary,
};
use crate::text_line::{ClipContext, TextAdjustment, TextLine};
use crate::text_style::{
    Block, BlockIndex, BlockRange, EMPTY_BLOCK, EMPTY_TEXT, Placeholder, StyleType, TextIndex,
    TextRange, TextStyle,
};
use crate::text_wrapper::TextWrapper;
use skia_rust_core::paint::Paint;

// Port of: modules/skparagraph/src/ParagraphImpl.cpp#L36-L47 (chrome/m156)
fn little_round(a: scalar) -> scalar {
    // This rounding is done to match Flutter tests. Must be removed..
    let val = scalar_abs(a);
    if val < 10000.0 {
        (f64::from(scalar_round_to_scalar((f64::from(a) * 100.0) as scalar)) / 100.0) as scalar
    } else if val < 100_000.0 {
        (f64::from(scalar_round_to_scalar((f64::from(a) * 10.0) as scalar)) / 10.0) as scalar
    } else {
        scalar_floor_to_scalar(a)
    }
}

// Port of: modules/skparagraph/src/ParagraphImpl.h#L81-L86 (chrome/m156)
/// `ResolvedFontDescriptor`: the font used from a text index on.
#[derive(Debug, Clone)]
pub struct ResolvedFontDescriptor {
    /// `fFont`.
    pub font: Font,
    /// `fTextStart`.
    pub text_start: TextIndex,
}

impl ResolvedFontDescriptor {
    /// `ResolvedFontDescriptor(index, font)`.
    #[must_use]
    pub fn new(index: TextIndex, font: Font) -> Self {
        Self {
            font,
            text_start: index,
        }
    }
}

// Port of: modules/skparagraph/src/ParagraphImpl.h#L88-L95 (chrome/m156)
/// `InternalState`: how far a paragraph is processed.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum InternalState {
    /// `kUnknown`.
    Unknown = 0,
    /// `kIndexed`: text is indexed.
    Indexed = 1,
    /// `kShaped`: text is shaped.
    Shaped = 2,
    /// `kLineBroken`.
    LineBroken = 5,
    /// `kFormatted`.
    Formatted = 6,
    /// `kDrawn`.
    Drawn = 7,
}

bitflags! {
    // Port of: modules/skparagraph/include/Paragraph.h#L77-L79 (chrome/m156)
    /// `Paragraph::VisitorFlags`.
    #[doc(alias = "Paragraph::VisitorFlags")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct VisitorFlags: u32 {
        /// `kWhiteSpace_VisitorFlag`.
        const WHITE_SPACE = 1 << 0;
    }
}

// Port of: modules/skparagraph/include/Paragraph.h#L81-L90 (chrome/m156)
/// `Paragraph::VisitorInfo`: a run of glyphs for `visit`.
#[doc(alias = "Paragraph::VisitorInfo")]
#[derive(Debug, Clone)]
pub struct VisitorInfo<'a> {
    font: &'a Font,
    origin: Point,
    advance_x: scalar,
    count: usize,
    glyphs: &'a [GlyphId],
    positions: &'a [Point],
    utf8_starts: &'a [u32],
    flags: u32,
}

impl<'a> VisitorInfo<'a> {
    /// `font`.
    #[must_use]
    pub fn font(&self) -> &'a Font {
        self.font
    }

    /// `origin`.
    #[must_use]
    pub fn origin(&self) -> Point {
        self.origin
    }

    /// `advanceX`.
    #[must_use]
    pub fn advance_x(&self) -> scalar {
        self.advance_x
    }

    /// `count`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// `glyphs`: `count` values.
    #[must_use]
    pub fn glyphs(&self) -> &'a [GlyphId] {
        self.glyphs
    }

    /// `positions`: `count` values.
    #[must_use]
    pub fn positions(&self) -> &'a [Point] {
        self.positions
    }

    /// `utf8Starts`: `count + 1` values.
    #[must_use]
    pub fn utf8_starts(&self) -> &'a [u32] {
        self.utf8_starts
    }

    /// `flags`.
    #[must_use]
    pub fn flags(&self) -> VisitorFlags {
        VisitorFlags::from_bits_retain(self.flags)
    }
}

// Port of: modules/skparagraph/include/Paragraph.h#L104-L113 (chrome/m156)
/// `Paragraph::ExtendedVisitorInfo`: a run of glyphs with their bounds for `extendedVisit`.
#[doc(alias = "Paragraph::ExtendedVisitorInfo")]
#[derive(Debug, Clone)]
pub struct ExtendedVisitorInfo<'a> {
    font: &'a Font,
    origin: Point,
    advance: Size,
    count: usize,
    glyphs: &'a [GlyphId],
    positions: &'a [Point],
    bounds: &'a [Rect],
    utf8_starts: &'a [u32],
    flags: u32,
}

impl<'a> ExtendedVisitorInfo<'a> {
    /// `font`.
    #[must_use]
    pub fn font(&self) -> &'a Font {
        self.font
    }

    /// `origin`.
    #[must_use]
    pub fn origin(&self) -> Point {
        self.origin
    }

    /// `advance`.
    #[must_use]
    pub fn advance(&self) -> Size {
        self.advance
    }

    /// `count`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// `glyphs`: `count` values.
    #[must_use]
    pub fn glyphs(&self) -> &'a [GlyphId] {
        self.glyphs
    }

    /// `positions`: `count` values.
    #[must_use]
    pub fn positions(&self) -> &'a [Point] {
        self.positions
    }

    /// `bounds`: `count` values.
    #[must_use]
    pub fn bounds(&self) -> &'a [Rect] {
        self.bounds
    }

    /// `utf8Starts`.
    #[must_use]
    pub fn utf8_starts(&self) -> &'a [u32] {
        self.utf8_starts
    }

    /// `flags`.
    #[must_use]
    pub fn flags(&self) -> VisitorFlags {
        VisitorFlags::from_bits_retain(self.flags)
    }
}

// Port of: modules/skparagraph/include/Paragraph.h#L225-L229 (chrome/m156)
/// `Paragraph::GlyphClusterInfo`.
#[doc(alias = "Paragraph::GlyphClusterInfo")]
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphClusterInfo {
    /// `fBounds`.
    pub bounds: Rect,
    /// `fClusterTextRange`.
    pub text_range: TextRange,
    /// `fGlyphClusterPosition`.
    pub position: TextDirection,
}

impl Default for GlyphClusterInfo {
    /// C++ leaves the members of a default-constructed `GlyphClusterInfo` uninitialized; this
    /// is an empty cluster of an LTR run.
    fn default() -> Self {
        Self {
            bounds: Rect::default(),
            text_range: 0..0,
            position: TextDirection::LTR,
        }
    }
}

// Port of: modules/skparagraph/include/Paragraph.h#L253-L258 (chrome/m156)
/// `Paragraph::GlyphInfo`: the glyph and grapheme cluster information associated with a unicode
/// codepoint in the paragraph.
#[doc(alias = "Paragraph::GlyphInfo")]
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphInfo {
    /// `fGraphemeLayoutBounds`.
    pub grapheme_layout_bounds: Rect,
    /// `fGraphemeClusterTextRange`.
    pub grapheme_cluster_text_range: TextRange,
    /// `fDirection`.
    pub text_direction: TextDirection,
    /// `fIsEllipsis`.
    pub is_ellipsis: bool,
}

impl Default for GlyphInfo {
    // Port of: modules/skparagraph/include/Paragraph.h#L242-L246 (chrome/m156)
    fn default() -> Self {
        Self {
            grapheme_layout_bounds: Rect::default(),
            grapheme_cluster_text_range: 0..0,
            text_direction: TextDirection::LTR,
            is_ellipsis: false,
        }
    }
}

// Port of: modules/skparagraph/include/Paragraph.h#L274-L281 (chrome/m156)
/// `Paragraph::FontInfo`: a font and the text range it shapes.
#[doc(alias = "Paragraph::FontInfo")]
#[derive(Debug, Clone)]
pub struct FontInfo {
    /// `fFont`.
    pub font: Font,
    /// `fTextRange`.
    pub text_range: TextRange,
}

impl FontInfo {
    /// `FontInfo(font, textRange)`.
    #[must_use]
    pub fn new(font: Font, text_range: &TextRange) -> Self {
        Self {
            font,
            text_range: text_range.clone(),
        }
    }
}

/// The UTF-8 to UTF-16 index mappings of the text (`fUTF8IndexForUTF16Index` and
/// `fUTF16IndexForUTF8Index`).
#[derive(Debug, Default)]
struct Utf16Mapping {
    utf8_index_for_utf16_index: Vec<TextIndex>,
    utf16_index_for_utf8_index: Vec<usize>,
}

// Port of: modules/skparagraph/src/ParagraphImpl.h#L107-L309 (chrome/m156)
/// `Paragraph` (and `ParagraphImpl`): a paragraph of text.
#[doc(alias = "ParagraphImpl")]
pub struct Paragraph {
    // `Paragraph`
    pub(crate) font_collection: FontCollection,
    pub(crate) paragraph_style: ParagraphStyle,
    // Things for Flutter
    pub(crate) alphabetic_baseline: scalar,
    pub(crate) ideographic_baseline: scalar,
    pub(crate) height: scalar,
    pub(crate) width: scalar,
    pub(crate) max_intrinsic_width: scalar,
    pub(crate) min_intrinsic_width: scalar,
    pub(crate) longest_line: scalar,
    pub(crate) exceeded_max_lines: bool,
    // `ParagraphImpl`
    pub(crate) text_styles: Vec<Block>,
    pub(crate) placeholders: Vec<Placeholder>,
    pub(crate) text: Vec<u8>,
    pub(crate) state: InternalState,
    pub(crate) runs: Vec<Run>,
    pub(crate) clusters: Vec<Cluster>,
    pub(crate) code_unit_properties: Vec<CodeUnitFlags>,
    pub(crate) clusters_index_from_code_unit: Vec<usize>,
    pub(crate) words: Vec<usize>,
    pub(crate) bidi_regions: Vec<BidiRegion>,
    utf16_mapping: OnceLock<Utf16Mapping>,
    pub(crate) unresolved_glyphs: usize,
    pub(crate) unresolved_codepoints: HashSet<Unichar>,
    pub(crate) lines: Vec<TextLine>,
    pub(crate) font_switches: Vec<ResolvedFontDescriptor>,
    pub(crate) empty_metrics: InternalLineMetrics,
    pub(crate) strut_metrics: InternalLineMetrics,
    pub(crate) old_width: scalar,
    pub(crate) old_height: scalar,
    pub(crate) max_width_with_trailing_spaces: scalar,
    pub(crate) unicode: Arc<dyn Unicode>,
    pub(crate) has_line_breaks: bool,
    pub(crate) has_whitespaces_inside: bool,
    pub(crate) trailing_spaces: TextIndex,
}

impl std::fmt::Debug for Paragraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paragraph")
            .field("text", &String::from_utf8_lossy(&self.text))
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// `ParagraphImpl`, the implementation of `Paragraph`: the same type.
pub type ParagraphImpl = Paragraph;

impl Paragraph {
    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L63-L98 (chrome/m156)
    /// `ParagraphImpl(text, style, blocks, placeholders, fonts, unicode)`.
    #[must_use]
    pub fn new(
        text: Vec<u8>,
        style: ParagraphStyle,
        blocks: Vec<Block>,
        placeholders: Vec<Placeholder>,
        fonts: FontCollection,
        unicode: Arc<dyn Unicode>,
    ) -> Self {
        Self {
            font_collection: fonts,
            paragraph_style: style,
            alphabetic_baseline: 0.0,
            ideographic_baseline: 0.0,
            height: 0.0,
            width: 0.0,
            max_intrinsic_width: 0.0,
            min_intrinsic_width: 0.0,
            longest_line: 0.0,
            exceeded_max_lines: false,
            text_styles: blocks,
            placeholders,
            text,
            state: InternalState::Unknown,
            runs: Vec::new(),
            clusters: Vec::new(),
            code_unit_properties: Vec::new(),
            clusters_index_from_code_unit: Vec::new(),
            words: Vec::new(),
            bidi_regions: Vec::new(),
            utf16_mapping: OnceLock::new(),
            unresolved_glyphs: 0,
            unresolved_codepoints: HashSet::new(),
            lines: Vec::new(),
            font_switches: Vec::new(),
            empty_metrics: InternalLineMetrics::default(),
            strut_metrics: InternalLineMetrics::new(false),
            old_width: 0.0,
            old_height: 0.0,
            max_width_with_trailing_spaces: 0.0,
            unicode,
            has_line_breaks: false,
            has_whitespaces_inside: false,
            trailing_spaces: 0,
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L100-L117 (chrome/m156)
    /// `ParagraphImpl(utf16text, style, blocks, placeholders, fonts, unicode)`.
    #[must_use]
    pub fn from_utf16(
        utf16_text: &[u16],
        style: ParagraphStyle,
        blocks: Vec<Block>,
        placeholders: Vec<Placeholder>,
        fonts: FontCollection,
        unicode: Arc<dyn Unicode>,
    ) -> Self {
        Self::new(
            convert_utf16_to_utf8(utf16_text).into_bytes(),
            style,
            blocks,
            placeholders,
            fonts,
            unicode,
        )
    }

    // Accessors of `Paragraph`

    /// `getMaxWidth()`.
    #[must_use]
    #[doc(alias = "getMaxWidth")]
    pub fn max_width(&self) -> scalar {
        self.width
    }

    /// `getHeight()`.
    #[must_use]
    #[doc(alias = "getHeight")]
    pub fn height(&self) -> scalar {
        self.height
    }

    /// `getMinIntrinsicWidth()`.
    #[must_use]
    #[doc(alias = "getMinIntrinsicWidth")]
    pub fn min_intrinsic_width(&self) -> scalar {
        self.min_intrinsic_width
    }

    /// `getMaxIntrinsicWidth()`.
    #[must_use]
    #[doc(alias = "getMaxIntrinsicWidth")]
    pub fn max_intrinsic_width(&self) -> scalar {
        self.max_intrinsic_width
    }

    /// `getAlphabeticBaseline()`.
    #[must_use]
    #[doc(alias = "getAlphabeticBaseline")]
    pub fn alphabetic_baseline(&self) -> scalar {
        self.alphabetic_baseline
    }

    /// `getIdeographicBaseline()`.
    #[must_use]
    #[doc(alias = "getIdeographicBaseline")]
    pub fn ideographic_baseline(&self) -> scalar {
        self.ideographic_baseline
    }

    /// `getLongestLine()`.
    #[must_use]
    #[doc(alias = "getLongestLine")]
    pub fn longest_line(&self) -> scalar {
        self.longest_line
    }

    /// `didExceedMaxLines()`.
    #[must_use]
    #[doc(alias = "didExceedMaxLines")]
    pub fn did_exceed_max_lines(&self) -> bool {
        self.exceeded_max_lines
    }

    // Accessors of `ParagraphImpl`

    /// `lineNumber()`.
    #[must_use]
    #[doc(alias = "lineNumber")]
    pub fn line_number(&self) -> usize {
        self.lines.len()
    }

    /// `getApplyRoundingHack()`.
    #[must_use]
    #[doc(alias = "getApplyRoundingHack")]
    pub fn get_apply_rounding_hack(&self) -> bool {
        self.paragraph_style.apply_rounding_hack()
    }

    /// `text()`.
    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.text
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L875-L879 (chrome/m156)
    /// `text(textRange)`.
    #[must_use]
    pub fn text_of(&self, text_range: &TextRange) -> &[u8] {
        debug_assert!(text_range.start <= self.text.len() && text_range.end <= self.text.len());
        &self.text[text_range.start..text_range.start + text_range.width()]
    }

    /// `state()`.
    #[must_use]
    pub fn state(&self) -> InternalState {
        self.state
    }

    /// `runs()`.
    #[must_use]
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// `styles()`.
    #[must_use]
    pub fn styles(&self) -> &[Block] {
        &self.text_styles
    }

    /// `placeholders()`.
    #[must_use]
    pub fn placeholders(&self) -> &[Placeholder] {
        &self.placeholders
    }

    /// `lines()`.
    #[must_use]
    pub fn lines(&self) -> &[TextLine] {
        &self.lines
    }

    /// `lines()`, mutably.
    pub fn lines_mut(&mut self) -> &mut [TextLine] {
        &mut self.lines
    }

    /// `paragraphStyle()`.
    #[must_use]
    #[doc(alias = "paragraphStyle")]
    pub fn paragraph_style(&self) -> &ParagraphStyle {
        &self.paragraph_style
    }

    /// `clusters()`.
    #[must_use]
    pub fn clusters(&self) -> &[Cluster] {
        &self.clusters
    }

    /// `fontCollection()`.
    #[must_use]
    #[doc(alias = "fontCollection")]
    pub fn font_collection(&self) -> &FontCollection {
        &self.font_collection
    }

    /// `getUnicode()`.
    #[must_use]
    #[doc(alias = "getUnicode")]
    pub fn unicode(&self) -> &Arc<dyn Unicode> {
        &self.unicode
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.h#L172-L174 (chrome/m156)
    /// `codeUnitHasProperty(index, property)`.
    #[must_use]
    #[doc(alias = "codeUnitHasProperty")]
    pub fn code_unit_has_property(&self, index: usize, property: CodeUnitFlags) -> bool {
        (self.code_unit_properties[index] & property) == property
    }

    /// `getUTF16Index(index)`.
    #[must_use]
    #[doc(alias = "getUTF16Index")]
    pub fn get_utf16_index(&self, index: TextIndex) -> usize {
        self.utf16_mapping().utf16_index_for_utf8_index[index]
    }

    /// `strutEnabled()`.
    #[must_use]
    #[doc(alias = "strutEnabled")]
    pub fn strut_enabled(&self) -> bool {
        self.paragraph_style().strut_style().strut_enabled()
    }

    /// `strutForceHeight()`.
    #[must_use]
    #[doc(alias = "strutForceHeight")]
    pub fn strut_force_height(&self) -> bool {
        self.paragraph_style().strut_style().force_strut_height()
    }

    /// `strutHeightOverride()`.
    #[must_use]
    #[doc(alias = "strutHeightOverride")]
    pub fn strut_height_override(&self) -> bool {
        self.paragraph_style().strut_style().height_override()
    }

    /// `strutMetrics()`.
    #[must_use]
    #[doc(alias = "strutMetrics")]
    pub fn strut_metrics(&self) -> InternalLineMetrics {
        self.strut_metrics
    }

    /// `getEmptyMetrics()`.
    #[must_use]
    #[doc(alias = "getEmptyMetrics")]
    pub fn get_empty_metrics(&self) -> InternalLineMetrics {
        self.empty_metrics
    }

    /// `getStrutMetrics()`.
    #[must_use]
    #[doc(alias = "getStrutMetrics")]
    pub fn get_strut_metrics(&self) -> InternalLineMetrics {
        self.strut_metrics
    }

    /// `widthWithTrailingSpaces()`.
    #[must_use]
    #[doc(alias = "widthWithTrailingSpaces")]
    pub fn width_with_trailing_spaces(&self) -> scalar {
        self.max_width_with_trailing_spaces
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L869-L873 (chrome/m156)
    /// `clusters(clusterRange)`.
    #[must_use]
    #[doc(alias = "clusters")]
    pub fn clusters_of(&self, cluster_range: &ClusterRange) -> &[Cluster] {
        debug_assert!(
            cluster_range.start < self.clusters.len() && cluster_range.end <= self.clusters.len()
        );
        &self.clusters[cluster_range.start..cluster_range.end]
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L881-L884 (chrome/m156)
    /// `cluster(clusterIndex)`.
    #[must_use]
    pub fn cluster(&self, cluster_index: ClusterIndex) -> &Cluster {
        debug_assert!(cluster_index < self.clusters.len());
        &self.clusters[cluster_index]
    }

    /// `clusterIndex(textIndex)`.
    #[must_use]
    #[doc(alias = "clusterIndex")]
    pub fn cluster_index(&self, text_index: TextIndex) -> ClusterIndex {
        let cluster_index = self.clusters_index_from_code_unit[text_index];
        debug_assert!(cluster_index != EMPTY_INDEX);
        cluster_index
    }

    /// `run(runIndex)`.
    #[must_use]
    pub fn run(&self, run_index: RunIndex) -> &Run {
        debug_assert!(run_index < self.runs.len());
        &self.runs[run_index]
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L886-L889 (chrome/m156)
    /// `runByCluster(clusterIndex)`.
    #[must_use]
    #[doc(alias = "runByCluster")]
    pub fn run_by_cluster(&self, cluster_index: ClusterIndex) -> &Run {
        let start = self.cluster(cluster_index);
        self.run(start.run_index())
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L891-L896 (chrome/m156)
    /// `blocks(blockRange)`.
    #[must_use]
    pub fn blocks(&self, block_range: BlockRange) -> &[Block] {
        debug_assert!(
            block_range.start < self.text_styles.len() && block_range.end <= self.text_styles.len()
        );
        &self.text_styles[block_range.start..block_range.end]
    }

    /// `block(blockIndex)`.
    #[must_use]
    pub fn block(&self, block_index: BlockIndex) -> &Block {
        debug_assert!(block_index < self.text_styles.len());
        &self.text_styles[block_index]
    }

    /// `resolvedFonts()`.
    #[must_use]
    #[doc(alias = "resolvedFonts")]
    pub fn resolved_fonts(&self) -> Vec<ResolvedFontDescriptor> {
        self.font_switches.clone()
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.h#L152-L156 (chrome/m156)
    /// `markDirty()`.
    #[doc(alias = "markDirty")]
    pub fn mark_dirty(&mut self) {
        if self.state > InternalState::Indexed {
            self.state = InternalState::Indexed;
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L118-L124 (chrome/m156)
    /// `unresolvedGlyphs()`: the number of unresolved glyphs, or `None` if not applicable (has
    /// not been shaped yet - valid case).
    #[doc(alias = "unresolvedGlyphs")]
    pub fn unresolved_glyphs(&mut self) -> Option<usize> {
        if self.state < InternalState::Shaped {
            return None;
        }
        Some(self.unresolved_glyphs)
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L126-L128 (chrome/m156)
    /// `unresolvedCodepoints()`.
    #[must_use]
    #[doc(alias = "unresolvedCodepoints")]
    pub fn unresolved_codepoints(&self) -> HashSet<Unichar> {
        self.unresolved_codepoints.clone()
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L130-L136 (chrome/m156)
    /// `addUnresolvedCodepoints(textRange)`.
    #[doc(alias = "addUnresolvedCodepoints")]
    pub fn add_unresolved_codepoints(&mut self, text_range: TextRange) {
        let text = self.text[text_range.start..text_range.start + text_range.width()].to_vec();
        crate::one_line_shaper::for_each_codepoint(&text, |unichar, _start, _end, _count| {
            self.unresolved_codepoints.insert(unichar);
        });
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L138-L229 (chrome/m156)
    /// `layout(rawWidth)`.
    pub fn layout(&mut self, raw_width: scalar) {
        // TODO: This rounding is done to match Flutter tests. Must be removed...
        let mut floor_width = raw_width;
        if self.get_apply_rounding_hack() {
            floor_width = scalar_floor_to_scalar(floor_width);
        }

        if (!is_finite(raw_width) || self.longest_line <= floor_width)
            && self.state >= InternalState::LineBroken
            && self.lines.len() == 1
            && self.lines[0].ellipsis().is_none()
        {
            // Most common case: one line of text (and one line is never justified, so no cluster shifts)
            // We cannot mark it as kLineBroken because the new width can be bigger than the old width
            self.width = floor_width;
            self.state = InternalState::Shaped;
        } else if self.state >= InternalState::LineBroken && self.old_width != floor_width {
            // We can use the results from SkShaper but have to do EVERYTHING ELSE again
            self.state = InternalState::Shaped;
        } else {
            // Nothing changed case: we can reuse the data from the last layout
        }

        if self.state < InternalState::Shaped {
            // Check if we have the text in the cache and don't need to shape it again
            let font_collection = self.font_collection.clone();
            if !font_collection.paragraph_cache().find_paragraph(self) {
                if self.state < InternalState::Indexed {
                    // This only happens once at the first layout; the text is immutable
                    // and there is no reason to repeat it
                    if self.compute_code_unit_properties() {
                        self.state = InternalState::Indexed;
                    }
                }
                self.runs.clear();
                self.clusters.clear();
                self.clusters_index_from_code_unit.clear();
                self.clusters_index_from_code_unit
                    .resize(self.text.len() + 1, EMPTY_INDEX);
                if !self.shape_text_into_endless_line() {
                    self.reset_context();
                    // TODO: merge the two next calls - they always come together
                    self.resolve_strut();
                    self.compute_empty_metrics();
                    self.lines.clear();

                    // Set the important values that are not zero
                    self.width = floor_width;
                    self.height = self.empty_metrics.height();
                    if self.paragraph_style.strut_style().strut_enabled()
                        && self.paragraph_style.strut_style().force_strut_height()
                    {
                        self.height = self.strut_metrics.height();
                    }
                    self.alphabetic_baseline = self.empty_metrics.alphabetic_baseline();
                    self.ideographic_baseline = self.empty_metrics.ideographic_baseline();
                    // That is what flutter has: `FLT_MIN - FLT_MAX`
                    self.longest_line = f32::MIN_POSITIVE - f32::MAX;
                    self.min_intrinsic_width = 0.0;
                    self.max_intrinsic_width = 0.0;
                    self.old_width = floor_width;
                    self.old_height = self.height;

                    return;
                }
                // Add the paragraph to the cache
                font_collection.paragraph_cache().update_paragraph(self);
            }
            self.state = InternalState::Shaped;
        }

        if self.state == InternalState::Shaped {
            self.reset_context();
            self.resolve_strut();
            self.compute_empty_metrics();
            self.lines.clear();
            self.break_shaped_text_into_lines(floor_width);
            self.state = InternalState::LineBroken;
        }

        if self.state == InternalState::LineBroken {
            // Build the picture lazily not until we actually have to paint (or never)
            self.reset_shifts();
            let width = self.width;
            self.format_lines(width);
            self.state = InternalState::Formatted;
        }

        self.old_width = floor_width;
        self.old_height = self.height;

        if self.get_apply_rounding_hack() {
            // TODO: This rounding is done to match Flutter tests. Must be removed...
            self.min_intrinsic_width = little_round(self.min_intrinsic_width);
            self.max_intrinsic_width = little_round(self.max_intrinsic_width);
        }

        // TODO: This is strictly Flutter thing. Must be factored out into some flutter code
        if self.paragraph_style.max_lines_raw() == 1
            || (self.paragraph_style.unlimited_lines() && self.paragraph_style.ellipsized())
        {
            self.min_intrinsic_width = self.max_intrinsic_width;
        }

        // TODO: Since min and max are calculated differently it's possible to get a rounding error
        //  that would make min > max. Sort it out later, make it the same for now
        if self.max_intrinsic_width < self.min_intrinsic_width {
            self.max_intrinsic_width = self.min_intrinsic_width;
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L231-L234 (chrome/m156)
    /// `paint(canvas, x, y)`.
    pub fn paint(&self, canvas: &Canvas, p: impl Into<Point>) {
        let p = p.into();
        let mut painter = CanvasParagraphPainter::new(canvas);
        self.paint_with_painter(&mut painter, p.x, p.y);
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L236-L240 (chrome/m156)
    /// `paint(painter, x, y)`.
    pub fn paint_with_painter(&self, painter: &mut dyn ParagraphPainter, x: scalar, y: scalar) {
        for line in &self.lines {
            line.paint(self, painter, x, y);
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L242-L252 (chrome/m156)
    fn reset_context(&mut self) {
        self.alphabetic_baseline = 0.0;
        self.height = 0.0;
        self.width = 0.0;
        self.ideographic_baseline = 0.0;
        self.max_intrinsic_width = 0.0;
        self.min_intrinsic_width = 0.0;
        self.longest_line = 0.0;
        self.max_width_with_trailing_spaces = 0.0;
        self.exceeded_max_lines = false;
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L254-L303 (chrome/m156)
    /// `computeCodeUnitProperties()`: shapeTextIntoEndlessLine is the thing that calls this
    /// method.
    pub fn compute_code_unit_properties(&mut self) -> bool {
        // Get bidi regions
        let text_direction = if self.paragraph_style.text_direction() == TextDirection::LTR {
            UnicodeTextDirection::Ltr
        } else {
            UnicodeTextDirection::Rtl
        };
        let unicode = self.unicode.clone();
        if !unicode.get_bidi_regions(&self.text, text_direction, &mut self.bidi_regions) {
            return false;
        }

        // Collect all spaces and some extra information
        // (and also substitute \t with a space while we are at it)
        let replace_tabs = self.paragraph_style.replace_tab_characters();
        if !unicode.compute_code_unit_flags_utf8(
            &mut self.text,
            replace_tabs,
            &mut self.code_unit_properties,
        ) {
            return false;
        }

        // Get some information about trailing spaces / hard line breaks
        self.trailing_spaces = self.text.len();
        let mut first_whitespace: TextIndex = EMPTY_INDEX;
        for i in 0..self.code_unit_properties.len() {
            let flags = self.code_unit_properties[i];
            if flags.has_part_of_white_space_break_flag() {
                if self.trailing_spaces == self.text.len() {
                    self.trailing_spaces = i;
                }
                if first_whitespace == EMPTY_INDEX {
                    first_whitespace = i;
                }
            } else {
                self.trailing_spaces = self.text.len();
            }
            if flags.has_hard_line_break_flag() {
                self.has_line_breaks = true;
            }
        }

        if first_whitespace < self.trailing_spaces {
            self.has_whitespaces_inside = true;
        }

        true
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L392-L513 (chrome/m156)
    /// In some cases we apply spacing to glyphs first and then build the cluster table, in some we
    /// do the opposite - just to optimize the most common case.
    fn apply_spacing_and_build_cluster_table(&mut self) {
        // Check all text styles to see what we have to do (if anything)
        let mut letter_spacing_styles: usize = 0;
        let mut has_word_spacing = false;
        for block in &self.text_styles {
            if block.range.width() > 0 {
                if !skia_rust_core::scalar::Scalar::nearly_zero(&block.style.letter_spacing(), None)
                {
                    letter_spacing_styles += 1;
                }
                if !skia_rust_core::scalar::Scalar::nearly_zero(&block.style.word_spacing(), None) {
                    has_word_spacing = true;
                }
            }
        }

        if letter_spacing_styles == 0 && !has_word_spacing {
            // We don't have to do anything about spacing (most common case)
            self.build_cluster_table();
            return;
        }

        // Simple case: we have to letter space the entire paragraph (second most common case)
        // With one exception (there are whitespaces, and we have to space them as CSS spec requires)
        // which is treated as a common case down below
        if letter_spacing_styles == 1
            && !has_word_spacing
            && self.text_styles.len() == 1
            && self.text_styles[0].range.width() == self.text.len()
            && self.runs.len() == 1
            && !(self.has_whitespaces_inside && self.paragraph_style.letter_spacing_by_css_spec())
        {
            let letter_spacing = self.text_styles[0].style.letter_spacing();
            if !self.runs[0].is_cursive_script() {
                // Do not apply letter spacing for script languages (no exception here)
                self.runs[0].add_letter_spaces_evenly(letter_spacing);
            }

            self.build_cluster_table();

            // This is something Flutter requires
            for cluster in &mut self.clusters {
                cluster.set_half_letter_spacing(letter_spacing / 2.0);
            }

            return;
        }

        // The complex case: many text styles with spacing (possibly not adjusted to glyphs)
        self.build_cluster_table();

        // Walk through all the clusters in the direction of shaped text
        // (we have to walk through the styles in the same order, too)
        // Not breaking the iteration on every run!
        let mut shift: scalar = 0.0;
        let mut so_far_whitespaces_only = true;
        let mut word_spacing_pending = false;
        let mut last_space_cluster: Option<ClusterIndex> = None;
        let css_spec = self.paragraph_style.letter_spacing_by_css_spec();
        let code_unit_properties = std::mem::take(&mut self.code_unit_properties);
        for run_index in 0..self.runs.len() {
            // Skip placeholder runs
            if self.runs[run_index].is_placeholder() {
                continue;
            }

            // `run.iterateThroughClusters(visitor)`
            let (cluster_range, left_to_right) = {
                let run = &self.runs[run_index];
                (run.cluster_range.clone(), run.left_to_right())
            };
            for index in 0..cluster_range.width() {
                let correct_index = if left_to_right {
                    cluster_range.start + index
                } else {
                    cluster_range.end - index - 1
                };
                let cluster_index = correct_index;

                // Shift the cluster (shift collected from the previous clusters)
                {
                    let cluster = self.clusters[cluster_index].clone();
                    self.runs[run_index].shift_cluster(&cluster, shift);
                }

                // Synchronize styles (one cluster can be covered by few styles)
                let mut current_style = 0;
                while !self.clusters[cluster_index]
                    .starts_in(&self.text_styles[current_style].range)
                {
                    current_style += 1;
                    debug_assert!(current_style != self.text_styles.len());
                }

                debug_assert!(!self.text_styles[current_style].style.is_placeholder());

                // Process word spacing
                let word_spacing = self.text_styles[current_style].style.word_spacing();
                #[allow(clippy::float_cmp)] // mirrors `getWordSpacing() != 0`
                if word_spacing != 0.0 {
                    if self.clusters[cluster_index].is_whitespace_break()
                        && self.clusters[cluster_index].is_soft_break(&code_unit_properties)
                    {
                        if !so_far_whitespaces_only {
                            last_space_cluster = Some(cluster_index);
                            word_spacing_pending = true;
                        }
                    } else if word_spacing_pending {
                        let spacing = word_spacing;
                        let last = last_space_cluster.expect("a space cluster");
                        if self.clusters[cluster_index].run_index != self.clusters[last].run_index {
                            // If the last space cluster belongs to the previous run
                            // we have to extend that cluster and that run
                            let last_run = self.clusters[last].run_index;
                            let mut last_cluster = self.clusters[last].clone();
                            self.runs[last_run].add_spaces_at_the_end(spacing, &mut last_cluster);
                            self.clusters[last] = last_cluster.clone();
                            self.runs[last_run].extend(&last_cluster, spacing);
                        } else {
                            let mut last_cluster = self.clusters[last].clone();
                            self.runs[run_index].add_spaces_at_the_end(spacing, &mut last_cluster);
                            self.clusters[last] = last_cluster;
                        }

                        {
                            let cluster = self.clusters[cluster_index].clone();
                            self.runs[run_index].shift_cluster(&cluster, spacing);
                        }
                        shift += spacing;
                        word_spacing_pending = false;
                    }
                }
                // We do not apply letter spacing for script languages
                // except for one case (cluster is whitespace, and we have to follow CSS spec)
                let letter_spacing = self.text_styles[current_style].style.letter_spacing();
                #[allow(clippy::float_cmp)] // mirrors `getLetterSpacing() != 0`
                if letter_spacing != 0.0
                    && (!self.runs[run_index].is_cursive_script()
                        || (self.clusters[cluster_index].is_whitespace_break() && css_spec))
                {
                    let mut cluster = self.clusters[cluster_index].clone();
                    shift += self.runs[run_index]
                        .add_letter_spaces_evenly_in_cluster(letter_spacing, &mut cluster);
                    self.clusters[cluster_index] = cluster;
                }

                if so_far_whitespaces_only && !self.clusters[cluster_index].is_whitespace_break() {
                    so_far_whitespaces_only = false;
                }
            }
        }
        self.code_unit_properties = code_unit_properties;
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L515-L584 (chrome/m156)
    /// Clusters in the order of the input text.
    fn build_cluster_table(&mut self) {
        // It's possible that one grapheme includes few runs; we cannot handle it
        // so we break graphemes by the runs instead
        // It's not the ideal solution and has to be revisited later
        let mut cluster_count: usize = 1;
        for run in &self.runs {
            cluster_count += if run.is_placeholder() { 1 } else { run.size() };
            self.code_unit_properties[run.text_range.start] |= CodeUnitFlags::GRAPHEME_START;
            self.code_unit_properties[run.text_range.start] |= CodeUnitFlags::GLYPH_CLUSTER_START;
        }
        if let Some(last_run) = self.runs.last() {
            self.code_unit_properties[last_run.text_range().end] |= CodeUnitFlags::GRAPHEME_START;
            self.code_unit_properties[last_run.text_range().end] |=
                CodeUnitFlags::GLYPH_CLUSTER_START;
        }
        self.clusters
            .reserve_exact(self.clusters.len() + cluster_count);

        // Walk through all the run in the direction of input text
        for run_index in 0..self.runs.len() {
            let run_start = self.clusters.len();
            let run_text_range = self.runs[run_index].text_range();
            let run_ref_index = self.runs[run_index].index();
            if self.runs[run_index].is_placeholder() {
                // Add info to cluster indexes table (text -> cluster)
                for i in run_text_range.start..run_text_range.end {
                    self.clusters_index_from_code_unit[i] = self.clusters.len();
                }
                // There are no glyphs but we want to have one cluster
                let run_advance = self.runs[run_index].advance();
                let cluster = Cluster::new(
                    &self.code_unit_properties,
                    &self.text,
                    run_ref_index,
                    0,
                    1,
                    run_text_range.clone(),
                    run_advance.x,
                    run_advance.y,
                );
                self.clusters.push(cluster);
                self.code_unit_properties[run_text_range.start] |=
                    CodeUnitFlags::SOFT_LINE_BREAK_BEFORE;
                self.code_unit_properties[run_text_range.end] |=
                    CodeUnitFlags::SOFT_LINE_BREAK_BEFORE;
            } else {
                // Walk through the glyph in the direction of input text
                let mut visits: Vec<(usize, usize, usize, usize, scalar, scalar)> = Vec::new();
                self.runs[run_index].iterate_through_clusters_in_text_order(
                    |glyph_start, glyph_end, char_start, char_end, width, height| {
                        visits.push((glyph_start, glyph_end, char_start, char_end, width, height));
                    },
                );
                for (glyph_start, glyph_end, char_start, char_end, width, height) in visits {
                    debug_assert!(char_end >= char_start);
                    // Add info to cluster indexes table (text -> cluster)
                    for i in char_start..char_end {
                        self.clusters_index_from_code_unit[i] = self.clusters.len();
                    }

                    let cluster = Cluster::new(
                        &self.code_unit_properties,
                        &self.text,
                        run_ref_index,
                        glyph_start,
                        glyph_end,
                        char_start..char_end,
                        width,
                        height,
                    );
                    self.clusters.push(cluster);
                    self.code_unit_properties[char_start] |= CodeUnitFlags::GLYPH_CLUSTER_START;
                }
            }
            self.code_unit_properties[run_text_range.start] |= CodeUnitFlags::GLYPH_CLUSTER_START;

            let clusters_len = self.clusters.len();
            self.runs[run_index].set_cluster_range(run_start, clusters_len);
            self.max_intrinsic_width += self.runs[run_index].advance().x;
        }
        let text_len = self.text.len();
        self.clusters_index_from_code_unit[text_len] = self.clusters.len();
        let sentinel = Cluster::new(
            &self.code_unit_properties,
            &self.text,
            EMPTY_RUN,
            0,
            0,
            text_len..text_len,
            0.0,
            0.0,
        );
        self.clusters.push(sentinel);
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L586-L601 (chrome/m156)
    /// `shapeTextIntoEndlessLine()`.
    pub fn shape_text_into_endless_line(&mut self) -> bool {
        if self.text.is_empty() {
            return false;
        }

        self.unresolved_codepoints.clear();
        self.font_switches.clear();

        let (result, unresolved_glyphs) = {
            let mut one_line_shaper = OneLineShaper::new(self);
            let result = one_line_shaper.shape();
            (result, one_line_shaper.unresolved_glyphs())
        };
        self.unresolved_glyphs = unresolved_glyphs;

        self.apply_spacing_and_build_cluster_table();

        result
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L603-L712 (chrome/m156)
    /// `breakShapedTextIntoLines(maxWidth)`.
    fn break_shaped_text_into_lines(&mut self, max_width: scalar) {
        if !self.has_line_breaks
            && !self.has_whitespaces_inside
            && self.placeholders.len() == 1
            && self.runs.len() == 1
            && self.runs[0].advance.x <= max_width
        {
            // This is a short version of a line breaking when we know that:
            // 1. We have only one line of text
            // 2. It's shaped into a single run
            // 3. There are no placeholders
            // 4. There are no linebreaks (which will format text into multiple lines)
            // 5. There are no whitespaces so the minIntrinsicWidth=maxIntrinsicWidth
            // (To think about that, the last condition is not quite right;
            // we should calculate minIntrinsicWidth by soft line breaks.
            // However, it's how it's done in Flutter now)
            let run_advance = self.runs[0].advance();
            let mut advance = run_advance;
            let text_range: TextRange = 0..self.text().len();
            let text_excluding_spaces: TextRange = 0..self.trailing_spaces;
            let mut metrics = InternalLineMetrics::new(self.strut_force_height());
            metrics.add_run(&self.runs[0]);
            let disable_first_ascent = self
                .paragraph_style()
                .text_height_behavior()
                .disable_first_ascent();
            let disable_last_descent = self
                .paragraph_style()
                .text_height_behavior()
                .disable_last_descent();
            if disable_first_ascent {
                metrics.ascent = metrics.raw_ascent;
            }
            if disable_last_descent {
                metrics.descent = metrics.raw_descent;
            }
            if self.strut_enabled() {
                self.strut_metrics().update_line_metrics(&mut metrics);
            }
            let mut trailing_spaces: ClusterIndex = self.clusters.len();
            loop {
                trailing_spaces -= 1;
                let cluster = &self.clusters[trailing_spaces];
                if !cluster.is_whitespace_break() {
                    trailing_spaces += 1;
                    break;
                }
                advance.x -= cluster.width();
                if trailing_spaces == 0 {
                    break;
                }
            }

            advance.y = metrics.height();
            let cluster_range: ClusterRange = 0..trailing_spaces;
            let cluster_range_with_ghosts: ClusterRange = 0..(self.clusters().len() - 1);
            let run_advance_x = self.runs[0].advance().x;
            self.add_line(
                Point::new(0.0, 0.0),
                advance,
                text_excluding_spaces,
                text_range.clone(),
                text_range,
                cluster_range,
                cluster_range_with_ghosts,
                run_advance_x,
                metrics,
            );

            self.longest_line = if crate::text_style::nearly_zero_default(advance.x) {
                self.runs[0].advance.x
            } else {
                advance.x
            };
            self.height = advance.y;
            self.width = max_width;
            self.max_intrinsic_width = self.runs[0].advance.x;
            self.min_intrinsic_width = advance.x;
            self.alphabetic_baseline = match self.lines.first() {
                None => self.empty_metrics.alphabetic_baseline(),
                Some(line) => line.alphabetic_baseline(),
            };
            self.ideographic_baseline = match self.lines.first() {
                None => self.empty_metrics.ideographic_baseline(),
                Some(line) => line.ideographic_baseline(),
            };
            self.exceeded_max_lines = false;
            return;
        }

        let mut text_wrapper = TextWrapper::new();
        text_wrapper.break_text_into_lines(
            self,
            max_width,
            &mut |this: &mut Paragraph, args: crate::text_wrapper::AddLineArgs| {
                // TODO: Take in account clipped edges
                let width_with_spaces = args.width_with_spaces;
                this.add_line(
                    args.offset,
                    args.advance,
                    args.text_excluding_spaces,
                    args.text,
                    args.text_including_newlines,
                    args.clusters,
                    args.clusters_with_ghosts,
                    width_with_spaces,
                    args.metrics,
                );
                let mut line = this.lines.pop().expect("the line just added");
                if args.add_ellipsis {
                    let ellipsis = this.get_ellipsis();
                    line.create_ellipsis(this, max_width, &ellipsis, true);
                }
                if this.paragraph_style().render_soft_hyphens() {
                    line.create_soft_hyphen(this);
                }
                // `std::max(a, b)` is `a < b ? b : a`.
                let candidate = if crate::text_style::nearly_zero_default(line.width()) {
                    width_with_spaces
                } else {
                    line.width()
                };
                if this.longest_line < candidate {
                    this.longest_line = candidate;
                }
                this.lines.push(line);
            },
        );

        self.height = text_wrapper.height();
        self.width = max_width;
        self.max_intrinsic_width = text_wrapper.max_intrinsic_width();
        self.min_intrinsic_width = text_wrapper.min_intrinsic_width();
        self.alphabetic_baseline = match self.lines.first() {
            None => self.empty_metrics.alphabetic_baseline(),
            Some(line) => line.alphabetic_baseline(),
        };
        self.ideographic_baseline = match self.lines.first() {
            None => self.empty_metrics.ideographic_baseline(),
            Some(line) => line.ideographic_baseline(),
        };
        self.exceeded_max_lines = text_wrapper.exceeded_max_lines();
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L714-L731 (chrome/m156)
    /// `formatLines(maxWidth)`.
    pub fn format_lines(&mut self, max_width: scalar) {
        let effective_align = self.paragraph_style.effective_align();
        let is_left_aligned = effective_align == TextAlign::Left
            || (effective_align == TextAlign::Justify
                && self.paragraph_style.text_direction() == TextDirection::LTR);

        if !is_finite(max_width) && !is_left_aligned {
            // Special case: clean all text in case of maxWidth == INF & align != left
            // We had to go through shaping though because we need all the measurement numbers
            self.lines.clear();
            return;
        }

        let mut lines = std::mem::take(&mut self.lines);
        for line in &mut lines {
            line.format(self, effective_align, max_width);
        }
        self.lines = lines;
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L733-L783 (chrome/m156)
    /// `resolveStrut()`.
    pub fn resolve_strut(&mut self) {
        let strut_style = self.paragraph_style().strut_style().clone();
        if !strut_style.strut_enabled() || strut_style.font_size() < 0.0 {
            return;
        }

        let typefaces = self.font_collection.find_typefaces_with_font_arguments(
            strut_style.font_families(),
            strut_style.font_style(),
            None,
        );
        if typefaces.is_empty() {
            // `SkDEBUGF("Could not resolve strut font\n")`
            return;
        }

        let font = Font::from_size(typefaces[0].clone(), strut_style.font_size());
        let (_, metrics) = font.metrics();
        let strut_leading: scalar = if strut_style.leading() < 0.0 {
            0.0
        } else {
            strut_style.leading() * strut_style.font_size()
        };

        if strut_style.height_override() {
            let strut_ascent: scalar;
            let strut_descent: scalar;
            // The half leading flag doesn't take effect unless there's height override.
            if strut_style.half_leading() {
                let occupied_height = metrics.descent - metrics.ascent;
                let mut flexible_height =
                    strut_style.height() * strut_style.font_size() - occupied_height;
                // Distribute the flexible height evenly over and under.
                flexible_height /= 2.0;
                strut_ascent = metrics.ascent - flexible_height;
                strut_descent = metrics.descent + flexible_height;
            } else {
                let strut_metrics_height = metrics.descent - metrics.ascent + metrics.leading;
                #[allow(clippy::float_cmp)] // mirrors `strutMetricsHeight == 0`
                let strut_height_multiplier = if strut_metrics_height == 0.0 {
                    strut_style.height()
                } else {
                    strut_style.height() * strut_style.font_size() / strut_metrics_height
                };
                strut_ascent = metrics.ascent * strut_height_multiplier;
                strut_descent = metrics.descent * strut_height_multiplier;
            }
            self.strut_metrics = InternalLineMetrics::from_adl_raw(
                strut_ascent,
                strut_descent,
                strut_leading,
                metrics.ascent,
                metrics.descent,
                metrics.leading,
            );
        } else {
            self.strut_metrics =
                InternalLineMetrics::from_adl(metrics.ascent, metrics.descent, strut_leading);
        }
        let force = self.paragraph_style().strut_style().force_strut_height();
        self.strut_metrics.set_force_strut(force);
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L785-L811 (chrome/m156)
    /// `findAllBlocks(textRange)`.
    #[doc(alias = "findAllBlocks")]
    pub fn find_all_blocks(&self, text_range: TextRange) -> BlockRange {
        let mut begin: BlockIndex = EMPTY_BLOCK;
        let mut end: BlockIndex = EMPTY_BLOCK;
        for index in 0..self.text_styles.len() {
            let block = &self.text_styles[index];
            if block.range.end <= text_range.start {
                continue;
            }
            if block.range.start >= text_range.end {
                break;
            }
            if begin == EMPTY_BLOCK {
                begin = index;
            }
            end = index;
        }

        if begin == EMPTY_INDEX || end == EMPTY_INDEX {
            // It's possible if some text is not covered with any text style
            // Not in Flutter but in direct use of SkParagraph
            return EMPTY_RANGE;
        }

        begin..(end + 1)
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L813-L832 (chrome/m156)
    /// `addLine(offset, advance, textExcludingSpaces, text, textIncludingNewlines, clusters,
    /// clustersWithGhosts, widthWithSpaces, sizes)`: adds the line and returns it.
    #[allow(clippy::too_many_arguments)] // mirrors ParagraphImpl::addLine
    #[doc(alias = "addLine")]
    pub fn add_line(
        &mut self,
        offset: Point,
        advance: Point,
        text_excluding_spaces: TextRange,
        text: TextRange,
        text_including_new_lines: TextRange,
        clusters: ClusterRange,
        clusters_with_ghosts: ClusterRange,
        width_with_spaces: scalar,
        sizes: InternalLineMetrics,
    ) -> &mut TextLine {
        // Define a list of styles that covers the line
        let blocks = self.find_all_blocks(text_excluding_spaces.clone());
        let line = TextLine::new(
            self,
            self.lines.len(),
            offset,
            advance,
            blocks,
            text_excluding_spaces,
            text,
            text_including_new_lines,
            clusters,
            clusters_with_ghosts,
            width_with_spaces,
            sizes,
        );
        self.lines.push(line);
        self.lines.last_mut().expect("the line just added")
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L834-L893 (chrome/m156)
    /// `getRectsForRange(start, end, rectHeightStyle, rectWidthStyle)`: a vector of bounding
    /// boxes that enclose all text between start and end glyph indexes, including start and
    /// excluding end.
    #[must_use]
    #[doc(alias = "getRectsForRange")]
    pub fn get_rects_for_range(
        &self,
        range: Range<usize>,
        rect_height_style: RectHeightStyle,
        rect_width_style: RectWidthStyle,
    ) -> Vec<TextBox> {
        let (start, end) = (range.start, range.end);
        let mut results: Vec<TextBox> = Vec::new();
        if self.text.is_empty() {
            if start == 0 && end > 0 {
                // On account of implied "\n" that is always at the end of the text
                results.push(TextBox::new(
                    Rect::from_xywh(0.0, 0.0, 0.0, self.height),
                    self.paragraph_style.text_direction(),
                ));
            }
            return results;
        }

        self.ensure_utf16_mapping();
        let mapping = self.utf16_mapping();
        let utf8_for_utf16 = &mapping.utf8_index_for_utf16_index;

        if start >= end || start > utf8_for_utf16.len() || end == 0 {
            return results;
        }

        // Adjust the text to grapheme edges
        // Apparently, text editor CAN move inside graphemes but CANNOT select a part of it.
        // I don't know why - the solution I have here returns an empty box for every query that
        // does not contain an end of a grapheme.
        // Once a cursor is inside a complex grapheme I can press backspace and cause trouble.
        // To avoid any problems, I will not allow any selection of a part of a grapheme.
        // One flutter test fails because of it but the editing experience is correct
        // (although you have to press the cursor many times before it moves to the next grapheme).
        let mut text: TextRange = self.text.len()..self.text.len();
        // TODO: This is probably a temp change that makes SkParagraph work as TxtLib
        //  (so we can compare the results). We now include in the selection box only the graphemes
        //  that belongs to the given [start:end) range entirely (not the ones that intersect with it)
        if start < utf8_for_utf16.len() {
            let mut utf8 = utf8_for_utf16[start];
            // If start points to a trailing surrogate, skip it
            if start > 0 && utf8_for_utf16[start - 1] == utf8 {
                utf8 = utf8_for_utf16[start + 1];
            }
            text.start = self.find_next_grapheme_boundary(utf8);
        }
        if end < utf8_for_utf16.len() {
            let utf8 = self.find_previous_grapheme_boundary(utf8_for_utf16[end]);
            text.end = utf8;
        }
        for line in &self.lines {
            let line_text = line.text_with_newlines();
            let intersect = crate::text_line::range_intersection(&line_text, &text);
            if intersect.empty() && line_text.start != text.start {
                continue;
            }

            line.get_rects_for_range(
                self,
                intersect,
                rect_height_style,
                rect_width_style,
                &mut results,
            );
        }
        results
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L895-L923 (chrome/m156)
    /// `getRectsForPlaceholders()`.
    #[must_use]
    #[doc(alias = "getRectsForPlaceholders")]
    pub fn get_rects_for_placeholders(&self) -> Vec<TextBox> {
        let mut boxes: Vec<TextBox> = Vec::new();
        if self.text.is_empty() {
            return boxes;
        }
        if self.placeholders.len() == 1 {
            // We always have one fake placeholder
            return boxes;
        }
        for line in &self.lines {
            line.get_rects_for_placeholders(self, &mut boxes);
        }
        boxes
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L925-L953 (chrome/m156)
    /// `getGlyphPositionAtCoordinate(dx, dy)`: the index of the glyph that corresponds to the
    /// provided coordinate, with the top left corner as the origin, and +y direction as down.
    #[must_use]
    #[doc(alias = "getGlyphPositionAtCoordinate")]
    pub fn get_glyph_position_at_coordinate(&self, p: impl Into<Point>) -> PositionWithAffinity {
        let p = p.into();
        let (dx, dy) = (p.x, p.y);
        if self.text.is_empty() {
            return PositionWithAffinity {
                position: 0,
                affinity: Affinity::Downstream,
            };
        }

        self.ensure_utf16_mapping();

        for (index, line) in self.lines.iter().enumerate() {
            // Let's figure out if we can stop looking
            let offset_y = line.offset().y;
            if dy >= offset_y + line.height() && index + 1 != self.lines.len() {
                // This line is not good enough
                continue;
            }

            // This is so far the the line vertically closest to our coordinates
            // (or the first one, or the only one - all the same)

            return line.get_glyph_position_at_coordinate(self, dx);
        }

        PositionWithAffinity {
            position: 0,
            affinity: Affinity::Downstream,
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L955-L985 (chrome/m156)
    /// `getWordBoundary(offset)`: finds the first and last glyphs that define a word containing
    /// the glyph at index offset. By "glyph" they mean a character index - indicated by Minikin's
    /// code.
    #[must_use]
    #[doc(alias = "getWordBoundary")]
    pub fn get_word_boundary(&mut self, offset: u32) -> Range<usize> {
        if self.words.is_empty() {
            let mut words: Vec<usize> = Vec::new();
            if !self.unicode.get_words(&self.text, None, &mut words) {
                return 0..0;
            }
            self.words = words;
        }

        let mut start: i32 = 0;
        let mut end: i32 = 0;
        for i in 0..self.words.len() {
            let word = self.words[i];
            if word <= offset as usize {
                start = word as i32;
                end = word as i32;
            } else if word > offset as usize {
                end = word as i32;
                break;
            }
        }

        (start as u32 as usize)..(end as u32 as usize)
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L987-L993 (chrome/m156)
    /// `getLineMetrics(metrics)`.
    #[must_use]
    #[doc(alias = "getLineMetrics")]
    pub fn get_line_metrics(&self) -> Vec<LineMetrics<'_>> {
        let mut metrics = Vec::new();
        for line in &self.lines {
            metrics.push(line.get_metrics(self));
        }
        metrics
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1004-L1040 (chrome/m156)
    /// `setState(state)`.
    #[doc(alias = "setState")]
    pub fn set_state(&mut self, state: InternalState) {
        if self.state <= state {
            self.state = state;
            return;
        }

        self.state = state;
        // The fall-through of the C++ `switch`.
        if self.state == InternalState::Unknown {
            debug_assert!(false);
        }
        if self.state <= InternalState::Indexed {
            self.runs.clear();
            self.clusters.clear();
        }
        if self.state <= InternalState::Shaped {
            self.lines.clear();
        }
        // `fPicture = nullptr` for the states up to kLineBroken: there is no picture.
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1042-L1090 (chrome/m156)
    fn compute_empty_metrics(&mut self) {
        // The empty metrics is used to define the height of the empty lines
        // Unfortunately, Flutter has 2 different cases for that:
        // 1. An empty line inside the text
        // 2. An empty paragraph
        // In the first case SkParagraph takes the metrics from the default paragraph style
        // In the second case it should take it from the current text style
        let empty_paragraph = self.runs.is_empty();
        let mut text_style: TextStyle = self.paragraph_style().text_style().clone();
        if empty_paragraph && !self.text_styles.is_empty() {
            text_style = self.text_styles.last().expect("a block").style.clone();
        }

        let typefaces = self.font_collection().find_typefaces_with_font_arguments(
            text_style.font_families(),
            text_style.font_style(),
            text_style.font_arguments(),
        );
        let typeface = typefaces.first().cloned();

        let mut font = Font::from_typeface(typeface);
        font.set_size(text_style.font_size());
        self.empty_metrics = InternalLineMetrics::from_font(
            &font,
            self.paragraph_style().strut_style().force_strut_height(),
        );

        if !self.paragraph_style().strut_style().force_strut_height()
            && text_style.height_override()
        {
            let intrinsic_height = self.empty_metrics.height();
            let strut_height = text_style.height() * text_style.font_size();
            if self.paragraph_style().strut_style().half_leading() {
                self.empty_metrics.update(
                    self.empty_metrics.ascent(),
                    self.empty_metrics.descent(),
                    self.empty_metrics.leading() + strut_height - intrinsic_height,
                );
            } else {
                let multiplier = strut_height / intrinsic_height;
                self.empty_metrics.update(
                    self.empty_metrics.ascent() * multiplier,
                    self.empty_metrics.descent() * multiplier,
                    self.empty_metrics.leading() * multiplier,
                );
            }
        }

        if empty_paragraph {
            // For an empty text we apply both TextHeightBehaviour flags
            // In case of non-empty paragraph TextHeightBehaviour flags will be applied at the appropriate place
            // We have to do it here because we skip wrapping for an empty text
            let disable_first_ascent = self
                .paragraph_style()
                .text_height_behavior()
                .disable_first_ascent();
            let disable_last_descent = self
                .paragraph_style()
                .text_height_behavior()
                .disable_last_descent();
            self.empty_metrics.update(
                if disable_first_ascent {
                    self.empty_metrics.raw_ascent()
                } else {
                    self.empty_metrics.ascent()
                },
                if disable_last_descent {
                    self.empty_metrics.raw_descent()
                } else {
                    self.empty_metrics.descent()
                },
                self.empty_metrics.leading(),
            );
        }

        if self.paragraph_style.strut_style().strut_enabled() {
            let strut_metrics = self.strut_metrics;
            strut_metrics.update_line_metrics(&mut self.empty_metrics);
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1092-L1102 (chrome/m156)
    /// `getEllipsis()`.
    #[must_use]
    #[doc(alias = "getEllipsis")]
    pub fn get_ellipsis(&self) -> Vec<u8> {
        let ellipsis8 = self.paragraph_style.ellipsis();
        if !ellipsis8.is_empty() {
            ellipsis8.as_bytes().to_vec()
        } else {
            convert_utf16_to_utf8(self.paragraph_style.ellipsis_utf16()).into_bytes()
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1104-L1118 (chrome/m156)
    /// `updateFontSize(from, to, fontSize)`.
    #[doc(alias = "updateFontSize")]
    pub fn update_font_size(&mut self, from: usize, to: usize, font_size: scalar) {
        debug_assert!(from == 0 && to == self.text.len());
        let mut default_style = self.paragraph_style.text_style().clone();
        default_style.set_font_size(font_size);
        self.paragraph_style.set_text_style(&default_style);

        for text_style in &mut self.text_styles {
            text_style.style.set_font_size(font_size);
        }

        self.state = self.state.min(InternalState::Indexed);
        self.old_width = 0.0;
        self.old_height = 0.0;
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1120-L1127 (chrome/m156)
    /// `updateTextAlign(textAlign)`.
    #[doc(alias = "updateTextAlign")]
    pub fn update_text_align(&mut self, text_align: TextAlign) {
        self.paragraph_style.set_text_align(text_align);

        if self.state >= InternalState::LineBroken {
            self.state = InternalState::LineBroken;
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1129-L1138 (chrome/m156)
    /// `updateForegroundPaint(from, to, paint)`.
    #[doc(alias = "updateForegroundPaint")]
    pub fn update_foreground_paint(&mut self, from: usize, to: usize, paint: &Paint) {
        debug_assert!(from == 0 && to == self.text.len());
        let mut default_style = self.paragraph_style.text_style().clone();
        default_style.set_foreground_color(paint);
        self.paragraph_style.set_text_style(&default_style);

        for text_style in &mut self.text_styles {
            text_style.style.set_foreground_color(paint);
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1140-L1149 (chrome/m156)
    /// `updateBackgroundPaint(from, to, paint)`.
    #[doc(alias = "updateBackgroundPaint")]
    pub fn update_background_paint(&mut self, from: usize, to: usize, paint: &Paint) {
        debug_assert!(from == 0 && to == self.text.len());
        let mut default_style = self.paragraph_style.text_style().clone();
        default_style.set_background_color(paint);
        self.paragraph_style.set_text_style(&default_style);

        for text_style in &mut self.text_styles {
            text_style.style.set_background_color(paint);
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1151-L1164 (chrome/m156)
    /// `countSurroundingGraphemes(textRange)`.
    #[must_use]
    #[doc(alias = "countSurroundingGraphemes")]
    pub fn count_surrounding_graphemes(&self, text_range: TextRange) -> Vec<TextIndex> {
        let text_range = text_range.intersection(&(0..self.text.len()));
        let mut graphemes: Vec<TextIndex> = Vec::new();
        if (self.code_unit_properties[text_range.start] & CodeUnitFlags::GRAPHEME_START).is_empty()
        {
            // Count the previous partial grapheme
            graphemes.push(text_range.start);
        }
        for index in text_range.start..text_range.end {
            if !(self.code_unit_properties[index] & CodeUnitFlags::GRAPHEME_START).is_empty() {
                graphemes.push(index);
            }
        }
        graphemes
    }

    /// `findPreviousGraphemeBoundary(utf8)`.
    #[must_use]
    #[doc(alias = "findPreviousGraphemeBoundary")]
    pub fn find_previous_grapheme_boundary(&self, utf8: TextIndex) -> TextIndex {
        find_previous_grapheme_boundary(&self.code_unit_properties, utf8)
    }

    /// `findNextGraphemeBoundary(utf8)`.
    #[must_use]
    #[doc(alias = "findNextGraphemeBoundary")]
    pub fn find_next_grapheme_boundary(&self, utf8: TextIndex) -> TextIndex {
        find_next_grapheme_boundary(&self.code_unit_properties, self.text.len(), utf8)
    }

    /// `findNextGlyphClusterBoundary(utf8)`.
    #[must_use]
    #[doc(alias = "findNextGlyphClusterBoundary")]
    pub fn find_next_glyph_cluster_boundary(&self, utf8: TextIndex) -> TextIndex {
        find_next_glyph_cluster_boundary(&self.code_unit_properties, self.text.len(), utf8)
    }

    /// `findPreviousGlyphClusterBoundary(utf8)`.
    #[must_use]
    #[doc(alias = "findPreviousGlyphClusterBoundary")]
    pub fn find_previous_glyph_cluster_boundary(&self, utf8: TextIndex) -> TextIndex {
        find_previous_glyph_cluster_boundary(&self.code_unit_properties, utf8)
    }

    fn utf16_mapping(&self) -> &Utf16Mapping {
        self.utf16_mapping.get_or_init(|| {
            let mut mapping = Utf16Mapping::default();
            let Utf16Mapping {
                utf8_index_for_utf16_index,
                utf16_index_for_utf8_index,
            } = &mut mapping;
            extract_utf_conversion_mapping(
                &self.text,
                |index| utf8_index_for_utf16_index.push(index),
                |index| utf16_index_for_utf8_index.push(index),
            );
            mapping
        })
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1182-L1188 (chrome/m156)
    /// `ensureUTF16Mapping()`.
    #[doc(alias = "ensureUTF16Mapping")]
    pub fn ensure_utf16_mapping(&self) {
        let _ = self.utf16_mapping();
    }

    /// `fUTF8IndexForUTF16Index`.
    #[must_use]
    pub fn utf8_index_for_utf16_index(&self) -> &[TextIndex] {
        &self.utf16_mapping().utf8_index_for_utf16_index
    }

    /// `fUTF16IndexForUTF8Index`.
    #[must_use]
    pub fn utf16_index_for_utf8_index(&self) -> &[usize] {
        &self.utf16_mapping().utf16_index_for_utf8_index
    }

    /// `resetShifts()`.
    #[doc(alias = "resetShifts")]
    pub fn reset_shifts(&mut self) {
        for run in &mut self.runs {
            run.reset_justification_shifts();
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1190-L1241 (chrome/m156)
    /// `visit(visitor)`: `visitor(lineNumber, info)`; the info is `None` to signal the end of
    /// that line.
    pub fn visit(&mut self, mut visitor: impl FnMut(usize, Option<&VisitorInfo<'_>>)) {
        let mut line_number: usize = 0;
        for line in &self.lines {
            line.ensure_text_blob_cache_populated(self);
            for rec in line.text_blob_cache() {
                let Some(blob) = &rec.blob else {
                    continue;
                };

                let run_ref = line.run_of(self, rec.visitor_run);
                let cluster_storage: Vec<u32>;
                let cluster_slice: &[u32] = if run_ref.cluster_start > 0 {
                    #[allow(clippy::cast_possible_truncation)] // mirrors the uint32_t sum of C++
                    {
                        cluster_storage = run_ref
                            .cluster_indexes
                            .iter()
                            .map(|&index| (run_ref.cluster_start as u32).wrapping_add(index))
                            .collect();
                    }
                    &cluster_storage
                } else {
                    &run_ref.cluster_indexes
                };
                let mut cluster_ptr = rec.visitor_pos;

                let mut iter = blob.run_iter();
                while !iter.done() {
                    let font = iter.font().expect("a run with a font");
                    let count = iter.glyph_count();
                    let glyphs = iter.glyphs();
                    let positions = iter.points();
                    let info = VisitorInfo {
                        font,
                        origin: rec.offset,
                        advance_x: rec.clip_rect.right,
                        count,
                        glyphs,
                        positions,
                        utf8_starts: &cluster_slice[cluster_ptr..],
                        flags: 0,
                    };
                    visitor(line_number, Some(&info));
                    cluster_ptr += count;
                    iter.next();
                }
            }
            visitor(line_number, None); // signal end of line
            line_number += 1;
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1363-L1417 (chrome/m156)
    /// `extendedVisit(visitor)`.
    pub fn extended_visit(
        &mut self,
        mut visitor: impl FnMut(usize, Option<&ExtendedVisitorInfo<'_>>),
    ) {
        let mut line_number: usize = 0;
        for line in &self.lines {
            line.iterate_through_visual_runs(
                self,
                false,
                &mut |run: &Run,
                      run_offset_in_line,
                      text_range: TextRange,
                      run_width_in_line: &mut scalar| {
                    *run_width_in_line = line.iterate_through_single_run_by_styles(
                        self,
                        TextAdjustment::GLYPH_CLUSTER,
                        run,
                        run_offset_in_line,
                        text_range,
                        StyleType::None,
                        &mut |_text_range: TextRange,
                              style: &TextStyle,
                              context: &ClipContext<'_>| {
                            let corrected_baseline = scalar_floor_to_scalar(
                                (f64::from(line.baseline() + style.baseline_shift()) + 0.5)
                                    as scalar,
                            );
                            let offset = Point::new(
                                line.offset().x + context.text_shift,
                                line.offset().y + corrected_baseline,
                            );
                            let rect = context.clip.with_offset(line.offset());
                            let mut glyph_bounds = vec![Rect::new_empty(); run.size()];
                            run.font().get_widths_bounds(
                                run.glyphs(),
                                &mut [],
                                &mut glyph_bounds,
                                None,
                            );
                            let cluster_storage: Vec<u32>;
                            let cluster_slice: &[u32] = if run.cluster_start > 0 {
                                #[allow(clippy::cast_possible_truncation)]
                                // mirrors the uint32_t sum
                                {
                                    cluster_storage = (0..context.size)
                                        .map(|i| {
                                            (run.cluster_start as u32)
                                                .wrapping_add(run.cluster_indexes[i])
                                        })
                                        .collect();
                                }
                                &cluster_storage
                            } else {
                                run.cluster_indexes()
                            };
                            let info = ExtendedVisitorInfo {
                                font: run.font(),
                                origin: offset,
                                advance: Size::new(rect.width(), rect.height()),
                                count: context.size,
                                glyphs: &run.glyphs()[context.pos..],
                                positions: &run.positions[context.pos..],
                                bounds: &glyph_bounds[context.pos..],
                                utf8_starts: cluster_slice,
                                flags: 0,
                            };
                            visitor(line_number, Some(&info));
                        },
                    );
                    true
                },
            );
            visitor(line_number, None); // signal end of line
            line_number += 1;
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1243-L1260 (chrome/m156)
    /// `getLineNumberAt(codeUnitIndex)`: the line number of the line that contains the given
    /// UTF-8 index, or `None` if the index is out of bounds, or when the glyph is truncated or
    /// ellipsized away.
    #[must_use]
    #[doc(alias = "getLineNumberAt")]
    pub fn get_line_number_at(&self, code_unit_index: TextIndex) -> Option<usize> {
        if code_unit_index >= self.text.len() {
            return None;
        }
        let mut start_line: usize = 0;
        let mut end_line: usize = self.lines.len().wrapping_sub(1);
        if self.lines.is_empty() || self.lines[end_line].text_with_newlines().end <= code_unit_index
        {
            return None;
        }

        while end_line > start_line {
            // startLine + 1 <= endLine, so we have startLine <= midLine <= endLine - 1.
            let mid_line = (end_line + start_line) / 2;
            let mid_line_range = self.lines[mid_line].text_with_newlines();
            if code_unit_index < mid_line_range.start {
                end_line = mid_line - 1;
            } else if mid_line_range.end <= code_unit_index {
                start_line = mid_line + 1;
            } else {
                return Some(mid_line);
            }
        }
        debug_assert!(start_line == end_line);
        Some(start_line)
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1262-L1269 (chrome/m156)
    /// `getLineNumberAtUTF16Offset(codeUnitIndex)`.
    #[must_use]
    #[doc(alias = "getLineNumberAtUTF16Offset")]
    pub fn get_line_number_at_utf16_offset(&self, code_unit_index: usize) -> Option<usize> {
        self.ensure_utf16_mapping();
        let mapping = self.utf16_mapping();
        if code_unit_index >= mapping.utf8_index_for_utf16_index.len() {
            return None;
        }
        let utf8 = mapping.utf8_index_for_utf16_index[code_unit_index];
        self.get_line_number_at(utf8)
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1271-L1280 (chrome/m156)
    /// `getLineMetricsAt(lineNumber, lineMetrics)`.
    #[must_use]
    #[doc(alias = "getLineMetricsAt")]
    pub fn get_line_metrics_at(&self, line_number: usize) -> Option<LineMetrics<'_>> {
        if line_number >= self.lines.len() {
            return None;
        }
        Some(self.lines[line_number].get_metrics(self))
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1282-L1288 (chrome/m156)
    /// `getActualTextRange(lineNumber, includeSpaces)`.
    #[must_use]
    #[doc(alias = "getActualTextRange")]
    pub fn get_actual_text_range(&self, line_number: usize, include_spaces: bool) -> TextRange {
        if line_number >= self.lines.len() {
            return EMPTY_TEXT;
        }
        let line = &self.lines[line_number];
        if include_spaces {
            line.text()
        } else {
            line.trimmed_text()
        }
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1290-L1312 (chrome/m156)
    /// `getGlyphClusterAt(codeUnitIndex, glyphInfo)`.
    #[must_use]
    #[doc(alias = "getGlyphClusterAt")]
    pub fn get_glyph_cluster_at(&self, code_unit_index: TextIndex) -> Option<GlyphClusterInfo> {
        let line_number = self.get_line_number_at(code_unit_index)?;
        let line = &self.lines[line_number];
        for c in line.clusters_with_spaces() {
            let cluster = &self.clusters[c];
            if cluster.contains(code_unit_index) {
                let mut boxes: Vec<TextBox> = Vec::new();
                line.get_rects_for_range(
                    self,
                    cluster.text_range(),
                    RectHeightStyle::Tight,
                    RectWidthStyle::Tight,
                    &mut boxes,
                );
                if !boxes.is_empty() {
                    return Some(GlyphClusterInfo {
                        bounds: boxes[0].rect,
                        text_range: cluster.text_range(),
                        position: boxes[0].direct,
                    });
                }
            }
        }
        None
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1314-L1326 (chrome/m156)
    /// `getClosestGlyphClusterAt(dx, dy, glyphInfo)`.
    #[must_use]
    #[doc(alias = "getClosestGlyphClusterAt")]
    pub fn get_closest_glyph_cluster_at(&self, d: impl Into<Point>) -> Option<GlyphClusterInfo> {
        let res = self.get_glyph_position_at_coordinate(d);
        debug_assert!(res.position != 0 || res.affinity != Affinity::Upstream);
        let utf16_offset = (res.position as isize
            + if res.affinity == Affinity::Downstream {
                0
            } else {
                -1
            }) as usize;
        self.ensure_utf16_mapping();
        let mapping = self.utf16_mapping();
        debug_assert!(utf16_offset < mapping.utf8_index_for_utf16_index.len());
        self.get_glyph_cluster_at(mapping.utf8_index_for_utf16_index[utf16_offset])
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1328-L1361 (chrome/m156)
    /// `getGlyphInfoAtUTF16Offset(codeUnitIndex, glyphInfo)`: `None` if the offset is out of
    /// bounds or has no line.
    #[must_use]
    #[doc(alias = "getGlyphInfoAtUTF16Offset")]
    pub fn get_glyph_info_at_utf16_offset(&self, code_unit_index: usize) -> Option<GlyphInfo> {
        self.ensure_utf16_mapping();
        let mapping = self.utf16_mapping();
        if code_unit_index >= mapping.utf8_index_for_utf16_index.len() {
            return None;
        }
        let utf8 = mapping.utf8_index_for_utf16_index[code_unit_index];
        let line_number = self.get_line_number_at(utf8)?;
        let line = &self.lines[line_number];
        let start_index = self.find_previous_grapheme_boundary(utf8);
        let end_index = self.find_next_grapheme_boundary(utf8 + 1);
        let glyph_cluster_index = self.cluster_index(utf8);
        let glyph_cluster = self.cluster(glyph_cluster_index);

        // `startIndex` and `endIndex` must be on the same line.
        let mut boxes: Vec<TextBox> = Vec::new();
        line.get_rects_for_range(
            self,
            start_index..end_index,
            RectHeightStyle::Tight,
            RectWidthStyle::Tight,
            &mut boxes,
        );
        // TODO: currently placeholders with height=0 and width=0 are ignored so boxes
        // can be empty. These placeholders should still be reported for their
        // offset information.
        if !boxes.is_empty() {
            return Some(GlyphInfo {
                grapheme_layout_bounds: boxes[0].rect,
                grapheme_cluster_text_range: mapping.utf16_index_for_utf8_index[start_index]
                    ..mapping.utf16_index_for_utf8_index[end_index],
                text_direction: boxes[0].direct,
                is_ellipsis: glyph_cluster.run(&self.runs).is_ellipsis(),
            });
        }
        // The C++ returns true with the info untouched when there are no boxes; there is no
        // info to return here.
        None
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1363-L1369 (chrome/m156)
    /// `getClosestUTF16GlyphInfoAt(dx, dy, glyphInfo)`.
    #[must_use]
    #[doc(alias = "getClosestUTF16GlyphInfoAt")]
    pub fn get_closest_utf16_glyph_info_at(&self, d: impl Into<Point>) -> Option<GlyphInfo> {
        let res = self.get_glyph_position_at_coordinate(d);
        debug_assert!(res.position != 0 || res.affinity != Affinity::Upstream);
        let utf16_offset = (res.position as isize
            + if res.affinity == Affinity::Downstream {
                0
            } else {
                -1
            }) as usize;
        self.get_glyph_info_at_utf16_offset(utf16_offset)
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1371-L1380 (chrome/m156)
    /// `getFontAt(codeUnitIndex)`: the font that is used to shape the text at the position.
    #[must_use]
    #[doc(alias = "getFontAt")]
    pub fn get_font_at(&self, code_unit_index: TextIndex) -> Font {
        for run in &self.runs {
            let text_range = run.text_range();
            if text_range.start <= code_unit_index && code_unit_index < text_range.end {
                return run.font().clone();
            }
        }
        Font::default()
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1382-L1396 (chrome/m156)
    /// `getFontAtUTF16Offset(codeUnitIndex)`.
    #[must_use]
    #[doc(alias = "getFontAtUTF16Offset")]
    pub fn get_font_at_utf16_offset(&self, code_unit_index: usize) -> Font {
        self.ensure_utf16_mapping();
        let mapping = self.utf16_mapping();
        if code_unit_index >= mapping.utf8_index_for_utf16_index.len() {
            return Font::default();
        }
        let utf8 = mapping.utf8_index_for_utf16_index[code_unit_index];
        for run in &self.runs {
            let text_range = run.text_range();
            if text_range.start <= utf8 && utf8 < text_range.end {
                return run.font().clone();
            }
        }
        Font::default()
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1398-L1404 (chrome/m156)
    /// `getFonts()`: the information about all the fonts used to shape the paragraph text.
    #[must_use]
    #[doc(alias = "getFonts")]
    pub fn get_fonts(&self) -> Vec<FontInfo> {
        let mut results = Vec::new();
        for run in &self.runs {
            results.push(FontInfo::new(run.font().clone(), &run.text_range()));
        }
        results
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1419-L1472 (chrome/m156)
    /// `getPath(lineNumber, dest)`: the path of a line, and the number of glyphs that could not
    /// be converted to path.
    #[must_use]
    #[doc(alias = "getPath")]
    pub fn get_path_at(&self, line_number: usize) -> (usize, Path) {
        let mut builder = PathBuilder::new();
        let mut not_converted: usize = 0;
        let line = &self.lines[line_number];
        line.iterate_through_visual_runs(
            self,
            false,
            &mut |run: &Run,
                  run_offset_in_line,
                  text_range: TextRange,
                  run_width_in_line: &mut scalar| {
                *run_width_in_line = line.iterate_through_single_run_by_styles(
                    self,
                    TextAdjustment::GLYPH_CLUSTER,
                    run,
                    run_offset_in_line,
                    text_range,
                    StyleType::None,
                    &mut |_text_range: TextRange, style: &TextStyle, context: &ClipContext<'_>| {
                        let font = run.font();
                        let corrected_baseline = scalar_floor_to_scalar(
                            (f64::from(line.baseline() + style.baseline_shift()) + 0.5) as scalar,
                        );
                        let offset = Point::new(
                            line.offset().x + context.text_shift,
                            line.offset().y + corrected_baseline,
                        );
                        let rect = context.clip.with_offset(offset);
                        let rec_offset = Point::new(rect.left, rect.top);
                        let positions = &run.positions()[context.pos..];
                        let mut pos_index: usize = 0;
                        font.get_paths(
                            &run.glyphs()[context.pos..context.pos + context.size],
                            |path: Option<&Path>, mx: &Matrix| {
                                if let Some(path) = path {
                                    let mut total = mx.clone();
                                    total.post_translate((
                                        positions[pos_index].x + rec_offset.x,
                                        positions[pos_index].y + rec_offset.y,
                                    ));
                                    builder.add_path_with_transform(path, &total, None);
                                } else {
                                    not_converted += 1;
                                }
                                pos_index += 1; // move to the next glyph's position
                            },
                        );
                    },
                );
                true
            },
        );
        (not_converted, builder.detach())
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1474-L1494 (chrome/m156)
    /// `Paragraph::GetPath(textBlob)`: the path of a text blob.
    #[must_use]
    #[doc(alias = "GetPath")]
    pub fn get_path(text_blob: &TextBlob) -> Path {
        let mut builder = PathBuilder::new();
        let mut iter = text_blob.run_iter();
        while !iter.done() {
            let font = iter.font().expect("a run with a font").clone();
            let rec_offset = Point::new(text_blob.bounds().left, text_blob.bounds().top);
            let points = iter.points();
            let mut pos_index: usize = 0;
            font.get_paths(iter.glyphs(), |src: Option<&Path>, mx: &Matrix| {
                if let Some(src) = src {
                    let mut tmp = mx.clone();
                    tmp.post_translate((
                        points[pos_index].x - rec_offset.x,
                        points[pos_index].y - rec_offset.y,
                    ));
                    builder.add_path_with_transform(src, &tmp, None);
                }
                pos_index += 1;
            });
            iter.next();
        }
        builder.detach()
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1496-L1514 (chrome/m156)
    /// `containsEmoji(textBlob)`: checks if a given text blob contains glyph with emoji.
    #[must_use]
    #[doc(alias = "containsEmoji")]
    pub fn contains_emoji(&self, text_blob: &TextBlob) -> bool {
        let mut result = false;
        let mut iter = text_blob.run_iter();
        while !iter.done() && !result {
            // Walk through all the text by codepoints
            crate::one_line_shaper::for_each_codepoint(
                iter.text(),
                |unichar, _start, _end, _count| {
                    if self.unicode.is_emoji(unichar) {
                        result = true;
                    }
                },
            );
            iter.next();
        }
        result
    }

    // Port of: modules/skparagraph/src/ParagraphImpl.cpp#L1516-L1531 (chrome/m156)
    /// `containsColorFontOrBitmap(textBlob)`: checks if a given text blob contains colored font
    /// or bitmap.
    #[must_use]
    #[doc(alias = "containsColorFontOrBitmap")]
    pub fn contains_color_font_or_bitmap(&self, text_blob: &TextBlob) -> bool {
        let mut iter = text_blob.run_iter();
        let mut flag = false;
        while !iter.done() && !flag {
            iter.font().expect("a run with a font").get_paths(
                iter.glyphs(),
                |path: Option<&Path>, _mx: &Matrix| {
                    if path.is_none() {
                        flag = true;
                    }
                },
            );
            iter.next();
        }
        flag
    }
}

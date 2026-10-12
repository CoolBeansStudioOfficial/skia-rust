// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/src/TextWrapper.h,
// modules/skparagraph/src/TextWrapper.cpp
//
// Skia's `TextWrapper` walks the paragraph's clusters with `Cluster*` pointers. They are indices
// here (`isize`): -1 is a null pointer (and the pointer before the first cluster, which C++ forms
// and compares but never dereferences).

//! `TextWrapper`: breaks the clusters of a paragraph into lines.

use skia_rust_core::floating_point::is_finite;
use skia_rust_core::point::Vector;
use skia_rust_core::scalar::{scalar, scalar_floor_to_scalar, scalar_round_to_scalar};

use crate::dart_types::{LineMetricStyle, TextAlign};
use crate::paragraph_impl::Paragraph;
use crate::run::{Cluster, ClusterRange, InternalLineMetrics, Run};
use crate::text_style::{TextRange, nearly_zero_default};

/// `Cluster*` as an index into the paragraph's clusters; -1 is null.
type ClusterPtr = isize;

const NULL_CLUSTER: ClusterPtr = -1;

fn cluster_at(parent: &Paragraph, cluster: ClusterPtr) -> &Cluster {
    &parent.clusters[usize::try_from(cluster).expect("a cluster of the paragraph")]
}

/// The run of a cluster, or `None` for `runOrNull() == nullptr`.
fn run_or_null_of(parent: &Paragraph, cluster: ClusterPtr) -> Option<&Run> {
    cluster_at(parent, cluster).run_or_null(&parent.runs)
}

/// C++ `std::max(a, b)` of floats: `a < b ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

// Port of: modules/skparagraph/src/TextWrapper.cpp#L11-L47 (chrome/m156)
struct LineBreakerWithLittleRounding {
    lower: scalar,
    max_width: scalar,
    upper: scalar,
    apply_rounding_hack: bool,
}

impl LineBreakerWithLittleRounding {
    fn new(max_width: scalar, apply_rounding_hack: bool) -> Self {
        Self {
            lower: max_width - 0.25,
            max_width,
            upper: max_width + 0.25,
            apply_rounding_hack,
        }
    }

    fn break_line(&self, width: scalar) -> bool {
        if width < self.lower {
            return false;
        } else if width > self.upper {
            return true;
        }

        let val = width.abs();
        let rounded_width: scalar;
        if self.apply_rounding_hack {
            if val < 10000.0 {
                rounded_width = scalar_round_to_scalar(width * 100.0) * (1.0 / 100.0);
            } else if val < 100_000.0 {
                rounded_width = scalar_round_to_scalar(width * 10.0) * (1.0 / 10.0);
            } else {
                rounded_width = scalar_floor_to_scalar(width);
            }
        } else if val < 10000.0 {
            rounded_width = scalar_floor_to_scalar(width * 100.0) * (1.0 / 100.0);
        } else if val < 100_000.0 {
            rounded_width = scalar_floor_to_scalar(width * 10.0) * (1.0 / 10.0);
        } else {
            rounded_width = scalar_floor_to_scalar(width);
        }
        rounded_width > self.max_width
    }
}

// Port of: modules/skparagraph/src/TextWrapper.h#L19-L42 (chrome/m156)
#[derive(Debug, Copy, Clone)]
struct ClusterPos {
    cluster: ClusterPtr,
    pos: usize,
}

impl ClusterPos {
    fn new(cluster: ClusterPtr, pos: usize) -> Self {
        Self { cluster, pos }
    }

    fn clean(&mut self) {
        self.cluster = NULL_CLUSTER;
        self.pos = 0;
    }

    fn move_by(&mut self, parent: &Paragraph, up: bool) {
        self.cluster += if up { 1 } else { -1 };
        self.pos = if up {
            0
        } else {
            cluster_at(parent, self.cluster).end_pos()
        };
    }
}

impl Default for ClusterPos {
    fn default() -> Self {
        Self::new(NULL_CLUSTER, 0)
    }
}

// Port of: modules/skparagraph/src/TextWrapper.h#L44-L148 (chrome/m156)
#[derive(Debug, Copy, Clone)]
struct TextStretch {
    start: ClusterPos,
    end: ClusterPos,
    brk: ClusterPos,
    metrics: InternalLineMetrics,
    width: scalar,
    width_with_ghost_spaces: scalar,
}

impl Default for TextStretch {
    fn default() -> Self {
        Self {
            start: ClusterPos::default(),
            end: ClusterPos::default(),
            brk: ClusterPos::default(),
            metrics: InternalLineMetrics::default(),
            width: 0.0,
            width_with_ghost_spaces: 0.0,
        }
    }
}

impl TextStretch {
    // Port of: modules/skparagraph/src/TextWrapper.h#L47-L60 (chrome/m156)
    fn new(parent: &Paragraph, s: ClusterPtr, e: ClusterPtr, force_strut: bool) -> Self {
        let mut stretch = TextStretch {
            start: ClusterPos::new(s, 0),
            end: ClusterPos::new(e, cluster_at(parent, e).end_pos()),
            brk: ClusterPos::default(),
            metrics: InternalLineMetrics::new(force_strut),
            width: 0.0,
            width_with_ghost_spaces: 0.0,
        };
        let mut c = s;
        while c <= e {
            if let Some(r) = run_or_null_of(parent, c) {
                stretch.metrics.add_run(r);
            }
            if c < e {
                stretch.width += cluster_at(parent, c).width();
            }
            c += 1;
        }
        stretch.width_with_ghost_spaces = stretch.width;
        stretch
    }

    fn width(&self) -> scalar {
        self.width
    }

    fn width_with_ghost_spaces(&self) -> scalar {
        self.width_with_ghost_spaces
    }

    fn start_cluster(&self) -> ClusterPtr {
        self.start.cluster
    }

    fn end_cluster(&self) -> ClusterPtr {
        self.end.cluster
    }

    fn break_cluster(&self) -> ClusterPtr {
        self.brk.cluster
    }

    fn start_pos(&self) -> usize {
        self.start.pos
    }

    fn end_pos(&self) -> usize {
        self.end.pos
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L70-L72 (chrome/m156)
    fn end_of_cluster(&self, parent: &Paragraph) -> bool {
        self.end.pos == cluster_at(parent, self.end.cluster).end_pos()
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L73-L76 (chrome/m156)
    fn end_of_word(&self, parent: &Paragraph) -> bool {
        let cluster = cluster_at(parent, self.end.cluster);
        self.end_of_cluster(parent)
            && (cluster.is_hard_break() || cluster.is_soft_break(&parent.code_unit_properties))
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L77-L82 (chrome/m156)
    fn extend_stretch(&mut self, stretch: &mut TextStretch) {
        self.metrics.add(stretch.metrics);
        self.end = stretch.end;
        self.width += stretch.width;
        stretch.clean();
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L84-L85 (chrome/m156)
    fn empty(&self) -> bool {
        self.start.cluster == self.end.cluster && self.start.pos == self.end.pos
    }

    fn set_metrics(&mut self, metrics: InternalLineMetrics) {
        self.metrics = metrics;
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L90-L100 (chrome/m156)
    fn extend_cluster(&mut self, parent: &Paragraph, cluster: ClusterPtr) {
        let c = cluster_at(parent, cluster);
        if self.start.cluster == NULL_CLUSTER {
            self.start = ClusterPos::new(cluster, c.start_pos());
        }
        self.end = ClusterPos::new(cluster, c.end_pos());
        let r = c.run(&parent.runs);
        if !c.is_hard_break() && !r.is_placeholder() {
            self.metrics.add_run(r);
        }
        self.width += c.width();
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L107-L117 (chrome/m156)
    fn start_from(&mut self, parent: &Paragraph, cluster: ClusterPtr, pos: usize) {
        self.start = ClusterPos::new(cluster, pos);
        self.end = ClusterPos::new(cluster, pos);
        if let Some(r) = run_or_null_of(parent, cluster) {
            if !r.is_placeholder() {
                self.metrics.add_run(r);
            }
        }
        self.width = 0.0;
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L119-L122 (chrome/m156)
    fn save_break(&mut self) {
        self.width_with_ghost_spaces = self.width;
        self.brk = self.end;
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L124-L127 (chrome/m156)
    fn restore_break(&mut self) {
        self.width = self.width_with_ghost_spaces;
        self.end = self.brk;
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L129-L131 (chrome/m156)
    fn shift_break(&mut self, parent: &Paragraph) {
        self.brk.move_by(parent, true);
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L133-L141 (chrome/m156)
    fn trim(&mut self, parent: &Paragraph) {
        if self.end.cluster != NULL_CLUSTER {
            let cluster = cluster_at(parent, self.end.cluster);
            if let Some(run) = cluster.run_or_null(&parent.runs) {
                if !run.is_placeholder() && self.width > 0.0 {
                    self.width -=
                        cluster.width() - cluster.trimmed_width(&parent.runs, self.end.pos);
                }
            }
        }
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L143-L153 (chrome/m156)
    fn trim_cluster(&mut self, parent: &Paragraph, cluster: ClusterPtr) {
        debug_assert!(self.end.cluster == cluster);
        if self.end.cluster > self.start.cluster {
            self.end.move_by(parent, false);
            self.width -= cluster_at(parent, cluster).width();
        } else {
            self.end.pos = self.start.pos;
            self.width = 0.0;
        }
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L155-L160 (chrome/m156)
    fn clean(&mut self) {
        self.start.clean();
        self.end.clean();
        self.width = 0.0;
        self.metrics.clean();
    }
}

/// The arguments of the `AddLineToParagraph` callback of `TextWrapper::breakTextIntoLines`.
// Port of: modules/skparagraph/src/TextWrapper.h#L170-L182 (chrome/m156)
#[derive(Debug, Clone)]
pub struct AddLineArgs {
    /// `textExcludingSpaces`.
    pub text_excluding_spaces: TextRange,
    /// `text`.
    pub text: TextRange,
    /// `textIncludingNewlines`.
    pub text_including_newlines: TextRange,
    /// `clusters`.
    pub clusters: ClusterRange,
    /// `clustersWithGhosts`.
    pub clusters_with_ghosts: ClusterRange,
    /// `widthWithSpaces`.
    pub width_with_spaces: scalar,
    /// `startClip`.
    pub start_clip: usize,
    /// `endClip`.
    pub end_clip: usize,
    /// `offset`.
    pub offset: Vector,
    /// `advance`.
    pub advance: Vector,
    /// `metrics`.
    pub metrics: InternalLineMetrics,
    /// `addEllipsis`.
    pub add_ellipsis: bool,
}

// Port of: modules/skparagraph/src/TextWrapper.h#L162-L216 (chrome/m156)
/// `TextWrapper`: breaks the clusters of a paragraph into lines.
#[derive(Debug)]
pub struct TextWrapper {
    words: TextStretch,
    clusters: TextStretch,
    clip: TextStretch,
    end_line: TextStretch,
    line_number: usize,
    too_long_word: bool,
    too_long_cluster: bool,
    hard_line_break: bool,
    exceeded_max_lines: bool,
    height: scalar,
    min_intrinsic_width: scalar,
    max_intrinsic_width: scalar,
}

impl Default for TextWrapper {
    // Port of: modules/skparagraph/src/TextWrapper.h#L164-L168 (chrome/m156)
    fn default() -> Self {
        Self {
            words: TextStretch::default(),
            clusters: TextStretch::default(),
            clip: TextStretch::default(),
            end_line: TextStretch::default(),
            line_number: 1,
            too_long_word: false,
            too_long_cluster: false,
            hard_line_break: false,
            exceeded_max_lines: false,
            height: 0.0,
            min_intrinsic_width: 0.0,
            max_intrinsic_width: 0.0,
        }
    }
}

impl TextWrapper {
    /// `TextWrapper()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `height()`.
    #[must_use]
    pub fn height(&self) -> scalar {
        self.height
    }

    /// `minIntrinsicWidth()`.
    #[must_use]
    #[doc(alias = "minIntrinsicWidth")]
    pub fn min_intrinsic_width(&self) -> scalar {
        self.min_intrinsic_width
    }

    /// `maxIntrinsicWidth()`.
    #[must_use]
    #[doc(alias = "maxIntrinsicWidth")]
    pub fn max_intrinsic_width(&self) -> scalar {
        self.max_intrinsic_width
    }

    /// `exceededMaxLines()`.
    #[must_use]
    #[doc(alias = "exceededMaxLines")]
    pub fn exceeded_max_lines(&self) -> bool {
        self.exceeded_max_lines
    }

    // Port of: modules/skparagraph/src/TextWrapper.h#L190-L197 (chrome/m156)
    fn reset(&mut self) {
        self.words.clean();
        self.clusters.clean();
        self.clip.clean();
        self.too_long_cluster = false;
        self.too_long_word = false;
        self.hard_line_break = false;
    }

    // Port of: modules/skparagraph/src/TextWrapper.cpp#L49-L190 (chrome/m156)
    /// Since we allow cluster clipping when they don't fit we have to work with stretches - parts
    /// of clusters.
    fn look_ahead(
        &mut self,
        parent: &Paragraph,
        max_width: scalar,
        end_of_clusters: ClusterPtr,
        apply_rounding_hack: bool,
    ) {
        self.reset();
        self.end_line.metrics.clean();
        let start_cluster = self.end_line.start_cluster();
        let start_pos = self.end_line.start_pos();
        self.words.start_from(parent, start_cluster, start_pos);
        self.clusters.start_from(parent, start_cluster, start_pos);
        self.clip.start_from(parent, start_cluster, start_pos);

        let breaker = LineBreakerWithLittleRounding::new(max_width, apply_rounding_hack);
        let mut next_non_breaking_space: ClusterPtr = NULL_CLUSTER;
        let mut cluster = self.end_line.end_cluster();
        while cluster < end_of_clusters {
            let c = cluster_at(parent, cluster);
            if c.is_hard_break() {
            } else {
                // TODO: Trying to deal with flutter rounding problem. Must be removed...
                let width = self.words.width() + self.clusters.width() + c.width();
                if breaker.break_line(width) {
                    if c.is_whitespace_break() {
                        // It's the end of the word
                        self.clusters.extend_cluster(parent, cluster);
                        self.min_intrinsic_width = std_max(
                            self.min_intrinsic_width,
                            self.get_clusters_trimmed_width(parent),
                        );
                        self.words.extend_stretch(&mut self.clusters);
                        cluster += 1;
                        continue;
                    } else if c.run(&parent.runs).is_placeholder() {
                        if !self.clusters.empty() {
                            // Placeholder ends the previous word
                            self.min_intrinsic_width = std_max(
                                self.min_intrinsic_width,
                                self.get_clusters_trimmed_width(parent),
                            );
                            self.words.extend_stretch(&mut self.clusters);
                        }

                        if c.width() > max_width && self.words.empty() {
                            // Placeholder is the only text and it's longer than the line;
                            // it does not count in fMinIntrinsicWidth
                            self.clusters.extend_cluster(parent, cluster);
                            self.too_long_cluster = true;
                            self.too_long_word = true;
                        } else {
                            // Placeholder does not fit the line; it will be considered again on the next line
                        }
                        break;
                    }

                    // Walk further to see if there is a too long word, cluster or glyph
                    let mut next_word_length = self.clusters.width();
                    let mut next_short_word_length = next_word_length;
                    let mut further = cluster;
                    while further != end_of_clusters {
                        let f = cluster_at(parent, further);
                        if f.is_soft_break(&parent.code_unit_properties)
                            || f.is_hard_break()
                            || f.is_whitespace_break()
                        {
                            break;
                        }
                        if f.run(&parent.runs).is_placeholder() {
                            // Placeholder ends the word
                            break;
                        }

                        if next_word_length > 0.0
                            && next_word_length <= max_width
                            && f.is_intra_word_break()
                        {
                            // The cluster is spaces but not the end of the word in a normal sense
                            next_non_breaking_space = further;
                            next_short_word_length = next_word_length;
                        }

                        #[allow(clippy::float_cmp)] // mirrors `maxWidth == 0`
                        if max_width == 0.0 {
                            // This is a tricky flutter case: layout(width:0) places 1 cluster on each line
                            next_word_length = std_max(next_word_length, f.width());
                        } else {
                            next_word_length += f.width();
                        }
                        further += 1;
                    }
                    if next_word_length > max_width {
                        if next_non_breaking_space != NULL_CLUSTER {
                            // We only get here if the non-breaking space improves our situation
                            // (allows us to break the text to fit the word)
                            let short_length = self.words.width() + next_short_word_length;
                            if !breaker.break_line(short_length) {
                                // We can add the short word to the existing line
                                self.clusters = TextStretch::new(
                                    parent,
                                    self.clusters.start_cluster(),
                                    next_non_breaking_space,
                                    self.clusters.metrics.force_strut(),
                                );
                                self.min_intrinsic_width =
                                    std_max(self.min_intrinsic_width, next_short_word_length);
                                self.words.extend_stretch(&mut self.clusters);
                            } else {
                                // We can place the short word on the next line
                                self.clusters.clean();
                            }
                            // Either way we are not in "word is too long" situation anymore
                            break;
                        }
                        // If the word is too long we can break it right now and hope it's enough
                        self.min_intrinsic_width =
                            std_max(self.min_intrinsic_width, next_word_length);
                        if self
                            .clusters
                            .end_pos()
                            .wrapping_sub(self.clusters.start_pos())
                            > 1
                            || self.words.empty()
                        {
                            self.too_long_word = true;
                        } else {
                            // Even if the word is too long there is a very little space on this line.
                            // let's deal with it on the next line.
                        }
                    }
                    if c.width() > max_width {
                        self.clusters.extend_cluster(parent, cluster);
                        self.too_long_cluster = true;
                        self.too_long_word = true;
                    }
                    break;
                }
            }

            if c.run(&parent.runs).is_placeholder() {
                if !self.clusters.empty() {
                    // Placeholder ends the previous word (placeholders are ignored in trimming)
                    self.min_intrinsic_width = std_max(
                        self.min_intrinsic_width,
                        self.get_clusters_trimmed_width(parent),
                    );
                    self.words.extend_stretch(&mut self.clusters);
                }

                // Placeholder is separate word and its width now is counted in minIntrinsicWidth
                self.min_intrinsic_width = std_max(self.min_intrinsic_width, c.width());
                self.words.extend_cluster(parent, cluster);
            } else {
                self.clusters.extend_cluster(parent, cluster);

                // Keep adding clusters/words
                if self.clusters.end_of_word(parent) {
                    self.min_intrinsic_width = std_max(
                        self.min_intrinsic_width,
                        self.get_clusters_trimmed_width(parent),
                    );
                    self.words.extend_stretch(&mut self.clusters);
                }
            }

            self.hard_line_break = c.is_hard_break();
            if self.hard_line_break {
                // Stop at the hard line break
                break;
            }
            cluster += 1;
        }
    }

    // Port of: modules/skparagraph/src/TextWrapper.cpp#L192-L213 (chrome/m156)
    fn move_forward(&mut self, has_ellipsis: bool) {
        // We normally break lines by words.
        // The only way we may go to clusters is if the word is too long or
        // it's the first word and it has an ellipsis attached to it.
        // If nothing fits we show the clipping.
        if !self.words.empty() {
            self.end_line.extend_stretch(&mut self.words);
            // `SK_IGNORE_SKPARAGRAPH_ELLIPSIS_FIX` is not defined: the ellipsis is added to a
            // grapheme.
            if !self.too_long_word && !has_ellipsis {
                return;
            }
        }
        if !self.clusters.empty() {
            self.end_line.extend_stretch(&mut self.clusters);
            if !self.too_long_cluster {
                return;
            }
        }

        if !self.clip.empty() {
            // Flutter: forget the clipped cluster but keep the metrics
            self.end_line.metrics.add(self.clip.metrics);
        }
    }

    // Port of: modules/skparagraph/src/TextWrapper.cpp#L215-L227 (chrome/m156)
    /// Special case for start/end cluster since they can be clipped.
    fn trim_end_spaces(&mut self, parent: &Paragraph, _align: TextAlign) {
        // Remember the breaking position
        self.end_line.save_break();
        // Skip all space cluster at the end
        let mut cluster = self.end_line.end_cluster();
        while cluster >= self.end_line.start_cluster()
            && cluster_at(parent, cluster).is_whitespace_break()
        {
            self.end_line.trim_cluster(parent, cluster);
            cluster -= 1;
        }
        self.end_line.trim(parent);
    }

    // Port of: modules/skparagraph/src/TextWrapper.cpp#L229-L248 (chrome/m156)
    fn get_clusters_trimmed_width(&self, parent: &Paragraph) -> scalar {
        // Move the end of the line to the left
        let mut width: scalar = 0.0;
        let mut trailing_spaces = true;
        let mut cluster = self.clusters.end_cluster();
        while cluster >= self.clusters.start_cluster() {
            let c = cluster_at(parent, cluster);
            if c.run(&parent.runs).is_placeholder() {
                cluster -= 1;
                continue;
            }
            if trailing_spaces {
                if !c.is_whitespace_break() {
                    width += c.trimmed_width(&parent.runs, c.end_pos());
                    trailing_spaces = false;
                }
                cluster -= 1;
                continue;
            }
            width += c.width();
            cluster -= 1;
        }
        width
    }

    // Port of: modules/skparagraph/src/TextWrapper.cpp#L250-L282 (chrome/m156)
    /// Trim the beginning spaces in case of soft line break.
    fn trim_start_spaces(
        &mut self,
        parent: &Paragraph,
        end_of_clusters: ClusterPtr,
    ) -> (ClusterPtr, usize, scalar) {
        if self.hard_line_break {
            // End of line is always end of cluster, but need to skip \n
            let mut width = self.end_line.width();
            let mut cluster = self.end_line.end_cluster() + 1;
            while cluster < self.end_line.break_cluster()
                && cluster_at(parent, cluster).is_whitespace_break()
            {
                width += cluster_at(parent, cluster).width();
                cluster += 1;
            }
            return (self.end_line.break_cluster() + 1, 0, width);
        }

        // breakCluster points to the end of the line;
        // It's a soft line break so we need to move lineStart forward skipping all the spaces
        let mut width = self.end_line.width_with_ghost_spaces();
        let mut cluster = self.end_line.break_cluster() + 1;
        while cluster < end_of_clusters && cluster_at(parent, cluster).is_whitespace_break() {
            width += cluster_at(parent, cluster).width();
            cluster += 1;
        }

        if cluster_at(parent, self.end_line.break_cluster()).is_whitespace_break()
            && self.end_line.break_cluster() < end_of_clusters
        {
            // In case of a soft line break by the whitespace
            // fBreak should point to the beginning of the next line
            // (it only matters when there are trailing spaces)
            self.end_line.shift_break(parent);
        }

        (cluster, 0, width)
    }

    // Port of: modules/skparagraph/src/TextWrapper.cpp#L284-L517 (chrome/m156)
    /// `breakTextIntoLines(parent, maxWidth, addLine)`. TODO: refactor the code for line ending
    /// (with/without ellipsis).
    #[doc(alias = "breakTextIntoLines")]
    #[allow(clippy::too_many_lines)] // one function in C++
    pub fn break_text_into_lines(
        &mut self,
        parent: &mut Paragraph,
        max_width: scalar,
        add_line: &mut dyn FnMut(&mut Paragraph, AddLineArgs),
    ) {
        self.height = 0.0;
        self.min_intrinsic_width = f32::MIN_POSITIVE;
        self.max_intrinsic_width = f32::MIN_POSITIVE;

        let span_len = parent.clusters.len();
        if span_len == 0 {
            return;
        }

        let max_lines = parent.paragraph_style().max_lines_raw();
        let align = parent.paragraph_style().effective_align();
        let unlimited_lines = max_lines == usize::MAX;
        let endless_line = !is_finite(max_width);
        let has_ellipsis = parent.paragraph_style().ellipsized();

        let disable_first_ascent = parent
            .paragraph_style()
            .text_height_behavior()
            .disable_first_ascent();
        let disable_last_descent = parent
            .paragraph_style()
            .text_height_behavior()
            .disable_last_descent();
        let mut first_line = true; // We only interested in fist line if we have to disable the first ascent

        let mut soft_line_max_intrinsic_width: scalar = 0.0;

        self.end_line = TextStretch::new(parent, 0, 0, parent.strut_force_height());
        let start: ClusterPtr = 0;
        let end: ClusterPtr = span_len as isize - 1;
        let mut max_run_metrics = InternalLineMetrics::default();
        let mut need_ellipsis = false;
        while self.end_line.end_cluster() != end {
            self.look_ahead(parent, max_width, end, parent.get_apply_rounding_hack());

            let last_line = (has_ellipsis && unlimited_lines) || self.line_number >= max_lines;
            need_ellipsis = has_ellipsis && !endless_line && last_line;

            self.move_forward(need_ellipsis);
            need_ellipsis &= self.end_line.end_cluster() < end - 1; // Only if we have some text to ellipsize

            // Do not trim end spaces on the naturally last line of the left aligned text
            self.trim_end_spaces(parent, align);

            // For soft line breaks add to the line all the spaces next to it
            let (start_line, pos, mut width_with_spaces) = self.trim_start_spaces(parent, end);

            if need_ellipsis && !self.hard_line_break {
                // This is what we need to do to preserve a space before the ellipsis
                self.end_line.restore_break();
                width_with_spaces = self.end_line.width_with_ghost_spaces();
            }

            // If the line is empty with the hard line break, let's take the paragraph font (flutter???)
            if self.end_line.metrics.is_clean() {
                self.end_line.set_metrics(parent.get_empty_metrics());
            }

            // Deal with placeholder clusters == runs[@size==1]
            let mut last_run: Option<usize> = None;
            let mut cluster = self.end_line.start_cluster();
            while cluster <= self.end_line.end_cluster() {
                let r = cluster_at(parent, cluster).run_index();
                let r = if r < parent.runs.len() { Some(r) } else { None };
                if r == last_run {
                    cluster += 1;
                    continue;
                }
                last_run = r;
                let run_index = last_run.expect("a run");
                if parent.runs[run_index].is_placeholder() {
                    debug_assert!(parent.runs[run_index].size() == 1);
                    // Update the placeholder metrics so we can get the placeholder positions later
                    // and the line metrics (to make sure the placeholder fits)
                    let placeholder_style =
                        parent.placeholders[parent.runs[run_index].placeholder_index].style;
                    parent.runs[run_index]
                        .update_metrics(&placeholder_style, &mut self.end_line.metrics);
                }
                cluster += 1;
            }

            // Before we update the line metrics with struts,
            // let's save it for GetRectsForRange(RectHeightStyle::kMax)
            max_run_metrics = self.end_line.metrics;
            max_run_metrics.force_strut = false;

            // TODO: keep start/end/break info for text and runs but in a better way that below
            let start_cluster = cluster_at(parent, self.end_line.start_cluster());
            let end_cluster = cluster_at(parent, self.end_line.end_cluster());
            let text_excluding_spaces: TextRange =
                start_cluster.text_range().start..end_cluster.text_range().end;
            let mut text: TextRange = start_cluster.text_range().start
                ..cluster_at(parent, self.end_line.break_cluster())
                    .text_range()
                    .start;
            let mut text_including_newlines: TextRange =
                start_cluster.text_range().start..cluster_at(parent, start_line).text_range().start;
            if start_line == end {
                text_including_newlines.end = parent.text().len();
                text.end = parent.text().len();
            }
            let mut clusters: ClusterRange = (self.end_line.start_cluster() - start) as usize
                ..((self.end_line.end_cluster() - start) as usize + 1);
            let clusters_with_ghosts: ClusterRange =
                (self.end_line.start_cluster() - start) as usize..(start_line - start) as usize;

            if disable_first_ascent && first_line {
                self.end_line.metrics.ascent = self.end_line.metrics.raw_ascent;
            }
            if disable_last_descent && (last_line || (start_line == end && !self.hard_line_break)) {
                self.end_line.metrics.descent = self.end_line.metrics.raw_descent;
            }

            if parent.strut_enabled() {
                // Make sure font metrics are not less than the strut
                parent
                    .strut_metrics()
                    .update_line_metrics(&mut self.end_line.metrics);
            }

            let line_height: scalar = self.end_line.metrics.height();
            first_line = false;

            let mut text_excluding_spaces = text_excluding_spaces;
            if self.end_line.empty() {
                // Correct text and clusters (make it empty for an empty line)
                text_excluding_spaces.end = text_excluding_spaces.start;
                clusters.end = clusters.start;
            }

            // In case of a force wrapping we don't have a break cluster and have to use the end cluster
            text.end = text.end.max(text_excluding_spaces.end);

            add_line(
                parent,
                AddLineArgs {
                    text_excluding_spaces,
                    text,
                    text_including_newlines,
                    clusters,
                    clusters_with_ghosts,
                    width_with_spaces,
                    start_clip: self.end_line.start_pos(),
                    end_clip: self.end_line.end_pos(),
                    offset: Vector::new(0.0, self.height),
                    advance: Vector::new(self.end_line.width(), line_height),
                    metrics: self.end_line.metrics,
                    add_ellipsis: need_ellipsis && !self.hard_line_break,
                },
            );

            soft_line_max_intrinsic_width += width_with_spaces;

            self.max_intrinsic_width =
                std_max(self.max_intrinsic_width, soft_line_max_intrinsic_width);
            if self.hard_line_break {
                soft_line_max_intrinsic_width = 0.0;
            }
            // Start a new line
            self.height += line_height;
            if !self.hard_line_break || start_line != end {
                self.end_line.clean();
            }
            self.end_line.start_from(parent, start_line, pos);
            parent.max_width_with_trailing_spaces =
                std_max(parent.max_width_with_trailing_spaces, width_with_spaces);

            if has_ellipsis && unlimited_lines {
                // There is one case when we need an ellipsis on a separate line
                // after a line break when width is infinite
                if !self.hard_line_break {
                    break;
                }
            } else if last_line {
                // There is nothing more to draw
                self.hard_line_break = false;
                break;
            }

            self.line_number += 1;
        }

        // We finished formatting the text but we need to scan the rest for some numbers
        // TODO: make it a case of a normal flow
        if self.end_line.end_cluster() != NULL_CLUSTER {
            let mut last_word_length: scalar = 0.0;
            let mut cluster = self.end_line.end_cluster();
            while cluster != end
                || cluster_at(parent, cluster).end_pos() < cluster_at(parent, end).end_pos()
            {
                self.exceeded_max_lines = true;
                let c = cluster_at(parent, cluster);
                if c.is_hard_break() {
                    // Hard line break ends the word and the line
                    self.max_intrinsic_width =
                        std_max(self.max_intrinsic_width, soft_line_max_intrinsic_width);
                    soft_line_max_intrinsic_width = 0.0;
                    self.min_intrinsic_width = std_max(self.min_intrinsic_width, last_word_length);
                    last_word_length = 0.0;
                } else if c.is_whitespace_break() {
                    // Whitespaces end the word
                    soft_line_max_intrinsic_width += c.width();
                    self.min_intrinsic_width = std_max(self.min_intrinsic_width, last_word_length);
                    last_word_length = 0.0;
                } else if c.run(&parent.runs).is_placeholder() {
                    // Placeholder ends the previous word and creates a separate one
                    self.min_intrinsic_width = std_max(self.min_intrinsic_width, last_word_length);
                    // Placeholder width now counts in fMinIntrinsicWidth
                    soft_line_max_intrinsic_width += c.width();
                    self.min_intrinsic_width = std_max(self.min_intrinsic_width, c.width());
                    last_word_length = 0.0;
                } else {
                    // Nothing out of ordinary - just add this cluster to the word and to the line
                    soft_line_max_intrinsic_width += c.width();
                    last_word_length += c.width();
                }
                cluster += 1;
            }
            self.min_intrinsic_width = std_max(self.min_intrinsic_width, last_word_length);
            self.max_intrinsic_width =
                std_max(self.max_intrinsic_width, soft_line_max_intrinsic_width);

            if parent.lines.is_empty() {
                // In case we could not place even a single cluster on the line
                if disable_first_ascent {
                    self.end_line.metrics.ascent = self.end_line.metrics.raw_ascent;
                }
                if disable_last_descent && !self.hard_line_break {
                    self.end_line.metrics.descent = self.end_line.metrics.raw_descent;
                }
                self.height = std_max(self.height, self.end_line.metrics.height());
            }
        }

        if self.hard_line_break {
            if disable_last_descent {
                self.end_line.metrics.descent = self.end_line.metrics.raw_descent;
            }

            // Last character is a line break
            if parent.strut_enabled() {
                // Make sure font metrics are not less than the strut
                parent
                    .strut_metrics()
                    .update_line_metrics(&mut self.end_line.metrics);
            }

            let clusters: ClusterRange = (self.end_line.break_cluster() - start) as usize
                ..(self.end_line.end_cluster() - start) as usize;
            add_line(
                parent,
                AddLineArgs {
                    text_excluding_spaces: cluster_at(parent, self.end_line.break_cluster())
                        .text_range(),
                    text: cluster_at(parent, self.end_line.break_cluster()).text_range(),
                    text_including_newlines: cluster_at(parent, self.end_line.end_cluster())
                        .text_range(),
                    clusters: clusters.clone(),
                    clusters_with_ghosts: clusters,
                    width_with_spaces: 0.0,
                    start_clip: 0,
                    end_clip: 0,
                    offset: Vector::new(0.0, self.height),
                    advance: Vector::new(0.0, self.end_line.metrics.height()),
                    metrics: self.end_line.metrics,
                    add_ellipsis: need_ellipsis,
                },
            );
            self.height += self.end_line.metrics.height();
            parent
                .lines
                .last_mut()
                .expect("the line just added")
                .set_max_run_metrics(max_run_metrics);
        }

        if parent.lines.is_empty() {
            return;
        }

        // Correct line metric styles for the first and for the last lines if needed
        if disable_first_ascent {
            parent
                .lines
                .first_mut()
                .expect("a line")
                .set_ascent_style(LineMetricStyle::Typographic);
        }
        if disable_last_descent {
            parent
                .lines
                .last_mut()
                .expect("a line")
                .set_descent_style(LineMetricStyle::Typographic);
        }
        let _ = nearly_zero_default;
    }
}

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/src/OneLineShaper.h,
// modules/skparagraph/src/OneLineShaper.cpp
//
// `RunBlock` shares its `Run` with `std::shared_ptr`. The runs the shaper makes live in one
// vector here and the blocks refer to them by index.

//! `OneLineShaper`: shapes the text of a paragraph as one endless line, resolving the fonts.

use std::collections::{HashSet, VecDeque};

use skia_rust_core::font::Font;
use skia_rust_core::font_style::{FontStyle, Slant};
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::scalar::scalar;
use skia_rust_core::typeface::{Typeface, TypefaceId};
use skia_rust_core::utf::{Unichar, next_utf8_with_replacement};
use skia_rust_shaper::harfbuzz;
use skia_rust_shaper::run_handler::{Buffer, RunHandler, RunInfo};
use skia_rust_shaper::run_iterator::{TrivialBiDiRunIterator, TrivialFontRunIterator};
use skia_rust_shaper::{Feature, Options};
use skia_rust_unicode::unicode::{CodeUnitFlags, Unicode, for_each_codepoint_utf8};

use crate::dart_types::{EMPTY_INDEX, EMPTY_RANGE, RangeExtensions};
use crate::font_arguments::FontArguments;
use crate::iterators::LangIterator;
use crate::paragraph_impl::{Paragraph, ResolvedFontDescriptor};
use crate::run::{ClusterIndex, GlyphIndex, GlyphRange, Run};
use crate::text_style::{Block, EMPTY_TEXT, StyleType, TextRange, TextStyle};

// Port of: modules/skparagraph/src/OneLineShaper.h#L38-L59 (chrome/m156)
#[derive(Debug, Clone)]
struct RunBlock {
    /// `fRun`: an index in the shaper's runs.
    run: Option<usize>,
    text: TextRange,
    glyphs: GlyphRange,
}

impl RunBlock {
    fn new(text: TextRange) -> Self {
        Self {
            run: None,
            text,
            glyphs: 0..0,
        }
    }

    fn with_run(run: usize, text: TextRange, glyphs: GlyphRange) -> Self {
        Self {
            run: Some(run),
            text,
            glyphs,
        }
    }

    fn is_fully_resolved(&self, runs: &[Run]) -> bool {
        match self.run {
            Some(run) => self.glyphs.width() == runs[run].size(),
            None => false,
        }
    }
}

// Port of: modules/skparagraph/src/OneLineShaper.h#L70-L74 (chrome/m156)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Resolved {
    Nothing,
    Something,
    Everything,
}

// Port of: modules/skparagraph/src/OneLineShaper.h#L131-L149 (chrome/m156)
#[derive(Debug, Clone, PartialEq)]
struct FontKey {
    unicode: Unichar,
    font_style: FontStyle,
    locale: String,
    font_args: Option<FontArguments>,
}

// Port of: modules/skparagraph/src/OneLineShaper.h#L24-L153 (chrome/m156)
/// `OneLineShaper`: shapes the text of a paragraph into runs, with the fonts resolved.
pub struct OneLineShaper<'a> {
    paragraph: &'a mut Paragraph,
    current_text: TextRange,
    height: scalar,
    use_half_leading: bool,
    baseline_shift: scalar,
    advance: Vector,
    unresolved_glyphs: usize,
    unique_run_id: usize,
    /// The runs of the blocks (`std::shared_ptr<Run>`).
    runs: Vec<Run>,
    current_run: usize,
    unresolved_blocks: VecDeque<RunBlock>,
    resolved_blocks: Vec<RunBlock>,
    fallback_fonts: Vec<(FontKey, Typeface)>,
}

impl std::fmt::Debug for OneLineShaper<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneLineShaper").finish_non_exhaustive()
    }
}

impl RunHandler for OneLineShaper<'_> {
    // Port of: modules/skparagraph/src/OneLineShaper.h#L88-L90 (chrome/m156)
    fn begin_line(&mut self) {}

    fn run_info(&mut self, _info: &RunInfo<'_>) {}

    fn commit_run_info(&mut self) {}

    fn commit_line(&mut self) {}

    // Port of: modules/skparagraph/src/OneLineShaper.h#L92-L102 (chrome/m156)
    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
        self.unique_run_id += 1;
        let run = Run::new(
            info,
            self.current_text.start,
            self.height,
            self.use_half_leading,
            self.baseline_shift,
            self.unique_run_id,
            self.advance.x,
        );
        self.runs.push(run);
        self.current_run = self.runs.len() - 1;
        let run = &mut self.runs[self.current_run];
        Buffer {
            glyphs: run.glyphs.as_mut_slice(),
            positions: run.positions.as_mut_slice(),
            offsets: Some(run.offsets.as_mut_slice()),
            clusters: Some(run.cluster_indexes.as_mut_slice()),
            point: run.offset,
        }
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L16-L58 (chrome/m156)
    fn commit_run_buffer(&mut self, _info: &RunInfo<'_>) {
        self.runs[self.current_run].commit();

        let old_unresolved_count = self.unresolved_blocks.len();

        // Find all unresolved blocks
        for block in self.sort_out_glyphs() {
            if block.width() == 0 {
                continue;
            }
            self.add_unresolved_with_run(block);
        }

        // Fill all the gaps between unresolved blocks with resolved ones
        if old_unresolved_count == self.unresolved_blocks.len() {
            // No unresolved blocks added - we resolved the block with one run entirely
            self.add_fully_resolved();
            return;
        } else if old_unresolved_count == self.unresolved_blocks.len() - 1 {
            let current_text_range = self.runs[self.current_run].text_range();
            let front_run = self.unresolved_blocks.front().expect("a block").run;
            let front_glyphs = self
                .unresolved_blocks
                .front()
                .expect("a block")
                .glyphs
                .clone();
            let unresolved = self.unresolved_blocks.back_mut().expect("a block");
            if current_text_range == unresolved.text {
                // Nothing was resolved; preserve the initial run if it makes sense
                if front_run.is_some() {
                    unresolved.run = front_run;
                    unresolved.glyphs = front_glyphs;
                }
                return;
            }
        }

        self.fill_gaps(old_unresolved_count);
    }
}

impl<'a> OneLineShaper<'a> {
    // Port of: modules/skparagraph/src/OneLineShaper.h#L27-L33 (chrome/m156)
    /// `OneLineShaper(paragraph)`.
    #[must_use]
    pub fn new(paragraph: &'a mut Paragraph) -> Self {
        let unique_run_id = paragraph.runs.len();
        Self {
            paragraph,
            current_text: 0..0,
            height: 0.0,
            use_half_leading: false,
            baseline_shift: 0.0,
            advance: Vector::new(0.0, 0.0),
            unresolved_glyphs: 0,
            unique_run_id,
            runs: Vec::new(),
            current_run: 0,
            unresolved_blocks: VecDeque::new(),
            resolved_blocks: Vec::new(),
            fallback_fonts: Vec::new(),
        }
    }

    /// `unresolvedGlyphs()`.
    #[must_use]
    #[doc(alias = "unresolvedGlyphs")]
    pub fn unresolved_glyphs(&self) -> usize {
        self.unresolved_glyphs
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L82-L152 (chrome/m156)
    fn fill_gaps(&mut self, starting_count: usize) {
        // Fill out gaps between all unresolved blocks
        let current_run = self.current_run;
        let current_left_to_right = self.runs[current_run].left_to_right();
        let current_size = self.runs[current_run].size();
        let mut resolved_text_limits = self.runs[current_run].text_range();
        if !current_left_to_right {
            std::mem::swap(
                &mut resolved_text_limits.start,
                &mut resolved_text_limits.end,
            );
        }
        let mut resolved_text_start = resolved_text_limits.start;
        let mut resolved_glyphs_start: GlyphIndex = 0;

        let mut prev_text: TextRange = EMPTY_TEXT;
        // Skip the old ones, do the new ones
        for index in starting_count..self.unresolved_blocks.len() {
            if self.unresolved_blocks[index].text == prev_text {
                // Clean up repetitive blocks that appear inside the same grapheme block
                self.unresolved_blocks[index].text = EMPTY_TEXT;
                continue;
            }
            prev_text = self.unresolved_blocks[index].text.clone();

            let unresolved_text = self.unresolved_blocks[index].text.clone();
            let unresolved_glyphs = self.unresolved_blocks[index].glyphs.clone();
            let mut resolved_text: TextRange = resolved_text_start..(if current_left_to_right {
                unresolved_text.start
            } else {
                unresolved_text.end
            });
            if resolved_text.width() > 0 {
                if !current_left_to_right {
                    std::mem::swap(&mut resolved_text.start, &mut resolved_text.end);
                }

                let resolved_glyphs: GlyphRange = resolved_glyphs_start..unresolved_glyphs.start;
                let resolved =
                    RunBlock::with_run(current_run, resolved_text, resolved_glyphs.clone());

                if resolved_glyphs.width() == 0 {
                    // Extend the unresolved block with an empty resolved
                    let unresolved = &mut self.unresolved_blocks[index];
                    if unresolved.text.end <= resolved.text.start {
                        unresolved.text.end = resolved.text.end;
                    }
                    if unresolved.text.start >= resolved.text.end {
                        unresolved.text.start = resolved.text.start;
                    }
                } else {
                    self.resolved_blocks.push(resolved);
                }
            }
            let unresolved = &self.unresolved_blocks[index];
            resolved_glyphs_start = unresolved.glyphs.end;
            resolved_text_start = if current_left_to_right {
                unresolved.text.end
            } else {
                unresolved.text.start
            };
        }

        let mut resolved_text: TextRange = resolved_text_start..resolved_text_limits.end;
        if resolved_text.width() > 0 {
            if !current_left_to_right {
                std::mem::swap(&mut resolved_text.start, &mut resolved_text.end);
            }

            let resolved_glyphs: GlyphRange = resolved_glyphs_start..current_size;
            let resolved = RunBlock::with_run(current_run, resolved_text, resolved_glyphs);
            self.resolved_blocks.push(resolved);
        }
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L154-L238 (chrome/m156)
    fn finish(&mut self, block: &Block, height: scalar, advance_x: &mut scalar) {
        let block_text = block.range.clone();

        // Add all unresolved blocks to resolved blocks
        while let Some(unresolved) = self.unresolved_blocks.pop_front() {
            if unresolved.text.width() == 0 {
                continue;
            }
            self.unresolved_glyphs += unresolved.glyphs.width();
            self.paragraph
                .add_unresolved_codepoints(unresolved.text.clone());
            self.resolved_blocks.push(unresolved);
        }

        // Sort all pieces by text
        self.resolved_blocks.sort_by_key(|block| block.text.start);

        // Go through all of them
        let mut last_text_end = block_text.start;
        for block_index in 0..self.resolved_blocks.len() {
            let resolved_block = self.resolved_blocks[block_index].clone();
            if resolved_block.text.end <= block_text.start {
                continue;
            }

            if let Some(run) = resolved_block.run {
                self.paragraph
                    .font_switches
                    .push(ResolvedFontDescriptor::new(
                        resolved_block.text.start,
                        self.runs[run].font.clone(),
                    ));
            }

            let run = resolved_block.run;
            let glyphs = resolved_block.glyphs.clone();
            let text = resolved_block.text.clone();
            if last_text_end != text.start {
                // `SkDEBUGF("Text ranges mismatch: ...")` and `SkASSERT(false)`
                debug_assert!(false, "Text ranges mismatch");
            }
            last_text_end = text.end;

            if resolved_block.is_fully_resolved(&self.runs) {
                // Just move the entire run
                let run = run.expect("a run");
                self.runs[run].index = self.paragraph.runs.len();
                self.paragraph.runs.push(self.runs[run].clone());
                self.resolved_blocks[block_index].run = None;
                continue;
            }
            let Some(run) = run else {
                continue;
            };
            let run_ref = &self.runs[run];

            let run_advance = Vector::new(
                run_ref.pos_x(glyphs.end) - run_ref.pos_x(glyphs.start),
                run_ref.advance.y,
            );
            let info = RunInfo {
                font: &run_ref.font,
                bidi_level: run_ref.bidi_level,
                script: run_ref.script,
                language: Some(run_ref.language.as_str()),
                advance: run_advance,
                glyph_count: glyphs.width(),
                utf8_range: (text.start - run_ref.cluster_start)
                    ..(text.start - run_ref.cluster_start + text.width()),
            };
            let mut piece = Run::new(
                &info,
                run_ref.cluster_start,
                height,
                block.style.half_leading(),
                block.style.baseline_shift(),
                self.paragraph.runs.len(),
                *advance_x,
            );

            // TODO: Optimize copying
            let zero = Point::new(run_ref.positions[glyphs.start].x, 0.0);
            for i in glyphs.start..=glyphs.end {
                let index = i - glyphs.start;
                if i < glyphs.end {
                    // There are only n glyphs in a run, not n+1.
                    piece.glyphs[index] = run_ref.glyphs[i];

                    // fClusterIndexes n+1 is already set to the end of the run.
                    // Do not attempt to overwrite this value with the cluster index
                    // that starts the next Run.
                    // It is assumed later that all clusters in a Run are contained by the Run.
                    piece.cluster_indexes[index] = run_ref.cluster_indexes[i];
                }
                piece.positions[index] = run_ref.positions[i] - zero;
                piece.offsets[index] = run_ref.offsets[i];
                piece.add_x(index, *advance_x);
            }

            self.paragraph.runs.push(piece);

            // Carve out the line text out of the entire run text
            self.advance.x += run_advance.x;
            self.advance.y = if self.advance.y < run_advance.y {
                run_advance.y
            } else {
                self.advance.y
            };
        }

        *advance_x = self.advance.x;
        if last_text_end != block_text.end {
            // `SkDEBUGF("Last range mismatch: %zu - %zu\n", ...)` and `SkASSERT(false)`
            debug_assert!(false, "Last range mismatch");
        }
    }

    // Port of: modules/skparagraph/src/OneLineShaper.h#L124-L133 (chrome/m156)
    fn cluster_index(&self, glyph: GlyphIndex) -> ClusterIndex {
        self.current_text.start + self.runs[self.current_run].cluster_indexes[glyph] as usize
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L240-L252 (chrome/m156)
    /// Make it [left:right) regardless of a text direction.
    fn normalize_text_range(&self, glyph_range: &GlyphRange) -> TextRange {
        let run = &self.runs[self.current_run];
        if run.left_to_right() {
            self.cluster_index(glyph_range.start)..self.cluster_index(glyph_range.end)
        } else {
            self.cluster_index(glyph_range.end - 1)..(if glyph_range.start > 0 {
                self.cluster_index(glyph_range.start - 1)
            } else {
                run.text_range.end
            })
        }
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L254-L264 (chrome/m156)
    fn add_fully_resolved(&mut self) {
        let run = &self.runs[self.current_run];
        if run.size() == 0 {
            return;
        }
        let resolved = RunBlock::with_run(self.current_run, run.text_range.clone(), 0..run.size());
        self.resolved_blocks.push(resolved);
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L266-L303 (chrome/m156)
    fn add_unresolved_with_run(&mut self, mut glyph_range: GlyphRange) {
        let extended_text = self.clustered_text(&mut glyph_range); // It also modifies glyphRange if needed
        let unresolved = RunBlock::with_run(self.current_run, extended_text, glyph_range.clone());
        if unresolved.glyphs.width() == self.runs[self.current_run].size() {
            debug_assert!(
                unresolved.text.width() == self.runs[self.current_run].text_range.width()
            );
        } else if !self.unresolved_blocks.is_empty() {
            let last_run = self.unresolved_blocks.back().and_then(|b| b.run);
            if let Some(last_run) = last_run {
                if self.runs[last_run].index == self.runs[self.current_run].index {
                    let (last_text, last_glyphs) = {
                        let last_unresolved = self.unresolved_blocks.back().expect("a block");
                        (last_unresolved.text.clone(), last_unresolved.glyphs.clone())
                    };
                    if last_text.end == unresolved.text.start {
                        // Two pieces next to each other - can join them
                        let last_unresolved = self.unresolved_blocks.back_mut().expect("a block");
                        last_unresolved.text.end = unresolved.text.end;
                        last_unresolved.glyphs.end = glyph_range.end;
                        return;
                    } else if last_text == unresolved.text {
                        // Nothing was resolved; ignore it
                        return;
                    } else if RangeExtensions::contains(&last_text, &unresolved.text) {
                        // We get here for the very first unresolved piece
                        return;
                    } else if last_text.intersects(&unresolved.text) {
                        // Few pieces of the same unresolved text block can ignore the second one
                        let mut glyphs = last_glyphs;
                        glyphs.start = glyphs.start.min(glyph_range.start);
                        glyphs.end = glyphs.end.max(glyph_range.end);
                        let text = self.clustered_text(&mut glyphs);
                        let last_unresolved = self.unresolved_blocks.back_mut().expect("a block");
                        last_unresolved.glyphs = glyphs;
                        last_unresolved.text = text;
                        return;
                    }
                }
            }
        }
        self.unresolved_blocks.push_back(unresolved);
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L305-L360 (chrome/m156)
    /// The ranges of glyphs that are unresolved. Glue whitespaces to the next/prev unresolved
    /// blocks (so we don't have chinese text with english whitespaces broken into millions of
    /// tiny runs).
    fn sort_out_glyphs(&self) -> Vec<GlyphRange> {
        let mut blocks = Vec::new();
        let run = &self.runs[self.current_run];
        let mut block: GlyphRange = EMPTY_RANGE;
        let mut grapheme_resolved = false;
        let mut grapheme_start: usize = EMPTY_INDEX;
        for i in 0..run.size() {
            let ci = self.cluster_index(i);
            // Removing all pretty optimizations for whitespaces
            // because they get in a way of grapheme rounding
            // Inspect the glyph
            let glyph = run.glyphs[i];

            let gi = self.paragraph.find_previous_grapheme_boundary(ci);
            if (if run.left_to_right() {
                gi > grapheme_start
            } else {
                gi < grapheme_start
            }) || grapheme_start == EMPTY_INDEX
            {
                // This is the Flutter change
                // Do not count control codepoints as unresolved
                let is_control8 = self
                    .paragraph
                    .code_unit_has_property(ci, CodeUnitFlags::CONTROL);
                // We only count glyph resolved if all the glyphs in its grapheme are resolved
                grapheme_resolved = glyph != 0 || is_control8;
                grapheme_start = gi;
            } else if glyph == 0 {
                // Found unresolved glyph - the entire grapheme is unresolved now
                grapheme_resolved = false;
            }

            if !grapheme_resolved {
                // Unresolved glyph and not control codepoint
                if block.start == EMPTY_INDEX {
                    // Start new unresolved block
                    block.start = i;
                    block.end = EMPTY_INDEX;
                } else {
                    // Keep skipping unresolved block
                }
            } else {
                // Resolved glyph or control codepoint
                if block.start == EMPTY_INDEX {
                    // Keep skipping resolved code points
                } else {
                    // This is the end of unresolved block
                    block.end = i;
                    blocks.push(block.clone());
                    block = EMPTY_RANGE;
                }
            }
        }

        // One last block could have been left
        if block.start != EMPTY_INDEX {
            block.end = run.size();
            blocks.push(block);
        }
        blocks
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L362-L418 (chrome/m156)
    /// `iterateThroughFontStyles(textRange, styleSpan, visitor)`: `visitor(block, features)`
    /// gets each combined block of the same font attributes.
    fn iterate_through_font_styles(
        &mut self,
        text_range: &TextRange,
        style_span: &[Block],
        visitor: &mut dyn FnMut(&mut Self, Block, Vec<Feature>),
    ) {
        let mut combined_block = Block::default();
        let mut features: Vec<Feature> = Vec::new();

        let add_features = |features: &mut Vec<Feature>, block: &Block| {
            for ff in block.style.font_features() {
                if ff.name.len() != 4 {
                    // `SkDEBUGF("Incorrect font feature: %s=%d\n", ...)`
                    continue;
                }
                let name = ff.name.as_bytes();
                let feature = Feature {
                    tag: set_four_byte_tag(name[0], name[1], name[2], name[3]),
                    value: ff.value as u32,
                    start: block.range.start,
                    end: block.range.end,
                };
                features.push(feature);
            }
            // Disable ligatures if letter spacing is enabled.
            if block.style.letter_spacing() > 0.0 {
                features.push(Feature {
                    tag: set_four_byte_tag(b'l', b'i', b'g', b'a'),
                    value: 0,
                    start: block.range.start,
                    end: block.range.end,
                });
            }
        };

        for block in style_span {
            let block_range: TextRange =
                block.range.start.max(text_range.start)..block.range.end.min(text_range.end);
            if block_range.empty() {
                continue;
            }
            debug_assert!(
                combined_block.range.width() == 0 || combined_block.range.end == block.range.start
            );

            if !combined_block.range.empty() {
                if block
                    .style
                    .match_one_attribute(StyleType::Font, &combined_block.style)
                {
                    combined_block.add(block_range);
                    add_features(&mut features, block);
                    continue;
                }

                // Resolve all characters in the block for this style
                visitor(self, combined_block.clone(), features.clone());
            }

            combined_block.range = block_range;
            combined_block.style = block.style.clone();
            features.clear();
            add_features(&mut features, block);
        }

        visitor(self, combined_block, features);
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L420-L546 (chrome/m156)
    fn match_resolved_fonts(
        &mut self,
        text_style: &TextStyle,
        visitor: &mut dyn FnMut(&mut Self, Typeface) -> Resolved,
    ) {
        let typefaces = self
            .paragraph
            .font_collection
            .find_typefaces_with_font_arguments(
                text_style.font_families(),
                text_style.font_style(),
                text_style.font_arguments(),
            );

        for typeface in typefaces {
            if visitor(self, typeface) == Resolved::Everything {
                // Resolved everything
                return;
            }
        }

        if self.paragraph.font_collection.font_fallback_enabled() {
            // Give fallback a clue
            // Some unresolved subblocks might be resolved with different fallback fonts
            let mut hopeless_blocks: Vec<RunBlock> = Vec::new();
            while !self.unresolved_blocks.is_empty() {
                let unresolved_range = self
                    .unresolved_blocks
                    .front()
                    .expect("a block")
                    .text
                    .clone();
                let unresolved_text: Vec<u8> = self.paragraph.text_of(&unresolved_range).to_vec();
                let mut ch: &[u8] = &unresolved_text;
                // We have the global cache for all already found typefaces for SkUnichar
                // but we still need to keep track of all SkUnichars used in this unresolved block
                let mut already_tried_codepoints: HashSet<Unichar> = HashSet::new();
                let mut already_tried_typefaces: HashSet<TypefaceId> = HashSet::new();
                loop {
                    if ch.is_empty() {
                        // Not a single codepoint could be resolved but we finished the block
                        hopeless_blocks
                            .push(self.unresolved_blocks.front().expect("a block").clone());
                        self.unresolved_blocks.pop_front();
                        break;
                    }

                    // See if we can switch to the next DIFFERENT codepoint/emoji
                    let mut codepoint: Unichar = -1;
                    let mut emoji_start: Unichar = -1;
                    // We may loop until we find a new codepoint/emoji run
                    while !ch.is_empty() {
                        emoji_start =
                            get_emoji_sequence_start(&**self.paragraph.unicode(), &mut ch);
                        if emoji_start != -1 {
                            // We do not keep a cache of emoji runs, but we need to move the cursor
                            break;
                        }
                        codepoint = next_utf8_with_replacement(&mut ch);
                        if !already_tried_codepoints.contains(&codepoint) {
                            already_tried_codepoints.insert(codepoint);
                            break;
                        }
                    }

                    debug_assert!(codepoint != -1 || emoji_start != -1);

                    let mut typeface: Option<Typeface> = None;
                    if emoji_start == -1 {
                        // First try to find in in a cache
                        let font_key = FontKey {
                            unicode: codepoint,
                            font_style: text_style.font_style(),
                            locale: text_style.locale().to_owned(),
                            font_args: text_style.font_arguments().cloned(),
                        };
                        if let Some((_, found)) =
                            self.fallback_fonts.iter().find(|(key, _)| *key == font_key)
                        {
                            typeface = Some(found.clone());
                        }
                        if typeface.is_none() {
                            typeface = self.paragraph.font_collection.default_fallback_char(
                                codepoint,
                                text_style.font_families(),
                                text_style.font_style(),
                                text_style.locale(),
                                text_style.font_arguments(),
                            );
                            if let Some(typeface) = &typeface {
                                // `fFallbackFonts.set(fontKey, typeface)`
                                if let Some(entry) = self
                                    .fallback_fonts
                                    .iter_mut()
                                    .find(|(key, _)| *key == font_key)
                                {
                                    entry.1 = typeface.clone();
                                } else {
                                    self.fallback_fonts.push((font_key, typeface.clone()));
                                }
                            }
                        }
                    } else {
                        typeface = self.paragraph.font_collection.default_emoji_fallback(
                            emoji_start,
                            text_style.font_style(),
                            text_style.locale(),
                        );
                    }

                    let Some(typeface) = typeface else {
                        // There is no fallback font for this character,
                        // so move on to the next character.
                        continue;
                    };

                    // Check if we already tried this font on this text range
                    if already_tried_typefaces.contains(&typeface.unique_id()) {
                        continue;
                    }
                    already_tried_typefaces.insert(typeface.unique_id());

                    let resolved_blocks_before = self.resolved_blocks.len();
                    let mut resolved = visitor(self, typeface);
                    if resolved == Resolved::Everything {
                        if hopeless_blocks.is_empty() {
                            // Resolved everything, no need to try another font
                            return;
                        } else if resolved_blocks_before < self.resolved_blocks.len() {
                            // There are some resolved blocks
                            resolved = Resolved::Something;
                        } else {
                            // All blocks are hopeless
                            resolved = Resolved::Nothing;
                        }
                    }

                    if resolved == Resolved::Something {
                        // Resolved something, no need to try another codepoint
                        break;
                    }
                }
            }

            // Return hopeless blocks back
            for block in hopeless_blocks {
                self.unresolved_blocks.push_front(block);
            }
        }
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L548-L632 (chrome/m156)
    /// `iterateThroughShapingRegions(shape)`: `shape(this, textRange, blocks, advanceX, textStart,
    /// bidiLevel)` shapes the text between placeholders by bidi regions.
    fn iterate_through_shaping_regions(
        &mut self,
        shape: &mut dyn FnMut(&mut Self, &TextRange, &[Block], &mut scalar, usize, u8) -> bool,
    ) -> bool {
        let mut bidi_index: usize = 0;

        let mut advance_x: scalar = 0.0;
        for placeholder_index in 0..self.paragraph.placeholders.len() {
            let placeholder_text_before = self.paragraph.placeholders[placeholder_index]
                .text_before
                .clone();
            if placeholder_text_before.width() > 0 {
                // Shape the text by bidi regions
                while bidi_index < self.paragraph.bidi_regions.len() {
                    let bidi_region = self.paragraph.bidi_regions[bidi_index];
                    let start = bidi_region.start.max(placeholder_text_before.start);
                    let end = bidi_region.end.min(placeholder_text_before.end);

                    // Set up the iterators (the style iterator points to a bigger region that it could
                    let text_range: TextRange = start..end;
                    let block_range = self.paragraph.find_all_blocks(text_range.clone());
                    if !block_range.empty() {
                        let style_span: Vec<Block> = self.paragraph.blocks(block_range).to_vec();

                        // Shape the text between placeholders
                        if !shape(
                            self,
                            &text_range,
                            &style_span,
                            &mut advance_x,
                            start,
                            bidi_region.level,
                        ) {
                            return false;
                        }
                    }

                    if end == bidi_region.end {
                        bidi_index += 1;
                    } else {
                        // `if (end == placeholder.fTextBefore.end)`
                        break;
                    }
                }
            }

            let placeholder = self.paragraph.placeholders[placeholder_index].clone();
            if placeholder.range.width() == 0 {
                continue;
            }

            // Get the placeholder font
            let typefaces = self
                .paragraph
                .font_collection
                .find_typefaces_with_font_arguments(
                    placeholder.text_style.font_families(),
                    placeholder.text_style.font_style(),
                    placeholder.text_style.font_arguments(),
                );
            let typeface = typefaces.first().cloned();
            let mut font = Font::from_typeface(typeface);
            font.set_size(placeholder.text_style.font_size());

            font.set_edging(placeholder.text_style.font_edging());
            font.set_hinting(placeholder.text_style.font_hinting());
            font.set_subpixel(placeholder.text_style.subpixel());

            // "Shape" the placeholder
            let bidi_level: u8 = if bidi_index < self.paragraph.bidi_regions.len() {
                self.paragraph.bidi_regions[bidi_index].level
            } else {
                2
            };
            let run_info = RunInfo {
                font: &font,
                bidi_level,
                script: 0,
                language: Some(""),
                advance: Vector::new(placeholder.style.width, placeholder.style.height),
                glyph_count: 1,
                utf8_range: 0..placeholder.range.width(),
            };
            let run_index = self.paragraph.runs.len();
            let mut run = Run::new(
                &run_info,
                placeholder.range.start,
                0.0,
                false,
                0.0,
                run_index,
                advance_x,
            );

            run.positions[0] = Point::new(advance_x, 0.0);
            run.offsets[0] = Point::new(0.0, 0.0);
            run.cluster_indexes[0] = 0;
            run.placeholder_index = placeholder_index;
            self.paragraph.runs.push(run);
            advance_x += placeholder.style.width;
        }
        true
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L634-L738 (chrome/m156)
    /// `shape()`.
    pub fn shape(&mut self) -> bool {
        // The text can be broken into many shaping sequences
        // (by place holders, possibly, by hard line breaks or tabs, too)
        let limitless_width = f32::MAX;

        self.iterate_through_shaping_regions(
            &mut |this: &mut Self,
                  text_range: &TextRange,
                  style_span: &[Block],
                  advance_x: &mut scalar,
                  _text_start: usize,
                  default_bidi_level: u8| {
                // Set up the shaper and shape the next
                let shaper = harfbuzz::shape_dont_wrap_or_reorder(
                    this.paragraph.unicode().clone(),
                    Some(skia_rust_core::font_mgr::FontMgr::empty()), // no fallback
                );

                this.iterate_through_font_styles(text_range, style_span, &mut |this: &mut Self,
                                                                               block: Block,
                                                                               features: Vec<
                    Feature,
                >| {
                    let block_span = std::slice::from_ref(&block);

                    // Start from the beginning (hoping that it's a simple case one block - one run)
                    this.height = if block.style.height_override() {
                        block.style.height()
                    } else {
                        0.0
                    };
                    this.use_half_leading = block.style.half_leading();
                    this.baseline_shift = block.style.baseline_shift();
                    this.advance = Vector::new(*advance_x, 0.0);
                    this.current_text = block.range.clone();
                    this.unresolved_blocks
                        .push_back(RunBlock::new(block.range.clone()));

                    this.match_resolved_fonts(&block.style, &mut |this: &mut Self,
                                                                  typeface: Typeface|
                     -> Resolved {
                        // Create one more font to try
                        let mut font = Font::from_typeface(Some(typeface));
                        font.set_size(block.style.font_size());
                        font.set_edging(block.style.font_edging());
                        font.set_hinting(block.style.font_hinting());
                        font.set_subpixel(block.style.subpixel());

                        if this.paragraph.paragraph_style().fake_missing_font_styles() {
                            // Apply fake bold and/or italic settings to the font if the
                            // typeface's attributes do not match the intended font style.
                            let wanted_weight = *block.style.font_style().weight();
                            let fake_bold = wanted_weight
                                >= *skia_rust_core::font_style::Weight::SEMI_BOLD
                                && wanted_weight - *font.typeface().font_style().weight() >= 200;
                            let fake_italic = block.style.font_style().slant() == Slant::Italic
                                && font.typeface().font_style().slant() != Slant::Italic;
                            font.set_embolden(fake_bold);
                            font.set_skew_x(if fake_italic { -1.0 / 4.0 } else { 0.0 });
                        }

                        // Walk through all the currently unresolved blocks
                        // (ignoring those that appear later)
                        let resolved_count = this.resolved_blocks.len();
                        let mut unresolved_count = this.unresolved_blocks.len();
                        while unresolved_count > 0 {
                            unresolved_count -= 1;
                            let unresolved_range = this
                                .unresolved_blocks
                                .front()
                                .expect("a block")
                                .text
                                .clone();
                            if unresolved_range == EMPTY_TEXT {
                                // Duplicate blocks should be ignored
                                this.unresolved_blocks.pop_front();
                                continue;
                            }
                            let unresolved_text: Vec<u8> =
                                this.paragraph.text_of(&unresolved_range).to_vec();

                            let mut font_iter =
                                TrivialFontRunIterator::new(&font, unresolved_text.len());
                            let mut lang_iter = LangIterator::new(
                                unresolved_text.len(),
                                block_span,
                                this.paragraph.paragraph_style().text_style(),
                            );
                            let mut bidi_iter = TrivialBiDiRunIterator::new(
                                default_bidi_level,
                                unresolved_text.len(),
                            );
                            let mut script_iter = harfbuzz::script_run_iterator(&unresolved_text);
                            this.current_text = unresolved_range.clone();

                            // Map the block's features to subranges within the unresolved range.
                            let mut adjusted_features: Vec<Feature> =
                                Vec::with_capacity(features.len());
                            for feature in &features {
                                let feature_range: TextRange = feature.start..feature.end;
                                if unresolved_range.intersects(&feature_range) {
                                    let mut adjusted_range =
                                        unresolved_range.intersection(&feature_range);
                                    adjusted_range.shift(-(unresolved_range.start as isize));
                                    adjusted_features.push(Feature {
                                        tag: feature.tag,
                                        value: feature.value,
                                        start: adjusted_range.start,
                                        end: adjusted_range.end,
                                    });
                                }
                            }

                            shaper.shape(
                                &unresolved_text,
                                &mut font_iter,
                                &mut bidi_iter,
                                &mut *script_iter,
                                &mut lang_iter,
                                &adjusted_features,
                                &Options {
                                    width: limitless_width,
                                    ..Options::default()
                                },
                                this,
                            );

                            // Take off the queue the block we tried to resolved -
                            // whatever happened, we have now smaller pieces of it to deal with
                            this.unresolved_blocks.pop_front();
                        }

                        if this.unresolved_blocks.is_empty() {
                            // In some cases it does not mean everything
                            // (when we excluded some hopeless blocks from the list)
                            Resolved::Everything
                        } else if resolved_count < this.resolved_blocks.len() {
                            Resolved::Something
                        } else {
                            Resolved::Nothing
                        }
                    });

                    let height = this.height;
                    this.finish(&block, height, advance_x);
                });

                true
            },
        )
    }

    // Port of: modules/skparagraph/src/OneLineShaper.cpp#L740-L783 (chrome/m156)
    /// `clusteredText(glyphs)`: when we extend TextRange to the grapheme edges, we also extend
    /// glyphs range.
    fn clustered_text(&self, glyphs: &mut GlyphRange) -> TextRange {
        let run = &self.runs[self.current_run];

        #[derive(PartialEq)]
        enum Dir {
            Left,
            Right,
        }

        // [left: right)
        let find_base_char = |mut index: usize, dir: Dir| -> usize {
            if dir == Dir::Right {
                while index < run.text_range.end {
                    if self
                        .paragraph
                        .code_unit_has_property(index, CodeUnitFlags::GRAPHEME_START)
                    {
                        return index;
                    }
                    index += 1;
                }
                run.text_range.end
            } else {
                while index > run.text_range.start {
                    if self
                        .paragraph
                        .code_unit_has_property(index, CodeUnitFlags::GRAPHEME_START)
                    {
                        return index;
                    }
                    index -= 1;
                }
                run.text_range.start
            }
        };

        let mut text_range = self.normalize_text_range(glyphs);
        text_range.start = find_base_char(text_range.start, Dir::Left);
        text_range.end = find_base_char(text_range.end, Dir::Right);

        // Correct the glyphRange in case we extended the text to the grapheme edges
        // TODO: code it without if (as a part of LTR/RTL refactoring)
        if run.left_to_right() {
            while glyphs.start > 0 && self.cluster_index(glyphs.start) > text_range.start {
                glyphs.start -= 1;
            }
            while glyphs.end < run.size() && self.cluster_index(glyphs.end) < text_range.end {
                glyphs.end += 1;
            }
        } else {
            while glyphs.start > 0 && self.cluster_index(glyphs.start - 1) < text_range.end {
                glyphs.start -= 1;
            }
            while glyphs.end < run.size() && self.cluster_index(glyphs.end) > text_range.start {
                glyphs.end += 1;
            }
        }

        text_range.start..text_range.end
    }
}

// Port of: modules/skparagraph/src/OneLineShaper.cpp#L785-L832 (chrome/m156)
/// `getEmojiSequenceStart(unicode, &begin, end)`: determines if the string begins with an emoji
/// sequence (see https://unicode.org/reports/tr51/#def_emoji_sequence) and, if so, returns the
/// first codepoint, moving `text` past it. Otherwise it does not move `text` and returns -1.
///
/// By definition any emoji_sequence starts from a codepoint that has UCHAR_EMOJI property. If the
/// first codepoint does not have UCHAR_EMOJI_COMPONENT property, we have an emoji sequence right
/// away. In two (and only two) cases an emoji sequence starts with a codepoint that also has
/// UCHAR_EMOJI_COMPONENT property.
/// emoji_flag_sequence   := regional_indicator regional_indicator
/// emoji_keycap_sequence := [0-9#*] \x{FE0F 20E3}
/// These two cases require additional checks of the next codepoint(s).
#[doc(alias = "getEmojiSequenceStart")]
pub fn get_emoji_sequence_start(unicode: &dyn Unicode, text: &mut &[u8]) -> Unichar {
    let mut next = *text;
    let codepoint1 = next_utf8_with_replacement(&mut next);

    if !unicode.is_emoji(codepoint1) {
        // This is not a basic emoji nor it an emoji sequence
        return -1;
    }

    if !unicode.is_emoji_component(codepoint1) {
        // This is an emoji sequence start
        *text = next;
        return codepoint1;
    }

    // Now we need to look at the next codepoint to see what is going on
    let mut last = next;
    let codepoint2 = next_utf8_with_replacement(&mut last);

    // emoji_flag_sequence
    if unicode.is_regional_indicator(codepoint2) {
        // We expect a second regional indicator here
        if unicode.is_regional_indicator(codepoint2) {
            *text = next;
            return codepoint1;
        }
        // That really should not happen assuming correct UTF8 text
        return -1;
    }

    // emoji_keycap_sequence
    if codepoint2 == 0xFE0F {
        let codepoint3 = next_utf8_with_replacement(&mut last);
        if codepoint3 == 0x20E3 {
            *text = next;
            return codepoint1;
        }
    }

    -1
}

/// Calls `callback(unichar, start, end, count)` for each code point of `utf8`, as
/// `SkUnicode::forEachCodepoint` does.
pub(crate) fn for_each_codepoint(utf8: &[u8], callback: impl FnMut(Unichar, usize, usize, usize)) {
    for_each_codepoint_utf8(utf8, callback);
}

// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/src/SkShaper_harfbuzz.cpp,
// modules/skshaper/include/SkShaper_harfbuzz.h.

//! The HarfBuzz shapers (`SkShapers::HB`) on HarfRust: [`shaper_driven_wrapper`],
//! [`shape_then_wrap`] and [`shape_dont_wrap_or_reorder`], the HarfBuzz [`script_run_iterator`]
//! and the cache of faces.
//!
//! HarfRust 0.9.0 is the vendored `third_party/harfrust`, which differs from the upstream crate
//! where HarfBuzz at Skia's pin does (docs/design/modules.md decision 5).

// `HarfBuzz`, `HarfRust` and `Skia` are proper names, not code. The local types of the wrapping
// functions mirror the C++ lambdas and local structs.
#![allow(clippy::doc_markdown, clippy::items_after_statements)]
// The arithmetic mirrors C++ casts between `size_t`, `int`, `unsigned`, `float` and `double`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod face;

use std::ops::Range;
use std::sync::Arc;

use harfrust::{
    BufferClusterLevel, Direction, Feature as HbFeature, GlyphBuffer, Language, Script,
    ShapeOptions, Tag, UnicodeBuffer, script,
};
use skia_rust_core::font::Font;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_types::{FourByteTag, GlyphId};
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::scalar::scalar;
use skia_rust_core::utf::{self, Unichar};
use skia_rust_unicode::unicode::{BidiLevel, BreakIterator, BreakType, Unicode};

pub use self::face::{SkiaFontFuncs, skhb_position};
use self::face::{glyph_bounds, hb_font_for};
use crate::run_handler::{Buffer, RunHandler, RunInfo};
use crate::run_iterator::{
    BiDiRunIterator, FontRunIterator, LanguageRunIterator, RunIterator, ScriptRunIterator,
};
use crate::shaper::{Feature, Options, Shaper, ShaperBackend};

pub use self::face::purge_caches;

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L411-L416 (chrome/m156), `utf8_next`:
// replaces invalid UTF-8 sequences with REPLACEMENT CHARACTER U+FFFD.
fn utf8_next(utf8: &[u8], at: &mut usize) -> Unichar {
    let mut rest = &utf8[*at..];
    let val = utf::next_utf8_with_replacement(&mut rest);
    *at = utf8.len() - rest.len();
    val
}

// The HarfBuzz decoder of the buffer context: invalid input is a replacement character.
fn context_char(c: Unichar) -> u32 {
    match u32::try_from(c) {
        Ok(c) if char::from_u32(c).is_some() => c,
        _ => 0xFFFD,
    }
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L346-L408 (chrome/m156),
// `LanguageBasedLineBreaker`: collects and buffers line break opportunities per language run. By
// operating with language run granularity, as opposed to arbitrary run segment granularity, we
// ensure that the maximal context is made available to ICU (for e.g. phrase-based breaking).
struct LanguageBasedLineBreaker<'u> {
    unicode: &'u dyn Unicode,
    current_language: String,
    current_language_end_of_run: usize,
    line_breaks: Vec<i32>,
}

impl<'u> LanguageBasedLineBreaker<'u> {
    fn new(unicode: &'u dyn Unicode) -> Self {
        Self {
            unicode,
            current_language: String::new(),
            current_language_end_of_run: 0,
            line_breaks: Vec::new(),
        }
    }

    fn current_breaks(&self) -> &[i32] {
        &self.line_breaks
    }

    fn update_for_language(
        &mut self,
        utf8: &[u8],
        utf8_run_start: usize,
        language: &dyn LanguageRunIterator,
    ) -> bool {
        if !self.line_breaks.is_empty()
            && self.current_language == language.current_language()
            && self.current_language_end_of_run == language.end_of_current_run()
        {
            // Same language run, we've already buffered its breaks.
            return true;
        }

        // New language run, grab all its line breaks.
        language
            .current_language()
            .clone_into(&mut self.current_language);
        self.current_language_end_of_run = language.end_of_current_run();

        debug_assert!(utf8_run_start <= self.current_language_end_of_run);
        let lang_run_offset = utf8_run_start;
        let lang_run_length = self.current_language_end_of_run - lang_run_offset;

        let Some(mut lb_iter) = self
            .unicode
            .make_break_iterator_with_locale(&self.current_language, BreakType::Lines)
        else {
            return false;
        };
        if !lb_iter.set_text_utf8(&utf8[utf8_run_start..utf8_run_start + lang_run_length]) {
            return false;
        }

        self.line_breaks.clear();

        let mut pos = lb_iter.next();
        while !lb_iter.is_done() {
            // Adjust positions from local language indices back to global indices.
            self.line_breaks.push(pos + lang_run_offset as i32);
            pos = lb_iter.next();
        }

        // Line break iterators are expected to emit at least one position (for end-of-input).
        debug_assert_ne!(self.line_breaks.len(), 0);

        true
    }
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L422-L479 (chrome/m156),
// `SkUnicodeHbScriptRunIterator`.
#[derive(Debug)]
struct HbScriptRunIterator {
    utf8: Vec<u8>,
    current: usize,
    current_script: Option<Script>,
}

impl HbScriptRunIterator {
    fn script_for_unichar(u: Unichar) -> Script {
        harfrust::unicode_script(u as u32)
    }
}

impl RunIterator for HbScriptRunIterator {
    fn consume(&mut self) {
        debug_assert!(self.current < self.utf8.len());
        let end = self.utf8.len();
        let u = utf8_next(&self.utf8, &mut self.current);
        let mut current_script = Self::script_for_unichar(u);
        while self.current < end {
            let prev = self.current;
            let u = utf8_next(&self.utf8, &mut self.current);
            let script = Self::script_for_unichar(u);
            if script != current_script {
                if current_script == script::INHERITED || current_script == script::COMMON {
                    current_script = script;
                } else if script == script::INHERITED || script == script::COMMON {
                    // Keep the current script and go on.
                } else {
                    self.current = prev;
                    break;
                }
            }
        }
        if current_script == script::INHERITED {
            current_script = script::COMMON;
        }
        self.current_script = Some(current_script);
    }

    fn end_of_current_run(&self) -> usize {
        self.current
    }

    fn at_end(&self) -> bool {
        self.current == self.utf8.len()
    }
}

impl ScriptRunIterator for HbScriptRunIterator {
    fn current_script(&self) -> FourByteTag {
        self.current_script
            .map_or(0, |s| u32::from_be_bytes(s.tag().to_be_bytes()))
    }
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1688-L1696 (chrome/m156),
// `SkShapers::HB::ScriptRunIterator`.
/// `SkShapers::HB::ScriptRunIterator(utf8, utf8Bytes)`: runs of one script, found with HarfBuzz's
/// Unicode data. The script before the first run is unknown.
#[must_use]
#[doc(alias = "ScriptRunIterator")]
pub fn script_run_iterator(utf8: &[u8]) -> Box<dyn ScriptRunIterator> {
    Box::new(HbScriptRunIterator {
        utf8: utf8.to_vec(),
        current: 0,
        current_script: Some(script::UNKNOWN),
    })
}

/// `SkShapers::HB::ScriptRunIterator(utf8, utf8Bytes, script)`: as [`script_run_iterator`], with
/// `script` as the script before the first run.
#[must_use]
#[doc(alias = "ScriptRunIterator")]
pub fn script_run_iterator_with_script(
    utf8: &[u8],
    script: FourByteTag,
) -> Box<dyn ScriptRunIterator> {
    Box::new(HbScriptRunIterator {
        utf8: utf8.to_vec(),
        current: 0,
        current_script: Script::from_iso15924_tag(Tag::from_u32(script)),
    })
}

// The four run iterators of a `shape` call.
struct Iterators<'a> {
    font: &'a mut dyn FontRunIterator,
    bidi: &'a mut dyn BiDiRunIterator,
    script: &'a mut dyn ScriptRunIterator,
    language: &'a mut dyn LanguageRunIterator,
}

impl Iterators<'_> {
    // The iterator `which` of `RunIteratorQueue`: 0 font, 1 bidi, 2 script, 3 language.
    fn end_of(&self, which: usize) -> usize {
        match which {
            0 => self.font.end_of_current_run(),
            1 => self.bidi.end_of_current_run(),
            2 => self.script.end_of_current_run(),
            _ => self.language.end_of_current_run(),
        }
    }

    fn at_end(&self, which: usize) -> bool {
        match which {
            0 => self.font.at_end(),
            1 => self.bidi.at_end(),
            2 => self.script.at_end(),
            _ => self.language.at_end(),
        }
    }

    fn consume(&mut self, which: usize) {
        match which {
            0 => self.font.consume(),
            1 => self.bidi.consume(),
            2 => self.script.consume(),
            _ => self.language.consume(),
        }
    }
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L481-L542 (chrome/m156), `RunIteratorQueue`,
// a priority queue (`SkTDPQueue`) of the iterators ordered by the end of their current run, and
// by priority on a tie. There are four entries, so the least one is found by a scan.
struct RunIteratorQueue {
    // (iterator, priority)
    entries: Vec<(usize, i32)>,
}

impl RunIteratorQueue {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn insert(&mut self, which: usize, priority: i32) {
        self.entries.push((which, priority));
    }

    // `CompareEntry`
    fn less(its: &Iterators<'_>, a: (usize, i32), b: (usize, i32)) -> bool {
        let a_end = its.end_of(a.0);
        let b_end = its.end_of(b.0);
        a_end < b_end || (a_end == b_end && a.1 < b.1)
    }

    fn peek_index(&self, its: &Iterators<'_>) -> usize {
        let mut least = 0;
        for i in 1..self.entries.len() {
            if Self::less(its, self.entries[i], self.entries[least]) {
                least = i;
            }
        }
        least
    }

    fn advance_runs(&mut self, its: &mut Iterators<'_>) -> bool {
        let least_run = self.entries[self.peek_index(its)].0;
        if its.at_end(least_run) {
            debug_assert!(self.entries.iter().all(|e| its.at_end(e.0)));
            return false;
        }
        let least_end = its.end_of(least_run);
        loop {
            let index = self.peek_index(its);
            let (current_run, priority) = self.entries[index];
            if its.end_of(current_run) > least_end {
                break;
            }
            self.entries.swap_remove(index);
            #[cfg(debug_assertions)]
            let previous_end_of_current_run = its.end_of(current_run);
            its.consume(current_run);
            #[cfg(debug_assertions)]
            debug_assert!(previous_end_of_current_run < its.end_of(current_run));
            self.entries.push((current_run, priority));
        }
        true
    }

    fn end_of_current_run(&self, its: &Iterators<'_>) -> usize {
        its.end_of(self.entries[self.peek_index(its)].0)
    }
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L544-L554 (chrome/m156), `ShapedGlyph`.
// Mirrors the C++ struct of flags.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
struct ShapedGlyph {
    id: GlyphId,
    cluster: u32,
    offset: Point,
    advance: Vector,
    may_line_break_before: bool,
    must_line_break_before: bool,
    has_visual: bool,
    grapheme_break_before: bool,
    unsafe_to_break: bool,
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L555-L584 (chrome/m156), `ShapedRun`.
// `utf8_range` is the C++ `Range(begin, size)` as `begin..end`.
#[derive(Debug, Clone)]
struct ShapedRun {
    utf8_range: Range<usize>,
    font: Font,
    level: BidiLevel,
    script: FourByteTag,
    language: String,
    glyphs: Vec<ShapedGlyph>,
    num_glyphs: usize,
    advance: Vector,
}

impl ShapedRun {
    fn run_info(&self) -> RunInfo<'_> {
        RunInfo {
            font: &self.font,
            bidi_level: self.level,
            script: self.script,
            language: Some(&self.language),
            advance: self.advance,
            glyph_count: self.num_glyphs,
            utf8_range: self.utf8_range.clone(),
        }
    }
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L585-L588 (chrome/m156), `ShapedLine`.
#[derive(Debug, Default)]
struct ShapedLine {
    runs: Vec<ShapedRun>,
    advance: Vector,
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L590-L592 (chrome/m156), `is_LTR`.
const fn is_ltr(level: BidiLevel) -> bool {
    (level & 1) == 0
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L594-L620 (chrome/m156), `append`.
fn append(
    handler: &mut dyn RunHandler,
    run_info: &RunInfo<'_>,
    run: &ShapedRun,
    start_glyph_index: usize,
    end_glyph_index: usize,
) {
    debug_assert!(start_glyph_index <= end_glyph_index);
    let glyph_len = end_glyph_index - start_glyph_index;

    {
        let Buffer {
            glyphs,
            positions,
            mut offsets,
            mut clusters,
            point,
        } = handler.run_buffer(run_info);

        let mut advance = Vector::new(0.0, 0.0);
        for i in 0..glyph_len {
            // Glyphs are in logical order, but output ltr since PDF readers seem to expect that.
            let glyph = &run.glyphs[if is_ltr(run.level) {
                start_glyph_index + i
            } else {
                end_glyph_index - 1 - i
            }];
            glyphs[i] = glyph.id;
            if let Some(offsets) = offsets.as_deref_mut() {
                positions[i] = advance + point;
                offsets[i] = glyph.offset;
            } else {
                positions[i] = advance + point + glyph.offset;
            }
            if let Some(clusters) = clusters.as_deref_mut() {
                clusters[i] = glyph.cluster;
            }
            advance += glyph.advance;
        }
    }
    handler.commit_run_buffer(run_info);
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L622-L665 (chrome/m156), `emit`.
fn emit(unicode: &dyn Unicode, line: &ShapedLine, handler: &mut dyn RunHandler) {
    // Reorder the runs and glyphs per line and write them out.
    handler.begin_line();

    let num_runs = line.runs.len();
    let run_levels: Vec<BidiLevel> = line.runs.iter().map(|r| r.level).collect();
    let mut logical_from_visual = vec![0i32; num_runs];
    unicode.reorder_visual(&run_levels, &mut logical_from_visual);

    for &logical_index in &logical_from_visual {
        let run = &line.runs[logical_index as usize];
        handler.run_info(&run.run_info());
    }
    handler.commit_run_info();
    for &logical_index in &logical_from_visual {
        let run = &line.runs[logical_index as usize];
        append(handler, &run.run_info(), run, 0, run.num_glyphs);
    }

    handler.commit_line();
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L667-L726 (chrome/m156),
// `ShapedRunGlyphIterator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapedRunGlyphIterator {
    run_index: usize,
    glyph_index: usize,
}

impl ShapedRunGlyphIterator {
    fn new() -> Self {
        Self {
            run_index: 0,
            glyph_index: 0,
        }
    }

    // `next`: advances and returns whether there is a glyph at the new position.
    fn next(&mut self, runs: &[ShapedRun]) -> bool {
        debug_assert!(self.run_index < runs.len());
        debug_assert!(self.glyph_index < runs[self.run_index].num_glyphs);

        self.glyph_index += 1;
        if self.glyph_index == runs[self.run_index].num_glyphs {
            self.glyph_index = 0;
            self.run_index += 1;
            if self.run_index >= runs.len() {
                return false;
            }
        }
        true
    }

    // `current`
    fn current<'a>(&self, runs: &'a [ShapedRun]) -> Option<&'a ShapedGlyph> {
        runs.get(self.run_index)
            .map(|run| &run.glyphs[self.glyph_index])
    }
}

/// Which of the three shapers of `SkShapers::HB` a [`HarfbuzzShaper`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wrap {
    // `ShaperDrivenWrapper`
    ShaperDriven,
    // `ShapeThenWrap`
    ShapeThen,
    // `ShapeDontWrapOrReorder`
    DontWrapOrReorder,
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L728-L826 (chrome/m156), `ShaperHarfBuzz`
// with its three subclasses.
struct HarfbuzzShaper {
    unicode: Arc<dyn Unicode>,
    // for fallback; only the legacy `shape` overload with a single font uses it.
    font_mgr: Option<FontMgr>,
    wrap: Wrap,
}

impl std::fmt::Debug for HarfbuzzShaper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarfbuzzShaper")
            .field("wrap", &self.wrap)
            .finish_non_exhaustive()
    }
}

impl ShaperBackend for HarfbuzzShaper {
    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L880-L896 (chrome/m156),
    // `ShaperHarfBuzz::shape(SkSpan<const char>, ..., const Options&, RunHandler*)`.
    fn shape(
        &self,
        utf8: &[u8],
        font_runs: &mut dyn FontRunIterator,
        bidi_runs: &mut dyn BiDiRunIterator,
        script_runs: &mut dyn ScriptRunIterator,
        language_runs: &mut dyn LanguageRunIterator,
        features: &[Feature],
        options: &Options,
        handler: &mut dyn RunHandler,
    ) {
        let mut its = Iterators {
            font: font_runs,
            bidi: bidi_runs,
            script: script_runs,
            language: language_runs,
        };
        let mut run_segmenter = RunIteratorQueue::new();
        run_segmenter.insert(0, 3); // The font iterator is always run last in case of tie.
        run_segmenter.insert(1, 2);
        run_segmenter.insert(2, 1);
        run_segmenter.insert(3, 0);

        match self.wrap {
            Wrap::ShaperDriven => self.wrap_shaper_driven(
                utf8,
                &mut its,
                &mut run_segmenter,
                features,
                options.width,
                options.tracking,
                handler,
            ),
            Wrap::ShapeThen => self.wrap_shape_then(
                utf8,
                &mut its,
                &mut run_segmenter,
                features,
                options.width,
                options.tracking,
                handler,
            ),
            Wrap::DontWrapOrReorder => Self::wrap_dont_wrap_or_reorder(
                utf8,
                &mut its,
                &mut run_segmenter,
                features,
                options.tracking,
                handler,
            ),
        }
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L828-L862 (chrome/m156), the legacy
    // `ShaperHarfBuzz::shape(utf8, utf8Bytes, const SkFont&, bool leftToRight, ...)`.
    fn shape_font(
        &self,
        utf8: &[u8],
        font: &Font,
        left_to_right: bool,
        width: scalar,
        handler: &mut dyn RunHandler,
    ) {
        let default_level = u8::from(!left_to_right);
        let Some(mut bidi) = unicode_bidi_run_iterator(&*self.unicode, utf8, default_level) else {
            return;
        };
        let bidi: &mut dyn BiDiRunIterator = &mut *bidi;
        let mut language = crate::run_iterator::make_std_language_run_iterator(utf8.len());
        let mut script = HbScriptRunIterator {
            utf8: utf8.to_vec(),
            current: 0,
            current_script: Some(script::UNKNOWN),
        };
        let mut font_runs =
            crate::run_iterator::make_font_mgr_run_iterator(utf8, font, self.font_mgr.clone());
        let options = Options {
            width,
            ..Options::default()
        };
        self.shape(
            utf8,
            &mut font_runs,
            bidi,
            &mut script,
            &mut language,
            &[],
            &options,
            handler,
        );
    }
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1318-L1346 (chrome/m156), the typeface's
// `HBFont` from the cache, `create_sub_hb_font` and `hb_shape`.
/// Shapes the content of `buffer` with the font of `font`: the face of its typeface from the cache
/// of faces, Skia's font functions over a copy of `font`, and the scale of its size. `None` when
/// the typeface cannot be made into a face.
#[must_use]
pub fn shape_buffer(
    font: &Font,
    buffer: UnicodeBuffer,
    features: &[HbFeature],
) -> Option<GlyphBuffer> {
    let hb_font = hb_font_for(font.typeface())?;
    let font_ref = hb_font.font_ref()?;

    // create_sub_hb_font: Skia's font functions and the scale of the font size.
    let mut funcs = SkiaFontFuncs { font: font.clone() };
    let scale = skhb_position(font.size());
    let shaper = hb_font
        .shaper_data()
        .shaper(&font_ref)
        .instance(Some(hb_font.instance()))
        .build();
    Some(
        shaper.shape(
            buffer,
            ShapeOptions::new()
                .scale(Some(scale))
                .features(features)
                .font_funcs(Some(&mut funcs)),
        ),
    )
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1235-L1393 (chrome/m156),
// `ShaperHarfBuzz::shape(char const*, ..., ShapedRun)`: shapes `utf8[utf8_start..utf8_end]`.
impl HarfbuzzShaper {
    fn shape_run(
        utf8: &[u8],
        utf8_start: usize,
        utf8_end: usize,
        its: &Iterators<'_>,
        features: &[Feature],
        text_tracking: f32,
    ) -> ShapedRun {
        let utf8_run_length = utf8_end - utf8_start;
        let current_font = its.font.current_font();
        let mut run = ShapedRun {
            utf8_range: utf8_start..utf8_start + utf8_run_length,
            font: current_font.clone(),
            level: its.bidi.current_level(),
            script: its.script.current_script(),
            language: its.language.current_language().to_owned(),
            glyphs: Vec::new(),
            num_glyphs: 0,
            advance: Vector::new(0.0, 0.0),
        };

        let mut buffer = UnicodeBuffer::new();
        buffer.set_cluster_level(BufferClusterLevel::MonotoneCharacters);

        // Documentation for HB_BUFFER_FLAG_BOT/EOT at 763e5466c0a03a7c27020e1e2598e488612529a7.
        // Currently BOT forces a dotted circle when first codepoint is a mark; EOT has no effect.
        // Avoid adding dotted circle, re-evaluate if BOT/EOT change. See https://skbug.com/40040947.
        // (No flags are set.)

        // Add precontext: up to five code points before the run, nearest first.
        let mut pre: Vec<u32> = Vec::new();
        {
            let prefix = &utf8[..utf8_start];
            let mut end = prefix.len();
            while end > 0 && pre.len() < 5 {
                let mut start = end - 1;
                while start > 0 && end - start < 4 && (prefix[start] & 0xC0) == 0x80 {
                    start -= 1;
                }
                match std::str::from_utf8(&prefix[start..end]) {
                    Ok(s) if s.chars().count() == 1 => {
                        pre.push(s.chars().next().map_or(0xFFFD, u32::from));
                        end = start;
                    }
                    _ => {
                        // An invalid byte is a replacement character.
                        pre.push(0xFFFD);
                        end -= 1;
                    }
                }
            }
        }
        buffer.set_pre_context_codepoints(&pre);

        // Populate the hb_buffer directly with utf8 cluster indexes.
        let mut utf8_current = utf8_start;
        while utf8_current < utf8_end {
            let cluster = utf8_current as u32;
            let u = utf8_next(&utf8[..utf8_end], &mut utf8_current);
            buffer.add(
                char::from_u32(context_char(u)).unwrap_or('\u{FFFD}'),
                cluster,
            );
        }

        // Add postcontext: up to five code points after the run.
        let mut post: Vec<u32> = Vec::new();
        {
            let mut at = utf8_current;
            while at < utf8.len() && post.len() < 5 {
                let c = utf8_next(utf8, &mut at);
                post.push(context_char(c));
            }
        }
        buffer.set_post_context_codepoints(&post);

        let direction = if is_ltr(run.level) {
            Direction::LeftToRight
        } else {
            Direction::RightToLeft
        };
        buffer.set_direction(direction);
        if let Some(script) = Script::from_iso15924_tag(Tag::from_u32(run.script)) {
            buffer.set_script(script);
        }
        // Buffers with HB_LANGUAGE_INVALID race since hb_language_get_default is not thread safe.
        // The user must provide a language, but may provide data hb_language_from_string cannot use.
        // Use "und" for the undefined language in this case (RFC5646 4.1 5).
        let language = Language::new(run.language.as_bytes())
            .or_else(|| Language::new(b"und"))
            .expect("a non-empty language");
        buffer.set_language(language);
        buffer.guess_segment_properties();

        let mut hb_features: Vec<HbFeature> = Vec::new();
        for feature in features {
            if feature.end < utf8_start || utf8_end <= feature.start {
                continue;
            }
            if feature.start <= utf8_start && utf8_end <= feature.end {
                hb_features.push(HbFeature {
                    tag: Tag::from_u32(feature.tag),
                    value: feature.value,
                    start: 0,
                    end: u32::MAX,
                });
            } else {
                hb_features.push(HbFeature {
                    tag: Tag::from_u32(feature.tag),
                    value: feature.value,
                    start: feature.start as u32,
                    end: feature.end as u32,
                });
            }
        }

        let Some(glyph_buffer) = shape_buffer(current_font, buffer, &hb_features) else {
            return run;
        };
        let len = glyph_buffer.len();
        if len == 0 {
            return run;
        }

        let mut infos: Vec<_> = glyph_buffer.glyph_infos().to_vec();
        let mut positions: Vec<_> = glyph_buffer.glyph_positions().to_vec();
        if direction == Direction::RightToLeft {
            // Put the clusters back in logical order.
            // Note that the advances remain ltr.
            infos.reverse();
            positions.reverse();
        }

        run.num_glyphs = len;
        run.glyphs = vec![ShapedGlyph::default(); len];

        // Undo skhb_position with (1.0/(1<<16)) and scale as needed.
        let glyph_ids: Vec<GlyphId> = infos
            .iter()
            .map(|i| GlyphId::try_from(i.glyph_id & 0xFFFF).unwrap_or(0))
            .collect();
        let bounds = glyph_bounds(&run.font, &glyph_ids);

        let sk_scalar_from_hb_pos_x: f64 = 1.525_878_906_25e-5 * f64::from(run.font.scale_x());
        let sk_scalar_from_hb_pos_y: f64 = -1.525_878_906_25e-5; // HarfBuzz y-up, Skia y-down

        // Tracking is specified in em units; turn it into an absolute advance increment.
        let tracking_advance = text_tracking * run.font.size() * run.font.scale_x();

        let mut run_advance = Vector::new(0.0, 0.0);
        for i in 0..len {
            let glyph = &mut run.glyphs[i];
            let info = &infos[i];
            let pos = &positions[i];
            glyph.id = glyph_ids[i];
            glyph.cluster = info.cluster;
            glyph.offset.x = (f64::from(pos.x_offset) * sk_scalar_from_hb_pos_x) as f32;
            glyph.offset.y = (f64::from(pos.y_offset) * sk_scalar_from_hb_pos_y) as f32;
            glyph.advance.x = (f64::from(pos.x_advance) * sk_scalar_from_hb_pos_x
                + f64::from(tracking_advance)) as f32;
            glyph.advance.y = (f64::from(pos.y_advance) * sk_scalar_from_hb_pos_y) as f32;

            glyph.has_visual = !bounds[i].is_empty(); // !font->currentTypeface()->glyphBoundsAreZero(glyph.fID);
            glyph.unsafe_to_break = info.unsafe_to_break();
            glyph.must_line_break_before = false;

            run_advance += glyph.advance;
        }
        run.advance = run_advance;

        run
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L898-L1087 (chrome/m156),
    // `ShaperDrivenWrapper::wrap`.
    fn wrap_shaper_driven(
        &self,
        utf8: &[u8],
        its: &mut Iterators<'_>,
        run_segmenter: &mut RunIteratorQueue,
        features: &[Feature],
        width: scalar,
        text_tracking: f32,
        handler: &mut dyn RunHandler,
    ) {
        let mut line = ShapedLine::default();

        // The current runSegmenter (sub) segment.
        // Starts off as a full segment, and we consume from the beginning until complete.
        let mut utf8_end = 0usize;

        // Whether we are allowed to break at the beginning of this run segment.
        let mut can_break_at_start = true;

        let mut lb = LanguageBasedLineBreaker::new(&*self.unicode);

        while run_segmenter.advance_runs(its) {
            // For each item
            let mut utf8_start = utf8_end;
            utf8_end = run_segmenter.end_of_current_run(its);

            // model holds the full remaining run segment shaped in one pass
            let mut model: Option<ShapedRun> = None;
            let mut model_glyph_offset = 0usize;

            // map from character position to [safe to break, glyph position, advance]
            #[derive(Clone, Copy, Default)]
            struct TextProps {
                glyph_len: usize,
                advance: Vector,
            }
            let mut model_text: Vec<TextProps> = Vec::new();
            let mut model_text_offset = 0usize;
            let mut model_advance_offset = Vector::new(0.0, 0.0);

            while utf8_start < utf8_end {
                // While there are still code points left in this item
                // Refresh line break data if needed (if the language has changed).
                if !lb.update_for_language(utf8, utf8_start, &*its.language) {
                    return;
                }

                let utf8_run_length = utf8_end - utf8_start;
                if model.is_none() {
                    let shaped =
                        Self::shape_run(utf8, utf8_start, utf8_end, its, features, text_tracking);
                    model_glyph_offset = 0;

                    let mut advance = Vector::new(0.0, 0.0);
                    model_text = vec![TextProps::default(); utf8_run_length + 1];
                    let model_start_cluster = utf8_start;
                    let mut previous_cluster = 0usize;
                    for i in 0..shaped.num_glyphs {
                        debug_assert!(model_start_cluster <= shaped.glyphs[i].cluster as usize);
                        debug_assert!((shaped.glyphs[i].cluster as usize) < utf8_end);
                        if !shaped.glyphs[i].unsafe_to_break {
                            // Store up to the first glyph in the cluster.
                            let current_cluster =
                                shaped.glyphs[i].cluster as usize - model_start_cluster;
                            if previous_cluster != current_cluster {
                                previous_cluster = current_cluster;
                                model_text[current_cluster].glyph_len = i;
                                model_text[current_cluster].advance = advance;
                            }
                        }
                        advance += shaped.glyphs[i].advance;
                    }
                    // Assume it is always safe to break after the end of an item
                    model_text[utf8_run_length].glyph_len = shaped.num_glyphs;
                    model_text[utf8_run_length].advance = shaped.advance;
                    model_text_offset = 0;
                    model_advance_offset = Vector::new(0.0, 0.0);
                    model = Some(shaped);
                }

                let mut best: Option<ShapedRun> = None;
                let mut best_uses_model_for_glyphs = false;
                let width_left = width - line.advance.x;

                // Returns true if the candidate is the new best.
                let mut evaluate_candidate = |break_iterator_current: usize| -> bool {
                    let mut candidate_uses_model_for_glyphs = false;
                    let props = model_text[break_iterator_current + model_text_offset];
                    let candidate = if props.glyph_len != 0 {
                        candidate_uses_model_for_glyphs = true;
                        ShapedRun {
                            utf8_range: utf8_start..utf8_start + break_iterator_current,
                            font: its.font.current_font().clone(),
                            level: its.bidi.current_level(),
                            script: its.script.current_script(),
                            language: its.language.current_language().to_owned(),
                            glyphs: Vec::new(),
                            num_glyphs: props.glyph_len.wrapping_sub(model_glyph_offset),
                            advance: props.advance - model_advance_offset,
                        }
                    } else {
                        Self::shape_run(
                            utf8,
                            utf8_start,
                            utf8_start + break_iterator_current,
                            its,
                            features,
                            text_tracking,
                        )
                    };
                    let score = |run: &ShapedRun| -> scalar {
                        if run.advance.x < width_left {
                            run.utf8_range.len() as scalar
                        } else {
                            width_left - run.advance.x
                        }
                    };
                    if best.as_ref().is_none_or(|b| score(b) < score(&candidate)) {
                        best = Some(candidate);
                        best_uses_model_for_glyphs = candidate_uses_model_for_glyphs;
                        return true;
                    }
                    false
                };

                // Scan all buffered line break opportunities for the current language run,
                // and check if any are applicable to the current segment.
                let mut evaluated_segment_end = false;
                let mut valid_break_candidate = false;
                for &pos in lb.current_breaks() {
                    // Break position relative to the current segment start.
                    debug_assert!(0 <= utf8_start as i64);
                    let break_iterator_current = i64::from(pos) - utf8_start as i64;

                    if break_iterator_current <= 0 {
                        continue;
                    }
                    if break_iterator_current as usize > utf8_run_length {
                        break;
                    }

                    // We found a break opportunity within the current run segment, let's shape it
                    // and evaluate its score.
                    if evaluate_candidate(break_iterator_current as usize) {
                        valid_break_candidate = true;
                    }

                    // Track whether we've seen the segment end as a potential break.
                    evaluated_segment_end |= break_iterator_current as usize == utf8_run_length;
                }

                if !evaluated_segment_end {
                    // If not done already, always evaluate the end of the current run segment as a
                    // candidate to allow transitioning to the next segment on the same line if it
                    // fits.
                    //
                    // If we end up selecting the full segment at this stage, we'll consume it (since
                    // it fits), but we cannot break after because there are no corresponding lb
                    // breaks.
                    if evaluate_candidate(utf8_run_length) {
                        valid_break_candidate = false;
                    }
                }

                let mut best = best.expect("a candidate was evaluated");

                // If nothing fit (best score is negative), and the line is not empty, and we are
                // allowed to beak at the start of the current candidate, flush the pending line.
                if width < line.advance.x + best.advance.x
                    && !line.runs.is_empty()
                    && can_break_at_start
                {
                    emit(&*self.unicode, &line, handler);
                    line.runs.clear();
                    line.advance = Vector::new(0.0, 0.0);
                } else {
                    if best_uses_model_for_glyphs {
                        let model_ref = model.as_ref().expect("the model");
                        best.glyphs = model_ref.glyphs
                            [model_glyph_offset..model_glyph_offset + best.num_glyphs]
                            .to_vec();
                        model_glyph_offset += best.num_glyphs;
                        model_text_offset += best.utf8_range.len();
                        model_advance_offset += best.advance;
                    } else {
                        model = None;
                    }
                    utf8_start += best.utf8_range.len();
                    line.advance += best.advance;
                    line.runs.push(best);

                    // If item broken, emit line (prevent remainder from accidentally fitting)
                    if utf8_start != utf8_end {
                        emit(&*self.unicode, &line, handler);
                        line.runs.clear();
                        line.advance = Vector::new(0.0, 0.0);
                    }
                }

                // Update state: we are allowed to break at the beginning of the next run segment
                // if we found a valid break candidate (thus the next segment is known to start
                // after a break).
                can_break_at_start = valid_break_candidate;
            }
        }
        emit(&*self.unicode, &line, handler);
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1089-L1411 (chrome/m156),
    // `ShapeThenWrap::wrap`.
    fn wrap_shape_then(
        &self,
        utf8: &[u8],
        its: &mut Iterators<'_>,
        run_segmenter: &mut RunIteratorQueue,
        features: &[Feature],
        width: scalar,
        text_tracking: f32,
        handler: &mut dyn RunHandler,
    ) {
        let mut runs: Vec<ShapedRun> = Vec::new();
        {
            let mut current_language = String::new();
            let mut line_break_iterator: Option<Box<dyn BreakIterator + '_>> = None;
            let mut grapheme_break_iterator: Option<Box<dyn BreakIterator + '_>> = None;
            let mut need_iterator_init = true;
            let mut utf8_end = 0usize;
            while run_segmenter.advance_runs(its) {
                let utf8_start = utf8_end;
                utf8_end = run_segmenter.end_of_current_run(its);

                runs.push(Self::shape_run(
                    utf8,
                    utf8_start,
                    utf8_end,
                    its,
                    features,
                    text_tracking,
                ));
                let run = runs.last_mut().expect("a run was pushed");

                if need_iterator_init || current_language != its.language.current_language() {
                    its.language
                        .current_language()
                        .clone_into(&mut current_language);
                    line_break_iterator = self
                        .unicode
                        .make_break_iterator_with_locale(&current_language, BreakType::Lines);
                    if line_break_iterator.is_none() {
                        return;
                    }
                    grapheme_break_iterator = self
                        .unicode
                        .make_break_iterator_with_locale(&current_language, BreakType::Graphemes);
                    if grapheme_break_iterator.is_none() {
                        return;
                    }
                    need_iterator_init = false;
                }
                let (Some(line_break_iterator), Some(grapheme_break_iterator)) = (
                    line_break_iterator.as_mut(),
                    grapheme_break_iterator.as_mut(),
                ) else {
                    return;
                };
                if !line_break_iterator.set_text_utf8(&utf8[utf8_start..utf8_end]) {
                    return;
                }
                if !grapheme_break_iterator.set_text_utf8(&utf8[utf8_start..utf8_end]) {
                    return;
                }

                let mut previous_cluster = 0xFFFF_FFFFu32;
                for i in 0..run.num_glyphs {
                    let glyph = &mut run.glyphs[i];
                    let glyph_cluster = glyph.cluster as i32;

                    let mut line_break_iterator_current = line_break_iterator.current();
                    while !line_break_iterator.is_done()
                        && line_break_iterator_current < glyph_cluster
                    {
                        line_break_iterator_current = line_break_iterator.next();
                    }
                    glyph.may_line_break_before = glyph.cluster != previous_cluster
                        && line_break_iterator_current == glyph_cluster;

                    let mut grapheme_break_iterator_current = grapheme_break_iterator.current();
                    while !grapheme_break_iterator.is_done()
                        && grapheme_break_iterator_current < glyph_cluster
                    {
                        grapheme_break_iterator_current = grapheme_break_iterator.next();
                    }
                    glyph.grapheme_break_before = glyph.cluster != previous_cluster
                        && grapheme_break_iterator_current == glyph_cluster;

                    previous_cluster = glyph.cluster;
                }
            }
        }

        // Iterate over the glyphs in logical order to find potential line lengths.
        {
            /* The position of the beginning of the line. */
            let mut beginning = ShapedRunGlyphIterator::new();

            /* The position of the candidate line break. */
            let mut candidate_line_break = ShapedRunGlyphIterator::new();
            let mut candidate_line_break_width: scalar = 0.0;

            /* The position of the candidate grapheme break. */
            let mut candidate_grapheme_break = ShapedRunGlyphIterator::new();
            let mut candidate_grapheme_break_width: scalar = 0.0;

            /* The position of the current location. */
            let mut current = ShapedRunGlyphIterator::new();
            let mut current_width: scalar = 0.0;
            while let Some(&glyph) = current.current(&runs) {
                // 'Break' at graphemes until a line boundary, then only at line boundaries.
                // Only break at graphemes if no line boundary is valid.
                if current != beginning
                    && (glyph.grapheme_break_before || glyph.may_line_break_before)
                {
                    // TODO: preserve line breaks <= grapheme breaks
                    // and prevent line breaks inside graphemes
                    candidate_grapheme_break = current;
                    candidate_grapheme_break_width = current_width;
                    if glyph.may_line_break_before {
                        candidate_line_break = current;
                        candidate_line_break_width = current_width;
                    }
                }

                let glyph_width = glyph.advance.x;
                // Break when overwidth, the glyph has a visual representation, and some space is used.
                if width < current_width + glyph_width
                    && glyph.has_visual
                    && candidate_grapheme_break_width > 0.0
                {
                    if candidate_line_break != beginning {
                        beginning = candidate_line_break;
                        current_width -= candidate_line_break_width;
                        candidate_grapheme_break_width -= candidate_line_break_width;
                        candidate_line_break_width = 0.0;
                    } else if candidate_grapheme_break != beginning {
                        beginning = candidate_grapheme_break;
                        candidate_line_break = beginning;
                        current_width -= candidate_grapheme_break_width;
                        candidate_grapheme_break_width = 0.0;
                        candidate_line_break_width = 0.0;
                    } else {
                        panic!("ShapeThenWrap: no break candidate");
                    }

                    if width < current_width {
                        if width < candidate_grapheme_break_width {
                            candidate_grapheme_break = candidate_line_break;
                            candidate_grapheme_break_width = candidate_line_break_width;
                        }
                        current = candidate_grapheme_break;
                        current_width = candidate_grapheme_break_width;
                    }

                    if beginning.current(&runs).is_some() {
                        runs[beginning.run_index].glyphs[beginning.glyph_index]
                            .must_line_break_before = true;
                    }
                } else {
                    current.next(&runs);
                    current_width += glyph_width;
                }
            }
        }

        // Reorder the runs and glyphs per line and write them out.
        {
            let mut previous_break = ShapedRunGlyphIterator::new();
            let mut glyph_iterator = ShapedRunGlyphIterator::new();
            while glyph_iterator.current(&runs).is_some() {
                let current = glyph_iterator;
                let has_next = glyph_iterator.next(&runs);
                let next_must_break = has_next
                    && glyph_iterator
                        .current(&runs)
                        .is_some_and(|g| g.must_line_break_before);

                // Nothing can be written until the baseline is known.
                if has_next && !next_must_break {
                    continue;
                }

                let num_runs = current.run_index - previous_break.run_index + 1;
                let run_levels: Vec<BidiLevel> = (0..num_runs)
                    .map(|i| runs[previous_break.run_index + i].level)
                    .collect();
                let mut logical_from_visual = vec![0i32; num_runs];
                self.unicode
                    .reorder_visual(&run_levels, &mut logical_from_visual);

                // step through the runs in reverse visual order and the glyphs in reverse logical order
                // until a visible glyph is found and force them to the end of the visual line.

                handler.begin_line();

                struct SubRun<'r> {
                    run: &'r ShapedRun,
                    start_glyph_index: usize,
                    end_glyph_index: usize,
                }
                impl<'r> SubRun<'r> {
                    // `makeRunInfo`
                    fn run_info(&self) -> RunInfo<'r> {
                        let sub = self;
                        let start_utf8 = sub.run.glyphs[sub.start_glyph_index].cluster;
                        let end_utf8 = if sub.end_glyph_index < sub.run.num_glyphs {
                            sub.run.glyphs[sub.end_glyph_index].cluster
                        } else {
                            sub.run.utf8_range.end as u32
                        };

                        let mut advance = Vector::new(0.0, 0.0);
                        for i in sub.start_glyph_index..sub.end_glyph_index {
                            advance += sub.run.glyphs[i].advance;
                        }

                        RunInfo {
                            font: &sub.run.font,
                            bidi_level: sub.run.level,
                            script: sub.run.script,
                            language: Some(&sub.run.language),
                            advance,
                            glyph_count: sub.end_glyph_index - sub.start_glyph_index,
                            utf8_range: start_utf8 as usize..end_utf8 as usize,
                        }
                    }
                }
                let make_sub_run = |visual_index: usize| -> SubRun<'_> {
                    let logical_index =
                        previous_break.run_index + logical_from_visual[visual_index] as usize;
                    let run = &runs[logical_index];
                    let start_glyph_index = if logical_index == previous_break.run_index {
                        previous_break.glyph_index
                    } else {
                        0
                    };
                    let end_glyph_index = if logical_index == current.run_index {
                        current.glyph_index + 1
                    } else {
                        run.num_glyphs
                    };
                    SubRun {
                        run,
                        start_glyph_index,
                        end_glyph_index,
                    }
                };
                for i in 0..num_runs {
                    handler.run_info(&make_sub_run(i).run_info());
                }
                handler.commit_run_info();
                for i in 0..num_runs {
                    let sub = make_sub_run(i);
                    append(
                        handler,
                        &sub.run_info(),
                        sub.run,
                        sub.start_glyph_index,
                        sub.end_glyph_index,
                    );
                }

                handler.commit_line();

                previous_break = glyph_iterator;
            }
        }
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1413-L1462 (chrome/m156),
    // `ShapeDontWrapOrReorder::wrap`.
    fn wrap_dont_wrap_or_reorder(
        utf8: &[u8],
        its: &mut Iterators<'_>,
        run_segmenter: &mut RunIteratorQueue,
        features: &[Feature],
        text_tracking: f32,
        handler: &mut dyn RunHandler,
    ) {
        let mut runs: Vec<ShapedRun> = Vec::new();

        let mut utf8_end = 0usize;
        while run_segmenter.advance_runs(its) {
            let utf8_start = utf8_end;
            utf8_end = run_segmenter.end_of_current_run(its);

            runs.push(Self::shape_run(
                utf8,
                utf8_start,
                utf8_end,
                its,
                features,
                text_tracking,
            ));
        }

        handler.begin_line();
        for run in &runs {
            handler.run_info(&run.run_info());
        }
        handler.commit_run_info();
        for run in &runs {
            append(handler, &run.run_info(), run, 0, run.num_glyphs);
        }
        handler.commit_line();
    }
}

fn make_shaper(unicode: Arc<dyn Unicode>, fallback: Option<FontMgr>, wrap: Wrap) -> Shaper {
    Shaper::from_backend(Box::new(HarfbuzzShaper {
        unicode,
        font_mgr: fallback,
        wrap,
    }))
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1674-L1685 (chrome/m156),
// `SkShapers::HB::ShaperDrivenWrapper`.
/// `SkShapers::HB::ShaperDrivenWrapper(unicode, fallback)`: shapes the text, breaking it into lines
/// at the line break opportunities that fit.
#[must_use]
#[doc(alias = "ShaperDrivenWrapper")]
pub fn shaper_driven_wrapper(unicode: Arc<dyn Unicode>, fallback: Option<FontMgr>) -> Shaper {
    make_shaper(unicode, fallback, Wrap::ShaperDriven)
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1687-L1699 (chrome/m156),
// `SkShapers::HB::ShapeThenWrap`.
/// `SkShapers::HB::ShapeThenWrap(unicode, fallback)`: shapes the whole text, then breaks the glyphs
/// into lines.
#[must_use]
#[doc(alias = "ShapeThenWrap")]
pub fn shape_then_wrap(unicode: Arc<dyn Unicode>, fallback: Option<FontMgr>) -> Shaper {
    make_shaper(unicode, fallback, Wrap::ShapeThen)
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1701-L1713 (chrome/m156),
// `SkShapers::HB::ShapeDontWrapOrReorder`.
/// `SkShapers::HB::ShapeDontWrapOrReorder(unicode, fallback)`: shapes the text as one line, with
/// the runs in logical order.
#[must_use]
#[doc(alias = "ShapeDontWrapOrReorder")]
pub fn shape_dont_wrap_or_reorder(unicode: Arc<dyn Unicode>, fallback: Option<FontMgr>) -> Shaper {
    make_shaper(unicode, fallback, Wrap::DontWrapOrReorder)
}

use crate::unicode_shaper::unicode_bidi_run_iterator;

// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Checks of the `HarfBuzz` shapers through the run handler interface, with the empty typeface: it
//! has no tables and maps no code point, so the shapers run on the synthesized sfnt and every
//! glyph is glyph 0 (docs/design/modules.md §2.1).

use skia_rust_core::font::Font;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::point::Point;
use skia_rust_shaper::harfbuzz::{
    script_run_iterator, shape_dont_wrap_or_reorder, shape_then_wrap, shaper_driven_wrapper,
};
use skia_rust_shaper::run_iterator::{TrivialFontRunIterator, TrivialLanguageRunIterator};
use skia_rust_shaper::unicode_shaper::unicode_bidi_run_iterator;
use skia_rust_shaper::{Buffer, RunHandler, RunInfo, Shaper, best_available};
use skia_rust_unicode::unicode_icu;

#[derive(Default)]
struct Recorder {
    lines: usize,
    runs: Vec<(usize, usize, usize)>,
    glyphs: Vec<GlyphId>,
    positions: Vec<Point>,
    clusters: Vec<u32>,
}

impl RunHandler for Recorder {
    fn begin_line(&mut self) {
        self.lines += 1;
    }

    fn run_info(&mut self, _info: &RunInfo<'_>) {}

    fn commit_run_info(&mut self) {}

    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
        self.runs
            .push((info.utf8_range.start, info.utf8_range.end, info.glyph_count));
        self.glyphs = vec![7; info.glyph_count];
        self.positions = vec![Point::default(); info.glyph_count];
        self.clusters = vec![0; info.glyph_count];
        Buffer {
            glyphs: &mut self.glyphs,
            positions: &mut self.positions,
            offsets: None,
            clusters: Some(&mut self.clusters),
            point: Point::new(0.0, 0.0),
        }
    }

    fn commit_run_buffer(&mut self, _info: &RunInfo<'_>) {}

    fn commit_line(&mut self) {}
}

fn shape(shaper: &Shaper, text: &str, width: f32) -> Recorder {
    let unicode = unicode_icu::make().expect("a unicode");
    let utf8 = text.as_bytes();
    let font = Font::default();
    let mut font_runs = TrivialFontRunIterator::new(&font, utf8.len());
    let mut bidi = unicode_bidi_run_iterator(&*unicode, utf8, 0).expect("bidi");
    let mut script = script_run_iterator(utf8);
    let mut language = TrivialLanguageRunIterator::new("en-US", utf8.len());
    let mut recorder = Recorder::default();
    shaper.shape_with_iterators(
        utf8,
        &mut font_runs,
        &mut *bidi,
        &mut *script,
        &mut language,
        width,
        &mut recorder,
    );
    recorder
}

#[test]
fn every_shaper_reports_the_whole_text() {
    let unicode = unicode_icu::make().expect("a unicode");
    let shapers = [
        shaper_driven_wrapper(unicode.clone(), None),
        shape_then_wrap(unicode.clone(), None),
        shape_dont_wrap_or_reorder(unicode, None),
    ];
    for shaper in &shapers {
        let r = shape(shaper, "abc def", f32::MAX);
        assert_eq!(r.lines, 1);
        let covered: usize = r.runs.iter().map(|&(start, end, _)| end - start).sum();
        assert_eq!(covered, "abc def".len());
        // One glyph per code point: the empty typeface maps none, so each is glyph 0.
        assert_eq!(r.runs.iter().map(|r| r.2).sum::<usize>(), 7);
    }
}

#[test]
fn the_wrapping_shapers_break_at_the_line_break_opportunities() {
    let unicode = unicode_icu::make().expect("a unicode");
    // The empty typeface has no advances, so nothing overflows: only a line that cannot fit
    // anything at width 0 shows the break, and glyphs of zero width never overflow.
    for shaper in [
        shaper_driven_wrapper(unicode.clone(), None),
        shape_then_wrap(unicode, None),
    ] {
        let r = shape(&shaper, "foo bar", 0.0);
        assert!(r.lines >= 1);
        let covered: usize = r.runs.iter().map(|&(start, end, _)| end - start).sum();
        assert_eq!(covered, 7);
    }
}

#[test]
fn the_script_run_iterator_splits_at_a_change_of_script() {
    let text = "ab\u{3b1}\u{3b2}";
    let mut iter = script_run_iterator(text.as_bytes());
    iter.consume();
    assert_eq!(iter.end_of_current_run(), 2);
    assert_eq!(iter.current_script(), u32::from_be_bytes(*b"Latn"));
    assert!(!iter.at_end());
    iter.consume();
    assert_eq!(iter.end_of_current_run(), text.len());
    assert_eq!(iter.current_script(), u32::from_be_bytes(*b"Grek"));
    assert!(iter.at_end());
}

#[test]
fn best_available_is_the_harfbuzz_factory() {
    let factory = best_available();
    assert!(factory.unicode().is_some());
    let shaper = factory.make_shaper(None);
    let r = shape(&shaper, "abc", f32::MAX);
    assert_eq!(r.lines, 1);
    // The factory's iterators cover the text.
    let mut script = factory.make_script_run_iterator(b"abc", u32::from_be_bytes(*b"Latn"));
    script.consume();
    assert_eq!(script.end_of_current_run(), 3);
}

#[test]
fn purging_the_caches_keeps_shaping() {
    let unicode = unicode_icu::make().expect("a unicode");
    let shaper = shaper_driven_wrapper(unicode, None);
    let first = shape(&shaper, "abc", f32::MAX);
    Shaper::purge_caches();
    let second = shape(&shaper, "abc", f32::MAX);
    assert_eq!(first.runs, second.runs);
}

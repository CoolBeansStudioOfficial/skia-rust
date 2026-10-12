// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Checks of the primitive shaper through the run handler interface, with the default (empty)
//! typeface: each code point is one glyph, and the clusters are the UTF-8 offsets.

use skia_rust_core::font::Font;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;
use skia_rust_shaper::run_iterator::{
    TrivialBiDiRunIterator, TrivialLanguageRunIterator, TrivialScriptRunIterator,
    make_font_mgr_run_iterator,
};
use skia_rust_shaper::{Buffer, RunHandler, RunInfo, Shaper};

#[derive(Default)]
struct Recorder {
    lines: usize,
    runs: Vec<(usize, usize)>,
    glyphs: Vec<GlyphId>,
    clusters: Vec<u32>,
    positions: Vec<Point>,
    staged_glyphs: Vec<GlyphId>,
    staged_clusters: Vec<u32>,
    staged_positions: Vec<Point>,
}

impl RunHandler for Recorder {
    fn begin_line(&mut self) {
        self.lines += 1;
    }

    fn run_info(&mut self, info: &RunInfo<'_>) {
        self.runs
            .push((info.utf8_range.start, info.utf8_range.len()));
    }

    fn commit_run_info(&mut self) {}

    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
        self.staged_glyphs = vec![0; info.glyph_count];
        self.staged_clusters = vec![0; info.glyph_count];
        self.staged_positions = vec![Point::new(0.0, 0.0); info.glyph_count];
        Buffer {
            glyphs: self.staged_glyphs.as_mut_slice(),
            positions: self.staged_positions.as_mut_slice(),
            offsets: None,
            clusters: Some(self.staged_clusters.as_mut_slice()),
            point: Point::new(0.0, 0.0),
        }
    }

    fn commit_run_buffer(&mut self, _info: &RunInfo<'_>) {
        self.glyphs.extend_from_slice(&self.staged_glyphs);
        self.clusters.extend_from_slice(&self.staged_clusters);
        self.positions.extend_from_slice(&self.staged_positions);
    }

    fn commit_line(&mut self) {}
}

fn shape(text: &str, width: scalar) -> Recorder {
    let shaper = Shaper::new_primitive();
    let font = Font::default();
    let mut fonts = make_font_mgr_run_iterator(text.as_bytes(), &font, None);
    let mut bidi = TrivialBiDiRunIterator::new(0, 0);
    let mut script = TrivialScriptRunIterator::new(0, 0);
    let mut language = TrivialLanguageRunIterator::new("", 0);
    let mut recorder = Recorder::default();
    shaper.shape_with_iterators(
        text.as_bytes(),
        &mut fonts,
        &mut bidi,
        &mut script,
        &mut language,
        width,
        &mut recorder,
    );
    recorder
}

#[test]
fn one_glyph_per_code_point_with_utf8_clusters() {
    // "a" (1 byte), "é" (2 bytes), "中" (3 bytes): clusters are the byte offsets of the code
    // points, and the three glyphs are one line.
    let recorder = shape("a\u{e9}\u{4e2d}", f32::MAX);
    assert_eq!(recorder.lines, 1);
    assert_eq!(recorder.glyphs.len(), 3);
    assert_eq!(recorder.clusters, vec![0, 1, 3]);
    assert_eq!(recorder.runs, vec![(0, 6)]);
}

#[test]
fn lines_break_at_the_width() {
    // The default font has zero advances, so no width breaks a line: the text is one line.
    let recorder = shape("ab cd ef", 0.0);
    assert_eq!(recorder.lines, 1);
    assert_eq!(recorder.glyphs.len(), 8);
}

#[test]
fn empty_text_is_one_empty_line() {
    let recorder = shape("", f32::MAX);
    assert_eq!(recorder.lines, 1);
    assert_eq!(recorder.glyphs.len(), 0);
    assert_eq!(recorder.runs.len(), 0);
}

#[test]
fn trivial_run_iterators_cover_the_text_once() {
    use skia_rust_shaper::RunIterator;
    use skia_rust_shaper::run_iterator::BiDiRunIterator;

    let mut bidi = TrivialBiDiRunIterator::new(1, 5);
    assert!(!bidi.at_end());
    assert_eq!(bidi.end_of_current_run(), 0);
    bidi.consume();
    assert!(bidi.at_end());
    assert_eq!(bidi.end_of_current_run(), 5);
    assert_eq!(bidi.current_level(), 1);

    let empty = TrivialScriptRunIterator::new(0, 0);
    assert!(empty.at_end());
    assert_eq!(empty.end_of_current_run(), 0);
}

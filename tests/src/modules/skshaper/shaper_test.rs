// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skshaper/tests/ShaperTest.cpp (chrome/m156)

#![cfg(test)]
// The local classes of `Shaper_LineBreaks` stay local, as in C++; the widths are small integers.
#![allow(clippy::items_after_statements, clippy::cast_precision_loss)]

use std::ops::Range;
use std::sync::Arc;

use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_types::{FourByteTag, GlyphId, set_four_byte_tag};
use skia_rust_core::point::Point;
use skia_rust_shaper::harfbuzz;
use skia_rust_shaper::run_iterator::{
    RunIterator, ScriptRunIterator, TrivialBiDiRunIterator, TrivialFontRunIterator,
    TrivialLanguageRunIterator, TrivialScriptRunIterator, make_font_mgr_run_iterator,
    make_std_language_run_iterator,
};
use skia_rust_shaper::unicode_shaper::unicode_bidi_run_iterator;
use skia_rust_shaper::{Buffer, RunHandler, RunInfo};
use skia_rust_tools::font_tool_utils::default_font;
use skia_rust_unicode::unicode::Unicode;
use skia_rust_unicode::unicode_icu;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, errorf, reporter_assert};

// Port of: modules/skshaper/tests/ShaperTest.cpp#L47-L63 (chrome/m156)
fn get_unicode() -> Option<Arc<dyn Unicode>> {
    // `SkUnicodes::ICU::Make()`, then `SkUnicodes::ICU4X::Make()`.
    unicode_icu::make().or_else(unicode_icu::make_icu4x)
}

// Port of: modules/skshaper/tests/ShaperTest.cpp#L65-L163 (chrome/m156)
struct TestRunHandler<'a> {
    resource: &'a str,
    reporter: &'a mut Reporter,
    utf8: &'a [u8],
    glyphs: Vec<GlyphId>,
    positions: Vec<Point>,
    clusters: Vec<u32>,
    range: Range<usize>,
    glyph_count: usize,

    begin_line: bool,
    commit_run_info: bool,
    commit_line: bool,
}

impl<'a> TestRunHandler<'a> {
    fn new(resource: &'a str, reporter: &'a mut Reporter, utf8: &'a [u8]) -> Self {
        Self {
            resource,
            reporter,
            utf8,
            glyphs: Vec::new(),
            positions: Vec::new(),
            clusters: Vec::new(),
            range: 0..0,
            glyph_count: 0,
            begin_line: false,
            commit_run_info: false,
            commit_line: false,
        }
    }
}

impl RunHandler for TestRunHandler<'_> {
    fn begin_line(&mut self) {
        self.begin_line = true;
    }

    fn run_info(&mut self, _info: &RunInfo<'_>) {}

    fn commit_run_info(&mut self) {
        self.commit_run_info = true;
    }

    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
        self.glyph_count = info.glyph_count;
        self.range = info.utf8_range.clone();
        self.glyphs = vec![0; info.glyph_count];
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

    fn commit_run_buffer(&mut self, info: &RunInfo<'_>) {
        let resource = self.resource;
        reporter_assert!(
            self.reporter,
            self.glyph_count == info.glyph_count,
            "{resource}"
        );
        reporter_assert!(
            self.reporter,
            self.range.start == info.utf8_range.start,
            "{resource}"
        );
        reporter_assert!(
            self.reporter,
            self.range.len() == info.utf8_range.len(),
            "{resource}"
        );
        if self.range.start + self.range.len() > self.utf8.len() {
            reporter_assert!(
                self.reporter,
                self.range.start + self.range.len() <= self.utf8.len(),
                "{resource}"
            );
            return;
        }

        // The `if ((false))` block of the C++ prints the run; it never runs.

        for i in 0..self.glyph_count {
            let cluster = self.clusters[i];
            let (begin, end) = (self.range.start, self.range.end);
            let glyph_count = self.glyph_count;
            reporter_assert!(
                self.reporter,
                cluster as usize >= begin,
                "{cluster} >= {begin} {resource} i:{i} glyphCount:{glyph_count}"
            );
            reporter_assert!(
                self.reporter,
                (cluster as usize) < end,
                "{cluster} < {end} {resource} i:{i} glyphCount:{glyph_count}"
            );
        }
    }

    fn commit_line(&mut self) {
        self.commit_line = true;
    }
}

// Port of: modules/skshaper/tests/ShaperTest.cpp#L166-L233 (chrome/m156)
fn shaper_test(reporter: &mut Reporter, name: &str, data: &[u8]) {
    // skiatest::ReporterContext context(reporter, name);
    let saved_context = reporter.context().map(str::to_owned);
    reporter.set_context(Some(name.to_owned()));
    shaper_test_body(reporter, name, data);
    reporter.set_context(saved_context);
}

fn shaper_test_body(reporter: &mut Reporter, name: &str, data: &[u8]) {
    let Some(unicode) = get_unicode() else {
        errorf!(reporter, "Could not create unicode.");
        return;
    };

    let shaper = harfbuzz::shaper_driven_wrapper(unicode.clone(), Some(FontMgr::empty())); // no fallback
    const K_WIDTH: f32 = 400.0;
    let font = default_font();
    let utf8 = data;

    let mut rh = TestRunHandler::new(name, reporter, utf8);

    const DEFAULT_LEVEL: u8 = 0; // SkBidiIterator::kLTR
    let mut bidi =
        unicode_bidi_run_iterator(&*unicode, utf8, DEFAULT_LEVEL).expect("SkASSERT(bidi) failed");

    let mut language = make_std_language_run_iterator(utf8.len());

    let mut script = harfbuzz::script_run_iterator(utf8);

    let mut font_runs = make_font_mgr_run_iterator(utf8, &font, Some(FontMgr::empty()));
    shaper.shape_with_iterators(
        utf8,
        &mut font_runs,
        &mut *bidi,
        &mut *script,
        &mut language,
        K_WIDTH,
        &mut rh,
    );

    // Even on empty input, expect that the line is started, that the zero run infos are committed,
    // and the empty line is committed. This allows the user to properly handle empty runs.
    reporter_assert!(rh.reporter, rh.begin_line);
    reporter_assert!(rh.reporter, rh.commit_run_info);
    reporter_assert!(rh.reporter, rh.commit_line);

    let latn = set_four_byte_tag(b'l', b'a', b't', b'n');
    let mut font_iterator = TrivialFontRunIterator::new(&font, data.len());
    let mut bidi_iterator = TrivialBiDiRunIterator::new(0, data.len());
    let mut script_iterator = TrivialScriptRunIterator::new(latn, data.len());
    let mut language_iterator = TrivialLanguageRunIterator::new("en-US", data.len());
    shaper.shape_with_iterators(
        data,
        &mut font_iterator,
        &mut bidi_iterator,
        &mut script_iterator,
        &mut language_iterator,
        K_WIDTH,
        &mut rh,
    );
}

// Port of: modules/skshaper/tests/ShaperTest.cpp#L235-L245 (chrome/m156)
fn cluster_test(reporter: &mut Reporter, resource: &str) {
    let Some(data) = get_resource_as_data(resource) else {
        errorf!(reporter, "Could not get resource {resource}.");
        return;
    };

    shaper_test(reporter, resource, &data);
}

// Port of: modules/skshaper/tests/ShaperTest.cpp#L238 (chrome/m156)
def_test!(Shaper_cluster_empty, |reporter| {
    shaper_test(reporter, "empty", &[]);
});

// Port of: modules/skshaper/tests/ShaperTest.cpp#L240-L275 (chrome/m156), `SHAPER_TEST(X)`: Skia
// registers `Shaper_cluster_X`; the manifest names the entries `X`.
macro_rules! shaper_test {
    ($($name:ident),+ $(,)?) => {
        $(
            def_test!($name, |reporter| {
                cluster_test(reporter, concat!("text/", stringify!($name), ".txt"));
            });
        )+
    };
}

shaper_test!(
    arabic,
    armenian,
    balinese,
    buginese,
    cherokee,
    cyrillic,
    emoji,
    english,
    ethiopic,
    greek,
    hangul,
    han_simplified,
    han_traditional,
    hebrew,
    javanese,
    kana,
    lao,
    mandaic,
    newtailue,
    nko,
    sinhala,
    sundanese,
    syriac,
    thaana,
    thai,
    tibetan,
    tifnagh,
    vai,
    bengali,
    devanagari,
    khmer,
    myanmar,
    taitham,
    tamil,
);

// Port of: modules/skshaper/tests/ShaperTest.cpp#L277-L477 (chrome/m156), `Shaper_LineBreaks`.
def_test!(Shaper_LineBreaks, |reporter| {
    let Some(unicode) = get_unicode() else {
        return;
    };

    let shaper = harfbuzz::shaper_driven_wrapper(unicode, Some(FontMgr::empty()));

    let font = default_font();

    struct CustomScriptRunIterator {
        boundaries: Vec<usize>,
        total: usize,
        current: usize,
        index: usize,
        at_end: bool,
        script: FourByteTag,
    }

    impl CustomScriptRunIterator {
        fn new(boundaries: &[usize], total: usize, script: FourByteTag) -> Self {
            Self {
                boundaries: boundaries.to_vec(),
                total,
                current: 0,
                index: 0,
                at_end: false,
                script,
            }
        }
    }

    impl RunIterator for CustomScriptRunIterator {
        fn consume(&mut self) {
            if self.index < self.boundaries.len() {
                self.current = self.boundaries[self.index];
                self.index += 1;
            } else {
                self.current = self.total;
                self.at_end = true;
            }
        }
        fn end_of_current_run(&self) -> usize {
            self.current
        }
        fn at_end(&self) -> bool {
            self.at_end
        }
    }

    impl ScriptRunIterator for CustomScriptRunIterator {
        fn current_script(&self) -> FourByteTag {
            self.script
        }
    }

    // `start` and `end` are recorded and never read, as in C++.
    #[allow(dead_code)]
    struct Run {
        start: usize,
        end: usize,
    }
    struct Line {
        start: usize,
        runs: Vec<Run>,
    }
    #[derive(Default)]
    struct LineBreakTracker {
        lines: Vec<Line>,
        current_line_start: usize,
        glyphs: Vec<GlyphId>,
        pos: Vec<Point>,
    }

    impl RunHandler for LineBreakTracker {
        fn begin_line(&mut self) {
            self.lines.push(Line {
                start: self.current_line_start,
                runs: Vec::new(),
            });
        }
        fn run_info(&mut self, _info: &RunInfo<'_>) {}
        fn commit_run_info(&mut self) {}
        fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
            self.glyphs = vec![0; info.glyph_count.max(100)];
            self.pos = vec![Point::default(); info.glyph_count.max(100)];
            self.lines
                .last_mut()
                .expect("beginLine was called")
                .runs
                .push(Run {
                    start: info.utf8_range.start,
                    end: info.utf8_range.end,
                });
            Buffer {
                glyphs: &mut self.glyphs,
                positions: &mut self.pos,
                offsets: None,
                clusters: None,
                point: Point::new(0.0, 0.0),
            }
        }
        fn commit_run_buffer(&mut self, info: &RunInfo<'_>) {
            self.current_line_start = info.utf8_range.end;
        }
        fn commit_line(&mut self) {}
    }

    let check_line_breaks = |reporter: &mut Reporter,
                             txt: &str,
                             script_boundaries: &[usize],
                             expected_line_breaks: &[usize]| {
        let len = txt.len();
        let mut breaks: Vec<usize> = Vec::new();

        let mut previous_line_count = usize::MAX;
        for i in 0..100usize {
            let width = 5.0f32 + (i * 5) as f32;

            let mut font_iter = TrivialFontRunIterator::new(&font, len);
            let mut bidi = TrivialBiDiRunIterator::new(0, len);
            let mut script_iter = CustomScriptRunIterator::new(
                script_boundaries,
                len,
                set_four_byte_tag(b'L', b'a', b't', b'n'),
            );
            let mut lang = TrivialLanguageRunIterator::new("en-US", len);

            let mut tracker = LineBreakTracker::default();
            shaper.shape_with_iterators(
                txt.as_bytes(),
                &mut font_iter,
                &mut bidi,
                &mut script_iter,
                &mut lang,
                width,
                &mut tracker,
            );

            // Line counts should be monotonic.
            let line_count = tracker.lines.len();
            reporter_assert!(
                reporter,
                line_count <= previous_line_count,
                "Line count should be monotonic. Width: {width:.6}, Line count: {line_count}, Prev: {previous_line_count}"
            );
            previous_line_count = line_count;

            for line in &tracker.lines {
                if line.start > 0 {
                    if i == 0 {
                        // The first iteration (minimal width) is expected to produce the maximal
                        // line break set - so we just collect them here.
                        breaks.push(line.start);
                    } else {
                        // Subsequent iterations are expected to produce a subset.
                        let start = line.start;
                        reporter_assert!(
                            reporter,
                            breaks.contains(&line.start),
                            "Break at offset {start} for width {width:.6} is unexpected for '{txt}'!"
                        );
                    }
                }
            }
        }

        reporter_assert!(
            reporter,
            previous_line_count == 1,
            "Text '{txt}' should fit on a single line at max width, but had {previous_line_count} lines"
        );

        reporter_assert!(
            reporter,
            breaks == expected_line_breaks,
            "Observed breaks did not match expected breaks for: '{txt}'"
        );
    };

    struct Case {
        text: &'static str,
        script_boundaries: &'static [usize],
        expected_line_breaks: &'static [usize],
    }
    macro_rules! case {
        ($text:expr, [$($b:expr),*], [$($e:expr),*]) => {
            Case { text: $text, script_boundaries: &[$($b),*], expected_line_breaks: &[$($e),*] }
        };
    }
    let g_tests: &[Case] = &[
        case!("foo", [], []),
        case!("foo", [1], []),
        case!("foo", [2], []),
        case!("foobar", [1, 2, 3, 4, 5], []),
        case!("foo bar", [], [4]),
        case!("foo bar", [1], [4]),
        case!("foo bar", [2], [4]),
        case!("foo bar", [3], [4]),
        case!("foo bar", [4], [4]),
        case!("foo bar", [5], [4]),
        case!("foo bar", [6], [4]),
        case!("foo bar", [1, 2, 3, 4, 5, 6], [4]),
        case!("foo, bar baz", [], [5, 9]),
        case!("foo, bar baz", [6, 7], [5, 9]),
        case!("foo, bar baz", [2, 10], [5, 9]),
        case!("foo, bar baz", [3, 10], [5, 9]),
        case!("foo, bar baz", [3, 11], [5, 9]),
        case!("foo, bar baz", [2, 11], [5, 9]),
        case!("foo, bar baz", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], [5, 9]),
        case!("abra cadabra", [], [5]),
        case!("abra cadabra", [1, 2, 3], [5]),
        case!("abra cadabra", [1, 2, 4], [5]),
        case!("abra cadabra", [1, 2, 5], [5]),
        case!("abra cadabra", [1, 2, 6], [5]),
        case!("abra cadabra", [1, 2, 7], [5]),
        case!("abra cadabra", [1, 2, 8], [5]),
        case!("abra cadabra", [1, 2, 9], [5]),
        case!("abra cadabra", [1, 2, 10], [5]),
        case!("abra cadabra", [1, 2, 11], [5]),
        case!("abra cadabra", [1, 3, 11], [5]),
        case!("abra cadabra", [1, 4, 11], [5]),
        case!("abra cadabra", [1, 5, 11], [5]),
        case!("abra cadabra", [1, 6, 11], [5]),
        case!("abra cadabra", [1, 7, 11], [5]),
        case!("abra cadabra", [1, 8, 11], [5]),
        case!("abra cadabra", [1, 9, 11], [5]),
        case!("abra cadabra", [1, 10, 11], [5]),
        case!("abra cadabra", [2, 10, 11], [5]),
        case!("abra cadabra", [3, 10, 11], [5]),
        case!("abra cadabra", [4, 10, 11], [5]),
        case!("abra cadabra", [5, 10, 11], [5]),
        case!("abra cadabra", [6, 10, 11], [5]),
        case!("abra cadabra", [7, 10, 11], [5]),
        case!("abra cadabra", [8, 10, 11], [5]),
        case!("abra cadabra", [9, 10, 11], [5]),
    ];

    for test in g_tests {
        check_line_breaks(
            reporter,
            test.text,
            test.script_boundaries,
            test.expected_line_breaks,
        );
    }

    {
        const K_TEXT: &str = "abra cadabra";
        let expected_breaks = [5usize];
        let k_len = K_TEXT.len();

        // exhaustive combinatorics for 3 runs
        for i0 in 1..k_len - 2 {
            for i1 in i0 + 1..k_len - 1 {
                check_line_breaks(reporter, K_TEXT, &[i0, i1], &expected_breaks);
            }
        }
    }
});

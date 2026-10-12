// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skottie/tests/Shaper.cpp (chrome/m156)
//
// `Skottie_Shaper_CTStrict` is a CoreText test (`SK_SHAPER_CORETEXT_AVAILABLE`): excluded.
// The tests under `#if defined(SK_SHAPER_HARFBUZZ_AVAILABLE) && !defined(SK_BUILD_FOR_WIN)` run on
// every host here: Skia leaves them out of its Windows builds for the native Windows fonts, which
// skia-rust does not use.

#![cfg(test)]
// The local classes of the tests stay local, as in C++.
#![allow(clippy::items_after_statements)]
// REPORTER_ASSERT of float comparisons, as in the C++ (NaN fails the assert either way).
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use skia_rust_core::data::Data;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_mgr::{FontMgr, FontMgrBase, FontStyleSet};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;
use skia_rust_core::utils::text_utils::Align;
use skia_rust_shaper::best_available;
use skia_rust_skottie::internal::text::unicode::make_intersecting_break_iterator_for_testing;
use skia_rust_skottie::text_shaper::{
    Capitalization, Direction, Flags, LinebreakPolicy, ResizePolicy, Shaper, TextDesc, VAlign,
};
use skia_rust_tools::font_tool_utils::{
    create_portable_typeface, default_portable_typeface, default_typeface, test_font_mgr,
};
use skia_rust_unicode::unicode::{BreakIterator, BreakPosition, BreakStatus};

use crate::{Reporter, def_font_test, def_test, reporter_assert};

// Port of: modules/skottie/tests/Shaper.cpp#L26-L76 (chrome/m156)
def_font_test!(Skottie_Shaper_Clusters, |r| {
    let text = "Foo \rbar \rBaz.";

    fn check_clusters(r: &mut Reporter, text: &str, flags: Flags, expected_clusters: &[usize]) {
        let desc = TextDesc {
            typeface: Some(create_portable_typeface(
                Some("Serif"),
                FontStyle::default(),
            )),
            text_size: 18.0,
            min_text_size: 0.0,
            max_text_size: 18.0,
            line_height: 18.0,
            line_shift: 0.0,
            ascent: 0.0,
            h_align: Align::Center,
            v_align: VAlign::Top,
            resize: ResizePolicy::None,
            linebreak: LinebreakPolicy::Paragraph,
            direction: Direction::LTR,
            capitalization: Capitalization::None,
            max_lines: 0,
            flags,
            locale: None,
            font_family: None,
            text_tracking: 0.0,
        };
        let result = Shaper::shape_in_box(
            text,
            &desc,
            &Rect::from_wh(1000.0, 1000.0),
            Some(&test_font_mgr()),
            &best_available(),
        );
        reporter_assert!(r, !result.fragments.is_empty());

        let mut i = 0;
        for frag in &result.fragments {
            let glyphs = &frag.glyphs;

            if flags.contains(Flags::CLUSTERS) {
                reporter_assert!(r, glyphs.clusters.len() == glyphs.glyph_ids.len());
            }

            for &utf_cluster in &glyphs.clusters {
                reporter_assert!(r, i < expected_clusters.len());
                // C++ reads `expected_clusters[i++]` past the end after a failed assert.
                if i < expected_clusters.len() {
                    reporter_assert!(r, utf_cluster == expected_clusters[i]);
                }
                i += 1;
            }
        }

        reporter_assert!(r, i == expected_clusters.len());
    }

    check_clusters(r, text, Flags::empty(), &[]);
    check_clusters(r, text, Flags::FRAGMENT_GLYPHS, &[]);
    check_clusters(
        r,
        text,
        Flags::CLUSTERS,
        &[0, 1, 2, 3, 5, 6, 7, 8, 10, 11, 12, 13],
    );
    check_clusters(
        r,
        text,
        Flags::CLUSTERS | Flags::FRAGMENT_GLYPHS,
        &[0, 1, 2, 3, 5, 6, 7, 8, 10, 11, 12, 13],
    );
});

// Port of: modules/skottie/tests/Shaper.cpp#L78-L149 (chrome/m156)
def_font_test!(Skottie_Shaper_HAlign, |reporter| {
    let typeface = default_typeface();
    // REPORTER_ASSERT(reporter, typeface): skia-rust: not expressible in Rust: a typeface is
    // never null.

    struct TestSize {
        text_size: f32,
        tolerance: f32,
    }
    // These gross tolerances are required for the test to pass on NativeFonts bots.
    // Might be worth investigating why we need so much slack.
    let test_sizes = [
        TestSize {
            text_size: 5.0,
            tolerance: 2.0,
        },
        TestSize {
            text_size: 10.0,
            tolerance: 2.0,
        },
        TestSize {
            text_size: 15.0,
            tolerance: 2.4,
        },
        TestSize {
            text_size: 25.0,
            tolerance: 4.4,
        },
    ];

    struct TestAlign {
        align: Align,
        l_selector: f32,
        r_selector: f32,
    }
    let test_aligns = [
        TestAlign {
            align: Align::Left,
            l_selector: 0.0,
            r_selector: 1.0,
        },
        TestAlign {
            align: Align::Center,
            l_selector: 0.5,
            r_selector: 0.5,
        },
        TestAlign {
            align: Align::Right,
            l_selector: 1.0,
            r_selector: 0.0,
        },
    ];

    let text = "Foo, bar.\rBaz.";
    let text_point = Point::new(100.0, 100.0);

    for tsize in &test_sizes {
        for talign in &test_aligns {
            let desc = TextDesc {
                typeface: Some(typeface.clone()),
                text_size: tsize.text_size,
                min_text_size: 0.0,
                max_text_size: tsize.text_size,
                line_height: tsize.text_size,
                line_shift: 0.0,
                ascent: 0.0,
                h_align: talign.align,
                v_align: VAlign::TopBaseline,
                resize: ResizePolicy::None,
                linebreak: LinebreakPolicy::Explicit,
                direction: Direction::LTR,
                capitalization: Capitalization::None,
                max_lines: 0,
                flags: Flags::empty(),
                locale: None,
                font_family: None,
                text_tracking: 0.0,
            };

            let shape_result = Shaper::shape_at_point(
                text,
                &desc,
                text_point,
                Some(&test_font_mgr()),
                &best_available(),
            );
            reporter_assert!(reporter, shape_result.fragments.len() == 1);
            reporter_assert!(reporter, !shape_result.fragments[0].glyphs.runs.is_empty());

            let shape_bounds = shape_result.compute_visual_bounds();
            reporter_assert!(reporter, !shape_bounds.is_empty());

            let expected_l = text_point.x - shape_bounds.width() * talign.l_selector;
            reporter_assert!(
                reporter,
                (shape_bounds.left() - expected_l).abs() < tsize.tolerance,
                "{} {} {} {} {:?}",
                shape_bounds.left(),
                expected_l,
                tsize.tolerance,
                tsize.text_size,
                talign.align
            );

            let expected_r = text_point.x + shape_bounds.width() * talign.r_selector;
            reporter_assert!(
                reporter,
                (shape_bounds.right() - expected_r).abs() < tsize.tolerance,
                "{} {} {} {} {:?}",
                shape_bounds.right(),
                expected_r,
                tsize.tolerance,
                tsize.text_size,
                talign.align
            );
        }
    }
});

// Port of: modules/skottie/tests/Shaper.cpp#L151-L222 (chrome/m156)
def_font_test!(Skottie_Shaper_VAlign, |reporter| {
    let typeface = default_typeface();
    // REPORTER_ASSERT(reporter, typeface): skia-rust: not expressible in Rust: a typeface is
    // never null.

    struct TestSize {
        text_size: f32,
        tolerance: f32,
    }
    // These gross tolerances are required for the test to pass on NativeFonts bots.
    // Might be worth investigating why we need so much slack.
    let test_sizes = [
        TestSize {
            text_size: 5.0,
            tolerance: 2.0,
        },
        TestSize {
            text_size: 10.0,
            tolerance: 4.0,
        },
        TestSize {
            text_size: 15.0,
            tolerance: 5.5,
        },
        TestSize {
            text_size: 25.0,
            tolerance: 8.0,
        },
    ];

    struct TestAlign {
        align: VAlign,
        top_factor: f32,
    }
    let test_aligns = [
        TestAlign {
            align: VAlign::HybridTop,
            top_factor: 0.0,
        },
        TestAlign {
            align: VAlign::HybridCenter,
            top_factor: 0.5,
        },
        // TODO: any way to test kTopBaseline?
    ];

    let text = "Foo, bar.\rBaz.";
    let text_box = Rect::from_xywh(100.0, 100.0, 1000.0, 1000.0); // large-enough to avoid breaks.

    for tsize in &test_sizes {
        for talign in &test_aligns {
            let desc = TextDesc {
                typeface: Some(typeface.clone()),
                text_size: tsize.text_size,
                min_text_size: 0.0,
                max_text_size: tsize.text_size,
                line_height: tsize.text_size,
                line_shift: 0.0,
                ascent: 0.0,
                h_align: Align::Center,
                v_align: talign.align,
                resize: ResizePolicy::None,
                linebreak: LinebreakPolicy::Paragraph,
                direction: Direction::LTR,
                capitalization: Capitalization::None,
                max_lines: 0,
                flags: Flags::empty(),
                locale: None,
                font_family: None,
                text_tracking: 0.0,
            };

            let shape_result = Shaper::shape_in_box(
                text,
                &desc,
                &text_box,
                Some(&test_font_mgr()),
                &best_available(),
            );
            reporter_assert!(reporter, shape_result.fragments.len() == 1);
            reporter_assert!(reporter, !shape_result.fragments[0].glyphs.runs.is_empty());

            let shape_bounds = shape_result.compute_visual_bounds();
            reporter_assert!(reporter, !shape_bounds.is_empty());

            let v_diff = text_box.height() - shape_bounds.height();

            let expected_t = text_box.top() + v_diff * talign.top_factor;
            reporter_assert!(
                reporter,
                (shape_bounds.top() - expected_t).abs() < tsize.tolerance,
                "{} {} {} {} {}",
                shape_bounds.top(),
                expected_t,
                tsize.tolerance,
                tsize.text_size,
                talign.align as u32
            );

            let expected_b = text_box.bottom() - v_diff * (1.0 - talign.top_factor);
            reporter_assert!(
                reporter,
                (shape_bounds.bottom() - expected_b).abs() < tsize.tolerance,
                "{} {} {} {} {}",
                shape_bounds.bottom(),
                expected_b,
                tsize.tolerance,
                tsize.text_size,
                talign.align as u32
            );
        }
    }
});

// Port of: modules/skottie/tests/Shaper.cpp#L224-L266 (chrome/m156)
def_font_test!(Skottie_Shaper_FragmentGlyphs, |reporter| {
    let mut desc = TextDesc {
        typeface: Some(default_typeface()),
        text_size: 18.0,
        min_text_size: 0.0,
        max_text_size: 18.0,
        line_height: 18.0,
        line_shift: 0.0,
        ascent: 0.0,
        h_align: Align::Center,
        v_align: VAlign::Top,
        resize: ResizePolicy::None,
        linebreak: LinebreakPolicy::Paragraph,
        direction: Direction::LTR,
        capitalization: Capitalization::None,
        max_lines: 0,
        flags: Flags::empty(),
        locale: None,
        font_family: None,
        text_tracking: 0.0,
    };

    let text = "Foo bar baz";
    let text_box = Rect::from_wh(100.0, 100.0);

    {
        let shape_result = Shaper::shape_in_box(
            text,
            &desc,
            &text_box,
            Some(&test_font_mgr()),
            &best_available(),
        );
        // Default/consolidated mode => single blob result.
        reporter_assert!(reporter, shape_result.fragments.len() == 1);
        debug_assert!(!shape_result.fragments.is_empty());
        reporter_assert!(reporter, !shape_result.fragments[0].glyphs.runs.is_empty());
    }

    {
        desc.flags = Flags::FRAGMENT_GLYPHS;
        let shape_result = Shaper::shape_in_box(
            text,
            &desc,
            &text_box,
            Some(&test_font_mgr()),
            &best_available(),
        );
        // Fragmented mode => one blob per glyph.
        let expected_size = text.len();
        reporter_assert!(reporter, shape_result.fragments.len() == expected_size);
        debug_assert!(!shape_result.fragments.is_empty());
        for i in 0..expected_size.min(shape_result.fragments.len()) {
            reporter_assert!(reporter, !shape_result.fragments[i].glyphs.runs.is_empty());
        }
    }
});

// Port of: modules/skottie/tests/Shaper.cpp#L270-L361 (chrome/m156)
def_test!(Skottie_Shaper_ExplicitFontMgr, |reporter| {
    #[derive(Debug, Default)]
    struct CountingFontMgr {
        fallback_count: AtomicUsize,
    }

    impl CountingFontMgr {
        fn fallback_count(&self) -> usize {
            self.fallback_count.load(Ordering::Relaxed)
        }
    }

    impl FontMgrBase for CountingFontMgr {
        fn on_count_families(&self) -> usize {
            0
        }
        fn on_get_family_name(&self, _index: usize) -> String {
            unreachable!("onGetFamilyName called with bad index");
        }
        fn on_create_style_set(&self, _index: usize) -> Option<FontStyleSet> {
            unreachable!("onCreateStyleSet called with bad index");
        }
        fn on_match_family(&self, _family_name: Option<&str>) -> Option<FontStyleSet> {
            Some(FontStyleSet::create_empty())
        }

        fn on_match_family_style(
            &self,
            _family_name: Option<&str>,
            _style: &FontStyle,
        ) -> Option<Typeface> {
            None
        }
        fn on_match_family_style_character(
            &self,
            _family_name: Option<&str>,
            _style: &FontStyle,
            _bcp47: &[&str],
            _character: Unichar,
        ) -> Option<Typeface> {
            self.fallback_count.fetch_add(1, Ordering::Relaxed);
            None
        }

        fn on_make_from_data(&self, _data: &Data, _tt_index: i32) -> Option<Typeface> {
            None
        }
        fn on_make_from_stream_index(
            &self,
            _stream: Box<dyn StreamAsset>,
            _tt_index: i32,
        ) -> Option<Typeface> {
            None
        }
        fn on_make_from_stream_args(
            &self,
            _stream: Box<dyn StreamAsset>,
            _args: &FontArguments<'_, '_>,
        ) -> Option<Typeface> {
            None
        }
        fn on_make_from_file(&self, _path: &str, _tt_index: i32) -> Option<Typeface> {
            None
        }
        fn on_legacy_make_typeface(
            &self,
            _family_name: Option<&str>,
            _style: FontStyle,
        ) -> Option<Typeface> {
            None
        }
    }

    let counting = Arc::new(CountingFontMgr::default());
    let fontmgr = FontMgr::new(counting.clone());

    let desc = TextDesc {
        typeface: Some(default_portable_typeface()),
        text_size: 18.0,
        min_text_size: 0.0,
        max_text_size: 18.0,
        line_height: 18.0,
        line_shift: 0.0,
        ascent: 0.0,
        h_align: Align::Center,
        v_align: VAlign::Top,
        resize: ResizePolicy::None,
        linebreak: LinebreakPolicy::Paragraph,
        direction: Direction::LTR,
        capitalization: Capitalization::None,
        max_lines: 0,
        flags: Flags::empty(),
        locale: None,
        font_family: None,
        text_tracking: 0.0,
    };

    let text_box = Rect::from_wh(100.0, 100.0);

    {
        let shape_result = Shaper::shape_in_box(
            "foo bar",
            &desc,
            &text_box,
            Some(&fontmgr),
            &best_available(),
        );

        reporter_assert!(reporter, shape_result.fragments.len() == 1);
        reporter_assert!(reporter, !shape_result.fragments[0].glyphs.runs.is_empty());
        reporter_assert!(reporter, counting.fallback_count() == 0);
        reporter_assert!(reporter, shape_result.missing_glyph_count == 0);
    }

    {
        // An unassigned codepoint should trigger fallback.
        let shape_result = Shaper::shape_in_box(
            "foo\u{DFFFF}bar",
            &desc,
            &text_box,
            Some(&fontmgr),
            &best_available(),
        );

        reporter_assert!(reporter, shape_result.fragments.len() == 1);
        reporter_assert!(reporter, !shape_result.fragments[0].glyphs.runs.is_empty());
        reporter_assert!(reporter, counting.fallback_count() == 1);
        reporter_assert!(reporter, shape_result.missing_glyph_count == 1);
    }
});

// Port of: modules/skottie/tests/Shaper.cpp#L368-L495 (chrome/m156)
def_test!(Skottie_Shaper_breakiter, |r| {
    struct MockBreakIterator {
        breaks: Vec<BreakPosition>,
        current: BreakPosition,
        done: bool,
        index: usize,
    }

    impl MockBreakIterator {
        fn new(breaks: Vec<BreakPosition>) -> Self {
            Self {
                breaks,
                current: 0,
                done: false,
                index: 0,
            }
        }

        fn reset(&mut self) {
            self.current = 0;
            self.index = 0;
            self.done = false;
        }
    }

    impl BreakIterator for MockBreakIterator {
        fn first(&mut self) -> BreakPosition {
            self.reset();
            self.current
        }

        fn current(&mut self) -> BreakPosition {
            self.current
        }
        fn next(&mut self) -> BreakPosition {
            if self.index < self.breaks.len() {
                self.current = self.breaks[self.index];
                self.index += 1;
            } else {
                self.current = -1;
                self.done = true;
            }
            self.current
        }
        fn status(&mut self) -> BreakStatus {
            0
        }
        fn is_done(&mut self) -> bool {
            self.done
        }
        fn set_text_utf8(&mut self, _utf8: &[u8]) -> bool {
            self.reset();
            true
        }
        fn set_text_utf16(&mut self, _utf16: &[u16]) -> bool {
            self.reset();
            true
        }
    }

    let do_do_check =
        |r: &mut Reporter, it: &mut Box<dyn BreakIterator + '_>, expected: &[BreakPosition]| {
            // Initial state.
            reporter_assert!(r, !it.is_done());
            reporter_assert!(r, it.current() == 0);

            for &pos in expected {
                reporter_assert!(r, it.next() == pos);
                reporter_assert!(r, it.current() == pos);
                reporter_assert!(r, !it.is_done());
            }

            // Final state.
            reporter_assert!(r, it.next() < 0);
            reporter_assert!(r, it.current() < 0);
            reporter_assert!(r, it.is_done());

            // One more time for good measure.
            reporter_assert!(r, it.next() < 0);
            reporter_assert!(r, it.current() < 0);
            reporter_assert!(r, it.is_done());
        };

    let do_check =
        |r: &mut Reporter, a: &[BreakPosition], b: &[BreakPosition], expected: &[BreakPosition]| {
            let mut it = make_intersecting_break_iterator_for_testing(
                Box::new(MockBreakIterator::new(a.to_vec())),
                Box::new(MockBreakIterator::new(b.to_vec())),
            );

            do_do_check(r, &mut it, expected);

            // first() resets state.
            reporter_assert!(r, it.first() == 0);
            do_do_check(r, &mut it, expected);

            // as does setText()
            reporter_assert!(r, it.set_text_utf8(b"foo"));
            do_do_check(r, &mut it, expected);
        };

    let check =
        |r: &mut Reporter, a: &[BreakPosition], b: &[BreakPosition], expected: &[BreakPosition]| {
            // Order should not matter.
            do_check(r, a, b, expected);
            do_check(r, b, a, expected);
        };

    check(r, &[], &[], &[]);
    check(r, &[42], &[], &[]);
    check(r, &[42], &[43], &[]);
    check(r, &[42], &[42], &[42]);

    check(r, &[1, 3], &[1, 3], &[1, 3]);
    check(r, &[1, 3, 5], &[1, 3], &[1, 3]);
    check(r, &[1, 3, 5], &[3, 5], &[3, 5]);
    check(r, &[1, 3, 5], &[1, 5], &[1, 5]);

    check(r, &[1, 3, 5, 7], &[2, 4, 6, 8], &[]);
    check(r, &[1, 3, 5, 7], &[1], &[1]);
    check(r, &[1, 3, 5, 7], &[1, 5], &[1, 5]);
    check(r, &[1, 3, 5, 7], &[3, 7], &[3, 7]);
    check(r, &[1, 3, 5, 7], &[1, 7], &[1, 7]);
    check(r, &[1, 3, 5, 7], &[1, 3, 7], &[1, 3, 7]);
    check(r, &[1, 3, 5, 7], &[1, 5, 7], &[1, 5, 7]);

    check(r, &[1, 5, 9], &[2, 3, 4], &[]);
    check(r, &[1, 5, 9], &[6, 7, 8], &[]);
    check(r, &[1, 5, 9], &[2, 3, 7, 8], &[]);
    check(r, &[1, 5, 9], &[1, 2, 3], &[1]);
    check(r, &[1, 5, 9], &[7, 8, 9], &[9]);
    check(r, &[1, 5, 9], &[2, 3, 5, 7], &[5]);
    check(r, &[1, 5, 9], &[4, 5, 7, 8], &[5]);
    check(r, &[1, 5, 9], &[1, 2, 3, 5], &[1, 5]);
    check(r, &[1, 5, 9], &[1, 3, 5, 7], &[1, 5]);
    check(r, &[1, 5, 9], &[5, 7, 8, 9], &[5, 9]);
    check(r, &[1, 5, 9], &[3, 5, 7, 9], &[5, 9]);
    check(r, &[1, 5, 9], &[1, 3, 5, 9], &[1, 5, 9]);
    check(r, &[1, 5, 9], &[1, 5, 7, 9], &[1, 5, 9]);
});

// Port of: modules/skottie/tests/Shaper.cpp#L497-L513 (chrome/m156)
def_font_test!(Skottie_Shaper_fallback_metrics, |reporter| {
    let text = "Foo 推动全球交通可";
    let desc = TextDesc {
        typeface: Some(default_typeface()),
        text_size: 18.0,
        line_height: 18.0,
        flags: Flags::FRAGMENT_GLYPHS | Flags::TRACK_FRAGMENT_ADVANCE_ASCENT,
        ..TextDesc::default()
    };
    let result = Shaper::shape_in_box(
        text,
        &desc,
        &Rect::from_wh(1000.0, 1000.0),
        Some(&test_font_mgr()),
        &best_available(),
    );
    reporter_assert!(reporter, !result.fragments.is_empty());

    for frag in &result.fragments {
        reporter_assert!(reporter, frag.advance > 0.0);
    }
});

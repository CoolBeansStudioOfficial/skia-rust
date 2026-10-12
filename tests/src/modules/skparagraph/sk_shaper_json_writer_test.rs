// Copyright 2019 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skparagraph/tests/SkShaperJSONWriterTest.cpp (chrome/m156)

#![cfg(test)]
// The helpers mirror the C++ conversions between `size_t`, `uint32_t` and `int` (the text is far
// shorter than 4 GiB).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::struct_field_names
)]

//! `SkShaperJSONWriterTest.cpp` and the two static helpers of `SkShaperJSONWriter.cpp` it tests
//! (`BreakupClusters` and `VisualizeClusters`). The rest of the helper, the `RunHandler` that
//! writes JSON through `SkJSONWriter`, is used only by `SkShaperTest_basic`, which is compiled
//! out in Skia (`#if 0`).

use skia_rust_core::font_types::GlyphId;
use skia_rust_core::utf::count_utf8;

use crate::{def_test, reporter_assert};

/// `SkShaperJSONWriter::BreakupClustersCallback`.
type BreakupClustersCallback<'a> = dyn FnMut(usize, usize, u32, u32) + 'a;

/// `SkShaperJSONWriter::VisualizeClustersCallback`.
type VisualizeClustersCallback<'a> = dyn FnMut(usize, &[u8], &[GlyphId]) + 'a;

// Port of: modules/skparagraph/tests/SkShaperJSONWriter.cpp#L137-L173 (chrome/m156)
/// Breaks up a cluster into a set of ranges for the UTF-8 and the glyph IDs.
fn breakup_clusters(
    utf8_begin: usize,
    utf8_end: usize,
    clusters: &[u32],
    process_m_to_n: &mut BreakupClustersCallback<'_>,
) {
    if clusters[0] <= clusters[clusters.len() - 1] {
        // Handle left-to-right text direction
        let mut glyph_start_index = 0;
        for glyph_end_index in 0..clusters.len() {
            if clusters[glyph_start_index] == clusters[glyph_end_index] {
                continue;
            }

            process_m_to_n(
                glyph_start_index,
                glyph_end_index,
                clusters[glyph_start_index],
                clusters[glyph_end_index],
            );

            glyph_start_index = glyph_end_index;
        }

        process_m_to_n(
            glyph_start_index,
            clusters.len(),
            clusters[glyph_start_index],
            utf8_end as u32,
        );
    } else {
        // Handle right-to-left text direction.
        debug_assert!(clusters.len() >= 2);
        let mut glyph_start_index = 0;
        let mut utf8_end_index = utf8_end as u32;
        for glyph_end_index in 0..clusters.len() {
            if clusters[glyph_start_index] == clusters[glyph_end_index] {
                continue;
            }

            process_m_to_n(
                glyph_start_index,
                glyph_end_index,
                clusters[glyph_start_index],
                utf8_end_index,
            );

            utf8_end_index = clusters[glyph_start_index];
            glyph_start_index = glyph_end_index;
        }
        process_m_to_n(
            glyph_start_index,
            clusters.len(),
            utf8_begin as u32,
            clusters[glyph_start_index - 1],
        );
    }
}

// Port of: modules/skparagraph/tests/SkShaperJSONWriter.cpp#L175-L227 (chrome/m156)
/// Gathers runs of 1:1 into larger runs, and displays M:N as single entries.
fn visualize_clusters(
    utf8: &[u8],
    utf8_begin: usize,
    utf8_end: usize,
    glyph_ids: &[GlyphId],
    clusters: &[u32],
    process_m_to_n: &mut VisualizeClustersCallback<'_>,
) {
    // `glyphRangeStart`, `glyphRangeEnd`, `utf8RangeStart` and `utf8RangeEnd`.
    struct Ranges {
        glyph_range_start: usize,
        glyph_range_end: usize,
        utf8_range_start: u32,
        utf8_range_end: u32,
    }

    fn reset_ranges(ranges: &mut Ranges) {
        ranges.glyph_range_start = usize::MAX;
        ranges.glyph_range_end = 0;
        ranges.utf8_range_start = u32::MAX;
        ranges.utf8_range_end = 0;
    }

    fn check_ranges_and_process(
        ranges: &mut Ranges,
        utf8: &[u8],
        glyph_ids: &[GlyphId],
        process_m_to_n: &mut VisualizeClustersCallback<'_>,
    ) {
        if ranges.glyph_range_start < ranges.glyph_range_end {
            let glyph_range_count = ranges.glyph_range_end - ranges.glyph_range_start;
            let utf8_span = &utf8[ranges.utf8_range_start as usize..ranges.utf8_range_end as usize];
            let glyph_span = &glyph_ids[ranges.glyph_range_start..ranges.glyph_range_end];

            // Glyph count is the same as codepoint count for 1:1.
            process_m_to_n(glyph_range_count, utf8_span, glyph_span);
        }
        reset_ranges(ranges);
    }

    let mut ranges = Ranges {
        glyph_range_start: 0,
        glyph_range_end: 0,
        utf8_range_start: 0,
        utf8_range_end: 0,
    };
    reset_ranges(&mut ranges);

    let mut gather_runs = |glyph_start_index: usize,
                           glyph_end_index: usize,
                           utf8_start_index: u32,
                           utf8_end_index: u32| {
        let possible_count = count_utf8(&utf8[utf8_start_index as usize..utf8_end_index as usize]);
        if possible_count == -1 {
            return;
        }
        let code_point_count = possible_count as usize;
        if code_point_count == 1 && glyph_end_index - glyph_start_index == 1 {
            ranges.glyph_range_start = ranges.glyph_range_start.min(glyph_start_index);
            ranges.glyph_range_end = ranges.glyph_range_end.max(glyph_end_index);
            ranges.utf8_range_start = ranges.utf8_range_start.min(utf8_start_index);
            ranges.utf8_range_end = ranges.utf8_range_end.max(utf8_end_index);
        } else {
            check_ranges_and_process(&mut ranges, utf8, glyph_ids, process_m_to_n);

            let utf8_span = &utf8[utf8_start_index as usize..utf8_end_index as usize];
            let glyph_span = &glyph_ids[glyph_start_index..glyph_end_index];

            process_m_to_n(code_point_count, utf8_span, glyph_span);
        }
    };

    breakup_clusters(utf8_begin, utf8_end, clusters, &mut gather_runs);
    check_ranges_and_process(&mut ranges, utf8, glyph_ids, process_m_to_n);
}

// Port of: modules/skparagraph/tests/SkShaperJSONWriterTest.cpp#L16-L71 (chrome/m156)
def_test!(SkShaperTest_cluster, |reporter| {
    struct Answer {
        glyph_start_index: usize,
        glyph_end_index: usize,
        utf8_start_index: u32,
        utf8_end_index: u32,
    }

    struct TestCase {
        utf8_len: usize,
        clusters: Vec<u32>,
        answers: Vec<Answer>,
    }

    let a = |g0, g1, u0, u1| Answer {
        glyph_start_index: g0,
        glyph_end_index: g1,
        utf8_start_index: u0,
        utf8_end_index: u1,
    };
    let cases = vec![
        /*1:1*/
        TestCase {
            utf8_len: 1,
            clusters: vec![0],
            answers: vec![a(0, 1, 0, 1)],
        },
        /*1:2*/
        TestCase {
            utf8_len: 1,
            clusters: vec![0, 0],
            answers: vec![a(0, 2, 0, 1)],
        },
        /*2:1*/
        TestCase {
            utf8_len: 2,
            clusters: vec![0],
            answers: vec![a(0, 1, 0, 2)],
        },
        /*2:3*/
        TestCase {
            utf8_len: 2,
            clusters: vec![0, 0, 0],
            answers: vec![a(0, 3, 0, 2)],
        },
        /*3:2*/
        TestCase {
            utf8_len: 3,
            clusters: vec![0, 0],
            answers: vec![a(0, 2, 0, 3)],
        },
        // cluster runs
        TestCase {
            utf8_len: 2,
            clusters: vec![0, 1],
            answers: vec![a(0, 1, 0, 1), a(1, 2, 1, 2)],
        },
        TestCase {
            utf8_len: 2,
            clusters: vec![1, 0],
            answers: vec![a(0, 1, 1, 2), a(1, 2, 0, 1)],
        },
        TestCase {
            utf8_len: 2,
            clusters: vec![0, 0, 1],
            answers: vec![a(0, 2, 0, 1), a(2, 3, 1, 2)],
        },
        TestCase {
            utf8_len: 2,
            clusters: vec![1, 0, 0],
            answers: vec![a(0, 1, 1, 2), a(1, 3, 0, 1)],
        },
        TestCase {
            utf8_len: 2,
            clusters: vec![0, 1, 1],
            answers: vec![a(0, 1, 0, 1), a(1, 3, 1, 2)],
        },
        TestCase {
            utf8_len: 2,
            clusters: vec![1, 1, 0],
            answers: vec![a(0, 2, 1, 2), a(2, 3, 0, 1)],
        },
        TestCase {
            utf8_len: 3,
            clusters: vec![0, 0, 1],
            answers: vec![a(0, 2, 0, 1), a(2, 3, 1, 3)],
        },
        TestCase {
            utf8_len: 3,
            clusters: vec![1, 0, 0],
            answers: vec![a(0, 1, 1, 3), a(1, 3, 0, 1)],
        },
        TestCase {
            utf8_len: 3,
            clusters: vec![0, 1, 1],
            answers: vec![a(0, 1, 0, 1), a(1, 3, 1, 3)],
        },
        TestCase {
            utf8_len: 3,
            clusters: vec![1, 1, 0],
            answers: vec![a(0, 2, 1, 3), a(2, 3, 0, 1)],
        },
        TestCase {
            utf8_len: 4,
            clusters: vec![3, 2, 1, 0],
            answers: vec![a(0, 1, 3, 4), a(1, 2, 2, 3), a(2, 3, 1, 2), a(3, 4, 0, 1)],
        },
    ];

    for one_case in &cases {
        let mut answer_count = 0;
        let mut checker = |glyph_start_index: usize,
                           glyph_end_index: usize,
                           utf8_start_index: u32,
                           utf8_end_index: u32| {
            if answer_count < one_case.answers.len() {
                let a = &one_case.answers[answer_count];
                reporter_assert!(reporter, a.glyph_start_index == glyph_start_index);
                reporter_assert!(reporter, a.glyph_end_index == glyph_end_index);
                reporter_assert!(reporter, a.utf8_start_index == utf8_start_index);
                reporter_assert!(reporter, a.utf8_end_index == utf8_end_index);
            } else {
                reporter_assert!(reporter, false, "Too many clusters");
            }
            answer_count += 1;
        };

        breakup_clusters(0, one_case.utf8_len, &one_case.clusters, &mut checker);
        reporter_assert!(reporter, answer_count == one_case.answers.len());
    }
});

// Port of: modules/skparagraph/tests/SkShaperJSONWriterTest.cpp#L73-L121 (chrome/m156)
def_test!(SkShaperTest_VisualizeCluster, |reporter| {
    struct Answer {
        utf8: &'static str,
        glyph_ids: Vec<GlyphId>,
    }
    struct TestCase {
        utf8: &'static str,
        glyph_ids: Vec<GlyphId>,
        clusters: Vec<u32>,
        answers: Vec<Answer>,
    }

    let ans = |utf8, glyph_ids| Answer { utf8, glyph_ids };
    let cases = vec![
        TestCase {
            utf8: "A",
            glyph_ids: vec![7],
            clusters: vec![0],
            answers: vec![ans("A", vec![7])],
        },
        TestCase {
            utf8: "ABCD",
            glyph_ids: vec![7, 8, 9, 10],
            clusters: vec![0, 1, 2, 3],
            answers: vec![ans("ABCD", vec![7, 8, 9, 10])],
        },
        TestCase {
            utf8: "A",
            glyph_ids: vec![7, 8],
            clusters: vec![0, 0],
            answers: vec![ans("A", vec![7, 8])],
        },
        TestCase {
            utf8: "AB",
            glyph_ids: vec![7],
            clusters: vec![0],
            answers: vec![ans("AB", vec![7])],
        },
        TestCase {
            utf8: "AB",
            glyph_ids: vec![7, 8, 9],
            clusters: vec![0, 0, 0],
            answers: vec![ans("AB", vec![7, 8, 9])],
        },
        TestCase {
            utf8: "ABC",
            glyph_ids: vec![7, 8],
            clusters: vec![0, 0],
            answers: vec![ans("ABC", vec![7, 8])],
        },
        TestCase {
            utf8: "ABCD",
            glyph_ids: vec![7, 8, 9, 10],
            clusters: vec![3, 2, 1, 0],
            answers: vec![ans("ABCD", vec![7, 8, 9, 10])],
        },
        TestCase {
            utf8: "\u{627}\u{644}\u{645}\u{627}\u{62F}\u{629}",
            glyph_ids: vec![246, 268, 241, 205, 240],
            clusters: vec![10, 8, 6, 2, 0],
            answers: vec![
                ans("\u{627}\u{62F}\u{629}", vec![246, 268, 241]),
                ans("\u{644}\u{645}", vec![205]),
                ans("\u{627}", vec![240]),
            ],
        },
    ];

    for one_case in &cases {
        let mut answer_count = 0;
        let mut checker = |_code_point_count: usize, utf1to1: &[u8], glyph1to1: &[GlyphId]| {
            if answer_count < one_case.answers.len() {
                let a = &one_case.answers[answer_count];
                let to_check_utf8 = String::from_utf8_lossy(utf1to1);
                reporter_assert!(reporter, a.utf8 == to_check_utf8);
                let to_check_glyph_ids: Vec<GlyphId> = glyph1to1.to_vec();
                reporter_assert!(reporter, a.glyph_ids == to_check_glyph_ids);
            } else {
                reporter_assert!(reporter, false, "Too many clusters");
            }
            answer_count += 1;
        };

        visualize_clusters(
            one_case.utf8.as_bytes(),
            0,
            one_case.utf8.len(),
            &one_case.glyph_ids,
            &one_case.clusters,
            &mut checker,
        );
    }
});

// `SkShaperTest_basic` (`modules/skparagraph/tests/SkShaperJSONWriterTest.cpp#L126-L146`) is
// inside `#if 0` in Skia, and is not ported (manifest status `excluded`).

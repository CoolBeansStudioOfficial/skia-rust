// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ParsePathTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::utils::parse_path;

// Port of: tests/ParsePathTest.cpp#L19-L35 (chrome/m156)
fn test_to_from(reporter: &mut Reporter, path: &Path) {
    let str = parse_path::to_svg(path);

    let path2 = parse_path::from_svg(&str);
    reporter_assert!(reporter, path2.is_some());
    let Some(path2) = path2 else { return };

    let str2 = parse_path::to_svg(&path2);
    reporter_assert!(reporter, str == str2);
    // #if 0 in C++: closed paths are not equal, the iter explicitly gives the closing
    // edge, even if it is not in the path.
}

struct Rec {
    str: &'static str,
    bounds: Rect,
}

// Port of: tests/ParsePathTest.cpp#L41-L48 (chrome/m156)
#[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
fn recs() -> [Rec; 4] {
    [
        Rec {
            str: "M1,1 l-2.58-2.828-3.82-0.113, 1.9-3.3223-1.08-3.6702, 3.75,0.7744,3.16-2.1551,\
                  0.42,3.8008,3.02,2.3384-3.48,1.574-1.29,3.601z",
            bounds: Rect::new(-5.399_999_62, -10.3142, 5.770_000_46, 1.0),
        },
        Rec {
            str: "",
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        },
        Rec {
            str: "M0,0L10,10",
            bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
        },
        Rec {
            str: "M-5.5,-0.5 Q 0 0 6,6.50",
            bounds: Rect::new(-5.5, -0.5, 6.0, 6.5),
        },
    ]
}

// Port of: tests/ParsePathTest.cpp#L50-L71 (chrome/m156)
def_test!(ParsePath, |reporter| {
    for rec in &recs() {
        let path = parse_path::from_svg(rec.str);
        reporter_assert!(reporter, path.is_some());
        let Some(path) = path else { continue };
        let expected_bounds = rec.bounds;
        let path_bounds = *path.bounds();
        reporter_assert!(reporter, expected_bounds == path_bounds);

        test_to_from(reporter, &path);
    }

    let mut r = Rect::default();
    r.set_ltrb(0.0, 0.0, 10.0, 10.5);
    let mut p = PathBuilder::new();
    p.add_rect(r, None, None);
    test_to_from(reporter, &p.snapshot());
    p.add_oval(r, None, None);
    test_to_from(reporter, &p.snapshot());
    p.add_rrect(RRect::new_rect_xy(r, 4.0, 4.5), None, None);
    test_to_from(reporter, &p.snapshot());
});

// Port of: tests/ParsePathTest.cpp#L73-L78 (chrome/m156)
fn test_invalid_path(reporter: &mut Reporter, _name: &str, input: &str) {
    // skia-rust: `skiatest::ReporterContext` is not ported; `name` is unused.
    let path = parse_path::from_svg(input);
    reporter_assert!(reporter, path.is_none());
}

// Port of: tests/ParsePathTest.cpp#L80-L91 (chrome/m156)
def_test!(ParsePath_InvalidDoesNotCrash, |r| {
    test_invalid_path(r, "empty move", "M");
    test_invalid_path(r, "partial move", "M 5");
    test_invalid_path(r, "partial vertical line", "V"); // oss-fuzz:68723
    test_invalid_path(r, "partial horizontal line", "H");
    test_invalid_path(r, "partial cubic", "C 1 2");
    test_invalid_path(r, "partial continued cubic", "S 6 7");
    test_invalid_path(r, "partial quad", "Q 3 4 5");
    test_invalid_path(r, "partial continued quad", "T");
    test_invalid_path(r, "partial arc", "A 1 2 3 4 5 6");
    test_invalid_path(r, "partial ~", "~ 7 6 5");
});

// Port of: tests/ParsePathTest.cpp#L93-L129 (chrome/m156)
def_test!(ParsePathOptionalCommand, |r| {
    struct Tests {
        str: &'static str,
        verbs: usize,
        points: usize,
    }
    let tests = [
        Tests {
            str: "",
            verbs: 0,
            points: 0,
        },
        Tests {
            str: "H100 200 ",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "H-100-200",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "H+100+200",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "H.10.20",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "H-.10-.20",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "H+.10+.20",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "L100 100 200 200",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "L-100-100-200-200",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "L+100+100+200+200",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "L.10.10.20.20",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "L-.10-.10-.20-.20",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "L+.10+.10+.20+.20",
            verbs: 3,
            points: 3,
        },
        Tests {
            str: "C100 100 200 200 300 300 400 400 500 500 600 600",
            verbs: 3,
            points: 7,
        },
        Tests {
            str: "C100-100-200-200-300-300-400-400-500-500-600-600",
            verbs: 3,
            points: 7,
        },
        Tests {
            str: "C100+100+200+200+300+300+400+400+500+500+600+600",
            verbs: 3,
            points: 7,
        },
        Tests {
            str: "C.10.10.20.20.30.30.40.40.50.50.60.60",
            verbs: 3,
            points: 7,
        },
        Tests {
            str: "C-.10-.10-.20-.20-.30-.30-.40-.40-.50-.50-.60-.60",
            verbs: 3,
            points: 7,
        },
        Tests {
            str: "C+.10+.10+.20+.20+.30+.30+.40+.40+.50+.50+.60+.60",
            verbs: 3,
            points: 7,
        },
        Tests {
            str: "c-1.49.71-2.12 2.5-1.4 4 .71 1.49 2.5 2.12 4 1.4z",
            verbs: 4,
            points: 7,
        },
    ];

    for t in &tests {
        let path = parse_path::from_svg(t.str);
        reporter_assert!(r, path.is_some());
        let Some(path) = path else { continue };
        reporter_assert!(r, path.count_verbs() == t.verbs);
        reporter_assert!(r, path.count_points() == t.points);
    }
});

// Port of: tests/ParsePathTest.cpp#L131-L140 (chrome/m156)
def_test!(ParsePathArcFlags, |r| {
    let arcs = "M10 10a2.143 2.143 0 100-4.285 2.143 2.143 0 000 4.286";
    let path = parse_path::from_svg(arcs);
    reporter_assert!(r, path.is_some());
    let Some(path) = path else { return };
    // Arcs decompose to two conics.
    reporter_assert!(r, path.count_verbs() == 5);
    // One for move, 2x per conic.
    reporter_assert!(r, path.count_points() == 9);
});

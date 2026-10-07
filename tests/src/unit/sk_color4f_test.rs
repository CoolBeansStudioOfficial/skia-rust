// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkColor4fTest.cpp (chrome/m156)

use skia_rust_core::color::{Color, Color4f};

use crate::{def_test, reporter_assert};

// Port of: tests/SkColor4fTest.cpp#L13-L27 (chrome/m156)
def_test!(SkColor4f_FromColor, |reporter| {
    struct Rec {
        c: Color,
        c4: Color4f,
    }
    let recs = [
        Rec {
            c: Color::BLACK,
            c4: Color4f::new(0.0, 0.0, 0.0, 1.0),
        },
        Rec {
            c: Color::WHITE,
            c4: Color4f::new(1.0, 1.0, 1.0, 1.0),
        },
        Rec {
            c: Color::RED,
            c4: Color4f::new(1.0, 0.0, 0.0, 1.0),
        },
        Rec {
            c: Color::GREEN,
            c4: Color4f::new(0.0, 1.0, 0.0, 1.0),
        },
        Rec {
            c: Color::BLUE,
            c4: Color4f::new(0.0, 0.0, 1.0, 1.0),
        },
        Rec {
            c: Color::new(0),
            c4: Color4f::new(0.0, 0.0, 0.0, 0.0),
        },
    ];

    for r in &recs {
        let c4 = Color4f::from_color(r.c);
        reporter_assert!(reporter, c4 == r.c4);
    }
});

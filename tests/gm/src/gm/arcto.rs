// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/arcto.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path_builder::{ArcSize, PathBuilder};
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::string::{str_append_scalar, str_append_u32};
use skia_rust_core::utils::parse_path;
use skia_rust_effects::dash_path_effect;
use std::fmt::Write;

// The test below generates a reference image using SVG. To compare the result for correctness,
// enable the define below and then view the generated SVG in a browser.
// (The SVG generation itself is not ported: it only writes a file.)
// Port of: gm/arcto.cpp#L26 (chrome/m156)

// The arcto test below should draw the same as the SVG in the C++ comment.
// Port of: gm/arcto.cpp#L58-L127 (chrome/m156)
crate::def_simple_gm!(arcto, canvas, 500, 600, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(2.0);
    paint.set_color(Color::new(0xFF660000));
    let mut oval = Rect::from_xywh(100.0, 100.0, 100.0, 100.0);

    let mut angle: f32 = 0.0;
    while angle <= 45.0 {
        for o_height in (1..=2).rev() {
            let mut svg_arc = PathBuilder::new();
            #[allow(clippy::cast_precision_loss)] // oval.height() / oHeight
            let oval_height = oval.height() / o_height as f32;
            svg_arc.move_to((oval.left, oval.top));
            svg_arc.arc_to_radius(
                (oval.width() / 2.0, oval_height),
                angle,
                ArcSize::Small,
                PathDirection::CW,
                (oval.right, oval.bottom),
            );
            canvas.draw_path(&svg_arc.detach(), &paint);

            svg_arc.move_to((oval.left + 100.0, oval.top + 100.0));
            svg_arc.arc_to_radius(
                (oval.width() / 2.0, oval_height),
                angle,
                ArcSize::Large,
                PathDirection::CCW,
                (oval.right, oval.bottom + 100.0),
            );
            canvas.draw_path(&svg_arc.detach(), &paint);
            oval.offset((50.0, 0.0));
        }
        angle += 45.0;
    }

    paint.set_stroke_width(5.0);
    let purple = Color::new(0xFF800080);
    let darkgreen = Color::new(0xFF008000);
    let colors = [Color::RED, darkgreen, purple, Color::BLUE];
    let arcstrs = [
        "M250,400  A120,80 0 0,0 250,500",
        "M250,400  A120,80 0 1,1 250,500",
        "M250,400  A120,80 0 1,0 250,500",
        "M250,400  A120,80 0 0,1 250,500",
    ];
    let mut c_index = 0;
    for arcstr in arcstrs {
        if let Some(path) = parse_path::from_svg(arcstr) {
            paint.set_color(colors[c_index]);
            c_index += 1;
            canvas.draw_path(&path, &paint);
        }
    }

    // test that zero length arcs still draw round cap
    paint.set_stroke_cap(Cap::Round);
    let mut path = PathBuilder::new();
    path.move_to((100.0, 100.0)).arc_to_radius(
        (0.0, 0.0),
        0.0,
        ArcSize::Large,
        PathDirection::CW,
        (200.0, 200.0),
    );
    canvas.draw_path(&path.detach(), &paint);

    path.move_to((200.0, 100.0)).arc_to_radius(
        (80.0, 80.0),
        0.0,
        ArcSize::Large,
        PathDirection::CW,
        (200.0, 100.0),
    );
    canvas.draw_path(&path.detach(), &paint);
});

// Port of: gm/arcto.cpp#L129 (chrome/m156)
const PARSE_PATH_TEST_DIMENSION: i32 = 500;

// Port of: gm/arcto.cpp#L133-L136 (chrome/m156)
struct Legal {
    symbol: u8,
    scalars: i32,
}

// Port of: gm/arcto.cpp#L137-L148 (chrome/m156)
const G_LEGAL: [Legal; 10] = [
    Legal {
        symbol: b'M',
        scalars: 2,
    },
    Legal {
        symbol: b'H',
        scalars: 1,
    },
    Legal {
        symbol: b'V',
        scalars: 1,
    },
    Legal {
        symbol: b'L',
        scalars: 2,
    },
    Legal {
        symbol: b'Q',
        scalars: 4,
    },
    Legal {
        symbol: b'T',
        scalars: 2,
    },
    Legal {
        symbol: b'C',
        scalars: 6,
    },
    Legal {
        symbol: b'S',
        scalars: 4,
    },
    Legal {
        symbol: b'A',
        scalars: 4,
    },
    Legal {
        symbol: b'Z',
        scalars: 0,
    },
];

// set to true while debugging to suppress unusual whitespace
// Port of: gm/arcto.cpp#L150 (chrome/m156)
const G_EASY: bool = false;

// mostly do nothing, then bias towards spaces
// Port of: gm/arcto.cpp#L152-L169 (chrome/m156)
const G_WHITE_SPACE: [u8; 15] = [
    0, 0, 0, 0, 0, 0, 0, 0, b' ', b' ', b' ', b' ', 0x09, 0x0D, 0x0A,
];

// Port of: gm/arcto.cpp#L171-L183 (chrome/m156)
fn add_white(rand: &mut Random, atom: &mut String) {
    if G_EASY {
        atom.push(' ');
        return;
    }
    let reps = rand.next_range_u(0, 2);
    for _ in 0..reps {
        let index = rand.next_range_u(0, u32::try_from(G_WHITE_SPACE.len()).unwrap() - 1) as usize;
        if G_WHITE_SPACE[index] != 0 {
            atom.push(char::from(G_WHITE_SPACE[index]));
        }
    }
}

// Port of: gm/arcto.cpp#L185-L199 (chrome/m156)
fn add_comma(rand: &mut Random, atom: &mut String) {
    if G_EASY {
        atom.push(',');
        return;
    }
    let count = atom.len();
    add_white(rand, atom);
    if rand.next_bool() {
        atom.push(',');
    }
    loop {
        add_white(rand, atom);
        if count != atom.len() {
            break;
        }
    }
}

// Port of: gm/arcto.cpp#L201-L207 (chrome/m156)
fn add_some_white(rand: &mut Random, atom: &mut String) {
    let count = atom.len();
    loop {
        add_white(rand, atom);
        if count != atom.len() {
            break;
        }
    }
}

// Port of: gm/arcto.cpp#L209-L240 (chrome/m156)
fn make_random_svg_path(rand: &mut Random) -> String {
    let mut atom = String::new();
    let legal_index = rand.next_range_u(0, u32::try_from(G_LEGAL.len()).unwrap() - 1) as usize;
    let legal = &G_LEGAL[legal_index];
    if G_EASY {
        atom.push('\n');
    } else {
        add_white(rand, &mut atom);
    }
    let symbol = legal.symbol | (if rand.next_bool() { 0x20 } else { 0 });
    atom.push(char::from(symbol));
    let reps = rand.next_range_u(1, 3);
    for rep in 0..reps {
        for index in 0..legal.scalars {
            let coord = rand.next_range_f(0.0, 100.0);
            add_white(rand, &mut atom);
            str_append_scalar(&mut atom, coord);
            #[allow(clippy::cast_possible_wrap)] // reps is 1..=3
            let last_rep = rep as i32 == reps as i32 - 1;
            if !last_rep && index < legal.scalars - 1 {
                add_comma(rand, &mut atom);
            } else {
                add_some_white(rand, &mut atom);
            }
            if b'A' == legal.symbol && 1 == index {
                str_append_scalar(&mut atom, rand.next_range_f(-720.0, 720.0));
                add_comma(rand, &mut atom);
                str_append_u32(&mut atom, rand.next_range_u(0, 1));
                add_comma(rand, &mut atom);
                str_append_u32(&mut atom, rand.next_range_u(0, 1));
                add_comma(rand, &mut atom);
            }
        }
    }
    atom
}

// Port of: gm/arcto.cpp#L242-L309 (chrome/m156)
crate::def_simple_gm!(
    #[ignore = "see notes/gm_arcto_cpp_parsedpaths.md"]
    parsedpaths,
    canvas,
    PARSE_PATH_TEST_DIMENSION,
    PARSE_PATH_TEST_DIMENSION,
    {
        let mut rand = Random::default();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        for _x_start in (0..PARSE_PATH_TEST_DIMENSION).step_by(100) {
            canvas.save();
            for _y_start in (0..PARSE_PATH_TEST_DIMENSION).step_by(100) {
                let mut count = 3;
                loop {
                    let mut spec = String::new();
                    let y = rand.next_range_u(30, 70);
                    let x = rand.next_range_u(30, 70);
                    write!(spec, "M {x},{y}\n").unwrap();
                    let mut i = rand.next_range_u(0, 10);
                    while i > 0 {
                        i -= 1;
                        spec.push_str(&make_random_svg_path(&mut rand));
                    }
                    let path = parse_path::from_svg(&spec);
                    let path = path.expect("SkAssertResult");
                    paint.set_color(rand.next_u());
                    canvas.save();
                    canvas.clip_rect(Rect::from_iwh(100, 100), None, None);
                    canvas.draw_path(&path, &paint);
                    canvas.restore();
                    count -= 1;
                    if count <= 0 {
                        break;
                    }
                }
                canvas.translate((0.0, 100.0));
            }
            canvas.restore();
            canvas.translate((100.0, 0.0));
        }
    }
);

// Port of: gm/arcto.cpp#L311-L329 (chrome/m156)
crate::def_simple_gm!(bug593049, canvas, 300, 300, {
    canvas.translate((111.0, 0.0));

    let mut p = PathBuilder::new();
    p.move_to((-43.44464063610148, 79.43535936389853));
    let y_offset: f32 = 122.88;
    let radius: f32 = 61.44;
    let oval = Rect::from_xywh(-radius, y_offset - radius, 2.0 * radius, 2.0 * radius);
    p.arc_to(oval, 1.25 * 180.0, 0.5 * 180.0, false);

    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_cap(Cap::Round);
    paint.set_stroke_width(15.36);

    canvas.draw_path(&p.detach(), &paint);
});

// Port of: gm/arcto.cpp#L331-L346 (chrome/m156)
crate::def_simple_gm!(bug583299, canvas, 300, 300, {
    let d = "M60,60 A50,50 0 0 0 160,60 A50,50 0 0 0 60,60z";
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    p.set_stroke_width(100.0);
    p.set_anti_alias(true);
    p.set_color(Color::new(0xFF008200));
    p.set_stroke_cap(Cap::Square);
    let path = parse_path::from_svg(d).unwrap_or_default();
    let meas = PathMeasure::new(&path, false, None);
    let length = meas.length();
    let intervals = [0.0, length];
    p.set_path_effect(dash_path_effect::new(&intervals, 0.0));
    canvas.draw_path(&path, &p);
});

// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsSimplifyRectThreadedTest.cpp (chrome/m156)

// The threaded runners are run single-threaded, in the order the C++ runnables are appended, with
// the same case enumeration. The verbose output of the generated test sources is not ported.
//
// The C++ loops assign their own counters inside the body (`aXAlign = 5` ends the loop after the
// current pass), so the loops are ported as `while` loops whose counters the body may assign.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::cognitive_complexity
)]
#![cfg(test)]

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::rect::Rect;

use crate::unit::path_ops_extended_test::test_simplify_threaded;
use crate::{Reporter, def_test};

/// `SkIntToScalar` of the small integer rect coordinates (exact in f32).
fn s(v: i32) -> f32 {
    v as f32
}

/// `state.fA >> 2 ? SkPathDirection::kCCW : SkPathDirection::kCW`.
fn direction(v: u8) -> PathDirection {
    if (v >> 2) != 0 {
        PathDirection::CCW
    } else {
        PathDirection::CW
    }
}

// Port of: tests/PathOpsSimplifyRectThreadedTest.cpp#L27-L196 (chrome/m156)
fn test_simplify_4x4_rects_main(reporter: &mut Reporter, a: u8, b: u8, c: u8, d: u8) {
    let a_shape = i32::from(a & 0x03);
    let a_cw = direction(a);
    let b_shape = i32::from(b & 0x03);
    let b_cw = direction(b);
    let c_shape = i32::from(c & 0x03);
    let c_cw = direction(c);
    let d_shape = i32::from(d & 0x03);
    let d_cw = direction(d);

    let mut a_x_align = 0;
    while a_x_align < 5 {
        let mut a_y_align = 0;
        while a_y_align < 5 {
            let mut b_x_align = 0;
            while b_x_align < 5 {
                let mut b_y_align = 0;
                while b_y_align < 5 {
                    let mut c_x_align = 0;
                    while c_x_align < 5 {
                        let mut c_y_align = 0;
                        while c_y_align < 5 {
                            let mut d_x_align = 0;
                            while d_x_align < 5 {
                                let mut d_y_align = 0;
                                while d_y_align < 5 {
                                    let mut builder = PathBuilder::new();
                                    if a_shape != 0 {
                                        let (l, t, r, b) = match a_shape {
                                            1 => {
                                                // square
                                                a_x_align = 5;
                                                a_y_align = 5;
                                                (0, 0, 60, 60)
                                            }
                                            2 => {
                                                let l = a_x_align * 12;
                                                a_y_align = 5;
                                                (l, 0, l + 30, 60)
                                            }
                                            3 => {
                                                let l = 0;
                                                let t = a_y_align * 12;
                                                a_x_align = 5;
                                                (l, t, 60, l + 30)
                                            }
                                            _ => unreachable!("aShape is 1..=3 here"),
                                        };
                                        builder.add_rect(
                                            Rect::new(s(l), s(t), s(r), s(b)),
                                            a_cw,
                                            None,
                                        );
                                    } else {
                                        a_x_align = 5;
                                        a_y_align = 5;
                                    }
                                    if b_shape != 0 {
                                        let (l, t, r, b) = match b_shape {
                                            1 => {
                                                // square
                                                let l = b_x_align * 10;
                                                (l, b_y_align * 10, l + 20, l + 20)
                                            }
                                            2 => {
                                                let l = b_x_align * 10;
                                                b_y_align = 5;
                                                (l, 10, l + 20, 40)
                                            }
                                            3 => {
                                                let t = b_y_align * 10;
                                                b_x_align = 5;
                                                (10, t, 40, 30)
                                            }
                                            _ => unreachable!("bShape is 1..=3 here"),
                                        };
                                        builder.add_rect(
                                            Rect::new(s(l), s(t), s(r), s(b)),
                                            b_cw,
                                            None,
                                        );
                                    } else {
                                        b_x_align = 5;
                                        b_y_align = 5;
                                    }
                                    if c_shape != 0 {
                                        let (l, t, r, b) = match c_shape {
                                            1 => {
                                                // square
                                                let l = c_x_align * 6;
                                                (l, c_y_align * 6, l + 12, l + 12)
                                            }
                                            2 => {
                                                let l = c_x_align * 6;
                                                c_y_align = 5;
                                                (l, 20, l + 12, 30)
                                            }
                                            3 => {
                                                let t = c_y_align * 6;
                                                c_x_align = 5;
                                                (20, t, 30, 40)
                                            }
                                            _ => unreachable!("cShape is 1..=3 here"),
                                        };
                                        builder.add_rect(
                                            Rect::new(s(l), s(t), s(r), s(b)),
                                            c_cw,
                                            None,
                                        );
                                    } else {
                                        c_x_align = 5;
                                        c_y_align = 5;
                                    }
                                    if d_shape != 0 {
                                        let (l, t, r, b) = match d_shape {
                                            1 => {
                                                // square
                                                let l = d_x_align * 4;
                                                (l, d_y_align * 4, l + 9, l + 9)
                                            }
                                            2 => {
                                                let l = d_x_align * 6;
                                                d_y_align = 5;
                                                (l, 32, l + 9, 36)
                                            }
                                            3 => {
                                                let t = d_y_align * 6;
                                                d_x_align = 5;
                                                (32, t, 36, 41)
                                            }
                                            _ => unreachable!("dShape is 1..=3 here"),
                                        };
                                        builder.add_rect(
                                            Rect::new(s(l), s(t), s(r), s(b)),
                                            d_cw,
                                            None,
                                        );
                                    } else {
                                        d_x_align = 5;
                                        d_y_align = 5;
                                    }
                                    builder.close();
                                    let path = builder.detach();
                                    test_simplify_threaded(reporter, &path, false);
                                    // path.setFillType(kEvenOdd) followed by testSimplify(path, true, ...).
                                    test_simplify_threaded(reporter, &path, true);

                                    d_y_align += 1;
                                }
                                d_x_align += 1;
                            }
                            c_y_align += 1;
                        }
                        c_x_align += 1;
                    }
                    b_y_align += 1;
                }
                b_x_align += 1;
            }
            a_y_align += 1;
        }
        a_x_align += 1;
    }
}

// Port of: tests/PathOpsSimplifyRectThreadedTest.cpp#L198-L214 (chrome/m156)
def_test!(PathOpsSimplifyRectsThreaded, |reporter| {
    // Runnables are collected first, as the C++ appends them before render(). With
    // allowExtendedTest() false the enumeration stops after the first `c` iteration.
    let mut runnables = Vec::new();
    'finish: for a in 0..8u8 {
        for b in a..8 {
            for c in b..8 {
                for d in c..8 {
                    runnables.push((a, b, c, d));
                }
                if !reporter.allow_extended_test() {
                    break 'finish;
                }
            }
        }
    }
    for (a, b, c, d) in runnables {
        test_simplify_4x4_rects_main(reporter, a, b, c, d);
    }
});

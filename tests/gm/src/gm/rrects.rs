// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rrects.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

const K_IMAGE_WIDTH: i32 = 640;
const K_IMAGE_HEIGHT: i32 = 480;

const K_TILE_X: i32 = 80;
const K_TILE_Y: i32 = 40;

const K_NUM_SIMPLE_CASES: usize = 7;
const K_NUM_COMPLEX_CASES: usize = 35;

const K_NUM_RRECTS: usize = K_NUM_SIMPLE_CASES + K_NUM_COMPLEX_CASES + 1 /* extra big */;

// Port of: gm/rrects.cpp#L43-L49 (chrome/m156)
#[derive(Clone, Copy, PartialEq, Eq)]
enum Type {
    BwDraw,
    AaDraw,
}

// Port of: gm/rrects.cpp#L41-L225 (chrome/m156)
//
// skia-rust: the `kBW_Clip_Type` / `kAA_Clip_Type` variants (they need a linear gradient shader)
// and `kEffect_Type` (GPU only) are not ported yet; see the manifest entries.
struct RRectGM {
    type_: Type,
    rrects: [RRect; K_NUM_RRECTS],
    bg: Color,
}

impl RRectGM {
    fn new(type_: Type) -> Self {
        Self {
            type_,
            rrects: [RRect::default(); K_NUM_RRECTS],
            bg: Color::WHITE,
        }
    }

    // Port of: gm/rrects.cpp#L183-L205 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of small constants
    fn set_up_rrects(&mut self) {
        // each RRect must fit in a 0x0 -> (kTileX-2)x(kTileY-2) block. These will be tiled across
        // the screen in kTileX x kTileY tiles. The extra empty pixels on each side are for AA.
        let tile_x = K_TILE_X as f32;
        let tile_y = K_TILE_Y as f32;
        let rr = &mut self.rrects;

        // simple cases
        rr[0].set_rect(Rect::from_wh(tile_x - 2.0, tile_y - 2.0));
        rr[1].set_oval(Rect::from_wh(tile_x - 2.0, tile_y - 2.0));
        rr[2].set_rect_xy(Rect::from_wh(tile_x - 2.0, tile_y - 2.0), 10.0, 10.0);
        rr[3].set_rect_xy(Rect::from_wh(tile_x - 2.0, tile_y - 2.0), 10.0, 5.0);
        // small circular corners are an interesting test case for gpu clipping
        rr[4].set_rect_xy(Rect::from_wh(tile_x - 2.0, tile_y - 2.0), 1.0, 1.0);
        rr[5].set_rect_xy(Rect::from_wh(tile_x - 2.0, tile_y - 2.0), 0.5, 0.5);
        rr[6].set_rect_xy(Rect::from_wh(tile_x - 2.0, tile_y - 2.0), 0.2, 0.2);

        // The first complex case needs special handling since it is a square
        let g_radii = g_radii();
        rr[K_NUM_SIMPLE_CASES]
            .set_rect_radii(Rect::from_wh(tile_y - 2.0, tile_y - 2.0), &g_radii[0]);
        for (i, radii) in g_radii.iter().enumerate().skip(1) {
            rr[K_NUM_SIMPLE_CASES + i]
                .set_rect_radii(Rect::from_wh(tile_x - 2.0, tile_y - 2.0), radii);
        }
        // The last case is larger than kTileX-2 x kTileY-2 but will be drawn at an offset
        // into a clip rect that respects the tile size and highlights the rrect's corner curve.
        rr[K_NUM_RRECTS - 1].set_rect_xy(Rect::new(9.0, 9.0, 1699.0, 1699.0), 843.749, 843.75);
    }
}

impl GM for RRectGM {
    // Port of: gm/rrects.cpp#L59-L79 (chrome/m156)
    fn name(&self) -> String {
        let mut name = String::from("rrect");
        match self.type_ {
            Type::BwDraw => name.push_str("_draw_bw"),
            Type::AaDraw => name.push_str("_draw_aa"),
        }
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(K_IMAGE_WIDTH, K_IMAGE_HEIGHT)
    }

    fn bg_color(&self) -> Color {
        self.bg
    }

    fn on_once_before_draw(&mut self) {
        self.bg = Color::from(0xFFDD_DDDD);
        self.set_up_rrects();
    }

    // Port of: gm/rrects.cpp#L83-L181 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of small constants
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        if Type::AaDraw == self.type_ {
            paint.set_anti_alias(true);
        }

        let mut y = 1;
        // lastEdgeType is 0 for the draw types: a single pass
        let mut x = 1;
        for cur_rrect in 0..K_NUM_RRECTS {
            canvas.save();
            canvas.translate((x as f32, y as f32));

            let rrect = self.rrects[cur_rrect];
            if cur_rrect == K_NUM_RRECTS - 1 {
                canvas.clip_rect(
                    Rect::new(0.0, 0.0, (K_TILE_X - 2) as f32, (K_TILE_Y - 2) as f32),
                    None,
                    None,
                );
                canvas.translate((-0.14 * rrect.rect().width(), -0.14 * rrect.rect().height()));
            }
            canvas.draw_rrect(rrect, &paint);

            canvas.restore();
            x += K_TILE_X;
            if x > K_IMAGE_WIDTH {
                x = 1;
                y += K_TILE_Y;
            }
        }
        if x != 1 {
            y += K_TILE_Y;
        }
        let _ = y;
    }
}

// Radii for the various test cases. Order is UL, UR, LR, LL
// Port of: gm/rrects.cpp#L227-L288 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // kTileY is a small int constant
fn g_radii() -> [[Vector; 4]; K_NUM_COMPLEX_CASES] {
    let t = K_TILE_Y as f32;
    let v = Vector::new;
    let z = v(0.0, 0.0);
    [
        // a circle
        [v(t, t), v(t, t), v(t, t), v(t, t)],
        // odd ball cases
        [v(8.0, 8.0), v(32.0, 32.0), v(8.0, 8.0), v(32.0, 32.0)],
        [v(16.0, 8.0), v(8.0, 16.0), v(16.0, 8.0), v(8.0, 16.0)],
        [z, v(16.0, 16.0), v(8.0, 8.0), v(32.0, 32.0)],
        // UL
        [v(30.0, 30.0), z, z, z],
        [v(30.0, 15.0), z, z, z],
        [v(15.0, 30.0), z, z, z],
        // UR
        [z, v(30.0, 30.0), z, z],
        [z, v(30.0, 15.0), z, z],
        [z, v(15.0, 30.0), z, z],
        // LR
        [z, z, v(30.0, 30.0), z],
        [z, z, v(30.0, 15.0), z],
        [z, z, v(15.0, 30.0), z],
        // LL
        [z, z, z, v(30.0, 30.0)],
        [z, z, z, v(30.0, 15.0)],
        [z, z, z, v(15.0, 30.0)],
        // over-sized radii
        [z, v(100.0, 400.0), z, z],
        [z, v(400.0, 400.0), z, z],
        [
            v(400.0, 400.0),
            v(400.0, 400.0),
            v(400.0, 400.0),
            v(400.0, 400.0),
        ],
        // circular corner tabs
        [z, v(20.0, 20.0), v(20.0, 20.0), z],
        [v(20.0, 20.0), v(20.0, 20.0), z, z],
        [z, z, v(20.0, 20.0), v(20.0, 20.0)],
        [v(20.0, 20.0), z, z, v(20.0, 20.0)],
        // small radius circular corner tabs
        [z, v(0.2, 0.2), v(0.2, 0.2), z],
        [v(0.3, 0.3), v(0.3, 0.3), z, z],
        // single circular corner cases
        [z, z, z, v(15.0, 15.0)],
        [z, z, v(15.0, 15.0), z],
        [z, v(15.0, 15.0), z, z],
        [v(15.0, 15.0), z, z, z],
        // nine patch elliptical
        [v(5.0, 7.0), v(8.0, 7.0), v(8.0, 12.0), v(5.0, 12.0)],
        [v(0.0, 7.0), v(8.0, 7.0), v(8.0, 12.0), v(0.0, 12.0)],
        // nine patch elliptical, small radii
        [v(0.4, 7.0), v(8.0, 7.0), v(8.0, 12.0), v(0.4, 12.0)],
        [v(0.4, 0.4), v(8.0, 0.4), v(8.0, 12.0), v(0.4, 12.0)],
        [v(20.0, 0.4), v(18.0, 0.4), v(18.0, 0.4), v(20.0, 0.4)],
        [v(0.3, 0.4), v(0.3, 0.4), v(0.3, 0.4), v(0.3, 0.4)],
    ]
}

// Port of: gm/rrects.cpp#L292-L296 (chrome/m156)
crate::def_gm!(
    RRectGM_kAA_Draw_Type = "RRectGM(RRectGM::kAA_Draw_Type)",
    RRectGM::new(Type::AaDraw)
);
crate::def_gm!(
    RRectGM_kBW_Draw_Type = "RRectGM(RRectGM::kBW_Draw_Type)",
    RRectGM::new(Type::BwDraw)
);

// This GM is designed to test a variety of fill and stroked rectangles and round rectangles, with
// different stroke width and join type scenarios. The geometry parameters are chosen so that
// Graphite should be able to use its AnalyticRoundRectRenderStep and batch into a single draw.
// Port of: gm/rrects.cpp#L298-L474 (chrome/m156)
#[allow(clippy::too_many_lines, clippy::explicit_counter_loop)] // mirrors the C++ function
fn draw_stroke_rect_rrects(canvas: &Canvas) {
    canvas.scale((0.5, 0.5));
    canvas.translate((50.0, 50.0));

    #[allow(clippy::cast_precision_loss)] // int * float in C++
    let draw = |cx: i32, cy: i32, rrect: bool, width: f32, join: Join| {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_stroke_width(width);
        p.set_style(if width >= 0.0 {
            Style::Stroke
        } else {
            Style::Fill
        });
        p.set_stroke_join(join);

        canvas.save();
        canvas.translate((cx as f32 * 110.0, cy as f32 * 110.0));
        let dx = if cx % 2 != 0 { 0.5 } else { 0.0 };
        let dy = if cy % 2 != 0 { 0.5 } else { 0.0 };
        let mut rect = Rect::from_wh(50.0, 40.0);
        rect.offset((dx, dy));

        if width < 0.0 {
            rect.outset((25.0, 25.0)); // make it the same size as the largest stroke
        }

        // Filled rounded rects can have arbitrary corners
        let corner_scale = rect.width().min(rect.height());
        let outer_radii: [Vector; 4] = [
            Vector::new(0.25 * corner_scale, 0.75 * corner_scale),
            Vector::new(0.0, 0.0),
            Vector::new(0.50 * corner_scale, 0.50 * corner_scale),
            Vector::new(0.75 * corner_scale, 0.25 * corner_scale),
        ];
        // Stroked rounded rects will only have circular corners so that they remain compatible with
        // Graphite's AnalyticRoundRectRenderStep's requirements.
        let stroke_radii: [Vector; 4] = [
            Vector::new(0.25 * corner_scale, 0.25 * corner_scale),
            Vector::new(0.0, 0.0), // this corner matches join type
            Vector::new(0.50 * corner_scale, 0.50 * corner_scale),
            Vector::new(0.75 * corner_scale, 0.75 * corner_scale),
        ];

        if rrect {
            let mut r = RRect::default();
            if width >= 0.0 {
                r.set_rect_radii(rect, &stroke_radii);
            } else {
                r.set_rect_radii(rect, &outer_radii);
            }
            canvas.draw_rrect(r, &p);
        } else {
            canvas.draw_rect(rect, &p);
        }
        canvas.restore();
    };

    // The stroke widths are chosen to test when the inner stroke edges have completely crossed
    // over (50); when the inner corner circles intersect each other (30); a typical "nice"
    // stroke (10); a skinny stroke (1); and a hairline (0).
    let mut i = 0;
    for width in [-1.0f32, 50.0, 30.0, 10.0, 1.0, 0.0] {
        let mut j = 0;
        for join in [Join::Miter, Join::Bevel, Join::Round] {
            if width < 0.0 && join != Join::Miter {
                continue; // Don't repeat fills, since join type is ignored
            }
            draw(2 * i, 2 * j, false, width, join);
            draw(2 * i + 1, 2 * j, false, width, join);
            draw(2 * i, 2 * j + 1, false, width, join);
            draw(2 * i + 1, 2 * j + 1, false, width, join);
            j += 1;
        }
        i += 1;
    }

    canvas.translate((0.0, 50.0));

    i = 0;
    for width in [-1.0f32, 50.0, 30.0, 10.0, 1.0, 0.0] {
        let mut j = 3;
        for join in [Join::Miter, Join::Bevel, Join::Round] {
            if width < 0.0 && join != Join::Miter {
                continue;
            }
            draw(2 * i, 2 * j, true, width, join);
            draw(2 * i + 1, 2 * j, true, width, join);
            draw(2 * i, 2 * j + 1, true, width, join);
            draw(2 * i + 1, 2 * j + 1, true, width, join);
            j += 1;
        }
        i += 1;
    }

    // Rotated "footballs"
    #[allow(clippy::cast_precision_loss)] // int * float in C++
    let draw_complex = |cx: i32, cy: i32, width: f32, stretch: f32| {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_stroke_width(width);
        p.set_style(Style::Stroke);
        p.set_stroke_join(Join::Bevel);

        canvas.save();
        canvas.translate((cx as f32 * 110.0, cy as f32 * 110.0));

        let rect = Rect::from_wh(
            if cx % 2 != 0 { 50.0 } else { 40.0 + stretch },
            if cx % 2 != 0 { 40.0 + stretch } else { 50.0 },
        );
        let k_big_corner = Vector::new(30.0, 30.0);
        let k_rect_corner = Vector::new(0.0, 0.0);

        let stroke_radii: [Vector; 4] = [
            if cy % 2 != 0 {
                k_rect_corner
            } else {
                k_big_corner
            },
            if cy % 2 != 0 {
                k_big_corner
            } else {
                k_rect_corner
            },
            if cy % 2 != 0 {
                k_rect_corner
            } else {
                k_big_corner
            },
            if cy % 2 != 0 {
                k_big_corner
            } else {
                k_rect_corner
            },
        ];

        let mut r = RRect::default();
        r.set_rect_radii(rect, &stroke_radii);
        canvas.draw_rrect(r, &p);

        canvas.restore();
    };

    canvas.translate((0.0, -50.0));
    i = 6;
    for width in [50.0f32, 30.0, 20.0, 10.0, 1.0, 0.0] {
        let mut j = 0;
        for stretch in [0.0f32, 5.0, 10.0] {
            draw_complex(2 * i, 2 * j, width, stretch);
            draw_complex(2 * i + 1, 2 * j, width, stretch);
            draw_complex(2 * i, 2 * j + 1, width, stretch);
            draw_complex(2 * i + 1, 2 * j + 1, width, stretch);
            j += 1;
        }
        i += 1;
    }

    // Rotated "D"s
    #[allow(clippy::cast_precision_loss)] // int * float in C++
    let draw_complex2 = |cx: i32, cy: i32, width: f32, stretch: f32| {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_stroke_width(width);
        p.set_style(Style::Stroke);
        p.set_stroke_join(Join::Miter);

        canvas.save();
        canvas.translate((cx as f32 * 110.0, cy as f32 * 110.0));

        let rect = Rect::from_wh(
            if cx % 2 != 0 { 50.0 } else { 40.0 + stretch },
            if cx % 2 != 0 { 40.0 + stretch } else { 50.0 },
        );
        let k_big_corner = Vector::new(30.0, 30.0);
        let k_rect_corner = Vector::new(0.0, 0.0);

        let xor = (cx % 2 != 0) ^ (cy % 2 != 0);
        let stroke_radii: [Vector; 4] = [
            if cx % 2 != 0 {
                k_rect_corner
            } else {
                k_big_corner
            },
            if xor { k_big_corner } else { k_rect_corner },
            if cx % 2 != 0 {
                k_big_corner
            } else {
                k_rect_corner
            },
            if xor { k_rect_corner } else { k_big_corner },
        ];

        let mut r = RRect::default();
        r.set_rect_radii(rect, &stroke_radii);
        canvas.draw_rrect(r, &p);

        canvas.restore();
    };

    canvas.translate((0.0, 50.0));
    i = 6;
    for width in [50.0f32, 30.0, 20.0, 10.0, 1.0, 0.0] {
        let mut j = 3;
        for stretch in [0.0f32, 5.0, 10.0] {
            draw_complex2(2 * i, 2 * j, width, stretch);
            draw_complex2(2 * i + 1, 2 * j, width, stretch);
            draw_complex2(2 * i, 2 * j + 1, width, stretch);
            draw_complex2(2 * i + 1, 2 * j + 1, width, stretch);
            j += 1;
        }
        i += 1;
    }
}

crate::def_simple_gm!(stroke_rect_rrects, canvas, 1350, 700, {
    draw_stroke_rect_rrects(canvas);
});

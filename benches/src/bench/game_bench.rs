// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/GameBench.cpp

//! `GameBench`: the canvas calls an HTML5 canvas game makes. Scaled, translated or rotated
//! checkerboard tiles or atlas cells are drawn in batches of 100, cleared fully or partially
//! (rendering benches). `CanvasMatrixBench`: repeated `translate`, `scale` or `concat` calls.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::image::Image;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::{SCALAR_SQRT2, int_to_scalar, scalar};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::vertices::{VertexMode, Vertices};

use crate::def_bench;
use crate::prelude::*;

/// `GameBench::Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameType {
    Scale,
    Translate,
    Rotate,
}

/// `GameBench::Clear`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameClear {
    Full,
    Partial,
}

/// `GameBench::kCheckerboardWidth`.
// Port of: bench/GameBench.cpp#L23-L312 (chrome/m156)
const CHECKERBOARD_WIDTH: i32 = 64;
/// `GameBench::kCheckerboardHeight`.
const CHECKERBOARD_HEIGHT: i32 = 128;
/// `GameBench::kAtlasCellWidth`.
const ATLAS_CELL_WIDTH: i32 = 48;
/// `GameBench::kAtlasCellHeight`.
const ATLAS_CELL_HEIGHT: i32 = 36;
/// `GameBench::kNumAtlasedX`.
const NUM_ATLASED_X: usize = 5;
/// `GameBench::kNumAtlasedY`.
const NUM_ATLASED_Y: usize = 5;
/// `GameBench::kAtlasSpacer`.
const ATLAS_SPACER: i32 = 2;
/// `GameBench::kTotAtlasWidth`.
const TOT_ATLAS_WIDTH: i32 = 5 * ATLAS_CELL_WIDTH + (5 + 1) * ATLAS_SPACER;
/// `GameBench::kTotAtlasHeight`.
const TOT_ATLAS_HEIGHT: i32 = 5 * ATLAS_CELL_HEIGHT + (5 + 1) * ATLAS_SPACER;
/// `GameBench::kNumBeforeClear`.
const NUM_BEFORE_CLEAR: usize = 100;

/// `class GameBench`.
// Port of: bench/GameBench.cpp#L23-L312 (chrome/m156)
// Five flags, as in the C++ constructor; they do not form a state machine.
#[allow(clippy::struct_excessive_bools)]
struct GameBench {
    ty: GameType,
    clear: GameClear,
    aligned: bool,
    use_atlas: bool,
    use_draw_vertices: bool,
    name: String,
    /// `fNumSaved`: draws stored in `saved`. It persists across calls, as the member does.
    num_saved: usize,
    initialized: bool,
    /// 0 and 1 are always the x and y translate; 2 is either the scale or the rotate.
    saved: [[scalar; 3]; NUM_BEFORE_CLEAR],
    checkerboard: Option<Image>,
    atlas: Option<Image>,
    atlas_rects: [[IRect; NUM_ATLASED_Y]; NUM_ATLASED_X],
}

impl GameBench {
    // GameBench(Type type, Clear clear, bool aligned = false, bool useAtlas = false,
    //           bool useDrawVertices = false)
    // Port of: bench/GameBench.cpp#L27-L63 (chrome/m156)
    fn new(
        ty: GameType,
        clear: GameClear,
        aligned: bool,
        use_atlas: bool,
        use_draw_vertices: bool,
    ) -> Self {
        // fName("game")
        let mut name = String::from("game");
        // switch (fType) { ... }
        match ty {
            GameType::Scale => name.push_str("_scale"),
            GameType::Translate => name.push_str("_trans"),
            GameType::Rotate => name.push_str("_rot"),
        }
        // if (aligned) fName.append("_aligned");
        if aligned {
            name.push_str("_aligned");
        }
        // if (kPartial_Clear == clear) "_partial" else "_full"
        if clear == GameClear::Partial {
            name.push_str("_partial");
        } else {
            name.push_str("_full");
        }
        // if (useAtlas) fName.append("_atlas");
        if use_atlas {
            name.push_str("_atlas");
        }
        // if (useDrawVertices) fName.append("_drawVerts");
        if use_draw_vertices {
            name.push_str("_drawVerts");
        }
        // It's HTML 5 canvas, so always AA
        name.push_str("_aa");

        Self {
            ty,
            clear,
            aligned,
            use_atlas,
            use_draw_vertices,
            name,
            num_saved: 0,
            initialized: false,
            saved: [[0.0; 3]; NUM_BEFORE_CLEAR],
            checkerboard: None,
            atlas: None,
            atlas_rects: [[IRect::new_empty(); NUM_ATLASED_Y]; NUM_ATLASED_X],
        }
    }

    // makeCheckerboard(): the resulting checkerboard has transparency.
    // Port of: bench/GameBench.cpp#L224-L240 (chrome/m156)
    fn make_checkerboard(&mut self) {
        // static int kCheckSize = 16;
        const CHECK_SIZE: i32 = 16;
        // SkBitmap bm;
        // bm.allocN32Pixels(kCheckerboardWidth, kCheckerboardHeight);
        let mut bm = Bitmap::new();
        bm.alloc_n32_pixels((CHECKERBOARD_WIDTH, CHECKERBOARD_HEIGHT), None);
        for y in 0..CHECKERBOARD_HEIGHT {
            let even = (y / CHECK_SIZE) % 2;
            for x in 0..CHECKERBOARD_WIDTH {
                // *scanline++ = (even == (x / kCheckSize) % 2) ? 0xFFFF0000 : 0x00000000;
                let value = if even == (x / CHECK_SIZE) % 2 {
                    0xFFFF_0000
                } else {
                    0x0000_0000
                };
                bm.set_addr32(x, y, value);
            }
        }
        // fCheckerboard = bm.asImage();
        self.checkerboard = bm.as_image();
    }

    // makeAtlas(): the resulting atlas has transparency.
    // Port of: bench/GameBench.cpp#L242-L290 (chrome/m156)
    // The loops index two arrays by [x][y], as the C++ does.
    #[allow(clippy::needless_range_loop)]
    fn make_atlas(&mut self) {
        // SkRandom rand;
        let mut rand = Random::default();
        // std::array<std::array<SkColor, kNumAtlasedY>, kNumAtlasedX> colors;
        let mut colors = [[0u32; NUM_ATLASED_Y]; NUM_ATLASED_X];
        for y in 0..NUM_ATLASED_Y {
            for x in 0..NUM_ATLASED_X {
                // colors[x][y] = rand.nextU() | 0xff000000;
                colors[x][y] = rand.next_u() | 0xff00_0000;
                // fAtlasRects[x][y] = SkIRect::MakeXYWH(kAtlasSpacer + x * (kAtlasCellWidth +
                //     kAtlasSpacer), kAtlasSpacer + y * (kAtlasCellHeight + kAtlasSpacer),
                //     kAtlasCellWidth, kAtlasCellHeight);
                let xi = i32::try_from(x).expect("atlas index fits in i32");
                let yi = i32::try_from(y).expect("atlas index fits in i32");
                self.atlas_rects[x][y] = IRect::from_xywh(
                    ATLAS_SPACER + xi * (ATLAS_CELL_WIDTH + ATLAS_SPACER),
                    ATLAS_SPACER + yi * (ATLAS_CELL_HEIGHT + ATLAS_SPACER),
                    ATLAS_CELL_WIDTH,
                    ATLAS_CELL_HEIGHT,
                );
            }
        }

        // SkBitmap bm;
        // bm.allocN32Pixels(kTotAtlasWidth, kTotAtlasHeight);
        let mut bm = Bitmap::new();
        bm.alloc_n32_pixels((TOT_ATLAS_WIDTH, TOT_ATLAS_HEIGHT), None);
        for y in 0..TOT_ATLAS_HEIGHT {
            let color_y = y / (ATLAS_CELL_HEIGHT + ATLAS_SPACER);
            let in_color_y = (y % (ATLAS_CELL_HEIGHT + ATLAS_SPACER)) >= ATLAS_SPACER;
            for x in 0..TOT_ATLAS_WIDTH {
                let color_x = x / (ATLAS_CELL_WIDTH + ATLAS_SPACER);
                let in_color_x = (x % (ATLAS_CELL_WIDTH + ATLAS_SPACER)) >= ATLAS_SPACER;
                // *scanline = colors[colorX][colorY] when inside a cell, else transparent.
                let value = if in_color_x && in_color_y {
                    // SkASSERT(colorX < kNumAtlasedX && colorY < kNumAtlasedY);
                    let cx = usize::try_from(color_x).expect("cell inside the atlas");
                    let cy = usize::try_from(color_y).expect("cell inside the atlas");
                    debug_assert!(cx < NUM_ATLASED_X && cy < NUM_ATLASED_Y);
                    colors[cx][cy]
                } else {
                    0x0000_0000
                };
                bm.set_addr32(x, y, value);
            }
        }
        // fAtlas = bm.asImage();
        self.atlas = bm.as_image();
    }
}

impl Benchmark for GameBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDelayedSetup()
    // Port of: bench/GameBench.cpp#L65-L72 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        if !self.initialized {
            self.make_checkerboard();
            self.make_atlas();
            self.initialized = true;
        }
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/GameBench.cpp#L74-L212 (chrome/m156)
    // One function, as in C++ (the per-draw state is shared across the whole body).
    #[allow(clippy::too_many_lines)]
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("GameBench is a rendering bench");
        // SkRandom scaleRand; SkRandom transRand; SkRandom rotRand;
        let mut scale_rand = Random::default();
        let mut trans_rand = Random::default();
        let mut rot_rand = Random::default();

        // int width, height;
        let (width, height) = if self.use_atlas {
            (ATLAS_CELL_WIDTH, ATLAS_CELL_HEIGHT)
        } else {
            (CHECKERBOARD_WIDTH, CHECKERBOARD_HEIGHT)
        };

        // SkPaint clearPaint; clearPaint.setColor(0xFF000000); clearPaint.setAntiAlias(true);
        let mut clear_paint = Paint::default();
        clear_paint.set_color(Color::new(0xFF00_0000));
        clear_paint.set_anti_alias(true);

        // SkISize size = canvas->getBaseLayerSize();
        let size = canvas.base_layer_size();
        // SkScalar maxTransX, maxTransY;
        let (max_trans_x, max_trans_y): (scalar, scalar) = if self.ty == GameType::Scale {
            // maxTransX = size.fWidth  - (1.5f * width);
            (
                int_to_scalar(size.width) - (1.5 * int_to_scalar(width)),
                int_to_scalar(size.height) - (1.5 * int_to_scalar(height)),
            )
        } else if self.ty == GameType::Translate {
            // maxTransX = SkIntToScalar(size.fWidth  - width);
            (
                int_to_scalar(size.width - width),
                int_to_scalar(size.height - height),
            )
        } else {
            // Yes, some rotations will be off the top and left sides
            // maxTransX = size.fWidth  - SK_ScalarSqrt2 * height;
            (
                int_to_scalar(size.width) - SCALAR_SQRT2 * int_to_scalar(height),
                int_to_scalar(size.height) - SCALAR_SQRT2 * int_to_scalar(height),
            )
        };

        // SkMatrix mat;
        let mut mat: Matrix;
        // SkRect dst = { 0, 0, SkIntToScalar(width), SkIntToScalar(height) };
        let dst = Rect::from_ltrb(0.0, 0.0, int_to_scalar(width), int_to_scalar(height));
        // SkRect clearRect = { -1.0f, -1.0f, width+1.0f, height+1.0f };
        let clear_rect = Rect::from_ltrb(
            -1.0,
            -1.0,
            int_to_scalar(width) + 1.0,
            int_to_scalar(height) + 1.0,
        );
        // SkPoint verts[4] = { // for drawVertices path
        //     { 0, 0 }, { 0, SkIntToScalar(height) },
        //     { SkIntToScalar(width), SkIntToScalar(height) }, { SkIntToScalar(width), 0 } };
        let verts = [
            Point::new(0.0, 0.0),
            Point::new(0.0, int_to_scalar(height)),
            Point::new(int_to_scalar(width), int_to_scalar(height)),
            Point::new(int_to_scalar(width), 0.0),
        ];
        // uint16_t indices[6] = { 0, 1, 2, 0, 2, 3 };
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        // SkPaint p; p.setColor(0xFF000000);
        let mut p = Paint::default();
        p.set_color(Color::new(0xFF00_0000));
        // SkPaint p2;         // for drawVertices path
        // p2.setColor(0xFF000000);
        // p2.setShader(fAtlas->makeShader(SkSamplingOptions(SkFilterMode::kLinear)));
        let mut p2 = Paint::default();
        p2.set_color(Color::new(0xFF00_0000));
        let atlas = self.atlas.as_ref().expect("set in onDelayedSetup");
        p2.set_shader(atlas.to_shader(
            (TileMode::Clamp, TileMode::Clamp),
            SamplingOptions::from(FilterMode::Linear),
            None,
        ));

        // for (int i = 0; i < loops; ++i, ++fNumSaved) { ... }
        // kNumBeforeClear as the loop's int type.
        let before_clear = i32::try_from(NUM_BEFORE_CLEAR).expect("constant fits in i32");
        for i in 0..loops {
            // if (0 == i % kNumBeforeClear) { ... }
            if i % before_clear == 0 {
                if self.clear == GameClear::Partial {
                    // for (int j = 0; j < fNumSaved; ++j) { ... }
                    for j in 0..self.num_saved {
                        // canvas->setMatrix(SkMatrix::I());
                        // (SkCanvas::setMatrix(SkMatrix::I()) is SkCanvas::resetMatrix().)
                        canvas.reset_matrix();
                        // mat.setTranslate(fSaved[j][0], fSaved[j][1]);
                        mat = Matrix::translate((self.saved[j][0], self.saved[j][1]));
                        if self.ty == GameType::Scale {
                            // mat.preScale(fSaved[j][2], fSaved[j][2]);
                            mat.pre_scale((self.saved[j][2], self.saved[j][2]), None);
                        } else if self.ty == GameType::Rotate {
                            // mat.preRotate(fSaved[j][2]);
                            mat.pre_rotate(self.saved[j][2], None);
                        }
                        // canvas->concat(mat);
                        canvas.concat(&mat);
                        // canvas->drawRect(clearRect, clearPaint);
                        canvas.draw_rect(clear_rect, &clear_paint);
                    }
                } else {
                    // canvas->clear(0xFF000000);
                    canvas.clear(Color4f::from_color(Color::new(0xFF00_0000)));
                }
                // fNumSaved = 0;
                self.num_saved = 0;
            }

            // SkASSERT(fNumSaved < kNumBeforeClear);
            debug_assert!(self.num_saved < NUM_BEFORE_CLEAR);
            // canvas->setMatrix(SkMatrix::I());
            canvas.reset_matrix();
            let n = self.num_saved;
            // fSaved[fNumSaved][0] = transRand.nextRangeScalar(0.0f, maxTransX);
            self.saved[n][0] = trans_rand.next_range_scalar(0.0, max_trans_x);
            // fSaved[fNumSaved][1] = transRand.nextRangeScalar(0.0f, maxTransY);
            self.saved[n][1] = trans_rand.next_range_scalar(0.0, max_trans_y);
            if self.aligned {
                // make the translations integer aligned
                // fSaved[fNumSaved][0] = SkScalarFloorToScalar(fSaved[fNumSaved][0]);
                self.saved[n][0] = self.saved[n][0].floor();
                // fSaved[fNumSaved][1] = SkScalarFloorToScalar(fSaved[fNumSaved][1]);
                self.saved[n][1] = self.saved[n][1].floor();
            }
            // mat.setTranslate(fSaved[fNumSaved][0], fSaved[fNumSaved][1]);
            mat = Matrix::translate((self.saved[n][0], self.saved[n][1]));
            if self.ty == GameType::Scale {
                // fSaved[fNumSaved][2] = scaleRand.nextRangeScalar(0.5f, 1.5f);
                self.saved[n][2] = scale_rand.next_range_scalar(0.5, 1.5);
                // mat.preScale(fSaved[fNumSaved][2], fSaved[fNumSaved][2]);
                mat.pre_scale((self.saved[n][2], self.saved[n][2]), None);
            } else if self.ty == GameType::Rotate {
                // fSaved[fNumSaved][2] = rotRand.nextRangeScalar(0.0f, 360.0f);
                self.saved[n][2] = rot_rand.next_range_scalar(0.0, 360.0);
                // mat.preRotate(fSaved[fNumSaved][2]);
                mat.pre_rotate(self.saved[n][2], None);
            }
            // canvas->concat(mat);
            canvas.concat(&mat);

            if self.use_atlas {
                // const int curCell = i % (kNumAtlasedX * kNumAtlasedY);
                let cur_cell = usize::try_from(i).expect("loop counter is positive")
                    % (NUM_ATLASED_X * NUM_ATLASED_Y);
                // SkRect src = SkRect::Make(fAtlasRects[curCell % kNumAtlasedX][curCell / kNumAtlasedX]);
                let src = Rect::from(
                    self.atlas_rects[cur_cell % NUM_ATLASED_X][cur_cell / NUM_ATLASED_X],
                );
                if self.use_draw_vertices {
                    // SkPoint uvs[4] = { { src.fLeft, src.fBottom }, { src.fLeft, src.fTop },
                    //                    { src.fRight, src.fTop }, { src.fRight, src.fBottom } };
                    let uvs = [
                        Point::new(src.left(), src.bottom()),
                        Point::new(src.left(), src.top()),
                        Point::new(src.right(), src.top()),
                        Point::new(src.right(), src.bottom()),
                    ];
                    // canvas->drawVertices(SkVertices::MakeCopy(kTriangles, 4, verts, uvs,
                    //                                           nullptr, 6, indices),
                    //                      SkBlendMode::kModulate, p2);
                    let vertices = Vertices::new_copy(
                        VertexMode::Triangles,
                        &verts,
                        Some(&uvs),
                        None,
                        Some(&indices),
                    )
                    .expect("triangle vertices are valid");
                    canvas.draw_vertices(&vertices, BlendMode::Modulate, &p2);
                } else {
                    // canvas->drawImageRect(fAtlas, src, dst, SkSamplingOptions(), &p,
                    //                       SkCanvas::kFast_SrcRectConstraint);
                    canvas.draw_image_rect(atlas, Some((&src, SrcRectConstraint::Fast)), dst, &p);
                }
            } else {
                // canvas->drawImageRect(fCheckerboard, dst, SkSamplingOptions(), &p);
                let checkerboard = self.checkerboard.as_ref().expect("set in onDelayedSetup");
                canvas.draw_image_rect(checkerboard, None, dst, &p);
            }

            // ++fNumSaved (the increment of the for statement)
            self.num_saved += 1;
        }
    }
}

/// `class CanvasMatrixBench`.
// Port of: bench/GameBench.cpp#L332-L385 (chrome/m156)
struct CanvasMatrixBench {
    name: String,
    ty: CanvasMatrixType,
}

/// `CanvasMatrixBench::Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CanvasMatrixType {
    Translate,
    Scale,
    Matrix2x3,
    Matrix3x3,
    Matrix4x4,
}

impl CanvasMatrixBench {
    // CanvasMatrixBench(Type t) : fType(t)
    // Port of: bench/GameBench.cpp#L338-L348 (chrome/m156)
    fn new(ty: CanvasMatrixType) -> Self {
        // fName.set("canvas_matrix");
        let mut name = String::from("canvas_matrix");
        // switch (fType) { ... }
        name.push_str(match ty {
            CanvasMatrixType::Translate => "_trans",
            CanvasMatrixType::Scale => "_scale",
            CanvasMatrixType::Matrix2x3 => "_2x3",
            CanvasMatrixType::Matrix3x3 => "_3x3",
            CanvasMatrixType::Matrix4x4 => "_4x4",
        });
        Self { name, ty }
    }
}

impl Benchmark for CanvasMatrixBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/GameBench.cpp#L356-L384 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("CanvasMatrixBench is a rendering bench");
        // SkMatrix m;
        // m.setRotate(1);
        let mut m = Matrix::rotate_deg(1.0);
        // if (fType == k3x3_Type) m[7] = 0.0001f;
        if self.ty == CanvasMatrixType::Matrix3x3 {
            m[7] = 0.0001;
        }
        // SkM44 m4(m);
        let m4 = M44::from(&m);
        for _ in 0..loops {
            // canvas->save();
            canvas.save();
            // for (int j = 0; j < 10000; ++j) { ... }
            for _ in 0..10000 {
                match self.ty {
                    // canvas->translate(0.0001f, 0.0001f);
                    CanvasMatrixType::Translate => {
                        canvas.translate((0.0001, 0.0001));
                    }
                    // canvas->scale(1.0001f, 0.9999f);
                    CanvasMatrixType::Scale => {
                        canvas.scale((1.0001, 0.9999));
                    }
                    CanvasMatrixType::Matrix2x3 | CanvasMatrixType::Matrix3x3 => {
                        canvas.concat(&m);
                    }
                    CanvasMatrixType::Matrix4x4 => {
                        canvas.concat_44(&m4);
                    }
                }
            }
            // canvas->restore();
            canvas.restore();
        }
    }
}

// Port of: bench/GameBench.cpp#L315-L315 (chrome/m156)
def_bench!(
    game_bench_scale_partial = "GameBench(GameBench::kScale_Type, GameBench::kPartial_Clear)",
    GameBench::new(GameType::Scale, GameClear::Partial, false, false, false)
);
// Port of: bench/GameBench.cpp#L316-L316 (chrome/m156)
def_bench!(
    game_bench_translate_partial =
        "GameBench(GameBench::kTranslate_Type, GameBench::kPartial_Clear)",
    GameBench::new(GameType::Translate, GameClear::Partial, false, false, false)
);
// Port of: bench/GameBench.cpp#L317-L317 (chrome/m156)
def_bench!(
    game_bench_translate_partial_aligned =
        "GameBench(GameBench::kTranslate_Type, GameBench::kPartial_Clear, true)",
    GameBench::new(GameType::Translate, GameClear::Partial, true, false, false)
);
// Port of: bench/GameBench.cpp#L318-L318 (chrome/m156)
def_bench!(
    game_bench_rotate_partial = "GameBench(GameBench::kRotate_Type, GameBench::kPartial_Clear)",
    GameBench::new(GameType::Rotate, GameClear::Partial, false, false, false)
);
// Port of: bench/GameBench.cpp#L321-L321 (chrome/m156)
def_bench!(
    game_bench_scale_full = "GameBench(GameBench::kScale_Type, GameBench::kFull_Clear)",
    GameBench::new(GameType::Scale, GameClear::Full, false, false, false)
);
// Port of: bench/GameBench.cpp#L322-L322 (chrome/m156)
def_bench!(
    game_bench_translate_full = "GameBench(GameBench::kTranslate_Type, GameBench::kFull_Clear)",
    GameBench::new(GameType::Translate, GameClear::Full, false, false, false)
);
// Port of: bench/GameBench.cpp#L323-L323 (chrome/m156)
def_bench!(
    game_bench_translate_full_aligned =
        "GameBench(GameBench::kTranslate_Type, GameBench::kFull_Clear, true)",
    GameBench::new(GameType::Translate, GameClear::Full, true, false, false)
);
// Port of: bench/GameBench.cpp#L324-L324 (chrome/m156)
def_bench!(
    game_bench_rotate_full = "GameBench(GameBench::kRotate_Type, GameBench::kFull_Clear)",
    GameBench::new(GameType::Rotate, GameClear::Full, false, false, false)
);
// Port of: bench/GameBench.cpp#L327-L327 (chrome/m156)
def_bench!(
    game_bench_translate_full_atlas =
        "GameBench(GameBench::kTranslate_Type, GameBench::kFull_Clear, false, true)",
    GameBench::new(GameType::Translate, GameClear::Full, false, true, false)
);
// Port of: bench/GameBench.cpp#L328-L329 (chrome/m156)
def_bench!(
    game_bench_translate_full_atlas_draw_verts =
        "GameBench( GameBench::kTranslate_Type, GameBench::kFull_Clear, false, true, true)",
    GameBench::new(GameType::Translate, GameClear::Full, false, true, true)
);
// Port of: bench/GameBench.cpp#L387-L387 (chrome/m156)
def_bench!(
    canvas_matrix_bench_translate = "CanvasMatrixBench(CanvasMatrixBench::kTranslate_Type)",
    CanvasMatrixBench::new(CanvasMatrixType::Translate)
);
// Port of: bench/GameBench.cpp#L388-L388 (chrome/m156)
def_bench!(
    canvas_matrix_bench_scale = "CanvasMatrixBench(CanvasMatrixBench::kScale_Type)",
    CanvasMatrixBench::new(CanvasMatrixType::Scale)
);
// Port of: bench/GameBench.cpp#L389-L389 (chrome/m156)
def_bench!(
    canvas_matrix_bench_2x3 = "CanvasMatrixBench(CanvasMatrixBench::k2x3_Type)",
    CanvasMatrixBench::new(CanvasMatrixType::Matrix2x3)
);
// Port of: bench/GameBench.cpp#L390-L390 (chrome/m156)
def_bench!(
    canvas_matrix_bench_3x3 = "CanvasMatrixBench(CanvasMatrixBench::k3x3_Type)",
    CanvasMatrixBench::new(CanvasMatrixType::Matrix3x3)
);
// Port of: bench/GameBench.cpp#L391-L391 (chrome/m156)
def_bench!(
    canvas_matrix_bench_4x4 = "CanvasMatrixBench(CanvasMatrixBench::k4x4_Type)",
    CanvasMatrixBench::new(CanvasMatrixType::Matrix4x4)
);

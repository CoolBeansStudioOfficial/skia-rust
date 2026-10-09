// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/VertBench.cpp

//! `VertBench` (the colored, perspective variants): a 20 x 20 grid of triangles with random
//! vertex colors, drawn with `drawVertices` (rendering). The texture variants sample
//! `images/mandrill_256.png`, which needs a PNG decoder (not ported), so they are not
//! registered here (the manifest keeps them `todo`).

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::scalar;
use skia_rust_core::vertices::{VertexMode, Vertices};

use crate::def_bench;
use crate::prelude::*;

/// `VertBench::W`, `H`, `ROW`, `COL`, `PTS` and `IDX`.
// Port of: bench/VertBench.cpp#L37-L42 (chrome/m156)
const W: i32 = 64 * 2;
const H: i32 = 48 * 2;
const ROW: usize = 20;
const COL: usize = 20;
const PTS: usize = (ROW + 1) * (COL + 1);
const IDX: usize = ROW * COL * 6;

/// `VertFlags`: the colored and perspective variants.
// Port of: bench/VertBench.cpp#L26-L31 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VertFlags {
    colors: bool,
    persp: bool,
}

/// `class VertBench` for the flags it is registered with.
// Port of: bench/VertBench.cpp#L33-L114 (chrome/m156)
struct VertBench {
    /// `fName`.
    name: String,
    /// `fPts`: the grid corners.
    pts: Vec<Point>,
    /// `fColors`: a random opaque color per corner.
    colors: Vec<Color>,
    /// `fIdx`: two triangles per cell.
    idx: Vec<u16>,
    flags: VertFlags,
}

/// `VertBench::load_2_tris`.
// Port of: bench/VertBench.cpp#L44-L49 (chrome/m156)
fn load_2_tris(idx: &mut [u16], x: usize, y: usize, rb: usize) {
    let n = y * rb + x;
    let n = u16::try_from(n).expect("a grid index fits in 16 bits");
    let rb = u16::try_from(rb).expect("a row fits in 16 bits");
    idx[0] = n;
    idx[1] = n + 1;
    idx[2] = rb + n + 1;
    idx[3] = n;
    idx[4] = rb + n + 1;
    idx[5] = n + rb;
}

impl VertBench {
    // Port of: bench/VertBench.cpp#L56-L107 (chrome/m156)
    // The grid sizes are small integers converted to scalars, as `SkIntToScalar` does.
    #[allow(clippy::cast_precision_loss)]
    fn new(flags: VertFlags) -> Self {
        let dx = W as scalar / COL as scalar;
        let dy = H as scalar / COL as scalar;
        let mut pts = Vec::with_capacity(PTS);
        let mut idx = vec![0u16; IDX];
        let mut idx_pos = 0;
        let mut yy: scalar = 0.0;
        for y in 0..=ROW {
            let mut xx: scalar = 0.0;
            for x in 0..=COL {
                // pts->set(xx, yy); pts += 1;
                pts.push(Point::new(xx, yy));
                // xx += dx;
                xx += dx;
                if x < COL && y < ROW {
                    // load_2_tris(idx, x, y, COL + 1); idx += 6;
                    load_2_tris(&mut idx[idx_pos..idx_pos + 6], x, y, COL + 1);
                    idx_pos += 6;
                }
            }
            // yy += dy;
            yy += dy;
        }
        // SkASSERT(PTS == pts - fPts); SkASSERT(IDX == idx - fIdx);
        debug_assert_eq!(pts.len(), PTS);
        debug_assert_eq!(idx_pos, IDX);

        // SkRandom rand;
        let mut rand = Random::default();
        let colors = (0..PTS)
            // fColors[i] = rand.nextU() | (0xFF << 24);
            .map(|_| Color::new(rand.next_u() | (0xFF << 24)))
            .collect();

        // fName.set("verts");
        let mut name = "verts".to_owned();
        // if (fFlags & kColors_VertFlag) fName.append("_colors");
        if flags.colors {
            name.push_str("_colors");
        }
        // if (fFlags & kPersp_VertFlag) fName.append("_persp");
        if flags.persp {
            name.push_str("_persp");
        }
        Self {
            name,
            pts,
            colors,
            idx,
            flags,
        }
    }
}

impl Benchmark for VertBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/VertBench.cpp#L84-L104 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("VertBench is a rendering bench");
        // SkPaint paint; this->setupPaint(&paint); paint.setShader(fShader);
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        if self.flags.persp {
            // tiny_persp_effect(canvas): SkMatrix m; m.reset(); m[7] = 0.000001f; canvas->concat(m);
            let mut m = Matrix::default();
            m.reset();
            m[7] = 0.000_001;
            canvas.concat(&m);
        }
        // const SkColor* cols = (fFlags & kColors_VertFlag) ? fColors : nullptr;
        let cols = self.flags.colors.then_some(self.colors.as_slice());
        // auto verts = SkVertices::MakeCopy(kTriangles, PTS, fPts, nullptr, cols, IDX, fIdx);
        let verts = Vertices::new_copy(
            VertexMode::Triangles,
            &self.pts,
            None,
            cols,
            Some(&self.idx),
        )
        .expect("a triangle mesh of the grid");
        for _ in 0..loops {
            // canvas->drawVertices(verts, SkBlendMode::kModulate, paint);
            canvas.draw_vertices(&verts, BlendMode::Modulate, &paint);
        }
    }
}

// Port of: bench/VertBench.cpp#L125-L128 (chrome/m156)
def_bench!(
    vert_colors_persp = "VertBench(kColors_VertFlag | kPersp_VertFlag)",
    VertBench::new(VertFlags {
        colors: true,
        persp: true
    })
);
// Port of: bench/VertBench.cpp#L135 (chrome/m156)
def_bench!(
    vert_colors = "VertBench(kColors_VertFlag)",
    VertBench::new(VertFlags {
        colors: true,
        persp: false
    })
);

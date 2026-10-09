// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PathTextBench.cpp

//! `PathTextBench`: 2000 glyph outlines drawn at random scales, rotations and positions on a
//! 1500 x 1500 canvas, each with its own paint (rendering). The clipped variant needs
//! `ToolUtils::make_star` (not ported), so it is not registered.

use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::scalar;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike_spec::{BulkGlyphMetricsAndPaths, StrikeSpec};
use skia_rust_core::utf::Unichar;
use skia_rust_tools::font_tool_utils::default_font;

use crate::def_bench;
use crate::prelude::*;

/// `kScreenWidth` and `kScreenHeight`.
// Port of: bench/PathTextBench.cpp#L22-L23 (chrome/m156)
const SCREEN_WIDTH: i32 = 1500;
const SCREEN_HEIGHT: i32 = 1500;

/// `kNumDraws`.
// Port of: bench/PathTextBench.cpp#L25 (chrome/m156)
const NUM_DRAWS: usize = 2000;

/// `kGlyphs`: the 52 characters drawn, "I and l are rects on OS X".
// Port of: bench/PathTextBench.cpp#L28-L29 (chrome/m156)
const GLYPHS: &[u8] = b"ABCDEFGH7JKLMNOPQRSTUVWXYZabcdefghijk1mnopqrstuvwxyz";

/// `class PathTextBench`.
// Port of: bench/PathTextBench.cpp#L35-L115 (chrome/m156)
struct PathTextBench {
    clipped: bool,
    uncached: bool,
    /// `fGlyphs`: one path per character.
    glyphs: Vec<Path>,
    /// `fPaints`.
    paints: Vec<Paint>,
    /// `fXforms`.
    xforms: Vec<Matrix>,
}

impl PathTextBench {
    fn new(clipped: bool, uncached: bool) -> Self {
        Self {
            clipped,
            uncached,
            glyphs: vec![Path::default(); GLYPHS.len()],
            paints: vec![Paint::default(); NUM_DRAWS],
            xforms: vec![Matrix::default(); NUM_DRAWS],
        }
    }
}

impl Benchmark for PathTextBench {
    // Port of: bench/PathTextBench.cpp#L40-L48 (chrome/m156)
    fn name(&self) -> String {
        let mut name = "path_text".to_owned();
        if self.clipped {
            name.push_str("_clipped");
        }
        if self.uncached {
            name.push_str("_uncached");
        }
        name
    }

    // Port of: bench/PathTextBench.cpp#L50 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    // Port of: bench/PathTextBench.cpp#L52-L92 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // SkFont defaultFont = ToolUtils::DefaultFont();
        let default_font = default_font();
        // SkStrikeSpec strikeSpec = SkStrikeSpec::MakeWithNoDevice(defaultFont);
        let strike_spec =
            StrikeSpec::make_with_no_device(&default_font, None, ScalerContextBuildFlags::NONE);
        // SkBulkGlyphMetricsAndPaths pathMaker{strikeSpec};
        let path_maker = BulkGlyphMetricsAndPaths::new(&strike_spec);
        for (i, &ch) in GLYPHS.iter().enumerate() {
            // SkGlyphID id(defaultFont.unicharToGlyph(kGlyphs[i]));
            let id = default_font.unichar_to_glyph(Unichar::from(ch));
            // const SkGlyph* glyph = pathMaker.glyph(id);
            let glyph = path_maker.glyph(id);
            // if (glyph->path()) fGlyphs[i] = *glyph->path();
            if let Some(path) = glyph.path() {
                self.glyphs[i] = path.clone();
            }
            // fGlyphs[i].setIsVolatile(fUncached);
            self.glyphs[i].set_is_volatile(self.uncached);
        }

        // SkRandom rand;
        let mut rand = Random::default();
        for i in 0..NUM_DRAWS {
            let glyph = &self.glyphs[i % GLYPHS.len()];
            // const SkRect& bounds = glyph.getBounds();
            let bounds = *glyph.bounds();
            // float glyphSize = std::max(bounds.width(), bounds.height());
            let glyph_size: scalar = bounds.width().max(bounds.height());
            // SkASSERT(glyphSize > 0);
            debug_assert!(glyph_size > 0.0);

            // float t0 = pow(rand.nextF(), 100);
            let t0 = f64::from(rand.next_f()).powf(100.0) as scalar;
            // float size = (1 - t0) * std::min(kScreenWidth, kScreenHeight) / 50 +
            //              t0 * std::min(kScreenWidth, kScreenHeight) / 3;
            let min_screen = SCREEN_WIDTH.min(SCREEN_HEIGHT) as scalar;
            let size = (1.0 - t0) * min_screen / 50.0 + t0 * min_screen / 3.0;
            // float scale = size / glyphSize;
            let scale = size / glyph_size;
            // float t1 = rand.nextF(), t2 = rand.nextF();
            let t1 = rand.next_f();
            let t2 = rand.next_f();
            // The C++ `sqrt(2)` is a double, so these expressions are evaluated in double
            // and narrowed when passed to `setTranslate`.
            let sqrt2 = 2.0_f64.sqrt();
            let scale_d = f64::from(scale);
            let glyph_size_d = f64::from(glyph_size);
            let x = f64::from(1.0 - t1) * sqrt2 * scale_d / 2.0 * glyph_size_d
                + f64::from(t1) * (f64::from(SCREEN_WIDTH) - sqrt2 * scale_d / 2.0 * glyph_size_d);
            let y = f64::from(1.0 - t2) * sqrt2 * scale_d / 2.0 * glyph_size_d
                + f64::from(t2) * (f64::from(SCREEN_HEIGHT) - sqrt2 * scale_d / 2.0 * glyph_size_d);
            let xform = &mut self.xforms[i];
            // fXforms[i].setTranslate(x, y);
            xform.set_translate((x as scalar, y as scalar));
            // fXforms[i].preRotate(rand.nextF() * 360);
            xform.pre_rotate(rand.next_f() * 360.0, None);
            // fXforms[i].preTranslate(-scale/2 * bounds.width(), -scale/2 * bounds.height());
            xform.pre_translate((
                -scale / 2.0 * bounds.width(),
                -scale / 2.0 * bounds.height(),
            ));
            // fXforms[i].preScale(scale, scale);
            xform.pre_scale((scale, scale), None);
            // fPaints[i].setAntiAlias(true);
            self.paints[i].set_anti_alias(true);
            // fPaints[i].setColor(rand.nextU() | 0x80808080);
            self.paints[i].set_color(rand.next_u() | 0x8080_8080);
        }

        // if (fClipped) { fClipPath = ToolUtils::make_star(...); fClipPath.setIsVolatile(...); }
        assert!(
            !self.clipped,
            "the clipped variant needs ToolUtils::make_star, which is not ported"
        );
    }

    // Port of: bench/PathTextBench.cpp#L94-L113 (chrome/m156)
    fn on_draw(&mut self, _loops: i32, canvas: Option<&Canvas>) {
        // The C++ body ignores `loops` and draws every path once.
        let canvas = canvas.expect("PathTextBench is a rendering bench");
        // SkAutoCanvasRestore acr(canvas, true);
        let save_count = canvas.save_count();
        canvas.save();
        for i in 0..NUM_DRAWS {
            let glyph = &self.glyphs[i % GLYPHS.len()];
            // canvas->setMatrix(fXforms[i]);
            canvas.set_matrix(&M44::from(&self.xforms[i]));
            // canvas->drawPath(glyph, fPaints[i]);
            canvas.draw_path(glyph, &self.paints[i]);
        }
        canvas.restore_to_count(save_count);
    }
}

// Port of: bench/PathTextBench.cpp#L117 (chrome/m156)
def_bench!(
    path_text_cached = "PathTextBench(false, false)",
    PathTextBench::new(false, false)
);
// Port of: bench/PathTextBench.cpp#L118 (chrome/m156)
def_bench!(
    path_text_uncached = "PathTextBench(false, true)",
    PathTextBench::new(false, true)
);

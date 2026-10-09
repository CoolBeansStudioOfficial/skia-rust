// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/BlendmodeBench.cpp

//! `XfermodeBench`: text (`mask`) or non-AA rects (`rect`) drawn with one blend mode, one
//! `BENCH(mode)` wrapper per mode. The `sprite` runtime benchmark draws
//! `images/color_wheel.png`, which needs the codec stack (not landed), so each wrapper omits it.

// The int-to-scalar casts mirror the C++ `SkScalar(size.fWidth)` conversions.
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::default_font;

use crate::def_bench_set;
use crate::prelude::*;

/// `enum Type { kText, kRect, kSprite }`. `kSprite` is not ported (see the module docs).
// Port of: bench/BlendmodeBench.cpp#L22-L26 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    Text,
    Rect,
}

/// `gTypeNames`, indexed by [`Type`]: the text type is named `mask`.
// Port of: bench/BlendmodeBench.cpp#L28-L32 (chrome/m156)
fn type_name(t: Type) -> &'static str {
    match t {
        Type::Text => "mask",
        Type::Rect => "rect",
    }
}

/// Benchmark that draws non-AA rects or AA text with an `SkBlendMode`.
// Port of: bench/BlendmodeBench.cpp#L34-L84 (chrome/m156)
struct XfermodeBench {
    blend_mode: BlendMode,
    ty: Type,
    name: String,
}

impl XfermodeBench {
    // Port of: bench/BlendmodeBench.cpp#L36-L41 (chrome/m156)
    fn new(mode: BlendMode, t: Type) -> Self {
        // fName.printf("blendmicro_%s_%s", gTypeNames[t], SkBlendMode_Name(mode));
        let name = format!("blendmicro_{}_{}", type_name(t), mode.name());
        Self {
            blend_mode: mode,
            ty: t,
            name,
        }
    }
}

impl Benchmark for XfermodeBench {
    // Port of: bench/BlendmodeBench.cpp#L43 (chrome/m156)
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/BlendmodeBench.cpp#L51-L100 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("XfermodeBench is a rendering bench");
        let text = "Hamburgefons";
        // SkISize size = canvas->getBaseLayerSize();
        let size = canvas.base_layer_size();
        // SkRandom random;
        let mut random = Random::default();
        let mut loops = loops;
        // while (loops > 0) {
        while loops > 0 {
            // SkPaint paint;
            let mut paint = Paint::default();
            // paint.setBlendMode(fBlendMode);
            paint.set_blend_mode(self.blend_mode);
            // paint.setColor(random.nextU());
            paint.set_color(random.next_u());
            match self.ty {
                Type::Text => {
                    // Draw text to exercise AA code paths.
                    // SkFont font = ToolUtils::DefaultFont();
                    let mut font = default_font();
                    // font.setSize(random.nextRangeScalar(12, 96));
                    font.set_size(random.next_range_scalar(12.0, 96.0));
                    // SkScalar x = random.nextRangeScalar(0, (SkScalar)size.fWidth),
                    //          y = random.nextRangeScalar(0, (SkScalar)size.fHeight);
                    let x = random.next_range_scalar(0.0, size.width as scalar);
                    let y = random.next_range_scalar(0.0, size.height as scalar);
                    // auto blob = SkTextBlob::MakeFromText(text, len, font, SkTextEncoding::kUTF8);
                    let blob = TextBlob::from_text(text.as_bytes(), TextEncoding::UTF8, &font)
                        .expect("MakeFromText of a non-empty UTF-8 string");
                    // int iterations = std::min(1000, loops);
                    let iterations = loops.min(1000);
                    // for (int j = 0; j < iterations; ++j) { canvas->drawTextBlob(blob, x, y, paint); }
                    for _ in 0..iterations {
                        canvas.draw_text_blob(&blob, (x, y), &paint);
                    }
                    // loops -= iterations;
                    loops -= iterations;
                }
                Type::Rect => {
                    // Draw rects to exercise non-AA code paths.
                    // SkScalar w = random.nextRangeScalar(50, 100);
                    let w = random.next_range_scalar(50.0, 100.0);
                    // SkScalar h = random.nextRangeScalar(50, 100);
                    let h = random.next_range_scalar(50.0, 100.0);
                    // SkRect rect = SkRect::MakeXYWH(random.nextUScalar1() * (size.fWidth - w), ...
                    let left = random.next_u_scalar1() * (size.width as scalar - w);
                    let top = random.next_u_scalar1() * (size.height as scalar - h);
                    let rect = Rect::from_xywh(left, top, w, h);
                    // int iterations = std::min(1000, loops);
                    let iterations = loops.min(1000);
                    // for (int j = 0; j < iterations; ++j) { canvas->drawRect(rect, paint); }
                    for _ in 0..iterations {
                        canvas.draw_rect(rect, &paint);
                    }
                    // loops -= iterations;
                    loops -= iterations;
                }
            }
        }
    }
}

/// The `BENCH(mode)` wrapper: `kText` and `kRect` (the `kSprite` runtime benchmark is omitted,
/// see the module docs).
// Port of: bench/BlendmodeBench.cpp#L113-L117 (chrome/m156), the BENCH macro
fn xfermode_benches(mode: BlendMode) -> Vec<Box<dyn Benchmark>> {
    vec![
        Box::new(XfermodeBench::new(mode, Type::Text)) as Box<dyn Benchmark>,
        Box::new(XfermodeBench::new(mode, Type::Rect)) as Box<dyn Benchmark>,
    ]
}

// Registrations: one `BENCH(mode)` per manifest entry, in source order.
// Port of: bench/BlendmodeBench.cpp#L119-L119 (chrome/m156)
def_bench_set!(
    blendmode_bench_clear = "SkBlendMode::kClear",
    xfermode_benches(BlendMode::Clear);
    omitted = ["blendmicro_sprite_Clear"]
);
// Port of: bench/BlendmodeBench.cpp#L120-L120 (chrome/m156)
def_bench_set!(
    blendmode_bench_src = "SkBlendMode::kSrc",
    xfermode_benches(BlendMode::Src);
    omitted = ["blendmicro_sprite_Src"]
);
// Port of: bench/BlendmodeBench.cpp#L121-L121 (chrome/m156)
def_bench_set!(
    blendmode_bench_dst = "SkBlendMode::kDst",
    xfermode_benches(BlendMode::Dst);
    omitted = ["blendmicro_sprite_Dst"]
);
// Port of: bench/BlendmodeBench.cpp#L122-L122 (chrome/m156)
def_bench_set!(
    blendmode_bench_srcover = "SkBlendMode::kSrcOver",
    xfermode_benches(BlendMode::SrcOver);
    omitted = ["blendmicro_sprite_SrcOver"]
);
// Port of: bench/BlendmodeBench.cpp#L123-L123 (chrome/m156)
def_bench_set!(
    blendmode_bench_dstover = "SkBlendMode::kDstOver",
    xfermode_benches(BlendMode::DstOver);
    omitted = ["blendmicro_sprite_DstOver"]
);
// Port of: bench/BlendmodeBench.cpp#L124-L124 (chrome/m156)
def_bench_set!(
    blendmode_bench_srcin = "SkBlendMode::kSrcIn",
    xfermode_benches(BlendMode::SrcIn);
    omitted = ["blendmicro_sprite_SrcIn"]
);
// Port of: bench/BlendmodeBench.cpp#L125-L125 (chrome/m156)
def_bench_set!(
    blendmode_bench_dstin = "SkBlendMode::kDstIn",
    xfermode_benches(BlendMode::DstIn);
    omitted = ["blendmicro_sprite_DstIn"]
);
// Port of: bench/BlendmodeBench.cpp#L126-L126 (chrome/m156)
def_bench_set!(
    blendmode_bench_srcout = "SkBlendMode::kSrcOut",
    xfermode_benches(BlendMode::SrcOut);
    omitted = ["blendmicro_sprite_SrcOut"]
);
// Port of: bench/BlendmodeBench.cpp#L127-L127 (chrome/m156)
def_bench_set!(
    blendmode_bench_dstout = "SkBlendMode::kDstOut",
    xfermode_benches(BlendMode::DstOut);
    omitted = ["blendmicro_sprite_DstOut"]
);
// Port of: bench/BlendmodeBench.cpp#L128-L128 (chrome/m156)
def_bench_set!(
    blendmode_bench_srcatop = "SkBlendMode::kSrcATop",
    xfermode_benches(BlendMode::SrcATop);
    omitted = ["blendmicro_sprite_SrcATop"]
);
// Port of: bench/BlendmodeBench.cpp#L129-L129 (chrome/m156)
def_bench_set!(
    blendmode_bench_dstatop = "SkBlendMode::kDstATop",
    xfermode_benches(BlendMode::DstATop);
    omitted = ["blendmicro_sprite_DstATop"]
);
// Port of: bench/BlendmodeBench.cpp#L130-L130 (chrome/m156)
def_bench_set!(
    blendmode_bench_xor = "SkBlendMode::kXor",
    xfermode_benches(BlendMode::Xor);
    omitted = ["blendmicro_sprite_Xor"]
);
// Port of: bench/BlendmodeBench.cpp#L132-L132 (chrome/m156)
def_bench_set!(
    blendmode_bench_plus = "SkBlendMode::kPlus",
    xfermode_benches(BlendMode::Plus);
    omitted = ["blendmicro_sprite_Plus"]
);
// Port of: bench/BlendmodeBench.cpp#L133-L133 (chrome/m156)
def_bench_set!(
    blendmode_bench_modulate = "SkBlendMode::kModulate",
    xfermode_benches(BlendMode::Modulate);
    omitted = ["blendmicro_sprite_Modulate"]
);
// Port of: bench/BlendmodeBench.cpp#L134-L134 (chrome/m156)
def_bench_set!(
    blendmode_bench_screen = "SkBlendMode::kScreen",
    xfermode_benches(BlendMode::Screen);
    omitted = ["blendmicro_sprite_Screen"]
);
// Port of: bench/BlendmodeBench.cpp#L136-L136 (chrome/m156)
def_bench_set!(
    blendmode_bench_overlay = "SkBlendMode::kOverlay",
    xfermode_benches(BlendMode::Overlay);
    omitted = ["blendmicro_sprite_Overlay"]
);
// Port of: bench/BlendmodeBench.cpp#L137-L137 (chrome/m156)
def_bench_set!(
    blendmode_bench_darken = "SkBlendMode::kDarken",
    xfermode_benches(BlendMode::Darken);
    omitted = ["blendmicro_sprite_Darken"]
);
// Port of: bench/BlendmodeBench.cpp#L138-L138 (chrome/m156)
def_bench_set!(
    blendmode_bench_lighten = "SkBlendMode::kLighten",
    xfermode_benches(BlendMode::Lighten);
    omitted = ["blendmicro_sprite_Lighten"]
);
// Port of: bench/BlendmodeBench.cpp#L139-L139 (chrome/m156)
def_bench_set!(
    blendmode_bench_colordodge = "SkBlendMode::kColorDodge",
    xfermode_benches(BlendMode::ColorDodge);
    omitted = ["blendmicro_sprite_ColorDodge"]
);
// Port of: bench/BlendmodeBench.cpp#L140-L140 (chrome/m156)
def_bench_set!(
    blendmode_bench_colorburn = "SkBlendMode::kColorBurn",
    xfermode_benches(BlendMode::ColorBurn);
    omitted = ["blendmicro_sprite_ColorBurn"]
);
// Port of: bench/BlendmodeBench.cpp#L141-L141 (chrome/m156)
def_bench_set!(
    blendmode_bench_hardlight = "SkBlendMode::kHardLight",
    xfermode_benches(BlendMode::HardLight);
    omitted = ["blendmicro_sprite_HardLight"]
);
// Port of: bench/BlendmodeBench.cpp#L142-L142 (chrome/m156)
def_bench_set!(
    blendmode_bench_softlight = "SkBlendMode::kSoftLight",
    xfermode_benches(BlendMode::SoftLight);
    omitted = ["blendmicro_sprite_SoftLight"]
);
// Port of: bench/BlendmodeBench.cpp#L143-L143 (chrome/m156)
def_bench_set!(
    blendmode_bench_difference = "SkBlendMode::kDifference",
    xfermode_benches(BlendMode::Difference);
    omitted = ["blendmicro_sprite_Difference"]
);
// Port of: bench/BlendmodeBench.cpp#L144-L144 (chrome/m156)
def_bench_set!(
    blendmode_bench_exclusion = "SkBlendMode::kExclusion",
    xfermode_benches(BlendMode::Exclusion);
    omitted = ["blendmicro_sprite_Exclusion"]
);
// Port of: bench/BlendmodeBench.cpp#L145-L145 (chrome/m156)
def_bench_set!(
    blendmode_bench_multiply = "SkBlendMode::kMultiply",
    xfermode_benches(BlendMode::Multiply);
    omitted = ["blendmicro_sprite_Multiply"]
);
// Port of: bench/BlendmodeBench.cpp#L147-L147 (chrome/m156)
def_bench_set!(
    blendmode_bench_hue = "SkBlendMode::kHue",
    xfermode_benches(BlendMode::Hue);
    omitted = ["blendmicro_sprite_Hue"]
);
// Port of: bench/BlendmodeBench.cpp#L148-L148 (chrome/m156)
def_bench_set!(
    blendmode_bench_saturation = "SkBlendMode::kSaturation",
    xfermode_benches(BlendMode::Saturation);
    omitted = ["blendmicro_sprite_Saturation"]
);
// Port of: bench/BlendmodeBench.cpp#L149-L149 (chrome/m156)
def_bench_set!(
    blendmode_bench_color = "SkBlendMode::kColor",
    xfermode_benches(BlendMode::Color);
    omitted = ["blendmicro_sprite_Color"]
);
// Port of: bench/BlendmodeBench.cpp#L150-L150 (chrome/m156)
def_bench_set!(
    blendmode_bench_luminosity = "SkBlendMode::kLuminosity",
    xfermode_benches(BlendMode::Luminosity);
    omitted = ["blendmicro_sprite_Luminosity"]
);

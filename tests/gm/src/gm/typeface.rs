// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/typeface.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::{AutoCanvasRestore, SaveLayerRec};
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{FontHinting, GlyphId, TextEncoding};
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{int_to_scalar, scalar_ceil_to_scalar};
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{
    create_test_typeface, create_typeface_from_resource, default_font, default_portable_typeface,
};

// Port of: gm/typeface.cpp#L14-L25 (chrome/m156), getGlyphPositions
fn get_glyph_positions(font: &Font, glyphs: &[GlyphId], mut x: f32, y: f32, pos: &mut [Point]) {
    let count = glyphs.len();
    let mut widths = vec![0.0_f32; count];
    font.get_widths(glyphs, &mut widths);
    for i in 0..count {
        pos[i] = Point::new(x, y);
        x += widths[i];
    }
}

// Port of: gm/typeface.cpp#L27-L35 (chrome/m156), applyKerning
#[allow(clippy::cast_precision_loss)] // mirrors the int * SkScalar arithmetic of getUnitsPerEm
fn apply_kerning(pos: &mut [Point], adjustments: &[i32], count: usize, font: &Font) {
    let scale = font.size() / font.typeface().units_per_em().unwrap_or(0) as f32;
    let mut global_adj: f32 = 0.0;
    for i in 0..count - 1 {
        global_adj += adjustments[i] as f32 * scale;
        pos[i + 1].x += global_adj;
    }
}

// Port of: gm/typeface.cpp#L37-L56 (chrome/m156), drawKernText
fn draw_kern_text(canvas: &Canvas, text: &[u8], x: f32, y: f32, font: &Font, paint: &Paint) {
    // SkFont never holds a null typeface (`setTypeface(nullptr)` installs the empty typeface), so
    // the C++ `if (!face)` branch is unreachable and the typeface is always present.
    let face = font.typeface();
    let mut glyphs = vec![GlyphId::default(); text.len()];
    let glyph_count = font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);
    if glyph_count < 1 {
        return;
    }
    let mut adjustments = vec![0_i32; glyph_count - 1];
    if !face.get_kerning_pair_adjustments(&glyphs[..glyph_count], &mut adjustments) {
        canvas.draw_simple_text(text, TextEncoding::UTF8, (x, y), font, paint);
        return;
    }
    let mut builder = TextBlobBuilder::new();
    let (run_glyphs, run_points) = builder.alloc_run_pos(font, glyph_count, None);
    run_glyphs.copy_from_slice(&glyphs[..glyph_count]);
    get_glyph_positions(font, &glyphs[..glyph_count], x, y, run_points);
    apply_kerning(run_points, &adjustments, glyph_count, font);
    if let Some(blob) = builder.make() {
        canvas.draw_text_blob(&blob, (0.0, 0.0), paint);
    }
}

// Port of: gm/typeface.cpp#L58-L63 (chrome/m156), gStyles
fn g_styles() -> [FontStyle; 4] {
    [
        FontStyle::normal(),
        FontStyle::bold(),
        FontStyle::italic(),
        FontStyle::bold_italic(),
    ]
}

// Port of: gm/typeface.cpp#L67-L100 (chrome/m156), TypefaceStylesGM
struct TypefaceStylesGm {
    faces: Vec<Typeface>,
    apply_kerning: bool,
}

impl TypefaceStylesGm {
    // Port of: gm/typeface.cpp#L77 (chrome/m156), the constructor
    fn new(apply_kerning: bool) -> Self {
        Self {
            faces: Vec::new(),
            apply_kerning,
        }
    }
}

impl GM for TypefaceStylesGm {
    // Port of: gm/typeface.cpp#L79-L84 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.faces = g_styles()
            .into_iter()
            .map(|style| create_test_typeface(None, style))
            .collect();
    }

    // Port of: gm/typeface.cpp#L86-L93 (chrome/m156), getName
    fn name(&self) -> String {
        if self.apply_kerning {
            "typefacestyles_kerning".to_owned()
        } else {
            "typefacestyles".to_owned()
        }
    }

    // Port of: gm/typeface.cpp#L94 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/typeface.cpp#L96-L117 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // Need to use a font to get dy below.
        let mut font = default_font();
        font.set_size(30.0);
        let text = if self.apply_kerning {
            "Type AWAY"
        } else {
            "Hamburgefons"
        };
        let x = int_to_scalar(10);
        let dy = font.metrics().0;
        let mut y = dy;
        if self.apply_kerning {
            font.set_subpixel(true);
        } else {
            font.set_linear_metrics(true);
        }
        let paint = Paint::default();
        for face in &self.faces {
            font.set_typeface(Some(face.clone()));
            canvas.draw_simple_text(text.as_bytes(), TextEncoding::UTF8, (x, y), &font, &paint);
            if self.apply_kerning {
                draw_kern_text(canvas, text.as_bytes(), x + 240.0, y, &font, &paint);
            }
            y += dy;
        }
    }
}

// Port of: gm/typeface.cpp#L119-L120 (chrome/m156), TypefaceStylesGM(false) registration
crate::def_gm!(
    TypefaceStylesGM_false = "TypefaceStylesGM(false)",
    TypefaceStylesGm::new(false)
);
// Port of: gm/typeface.cpp#L119-L120 (chrome/m156), TypefaceStylesGM(true) registration
crate::def_gm!(
    TypefaceStylesGM_true = "TypefaceStylesGM(true)",
    TypefaceStylesGm::new(true)
);

#[derive(Clone, Copy)]
struct AliasType {
    edging: Edging,
    in_layer: bool,
}

#[derive(Clone, Copy)]
struct SubpixelType {
    requested: bool,
    offset: (f32, f32),
}

#[derive(Clone, Copy)]
struct StyleTests {
    style: Style,
    stroke_width: f32,
}

#[derive(Clone, Copy)]
struct MaskTests {
    style: BlurStyle,
    sigma: f32,
}

// Port of: gm/typeface.cpp#L122-L368 (chrome/m156), draw_typeface_rendering_gm
#[allow(clippy::too_many_lines)] // mirrors the single C++ function body
fn draw_typeface_rendering_gm(canvas: &Canvas, face: &Typeface, glyph: GlyphId) {
    // This gm crashes on iOS when drawing an embedded bitmap when requesting aliased rendering,
    // so the iOS-only exclusion is not applied here (`#ifndef SK_BUILD_FOR_IOS`).
    let alias_types = [
        AliasType {
            edging: Edging::Alias,
            in_layer: false,
        },
        AliasType {
            edging: Edging::AntiAlias,
            in_layer: false,
        },
        AliasType {
            edging: Edging::SubpixelAntiAlias,
            in_layer: false,
        },
        AliasType {
            edging: Edging::AntiAlias,
            in_layer: true,
        },
        AliasType {
            edging: Edging::SubpixelAntiAlias,
            in_layer: true,
        },
    ];
    // The hintgasp.ttf is designed for the following sizes to be different.
    // Odd sizes have embedded bitmaps.
    let text_sizes: [f32; 8] = [9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0];
    let hinting_types = [
        FontHinting::None,
        FontHinting::Slight,
        FontHinting::Normal,
        FontHinting::Full,
    ];
    let subpixel_types = [
        SubpixelType {
            requested: false,
            offset: (0.00, 0.00),
        },
        SubpixelType {
            requested: true,
            offset: (0.00, 0.00),
        },
        SubpixelType {
            requested: true,
            offset: (0.25, 0.00),
        },
        SubpixelType {
            requested: true,
            offset: (0.25, 0.25),
        },
    ];
    let rotate_a_bit_types = [false, true];
    let glyph_bytes = glyph.to_ne_bytes();

    let mut y: f32 = 0.0; // The baseline of the previous output
    {
        let paint = Paint::default();
        let mut font = Font::from_typeface(Some(face.clone()));
        font.set_embedded_bitmaps(true);
        let mut x: f32 = 0.0;
        let mut x_max = x;
        let mut x_base: f32 = 0.0;
        for subpixel in subpixel_types {
            y = 0.0;
            font.set_subpixel(subpixel.requested);
            for alias in alias_types {
                font.set_edging(alias.edging);
                let _acr1 = AutoCanvasRestore::guard(canvas, false);
                if alias.in_layer {
                    canvas.save_layer(&SaveLayerRec::default().paint(&paint));
                }
                for text_size in text_sizes {
                    x = x_base + 5.0;
                    font.set_size(text_size);
                    let dy = scalar_ceil_to_scalar(font.metrics().0);
                    y += dy;
                    for hinting in hinting_types {
                        font.set_hinting(hinting);
                        for rotate_a_bit in rotate_a_bit_types {
                            let _acr2 = AutoCanvasRestore::guard(canvas, true);
                            if rotate_a_bit {
                                canvas.rotate(
                                    2.0,
                                    Some(Point::new(x + subpixel.offset.0, y + subpixel.offset.1)),
                                );
                            }
                            canvas.draw_simple_text(
                                glyph_bytes,
                                TextEncoding::GlyphId,
                                (x + subpixel.offset.0, y + subpixel.offset.1),
                                &font,
                                &paint,
                            );
                            let dx = scalar_ceil_to_scalar(
                                font.measure_text(&glyph_bytes, TextEncoding::GlyphId, None)
                                    .0,
                            ) + 5.0;
                            x += dx;
                            // std::max(x, xMax)
                            x_max = if x < x_max { x_max } else { x };
                        }
                    }
                }
                y += 10.0;
            }
            x_base = x_max;
        }
    }

    let style_types = [
        StyleTests {
            style: Style::Fill,
            stroke_width: 0.0,
        },
        StyleTests {
            style: Style::Stroke,
            stroke_width: 0.0,
        },
        StyleTests {
            style: Style::Stroke,
            stroke_width: 0.5,
        },
        StyleTests {
            style: Style::StrokeAndFill,
            stroke_width: 1.0,
        },
    ];
    let fake_bold_types = [false, true];
    {
        let mut paint = Paint::default();
        let mut font = Font::from_size(face.clone(), 16.0);
        let mut x: f32;
        for fake_bold in fake_bold_types {
            let dy = scalar_ceil_to_scalar(font.metrics().0);
            y += dy;
            x = 5.0;
            font.set_embolden(fake_bold);
            for alias in alias_types {
                font.set_edging(alias.edging);
                let _acr = AutoCanvasRestore::guard(canvas, false);
                if alias.in_layer {
                    canvas.save_layer(&SaveLayerRec::default().paint(&paint));
                }
                for style in style_types {
                    paint.set_style(style.style);
                    paint.set_stroke_width(style.stroke_width);
                    canvas.draw_simple_text(
                        glyph_bytes,
                        TextEncoding::GlyphId,
                        (x, y),
                        &font,
                        &paint,
                    );
                    let dx = scalar_ceil_to_scalar(
                        font.measure_text(&glyph_bytes, TextEncoding::GlyphId, None)
                            .0,
                    ) + 5.0;
                    x += dx;
                }
            }
            y += 10.0;
        }
    }

    let mask_types = [
        MaskTests {
            style: BlurStyle::Normal,
            sigma: 0.0,
        },
        MaskTests {
            style: BlurStyle::Solid,
            sigma: 0.0,
        },
        MaskTests {
            style: BlurStyle::Outer,
            sigma: 0.0,
        },
        MaskTests {
            style: BlurStyle::Inner,
            sigma: 0.0,
        },
        MaskTests {
            style: BlurStyle::Normal,
            sigma: 0.5,
        },
        MaskTests {
            style: BlurStyle::Solid,
            sigma: 0.5,
        },
        MaskTests {
            style: BlurStyle::Outer,
            sigma: 0.5,
        },
        MaskTests {
            style: BlurStyle::Inner,
            sigma: 0.5,
        },
        MaskTests {
            style: BlurStyle::Normal,
            sigma: 2.0,
        },
        MaskTests {
            style: BlurStyle::Solid,
            sigma: 2.0,
        },
        MaskTests {
            style: BlurStyle::Outer,
            sigma: 2.0,
        },
        MaskTests {
            style: BlurStyle::Inner,
            sigma: 2.0,
        },
    ];
    {
        let mut paint = Paint::default();
        let mut font = Font::from_size(face.clone(), 16.0);
        let mut x: f32;
        for alias in alias_types {
            let dy = scalar_ceil_to_scalar(font.metrics().0);
            y += dy;
            x = 5.0;
            font.set_edging(alias.edging);
            let _acr = AutoCanvasRestore::guard(canvas, false);
            if alias.in_layer {
                canvas.save_layer(&SaveLayerRec::default().paint(&paint));
            }
            for mask in mask_types {
                paint.set_mask_filter(MaskFilter::blur(mask.style, mask.sigma, None));
                canvas.draw_simple_text(glyph_bytes, TextEncoding::GlyphId, (x, y), &font, &paint);
                let dx = scalar_ceil_to_scalar(
                    font.measure_text(&glyph_bytes, TextEncoding::GlyphId, None)
                        .0,
                ) + 5.0;
                x += dx;
            }
            paint.set_mask_filter(None);
        }
        y += 10.0;
    }
    // the C++ final `y += 10` is never read
    let _ = y;
}

// Port of: gm/typeface.cpp#L370-L380 (chrome/m156), typefacerendering
crate::def_simple_gm_can_fail!(typefacerendering, canvas, error_msg, 640, 840, {
    let Some(face) = create_typeface_from_resource(None, 0) else {
        return DrawResult::Skip;
    };
    draw_typeface_rendering_gm(canvas, &face, face.unichar_to_glyph('A' as i32));
    // Should draw nothing and not do anything undefined.
    draw_typeface_rendering_gm(canvas, &face, 0xFFFF);
    DrawResult::Ok
});

// Type1 fonts don't currently work in Skia on Windows.
// Port of: gm/typeface.cpp#L385-L392 (chrome/m156), typefacerendering_pfa
crate::def_simple_gm_can_fail!(typefacerendering_pfa, canvas, error_msg, 640, 840, {
    let Some(face) = create_typeface_from_resource(None, 0) else {
        return DrawResult::Skip;
    };
    draw_typeface_rendering_gm(canvas, &face, face.unichar_to_glyph('O' as i32));
    DrawResult::Ok
});

// Port of: gm/typeface.cpp#L394-L401 (chrome/m156), typefacerendering_pfb
crate::def_simple_gm_can_fail!(typefacerendering_pfb, canvas, error_msg, 640, 840, {
    let Some(face) = create_typeface_from_resource(None, 0) else {
        return DrawResult::Skip;
    };
    draw_typeface_rendering_gm(canvas, &face, face.unichar_to_glyph('O' as i32));
    DrawResult::Ok
});

// Exercise different paint styles and embolden, and compare with strokeandfill patheffect
// Port of: gm/typeface.cpp#L403-L455 (chrome/m156), typeface_styling
crate::def_simple_gm!(typeface_styling, canvas, 710, 360, {
    let face = create_typeface_from_resource(None, 0).unwrap_or_else(default_portable_typeface);
    let mut font = Font::from_size(face.clone(), 100.0);
    font.set_edging(Edging::AntiAlias);
    let glyphs = [face.unichar_to_glyph('A' as i32)];
    let pos = [Point::new(0.0, 0.0)];

    // Draws 3 rows:
    //  1. normal
    //  2. emboldened
    //  3. normal(white) on top of emboldened (to show the delta)
    let mut draw = |style: Style, width: f32| {
        let mut paint = Paint::default();
        paint.set_style(style);
        paint.set_stroke_width(width);
        font.set_embolden(true);
        canvas.draw_glyphs_at(&glyphs, &pos[..], (20.0, 120.0 * 2.0), &font, &paint);
        canvas.draw_glyphs_at(&glyphs, &pos[..], (20.0, 120.0 * 3.0), &font, &paint);
        font.set_embolden(false);
        canvas.draw_glyphs_at(&glyphs, &pos[..], (20.0, 120.0 * 1.0), &font, &paint);
        paint.set_color(Color::YELLOW);
        canvas.draw_glyphs_at(&glyphs, &pos[..], (20.0, 120.0 * 3.0), &font, &paint);
    };

    let recs = [
        (Style::Fill, 0.0_f32),
        (Style::Stroke, 0.0),
        (Style::Stroke, 3.0),
        (Style::StrokeAndFill, 0.0),
        (Style::StrokeAndFill, 3.0),
    ];
    canvas.translate((0.0, -20.0));
    for (style, width) in recs {
        draw(style, width);
        canvas.translate((100.0, 0.0));
    }
});

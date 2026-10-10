// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fontmgr.cpp (chrome/m156)

// Single-letter names mirror the C++ GM (x, y, i, j).
#![allow(clippy::many_single_char_names)]
// The glyph index and the int-to-scalar casts mirror the C++ conversions.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_metrics::Flags;
use skia_rust_core::font_mgr::{CMapEntry, FontMgr, FontStyleSet, Request};
use skia_rust_core::font_priv::get_font_bounds;
use skia_rust_core::font_style::{FontStyle, Slant};
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::graphics::set_font_cache_limit;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::utf::Unichar;
use skia_rust_effects::dash_path_effect;
use skia_rust_tools::font_tool_utils::{
    default_font, default_portable_typeface, default_typeface, test_font_mgr,
};

// limit this just so we don't take too long to draw
// Port of: gm/fontmgr.cpp#L37 (chrome/m156)
const MAX_FAMILIES: usize = 30;

// Port of: gm/fontmgr.cpp#L39-L43 (chrome/m156)
fn draw_string(canvas: &Canvas, text: &str, x: scalar, y: scalar, font: &Font) -> scalar {
    canvas.draw_str(text, (x, y), font, &Paint::default());
    x + font
        .measure_text(text.as_bytes(), TextEncoding::UTF8, None)
        .0
}

// Port of: gm/fontmgr.cpp#L45-L92 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn draw_character(
    canvas: &Canvas,
    character: Unichar,
    x: scalar,
    y: scalar,
    orig_font: &Font,
    fm: &FontMgr,
    font_name: Option<&str>,
    bcp47: &[&str],
    font_style: &FontStyle,
) -> scalar {
    let mut x = x;
    let mut font = orig_font.clone();
    // find typeface containing the requested character and draw it
    let ch = char::from_u32(character as u32)
        .unwrap_or('\u{FFFD}')
        .to_string();
    let typeface = fm.match_family_style_character(font_name, font_style, bcp47, character);
    font.set_typeface(typeface.clone());
    x = draw_string(canvas, &ch, x, y, &font) + 20.0;

    // repeat the process, but this time use the family name of the typeface
    // from the first pass.  This emulates the behavior in Blink where it
    // it expects to get the same glyph when following this pattern.
    if let Some(typeface) = &typeface {
        let family_name = typeface.family_name();
        font.set_typeface(fm.legacy_make_typeface(Some(&family_name), typeface.font_style()));
        x = draw_string(canvas, &ch, x, y, &font) + 20.0;
    }

    // find typeface containing the requested character and draw it
    let cmap_entry = [CMapEntry {
        character,
        variation: 0,
    }];
    let model = Request::set_model(*font_style);
    let request = Request {
        cmap_entries: &cmap_entry,
        bcp47,
        family_name: font_name,
        model: &model,
        synthetic_bold: None,
        synthetic_oblique: None,
    };
    let typeface1 = fm.fallback(&request);
    font.set_typeface(typeface1);
    x = draw_string(canvas, &ch, x, y, &font) + 20.0;

    x
}

// Port of: gm/fontmgr.cpp#L94-L95 (chrome/m156)
const ZH: &str = "zh";
const JA: &str = "ja";

// Port of: gm/fontmgr.cpp#L97-L152 (chrome/m156), FontMgrGM
#[derive(Default)]
struct FontMgrGm {
    fm: Option<FontMgr>,
}

impl GM for FontMgrGm {
    // Port of: gm/fontmgr.cpp#L100-L103 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let _ = set_font_cache_limit(16 * 1024 * 1024);
        self.fm = Some(test_font_mgr());
    }

    // Port of: gm/fontmgr.cpp#L105 (chrome/m156), getName
    fn name(&self) -> String {
        "fontmgr_iter".to_string()
    }

    // Port of: gm/fontmgr.cpp#L107 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1536, 768)
    }

    // Port of: gm/fontmgr.cpp#L109-L151 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let mut y: scalar = 20.0;
        let mut font = default_font();
        font.set_edging(Edging::SubpixelAntiAlias);
        font.set_subpixel(true);
        font.set_size(17.0);

        let Some(fm) = &self.fm else {
            return DrawResult::Fail;
        };
        let count = fm.count_families().min(MAX_FAMILIES);
        if count == 0 {
            *error_msg = "No families in SkFontMgr".to_string();
            return DrawResult::Skip;
        }

        for i in 0..count {
            let family_name = fm.family_name(i);
            font.set_typeface(Some(default_typeface()));
            let _ = draw_string(canvas, &family_name, 20.0, y, &font);

            let mut x: scalar = 220.0;

            let set = fm.create_style_set(i);
            for j in 0..set.count() {
                let (fs, mut sname) = set.get_style(j);
                sname.push_str(&format!(
                    " [{} {} {}]",
                    *fs.weight(),
                    *fs.width(),
                    fs.slant() as i32
                ));

                font.set_typeface(set.create_typeface(j));
                x = draw_string(canvas, &sname, x, y, &font) + 20.0;

                // check to see that we get different glyphs in japanese and chinese
                x = draw_character(
                    canvas,
                    0x5203,
                    x,
                    y,
                    &font,
                    fm,
                    Some(&family_name),
                    &[ZH],
                    &fs,
                );
                x = draw_character(
                    canvas,
                    0x5203,
                    x,
                    y,
                    &font,
                    fm,
                    Some(&family_name),
                    &[JA],
                    &fs,
                );
                // check that emoji characters are found
                x = draw_character(
                    canvas,
                    0x1f601,
                    x,
                    y,
                    &font,
                    fm,
                    Some(&family_name),
                    &[],
                    &fs,
                );
            }
            y += 24.0;
        }
        DrawResult::Ok
    }
}

// Port of: gm/fontmgr.cpp#L154-L252 (chrome/m156), FontMgrMatchGM
#[derive(Default)]
struct FontMgrMatchGm {
    fm: Option<FontMgr>,
}

impl FontMgrMatchGm {
    // Port of: gm/fontmgr.cpp#L170-L192 (chrome/m156), iterateFamily
    fn iterate_family(&self, canvas: &Canvas, font: &Font, fset: &FontStyleSet) {
        let Some(fm) = &self.fm else {
            return;
        };
        let mut f = font.clone();
        let mut y: scalar = 0.0;

        for j in 0..fset.count() {
            let (fs, mut sname) = fset.get_style(j);

            sname.push_str(&format!(" [{} {}]", *fs.weight(), *fs.width()));

            f.set_typeface(fset.create_typeface(j));
            let mut x: scalar = 0.0;
            x = draw_string(canvas, &sname, x, y, &f) + 20.0;
            // check to see that we get different glyphs in japanese and chinese
            // and the style matches with no name
            x = draw_character(canvas, 0x5203, x, y, font, fm, None, &[ZH], &fs);
            let _ = draw_character(canvas, 0x5203, x, y, font, fm, None, &[JA], &fs);
            y += 24.0;
        }
    }

    // Port of: gm/fontmgr.cpp#L194-L216 (chrome/m156), exploreFamily
    fn explore_family(&self, canvas: &Canvas, font: &Font, fset: &FontStyleSet) {
        let Some(fm) = &self.fm else {
            return;
        };
        let mut f = font.clone();
        let mut y: scalar = 0.0;

        for weight in (100..=900).step_by(200) {
            for width in (1..=9).step_by(2) {
                let fs = FontStyle::new(weight.into(), width.into(), Slant::Upright);
                let face = fset.match_style(&fs);
                if face.is_some() {
                    let str = format!("request [{} {}]", *fs.weight(), *fs.width());
                    f.set_typeface(face);
                    let mut x: scalar = 0.0;
                    x = draw_string(canvas, &str, x, y, &f) + 20.0;
                    // check to see that we get different glyphs in japanese and chinese
                    // and the style matches with no name
                    x = draw_character(canvas, 0x5203, x, y, font, fm, None, &[ZH], &fs);
                    let _ = draw_character(canvas, 0x5203, x, y, font, fm, None, &[JA], &fs);
                    y += 24.0;
                }
            }
        }
    }
}

impl GM for FontMgrMatchGm {
    // Port of: gm/fontmgr.cpp#L157-L160 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.fm = Some(test_font_mgr());
        let _ = set_font_cache_limit(16 * 1024 * 1024);
    }

    // Port of: gm/fontmgr.cpp#L162 (chrome/m156), getName
    fn name(&self) -> String {
        "fontmgr_match".to_string()
    }

    // Port of: gm/fontmgr.cpp#L164 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(640, 1024)
    }

    // Port of: gm/fontmgr.cpp#L218-L251 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let mut font = default_font();
        font.set_edging(Edging::SubpixelAntiAlias);
        font.set_subpixel(true);
        font.set_size(17.0);

        const G_NAMES: [&str; 4] = ["Helvetica Neue", "Arial", "sans", "Roboto"];

        let Some(fm) = &self.fm else {
            return DrawResult::Fail;
        };
        let mut fset = None;
        for name in G_NAMES {
            let set = fm.match_family(Some(name));
            let found = set.count() > 0;
            fset = Some(set);
            if found {
                break;
            }
        }
        let fset = match fset {
            Some(fset) if fset.count() > 0 => fset,
            _ => {
                *error_msg = "No SkFontStyleSet".to_string();
                return DrawResult::Skip;
            }
        };

        canvas.translate((20.0, 40.0));
        self.explore_family(canvas, &font, &fset);
        canvas.translate((350.0, 0.0));
        self.iterate_family(canvas, &font, &fset);
        DrawResult::Ok
    }
}

// Port of: gm/fontmgr.cpp#L254-L412 (chrome/m156), FontMgrBoundsGM
struct FontMgrBoundsGm {
    fm: Option<FontMgr>,
    scale_x: scalar,
    skew_x: scalar,
    label_bounds: bool,
}

impl FontMgrBoundsGm {
    // Port of: gm/fontmgr.cpp#L256 (chrome/m156), the constructor
    fn new(scale: scalar, skew: scalar) -> Self {
        Self {
            fm: None,
            scale_x: scale,
            skew_x: skew,
            label_bounds: false,
        }
    }

    // Port of: gm/fontmgr.cpp#L279-L378 (chrome/m156), show_bounds
    fn show_bounds(
        canvas: &Canvas,
        font: &Font,
        x: scalar,
        y: scalar,
        bounds_color: Color,
        label_bounds: bool,
    ) -> Rect {
        let mut left: GlyphId = 0;
        let mut right: GlyphId = 0;
        let mut top: GlyphId = 0;
        let mut bottom: GlyphId = 0;
        let mut min = Rect::from_ltrb(
            scalar::INFINITY,
            scalar::INFINITY,
            scalar::NEG_INFINITY,
            scalar::NEG_INFINITY,
        );
        {
            let num_glyphs = font.typeface().count_glyphs();
            for i in 0..num_glyphs {
                let glyph_id = i as GlyphId;
                let mut cur = [Rect::default()];
                font.get_widths_bounds(&[glyph_id], &mut [], &mut cur, None);
                let cur = cur[0];
                if cur.left < min.left {
                    min.left = cur.left;
                    left = glyph_id;
                }
                if cur.top < min.top {
                    min.top = cur.top;
                    top = glyph_id;
                }
                if min.right < cur.right {
                    min.right = cur.right;
                    right = glyph_id;
                }
                if min.bottom < cur.bottom {
                    min.bottom = cur.bottom;
                    bottom = glyph_id;
                }
            }
        }

        let font_bounds = get_font_bounds(font);

        let mut draw_bounds = min;
        draw_bounds.join(font_bounds);

        let acr = AutoCanvasRestore::guard(canvas, true);
        let canvas = &*acr;
        canvas.translate((x - draw_bounds.left(), y));

        let mut bounds_paint = Paint::default();
        bounds_paint.set_anti_alias(true);
        bounds_paint.set_color(bounds_color);
        bounds_paint.set_style(Style::Stroke);
        canvas.draw_rect(font_bounds, &bounds_paint);

        let intervals: [scalar; 2] = [10.0, 10.0];
        bounds_paint.set_path_effect(dash_path_effect::new(&intervals, 0.0));
        canvas.draw_rect(min, &bounds_paint);

        let (_, fm) = font.metrics();
        let mut metrics_paint = bounds_paint.clone();
        metrics_paint.set_style(Style::Fill);
        metrics_paint.set_alpha_f(0.25);
        if fm.flags.contains(Flags::UNDERLINE_POSITION_IS_VALID)
            && fm.flags.contains(Flags::UNDERLINE_THICKNESS_IS_VALID)
        {
            let underline = Rect::from_ltrb(
                min.left,
                fm.underline_position,
                min.right,
                fm.underline_position + fm.underline_thickness,
            );
            canvas.draw_rect(underline, &metrics_paint);
        }

        if fm.flags.contains(Flags::STRIKEOUT_POSITION_IS_VALID)
            && fm.flags.contains(Flags::STRIKEOUT_THICKNESS_IS_VALID)
        {
            let strikeout = Rect::from_ltrb(
                min.left,
                fm.strikeout_position - fm.strikeout_thickness,
                min.right,
                fm.strikeout_position,
            );
            canvas.draw_rect(strikeout, &metrics_paint);
        }

        // Port of: gm/fontmgr.cpp#L336-L346 (chrome/m156), GlyphToDraw
        struct GlyphToDraw {
            id: GlyphId,
            location: Point,
            rotation: scalar,
        }
        let glyphs_to_draw = [
            GlyphToDraw {
                id: left,
                location: Point::new(min.left(), min.center_y()),
                rotation: 270.0,
            },
            GlyphToDraw {
                id: right,
                location: Point::new(min.right(), min.center_y()),
                rotation: 90.0,
            },
            GlyphToDraw {
                id: top,
                location: Point::new(min.center_x(), min.top()),
                rotation: 0.0,
            },
            GlyphToDraw {
                id: bottom,
                location: Point::new(min.center_x(), min.bottom()),
                rotation: 180.0,
            },
        ];

        let mut label_font = Font::default();
        label_font.set_edging(Edging::AntiAlias);
        label_font.set_typeface(Some(default_portable_typeface()));

        if label_bounds {
            let name = font.typeface().family_name();
            canvas.draw_str(
                &name,
                (min.left, min.bottom),
                &label_font,
                &Paint::default(),
            );
        }
        for glyph_to_draw in &glyphs_to_draw {
            let path = font.get_path(glyph_to_draw.id).unwrap_or_else(Path::new);
            let style = if path.is_empty() {
                Style::Fill
            } else {
                Style::Stroke
            };
            let mut glyph_paint = Paint::default();
            glyph_paint.set_style(style);
            canvas.draw_simple_text(
                glyph_to_draw.id.to_ne_bytes(),
                TextEncoding::GlyphId,
                (0.0, 0.0),
                font,
                &glyph_paint,
            );

            if label_bounds {
                let acr2 = AutoCanvasRestore::guard(canvas, true);
                acr2.translate((glyph_to_draw.location.x, glyph_to_draw.location.y));
                acr2.rotate(glyph_to_draw.rotation, None);
                let glyph_str = i32::from(glyph_to_draw.id).to_string();
                acr2.draw_str(&glyph_str, (0.0, 0.0), &label_font, &Paint::default());
            }
        }

        draw_bounds
    }
}

impl GM for FontMgrBoundsGm {
    // Port of: gm/fontmgr.cpp#L260-L265 (chrome/m156), getName
    fn name(&self) -> String {
        if self.scale_x != 1.0 || self.skew_x != 0.0 {
            return format!("fontmgr_bounds_{}_{}", self.scale_x, self.skew_x);
        }
        "fontmgr_bounds".to_string()
    }

    // Port of: gm/fontmgr.cpp#L267 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.fm = Some(test_font_mgr());
    }

    // Port of: gm/fontmgr.cpp#L380 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1024, 850)
    }

    // Port of: gm/fontmgr.cpp#L382-L424 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let mut font = default_font();
        font.set_edging(Edging::AntiAlias);
        font.set_subpixel(true);
        font.set_size(100.0);
        font.set_scale_x(self.scale_x);
        font.set_skew_x(self.skew_x);

        let bounds_colors: [Color; 2] = [Color::RED, Color::BLUE];

        let Some(fm) = &self.fm else {
            return DrawResult::Fail;
        };
        let count = fm.count_families();
        if count == 0 {
            *error_msg = "No families in SkFontMgr under test.".to_string();
            return DrawResult::Skip;
        }

        let mut index: usize = 0;
        let mut x: scalar = 0.0;
        let mut y: scalar = 0.0;

        canvas.translate((10.0, 120.0));

        let mut typefaces_visited = 0;
        let mut i = 0;
        while i < count && typefaces_visited < 32 {
            let set = fm.create_style_set(i);
            let mut styles_visited = 0;
            let mut j = 0;
            while j < set.count() && typefaces_visited < 32 && styles_visited < 3 {
                font.set_typeface(set.create_typeface(j));
                // Fonts with lots of glyphs are interesting, but can take a long time to find
                // the glyphs which make up the maximum extent.
                let glyph_count = font.typeface().count_glyphs();
                if 0 < glyph_count && glyph_count < 1000 {
                    typefaces_visited += 1;
                    styles_visited += 1;

                    let color = bounds_colors[index & 1];
                    let draw_bounds =
                        Self::show_bounds(canvas, &font, x, y, color, self.label_bounds);
                    x += draw_bounds.width() + 20.0;
                    index += 1;
                    if x > 900.0 {
                        x = 0.0;
                        y += 160.0;
                    }
                    if y >= 700.0 {
                        return DrawResult::Ok;
                    }
                }
                j += 1;
            }
            i += 1;
        }
        DrawResult::Ok
    }
}

// Port of: gm/fontmgr.cpp#L418 (chrome/m156)
crate::def_gm!(FontMgrGM, FontMgrGm::default());
// Port of: gm/fontmgr.cpp#L419 (chrome/m156)
crate::def_gm!(FontMgrMatchGM, FontMgrMatchGm::default());
// Port of: gm/fontmgr.cpp#L420 (chrome/m156)
crate::def_gm!(
    FontMgrBoundsGM_1_0 = "FontMgrBoundsGM(1, 0)",
    FontMgrBoundsGm::new(1.0, 0.0)
);
// Port of: gm/fontmgr.cpp#L421 (chrome/m156)
crate::def_gm!(
    FontMgrBoundsGM_0_75_0 = "FontMgrBoundsGM(0.75f, 0)",
    FontMgrBoundsGm::new(0.75, 0.0)
);
// Port of: gm/fontmgr.cpp#L422 (chrome/m156)
crate::def_gm!(
    FontMgrBoundsGM_1_n0_25 = "FontMgrBoundsGM(1, -0.25f)",
    FontMgrBoundsGm::new(1.0, -0.25)
);

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/text/TextValue.h, TextValue.cpp (chrome/m156)

use skia_rust_core::paint::Join;
use skia_rust_core::point::Vector;
use skia_rust_core::utils::text_utils::Align;

use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::Value;
use crate::skottie::LoggerLevel;
use crate::skottie_json::{Parse, ValueExt, parse_default, string_text};
use crate::skottie_property::{TextPaintOrder, TextPropertyValue};
use crate::skottie_value::{ColorValue, VectorValue};
use crate::text_shaper::{Capitalization, Direction, LinebreakPolicy, ResizePolicy, VAlign};

/// The value of a text property.
// Port of: modules/skottie/src/text/TextValue.h#L16 (chrome/m156) (`TextValue`)
pub type TextValue = TextPropertyValue;

/// Parses a text value. Unlike other types, `TextValue` parsing requires access to an
/// `AnimationBuilder`.
// Port of: modules/skottie/src/text/TextValue.cpp#L33-L231 (chrome/m156) (`Parse`)
#[must_use]
#[allow(clippy::too_many_lines)] // one function in C++ as well
#[allow(clippy::items_after_statements)] // mirrors the function-local tables of C++
pub fn parse(jv: &Value, abuilder: &AnimationBuilder<'_>, v: &mut TextValue) -> bool {
    let Some(jtxt) = jv.as_object() else {
        return false;
    };

    let font_name = jtxt.get("f").as_string();
    let text = jtxt.get("t").as_string();
    let text_size = jtxt.get("s").as_number();
    let line_height = jtxt.get("lh").as_number();
    let (Some(font_name), Some(text), Some(text_size), Some(line_height)) =
        (font_name, text, text_size, line_height)
    else {
        return false;
    };

    let font_name = string_text(font_name);
    let Some(font) = abuilder.find_font(&font_name) else {
        abuilder.log(
            LoggerLevel::Error,
            &format!("Unknown font: \"{font_name}\"."),
        );
        return false;
    };

    v.text = string_text(text);
    #[allow(clippy::cast_possible_truncation)] // mirrors the double to float conversion
    {
        v.text_size = text_size.value() as f32;
        v.line_height = line_height.value() as f32;
    }
    v.typeface.clone_from(&font.typeface);
    v.font_family.clone_from(&font.family);
    v.ascent = font.ascent_pct * -0.01_f32 * v.text_size; // negative ascent per SkFontMetrics
    drop(font);
    v.line_shift = parse_default::<f32>(jtxt.get("ls"), 0.0);
    v.text_tracking = parse_default::<f32>(jtxt.get("tr"), 0.0);

    const ALIGN_MAP: [Align; 3] = [
        Align::Left,   // 'j': 0
        Align::Right,  // 'j': 1
        Align::Center, // 'j': 2
    ];
    v.h_align = ALIGN_MAP[parse_default::<usize>(jtxt.get("j"), 0).min(ALIGN_MAP.len() - 1)];

    // Optional text box size.
    if let Some(jsz) = jtxt.get("sz").as_array()
        && jsz.size() == 2
    {
        v.box_.set_wh(
            parse_default::<f32>(&jsz[0], 0.0),
            parse_default::<f32>(&jsz[1], 0.0),
        );
    }

    // Optional text box position.
    if let Some(jps) = jtxt.get("ps").as_array()
        && jps.size() == 2
    {
        v.box_.offset(Vector::new(
            parse_default::<f32>(&jps[0], 0.0),
            parse_default::<f32>(&jps[1], 0.0),
        ));
    }

    const DIRECTION_MAP: [Direction; 2] = [
        Direction::LTR, // 'd': 0
        Direction::RTL, // 'd': 1
    ];
    v.direction =
        DIRECTION_MAP[parse_default::<usize>(jtxt.get("d"), 0).min(DIRECTION_MAP.len() - 1)];

    const RESIZE_MAP: [ResizePolicy; 3] = [
        ResizePolicy::None,           // 'rs': 0
        ResizePolicy::ScaleToFit,     // 'rs': 1
        ResizePolicy::DownscaleToFit, // 'rs': 2
    ];
    // TODO: remove "sk_rs" support after migrating clients.
    v.resize = RESIZE_MAP[parse_default::<usize>(jtxt.get("rs"), 0)
        .max(parse_default::<usize>(jtxt.get("sk_rs"), 0))
        .min(RESIZE_MAP.len() - 1)];

    // Optional min/max font size and line count (used when aute-resizing)
    v.min_text_size = parse_default::<f32>(jtxt.get("mf"), 0.0);
    v.max_text_size = parse_default::<f32>(jtxt.get("xf"), f32::MAX);
    v.max_lines = parse_default::<usize>(jtxt.get("xl"), 0);

    // At the moment, BM uses the paragraph box to discriminate point mode vs. paragraph mode.
    v.line_break = if v.box_.is_empty() {
        LinebreakPolicy::Explicit
    } else {
        LinebreakPolicy::Paragraph
    };

    // Optional explicit text mode.
    // N.b.: this is not being exported by BM, only used for testing.
    let text_mode = parse_default::<i32>(jtxt.get("m"), -1);
    if text_mode >= 0 {
        // Explicit text mode.
        v.line_break = if text_mode == 0 {
            LinebreakPolicy::Explicit // 'm': 0 -> point text
        } else {
            LinebreakPolicy::Paragraph // 'm': 1 -> paragraph text
        };
    }

    // Optional capitalization.
    const CAP_MAP: [Capitalization; 2] = [
        Capitalization::None,      // 'ca': 0
        Capitalization::UpperCase, // 'ca': 1
    ];
    v.capitalization = CAP_MAP[parse_default::<usize>(jtxt.get("ca"), 0).min(CAP_MAP.len() - 1)];

    // In point mode, the text is baseline-aligned.
    v.v_align = if v.box_.is_empty() {
        VAlign::TopBaseline
    } else {
        VAlign::Top
    };

    const VALIGN_MAP: [VAlign; 6] = [
        VAlign::HybridTop,    // 'vj': 0
        VAlign::HybridCenter, // 'vj': 1
        VAlign::HybridBottom, // 'vj': 2
        VAlign::VisualTop,    // 'vj': 3
        VAlign::VisualCenter, // 'vj': 4
        VAlign::VisualBottom, // 'vj': 5
    ];
    let mut vj = 0_usize;
    if usize::parse(jtxt.get("vj"), &mut vj) {
        if vj < VALIGN_MAP.len() {
            v.v_align = VALIGN_MAP[vj];
        } else {
            abuilder.log(
                LoggerLevel::Warning,
                &format!("Ignoring unknown 'vj' value: {vj}"),
            );
        }
    } else if usize::parse(jtxt.get("sk_vj"), &mut vj) {
        // Legacy sk_vj values.
        // TODO: remove after clients update.
        match vj {
            0..=2 => {
                v.v_align = VALIGN_MAP[vj];
            }
            3 => {
                // 'sk_vj': 3 -> kHybridCenter/kScaleToFit
                v.v_align = VAlign::HybridCenter;
                v.resize = ResizePolicy::ScaleToFit;
            }
            4 => {
                // 'sk_vj': 4 -> kHybridCenter/kDownscaleToFit
                v.v_align = VAlign::HybridCenter;
                v.resize = ResizePolicy::DownscaleToFit;
            }
            _ => {
                abuilder.log(
                    LoggerLevel::Warning,
                    &format!("Ignoring unknown 'sk_vj' value: {vj}"),
                );
            }
        }
    }

    let parse_color = |jcolor: &Value, c: &mut skia_rust_core::color::Color| {
        // `const skjson::ArrayValue* jcolor`: a null pointer if it is not an array.
        if jcolor.as_array().is_none() {
            return false;
        }

        let mut color_vec = ColorValue::new();
        if !VectorValue::parse(jcolor, &mut color_vec) {
            return false;
        }

        *c = color_vec.to_color();
        true
    };

    v.has_fill = parse_color(jtxt.get("fc"), &mut v.fill_color);
    v.has_stroke = parse_color(jtxt.get("sc"), &mut v.stroke_color);

    if v.has_stroke {
        v.stroke_width = parse_default::<f32>(jtxt.get("sw"), 1.0);
        v.paint_order = if parse_default::<bool>(jtxt.get("of"), true) {
            TextPaintOrder::FillStroke
        } else {
            TextPaintOrder::StrokeFill
        };

        const JOINS: [Join; 3] = [
            Join::Miter, // lj: 1
            Join::Round, // lj: 2
            Join::Bevel, // lj: 3
        ];
        // `ParseDefault<size_t>(...) - 1` wraps for 0, which pins to the last join.
        v.stroke_join = JOINS[parse_default::<usize>(jtxt.get("lj"), 1)
            .wrapping_sub(1)
            .min(JOINS.len() - 1)];
    }

    true
}

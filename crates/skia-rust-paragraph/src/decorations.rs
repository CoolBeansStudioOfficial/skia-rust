// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/src/Decorations.h,
// modules/skparagraph/src/Decorations.cpp

//! `Decorations`: the underlines, overlines and strikethroughs of a text style.

use skia_rust_core::color::Color;
use skia_rust_core::font_metrics::{Flags as FontMetricsFlags, FontMetrics};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_core::typeface::Typeface;

use crate::paragraph_painter::{DashPathEffect, DecorationStyle, ParagraphPainter};
use crate::text_line::ClipContext;
use crate::text_style::{
    ALL_TEXT_DECORATIONS, TextDecoration, TextDecorationMode, TextDecorationStyle, TextStyle,
};

// Port of: modules/skparagraph/src/Decorations.cpp#L14-L22 (chrome/m156)
fn draw_line_as_rect(
    painter: &mut dyn ParagraphPainter,
    x: scalar,
    y: scalar,
    width: scalar,
    decor_style: &DecorationStyle,
) {
    debug_assert!(decor_style.paint().path_effect().is_none());
    debug_assert!(decor_style.paint().stroke_cap() == skia_rust_core::paint::Cap::Butt);
    // this trick won't work for hairlines
    debug_assert!(decor_style.paint().stroke_width() > 0.0);
    let radius = decor_style.stroke_width() * 0.5;
    painter.draw_filled_rect(
        &Rect::new(x, y - radius, x + width, y + radius),
        decor_style,
    );
}

// Port of: modules/skparagraph/src/Decorations.cpp#L24 (chrome/m156)
const DOUBLE_DECORATION_SPACING: scalar = 3.0;

// Port of: modules/skparagraph/src/Decorations.h#L18-L34 (chrome/m156)
/// `Decorations`: paints the decorations of one piece of text.
#[derive(Debug, Default)]
pub struct Decorations {
    thickness: scalar,
    position: scalar,
    font_metrics: FontMetrics,
    decor_style: DecorationStyle,
    path: Path,
}

impl Decorations {
    // Port of: modules/skparagraph/src/Decorations.cpp#L28-L117 (chrome/m156)
    /// `paint(painter, textStyle, context, baseline)`.
    pub fn paint(
        &mut self,
        painter: &mut dyn ParagraphPainter,
        text_style: &TextStyle,
        context: &ClipContext<'_>,
        baseline: scalar,
    ) {
        if text_style.decoration_type() == TextDecoration::NO_DECORATION {
            return;
        }

        // Get thickness and position
        self.calculate_thickness(text_style.clone(), context.run.font().typeface().clone());

        for decoration in ALL_TEXT_DECORATIONS {
            if (text_style.decoration_type() & decoration).is_empty() {
                continue;
            }

            self.calculate_position(
                decoration,
                if decoration == TextDecoration::OVERLINE {
                    context.run.correct_ascent() - context.run.ascent()
                } else {
                    context.run.correct_ascent()
                },
            );

            self.calculate_paint(text_style);

            let width = context.clip.width();
            let x = context.clip.left();
            let y = context.clip.top() + self.position;

            let draw_gaps = text_style.decoration_mode() == TextDecorationMode::Gaps
                && text_style.decoration_type() == TextDecoration::UNDERLINE;

            match text_style.decoration_style() {
                TextDecorationStyle::Wavy => {
                    self.calculate_waves(text_style, context.clip);
                    self.path = self.path.make_offset((x, y));
                    painter.draw_path(&self.path, &self.decor_style);
                }
                TextDecorationStyle::Double => {
                    let bottom = y + DOUBLE_DECORATION_SPACING;
                    if draw_gaps {
                        let left = x - context.text_shift;
                        painter.translate(context.text_shift, 0.0);
                        self.calculate_gaps(
                            context,
                            &Rect::from_xywh(left, y, width, self.thickness),
                            baseline,
                            self.thickness,
                        );
                        painter.draw_path(&self.path, &self.decor_style);
                        self.calculate_gaps(
                            context,
                            &Rect::from_xywh(left, bottom, width, self.thickness),
                            baseline,
                            self.thickness,
                        );
                        painter.draw_path(&self.path, &self.decor_style);
                    } else {
                        draw_line_as_rect(painter, x, y, width, &self.decor_style);
                        draw_line_as_rect(painter, x, bottom, width, &self.decor_style);
                    }
                }
                TextDecorationStyle::Dashed | TextDecorationStyle::Dotted => {
                    if draw_gaps {
                        let left = x - context.text_shift;
                        painter.translate(context.text_shift, 0.0);
                        self.calculate_gaps(
                            context,
                            &Rect::from_xywh(left, y, width, self.thickness),
                            baseline,
                            0.0,
                        );
                        painter.draw_path(&self.path, &self.decor_style);
                    } else {
                        painter.draw_line(x, y, x + width, y, &self.decor_style);
                    }
                }
                TextDecorationStyle::Solid => {
                    if draw_gaps {
                        let left = x - context.text_shift;
                        painter.translate(context.text_shift, 0.0);
                        self.calculate_gaps(
                            context,
                            &Rect::from_xywh(left, y, width, self.thickness),
                            baseline,
                            self.thickness,
                        );
                        painter.draw_path(&self.path, &self.decor_style);
                    } else {
                        draw_line_as_rect(painter, x, y, width, &self.decor_style);
                    }
                }
            }
        }
    }

    // Port of: modules/skparagraph/src/Decorations.cpp#L119-L158 (chrome/m156)
    fn calculate_gaps(
        &mut self,
        context: &ClipContext<'_>,
        rect: &Rect,
        baseline: scalar,
        halo: scalar,
    ) {
        // Create a special text blob for decorations
        let mut builder = TextBlobBuilder::new();
        context.run.copy_to(&mut builder, context.pos, context.size);
        let Some(blob) = builder.make() else {
            // There is no text really
            return;
        };

        // Since we do not shift down the text by {baseline}
        // (it now happens on drawTextBlob but we do not draw text here)
        // we have to shift up the bounds to compensate
        // This baseline thing ends with getIntercepts
        let bounds = [rect.top - baseline, rect.bottom - baseline];
        let decor_paint = self.decor_style.paint();
        let intersections = blob.get_intercepts(bounds, Some(decor_paint));

        let mut path = PathBuilder::new();
        let mut start = rect.left;
        path.move_to((rect.left, rect.top));
        let mut i = 0;
        while i < intersections.len() {
            let end = intersections[i] - halo;
            if end - start >= halo {
                start = intersections[i + 1] + halo;
                path.line_to((end, rect.top)).move_to((start, rect.top));
            }
            i += 2;
        }
        if !intersections.is_empty() && (rect.right - start > halo) {
            path.line_to((rect.right, rect.top));
        }
        self.path = path.detach();
    }

    // Port of: modules/skparagraph/src/Decorations.cpp#L160-L178 (chrome/m156)
    /// This is how flutter calculates the thickness.
    fn calculate_thickness(&mut self, mut text_style: TextStyle, typeface: Typeface) {
        text_style.set_typeface(Some(typeface));
        self.font_metrics = text_style.font_metrics();
        self.thickness = text_style.font_size() / 14.0;
        if self
            .font_metrics
            .flags
            .contains(FontMetricsFlags::UNDERLINE_THICKNESS_IS_VALID)
            && self.font_metrics.underline_thickness > 0.0
        {
            self.thickness = self.font_metrics.underline_thickness;
        }

        if text_style.decoration_type() == TextDecoration::LINE_THROUGH
            && self
                .font_metrics
                .flags
                .contains(FontMetricsFlags::STRIKEOUT_THICKNESS_IS_VALID)
            && self.font_metrics.strikeout_thickness > 0.0
        {
            self.thickness = self.font_metrics.strikeout_thickness;
        }
        self.thickness *= text_style.decoration_thickness_multiplier();
    }

    // Port of: modules/skparagraph/src/Decorations.cpp#L180-L207 (chrome/m156)
    /// This is how flutter calculates the positioning.
    fn calculate_position(&mut self, decoration: TextDecoration, ascent: scalar) {
        if decoration == TextDecoration::UNDERLINE {
            if self
                .font_metrics
                .flags
                .contains(FontMetricsFlags::UNDERLINE_POSITION_IS_VALID)
                && self.font_metrics.underline_position > 0.0
            {
                self.position = self.font_metrics.underline_position;
            } else {
                self.position = self.thickness;
            }
            self.position -= ascent;
        } else if decoration == TextDecoration::OVERLINE {
            self.position = -ascent;
        } else if decoration == TextDecoration::LINE_THROUGH {
            self.position = if self
                .font_metrics
                .flags
                .contains(FontMetricsFlags::STRIKEOUT_POSITION_IS_VALID)
            {
                self.font_metrics.strikeout_position
            } else {
                self.font_metrics.x_height / -2.0
            };
            self.position -= ascent;
        } else {
            debug_assert!(false);
        }
    }

    // Port of: modules/skparagraph/src/Decorations.cpp#L209-L235 (chrome/m156)
    fn calculate_paint(&mut self, text_style: &TextStyle) {
        let mut dash_path_effect: Option<DashPathEffect> = None;
        let scale_factor = text_style.font_size() / 14.0;
        match text_style.decoration_style() {
            // Note: the intervals are scaled by the thickness of the line, so it is
            // possible to change spacing by changing the decoration_thickness
            // property of TextStyle.
            TextDecorationStyle::Dotted => {
                dash_path_effect =
                    Some(DashPathEffect::new(1.0 * scale_factor, 1.5 * scale_factor));
            }
            // Note: the intervals are scaled by the thickness of the line, so it is
            // possible to change spacing by changing the decoration_thickness
            // property of TextStyle.
            TextDecorationStyle::Dashed => {
                dash_path_effect =
                    Some(DashPathEffect::new(4.0 * scale_factor, 2.0 * scale_factor));
            }
            _ => {}
        }

        let color: Color = if text_style.decoration_color() == Color::TRANSPARENT {
            text_style.color()
        } else {
            text_style.decoration_color()
        };
        self.decor_style = DecorationStyle::new(color, self.thickness, dash_path_effect);
    }

    // Port of: modules/skparagraph/src/Decorations.cpp#L237-L275 (chrome/m156)
    fn calculate_waves(&mut self, _text_style: &TextStyle, clip: Rect) {
        let mut builder = PathBuilder::new();
        let mut wave_count: i32 = 0;
        let mut x_start: scalar = 0.0;
        let quarter_wave = self.thickness;
        builder.move_to((0.0, 0.0));
        while x_start + quarter_wave * 2.0 < clip.width() {
            builder.r_quad_to(
                (
                    quarter_wave,
                    if wave_count % 2 != 0 {
                        quarter_wave
                    } else {
                        -quarter_wave
                    },
                ),
                (quarter_wave * 2.0, 0.0),
            );
            x_start += quarter_wave * 2.0;
            wave_count += 1;
        }

        // The rest of the wave
        let remaining = clip.width() - x_start;
        if remaining > 0.0 {
            // These are doubles that hold floats and are passed as floats again.
            let x1 = f64::from(remaining / 2.0);
            let y1 = f64::from(remaining / 2.0 * (if wave_count % 2 == 0 { -1.0 } else { 1.0 }));
            let x2 = f64::from(remaining);
            let y2 = f64::from(
                (remaining - remaining * remaining / (quarter_wave * 2.0))
                    * (if wave_count % 2 == 0 { -1.0 } else { 1.0 }),
            );
            builder.r_quad_to((x1 as scalar, y1 as scalar), (x2 as scalar, y2 as scalar));
        }
        self.path = builder.detach();
    }
}

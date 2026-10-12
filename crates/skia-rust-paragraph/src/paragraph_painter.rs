// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/ParagraphPainter.h,
// modules/skparagraph/src/ParagraphPainterImpl.h, modules/skparagraph/src/ParagraphPainterImpl.cpp

//! `ParagraphPainter`: what a paragraph draws with, and `CanvasParagraphPainter`, which draws on a
//! [`Canvas`].

use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_effects::{dash_path_effect, discrete_path_effect};

// Port of: modules/skparagraph/include/ParagraphPainter.h#L22 (chrome/m156)
/// `ParagraphPainter::PaintID`.
#[doc(alias = "ParagraphPainter::PaintID")]
pub type PaintId = i32;

// Port of: modules/skparagraph/include/ParagraphPainter.h#L23 (chrome/m156)
/// `ParagraphPainter::SkPaintOrID`: a paint, or the id of one for a custom painter.
#[doc(alias = "ParagraphPainter::SkPaintOrID")]
#[derive(Debug, Clone, PartialEq)]
pub enum PaintOrId {
    /// An `SkPaint`.
    Paint(Paint),
    /// A `PaintID`.
    Id(PaintId),
}

impl Default for PaintOrId {
    // `std::variant<SkPaint, PaintID>` default-constructs its first alternative.
    fn default() -> Self {
        PaintOrId::Paint(Paint::default())
    }
}

// Port of: modules/skparagraph/include/ParagraphPainter.h#L25-L28 (chrome/m156)
/// `ParagraphPainter::DashPathEffect`.
#[doc(alias = "ParagraphPainter::DashPathEffect")]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DashPathEffect {
    /// `fOnLength`.
    pub on_length: scalar,
    /// `fOffLength`.
    pub off_length: scalar,
}

impl DashPathEffect {
    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L18-L19 (chrome/m156)
    /// `DashPathEffect(SkScalar onLength, SkScalar offLength)`.
    #[must_use]
    pub fn new(on_length: scalar, off_length: scalar) -> Self {
        Self {
            on_length,
            off_length,
        }
    }
}

// Port of: modules/skparagraph/include/ParagraphPainter.h#L30-L47 (chrome/m156)
/// `ParagraphPainter::DecorationStyle`: the paint of a decoration.
#[doc(alias = "ParagraphPainter::DecorationStyle")]
#[derive(Debug, Clone)]
pub struct DecorationStyle {
    color: Color,
    stroke_width: scalar,
    dash_path_effect: Option<DashPathEffect>,
    paint: Paint,
}

impl Default for DecorationStyle {
    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L21-L22 (chrome/m156)
    fn default() -> Self {
        Self::new(Color::TRANSPARENT, 0.0, None)
    }
}

impl DecorationStyle {
    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L24-L42 (chrome/m156)
    /// `DecorationStyle(color, strokeWidth, dashPathEffect)`.
    #[must_use]
    pub fn new(
        color: Color,
        stroke_width: scalar,
        dash_path_effect: Option<DashPathEffect>,
    ) -> Self {
        let mut paint = Paint::default();
        paint.set_style(skia_rust_core::paint::Style::Stroke);
        paint.set_anti_alias(true);
        paint.set_color(color);
        paint.set_stroke_width(stroke_width);
        if let Some(dash) = &dash_path_effect {
            let intervals = [
                dash.on_length,
                dash.off_length,
                dash.on_length,
                dash.off_length,
            ];
            // `SkPathEffect::MakeCompose(SkDashPathEffect::Make(intervals, 0.0f),
            // SkDiscretePathEffect::Make(0, 0))`: the discrete effect is null for a zero segment
            // length, and `MakeCompose` returns the other effect when one is null.
            let dash = dash_path_effect::new(&intervals, 0.0);
            let discrete = discrete_path_effect::new(0.0, 0.0, None);
            let composed = match (dash, discrete) {
                (Some(outer), Some(inner)) => Some(PathEffect::compose(outer, inner)),
                (Some(effect), None) | (None, Some(effect)) => Some(effect),
                (None, None) => None,
            };
            paint.set_path_effect(composed);
        }
        Self {
            color,
            stroke_width,
            dash_path_effect,
            paint,
        }
    }

    /// `getColor()`.
    #[must_use]
    #[doc(alias = "getColor")]
    pub fn color(&self) -> Color {
        self.color
    }

    /// `getStrokeWidth()`.
    #[must_use]
    #[doc(alias = "getStrokeWidth")]
    pub fn stroke_width(&self) -> scalar {
        self.stroke_width
    }

    /// `getDashPathEffect()`.
    #[must_use]
    #[doc(alias = "getDashPathEffect")]
    pub fn dash_path_effect(&self) -> Option<DashPathEffect> {
        self.dash_path_effect
    }

    /// `skPaint()`.
    #[must_use]
    #[doc(alias = "skPaint")]
    pub fn paint(&self) -> &Paint {
        &self.paint
    }
}

// Port of: modules/skparagraph/include/ParagraphPainter.h#L20-L61 (chrome/m156)
/// `ParagraphPainter`: the drawing operations a paragraph needs.
pub trait ParagraphPainter {
    /// `drawTextBlob(blob, x, y, paint)`.
    #[doc(alias = "drawTextBlob")]
    fn draw_text_blob(&mut self, blob: Option<&TextBlob>, x: scalar, y: scalar, paint: &PaintOrId);

    /// `drawTextShadow(blob, x, y, color, blurSigma)`.
    #[doc(alias = "drawTextShadow")]
    fn draw_text_shadow(
        &mut self,
        blob: Option<&TextBlob>,
        x: scalar,
        y: scalar,
        color: Color,
        blur_sigma: scalar,
    );

    /// `drawRect(rect, paint)`.
    #[doc(alias = "drawRect")]
    fn draw_rect(&mut self, rect: &Rect, paint: &PaintOrId);

    /// `drawFilledRect(rect, decorStyle)`.
    #[doc(alias = "drawFilledRect")]
    fn draw_filled_rect(&mut self, rect: &Rect, decor_style: &DecorationStyle);

    /// `drawPath(path, decorStyle)`.
    #[doc(alias = "drawPath")]
    fn draw_path(&mut self, path: &Path, decor_style: &DecorationStyle);

    /// `drawLine(x0, y0, x1, y1, decorStyle)`.
    #[doc(alias = "drawLine")]
    fn draw_line(
        &mut self,
        x0: scalar,
        y0: scalar,
        x1: scalar,
        y1: scalar,
        decor_style: &DecorationStyle,
    );

    /// `clipRect(rect)`.
    #[doc(alias = "clipRect")]
    fn clip_rect(&mut self, rect: &Rect);

    /// `translate(dx, dy)`.
    fn translate(&mut self, dx: scalar, dy: scalar);

    /// `save()`.
    fn save(&mut self);

    /// `restore()`.
    fn restore(&mut self);
}

// Port of: modules/skparagraph/src/ParagraphPainterImpl.h#L16-L36 (chrome/m156)
/// `CanvasParagraphPainter`: draws on a [`Canvas`].
#[derive(Debug)]
pub struct CanvasParagraphPainter<'a> {
    canvas: &'a Canvas,
}

impl<'a> CanvasParagraphPainter<'a> {
    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L44-L45 (chrome/m156)
    /// `CanvasParagraphPainter(SkCanvas* canvas)`.
    #[must_use]
    pub fn new(canvas: &'a Canvas) -> Self {
        Self { canvas }
    }
}

impl ParagraphPainter for CanvasParagraphPainter<'_> {
    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L47-L51 (chrome/m156)
    fn draw_text_blob(&mut self, blob: Option<&TextBlob>, x: scalar, y: scalar, paint: &PaintOrId) {
        // `SkASSERT(std::holds_alternative<SkPaint>(paint))`, then `std::get<SkPaint>(paint)`.
        let PaintOrId::Paint(paint) = paint else {
            unreachable!("CanvasParagraphPainter::drawTextBlob needs an SkPaint");
        };
        if let Some(blob) = blob {
            self.canvas.draw_text_blob(blob, (x, y), paint);
        }
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L53-L65 (chrome/m156)
    fn draw_text_shadow(
        &mut self,
        blob: Option<&TextBlob>,
        x: scalar,
        y: scalar,
        color: Color,
        blur_sigma: scalar,
    ) {
        let mut paint = Paint::default();
        paint.set_color(color);
        #[allow(clippy::float_cmp)] // mirrors `blurSigma != 0.0f`
        if blur_sigma != 0.0 {
            let filter = MaskFilter::blur(BlurStyle::Normal, blur_sigma, false);
            paint.set_mask_filter(filter);
        }
        if let Some(blob) = blob {
            self.canvas.draw_text_blob(blob, (x, y), &paint);
        }
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L67-L70 (chrome/m156)
    fn draw_rect(&mut self, rect: &Rect, paint: &PaintOrId) {
        let PaintOrId::Paint(paint) = paint else {
            unreachable!("CanvasParagraphPainter::drawRect needs an SkPaint");
        };
        self.canvas.draw_rect(rect, paint);
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L72-L76 (chrome/m156)
    fn draw_filled_rect(&mut self, rect: &Rect, decor_style: &DecorationStyle) {
        let mut p = decor_style.paint().clone();
        p.set_stroke(false);
        self.canvas.draw_rect(rect, &p);
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L78-L80 (chrome/m156)
    fn draw_path(&mut self, path: &Path, decor_style: &DecorationStyle) {
        self.canvas.draw_path(path, decor_style.paint());
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L82-L84 (chrome/m156)
    fn draw_line(
        &mut self,
        x0: scalar,
        y0: scalar,
        x1: scalar,
        y1: scalar,
        decor_style: &DecorationStyle,
    ) {
        self.canvas
            .draw_line((x0, y0), (x1, y1), decor_style.paint());
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L86-L88 (chrome/m156)
    fn clip_rect(&mut self, rect: &Rect) {
        self.canvas.clip_rect(rect, None, None);
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L90-L92 (chrome/m156)
    fn translate(&mut self, dx: scalar, dy: scalar) {
        self.canvas.translate((dx, dy));
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L94-L96 (chrome/m156)
    fn save(&mut self) {
        self.canvas.save();
    }

    // Port of: modules/skparagraph/src/ParagraphPainterImpl.cpp#L98-L100 (chrome/m156)
    fn restore(&mut self) {
        self.canvas.restore();
    }
}

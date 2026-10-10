// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGAttribute.h, modules/svg/src/SkSVGAttribute.cpp

//! The attributes of SVG nodes and the presentation attributes they inherit.

use skia_rust_core::color::Color;

use crate::types::{
    ColorType, Colorspace, DashArray, DashArrayType, Display, Fill, FillRule, FillRuleType,
    FontFamily, FontSize, FontStyle, FontStyleType, FontWeight, FontWeightType, FuncIri, Length,
    LineCap, LineJoin, LineJoinType, NumberType, Paint, PaintType, Property, TextAnchor,
    TextAnchorType, Visibility, VisibilityType,
};

/// `SkSVGAttribute`.
// Port of: modules/svg/include/SkSVGAttribute.h#L13-L66 (chrome/m156)
#[doc(alias = "SkSVGAttribute")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    ClipRule,
    Color,
    ColorInterpolation,
    ColorInterpolationFilters,
    /// `<circle>`, `<ellipse>`, `<radialGradient>`: center x position.
    Cx,
    /// `<circle>`, `<ellipse>`, `<radialGradient>`: center y position.
    Cy,
    Fill,
    FillOpacity,
    FillRule,
    Filter,
    FilterUnits,
    FontFamily,
    FontSize,
    FontStyle,
    FontWeight,
    /// `<radialGradient>`: focal point x position.
    Fx,
    /// `<radialGradient>`: focal point y position.
    Fy,
    GradientUnits,
    GradientTransform,
    Height,
    Href,
    Opacity,
    Points,
    PreserveAspectRatio,
    /// `<circle>`, `<radialGradient>`: radius.
    R,
    /// `<ellipse>`, `<rect>`: horizontal (corner) radius.
    Rx,
    /// `<ellipse>`, `<rect>`: vertical (corner) radius.
    Ry,
    SpreadMethod,
    Stroke,
    StrokeDashArray,
    StrokeDashOffset,
    StrokeOpacity,
    StrokeLineCap,
    StrokeLineJoin,
    StrokeMiterLimit,
    StrokeWidth,
    Transform,
    Text,
    TextAnchor,
    ViewBox,
    Visibility,
    Width,
    X,
    /// `<line>`: first endpoint x.
    X1,
    /// `<line>`: second endpoint x.
    X2,
    Y,
    /// `<line>`: first endpoint y.
    Y1,
    /// `<line>`: second endpoint y.
    Y2,

    Unknown,
}

/// The presentation attributes of a node (`SkSVGPresentationAttributes`).
// Port of: modules/svg/include/SkSVGAttribute.h#L68-L108 (chrome/m156)
#[doc(alias = "SkSVGPresentationAttributes")]
#[derive(Debug, Clone, Default)]
pub struct PresentationAttributes {
    pub fill: Property<Paint, true>,
    pub fill_opacity: Property<NumberType, true>,
    pub fill_rule: Property<FillRule, true>,
    pub clip_rule: Property<FillRule, true>,

    pub stroke: Property<Paint, true>,
    pub stroke_dash_array: Property<DashArray, true>,
    pub stroke_dash_offset: Property<Length, true>,
    pub stroke_line_cap: Property<LineCap, true>,
    pub stroke_line_join: Property<LineJoin, true>,
    pub stroke_miter_limit: Property<NumberType, true>,
    pub stroke_opacity: Property<NumberType, true>,
    pub stroke_width: Property<Length, true>,

    pub visibility: Property<Visibility, true>,

    pub color: Property<ColorType, true>,
    pub color_interpolation: Property<Colorspace, true>,
    pub color_interpolation_filters: Property<Colorspace, true>,

    pub font_family: Property<FontFamily, true>,
    pub font_style: Property<FontStyle, true>,
    pub font_size: Property<FontSize, true>,
    pub font_weight: Property<FontWeight, true>,
    pub text_anchor: Property<TextAnchor, true>,

    // uninherited
    pub opacity: Property<NumberType, false>,
    pub clip_path: Property<FuncIri, false>,
    pub display: Property<Display, false>,
    pub mask: Property<FuncIri, false>,
    pub filter: Property<FuncIri, false>,
    pub stop_color: Property<Fill, false>,
    pub stop_opacity: Property<NumberType, false>,
    pub flood_color: Property<Fill, false>,
    pub flood_opacity: Property<NumberType, false>,
    pub lighting_color: Property<Fill, false>,
}

impl PresentationAttributes {
    // Port of: modules/svg/src/SkSVGAttribute.cpp#L14-L49 (chrome/m156)
    #[doc(alias = "MakeInitial")]
    #[must_use]
    pub fn make_initial() -> Self {
        let mut result = Self::default();

        result.fill.set(Paint::from_color(Fill::new(Color::BLACK)));
        result.fill_opacity.set(1.0);
        result.fill_rule.set(FillRule::new(FillRuleType::NonZero));
        result.clip_rule.set(FillRule::new(FillRuleType::NonZero));

        result.stroke.set(Paint::with_type(PaintType::None));
        result
            .stroke_dash_array
            .set(DashArray::with_type(DashArrayType::None));
        result.stroke_dash_offset.set(Length::new(0.0));
        result.stroke_line_cap.set(LineCap::Butt);
        result
            .stroke_line_join
            .set(LineJoin::new(LineJoinType::Miter));
        result.stroke_miter_limit.set(4.0);
        result.stroke_opacity.set(1.0);
        result.stroke_width.set(Length::new(1.0));

        result
            .visibility
            .set(Visibility::new(VisibilityType::Visible));

        result.color.set(Color::BLACK);
        result.color_interpolation.set(Colorspace::SRGB);
        result
            .color_interpolation_filters
            .set(Colorspace::LinearRGB);

        result.font_family.init(FontFamily::new("Sans"));
        result
            .font_style
            .init(FontStyle::new(FontStyleType::Normal));
        result.font_size.init(FontSize::new(Length::new(24.0)));
        result
            .font_weight
            .init(FontWeight::new(FontWeightType::Normal));
        result
            .text_anchor
            .init(TextAnchor::new(TextAnchorType::Start));

        result.display.init(Display::Inline);

        result.stop_color.set(Fill::new(Color::BLACK));
        result.stop_opacity.set(1.0);
        result.flood_color.set(Fill::new(Color::BLACK));
        result.flood_opacity.set(1.0);
        result.lighting_color.set(Fill::new(Color::WHITE));

        result
    }
}

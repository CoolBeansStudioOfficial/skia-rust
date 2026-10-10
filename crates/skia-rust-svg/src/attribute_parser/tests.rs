// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

use super::*;
use crate::types::{ColorKind, FuncIriType, PaintType, PointsType};

fn parse<T: Parse>(s: &str) -> Option<T> {
    AttributeParser::parse_value::<T>(s)
}

#[test]
fn colors() {
    assert_eq!(
        parse::<ColorType>("#f00"),
        Some(Color::from_rgb(0xff, 0, 0))
    );
    assert_eq!(
        parse::<ColorType>(" #0a0b0c "),
        Some(Color::from_rgb(10, 11, 12))
    );
    assert_eq!(parse::<ColorType>("red"), Some(Color::from_rgb(0xff, 0, 0)));
    assert_eq!(
        parse::<ColorType>("rgb(1, 2, 3)"),
        Some(Color::from_rgb(1, 2, 3))
    );
    assert_eq!(
        parse::<ColorType>("rgb(100%, 0%, 50%)"),
        Some(Color::from_rgb(255, 0, 128))
    );
    assert_eq!(
        parse::<ColorType>("rgba(1, 2, 3, 0.5)"),
        Some(Color::from_argb(128, 1, 2, 3))
    );
    assert_eq!(parse::<ColorType>("#12"), None);
    assert_eq!(parse::<ColorType>("nocolor"), None);
}

#[test]
fn paints() {
    let p = parse::<Paint>("none").unwrap();
    assert_eq!(p.ty(), PaintType::None);
    let p = parse::<Paint>("url(#grad) blue").unwrap();
    assert_eq!(p.ty(), PaintType::IRI);
    assert_eq!(p.iri().iri(), "grad");
    assert_eq!(p.color().color(), Color::from_rgb(0, 0, 0xff));
    let p = parse::<Paint>("currentColor").unwrap();
    assert_eq!(p.color().ty(), ColorKind::CurrentColor);
    assert!(parse::<Paint>("url(#a").is_none());
}

#[test]
fn lengths() {
    assert_eq!(parse::<Length>("10"), Some(Length::new(10.0)));
    assert_eq!(
        parse::<Length>("5.5mm"),
        Some(Length::with_unit(5.5, LengthUnit::MM))
    );
    assert_eq!(
        parse::<Length>("50%"),
        Some(Length::with_unit(50.0, LengthUnit::Percentage))
    );
    assert_eq!(parse::<Length>("abc"), None);
}

#[test]
fn transforms() {
    let m = parse::<TransformType>("translate(10 20) scale(2)").unwrap();
    assert_eq!(m.map_xy(1.0, 1.0), Point::new(12.0, 22.0));
    let m = parse::<TransformType>("matrix(1,0,0,1,5,6)").unwrap();
    assert_eq!(m.translate_x(), 5.0);
    assert!(parse::<TransformType>("translate(").is_none());
    assert!(parse::<TransformType>("").is_none());
}

#[test]
fn view_box_and_aspect_ratio() {
    assert_eq!(
        AttributeParser::new("0 0 10 20").parse_view_box(),
        Some(Rect::from_xywh(0.0, 0.0, 10.0, 20.0))
    );
    assert_eq!(AttributeParser::new("0 0 10").parse_view_box(), None);
    let par = AttributeParser::new("xMinYMax slice")
        .parse_preserve_aspect_ratio()
        .unwrap();
    assert_eq!(par.align, Align::XMinYMax);
    assert_eq!(par.scale, Scale::Slice);
}

#[test]
fn properties() {
    let pr =
        AttributeParser::parse_property::<NumberType, true>("fill-opacity", "fill-opacity", "0.5")
            .unwrap();
    assert!(pr.is_value());
    let pr = AttributeParser::parse_property::<NumberType, true>(
        "fill-opacity",
        "fill-opacity",
        "inherit",
    )
    .unwrap();
    assert!(!pr.is_value());
    assert!(
        AttributeParser::parse_property::<NumberType, true>("fill-opacity", "stroke", "1")
            .is_none()
    );
}

#[test]
fn func_iri_points_and_dashes() {
    assert_eq!(parse::<FuncIri>("none").unwrap().ty(), FuncIriType::None);
    assert_eq!(parse::<FuncIri>("url(#clip)").unwrap().iri().iri(), "clip");
    let pts = parse::<PointsType>("0,0 10,0 10-5").unwrap();
    assert_eq!(
        pts,
        [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, -5.0)
        ]
    );
    let da = parse::<DashArray>("1 2, 3").unwrap();
    assert_eq!(da.dash_array().len(), 3);
    assert_eq!(
        parse::<DashArray>("none").unwrap().ty(),
        DashArrayType::None
    );
}

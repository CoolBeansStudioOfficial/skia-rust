// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGValue.h, modules/svg/src/SkSVGValue.cpp

//! Typed views of attribute values (`SkSVGValue`).

use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;

use crate::types::{
    ColorType, Length, NumberType, ObjectBoundingBoxUnits, PreserveAspectRatio, StopColor,
    StringType,
};

/// A borrowed, typed attribute value. `SkSVGValue` is the stack-only wrapper that
/// `SkSVGNode::setAttribute` takes; each `as<SkSVGXValue>()` becomes an `as_x()` accessor.
// Port of: modules/svg/include/SkSVGValue.h#L16-L83 (chrome/m156)
#[doc(alias = "SkSVGValue")]
#[derive(Debug, Clone, Copy)]
pub enum Value<'a> {
    #[doc(alias = "SkSVGColorValue")]
    Color(&'a ColorType),
    #[doc(alias = "SkSVGLengthValue")]
    Length(&'a Length),
    #[doc(alias = "SkSVGNumberValue")]
    Number(&'a NumberType),
    #[doc(alias = "SkSVGObjectBoundingBoxUnitsValue")]
    ObjectBoundingBoxUnits(&'a ObjectBoundingBoxUnits),
    #[doc(alias = "SkSVGPreserveAspectRatioValue")]
    PreserveAspectRatio(&'a PreserveAspectRatio),
    #[doc(alias = "SkSVGStopColorValue")]
    StopColor(&'a StopColor),
    #[doc(alias = "SkSVGStringValue")]
    String(&'a StringType),
    #[doc(alias = "SkSVGTransformValue")]
    Transform(&'a Matrix),
    #[doc(alias = "SkSVGViewBoxValue")]
    ViewBox(&'a Rect),
}

impl<'a> Value<'a> {
    #[must_use]
    pub fn as_color(&self) -> Option<&'a ColorType> {
        match self {
            Value::Color(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_length(&self) -> Option<&'a Length> {
        match self {
            Value::Length(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_number(&self) -> Option<&'a NumberType> {
        match self {
            Value::Number(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_object_bounding_box_units(&self) -> Option<&'a ObjectBoundingBoxUnits> {
        match self {
            Value::ObjectBoundingBoxUnits(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_preserve_aspect_ratio(&self) -> Option<&'a PreserveAspectRatio> {
        match self {
            Value::PreserveAspectRatio(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_stop_color(&self) -> Option<&'a StopColor> {
        match self {
            Value::StopColor(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_string(&self) -> Option<&'a StringType> {
        match self {
            Value::String(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_transform(&self) -> Option<&'a Matrix> {
        match self {
            Value::Transform(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_view_box(&self) -> Option<&'a Rect> {
        match self {
            Value::ViewBox(v) => Some(v),
            _ => None,
        }
    }
}

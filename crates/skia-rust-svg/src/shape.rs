// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGShape.h, modules/svg/src/SkSVGShape.cpp

//! Shared behaviour of the shape elements (`SkSVGShape`).

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path_types::PathFillType;

use crate::render_context::{LengthContext, RenderContext};

/// Declares the accessor pair of an `SVG_ATTR` field.
macro_rules! svg_attr {
    ($(#[$meta:meta])* $get:ident, $set:ident, $field:ident, $ty:ty) => {
        $(#[$meta])*
        #[must_use]
        pub fn $get(&self) -> &$ty {
            &self.$field
        }

        $(#[$meta])*
        pub fn $set(&mut self, v: $ty) {
            self.$field = v;
        }
    };
}
pub(crate) use svg_attr;

/// Declares the accessor pair of an `SVG_OPTIONAL_ATTR` field.
macro_rules! svg_optional_attr {
    ($(#[$meta:meta])* $get:ident, $set:ident, $field:ident, $ty:ty) => {
        $(#[$meta])*
        #[must_use]
        pub fn $get(&self) -> Option<&$ty> {
            self.$field.as_ref()
        }

        $(#[$meta])*
        pub fn $set(&mut self, v: $ty) {
            self.$field = Some(v);
        }
    };
}
pub(crate) use svg_optional_attr;

/// Sets `slot` from a parse result and reports whether there was one (the `set##attr_name`
/// overloads taking a `ParseResult`).
pub(crate) fn set_parsed<T>(slot: &mut T, parsed: Option<T>) -> bool {
    match parsed {
        Some(v) => {
            *slot = v;
            true
        }
        None => false,
    }
}

/// Like [`set_parsed`], for `SVG_OPTIONAL_ATTR`.
pub(crate) fn set_parsed_optional<T>(slot: &mut Option<T>, parsed: Option<T>) -> bool {
    match parsed {
        Some(v) => {
            *slot = Some(v);
            true
        }
        None => false,
    }
}

/// `SkSVGShape::onRender`: draws the shape with the fill paint and then the stroke paint.
///
/// `on_draw` is the shape's `onDraw`.
// Port of: modules/svg/src/SkSVGShape.cpp#L18-L31 (chrome/m156)
pub(crate) fn render_shape(
    ctx: &RenderContext<'_>,
    on_draw: impl Fn(&Canvas, &LengthContext, &SkPaint, PathFillType),
) {
    let fill_type = ctx
        .presentation_context()
        .inherited
        .fill_rule
        .as_fill_type();

    let fill_paint = ctx.fill_paint();
    let stroke_paint = ctx.stroke_paint();

    // TODO: this approach forces duplicate geometry resolution in onDraw(); refactor to avoid.
    if let Some(fill_paint) = &fill_paint {
        on_draw(ctx.canvas(), &ctx.length_context(), fill_paint, fill_type);
    }

    if let Some(stroke_paint) = &stroke_paint {
        on_draw(ctx.canvas(), &ctx.length_context(), stroke_paint, fill_type);
    }
}

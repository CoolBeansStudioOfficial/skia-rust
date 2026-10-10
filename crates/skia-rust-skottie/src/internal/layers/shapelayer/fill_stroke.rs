// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/shapelayer/FillStroke.cpp (chrome/m156)

use std::rc::{Rc, Weak};

use skia_rust_core::color::Color;
use skia_rust_core::paint::{Cap, Join, Style};
use skia_rust_sksg::{Color as SgColor, DashEffect, GeometryNode, PaintNode};

use crate::internal::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::{ArrayValue, ObjectValue};
use crate::skottie_json::{ValueExt, parse_default};
use crate::skottie_value::ColorValue;

use super::Geometries;
use super::geometry::{ShapeBuilder, shape_adapter};

/// Whether a fill/stroke adapter drives a color or a gradient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShaderType {
    Color,
    Gradient,
}

/// Whether the adapter drives a fill or a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FillStrokeType {
    Fill,
    Stroke,
}

/// Drives the paint of a fill or stroke.
// Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L28-L103 (chrome/m156) (`class FillStrokeAdapter`)
struct FillStrokeAdapter {
    base: DiscardableAdapterBase<dyn PaintNode>,
    shader_type: ShaderType,
    /// The paint node, when it is a color.
    color_node: Option<Rc<SgColor>>,
    color: Prop<ColorValue>,
    opacity: Prop<f32>,
    stroke_width: Prop<f32>,
}

impl FillStrokeAdapter {
    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L32-L77 (chrome/m156)
    fn make(
        jpaint: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        paint_node: Rc<dyn PaintNode>,
        color_node: Option<Rc<SgColor>>,
        gradient_adapter: Option<Rc<dyn AnimatablePropertyContainer>>,
        fs_type: FillStrokeType,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let shader_type = if gradient_adapter.is_some() {
                ShaderType::Gradient
            } else {
                ShaderType::Color
            };
            let base = DiscardableAdapterBase::new(weak.clone(), paint_node);
            let color = Prop::new(ColorValue::new());
            let opacity = Prop::new(100.0);
            let stroke_width = Prop::new(1.0);

            base.container().attach_discardable_adapter(gradient_adapter);

            base.container().bind(abuilder, jpaint.get("o"), &opacity);

            base.node().set_anti_alias(true);

            if fs_type == FillStrokeType::Stroke {
                base.container().bind(abuilder, jpaint.get("w"), &stroke_width);

                base.node().set_style(Style::Stroke);
                base.node()
                    .set_stroke_miter(parse_default::<f32>(jpaint.get("ml"), 4.0));

                const JOINS: [Join; 3] = [Join::Miter, Join::Round, Join::Bevel];
                // size_t arithmetic: "lj": 0 wraps around, and pins to the last join.
                let lj = parse_default::<usize>(jpaint.get("lj"), 1).wrapping_sub(1);
                base.node().set_stroke_join(JOINS[lj.min(JOINS.len() - 1)]);

                const CAPS: [Cap; 3] = [Cap::Butt, Cap::Round, Cap::Square];
                // size_t arithmetic: "lc": 0 wraps around, and pins to the last cap.
                let lc = parse_default::<usize>(jpaint.get("lc"), 1).wrapping_sub(1);
                base.node().set_stroke_cap(CAPS[lc.min(CAPS.len() - 1)]);
            }

            if shader_type == ShaderType::Color {
                base.container().bind(abuilder, jpaint.get("c"), &color);
            }

            Self {
                base,
                shader_type,
                color_node,
                color,
                opacity,
                stroke_width,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L79-L89 (chrome/m156) (`onSync`)
    fn sync(&self) {
        self.base.node().set_opacity(self.opacity.get() * 0.01);
        self.base.node().set_stroke_width(self.stroke_width.get());

        if self.shader_type == ShaderType::Color {
            if let Some(color_node) = &self.color_node {
                color_node.set_color(self.color.borrow().to_color());
            }
        }
    }
}

shape_adapter!(FillStrokeAdapter);

/// Drives a dash effect.
// Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L105-L141 (chrome/m156) (`class DashAdapter`)
struct DashAdapter {
    base: DiscardableAdapterBase<DashEffect>,
    intervals: Vec<Prop<f32>>,
    offset: Prop<f32>,
}

impl DashAdapter {
    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L107-L128 (chrome/m156)
    fn make(
        jdash: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        geo: Rc<dyn GeometryNode>,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                DashEffect::make(Some(geo)).expect("the geometry is not null"),
            );
            debug_assert!(jdash.size() > 1);

            // The dash is encoded as an arbitrary number of intervals (alternating dash/gap),
            // plus a single trailing offset. Each value can be animated independently.
            let interval_count = jdash.size() - 1;
            let intervals: Vec<Prop<f32>> = (0..interval_count).map(|_| Prop::new(0.0)).collect();
            let offset = Prop::new(0.0);

            for i in 0..jdash.size() {
                if let Some(jint) = jdash[i].as_object() {
                    let target = if i < interval_count {
                        &intervals[i]
                    } else {
                        &offset
                    };
                    base.container().bind(abuilder, jint.get("v"), target);
                }
            }

            Self {
                base,
                intervals,
                offset,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L130-L133 (chrome/m156) (`onSync`)
    fn sync(&self) {
        self.base.node().set_phase(self.offset.get());
        self.base
            .node()
            .set_intervals(self.intervals.iter().map(Prop::get).collect());
    }
}

shape_adapter!(DashAdapter);

impl ShapeBuilder {
    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L145-L157 (chrome/m156) (`AttachFill`)
    pub(super) fn attach_fill(
        jpaint: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        paint_node: Rc<dyn PaintNode>,
        color_node: Option<Rc<SgColor>>,
        gradient: Option<Rc<dyn AnimatablePropertyContainer>>,
    ) -> Rc<dyn PaintNode> {
        let adapter = FillStrokeAdapter::make(
            jpaint,
            abuilder,
            paint_node,
            color_node,
            gradient,
            FillStrokeType::Fill,
        );
        let node = Rc::clone(adapter.base.node());
        abuilder.attach_discardable_adapter(&adapter);
        node
    }

    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L159-L171 (chrome/m156) (`AttachStroke`)
    pub(super) fn attach_stroke(
        jpaint: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        paint_node: Rc<dyn PaintNode>,
        color_node: Option<Rc<SgColor>>,
        gradient: Option<Rc<dyn AnimatablePropertyContainer>>,
    ) -> Rc<dyn PaintNode> {
        let adapter = FillStrokeAdapter::make(
            jpaint,
            abuilder,
            paint_node,
            color_node,
            gradient,
            FillStrokeType::Stroke,
        );
        let node = Rc::clone(adapter.base.node());
        abuilder.attach_discardable_adapter(&adapter);
        node
    }

    /// Attaches a color fill.
    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L173-L180 (chrome/m156) (`AttachColorFill`)
    #[doc(alias = "AttachColorFill")]
    #[must_use]
    pub fn attach_color_fill(
        jpaint: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn PaintNode>> {
        let color_node = SgColor::make(Color::BLACK);
        let color_paint = Self::attach_fill(
            jpaint,
            abuilder,
            Rc::clone(&color_node) as Rc<dyn PaintNode>,
            Some(Rc::clone(&color_node)),
            None,
        );
        abuilder.dispatch_color_property(&color_node);
        Some(color_paint)
    }

    /// Attaches a color stroke.
    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L182-L189 (chrome/m156) (`AttachColorStroke`)
    #[doc(alias = "AttachColorStroke")]
    #[must_use]
    pub fn attach_color_stroke(
        jpaint: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn PaintNode>> {
        let color_node = SgColor::make(Color::BLACK);
        let color_paint = Self::attach_stroke(
            jpaint,
            abuilder,
            Rc::clone(&color_node) as Rc<dyn PaintNode>,
            Some(Rc::clone(&color_node)),
            None,
        );
        abuilder.dispatch_color_property(&color_node);
        Some(color_paint)
    }

    /// Adjusts the geometry of a stroke: it is dashed, if the stroke has a dash pattern.
    // Port of: modules/skottie/src/layers/shapelayer/FillStroke.cpp#L191-L205 (chrome/m156) (`AdjustStrokeGeometry`)
    #[doc(alias = "AdjustStrokeGeometry")]
    #[must_use]
    pub fn adjust_stroke_geometry(
        jstroke: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        mut geos: Geometries,
    ) -> Geometries {
        if let Some(jdash) = jstroke.get("d").as_array() {
            if jdash.size() > 1 {
                for geo in &mut geos {
                    let adapter = DashAdapter::make(jdash, abuilder, Rc::clone(geo));
                    let node = Rc::clone(adapter.base.node());
                    abuilder.attach_discardable_adapter(&adapter);
                    *geo = node as Rc<dyn GeometryNode>;
                }
            }
        }

        geos
    }
}

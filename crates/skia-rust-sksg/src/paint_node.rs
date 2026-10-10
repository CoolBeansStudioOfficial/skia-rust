// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGPaint.h, modules/sksg/src/SkSGPaint.cpp (chrome/m156)

use std::cell::Cell;
use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color as SkColor;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::rect::Rect;

use crate::node::{Node, NodeCore, inval_traits};
use crate::shader::ShaderNode;
use crate::util::{float_round_to_int, pin};

/// The attributes of a paint node (the private fields of `PaintNode`).
// Port of: modules/sksg/include/SkSGPaint.h#L36-L46 (chrome/m156)
#[derive(Debug)]
pub struct PaintAttrs {
    pub(crate) opacity: Cell<f32>,
    pub(crate) stroke_width: Cell<f32>,
    pub(crate) stroke_miter: Cell<f32>,
    pub(crate) anti_alias: Cell<bool>,
    pub(crate) blend_mode: Cell<BlendMode>,
    pub(crate) style: Cell<Style>,
    pub(crate) stroke_join: Cell<Join>,
    pub(crate) stroke_cap: Cell<Cap>,
}

impl Default for PaintAttrs {
    // Port of: modules/sksg/include/SkSGPaint.h#L36-L46 (chrome/m156) (member initializers)
    fn default() -> Self {
        Self {
            opacity: Cell::new(1.0),
            stroke_width: Cell::new(1.0),
            stroke_miter: Cell::new(4.0),
            anti_alias: Cell::new(false),
            blend_mode: Cell::new(BlendMode::SrcOver),
            style: Cell::new(Style::Fill),
            stroke_join: Cell::new(Join::Miter),
            stroke_cap: Cell::new(Cap::Butt),
        }
    }
}

/// Generates a getter and a setter for one paint attribute. The setter invalidates the node only
/// when the value changes (`SG_ATTRIBUTE`).
macro_rules! sg_attribute {
    ($get:ident, $set:ident, $ty:ty, $field:ident) => {
        fn $get(&self) -> $ty {
            self.paint_attrs().$field.get()
        }

        #[allow(clippy::float_cmp)] // SG_ATTRIBUTE compares the attribute with ==, as Skia does
        fn $set(&self, value: $ty) {
            if self.paint_attrs().$field.get() == value {
                return;
            }
            self.paint_attrs().$field.set(value);
            self.invalidate();
        }
    };
}

/// A node that describes how a shape is painted. [`PaintNode::make_paint`] returns the paint.
// Port of: modules/sksg/include/SkSGPaint.h#L20-L54 (chrome/m156) (`class PaintNode`)
#[doc(alias = "sksg::PaintNode")]
pub trait PaintNode: Node {
    /// The attributes of the node.
    fn paint_attrs(&self) -> &PaintAttrs;

    /// Applies the subclass properties to `paint` (`onApplyToPaint`).
    fn on_apply_to_paint(&self, paint: &mut Paint);

    sg_attribute!(anti_alias, set_anti_alias, bool, anti_alias);
    sg_attribute!(opacity, set_opacity, f32, opacity);
    sg_attribute!(blend_mode, set_blend_mode, BlendMode, blend_mode);
    sg_attribute!(stroke_width, set_stroke_width, f32, stroke_width);
    sg_attribute!(stroke_miter, set_stroke_miter, f32, stroke_miter);
    sg_attribute!(style, set_style, Style, style);
    sg_attribute!(stroke_join, set_stroke_join, Join, stroke_join);
    sg_attribute!(stroke_cap, set_stroke_cap, Cap, stroke_cap);

    /// The paint described by the node.
    // Port of: modules/sksg/src/SkSGPaint.cpp#L25-L42 (chrome/m156) (`PaintNode::makePaint`)
    #[doc(alias = "makePaint")]
    fn make_paint(&self) -> Paint {
        debug_assert!(!self.core().has_inval());
        let attrs = self.paint_attrs();
        let mut paint = Paint::default();
        paint.set_anti_alias(attrs.anti_alias.get());
        paint.set_blend_mode(attrs.blend_mode.get());
        paint.set_style(attrs.style.get());
        paint.set_stroke_width(attrs.stroke_width.get());
        paint.set_stroke_miter(attrs.stroke_miter.get());
        paint.set_stroke_join(attrs.stroke_join.get());
        paint.set_stroke_cap(attrs.stroke_cap.get());
        self.on_apply_to_paint(&mut paint);
        // Compose opacity on top of the subclass value.
        let alpha = f32::from(paint.alpha()) * pin(attrs.opacity.get(), 0.0, 1.0);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // alpha is in 0..=255
        paint.set_alpha(float_round_to_int(alpha) as u8);
        paint
    }
}

/// The traits of paint nodes: their damage bubbles up to the draws that use them.
// Port of: modules/sksg/src/SkSGPaint.cpp#L23-L23 (chrome/m156) (`PaintNode::PaintNode`)
pub const PAINT_TRAITS: u32 = inval_traits::BUBBLE_DAMAGE;

/// A solid color paint.
// Port of: modules/sksg/include/SkSGPaint.h#L56-L70 (chrome/m156) (`class Color`)
#[doc(alias = "sksg::Color")]
#[derive(Debug)]
#[allow(clippy::struct_field_names)] // mirrors SkSGPaint.h, where the field is `fColor`
pub struct Color {
    core: NodeCore,
    attrs: PaintAttrs,
    color: Cell<SkColor>,
}

impl Color {
    /// A solid color paint (`Color::Make`).
    // Port of: modules/sksg/src/SkSGPaint.cpp#L44-L46 (chrome/m156) (`Color::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(c: SkColor) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(PAINT_TRAITS, weak.clone()),
            attrs: PaintAttrs::default(),
            color: Cell::new(c),
        })
    }

    /// The color.
    #[must_use]
    pub fn color(&self) -> SkColor {
        self.color.get()
    }

    /// Sets the color, invalidating the node if it changed (`setColor`).
    #[doc(alias = "setColor")]
    pub fn set_color(&self, c: SkColor) {
        if self.color.get() == c {
            return;
        }
        self.color.set(c);
        self.invalidate();
    }
}

impl Node for Color {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGPaint.cpp#L48-L51 (chrome/m156) (`Color::onRevalidate`)
    fn on_revalidate(
        &self,
        _ic: Option<&mut crate::invalidation_controller::InvalidationController>,
        _ctm: &Matrix,
    ) -> Rect {
        debug_assert!(self.core.has_inval());
        Rect::new_empty()
    }
}

impl PaintNode for Color {
    fn paint_attrs(&self) -> &PaintAttrs {
        &self.attrs
    }

    // Port of: modules/sksg/src/SkSGPaint.cpp#L53-L55 (chrome/m156) (`Color::onApplyToPaint`)
    fn on_apply_to_paint(&self, paint: &mut Paint) {
        paint.set_color(self.color.get());
    }
}

/// A shader paint: the paint takes its shader from a shader node (`ShaderPaint`).
// Port of: modules/sksg/include/SkSGPaint.h#L72-L85 (chrome/m156) (`class ShaderPaint`)
#[doc(alias = "sksg::ShaderPaint")]
#[derive(Debug)]
pub struct ShaderPaint {
    core: NodeCore,
    attrs: PaintAttrs,
    shader: Rc<dyn ShaderNode>,
}

impl ShaderPaint {
    /// `ShaderPaint::Make(shader)`: `None` if there is no shader.
    // Port of: modules/sksg/src/SkSGPaint.cpp#L60-L63 (chrome/m156) (`ShaderPaint::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(shader: Option<Rc<dyn ShaderNode>>) -> Option<Rc<Self>> {
        let shader = shader?;
        let paint = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(PAINT_TRAITS, weak.clone()),
            attrs: PaintAttrs::default(),
            shader: Rc::clone(&shader),
        });
        // Port of: modules/sksg/src/SkSGPaint.cpp#L65-L68 (chrome/m156) (`ShaderPaint::ShaderPaint`)
        paint.observe_inval(paint.shader.as_ref());
        Some(paint)
    }
}

impl Drop for ShaderPaint {
    // Port of: modules/sksg/src/SkSGPaint.cpp#L70-L72 (chrome/m156) (`ShaderPaint::~ShaderPaint`)
    fn drop(&mut self) {
        self.unobserve_inval(self.shader.as_ref());
    }
}

impl Node for ShaderPaint {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGPaint.cpp#L74-L78 (chrome/m156) (`ShaderPaint::onRevalidate`)
    fn on_revalidate(
        &self,
        ic: Option<&mut crate::invalidation_controller::InvalidationController>,
        ctm: &Matrix,
    ) -> Rect {
        debug_assert!(self.core.has_inval());
        self.shader.revalidate(ic, ctm)
    }
}

impl PaintNode for ShaderPaint {
    fn paint_attrs(&self) -> &PaintAttrs {
        &self.attrs
    }

    // Port of: modules/sksg/src/SkSGPaint.cpp#L80-L82 (chrome/m156) (`ShaderPaint::onApplyToPaint`)
    fn on_apply_to_paint(&self, paint: &mut Paint) {
        paint.set_shader(self.shader.shader());
    }
}

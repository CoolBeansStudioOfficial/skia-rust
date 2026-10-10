// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/shapelayer/Repeater.cpp (chrome/m156)

use std::cell::Cell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::{AutoCanvasRestore, Canvas};
use skia_rust_core::m44::V2;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_pow;
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::node::inval_traits;
use skia_rust_sksg::render_node::{observe_children, unobserve_children};
use skia_rust_sksg::{
    InvalidationController, Node, NodeCore, RenderContext, RenderNode, ScopedRenderContext,
};

use crate::internal::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::ObjectValue;
use crate::skottie_json::{ValueExt, parse_default};

use super::geometry::{ShapeBuilder, shape_adapter};
use super::Draws;

/// How the instances are composited.
// Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L36 (chrome/m156) (`RepeaterRenderNode::CompositeMode`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositeMode {
    Below,
    Above,
}

/// Renders its children repeatedly, with a transform and opacity that vary by instance.
// Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L32-L125 (chrome/m156) (`class RepeaterRenderNode`)
#[derive(Debug)]
struct RepeaterRenderNode {
    core: NodeCore,
    children: Vec<Rc<dyn RenderNode>>,
    mode: CompositeMode,

    /// Cached.
    children_bounds: Cell<Rect>,

    count: Cell<usize>,
    offset: Cell<f32>,
    rotation: Cell<f32>,
    start_opacity: Cell<f32>,
    end_opacity: Cell<f32>,
    anchor_point: Cell<V2>,
    position: Cell<V2>,
    scale: Cell<V2>,
}

/// Generates the `SG_ATTRIBUTE` setter of an attribute.
macro_rules! sg_attribute {
    ($set:ident, $ty:ty, $field:ident) => {
        #[allow(clippy::float_cmp)] // SG_ATTRIBUTE compares the attribute with ==, as Skia does
        fn $set(&self, v: $ty) {
            if self.$field.get() == v {
                return;
            }
            self.$field.set(v);
            self.invalidate();
        }
    };
}

impl RepeaterRenderNode {
    fn new(children: Vec<Rc<dyn RenderNode>>, mode: CompositeMode) -> Rc<Self> {
        let node = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            // CustomRenderNode: cannot make assumptions about its children's damage.
            core: NodeCore::new(inval_traits::OVERRIDE_DAMAGE, weak.clone()),
            children: children.clone(),
            mode,
            children_bounds: Cell::new(Rect::new_empty()),
            count: Cell::new(0),
            offset: Cell::new(0.0),
            rotation: Cell::new(0.0),
            start_opacity: Cell::new(1.0),
            end_opacity: Cell::new(1.0),
            anchor_point: Cell::new(V2::new(0.0, 0.0)),
            position: Cell::new(V2::new(0.0, 0.0)),
            scale: Cell::new(V2::new(1.0, 1.0)),
        });
        observe_children(node.as_ref(), &children);
        node
    }

    sg_attribute!(set_count, usize, count);
    sg_attribute!(set_offset, f32, offset);
    sg_attribute!(set_anchor_point, V2, anchor_point);
    sg_attribute!(set_position, V2, position);
    sg_attribute!(set_scale, V2, scale);
    sg_attribute!(set_rotation, f32, rotation);
    sg_attribute!(set_start_opacity, f32, start_opacity);
    sg_attribute!(set_end_opacity, f32, end_opacity);

    // Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L52-L65 (chrome/m156) (`instanceTransform`)
    fn instance_transform(&self, i: usize) -> Matrix {
        #[allow(clippy::cast_precision_loss)] // mirrors the implicit size_t -> float
        let t = self.offset.get() + i as f32;
        let anchor_point = self.anchor_point.get();
        let position = self.position.get();
        let scale = self.scale.get();

        // Position, scale & rotation are "scaled" by index/offset.
        // skia-rust: libm (std::pow)
        let m = &Matrix::translate((
            t * position.x + anchor_point.x,
            t * position.y + anchor_point.y,
        )) * &Matrix::rotate_deg(t * self.rotation.get());
        let m = &m * &Matrix::scale((scalar_pow(scale.x, t), scalar_pow(scale.y, t)));
        &m * &Matrix::translate((-anchor_point.x, -anchor_point.y))
    }
}

impl Drop for RepeaterRenderNode {
    fn drop(&mut self) {
        unobserve_children(self, &self.children);
    }
}

impl Node for RepeaterRenderNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L67-L81 (chrome/m156) (`onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let mut children_bounds = Rect::new_empty();
        for child in &self.children {
            children_bounds.join(child.revalidate(ic.as_deref_mut(), ctm));
        }
        self.children_bounds.set(children_bounds);

        let mut bounds = Rect::new_empty();
        for i in 0..self.count.get() {
            bounds.join(self.instance_transform(i).map_rect(children_bounds).0);
        }

        bounds
    }
}

impl RenderNode for RepeaterRenderNode {
    // Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L83-L111 (chrome/m156) (`onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        // To cover the full opacity range, the denominator below should be (fCount - 1).
        // Interstingly, that's not what AE does. Off-by-one bug?
        let count = self.count.get();
        #[allow(clippy::cast_precision_loss)] // mirrors the implicit size_t -> float
        let d_opacity = if count > 1 {
            (self.end_opacity.get() - self.start_opacity.get()) / count as f32
        } else {
            0.0
        };

        for i in 0..count {
            let render_index = if self.mode == CompositeMode::Above {
                i
            } else {
                count - i - 1
            };
            #[allow(clippy::cast_precision_loss)] // mirrors the implicit size_t -> float
            let opacity = self.start_opacity.get() + d_opacity * render_index as f32;

            if opacity <= 0.0 {
                continue;
            }

            let acr = AutoCanvasRestore::guard(canvas, true);
            canvas.concat(&self.instance_transform(render_index));

            let local_ctx = ScopedRenderContext::new(canvas, ctx)
                .modulate_opacity(opacity)
                .set_isolation(
                    &self.children_bounds.get(),
                    &canvas.total_matrix(),
                    self.children.len() > 1,
                );
            for child in &self.children {
                child.render(canvas, Some(local_ctx.context()));
            }
            drop(local_ctx);
            drop(acr);
        }
    }

    // No hit-testing.
    // Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L46 (chrome/m156) (`onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<skia_rust_sksg::render_node::Hit> {
        None
    }
}

/// Drives the repeater node.
// Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L127-L186 (chrome/m156) (`class RepeaterAdapter`)
struct RepeaterAdapter {
    base: DiscardableAdapterBase<RepeaterRenderNode>,

    // Repeater props
    count: Prop<f32>,
    offset: Prop<f32>,

    // Transform props
    anchor_point: Prop<V2>,
    position: Prop<V2>,
    scale: Prop<V2>,
    rotation: Prop<f32>,
    start_opacity: Prop<f32>,
    end_opacity: Prop<f32>,
}

impl RepeaterAdapter {
    // Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L129-L151 (chrome/m156)
    fn make(
        jrepeater: &ObjectValue,
        jtransform: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        draws: Draws,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let mode = if parse_default::<i32>(jrepeater.get("m"), 1) == 1 {
                CompositeMode::Below
            } else {
                CompositeMode::Above
            };
            let base = DiscardableAdapterBase::new(weak.clone(), RepeaterRenderNode::new(draws, mode));

            let count = Prop::new(0.0);
            let offset = Prop::new(0.0);
            let anchor_point = Prop::new(V2::new(0.0, 0.0));
            let position = Prop::new(V2::new(0.0, 0.0));
            let scale = Prop::new(V2::new(100.0, 100.0));
            let rotation = Prop::new(0.0);
            let start_opacity = Prop::new(100.0);
            let end_opacity = Prop::new(100.0);

            let c = base.container();
            c.bind(abuilder, jrepeater.get("c"), &count);
            c.bind(abuilder, jrepeater.get("o"), &offset);

            c.bind(abuilder, jtransform.get("a"), &anchor_point);
            c.bind(abuilder, jtransform.get("p"), &position);
            c.bind(abuilder, jtransform.get("s"), &scale);
            c.bind(abuilder, jtransform.get("r"), &rotation);
            c.bind(abuilder, jtransform.get("so"), &start_opacity);
            c.bind(abuilder, jtransform.get("eo"), &end_opacity);

            Self {
                base,
                count,
                offset,
                anchor_point,
                position,
                scale,
                rotation,
                start_opacity,
                end_opacity,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L153-L166 (chrome/m156) (`onSync`)
    fn sync(&self) {
        const MAX_COUNT: f32 = 1024.0;
        let node = self.base.node();
        // The pinned count is in [0 .. 1024]: the cast truncates, as static_cast<size_t>.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let count = (t_pin(self.count.get(), 0.0, MAX_COUNT) + 0.5) as usize;
        node.set_count(count);
        node.set_offset(self.offset.get());
        node.set_anchor_point(self.anchor_point.get());
        node.set_position(self.position.get());
        node.set_scale(self.scale.get() * 0.01);
        node.set_rotation(self.rotation.get());
        node.set_start_opacity(t_pin(self.start_opacity.get() * 0.01, 0.0, 1.0));
        node.set_end_opacity(t_pin(self.end_opacity.get() * 0.01, 0.0, 1.0));
    }
}

shape_adapter!(RepeaterAdapter);

impl ShapeBuilder {
    /// Attaches the repeater.
    // Port of: modules/skottie/src/layers/shapelayer/Repeater.cpp#L190-L212 (chrome/m156) (`AttachRepeaterDrawEffect`)
    #[doc(alias = "AttachRepeaterDrawEffect")]
    #[must_use]
    pub fn attach_repeater_draw_effect(
        jrepeater: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        mut draws: Draws,
    ) -> Draws {
        if let Some(jtransform) = jrepeater.get("tr").as_object() {
            // input draws are in top->bottom order - reverse for paint order
            draws.reverse();

            let adapter = RepeaterAdapter::make(jrepeater, jtransform, abuilder, draws);
            let node = Rc::clone(adapter.base.node());
            abuilder.attach_discardable_adapter(&adapter);
            vec![node as Rc<dyn RenderNode>]
        } else {
            draws
        }
    }
}

// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/MotionTileEffect.cpp (chrome/m156)
//
// The motion tile effect (AE "Tile"): the layer is mapped to a tile that is repeated (or mirrored)
// across the output area, with an optional phase shift of alternating columns or rows.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::shader::Shader;
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::color::Color4f;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::render_node::{Hit, has_children_inval};
use skia_rust_sksg::{Node, NodeCore, RenderContext, RenderNode};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ScalarValue, Vec2Value};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// The custom render node of the tile: it draws the tiled layer, and the phase pass if any.
// Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L22-L140 (chrome/m156) (`TileRenderNode`)
#[derive(Debug)]
pub(super) struct TileRenderNode {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    layer_size: (f32, f32),
    tile_center: Cell<Point>,
    tile_w: Cell<f32>,
    tile_h: Cell<f32>,
    output_w: Cell<f32>,
    output_h: Cell<f32>,
    phase: Cell<f32>,
    mirror_edges: Cell<bool>,
    horizontal_phase: Cell<bool>,
    // These are computed/cached on revalidation.
    layer_picture: RefCell<Option<Picture>>,
    main_pass_shader: RefCell<Option<Shader>>,
    phase_pass_shader: RefCell<Option<Shader>>,
}

impl TileRenderNode {
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L28-L30 (chrome/m156) (`TileRenderNode::TileRenderNode`)
    fn make(layer_size: (f32, f32), layer: Rc<dyn RenderNode>) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let node = Self {
                core: NodeCore::new(0, weak.clone()),
                child: Rc::clone(&layer),
                layer_size,
                tile_center: Cell::new(Point { x: 0.0, y: 0.0 }),
                tile_w: Cell::new(1.0),
                tile_h: Cell::new(1.0),
                output_w: Cell::new(1.0),
                output_h: Cell::new(1.0),
                phase: Cell::new(0.0),
                mirror_edges: Cell::new(false),
                horizontal_phase: Cell::new(false),
                layer_picture: RefCell::new(None),
                main_pass_shader: RefCell::new(None),
                phase_pass_shader: RefCell::new(None),
            };
            // The custom node observes its child.
            node.observe_inval(node.child.as_ref());
            node
        })
    }

    /// Sets the tile center, invalidating the node if it changed (`setTileCenter`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L31-L40 (chrome/m156) (`SG_ATTRIBUTE(TileCenter)`)
    pub(super) fn set_tile_center(&self, center: Point) {
        if self.tile_center.get() != center {
            self.tile_center.set(center);
            self.invalidate();
        }
    }

    /// Sets the tile width in percent (`setTileWidth`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE(TileWidth)`)
    pub(super) fn set_tile_width(&self, value: f32) {
        set_cell(self, &self.tile_w, value);
    }

    /// Sets the tile height in percent (`setTileHeight`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE(TileHeight)`)
    pub(super) fn set_tile_height(&self, value: f32) {
        set_cell(self, &self.tile_h, value);
    }

    /// Sets the output width in percent (`setOutputWidth`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE(OutputWidth)`)
    pub(super) fn set_output_width(&self, value: f32) {
        set_cell(self, &self.output_w, value);
    }

    /// Sets the output height in percent (`setOutputHeight`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE(OutputHeight)`)
    pub(super) fn set_output_height(&self, value: f32) {
        set_cell(self, &self.output_h, value);
    }

    /// Sets the phase (`setPhase`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE(Phase)`)
    pub(super) fn set_phase(&self, value: f32) {
        set_cell(self, &self.phase, value);
    }

    /// Sets the mirror mode (`setMirrorEdges`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE(MirrorEdges)`)
    pub(super) fn set_mirror_edges(&self, value: bool) {
        if self.mirror_edges.get() != value {
            self.mirror_edges.set(value);
            self.invalidate();
        }
    }

    /// Sets the horizontal phase mode (`setHorizontalPhase`).
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE(HorizontalPhase)`)
    pub(super) fn set_horizontal_phase(&self, value: bool) {
        if self.horizontal_phase.get() != value {
            self.horizontal_phase.set(value);
            self.invalidate();
        }
    }
}

/// Sets a scalar attribute of a tile node, invalidating it if it changed.
// Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L41-L49 (chrome/m156) (`SG_ATTRIBUTE`)
fn set_cell(node: &TileRenderNode, cell: &Cell<f32>, value: f32) {
    if cell.get() != value {
        cell.set(value);
        node.invalidate();
    }
}

impl Drop for TileRenderNode {
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L258-L262 (chrome/m156) (`CustomRenderNode::~CustomRenderNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for TileRenderNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L50-L128 (chrome/m156) (`TileRenderNode::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let (layer_w, layer_h) = self.layer_size;
        // Re-record the layer picture if needed.
        if self.layer_picture.borrow().is_none() || has_children_inval(std::slice::from_ref(&self.child))
        {
            self.child.revalidate(ic.as_deref_mut(), ctm);
            let mut recorder = PictureRecorder::new();
            let canvas = recorder.begin_recording(Rect::from_wh(layer_w, layer_h), false);
            self.child.render(canvas, None);
            *self.layer_picture.borrow_mut() = recorder.finish_recording_as_picture(None);
        }

        // tileW and tileH use layer size percentage units.
        let tile_w = t_pin(self.tile_w.get(), 0.0, 100.0) * 0.01 * layer_w;
        let tile_h = t_pin(self.tile_h.get(), 0.0, 100.0) * 0.01 * layer_h;
        let tile_size = (std_max(tile_w, 1.0), std_max(tile_h, 1.0));
        let center = self.tile_center.get();
        let tile = Rect::from_xywh(
            center.x - 0.5 * tile_size.0,
            center.y - 0.5 * tile_size.1,
            tile_size.0,
            tile_size.1,
        );
        let layer_shader_matrix = Matrix::rect_to_rect_or_identity(
            Rect::from_wh(layer_w, layer_h),
            tile,
            None,
        );
        let tm = if self.mirror_edges.get() {
            TileMode::Mirror
        } else {
            TileMode::Repeat
        };
        let layer_shader = self.layer_picture.borrow().as_ref().and_then(|picture| {
            picture.to_shader((tm, tm), FilterMode::Linear, &layer_shader_matrix, None)
        });

        let phase = self.phase.get();
        if phase != 0.0 && layer_shader.is_some() && tile.is_finite() {
            // To implement AE phase semantics, we construct a mask shader for the pass-through
            // rows/columns. We then draw the layer content through this mask, and then again
            // through the inverse mask with a phase shift.
            let phase_vec = if self.horizontal_phase.get() {
                Point::new(tile.width(), 0.0)
            } else {
                Point::new(0.0, tile.height())
            };
            let phase_factor = (phase * (1.0_f32 / 360.0_f32)) % 1.0_f32;
            let phase_shift = Point::new(phase_vec.x * phase_factor, phase_vec.y * phase_factor);
            let phase_shader_matrix = Matrix::translate(Vector::new(phase_shift.x, phase_shift.y));

            // The mask is generated using a step gradient shader, spanning 2 x tile width/height,
            // and perpendicular to the phase vector.
            let colors = [Color4f::new(1.0, 1.0, 1.0, 1.0), Color4f::new(0.0, 0.0, 0.0, 0.0)];
            let pos = [0.5_f32, 0.5];
            let pts = [
                Point::new(tile.x(), tile.y()),
                Point::new(
                    tile.x() + 2.0 * (tile.width() - phase_vec.x),
                    tile.y() + 2.0 * (tile.height() - phase_vec.y),
                ),
            ];
            let gradient = Gradient::new(
                Colors::new(&colors[..], Some(&pos[..]), TileMode::Repeat, None),
                Interpolation::default(),
            );
            // A null mask or layer leaves no shader, as Skia's blend of a null shader does.
            let mask_shader = shaders::linear_gradient((pts[0], pts[1]), &gradient, None);
            let (main, phase_pass) = match (mask_shader, layer_shader) {
                (Some(mask), Some(layer)) => {
                    // First drawing pass: in-place masked layer content.
                    let main = skia_rust_core::shaders::blend(
                        BlendMode::SrcIn,
                        mask.clone(),
                        layer.clone(),
                    );
                    // Second pass: phased-shifted layer content, with an inverse mask.
                    let phase_pass = skia_rust_core::shaders::blend(BlendMode::SrcOut, mask, layer)
                        .with_local_matrix(&phase_shader_matrix);
                    (Some(main), Some(phase_pass))
                }
                _ => (None, None),
            };
            *self.main_pass_shader.borrow_mut() = main;
            *self.phase_pass_shader.borrow_mut() = phase_pass;
        } else {
            *self.main_pass_shader.borrow_mut() = layer_shader;
            *self.phase_pass_shader.borrow_mut() = None;
        }

        // outputW and outputH also use layer size percentage units.
        let output_w = self.output_w.get() * 0.01 * layer_w;
        let output_h = self.output_h.get() * 0.01 * layer_h;
        Rect::from_xywh(
            (layer_w - output_w) * 0.5,
            (layer_h - output_h) * 0.5,
            output_w,
            output_h,
        )
    }
}

impl RenderNode for TileRenderNode {
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L130-L150 (chrome/m156) (`TileRenderNode::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let bounds = self.core().bounds();
        // AE allow one of the tile dimensions to collapse, but not both.
        if bounds.is_empty() || (self.tile_w.get() <= 0.0 && self.tile_h.get() <= 0.0) {
            return;
        }
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        if let Some(ctx) = ctx {
            // apply any pending paint effects via the shader paint
            ctx.modulate_paint(&canvas.local_to_device_as_3x3(), &mut paint, false);
        }
        paint.set_shader(self.main_pass_shader.borrow().clone());
        canvas.draw_rect(bounds, &paint);
        if let Some(phase_shader) = self.phase_pass_shader.borrow().clone() {
            paint.set_shader(Some(phase_shader));
            canvas.draw_rect(bounds, &paint);
        }
    }

    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L64-L65 (chrome/m156) (`TileRenderNode::onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<Hit> {
        // no hit-testing
        None
    }
}

/// `std::max(a, b)`: `b` only if `a < b`.
// Port of: <algorithm> std::max (chrome/m156)
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// The motion tile adapter: the tile, output, phase and mode properties.
// Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L152-L218 (chrome/m156) (`MotionTileAdapter`)
struct MotionTileAdapter {
    base: DiscardableAdapterBase<TileRenderNode>,
    tile_center: Prop<Vec2Value>,
    tile_w: Prop<ScalarValue>,
    tile_h: Prop<ScalarValue>,
    output_w: Prop<ScalarValue>,
    output_h: Prop<ScalarValue>,
    mirror_edges: Prop<ScalarValue>,
    phase: Prop<ScalarValue>,
    horizontal_phase: Prop<ScalarValue>,
}

impl MotionTileAdapter {
    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L155-L180 (chrome/m156) (`MotionTileAdapter::MotionTileAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
        layer_size: (f32, f32),
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), TileRenderNode::make(layer_size, layer));
            let tile_center = Prop::new(Vec2Value::new(0.0, 0.0));
            let tile_w = Prop::new(1.0);
            let tile_h = Prop::new(1.0);
            let output_w = Prop::new(1.0);
            let output_h = Prop::new(1.0);
            let mirror_edges = Prop::new(0.0);
            let phase = Prop::new(0.0);
            let horizontal_phase = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &tile_center)
                .bind(1, &tile_w)
                .bind(2, &tile_h)
                .bind(3, &output_w)
                .bind(4, &output_h)
                .bind(5, &mirror_edges)
                .bind(6, &phase)
                .bind(7, &horizontal_phase);
            Self {
                base,
                tile_center,
                tile_w,
                tile_h,
                output_w,
                output_h,
                mirror_edges,
                phase,
                horizontal_phase,
            }
        })
    }
}

impl AnimatablePropertyContainer for MotionTileAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L182-L195 (chrome/m156) (`MotionTileAdapter::onSync`)
    fn on_sync(&self) {
        let tiler = self.base.node();
        let center = *self.tile_center.borrow();
        tiler.set_tile_center(Point::new(center.x, center.y));
        tiler.set_tile_width(*self.tile_w.borrow());
        tiler.set_tile_height(*self.tile_h.borrow());
        tiler.set_output_width(*self.output_w.borrow());
        tiler.set_output_height(*self.output_h.borrow());
        tiler.set_phase(*self.phase.borrow());
        tiler.set_mirror_edges(*self.mirror_edges.borrow() != 0.0);
        tiler.set_horizontal_phase(*self.horizontal_phase.borrow() != 0.0);
    }
}

impl_container_animator!(MotionTileAdapter);

/// The motion tile effect (`ADBE Tile`).
// Port of: modules/skottie/src/effects/MotionTileEffect.cpp#L220-L226 (chrome/m156) (`EffectBuilder::attachMotionTileEffect`)
pub(super) fn attach_motion_tile_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let size = eb.layer_size();
    let adapter = MotionTileAdapter::make(jprops, layer, eb.builder(), (size.width, size.height));
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

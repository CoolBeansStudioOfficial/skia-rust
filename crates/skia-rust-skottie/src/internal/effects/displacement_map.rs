// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/DisplacementMapEffect.cpp (chrome/m156)
//
// The displacement map effect: a custom render node that displaces the layer by a second layer
// (the map), with an SkSL runtime shader that selects and scales the displacement channels.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color_data::{LUM_COEFF_B, LUM_COEFF_G, LUM_COEFF_R};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeEffectBuilder};
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, scalar_abs};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::render_node::Hit;
use skia_rust_sksg::{Node, NodeCore, RenderContext, RenderNode, ScopedRenderContext};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_json::{ValueExt, parse_default};
use crate::skottie_value::ScalarValue;

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{
    EffectBinder, EffectBuilder, LayerContent, attach_adapter_node, get_content_picture,
};

/// The displacement SkSL: the selector matrix picks the displacement and the coverage from the
/// map, and the child is sampled at the displaced position.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L23-L33 (chrome/m156) (`gDisplacementSkSL`)
const DISPLACEMENT_SKSL: &str = concat!(
    "uniform shader child;",
    "uniform shader displ;",
    "uniform half4x4 selector_matrix;",
    "uniform half4   selector_offset;",
    "half4 main(float2 xy) {",
    "half4 d = displ.eval(xy);",
    "d = selector_matrix*unpremul(d) + selector_offset;",
    "return child.eval(xy + d.xy*d.zw);",
    "}",
);

thread_local! {
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L35-L44 (chrome/m156) (`displacement_effect_singleton`)
    static DISPLACEMENT_EFFECT: RuntimeEffect =
        RuntimeEffect::make_for_shader(DISPLACEMENT_SKSL, None)
            .expect("the displacement effect compiles");
}

/// The position of the map in the layer (`DisplacementNode::Pos`).
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L64-L69 (chrome/m156) (`DisplacementNode::Pos`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pos {
    Center,
    Stretch,
    Tile,
}

impl Pos {
    /// The last valid position (`kLast`).
    const LAST: u32 = 2;
}

/// The channel selector of the map (`DisplacementNode::Selector`).
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L71-L84 (chrome/m156) (`DisplacementNode::Selector`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Selector {
    R,
    G,
    B,
    A,
    Luminance,
    Hue,
    Lightness,
    Saturation,
    Full,
    Half,
    Off,
}

impl Selector {
    /// The last valid selector (`kLast`).
    const LAST: u32 = 10;
}

/// The displacement and coverage coefficients of a selector (`SelectorCoeffs`).
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L150-L155 (chrome/m156) (`SelectorCoeffs`)
#[derive(Debug, Clone, Copy)]
struct SelectorCoeffs {
    dr: f32,
    dg: f32,
    db: f32,
    da: f32,
    d_offset: f32,
    c_scale: f32,
    c_offset: f32,
}

/// The coefficients of a selector (`Coeffs`).
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L157-L177 (chrome/m156) (`DisplacementNode::Coeffs`)
fn coeffs(sel: Selector) -> SelectorCoeffs {
    // D = displacement input
    // C = displacement coverage
    let c = |dr, dg, db, da, d_offset, c_scale, c_offset| SelectorCoeffs {
        dr,
        dg,
        db,
        da,
        d_offset,
        c_scale,
        c_offset,
    };
    match sel {
        Selector::R => c(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0), // kR: D = r, C = a
        Selector::G => c(0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0), // kG: D = g, C = a
        Selector::B => c(0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0), // kB: D = b, C = a
        Selector::A => c(0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0), // kA: D = a, C = 1.0
        Selector::Luminance => c(LUM_COEFF_R, LUM_COEFF_G, LUM_COEFF_B, 0.0, 0.0, 1.0, 0.0),
        Selector::Hue => c(1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0), // kH: D = h, C = 1.0 (HSLA)
        Selector::Lightness => c(0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0), // kL: D = l, C = 1.0
        Selector::Saturation => c(0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0), // kS: D = s, C = 1.0
        Selector::Full => c(0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0), // kFull: D = 1.0, C = 1.0
        Selector::Half => c(0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 1.0), // kHalf: D = 0.5, C = 1.0
        Selector::Off => c(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0),  // kOff: D = 0.0, C = 1.0
    }
}

/// True for the selectors that generate a constant map (`IsConst`).
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L179-L183 (chrome/m156) (`DisplacementNode::IsConst`)
fn is_const(s: Selector) -> bool {
    matches!(s, Selector::Full | Selector::Half | Selector::Off)
}

/// The displacement node: displaces its child by the map layer.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L49-L62 (chrome/m156) (`DisplacementNode`)
#[derive(Debug)]
pub(super) struct DisplacementNode {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    displ_source: Rc<dyn RenderNode>,
    displ_size: Size,
    child_size: Size,
    // Cached top-level shader.
    effect_shader: RefCell<Option<Shader>>,
    scale: Cell<(f32, f32)>,
    child_tile_mode: Cell<TileMode>,
    pos: Cell<Pos>,
    x_selector: Cell<Selector>,
    y_selector: Cell<Selector>,
    expand_bounds: Cell<bool>,
}

impl DisplacementNode {
    /// `DisplacementNode::Make`: `None` if either the child or the map is missing.
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L63-L72 (chrome/m156) (`DisplacementNode::Make`)
    fn make(
        child: Option<Rc<dyn RenderNode>>,
        child_size: Size,
        displ: Option<Rc<dyn RenderNode>>,
        displ_size: Size,
    ) -> Option<Rc<Self>> {
        let child = child?;
        let displ = displ?;
        let node = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            displ_source: Rc::clone(&displ),
            displ_size,
            child_size,
            effect_shader: RefCell::new(None),
            scale: Cell::new((0.0, 0.0)),
            child_tile_mode: Cell::new(TileMode::Decal),
            pos: Cell::new(Pos::Center),
            x_selector: Cell::new(Selector::R),
            y_selector: Cell::new(Selector::R),
            expand_bounds: Cell::new(false),
        });
        // The custom node observes its child and its map.
        node.observe_inval(node.child.as_ref());
        node.observe_inval(node.displ_source.as_ref());
        Some(node)
    }

    /// Sets the displacement scale, invalidating the node if it changed (`setScale`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L133-L138 (chrome/m156) (`SG_ATTRIBUTE(Scale)`)
    pub(super) fn set_scale(&self, scale: (f32, f32)) {
        if self.scale.get() != scale {
            self.scale.set(scale);
            self.invalidate();
        }
    }

    /// Sets the tile mode of the child (`setChildTileMode`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L133-L138 (chrome/m156) (`SG_ATTRIBUTE(ChildTileMode)`)
    pub(super) fn set_child_tile_mode(&self, mode: TileMode) {
        if self.child_tile_mode.get() != mode {
            self.child_tile_mode.set(mode);
            self.invalidate();
        }
    }

    /// Sets the position of the map (`setPos`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L133-L138 (chrome/m156) (`SG_ATTRIBUTE(Pos)`)
    pub(super) fn set_pos(&self, pos: Pos) {
        if self.pos.get() != pos {
            self.pos.set(pos);
            self.invalidate();
        }
    }

    /// Sets the horizontal selector (`setXSelector`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L133-L138 (chrome/m156) (`SG_ATTRIBUTE(XSelector)`)
    pub(super) fn set_x_selector(&self, sel: Selector) {
        if self.x_selector.get() != sel {
            self.x_selector.set(sel);
            self.invalidate();
        }
    }

    /// Sets the vertical selector (`setYSelector`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L133-L138 (chrome/m156) (`SG_ATTRIBUTE(YSelector)`)
    pub(super) fn set_y_selector(&self, sel: Selector) {
        if self.y_selector.get() != sel {
            self.y_selector.set(sel);
            self.invalidate();
        }
    }

    /// Sets whether the bounds are expanded by the displacement (`setExpandBounds`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L133-L138 (chrome/m156) (`SG_ATTRIBUTE(ExpandBounds)`)
    pub(super) fn set_expand_bounds(&self, expand: bool) {
        if self.expand_bounds.get() != expand {
            self.expand_bounds.set(expand);
            self.invalidate();
        }
    }

    /// The tile mode of the map (`displacementTileMode`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L195-L199 (chrome/m156) (`DisplacementNode::displacementTileMode`)
    fn displacement_tile_mode(&self) -> TileMode {
        if self.pos.get() == Pos::Tile {
            TileMode::Repeat
        } else {
            TileMode::Clamp
        }
    }

    /// The transform of the map in the layer (`displacementMatrix`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L201-L214 (chrome/m156) (`DisplacementNode::displacementMatrix`)
    fn displacement_matrix(&self) -> Matrix {
        match self.pos.get() {
            Pos::Center => Matrix::translate(Vector::new(
                (self.child_size.width - self.displ_size.width) / 2.0,
                (self.child_size.height - self.displ_size.height) / 2.0,
            )),
            Pos::Stretch => Matrix::scale((
                self.child_size.width / self.displ_size.width,
                self.child_size.height / self.displ_size.height,
            )),
            Pos::Tile => Matrix::new_identity(),
        }
    }

    /// Builds the displacement shader (`buildEffectShader`).
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L81-L148 (chrome/m156) (`DisplacementNode::buildEffectShader`)
    fn build_effect_shader(
        &self,
        mut ic: Option<&mut InvalidationController>,
        ctm: &Matrix,
    ) -> Option<Shader> {
        let (sx, sy) = self.scale.get();
        let x_selector = self.x_selector.get();
        let y_selector = self.y_selector.get();
        // AE quirk: combining two const/generated modes does not displace - we need at least one
        // non-const selector to trigger the effect.
        if (is_const(x_selector) && is_const(y_selector))
            || (scalar_abs(sx) <= SCALAR_NEARLY_ZERO && scalar_abs(sy) <= SCALAR_NEARLY_ZERO)
        {
            return None;
        }

        let child_content = get_content_picture(Some(&self.child), ic.as_deref_mut(), ctm);
        let displ_content = get_content_picture(Some(&self.displ_source), ic, ctm);
        let child_content = child_content?;
        let displ_content = displ_content?;

        let child_tile = Rect::from_size(self.child_size);
        let child_mode = self.child_tile_mode.get();
        let child_shader = child_content.to_shader(
            (child_mode, child_mode),
            FilterMode::Linear,
            None::<&Matrix>,
            &child_tile,
        );

        let displ_tile = Rect::from_size(self.displ_size);
        let displ_mode = self.displacement_tile_mode();
        let displ_matrix = self.displacement_matrix();
        let displ_shader = displ_content.to_shader(
            (displ_mode, displ_mode),
            FilterMode::Linear,
            &displ_matrix,
            &displ_tile,
        );

        let xc = coeffs(x_selector);
        let yc = coeffs(y_selector);
        let s = (sx * 2.0, sy * 2.0);
        let selector_m: [f32; 16] = [
            xc.dr * s.0,
            yc.dr * s.1,
            0.0,
            0.0,
            xc.dg * s.0,
            yc.dg * s.1,
            0.0,
            0.0,
            xc.db * s.0,
            yc.db * s.1,
            0.0,
            0.0,
            xc.da * s.0,
            yc.da * s.1,
            xc.c_scale,
            yc.c_scale,
        ];
        let selector_o: [f32; 4] = [
            (xc.d_offset - 0.5) * s.0,
            (yc.d_offset - 0.5) * s.1,
            xc.c_offset,
            yc.c_offset,
        ];

        DISPLACEMENT_EFFECT.with(|effect| {
            let mut builder = RuntimeEffectBuilder::new(effect.clone());
            assign_child(&mut builder, "child", child_shader);
            assign_child(&mut builder, "displ", displ_shader);
            builder.uniform("selector_matrix").set_f32(&selector_m);
            builder.uniform("selector_offset").set_f32(&selector_o);
            // TODO in Skia: RGB->HSL stage
            builder.make_shader(None)
        })
    }
}

/// Assigns a child shader of the runtime shader builder (a null shader is assigned as null).
fn assign_child(
    builder: &mut RuntimeEffectBuilder,
    name: &str,
    shader: Option<Shader>,
) {
    match shader {
        Some(shader) => {
            builder.child(name).assign(shader);
        }
        None => {
            builder.child(name).assign_null();
        }
    }
}

impl Drop for DisplacementNode {
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L52-L54 (chrome/m156) (`DisplacementNode::~DisplacementNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.displ_source.as_ref());
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for DisplacementNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L156-L170 (chrome/m156) (`DisplacementNode::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let shader = self.build_effect_shader(ic.as_deref_mut(), ctm);
        *self.effect_shader.borrow_mut() = shader;
        let mut bounds = self.child.revalidate(ic, ctm);
        if self.expand_bounds.get() {
            // Expand the bounds to accommodate max displacement (which is |scale|).
            let (sx, sy) = self.scale.get();
            bounds.outset(Vector::new(scalar_abs(sx), scalar_abs(sy)));
        }
        bounds
    }
}

impl RenderNode for DisplacementNode {
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L172-L187 (chrome/m156) (`DisplacementNode::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let shader = self.effect_shader.borrow().clone();
        let Some(shader) = shader else {
            // no displacement effect - just render the content
            self.child.render(canvas, ctx);
            return;
        };
        let bounds = self.core().bounds();
        let _scope = ScopedRenderContext::new(canvas, ctx).set_isolation(
            &bounds,
            &canvas.total_matrix(),
            true,
        );
        let mut shader_paint = Paint::default();
        shader_paint.set_shader(Some(shader));
        canvas.draw_rect(bounds, &shader_paint);
    }

    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L189-L190 (chrome/m156) (`DisplacementNode::onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<Hit> {
        // no hit-testing
        None
    }
}

/// Maps a one-based float "enum" to the enum type (`ToEnum`): the value is clamped to the last
/// entry.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L232-L238 (chrome/m156) (`DisplacementMapAdapter::ToEnum`)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// mirrors the `static_cast<unsigned>(v)` of the one-based float enums
fn to_index(v: f32, last: u32) -> usize {
    (v as u32).wrapping_sub(1).min(last) as usize
}

/// The position of the map from its one-based float value.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L232-L238 (chrome/m156) (`ToEnum<Pos>`)
fn pos_from_value(v: f32) -> Pos {
    [Pos::Center, Pos::Stretch, Pos::Tile][to_index(v, Pos::LAST)]
}

/// The selector from its one-based float value.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L232-L238 (chrome/m156) (`ToEnum<Selector>`)
fn selector_from_value(v: f32) -> Selector {
    const SELECTORS: [Selector; 11] = [
        Selector::R,
        Selector::G,
        Selector::B,
        Selector::A,
        Selector::Luminance,
        Selector::Hue,
        Selector::Lightness,
        Selector::Saturation,
        Selector::Full,
        Selector::Half,
        Selector::Off,
    ];
    SELECTORS[to_index(v, Selector::LAST)]
}

/// The displacement map adapter: the selectors, the scales and the behaviors.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L213-L262 (chrome/m156) (`DisplacementMapAdapter`)
struct DisplacementMapAdapter {
    base: DiscardableAdapterBase<DisplacementNode>,
    horizontal_selector: Prop<ScalarValue>,
    max_horizontal: Prop<ScalarValue>,
    vertical_selector: Prop<ScalarValue>,
    max_vertical: Prop<ScalarValue>,
    map_behavior: Prop<ScalarValue>,
    edge_behavior: Prop<ScalarValue>,
    expand_output: Prop<ScalarValue>,
}

impl DisplacementMapAdapter {
    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L227-L243 (chrome/m156) (`DisplacementMapAdapter::DisplacementMapAdapter`)
    fn make(
        jprops: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        node: Rc<DisplacementNode>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let horizontal_selector = Prop::new(0.0);
            let max_horizontal = Prop::new(0.0);
            let vertical_selector = Prop::new(0.0);
            let max_vertical = Prop::new(0.0);
            let map_behavior = Prop::new(0.0);
            let edge_behavior = Prop::new(0.0);
            let expand_output = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(USE_FOR_HORIZONTAL_INDEX, &horizontal_selector)
                .bind(MAX_HORIZONTAL_INDEX, &max_horizontal)
                .bind(USE_FOR_VERTICAL_INDEX, &vertical_selector)
                .bind(MAX_VERTICAL_INDEX, &max_vertical)
                .bind(MAP_BEHAVIOR_INDEX, &map_behavior)
                .bind(EDGE_BEHAVIOR_INDEX, &edge_behavior)
                .bind(EXPAND_OUTPUT_INDEX, &expand_output);
            Self {
                base,
                horizontal_selector,
                max_horizontal,
                vertical_selector,
                max_vertical,
                map_behavior,
                edge_behavior,
                expand_output,
            }
        })
    }
}

/// The index of the map layer property.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L213-L224 (chrome/m156) (`kMapLayer_Index`)
const MAP_LAYER_INDEX: usize = 0;
const USE_FOR_HORIZONTAL_INDEX: usize = 1;
const MAX_HORIZONTAL_INDEX: usize = 2;
const USE_FOR_VERTICAL_INDEX: usize = 3;
const MAX_VERTICAL_INDEX: usize = 4;
const MAP_BEHAVIOR_INDEX: usize = 5;
const EDGE_BEHAVIOR_INDEX: usize = 6;
const EXPAND_OUTPUT_INDEX: usize = 7;

impl AnimatablePropertyContainer for DisplacementMapAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L245-L256 (chrome/m156) (`DisplacementMapAdapter::onSync`)
    fn on_sync(&self) {
        let node = self.base.node();
        node.set_scale((*self.max_horizontal.borrow(), *self.max_vertical.borrow()));
        node.set_child_tile_mode(if *self.edge_behavior.borrow() != 0.0 {
            TileMode::Repeat
        } else {
            TileMode::Decal
        });
        node.set_pos(pos_from_value(*self.map_behavior.borrow()));
        node.set_x_selector(selector_from_value(*self.horizontal_selector.borrow()));
        node.set_y_selector(selector_from_value(*self.vertical_selector.borrow()));
        node.set_expand_bounds(*self.expand_output.borrow() != 0.0);
    }
}

impl_container_animator!(DisplacementMapAdapter);

/// The displacement source: the content and size of the map layer (`GetDisplacementSource`).
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L214-L220 (chrome/m156) (`DisplacementMapAdapter::GetDisplacementSource`)
fn get_displacement_source(jprops: &ArrayValue, eb: &EffectBuilder<'_, '_>) -> LayerContent {
    match EffectBuilder::get_prop_value(jprops, MAP_LAYER_INDEX).as_object() {
        Some(jv) => eb.get_layer_content(parse_default::<i32>(jv.get("k"), -1)),
        None => LayerContent {
            content: None,
            size: Size::new(0.0, 0.0),
        },
    }
}

/// The displacement map effect (`ADBE Displacement Map`).
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L264-L279 (chrome/m156) (`EffectBuilder::attachDisplacementMapEffect`)
pub(super) fn attach_displacement_map_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let displ = get_displacement_source(jprops, eb);
    let Some(displ_node) = DisplacementNode::make(
        layer.clone(),
        eb.layer_size(),
        displ.content,
        displ.size,
    ) else {
        return layer;
    };
    let adapter = DisplacementMapAdapter::make(jprops, eb.builder(), displ_node);
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

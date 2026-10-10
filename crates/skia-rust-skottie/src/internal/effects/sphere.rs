// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/SphereEffect.cpp (chrome/m156)
//
// The CC Sphere effect: a custom render node that maps the layer onto a sphere, lit by a Phong-like
// model. The sphere shader is an SkSL runtime shader, specialized by the lighting model.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::floating_point::{FLOAT_PI, float_degrees_to_radians};
use skia_rust_core::m44::{M44, V3};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeEffectBuilder};
use skia_rust_core::scalar::{scalar_cos, scalar_round_to_int, scalar_sin};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::render_node::{Hit, has_children_inval};
use skia_rust_sksg::{Node, NodeCore, RenderContext, RenderNode};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ColorValue, ScalarValue, Vec2Value};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node, repeating_content_shader};

/// The sphere SkSL: the eye ray is cast to the unit sphere, rotated, UV-mapped, and lit by the
/// `apply_light()` of the lighting model (`%s`).
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L31-L63 (chrome/m156) (`gSphereSkSL`)
const SPHERE_SKSL: &str = concat!(
    "uniform shader child;",
    "uniform half3x3 rot_matrix;",
    "uniform half2 child_scale;",
    "uniform half side_select;",
    "%s",
    "half3 to_sphere(half3 EYE) {",
    "half eye_z2 = EYE.z*EYE.z;",
    "half a = dot(EYE, EYE),",
    "b = -2*eye_z2,",
    "c = eye_z2 - 1,",
    "t = (-b + side_select*sqrt(b*b - 4*a*c))/(2*a);",
    "return half3(0, 0, -EYE.z) + EYE*t;",
    "}",
    "half4 main(float2 xy) {",
    "half3 EYE = half3(xy, -5.5),",
    "N = to_sphere(EYE),",
    "RN = rot_matrix*N;",
    "half kRPI = 1/3.1415927;",
    "half2 UV = half2(",
    "0.5 + kRPI * 0.5 * atan(RN.x, RN.z),",
    "0.5 + kRPI * asin(RN.y)",
    ");",
    "return apply_light(EYE, N, child.eval(UV*child_scale));",
    "}",
);

/// The basic lighting model: the ambient light only.
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L72-L78 (chrome/m156) (`gBasicLightSkSL`)
const BASIC_LIGHT_SKSL: &str = concat!(
    "uniform half l_coeff_ambient;",
    "half4 apply_light(half3 EYE, half3 N, half4 c) {",
    "c.rgb *= l_coeff_ambient;",
    "return c;",
    "}",
);

/// The Phong-like lighting model: ambient, diffuse and specular components.
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L80-L99 (chrome/m156) (`gFancyLightSkSL`)
const FANCY_LIGHT_SKSL: &str = concat!(
    "uniform half3 l_vec;",
    "uniform half3 l_color;",
    "uniform half l_coeff_ambient;",
    "uniform half l_coeff_diffuse;",
    "uniform half l_coeff_specular;",
    "uniform half l_specular_exp;",
    "half4 apply_light(half3 EYE, half3 N, half4 c) {",
    "half3 LR = reflect(-l_vec*side_select, N);",
    "half s_base = max(dot(normalize(EYE), LR), 0),",
    "a = l_coeff_ambient,",
    "d = l_coeff_diffuse * max(dot(l_vec, N), 0),",
    "s = l_coeff_specular * saturate(pow(s_base, l_specular_exp));",
    "c.rgb = (a + d*l_color)*c.rgb + s*l_color*c.a;",
    "return c;",
    "}",
);

thread_local! {
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L101-L108 (chrome/m156) (`sphere_fancylight_effect`)
    static FANCY_LIGHT_EFFECT: RuntimeEffect = RuntimeEffect::make_for_shader(
        SPHERE_SKSL.replacen("%s", FANCY_LIGHT_SKSL, 1),
        None,
    )
    .expect("the fancy light sphere effect compiles");

    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L110-L115 (chrome/m156) (`sphere_basiclight_effect`)
    static BASIC_LIGHT_EFFECT: RuntimeEffect = RuntimeEffect::make_for_shader(
        SPHERE_SKSL.replacen("%s", BASIC_LIGHT_SKSL, 1),
        None,
    )
    .expect("the basic light sphere effect compiles");
}

/// The sides of the sphere that are rendered (`SphereNode::RenderSide`).
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L131-L135 (chrome/m156) (`RenderSide`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RenderSide {
    Full,
    Outside,
    Inside,
}

/// The sphere node: maps the layer onto a lit sphere.
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L125-L210 (chrome/m156) (`SphereNode`)
#[derive(Debug)]
pub(super) struct SphereNode {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    child_size: Size,
    // Cached shaders.
    sphere_shader: RefCell<Option<Shader>>,
    content_shader: RefCell<Option<Shader>>,
    // Effect controls.
    rot: Cell<M44>,
    center: Cell<Point>,
    radius: Cell<f32>,
    side: Cell<RenderSide>,
    light_vec: Cell<V3>,
    light_color: Cell<V3>,
    ambient_light: Cell<f32>,
    diffuse_light: Cell<f32>,
    specular_light: Cell<f32>,
    specular_exp: Cell<f32>,
}

impl SphereNode {
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L128-L130 (chrome/m156) (`SphereNode::SphereNode`)
    fn make(child: Rc<dyn RenderNode>, child_size: Size) -> Rc<Self> {
        let node = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            child_size,
            sphere_shader: RefCell::new(None),
            content_shader: RefCell::new(None),
            rot: Cell::new(M44::new_identity()),
            center: Cell::new(Point { x: 0.0, y: 0.0 }),
            radius: Cell::new(0.0),
            side: Cell::new(RenderSide::Full),
            light_vec: Cell::new(V3::new(0.0, 0.0, 1.0)),
            light_color: Cell::new(V3::new(1.0, 1.0, 1.0)),
            ambient_light: Cell::new(1.0),
            diffuse_light: Cell::new(0.0),
            specular_light: Cell::new(0.0),
            specular_exp: Cell::new(0.0),
        });
        // The custom node observes its child.
        node.observe_inval(node.child.as_ref());
        node
    }

    /// Sets the sphere center, invalidating the node if it changed (`setCenter`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L137-L141 (chrome/m156) (`SG_ATTRIBUTE(Center)`)
    pub(super) fn set_center(&self, center: Point) {
        if self.center.get() != center {
            self.center.set(center);
            self.invalidate();
        }
    }

    /// Sets the sphere radius (`setRadius`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L142-L146 (chrome/m156) (`SG_ATTRIBUTE(Radius)`)
    pub(super) fn set_radius(&self, radius: f32) {
        set_scalar(self, &self.radius, radius);
    }

    /// Sets the rotation (`setRotation`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L143-L147 (chrome/m156) (`SG_ATTRIBUTE(Rotation)`)
    pub(super) fn set_rotation(&self, rot: M44) {
        if self.rot.get() != rot {
            self.rot.set(rot);
            self.invalidate();
        }
    }

    /// Sets the rendered sides (`setSide`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L148-L152 (chrome/m156) (`SG_ATTRIBUTE(Side)`)
    pub(super) fn set_side(&self, side: RenderSide) {
        if self.side.get() != side {
            self.side.set(side);
            self.invalidate();
        }
    }

    /// Sets the light vector (`setLightVec`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L153-L157 (chrome/m156) (`SG_ATTRIBUTE(LightVec)`)
    pub(super) fn set_light_vec(&self, v: V3) {
        if self.light_vec.get() != v {
            self.light_vec.set(v);
            self.invalidate();
        }
    }

    /// Sets the light color (`setLightColor`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L158-L162 (chrome/m156) (`SG_ATTRIBUTE(LightColor)`)
    pub(super) fn set_light_color(&self, v: V3) {
        if self.light_color.get() != v {
            self.light_color.set(v);
            self.invalidate();
        }
    }

    /// Sets the ambient light (`setAmbientLight`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L163-L167 (chrome/m156) (`SG_ATTRIBUTE(AmbientLight)`)
    pub(super) fn set_ambient_light(&self, v: f32) {
        set_scalar(self, &self.ambient_light, v);
    }

    /// Sets the diffuse light (`setDiffuseLight`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L168-L172 (chrome/m156) (`SG_ATTRIBUTE(DiffuseLight)`)
    pub(super) fn set_diffuse_light(&self, v: f32) {
        set_scalar(self, &self.diffuse_light, v);
    }

    /// Sets the specular light (`setSpecularLight`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L173-L177 (chrome/m156) (`SG_ATTRIBUTE(SpecularLight)`)
    pub(super) fn set_specular_light(&self, v: f32) {
        set_scalar(self, &self.specular_light, v);
    }

    /// Sets the specular exponent (`setSpecularExp`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L178-L182 (chrome/m156) (`SG_ATTRIBUTE(SpecularExp)`)
    pub(super) fn set_specular_exp(&self, v: f32) {
        set_scalar(self, &self.specular_exp, v);
    }

    /// The layer content as a repeating picture shader (`contentShader`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L184-L195 (chrome/m156) (`SphereNode::contentShader`)
    fn content_shader(&self) -> Option<Shader> {
        if self.content_shader.borrow().is_none()
            || has_children_inval(std::slice::from_ref(&self.child))
        {
            *self.content_shader.borrow_mut() =
                repeating_content_shader(&self.child, self.child_size);
        }
        self.content_shader.borrow().clone()
    }

    /// The shader of one side of the sphere (`buildEffectShader`).
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L197-L238 (chrome/m156) (`SphereNode::buildEffectShader`)
    fn build_effect_shader(&self, selector: f32) -> Option<Shader> {
        let light_vec = self.light_vec.get();
        let diffuse = self.diffuse_light.get();
        let specular = self.specular_light.get();
        let has_fancy_light = light_vec.length() > 0.0 && (diffuse > 0.0 || specular > 0.0);

        let child_shader = self.content_shader();
        let rot = self.rot.get();
        let child_size = self.child_size;
        let build = |effect: &RuntimeEffect| {
            let mut builder = RuntimeEffectBuilder::new(effect.clone());
            assign_child(&mut builder, child_shader.clone());
            builder
                .uniform("child_scale")
                .set_f32(&[child_size.width, child_size.height]);
            builder.uniform("side_select").set_f32(&[selector]);
            let r = |row, col| rot.rc(row, col);
            builder.uniform("rot_matrix").set_f32(&[
                r(0, 0),
                r(0, 1),
                r(0, 2),
                r(1, 0),
                r(1, 1),
                r(1, 2),
                r(2, 0),
                r(2, 1),
                r(2, 2),
            ]);
            builder
                .uniform("l_coeff_ambient")
                .set_f32(&[self.ambient_light.get()]);
            if has_fancy_light {
                let l_vec = light_vec * -selector;
                let l_color = self.light_color.get();
                builder.uniform("l_vec").set_f32(&[l_vec.x, l_vec.y, l_vec.z]);
                builder
                    .uniform("l_color")
                    .set_f32(&[l_color.x, l_color.y, l_color.z]);
                builder.uniform("l_coeff_diffuse").set_f32(&[diffuse]);
                builder.uniform("l_coeff_specular").set_f32(&[specular]);
                builder
                    .uniform("l_specular_exp")
                    .set_f32(&[self.specular_exp.get()]);
            }
            let center = self.center.get();
            let radius = self.radius.get();
            let lm = Matrix::translate(center) * Matrix::scale((radius, radius));
            builder.make_shader(&lm)
        };

        if has_fancy_light {
            FANCY_LIGHT_EFFECT.with(build)
        } else {
            BASIC_LIGHT_EFFECT.with(build)
        }
    }
}

/// Sets a scalar attribute of the sphere node, invalidating it if it changed.
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L142-L182 (chrome/m156) (`SG_ATTRIBUTE`)
fn set_scalar(node: &SphereNode, cell: &Cell<f32>, value: f32) {
    if cell.get() != value {
        cell.set(value);
        node.invalidate();
    }
}

/// Assigns the child shader of the sphere builder (a null shader is assigned as null).
fn assign_child(builder: &mut RuntimeEffectBuilder, shader: Option<Shader>) {
    match shader {
        Some(shader) => {
            builder.child("child").assign(shader);
        }
        None => {
            builder.child("child").assign_null();
        }
    }
}

impl Drop for SphereNode {
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L258-L262 (chrome/m156) (`CustomRenderNode::~CustomRenderNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for SphereNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L240-L258 (chrome/m156) (`SphereNode::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        *self.sphere_shader.borrow_mut() = None;
        if self.side.get() != RenderSide::Outside {
            *self.sphere_shader.borrow_mut() = self.build_effect_shader(1.0);
        }
        if self.side.get() != RenderSide::Inside {
            let outside = self.build_effect_shader(-1.0);
            let current = self.sphere_shader.borrow().clone();
            *self.sphere_shader.borrow_mut() = match (current, outside) {
                (Some(inside), Some(outside)) => Some(skia_rust_core::shaders::blend(
                    BlendMode::SrcOver,
                    inside,
                    outside,
                )),
                (current, outside) => current.or(outside),
            };
        }
        let center = self.center.get();
        let radius = self.radius.get();
        Rect::from_ltrb(
            center.x - radius,
            center.y - radius,
            center.x + radius,
            center.y + radius,
        )
    }
}

impl RenderNode for SphereNode {
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L260-L268 (chrome/m156) (`SphereNode::onRender`)
    fn on_render(&self, canvas: &Canvas, _ctx: Option<&RenderContext>) {
        let radius = self.radius.get();
        if radius <= 0.0 {
            return;
        }
        let mut sphere_paint = Paint::default();
        sphere_paint.set_anti_alias(true);
        sphere_paint.set_shader(self.sphere_shader.borrow().clone());
        canvas.draw_circle(self.center.get(), radius, &sphere_paint);
    }

    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L270 (chrome/m156) (`SphereNode::onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<Hit> {
        // no hit-testing
        None
    }
}

/// The sphere adapter: the geometry, rotation, rendering side, light and shading properties.
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L272-L345 (chrome/m156) (`SphereAdapter`)
struct SphereAdapter {
    base: DiscardableAdapterBase<SphereNode>,
    offset: Prop<Vec2Value>,
    radius: Prop<ScalarValue>,
    rot_x: Prop<ScalarValue>,
    rot_y: Prop<ScalarValue>,
    rot_z: Prop<ScalarValue>,
    rot_order: Prop<ScalarValue>,
    render: Prop<ScalarValue>,
    light_intensity: Prop<ScalarValue>,
    light_color: Prop<ColorValue>,
    light_height: Prop<ScalarValue>,
    light_direction: Prop<ScalarValue>,
    ambient: Prop<ScalarValue>,
    diffuse: Prop<ScalarValue>,
    specular: Prop<ScalarValue>,
    roughness: Prop<ScalarValue>,
}

impl SphereAdapter {
    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L276-L305 (chrome/m156) (`SphereAdapter::SphereAdapter`)
    fn make(
        jprops: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        node: Rc<SphereNode>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let offset = Prop::new(Vec2Value::new(0.0, 0.0));
            let radius = Prop::new(0.0);
            let rot_x = Prop::new(0.0);
            let rot_y = Prop::new(0.0);
            let rot_z = Prop::new(0.0);
            let rot_order = Prop::new(1.0);
            let render = Prop::new(1.0);
            let light_intensity = Prop::new(0.0);
            let light_color = Prop::new(ColorValue::new());
            let light_height = Prop::new(0.0);
            let light_direction = Prop::new(0.0);
            let ambient = Prop::new(100.0);
            let diffuse = Prop::new(0.0);
            let specular = Prop::new(0.0);
            let roughness = Prop::new(0.5);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(7, &offset)
                .bind(6, &radius)
                .bind(1, &rot_x)
                .bind(2, &rot_y)
                .bind(3, &rot_z)
                .bind(4, &rot_order)
                .bind(8, &render)
                .bind(10, &light_intensity)
                .bind(11, &light_color)
                .bind(12, &light_height)
                .bind(13, &light_direction)
                .bind(16, &ambient)
                .bind(17, &diffuse)
                .bind(18, &specular)
                .bind(19, &roughness);
            Self {
                base,
                offset,
                radius,
                rot_x,
                rot_y,
                rot_z,
                rot_order,
                render,
                light_intensity,
                light_color,
                light_height,
                light_direction,
                ambient,
                diffuse,
                specular,
                roughness,
            }
        })
    }
}

/// The side of the sphere from its one-based value (`side` of `onSync`).
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L308-L316 (chrome/m156) (`SphereAdapter::onSync::side`)
fn side_from_value(s: f32) -> RenderSide {
    match scalar_round_to_int(s) {
        1 => RenderSide::Full,
        2 => RenderSide::Outside,
        _ => RenderSide::Inside,
    }
}

/// The rotation from the rotation order and the x, y, z angles in degrees (`rotation`).
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L317-L332 (chrome/m156) (`SphereAdapter::onSync::rotation`)
fn rotation_from_values(order: f32, x: f32, y: f32, z: f32) -> M44 {
    let rx = M44::rotate(V3::new(1.0, 0.0, 0.0), float_degrees_to_radians(x));
    let ry = M44::rotate(V3::new(0.0, 1.0, 0.0), float_degrees_to_radians(y));
    let rz = M44::rotate(V3::new(0.0, 0.0, 1.0), float_degrees_to_radians(-z));
    match scalar_round_to_int(order) {
        1 => &(&rx * &ry) * &rz,
        2 => &(&rx * &rz) * &ry,
        3 => &(&ry * &rx) * &rz,
        4 => &(&ry * &rz) * &rx,
        5 => &(&rz * &rx) * &ry,
        _ => &(&rz * &ry) * &rx,
    }
}

/// The light vector from its height (in `[-1, 1]`) and its direction in radians (`light_vec`).
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L333-L339 (chrome/m156) (`SphereAdapter::onSync::light_vec`)
fn light_vec(height: f32, direction: f32) -> V3 {
    let z = scalar_sin(height * FLOAT_PI / 2.0);
    let r = (1.0_f32 - z * z).sqrt();
    let x = scalar_cos(direction) * r;
    let y = scalar_sin(direction) * r;
    V3::new(x, y, z)
}

impl AnimatablePropertyContainer for SphereAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/SphereEffect.cpp#L306-L345 (chrome/m156) (`SphereAdapter::onSync`)
    fn on_sync(&self) {
        let sph = self.base.node();
        let offset = *self.offset.borrow();
        sph.set_center(Point::new(offset.x, offset.y));
        sph.set_radius(*self.radius.borrow());
        sph.set_side(side_from_value(*self.render.borrow()));
        sph.set_rotation(rotation_from_values(
            *self.rot_order.borrow(),
            *self.rot_x.borrow(),
            *self.rot_y.borrow(),
            *self.rot_z.borrow(),
        ));
        sph.set_ambient_light(t_pin(*self.ambient.borrow() * 0.01, 0.0, 2.0));
        let intensity = t_pin(*self.light_intensity.borrow() * 0.01, 0.0, 10.0);
        sph.set_diffuse_light(t_pin(*self.diffuse.borrow() * 0.01, 0.0, 1.0) * intensity);
        sph.set_specular_light(t_pin(*self.specular.borrow() * 0.01, 0.0, 1.0) * intensity);
        sph.set_light_vec(light_vec(
            t_pin(*self.light_height.borrow() * 0.01, -1.0, 1.0),
            float_degrees_to_radians(*self.light_direction.borrow() - 90.0),
        ));
        let lc = self.light_color.borrow().to_color4f();
        sph.set_light_color(V3::new(lc.r, lc.g, lc.b));
        sph.set_specular_exp(1.0 / t_pin(*self.roughness.borrow(), 0.001, 0.5));
    }
}

impl_container_animator!(SphereAdapter);

/// The CC Sphere effect (`CC Sphere`).
// Port of: modules/skottie/src/effects/SphereEffect.cpp#L347-L352 (chrome/m156) (`EffectBuilder::attachSphereEffect`)
pub(super) fn attach_sphere_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let sphere = SphereNode::make(layer, eb.layer_size());
    let adapter = SphereAdapter::make(jprops, eb.builder(), Rc::clone(&sphere));
    Some(attach_adapter_node(eb.builder(), &adapter, sphere))
}

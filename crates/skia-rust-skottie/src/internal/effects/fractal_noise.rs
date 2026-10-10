// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/FractalNoiseEffect.cpp (chrome/m156)
//
// The ADBE Fractal Noise effect: a custom render node that fills its layer with a fractal noise
// shader. The noise is an SkSL runtime shader specialized by the filter, the fractal type and the
// octave loop count.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::floating_point::{FLOAT_PI, float_degrees_to_radians};
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeEffectBuilder};
use skia_rust_core::scalar::{scalar_floor_to_scalar, scalar_round_to_int, scalar_round_to_scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::render_node::Hit;
use skia_rust_sksg::{Node, NodeCore, RenderContext, RenderNode, ScopedRenderContext};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ScalarValue, Vec2Value};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// The noise SkSL template: the sublayer loop over the octaves, with the filter and fractal
/// functions spliced in (the `%s` and `%u` placeholders of `gNoiseEffectSkSL`).
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L39-L105 (chrome/m156) (`gNoiseEffectSkSL`)
const NOISE_EFFECT_SKSL: &str = concat!(
    "uniform float3x3 u_submatrix;",
    "uniform float2 u_noise_planes;",
    "uniform float u_noise_weight,",
    "u_octaves,",
    "u_persistence;",
    "float hash(float3 v) {",
    "v = fract(v*0.1031);",
    "v += dot(v, v.zxy + 31.32);",
    "return fract((v.x + v.y)*v.z);",
    "}",
    "float sample_noise(float2 xy) {",
    "xy = floor(xy);",
    "float n0 = hash(float3(xy, u_noise_planes.x)),",
    "n1 = hash(float3(xy, u_noise_planes.y));",
    "return mix(n0, n1, u_noise_weight);",
    "}",
    "%s",
    "%s",
    "float4 main(vec2 xy) {",
    "float oct = u_octaves,",
    "amp = 1,",
    "wacc = 0,",
    "n = 0;",
    "for (float i = 0; i < %u; ++i) {",
    "float w = amp*min(oct,1.0);",
    "n += w*fractal(filter(xy));",
    "wacc += w;",
    "if (oct <= 1.0) { break; }",
    "oct -= 1.0;",
    "amp *= u_persistence;",
    "xy = (u_submatrix*float3(xy,1)).xy;",
    "}",
    "n /= wacc;",
    "return float4(n,n,n,1);",
    "}",
);

/// The nearest filter of the noise samples.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L107-L111 (chrome/m156) (`gFilterNearestSkSL`)
const FILTER_NEAREST_SKSL: &str = "float filter(float2 xy) { return sample_noise(xy); }";

/// The bilinear filter of the noise samples.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L113-L124 (chrome/m156) (`gFilterLinearSkSL`)
const FILTER_LINEAR_SKSL: &str = concat!(
    "float filter(float2 xy) {",
    "xy -= 0.5;",
    "float n00 = sample_noise(xy + float2(0,0)),",
    "n10 = sample_noise(xy + float2(1,0)),",
    "n01 = sample_noise(xy + float2(0,1)),",
    "n11 = sample_noise(xy + float2(1,1));",
    "float2 t = fract(xy);",
    "return mix(mix(n00, n10, t.x), mix(n01, n11, t.x), t.y);",
    "}",
);

/// The soft linear filter of the noise samples.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L126-L137 (chrome/m156) (`gFilterSoftLinearSkSL`)
const FILTER_SOFT_LINEAR_SKSL: &str = concat!(
    "float filter(float2 xy) {",
    "xy -= 0.5;",
    "float n00 = sample_noise(xy + float2(0,0)),",
    "n10 = sample_noise(xy + float2(1,0)),",
    "n01 = sample_noise(xy + float2(0,1)),",
    "n11 = sample_noise(xy + float2(1,1));",
    "float2 t = smoothstep(0, 1, fract(xy));",
    "return mix(mix(n00, n10, t.x), mix(n01, n11, t.x), t.y);",
    "}",
);

/// The basic fractal: the noise itself.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L139-L143 (chrome/m156) (`gFractalBasicSkSL`)
const FRACTAL_BASIC_SKSL: &str = "float fractal(float n) { return n; }";

/// The turbulent basic fractal.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L145-L149 (chrome/m156) (`gFractalTurbulentBasicSkSL`)
const FRACTAL_TURBULENT_BASIC_SKSL: &str = "float fractal(float n) { return 2*abs(0.5 - n); }";

/// The turbulent smooth fractal.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L151-L156 (chrome/m156) (`gFractalTurbulentSmoothSkSL`)
const FRACTAL_TURBULENT_SMOOTH_SKSL: &str =
    "float fractal(float n) { n = 2*abs(0.5 - n); return n*n; }";

/// The turbulent sharp fractal.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L158-L162 (chrome/m156) (`gFractalTurbulentSharpSkSL`)
const FRACTAL_TURBULENT_SHARP_SKSL: &str = "float fractal(float n) { return sqrt(2*abs(0.5 - n)); }";

/// The sample filter of the noise (`NoiseFilter`).
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L164-L169 (chrome/m156) (`NoiseFilter`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum NoiseFilter {
    Nearest,
    Linear,
    SoftLinear,
}

/// The fractal transform of the noise (`NoiseFractal`).
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L171-L177 (chrome/m156) (`NoiseFractal`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum NoiseFractal {
    Basic,
    TurbulentBasic,
    TurbulentSmooth,
    TurbulentSharp,
}

/// The loop count of the octaves: the bin of `octaves` (`kLoopBins`) and its loop count.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L213-L221 (chrome/m156) (`kLoopBins`)
const LOOP_BINS: [(f32, u32); 6] = [(8.0, 20), (4.0, 8), (3.0, 4), (2.0, 3), (1.0, 2), (0.0, 1)];

/// The noise effect for the given octaves, filter and fractal (`noise_effect`). Effects are
/// cached by their loop bin, filter and fractal.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L223-L256 (chrome/m156) (`noise_effect`)
fn noise_effect(octaves: f32, filter: NoiseFilter, fractal: NoiseFractal) -> Option<RuntimeEffect> {
    thread_local! {
        static EFFECT_CACHE: RefCell<HashMap<(usize, NoiseFilter, NoiseFractal), Option<RuntimeEffect>>> =
            RefCell::new(HashMap::new());
    }

    // Bin the loop counter based on the number of octaves (range: [1..20]).
    let bin = LOOP_BINS
        .iter()
        .position(|&(threshold, _)| octaves > threshold)
        .unwrap_or(LOOP_BINS.len() - 1);

    EFFECT_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((bin, filter, fractal))
            .or_insert_with(|| {
                let filter_sksl = match filter {
                    NoiseFilter::Nearest => FILTER_NEAREST_SKSL,
                    NoiseFilter::Linear => FILTER_LINEAR_SKSL,
                    NoiseFilter::SoftLinear => FILTER_SOFT_LINEAR_SKSL,
                };
                let fractal_sksg = match fractal {
                    NoiseFractal::Basic => FRACTAL_BASIC_SKSL,
                    NoiseFractal::TurbulentBasic => FRACTAL_TURBULENT_BASIC_SKSL,
                    NoiseFractal::TurbulentSmooth => FRACTAL_TURBULENT_SMOOTH_SKSL,
                    NoiseFractal::TurbulentSharp => FRACTAL_TURBULENT_SHARP_SKSL,
                };
                let loops = LOOP_BINS[bin].1;
                let sksl = NOISE_EFFECT_SKSL
                    .replacen("%s", filter_sksl, 1)
                    .replacen("%s", fractal_sksg, 1)
                    .replacen("%u", &loops.to_string(), 1);
                RuntimeEffect::make_for_shader(sksl, None).ok()
            })
            .clone()
    })
}

/// The custom render node of the noise: it fills its child with the noise shader.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L258-L266 (chrome/m156) (`FractalNoiseNode`)
#[derive(Debug)]
pub(super) struct FractalNoiseNode {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    // Cached top-level shader.
    effect_shader: RefCell<Option<Shader>>,
    matrix: RefCell<Matrix>,
    sub_matrix: RefCell<Matrix>,
    filter: Cell<NoiseFilter>,
    fractal: Cell<NoiseFractal>,
    noise_planes: Cell<(f32, f32)>,
    noise_weight: Cell<f32>,
    octaves: Cell<f32>,
    persistence: Cell<f32>,
}

impl FractalNoiseNode {
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L268-L270 (chrome/m156) (`FractalNoiseNode::FractalNoiseNode`)
    fn make(child: Rc<dyn RenderNode>) -> Rc<Self> {
        let node = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            effect_shader: RefCell::new(None),
            matrix: RefCell::new(Matrix::new_identity()),
            sub_matrix: RefCell::new(Matrix::new_identity()),
            filter: Cell::new(NoiseFilter::Nearest),
            fractal: Cell::new(NoiseFractal::Basic),
            noise_planes: Cell::new((0.0, 0.0)),
            noise_weight: Cell::new(0.0),
            octaves: Cell::new(1.0),
            persistence: Cell::new(1.0),
        });
        // The custom node observes its child.
        node.observe_inval(node.child.as_ref());
        node
    }

    /// Sets the local matrix, invalidating the node if it changed (`setMatrix`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L271-L280 (chrome/m156) (`SG_ATTRIBUTE(Matrix)`)
    pub(super) fn set_matrix(&self, matrix: Matrix) {
        if *self.matrix.borrow() != matrix {
            *self.matrix.borrow_mut() = matrix;
            self.invalidate();
        }
    }

    /// Sets the sublayer matrix, invalidating the node if it changed (`setSubMatrix`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L271-L280 (chrome/m156) (`SG_ATTRIBUTE(SubMatrix)`)
    pub(super) fn set_sub_matrix(&self, matrix: Matrix) {
        if *self.sub_matrix.borrow() != matrix {
            *self.sub_matrix.borrow_mut() = matrix;
            self.invalidate();
        }
    }

    /// Sets the sample filter, invalidating the node if it changed (`setNoiseFilter`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L281-L290 (chrome/m156) (`SG_ATTRIBUTE(NoiseFilter)`)
    pub(super) fn set_noise_filter(&self, filter: NoiseFilter) {
        if self.filter.get() != filter {
            self.filter.set(filter);
            self.invalidate();
        }
    }

    /// Sets the fractal type, invalidating the node if it changed (`setNoiseFractal`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L281-L290 (chrome/m156) (`SG_ATTRIBUTE(NoiseFractal)`)
    pub(super) fn set_noise_fractal(&self, fractal: NoiseFractal) {
        if self.fractal.get() != fractal {
            self.fractal.set(fractal);
            self.invalidate();
        }
    }

    /// Sets the noise planes, invalidating the node if they changed (`setNoisePlanes`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L291-L300 (chrome/m156) (`SG_ATTRIBUTE(NoisePlanes)`)
    pub(super) fn set_noise_planes(&self, planes: (f32, f32)) {
        if self.noise_planes.get() != planes {
            self.noise_planes.set(planes);
            self.invalidate();
        }
    }

    /// Sets the noise weight, invalidating the node if it changed (`setNoiseWeight`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L291-L300 (chrome/m156) (`SG_ATTRIBUTE(NoiseWeight)`)
    pub(super) fn set_noise_weight(&self, weight: f32) {
        if self.noise_weight.get() != weight {
            self.noise_weight.set(weight);
            self.invalidate();
        }
    }

    /// Sets the octaves, invalidating the node if they changed (`setOctaves`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L291-L300 (chrome/m156) (`SG_ATTRIBUTE(Octaves)`)
    pub(super) fn set_octaves(&self, octaves: f32) {
        if self.octaves.get() != octaves {
            self.octaves.set(octaves);
            self.invalidate();
        }
    }

    /// Sets the persistence, invalidating the node if it changed (`setPersistence`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L291-L300 (chrome/m156) (`SG_ATTRIBUTE(Persistence)`)
    pub(super) fn set_persistence(&self, persistence: f32) {
        if self.persistence.get() != persistence {
            self.persistence.set(persistence);
            self.invalidate();
        }
    }

    /// The noise shader (`buildEffectShader`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L302-L320 (chrome/m156) (`FractalNoiseNode::buildEffectShader`)
    fn build_effect_shader(&self) -> Option<Shader> {
        let effect = noise_effect(
            self.octaves.get(),
            self.filter.get(),
            self.fractal.get(),
        )?;
        let mut builder = RuntimeEffectBuilder::new(effect);
        let (planes_x, planes_y) = self.noise_planes.get();
        builder.uniform("u_noise_planes").set_f32(&[planes_x, planes_y]);
        builder.uniform("u_noise_weight").set_f32(&[self.noise_weight.get()]);
        builder.uniform("u_octaves").set_f32(&[self.octaves.get()]);
        builder.uniform("u_persistence").set_f32(&[self.persistence.get()]);
        let sub = self.sub_matrix.borrow();
        builder.uniform("u_submatrix").set_f32(&[
            sub.rc(0, 0),
            sub.rc(1, 0),
            sub.rc(2, 0),
            sub.rc(0, 1),
            sub.rc(1, 1),
            sub.rc(2, 1),
            sub.rc(0, 2),
            sub.rc(1, 2),
            sub.rc(2, 2),
        ]);
        let matrix = self.matrix.borrow();
        builder.make_shader(&*matrix)
    }
}

impl Drop for FractalNoiseNode {
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L258-L262 (chrome/m156) (`CustomRenderNode::~CustomRenderNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for FractalNoiseNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L322-L327 (chrome/m156) (`FractalNoiseNode::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let bounds = self.child.revalidate(ic, ctm);
        *self.effect_shader.borrow_mut() = self.build_effect_shader();
        bounds
    }
}

impl RenderNode for FractalNoiseNode {
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L329-L340 (chrome/m156) (`FractalNoiseNode::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let bounds = self.core().bounds();
        let scope = ScopedRenderContext::new(canvas, ctx).set_isolation(
            &bounds,
            &canvas.total_matrix(),
            true,
        );
        canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
        self.child.render(canvas, Some(scope.context()));

        let mut effect_paint = Paint::default();
        effect_paint.set_shader(self.effect_shader.borrow().clone());
        effect_paint.set_blend_mode(BlendMode::SrcIn);
        canvas.draw_paint(&effect_paint);
    }

    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L342 (chrome/m156) (`FractalNoiseNode::onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<Hit> {
        // no hit-testing
        None
    }
}

/// The fractal noise adapter: the noise, transform and evolution properties.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L344-L440 (chrome/m156) (`FractalNoiseAdapter`)
struct FractalNoiseAdapter {
    base: DiscardableAdapterBase<FractalNoiseNode>,
    offset: Prop<Vec2Value>,
    sub_offset: Prop<Vec2Value>,
    fractal_type: Prop<ScalarValue>,
    noise_type: Prop<ScalarValue>,
    rotation: Prop<ScalarValue>,
    uniform_scaling: Prop<ScalarValue>,
    scale: Prop<ScalarValue>,
    scale_width: Prop<ScalarValue>,
    scale_height: Prop<ScalarValue>,
    complexity: Prop<ScalarValue>,
    sub_influence: Prop<ScalarValue>,
    sub_scale: Prop<ScalarValue>,
    sub_rotation: Prop<ScalarValue>,
    evolution: Prop<ScalarValue>,
    cycle_evolution: Prop<ScalarValue>,
    cycle_revolutions: Prop<ScalarValue>,
    random_seed: Prop<ScalarValue>,
}

impl FractalNoiseAdapter {
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L350-L392 (chrome/m156) (`FractalNoiseAdapter::FractalNoiseAdapter`)
    fn make(
        jprops: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        node: Rc<FractalNoiseNode>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let fractal_type = Prop::new(0.0);
            let noise_type = Prop::new(0.0);
            // TODO in Skia: invert, contrast and brightness are bound but not applied.
            let invert = Prop::new(0.0);
            let contrast = Prop::new(100.0);
            let brightness = Prop::new(0.0);
            let rotation = Prop::new(0.0);
            let uniform_scaling = Prop::new(0.0);
            let scale = Prop::new(100.0);
            let scale_width = Prop::new(100.0);
            let scale_height = Prop::new(100.0);
            let offset = Prop::new(Vec2Value::new(0.0, 0.0));
            let complexity = Prop::new(1.0);
            let sub_influence = Prop::new(100.0);
            let sub_scale = Prop::new(50.0);
            let sub_rotation = Prop::new(0.0);
            let sub_offset = Prop::new(Vec2Value::new(0.0, 0.0));
            let evolution = Prop::new(0.0);
            let cycle_evolution = Prop::new(0.0);
            let cycle_revolutions = Prop::new(0.0);
            let random_seed = Prop::new(0.0);
            // TODO in Skia: opacity is bound but not applied.
            let opacity = Prop::new(100.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &fractal_type)
                .bind(1, &noise_type)
                .bind(2, &invert)
                .bind(3, &contrast)
                .bind(4, &brightness)
                // 5 -- overflow
                // 6 -- transform begin-group
                .bind(7, &rotation)
                .bind(8, &uniform_scaling)
                .bind(9, &scale)
                .bind(10, &scale_width)
                .bind(11, &scale_height)
                .bind(12, &offset)
                // 13 -- TODO: perspective offset
                // 14 -- transform end-group
                .bind(15, &complexity)
                // 16 -- sub settings begin-group
                .bind(17, &sub_influence)
                .bind(18, &sub_scale)
                .bind(19, &sub_rotation)
                .bind(20, &sub_offset)
                // 21 -- center subscale
                // 22 -- sub settings end-group
                .bind(23, &evolution)
                // 24 -- evolution options begin-group
                .bind(25, &cycle_evolution)
                .bind(26, &cycle_revolutions)
                .bind(27, &random_seed)
                // 28 -- evolution options end-group
                .bind(29, &opacity);
            // 30 -- TODO: blending mode
            Self {
                base,
                offset,
                sub_offset,
                fractal_type,
                noise_type,
                rotation,
                uniform_scaling,
                scale,
                scale_width,
                scale_height,
                complexity,
                sub_influence,
                sub_scale,
                sub_rotation,
                evolution,
                cycle_evolution,
                cycle_revolutions,
                random_seed,
            }
        })
    }

    /// The noise planes and the plane weight of the evolution (`noise`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L357-L386 (chrome/m156) (`FractalNoiseAdapter::noise`)
    fn noise(&self) -> ((f32, f32), f32) {
        // Constant chosen to visually match AE's evolution rate.
        const EVOLUTION_SCALE: f32 = 0.25;
        let cycle_evolution = *self.cycle_evolution.borrow() != 0.0;
        let rev_rad = std_max(*self.cycle_revolutions.borrow(), 1.0) * FLOAT_PI * 2.0;
        let cycle = if cycle_evolution {
            scalar_round_to_scalar(rev_rad * EVOLUTION_SCALE)
        } else {
            f32::MAX
        };
        // Adjust scale when cycling to ensure an integral period (post scaling).
        let scale = if cycle_evolution {
            cycle / rev_rad
        } else {
            EVOLUTION_SCALE
        };
        let evo_rad = float_degrees_to_radians(*self.evolution.borrow());
        // SkRandom(seed).nextRangeU(0, 100), as a float.
        let offset = Random::new(*self.random_seed.borrow() as u32).next_range_u(0, 100) as f32;
        let evo = evo_rad * scale;
        let evo_ = scalar_floor_to_scalar(evo);
        let weight = evo - evo_;

        let planes = (
            glsl_mod(evo_ + 0.0, cycle) + offset,
            glsl_mod(evo_ + 1.0, cycle) + offset,
        );
        (planes, weight)
    }

    /// The transform of the noise (`shaderMatrix`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L388-L399 (chrome/m156) (`FractalNoiseAdapter::shaderMatrix`)
    fn shader_matrix(&self) -> Matrix {
        const GRID_SIZE: f32 = 64.0;
        let (sx, sy) = if scalar_round_to_int(*self.uniform_scaling.borrow()) == 1 {
            (*self.scale.borrow(), *self.scale.borrow())
        } else {
            (*self.scale_width.borrow(), *self.scale_height.borrow())
        };
        let offset = *self.offset.borrow();
        Matrix::translate(Point::new(offset.x, offset.y))
            * Matrix::scale((
                t_pin(sx, 1.0, 10000.0) * 0.01,
                t_pin(sy, 1.0, 10000.0) * 0.01,
            ))
            * Matrix::rotate_deg(*self.rotation.borrow())
            * Matrix::scale((GRID_SIZE, GRID_SIZE))
    }

    /// The transform of the sublayers (`subMatrix`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L401-L406 (chrome/m156) (`FractalNoiseAdapter::subMatrix`)
    fn sub_matrix(&self) -> Matrix {
        let scale = 100.0 / t_pin(*self.sub_scale.borrow(), 10.0, 10000.0);
        let sub_offset = *self.sub_offset.borrow();
        Matrix::translate(Point::new(
            -sub_offset.x * 0.01,
            -sub_offset.y * 0.01,
        )) * Matrix::rotate_deg(-*self.sub_rotation.borrow())
            * Matrix::scale((scale, scale))
    }

    /// The sample filter of the noise type (`noiseFilter`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L408-L414 (chrome/m156) (`FractalNoiseAdapter::noiseFilter`)
    fn noise_filter(&self) -> NoiseFilter {
        match scalar_round_to_int(*self.noise_type.borrow()) {
            1 => NoiseFilter::Nearest,
            2 => NoiseFilter::Linear,
            _ => NoiseFilter::SoftLinear,
        }
    }

    /// The fractal type of the noise (`noiseFractal`).
    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L416-L423 (chrome/m156) (`FractalNoiseAdapter::noiseFractal`)
    fn noise_fractal(&self) -> NoiseFractal {
        match scalar_round_to_int(*self.fractal_type.borrow()) {
            1 => NoiseFractal::Basic,
            3 => NoiseFractal::TurbulentSmooth,
            4 => NoiseFractal::TurbulentBasic,
            _ => NoiseFractal::TurbulentSharp,
        }
    }
}

/// The GLSL `mod`: `x - y*floor(x/y)`.
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L370-L372 (chrome/m156) (`glsl_mod`)
fn glsl_mod(x: f32, y: f32) -> f32 {
    x - y * scalar_floor_to_scalar(x / y)
}

/// `std::max(a, b)`: `b` only if `a < b`.
// Port of: <algorithm> std::max (chrome/m156)
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

impl AnimatablePropertyContainer for FractalNoiseAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L425-L441 (chrome/m156) (`FractalNoiseAdapter::onSync`)
    fn on_sync(&self) {
        let node = self.base.node();
        let (noise_planes, noise_weight) = self.noise();
        node.set_octaves(t_pin(*self.complexity.borrow(), 1.0, 20.0));
        node.set_persistence(t_pin(*self.sub_influence.borrow() * 0.01, 0.0, 100.0));
        node.set_noise_planes(noise_planes);
        node.set_noise_weight(noise_weight);
        node.set_noise_filter(self.noise_filter());
        node.set_noise_fractal(self.noise_fractal());
        node.set_matrix(self.shader_matrix());
        node.set_sub_matrix(self.sub_matrix());
    }
}

impl_container_animator!(FractalNoiseAdapter);

/// The fractal noise effect (`ADBE Fractal Noise`).
// Port of: modules/skottie/src/effects/FractalNoiseEffect.cpp#L443-L449 (chrome/m156) (`EffectBuilder::attachFractalNoiseEffect`)
pub(super) fn attach_fractal_noise_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let node = FractalNoiseNode::make(layer);
    let adapter = FractalNoiseAdapter::make(jprops, eb.builder(), Rc::clone(&node));
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

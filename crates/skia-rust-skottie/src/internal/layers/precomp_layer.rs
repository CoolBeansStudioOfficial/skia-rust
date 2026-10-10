// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/PrecompLayer.cpp (chrome/m156)

use std::cell::Cell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::floating_point::ieee_float_divide;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::Scalar;
use skia_rust_core::size::Size;
use skia_rust_sksg::util::scalar_changed;
use skia_rust_sksg::{
    InvalidationController, Node, NodeCore, RenderContext, RenderNode, ScopedRenderContext,
};
use skia_rust_sksg::render_node::Hit;

use crate::external_layer::ExternalLayer;
use crate::impl_container_animator;
use crate::internal::animator::{
    AnimatablePropertyContainer, Animator, Prop, PropertyContainer, StateChanged,
};
use crate::internal::composition::CompositionBuilder;
use crate::internal::skottie_priv::{
    AnimationBuilder, AnimatorScope, AutoPropertyTracker, AutoScope, ScopedAssetRef,
};
use crate::json::ObjectValue;
use crate::skottie::LayerInfo;
use crate::skottie_json::{ValueExt, parse_default, string_text};
use crate::skottie_property::NodeType;

/// "Animates" time based on the layer's "tm" property.
// Port of: modules/skottie/src/layers/PrecompLayer.cpp#L39-L58 (chrome/m156) (`class TimeRemapper`)
struct TimeRemapper {
    container: PropertyContainer,
    scale: f32,
    t: Prop<f32>,
}

impl TimeRemapper {
    fn new(jtm: &ObjectValue, abuilder: &AnimationBuilder<'_>, scale: f32) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let container = PropertyContainer::new(weak.clone());
            let t = Prop::new(0.0);
            container.bind(abuilder, jtm, &t);
            Self { container, scale, t }
        })
    }

    fn t(&self) -> f32 {
        self.t.get() * self.scale
    }
}

impl AnimatablePropertyContainer for TimeRemapper {
    fn container(&self) -> &PropertyContainer {
        &self.container
    }

    fn on_sync(&self) {
        // nothing to sync - we just track t
    }
}

impl_container_animator!(TimeRemapper);

/// Applies a bias/scale/remap t-adjustment to child animators.
// Port of: modules/skottie/src/layers/PrecompLayer.cpp#L60-L96 (chrome/m156) (`class CompTimeMapper`)
struct CompTimeMapper {
    animators: AnimatorScope,
    remapper: Option<Rc<TimeRemapper>>,
    time_bias: f32,
    time_scale: f32,
}

impl Animator for CompTimeMapper {
    fn seek(&self, t: f32) -> StateChanged {
        let t = if let Some(remapper) = &self.remapper {
            // When time remapping is active, |t| is fully driven externally.
            remapper.seek(t);
            remapper.t()
        } else {
            (t + self.time_bias) * self.time_scale
        };

        let mut changed = false;

        for anim in &self.animators {
            changed |= anim.seek(t);
        }

        changed
    }
}

/// Attaches an `ExternalLayer` implementation to the animation scene graph.
// Port of: modules/skottie/src/layers/PrecompLayer.cpp#L121-L158 (chrome/m156) (`class SGAdapter`)
struct SgAdapter {
    core: NodeCore,
    external: Rc<dyn ExternalLayer>,
    size: Size,
    current_t: Cell<f32>,
}

impl std::fmt::Debug for SgAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SgAdapter").field("size", &self.size).finish_non_exhaustive()
    }
}

impl SgAdapter {
    fn new(external: Rc<dyn ExternalLayer>, layer_size: Size) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            external,
            size: layer_size,
            current_t: Cell::new(0.0),
        })
    }

    /// `SG_ATTRIBUTE(T, float, fCurrentT)`.
    fn set_t(&self, t: f32) {
        if scalar_changed(self.current_t.get(), t) {
            self.current_t.set(t);
            self.invalidate();
        }
    }
}

impl Node for SgAdapter {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        Rect::from_wh(self.size.width, self.size.height)
    }
}

impl RenderNode for SgAdapter {
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        // Commit all pending effects via a layer if needed,
        // since we don't have knowledge of the external content.
        let _local_scope = ScopedRenderContext::new(canvas, ctx).set_isolation(
            &self.core.bounds(),
            &canvas.total_matrix(),
            true,
        );
        self.external.render(canvas, f64::from(self.current_t.get()));
    }

    fn on_node_at(&self, p: Point) -> Option<Hit> {
        debug_assert!(skia_rust_sksg::util::rect_contains(&self.core.bounds(), p));
        Some(Hit::This)
    }
}

/// Connects an `SgAdapter` to the animator tree and dispatches seek events.
// Port of: modules/skottie/src/layers/PrecompLayer.cpp#L160-L173 (chrome/m156) (`class AnimatorAdapter`)
struct AnimatorAdapter {
    sg_adapter: Rc<SgAdapter>,
    fps: f32,
}

impl Animator for AnimatorAdapter {
    fn seek(&self, t: f32) -> StateChanged {
        self.sg_adapter.set_t(t / self.fps);

        true
    }
}

impl<'j> AnimationBuilder<'j> {
    /// Attaches the content an `PrecompInterceptor` provides for the layer, if any.
    // Port of: modules/skottie/src/layers/PrecompLayer.cpp#L100-L178 (chrome/m156) (`attachExternalPrecompLayer`)
    fn attach_external_precomp_layer(
        &self,
        jlayer: &ObjectValue,
        layer_info: &LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        let precomp_interceptor = self.precomp_interceptor()?;

        let id = jlayer.get("refId").as_string();
        let nm = jlayer.get("nm").as_string();

        let (Some(id), Some(nm)) = (id, nm) else {
            return None;
        };

        let external_layer =
            precomp_interceptor.on_load_precomp(&string_text(id), &string_text(nm), layer_info.size)?;

        let sg_adapter = SgAdapter::new(external_layer, layer_info.size);

        self.push_animator(Rc::new(AnimatorAdapter {
            sg_adapter: Rc::clone(&sg_adapter),
            fps: self.frame_rate(),
        }));

        Some(sg_adapter as Rc<dyn RenderNode>)
    }

    /// Attaches a precomp layer: the composition of its asset, with a time mapping if needed.
    // Port of: modules/skottie/src/layers/PrecompLayer.cpp#L180-L234 (chrome/m156) (`attachPrecompLayer`)
    #[doc(alias = "attachPrecompLayer")]
    #[must_use]
    pub fn attach_precomp_layer(
        &self,
        jlayer: &ObjectValue,
        layer_info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        let time_remapper = jlayer
            .get("tm")
            .as_object()
            .map(|jtm| TimeRemapper::new(jtm, self, self.frame_rate()));

        let start_time = parse_default::<f32>(jlayer.get("st"), 0.0);
        let stretch_time = parse_default::<f32>(jlayer.get("sr"), 1.0);
        let requires_time_mapping = !f32::nearly_equal(start_time, 0.0, None)
            || !f32::nearly_equal(stretch_time, 1.0, None)
            || time_remapper.is_some();

        // Precomp layers are sized explicitly.
        let parse_size = |jlayer: &ObjectValue| {
            Size::new(
                parse_default::<f32>(jlayer.get("w"), 0.0),
                parse_default::<f32>(jlayer.get("h"), 0.0),
            )
        };
        layer_info.size = parse_size(jlayer);

        let mut local_scope = None;
        if requires_time_mapping {
            local_scope = Some(AutoScope::new(self));
        }

        let mut precomp_layer = self.attach_external_precomp_layer(jlayer, layer_info);

        if precomp_layer.is_none() {
            let precomp_asset = ScopedAssetRef::new(self, jlayer);
            if let Some(asset) = precomp_asset.asset() {
                // Unlike regular precomp layers, glyph precomps don't have an explicit size - they
                // use the actual asset comp size.
                if layer_info.size.is_empty() {
                    layer_info.size = parse_size(asset);
                }

                let _apt = AutoPropertyTracker::new(self, asset, NodeType::Composition);
                precomp_layer = CompositionBuilder::new(self, layer_info.size, asset).build(self);
            }
        }

        if let Some(local_scope) = local_scope {
            let t_bias = -start_time;
            let t_scale = ieee_float_divide(1.0, stretch_time);
            let time_mapper = CompTimeMapper {
                animators: local_scope.release(),
                remapper: time_remapper,
                time_bias: t_bias,
                time_scale: if t_scale.is_finite() { t_scale } else { 0.0 },
            };

            self.push_animator(Rc::new(time_mapper));
        }

        precomp_layer
    }
}

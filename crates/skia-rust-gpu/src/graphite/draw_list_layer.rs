// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawListLayer.h, src/gpu/graphite/DrawListLayer.cpp

//! [`DrawListLayer`]: the layer-based draw list. Draws are placed in layers of binding lists as
//! they are recorded, so snapping a pass only walks the layers in order.
//!
//! Skia's arena (`fStorage`) and intrusive lists are index-addressed `Vec`s here (see
//! [`crate::graphite::draw_list_types`]), and the `DrawParams*`/`Layer*` that `recordDraw()`
//! returns are [`DrawParamsId`] and [`LayerId`].

use std::rc::Rc;
use std::sync::Arc;

use skia_rust_core::color::Color4f;
use skia_rust_core::rect::{Contains, IRect};

use crate::gpu::sk_log::skia_log_w;
use crate::graphite::draw_list_base::{
    DrawListBaseState, RecordDrawArgs, SharedCommandList, SnapArgs, TextureTracker, UniformTracker,
    make_draw_writer, render_state_flags,
};
use crate::graphite::draw_list_types::{
    BindingArena, BindingId, BoundsFlags, BoundsTestResult, DrawParamsId, Layer, LayerId, LayerKey,
};
use crate::graphite::draw_order::{CompressedPaintersOrder, DrawOrder};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_pass::DrawPass;
use crate::graphite::draw_types::{BarrierType, DstUsage, UniformSlot};
use crate::graphite::geom::rect::ComplementRect;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::graphite_types::DepthStencilFlags;
use crate::graphite::pipeline_data::{K_INVALID_INDEX, PipelineDataGatherer};
use crate::graphite::render_step::RenderStep;
use crate::graphite::resource_types::{DstReadStrategy, LoadOp, StoreOp};
use crate::graphite::storage_context::StorageContext;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

/// Whether `flags` includes stencil (`flags & DepthStencilFlags::kStencil`).
fn has_stencil(flags: DepthStencilFlags) -> bool {
    matches!(
        flags,
        DepthStencilFlags::Stencil | DepthStencilFlags::DepthStencil
    )
}

/// `DrawListLayer`.
// Port of: src/gpu/graphite/DrawListLayer.h#L28-L81 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawListLayer")]
#[derive(Debug)]
pub struct DrawListLayer {
    state: DrawListBaseState,

    // The `DrawParams` of every recorded draw (allocated in `fStorage` in C++).
    draw_params: Vec<DrawParams>,
    arena: BindingArena,
    // `fLayers`: appended to the tail only, so the layer before layer `i` is layer `i - 1`.
    layers: Vec<Layer>,

    draw_count: usize,
    order_counter: CompressedPaintersOrder,

    storage_buffer_support: bool,
}

impl DrawListLayer {
    /// `DrawListLayer(storageBufferSupport)`.
    // Port of: src/gpu/graphite/DrawListLayer.h#L31-L33 (chrome/m156)
    #[must_use]
    pub fn new(storage_buffer_support: bool) -> Self {
        Self {
            state: DrawListBaseState::default(),
            draw_params: Vec::new(),
            arena: BindingArena::default(),
            layers: Vec::new(),
            draw_count: 0,
            order_counter: CompressedPaintersOrder::first(),
            storage_buffer_support,
        }
    }

    pub(crate) fn state(&self) -> &DrawListBaseState {
        &self.state
    }

    /// The `DrawParams` of a recorded draw.
    #[must_use]
    pub fn draw_params(&self, id: DrawParamsId) -> &DrawParams {
        &self.draw_params[id.0 as usize]
    }

    /// The paint order of the layer `id`.
    #[must_use]
    pub fn layer_order(&self, id: LayerId) -> CompressedPaintersOrder {
        self.layers[id.0 as usize].order
    }

    /// The number of layers.
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// The number of draws in each binding list of each layer, in order (for tests).
    #[must_use]
    pub fn binding_draw_counts(&self) -> Vec<Vec<usize>> {
        self.layers
            .iter()
            .map(|layer| {
                layer
                    .bindings
                    .ids(&self.arena)
                    .into_iter()
                    .map(|id| {
                        let mut count = 0;
                        let mut draw = self.arena.bindings[id as usize].head;
                        while let Some(d) = draw {
                            count += 1;
                            draw = self.arena.draws[d as usize].next;
                        }
                        count
                    })
                    .collect()
            })
            .collect()
    }

    /// `reset(loadOp, color)`.
    // Port of: src/gpu/graphite/DrawListLayer.cpp#L27-L34 (chrome/m156)
    pub fn reset(&mut self, load_op: LoadOp, color: Color4f) {
        self.state.reset(load_op, color);

        self.draw_params.clear();
        self.arena.reset();
        self.layers.clear();
        self.draw_count = 0;
        self.order_counter = CompressedPaintersOrder::first();
    }

    /// `searchBackwards(step, key, testMask, drawParams, stop)`.
    // Port of: src/gpu/graphite/DrawListLayer.cpp#L36-L124 (chrome/m156)
    #[allow(clippy::single_match_else)] // mirrors the C++ if/else
    #[allow(clippy::similar_names)] // mirrors the C++ names
    fn search_backwards(
        &mut self,
        step: &Arc<dyn RenderStep>,
        key: &LayerKey,
        test_mask: BoundsFlags,
        draw_params: DrawParamsId,
        stop: CompressedPaintersOrder,
    ) -> (usize, BindingId) {
        // CPU performance is sensitive to increasing this value. Searching for longer *can*
        // reduce the draw count and pipeline change count
        const MAX_SEARCH_LIMIT: i32 = 8;

        let draw_bounds =
            ComplementRect::new(self.draw_params[draw_params.0 as usize].draw_bounds());

        let mut target_layer: Option<usize> = None;
        let mut current = self.layers.len().checked_sub(1);
        let mut limit = MAX_SEARCH_LIMIT;
        while limit > 0 {
            let Some(current_index) = current else { break };
            let layer = &self.layers[current_index];
            // NOTE: This test does not search within the layer's binding lists for a match.
            // Searching each layer that can allow the draw adds overhead for minimal batching
            // improvements. Instead the heuristic to just add to the deepest layer possible and
            // search only that layer for a good binding match batches about as well in the
            // limit.
            let result = layer.test(draw_bounds, test_mask);

            if result.contains(BoundsTestResult::ALLOWED_IN_LAYER) {
                // Allowed in the layer, so remember it. In complex scenes, we want to search
                // deeper in the layer list than just the first compatible overlap we encounter.
                // Stopping early reduces search time but fragments batching. Inserting early
                // blocks subsequent draws from reaching those denser, later candidates
                // (particularly when this is a clip draw as that propagates into the stop layer
                // for subsequent draws).
                target_layer = Some(current_index);
            }

            if !result.contains(BoundsTestResult::ALLOWED_BEFORE_LAYER) || layer.order == stop {
                break;
            }
            current = current_index.checked_sub(1);
            limit -= 1;
        }

        debug_assert!(target_layer.is_none_or(|t| self.layers[t].order >= stop));

        let mut target_match: Option<BindingId> = None;
        let mut forward_merge: Option<BindingId> = None;
        if let Some(target) = target_layer {
            // `targetLayer` is non-null only if the test returned kAllowedInLayer, which means
            // it is disjoint from everything else in the layer. We can safely combine it with
            // an exact match or place it near a partial match.
            target_match = self.layers[target].search_binding(&self.arena, key, None);
            // if `targetMatch` is null, we could try and search `current` for a match but it
            // would only be valid if the key is simple-shading and the match was the last, at
            // which point we can allow for overlap. We could also try to do a more detailed
            // per-binding list bounds check to see if the draw could skip past some of the
            // bindings to find a match. However, since we have a targetLayer already, `current`
            // would only be one deeper, so it's often not worth the trade off of additional
            // search time.
            // FIXME this didn't come up with just checking the tail binding, but maybe if we did
            // per binding list bounds checks, we would find more matches?
        } else if key.is_simple_shading() && !self.layers.is_empty() {
            // As a simple-shading draw, there is the potential to pull a previous binding list
            // forward to a new layer. This must be an exact match so that we can rely on
            // rasterization order resolving any overlap (since !targetLayer implies the test
            // originally failed for fLayers.tail()).
            let tail = self.layers.len() - 1;
            forward_merge = self.layers[tail].search_binding(&self.arena, key, None);
            if let Some(merge) = forward_merge {
                let binding = &self.arena.bindings[merge as usize];
                if !binding.key.is_equal(key) {
                    forward_merge = None;
                } else if binding.prev().is_none() && binding.next().is_none() {
                    // The tail had a single exact matching binding list, so just append to it.
                    // There are no other incompatible bindings whose overlap we need to worry
                    // about.
                    target_layer = Some(tail);
                    target_match = forward_merge;
                    forward_merge = None;
                } // else we'll transfer the forward merge list into a new layer below
            } // else didn't find a match in the last layer
        } // else not forward-merge eligible and no target, so we'll put it in a new layer

        let target_layer = match target_layer {
            Some(target) => target,
            None => {
                self.order_counter = self.order_counter.next();
                let mut new_layer = Layer::new(self.order_counter);
                if let Some(merge) = forward_merge {
                    let tail = self.layers.len() - 1;
                    let draw_params = &self.draw_params;
                    self.layers[tail].transfer(&mut self.arena, merge, &mut new_layer, &|id| {
                        draw_params[id.0 as usize].draw_bounds()
                    });
                    target_match = Some(merge);
                }
                self.layers.push(new_layer);
                self.layers.len() - 1
            }
        };

        let target_match = match target_match {
            Some(existing) if self.arena.bindings[existing as usize].key.is_equal(key) => {
                debug_assert!(
                    self.layers[target_layer]
                        .bindings
                        .is_in_list(&self.arena, existing)
                );
                existing
            }
            // If targetMatch is just a pipeline match, we can insert right before it because
            // such a match is only returned when the new draw can be ordered in front of it.
            other => self.layers[target_layer].add_new_binding(
                &mut self.arena,
                other,
                *key,
                step.clone(),
            ),
        };

        (target_layer, target_match)
    }

    /// `findOrCreateBindingInLayer(layer, parent, step, key)`.
    // Port of: src/gpu/graphite/DrawListLayer.cpp#L126-L161 (chrome/m156)
    fn find_or_create_binding_in_layer(
        &mut self,
        layer: usize,
        mut parent: Option<BindingId>,
        step: &Arc<dyn RenderStep>,
        key: &LayerKey,
    ) -> BindingId {
        // If we have a parent step's BindingList to insert before, it must be in `layer`.
        debug_assert!(
            parent.is_none_or(|p| self.layers[layer].bindings.is_in_list(&self.arena, p))
        );

        let mut target_match: Option<BindingId> = None;

        // If we don't have a parent, search through all bindings of the layer as this is the
        // first time through the layer. If we do have a parent, search through the preceding
        // bindings (exclusive). This is handled automatically by searchBinding's `parent`
        // handling; when there are no preceding bindings (e.g. parent && !parent->fPrev),
        // `match` will just be null.
        if let Some(found) = self.layers[layer].search_binding(&self.arena, key, parent) {
            if self.arena.bindings[found as usize].key.is_equal(key) {
                target_match = Some(found);
            } else {
                // NOTE: Treat any pipeline match as the new parent that a new binding list will
                // be inserted before. Since the search started from the original parent
                // (exclusive), any found pipeline match will still be before that parent.
                parent = Some(found);
            }
        }

        match target_match {
            Some(existing) => existing,
            None => self.layers[layer].add_new_binding(&mut self.arena, parent, *key, step.clone()),
        }
    }

    /// `recordDraw(...)`: `Layer` has dual purpose here:
    ///  1. (Producer) If recording a depth-only draw, the returned layer is remembered as the
    ///     earliest possible layer that a later clipped draw can be added to. This is stored on
    ///     the `ClipStack::Element` that produced the depth-only draw.
    ///  2. (Consumer) If recording a clipped draw, the layer passed in is the latest layer
    ///     inserted into across *all depth only draws* which affect this draw. If the draw has
    ///     no other bounds dependencies, this represents the layer that it can be directly added
    ///     to.
    ///
    /// `DrawListLayer` requires that all `Transform`s be valid and asserts as much; invalid
    /// transforms should be detected at the `Device` level or similar. The provided `Renderer`
    /// must be compatible with the 'shape' and 'stroke' parameters.
    // Port of: src/gpu/graphite/DrawListLayer.cpp#L163-L335 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::similar_names)] // mirrors the C++ names
    pub(crate) fn record_draw(
        &mut self,
        args: &RecordDrawArgs<'_>,
        gatherer: &mut PipelineDataGatherer,
        mut storage_context: Option<&mut StorageContext>,
        last_insertion: Option<LayerId>,
    ) -> (Option<DrawParamsId>, Option<LayerId>) {
        let renderer = args.renderer;
        debug_assert!(args.local_to_device.valid());
        debug_assert!(
            !args.geometry.is_empty() && !args.clip.draw_bounds().is_empty_negative_or_nan()
        );

        // `testMask` limits what we test against when searching backwards, which is based on
        // the Renderer's aggregate requirements so that the layer we find will be valid for all
        // steps. This is particularly important for stencil-based renderers, which consist of a
        // non-shading "producer" step, which writes into the stencil buffer, and shading
        // "consumer" render steps which test against the stencil mask and clear the buffer
        // afterwards. This guarantees atomicity within a single layer, where the last step finds
        // a safe layer and all earlier steps are explicitly inserted before that. This minimizes
        // pipeline switches as rendering can proceed through the steps in bulk.
        let mut test_mask = BoundsFlags::NONE;
        if has_stencil(renderer.depth_stencil_flags()) {
            test_mask |= BoundsFlags::STENCIL;
        }
        // Draws that blend must respect painter's order, and clipping depth-only draws cannot be
        // ordered in front of shading draws.
        let is_depth_only = !args.paint_id.is_valid();
        let depends_on_dst = args.dst_usage.contains(DstUsage::DEPENDS_ON_DST);
        if depends_on_dst || is_depth_only {
            test_mask |= BoundsFlags::COLOR;
        }

        // In simple situations, we can allow overlaps within a BindingList and let GPU
        // rasterization resolve the rendering order automatically. This does not apply if
        // barriers are required, and it does not apply when the Renderer has multiple steps
        // (must keep the sets of draws in each step disjoint so there isn't interference).
        let mut base_layer_mask = BoundsFlags::NONE;
        if args.barrier_before_draws != BarrierType::None || renderer.num_render_steps() > 1 {
            base_layer_mask |= BoundsFlags::MUST_BE_DISJOINT;
        }

        // Currently, the draw params are created once per record draw call, and the id is passed
        // to each draw call. This is storage effecient but will still introduce some pointer
        // chasing, because the params will likely no longer be on the same cache line for
        // successor render steps. We should test whether it is faster for each step to hold a
        // copy of the params except in the case of clipped draws (which must share a copy
        // because they are mutated later).
        let draw_params_id =
            DrawParamsId(u32::try_from(self.draw_params.len()).expect("draw count fits"));
        self.draw_params.push(DrawParams::new(
            *args.local_to_device,
            args.geometry.clone(),
            args.clip,
            args.ordering,
            args.stroke,
            args.barrier_before_draws,
        ));

        let mut insertion_layer: Option<usize> = None;
        let mut last_step_binding: Option<BindingId> = None;
        // If we're an easy draw, jump to the latestInsertion layer since we don't have to test
        if test_mask == BoundsFlags::NONE && base_layer_mask == BoundsFlags::NONE {
            insertion_layer = match last_insertion {
                Some(layer) => Some(layer.0 as usize),
                None => (!self.layers.is_empty()).then_some(0),
            };
        }

        let mut all_layer_masks = BoundsFlags::NONE;
        for step_index in (0..renderer.num_render_steps()).rev() {
            let step = &renderer.steps()[step_index];
            let performs_shading = step.performs_shading() && args.paint_id.is_valid();

            if let Some(storage_context) = storage_context.as_deref_mut()
                && step.base().storage_uniform_stride() > 0
            {
                storage_context.record_alignment(
                    step.base().storage_uniform_stride(),
                    step.base().storage_uniform_alignment(),
                );
            }

            gatherer.mark_offset_and_align(performs_shading, step.base().uniform_alignment());

            let pipeline_index = self.state.pipeline_cache.insert(GraphicsPipelineDesc::new(
                step.render_step_id(),
                if performs_shading {
                    args.paint_id
                } else {
                    UniquePaintParamsID::invalid()
                },
            ));

            step.write_uniforms_and_textures(
                &self.draw_params[draw_params_id.0 as usize],
                gatherer,
            );

            let (combined_uniforms, combined_textures) =
                gatherer.end_combined_data(performs_shading);

            let uniform_index = if combined_uniforms.is_empty() {
                K_INVALID_INDEX
            } else {
                self.state.uniform_data_cache.insert(combined_uniforms)
            };
            let texture_binding_index = if combined_textures.empty() {
                K_INVALID_INDEX
            } else {
                self.state.texture_data_cache.insert(combined_textures)
            };

            // `layerMask` defines what this draw will block in new draws from going backwards.
            // This is per-step so that stencil-only draws can be grouped between shading and
            // clip draws.
            let mut layer_mask = base_layer_mask;
            if has_stencil(crate::graphite::renderer::depth_stencil_flags_of(
                step.base().depth_stencil_settings(),
            )) {
                layer_mask |= BoundsFlags::STENCIL;
            }
            if step.performs_shading() && args.paint_id.is_valid() {
                // NOTE: This is not dependsOnDst because it represents what is written by the
                // draw, not what might be read for blending the draw.
                layer_mask |= BoundsFlags::COLOR;
            }

            let key = LayerKey {
                pipeline_index,
                texture_index: texture_binding_index,
                uniform_index: if self.storage_buffer_support {
                    K_INVALID_INDEX
                } else {
                    uniform_index
                },
                flags: layer_mask,
            };
            all_layer_masks |= layer_mask;

            if let Some(layer) = insertion_layer {
                // Put the earlier steps in the same layer (valid because we used BoundsFlags for
                // the whole Renderer).
                last_step_binding = Some(self.find_or_create_binding_in_layer(
                    layer,
                    last_step_binding,
                    step,
                    &key,
                ));
            } else {
                // Since we don't have a layer yet, search from the most recent layer back.
                let stop = match last_insertion {
                    Some(layer) => self.layers[layer.0 as usize].order,
                    None => DrawOrder::K_NO_INTERSECTION,
                };
                let (layer, binding) =
                    self.search_backwards(step, &key, test_mask, draw_params_id, stop);
                insertion_layer = Some(layer);
                last_step_binding = Some(binding);
            }

            let binding = last_step_binding.expect("a binding was found or created");
            let draw_bounds = self.draw_params[draw_params_id.0 as usize].draw_bounds();
            self.arena.add_draw(
                binding,
                draw_params_id,
                draw_bounds,
                uniform_index,
                depends_on_dst,
            );

            gatherer.rewind_for_render_step();
        }

        // This must be called once for the layer the draw's rendersteps were added into, so do
        // it at the end since we'll always have the layer at this point. This uses bounds flags
        // applying to whole Renderer.
        let insertion_layer = insertion_layer.expect("a layer was found or created");
        self.layers[insertion_layer].update_for_draw(
            self.draw_params[draw_params_id.0 as usize].draw_bounds(),
            all_layer_masks,
        );

        self.state.render_step_count += renderer.num_render_steps();
        self.draw_count += 1;
        self.state
            .record_draw_bounds_and_flags(renderer, args.clip, args.dst_usage);

        (
            Some(draw_params_id),
            Some(LayerId(
                u32::try_from(insertion_layer).expect("layer count fits"),
            )),
        )
    }

    /// `snapDrawPass(...)`.
    // Port of: src/gpu/graphite/DrawListLayer.cpp#L337-L494 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    pub(crate) fn snap_draw_pass(
        &mut self,
        mut storage_context: Option<&mut StorageContext>,
        args: SnapArgs<'_, '_>,
    ) -> Option<DrawPass> {
        let recorder = args.recorder;
        let record_dependency = args.record_dependency;
        let buffer_mgr = recorder.draw_buffer_manager();
        let mut command_list = SharedCommandList::new();
        let direct = Rc::clone(&command_list.0);
        let mut draw_writer = make_draw_writer(&mut command_list, buffer_mgr);

        let mut uniform_tracker = UniformTracker::new(self.storage_buffer_support);
        let mut texture_binding_tracker = TextureTracker::new();

        let rebind_textures_on_pipeline_change =
            args.dst_read_strategy == DstReadStrategy::TextureCopy;

        if let Some(storage_context) = storage_context.as_deref_mut() {
            storage_context.finalize_precached_storage_data();
        }

        let mut last_pipeline = K_INVALID_INDEX;
        let target_bounds = IRect::from_size(args.target_dimensions);
        let mut last_scissor = target_bounds;

        debug_assert!(
            args.target.is_fully_lazy()
                || IRect::from_size(args.target.dimensions()).contains(&last_scissor)
        );
        direct.borrow_mut().set_scissor(last_scissor);
        // Accumulate rough pixel area touched by each pipeline
        let mut pipeline_draw_areas = vec![0.0_f32; self.state.pipeline_cache.count()];

        for layer in &self.layers {
            for list_id in layer.bindings.ids(&self.arena) {
                let list = &self.arena.bindings[list_id as usize];
                debug_assert!(list.head.is_some()); // not empty

                // The first draw of the BindingList will be changing bindings
                let mut current = list.head;
                let mut first = true;
                while let Some(draw_id) = current {
                    let draw = self.arena.draws[draw_id as usize];
                    let bindings_are_invariant = !first;
                    let start_of_layer = first && list.prev().is_none();
                    first = false;

                    let draw_params = &self.draw_params[draw.draw_params.0 as usize];
                    let render_step = &list.step;
                    let key = &list.key;

                    let mut pipeline_change = false;
                    let mut texture_bindings_change = false;

                    if !bindings_are_invariant {
                        pipeline_change = key.pipeline_index != last_pipeline;

                        texture_bindings_change = texture_binding_tracker
                            .set_current_texture_bindings(key.texture_index)
                            || (rebind_textures_on_pipeline_change
                                && pipeline_change
                                && key.texture_index != K_INVALID_INDEX);
                    }

                    // Uniforms are binding invariant when SSBOs are disabled, but it's simpler
                    // to just let `uniformBindingChange` eval to false more often. The uniform
                    // index must come from the Draw to get the right value when SSBOs are
                    // enabled.
                    let uniform_binding_change = uniform_tracker.write_uniforms(
                        &mut self.state.uniform_data_cache,
                        buffer_mgr,
                        draw.uniform_index,
                    );

                    pipeline_draw_areas[key.pipeline_index as usize] +=
                        draw_params.draw_bounds().area();

                    let new_scissor =
                        render_step.get_scissor(draw_params, last_scissor, target_bounds);

                    if pipeline_change {
                        draw_writer.new_pipeline_state(
                            render_step.base().primitive_type(),
                            render_step.base().static_data_stride(),
                            render_step.append_data_stride(draw_params),
                            render_state_flags(&**render_step),
                            draw_params.barrier_before_draws(),
                        );
                    } else if uniform_binding_change
                        || texture_bindings_change
                        || new_scissor.is_some()
                    {
                        draw_writer.new_dynamic_state();
                    } else if draw_params.barrier_before_draws() != BarrierType::None
                        && start_of_layer
                    {
                        // Taking this branch means there were no state or pipeline changes
                        // between old layer and this layer's first draw. This only happens if
                        // the draws overlap, so flush the drawWriter since the draw requires a
                        // barrier.
                        draw_writer.flush();
                    }

                    if pipeline_change {
                        direct
                            .borrow_mut()
                            .bind_graphics_pipeline(key.pipeline_index);
                        last_pipeline = key.pipeline_index;
                    }
                    if uniform_binding_change {
                        uniform_tracker
                            .bind_uniforms(UniformSlot::CombinedUniforms, &mut direct.borrow_mut());
                    }
                    if texture_bindings_change {
                        texture_binding_tracker.bind_textures(
                            &self.state.texture_data_cache,
                            &mut direct.borrow_mut(),
                        );
                    }
                    if let Some(new_scissor) = new_scissor {
                        direct.borrow_mut().set_scissor(new_scissor);
                        last_scissor = new_scissor;
                    }

                    let uniform_ssbo_index = if self.storage_buffer_support {
                        uniform_tracker.ssbo_index()
                    } else {
                        0
                    };
                    render_step.write_vertices(&mut draw_writer, draw_params, uniform_ssbo_index);

                    // Either stop early on failure, or advance to the next Draw
                    if buffer_mgr.has_mapping_failed() {
                        break;
                    }
                    current = draw.next;
                }
            }
        }
        draw_writer.flush();

        if buffer_mgr.has_mapping_failed() {
            skia_log_w!("Failed to write necessary vertex/instance data for DrawPass, dropping!");
            drop(draw_writer);
            self.reset(LoadOp::Load, Color4f::new(0.0, 0.0, 0.0, 0.0));
            return None;
        }
        drop(draw_writer);

        let mut storage_result = None;
        if let Some(storage_context) = storage_context {
            let has_storage_data = !storage_context.is_empty();
            storage_result = storage_context.finalize(recorder, record_dependency);
            if has_storage_data && storage_result.is_none() {
                skia_log_w!("Failed to write Storage Data for Draw pass, dropping!");
                self.reset(LoadOp::Load, Color4f::new(0.0, 0.0, 0.0, 0.0));
                return None;
            }
        }

        drop(direct);
        let mut draw_pass = DrawPass::new(
            args.target,
            (self.state.load_op, StoreOp::Store),
            self.state.clear_color,
            recorder.pipeline_manager(),
        );
        draw_pass.command_list = command_list.into_list();
        if let Some(storage_result) = storage_result {
            draw_pass.set_storage_result(storage_result);
        }
        draw_pass.pipeline_draw_areas = pipeline_draw_areas;

        let mut pass_bounds = self.state.pass_bounds;
        pass_bounds.round_out();
        draw_pass.bounds = pass_bounds.as_sk_irect();
        draw_pass.pipeline_descs = self.state.pipeline_cache.detach();
        draw_pass.sampled_textures = self.state.texture_data_cache.detach_textures();

        self.reset(LoadOp::Load, Color4f::new(0.0, 0.0, 0.0, 0.0));

        Some(draw_pass)
    }
}

// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawList.h, src/gpu/graphite/DrawList.cpp

//! [`DrawList`]: the sort-based draw list. Every draw records one sort key per render step, and
//! the keys are sorted when the list is snapped into a [`DrawPass`].
//!
//! Skia's `SortKey` holds a `const Draw*`; here it holds the index of the draw in the list's
//! `Vec<Draw>` (and an `Arc` of the render step it sorts), with the same 128-bit key and the same
//! comparison. The sort is stable, so keys that compare equal keep their recording order.

use std::cmp::Ordering;
use std::sync::Arc;

use skia_rust_core::color::Color4f;
use skia_rust_core::rect::{Contains, IRect};

use crate::gpu::sk_log::skia_log_w;
use crate::graphite::draw_list_base::{
    DrawListBaseState, MAX_RENDER_STEPS, RecordDrawArgs, SharedCommandList, SnapArgs,
    TextureTracker, UniformTracker, make_draw_writer, render_state_flags,
};
use crate::graphite::draw_list_types::{DrawParamsId, LayerId};
use crate::graphite::draw_order::{CompressedPaintersOrder, DrawOrder};
use crate::graphite::draw_params::DrawParams;
use crate::graphite::draw_pass::DrawPass;
use crate::graphite::draw_types::{BarrierType, UniformSlot};
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::graphite_types::DepthStencilFlags;
use crate::graphite::pipeline_data::{K_INVALID_INDEX, PipelineDataGatherer};
use crate::graphite::render_step::RenderStep;
use crate::graphite::resource_types::{DstReadStrategy, LoadOp, StoreOp};
use crate::graphite::storage_context::StorageContext;

// `Bitfield<Bits, Offset>`: fields are ordered from most-significant to least when sorting by
// 128-bit value. We don't use bit fields because field ordering is implementation defined and we
// need to sort consistently.
// Port of: src/gpu/graphite/DrawList.h#L76-L83 (chrome/m156)
fn field_set(bits: u32, offset: u32, v: u32) -> u64 {
    (u64::from(v) & ((1_u64 << bits) - 1)) << offset
}

fn field_get(bits: u32, offset: u32, v: u64) -> u32 {
    // Truncation is the point: every field is at most 30 bits wide, except `UniformField`,
    // which is 34 bits to hold an extra "no-data" bit and is truncated as in the C++.
    #[allow(clippy::cast_possible_truncation)]
    {
        ((v >> offset) & ((1_u64 << bits) - 1)) as u32
    }
}

// `ColorDepthOrderField`, `StencilIndexField`, `RenderStepField`, `PipelineField`.
const COLOR_DEPTH_ORDER: (u32, u32) = (16, 48);
const STENCIL_INDEX: (u32, u32) = (16, 32);
const RENDER_STEP: (u32, u32) = (2, 30);
const PIPELINE: (u32, u32) = (30, 0);
// The uniform/texture index fields need 1 extra bit to encode "no-data". Values that are greater
// than or equal to 2^(bits-1) represent "no-data", while values between [0, 2^(bits-1)-1] can
// access data arrays without extra logic.
const UNIFORM: (u32, u32) = (34, 30);
const TEXTURE_BINDINGS: (u32, u32) = (30, 0);

/// Each `Draw` in a `DrawList` might be processed by multiple `RenderStep`s (determined by the
/// `Draw`'s `Renderer`), which can be sorted independently. Each (step, draw) pair produces its
/// own `SortKey`.
///
/// The goal of sorting draws for the `DrawPass` is to minimize pipeline transitions and dynamic
/// binds within a pipeline, while still respecting the overall painter's order. This decreases
/// the number of low-level draw commands in a command buffer and increases the size of those,
/// allowing the GPU to operate more efficiently and have fewer bubbles within its own instruction
/// stream.
///
/// The `Draw`'s `CompressedPaintersOrder` and `DisjointStencilIndex` represent the most
/// significant bits of the key, and are shared by all `SortKey`s produced by the same draw. Next,
/// the pipeline description is encoded in two steps:
///  1. The index of the `RenderStep` packed in the high bits to ensure each step for a draw is
///     ordered correctly.
///  2. An index into a cache of pipeline descriptions is used to encode the identity of the
///     pipeline (`SortKey`s that differ in the bits from #1 necessarily would have different
///     descriptions, but then the specific ordering of the `RenderStep`s isn't enforced). Last,
///     the `SortKey` encodes an index into the set of uniform bindings accumulated for a
///     `DrawPass`. This allows the `SortKey` to cluster draw steps that have both a compatible
///     pipeline and do not require rebinding uniform data or other state (e.g. scissor). Since
///     the uniform data index and the pipeline description index are packed into indices and not
///     actual pointers, a given `SortKey` is only valid for the a specific `DrawList` to
///     `DrawPass` conversion.
// Port of: src/gpu/graphite/DrawList.h#L66-L172 (chrome/m156)
#[derive(Debug)]
struct SortKey {
    pipeline_key: u64,
    uniform_key: u64,
    // Backpointer to the draw that produced the sort key
    draw: usize,
    step: Arc<dyn RenderStep>,
}

impl SortKey {
    // Port of: src/gpu/graphite/DrawList.h#L86-L103 (chrome/m156)
    fn new(
        draw: usize,
        draw_order: DrawOrder,
        step: Arc<dyn RenderStep>,
        render_step: usize,
        pipeline_index: u32,
        uniform_index: u32,
        texture_binding_index: u32,
    ) -> Self {
        debug_assert!(pipeline_index < K_INVALID_INDEX);
        let render_step = u32::try_from(render_step).expect("render step index fits");
        Self {
            pipeline_key: field_set(
                COLOR_DEPTH_ORDER.0,
                COLOR_DEPTH_ORDER.1,
                u32::from(draw_order.paint_order().bits()),
            ) | field_set(
                STENCIL_INDEX.0,
                STENCIL_INDEX.1,
                u32::from(draw_order.stencil_index().bits()),
            ) | field_set(RENDER_STEP.0, RENDER_STEP.1, render_step)
                | field_set(PIPELINE.0, PIPELINE.1, pipeline_index),
            uniform_key: field_set(UNIFORM.0, UNIFORM.1, uniform_index)
                | field_set(
                    TEXTURE_BINDINGS.0,
                    TEXTURE_BINDINGS.1,
                    texture_binding_index,
                ),
            draw,
            step,
        }
    }

    fn pipeline_index(&self) -> u32 {
        field_get(PIPELINE.0, PIPELINE.1, self.pipeline_key)
    }

    fn uniform_index(&self) -> u32 {
        field_get(UNIFORM.0, UNIFORM.1, self.uniform_key)
    }

    fn texture_binding_index(&self) -> u32 {
        field_get(TEXTURE_BINDINGS.0, TEXTURE_BINDINGS.1, self.uniform_key)
    }
}

impl PartialEq for SortKey {
    fn eq(&self, other: &Self) -> bool {
        self.pipeline_key == other.pipeline_key && self.uniform_key == other.uniform_key
    }
}

impl Eq for SortKey {}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortKey {
    // Port of: src/gpu/graphite/DrawList.h#L105-L108 (chrome/m156), `operator<`
    fn cmp(&self, other: &Self) -> Ordering {
        self.pipeline_key
            .cmp(&other.pipeline_key)
            .then(self.uniform_key.cmp(&other.uniform_key))
    }
}

/// `DrawList::Draw`.
// Port of: src/gpu/graphite/DrawList.h#L56-L70 (chrome/m156)
#[derive(Debug)]
struct Draw {
    num_render_steps: usize,
    draw_params: DrawParams,
}

/// The sort-based draw list: it keeps every draw and one sort key per render step, and sorts the
/// keys to produce the commands of the draw pass.
// Port of: src/gpu/graphite/DrawList.h#L32-L175 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawList")]
#[derive(Debug, Default)]
pub struct DrawList {
    state: DrawListBaseState,
    draws: Vec<Draw>,
    sort_keys: Vec<SortKey>,
}

impl DrawList {
    /// An empty list that loads the target.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn state(&self) -> &DrawListBaseState {
        &self.state
    }

    /// `recordDraw(...)`: requires that all `Transform`s be valid and asserts as much; invalid
    /// transforms should be detected at the `Device` level or similar. The provided `Renderer`
    /// must be compatible with the 'shape' and 'stroke' parameters. If the renderer uses
    /// coverage AA, 'ordering' must have a compressed painters order that reflects that. If the
    /// renderer uses stencil, the 'ordering' must have a valid stencil index as well.
    // Port of: src/gpu/graphite/DrawList.cpp#L24-L98 (chrome/m156)
    pub(crate) fn record_draw(
        &mut self,
        args: &RecordDrawArgs<'_>,
        gatherer: &mut PipelineDataGatherer,
        mut storage_context: Option<&mut StorageContext>,
    ) -> (Option<DrawParamsId>, Option<LayerId>) {
        let renderer = args.renderer;
        debug_assert!(args.local_to_device.valid());
        debug_assert!(
            !args.geometry.is_empty() && !args.clip.draw_bounds().is_empty_negative_or_nan()
        );
        debug_assert!(
            !matches!(
                renderer.depth_stencil_flags(),
                DepthStencilFlags::Stencil | DepthStencilFlags::DepthStencil
            ) || args.ordering.stencil_index() != DrawOrder::K_UNASSIGNED
        );

        // TODO: Add validation that the renderer's expected shape type and stroke params match
        // provided
        let draw_index = self.draws.len();
        self.draws.push(Draw {
            num_render_steps: renderer.num_render_steps(),
            draw_params: DrawParams::new(
                *args.local_to_device,
                args.geometry.clone(),
                args.clip,
                args.ordering,
                args.stroke,
                args.barrier_before_draws,
            ),
        });
        let draw = &self.draws[draw_index];

        self.state.render_step_count += renderer.num_render_steps();
        // Create a sort key for every render step in this draw
        for step_index in 0..renderer.num_render_steps() {
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
                    crate::graphite::unique_paint_params_id::UniquePaintParamsID::invalid()
                },
            ));

            step.write_uniforms_and_textures(&draw.draw_params, gatherer);

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

            self.sort_keys.push(SortKey::new(
                draw_index,
                draw.draw_params.order(),
                step.clone(),
                step_index,
                pipeline_index,
                uniform_index,
                texture_binding_index,
            ));
            gatherer.rewind_for_render_step();
        }

        self.state
            .record_draw_bounds_and_flags(renderer, args.clip, args.dst_usage);

        (None, None)
    }

    /// `snapDrawPass(...)`: sorts the keys and converts the draws into a `DrawPass`. The pass is
    /// immutable from here on.
    // Port of: src/gpu/graphite/DrawList.cpp#L100-L271 (chrome/m156)
    #[allow(clippy::too_many_lines)] // a single C++ function (DrawList::snapDrawPass)
    pub(crate) fn snap_draw_pass(
        &mut self,
        mut storage_context: Option<&mut StorageContext>,
        args: SnapArgs<'_, '_>,
    ) -> Option<DrawPass> {
        // TODO: Explore sorting algorithms; in all likelihood this will be mostly sorted
        // already, so algorithms that approach O(n) in that condition may be favorable.
        // Alternatively, could explore radix sort that is always O(n).
        // TODO: It's not strictly necessary, but would a stable sort be useful or just end up
        // hiding bugs in the DrawOrder determination code? Skia sorts with `std::sort`; this is
        // stable so keys that compare equal keep their recording order.
        self.sort_keys.sort();

        // The DrawList is converted directly into the DrawPass' data structures, but once the
        // DrawPass is returned from Make(), it is considered immutable.
        let recorder = args.recorder;
        let record_dependency = args.record_dependency;
        let buffer_mgr = recorder.draw_buffer_manager();
        let mut command_list = SharedCommandList::new();
        let direct = std::rc::Rc::clone(&command_list.0);
        let mut draw_writer = make_draw_writer(&mut command_list, buffer_mgr);
        let mut last_pipeline = K_INVALID_INDEX;
        let target_bounds = IRect::from_size(args.target_dimensions);
        let mut last_scissor = target_bounds;

        debug_assert!(
            args.target.is_fully_lazy()
                || IRect::from_size(args.target.dimensions()).contains(&last_scissor)
        );
        direct.borrow_mut().set_scissor(last_scissor);

        let caps = recorder.caps();
        let use_storage_buffers = caps.storage_buffer_support();
        let mut uniform_tracker = UniformTracker::new(use_storage_buffers);

        if let Some(storage_context) = storage_context.as_deref_mut() {
            storage_context.finalize_precached_storage_data();
        }

        // TODO(b/372953722): Remove this forced binding command behavior once dst copies are
        // always bound separately from the rest of the textures.
        let rebind_textures_on_pipeline_change =
            args.dst_read_strategy == DstReadStrategy::TextureCopy;
        // Keep track of the prior draw's PaintOrder. If the current draw requires barriers and
        // there is no pipeline or state change, then we must compare the current and prior
        // draw's PaintOrders to determine if the draws overlap. If they do, we must inject a
        // flush between them such that the barrier addition and draw commands are ordered
        // correctly.
        let mut prior_draw_paint_order = CompressedPaintersOrder::first();

        // Accumulate rough pixel area touched by each pipeline as we iterate the SortKeys
        let mut pipeline_draw_areas = vec![0.0_f32; self.state.pipeline_cache.count()];

        let mut texture_binding_tracker = TextureTracker::new();
        for key in &self.sort_keys {
            let draw = &self.draws[key.draw];
            debug_assert!(draw.num_render_steps > 0);
            let render_step = &key.step;
            let params = &draw.draw_params;

            let pipeline_change = key.pipeline_index() != last_pipeline;
            pipeline_draw_areas[key.pipeline_index() as usize] += params.draw_bounds().area();

            let uniform_binding_change = uniform_tracker.write_uniforms(
                &mut self.state.uniform_data_cache,
                buffer_mgr,
                key.uniform_index(),
            );

            // TODO(b/372953722): The Dawn and Vulkan CommandBuffer implementations currently
            // append any dst copy to the texture bind group/descriptor set automatically when
            // processing a BindTexturesAndSamplers call because they use a single group to
            // contain all textures. However, from the DrawPass POV, we can run into the scenario
            // where two pipelines have the same textures+samplers except one requires a dst-copy
            // and the other does not. In this case we wouldn't necessarily insert a new command
            // when the pipeline changed and then end up with layout validation errors.
            let texture_bindings_change = texture_binding_tracker
                .set_current_texture_bindings(key.texture_binding_index())
                || (rebind_textures_on_pipeline_change
                    && pipeline_change
                    && key.texture_binding_index() != K_INVALID_INDEX);

            let new_scissor = render_step.get_scissor(params, last_scissor, target_bounds);

            let state_change =
                uniform_binding_change || texture_bindings_change || new_scissor.is_some();

            // Update DrawWriter *before* we actually change any state so that accumulated draws
            // from the previous state use the proper state.
            if pipeline_change {
                draw_writer.new_pipeline_state(
                    render_step.base().primitive_type(),
                    render_step.base().static_data_stride(),
                    render_step.append_data_stride(params),
                    render_state_flags(&**render_step),
                    params.barrier_before_draws(),
                );
            } else if state_change {
                draw_writer.new_dynamic_state();
            } else if params.barrier_before_draws() != BarrierType::None
                && prior_draw_paint_order != params.order().paint_order()
            {
                // Even if there is no pipeline or state change, we must consider whether a
                // DrawPassCommand to add barriers must be inserted before any draw commands. If
                // so, then determine if the current and prior draws overlap (ie, their
                // PaintOrders are unequal). If so, perform a flush() to make sure the draw and
                // add barrier commands are appended to the command list in the proper order.
                draw_writer.flush();
            }

            // Make state changes before accumulating new draw data
            if pipeline_change {
                direct
                    .borrow_mut()
                    .bind_graphics_pipeline(key.pipeline_index());
                last_pipeline = key.pipeline_index();
            }
            if state_change {
                if uniform_binding_change {
                    uniform_tracker
                        .bind_uniforms(UniformSlot::CombinedUniforms, &mut direct.borrow_mut());
                }
                if texture_bindings_change {
                    texture_binding_tracker
                        .bind_textures(&self.state.texture_data_cache, &mut direct.borrow_mut());
                }
                if let Some(new_scissor) = new_scissor {
                    direct.borrow_mut().set_scissor(new_scissor);
                    last_scissor = new_scissor;
                }
            }

            let uniform_ssbo_index = if use_storage_buffers {
                uniform_tracker.ssbo_index()
            } else {
                0
            };
            render_step.write_vertices(&mut draw_writer, params, uniform_ssbo_index);

            if buffer_mgr.has_mapping_failed() {
                skia_log_w!(
                    "Failed to write necessary vertex/instance data for DrawPass, dropping!"
                );
                drop(draw_writer);
                self.reset(LoadOp::Load, Color4f::new(0.0, 0.0, 0.0, 0.0));
                return None;
            }

            // Update priorDrawPaintOrder value before iterating to analyze the next draw.
            prior_draw_paint_order = params.order().paint_order();
        }
        // Finish recording draw calls for any collected data still pending at end of the loop
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

    /// `reset(op, clearColor)`: discards all previously recorded draws and sets the requested
    /// load op (with optional clear color).
    // Port of: src/gpu/graphite/DrawList.cpp#L273-L278 (chrome/m156)
    pub fn reset(&mut self, op: LoadOp, clear_color: Color4f) {
        self.draws.clear();
        self.sort_keys.clear();
        self.state.reset(op, clear_color);
    }
}

// Keeps `MAX_RENDER_STEPS` referenced: the packed fields are sized for it
// (`static_assert(PipelineField::kBits >= SkNextLog2(DrawListBase::kMaxRenderSteps))`).
const _: () = assert!(PIPELINE.0 >= MAX_RENDER_STEPS.next_power_of_two().trailing_zeros());
const _: () = assert!(TEXTURE_BINDINGS.0 > MAX_RENDER_STEPS.next_power_of_two().trailing_zeros());
const _: () = assert!(UNIFORM.0 > MAX_RENDER_STEPS.next_power_of_two().trailing_zeros());

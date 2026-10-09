// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawListBase.h

//! [`DrawListBase`]: the draws recorded into a `DrawContext` before they are snapped into a
//! [`DrawPass`], either sorted at snap time ([`DrawList`]) or collected in layers
//! ([`DrawListLayer`]).
//!
//! `DrawListBase` is an enum over the two implementations (`docs/design/gpu.md` §4.1), and
//! [`DrawListBaseState`] holds the members and the helper classes (`UniformTracker`,
//! `TextureTracker`) the two share.
//!
//! Deviations from the C++:
//!
//! - `snapDrawPass()` takes the recorder's `RecorderPriv` and a `record_dependency` closure where
//!   Skia takes a `Recorder*` and the `DrawContext*` (which `StorageContext::finalize()` only
//!   uses for `DrawContext::recordDependency`).
//! - `RenderStep::writeVertices()` is called without the `StorageContext*`: no ported step reads
//!   it.
//! - `fTransforms`, the arena that de-duplicates the transform of consecutive draws, is not kept:
//!   `DrawParams` holds its (small, `Copy`) transform.
//! - `fCoverageMaskShapeDrawCount` (`SK_DEBUG`) waits for `Geometry::isCoverageMaskShape()`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use skia_rust_core::size::ISize;

use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::buffer_manager::{BufferSubAllocator, DrawBufferManager};
use crate::graphite::draw_list::DrawList;
use crate::graphite::draw_list_layer::DrawListLayer;
use crate::graphite::draw_list_types::{DrawParamsId, LayerId};
use crate::graphite::draw_order::DrawOrder;
use crate::graphite::draw_params::{Clip, StrokeStyle};
use crate::graphite::draw_pass::{CommandList, DrawPass};
use crate::graphite::draw_types::{BarrierType, DstUsage, RenderStateFlags, UniformSlot};
use crate::graphite::draw_writer::{DrawPassCommandList, DrawWriter};
use crate::graphite::geom::geometry::Geometry;
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::transform::Transform;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineCache;
use crate::graphite::graphite_types::DepthStencilFlags;
use crate::graphite::pipeline_data::{
    K_INVALID_INDEX, PipelineDataGatherer, TextureDataCache, UniformDataCache,
};
use crate::graphite::recorder::RecorderPriv;
use crate::graphite::render_step::{RenderStep, RenderStepFlags};
use crate::graphite::renderer::Renderer;
use crate::graphite::resource_types::{DstReadStrategy, LoadOp};
use crate::graphite::storage_context::StorageContext;
use crate::graphite::task::TaskRef;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

/// `DrawListBase::kMaxRenderSteps`.
///
/// TODO (thomsmit): Remove this limit when `DrawListLayer` is used.
// Port of: src/gpu/graphite/DrawListBase.h#L40 (chrome/m156)
pub const MAX_RENDER_STEPS: usize = 4096;

/// `RenderStep::getRenderStateFlags()`.
// Port of: src/gpu/graphite/Renderer.h#L196-L205 (chrome/m156)
#[must_use]
pub fn render_state_flags(step: &dyn RenderStep) -> RenderStateFlags {
    let flags = step.base().flags();
    let mut rs = RenderStateFlags::NONE;
    if flags.contains(RenderStepFlags::FIXED) {
        rs |= RenderStateFlags::FIXED;
    }
    if flags.contains(RenderStepFlags::APPEND_VERTICES) {
        rs |= RenderStateFlags::APPEND_VERTICES;
    }
    if flags.contains(RenderStepFlags::APPEND_INSTANCES) {
        rs |= RenderStateFlags::APPEND_INSTANCES;
    }
    if flags.contains(RenderStepFlags::APPEND_DYNAMIC_INSTANCES) {
        rs |= RenderStateFlags::APPEND_DYNAMIC_INSTANCES;
    }
    rs
}

/// The members `DrawList` and `DrawListLayer` share (the data members of `DrawListBase`).
// Port of: src/gpu/graphite/DrawListBase.h#L116-L141 (chrome/m156)
#[derive(Debug)]
pub struct DrawListBaseState {
    pub(crate) uniform_data_cache: UniformDataCache,
    pub(crate) texture_data_cache: TextureDataCache,
    pub(crate) pipeline_cache: GraphicsPipelineCache,

    pub(crate) render_step_count: usize,

    pub(crate) dst_read_bounds: Rect,
    pub(crate) pass_bounds: Rect,
    pub(crate) requires_msaa: bool,
    pub(crate) depth_stencil_flags: DepthStencilFlags,

    pub(crate) load_op: LoadOp,
    pub(crate) clear_color: [f32; 4],
}

impl Default for DrawListBaseState {
    fn default() -> Self {
        Self {
            uniform_data_cache: UniformDataCache::new(),
            texture_data_cache: TextureDataCache::new(),
            pipeline_cache: GraphicsPipelineCache::default(),
            render_step_count: 0,
            dst_read_bounds: Rect::infinite_inverted(),
            pass_bounds: Rect::infinite_inverted(),
            requires_msaa: false,
            depth_stencil_flags: DepthStencilFlags::None,
            load_op: LoadOp::Load,
            clear_color: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl DrawListBaseState {
    /// `DrawListBase::reset(op, clearColor)`: discards all previously recorded draws and sets
    /// the requested load op (with optional clear color, which is premultiplied).
    // Port of: src/gpu/graphite/DrawListBase.h#L98-L114 (chrome/m156)
    pub fn reset(&mut self, op: LoadOp, clear_color: skia_rust_core::color::Color4f) {
        self.load_op = op;
        self.clear_color = clear_color.premul().as_array();
        self.render_step_count = 0;
        self.dst_read_bounds = Rect::infinite_inverted();
        self.pass_bounds = Rect::infinite_inverted();
        self.requires_msaa = false;
        self.depth_stencil_flags = DepthStencilFlags::None;

        self.uniform_data_cache.reset();
        self.texture_data_cache.reset();
        self.pipeline_cache.reset();
    }

    /// The bookkeeping `recordDraw()` does after it has recorded the steps of a draw.
    pub(crate) fn record_draw_bounds_and_flags(
        &mut self,
        renderer: &Renderer,
        clip: &Clip,
        dst_usage: DstUsage,
    ) {
        self.pass_bounds.join(clip.draw_bounds());
        self.requires_msaa |= renderer.requires_msaa();
        self.depth_stencil_flags = crate::graphite::renderer::or_depth_stencil_flags(
            self.depth_stencil_flags,
            renderer.depth_stencil_flags(),
        );
        if dst_usage.contains(DstUsage::DST_READ_REQUIRED) {
            // For paints that read from the dst, update the bounds. It may later be determined
            // that the DstReadStrategy does not require them, but they are inexpensive to track.
            self.dst_read_bounds.join(clip.draw_bounds());
        }
    }
}

/// Whether two bindings name the same buffer (`fBuffer == other.fBuffer`).
fn same_buffer(a: &BindBufferInfo, b: &BindBufferInfo) -> bool {
    match (&a.buffer, &b.buffer) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

/// Writes uniform data either to uniform buffers or to shared storage buffers, and tracks when
/// bindings need to change between draws (`UniformTracker`).
// Port of: src/gpu/graphite/DrawListBase.h#L143-L238 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawListBase::UniformTracker")]
#[derive(Debug)]
pub struct UniformTracker {
    // The GPU buffer data is being written into; for SSBOs, Graphite will only record a bind
    // command when this changes. Sub-allocations will be aligned such that they can be randomly
    // accessed even if the data is heterogenous. UBOs will always have to issue binding
    // commands when a draw needs to use a different set of uniform values.
    current_buffer: BufferSubAllocator,

    // Internally track the last binding returned, so that we know whether new uploads or
    // rebindings are necessary. If we're using SSBOs, this is treated specially -- the offset
    // field holds the index in the storage buffer of the last-written uniforms, and the offsets
    // used for actual bindings are always zero.
    last_binding: BindBufferInfo,

    // This keeps track of the last index used for writing uniforms from a provided uniform
    // cache. If a provided index matches the last index, the uniforms are assumed to already be
    // written and no additional uploading is performed. This assumes a UniformTracker will
    // always be provided with the same uniform cache.
    last_index: u32,

    use_storage_buffers: bool,
}

impl UniformTracker {
    /// `UniformTracker(useStorageBuffers)`.
    #[must_use]
    pub fn new(use_storage_buffers: bool) -> Self {
        Self {
            current_buffer: BufferSubAllocator::default(),
            last_binding: BindBufferInfo::default(),
            last_index: K_INVALID_INDEX,
            use_storage_buffers,
        }
    }

    /// `writeUniforms(uniformCache, bufferMgr, index)`: returns whether the binding changed.
    // Port of: src/gpu/graphite/DrawListBase.h#L149-L205 (chrome/m156)
    #[doc(alias = "writeUniforms")]
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn write_uniforms(
        &mut self,
        uniform_cache: &mut UniformDataCache,
        buffer_mgr: &DrawBufferManager,
        index: u32,
    ) -> bool {
        if index >= K_INVALID_INDEX {
            return false;
        }

        if index == self.last_index {
            return false;
        }
        self.last_index = index;

        let uniform_data = uniform_cache.lookup_mut(index);
        let uniform_data_size = uniform_data.cpu_data().size();
        let uniform_binding_size = if self.use_storage_buffers {
            uniform_data_size
        } else {
            uniform_data_size.next_power_of_two()
        };

        // Upload the uniform data if we haven't already.
        // Alternatively, re-upload the uniform data to avoid a rebind if we're using storage
        // buffers. This will result in more data uploaded, but the tradeoff seems worthwhile.
        if uniform_data.buffer_binding().buffer.is_none()
            || (self.use_storage_buffers
                && !same_buffer(uniform_data.buffer_binding(), &self.last_binding))
        {
            let mut binding = self
                .current_buffer
                .get_mapped_subrange_with_headroom(uniform_data_size, uniform_binding_size)
                .map(|(mut writer, binding)| {
                    writer.write_bytes(uniform_data.cpu_data().data());
                    binding
                });
            if binding.is_none() {
                // Allocate a new buffer
                let mut allocation = if self.use_storage_buffers {
                    buffer_mgr.get_mapped_storage_buffer(1, uniform_data_size)
                } else {
                    buffer_mgr.get_mapped_uniform_buffer(uniform_data_size, uniform_binding_size)
                };
                let Some(allocation) = allocation.as_mut() else {
                    return false; // Allocation failed so early out
                };
                allocation
                    .writer()
                    .write_bytes(uniform_data.cpu_data().data());
                binding = Some(allocation.binding.clone());
                self.current_buffer = std::mem::take(&mut allocation.allocator);
            }
            let mut binding = binding.expect("a binding was allocated");

            if self.use_storage_buffers {
                // When using storage buffers, store the SSBO index in the binding's offset
                // field and always use the entire buffer's size in the size field.
                debug_assert_eq!(binding.offset as usize % uniform_data_size, 0);
                binding.offset /= u32::try_from(uniform_data_size).expect("size fits");
                binding.size = u32::try_from(
                    binding
                        .buffer
                        .as_ref()
                        .expect("a binding has a buffer")
                        .size(),
                )
                .expect("size fits");
            } else {
                // Every new set of uniform data has to be bound, this ensures its aligned
                // correctly
                binding.size = u32::try_from(uniform_binding_size).expect("size fits");
                self.current_buffer.reset_for_new_binding();
            }
            uniform_data.set_buffer_binding(binding);
        }

        let binding = uniform_data.buffer_binding().clone();
        let needs_rebind = !same_buffer(&binding, &self.last_binding)
            || (!self.use_storage_buffers && binding.offset != self.last_binding.offset);

        self.last_binding = binding;

        needs_rebind
    }

    /// `bindUniforms(slot, commandList)`.
    // Port of: src/gpu/graphite/DrawListBase.h#L207-L214 (chrome/m156)
    #[doc(alias = "bindUniforms")]
    pub fn bind_uniforms(&self, slot: UniformSlot, command_list: &mut CommandList) {
        let mut binding = self.last_binding.clone();
        if self.use_storage_buffers {
            // Track the SSBO index in fLastBinding, but set offset = 0 in the actual used
            // binding.
            binding.offset = 0;
        }
        command_list.bind_uniform_buffer(binding, slot);
    }

    /// `ssboIndex()`: the SSBO index for the last-bound storage buffer is stored in the
    /// binding's offset field.
    // Port of: src/gpu/graphite/DrawListBase.h#L216-L220 (chrome/m156)
    #[doc(alias = "ssboIndex")]
    #[must_use]
    pub fn ssbo_index(&self) -> u32 {
        self.last_binding.offset
    }
}

/// Tracks when to issue `BindTexturesAndSamplers` commands to a command list and converts
/// `TextureDataBlock`s to that representation as needed (`TextureTracker`).
// Port of: src/gpu/graphite/DrawListBase.h#L222-L250 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawListBase::TextureTracker")]
#[derive(Debug)]
pub struct TextureTracker {
    last_index: u32,
}

impl Default for TextureTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureTracker {
    /// `TextureTracker(textureCache)`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_index: K_INVALID_INDEX,
        }
    }

    /// `setCurrentTextureBindings(bindingIndex)`: whether the bindings changed.
    // Port of: src/gpu/graphite/DrawListBase.h#L228-L235 (chrome/m156)
    #[doc(alias = "setCurrentTextureBindings")]
    pub fn set_current_texture_bindings(&mut self, binding_index: u32) -> bool {
        if binding_index < K_INVALID_INDEX && self.last_index != binding_index {
            self.last_index = binding_index;
            return true;
        }
        // No binding change
        false
    }

    /// `bindTextures(commandList)`.
    // Port of: src/gpu/graphite/DrawListBase.h#L237-L249 (chrome/m156)
    #[doc(alias = "bindTextures")]
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn bind_textures(&self, texture_cache: &TextureDataCache, command_list: &mut CommandList) {
        debug_assert!(self.last_index < K_INVALID_INDEX);
        let binding = texture_cache.lookup(self.last_index);

        let mut textures = Vec::with_capacity(binding.num_textures());
        let mut samplers = Vec::with_capacity(binding.num_textures());
        for i in 0..binding.num_textures() {
            let (texture, sampler) = binding.texture(i);
            textures.push(
                texture
                    .clone()
                    .expect("a recorded draw samples real textures"),
            );
            samplers.push(*sampler);
        }
        command_list.bind_textures_and_samplers(textures, samplers);
    }
}

/// Lets the draw writer and the snapping code both append to one command list: the writer holds
/// this adapter while the snapping code appends the binding commands directly (in C++ both hold a
/// pointer to `fCommandList`).
#[derive(Debug, Clone)]
pub(crate) struct SharedCommandList(pub(crate) Rc<RefCell<CommandList>>);

impl SharedCommandList {
    pub(crate) fn new() -> Self {
        Self(Rc::new(RefCell::new(CommandList::new())))
    }

    /// The finished list. All the other handles must have been dropped.
    pub(crate) fn into_list(self) -> CommandList {
        Rc::try_unwrap(self.0)
            .expect("the draw writer is gone")
            .into_inner()
    }
}

impl DrawPassCommandList for SharedCommandList {
    fn bind_append_data_buffer(&mut self, append_attribs: BindBufferInfo) {
        self.0.borrow_mut().bind_append_data_buffer(append_attribs);
    }

    fn bind_static_data_buffer(&mut self, static_attribs: BindBufferInfo) {
        self.0.borrow_mut().bind_static_data_buffer(static_attribs);
    }

    fn bind_index_buffer(&mut self, indices: BindBufferInfo) {
        self.0.borrow_mut().bind_index_buffer(indices);
    }

    fn add_barrier(&mut self, barrier: BarrierType) {
        self.0.borrow_mut().add_barrier(barrier);
    }

    fn draw(
        &mut self,
        primitive: crate::graphite::draw_types::PrimitiveType,
        base_vertex: u32,
        vertex_count: u32,
    ) {
        self.0
            .borrow_mut()
            .draw(primitive, base_vertex, vertex_count);
    }

    fn draw_indexed(
        &mut self,
        primitive: crate::graphite::draw_types::PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
    ) {
        self.0
            .borrow_mut()
            .draw_indexed(primitive, base_index, index_count, base_vertex);
    }

    fn draw_instanced(
        &mut self,
        primitive: crate::graphite::draw_types::PrimitiveType,
        base_vertex: u32,
        vertex_count: u32,
        base_instance: u32,
        instance_count: u32,
    ) {
        self.0.borrow_mut().draw_instanced(
            primitive,
            base_vertex,
            vertex_count,
            base_instance,
            instance_count,
        );
    }

    fn draw_indexed_instanced(
        &mut self,
        primitive: crate::graphite::draw_types::PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
        base_instance: u32,
        instance_count: u32,
    ) {
        self.0.borrow_mut().draw_indexed_instanced(
            primitive,
            base_index,
            index_count,
            base_vertex,
            base_instance,
            instance_count,
        );
    }
}

/// Creates a draw writer over `list` and `buffer_manager`.
pub(crate) fn make_draw_writer<'a>(
    list: &'a mut SharedCommandList,
    buffer_manager: &'a DrawBufferManager,
) -> DrawWriter<'a> {
    DrawWriter::new(list, buffer_manager)
}

/// The arguments of `recordDraw()` that don't vary between the two draw lists.
#[derive(Debug)]
pub struct RecordDrawArgs<'a> {
    /// `renderer`.
    pub renderer: &'a Renderer,
    /// `localToDevice`.
    pub local_to_device: &'a Transform,
    /// `geometry`.
    pub geometry: &'a Geometry,
    /// `clip`.
    pub clip: &'a Clip,
    /// `ordering`.
    pub ordering: DrawOrder,
    /// `paintID`.
    pub paint_id: UniquePaintParamsID,
    /// `dstUsage`.
    pub dst_usage: DstUsage,
    /// `barrierBeforeDraws`.
    pub barrier_before_draws: BarrierType,
    /// `stroke`.
    pub stroke: Option<&'a StrokeStyle>,
}

/// What `snapDrawPass()` needs from the draw context.
pub struct SnapArgs<'a, 'r> {
    /// `recorder->priv()`.
    pub recorder: &'a RecorderPriv<'r>,
    /// `drawContext->recordDependency`, which `StorageContext::finalize()` calls.
    pub record_dependency: &'a mut dyn FnMut(TaskRef),
    /// `target`.
    pub target: Arc<TextureProxy>,
    /// `targetInfo.dimensions()`.
    pub target_dimensions: ISize,
    /// `dstReadStrategy`.
    pub dst_read_strategy: DstReadStrategy,
}

impl std::fmt::Debug for SnapArgs<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnapArgs")
            .field("target_dimensions", &self.target_dimensions)
            .field("dst_read_strategy", &self.dst_read_strategy)
            .finish_non_exhaustive()
    }
}

/// The draws recorded into a `DrawContext`, as either of the two implementations.
// Port of: src/gpu/graphite/DrawListBase.h#L33-L96 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawListBase")]
#[derive(Debug)]
pub enum DrawListBase {
    /// `DrawList`: sorts the draws when the pass is snapped.
    List(DrawList),
    /// `DrawListLayer`: collects the draws in layers as they are recorded.
    Layer(DrawListLayer),
}

/// `step->writeUniformsAndTextures(params, gatherer)`. Each step declares and validates its own
/// uniforms (`UniformExpectationsValidator uev(gatherer, this->uniforms())`), as in C++.
// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L212-L218 (chrome/m156), and
// the other steps' `writeUniformsAndTextures()`
pub(crate) fn write_step_uniforms_and_textures(
    step: &dyn RenderStep,
    params: &crate::graphite::draw_params::DrawParams,
    gatherer: &mut PipelineDataGatherer,
) {
    step.write_uniforms_and_textures(params, gatherer);
}

impl DrawListBase {
    fn state(&self) -> &DrawListBaseState {
        match self {
            DrawListBase::List(list) => list.state(),
            DrawListBase::Layer(list) => list.state(),
        }
    }

    /// `recordDraw(...)`: the `DrawParams` of the draw and the `Layer` it was recorded in (both
    /// `None` for the sort-based `DrawList`).
    ///
    /// Without storage uniforms in the renderer's steps, `storage_context` is not touched.
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[doc(alias = "recordDraw")]
    pub fn record_draw(
        &mut self,
        args: &RecordDrawArgs<'_>,
        gatherer: &mut PipelineDataGatherer,
        storage_context: Option<&mut StorageContext>,
        last_insertion: Option<LayerId>,
    ) -> (Option<DrawParamsId>, Option<LayerId>) {
        match self {
            DrawListBase::List(list) => list.record_draw(args, gatherer, storage_context),
            DrawListBase::Layer(list) => {
                list.record_draw(args, gatherer, storage_context, last_insertion)
            }
        }
    }

    /// `snapDrawPass(...)`: converts the recorded draws into a pass, leaving the list reset to
    /// the load op. `None` if the pass could not be created.
    #[doc(alias = "snapDrawPass")]
    pub fn snap_draw_pass(
        &mut self,
        storage_context: Option<&mut StorageContext>,
        args: SnapArgs<'_, '_>,
    ) -> Option<DrawPass> {
        match self {
            DrawListBase::List(list) => list.snap_draw_pass(storage_context, args),
            DrawListBase::Layer(list) => list.snap_draw_pass(storage_context, args),
        }
    }

    /// `reset(op, clearColor)`.
    pub fn reset(&mut self, op: LoadOp, clear_color: skia_rust_core::color::Color4f) {
        match self {
            DrawListBase::List(list) => list.reset(op, clear_color),
            DrawListBase::Layer(list) => list.reset(op, clear_color),
        }
    }

    /// `renderStepCount()`.
    #[doc(alias = "renderStepCount")]
    #[must_use]
    pub fn render_step_count(&self) -> usize {
        self.state().render_step_count
    }

    /// `modifiesTarget()`.
    #[doc(alias = "modifiesTarget")]
    #[must_use]
    pub fn modifies_target(&self) -> bool {
        self.render_step_count() > 0 || self.state().load_op == LoadOp::Clear
    }

    /// `samplesTexture(texture)`.
    #[doc(alias = "samplesTexture")]
    #[must_use]
    pub fn samples_texture(&self, texture: &Arc<TextureProxy>) -> bool {
        self.state().texture_data_cache.has_texture(texture)
    }

    /// `dstReadBounds()`.
    #[doc(alias = "dstReadBounds")]
    #[must_use]
    pub fn dst_read_bounds(&self) -> Rect {
        self.state().dst_read_bounds
    }

    /// `passBounds()`.
    #[doc(alias = "passBounds")]
    #[must_use]
    pub fn pass_bounds(&self) -> Rect {
        self.state().pass_bounds
    }

    /// `drawsReadDst()`.
    #[doc(alias = "drawsReadDst")]
    #[must_use]
    pub fn draws_read_dst(&self) -> bool {
        !self.state().dst_read_bounds.is_empty_negative_or_nan()
    }

    /// `drawsRequireMSAA()`.
    #[doc(alias = "drawsRequireMSAA")]
    #[must_use]
    pub fn draws_require_msaa(&self) -> bool {
        self.state().requires_msaa
    }

    /// `depthStencilFlags()`.
    #[doc(alias = "depthStencilFlags")]
    #[must_use]
    pub fn depth_stencil_flags(&self) -> DepthStencilFlags {
        self.state().depth_stencil_flags
    }

    /// `loadOp()`: the load op the next pass will use.
    #[must_use]
    pub fn load_op(&self) -> LoadOp {
        self.state().load_op
    }

    /// The paint order of the layer `id` (`Layer::fOrder`); `None` for the sort-based
    /// `DrawList`, which has no layers.
    #[must_use]
    pub fn layer_order(
        &self,
        id: LayerId,
    ) -> Option<crate::graphite::draw_order::CompressedPaintersOrder> {
        match self {
            DrawListBase::List(_) => None,
            DrawListBase::Layer(list) => Some(list.layer_order(id)),
        }
    }

    /// Updates a recorded depth-only clip draw (`DrawListLayer` only; a no-op for `DrawList`).
    pub fn update_clip_draw(
        &mut self,
        id: DrawParamsId,
        order: crate::graphite::draw_order::DrawOrder,
        draw_bounds: Rect,
        scissor: skia_rust_core::rect::IRect,
    ) {
        if let DrawListBase::Layer(list) = self {
            list.update_clip_draw(id, order, draw_bounds, scissor);
        }
    }

    /// The `DrawParams` of a recorded draw, if the list keeps them (`DrawListLayer`).
    #[must_use]
    pub fn draw_params(
        &self,
        id: DrawParamsId,
    ) -> Option<&crate::graphite::draw_params::DrawParams> {
        match self {
            DrawListBase::List(_) => None,
            DrawListBase::Layer(list) => Some(list.draw_params(id)),
        }
    }
}

// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawPass.h, src/gpu/graphite/DrawPass.cpp,
//                   src/gpu/graphite/DrawCommands.h

//! [`DrawPass`]: the immutable result of snapping a draw list, and its command list.
//!
//! A `DrawPass` holds the commands to replay (`DrawPassCommands::List`, here [`CommandList`] of
//! [`DrawPassCommand`]s), the de-duplicated pipeline descriptions the commands index into, and
//! the textures the commands sample. Its `GraphicsPipelineDesc`s become handles in
//! `prepareResources()` and pipelines in `addResourceRefs()`.
//!
//! Deviations from the C++:
//!
//! - The commands are an enum in a `Vec` instead of arena-allocated structs in a block list. The
//!   textures of a `BindTexturesAndSamplers` command are `Arc`s (Skia holds raw pointers and
//!   relies on `fSampledTextures` to keep the proxies alive).
//! - The pipeline manager is a [`PipelineHandleFactory`] the draw list captures from the shared
//!   context when it snaps the pass (Skia reaches it through `resourceProvider->sharedContext()`).
//! - `addResourceRefs()` does not call `commandBuffer->trackResource(pipeline)` yet: a
//!   `GraphicsPipeline` is not a tracked `Resource` until G11b.

use std::sync::Arc;

use skia_rust_core::rect::IRect;

use crate::gpu::sk_log::skia_log_w;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::command_buffer::{CommandBuffer, Scissor};
use crate::graphite::draw_types::{BarrierType, PipelineStageFlags, PrimitiveType, UniformSlot};
use crate::graphite::draw_writer::DrawPassCommandList;
use crate::graphite::graphics_pipeline::{GraphicsPipeline, PipelineCreationFlags};
use crate::graphite::graphics_pipeline_desc::{
    GraphicsPipelineDesc, GraphicsPipelineHandle, PipelineHandleFactory,
};
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::resource_types::{LoadOp, SamplerDesc, StoreOp};
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::storage_context::StorageContextResult;
use crate::graphite::task::render_pass_task::DrawPass as DrawPassTrait;
use crate::graphite::texture_proxy::TextureProxy;

/// One command of a draw pass (the structs of `DrawPassCommands`).
///
/// The enum order is the order of `SKGPU_DRAW_PASS_COMMAND_TYPES`.
// Port of: src/gpu/graphite/DrawCommands.h#L67-L169 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawPassCommands")]
#[derive(Clone, Debug)]
pub enum DrawPassCommand {
    /// `BindGraphicsPipeline`: an index into the pass's pipelines.
    BindGraphicsPipeline {
        /// `fPipelineIndex`.
        pipeline_index: u32,
    },
    /// `SetBlendConstants`.
    SetBlendConstants {
        /// `fBlendConstants`.
        blend_constants: [f32; 4],
    },
    /// `BindUniformBuffer`.
    BindUniformBuffer {
        /// `fInfo`.
        info: BindBufferInfo,
        /// `fSlot`.
        slot: UniformSlot,
    },
    /// `BindStaticDataBuffer`.
    BindStaticDataBuffer {
        /// `fStaticData`.
        static_data: BindBufferInfo,
    },
    /// `BindAppendDataBuffer`.
    BindAppendDataBuffer {
        /// `fAppendData`.
        append_data: BindBufferInfo,
    },
    /// `BindIndirectBuffer`.
    BindIndirectBuffer {
        /// `fIndirect`.
        indirect: BindBufferInfo,
    },
    /// `BindIndexBuffer`.
    BindIndexBuffer {
        /// `fIndices`.
        indices: BindBufferInfo,
    },
    /// `BindTexturesAndSamplers`: `textures[i]` is sampled with `samplers[i]`.
    BindTexturesAndSamplers {
        /// `fTextures`.
        textures: Vec<Arc<TextureProxy>>,
        /// `fSamplers`.
        samplers: Vec<SamplerDesc>,
    },
    /// `SetScissor`.
    SetScissor {
        /// `fScissor`.
        scissor: Scissor,
    },
    /// `Draw`.
    Draw {
        /// `fType`.
        primitive: PrimitiveType,
        /// `fBaseVertex`.
        base_vertex: u32,
        /// `fVertexCount`.
        vertex_count: u32,
    },
    /// `DrawIndexed`.
    DrawIndexed {
        /// `fType`.
        primitive: PrimitiveType,
        /// `fBaseIndex`.
        base_index: u32,
        /// `fIndexCount`.
        index_count: u32,
        /// `fBaseVertex`.
        base_vertex: u32,
    },
    /// `DrawInstanced`.
    DrawInstanced {
        /// `fType`.
        primitive: PrimitiveType,
        /// `fBaseVertex`.
        base_vertex: u32,
        /// `fVertexCount`.
        vertex_count: u32,
        /// `fBaseInstance`.
        base_instance: u32,
        /// `fInstanceCount`.
        instance_count: u32,
    },
    /// `DrawIndexedInstanced`.
    DrawIndexedInstanced {
        /// `fType`.
        primitive: PrimitiveType,
        /// `fBaseIndex`.
        base_index: u32,
        /// `fIndexCount`.
        index_count: u32,
        /// `fBaseVertex`.
        base_vertex: u32,
        /// `fBaseInstance`.
        base_instance: u32,
        /// `fInstanceCount`.
        instance_count: u32,
    },
    /// `DrawIndirect`.
    DrawIndirect {
        /// `fType`.
        primitive: PrimitiveType,
    },
    /// `DrawIndexedIndirect`.
    DrawIndexedIndirect {
        /// `fType`.
        primitive: PrimitiveType,
    },
    /// `AddBarrier`.
    AddBarrier {
        /// `fType`.
        barrier: BarrierType,
    },
}

/// The list of commands of a draw pass (`DrawPassCommands::List`).
// Port of: src/gpu/graphite/DrawCommands.h#L171-L271 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawPassCommands::List")]
#[derive(Clone, Debug, Default)]
pub struct CommandList {
    commands: Vec<DrawPassCommand>,
}

impl CommandList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `count()`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.commands.len()
    }

    /// `commands()`: the commands in order.
    #[must_use]
    pub fn commands(&self) -> &[DrawPassCommand] {
        &self.commands
    }

    /// `bindGraphicsPipeline(pipelineIndex)`.
    #[doc(alias = "bindGraphicsPipeline")]
    pub fn bind_graphics_pipeline(&mut self, pipeline_index: u32) {
        self.commands
            .push(DrawPassCommand::BindGraphicsPipeline { pipeline_index });
    }

    /// `setBlendConstants(blendConstants)`.
    #[doc(alias = "setBlendConstants")]
    pub fn set_blend_constants(&mut self, blend_constants: [f32; 4]) {
        self.commands
            .push(DrawPassCommand::SetBlendConstants { blend_constants });
    }

    /// `bindUniformBuffer(info, slot)`.
    #[doc(alias = "bindUniformBuffer")]
    pub fn bind_uniform_buffer(&mut self, info: BindBufferInfo, slot: UniformSlot) {
        self.commands
            .push(DrawPassCommand::BindUniformBuffer { info, slot });
    }

    /// `bindDeferredTexturesAndSamplers(numTexSamplers)` and the writes into the arrays it
    /// returns, in one call: `textures[i]` is sampled with `samplers[i]`.
    #[doc(alias = "bindDeferredTexturesAndSamplers")]
    pub fn bind_textures_and_samplers(
        &mut self,
        textures: Vec<Arc<TextureProxy>>,
        samplers: Vec<SamplerDesc>,
    ) {
        debug_assert_eq!(textures.len(), samplers.len());
        self.commands
            .push(DrawPassCommand::BindTexturesAndSamplers { textures, samplers });
    }

    /// `setScissor(scissor)`.
    #[doc(alias = "setScissor")]
    pub fn set_scissor(&mut self, scissor: IRect) {
        self.commands.push(DrawPassCommand::SetScissor {
            scissor: Scissor::new(scissor),
        });
    }

    /// `bindIndirectBuffer(indirect)`.
    #[doc(alias = "bindIndirectBuffer")]
    pub fn bind_indirect_buffer(&mut self, indirect: BindBufferInfo) {
        self.commands
            .push(DrawPassCommand::BindIndirectBuffer { indirect });
    }

    /// `drawIndirect(type)`.
    #[doc(alias = "drawIndirect")]
    pub fn draw_indirect(&mut self, primitive: PrimitiveType) {
        self.commands
            .push(DrawPassCommand::DrawIndirect { primitive });
    }

    /// `drawIndexedIndirect(type)`.
    #[doc(alias = "drawIndexedIndirect")]
    pub fn draw_indexed_indirect(&mut self, primitive: PrimitiveType) {
        self.commands
            .push(DrawPassCommand::DrawIndexedIndirect { primitive });
    }
}

// The calls DrawWriter makes (`bindStaticDataBuffer`, `bindAppendDataBuffer`, `bindIndexBuffer`,
// `addBarrier`, `draw`, `drawIndexed`, `drawInstanced`, `drawIndexedInstanced`).
impl DrawPassCommandList for CommandList {
    fn bind_append_data_buffer(&mut self, append_attribs: BindBufferInfo) {
        self.commands.push(DrawPassCommand::BindAppendDataBuffer {
            append_data: append_attribs,
        });
    }

    fn bind_static_data_buffer(&mut self, static_attribs: BindBufferInfo) {
        self.commands.push(DrawPassCommand::BindStaticDataBuffer {
            static_data: static_attribs,
        });
    }

    fn bind_index_buffer(&mut self, indices: BindBufferInfo) {
        self.commands
            .push(DrawPassCommand::BindIndexBuffer { indices });
    }

    fn add_barrier(&mut self, barrier: BarrierType) {
        self.commands.push(DrawPassCommand::AddBarrier { barrier });
    }

    fn draw(&mut self, primitive: PrimitiveType, base_vertex: u32, vertex_count: u32) {
        self.commands.push(DrawPassCommand::Draw {
            primitive,
            base_vertex,
            vertex_count,
        });
    }

    fn draw_indexed(
        &mut self,
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
    ) {
        self.commands.push(DrawPassCommand::DrawIndexed {
            primitive,
            base_index,
            index_count,
            base_vertex,
        });
    }

    fn draw_instanced(
        &mut self,
        primitive: PrimitiveType,
        base_vertex: u32,
        vertex_count: u32,
        base_instance: u32,
        instance_count: u32,
    ) {
        self.commands.push(DrawPassCommand::DrawInstanced {
            primitive,
            base_vertex,
            vertex_count,
            base_instance,
            instance_count,
        });
    }

    fn draw_indexed_instanced(
        &mut self,
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
        base_instance: u32,
        instance_count: u32,
    ) {
        self.commands.push(DrawPassCommand::DrawIndexedInstanced {
            primitive,
            base_index,
            index_count,
            base_vertex,
            base_instance,
            instance_count,
        });
    }
}

/// The result of snapping a `DrawListBase`: the commands of one render pass, ready to be recorded
/// into a command buffer once its pipelines and textures are resolved.
// Port of: src/gpu/graphite/DrawPass.h#L36-L122 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawPass")]
#[derive(Debug)]
pub struct DrawPass {
    // `fCommandList`; the draw lists write into it while they snap the pass.
    pub(crate) command_list: CommandList,

    target: Arc<TextureProxy>,
    // Defined relative to the top-left corner of the surface the DrawPass renders to, and is
    // contained within its dimensions.
    pub(crate) bounds: IRect,

    ops: (LoadOp, StoreOp),
    clear_color: [f32; 4],

    // The pipelines are referenced by index in BindGraphicsPipeline, but that will index into
    // fPipelineHandles.
    pub(crate) pipeline_descs: Vec<GraphicsPipelineDesc>,
    pub(crate) pipeline_draw_areas: Vec<f32>,

    // These resources all get instantiated during prepareResources.
    pipeline_handles: Vec<GraphicsPipelineHandle>,
    pub(crate) sampled_textures: Vec<Arc<TextureProxy>>,

    // `SkDEBUGCODE(fPipelinesHaveBeenResolved)`: set in addResourceRefs.
    pipelines_have_been_resolved: bool,
    pipelines: Vec<Option<Arc<dyn GraphicsPipeline>>>,

    storage_buffer_info: BindBufferInfo,
    storage_fallback_texture: Option<Arc<TextureProxy>>,

    // In Vulkan, when storage buffers are not supported, image layout transition barriers must be
    // submitted before beginning the render pass (before any pipeline is active). So store a copy
    // of the shader stage visibility here.
    storage_buffer_stages: PipelineStageFlags,

    // The pipeline manager of the shared context (see the module docs).
    pipeline_factory: Option<Arc<dyn PipelineHandleFactory>>,
}

impl DrawPass {
    /// `DrawPass(target, ops, clearColor)`: called by the draw lists as they snap.
    // Port of: src/gpu/graphite/DrawPass.cpp#L32-L38 (chrome/m156)
    #[must_use]
    pub(crate) fn new(
        target: Arc<TextureProxy>,
        ops: (LoadOp, StoreOp),
        clear_color: [f32; 4],
        pipeline_factory: Option<Arc<dyn PipelineHandleFactory>>,
    ) -> Self {
        Self {
            command_list: CommandList::new(),
            target,
            bounds: IRect::new_empty(),
            ops,
            clear_color,
            pipeline_descs: Vec::new(),
            pipeline_draw_areas: Vec::new(),
            pipeline_handles: Vec::new(),
            sampled_textures: Vec::new(),
            pipelines_have_been_resolved: false,
            pipelines: Vec::new(),
            storage_buffer_info: BindBufferInfo::default(),
            storage_fallback_texture: None,
            storage_buffer_stages: PipelineStageFlags::NONE,
            pipeline_factory,
        }
    }

    /// `bounds()`: defined relative to the top-left corner of the surface the `DrawPass` renders
    /// to, and is contained within its dimensions.
    #[must_use]
    pub fn bounds(&self) -> IRect {
        self.bounds
    }

    /// `target()`.
    #[must_use]
    pub fn target(&self) -> &Arc<TextureProxy> {
        &self.target
    }

    /// `ops()`.
    #[must_use]
    pub fn ops(&self) -> (LoadOp, StoreOp) {
        self.ops
    }

    /// `clearColor()`.
    #[doc(alias = "clearColor")]
    #[must_use]
    pub fn clear_color(&self) -> [f32; 4] {
        self.clear_color
    }

    /// `vertexBufferSize()`.
    #[doc(alias = "vertexBufferSize")]
    #[must_use]
    pub fn vertex_buffer_size(&self) -> usize {
        0
    }

    /// `uniformBufferSize()`.
    #[doc(alias = "uniformBufferSize")]
    #[must_use]
    pub fn uniform_buffer_size(&self) -> usize {
        0
    }

    /// `commands()`.
    #[must_use]
    pub fn commands(&self) -> &[DrawPassCommand] {
        self.command_list.commands()
    }

    /// The pipeline descriptions that `BindGraphicsPipeline` indexes, before `prepareResources()`
    /// turns them into handles (`fPipelineDescs`).
    #[doc(alias = "fPipelineDescs")]
    #[must_use]
    pub fn pipeline_descs(&self) -> &[GraphicsPipelineDesc] {
        &self.pipeline_descs
    }

    /// `getPipeline(index)`: the handles aren't guaranteed to have been resolved to
    /// `GraphicsPipeline`s until after `addResourceRefs()` is called.
    #[doc(alias = "getPipeline")]
    #[must_use]
    pub fn get_pipeline(&self, index: usize) -> Option<&Arc<dyn GraphicsPipeline>> {
        debug_assert!(self.pipelines_have_been_resolved);
        self.pipelines.get(index).and_then(Option::as_ref)
    }

    /// `pipelineHandles()`.
    #[doc(alias = "pipelineHandles")]
    #[must_use]
    pub fn pipeline_handles(&self) -> &[GraphicsPipelineHandle] {
        &self.pipeline_handles
    }

    /// `sampledTextures()`: proxies are always valid but may not be instantiated until after
    /// `prepareResources()` is called.
    #[doc(alias = "sampledTextures")]
    #[must_use]
    pub fn sampled_textures(&self) -> &[Arc<TextureProxy>] {
        &self.sampled_textures
    }

    /// `storageBufferInfo()`: the `StorageContext` buffer bound by the pass.
    #[doc(alias = "storageBufferInfo")]
    #[must_use]
    pub fn storage_buffer_info(&self) -> &BindBufferInfo {
        &self.storage_buffer_info
    }

    /// `storageFallbackTexture()`.
    #[doc(alias = "storageFallbackTexture")]
    #[must_use]
    pub fn storage_fallback_texture(&self) -> Option<&Arc<TextureProxy>> {
        self.storage_fallback_texture.as_ref()
    }

    /// `storageBufferStages()`.
    #[doc(alias = "storageBufferStages")]
    #[must_use]
    pub fn storage_buffer_stages(&self) -> PipelineStageFlags {
        self.storage_buffer_stages
    }

    /// `setStorageResult(result)`.
    // Port of: src/gpu/graphite/DrawPass.cpp#L127-L135 (chrome/m156)
    #[doc(alias = "setStorageResult")]
    pub(crate) fn set_storage_result(&mut self, result: StorageContextResult) {
        match result {
            StorageContextResult::Buffer(info) => {
                debug_assert!(info.is_valid());
                self.storage_buffer_info = info;
            }
            StorageContextResult::Texture(texture) => {
                self.storage_fallback_texture = Some(texture);
            }
        }
    }

    /// `prepareResources()`: instantiates and prepares any resources used by the `DrawPass` that
    /// require the `Recorder`'s `ResourceProvider`. This includes things like `GraphicsPipeline`s,
    /// sampled textures, samplers, etc.
    ///
    /// Note that, due to possible threaded compilation, the pipelines are not guaranteed to be
    /// complete until `Context::insertRecording` time.
    // Port of: src/gpu/graphite/DrawPass.cpp#L42-L90 (chrome/m156)
    #[doc(alias = "prepareResources")]
    pub fn prepare_resources(
        &mut self,
        _resource_provider: &mut ResourceProvider,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        render_pass_desc: &RenderPassDesc,
    ) -> bool {
        self.pipeline_handles.reserve(self.pipeline_descs.len());
        for pipeline_desc in &self.pipeline_descs {
            let handle = match &self.pipeline_factory {
                Some(factory) => factory.create_handle(
                    runtime_dict,
                    pipeline_desc,
                    render_pass_desc,
                    PipelineCreationFlags::NONE,
                ),
                // Without a pipeline manager no pipeline can be created; addResourceRefs() then
                // drops the pass.
                None => GraphicsPipelineHandle::from_pipeline(None),
            };
            self.pipeline_handles.push(handle);
        }

        // The DrawPass may be long-lived on a Recording and we no longer need the
        // GraphicPipelineDescs once we've created pipeline handles, so we drop the storage for
        // them here.
        self.pipeline_descs.clear();

        for sampled_texture in &self.sampled_textures {
            // It should not have been possible to draw an Image that has an invalid texture info
            debug_assert!(sampled_texture.texture_info().is_valid());
            // Tasks should have been ordered to instantiate any scratch textures already, or any
            // client-owned image will have been instantiated at creation. However, if a
            // TextureProxy was cached for reuse across Recordings, it's possible that the
            // initializing Recording failed, leaving the TextureProxy in a bad state (and
            // currently with no way to reconstruct the tasks required to initialize it).
            // TODO(b/409888039): Once TextureProxies track their dependendent tasks to include in
            // all Recordings, this "should" be able to changed to asserts.
            if !sampled_texture.is_instantiated() && !sampled_texture.is_lazy() {
                skia_log_w!(
                    "Cannot sample from an uninstantiated TextureProxy, label {}",
                    sampled_texture.label()
                );
                return false;
            }
        }

        if let Some(fallback) = &self.storage_fallback_texture {
            debug_assert!(fallback.texture_info().is_valid());
            if !fallback.is_instantiated() && !fallback.is_lazy() {
                skia_log_w!(
                    "Cannot sample from an uninstantiated TextureProxy, label {}",
                    fallback.label()
                );
                return false;
            }
        }

        true
    }

    /// `addResourceRefs(resourceProvider, commandBuffer)`: resolves the pipeline handles and
    /// tracks the resources the pass uses on `command_buffer`. False if a pipeline could not be
    /// created, which drops the pass.
    // Port of: src/gpu/graphite/DrawPass.cpp#L92-L125 (chrome/m156)
    #[doc(alias = "addResourceRefs")]
    #[must_use]
    pub fn add_resource_refs(&mut self, command_buffer: &mut dyn CommandBuffer) -> bool {
        debug_assert_eq!(self.pipeline_handles.len(), self.pipeline_draw_areas.len());
        self.pipelines.clear();
        for handle in &self.pipeline_handles {
            let pipeline = match &self.pipeline_factory {
                Some(factory) => factory.resolve_handle(handle),
                None => handle.pipeline_or_null(),
            };
            let Some(pipeline) = pipeline else {
                skia_log_w!("Failed to create Pipeline for draw in RenderPass. Dropping draw!");
                return false;
            };

            self.storage_buffer_stages |= pipeline.storage_buffer_stages();

            // `commandBuffer->trackResource(std::move(pipeline))` comes with the pipeline's
            // resource handle (G11b).
            self.pipelines.push(Some(pipeline));
        }

        self.pipelines_have_been_resolved = true;

        for sampled_texture in &self.sampled_textures {
            if let Some(texture) = sampled_texture.ref_texture() {
                command_buffer.track_resource(texture.into_any());
            }
        }

        if let Some(fallback) = &self.storage_fallback_texture
            && let Some(texture) = fallback.ref_texture()
        {
            command_buffer.track_resource(texture.into_any());
        }

        true
    }
}

impl DrawPassTrait for DrawPass {
    fn bounds(&self) -> IRect {
        self.bounds
    }

    fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        render_pass_desc: &RenderPassDesc,
    ) -> bool {
        DrawPass::prepare_resources(self, resource_provider, runtime_dict, render_pass_desc)
    }

    fn pipelines(&self) -> Vec<Option<Arc<dyn GraphicsPipeline>>> {
        if self.pipelines_have_been_resolved {
            self.pipelines.clone()
        } else {
            self.pipeline_handles
                .iter()
                .map(GraphicsPipelineHandle::pipeline_or_null)
                .collect()
        }
    }

    fn sampled_textures(&self) -> &[Arc<TextureProxy>] {
        &self.sampled_textures
    }

    fn storage_fallback_texture(&self) -> Option<&Arc<TextureProxy>> {
        self.storage_fallback_texture.as_ref()
    }

    fn target(&self) -> Option<&Arc<TextureProxy>> {
        Some(&self.target)
    }

    fn commands(&self) -> &[DrawPassCommand] {
        self.command_list.commands()
    }

    fn storage_buffer_info(&self) -> Option<&BindBufferInfo> {
        Some(&self.storage_buffer_info)
    }

    fn storage_buffer_stages(&self) -> PipelineStageFlags {
        self.storage_buffer_stages
    }

    fn ops(&self) -> (LoadOp, StoreOp) {
        self.ops
    }

    fn clear_color(&self) -> [f32; 4] {
        self.clear_color
    }

    fn add_resource_refs(&mut self, command_buffer: &mut dyn CommandBuffer) -> bool {
        DrawPass::add_resource_refs(self, command_buffer)
    }

    fn pipeline_descs(&self) -> &[GraphicsPipelineDesc] {
        &self.pipeline_descs
    }
}

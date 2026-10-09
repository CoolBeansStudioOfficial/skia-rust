// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/CommandBuffer.h (the calls tasks make),
//                   src/gpu/graphite/CommandTypes.h (BufferTextureCopyData)

//! The seam between tasks and the command buffer.
//!
//! `CommandBuffer` (the backend-neutral half plus the wgpu half) is ported with G9b/G11c as one
//! concrete struct. [`CommandBuffer`] lists the calls the tasks, the buffer managers and
//! `Recording` make; the implementation keeps the C++ semantics (each call returns false on
//! failure, and the buffers and textures passed by handle are tracked until the work finishes).

use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;

use crate::gpu::gpu_types::{GpuStats, Protected};
use crate::gpu::ref_cnted_callback::RefCntedCallback;
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::buffer::Buffer;
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::resource::{AnyResourceRef, CommandBufferRef, Resource, ResourceRef};
use crate::graphite::resource_types::{LoadOp, SamplerDesc};
use crate::graphite::sampler::Sampler;
use crate::graphite::task::compute_task::DispatchGroup;
use crate::graphite::task::render_pass_task::DrawPass;
use crate::graphite::texture::Texture;

/// Specifies a single region for copying, either from buffer to texture, or vice versa.
// Port of: src/gpu/graphite/CommandTypes.h#L16-L22 (chrome/m156)
#[doc(alias = "skgpu::graphite::BufferTextureCopyData")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferTextureCopyData {
    /// `fBufferOffset`.
    pub buffer_offset: usize,
    /// `fBufferRowBytes`.
    pub buffer_row_bytes: usize,
    /// `fRect`.
    pub rect: IRect,
    /// `fMipLevel`.
    pub mip_level: u32,
}

/// Specifies a scissor, which can only be subsequently queried given a translation and clip which
/// are assumed to be applied to all commands in the render pass in which the scissor is set.
// Port of: src/gpu/graphite/CommandTypes.h#L28-L42 (chrome/m156)
#[doc(alias = "skgpu::graphite::Scissor")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scissor {
    rect: IRect,
}

impl Scissor {
    /// `Scissor(rect)`.
    #[must_use]
    pub const fn new(rect: IRect) -> Self {
        Self { rect }
    }

    /// `getRect(replayTranslation, replayClip)`.
    #[doc(alias = "getRect")]
    #[must_use]
    pub fn get_rect(&self, replay_translation: IPoint, replay_clip: IRect) -> IRect {
        let rect = self.rect.with_offset(replay_translation);
        IRect::intersect(&rect, &replay_clip).unwrap_or_else(IRect::new_empty)
    }
}

/// The `CommandBuffer` calls made by tasks and managers.
///
/// Buffers and textures that Skia passes as `const T*` (not owned by the task) are passed as
/// `&Arc<Resource<T>>`, the non-owning handle ([`ResourceRef::as_arc`]); those passed as
/// `sk_sp<T>` are passed as [`ResourceRef`]s.
// Port of: src/gpu/graphite/CommandBuffer.h (chrome/m156)
pub trait CommandBuffer {
    /// `trackResource()`: keeps `resource` alive (with a command buffer ref) until the GPU
    /// work finishes.
    #[doc(alias = "trackResource")]
    fn track_resource(&mut self, resource: AnyResourceRef);

    /// `addFinishedProc()`.
    #[doc(alias = "addFinishedProc")]
    fn add_finished_proc(&mut self, finished_proc: Arc<RefCntedCallback>);

    /// `setReplayTranslationAndClip()`: false if the clip does not intersect the render target
    /// bounds, so the whole render pass can be skipped.
    #[doc(alias = "setReplayTranslationAndClip")]
    fn set_replay_translation_and_clip(
        &mut self,
        translation: IPoint,
        clip: IRect,
        render_target_bounds: IRect,
    ) -> bool;

    /// `addRenderPass()`.
    #[doc(alias = "addRenderPass")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn add_render_pass(
        &mut self,
        render_pass_desc: &RenderPassDesc,
        color_texture: ResourceRef<Texture>,
        resolve_texture: Option<ResourceRef<Texture>>,
        depth_stencil_texture: Option<ResourceRef<Texture>>,
        dst_copy: Option<&Arc<Resource<Texture>>>,
        dst_read_bounds: IRect,
        resolve_offset: IPoint,
        viewport_dims: ISize,
        draw_passes: &[Box<dyn DrawPass>],
    ) -> bool;

    /// `addComputePass()`.
    #[doc(alias = "addComputePass")]
    fn add_compute_pass(&mut self, dispatches: &[Box<dyn DispatchGroup>]) -> bool;

    /// `copyBufferToBuffer()`.
    #[doc(alias = "copyBufferToBuffer")]
    fn copy_buffer_to_buffer(
        &mut self,
        src_buffer: &Arc<Resource<Buffer>>,
        src_offset: usize,
        dst_buffer: ResourceRef<Buffer>,
        dst_offset: usize,
        size: usize,
    ) -> bool;

    /// `copyTextureToBuffer()`.
    #[doc(alias = "copyTextureToBuffer")]
    fn copy_texture_to_buffer(
        &mut self,
        texture: ResourceRef<Texture>,
        src_rect: IRect,
        buffer: ResourceRef<Buffer>,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    ) -> bool;

    /// `copyBufferToTexture()`.
    #[doc(alias = "copyBufferToTexture")]
    fn copy_buffer_to_texture(
        &mut self,
        buffer: &Arc<Resource<Buffer>>,
        texture: ResourceRef<Texture>,
        copy_data: &[BufferTextureCopyData],
    ) -> bool;

    /// `copyTextureToTexture()`.
    #[doc(alias = "copyTextureToTexture")]
    fn copy_texture_to_texture(
        &mut self,
        src: ResourceRef<Texture>,
        src_rect: IRect,
        dst: ResourceRef<Texture>,
        dst_point: IPoint,
        dst_level: i32,
    ) -> bool;

    /// `synchronizeBufferToCpu()`.
    #[doc(alias = "synchronizeBufferToCpu")]
    fn synchronize_buffer_to_cpu(&mut self, buffer: ResourceRef<Buffer>) -> bool;

    /// `clearBuffer()`.
    #[doc(alias = "clearBuffer")]
    fn clear_buffer(&mut self, buffer: &Arc<Resource<Buffer>>, offset: usize, size: usize) -> bool;

    /// `isProtected()`.
    #[doc(alias = "isProtected")]
    fn is_protected(&self) -> Protected;

    /// `hasWork()`.
    #[doc(alias = "hasWork")]
    fn has_work(&self) -> bool;

    /// `setNewCommandBufferResources()`: the backend's setup when the queue manager takes the
    /// command buffer from its pool.
    #[doc(alias = "setNewCommandBufferResources")]
    fn set_new_command_buffer_resources(&mut self) -> bool;

    /// `resetCommandBuffer()`.
    #[doc(alias = "resetCommandBuffer")]
    fn reset_command_buffer(&mut self);

    /// `callFinishedProcs(success)`.
    #[doc(alias = "callFinishedProcs")]
    fn call_finished_procs(&mut self, success: bool);

    /// `addBuffersToAsyncMapOnSubmit()`.
    #[doc(alias = "addBuffersToAsyncMapOnSubmit")]
    fn add_buffers_to_async_map_on_submit(&mut self, buffers: &[ResourceRef<Buffer>]);

    /// `buffersToAsyncMapOnSubmit()`.
    #[doc(alias = "buffersToAsyncMapOnSubmit")]
    fn buffers_to_async_map_on_submit(&self) -> &[ResourceRef<Buffer>];
}

/// `SkIRect::intersect(r)`: intersects `r` into `rect` and returns true, or returns false and
/// leaves `rect` unchanged when they do not overlap.
fn intersect_in_place(rect: &mut IRect, other: &IRect) -> bool {
    match IRect::intersect(rect, other) {
        Some(intersection) => {
            *rect = intersection;
            true
        }
        None => false,
    }
}

/// `SkIRect::join(r)`: the bounding rect of `rect` and `other`.
fn join_in_place(rect: &mut IRect, other: &IRect) {
    *rect = IRect::join(rect, other);
}

/// The replay state a `CommandBuffer` keeps between `setReplayTranslationAndClip` and the render
/// passes it records (`fRenderTargetBounds`, `fRenderAreaBounds`, `fReplayTranslation`,
/// `fDstCopy`, `fDstReadBounds`). Backends read it in their hooks.
// Port of: src/gpu/graphite/CommandBuffer.h#L140-L165 (chrome/m156)
#[derive(Clone, Debug, Default)]
pub struct ReplayState {
    /// `fRenderTargetBounds`.
    pub render_target_bounds: IRect,
    /// `fRenderAreaBounds`: the union of the pass bounds, clipped to the render target.
    pub render_area_bounds: IRect,
    /// `fReplayTranslation`.
    pub replay_translation: IPoint,
    /// `fDstCopy.first`: the texture a draw in the pass samples as its dst copy.
    pub dst_copy: Option<Arc<Resource<Texture>>>,
    /// `fDstCopy.second`: the nearest-neighbor sampler for `dst_copy`, tracked by the command
    /// buffer.
    pub dst_copy_sampler: Option<Arc<Resource<Sampler>>>,
    /// `fDstReadBounds`.
    pub dst_read_bounds: IRect,
}

/// The arguments of a render pass, as the backend's `onAddRenderPass` receives them. The replay
/// state comes separately as a [`ReplayState`].
// Port of: src/gpu/graphite/CommandBuffer.h (the addRenderPass arguments) (chrome/m156)
#[derive(Debug)]
pub struct RenderPassCall<'a> {
    /// `renderPassDesc`.
    pub render_pass_desc: &'a RenderPassDesc,
    /// `colorTexture`.
    pub color_texture: &'a ResourceRef<Texture>,
    /// `resolveTexture`.
    pub resolve_texture: Option<&'a ResourceRef<Texture>>,
    /// `depthStencilTexture`.
    pub depth_stencil_texture: Option<&'a ResourceRef<Texture>>,
    /// `resolveOffset`.
    pub resolve_offset: IPoint,
    /// `viewport`: the replay translation applied to `viewportDims` (not intersected).
    pub viewport: IRect,
    /// `drawPasses`.
    pub draw_passes: &'a [Box<dyn DrawPass>],
}

/// The hooks the backend half of a command buffer implements (the `on*` virtuals of
/// `CommandBuffer.h`). [`CommandBufferCore`] calls them after the neutral checks.
// Port of: src/gpu/graphite/CommandBuffer.h (the virtual on* methods, chrome/m156)
pub trait CommandBufferBackend {
    /// `setNewCommandBufferResources()`: called when the command buffer is taken from the
    /// queue manager's pool.
    #[doc(alias = "setNewCommandBufferResources")]
    fn set_new_command_buffer_resources(&mut self) -> bool;

    /// `onResetCommandBuffer()`.
    #[doc(alias = "onResetCommandBuffer")]
    fn on_reset_command_buffer(&mut self);

    /// `gpuStats()`: `None` unless the backend collects stats.
    #[doc(alias = "gpuStats")]
    fn gpu_stats(&self) -> Option<GpuStats> {
        None
    }

    /// `onAddRenderPass()`.
    #[doc(alias = "onAddRenderPass")]
    fn on_add_render_pass(&mut self, state: &ReplayState, call: &RenderPassCall<'_>) -> bool;

    /// `onAddComputePass()`.
    #[doc(alias = "onAddComputePass")]
    fn on_add_compute_pass(&mut self, dispatch_groups: &[Box<dyn DispatchGroup>]) -> bool;

    /// `onCopyBufferToBuffer()`.
    #[doc(alias = "onCopyBufferToBuffer")]
    fn on_copy_buffer_to_buffer(
        &mut self,
        src_buffer: &Buffer,
        src_offset: usize,
        dst_buffer: &Buffer,
        dst_offset: usize,
        size: usize,
    ) -> bool;

    /// `onCopyTextureToBuffer()`.
    #[doc(alias = "onCopyTextureToBuffer")]
    fn on_copy_texture_to_buffer(
        &mut self,
        texture: &Texture,
        src_rect: IRect,
        buffer: &Buffer,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    ) -> bool;

    /// `onCopyBufferToTexture()`.
    #[doc(alias = "onCopyBufferToTexture")]
    fn on_copy_buffer_to_texture(
        &mut self,
        buffer: &Buffer,
        texture: &Texture,
        copy_data: &[BufferTextureCopyData],
    ) -> bool;

    /// `onCopyTextureToTexture()`.
    #[doc(alias = "onCopyTextureToTexture")]
    fn on_copy_texture_to_texture(
        &mut self,
        src: &Texture,
        src_rect: IRect,
        dst: &Texture,
        dst_point: IPoint,
        mip_level: i32,
    ) -> bool;

    /// `onSynchronizeBufferToCpu(buffer, &didResultInWork)`.
    #[doc(alias = "onSynchronizeBufferToCpu")]
    fn on_synchronize_buffer_to_cpu(
        &mut self,
        buffer: &Buffer,
        did_result_in_work: &mut bool,
    ) -> bool;

    /// `onClearBuffer()`.
    #[doc(alias = "onClearBuffer")]
    fn on_clear_buffer(&mut self, buffer: &Buffer, offset: usize, size: usize) -> bool;
}

/// The backend-neutral half of a command buffer: resource tracking, finished procs, the replay
/// state and the checks that run before a backend hook. `B` is the backend half.
// Port of: src/gpu/graphite/CommandBuffer.cpp (chrome/m156)
#[doc(alias = "skgpu::graphite::CommandBuffer")]
#[derive(Debug)]
pub struct CommandBufferCore<B: CommandBufferBackend> {
    is_protected: Protected,
    resource_provider: SharedResourceProvider,
    /// `fRenderTargetBounds`, `fRenderAreaBounds`, `fReplayTranslation`, `fDstCopy`,
    /// `fDstReadBounds`.
    state: ReplayState,
    /// `fHasWork`.
    has_work: bool,
    /// `fCommandBufferResources`.
    command_buffer_resources: Vec<CommandBufferRef>,
    /// `fFinishedProcs`.
    finished_procs: Vec<Arc<RefCntedCallback>>,
    /// `fBuffersToAsyncMap`.
    buffers_to_async_map: Vec<ResourceRef<Buffer>>,
    /// The backend half.
    backend: B,
}

impl<B: CommandBufferBackend> CommandBufferCore<B> {
    /// `CommandBuffer(isProtected)`. `resource_provider` is the one the command buffer's
    /// neutral code uses for the dst-copy sampler.
    // Port of: src/gpu/graphite/CommandBuffer.cpp#L37-L37 (chrome/m156)
    #[must_use]
    pub fn new(
        is_protected: Protected,
        resource_provider: SharedResourceProvider,
        backend: B,
    ) -> Self {
        Self {
            is_protected,
            resource_provider,
            state: ReplayState::default(),
            has_work: false,
            command_buffer_resources: Vec::new(),
            finished_procs: Vec::new(),
            buffers_to_async_map: Vec::new(),
            backend,
        }
    }

    /// `isProtected()`.
    #[must_use]
    pub fn is_protected(&self) -> Protected {
        self.is_protected
    }

    /// `hasWork()`.
    #[must_use]
    pub fn has_work(&self) -> bool {
        self.has_work
    }

    /// The replay state the backend hooks read.
    #[must_use]
    pub fn replay_state(&self) -> &ReplayState {
        &self.state
    }

    /// The backend half.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// `releaseResources()`: drops the command buffer's tracked resource refs.
    // Port of: src/gpu/graphite/CommandBuffer.cpp#L43-L47 (chrome/m156)
    #[doc(alias = "releaseResources")]
    fn release_resources(&mut self) {
        self.command_buffer_resources.clear();
    }

    /// `resetCommandBuffer()`.
    // Port of: src/gpu/graphite/CommandBuffer.cpp#L49-L59 (chrome/m156)
    #[doc(alias = "resetCommandBuffer")]
    pub fn reset_command_buffer(&mut self) {
        // The dst copy texture and sampler are kept alive by the tracked resources, so reset these
        // before we release their refs.
        self.state.dst_copy = None;
        self.state.dst_copy_sampler = None;
        self.release_resources();
        self.backend.on_reset_command_buffer();
        self.buffers_to_async_map.clear();
    }

    /// `callFinishedProcs(success)`: fails every finished proc, or hands the stats to those that
    /// want them, then forgets them.
    // Port of: src/gpu/graphite/CommandBuffer.cpp#L83-L98 (chrome/m156)
    #[doc(alias = "callFinishedProcs")]
    pub fn call_finished_procs(&mut self, success: bool) {
        if success {
            if let Some(stats) = self.backend.gpu_stats() {
                for finished_proc in &self.finished_procs {
                    if finished_proc.receives_gpu_stats() {
                        finished_proc.set_stats(&stats);
                    }
                }
            }
        } else {
            for finished_proc in &self.finished_procs {
                finished_proc.set_failure_result();
            }
        }
        self.finished_procs.clear();
    }

    /// `addBuffersToAsyncMapOnSubmit()`.
    // Port of: src/gpu/graphite/CommandBuffer.cpp#L100-L105 (chrome/m156)
    #[doc(alias = "addBuffersToAsyncMapOnSubmit")]
    pub fn add_buffers_to_async_map_on_submit(&mut self, buffers: &[ResourceRef<Buffer>]) {
        self.buffers_to_async_map.extend_from_slice(buffers);
    }

    /// `buffersToAsyncMapOnSubmit()`.
    // Port of: src/gpu/graphite/CommandBuffer.cpp#L107-L109 (chrome/m156)
    #[doc(alias = "buffersToAsyncMapOnSubmit")]
    #[must_use]
    pub fn buffers_to_async_map_on_submit(&self) -> &[ResourceRef<Buffer>] {
        &self.buffers_to_async_map
    }
}

impl<B: CommandBufferBackend> CommandBuffer for CommandBufferCore<B> {
    fn is_protected(&self) -> Protected {
        CommandBufferCore::is_protected(self)
    }

    fn has_work(&self) -> bool {
        CommandBufferCore::has_work(self)
    }

    fn set_new_command_buffer_resources(&mut self) -> bool {
        self.backend.set_new_command_buffer_resources()
    }

    fn reset_command_buffer(&mut self) {
        CommandBufferCore::reset_command_buffer(self);
    }

    fn call_finished_procs(&mut self, success: bool) {
        CommandBufferCore::call_finished_procs(self, success);
    }

    fn add_buffers_to_async_map_on_submit(&mut self, buffers: &[ResourceRef<Buffer>]) {
        CommandBufferCore::add_buffers_to_async_map_on_submit(self, buffers);
    }

    fn buffers_to_async_map_on_submit(&self) -> &[ResourceRef<Buffer>] {
        CommandBufferCore::buffers_to_async_map_on_submit(self)
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L61-L64 (chrome/m156)
    fn track_resource(&mut self, resource: AnyResourceRef) {
        self.command_buffer_resources
            .push(resource.ref_command_buffer());
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L79-L81 (chrome/m156)
    fn add_finished_proc(&mut self, finished_proc: Arc<RefCntedCallback>) {
        self.finished_procs.push(finished_proc);
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L315-L330 (chrome/m156)
    fn set_replay_translation_and_clip(
        &mut self,
        translation: IPoint,
        clip: IRect,
        render_target_bounds: IRect,
    ) -> bool {
        self.state.replay_translation = translation;
        self.state.render_target_bounds = render_target_bounds;
        // If a replay clip is defined, we intersect it with the render target bounds.
        if !clip.is_empty() {
            let offset_clip = clip.with_offset(translation);
            if !intersect_in_place(&mut self.state.render_target_bounds, &offset_clip) {
                return false;
            }
        }
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L111-L186 (chrome/m156)
    fn add_render_pass(
        &mut self,
        render_pass_desc: &RenderPassDesc,
        color_texture: ResourceRef<Texture>,
        resolve_texture: Option<ResourceRef<Texture>>,
        depth_stencil_texture: Option<ResourceRef<Texture>>,
        dst_copy: Option<&Arc<Resource<Texture>>>,
        mut dst_read_bounds: IRect,
        resolve_offset: IPoint,
        viewport_dims: ISize,
        draw_passes: &[Box<dyn DrawPass>],
    ) -> bool {
        self.state.render_area_bounds = IRect::new_empty();
        for draw_pass in draw_passes {
            join_in_place(&mut self.state.render_area_bounds, &draw_pass.bounds());
        }
        if render_pass_desc.color_attachment.load_op == LoadOp::Clear {
            let target_bounds = self.state.render_target_bounds;
            join_in_place(&mut self.state.render_area_bounds, &target_bounds);
        }
        self.state
            .render_area_bounds
            .offset(self.state.replay_translation);
        let target_bounds = self.state.render_target_bounds;
        if !intersect_in_place(&mut self.state.render_area_bounds, &target_bounds) {
            // The entire RenderPass is offscreen given the replay translation so skip adding the
            // pass at all.
            return true;
        }

        dst_read_bounds.offset(self.state.replay_translation);
        if !intersect_in_place(&mut dst_read_bounds, &target_bounds) {
            // The draws within the RenderPass that would sample from the dstCopy have been
            // translated off screen. Set the bounds to empty and let the GPU clipping do its job.
            dst_read_bounds = IRect::new_empty();
        }

        // Save the dstCopy texture so that it can be embedded into texture bind commands later
        // on. Stash the texture's full dimensions on the rect so we can calculate normalized
        // coords later.
        self.state.dst_copy = dst_copy.cloned();
        self.state.dst_read_bounds = match dst_copy {
            Some(texture) => {
                let dims = texture.dimensions();
                IRect::from_xywh(
                    dst_read_bounds.x(),
                    dst_read_bounds.y(),
                    dims.width,
                    dims.height,
                )
            }
            None => IRect::new_empty(),
        };
        if dst_copy.is_some() && self.state.dst_copy_sampler.is_none() {
            // Only look up the sampler the first time we require a dstCopy. The texture can
            // change on subsequent passes but it will always use the same nearest neighbor
            // sampling.
            let nearest_neighbor_desc = SamplerDesc::new(
                &SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                TileMode::Clamp,
            );
            let nearest_neighbor = self
                .resource_provider
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .find_or_create_compatible_sampler(&nearest_neighbor_desc);
            if let Some(nearest_neighbor) = nearest_neighbor {
                self.state.dst_copy_sampler = Some(nearest_neighbor.as_arc().clone());
                self.track_resource(nearest_neighbor.into_any());
            }
        }

        // We don't intersect the viewport with the render pass bounds or target size because it
        // just defines a linear transform, which we don't want to change just because a portion
        // of it maps to a region that gets clipped.
        let viewport = IRect::from_pt_size(self.state.replay_translation, viewport_dims);
        let call = RenderPassCall {
            render_pass_desc,
            color_texture: &color_texture,
            resolve_texture: resolve_texture.as_ref(),
            depth_stencil_texture: depth_stencil_texture.as_ref(),
            resolve_offset,
            viewport,
            draw_passes,
        };
        if !self.backend.on_add_render_pass(&self.state, &call) {
            return false;
        }
        self.track_resource(color_texture.into_any());
        if let Some(resolve_texture) = resolve_texture {
            self.track_resource(resolve_texture.into_any());
        }
        if let Some(depth_stencil_texture) = depth_stencil_texture {
            self.track_resource(depth_stencil_texture.into_any());
        }

        // We just assume if you are adding a render pass that the render pass will actually do
        // work. In theory we could have a discard load that doesn't submit any draws, clears,
        // etc. But hopefully something so trivial would be caught before getting here.
        self.has_work = true;
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L188-L198 (chrome/m156)
    fn add_compute_pass(&mut self, dispatch_groups: &[Box<dyn DispatchGroup>]) -> bool {
        if !self.backend.on_add_compute_pass(dispatch_groups) {
            return false;
        }
        self.has_work = true;
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L200-L218 (chrome/m156)
    fn copy_buffer_to_buffer(
        &mut self,
        src_buffer: &Arc<Resource<Buffer>>,
        src_offset: usize,
        dst_buffer: ResourceRef<Buffer>,
        dst_offset: usize,
        size: usize,
    ) -> bool {
        if !self.backend.on_copy_buffer_to_buffer(
            src_buffer,
            src_offset,
            &dst_buffer,
            dst_offset,
            size,
        ) {
            return false;
        }
        self.track_resource(dst_buffer.into_any());
        self.has_work = true;
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L220-L239 (chrome/m156)
    fn copy_texture_to_buffer(
        &mut self,
        texture: ResourceRef<Texture>,
        src_rect: IRect,
        buffer: ResourceRef<Buffer>,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    ) -> bool {
        if !self.backend.on_copy_texture_to_buffer(
            &texture,
            src_rect,
            &buffer,
            buffer_offset,
            buffer_row_bytes,
        ) {
            return false;
        }
        self.track_resource(texture.into_any());
        self.track_resource(buffer.into_any());
        self.has_work = true;
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L241-L259 (chrome/m156)
    fn copy_buffer_to_texture(
        &mut self,
        buffer: &Arc<Resource<Buffer>>,
        texture: ResourceRef<Texture>,
        copy_data: &[BufferTextureCopyData],
    ) -> bool {
        if !self
            .backend
            .on_copy_buffer_to_texture(buffer, &texture, copy_data)
        {
            return false;
        }
        self.track_resource(texture.into_any());
        self.has_work = true;
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L261-L284 (chrome/m156)
    fn copy_texture_to_texture(
        &mut self,
        src: ResourceRef<Texture>,
        src_rect: IRect,
        dst: ResourceRef<Texture>,
        dst_point: IPoint,
        dst_level: i32,
    ) -> bool {
        if src.texture_info().is_protected() == Protected::Yes
            && dst.texture_info().is_protected() != Protected::Yes
        {
            skia_log_e!("Can't copy from protected memory to non-protected");
            return false;
        }
        if !self
            .backend
            .on_copy_texture_to_texture(&src, src_rect, &dst, dst_point, dst_level)
        {
            return false;
        }
        self.track_resource(src.into_any());
        self.track_resource(dst.into_any());
        self.has_work = true;
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L286-L300 (chrome/m156)
    fn synchronize_buffer_to_cpu(&mut self, buffer: ResourceRef<Buffer>) -> bool {
        let mut did_result_in_work = false;
        if !self
            .backend
            .on_synchronize_buffer_to_cpu(&buffer, &mut did_result_in_work)
        {
            return false;
        }
        if did_result_in_work {
            self.track_resource(buffer.into_any());
            self.has_work = true;
        }
        true
    }

    // Port of: src/gpu/graphite/CommandBuffer.cpp#L302-L313 (chrome/m156)
    fn clear_buffer(&mut self, buffer: &Arc<Resource<Buffer>>, offset: usize, size: usize) -> bool {
        if !self.backend.on_clear_buffer(buffer, offset, size) {
            return false;
        }
        self.has_work = true;
        true
    }
}

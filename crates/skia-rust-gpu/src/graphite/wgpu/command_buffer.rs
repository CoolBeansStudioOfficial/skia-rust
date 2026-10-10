// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnCommandBuffer.h, DawnCommandBuffer.cpp

//! The wgpu half of `CommandBuffer`: a port of `DawnCommandBuffer`.
//!
//! The neutral half is [`CommandBufferCore`]. This is its backend, [`WgpuCommandBufferBackend`],
//! which records render passes, compute passes, copies and clears into a `wgpu::CommandEncoder`.
//!
//! # Differences from Dawn
//!
//! - A render pass is recorded inside one call of [`on_add_render_pass`], so the pass encoder is a
//!   local (`wgpu::RenderPass` borrows the encoder) instead of a member, and the draw pass state
//!   that Dawn keeps in members (the active pipeline, the indirect buffer) is a local
//!   [`PassState`].
//! - `DrawPass::addResourceRefs` runs through the [`ResourceTracker`] the neutral half passes in,
//!   because the hook runs inside the command buffer that owns the tracked resources.
//! - wgpu has no `ExpandResolveTexture` load op, partial resolve rects, render areas, transient
//!   attachments or undefined load/store ops (those are Dawn features, `docs/design/gpu.md`
//!   §1.3), so a render pass with a resolve texture takes Graphite's emulation path: the MSAA
//!   attachment is loaded from the resolve texture with a draw, and resolved after the pass with
//!   another. A discarded load clears, and a discarded store discards.
//! - wgpu validates `set_immediates` against the bound pipeline, so the intrinsic constants of a
//!   pass are set after every pipeline bind rather than once at the start of the pass.
//! - Stats queries (timestamps) are not ported.
//!
//! [`on_add_render_pass`]: CommandBufferBackend::on_add_render_pass

// These mirror the long functions of DawnCommandBuffer.cpp.
#![allow(clippy::too_many_lines)]
// The narrowing casts mirror the C++ conversions of `size_t` and `int` into wgpu's `u32` fields.
#![allow(clippy::cast_possible_truncation)]

use std::num::NonZeroU64;
use std::sync::{Arc, MutexGuard, PoisonError};

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;

use crate::gpu::gpu_types::Protected;
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::buffer::{BindBufferInfo, Buffer};
use crate::graphite::command_buffer::{
    BufferTextureCopyData, CommandBufferBackend, CommandBufferCore, RenderPassCall, ReplayState,
    ResourceTracker,
};
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::context_utils::collect_intrinsic_uniforms;
use crate::graphite::draw_pass::DrawPassCommand;
use crate::graphite::draw_types::{PrimitiveType, UniformSlot};
use crate::graphite::graphics_pipeline::GraphicsPipeline;
use crate::graphite::graphite_types::SampleCount;
use crate::graphite::pipeline_data::UniformDataBlock;
use crate::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::resource_types::{DstReadStrategy, Layout, LoadOp, SamplerDesc, StoreOp};
use crate::graphite::sampler::Sampler;
use crate::graphite::compute::dispatch_group::{BindingResource, DispatchGroup, GlobalSizeOrIndirect};
use crate::graphite::task::render_pass_task::DrawPass;
use crate::graphite::texture::Texture;
use crate::graphite::texture_format::{
    TextureFormat, texture_format_has_depth, texture_format_has_stencil,
};
use crate::graphite::texture_info::texture_info_priv;
use crate::graphite::uniform_manager::UniformManager;
use crate::graphite::wgpu::buffer::{WgpuBuffer, as_wgpu_buffer};
use crate::graphite::wgpu::caps::{
    COMBINED_UNIFORM_INDEX, DiscardLoadOp, DiscardStoreOp, INTRINSIC_UNIFORM_BUFFER_INDEX,
    INTRINSIC_UNIFORM_SIZE, MAX_NUM_UNIFORM_BUFFERS, STORAGE_BUFFER_INDEX,
    TEXTURE_BIND_GROUP_INDEX, UNIFORM_BUFFER_BIND_GROUP_INDEX,
};
use crate::graphite::wgpu::compute_pipeline::WgpuComputePipeline;
use crate::graphite::wgpu::graphics_pipeline::WgpuGraphicsPipeline;
use crate::graphite::wgpu::resource_provider::{
    WgpuResourceProvider, find_or_create_intrinsic_bind_buffer_info, wgpu_backend,
};
use crate::graphite::wgpu::sampler::{WgpuSampler, as_wgpu_sampler};
use crate::graphite::wgpu::shared_context::WgpuSharedContext;
use crate::graphite::wgpu::texture::{WgpuTexture, as_wgpu_texture};
use crate::graphite::wgpu::texture_info::WgpuTextureInfoData;

/// The wgpu command buffer: the neutral core over the wgpu backend half.
// Port of: src/gpu/graphite/dawn/DawnCommandBuffer.h (chrome/m156)
#[doc(alias = "skgpu::graphite::DawnCommandBuffer")]
pub type WgpuCommandBuffer = CommandBufferCore<WgpuCommandBufferBackend>;

/// Creates a wgpu command buffer, with its encoder (`DawnCommandBuffer::Make`).
// Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L53-L60 (chrome/m156)
#[doc(alias = "Make")]
#[must_use]
pub fn new_wgpu_command_buffer(
    // Dawn doesn't support protected memory.
    _is_protected: Protected,
    resource_provider: SharedResourceProvider,
    shared_context: Arc<WgpuSharedContext>,
) -> Option<WgpuCommandBuffer> {
    let mut command_buffer = CommandBufferCore::new(
        Protected::No,
        resource_provider.clone(),
        WgpuCommandBufferBackend::new(shared_context, resource_provider),
    );
    if !command_buffer
        .backend_mut()
        .set_new_command_buffer_resources()
    {
        return None;
    }
    Some(command_buffer)
}

/// A resource of a bind group being made.
enum Owned {
    Buffer(wgpu::Buffer, u64, Option<NonZeroU64>),
    View(wgpu::TextureView),
    Sampler(wgpu::Sampler),
}

/// The bind group entries for `owned` resources.
fn bind_entries(owned: &[(u32, Owned)]) -> Vec<wgpu::BindGroupEntry<'_>> {
    owned
        .iter()
        .map(|(binding, resource)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: match resource {
                Owned::Buffer(buffer, offset, size) => {
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: *offset,
                        size: *size,
                    })
                }
                Owned::View(view) => wgpu::BindingResource::TextureView(view),
                Owned::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
            },
        })
        .collect()
}

/// We expect to have up to 3 uniforms in the uniform buffers bind group.
const MAX_UNIFORMS_IN_GROUP: usize = 3;

/// The binding indices of the uniforms in the group, in `fBoundUniforms` order.
const BINDING_INDICES: [u32; MAX_UNIFORMS_IN_GROUP] = [
    INTRINSIC_UNIFORM_BUFFER_INDEX,
    COMBINED_UNIFORM_INDEX,
    STORAGE_BUFFER_INDEX,
];

/// The state of a render pass being recorded that Dawn keeps in members of the command buffer.
struct PassState<'a> {
    /// The replay state of the neutral half.
    state: &'a ReplayState,
    /// The wgpu half of the context's resource provider.
    provider: &'a mut WgpuResourceProvider,
    /// `fActiveGraphicsPipeline`.
    active_pipeline: Option<Arc<dyn GraphicsPipeline>>,
    /// `fCurrentIndirectBuffer` and `fCurrentIndirectBufferOffset`.
    current_indirect: Option<(wgpu::Buffer, u64)>,
    /// The intrinsic constants, when they are passed as immediate data.
    immediates: Option<UniformDataBlock>,
}

impl PassState<'_> {
    /// `fActiveGraphicsPipeline`, which must be set.
    fn active(&self) -> Arc<dyn GraphicsPipeline> {
        Arc::clone(
            self.active_pipeline
                .as_ref()
                .expect("a graphics pipeline is bound"),
        )
    }
}

/// `static_cast<const DawnGraphicsPipeline*>(pipeline)`.
fn wgpu_pipeline(pipeline: &Arc<dyn GraphicsPipeline>) -> Option<&WgpuGraphicsPipeline> {
    pipeline.as_any().downcast_ref::<WgpuGraphicsPipeline>()
}

/// What is needed to resolve the MSAA attachment into the resolve texture after the render pass
/// (`ResolveStepEmulationInfo`).
struct ResolveStepEmulationInfo {
    /// `fMSAATexture`'s render view and sample count.
    msaa_view: wgpu::TextureView,
    msaa_sample_count: SampleCount,
    /// `fResolveTexture`'s render view and view format.
    resolve_view: wgpu::TextureView,
    resolve_format: TextureFormat,
    /// `fMSAAAOffset`.
    msaa_offset: IPoint,
    /// `fResolveArea`.
    resolve_area: IRect,
}

/// The backend half of [`WgpuCommandBuffer`].
// Port of: src/gpu/graphite/dawn/DawnCommandBuffer.h#L35-L190 (chrome/m156)
#[derive(Debug)]
pub struct WgpuCommandBufferBackend {
    shared_context: Arc<WgpuSharedContext>,
    /// `fResourceProvider`: the context's.
    resource_provider: SharedResourceProvider,
    /// `fCommandEncoder`.
    command_encoder: Option<wgpu::CommandEncoder>,
    /// `fBoundUniforms`.
    bound_uniforms: [BindBufferInfo; MAX_NUM_UNIFORM_BUFFERS as usize],
    /// `fBoundUniformBuffersDirty`.
    bound_uniform_buffers_dirty: bool,
}

impl std::fmt::Debug for ResolveStepEmulationInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolveStepEmulationInfo")
            .field("msaa_offset", &self.msaa_offset)
            .field("resolve_area", &self.resolve_area)
            .finish_non_exhaustive()
    }
}

/// Locks the context's resource provider.
fn lock_provider(provider: &SharedResourceProvider) -> MutexGuard<'_, ResourceProvider> {
    provider.lock().unwrap_or_else(PoisonError::into_inner)
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

/// `dawnTextureInfo().fAspect`.
fn texture_aspect(texture: &Texture) -> wgpu::TextureAspect {
    texture
        .texture_info()
        .get::<WgpuTextureInfoData>()
        .map_or(wgpu::TextureAspect::All, |info| info.aspect)
}

/// The wgpu half of `texture`, if it is a wgpu texture.
fn texture_of(texture: &Texture) -> Option<&WgpuTexture> {
    as_wgpu_texture(texture)
}

/// The trace id of a buffer (0 if it is not a wgpu buffer).
#[cfg(feature = "trace")]
fn buffer_id(buffer: &Buffer) -> u64 {
    as_wgpu_buffer(buffer).map_or(0, WgpuBuffer::trace_id)
}

/// The trace id of a texture (0 if it is not a wgpu texture).
#[cfg(feature = "trace")]
fn texture_id(texture: &Texture) -> u64 {
    texture_of(texture).map_or(0, WgpuTexture::trace_id)
}

/// A bound buffer range as the trace names it: `buffer:offset:size`.
#[cfg(feature = "trace")]
fn range_name(info: &BindBufferInfo) -> String {
    let id = info.buffer.as_ref().map_or(0, |buffer| buffer_id(buffer));
    format!("{id}:{}:{}", info.offset, info.size)
}

/// `static_cast<const DawnBuffer*>(buffer)->dawnBuffer()`.
fn wgpu_buffer_of(buffer: &Buffer) -> Option<wgpu::Buffer> {
    as_wgpu_buffer(buffer).and_then(WgpuBuffer::wgpu_buffer)
}

impl WgpuCommandBufferBackend {
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L62-L66 (chrome/m156)
    fn new(
        shared_context: Arc<WgpuSharedContext>,
        resource_provider: SharedResourceProvider,
    ) -> Self {
        Self {
            shared_context,
            resource_provider,
            command_encoder: None,
            bound_uniforms: Default::default(),
            bound_uniform_buffers_dirty: false,
        }
    }

    /// `finishEncoding()`: the wgpu command buffer, or `None` if there is no encoder.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L192-L200 (chrome/m156)
    #[doc(alias = "finishEncoding")]
    pub fn finish_encoding(&mut self) -> Option<wgpu::CommandBuffer> {
        let command_encoder = self.command_encoder.take()?;
        let command_buffer = command_encoder.finish();
        if let Some(provider) = wgpu_backend(&mut lock_provider(&self.resource_provider)) {
            provider.release_pending_intrinsic_buffers();
        }
        Some(command_buffer)
    }

    /// `getSampler()`: dynamic samplers live in the global cache, requiring no tracking.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L216-L231 (chrome/m156)
    fn get_sampler(
        &self,
        desc: SamplerDesc,
    ) -> Option<crate::graphite::resource::ResourceRef<Sampler>> {
        if desc.is_immutable() {
            // wgpu has no immutable (YCbCr) samplers.
            skia_log_e!("Immutable samplers are not supported by the wgpu backend");
            return None;
        }
        // Use the shared Sampler held in the global cache
        self.shared_context
            .base()
            .global_cache()
            .dynamic_sampler(desc)
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L233-L296 (chrome/m156)
    fn add_render_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        state: &ReplayState,
        call: &mut RenderPassCall<'_>,
        tracker: &mut dyn ResourceTracker,
    ) -> bool {
        let shared = Arc::clone(&self.shared_context);
        let caps = shared.caps();

        // `viewport` has already been translated by the replay translation by the base
        // CommandBuffer
        if !IRect::intersects(&call.viewport, &state.render_target_bounds) {
            // The entire pass is offscreen
            return true;
        }

        let mut intrinsic_values = UniformManager::new(Layout::Std140);
        collect_intrinsic_uniforms(
            &**caps,
            call.viewport,
            state.dst_read_bounds,
            &mut intrinsic_values,
        );
        let uniform_data = UniformDataBlock::wrap(&mut intrinsic_values);
        let use_push_constant = caps
            .resource_binding_requirements()
            .use_push_constants_for_intrinsic_constants;

        // If push constant is not supported, update the intrinsic constant buffer before
        // starting a render pass.
        if !use_push_constant && !self.update_intrinsic_uniforms_as_ubo(&uniform_data, tracker) {
            return false;
        }

        let resource_provider = Arc::clone(&self.resource_provider);
        let mut provider_guard = lock_provider(&resource_provider);
        let Some(provider) = wgpu_backend(&mut provider_guard) else {
            skia_log_e!("The wgpu command buffer needs a wgpu resource provider");
            return false;
        };

        let Some(ActivePass {
            mut pass,
            resolve_step_emulation_info,
        }) = self.begin_render_pass(encoder, state, call, provider)
        else {
            return false;
        };

        let mut pass_state = PassState {
            state,
            provider,
            active_pipeline: None,
            current_indirect: None,
            immediates: None,
        };

        // If push constant is supported, update the intrinsic constants after starting a render
        // pass. wgpu validates the immediate data against the pipeline layout, so it can only be
        // set once a pipeline is bound: it is set after every pipeline bind instead
        // (`bind_graphics_pipeline()`).
        if use_push_constant {
            pass_state.immediates = Some(uniform_data);
        }

        Self::set_viewport(&mut pass, call.viewport);
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("set_viewport")
                .i("x", call.viewport.left)
                .i("y", call.viewport.top)
                .i("width", call.viewport.width())
                .i("height", call.viewport.height())
        );

        let mut success = true;
        for draw_pass in call.draw_passes.iter_mut() {
            if !self.add_draw_pass(&mut pass, &mut pass_state, draw_pass.as_mut(), tracker) {
                success = false;
                break;
            }
        }

        // endRenderPass(): the pass ends when it is dropped.
        drop(pass);
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("end_render_pass")
        );
        let resolved = self.resolve_step(encoder, resolve_step_emulation_info, pass_state.provider);
        success && resolved
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L298-L321 (chrome/m156)
    fn add_compute_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        dispatch_groups: &mut [Box<DispatchGroup>],
        tracker: &mut dyn ResourceTracker,
    ) -> bool {
        let device = self.shared_context.device();
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("begin_compute_pass")
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        for group in dispatch_groups.iter_mut() {
            group.add_resource_refs(tracker);
            for dispatch in group.dispatches() {
                let Some(pipeline) = group.pipeline(dispatch.pipeline_index) else {
                    skia_log_e!("A dispatch has no compute pipeline");
                    return false;
                };
                let Some(pipeline) = pipeline.as_any().downcast_ref::<WgpuComputePipeline>() else {
                    skia_log_e!("A dispatch has a compute pipeline of another backend");
                    return false;
                };
                // bindComputePipeline()
                pass.set_pipeline(pipeline.compute_pipeline());
                trace!(
                    self.shared_context,
                    crate::graphite::wgpu::trace::Record::new("set_compute_pipeline")
                        .u("index", dispatch.pipeline_index)
                );

                // bindDispatchResources(): bind all pipeline resources to a single new bind
                // group at index 0.
                // NOTE: Caching the bind groups here might be beneficial based on the layout and
                // the bound resources (though it's questionable how often a bind group will end
                // up getting reused since the bound objects change often).
                let mut owned = Vec::with_capacity(dispatch.bindings.len());
                for binding in &dispatch.bindings {
                    let resource = match &binding.resource {
                        BindingResource::Buffer(info) => {
                            let Some(buffer) = info.buffer.as_ref().and_then(|b| wgpu_buffer_of(b))
                            else {
                                skia_log_e!("A dispatch binds a buffer that does not exist");
                                return false;
                            };
                            Owned::Buffer(
                                buffer,
                                u64::from(info.offset),
                                NonZeroU64::new(u64::from(info.size)),
                            )
                        }
                        BindingResource::Texture(index) => {
                            let Some(view) = group
                                .texture(index.0)
                                .and_then(|texture| texture_of(&texture)?.sample_texture_view())
                            else {
                                skia_log_e!("A dispatch binds a texture that does not exist");
                                return false;
                            };
                            Owned::View(view)
                        }
                        BindingResource::Sampler(index) => {
                            let Some(sampler) = group
                                .sampler(index.0)
                                .and_then(|sampler| as_wgpu_sampler(&sampler)?.wgpu_sampler())
                            else {
                                skia_log_e!("A dispatch binds a sampler that does not exist");
                                return false;
                            };
                            Owned::Sampler(sampler)
                        }
                    };
                    owned.push((binding.index, resource));
                }
                let entries = bind_entries(&owned);
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: pipeline.group_layout(),
                    entries: &entries,
                });
                pass.set_bind_group(0, &bind_group, &[]);
                trace!(
                    self.shared_context,
                    crate::graphite::wgpu::trace::Record::new("set_bind_group")
                        .u("group", 0_u32)
                        .strings(
                            "bindings",
                            dispatch
                                .bindings
                                .iter()
                                .map(|binding| match &binding.resource {
                                    BindingResource::Buffer(info) => {
                                        format!("{}:buffer:{}", binding.index, range_name(info))
                                    }
                                    BindingResource::Texture(index) => {
                                        format!("{}:texture:{}", binding.index, index.0)
                                    }
                                    BindingResource::Sampler(index) => {
                                        format!("{}:sampler:{}", binding.index, index.0)
                                    }
                                })
                        )
                );

                match &dispatch.global_size_or_indirect {
                    GlobalSizeOrIndirect::Size(size) => {
                        // dispatchWorkgroups()
                        trace!(
                            self.shared_context,
                            crate::graphite::wgpu::trace::Record::new("dispatch_workgroups")
                                .u("x", size.width)
                                .u("y", size.height)
                                .u("z", size.depth)
                        );
                        pass.dispatch_workgroups(size.width, size.height, size.depth);
                    }
                    GlobalSizeOrIndirect::Indirect(indirect) => {
                        // dispatchWorkgroupsIndirect()
                        let Some(buffer) = indirect.buffer.as_ref().and_then(|b| wgpu_buffer_of(b))
                        else {
                            skia_log_e!("An indirect dispatch has no buffer");
                            return false;
                        };
                        trace!(
                            self.shared_context,
                            crate::graphite::wgpu::trace::Record::new(
                                "dispatch_workgroups_indirect"
                            )
                            .s("range", range_name(indirect))
                        );
                        pass.dispatch_workgroups_indirect(&buffer, u64::from(indirect.offset));
                    }
                }
            }
        }
        // endComputePass(): the pass ends when it is dropped.
        drop(pass);
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("end_compute_pass")
        );
        true
    }

    /// `beginRenderPass()`.
    ///
    /// Returns the pass, with the state to resolve the MSAA attachment after it if the load or
    /// store of the resolve is emulated.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L323-L527 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn begin_render_pass<'enc>(
        &self,
        encoder: &'enc mut wgpu::CommandEncoder,
        state: &ReplayState,
        call: &RenderPassCall<'_>,
        provider: &mut WgpuResourceProvider,
    ) -> Option<ActivePass<'enc>> {
        let caps = self.shared_context.caps();
        let render_pass_desc = call.render_pass_desc;

        let load_action = |op: LoadOp| match op {
            LoadOp::Load => LoadAction::Load,
            LoadOp::Clear => LoadAction::Clear,
            // `discardLoadOp()`: wgpu has no undefined load op, so a discarded load clears.
            LoadOp::Discard => match caps.discard_load_op() {
                DiscardLoadOp::Clear | DiscardLoadOp::Undefined => LoadAction::Clear,
            },
        };
        let store_action = |op: StoreOp| match op {
            StoreOp::Store => wgpu::StoreOp::Store,
            // `discardStoreOp()`: wgpu has no undefined store op, so a discarded store discards.
            StoreOp::Discard => match caps.discard_store_op() {
                DiscardStoreOp::Discard | DiscardStoreOp::Undefined => wgpu::StoreOp::Discard,
            },
        };

        // Validate attachment descs and textures
        let color_info = &render_pass_desc.color_attachment;
        let resolve_info = &render_pass_desc.color_resolve_attachment;
        let depth_stencil_info = &render_pass_desc.depth_stencil_attachment;
        debug_assert!(color_info.is_compatible(call.color_texture.texture_info()));
        debug_assert_eq!(
            call.resolve_texture.is_some(),
            resolve_info.format != TextureFormat::Unsupported
        );
        debug_assert_eq!(
            call.depth_stencil_texture.is_some(),
            depth_stencil_info.format != TextureFormat::Unsupported
        );

        // Set up color attachment
        let color_texture: &Texture = call.color_texture;
        let color_view = texture_of(color_texture)?.render_texture_view()?;
        let [r, g, b, a] = render_pass_desc.clear_color;
        let clear_color = wgpu::Color {
            r: f64::from(r),
            g: f64::from(g),
            b: f64::from(b),
            a: f64::from(a),
        };
        let mut color_load = match load_action(color_info.load_op) {
            LoadAction::Load => wgpu::LoadOp::Load,
            LoadAction::Clear => wgpu::LoadOp::Clear(clear_color),
        };
        let mut color_store = store_action(color_info.store_op);
        let mut resolve_view = None;
        let mut emulate_load_store_resolve_texture = false;

        // Set up resolve attachment
        if let Some(resolve_texture) = call.resolve_texture {
            debug_assert_eq!(resolve_info.store_op, StoreOp::Store);

            let resolve_texture: &Texture = resolve_texture;
            let view = texture_of(resolve_texture)?.render_texture_view()?;

            // Inclusion of a resolve texture implies the client wants to finish the
            // renderpass with a resolve.
            debug_assert_eq!(color_store, store_action(StoreOp::Discard));
            // But it also means we might have to load the resolve texture into the MSAA color
            // attachment

            if caps.emulate_load_store_resolve() {
                emulate_load_store_resolve_texture = true;
            } else if resolve_info.load_op == LoadOp::Load {
                // wgpu has no `ExpandResolveTexture` load op (a Dawn feature): no built-in
                // support, we need to manually load the resolve texture.
                emulate_load_store_resolve_texture = true;
            }

            if emulate_load_store_resolve_texture {
                // The resolve is done separately after the pass (`endRenderPass()`).
                resolve_view = None;
                drop(view);
            } else {
                // wgpu has no partial resolve rects: the resolve covers the whole attachment.
                debug_assert_eq!(call.resolve_offset, IPoint::default());
                resolve_view = Some(view);
            }
            // TODO: If the color resolve texture is read-only we can use a private (vs.
            // memoryless) msaa attachment that's coupled to the framebuffer and the
            // StoreAndMultisampleResolve action instead of loading as a draw.
        } else {
            // wgpu has no `MSAARenderToSingleSampled` (a Dawn feature).
            debug_assert!(
                !(render_pass_desc.sample_count > SampleCount::One
                    && color_texture.sample_count() == SampleCount::One),
                "multisampled render to single sampled is not supported by wgpu"
            );
        }

        // Set up stencil/depth attachment
        let depth_stencil_view = match call.depth_stencil_texture {
            Some(depth_stencil_texture) => {
                let depth_stencil_texture: &Texture = depth_stencil_texture;
                Some(texture_of(depth_stencil_texture)?.render_texture_view()?)
            }
            None => None,
        };
        let depth_ops =
            (texture_format_has_depth(depth_stencil_info.format)).then(|| wgpu::Operations {
                load: match load_action(depth_stencil_info.load_op) {
                    LoadAction::Load => wgpu::LoadOp::Load,
                    LoadAction::Clear => wgpu::LoadOp::Clear(render_pass_desc.clear_depth),
                },
                store: store_action(depth_stencil_info.store_op),
            });
        let stencil_ops =
            (texture_format_has_stencil(depth_stencil_info.format)).then(|| wgpu::Operations {
                load: match load_action(depth_stencil_info.load_op) {
                    LoadAction::Load => wgpu::LoadOp::Load,
                    LoadAction::Clear => wgpu::LoadOp::Clear(render_pass_desc.clear_stencil),
                },
                store: store_action(depth_stencil_info.store_op),
            });

        let mut resolve_step_emulation_info = None;
        let mut blit_after_load = None;
        if emulate_load_store_resolve_texture {
            // `emulateLoadMSAAFromResolveAndBeginRenderPassEncoder()`: override the render pass
            // to exclude the resolve texture. We will emulate the loading & resolve via blit.
            // The resolve step will be done separately after endRenderPass().
            let load_resolve = resolve_info.load_op == LoadOp::Load;
            color_store = wgpu::StoreOp::Store;
            if load_resolve {
                // If we intend to load the resolve texture, then override the loadOp of the MSAA
                // attachment to Load instead of Clear.
                // This path is intended to be used when the device doesn't support transient
                // attachments. Which most likely means it is a non-tiled GPU. On non-tiled GPUs,
                // load is a no-op so it should be faster than clearing the whole MSAA
                // attachment. Note: Dawn doesn't have any DontCare loadOp.
                color_load = wgpu::LoadOp::Load;
            }

            let resolve_texture: &Texture = call
                .resolve_texture
                .expect("emulation only with a resolve texture");
            let mut msaa_area = state.render_area_bounds;
            let _ = intersect_in_place(
                &mut msaa_area,
                &IRect::from_size(color_texture.dimensions()),
            );
            let mut resolve_area = state.render_area_bounds;
            resolve_area.offset(call.resolve_offset);
            let _ = intersect_in_place(
                &mut resolve_area,
                &IRect::from_size(resolve_texture.dimensions()),
            );
            let resolve_render_view = texture_of(resolve_texture)?.render_texture_view()?;

            if load_resolve {
                // Blit from the resolve texture to the MSAA texture
                let mut render_pass_without_resolve_desc = render_pass_desc.clone();
                render_pass_without_resolve_desc.color_resolve_attachment =
                    AttachmentDesc::default();
                blit_after_load = Some((
                    render_pass_without_resolve_desc,
                    resolve_render_view.clone(),
                    resolve_area.top_left(),
                    msaa_area,
                ));
            }
            resolve_step_emulation_info = Some(ResolveStepEmulationInfo {
                msaa_view: color_view.clone(),
                msaa_sample_count: color_texture.texture_info().sample_count(),
                resolve_view: resolve_render_view,
                resolve_format: texture_info_priv::view_format(resolve_texture.texture_info()),
                msaa_offset: msaa_area.top_left(),
                resolve_area,
            });
        }

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &color_view,
            depth_slice: None,
            resolve_target: resolve_view.as_ref(),
            ops: wgpu::Operations {
                load: color_load,
                store: color_store,
            },
        };
        let depth_stencil_attachment =
            depth_stencil_view
                .as_ref()
                .map(|view| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops,
                    stencil_ops,
                });
        let descriptor = wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        };
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("begin_render_pass")
                .u("color", texture_id(color_texture))
                .u(
                    "resolve",
                    call.resolve_texture
                        .map_or(0, |texture| texture_id(texture))
                )
                .u(
                    "depth_stencil",
                    call.depth_stencil_texture
                        .map_or(0, |texture| texture_id(texture))
                )
                .s("color_load", format!("{color_load:?}"))
                .s("color_store", format!("{color_store:?}"))
                .u(
                    "emulated_resolve",
                    u32::from(emulate_load_store_resolve_texture)
                )
        );
        let mut pass = encoder.begin_render_pass(&descriptor);

        if let Some((desc, src_view, src_offset, dst_bounds)) = blit_after_load
            && !Self::do_blit_with_draw(
                &self.shared_context,
                self.shared_context.device(),
                provider,
                &mut pass,
                &desc,
                &src_view,
                SampleCount::One,
                src_offset,
                dst_bounds,
            )
        {
            return None;
        }

        Some(ActivePass {
            pass,
            resolve_step_emulation_info,
        })
    }

    /// `doBlitWithDraw()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L585-L605 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn do_blit_with_draw(
        shared_context: &WgpuSharedContext,
        device: &wgpu::Device,
        provider: &mut WgpuResourceProvider,
        render_encoder: &mut wgpu::RenderPass<'_>,
        frontend_render_pass_desc_key: &RenderPassDesc,
        src_texture_view: &wgpu::TextureView,
        src_sample_count: SampleCount,
        src_offset: IPoint,
        dst_bounds: IRect,
    ) -> bool {
        let blit = provider
            .find_or_create_blit_with_draw_encoder(frontend_render_pass_desc_key, src_sample_count);
        if !blit.is_valid() {
            skia_log_e!("Unable to create pipeline to blit with draw");
            return false;
        }

        blit.encode_blit(
            device,
            render_encoder,
            src_texture_view,
            src_offset,
            dst_bounds,
        );
        trace!(
            shared_context,
            crate::graphite::wgpu::trace::Record::new("blit_with_draw")
                .u("src_sample_count", src_sample_count as u32)
                .i("src_x", src_offset.x)
                .i("src_y", src_offset.y)
                .i("dst_left", dst_bounds.left)
                .i("dst_top", dst_bounds.top)
                .i("dst_right", dst_bounds.right)
                .i("dst_bottom", dst_bounds.bottom)
        );

        true
    }

    /// `endRenderPass()`: ends the pass, then does the resolve step of an emulated resolve.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L607-L650 (chrome/m156)
    fn resolve_step(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        resolve_step_emulation_info: Option<ResolveStepEmulationInfo>,
        provider: &mut WgpuResourceProvider,
    ) -> bool {
        // Return early if no resolve step's emulation is needed.
        let Some(info) = resolve_step_emulation_info else {
            return true;
        };

        // Creating an intermediate render pass to copy from the MSAA texture -> resolve texture.
        let intermediate_render_pass_desc = RenderPassDesc {
            color_attachment: AttachmentDesc {
                format: info.resolve_format,
                load_op: LoadOp::Load,
                store_op: StoreOp::Store,
                sample_count: SampleCount::One,
            },
            ..RenderPassDesc::default()
        };

        let intermediate_color_attachment = wgpu::RenderPassColorAttachment {
            view: &info.resolve_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        };
        let mut render_pass_encoder = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(intermediate_color_attachment)],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        Self::do_blit_with_draw(
            &self.shared_context,
            self.shared_context.device(),
            provider,
            &mut render_pass_encoder,
            &intermediate_render_pass_desc,
            &info.msaa_view,
            info.msaa_sample_count,
            info.msaa_offset,
            info.resolve_area,
        )
    }

    /// `addDrawPass()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L652-L761 (chrome/m156)
    fn add_draw_pass(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        pass_state: &mut PassState<'_>,
        draw_pass: &mut dyn DrawPass,
        tracker: &mut dyn ResourceTracker,
    ) -> bool {
        // If there is storage buffer data to bind, it must be done prior to draws.
        if let Some(info) = draw_pass.storage_buffer_info()
            && info.is_valid()
        {
            let info = info.clone();
            self.bind_uniform_buffer(&info, UniformSlot::Storage);
        }

        if !draw_pass.add_resource_refs(tracker) {
            return false;
        }
        let pipelines = draw_pass.pipelines();

        for command in draw_pass.commands() {
            match command {
                DrawPassCommand::BindGraphicsPipeline { pipeline_index } => {
                    let Some(Some(pipeline)) = pipelines.get(*pipeline_index as usize) else {
                        return false;
                    };
                    if !self.bind_graphics_pipeline(pass, pass_state, pipeline) {
                        return false;
                    }
                }
                DrawPassCommand::SetBlendConstants { blend_constants } => {
                    // setBlendConstants()
                    pass.set_blend_constant(wgpu::Color {
                        r: f64::from(blend_constants[0]),
                        g: f64::from(blend_constants[1]),
                        b: f64::from(blend_constants[2]),
                        a: f64::from(blend_constants[3]),
                    });
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("set_blend_constant")
                            .f("r", blend_constants[0])
                            .f("g", blend_constants[1])
                            .f("b", blend_constants[2])
                            .f("a", blend_constants[3])
                    );
                }
                DrawPassCommand::BindUniformBuffer { info, slot } => {
                    self.bind_uniform_buffer(info, *slot);
                }
                DrawPassCommand::BindStaticDataBuffer { static_data } => {
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("set_vertex_buffer")
                            .u("slot", 0_u32)
                            .s("range", range_name(static_data))
                    );
                    Self::bind_input_buffer(
                        pass,
                        static_data,
                        crate::graphite::wgpu::graphics_pipeline::STATIC_DATA_BUFFER_INDEX as u32,
                    );
                }
                DrawPassCommand::BindAppendDataBuffer { append_data } => {
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("set_vertex_buffer")
                            .u("slot", 1_u32)
                            .s("range", range_name(append_data))
                    );
                    Self::bind_input_buffer(
                        pass,
                        append_data,
                        crate::graphite::wgpu::graphics_pipeline::APPEND_DATA_BUFFER_INDEX as u32,
                    );
                }
                DrawPassCommand::BindIndexBuffer { indices } => {
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("set_index_buffer")
                            .s("range", range_name(indices))
                    );
                    Self::bind_index_buffer(pass, indices);
                }
                DrawPassCommand::BindIndirectBuffer { indirect } => {
                    // bindIndirectBuffer()
                    pass_state.current_indirect = indirect
                        .buffer
                        .as_ref()
                        .and_then(|buffer| wgpu_buffer_of(buffer))
                        .map(|buffer| (buffer, u64::from(indirect.offset)));
                }
                DrawPassCommand::BindTexturesAndSamplers { textures, samplers } => {
                    if !self.bind_texture_and_samplers(pass, pass_state, textures, samplers) {
                        return false;
                    }
                }
                DrawPassCommand::SetScissor { scissor } => {
                    #[cfg(feature = "trace")]
                    {
                        let rect = scissor.get_rect(
                            pass_state.state.replay_translation,
                            pass_state.state.render_area_bounds,
                        );
                        trace!(
                            self.shared_context,
                            crate::graphite::wgpu::trace::Record::new("set_scissor_rect")
                                .i("x", rect.left)
                                .i("y", rect.top)
                                .i("width", rect.width())
                                .i("height", rect.height())
                        );
                    }
                    Self::set_scissor(pass, pass_state.state, scissor);
                }
                DrawPassCommand::Draw {
                    primitive,
                    base_vertex,
                    vertex_count,
                } => {
                    self.sync_before_draw(pass, pass_state, *primitive);
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("draw")
                            .u("vertex_count", *vertex_count)
                            .u("first_vertex", *base_vertex)
                    );
                    pass.draw(*base_vertex..*base_vertex + *vertex_count, 0..1);
                }
                DrawPassCommand::DrawIndexed {
                    primitive,
                    base_index,
                    index_count,
                    base_vertex,
                } => {
                    self.sync_before_draw(pass, pass_state, *primitive);
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("draw_indexed")
                            .u("index_count", *index_count)
                            .u("first_index", *base_index)
                            .i("base_vertex", base_vertex.cast_signed())
                    );
                    pass.draw_indexed(
                        *base_index..*base_index + *index_count,
                        base_vertex.cast_signed(),
                        0..1,
                    );
                }
                DrawPassCommand::DrawInstanced {
                    primitive,
                    base_vertex,
                    vertex_count,
                    base_instance,
                    instance_count,
                } => {
                    self.sync_before_draw(pass, pass_state, *primitive);
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("draw")
                            .u("vertex_count", *vertex_count)
                            .u("instance_count", *instance_count)
                            .u("first_vertex", *base_vertex)
                            .u("first_instance", *base_instance)
                    );
                    pass.draw(
                        *base_vertex..*base_vertex + *vertex_count,
                        *base_instance..*base_instance + *instance_count,
                    );
                }
                DrawPassCommand::DrawIndexedInstanced {
                    primitive,
                    base_index,
                    index_count,
                    base_vertex,
                    base_instance,
                    instance_count,
                } => {
                    self.sync_before_draw(pass, pass_state, *primitive);
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("draw_indexed")
                            .u("index_count", *index_count)
                            .u("instance_count", *instance_count)
                            .u("first_index", *base_index)
                            .i("base_vertex", base_vertex.cast_signed())
                            .u("first_instance", *base_instance)
                    );
                    pass.draw_indexed(
                        *base_index..*base_index + *index_count,
                        base_vertex.cast_signed(),
                        *base_instance..*base_instance + *instance_count,
                    );
                }
                DrawPassCommand::DrawIndirect { primitive } => {
                    self.sync_before_draw(pass, pass_state, *primitive);
                    let Some((buffer, offset)) = &pass_state.current_indirect else {
                        skia_log_e!("An indirect draw has no indirect buffer");
                        return false;
                    };
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("draw_indirect")
                            .u("offset", *offset)
                    );
                    pass.draw_indirect(buffer, *offset);
                }
                DrawPassCommand::DrawIndexedIndirect { primitive } => {
                    self.sync_before_draw(pass, pass_state, *primitive);
                    let Some((buffer, offset)) = &pass_state.current_indirect else {
                        skia_log_e!("An indirect draw has no indirect buffer");
                        return false;
                    };
                    trace!(
                        self.shared_context,
                        crate::graphite::wgpu::trace::Record::new("draw_indexed_indirect")
                            .u("offset", *offset)
                    );
                    pass.draw_indexed_indirect(buffer, *offset);
                }
                DrawPassCommand::AddBarrier { .. } => {
                    skia_log_e!("DawnCommandBuffer does not support the addition of barriers.");
                }
            }
        }

        true
    }

    /// The common start of the draw functions: the primitive type matches the pipeline, and the
    /// uniform buffers are synced.
    fn sync_before_draw(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        pass_state: &mut PassState<'_>,
        primitive: PrimitiveType,
    ) {
        debug_assert_eq!(
            wgpu_pipeline(&pass_state.active())
                .expect("a wgpu graphics pipeline")
                .primitive_type(),
            primitive
        );
        self.sync_uniform_buffers(pass, pass_state);
    }

    /// `bindGraphicsPipeline()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L763-L805 (chrome/m156)
    fn bind_graphics_pipeline(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        pass_state: &mut PassState<'_>,
        graphics_pipeline: &Arc<dyn GraphicsPipeline>,
    ) -> bool {
        let Some(wgpu_pipeline) = wgpu_pipeline(graphics_pipeline) else {
            return false;
        };
        pass.set_pipeline(wgpu_pipeline.render_pipeline());
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("set_pipeline")
                .s("label", graphics_pipeline.label())
        );
        if let Some(immediates) = &pass_state.immediates {
            Self::update_intrinsic_uniforms_as_push_constant(pass, immediates);
            trace!(
                self.shared_context,
                crate::graphite::wgpu::trace::Record::new("set_immediates").u(
                    "hash",
                    crate::graphite::wgpu::trace::hash_bytes(immediates.data())
                )
            );
        }
        pass_state.active_pipeline = Some(Arc::clone(graphics_pipeline));
        self.bound_uniform_buffers_dirty = true;

        if graphics_pipeline.dst_read_strategy() == DstReadStrategy::TextureCopy
            && graphics_pipeline.num_frag_textures_and_samplers() == 2
        {
            // The pipeline has a single paired texture+sampler and uses texture copies for dst
            // reads. This situation comes about when the program requires complex blending but
            // otherwise is not referencing any images. Since there are no other images in play,
            // the DrawPass will not have a BindTexturesAndSamplers command that we can tack the
            // dstCopy on to. Instead we need to set the texture BindGroup ASAP to just the
            // dstCopy.
            // TODO(b/366254117): Once we standardize on a pipeline layout across all backends,
            // the dst copy texture may not go in a group with the regular textures, in which
            // case this binding can hopefully happen in a single place (e.g. here or at the
            // start of the renderpass and not also every other time the textures are changed).
            let (Some(texture), Some(sampler)) = (
                pass_state.state.dst_copy.as_ref(),
                pass_state.state.dst_copy_sampler.as_ref(),
            ) else {
                skia_log_e!("A dst copy pipeline is bound without a dst copy");
                return false;
            };

            let Some(bind_group) = pass_state
                .provider
                .find_or_create_single_texture_sampler_bind_group(sampler, texture)
            else {
                return false;
            };

            pass.set_bind_group(TEXTURE_BIND_GROUP_INDEX, &bind_group, &[]);
        }

        true
    }

    /// `bindUniformBuffer()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L807-L825 (chrome/m156)
    fn bind_uniform_buffer(&mut self, info: &BindBufferInfo, slot: UniformSlot) {
        let buffer_index = match slot {
            UniformSlot::CombinedUniforms => COMBINED_UNIFORM_INDEX,
            UniformSlot::Storage => STORAGE_BUFFER_INDEX,
        };

        self.bound_uniforms[buffer_index as usize] = info.clone();
        self.bound_uniform_buffers_dirty = true;
    }

    /// `bindInputBuffer()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L827-L835 (chrome/m156)
    fn bind_input_buffer(
        pass: &mut wgpu::RenderPass<'_>,
        info: &BindBufferInfo,
        binding_index: u32,
    ) {
        if let Some(buffer) = &info.buffer
            && let Some(wgpu_buffer) = wgpu_buffer_of(buffer)
        {
            if u64::from(info.offset) >= wgpu_buffer.size() {
                // An empty slice cannot be bound in wgpu; nothing can be fetched from it.
                return;
            }
            pass.set_vertex_buffer(binding_index, wgpu_buffer.slice(u64::from(info.offset)..));
        }
    }

    /// `bindIndexBuffer()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L837-L845 (chrome/m156)
    fn bind_index_buffer(pass: &mut wgpu::RenderPass<'_>, info: &BindBufferInfo) {
        if let Some(buffer) = &info.buffer
            && let Some(wgpu_buffer) = wgpu_buffer_of(buffer)
        {
            if u64::from(info.offset) >= wgpu_buffer.size() {
                return;
            }
            pass.set_index_buffer(
                wgpu_buffer.slice(u64::from(info.offset)..),
                wgpu::IndexFormat::Uint16,
            );
        }
    }

    /// `bindTextureAndSamplers()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L856-L960 (chrome/m156)
    fn bind_texture_and_samplers(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pass_state: &mut PassState<'_>,
        textures: &[Arc<crate::graphite::texture_proxy::TextureProxy>],
        samplers: &[SamplerDesc],
    ) -> bool {
        let active_pipeline = pass_state.active();
        let active = wgpu_pipeline(&active_pipeline).expect("a wgpu graphics pipeline");

        // When there's an active graphics pipeline with a texture-copy dstread requirement, add
        // one to account for the intrinsic dstCopy texture we bind here.
        // NOTE: This is in units of pairs of textures and samplers, whereas the value reported
        // by the current pipeline is in net bindings (textures + samplers).
        let mut num_textures_and_samplers = textures.len();
        let uses_dst_copy =
            GraphicsPipeline::dst_read_strategy(active) == DstReadStrategy::TextureCopy;
        if uses_dst_copy {
            num_textures_and_samplers += 1;
        }
        debug_assert_eq!(
            usize::try_from(active.num_frag_textures_and_samplers()).ok(),
            Some(2 * num_textures_and_samplers)
        );

        let Some(group_layout) = active.group_layouts()[TEXTURE_BIND_GROUP_INDEX as usize].clone()
        else {
            skia_log_e!("The pipeline has no texture bind group layout");
            return false;
        };

        let bind_group = if num_textures_and_samplers == 1 {
            // Optimize for single texture with dynamic sampling.
            debug_assert!(!uses_dst_copy);
            let Some(texture) = textures[0].ref_texture() else {
                return false;
            };
            let Some(sampler) = self.get_sampler(samplers[0]) else {
                return false;
            };
            let Some(bind_group) = pass_state
                .provider
                .find_or_create_single_texture_sampler_bind_group(
                    sampler.resource(),
                    texture.resource(),
                )
            else {
                return false;
            };
            bind_group
        } else {
            let mut owned: Vec<(u32, Owned)> = Vec::with_capacity(2 * num_textures_and_samplers);

            for (i, (proxy, sampler_desc)) in textures.iter().zip(samplers).enumerate() {
                let i = u32::try_from(i).expect("a few textures");
                let Some(texture) = proxy.ref_texture() else {
                    return false;
                };
                let Some(view) = texture_of(&texture).and_then(WgpuTexture::sample_texture_view)
                else {
                    return false;
                };
                let Some(sampler) = self.get_sampler(*sampler_desc) else {
                    return false;
                };
                let Some(wgpu_sampler) =
                    as_wgpu_sampler(&sampler).and_then(WgpuSampler::wgpu_sampler)
                else {
                    return false;
                };

                // Assuming shader generator assigns binding slot to sampler then texture, then
                // the next sampler and texture, and so on, we need to use 2 * i as base binding
                // index of the sampler and texture.
                // TODO: https://b.corp.google.com/issues/259457090:
                // Better configurable way of assigning samplers and textures' bindings.
                owned.push((2 * i, Owned::Sampler(wgpu_sampler)));
                owned.push((2 * i + 1, Owned::View(view)));
            }

            if uses_dst_copy {
                // Append the dstCopy sampler and texture as the very last two bind group entries
                let n = u32::try_from(num_textures_and_samplers).expect("a few textures");
                let (Some(dst_texture), Some(dst_sampler)) = (
                    pass_state.state.dst_copy.as_ref(),
                    pass_state.state.dst_copy_sampler.as_ref(),
                ) else {
                    skia_log_e!("A dst copy pipeline is bound without a dst copy");
                    return false;
                };
                let Some(sampler) =
                    as_wgpu_sampler(dst_sampler).and_then(WgpuSampler::wgpu_sampler)
                else {
                    return false;
                };
                let Some(view) = texture_of(dst_texture).and_then(WgpuTexture::sample_texture_view)
                else {
                    return false;
                };
                owned.push((2 * n - 2, Owned::Sampler(sampler)));
                owned.push((2 * n - 1, Owned::View(view)));
            }

            let entries = bind_entries(&owned);
            pass_state
                .provider
                .create_bind_group(&entries, &group_layout)
        };

        pass.set_bind_group(TEXTURE_BIND_GROUP_INDEX, &bind_group, &[]);
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("set_bind_group")
                .u("group", TEXTURE_BIND_GROUP_INDEX)
                .strings(
                    "textures",
                    textures.iter().map(|proxy| {
                        proxy.ref_texture().map_or(String::from("0"), |texture| {
                            texture_id(&texture).to_string()
                        })
                    })
                )
                .strings("samplers", samplers.iter().map(|desc| format!("{desc:?}")))
        );
        true
    }

    /// `syncUniformBuffers()`: commits uniform buffers' changes if any before drawing.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L962-L1022 (chrome/m156)
    fn sync_uniform_buffers(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        pass_state: &mut PassState<'_>,
    ) {
        if !self.bound_uniform_buffers_dirty {
            return;
        }
        self.bound_uniform_buffers_dirty = false;

        let use_push_constants = self
            .shared_context
            .caps()
            .resource_binding_requirements()
            .use_push_constants_for_intrinsic_constants;

        // Until/unless uniform bind group structure gets reorganized, this should be equivalent
        // to the size of our bound uniform array.
        debug_assert_eq!(MAX_UNIFORMS_IN_GROUP, self.bound_uniforms.len());

        let active_pipeline = pass_state.active();
        let active = wgpu_pipeline(&active_pipeline).expect("a wgpu graphics pipeline");
        let mut dynamic_offsets = [0u32; MAX_UNIFORMS_IN_GROUP];
        // Check if we can use an optimized route for single-uniform buffer bind groups:
        let bind_group = if use_push_constants
            && !active.uses_storage_buffer()
            && active.has_combined_uniforms()
        {
            let buffer_info = &self.bound_uniforms[COMBINED_UNIFORM_INDEX as usize];
            dynamic_offsets[COMBINED_UNIFORM_INDEX as usize] = buffer_info.offset;
            pass_state
                .provider
                .find_or_create_single_uniform_bind_group(buffer_info)
        } else {
            let enabled = [
                !use_push_constants,            // intrinsic uniforms
                active.has_combined_uniforms(), // paint AND renderstep uniforms!
                active.uses_storage_buffer(),   // storage SSBO
            ];

            // The buffers of the entries: the bound ones, or the null buffer.
            let null_buffer = pass_state.provider.get_or_create_null_buffer().clone();
            let mut buffers: Vec<(wgpu::Buffer, Option<NonZeroU64>)> =
                Vec::with_capacity(MAX_UNIFORMS_IN_GROUP);
            for i in 0..MAX_UNIFORMS_IN_GROUP {
                let bound = &self.bound_uniforms[i];
                let buffer = if enabled[i] && bound.is_valid() {
                    bound
                        .buffer
                        .as_ref()
                        .and_then(|buffer| wgpu_buffer_of(buffer))
                } else {
                    None
                };
                match buffer {
                    Some(buffer) => {
                        dynamic_offsets[i] = bound.offset;
                        buffers.push((buffer, NonZeroU64::new(u64::from(bound.size))));
                    }
                    // Unused or null binding
                    None => buffers.push((null_buffer.clone(), None)),
                }
            }
            let entries: Vec<wgpu::BindGroupEntry<'_>> = buffers
                .iter()
                .enumerate()
                .map(|(i, (buffer, size))| wgpu::BindGroupEntry {
                    binding: BINDING_INDICES[i],
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: *size,
                    }),
                })
                .collect();

            let Some(layout) = &active.group_layouts()[UNIFORM_BUFFER_BIND_GROUP_INDEX as usize]
            else {
                skia_log_e!("The pipeline has no uniform buffers bind group layout");
                return;
            };
            Some(pass_state.provider.create_bind_group(&entries, layout))
        };

        let Some(bind_group) = bind_group else {
            skia_log_e!("Failed to make the uniform buffers bind group");
            return;
        };
        pass.set_bind_group(
            UNIFORM_BUFFER_BIND_GROUP_INDEX,
            &bind_group,
            &dynamic_offsets,
        );
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("set_bind_group")
                .u("group", UNIFORM_BUFFER_BIND_GROUP_INDEX)
                .strings("buffers", self.bound_uniforms.iter().map(range_name))
                .list(
                    "dynamic_offsets",
                    dynamic_offsets.iter().map(|offset| u64::from(*offset))
                )
        );
    }

    /// `setScissor()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1024-L1028 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // the scissor is clipped to the (non-negative) render area
    fn set_scissor(
        pass: &mut wgpu::RenderPass<'_>,
        state: &ReplayState,
        scissor: &crate::graphite::command_buffer::Scissor,
    ) {
        let rect = scissor.get_rect(state.replay_translation, state.render_area_bounds);
        pass.set_scissor_rect(
            rect.left as u32,
            rect.top as u32,
            rect.width() as u32,
            rect.height() as u32,
        );
    }

    /// `updateIntrinsicUniformsAsUBO()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1030-L1042 (chrome/m156)
    fn update_intrinsic_uniforms_as_ubo(
        &mut self,
        uniform_data: &UniformDataBlock,
        tracker: &mut dyn ResourceTracker,
    ) -> bool {
        let binding = find_or_create_intrinsic_bind_buffer_info(
            &mut lock_provider(&self.resource_provider),
            tracker,
            uniform_data,
        );

        if !binding.is_valid() {
            return false;
        } else if binding == self.bound_uniforms[INTRINSIC_UNIFORM_BUFFER_INDEX as usize] {
            return true; // no binding change needed
        }

        self.bound_uniforms[INTRINSIC_UNIFORM_BUFFER_INDEX as usize] = binding;
        self.bound_uniform_buffers_dirty = true;
        true
    }

    /// `updateIntrinsicUniformsAsPushConstant()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1044-L1054 (chrome/m156)
    fn update_intrinsic_uniforms_as_push_constant(
        pass: &mut wgpu::RenderPass<'_>,
        uniform_data: &UniformDataBlock,
    ) {
        debug_assert!(uniform_data.size() <= INTRINSIC_UNIFORM_SIZE as usize);
        pass.set_immediates(0, uniform_data.data());
    }

    /// `setViewport()`.
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1056-L1060 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // viewport coordinates are small integers
    fn set_viewport(pass: &mut wgpu::RenderPass<'_>, viewport: IRect) {
        pass.set_viewport(
            viewport.left as f32,
            viewport.top as f32,
            viewport.width() as f32,
            viewport.height() as f32,
            0.0,
            1.0,
        );
    }
}

/// How a load op reads, before the clear value is attached.
#[derive(Clone, Copy)]
enum LoadAction {
    Load,
    Clear,
}

/// A render pass being recorded, with the resolve step to do after it.
struct ActivePass<'enc> {
    pass: wgpu::RenderPass<'enc>,
    resolve_step_emulation_info: Option<ResolveStepEmulationInfo>,
}

impl CommandBufferBackend for WgpuCommandBufferBackend {
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L202-L228 (chrome/m156)
    fn set_new_command_buffer_resources(&mut self) -> bool {
        debug_assert!(self.command_encoder.is_none());
        self.command_encoder = Some(
            self.shared_context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default()),
        );
        true
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L178-L200 (chrome/m156)
    fn on_reset_command_buffer(&mut self) {
        self.command_encoder = None;

        for buffer_slot in &mut self.bound_uniforms {
            *buffer_slot = BindBufferInfo::default();
        }
        self.bound_uniform_buffers_dirty = true;
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L233-L296 (chrome/m156)
    fn on_add_render_pass(
        &mut self,
        state: &ReplayState,
        call: &mut RenderPassCall<'_>,
        tracker: &mut dyn ResourceTracker,
    ) -> bool {
        let Some(mut encoder) = self.command_encoder.take() else {
            skia_log_e!("The wgpu command buffer has no encoder");
            return false;
        };
        let result = self.add_render_pass(&mut encoder, state, call, tracker);
        self.command_encoder = Some(encoder);
        result
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L298-L321 (chrome/m156)
    fn on_add_compute_pass(
        &mut self,
        dispatch_groups: &mut [Box<DispatchGroup>],
        tracker: &mut dyn ResourceTracker,
    ) -> bool {
        let Some(mut encoder) = self.command_encoder.take() else {
            skia_log_e!("The wgpu command buffer has no encoder");
            return false;
        };
        let result = self.add_compute_pass(&mut encoder, dispatch_groups, tracker);
        self.command_encoder = Some(encoder);
        result
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1131-L1146 (chrome/m156)
    fn on_copy_buffer_to_buffer(
        &mut self,
        src_buffer: &Buffer,
        src_offset: usize,
        dst_buffer: &Buffer,
        dst_offset: usize,
        size: usize,
    ) -> bool {
        let (Some(wgpu_buffer_src), Some(wgpu_buffer_dst)) =
            (wgpu_buffer_of(src_buffer), wgpu_buffer_of(dst_buffer))
        else {
            return false;
        };
        let Some(encoder) = self.command_encoder.as_mut() else {
            return false;
        };

        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("copy_buffer_to_buffer")
                .u("src", buffer_id(src_buffer))
                .u("src_offset", src_offset as u64)
                .u("dst", buffer_id(dst_buffer))
                .u("dst_offset", dst_offset as u64)
                .u("size", size as u64)
        );
        encoder.copy_buffer_to_buffer(
            &wgpu_buffer_src,
            src_offset as u64,
            &wgpu_buffer_dst,
            dst_offset as u64,
            size as u64,
        );
        true
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1148-L1175 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // rects are non-negative texture regions
    fn on_copy_texture_to_buffer(
        &mut self,
        texture: &Texture,
        src_rect: IRect,
        buffer: &Buffer,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    ) -> bool {
        let (Some(wgpu_texture), Some(wgpu_buffer)) = (
            texture_of(texture).and_then(WgpuTexture::wgpu_texture),
            wgpu_buffer_of(buffer),
        ) else {
            return false;
        };
        let Some(encoder) = self.command_encoder.as_mut() else {
            return false;
        };

        let src = wgpu::TexelCopyTextureInfo {
            texture: &wgpu_texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: src_rect.left as u32,
                y: src_rect.top as u32,
                z: 0,
            },
            aspect: texture_aspect(texture),
        };
        let dst = wgpu::TexelCopyBufferInfo {
            buffer: &wgpu_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: buffer_offset as u64,
                bytes_per_row: Some(buffer_row_bytes as u32),
                rows_per_image: None,
            },
        };

        let copy_size = wgpu::Extent3d {
            width: src_rect.width() as u32,
            height: src_rect.height() as u32,
            depth_or_array_layers: 1,
        };
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("copy_texture_to_buffer")
                .u("texture", texture_id(texture))
                .i("x", src_rect.left)
                .i("y", src_rect.top)
                .u("width", copy_size.width)
                .u("height", copy_size.height)
                .u("buffer", buffer_id(buffer))
                .u("buffer_offset", buffer_offset as u64)
                .u("buffer_row_bytes", buffer_row_bytes as u64)
        );
        encoder.copy_texture_to_buffer(src, dst, copy_size);

        true
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1177-L1210 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // rects are non-negative texture regions
    fn on_copy_buffer_to_texture(
        &mut self,
        buffer: &Buffer,
        texture: &Texture,
        copy_data: &[BufferTextureCopyData],
    ) -> bool {
        let (Some(wgpu_texture), Some(wgpu_buffer)) = (
            texture_of(texture).and_then(WgpuTexture::wgpu_texture),
            wgpu_buffer_of(buffer),
        ) else {
            return false;
        };
        let Some(encoder) = self.command_encoder.as_mut() else {
            return false;
        };

        for data in copy_data {
            let src = wgpu::TexelCopyBufferInfo {
                buffer: &wgpu_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: data.buffer_offset as u64,
                    bytes_per_row: Some(data.buffer_row_bytes as u32),
                    rows_per_image: None,
                },
            };
            let dst = wgpu::TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: data.mip_level,
                origin: wgpu::Origin3d {
                    x: data.rect.left as u32,
                    y: data.rect.top as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            };

            let copy_size = wgpu::Extent3d {
                width: data.rect.width() as u32,
                height: data.rect.height() as u32,
                depth_or_array_layers: 1,
            };
            trace!(
                self.shared_context,
                crate::graphite::wgpu::trace::Record::new("copy_buffer_to_texture")
                    .u("buffer", buffer_id(buffer))
                    .u("buffer_offset", data.buffer_offset as u64)
                    .u("buffer_row_bytes", data.buffer_row_bytes as u64)
                    .u("texture", texture_id(texture))
                    .u("mip_level", data.mip_level)
                    .i("x", data.rect.left)
                    .i("y", data.rect.top)
                    .u("width", copy_size.width)
                    .u("height", copy_size.height)
            );
            encoder.copy_buffer_to_texture(src, dst, copy_size);
        }

        true
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1212-L1242 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // rects and points are non-negative texture coordinates
    fn on_copy_texture_to_texture(
        &mut self,
        src: &Texture,
        src_rect: IRect,
        dst: &Texture,
        dst_point: IPoint,
        mip_level: i32,
    ) -> bool {
        let (Some(wgpu_texture_src), Some(wgpu_texture_dst)) = (
            texture_of(src).and_then(WgpuTexture::wgpu_texture),
            texture_of(dst).and_then(WgpuTexture::wgpu_texture),
        ) else {
            return false;
        };
        let Some(encoder) = self.command_encoder.as_mut() else {
            return false;
        };

        let src_args = wgpu::TexelCopyTextureInfo {
            texture: &wgpu_texture_src,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: src_rect.left as u32,
                y: src_rect.top as u32,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        };
        let dst_args = wgpu::TexelCopyTextureInfo {
            texture: &wgpu_texture_dst,
            mip_level: mip_level as u32,
            origin: wgpu::Origin3d {
                x: dst_point.x as u32,
                y: dst_point.y as u32,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        };

        let copy_size = wgpu::Extent3d {
            width: src_rect.width() as u32,
            height: src_rect.height() as u32,
            depth_or_array_layers: 1,
        };

        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("copy_texture_to_texture")
                .u("src", texture_id(src))
                .i("src_x", src_rect.left)
                .i("src_y", src_rect.top)
                .u("dst", texture_id(dst))
                .i("dst_x", dst_point.x)
                .i("dst_y", dst_point.y)
                .i("mip_level", mip_level)
                .u("width", copy_size.width)
                .u("height", copy_size.height)
        );
        encoder.copy_texture_to_texture(src_args, dst_args, copy_size);

        true
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1244-L1246 (chrome/m156)
    fn on_synchronize_buffer_to_cpu(
        &mut self,
        _buffer: &Buffer,
        _did_result_in_work: &mut bool,
    ) -> bool {
        true
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp#L1248-L1259 (chrome/m156)
    fn on_clear_buffer(&mut self, buffer: &Buffer, offset: usize, size: usize) -> bool {
        let Some(wgpu_buffer) = wgpu_buffer_of(buffer) else {
            return false;
        };
        let Some(encoder) = self.command_encoder.as_mut() else {
            return false;
        };
        trace!(
            self.shared_context,
            crate::graphite::wgpu::trace::Record::new("clear_buffer")
                .u("buffer", buffer_id(buffer))
                .u("offset", offset as u64)
                .u("size", size as u64)
        );
        encoder.clear_buffer(&wgpu_buffer, offset as u64, Some(size as u64));

        true
    }
}

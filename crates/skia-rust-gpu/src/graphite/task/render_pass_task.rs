// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/RenderPassTask.h,
//                   src/gpu/graphite/task/RenderPassTask.cpp

//! `RenderPassTask`: prepares and records `DrawPass`es into a render pass.
//!
//! `DrawPass` is ported with G10a. The task is written against the [`DrawPass`] trait with the
//! members it uses; the pass is moved into the task as a `Box<dyn DrawPass>`.

use std::fmt::Debug;
use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

use crate::gpu::backing_fit::get_approx_size;
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::caps::{AttachmentSizePolicy, Caps};
use crate::graphite::command_buffer::{CommandBuffer, ResourceTracker};
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::draw_pass::DrawPassCommand;
use crate::graphite::draw_types::PipelineStageFlags;
use crate::graphite::graphics_pipeline::GraphicsPipeline;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::graphite_types::SampleCount;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::resource_types::{Discardable, LoadOp, StoreOp};
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::{ReplayTargetData, Status, Task, TaskRef};
use crate::graphite::texture::Texture;
use crate::graphite::texture_format::{TextureFormat, texture_format_is_depth_or_stencil};
use crate::graphite::texture_proxy::TextureProxy;

/// The members of `DrawPass` that `RenderPassTask` uses (G10a ports the class).
// Port of: src/gpu/graphite/DrawPass.h (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawPass")]
pub trait DrawPass: Send + Debug {
    /// `bounds()`.
    fn bounds(&self) -> IRect;

    /// `prepareResources()`.
    #[doc(alias = "prepareResources")]
    fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        render_pass_desc: &RenderPassDesc,
    ) -> bool;

    /// `pipelineHandles()`, resolved: `pipelineOrNull()` of every handle.
    #[doc(alias = "pipelineHandles")]
    fn pipelines(&self) -> Vec<Option<Arc<dyn GraphicsPipeline>>>;

    /// `sampledTextures()`.
    #[doc(alias = "sampledTextures")]
    fn sampled_textures(&self) -> &[Arc<TextureProxy>];

    /// `storageFallbackTexture()`.
    #[doc(alias = "storageFallbackTexture")]
    fn storage_fallback_texture(&self) -> Option<&Arc<TextureProxy>>;

    /// `target()`.
    fn target(&self) -> Option<&Arc<TextureProxy>> {
        None
    }

    /// `commands()`: the draw commands, in order.
    fn commands(&self) -> &[DrawPassCommand] {
        &[]
    }

    /// `storageBufferInfo()`: the `StorageContext` buffer of the pass, if any.
    #[doc(alias = "storageBufferInfo")]
    fn storage_buffer_info(&self) -> Option<&BindBufferInfo> {
        None
    }

    /// `storageBufferStages()`: valid after `addResourceRefs()`.
    #[doc(alias = "storageBufferStages")]
    fn storage_buffer_stages(&self) -> PipelineStageFlags {
        PipelineStageFlags::NONE
    }

    /// `ops()`.
    fn ops(&self) -> (LoadOp, StoreOp) {
        (LoadOp::Load, StoreOp::Store)
    }

    /// `clearColor()`.
    #[doc(alias = "clearColor")]
    fn clear_color(&self) -> [f32; 4] {
        [0.0; 4]
    }

    /// The pipeline descriptions the commands' `BindGraphicsPipeline` index, before
    /// `prepareResources()` turns them into handles (`fPipelineDescs`).
    fn pipeline_descs(&self) -> &[GraphicsPipelineDesc] {
        &[]
    }

    /// `addResourceRefs()`: resolves the pipeline handles and tracks the pass's resources on
    /// `command_buffer`. False if a pipeline could not be created, which drops the pass.
    #[doc(alias = "addResourceRefs")]
    fn add_resource_refs(&mut self, _tracker: &mut dyn ResourceTracker) -> bool {
        true
    }
}

// Get the required MSAA size for the render pass.
// In some scenarios, the MSAA size can be smaller than the target texture. As long as it is big
// enough to contain the draws' bounds.
// Port of: src/gpu/graphite/task/RenderPassTask.cpp#L40-L64 (chrome/m156)
fn get_msaa_size_and_resolve_offset(
    target_size: ISize,
    draw_bounds: IRect,
    caps: &dyn Caps,
    load_op: LoadOp,
) -> (ISize, IPoint) {
    if caps.attachment_size_policy() != AttachmentSizePolicy::Exact {
        // If possible, use approx size that can fit all draws. This reduces the MSAA texture
        // size and also reuses the textures better. When there is MSAA, we can do this for both
        // kApprox and kMSAARenderArea policies (even if in some cases we can make
        // kMSAARenderArea smaller).
        // Note: we don't do this if loadOp=Clear because it's supposed to update the whole
        // target texture.
        if caps.attachment_size_policy() == AttachmentSizePolicy::MsaaRenderArea
            && load_op != LoadOp::Clear
            && let Some(small_enough_bounds) =
                IRect::intersect(&draw_bounds, &IRect::from_size(target_size))
        {
            let resolve_offset = small_enough_bounds.top_left();
            return (get_approx_size(small_enough_bounds.size()), resolve_offset);
        }
        return (get_approx_size(target_size), IPoint::new(0, 0));
    }
    (target_size, IPoint::new(0, 0))
}

/// `RenderPassTask` handles preparing and recording `DrawList`s into a single render pass within
/// a command buffer. If the backend supports subpasses, and the `DrawList`s/surfaces are
/// compatible, a `RenderPassTask` can execute multiple `DrawList`s across different surfaces as
/// subpasses nested within a single render pass. If there is no such support, a `RenderPassTask`
/// is one-to-one with a "render pass" to specific surface.
// Port of: src/gpu/graphite/task/RenderPassTask.h#L31-L77 (chrome/m156)
#[doc(alias = "skgpu::graphite::RenderPassTask")]
#[derive(Debug)]
pub struct RenderPassTask {
    draw_passes: Vec<Box<dyn DrawPass>>,
    render_pass_desc: RenderPassDesc,
    target: Arc<TextureProxy>,
    dst_copy: Option<Arc<TextureProxy>>,
    dst_read_bounds: IRect,
}

impl RenderPassTask {
    /// `Make(passes, desc, target, dstCopy, dstReadBounds)`: `dst_copy` should only be provided
    /// if the draw passes require a texture copy for dst reads and must cover the union of all
    /// `DrawPass::dstReadBounds()` values in the render pass. It is assumed that the copy's
    /// (0,0) texel matches the top-left corner of the pass's dst copy bounds. The copy can be
    /// larger than the required bounds. `None` if there is no target.
    // Port of: src/gpu/graphite/task/RenderPassTask.cpp#L66-L122 (chrome/m156)
    #[allow(clippy::if_not_else)] // keeps the C++ branch order
    #[must_use]
    pub fn make(
        passes: Vec<Box<dyn DrawPass>>,
        desc: &RenderPassDesc,
        target: Option<Arc<TextureProxy>>,
        dst_copy: Option<Arc<TextureProxy>>,
        dst_read_bounds: IRect,
    ) -> Option<TaskRef> {
        // For now we have one DrawPass per RenderPassTask
        debug_assert_eq!(passes.len(), 1);

        // If we have a dst copy texture, ensure it is big enough to cover the copy bounds that
        // will be sampled.
        debug_assert!(dst_copy.as_ref().is_none_or(|dst_copy| {
            dst_copy.dimensions().width >= dst_read_bounds.width()
                && dst_copy.dimensions().height >= dst_read_bounds.height()
        }));

        let target = target?;

        if desc.color_resolve_attachment.format != TextureFormat::Unsupported {
            // The resolve attachment must match `target`, since that is what's resolved to.
            debug_assert!(
                desc.color_resolve_attachment
                    .is_compatible(target.texture_info())
            );
            // The resolve attachment should be single sampled and not depth/stencil
            debug_assert_eq!(desc.color_resolve_attachment.sample_count, SampleCount::One);
            debug_assert!(!texture_format_is_depth_or_stencil(
                desc.color_resolve_attachment.format
            ));
            // If there's a resolve attachment, the color attachment should have the same format
            // and more samples than the resolve.
            debug_assert_eq!(
                desc.color_attachment.format,
                desc.color_resolve_attachment.format
            );
            debug_assert!(desc.color_attachment.sample_count > SampleCount::One);
            // The render pass's sample count must match the color attachment's sample count
            debug_assert_eq!(desc.sample_count, desc.color_attachment.sample_count);
        } else {
            // The color attachment must match `target`, as it will be used to render directly
            // into.
            debug_assert!(desc.color_attachment.is_compatible(target.texture_info()));
            // The render pass's sample count must match or the color attachment's must be 1 and
            // the render pass has a higher sample count for msaa-render-to-single-sampled
            // extensions.
            debug_assert!(
                desc.color_attachment.sample_count == desc.sample_count
                    || (desc.color_attachment.sample_count == SampleCount::One
                        && desc.sample_count > SampleCount::One)
            );
        }

        if desc.depth_stencil_attachment.format != TextureFormat::Unsupported {
            // The sample count for any depth/stencil buffer must match the render pass.
            debug_assert!(texture_format_is_depth_or_stencil(
                desc.depth_stencil_attachment.format
            ));
            debug_assert_eq!(
                desc.depth_stencil_attachment.sample_count,
                desc.sample_count
            );
        }

        Some(
            Task::RenderPass(RenderPassTask {
                draw_passes: passes,
                render_pass_desc: desc.clone(),
                target,
                dst_copy,
                dst_read_bounds,
            })
            .into_ref(),
        )
    }

    /// The draw passes of the task (`fDrawPasses`).
    #[must_use]
    pub fn draw_passes(&self) -> &[Box<dyn DrawPass>] {
        &self.draw_passes
    }

    /// `prepareResources()`.
    // Port of: src/gpu/graphite/task/RenderPassTask.cpp#L136-L190 (chrome/m156)
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        scratch_manager: &mut ScratchResourceManager,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        let instantiated = if scratch_manager.pending_read_count(&self.target) == 0 {
            // TODO(b/389908339, b/338976898): If there are no pending reads on a scratch texture
            // instantiation request, it means that the scratch Device was caught by a
            // Recorder::flushTrackedDevices() event but hasn't actually been restored to its
            // parent. In this case, the eventual read of the surface will be in another
            // Recording and it can't be allocated as a true scratch resource.
            //
            // Without pending reads, DrawTask does not track its lifecycle to return the
            // scratch resource, so we need to match that and instantiate with a regular
            // non-shareable resource.
            TextureProxy::instantiate_if_not_lazy(resource_provider, &self.target)
        } else {
            TextureProxy::instantiate_if_not_lazy_scratch(
                scratch_manager,
                resource_provider,
                &self.target,
            )
        };
        if !instantiated {
            skia_log_w!("Failed to instantiate RenderPassTask target. Will not create renderpass!");
            skia_log_w!(
                "Dimensions are ({}, {}).",
                self.target.dimensions().width,
                self.target.dimensions().height
            );
            return Status::Fail;
        }

        // Assuming one draw pass per renderpasstask for now
        debug_assert_eq!(self.draw_passes.len(), 1);
        for draw_pass in &mut self.draw_passes {
            if !draw_pass.prepare_resources(resource_provider, runtime_dict, &self.render_pass_desc)
            {
                return Status::Fail;
            }
        }

        // Once all internal resources have been prepared and instantiated, reclaim any pending
        // returns from the scratch manager, since at the equivalent point in the task graph's
        // addCommands() phase, the renderpass will have sampled from any scratch textures and
        // their contents no longer have to be preserved.
        scratch_manager.notify_resources_consumed();

        Status::Success
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/RenderPassTask.cpp#L192-L284 (chrome/m156)
    #[allow(clippy::if_not_else)] // keeps the C++ branch order
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    pub fn add_commands(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_data: &ReplayTargetData,
    ) -> Status {
        // TBD: Expose the surfaces that will need to be attached within the renderpass?

        let caps = context.caps();

        // Instantiate the target
        debug_assert!(self.target.is_instantiated());
        debug_assert!(
            self.dst_copy
                .as_ref()
                .is_none_or(|dst_copy| dst_copy.is_instantiated())
        );

        // Assuming one draw pass per renderpasstask for now
        debug_assert_eq!(self.draw_passes.len(), 1);
        let draw_bounds = self.draw_passes[0].bounds();

        // Only apply the replay translation and clip if we're drawing to the final replay
        // target.
        let mut replay_translation = IPoint::new(0, 0);
        let mut replay_clip = IRect::new_empty();
        let target_texture = self
            .target
            .with_texture(|texture| texture.map(|texture| texture.as_arc().clone()));
        if replay_data.is_target(target_texture.as_ref()) {
            replay_translation = replay_data.translation;
            replay_clip = replay_data.clip;
        }

        // We don't instantiate the MSAA or DS attachments in prepareResources because we want
        // to use the discardable attachments from the Context.
        let resource_provider = context.resource_provider().clone();
        let mut resource_provider = resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let color_attachment: ResourceRef<Texture>;
        let mut resolve_attachment: Option<ResourceRef<Texture>> = None;
        let mut render_target_bounds = IRect::from_size(self.target.dimensions());
        let mut resolve_offset = IPoint::new(0, 0);
        if self.render_pass_desc.color_resolve_attachment.format != TextureFormat::Unsupported {
            // We always make color msaa attachments shareable. Between any render pass we
            // discard the values of the MSAA texture. Thus it is safe to be used by multiple
            // different render passes without worry of stomping on each other's data.
            // CommandBuffer::addRenderPass is responsible for loading this attachment with the
            // resolve target's original contents.
            let color_info = caps.get_default_attachment_texture_info(
                &self.render_pass_desc.color_attachment,
                self.target.is_protected(),
                Discardable::Yes,
            );
            let msaa_size;
            (msaa_size, resolve_offset) = get_msaa_size_and_resolve_offset(
                self.target.dimensions(),
                draw_bounds.with_offset(replay_translation),
                caps,
                self.render_pass_desc.color_attachment.load_op,
            );

            // Update the render target bounds to the possibly smaller MSAA attachment when using
            // the MSAARenderArea strategy. For kExact, there's no change in the
            // renderTargetArea and when it's kApprox, we want to keep the renderTargetArea
            // matching the resolve target's size.
            if caps.attachment_size_policy() == AttachmentSizePolicy::MsaaRenderArea {
                render_target_bounds = IRect::from_size(msaa_size);
            }

            let Some(attachment) = resource_provider.find_or_create_shareable_texture(
                msaa_size,
                &color_info,
                "DiscardableMSAAAttachment",
            ) else {
                skia_log_w!("Could not get Color attachment for RenderPassTask");
                return Status::Fail;
            };
            color_attachment = attachment;
            resolve_attachment = self.target.ref_texture();
        } else {
            let Some(attachment) = self.target.ref_texture() else {
                return Status::Fail;
            };
            color_attachment = attachment;
        }

        let mut depth_stencil_attachment: Option<ResourceRef<Texture>> = None;
        if self.render_pass_desc.depth_stencil_attachment.format != TextureFormat::Unsupported {
            debug_assert!(!caps.avoid_depth_mode());
            // We always make depth and stencil attachments shareable. Between any render pass
            // the values are reset. Thus it is safe to be used by multiple different render
            // passes without worry of stomping on each other's data.
            let ds_info = caps.get_default_attachment_texture_info(
                &self.render_pass_desc.depth_stencil_attachment,
                self.target.is_protected(),
                Discardable::Yes,
            );
            let mut dimensions = caps.get_depth_attachment_dimensions(
                color_attachment.texture_info(),
                color_attachment.dimensions(),
            );

            // Only adjust the depth dimensions when the policy is kApprox. When there is MSAA,
            // the color attachment dimensions have already been adjusted by the policy so the
            // call to GetApproxSize is a no-op. When there is no MSAA, the kMSAARenderArea
            // policy behaves the same as kExact.
            if caps.attachment_size_policy() == AttachmentSizePolicy::Approx {
                dimensions = get_approx_size(dimensions);
            }

            let Some(attachment) = resource_provider.find_or_create_shareable_texture(
                dimensions,
                &ds_info,
                "DepthStencilAttachment",
            ) else {
                skia_log_w!("Could not get DepthStencil attachment for RenderPassTask");
                return Status::Fail;
            };
            depth_stencil_attachment = Some(attachment);
        }
        drop(resource_provider);

        // The clip set here will intersect with the render target bounds, and then any scissor
        // set during this render pass. If there is no intersection between the clip and the
        // render target bounds, we can skip this entire render pass.
        // Note: if the MSAA texture is allocated smaller than the target texture, we need to
        // apply an additional translation (-resolveOffset) so that the draws' bounds' top left
        // corner will be at (0, 0) on the MSAA texture
        if !command_buffer.set_replay_translation_and_clip(
            replay_translation - resolve_offset,
            replay_clip,
            render_target_bounds,
        ) {
            return Status::Success;
        }

        // TODO(b/313629288) we always pass in the render target's dimensions as the viewport
        // here. Using the dimensions of the logical device that we're drawing to could reduce
        // flakiness in rendering.
        let dst_copy_texture = self.dst_copy.as_ref().and_then(|dst_copy| {
            dst_copy.with_texture(|texture| texture.map(|t| t.as_arc().clone()))
        });
        if command_buffer.add_render_pass(
            &self.render_pass_desc,
            color_attachment,
            resolve_attachment,
            depth_stencil_attachment,
            dst_copy_texture.as_ref(),
            self.dst_read_bounds,
            resolve_offset,
            self.target.dimensions(),
            &mut self.draw_passes,
        ) {
            Status::Success
        } else {
            Status::Fail
        }
    }

    /// `visitPipelines()`.
    // Port of: src/gpu/graphite/task/RenderPassTask.cpp#L286-L297 (chrome/m156)
    pub fn visit_pipelines(
        &mut self,
        visitor: &mut dyn FnMut(Option<&dyn GraphicsPipeline>) -> bool,
    ) -> bool {
        for pass in &self.draw_passes {
            for pipeline in pass.pipelines() {
                if !visitor(pipeline.as_deref()) {
                    return false;
                }
            }
        }
        true
    }

    /// `visitProxies()`.
    // Port of: src/gpu/graphite/task/RenderPassTask.cpp#L299-L327 (chrome/m156)
    pub fn visit_proxies(
        &mut self,
        visitor: &mut dyn FnMut(&Arc<TextureProxy>) -> bool,
        reads_only: bool,
    ) -> bool {
        for pass in &self.draw_passes {
            for proxy in pass.sampled_textures() {
                if !visitor(proxy) {
                    return false;
                }
            }
            if let Some(storage_fallback) = pass.storage_fallback_texture()
                && !visitor(storage_fallback)
            {
                return false;
            }
            if let Some(dst_copy) = &self.dst_copy
                && !visitor(dst_copy)
            {
                return false;
            }
            // Skip visiting the target if we're only visiting read textures
            if !reads_only && !visitor(&self.target) {
                return false;
            }
        }
        true
    }
}

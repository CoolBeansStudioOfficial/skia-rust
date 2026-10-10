// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/compute/DispatchGroup.h and DispatchGroup.cpp (chrome/m156)

//! [`DispatchGroup`]: a series of compute pipeline dispatches that run sequentially, and its
//! [`Builder`], which allocates the resources the steps bind.
//!
//! A group is built from [`ComputeStep`]s with a [`Builder`]: each step's resources are either
//! allocated (`Private`, or the first `Shared` use of a slot) or taken from the slot table filled
//! by an earlier step. Finalized groups are immutable and are recorded by a
//! [`ComputeTask`](crate::graphite::task::compute_task::ComputeTask).

use std::rc::Rc;
use std::sync::Arc;

use crate::gpu::sk_log::skia_log_w;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::caps::Caps;
use crate::graphite::command_buffer::ResourceTracker;
use crate::graphite::compute::compute_step::{
    ComputeStep, DataFlow, MAX_COMPUTE_DATA_FLOW_SLOTS, ResourceDesc, ResourcePolicy,
    ResourceType, WorkgroupBufferDesc, WorkgroupSize,
};
use crate::graphite::compute_pipeline::ComputePipeline;
use crate::graphite::compute_pipeline_desc::ComputePipelineDesc;
use crate::graphite::buffer_manager::DrawBufferManager;
use crate::graphite::recorder::{Recorder, RecorderInner};
use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_provider::ResourceProvider;
use crate::gpu::gpu_types::Budgeted;
use crate::graphite::resource_types::{ClearBuffer, SamplerDesc};
use crate::graphite::sampler::Sampler;
use crate::graphite::task::TaskRef;
use crate::graphite::task::clear_buffers_task::ClearBuffersTask;
use crate::graphite::texture::Texture;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::uniform_manager::UniformManager;

/// `BindingIndex`.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L26 (chrome/m156)
pub type BindingIndex = u32;

/// `TextureIndex`: an index into the group's textures.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L28 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureIndex(pub u32);

/// `SamplerIndex`: an index into the group's samplers.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L29 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamplerIndex(pub u32);

/// `DispatchResource`: what a dispatch binds (`std::variant<BindBufferInfo, TextureIndex,
/// SamplerIndex>`).
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L30 (chrome/m156)
#[derive(Clone, Debug)]
pub enum BindingResource {
    /// A range of a buffer.
    Buffer(BindBufferInfo),
    /// A texture of the group.
    Texture(TextureIndex),
    /// A sampler of the group.
    Sampler(SamplerIndex),
}

/// `ResourceBinding`.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L36-L41 (chrome/m156)
#[derive(Clone, Debug)]
pub struct ResourceBinding {
    /// `fIndex`: the binding index in the group's bind group.
    pub index: BindingIndex,
    /// `fResource`.
    pub resource: BindingResource,
}

/// `Dispatch::fGlobalSizeOrIndirect`: the number of workgroups, or a buffer holding it.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L43-L49 (chrome/m156)
#[derive(Clone, Debug)]
pub enum GlobalSizeOrIndirect {
    /// The number of workgroups.
    Size(WorkgroupSize),
    /// A buffer holding the number of workgroups.
    Indirect(BindBufferInfo),
}

/// `DispatchGroup::Dispatch`.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L43-L49 (chrome/m156)
#[doc(alias = "skgpu::graphite::DispatchGroup::Dispatch")]
#[derive(Clone, Debug)]
pub struct Dispatch {
    /// `fLocalSize`.
    pub local_size: WorkgroupSize,
    /// `fGlobalSizeOrIndirect`.
    pub global_size_or_indirect: GlobalSizeOrIndirect,
    /// `fGlobalDispatchSize`: only set by a backend that needs the resolved size up front.
    pub global_dispatch_size: Option<WorkgroupSize>,
    /// `fBindings`.
    pub bindings: Vec<ResourceBinding>,
    /// `fWorkgroupBuffers`.
    pub workgroup_buffers: Vec<WorkgroupBufferDesc>,
    /// `fPipelineIndex`.
    pub pipeline_index: u32,
}

/// `DispatchGroup::OutputTable`: what each shared slot of a finished step holds.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L124-L133 (chrome/m156)
#[doc(alias = "skgpu::graphite::DispatchGroup::Builder::OutputTable")]
#[derive(Clone, Debug, Default)]
pub struct OutputTable {
    /// `fSharedSlots`: `None` is `std::monostate`.
    pub shared_slots: [Option<BindingResource>; MAX_COMPUTE_DATA_FLOW_SLOTS as usize],
}

impl OutputTable {
    /// `reset()`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.h#L131 (chrome/m156)
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `DispatchGroup`: a series of compute pipeline dispatches that need to execute sequentially
/// (i.e. with a barrier), in the order they are encoded.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L51-L104 (chrome/m156)
#[doc(alias = "skgpu::graphite::DispatchGroup")]
#[derive(Debug, Default)]
pub struct DispatchGroup {
    dispatch_list: Vec<Dispatch>,
    clear_list: Vec<BindBufferInfo>,
    pipeline_descs: Vec<ComputePipelineDesc>,
    sampler_descs: Vec<SamplerDesc>,
    pipelines: Vec<Arc<dyn ComputePipeline>>,
    textures: Vec<Arc<TextureProxy>>,
    samplers: Vec<ResourceRef<Sampler>>,
}

impl DispatchGroup {
    /// `dispatches()`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.h#L62 (chrome/m156)
    #[must_use]
    pub fn dispatches(&self) -> &[Dispatch] {
        &self.dispatch_list
    }

    /// `getPipeline(index)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.h#L63 (chrome/m156)
    #[doc(alias = "getPipeline")]
    #[must_use]
    pub fn pipeline(&self, index: u32) -> Option<Arc<dyn ComputePipeline>> {
        self.pipelines.get(index as usize).cloned()
    }

    /// `getTexture(index)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L76-L82 (chrome/m156)
    #[doc(alias = "getTexture")]
    #[must_use]
    pub fn texture(&self, index: u32) -> Option<ResourceRef<Texture>> {
        self.textures.get(index as usize)?.ref_texture()
    }

    /// `getSampler(index)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L84-L90 (chrome/m156)
    #[doc(alias = "getSampler")]
    #[must_use]
    pub fn sampler(&self, index: u32) -> Option<ResourceRef<Sampler>> {
        self.samplers.get(index as usize).cloned()
    }

    /// `prepareResources()`: creates the pipelines, samplers and checks the textures.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L28-L63 (chrome/m156)
    #[doc(alias = "prepareResources")]
    pub fn prepare_resources(&mut self, resource_provider: &mut ResourceProvider) -> bool {
        self.pipelines.reserve(self.pipeline_descs.len());
        for desc in &self.pipeline_descs {
            let Some(pipeline) = resource_provider.find_or_create_compute_pipeline(desc) else {
                skia_log_w!("Failed to create ComputePipeline for dispatch group. Dropping group!");
                return false;
            };
            self.pipelines.push(pipeline);
        }

        for texture in &self.textures {
            if !texture.texture_info().is_valid() {
                skia_log_w!("Failed to validate bound texture. Dropping dispatch group!");
                return false;
            }
            if !TextureProxy::instantiate_if_not_lazy(resource_provider, texture) {
                skia_log_w!("Failed to instantiate bound texture. Dropping dispatch group!");
                return false;
            }
        }

        for desc in &self.sampler_descs {
            let Some(sampler) = resource_provider.find_or_create_compatible_sampler(desc) else {
                skia_log_w!("Failed to create sampler. Dropping dispatch group!");
                return false;
            };
            self.samplers.push(sampler);
        }

        self.pipeline_descs.clear();
        self.sampler_descs.clear();
        true
    }

    /// `addResourceRefs(CommandBuffer*)`: tracks the group's pipelines and textures.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L65-L73 (chrome/m156)
    #[doc(alias = "addResourceRefs")]
    pub fn add_resource_refs(&self, tracker: &mut dyn ResourceTracker) {
        for texture in &self.textures {
            if let Some(texture) = texture.ref_texture() {
                tracker.track_resource(texture.into_any());
            }
        }
    }

    /// `snapChildTask()`: the clears of this group's cleared shared buffers, if any.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L92-L97 (chrome/m156)
    #[doc(alias = "snapChildTask")]
    pub fn snap_child_task(&mut self) -> Option<TaskRef> {
        if self.clear_list.is_empty() {
            return None;
        }
        Some(ClearBuffersTask::make(std::mem::take(&mut self.clear_list)))
    }
}

/// `DispatchGroup::Builder`: builds a [`DispatchGroup`] from compute steps.
// Port of: src/gpu/graphite/compute/DispatchGroup.h#L135-L178 (chrome/m156)
#[doc(alias = "skgpu::graphite::DispatchGroup::Builder")]
#[derive(Debug)]
pub struct Builder {
    obj: Option<DispatchGroup>,
    recorder: Rc<RecorderInner>,
    output_table: OutputTable,
}

impl Builder {
    /// `Builder(Recorder*)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L99-L101 (chrome/m156)
    #[must_use]
    pub fn new(recorder: &Recorder) -> Self {
        Self {
            obj: Some(DispatchGroup::default()),
            recorder: recorder.inner(),
            output_table: OutputTable::default(),
        }
    }

    /// `outputTable()`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.h#L146 (chrome/m156)
    #[must_use]
    pub fn output_table(&self) -> &OutputTable {
        &self.output_table
    }

    /// `appendStep(step, globalSize)`: the global size defaults to
    /// `step->calculateGlobalDispatchSize()`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L103-L106 (chrome/m156)
    #[must_use]
    pub fn append_step(
        &mut self,
        step: &Arc<dyn ComputeStep>,
        global_size: Option<WorkgroupSize>,
    ) -> bool {
        let global = global_size.unwrap_or_else(|| step.calculate_global_dispatch_size());
        self.append_step_internal(step, GlobalSizeOrIndirect::Size(global))
    }

    /// `appendStepIndirect(step, indirectBuffer)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L108-L110 (chrome/m156)
    #[must_use]
    pub fn append_step_indirect(
        &mut self,
        step: &Arc<dyn ComputeStep>,
        indirect_buffer: BindBufferInfo,
    ) -> bool {
        self.append_step_internal(step, GlobalSizeOrIndirect::Indirect(indirect_buffer))
    }

    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L112-L225 (chrome/m156)
    fn append_step_internal(
        &mut self,
        step: &Arc<dyn ComputeStep>,
        global_size_or_indirect: GlobalSizeOrIndirect,
    ) -> bool {
        let mut dispatch_bindings: Vec<ResourceBinding> = Vec::new();
        let resources = step.resources();
        dispatch_bindings.reserve(resources.len());
        let mut next_index = 0usize;

        let caps = self.recorder.priv_().caps().clone();
        let binding_reqs = caps.resource_binding_requirements();
        let separate_sampler = binding_reqs.separate_texture_and_sampler_binding;
        let textures_use_distinct_idx_ranges =
            binding_reqs.compute_uses_distinct_idx_ranges_for_textures;
        debug_assert!(!(separate_sampler && textures_use_distinct_idx_ranges));

        let mut buffer_or_global_index: BindingIndex = 0;
        let mut tex_index: BindingIndex = 0;
        for r in resources {
            debug_assert!(r.slot == -1 || (r.slot >= 0 && r.slot < MAX_COMPUTE_DATA_FLOW_SLOTS));
            let index = next_index;
            next_index += 1;
            let maybe_resource: Option<BindingResource> = match r.flow {
                DataFlow::Private => {
                    debug_assert!(r.ty != ResourceType::ReadOnlyTexture);
                    debug_assert!(r.ty != ResourceType::SampledTexture);
                    self.allocate_resource(step.as_ref(), r, index)
                }
                DataFlow::Shared => {
                    debug_assert!(r.slot >= 0);
                    let slot = usize::try_from(r.slot).unwrap_or(0);
                    if self.output_table.shared_slots[slot].is_none() {
                        debug_assert!(r.ty != ResourceType::ReadOnlyTexture);
                        debug_assert!(r.ty != ResourceType::SampledTexture);
                        let allocated = self.allocate_resource(step.as_ref(), r, index);
                        self.output_table.shared_slots[slot] = allocated.clone();
                        allocated
                    } else {
                        let existing = self.output_table.shared_slots[slot].clone();
                        if r.ty == ResourceType::SampledTexture {
                            debug_assert!(matches!(existing, Some(BindingResource::Texture(_))));
                            let sampler_resource = self.allocate_resource(step.as_ref(), r, index);
                            let Some(BindingResource::Sampler(sampler_idx)) = sampler_resource
                            else {
                                debug_assert!(false, "a sampled texture allocates a sampler");
                                return false;
                            };
                            let binding_index = if textures_use_distinct_idx_ranges {
                                tex_index
                            } else if separate_sampler {
                                let current = buffer_or_global_index;
                                buffer_or_global_index += 1;
                                current
                            } else {
                                buffer_or_global_index
                            };
                            dispatch_bindings.push(ResourceBinding {
                                index: binding_index,
                                resource: BindingResource::Sampler(sampler_idx),
                            });
                        }
                        existing
                    }
                }
            };

            let Some(resource) = maybe_resource else {
                skia_log_w!("Failed to allocate resource for compute dispatch");
                return false;
            };
            let binding_index = match resource {
                BindingResource::Buffer(_) => {
                    let current = buffer_or_global_index;
                    buffer_or_global_index += 1;
                    current
                }
                BindingResource::Texture(_) => {
                    if textures_use_distinct_idx_ranges {
                        let current = tex_index;
                        tex_index += 1;
                        current
                    } else {
                        let current = buffer_or_global_index;
                        buffer_or_global_index += 1;
                        current
                    }
                }
                BindingResource::Sampler(_) => {
                    skia_log_w!("Failed to allocate resource for compute dispatch");
                    return false;
                }
            };
            dispatch_bindings.push(ResourceBinding {
                index: binding_index,
                resource,
            });
        }

        let workgroup_buffers = step.workgroup_buffers().to_vec();

        let obj = self
            .obj
            .as_mut()
            .expect("a Builder appends steps only before it is finalized");
        if obj
            .pipeline_descs
            .last()
            .is_none_or(|desc| desc.unique_id() != step.unique_id())
        {
            obj.pipeline_descs.push(ComputePipelineDesc::new(step.clone()));
        }
        let pipeline_index = u32::try_from(obj.pipeline_descs.len() - 1).unwrap_or(u32::MAX);

        obj.dispatch_list.push(Dispatch {
            local_size: step.local_dispatch_size(),
            global_size_or_indirect,
            global_dispatch_size: None,
            bindings: dispatch_bindings,
            workgroup_buffers,
            pipeline_index,
        });
        true
    }

    /// `assignSharedBuffer(buffer, slot, cleared)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L227-L237 (chrome/m156)
    pub fn assign_shared_buffer(
        &mut self,
        buffer: BindBufferInfo,
        slot: usize,
        cleared: ClearBuffer,
    ) {
        debug_assert!(buffer.is_valid());
        debug_assert!(buffer.size != 0);
        self.output_table.shared_slots[slot] = Some(BindingResource::Buffer(buffer.clone()));
        if cleared == ClearBuffer::Yes {
            if let Some(obj) = self.obj.as_mut() {
                obj.clear_list.push(buffer);
            }
        }
    }

    /// `assignSharedTexture(texture, slot)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L239-L245 (chrome/m156)
    pub fn assign_shared_texture(&mut self, texture: Arc<TextureProxy>, slot: usize) {
        let obj = self
            .obj
            .as_mut()
            .expect("a Builder assigns textures only before it is finalized");
        obj.textures.push(texture);
        let index = u32::try_from(obj.textures.len() - 1).unwrap_or(u32::MAX);
        self.output_table.shared_slots[slot] = Some(BindingResource::Texture(TextureIndex(index)));
    }

    /// `reset()`: starts a new, empty group and forgets the slot table. Skia compiles this only
    /// with `GPU_TEST_UTILS`; it is public here because the compute tests reuse one builder.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L255-L260 (chrome/m156)
    pub fn reset(&mut self) {
        self.output_table.reset();
        self.obj = Some(DispatchGroup::default());
    }

    /// `finalize()`: the group, which is immutable from here on. The builder is reset.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L247-L252 (chrome/m156)
    #[must_use]
    pub fn finalize(&mut self) -> Box<DispatchGroup> {
        let obj = self.obj.take().unwrap_or_default();
        self.output_table.reset();
        Box::new(obj)
    }

    /// `getSharedBufferResource(slot)`: the buffer at `slot`, or an invalid range.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L265-L273 (chrome/m156)
    #[must_use]
    pub fn get_shared_buffer_resource(&self, slot: usize) -> BindBufferInfo {
        match &self.output_table.shared_slots[slot] {
            Some(BindingResource::Buffer(info)) => info.clone(),
            _ => BindBufferInfo::default(),
        }
    }

    /// `getSharedTextureResource(slot)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L275-L285 (chrome/m156)
    #[must_use]
    pub fn get_shared_texture_resource(&self, slot: usize) -> Option<Arc<TextureProxy>> {
        let Some(BindingResource::Texture(idx)) = &self.output_table.shared_slots[slot] else {
            return None;
        };
        self.obj.as_ref()?.textures.get(idx.0 as usize).cloned()
    }

    /// `allocateResource(step, resource, resourceIdx)`.
    // Port of: src/gpu/graphite/compute/DispatchGroup.cpp#L287-L382 (chrome/m156)
    fn allocate_resource(
        &mut self,
        step: &dyn ComputeStep,
        resource: &ResourceDesc,
        resource_idx: usize,
    ) -> Option<BindingResource> {
        // The recorder is reached through its own handle, so `self.obj` stays free to borrow.
        let recorder = self.recorder.clone();
        let priv_ = recorder.priv_();
        let buffer_mgr: &DrawBufferManager = priv_.draw_buffer_manager();
        let caps: Arc<dyn Caps> = priv_.caps().clone();

        match resource.ty {
            ResourceType::ReadOnlyStorageBuffer | ResourceType::StorageBuffer => {
                let buffer_size = step.calculate_buffer_size(resource_idx, resource);
                debug_assert!(buffer_size != 0);
                if resource.policy == ResourcePolicy::Mapped {
                    let mut mapped = buffer_mgr.get_mapped_storage_buffer(buffer_size, 1)?;
                    step.prepare_storage_buffer(resource_idx, resource, mapped.writer());
                    Some(BindingResource::Buffer(mapped.binding.clone()))
                } else {
                    let cleared = if resource.policy == ResourcePolicy::Clear {
                        ClearBuffer::Yes
                    } else {
                        ClearBuffer::No
                    };
                    let buf_info = buffer_mgr.get_storage(buffer_size, cleared);
                    buf_info.is_valid().then_some(BindingResource::Buffer(buf_info))
                }
            }
            ResourceType::IndirectBuffer => {
                debug_assert!(resource.policy != ResourcePolicy::Mapped);
                let buffer_size = step.calculate_buffer_size(resource_idx, resource);
                debug_assert!(buffer_size != 0);
                let cleared = if resource.policy == ResourcePolicy::Clear {
                    ClearBuffer::Yes
                } else {
                    ClearBuffer::No
                };
                let buf_info = buffer_mgr.get_indirect_storage(buffer_size, cleared);
                buf_info.is_valid().then_some(BindingResource::Buffer(buf_info))
            }
            ResourceType::UniformBuffer => {
                debug_assert!(resource.policy == ResourcePolicy::Mapped);
                let resource_reqs = caps.resource_binding_requirements();
                let mut ubo_mgr = UniformManager::new(resource_reqs.uniform_buffer_layout);
                step.prepare_uniform_buffer(resource_idx, resource, &mut ubo_mgr);
                let data_block = ubo_mgr.finish();
                debug_assert!(!data_block.is_empty());
                let mut mapped = buffer_mgr.get_mapped_uniform_buffer(data_block.len(), 0)?;
                mapped.writer().write_bytes(data_block);
                Some(BindingResource::Buffer(mapped.binding.clone()))
            }
            ResourceType::WriteOnlyStorageTexture => {
                let (size, color_type) = step.calculate_texture_parameters(resource_idx, resource);
                debug_assert!(!size.is_empty());
                let texture_info = caps.get_default_storage_texture_info(color_type);
                let mut rp = recorder.lock_resource_provider();
                let texture = TextureProxy::make(
                    caps.as_ref(),
                    &mut rp,
                    size,
                    &texture_info,
                    Budgeted::Yes,
                    "DispatchWriteOnlyStorageTexture",
                )?;
                drop(rp);
                let obj = self.obj_mut();
                obj.textures.push(texture);
                let index = u32::try_from(obj.textures.len() - 1).unwrap_or(u32::MAX);
                Some(BindingResource::Texture(TextureIndex(index)))
            }
            ResourceType::ReadOnlyTexture => {
                panic!("a readonly texture must be externally assigned to a ComputeStep");
            }
            ResourceType::SampledTexture => {
                let obj = self.obj_mut();
                obj.sampler_descs
                    .push(step.calculate_sampler_parameters(resource_idx, resource));
                let index = u32::try_from(obj.sampler_descs.len() - 1).unwrap_or(u32::MAX);
                Some(BindingResource::Sampler(SamplerIndex(index)))
            }
        }
    }

    fn obj_mut(&mut self) -> &mut DispatchGroup {
        self.obj
            .as_mut()
            .expect("a Builder allocates resources only before it is finalized")
    }
}

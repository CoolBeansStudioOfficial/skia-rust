// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/compute/ComputeStep.h, ComputeStep.cpp and
// src/gpu/graphite/ComputeTypes.h (the parts the compute pipelines read)

//! [`ComputeStep`]: a compute program and its data binding layout.
//!
//! This is the interface of `ComputeStep` as the backend pipelines see it: the resource list, the
//! shader (`SkSL` or native WGSL), the workgroup size and the texture parameters. The hooks that
//! fill buffers (`prepareStorageBuffer`, `prepareUniformBuffer`, which take a `BufferWriter` and
//! the `UniformManager`) come with `DispatchGroup` in G13.

use std::fmt::Debug;
use std::sync::atomic::{AtomicU32, Ordering};

use skia_rust_core::color_type::ColorType;
use skia_rust_core::size::ISize;

use crate::gpu::buffer_writer::BufferWriter;
use crate::graphite::resource_types::SamplerDesc;
use crate::graphite::uniform_manager::UniformManager;

/// `kMaxComputeDataFlowSlots`: the maximum number of shared resource binding slots permitted for
/// `ComputeStep`s of a `DispatchGroup`.
// Port of: src/gpu/graphite/ComputeTypes.h#L16-L17 (chrome/m156)
#[doc(alias = "kMaxComputeDataFlowSlots")]
pub const MAX_COMPUTE_DATA_FLOW_SLOTS: i32 = 28;

/// `kIndirectDispatchArgumentSize`: the size of the `IndirectDispatchArgs` a `kIndirectBuffer`
/// holds (three workgroup counts, `global_size_x`, `global_size_y` and `global_size_z`).
// Port of: src/gpu/graphite/ComputeTypes.h#L20-L25 (chrome/m156)
#[doc(alias = "kIndirectDispatchArgumentSize")]
pub const INDIRECT_DISPATCH_ARGUMENT_SIZE: usize = 3 * std::mem::size_of::<u32>();

/// `WorkgroupSize`: the space that a compute shader operates on. The "work group count" (global
/// size) and the local size of a work group are both expressed with it.
// Port of: src/gpu/graphite/ComputeTypes.h#L46-L58 (chrome/m156)
#[doc(alias = "skgpu::graphite::WorkgroupSize")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkgroupSize {
    /// `fWidth`.
    pub width: u32,
    /// `fHeight`.
    pub height: u32,
    /// `fDepth`.
    pub depth: u32,
}

impl Default for WorkgroupSize {
    /// `WorkgroupSize()`: (1, 1, 1).
    fn default() -> Self {
        Self::new(1, 1, 1)
    }
}

impl WorkgroupSize {
    /// `WorkgroupSize(width, height, depth)`.
    #[must_use]
    pub const fn new(width: u32, height: u32, depth: u32) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    /// `scalarSize()`.
    #[doc(alias = "scalarSize")]
    #[must_use]
    pub const fn scalar_size(&self) -> u32 {
        self.width * self.height * self.depth
    }
}

/// `ComputeStep::DataFlow`.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L67-L77 (chrome/m156)
#[doc(alias = "ComputeStep::DataFlow")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFlow {
    /// `kPrivate`: a resource that is only visible to a single `ComputeStep` invocation.
    Private,
    /// `kShared`: bindings with a slot number that can be used to forward data between a series
    /// of `ComputeStep`s.
    Shared,
}

/// `ComputeStep::ResourceType`.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L79-L94 (chrome/m156)
#[doc(alias = "ComputeStep::ResourceType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceType {
    /// `kUniformBuffer`.
    UniformBuffer,
    /// `kStorageBuffer`.
    StorageBuffer,
    /// `kReadOnlyStorageBuffer`.
    ReadOnlyStorageBuffer,
    /// `kIndirectBuffer`: a storage buffer populated by this step to determine the global
    /// dispatch size of a subsequent step in the same `DispatchGroup`.
    IndirectBuffer,
    /// `kWriteOnlyStorageTexture`.
    WriteOnlyStorageTexture,
    /// `kReadOnlyTexture`.
    ReadOnlyTexture,
    /// `kSampledTexture`.
    SampledTexture,
}

/// `ComputeStep::ResourcePolicy`.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L96-L112 (chrome/m156)
#[doc(alias = "ComputeStep::ResourcePolicy")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourcePolicy {
    /// `kNone`.
    None,
    /// `kClear`: the memory of the resource will be initialized to 0.
    Clear,
    /// `kMapped`: the step is asked to initialize the memory on the CPU prior to execution.
    Mapped,
}

/// `ComputeStep::ResourceDesc`.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L114-L150 (chrome/m156)
#[doc(alias = "ComputeStep::ResourceDesc")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceDesc {
    /// `fType`.
    pub ty: ResourceType,
    /// `fFlow`.
    pub flow: DataFlow,
    /// `fPolicy`.
    pub policy: ResourcePolicy,
    /// `fSlot`: only has meaning (and must have a non-negative value) if `flow` is
    /// [`DataFlow::Shared`].
    pub slot: i32,
    /// `fSkSL`: the `SkSL` variable declaration code excluding the layout and type definitions.
    /// Ignored for a `ComputeStep` that supports native shader source.
    pub sksl: &'static str,
}

impl ResourceDesc {
    /// `ResourceDesc(type, flow, policy, slot = -1)`.
    #[must_use]
    pub const fn new(ty: ResourceType, flow: DataFlow, policy: ResourcePolicy) -> Self {
        Self::with_slot_and_sksl(ty, flow, policy, -1, "")
    }

    /// `ResourceDesc(type, flow, policy, slot)`.
    #[must_use]
    pub const fn with_slot(
        ty: ResourceType,
        flow: DataFlow,
        policy: ResourcePolicy,
        slot: i32,
    ) -> Self {
        Self::with_slot_and_sksl(ty, flow, policy, slot, "")
    }

    /// `ResourceDesc(type, flow, policy, slot, sksl)`.
    #[must_use]
    pub const fn with_slot_and_sksl(
        ty: ResourceType,
        flow: DataFlow,
        policy: ResourcePolicy,
        slot: i32,
        sksl: &'static str,
    ) -> Self {
        Self {
            ty,
            flow,
            policy,
            slot,
            sksl,
        }
    }

    /// `ResourceDesc(type, flow, policy, sksl)`.
    #[must_use]
    pub const fn with_sksl(
        ty: ResourceType,
        flow: DataFlow,
        policy: ResourcePolicy,
        sksl: &'static str,
    ) -> Self {
        Self::with_slot_and_sksl(ty, flow, policy, -1, sksl)
    }
}

/// `ComputeStep::WorkgroupBufferDesc`: on platforms that support late bound workgroup shared
/// resources (e.g. Metal) a step can provide a list of memory sizes and binding indices.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L152-L158 (chrome/m156)
#[doc(alias = "ComputeStep::WorkgroupBufferDesc")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkgroupBufferDesc {
    /// The buffer size in bytes.
    pub size: u32,
    /// The binding index.
    pub index: u32,
}

/// `ComputeStep::NativeShaderFormat`.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L171-L174 (chrome/m156)
#[doc(alias = "ComputeStep::NativeShaderFormat")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeShaderFormat {
    /// `kWGSL`.
    Wgsl,
    /// `kMSL`.
    Msl,
}

/// `ComputeStep::NativeShaderSource`: the shader module's text and the name of its compute entry
/// point.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L175-L178 (chrome/m156)
#[doc(alias = "ComputeStep::NativeShaderSource")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeShaderSource<'a> {
    /// `fSource`.
    pub source: &'a str,
    /// `fEntryPoint`.
    pub entry_point: String,
}

/// The state every `ComputeStep` carries (the private members and the protected constructor of
/// `ComputeStep`). A concrete step holds one and returns it from [`ComputeStep::base`].
// Port of: src/gpu/graphite/compute/ComputeStep.h#L255-L291 (chrome/m156)
#[derive(Clone, Debug)]
pub struct ComputeStepBase {
    unique_id: u32,
    supports_native_shader: bool,
    name: String,
    resources: Vec<ResourceDesc>,
    workgroup_buffers: Vec<WorkgroupBufferDesc>,
    local_dispatch_size: WorkgroupSize,
}

/// `next_id()`: not worried about overflow, since a `Context` isn't expected to have that many
/// `ComputeStep`s.
// Port of: src/gpu/graphite/compute/ComputeStep.cpp#L19-L25 (chrome/m156)
fn next_id() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(0);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

impl ComputeStepBase {
    /// The `ComputeStep(name, localDispatchSize, resources, workgroupBuffers, baseFlags)`
    /// constructor. `supports_native_shader` is `Flags::kSupportsNativeShader`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L29-L52 (chrome/m156)
    #[must_use]
    pub fn new(
        name: &str,
        local_dispatch_size: WorkgroupSize,
        resources: &[ResourceDesc],
        workgroup_buffers: &[WorkgroupBufferDesc],
        supports_native_shader: bool,
    ) -> Self {
        #[cfg(debug_assertions)]
        {
            let mut slots = std::collections::HashSet::new();
            for r in resources {
                // Validate that slot assignments within a ComputeStep are unique.
                if r.flow == DataFlow::Shared {
                    debug_assert!(r.slot > -1);
                    debug_assert!(r.slot < MAX_COMPUTE_DATA_FLOW_SLOTS);
                    debug_assert!(slots.insert(r.slot));
                }
            }
        }
        Self {
            unique_id: next_id(),
            supports_native_shader,
            name: name.to_owned(),
            resources: resources.to_vec(),
            workgroup_buffers: workgroup_buffers.to_vec(),
            local_dispatch_size,
        }
    }
}

/// A compute program and its data binding layout.
// Port of: src/gpu/graphite/compute/ComputeStep.h#L56-L291 (chrome/m156)
#[doc(alias = "skgpu::graphite::ComputeStep")]
pub trait ComputeStep: Send + Sync + Debug {
    /// The data shared by every step.
    fn base(&self) -> &ComputeStepBase;

    /// `computeSkSL()`: a complete `SkSL` compute program. It must declare all resource bindings in
    /// the order in which they are enumerated by [`resources`](Self::resources). A step that
    /// supports native shader source overrides `native_shader_source` instead.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::computeSkSL`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L68-L70 (chrome/m156)
    #[doc(alias = "computeSkSL")]
    fn compute_sksl(&self) -> String {
        panic!("ComputeSteps must override computeSkSL() unless they support native shader source");
    }

    /// `nativeShaderSource(format)`: the shader source in the requested format, to instantiate a
    /// compute pipeline from a pre-compiled shader module.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::nativeShaderSource`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L72-L74 (chrome/m156)
    #[doc(alias = "nativeShaderSource")]
    fn native_shader_source(&self, _format: NativeShaderFormat) -> NativeShaderSource<'_> {
        panic!("ComputeSteps that support native shader source must override nativeShaderSource()");
    }

    /// `prepareStorageBuffer(resourceIndex, resource, writer)`: fills a mapped storage buffer on
    /// the CPU before the dispatch runs.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::prepareStorageBuffer`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L54-L56 (chrome/m156)
    #[doc(alias = "prepareStorageBuffer")]
    fn prepare_storage_buffer(
        &self,
        _resource_index: usize,
        _resource: &ResourceDesc,
        _writer: BufferWriter<'_>,
    ) {
        panic!(
            "ComputeSteps that initialize a mapped storage buffer must override \
             prepareStorageBuffer()"
        );
    }

    /// `prepareUniformBuffer(resourceIndex, resource, uniformManager)`: adds the uniforms of a
    /// mapped uniform buffer.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::prepareUniformBuffer`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L58-L60 (chrome/m156)
    #[doc(alias = "prepareUniformBuffer")]
    fn prepare_uniform_buffer(
        &self,
        _resource_index: usize,
        _resource: &ResourceDesc,
        _uniform_manager: &mut UniformManager,
    ) {
        panic!("ComputeSteps that initialize a uniform buffer must override prepareUniformBuffer()");
    }

    /// `calculateBufferSize(resourceIndex, resource)`: the required allocation size of a buffer
    /// entry. Must be non-zero.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::calculateBufferSize`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L76-L78 (chrome/m156)
    #[doc(alias = "calculateBufferSize")]
    fn calculate_buffer_size(&self, _resource_index: usize, _resource: &ResourceDesc) -> usize {
        panic!("ComputeSteps that initialize a storage buffer must override calculateBufferSize()");
    }

    /// `calculateTextureParameters(resourceIndex, resource)`: the dimensions and color type of a
    /// storage texture entry.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::calculateTextureParameters`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L80-L83 (chrome/m156)
    #[doc(alias = "calculateTextureParameters")]
    fn calculate_texture_parameters(
        &self,
        _resource_index: usize,
        _resource: &ResourceDesc,
    ) -> (ISize, ColorType) {
        panic!("ComputeSteps that initialize a texture must override calculateTextureParameters()");
    }

    /// `calculateSamplerParameters(resourceIndex, resource)`: the sampling and tile mode options
    /// of a sampler entry.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::calculateSamplerParameters`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L85-L87 (chrome/m156)
    #[doc(alias = "calculateSamplerParameters")]
    fn calculate_sampler_parameters(
        &self,
        _resource_index: usize,
        _resource: &ResourceDesc,
    ) -> SamplerDesc {
        panic!("ComputeSteps that initialize a sampler must override calculateSamplerParameters()");
    }

    /// `calculateGlobalDispatchSize()`: the global dispatch size (the "workgroup count") of this
    /// step.
    ///
    /// # Panics
    /// By default, like `SK_ABORT` in `ComputeStep::calculateGlobalDispatchSize`.
    // Port of: src/gpu/graphite/compute/ComputeStep.cpp#L89-L92 (chrome/m156)
    #[doc(alias = "calculateGlobalDispatchSize")]
    fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
        panic!(
            "ComputeSteps must override calculateGlobalDispatchSize() unless the workgroup count \
             is determined out-of-band"
        );
    }

    /// `resources()`.
    // Port of: src/gpu/graphite/compute/ComputeStep.h#L231 (chrome/m156)
    fn resources(&self) -> &[ResourceDesc] {
        &self.base().resources
    }

    /// `workgroupBuffers()`.
    // Port of: src/gpu/graphite/compute/ComputeStep.h#L232 (chrome/m156)
    #[doc(alias = "workgroupBuffers")]
    fn workgroup_buffers(&self) -> &[WorkgroupBufferDesc] {
        &self.base().workgroup_buffers
    }

    /// `uniqueID()`: an identifier that can be used as part of a unique key for a compute
    /// pipeline state object associated with this step.
    // Port of: src/gpu/graphite/compute/ComputeStep.h#L236 (chrome/m156)
    #[doc(alias = "uniqueID")]
    fn unique_id(&self) -> u32 {
        self.base().unique_id
    }

    /// `name()`: a debug name for the subclass implementation.
    // Port of: src/gpu/graphite/compute/ComputeStep.h#L239 (chrome/m156)
    fn name(&self) -> &str {
        &self.base().name
    }

    /// `localDispatchSize()`: the size of the workgroup for this step's entry point function.
    // Port of: src/gpu/graphite/compute/ComputeStep.h#L244 (chrome/m156)
    #[doc(alias = "localDispatchSize")]
    fn local_dispatch_size(&self) -> WorkgroupSize {
        self.base().local_dispatch_size
    }

    /// `supportsNativeShader()`.
    // Port of: src/gpu/graphite/compute/ComputeStep.h#L246 (chrome/m156)
    #[doc(alias = "supportsNativeShader")]
    fn supports_native_shader(&self) -> bool {
        self.base().supports_native_shader
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Step(ComputeStepBase);

    impl ComputeStep for Step {
        fn base(&self) -> &ComputeStepBase {
            &self.0
        }
    }

    #[test]
    fn steps_get_distinct_ids_and_report_their_layout() {
        let resources = [
            ResourceDesc::new(
                ResourceType::UniformBuffer,
                DataFlow::Private,
                ResourcePolicy::Mapped,
            ),
            ResourceDesc::with_slot(
                ResourceType::StorageBuffer,
                DataFlow::Shared,
                ResourcePolicy::Clear,
                3,
            ),
        ];
        let a = Step(ComputeStepBase::new(
            "A",
            WorkgroupSize::new(8, 4, 1),
            &resources,
            &[],
            false,
        ));
        let b = Step(ComputeStepBase::new(
            "B",
            WorkgroupSize::default(),
            &[],
            &[],
            true,
        ));
        assert_ne!(a.unique_id(), b.unique_id());
        assert_eq!(a.name(), "A");
        assert_eq!(a.resources(), &resources);
        assert_eq!(a.local_dispatch_size().scalar_size(), 32);
        assert!(!a.supports_native_shader());
        assert!(b.supports_native_shader());
        assert_eq!(b.local_dispatch_size(), WorkgroupSize::new(1, 1, 1));
    }
}

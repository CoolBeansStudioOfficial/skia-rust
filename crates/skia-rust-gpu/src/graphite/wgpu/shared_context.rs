// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnSharedContext.h, DawnSharedContext.cpp,
//                   include/gpu/graphite/dawn/DawnBackendContext.h

//! `DawnSharedContext` on wgpu: the state a wgpu `Context`, its recorders and its resource
//! providers share.
//!
//! This is the concrete `SharedContext` (`docs/design/gpu.md` §4.1): the backend-neutral base's
//! members that exist so far are [`WgpuSharedContext::caps`] and the recorder-facing
//! [`RecorderSharedContext`] interface. The global cache, the pipeline manager, the executor, the
//! runtime-effect dictionary and the `ThreadSafeResourceProvider` come with G9b, G6 and G11b;
//! `createGraphicsPipeline` comes with `GraphicsPipeline` (G11b).

use std::sync::{Arc, Weak};

use crate::gpu::gpu_types::{BackendApi, Protected};
use crate::graphite::caps::Caps;
use crate::graphite::context_options::ContextOptions;
use crate::graphite::recorder::RecorderSharedContext;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::resource_types::Layout;
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::wgpu::async_wait::create_checked;
use crate::graphite::wgpu::caps::{
    COMBINED_UNIFORM_INDEX, CapsProfile, INTRINSIC_UNIFORM_BUFFER_INDEX, STORAGE_BUFFER_INDEX,
    WgpuCaps,
};
use crate::graphite::wgpu::resource_provider::WgpuResourceProvider;

/// `DawnBackendContext`: the wgpu objects the client creates and passes into
/// [`make_shared_context`](WgpuSharedContext::make) / [`make_context`](super::make_context).
///
/// Dawn's `fInstance` and `fTick` (`ProcessEvents`) have no wgpu counterpart: the device is
/// polled (`Device::poll`) wherever Dawn processes events, and `has_tick` says whether the
/// context may do that.
// Port of: include/gpu/graphite/dawn/DawnBackendContext.h#L55-L68 (chrome/m156)
#[doc(alias = "DawnBackendContext")]
#[derive(Clone, Debug)]
pub struct WgpuBackendContext {
    /// `fDevice`.
    pub device: wgpu::Device,
    /// `fQueue`.
    pub queue: wgpu::Queue,
    /// `fTick != nullptr`: whether the context can wait for the GPU. WebGPU in the browser needs
    /// the main thread loop to run to detect GPU progress, so there the context is
    /// "non-yielding": `SyncToCpu::kYes` is disallowed and the client must guarantee that GPU
    /// work has completed before destroying the `Context`.
    pub has_tick: bool,
}

impl WgpuBackendContext {
    /// A backend context for `device` and `queue`, ticking everywhere but in the browser.
    #[must_use]
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            has_tick: !cfg!(target_arch = "wasm32"),
        }
    }
}

/// The shared state of a wgpu Graphite context.
// Port of: src/gpu/graphite/dawn/DawnSharedContext.h#L36-L96 (chrome/m156)
#[doc(alias = "DawnSharedContext")]
#[derive(Debug)]
pub struct WgpuSharedContext {
    this: Weak<WgpuSharedContext>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    has_tick: bool,
    caps: Arc<WgpuCaps>,
    shader_dictionary: ShaderCodeDictionary,
    // A noop fragment shader, it is used to workaround a Dawn validation error (Dawn doesn't
    // allow a pipeline with a color attachment but without a fragment shader).
    noop_fragment: wgpu::ShaderModule,

    uniform_buffers_bind_group_layouts: [wgpu::BindGroupLayout; 4],
    single_texture_sampler_bind_group_layout: wgpu::BindGroupLayout,
}

// Port of: src/gpu/graphite/dawn/DawnSharedContext.cpp#L21-L40 (chrome/m156)
fn create_noop_fragment(device: &wgpu::Device, scoped: bool) -> Option<wgpu::ShaderModule> {
    create_checked(device, scoped, || {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("no-op"),
            source: wgpu::ShaderSource::Wgsl(
                "@fragment\n\
                 fn main() {}\n"
                    .into(),
            ),
        })
    })
}

// Port of: src/gpu/graphite/SharedContext.cpp#L30-L33 (chrome/m156)
fn get_binding_layout(caps: &WgpuCaps) -> Layout {
    let reqs = caps.resource_binding_requirements();
    if caps.storage_buffer_support() {
        reqs.storage_buffer_layout
    } else {
        reqs.uniform_buffer_layout
    }
}

impl WgpuSharedContext {
    /// `DawnSharedContext::Make()`: creates the shared context of `backend_context`, computing
    /// the caps from its device, or `None` if the shared objects cannot be created.
    // Port of: src/gpu/graphite/dawn/DawnSharedContext.cpp#L42-L63 (chrome/m156)
    #[must_use]
    pub fn make(
        backend_context: &WgpuBackendContext,
        options: &ContextOptions,
    ) -> Option<Arc<Self>> {
        let profile = CapsProfile::from_device(&backend_context.device, backend_context.has_tick);
        Self::make_with_profile(backend_context, &profile, options)
    }

    /// [`make`](Self::make) with the caps computed from `profile` rather than from the device:
    /// how tests and the oracle replay run a device with the capabilities Dawn had
    /// (`docs/design/gpu.md` §6.2). The device must be able to do what `profile` says; the noop
    /// device is.
    #[must_use]
    pub fn make_with_profile(
        backend_context: &WgpuBackendContext,
        profile: &CapsProfile,
        options: &ContextOptions,
    ) -> Option<Arc<Self>> {
        let caps = Arc::new(WgpuCaps::new(profile, options));
        let noop_fragment =
            create_noop_fragment(&backend_context.device, caps.allow_scoped_error_checks())?;

        // (The context options carry no user-defined known runtime effects yet.)
        let shader_dictionary = ShaderCodeDictionary::new(get_binding_layout(&caps), &[]);

        let uniform_buffers_bind_group_layouts =
            create_uniform_buffers_bind_group_layouts(&backend_context.device, &caps);
        let single_texture_sampler_bind_group_layout =
            create_single_texture_sampler_bind_group_layout(&backend_context.device, &caps);

        Some(Arc::new_cyclic(|this| Self {
            this: this.clone(),
            device: backend_context.device.clone(),
            queue: backend_context.queue.clone(),
            has_tick: backend_context.has_tick,
            caps,
            shader_dictionary,
            noop_fragment,
            uniform_buffers_bind_group_layouts,
            single_texture_sampler_bind_group_layout,
        }))
    }

    /// `dawnCaps()` / `caps()`.
    #[doc(alias = "dawnCaps")]
    #[must_use]
    pub fn caps(&self) -> &Arc<WgpuCaps> {
        &self.caps
    }

    /// `shaderCodeDictionary()`.
    #[doc(alias = "shaderCodeDictionary")]
    #[must_use]
    pub fn shader_code_dictionary(&self) -> &ShaderCodeDictionary {
        &self.shader_dictionary
    }

    /// `device()`.
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// `queue()`.
    #[must_use]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// `noopFragment()`.
    #[doc(alias = "noopFragment")]
    #[must_use]
    pub fn noop_fragment(&self) -> &wgpu::ShaderModule {
        &self.noop_fragment
    }

    /// `hasTick()`.
    #[doc(alias = "hasTick")]
    #[must_use]
    pub fn has_tick(&self) -> bool {
        self.has_tick
    }

    /// `tick()`: lets wgpu process finished work and run its callbacks (map callbacks, …),
    /// without waiting.
    // Port of: src/gpu/graphite/dawn/DawnSharedContext.h#L56-L60 (chrome/m156)
    ///
    /// # Panics
    /// If the context has no tick (`!has_tick()`), as `SkASSERT(this->hasTick())` does.
    pub fn tick(&self) {
        assert!(self.has_tick(), "the context cannot tick");
        // `Poll` never fails in a way a caller could act on: a lost device fails the next call.
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    /// Blocks until all submitted GPU work has finished.
    ///
    /// # Panics
    /// If the context has no tick.
    pub fn wait_for_gpu(&self) {
        assert!(self.has_tick(), "the context cannot tick");
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// `deviceTick(context)`: the device half; the caller then calls
    /// `Context::checkAsyncWorkCompletion()` (G9b).
    // Port of: src/gpu/graphite/dawn/DawnSharedContext.cpp#L100-L106 (chrome/m156)
    #[doc(alias = "deviceTick")]
    pub fn device_tick(&self) {
        if self.has_tick {
            self.tick();
        }
    }

    /// `getUniformBuffersBindGroupLayout()`: the layout of the uniform buffers bind group for
    /// which the storage buffer binding is visible in `storage_visibility`.
    ///
    /// Dawn indexes its array of four layouts with the stage bits and, for out-of-range bits,
    /// falls back to the `None` layout (its range check admits `Compute`, which would index
    /// past the array); here every visibility with a compute bit gets the `None` layout.
    // Port of: src/gpu/graphite/dawn/DawnSharedContext.h#L62-L77 (chrome/m156)
    #[doc(alias = "getUniformBuffersBindGroupLayout")]
    #[must_use]
    pub fn get_uniform_buffers_bind_group_layout(
        &self,
        storage_visibility: wgpu::ShaderStages,
    ) -> &wgpu::BindGroupLayout {
        let mut index = storage_visibility.bits() as usize;
        if index >= self.uniform_buffers_bind_group_layouts.len() {
            index = 0;
        }
        &self.uniform_buffers_bind_group_layouts[index]
    }

    /// `getSingleTextureSamplerBindGroupLayout()`.
    #[doc(alias = "getSingleTextureSamplerBindGroupLayout")]
    #[must_use]
    pub fn get_single_texture_sampler_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.single_texture_sampler_bind_group_layout
    }
}

// Port of: src/gpu/graphite/dawn/DawnSharedContext.cpp#L108-L170 (chrome/m156)
fn create_uniform_buffers_bind_group_layouts(
    device: &wgpu::Device,
    caps: &WgpuCaps,
) -> [wgpu::BindGroupLayout; 4] {
    use wgpu::ShaderStages as S;
    const VISIBILITIES: [wgpu::ShaderStages; 4] = [
        S::empty(),                   // index 0 (ShaderStage::None)
        S::VERTEX,                    // index 1 (ShaderStage::Vertex)
        S::FRAGMENT,                  // index 2 (ShaderStage::Fragment)
        S::VERTEX.union(S::FRAGMENT), // index 3 (Vertex | Fragment)
    ];
    let buffer_type = |ty| wgpu::BindingType::Buffer {
        ty,
        has_dynamic_offset: true,
        min_binding_size: None,
    };
    // StorageBuffer will only be used if supported and preferred, else set binding type as a
    // uniform when not in use to satisfy any binding type restrictions for non-supported ssbo
    // devices.
    let storage_or_uniform = if caps.storage_buffer_support() {
        wgpu::BufferBindingType::Storage { read_only: true }
    } else {
        wgpu::BufferBindingType::Uniform
    };

    VISIBILITIES.map(|storage_visibility| {
        let entries = [
            wgpu::BindGroupLayoutEntry {
                binding: INTRINSIC_UNIFORM_BUFFER_INDEX,
                visibility: S::VERTEX | S::FRAGMENT,
                ty: buffer_type(wgpu::BufferBindingType::Uniform),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: COMBINED_UNIFORM_INDEX,
                visibility: S::VERTEX | S::FRAGMENT,
                ty: buffer_type(storage_or_uniform),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: STORAGE_BUFFER_INDEX,
                visibility: storage_visibility,
                ty: buffer_type(storage_or_uniform),
                count: None,
            },
        ];
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: caps
                .set_backend_labels()
                .then_some("Uniform buffers bind group layout"),
            entries: &entries,
        })
    })
}

// Port of: src/gpu/graphite/dawn/DawnSharedContext.cpp#L172-L192 (chrome/m156)
fn create_single_texture_sampler_bind_group_layout(
    device: &wgpu::Device,
    caps: &WgpuCaps,
) -> wgpu::BindGroupLayout {
    let entries = [
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
    ];
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: caps
            .set_backend_labels()
            .then_some("Single texture + sampler bind group layout"),
        entries: &entries,
    })
}

impl RecorderSharedContext for WgpuSharedContext {
    fn caps(&self) -> Arc<dyn Caps> {
        self.caps.clone()
    }

    fn backend(&self) -> BackendApi {
        BackendApi::Dawn
    }

    fn is_protected(&self) -> Protected {
        // Dawn doesn't support protected memory.
        Protected::No
    }

    fn shader_code_dictionary(&self) -> &ShaderCodeDictionary {
        &self.shader_dictionary
    }

    // Port of: src/gpu/graphite/dawn/DawnSharedContext.cpp#L89-L98 (chrome/m156)
    fn make_resource_provider(&self, recorder_id: u32, resource_budget: usize) -> ResourceProvider {
        let shared_context = self
            .this
            .upgrade()
            .expect("the shared context is alive while it makes resource providers");
        ResourceProvider::new(
            Box::new(WgpuResourceProvider::new(shared_context)),
            recorder_id,
            resource_budget,
        )
    }
}

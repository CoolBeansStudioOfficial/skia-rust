//! The wgpu back end of Graphite: a port of `src/gpu/graphite/dawn/` onto wgpu
//! (`docs/design/gpu.md` §4.1).
//!
//! wgpu and Dawn implement the same WebGPU API, so each `Dawn*` class becomes a `Wgpu*` type in
//! the module named after its file (`dawn_caps` → [`caps`], …). Skia's virtual base classes have
//! exactly one subclass here, so the base and backend halves are one concrete type where that is
//! practical; where the neutral half already exists as a trait seam (`Texture`/`Buffer`/`Sampler`
//! owning a boxed `*Backend`, `ResourceProvider` owning a `ResourceProviderBackend`), the wgpu
//! type is the backend half and the seam stays, as it is also what the neutral code's tests
//! mock.
//!
//! G11a is the context, caps, resources and format tables; G11b is the graphics and compute
//! pipelines ([`graphics_pipeline`], [`compute_pipeline`]), the shader-module helper
//! ([`graphite_utils::compile_wgsl_shader_module`]) and the error checker ([`error_checker`]).
//! G11c is the `CommandBuffer` ([`command_buffer`]) and `QueueManager` ([`queue_manager`]), and [`adapter_backend_context`] for the tests that read pixels.
//!
//! # Platforms
//!
//! On native targets wgpu is built with all its backends and the `noop` backend
//! (`Backends::NOOP`): [`noop_backend_context`] creates a device that supports resource creation
//! but renders nothing, which is what the unit tests run on. On `wasm32` it is wgpu's WebGPU
//! backend with `fragile-send-sync-non-atomic-wasm`: build-only.

/// Emits a trace record on `$shared` (a `WgpuSharedContext`) when the `trace` feature is on:
/// `$record` is only evaluated if a sink is set. Without the feature it expands to nothing, and
/// `$record` is not compiled.
macro_rules! trace {
    ($shared:expr, $record:expr) => {
        #[cfg(feature = "trace")]
        $shared.trace(|| $record);
        #[cfg(not(feature = "trace"))]
        let _ = &$shared;
    };
}

pub mod async_wait;
pub mod backend_texture;
pub mod buffer;
pub mod caps;
pub mod command_buffer;
pub mod compute_pipeline;
pub mod context;
pub mod error_checker;
pub mod graphics_pipeline;
pub mod graphite_utils;
pub mod pipeline_shaders;
pub mod queue_manager;
pub mod resource_provider;
pub mod sampler;
pub mod shared_context;
pub mod texture;
pub mod texture_info;
#[cfg(feature = "trace")]
pub mod trace;

pub use backend_texture::backend_textures;
pub use caps::{CapsProfile, DeviceFeatures, WgpuCaps};
pub use context::{WgpuContext, make_context};
pub use shared_context::{WgpuBackendContext, WgpuSharedContext};
pub use texture_info::{WgpuTextureInfo, texture_infos};

#[cfg(not(target_arch = "wasm32"))]
mod adapter;
#[cfg(not(target_arch = "wasm32"))]
mod noop;
#[cfg(not(target_arch = "wasm32"))]
pub use adapter::adapter_backend_context;
#[cfg(not(target_arch = "wasm32"))]
pub use noop::{noop_backend_context, noop_backend_context_with_features};

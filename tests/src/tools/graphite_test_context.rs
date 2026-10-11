// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/graphite/ContextFactory.cpp, tools/graphite/GraphiteTestContext.h (chrome/m156),
//          adapted to wgpu's noop backend

//! The Graphite contexts that `DEF_GRAPHITE_TEST_FOR_ALL_CONTEXTS` tests run on.
//!
//! Skia runs such a test once per context type the build supports (Dawn on D3D12, Dawn on
//! Vulkan, Metal, …), each on a real device. The port runs it on wgpu's noop device, which
//! creates resources but renders nothing (`docs/design/gpu.md` §7), once per capability profile:
//! the device's own capabilities, the capabilities Dawn reported on D3D12, and those Dawn reported
//! on Vulkan restricted to what wgpu can express (`CapsProfile::wgpu_restricted`). Tests that
//! read pixels cannot be ported on top of this.
#![cfg(not(target_arch = "wasm32"))]

use skia_rust_gpu::graphite::buffer_manager::StaticBufferManager;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::graphite_types::{SubmitInfo, SyncToCpu};
use skia_rust_gpu::graphite::renderer_provider::RendererProvider;
use skia_rust_gpu::graphite::wgpu::{
    AdapterTestContext, CapsProfile, DeviceFeatures, WgpuContext, WgpuSharedContext,
    any_adapter_backend_context, make_context, noop_backend_context,
    noop_backend_context_for_profile, serialized_adapter_context,
};

/// `skiatest::IsDawnContextType` for a context name from [`all_contexts`]: the Dawn profiles are
/// named `dawn-…`; the noop device is not Dawn.
// Port of: tests/Test.h#L217 (chrome/m156), `IsDawnContextType`
#[must_use]
pub fn is_dawn_context(context_name: &str) -> bool {
    context_name.starts_with("dawn-")
}

/// The contexts `DEF_GRAPHITE_TEST_FOR_ALL_CONTEXTS` tests run on, with a name for each.
#[must_use]
pub fn all_contexts() -> Vec<(String, WgpuContext)> {
    all_contexts_with_options(&ContextOptions::default())
}

/// [`all_contexts`] created with `options` (`DEF_CONDITIONAL_GRAPHITE_TEST_FOR_CONTEXTS`'s
/// options-setting function has been applied to them).
///
/// # Panics
/// If the noop backend cannot create a device.
#[must_use]
pub fn all_contexts_with_options(options: &ContextOptions) -> Vec<(String, WgpuContext)> {
    let mut contexts = vec![(
        "wgpu-noop".to_owned(),
        make_context(&noop_backend_context(), options).expect("a context on the noop device"),
    )];
    // What Dawn reported on D3D12 (with its Dawn-only features), and what Dawn on Vulkan reports
    // that wgpu can express. The Vulkan profile has `ShaderF16`, which wgpu 30 cannot express for
    // these pipelines: its fragment output is a `half4`, and wgpu's pipeline validation rejects it
    // for an `Rgba8Unorm`/`Bgra8Unorm` target ("Output format Float32x4 is incompatible with the
    // shader Float16x4"), where Dawn accepts it. So the feature is removed, as `from_device`
    // leaves it out for real devices and `tests/draw_pass.rs` and `tests/wgpu_pipelines.rs` do.
    let mut vulkan = CapsProfile::dawn_vulkan().wgpu_restricted();
    vulkan.features -= DeviceFeatures::SHADER_F16;
    for profile in [CapsProfile::dawn_d3d12(), vulkan] {
        let shared_context = WgpuSharedContext::make_with_profile(
            &noop_backend_context_for_profile(&profile),
            &profile,
            options,
        )
        .expect("a shared context on the noop device");
        contexts.push((profile.name, WgpuContext::new(shared_context, options)));
    }
    contexts
}

/// The context the tests that read pixels run on: one on a real adapter (software adapters
/// first; `adapter_backend_context`) with its name, or `None` after saying so when the machine has
/// none. The device is the test binary's one shared device and the context holds the adapter test
/// lock until it is dropped, so adapter tests run one at a time (`docs/design/gpu.md` section 8).
/// With `SKIA_RUST_REQUIRE_ADAPTER` set, a missing adapter is an error (the GPU CI jobs).
///
/// # Panics
/// If `SKIA_RUST_REQUIRE_ADAPTER` is set and there is no adapter, or the adapter cannot make a
/// context.
#[must_use]
pub fn real_context() -> Option<(String, AdapterTestContext<WgpuContext>)> {
    real_context_with_options(&ContextOptions::default())
}

/// [`real_context`] created with `options`.
///
/// # Panics
/// As [`real_context`].
#[must_use]
pub fn real_context_with_options(
    options: &ContextOptions,
) -> Option<(String, AdapterTestContext<WgpuContext>)> {
    let Some((context, info)) = serialized_adapter_context(|backend_context| {
        make_context(backend_context, options).expect("a context on the adapter")
    }) else {
        assert!(
            std::env::var_os("SKIA_RUST_REQUIRE_ADAPTER").is_none(),
            "SKIA_RUST_REQUIRE_ADAPTER is set but there is no adapter"
        );
        eprintln!("no adapter that renders: skipping");
        return None;
    };
    Some((format!("{} ({:?})", info.name, info.backend), context))
}

/// `context->priv().rendererProvider()`.
///
/// The port's `Context` does not own a `RendererProvider` yet (`Context::finishInitialization`,
/// G10), so this makes one over the context's caps. The static buffers its steps write are not
/// finalized, which a test that only reads the steps' shader code does not need.
#[must_use]
pub fn renderer_provider(context: &WgpuContext) -> RendererProvider {
    let caps = ContextPriv::caps(context);
    let mut static_buffer_manager =
        StaticBufferManager::new(ContextPriv::resource_provider(context).clone(), caps);
    RendererProvider::new(
        caps.resource_binding_requirements().uniform_buffer_layout,
        caps.shader_caps().infinity_support,
        &mut static_buffer_manager,
    )
}

/// `GraphiteTestContext::syncedSubmit(context)`: submits the context's work and, where the
/// context cannot wait for the GPU, ticks it until the submitted work has finished.
// Port of: tools/graphite/GraphiteTestContext.cpp#L51-L62 (chrome/m156)
pub fn synced_submit(context: &mut WgpuContext) {
    let sync = if context.shared_context().caps().allow_cpu_sync() {
        SyncToCpu::Yes
    } else {
        SyncToCpu::No
    };
    let _ = context.submit(SubmitInfo::new(sync));
    if sync == SyncToCpu::No {
        while context.has_unfinished_gpu_work() {
            context.shared_context().tick();
            context.check_async_work_completion();
        }
    }
}

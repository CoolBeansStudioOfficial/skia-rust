// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/graphite/dawn/GraphiteDawnTestContext.cpp (CreateDevice),
//                   tools/graphite/dawn/GraphiteDawnToggles.cpp (AddPreferredFeatures)

//! A wgpu device on a real adapter, for the tests that read pixels.
//!
//! `GraphiteDawnTestContext` picks an adapter, requests every feature on Skia's preferred list
//! and the adapter's full limits. [`adapter_backend_context`] does the same with the features
//! wgpu has: software adapters (lavapipe, WARP) come first, as the pixel tests are meant to run
//! on them (`docs/design/gpu.md` §2).

use crate::graphite::wgpu::async_wait::block_on;
use crate::graphite::wgpu::shared_context::WgpuBackendContext;

/// The features Skia's test context asks for (`AddPreferredFeatures`) that wgpu has.
// Port of: tools/graphite/dawn/GraphiteDawnToggles.cpp#L46-L75 (chrome/m156)
const PREFERRED_FEATURES: wgpu::Features = wgpu::Features::DUAL_SOURCE_BLENDING
    .union(wgpu::Features::IMMEDIATES)
    .union(wgpu::Features::TEXTURE_COMPRESSION_BC)
    .union(wgpu::Features::TEXTURE_COMPRESSION_ETC2)
    .union(wgpu::Features::TIMESTAMP_QUERY)
    .union(wgpu::Features::FLOAT32_FILTERABLE)
    .union(wgpu::Features::FLOAT32_BLENDABLE)
    .union(wgpu::Features::RG11B10UFLOAT_RENDERABLE)
    .union(wgpu::Features::BGRA8UNORM_STORAGE)
    .union(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM);

/// [`adapter_backend_context`] over every backend the build has (for callers that do not depend
/// on wgpu themselves).
#[must_use]
pub fn any_adapter_backend_context() -> Option<(WgpuBackendContext, wgpu::AdapterInfo)> {
    adapter_backend_context(wgpu::Backends::all())
}

/// A device on a real (not noop) adapter of `backends`, with the adapter's full limits and the
/// preferred features it has, or `None` if there is no such adapter or it cannot make a device.
/// Software adapters are preferred. The adapter's info is returned with the context.
#[must_use]
pub fn adapter_backend_context(
    backends: wgpu::Backends,
) -> Option<(WgpuBackendContext, wgpu::AdapterInfo)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let mut adapters = block_on(instance.enumerate_adapters(backends));
    adapters.retain(|adapter| adapter.get_info().backend != wgpu::Backend::Noop);
    // Software adapters first (`false` sorts before `true`), keeping the order otherwise.
    adapters.sort_by_key(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu);
    let adapter = adapters.into_iter().next()?;
    let info = adapter.get_info();
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("adapter"),
        required_features: adapter.features() & PREFERRED_FEATURES,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .ok()?;
    Some((WgpuBackendContext::new(device, queue), info))
}

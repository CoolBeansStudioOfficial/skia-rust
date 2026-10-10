// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! A wgpu device on the `noop` backend (`Backends::NOOP`).
//!
//! The noop backend performs no rendering or computation, but lets code create buffers,
//! textures, samplers, shader modules, pipelines and bind groups, and map, write and copy
//! buffers (`wgpu-types` `Backend::Noop`). Texture contents are never stored. The unit tests of
//! the resource layer run on it (`docs/design/gpu.md` §7); tests that read pixels need a real
//! adapter.

use crate::graphite::wgpu::async_wait::block_on;
use crate::graphite::wgpu::shared_context::WgpuBackendContext;

/// A noop device that has `required_features` and `required_limits`, or `None` if wgpu cannot
/// create it.
///
/// The noop adapter reports every feature and maximally permissive limits, so any request
/// succeeds; a context made on the device computes its caps from the features requested here.
#[must_use]
pub fn noop_backend_context_with_features(
    required_features: wgpu::Features,
    required_limits: wgpu::Limits,
) -> Option<WgpuBackendContext> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::NOOP,
        backend_options: wgpu::BackendOptions {
            noop: wgpu::NoopBackendOptions::enabled(),
            ..Default::default()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("noop"),
        required_features,
        required_limits,
        ..Default::default()
    }))
    .ok()?;
    Some(WgpuBackendContext::new(device, queue))
}

/// A noop device with the default limits and no optional features.
///
/// # Panics
/// If wgpu cannot create the noop device, which means wgpu was built without the `noop` feature.
#[must_use]
pub fn noop_backend_context() -> WgpuBackendContext {
    noop_backend_context_with_features(wgpu::Features::empty(), wgpu::Limits::default())
        .expect("the noop backend creates a device")
}

/// A noop device that has what the pipelines of `profile` need: immediates (64 bytes), and the
/// half-float and dual-source-blending features if the profile has them. The pipelines the
/// tests make validate on it, which they do not on [`noop_backend_context`] (`var<immediate>`
/// needs the feature).
///
/// # Panics
/// If wgpu cannot create the noop device, which means wgpu was built without the `noop` feature.
#[must_use]
pub fn noop_backend_context_for_profile(
    profile: &crate::graphite::wgpu::CapsProfile,
) -> WgpuBackendContext {
    use crate::graphite::wgpu::DeviceFeatures;
    let mut features = wgpu::Features::IMMEDIATES;
    if profile.features.contains(DeviceFeatures::SHADER_F16) {
        features |= wgpu::Features::SHADER_F16;
    }
    if profile
        .features
        .contains(DeviceFeatures::DUAL_SOURCE_BLENDING)
    {
        features |= wgpu::Features::DUAL_SOURCE_BLENDING;
    }
    let limits = wgpu::Limits {
        max_immediate_size: 64,
        ..wgpu::Limits::default()
    };
    noop_backend_context_with_features(features, limits).expect("the noop backend creates a device")
}

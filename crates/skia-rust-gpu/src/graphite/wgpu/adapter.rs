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

use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

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

/// The one device a test binary shares: made on first use by [`adapter_backend_context`] with
/// every backend, and never dropped (a `static` is not), so an adapter test binary creates one
/// instance, adapter and device and tears none of them down.
///
/// The Windows CI jobs died intermittently with `STATUS_ACCESS_VIOLATION` inside the WARP D3D12
/// software adapter while the tests of one binary, which libtest runs on parallel threads, each
/// created and dropped their own instance and device (`docs/design/gpu.md` section 8).
static SHARED_BACKEND_CONTEXT: OnceLock<Option<(WgpuBackendContext, wgpu::AdapterInfo)>> =
    OnceLock::new();

/// Held by every [`AdapterTestContext`], so the adapter tests of one binary run one at a time.
static ADAPTER_TEST_LOCK: Mutex<()> = Mutex::new(());

/// A clone of the process-wide backend context [`adapter_backend_context`] makes (with
/// `Backends::all()`) the first time, or `None` if there is no adapter. For the tests that read
/// pixels; use [`serialized_adapter_context`] unless the test must hold the device itself.
#[must_use]
pub fn shared_adapter_backend_context() -> Option<(WgpuBackendContext, wgpu::AdapterInfo)> {
    SHARED_BACKEND_CONTEXT
        .get_or_init(any_adapter_backend_context)
        .clone()
}

/// A context made from the shared device, with the process-wide adapter test lock held until it
/// is dropped. It derefs to the `Context` (`WgpuContext`).
#[derive(Debug)]
pub struct AdapterTestContext<C> {
    // Declared first, so the context is dropped before the lock is released.
    context: C,
    _serial: MutexGuard<'static, ()>,
}

impl<C> Deref for AdapterTestContext<C> {
    type Target = C;
    fn deref(&self) -> &C {
        &self.context
    }
}

impl<C> DerefMut for AdapterTestContext<C> {
    fn deref_mut(&mut self) -> &mut C {
        &mut self.context
    }
}

/// Takes the process-wide adapter test lock (a test that panicked while holding it does not
/// poison it), then makes the context with `make` from the shared backend context. `None` if
/// there is no adapter.
///
/// The lock and the shared device are the mitigation for the Windows WARP crash described on
/// the shared device's `static`: no two adapter tests of a binary run at once, and none creates
/// or destroys a device.
#[must_use]
pub fn serialized_adapter_context<C>(
    make: impl FnOnce(&WgpuBackendContext) -> C,
) -> Option<(AdapterTestContext<C>, wgpu::AdapterInfo)> {
    let serial = ADAPTER_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let (backend_context, info) = shared_adapter_backend_context()?;
    let context = make(&backend_context);
    Some((
        AdapterTestContext {
            context,
            _serial: serial,
        },
        info,
    ))
}

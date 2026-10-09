// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp (ContextFactory::MakeDawn),
//                   include/gpu/graphite/Context.h (the part that exists before G9b)

//! `ContextFactory::MakeDawn` on wgpu: [`make_context`].
//!
//! The full `Context` (`insertRecording`, `submit`, `readPixels`, the queue manager, the global
//! cache and the pipeline manager) is G9b, and `DawnQueueManager` is G11c. [`WgpuContext`] is the
//! part of it that exists now: it owns the shared context and the context's resource provider
//! (`Context::fResourceProvider`) and makes recorders. G9b replaces it by the real `Context`;
//! [`make_context`] keeps its name and arguments, and its result keeps the methods below.

use std::sync::{Arc, Mutex};

use crate::gpu::gpu_types::{BackendApi, Protected};
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::caps::Caps;
use crate::graphite::context_options::ContextOptions;
use crate::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use crate::graphite::recorder::{Recorder, RecorderOptions, RecorderSharedContext};
use crate::graphite::wgpu::caps::WgpuCaps;
use crate::graphite::wgpu::shared_context::{WgpuBackendContext, WgpuSharedContext};

/// The wgpu `Context` as far as it exists before the submission side (G9b, G11c) is ported.
// Port of: include/gpu/graphite/Context.h#L45-L74 (chrome/m156)
#[doc(alias = "Context")]
#[derive(Debug)]
pub struct WgpuContext {
    shared_context: Arc<WgpuSharedContext>,
    resource_provider: SharedResourceProvider,
    options: ContextOptions,
}

/// `SK_InvalidGenID`: the recorder id of the context's own resource provider.
const INVALID_GEN_ID: u32 = 0;

/// Creates a Graphite context on a wgpu device (`ContextFactory::MakeDawn`), or `None` if the
/// shared objects cannot be created.
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L27-L46 (chrome/m156)
#[doc(alias = "MakeDawn")]
#[must_use]
pub fn make_context(
    backend_context: &WgpuBackendContext,
    options: &ContextOptions,
) -> Option<WgpuContext> {
    let shared_context = WgpuSharedContext::make(backend_context, options)?;
    Some(WgpuContext::new(shared_context, options))
}

impl WgpuContext {
    /// `Context(sharedContext, queueManager, options)`.
    // Port of: src/gpu/graphite/Context.cpp#L69-L80 (chrome/m156)
    #[must_use]
    pub fn new(shared_context: Arc<WgpuSharedContext>, options: &ContextOptions) -> Self {
        let resource_provider = Arc::new(Mutex::new(
            shared_context.make_resource_provider(INVALID_GEN_ID, options.gpu_budget_in_bytes),
        ));
        Self {
            shared_context,
            resource_provider,
            options: options.clone(),
        }
    }

    /// `backend()`.
    #[must_use]
    pub fn backend(&self) -> BackendApi {
        BackendApi::Dawn
    }

    /// `priv().caps()` as the concrete wgpu caps.
    #[doc(alias = "caps")]
    #[must_use]
    pub fn wgpu_caps(&self) -> &WgpuCaps {
        self.shared_context.caps()
    }

    /// The options the context was created with.
    #[must_use]
    pub fn options(&self) -> &ContextOptions {
        &self.options
    }

    /// The shared context.
    #[must_use]
    pub fn shared_context(&self) -> &Arc<WgpuSharedContext> {
        &self.shared_context
    }

    /// `supportsProtectedContent()`.
    // Port of: src/gpu/graphite/Context.cpp#L1001-L1003 (chrome/m156)
    #[doc(alias = "supportsProtectedContent")]
    #[must_use]
    pub fn supports_protected_content(&self) -> bool {
        self.shared_context.is_protected() == Protected::Yes
    }

    /// `deleteBackendTexture()`: deleting is safe from the context or any recorder.
    // Port of: src/gpu/graphite/Context.cpp#L928-L935 (chrome/m156)
    #[doc(alias = "deleteBackendTexture")]
    pub fn delete_backend_texture(&self, texture: &BackendTexture) {
        if !texture.is_valid() || texture.backend() != self.backend() {
            return;
        }
        self.resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .delete_backend_texture(texture);
    }

    /// `makeRecorder()`.
    // Port of: src/gpu/graphite/Context.cpp#L137-L151 (chrome/m156)
    #[doc(alias = "makeRecorder")]
    #[must_use]
    pub fn make_recorder(&self, options: Option<&RecorderOptions>) -> Recorder {
        let default_options = RecorderOptions::default();
        Recorder::new(
            self.shared_context.clone(),
            options.unwrap_or(&default_options),
            None,
        )
    }
}

impl ContextPriv for WgpuContext {
    fn caps(&self) -> &dyn Caps {
        &**self.shared_context.caps()
    }

    fn resource_provider(&self) -> &SharedResourceProvider {
        &self.resource_provider
    }
}

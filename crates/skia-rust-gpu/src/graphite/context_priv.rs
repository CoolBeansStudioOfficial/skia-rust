// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ContextPriv.h (the accessors tasks use)

//! The seam between tasks and the `Context`.
//!
//! Tasks receive a `Context*` in `addCommands()` and only reach through `context->priv()` for the
//! caps and the context's resource provider (and `ConditionalUploadContext::needsUpload`, which
//! hands the context to client code). `Context` itself is ported with G9b, which implements this
//! trait for it.

use std::sync::{Arc, Mutex};

use crate::graphite::caps::Caps;
use crate::graphite::resource_provider::ResourceProvider;

/// A resource provider shared by the context and the recorders it makes (Skia's raw
/// `ResourceProvider*`; `Context` and `Recorder` are single-owner, so the lock is never
/// contended).
pub type SharedResourceProvider = Arc<Mutex<ResourceProvider>>;

/// What tasks read from the `Context` (`ContextPriv`).
// Port of: src/gpu/graphite/ContextPriv.h (chrome/m156)
#[doc(alias = "skgpu::graphite::ContextPriv")]
pub trait ContextPriv {
    /// `caps()`.
    fn caps(&self) -> &dyn Caps;

    /// `resourceProvider()`.
    #[doc(alias = "resourceProvider")]
    fn resource_provider(&self) -> &SharedResourceProvider;
}

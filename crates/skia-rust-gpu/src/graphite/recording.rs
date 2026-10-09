// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/Recording.h, src/gpu/graphite/Recording.cpp,
//                   src/gpu/graphite/RecordingPriv.h

//! `Recording`: the GPU work a `Recorder` snapped, ready to be inserted into a `Context`.
//!
//! A `Recording` is `Send` (it holds task nodes, `Arc` proxies and resource handles, nothing
//! `Rc`), so it can be built on one thread and inserted on another. It is not `Sync`-shared: the
//! context mutates it while preparing and replaying it.
//!
//! `fCapturedPictures` (the `SkCaptureManager` hook) is not ported.

use std::sync::{Arc, Mutex};

use skia_rust_core::point::IVector;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{Budgeted, Mipmapped};
use crate::gpu::ref_cnted_callback::RefCntedCallback;
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::caps::Caps;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::graphite_types::Volatile;
use crate::graphite::resource::{AnyResourceRef, Resource, ResourceRef};
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::task_list::TaskList;
use crate::graphite::task::{ReplayTargetData, Status};
use crate::graphite::texture::Texture;
use crate::graphite::texture_info::TextureInfo;
use crate::graphite::texture_proxy::TextureProxy;

/// The lazily instantiated target of a deferred canvas (`Recording::LazyProxyData`): a lazy
/// proxy that is fulfilled with the texture a `Recording` is replayed onto.
// Port of: include/gpu/graphite/Recording.h#L47-L60 (chrome/m156)
#[doc(alias = "Recording::LazyProxyData")]
#[derive(Debug)]
pub struct LazyProxyData {
    // The lazy callback moves the texture out (`std::move(fTarget)`).
    target: Arc<Mutex<Option<ResourceRef<Texture>>>>,
    target_proxy: Option<Arc<TextureProxy>>,
}

impl LazyProxyData {
    /// `LazyProxyData(caps, dimensions, textureInfo)`.
    // Port of: src/gpu/graphite/Recording.cpp#L46-L66 (chrome/m156)
    #[must_use]
    pub fn new(caps: &dyn Caps, dimensions: ISize, texture_info: &TextureInfo) -> Self {
        let target: Arc<Mutex<Option<ResourceRef<Texture>>>> = Arc::new(Mutex::new(None));
        let callback_target = target.clone();
        let on_instantiate = Box::new(move |_: &mut ResourceProvider| {
            let texture = callback_target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            debug_assert!(texture.is_some());
            texture
        });

        // If the texture info specifies that mipmapping is required, that implies that the
        // final surface used to instantiate this proxy will be mipmapped, and that the
        // dimensions of that surface are known already.
        let target_proxy = if texture_info.mipmapped() == Mipmapped::Yes {
            TextureProxy::make_lazy(
                caps,
                dimensions,
                texture_info,
                Budgeted::No,
                Volatile::Yes,
                on_instantiate,
            )
        } else {
            Some(TextureProxy::make_fully_lazy(
                texture_info,
                Budgeted::No,
                Volatile::Yes,
                on_instantiate,
            ))
        };
        Self {
            target,
            target_proxy,
        }
    }

    /// `lazyProxy()`.
    #[doc(alias = "lazyProxy")]
    #[must_use]
    pub fn lazy_proxy(&self) -> Option<&Arc<TextureProxy>> {
        self.target_proxy.as_ref()
    }

    /// `refLazyProxy()`.
    #[doc(alias = "refLazyProxy")]
    #[must_use]
    pub fn ref_lazy_proxy(&self) -> Option<Arc<TextureProxy>> {
        self.target_proxy.clone()
    }

    /// `lazyInstantiate()`.
    // Port of: src/gpu/graphite/Recording.cpp#L72-L76 (chrome/m156)
    #[doc(alias = "lazyInstantiate")]
    pub fn lazy_instantiate(
        &self,
        resource_provider: &mut ResourceProvider,
        texture: ResourceRef<Texture>,
    ) -> bool {
        *self
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(texture);
        self.target_proxy
            .as_ref()
            .is_some_and(|proxy| proxy.lazy_instantiate(resource_provider))
    }
}

/// A set of the GPU work a `Recorder` recorded between two `snap()`s.
// Port of: include/gpu/graphite/Recording.h#L30-L90 (chrome/m156)
#[doc(alias = "skgpu::graphite::Recording")]
#[derive(Debug)]
pub struct Recording {
    unique_id: u32,
    recorder_id: u32,

    extra_resource_refs: Vec<AnyResourceRef>,

    root_task_list: TaskList,

    // `std::unordered_set<sk_sp<TextureProxy>>`, kept in first-seen order.
    non_volatile_lazy_proxies: Vec<Arc<TextureProxy>>,
    volatile_lazy_proxies: Vec<Arc<TextureProxy>>,

    target_proxy_data: Option<LazyProxyData>,

    finished_procs: Vec<Arc<RefCntedCallback>>,
}

// Inserts `proxy` unless the set already holds it.
fn insert_proxy(set: &mut Vec<Arc<TextureProxy>>, proxy: &Arc<TextureProxy>) {
    if !set.iter().any(|existing| Arc::ptr_eq(existing, proxy)) {
        set.push(proxy.clone());
    }
}

impl Recording {
    /// `Recording(uniqueID, recorderID, targetProxyData, finishedProcs)`.
    // Port of: src/gpu/graphite/Recording.cpp#L33-L44 (chrome/m156)
    #[must_use]
    pub(crate) fn new(
        unique_id: u32,
        recorder_id: u32,
        target_proxy_data: Option<LazyProxyData>,
        finished_procs: Vec<Arc<RefCntedCallback>>,
    ) -> Self {
        Self {
            unique_id,
            recorder_id,
            extra_resource_refs: Vec::new(),
            root_task_list: TaskList::new(),
            non_volatile_lazy_proxies: Vec::new(),
            volatile_lazy_proxies: Vec::new(),
            target_proxy_data,
            finished_procs,
        }
    }

    /// `priv()`: the Graphite-internal accessors.
    #[doc(alias = "priv")]
    pub fn priv_(&mut self) -> RecordingPriv<'_> {
        RecordingPriv { recording: self }
    }
}

impl Drop for Recording {
    // Port of: src/gpu/graphite/Recording.cpp#L46-L49 (chrome/m156)
    fn drop(&mut self) {
        // Any finished procs that haven't been passed to a CommandBuffer fail
        self.priv_().set_failure_result_for_finished_procs();
    }
}

/// `RecordingPriv`: the accessors the rest of Graphite uses on a `Recording`.
// Port of: src/gpu/graphite/RecordingPriv.h#L28-L81 (chrome/m156)
#[doc(alias = "skgpu::graphite::RecordingPriv")]
#[derive(Debug)]
pub struct RecordingPriv<'a> {
    recording: &'a mut Recording,
}

impl RecordingPriv<'_> {
    /// `deferredTargetProxy()`.
    // Port of: src/gpu/graphite/Recording.cpp#L153-L155 (chrome/m156)
    #[doc(alias = "deferredTargetProxy")]
    #[must_use]
    pub fn deferred_target_proxy(&self) -> Option<&Arc<TextureProxy>> {
        self.recording
            .target_proxy_data
            .as_ref()
            .and_then(LazyProxyData::lazy_proxy)
    }

    /// `setupDeferredTarget()`: fulfills the deferred canvas's lazy proxy with the texture of
    /// the surface it is replayed onto. `surface_texture` is the surface's `target().proxy()`
    /// (`Surface` is ported with G10d). Returns the texture, or `None` if the replay target is
    /// not compatible with the deferred canvas.
    ///
    /// # Panics
    /// If the recording has no deferred target or the surface texture is not instantiated.
    // Port of: src/gpu/graphite/Recording.cpp#L157-L205 (chrome/m156)
    #[doc(alias = "setupDeferredTarget")]
    pub fn setup_deferred_target(
        &mut self,
        resource_provider: &mut ResourceProvider,
        surface_texture: &Arc<TextureProxy>,
        target_translation: IVector,
        target_clip: IRect,
    ) -> Option<Arc<Resource<Texture>>> {
        let target_proxy_data = self
            .recording
            .target_proxy_data
            .as_ref()
            .expect("the recording has a deferred target");
        debug_assert!(surface_texture.is_instantiated());

        let target_proxy = target_proxy_data
            .lazy_proxy()
            .expect("the deferred target has a proxy");
        if surface_texture.mipmapped() != target_proxy.mipmapped() {
            skia_log_e!("Deferred canvas mipmap settings don't match instantiating target's.");
            return None;
        }

        // If the deferred canvas's texture proxy is not fully lazy, that means we used it for
        // draws that require specific dimensions and no translation. The only time this happens
        // is when a client requests a mipmapped deferred canvas and we automatically insert
        // commands to regenerate mipmaps.
        if !target_proxy.is_fully_lazy() {
            debug_assert_eq!(target_proxy.mipmapped(), Mipmapped::Yes);
            if target_proxy.dimensions() != surface_texture.dimensions() {
                skia_log_e!(
                    "Deferred canvas dimensions don't match instantiating target's dimensions."
                );
                return None;
            }
            if !target_translation.is_zero() {
                skia_log_e!(
                    "Replay translation is not allowed when replaying draws to a mipmapped deferred canvas."
                );
                return None;
            }
            if !target_clip.is_empty() {
                skia_log_e!(
                    "Replay clip is not allowed when replaying draws to a mipmapped deferred canvas."
                );
                return None;
            }
        }

        let texture = surface_texture.ref_texture()?;
        let texture_arc = texture.as_arc().clone();
        if !target_proxy_data.lazy_instantiate(resource_provider, texture) {
            skia_log_e!("Could not instantiate deferred texture proxy.");
            return None;
        }

        Some(texture_arc)
    }

    /// `hasVolatileLazyProxies()`.
    #[doc(alias = "hasVolatileLazyProxies")]
    #[must_use]
    pub fn has_volatile_lazy_proxies(&self) -> bool {
        !self.recording.volatile_lazy_proxies.is_empty()
    }

    /// `instantiateVolatileLazyProxies()`.
    // Port of: src/gpu/graphite/Recording.cpp#L108-L116 (chrome/m156)
    #[doc(alias = "instantiateVolatileLazyProxies")]
    pub fn instantiate_volatile_lazy_proxies(
        &mut self,
        resource_provider: &mut ResourceProvider,
    ) -> bool {
        debug_assert!(self.has_volatile_lazy_proxies());
        for proxy in &self.recording.volatile_lazy_proxies {
            if !proxy.lazy_instantiate(resource_provider) {
                return false;
            }
        }
        true
    }

    /// `deinstantiateVolatileLazyProxies()`.
    // Port of: src/gpu/graphite/Recording.cpp#L118-L126 (chrome/m156)
    #[doc(alias = "deinstantiateVolatileLazyProxies")]
    pub fn deinstantiate_volatile_lazy_proxies(&mut self) {
        if !self.has_volatile_lazy_proxies() {
            return;
        }
        for proxy in &self.recording.volatile_lazy_proxies {
            debug_assert!(proxy.is_volatile());
            proxy.deinstantiate();
        }
    }

    /// `hasNonVolatileLazyProxies()`.
    #[doc(alias = "hasNonVolatileLazyProxies")]
    #[must_use]
    pub fn has_non_volatile_lazy_proxies(&self) -> bool {
        !self.recording.non_volatile_lazy_proxies.is_empty()
    }

    /// `instantiateNonVolatileLazyProxies()`.
    // Port of: src/gpu/graphite/Recording.cpp#L92-L106 (chrome/m156)
    #[doc(alias = "instantiateNonVolatileLazyProxies")]
    pub fn instantiate_non_volatile_lazy_proxies(
        &mut self,
        resource_provider: &mut ResourceProvider,
    ) -> bool {
        debug_assert!(self.has_non_volatile_lazy_proxies());
        for proxy in &self.recording.non_volatile_lazy_proxies {
            if !proxy.lazy_instantiate(resource_provider) {
                return false;
            }
        }

        // Note: once all the lazy proxies have been instantiated, that's it - there are no more
        // chances to instantiate.
        self.recording.non_volatile_lazy_proxies.clear();
        true
    }

    /// `setFailureResultForFinishedProcs()`.
    // Port of: src/gpu/graphite/Recording.cpp#L128-L134 (chrome/m156)
    #[doc(alias = "setFailureResultForFinishedProcs")]
    pub fn set_failure_result_for_finished_procs(&mut self) {
        for finished_proc in &self.recording.finished_procs {
            finished_proc.set_failure_result();
        }
        self.recording.finished_procs.clear();
    }

    /// `prepareResources()`.
    // Port of: src/gpu/graphite/Recording.cpp#L207-L228 (chrome/m156)
    #[doc(alias = "prepareResources")]
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        scratch_manager: &mut ScratchResourceManager,
        rte_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> bool {
        let recording = &mut *self.recording;
        let status = recording.root_task_list.prepare_resources(
            resource_provider,
            scratch_manager,
            rte_dict,
        );
        if status == Status::Success {
            let non_volatile = &mut recording.non_volatile_lazy_proxies;
            let volatile = &mut recording.volatile_lazy_proxies;
            let _ = recording.root_task_list.visit_proxies(
                &mut |proxy| {
                    if proxy.is_lazy() {
                        if proxy.is_volatile() {
                            insert_proxy(volatile, proxy);
                        } else {
                            insert_proxy(non_volatile, proxy);
                        }
                    }
                    true
                },
                /*reads_only=*/ false,
            );
        }

        status != Status::Fail
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/Recording.cpp#L230-L255 (chrome/m156)
    #[doc(alias = "addCommands")]
    pub fn add_commands(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_target: Option<Arc<Resource<Texture>>>,
        target_translation: IVector,
        target_clip: IRect,
    ) -> bool {
        let recording = &mut *self.recording;
        for resource in &recording.extra_resource_refs {
            command_buffer.track_resource(resource.clone());
        }

        // There's no need to differentiate kSuccess and kDiscard at the root list level; if
        // every task is discarded, the Recording will automatically be a no-op on replay while
        // still correctly notifying any finish procs the client may have added.
        if recording.root_task_list.add_commands(
            context,
            command_buffer,
            &ReplayTargetData {
                target: replay_target,
                translation: target_translation,
                clip: target_clip,
            },
        ) == Status::Fail
        {
            return false;
        }

        for finished_proc in recording.finished_procs.drain(..) {
            command_buffer.add_finished_proc(finished_proc);
        }
        true
    }

    /// `addResourceRef()`: this will eventually lead to adding a Usage Ref on the
    /// `CommandBuffer`. For now that is fine since the only Resource's we are reffing here are
    /// Buffers. However, if we ever want to track Textures or GPU only Buffers as well, we
    /// should keep a second list for Refs that we want to put `CommandBuffer` refs on.
    // Port of: src/gpu/graphite/Recording.cpp#L257-L259 (chrome/m156)
    #[doc(alias = "addResourceRef")]
    pub fn add_resource_ref(&mut self, resource: AnyResourceRef) {
        self.recording.extra_resource_refs.push(resource);
    }

    /// `taskList()`.
    #[doc(alias = "taskList")]
    pub fn task_list(&mut self) -> &mut TaskList {
        &mut self.recording.root_task_list
    }

    /// `recorderID()`.
    #[doc(alias = "recorderID")]
    #[must_use]
    pub fn recorder_id(&self) -> u32 {
        self.recording.recorder_id
    }

    /// `uniqueID()`.
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.recording.unique_id
    }

    /// `isTargetProxyInstantiated()` (`GPU_TEST_UTILS`).
    ///
    /// # Panics
    /// If the recording has no deferred target.
    #[doc(alias = "isTargetProxyInstantiated")]
    #[must_use]
    pub fn is_target_proxy_instantiated(&self) -> bool {
        self.deferred_target_proxy()
            .expect("the recording has a deferred target")
            .is_instantiated()
    }

    /// `numVolatilePromiseImages()` (`GPU_TEST_UTILS`).
    #[doc(alias = "numVolatilePromiseImages")]
    #[must_use]
    pub fn num_volatile_promise_images(&self) -> usize {
        self.recording.volatile_lazy_proxies.len()
    }

    /// `numNonVolatilePromiseImages()` (`GPU_TEST_UTILS`).
    #[doc(alias = "numNonVolatilePromiseImages")]
    #[must_use]
    pub fn num_non_volatile_promise_images(&self) -> usize {
        self.recording.non_volatile_lazy_proxies.len()
    }

    /// `hasTasks()` (`GPU_TEST_UTILS`).
    #[doc(alias = "hasTasks")]
    #[must_use]
    pub fn has_tasks(&self) -> bool {
        self.recording.root_task_list.has_tasks()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Recording>();
    }
}

// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/Recorder.h, src/gpu/graphite/Recorder.cpp,
//                   src/gpu/graphite/RecorderPriv.h, src/gpu/graphite/RecorderOptionsPriv.h

//! `Recorder`: records the GPU work of the devices that draw through it into `Recording`s.
//!
//! # Ownership (`docs/design/gpu.md` §5.2)
//!
//! `Recorder` is `Recorder { inner: Rc<RecorderInner> }` and therefore `!Send`, as Skia
//! documents. Devices reach it through a `Weak<RecorderInner>`, and the recorder reaches them
//! through [`TrackedDevice`] handles it holds weakly, so neither keeps the other alive and no
//! lifetime appears on a public type. All recorder state is behind `RefCell`s; the recorder
//! calls into a device only at flush points, never while it holds a borrow of its own state, so
//! the devices can call back into [`RecorderPriv`] (`add()`, `addPendingRead()`, ...) while
//! flushing. A dropped recorder makes the device's upgrade fail, which is
//! `abandonRecorder()`'s "draws become no-ops".
//!
//! # Not yet ported
//!
//! The members that need other ports are left out and noted where they were: the atlas provider
//! (G12a), the strike cache and text blob cache (G12b), the `KeyAndDataBuilder` pool (G5a),
//! `makeDeferredCanvas()` and the target proxy device (G10a), the backend texture calls
//! (`BackendTexture`, G11a), `ImageProvider` (G10d), the capture manager and
//! `dumpMemoryStatistics()`.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{BackendApi, Protected, StdSteadyClockTimePoint};
use crate::gpu::ref_cnted_callback::{CallbackProc, RefCntedCallback};
use crate::gpu::token::TokenTracker;
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::buffer_manager::{DrawBufferManager, DrawBufferManagerOptions};
use crate::graphite::caps::Caps;
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::graphite_types::InsertFinishInfo;
use crate::graphite::recording::{LazyProxyData, Recording};
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::{ProxyReadCountMap, ScratchResourceManager};
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::task::TaskRef;
use crate::graphite::task::task_list::TaskList;
use crate::graphite::task::upload_task::{UploadList, UploadTask};
use crate::graphite::texture_info::TextureInfo;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::upload_buffer_manager::UploadBufferManager;

/// `kDefaultRecorderBudget`: 256 MiB.
// Port of: include/gpu/graphite/Recorder.h#L74 (chrome/m156)
pub const DEFAULT_RECORDER_BUDGET: usize = 256 * (1 << 20);

/// Private options that are only meant for testing within Skia's tools.
// Port of: src/gpu/graphite/RecorderOptionsPriv.h#L21-L24 (chrome/m156)
#[doc(alias = "skgpu::graphite::RecorderOptionsPriv")]
#[derive(Clone, Copy, Debug, Default)]
pub struct RecorderOptionsPriv {
    /// Override the default buffer sizes of the `DrawBufferManager` using this option.
    pub dbm_options: Option<DrawBufferManagerOptions>,
}

/// Configuration for recorder creation.
// Port of: include/gpu/graphite/Recorder.h#L66-L81 (chrome/m156)
#[doc(alias = "skgpu::graphite::RecorderOptions")]
#[derive(Clone, Copy, Debug)]
pub struct RecorderOptions {
    /// `fGpuBudgetInBytes`: the budget of the resource cache the recorder owns.
    pub gpu_budget_in_bytes: usize,
    /// `fRequireOrderedRecordings`: overrides the caps' answer if set.
    pub require_ordered_recordings: Option<bool>,
    /// `fRecorderOptionsPriv`.
    pub recorder_options_priv: Option<RecorderOptionsPriv>,
}

impl Default for RecorderOptions {
    fn default() -> Self {
        Self {
            gpu_budget_in_bytes: DEFAULT_RECORDER_BUDGET,
            require_ordered_recordings: None,
            recorder_options_priv: None,
        }
    }
}

/// What the recorder reads from the `SharedContext`.
///
/// `SharedContext` (the base class plus the wgpu half, one struct) is ported with G9b/G11a; it
/// implements this trait.
// Port of: src/gpu/graphite/SharedContext.h (chrome/m156)
pub trait RecorderSharedContext: Send + Sync + std::fmt::Debug {
    /// `caps()`.
    fn caps(&self) -> Arc<dyn Caps>;

    /// `backend()`.
    fn backend(&self) -> BackendApi;

    /// `isProtected()`.
    #[doc(alias = "isProtected")]
    fn is_protected(&self) -> Protected;

    /// `shaderCodeDictionary()`.
    #[doc(alias = "shaderCodeDictionary")]
    fn shader_code_dictionary(&self) -> &ShaderCodeDictionary;

    /// `makeResourceProvider()`: a resource provider with its own resource cache.
    #[doc(alias = "makeResourceProvider")]
    fn make_resource_provider(&self, recorder_id: u32, resource_budget: usize) -> ResourceProvider;
}

/// What the recorder calls on the devices that draw through it (`Device`, ported with G10a,
/// implements this for its `DeviceCore`).
// Port of: src/gpu/graphite/Device.h (chrome/m156)
pub trait TrackedDevice {
    /// `hasPendingReads(dependency)`: does the device have pending draws that read `dependency`?
    #[doc(alias = "hasPendingReads")]
    fn has_pending_reads(&self, dependency: &Arc<TextureProxy>) -> bool;

    /// `flushPendingWork(nullptr)`: adds the device's pending work to the recorder.
    #[doc(alias = "flushPendingWork")]
    fn flush_pending_work(&mut self);

    /// `recorder()`: false once the device has abandoned its recorder.
    fn has_recorder(&self) -> bool;

    /// `abandonRecorder()`.
    #[doc(alias = "abandonRecorder")]
    fn abandon_recorder(&mut self);

    /// `resetStorageCache()`.
    #[doc(alias = "resetStorageCache")]
    fn reset_storage_cache(&mut self);
}

/// A device the recorder tracks. The recorder holds it weakly: the surface's canvas owns the
/// device, and a dropped device simply leaves the tracked list on the next flush.
pub type TrackedDeviceRef = Weak<RefCell<dyn TrackedDevice>>;

// Port of: src/gpu/graphite/Recorder.cpp#L113-L121 (chrome/m156)
fn next_id() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(SK_INVALID_GEN_ID + 1);
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id != SK_INVALID_GEN_ID {
            return id;
        }
    }
}

/// `SK_InvalidGenID`.
const SK_INVALID_GEN_ID: u32 = 0;

/// The recorder's state; devices hold a `Weak` to it.
#[doc(alias = "skgpu::graphite::Recorder")]
pub struct RecorderInner {
    shared_context: Arc<dyn RecorderSharedContext>,
    caps: Arc<dyn Caps>,
    // May be the Context's resource provider.
    resource_provider: SharedResourceProvider,

    runtime_effect_dict: RefCell<Arc<RuntimeEffectDictionary>>,

    root_task_list: RefCell<TaskList>,
    root_uploads: RefCell<UploadList>,

    upload_buffer_manager: Rc<RefCell<UploadBufferManager>>,
    draw_buffer_manager: DrawBufferManager,
    proxy_read_counts: RefCell<ProxyReadCountMap>,

    tracked_devices: RefCell<Vec<Option<TrackedDeviceRef>>>,

    unique_id: u32, // Needed for MessageBox handling for text
    next_recording_id: Cell<u32>,
    require_ordered_recordings: bool,

    token_tracker: RefCell<TokenTracker>,

    finished_procs: RefCell<Vec<Arc<RefCntedCallback>>>,

    target_proxy_data: RefCell<Option<LazyProxyData>>,

    is_flushing_tracked_devices: Cell<bool>,
}

impl std::fmt::Debug for RecorderInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("unique_id", &self.unique_id)
            .field(
                "require_ordered_recordings",
                &self.require_ordered_recordings,
            )
            .finish_non_exhaustive()
    }
}

/// Records the GPU work of the devices that draw through it. See the module docs.
#[doc(alias = "skgpu::graphite::Recorder")]
#[derive(Debug)]
pub struct Recorder {
    inner: Rc<RecorderInner>,
}

impl Recorder {
    /// `Recorder(sharedContext, options, context)`: the recorder uses `context_resource_provider`
    /// (the context's) if given, else makes its own with `options.gpu_budget_in_bytes`.
    ///
    /// Recorders are made by `Context::makeRecorder()`.
    // Port of: src/gpu/graphite/Recorder.cpp#L123-L168 (chrome/m156)
    #[must_use]
    pub fn new(
        shared_context: Arc<dyn RecorderSharedContext>,
        options: &RecorderOptions,
        context_resource_provider: Option<SharedResourceProvider>,
    ) -> Self {
        let unique_id = next_id();
        let caps = shared_context.caps();
        let require_ordered_recordings = options
            .require_ordered_recordings
            .unwrap_or_else(|| caps.require_ordered_recordings());

        // fClientImageProvider (G10d) is not ported.
        let resource_provider = match context_resource_provider {
            Some(resource_provider) => resource_provider,
            None => Arc::new(std::sync::Mutex::new(
                shared_context.make_resource_provider(unique_id, options.gpu_budget_in_bytes),
            )),
        };

        let upload_buffer_manager = Rc::new(RefCell::new(UploadBufferManager::new(
            resource_provider.clone(),
            &*caps,
        )));
        let dbm_options = options
            .recorder_options_priv
            .and_then(|priv_options| priv_options.dbm_options)
            .unwrap_or_default();
        let draw_buffer_manager = DrawBufferManager::new(
            resource_provider.clone(),
            caps.clone(),
            upload_buffer_manager.clone(),
            &dbm_options,
        );

        Self {
            inner: Rc::new(RecorderInner {
                shared_context,
                caps,
                resource_provider,
                runtime_effect_dict: RefCell::new(Arc::new(RuntimeEffectDictionary::new())),
                root_task_list: RefCell::new(TaskList::new()),
                root_uploads: RefCell::new(UploadList::new()),
                upload_buffer_manager,
                draw_buffer_manager,
                proxy_read_counts: RefCell::new(ProxyReadCountMap::new()),
                tracked_devices: RefCell::new(Vec::new()),
                unique_id,
                next_recording_id: Cell::new(1),
                require_ordered_recordings,
                token_tracker: RefCell::new(TokenTracker::default()),
                finished_procs: RefCell::new(Vec::new()),
                target_proxy_data: RefCell::new(None),
                is_flushing_tracked_devices: Cell::new(false),
            }),
        }
    }

    /// The state devices hold a `Weak` to.
    #[must_use]
    pub fn downgrade(&self) -> Weak<RecorderInner> {
        Rc::downgrade(&self.inner)
    }

    /// `priv()`.
    #[doc(alias = "priv")]
    #[must_use]
    pub fn priv_(&self) -> RecorderPriv<'_> {
        self.inner.priv_()
    }

    /// `backend()`.
    #[must_use]
    pub fn backend(&self) -> BackendApi {
        self.inner.shared_context.backend()
    }

    /// `snap()`: finishes recording and creates a `Recording` that can be inserted into a
    /// `Context`, or `None` if recording failed.
    // Port of: src/gpu/graphite/Recorder.cpp#L195-L296 (chrome/m156)
    pub fn snap(&mut self) -> Option<Recording> {
        let inner = &*self.inner;

        // The target proxy device of a deferred canvas (G10a) would be made immutable and
        // released here.

        // Collect all pending tasks on the deferred recording canvas and any other tracked
        // device.
        inner.priv_().flush_tracked_devices("Recorder::Snap");

        // The scratch resources only need to be tracked until prepareResources() is finished,
        // so Recorder doesn't hold a persistent manager and it can be deleted when snap()
        // returns.
        let proxy_read_counts = std::mem::take(&mut *inner.proxy_read_counts.borrow_mut());
        let mut scratch_manager = ScratchResourceManager::new(proxy_read_counts);
        let recording_id = inner.next_recording_id.get();
        inner.next_recording_id.set(recording_id + 1);
        let mut recording = Recording::new(
            recording_id,
            if inner.require_ordered_recordings {
                inner.unique_id
            } else {
                SK_INVALID_GEN_ID
            },
            inner.target_proxy_data.borrow_mut().take(),
            std::mem::take(&mut *inner.finished_procs.borrow_mut()),
        );

        // Allow the buffer managers to add any collected tasks for data transfer or
        // initialization before moving the root task list to the Recording.
        let mut valid = inner
            .draw_buffer_manager
            .transfer_to_recording(&mut recording);
        // We create the Recording's full task list even if the DrawBufferManager failed because
        // it is a convenient way to ensure everything else is unmapped and reset for the next
        // Recording.
        inner
            .upload_buffer_manager
            .borrow_mut()
            .transfer_to_recording(&mut recording);

        // Add one task for all root uploads before the rest of the rendering tasks might depend
        // on them
        let upload_task = {
            let mut root_uploads = inner.root_uploads.borrow_mut();
            if root_uploads.size() > 0 {
                let upload_task = UploadTask::make(&mut root_uploads);
                debug_assert_eq!(root_uploads.size(), 0); // Drained by the newly added task
                upload_task
            } else {
                None
            }
        };
        if let Some(upload_task) = upload_task {
            recording.priv_().task_list().add(upload_task);
        }

        recording
            .priv_()
            .task_list()
            .append(std::mem::take(&mut *inner.root_task_list.borrow_mut()));
        debug_assert!(!inner.root_task_list.borrow().has_tasks());

        // In both the "task failed" case and the "everything is discarded" case, there's no work
        // that needs to be done in insertRecording(). However, we use nullptr as a failure
        // signal, so kDiscard will return a non-null Recording that has no tasks in it.
        let runtime_effect_dict = inner.runtime_effect_dict.borrow().clone();
        {
            let mut resource_provider = inner.lock_resource_provider();
            valid &= recording.priv_().prepare_resources(
                &mut resource_provider,
                &mut scratch_manager,
                Some(&runtime_effect_dict),
            );
        }

        let result = if valid {
            Some(recording)
        } else {
            // The atlas provider would invalidate its atlases here (G12a).
            drop(recording);
            None
        };

        // Process the return queue at least once to keep it from growing too large, as
        // otherwise it's only processed during an explicit cleanup or a cache miss.
        inner
            .lock_resource_provider()
            .force_process_returned_resources();

        // Remaining cleanup that must always happen regardless of success or failure
        *inner.runtime_effect_dict.borrow_mut() = Arc::new(RuntimeEffectDictionary::new());
        *inner.proxy_read_counts.borrow_mut() = ProxyReadCountMap::new();
        let mut index = 0;
        while index < inner.tracked_device_count() {
            if let Some(device) = inner.tracked_device(index) {
                device.borrow_mut().reset_storage_cache();
            }
            index += 1;
        }

        // The atlas provider would invalidate its atlases if recordings need not be ordered
        // (G12a), and the KeyAndDataBuilders would shrink their capacity (G5a).

        result
    }

    /// `maxTextureSize()`.
    #[doc(alias = "maxTextureSize")]
    #[must_use]
    pub fn max_texture_size(&self) -> i32 {
        self.inner.caps.max_texture_size()
    }

    /// `createBackendTexture()`: creates a texture the client owns, or an invalid one if `info`
    /// is invalid or of another backend, or the texture cannot be created.
    // Port of: src/gpu/graphite/Recorder.cpp#L364-L373 (chrome/m156)
    #[doc(alias = "createBackendTexture")]
    #[must_use]
    pub fn create_backend_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
    ) -> BackendTexture {
        if !info.is_valid() || info.backend() != self.backend() {
            return BackendTexture::new();
        }
        self.inner
            .resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .create_backend_texture(dimensions, info)
    }

    /// `deleteBackendTexture()`.
    // Port of: src/gpu/graphite/Recorder.cpp#L523-L531 (chrome/m156)
    #[doc(alias = "deleteBackendTexture")]
    pub fn delete_backend_texture(&mut self, texture: &BackendTexture) {
        if !texture.is_valid() || texture.backend() != self.backend() {
            return;
        }
        self.inner
            .resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .delete_backend_texture(texture);
    }

    /// `addFinishInfo()`: the finished proc is called when the next `Recording` snapped by this
    /// recorder finishes, or with a failure if it never runs.
    // Port of: src/gpu/graphite/Recorder.cpp#L525-L531 (chrome/m156)
    #[doc(alias = "addFinishInfo")]
    pub fn add_finish_info(&mut self, info: InsertFinishInfo) {
        if let Some(finished_proc) = info.finished_proc {
            let callback = RefCntedCallback::make(CallbackProc::Result(finished_proc));
            self.inner.finished_procs.borrow_mut().push(callback);
        }
    }

    /// `freeGpuResources()`.
    // Port of: src/gpu/graphite/Recorder.cpp#L533-L546 (chrome/m156)
    #[doc(alias = "freeGpuResources")]
    pub fn free_gpu_resources(&mut self) {
        // We don't want to free the Uniform or the Draw/UploadBufferManagers sinceall their
        // resources need to be held on to until a Recording is snapped. And once snapped, all
        // their held resources are released. The StrikeCache and TextBlobCache don't hold onto
        // any Gpu resources.

        // Notify the atlas and resource provider to free any resources it can (does not include
        // resources that are locked due to pending work). The atlas provider (G12a) and the
        // strike cache (G12b) are not ported.
        self.inner.lock_resource_provider().free_gpu_resources();
    }

    /// `performDeferredCleanup()`.
    // Port of: src/gpu/graphite/Recorder.cpp#L548-L554 (chrome/m156)
    #[doc(alias = "performDeferredCleanup")]
    pub fn perform_deferred_cleanup(
        &mut self,
        ms_not_used: Duration,
        micros_max_purging_dur: Option<Duration>,
    ) {
        // A purge time before the clock's epoch is before every resource's last access.
        let Some(purge_time) = StdSteadyClockTimePoint::now().checked_sub(ms_not_used) else {
            return;
        };
        self.inner
            .lock_resource_provider()
            .purge_resources_not_used_since(purge_time, micros_max_purging_dur);
    }

    /// `currentBudgetedBytes()`.
    #[doc(alias = "currentBudgetedBytes")]
    #[must_use]
    pub fn current_budgeted_bytes(&self) -> usize {
        self.inner
            .lock_resource_provider()
            .get_resource_cache_current_budgeted_bytes()
    }

    /// `currentPurgeableBytes()`.
    #[doc(alias = "currentPurgeableBytes")]
    #[must_use]
    pub fn current_purgeable_bytes(&self) -> usize {
        self.inner
            .lock_resource_provider()
            .get_resource_cache_current_purgeable_bytes()
    }

    /// `maxBudgetedBytes()`.
    #[doc(alias = "maxBudgetedBytes")]
    #[must_use]
    pub fn max_budgeted_bytes(&self) -> usize {
        self.inner
            .lock_resource_provider()
            .get_resource_cache_limit()
    }

    /// `setMaxBudgetedBytes()`.
    #[doc(alias = "setMaxBudgetedBytes")]
    pub fn set_max_budgeted_bytes(&mut self, bytes: usize) {
        self.inner
            .lock_resource_provider()
            .set_resource_cache_limit(bytes);
    }
}

impl RecorderInner {
    /// `priv()`.
    #[doc(alias = "priv")]
    #[must_use]
    pub fn priv_(&self) -> RecorderPriv<'_> {
        RecorderPriv { recorder: self }
    }

    fn lock_resource_provider(&self) -> std::sync::MutexGuard<'_, ResourceProvider> {
        self.resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // The number of entries of the tracked list, including null and dropped ones.
    fn tracked_device_count(&self) -> usize {
        self.tracked_devices.borrow().len()
    }

    // The tracked device at `index`, or `None` for a null or dropped entry. The tracked list is
    // never borrowed while a device is called.
    fn tracked_device(&self, index: usize) -> Option<Rc<RefCell<dyn TrackedDevice>>> {
        self.tracked_devices
            .borrow()
            .get(index)
            .and_then(|slot| slot.as_ref().and_then(Weak::upgrade))
    }

    /// `registerDevice()`: devices register themselves when created.
    ///
    /// The recorder holds the device weakly: the owner of the device keeps it alive, and the
    /// list is cleaned up on the next `flushTrackedDevices()`.
    // Port of: src/gpu/graphite/Recorder.cpp#L366-L373 (chrome/m156)
    #[doc(alias = "registerDevice")]
    pub fn register_device(&self, device: TrackedDeviceRef) {
        self.tracked_devices.borrow_mut().push(Some(device));
    }

    /// `deregisterDevice()`: devices deregister themselves when they are dropped.
    // Port of: src/gpu/graphite/Recorder.cpp#L375-L383 (chrome/m156)
    #[doc(alias = "deregisterDevice")]
    pub fn deregister_device(&self, device: &TrackedDeviceRef) {
        let mut tracked_devices = self.tracked_devices.borrow_mut();
        for slot in tracked_devices.iter_mut() {
            if slot
                .as_ref()
                .is_some_and(|tracked| Weak::ptr_eq(tracked, device))
            {
                // Don't modify the list structure of fTrackedDevices within this loop
                *slot = None;
                break;
            }
        }
    }
}

impl Drop for RecorderInner {
    // Port of: src/gpu/graphite/Recorder.cpp#L170-L193 (chrome/m156)
    fn drop(&mut self) {
        // Any finished procs that haven't been passed to a Recording fail
        for finished_proc in self.finished_procs.get_mut().iter() {
            finished_proc.set_failure_result();
        }

        for device in self.tracked_devices.get_mut().iter().flatten() {
            // deregisterDevice() may have left an entry as null previously.
            if let Some(device) = device.upgrade() {
                device.borrow_mut().abandon_recorder();
            }
        }
    }
}

/// `RecorderPriv`: the accessors the rest of Graphite uses on a `Recorder`.
// Port of: src/gpu/graphite/RecorderPriv.h#L50-L117 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct RecorderPriv<'a> {
    recorder: &'a RecorderInner,
}

impl RecorderPriv<'_> {
    /// `add()`: adds a task to the root task list.
    // Port of: src/gpu/graphite/Recorder.cpp#L632-L637 (chrome/m156)
    pub fn add(&self, task: TaskRef) {
        self.recorder.root_task_list.borrow_mut().add(task);
    }

    /// `flushTrackedDevices()`: flushes *all* tracked devices created by this `Recorder`.
    ///
    /// `flush_source` is the `SK_DUMP_TASKS` label of the flush.
    ///
    /// # Panics
    /// In debug builds if a flush is already in progress.
    // Port of: src/gpu/graphite/Recorder.cpp#L662-L705 (chrome/m156)
    #[doc(alias = "flushTrackedDevices")]
    pub fn flush_tracked_devices(&self, _flush_source: &str) {
        let recorder = self.recorder;
        debug_assert!(!recorder.is_flushing_tracked_devices.get());
        recorder.is_flushing_tracked_devices.set(true);

        let mut index = 0;
        while index < recorder.tracked_device_count() {
            // Entries may be set to null from a call to deregisterDevice(), which will be
            // cleaned up along with any immutable or uniquely held Devices once everything is
            // flushed.
            if let Some(device) = recorder.tracked_device(index) {
                device.borrow_mut().flush_pending_work();
            }
            index += 1;
        }

        // Issue next upload flush token. This is only used by the atlasing code which always
        // uses this method. Calling in Device::flushPendingWorkToRecorder may miss parent
        // device flushes, increment too often, and lead to atlas corruption.
        let _ = recorder.token_tracker.borrow_mut().issue_flush_token();

        // This version of flushTrackedDevices() is not re-entrant, so it *does* perform the
        // final cleanup on the fTrackedDevices.
        let mut i = 0;
        while i < recorder.tracked_device_count() {
            let device = recorder.tracked_device(i);
            // A device nobody else holds (dropped by its owner, so `unique()` in C++) or that
            // has abandoned its recorder leaves the list.
            let remove = device
                .as_ref()
                .is_none_or(|device| !device.borrow().has_recorder());
            if remove {
                if let Some(device) = &device {
                    device.borrow_mut().abandon_recorder(); // Keep ~Device() happy
                }
                recorder.tracked_devices.borrow_mut().swap_remove(i);
                // Keep i as-is to process what was just shuffled to the ith index.
            } else {
                i += 1;
            }
        }

        recorder.is_flushing_tracked_devices.set(false);
    }

    /// `flushTrackedDevices(dependency)`: flushes tracked devices that have pending reads from
    /// `dependency`.
    // Port of: src/gpu/graphite/Recorder.cpp#L639-L660 (chrome/m156)
    pub fn flush_tracked_devices_with_dependency(&self, dependency: &Arc<TextureProxy>) {
        // This version of flushTrackedDevices() must be re-entrant because it is entirely
        // possible for client-owned surfaces to read and write to each other, where this will be
        // called with different textures for `dependency`. The recursion stops once the
        // encountered surfaces have snapped remaining pending work from their DrawContext. But
        // because we might recurse, we do not perform any cleanup of the fTrackedDevices list.
        // That is deferred until snap() time.
        let mut index = 0;
        while index < self.recorder.tracked_device_count() {
            // Entries may be set to null from a call to deregisterDevice(), which will be
            // cleaned up along with any immutable or uniquely held Devices once everything is
            // snapped.
            if let Some(device) = self.recorder.tracked_device(index) {
                let has_pending_reads = device.borrow().has_pending_reads(dependency);
                if has_pending_reads {
                    device.borrow_mut().flush_pending_work();
                }
            }
            index += 1;
        }

        // TODO(michaelludwig): These flushes are currently only triggered for client-owned
        // surfaces drawn into other surfaces. This function could be used to flush a more
        // targeted set of devices when an atlas fills up; in that case we could increment the
        // flush token as part of that work. As-is, we don't increment the flush token because
        // there could be a tracked atlas that depended on the atlas's texture state that did
        // *not* depend on `dependency` so it still requires the atlas to be using the old flush
        // token. The surfaces that were flushed here could advance to a new token but the token
        // tracking isn't that precise. This all may be moot anyways if we can successfully
        // switch to a rolling atlas page system.
    }

    /// `caps()`.
    #[must_use]
    pub fn caps(&self) -> &Arc<dyn Caps> {
        &self.recorder.caps
    }

    /// `resourceProvider()`.
    #[doc(alias = "resourceProvider")]
    #[must_use]
    pub fn resource_provider(&self) -> &SharedResourceProvider {
        &self.recorder.resource_provider
    }

    /// `shaderCodeDictionary()`.
    #[doc(alias = "shaderCodeDictionary")]
    #[must_use]
    pub fn shader_code_dictionary(&self) -> &ShaderCodeDictionary {
        self.recorder.shared_context.shader_code_dictionary()
    }

    /// `runtimeEffectDictionary()`.
    #[doc(alias = "runtimeEffectDictionary")]
    #[must_use]
    pub fn runtime_effect_dictionary(&self) -> Arc<RuntimeEffectDictionary> {
        self.recorder.runtime_effect_dict.borrow().clone()
    }

    /// `isProtected()`.
    #[doc(alias = "isProtected")]
    #[must_use]
    pub fn is_protected(&self) -> Protected {
        self.recorder.shared_context.is_protected()
    }

    /// `rootUploadList()`.
    #[doc(alias = "rootUploadList")]
    #[must_use]
    pub fn root_upload_list(&self) -> &RefCell<UploadList> {
        &self.recorder.root_uploads
    }

    /// `drawBufferManager()`.
    #[doc(alias = "drawBufferManager")]
    #[must_use]
    pub fn draw_buffer_manager(&self) -> &DrawBufferManager {
        &self.recorder.draw_buffer_manager
    }

    /// `uploadBufferManager()`.
    #[doc(alias = "uploadBufferManager")]
    #[must_use]
    pub fn upload_buffer_manager(&self) -> &Rc<RefCell<UploadBufferManager>> {
        &self.recorder.upload_buffer_manager
    }

    /// `tokenTracker()`.
    #[doc(alias = "tokenTracker")]
    #[must_use]
    pub fn token_tracker(&self) -> &RefCell<TokenTracker> {
        &self.recorder.token_tracker
    }

    /// `addPendingRead()`: temporary access for `DrawTask` to manipulate pending read counts.
    // Port of: src/gpu/graphite/Recorder.cpp#L626-L630 (chrome/m156)
    #[doc(alias = "addPendingRead")]
    pub fn add_pending_read(&self, proxy: &TextureProxy) {
        self.recorder
            .proxy_read_counts
            .borrow_mut()
            .increment(proxy);
    }

    /// `uniqueID()`.
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.recorder.unique_id
    }

    /// `nextRecordingID()` (`SK_DEBUG`).
    #[doc(alias = "nextRecordingID")]
    #[must_use]
    pub fn next_recording_id(&self) -> u32 {
        self.recorder.next_recording_id.get()
    }

    /// `getResourceCacheLimit()`.
    #[doc(alias = "getResourceCacheLimit")]
    #[must_use]
    pub fn get_resource_cache_limit(&self) -> usize {
        self.recorder
            .lock_resource_provider()
            .get_resource_cache_limit()
    }

    /// `deviceIsRegistered()` (`GPU_TEST_UTILS`).
    #[doc(alias = "deviceIsRegistered")]
    #[must_use]
    pub fn device_is_registered(&self, device: &TrackedDeviceRef) -> bool {
        self.recorder.tracked_devices.borrow().iter().any(|slot| {
            slot.as_ref()
                .is_some_and(|tracked| Weak::ptr_eq(tracked, device))
        })
    }

    /// `issueFlushToken()` (`GPU_TEST_UTILS`).
    #[doc(alias = "issueFlushToken")]
    pub fn issue_flush_token(&self) {
        let _ = self.recorder.token_tracker.borrow_mut().issue_flush_token();
    }

    /// `numRootTasks()` (`GPU_TEST_UTILS`).
    #[doc(alias = "numRootTasks")]
    #[must_use]
    pub fn num_root_tasks(&self) -> usize {
        self.recorder.root_task_list.borrow().size()
    }
}

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
//! The members that need other ports are left out and noted where they were: the `KeyAndDataBuilder` pool (G5a),
//! `makeDeferredCanvas()` and the target proxy device (G10a), the backend texture calls
//! (`BackendTexture`, G11a), `ImageProvider` (G10d), the capture manager and
//! `dumpMemoryStatistics()`.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{BackendApi, Budgeted, Mipmapped, Protected, StdSteadyClockTimePoint};
use crate::gpu::ref_cnted_callback::{CallbackProc, RefCntedCallback};
use crate::gpu::token::TokenTracker;
use crate::graphite::atlas_provider::AtlasProvider;
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::buffer_manager::{DrawBufferManager, DrawBufferManagerOptions};
use crate::graphite::caps::Caps;
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::graphics_pipeline_desc::PipelineHandleFactory;
use crate::graphite::graphite_types::InsertFinishInfo;
use crate::graphite::paint_params_key::PaintParamsKeyBuilder;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::proxy_cache::ProxyCache;
use crate::graphite::recording::{LazyProxyData, Recording};
use crate::graphite::renderer_provider::PathRendererStrategy as RendererProviderStrategy;
use crate::graphite::renderer_provider::RendererProvider;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::{ProxyReadCountMap, ScratchResourceManager};
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::task::TaskRef;
use crate::graphite::task::task_list::TaskList;
use crate::graphite::task::upload_task::{UploadList, UploadTask};
use crate::graphite::texture_info::TextureInfo;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_utils::make_bitmap_proxy_view;
use crate::graphite::upload_buffer_manager::UploadBufferManager;
use crate::text_gpu::strike_cache::StrikeCache;
use crate::text_gpu::text_blob_redraw_coordinator::TextBlobRedrawCoordinator;

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

    /// `rendererProvider()`: the renderers draws are recorded with. They are shared by the
    /// context and all its recorders (`SharedContext::rendererProvider()`).
    #[doc(alias = "rendererProvider")]
    fn renderer_provider(&self) -> &RendererProvider;

    /// `pipelineManager()`: the pipeline manager draw passes create their pipelines with.
    /// `None` until `PipelineManager` is ported (G9b); draw passes then cannot create pipelines.
    #[doc(alias = "pipelineManager")]
    fn pipeline_manager(&self) -> Option<Arc<dyn PipelineHandleFactory>> {
        None
    }
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

    /// The device's ID (`DeviceLink`, `docs/design/gpu.md` §5.6); 0 for a device that images
    /// cannot link to.
    fn device_id(&self) -> u32 {
        0
    }

    /// The Graphite device behind this tracked device, for the image links that flush it
    /// (`Image_Base::notifyInUse` holds `sk_sp<Device>`).
    fn as_device_core(&mut self) -> Option<&mut crate::graphite::device::DeviceCore> {
        None
    }

    /// Whether `other` is this device's own cell (the device that is mutably borrowed while it
    /// records a draw cannot be borrowed again).
    fn is_cell(&self, _other: &Rc<RefCell<dyn TrackedDevice>>) -> bool {
        false
    }
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

/// `Recorder::kMaxKeyAndDataBuilders`.
// Port of: include/gpu/graphite/Recorder.h#L252 (chrome/m156)
const MAX_KEY_AND_DATA_BUILDERS: usize = 2;

/// The scratch state a draw collects its paint key and data in (`KeyAndDataBuilder`, a
/// `std::pair<PipelineDataGatherer, PaintParamsKeyBuilder>`). Both are in `RefCell`s because the
/// `KeyContext` that walks a paint holds them by shared reference, as it holds pointers in C++.
// Port of: include/gpu/graphite/Recorder.h (KeyAndDataBuilder) (chrome/m156)
#[derive(Debug)]
pub struct KeyAndDataBuilder {
    /// `first`.
    pub gatherer: RefCell<PipelineDataGatherer>,
    /// `second`.
    pub builder: RefCell<PaintParamsKeyBuilder>,
}

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

    key_and_data_builders: RefCell<Vec<KeyAndDataBuilder>>,

    /// `fAtlasProvider`: the path, clip and glyph atlases the draws of this recorder share.
    atlas_provider: RefCell<AtlasProvider>,

    /// `fStrikeCache`: the strikes of the glyphs on this recorder's atlases.
    strike_cache: RefCell<StrikeCache>,

    /// `fTextBlobCache`: the processed text blobs this recorder can draw again.
    text_blob_cache: TextBlobRedrawCoordinator,
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

        // fClientImageProvider is not ported: a client's ImageProvider is not an option of the
        // Copy-only RecorderOptions. Graphite uses the DefaultImageProvider (image_provider).
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
        // `fAtlasProvider(std::make_unique<AtlasProvider>(this))`: the clip atlas is used only by
        // the raster path atlas strategy.
        let raster_path_strategy = shared_context.renderer_provider().path_renderer_strategy()
            == RendererProviderStrategy::RasterAtlas;
        let atlas_provider = AtlasProvider::new(&*caps, raster_path_strategy);

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
                key_and_data_builders: RefCell::new(Vec::new()),
                atlas_provider: RefCell::new(atlas_provider),
                strike_cache: RefCell::new(StrikeCache::new()),
                text_blob_cache: TextBlobRedrawCoordinator::new(unique_id),
            }),
        }
    }

    /// The state devices hold a `Weak` to.
    #[must_use]
    pub fn downgrade(&self) -> Weak<RecorderInner> {
        Rc::downgrade(&self.inner)
    }

    /// A handle on the recorder a device upgraded its `Weak<RecorderInner>` to. Dropping the
    /// handle only drops that reference: it does not end the recorder.
    #[must_use]
    pub fn from_inner(inner: Rc<RecorderInner>) -> Self {
        Self { inner }
    }

    /// A shared handle to the recorder's state, for the objects that borrow it while the recorder
    /// is used (`DispatchGroup::Builder` in `Recorder::priv()` terms).
    pub(crate) fn inner(&self) -> Rc<RecorderInner> {
        self.inner.clone()
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
            inner.atlas_provider.borrow_mut().invalidate_atlases();
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

        if !inner.require_ordered_recordings {
            inner.atlas_provider.borrow_mut().invalidate_atlases();
        }

        // For each KeyAndDataBuilder owned by the Recorder, check if the high watermark of data
        // usage over the lifetime snap is less than half of allocated capacity. If so, shrink the
        // capacity.
        for key_db in inner.key_and_data_builders.borrow().iter() {
            key_db.gatherer.borrow_mut().try_shrink_capacity();
            key_db.builder.borrow_mut().try_shrink_capacity();
        }

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
        // resources that are locked due to pending work).
        let recorder: &Recorder = self;
        recorder
            .inner
            .atlas_provider
            .borrow_mut()
            .free_gpu_resources(recorder);
        self.inner.lock_resource_provider().free_gpu_resources();

        // This is technically not GPU memory, but there's no other place for the client to tell
        // us to clean this up, and without any cleanup it can grow unbounded.
        self.inner.strike_cache.borrow_mut().free_all();
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

    pub(crate) fn lock_resource_provider(&self) -> std::sync::MutexGuard<'_, ResourceProvider> {
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

impl<'a> RecorderPriv<'a> {
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
    pub fn flush_tracked_devices(&self, flush_source: &str) {
        (*self).flush_tracked_devices_with_current(flush_source, None);
    }

    /// `flushTrackedDevices()` called while `current` records a draw (an atlas draw of its device
    /// needs the atlases flushed first). `current` is mutably borrowed for the draw, so it is
    /// flushed through this reference, where C++ reaches it through the tracked list.
    // Port of: src/gpu/graphite/Recorder.cpp#L662-L705 (chrome/m156)
    pub fn flush_tracked_devices_and_current(
        &self,
        flush_source: &str,
        current: &mut dyn TrackedDevice,
    ) {
        (*self).flush_tracked_devices_with_current(flush_source, Some(current));
    }

    // The body of `flushTrackedDevices()`, with the device that is borrowed for the draw (if any).
    // Port of: src/gpu/graphite/Recorder.cpp#L662-L705 (chrome/m156)
    fn flush_tracked_devices_with_current(
        self,
        _flush_source: &str,
        mut current: Option<&mut dyn TrackedDevice>,
    ) {
        let recorder = self.recorder;
        debug_assert!(!recorder.is_flushing_tracked_devices.get());
        recorder.is_flushing_tracked_devices.set(true);

        let mut index = 0;
        while index < recorder.tracked_device_count() {
            // Entries may be set to null from a call to deregisterDevice(), which will be
            // cleaned up along with any immutable or uniquely held Devices once everything is
            // flushed.
            if let Some(device) = recorder.tracked_device(index) {
                if let Ok(mut device) = device.try_borrow_mut() {
                    device.flush_pending_work();
                } else if let Some(current) = current.as_deref_mut()
                    && current.is_cell(&device)
                {
                    // The device recording the draw flushes through the reference it lent.
                    current.flush_pending_work();
                }
                // Any other borrowed device is the one that triggered this flush from inside its
                // own operation (e.g. `Device::flushPendingWork()` flushing its dependencies), and
                // it flushes itself.
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
                .is_none_or(|device| device.try_borrow().is_ok_and(|d| !d.has_recorder()));
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
        self.flush_tracked_devices_with_dependency_and_current(dependency, None);
    }

    /// `flushTrackedDevices(dependency)` called while `current` records a draw (an image linked
    /// to another device was drawn into it, `Image_Base::notifyInUse`). `current` is mutably
    /// borrowed for the draw, so when it has pending reads of `dependency` it is flushed through
    /// this reference, where C++ reaches it through the tracked list.
    // Port of: src/gpu/graphite/Recorder.cpp#L639-L660 (chrome/m156)
    pub fn flush_tracked_devices_with_dependency_and_current(
        &self,
        dependency: &Arc<TextureProxy>,
        mut current: Option<&mut dyn TrackedDevice>,
    ) {
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
                if let Ok(borrowed) = device.try_borrow() {
                    let has_pending_reads = borrowed.has_pending_reads(dependency);
                    drop(borrowed);
                    if has_pending_reads {
                        device.borrow_mut().flush_pending_work();
                    }
                } else if let Some(current) = current.as_deref_mut()
                    && current.is_cell(&device)
                {
                    // The device recording a draw that reads `dependency`'s image.
                    if current.has_pending_reads(dependency) {
                        current.flush_pending_work();
                    }
                }
                // Any other borrowed device is the one that triggered this flush from inside its
                // own operation: it does not read its own target.
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

    /// `registerDevice(device)`.
    #[doc(alias = "registerDevice")]
    pub fn register_device(&self, device: TrackedDeviceRef) {
        self.recorder.register_device(device);
    }

    /// `deregisterDevice(device)`.
    #[doc(alias = "deregisterDevice")]
    pub fn deregister_device(&self, device: &TrackedDeviceRef) {
        self.recorder.deregister_device(device);
    }

    /// `popOrCreateKeyAndDataBuilder()`.
    // Port of: src/gpu/graphite/Recorder.cpp#L700-L715 (chrome/m156)
    #[doc(alias = "popOrCreateKeyAndDataBuilder")]
    #[must_use]
    pub fn pop_or_create_key_and_data_builder(&self) -> KeyAndDataBuilder {
        if let Some(key_db) = self.recorder.key_and_data_builders.borrow_mut().pop() {
            return key_db;
        }

        let use_storage_buffers = self.caps().storage_buffer_support();
        let binding_req = self.caps().resource_binding_requirements();
        let gatherer_layout = if use_storage_buffers {
            binding_req.storage_buffer_layout
        } else {
            binding_req.uniform_buffer_layout
        };

        KeyAndDataBuilder {
            gatherer: RefCell::new(PipelineDataGatherer::new(gatherer_layout)),
            builder: RefCell::new(PaintParamsKeyBuilder::new(self.shader_code_dictionary())),
        }
    }

    /// `pushKeyAndDataBuilder(keyDB)`.
    // Port of: src/gpu/graphite/Recorder.cpp#L717-L725 (chrome/m156)
    #[doc(alias = "pushKeyAndDataBuilder")]
    pub fn push_key_and_data_builder(&self, key_db: KeyAndDataBuilder) {
        let mut builders = self.recorder.key_and_data_builders.borrow_mut();
        if builders.len() < MAX_KEY_AND_DATA_BUILDERS {
            builders.push(key_db);
        }
        // If no empty slot was found, the "keyDB" goes out of scope here.
    }

    /// `rendererProvider()`.
    #[doc(alias = "rendererProvider")]
    #[must_use]
    pub fn renderer_provider(&self) -> &'a RendererProvider {
        let recorder: &'a RecorderInner = self.recorder;
        recorder.shared_context.renderer_provider()
    }

    /// `sharedContext()->pipelineManager()`.
    #[doc(alias = "pipelineManager")]
    #[must_use]
    pub fn pipeline_manager(&self) -> Option<Arc<dyn PipelineHandleFactory>> {
        self.recorder.shared_context.pipeline_manager()
    }

    /// `atlasProvider()`.
    #[doc(alias = "atlasProvider")]
    #[must_use]
    pub fn atlas_provider(&self) -> &'a RefCell<AtlasProvider> {
        let recorder: &'a RecorderInner = self.recorder;
        &recorder.atlas_provider
    }

    /// `strikeCache()`.
    #[doc(alias = "strikeCache")]
    #[must_use]
    pub fn strike_cache(&self) -> &'a RefCell<StrikeCache> {
        let recorder: &'a RecorderInner = self.recorder;
        &recorder.strike_cache
    }

    /// `textBlobCache()`.
    #[doc(alias = "textBlobCache")]
    #[must_use]
    pub fn text_blob_cache(&self) -> &'a TextBlobRedrawCoordinator {
        let recorder: &'a RecorderInner = self.recorder;
        &recorder.text_blob_cache
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

    /// `RecorderPriv::CreateCachedProxy(recorder, bitmap, label)`: the texture of `bitmap`, cached
    /// in the recorder's proxy cache by the bitmap's pixel identity. `None` without a recorder
    /// (the pre-compile path), or if the texture cannot be created.
    ///
    /// The cache entry is invalidated when the bitmap's pixel ref is destroyed or changes, as long
    /// as the bitmap is shared (otherwise nothing else can change its pixels).
    // Port of: src/gpu/graphite/Recorder.cpp#L727-L736 (chrome/m156), with the bitmap generator of
    // src/gpu/graphite/ProxyCache.cpp#L97-L140 (chrome/m156)
    #[doc(alias = "CreateCachedProxy")]
    pub fn create_cached_proxy(
        recorder: Option<&Recorder>,
        bitmap: &Bitmap,
        label: &str,
    ) -> Option<Arc<TextureProxy>> {
        debug_assert!(!bitmap.is_null());
        let recorder = recorder?;
        let priv_ = recorder.priv_();

        let key = ProxyCache::bitmap_key(bitmap);
        let shared = Arc::clone(priv_.resource_provider());
        // The provider's lock is held only while the cache is consulted: creating and uploading
        // the proxy lock it again.
        {
            let mut provider = shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(cached) = provider.proxy_cache()?.find_cache_entry(&key) {
                return Some(cached);
            }
        }

        // Cache miss: create the proxy and upload the bitmap into it.
        let proxy =
            make_bitmap_proxy_view(recorder, bitmap, None, Mipmapped::No, Budgeted::Yes, label)?
                .ref_proxy()?;

        // The bitmap may be held by more than just this call, so add a listener that removes the
        // entry when the pixels go away.
        let mut provider = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let listener = if bitmap.pixel_ref().is_some() && !bitmap.pixel_ref_is_unique() {
            let listener = provider
                .proxy_cache()?
                .make_unique_key_invalidation_listener(&key);
            if let Some(pixel_ref) = bitmap.pixel_ref() {
                pixel_ref.add_gen_id_change_listener(Some(Arc::clone(&listener)));
            }
            Some(listener)
        } else {
            None
        };

        provider
            .proxy_cache()?
            .insert_cache_entry(&key, Arc::clone(&proxy), listener);
        Some(proxy)
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

    /// The tracked device whose [`TrackedDevice::device_id`] is `device_id`, if it is still
    /// tracked and alive (the `sk_sp<Device>` an image link holds in C++).
    #[must_use]
    pub fn find_tracked_device(&self, device_id: u32) -> Option<Rc<RefCell<dyn TrackedDevice>>> {
        if device_id == 0 {
            return None;
        }
        (0..self.recorder.tracked_device_count())
            .filter_map(|index| self.recorder.tracked_device(index))
            .find(|device| {
                device
                    .try_borrow()
                    .is_ok_and(|device| device.device_id() == device_id)
            })
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

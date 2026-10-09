// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: tests/graphite/RecorderTest.cpp, tests/graphite/RecordingOrderTest.cpp,
//                    tests/graphite/SubmitWithFinishProcTest.cpp

//! `Recorder` and `Recording` against the mock back end. Skia's own tests need a GPU `Context`
//! and `Device`s (G9b/G10/G11), so they are not the 1:1 manifest ports; the scenarios and
//! assertions below follow them with mock devices and a mock command buffer.

mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use skia_rust_core::point::IVector;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, CallbackResult, Mipmapped};
use skia_rust_gpu::graphite::graphite_types::{InsertFinishInfo, SampleCount, Volatile};
use skia_rust_gpu::graphite::recorder::{
    Recorder, RecorderOptions, TrackedDevice, TrackedDeviceRef,
};
use skia_rust_gpu::graphite::task::draw_task::DrawTask;
use skia_rust_gpu::graphite::task::upload_task::{MipLevel, UploadSource};
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use skia_rust_gpu::graphite::texture_proxy_view::TextureProxyView;
use support::{
    Call, MockCaps, MockCommandBuffer, MockContext, make_recorder, make_recorder_with, rgba_info,
    texture_info,
};

/// A device the recorder tracks: counts flushes, can abandon its recorder.
struct MockDevice {
    recorder: Cell<bool>,
    flushes: Cell<usize>,
    resets: Cell<usize>,
    reads: RefCell<Vec<Arc<TextureProxy>>>,
}

impl MockDevice {
    fn make() -> Rc<RefCell<dyn TrackedDevice>> {
        Rc::new(RefCell::new(MockDevice {
            recorder: Cell::new(true),
            flushes: Cell::new(0),
            resets: Cell::new(0),
            reads: RefCell::new(Vec::new()),
        }))
    }
}

impl TrackedDevice for MockDevice {
    fn has_pending_reads(&self, dependency: &Arc<TextureProxy>) -> bool {
        self.reads
            .borrow()
            .iter()
            .any(|proxy| Arc::ptr_eq(proxy, dependency))
    }

    fn flush_pending_work(&mut self) {
        self.flushes.set(self.flushes.get() + 1);
    }

    fn has_recorder(&self) -> bool {
        self.recorder.get()
    }

    fn abandon_recorder(&mut self) {
        self.recorder.set(false);
    }

    fn reset_storage_cache(&mut self) {
        self.resets.set(self.resets.get() + 1);
    }
}

/// A concrete device whose counters tests can read (`Rc<RefCell<MockDevice>>` coerces to the
/// trait object the recorder tracks).
fn counting_device() -> (Rc<RefCell<MockDevice>>, Rc<RefCell<dyn TrackedDevice>>) {
    let device = Rc::new(RefCell::new(MockDevice {
        recorder: Cell::new(true),
        flushes: Cell::new(0),
        resets: Cell::new(0),
        reads: RefCell::new(Vec::new()),
    }));
    let tracked: Rc<RefCell<dyn TrackedDevice>> = device.clone();
    (device, tracked)
}

fn register(recorder: &Recorder, device: &Rc<RefCell<dyn TrackedDevice>>) -> TrackedDeviceRef {
    let weak = Rc::downgrade(device);
    recorder
        .downgrade()
        .upgrade()
        .expect("the recorder is alive")
        .register_device(weak.clone());
    weak
}

// Tests to make sure the managing of back pointers between Recorder and Device all work
// properly.
// Port of: tests/graphite/RecorderTest.cpp#L20-L96 (chrome/m156)
#[test]
fn recorder_device_ptr_test() {
    let (mut recorder, _) = make_recorder(MockCaps::default());
    let inner = recorder.downgrade();

    // Add multiple devices to later test different patterns of destruction.
    let device1 = MockDevice::make();
    let device2 = MockDevice::make();
    let device3 = MockDevice::make();
    let device4 = MockDevice::make();
    let w1 = register(&recorder, &device1);
    let w2 = register(&recorder, &device2);
    let w3 = register(&recorder, &device3);
    let w4 = register(&recorder, &device4);

    assert!(recorder.priv_().device_is_registered(&w1));
    assert!(recorder.priv_().device_is_registered(&w2));
    assert!(recorder.priv_().device_is_registered(&w3));
    assert!(recorder.priv_().device_is_registered(&w4));

    // Test freeing a device in the middle, deregistering it as setImmutable() would when ~Surface()
    // or our FilterResult is done with the device.
    inner.upgrade().unwrap().deregister_device(&w2);
    device2.borrow_mut().abandon_recorder();
    assert!(!device2.borrow().has_recorder());
    assert!(!recorder.priv_().device_is_registered(&w2));
    drop(device2);

    assert!(recorder.priv_().device_is_registered(&w1));
    assert!(recorder.priv_().device_is_registered(&w3));
    assert!(recorder.priv_().device_is_registered(&w4));

    // Test freeing a device that wasn't deregistered, which should be dropped from the tracked
    // list automatically when the recorder flushes.
    drop(device4);
    assert!(recorder.priv_().device_is_registered(&w4)); // the list still has the (dead) entry
    recorder.priv_().flush_tracked_devices("RecorderTest"); // should drop device4's entry now
    assert!(!recorder.priv_().device_is_registered(&w4));
    assert!(recorder.priv_().device_is_registered(&w1));
    assert!(recorder.priv_().device_is_registered(&w3));

    // Snapping flushes the remaining devices too.
    let _ = recorder.snap();

    // Delete the recorder and make sure remaining devices no longer have a valid recorder.
    drop(recorder);
    assert!(inner.upgrade().is_none());
    assert!(!device1.borrow().has_recorder());
    assert!(!device3.borrow().has_recorder());

    // Make sure freeing Devices after recorder doesn't cause any crash. This would get checked
    // naturally when these devices go out of scope, but manually dropping will give us a better
    // stack trace if something does go wrong.
    drop(device1);
    drop(device3);
}

#[test]
fn recorder_flushes_and_resets_tracked_devices() {
    let (mut recorder, _) = make_recorder(MockCaps::default());
    let (device, tracked) = counting_device();
    let _w = register(&recorder, &tracked);

    recorder.priv_().flush_tracked_devices("test");
    assert_eq!(device.borrow().flushes.get(), 1);

    // A flush for a dependency only flushes the devices that read it.
    let proxy =
        TextureProxy::make_fully_lazy(&rgba_info(), Budgeted::No, Volatile::No, Box::new(|_| None));
    recorder
        .priv_()
        .flush_tracked_devices_with_dependency(&proxy);
    assert_eq!(device.borrow().flushes.get(), 1);
    device.borrow().reads.borrow_mut().push(proxy.clone());
    recorder
        .priv_()
        .flush_tracked_devices_with_dependency(&proxy);
    assert_eq!(device.borrow().flushes.get(), 2);

    // snap() flushes every device and resets their storage caches.
    let recording = recorder.snap();
    assert!(recording.is_some());
    assert_eq!(device.borrow().flushes.get(), 3);
    assert_eq!(device.borrow().resets.get(), 1);
}

// Tests to make sure the Recorders can override the ordering requirement.
// Port of: tests/graphite/RecorderTest.cpp#L98-L136 (chrome/m156)
#[test]
fn recorder_ordering_test() {
    // Whether or not the caps require ordered recordings, the options override them.
    for caps_require_ordered in [true, false] {
        let caps = MockCaps {
            require_ordered_recordings: caps_require_ordered,
            ..MockCaps::default()
        };
        let (mut ordered_recorder, unordered_recorder);
        if caps_require_ordered {
            (ordered_recorder, _) = make_recorder(caps.clone());
            let opts = RecorderOptions {
                require_ordered_recordings: Some(false),
                ..RecorderOptions::default()
            };
            (unordered_recorder, _) = make_recorder_with(caps, &opts);
        } else {
            let opts = RecorderOptions {
                require_ordered_recordings: Some(true),
                ..RecorderOptions::default()
            };
            (ordered_recorder, _) = make_recorder_with(caps.clone(), &opts);
            (unordered_recorder, _) = make_recorder(caps);
        }
        let mut unordered_recorder = unordered_recorder;

        let mut o1 = ordered_recorder.snap().unwrap();
        let mut o2 = ordered_recorder.snap().unwrap();
        let mut o3 = ordered_recorder.snap().unwrap();
        let mut u1 = unordered_recorder.snap().unwrap();
        let mut u2 = unordered_recorder.snap().unwrap();

        // Recordings of an ordered recorder carry their recorder's id and a sequence number
        // the context checks the insertion order with; unordered ones carry no recorder id.
        let ordered_id = ordered_recorder.priv_().unique_id();
        assert_eq!(o1.priv_().recorder_id(), ordered_id);
        assert_eq!(o2.priv_().recorder_id(), ordered_id);
        assert_eq!(o3.priv_().recorder_id(), ordered_id);
        assert_eq!(u1.priv_().recorder_id(), 0);
        assert_eq!(u2.priv_().recorder_id(), 0);
        assert_eq!(
            [
                o1.priv_().unique_id(),
                o2.priv_().unique_id(),
                o3.priv_().unique_id()
            ],
            [1, 2, 3]
        );
        assert_eq!([u1.priv_().unique_id(), u2.priv_().unique_id()], [1, 2]);
        assert_ne!(ordered_id, unordered_recorder.priv_().unique_id());
    }
}

// Port of: tests/graphite/SubmitWithFinishProcTest.cpp#L28-L69 (chrome/m156)
#[test]
fn submit_with_finish_proc_pending_command_buffer() {
    let (mut recorder, _) = make_recorder(MockCaps::default());
    let results: Arc<Mutex<Vec<CallbackResult>>> = Arc::default();

    // Add work to the pending command buffer with a finish proc on the recording.
    let sink = results.clone();
    recorder.add_finish_info(InsertFinishInfo::new(Box::new(move |result| {
        sink.lock().unwrap().push(result);
    })));
    let mut recording = recorder.snap().unwrap();

    // Replaying the recording hands the finish proc to the command buffer.
    let (mut context, _) = MockContext::new(MockCaps::default());
    let mut command_buffer = MockCommandBuffer::default();
    assert!(recording.priv_().add_commands(
        &mut context,
        &mut command_buffer,
        None,
        IVector::default(),
        IRect::default(),
    ));
    assert!(command_buffer.calls.contains(&Call::FinishedProc));
    // The recording no longer owns the proc, so dropping it does not fail the proc.
    drop(recording);
    assert!(results.lock().unwrap().is_empty());

    // The GPU finishing the work drops the command buffer's reference: the proc runs with
    // success.
    drop(command_buffer);
    assert_eq!(*results.lock().unwrap(), [CallbackResult::Success]);
}

#[test]
fn finish_proc_fails_if_the_recording_is_never_inserted() {
    let (mut recorder, _) = make_recorder(MockCaps::default());
    let calls = Arc::new(AtomicU32::new(0));
    let failures = Arc::new(AtomicU32::new(0));
    let (c, f) = (calls.clone(), failures.clone());
    recorder.add_finish_info(InsertFinishInfo::new(Box::new(move |result| {
        c.fetch_add(1, Ordering::SeqCst);
        if result == CallbackResult::Failed {
            f.fetch_add(1, Ordering::SeqCst);
        }
    })));
    let recording = recorder.snap().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(recording);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(failures.load(Ordering::SeqCst), 1);

    // A finish proc added after the last snap fails when the recorder goes away.
    let (c, f) = (calls.clone(), failures.clone());
    recorder.add_finish_info(InsertFinishInfo::new(Box::new(move |result| {
        c.fetch_add(1, Ordering::SeqCst);
        if result == CallbackResult::Failed {
            f.fetch_add(1, Ordering::SeqCst);
        }
    })));
    drop(recorder);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(failures.load(Ordering::SeqCst), 2);

    // An info without a proc adds nothing.
    let (mut recorder, _) = make_recorder(MockCaps::default());
    recorder.add_finish_info(InsertFinishInfo::default());
    assert!(recorder.snap().is_some());
}

#[test]
fn snap_adds_root_uploads_and_prepares_resources() {
    let (mut recorder, shared_context) = make_recorder(MockCaps::default());
    let (mut context, _) = MockContext::new(MockCaps::default());
    let caps = shared_context.caps.clone();

    // A root upload to a 2x2 texture.
    let provider = recorder.priv_().resource_provider().clone();
    let proxy = TextureProxy::make(
        4096,
        &mut provider.lock().unwrap(),
        ISize::new(2, 2),
        &rgba_info(),
        Budgeted::Yes,
        "UploadTarget",
    )
    .unwrap();
    let pixels = [7u8; 16];
    let level = MipLevel {
        pixels: Some(&pixels),
        row_bytes: 8,
    };
    let color_info = skia_rust_core::image_info::ColorInfo::new(
        skia_rust_core::color_type::ColorType::RGBA8888,
        skia_rust_core::alpha_type::AlphaType::Premul,
        None,
    );
    let source = UploadSource::make(
        &*caps,
        &TextureProxyView::from_proxy(Some(proxy.clone())),
        &color_info,
        &color_info,
        &[level],
        IRect::from_wh(2, 2),
    );
    assert!(source.is_valid());
    {
        let priv_ = recorder.priv_();
        let mut upload_manager = priv_.upload_buffer_manager().borrow_mut();
        assert!(priv_.root_upload_list().borrow_mut().record_upload(
            &*caps,
            &mut upload_manager,
            &source,
            None,
        ));
    }

    // A draw task added after it.
    let target = TextureProxy::make(
        4096,
        &mut provider.lock().unwrap(),
        ISize::new(2, 2),
        &rgba_info(),
        Budgeted::Yes,
        "DrawTarget",
    )
    .unwrap();
    recorder
        .priv_()
        .add(DrawTask::new(target.clone()).into_ref());
    assert_eq!(recorder.priv_().num_root_tasks(), 1);

    let mut recording = recorder.snap().unwrap();
    // The upload task goes first. The draw task has no children, so prepareResources()
    // discards it.
    assert!(recording.priv_().has_tasks());
    assert_eq!(recorder.priv_().num_root_tasks(), 0);
    // The upload target was instantiated by prepareResources().
    assert!(proxy.is_instantiated());

    let mut command_buffer = MockCommandBuffer::default();
    assert!(recording.priv_().add_commands(
        &mut context,
        &mut command_buffer,
        None,
        IVector::default(),
        IRect::default(),
    ));
    assert!(
        command_buffer
            .calls
            .iter()
            .any(|call| matches!(call, Call::CopyBufferToTexture(copies) if copies.len() == 1))
    );
}

#[test]
fn recording_buffers_return_to_the_cache_once_the_recording_is_gone() {
    let (mut recorder, _) = make_recorder(MockCaps::default());
    {
        let priv_ = recorder.priv_();
        let mut info = priv_
            .draw_buffer_manager()
            .get_mapped_vertex_buffer(8, 4, 0, 1)
            .unwrap();
        info.writer().write_bytes(&[3u8; 32]);
    }
    let recording = recorder.snap().unwrap();
    // The recording holds the buffer, so none of the budgeted bytes is purgeable yet.
    let budgeted = recorder.current_budgeted_bytes();
    assert!(budgeted > 0);
    assert_eq!(recorder.current_purgeable_bytes(), 0);

    drop(recording);
    // Dropping the recording returns the buffer to the cache's return queue, which snap()
    // processes.
    let _ = recorder.snap();
    assert_eq!(recorder.current_purgeable_bytes(), budgeted);

    // Freeing the GPU resources drops them.
    recorder.free_gpu_resources();
    assert_eq!(recorder.current_budgeted_bytes(), 0);

    // The budget is settable.
    recorder.set_max_budgeted_bytes(123);
    assert_eq!(recorder.max_budgeted_bytes(), 123);
    assert_eq!(recorder.max_texture_size(), 4096);
}

#[test]
fn recording_instantiates_lazy_proxies_and_deferred_targets() {
    use skia_rust_gpu::graphite::recording::LazyProxyData;

    let (mut recorder, shared_context) = make_recorder(MockCaps::default());
    let provider = recorder.priv_().resource_provider().clone();
    let caps = shared_context.caps.clone();

    // A deferred target: a fully lazy volatile proxy, fulfilled with the replay target's
    // texture.
    let info = rgba_info();
    let data = LazyProxyData::new(&*caps, ISize::new(8, 8), &info);
    let lazy_proxy = data.ref_lazy_proxy().unwrap();
    assert!(lazy_proxy.is_fully_lazy());
    assert!(lazy_proxy.is_volatile());

    let surface_texture = TextureProxy::make(
        4096,
        &mut provider.lock().unwrap(),
        ISize::new(8, 8),
        &info,
        Budgeted::No,
        "ReplayTarget",
    )
    .unwrap();
    assert!(surface_texture.is_instantiated());
    assert!(data.lazy_instantiate(
        &mut provider.lock().unwrap(),
        surface_texture.ref_texture().unwrap()
    ));
    assert!(lazy_proxy.is_instantiated());
    lazy_proxy.deinstantiate();

    // A mipmapped deferred target has known dimensions and is not fully lazy.
    let mipmapped_info = texture_info(TextureFormat::RGBA8, SampleCount::One, Mipmapped::Yes);
    let mipmapped = LazyProxyData::new(&*caps, ISize::new(8, 8), &mipmapped_info);
    assert!(!mipmapped.lazy_proxy().unwrap().is_fully_lazy());

    // Recording::prepare_resources collects the lazy proxies a draw task's pass would visit;
    // an empty recorder has none.
    let mut recording = recorder.snap().unwrap();
    assert!(!recording.priv_().has_volatile_lazy_proxies());
    assert!(!recording.priv_().has_non_volatile_lazy_proxies());
    assert_eq!(recording.priv_().num_volatile_promise_images(), 0);
    assert_eq!(recording.priv_().num_non_volatile_promise_images(), 0);
}

#[test]
fn recording_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<skia_rust_gpu::graphite::recording::Recording>();
}

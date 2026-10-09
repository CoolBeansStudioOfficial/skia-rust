// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: tests/graphite/BufferManagerTest.cpp, tests/graphite/UploadBufferManagerTest.cpp

//! `DrawBufferManager`, `UploadBufferManager` and `StaticBufferManager` against the mock back
//! end. Skia's own tests need a GPU `Context` (G9b/G11), so they are not the 1:1 manifest
//! ports; the scenarios and assertions below follow them. Where a test reads a buffer's
//! contents through `map()`, the port reads the bytes committed to the (mock) GPU buffer when
//! the buffer is transferred to a `Recording`.

mod support;

use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_gpu::gpu::gpu_types::Protected;
use skia_rust_gpu::graphite::buffer::{BindBufferInfo, Buffer};
use skia_rust_gpu::graphite::buffer_manager::{
    DrawBufferManagerOptions, StaticBufferBinding, StaticBufferHost, StaticBufferManager,
    StaticFinishResult,
};
use skia_rust_gpu::graphite::recorder::{RecorderOptions, RecorderOptionsPriv};
use skia_rust_gpu::graphite::resource::ResourceRef;
use skia_rust_gpu::graphite::resource_types::ClearBuffer;
use skia_rust_gpu::graphite::task::task_list::TaskList;
use skia_rust_gpu::graphite::task::{ReplayTargetData, Status, TaskRef};
use skia_rust_gpu::graphite::upload_buffer_manager::UploadBufferManager;
use support::{
    Call, MockCaps, MockCommandBuffer, MockContext, committed_bytes, make_recorder,
    make_recorder_with, shared_provider,
};

fn is_offset_aligned(offset: u32, alignment: usize) -> bool {
    0 == (offset as usize & (alignment - 1))
}

fn same_buffer(a: &BindBufferInfo, b: &BindBufferInfo) -> bool {
    match (&a.buffer, &b.buffer) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

// Port of: tests/graphite/BufferManagerTest.cpp#L25-L69 (chrome/m156)
#[test]
fn buffer_manager_gpu_only_buffer_test() {
    let (mut recorder, shared_context) = make_recorder(MockCaps::default());
    let priv_ = recorder.priv_();
    let mgr = priv_.draw_buffer_manager();

    // Allocate a series of GPU-only buffers. These buffers should not be mapped before and after
    // they get transferred to the recording.
    let mut ssbo = mgr.get_storage(10, ClearBuffer::No);
    let vertex = mgr.get_vertex_storage(10);
    let index = mgr.get_index_storage(10);
    let indirect = mgr.get_indirect_storage(10, ClearBuffer::No);

    for binding in [&ssbo, &vertex, &index, &indirect] {
        assert!(!binding.buffer.as_ref().unwrap().is_mapped());
    }

    // Ensure that the buffers' starting alignment matches the required storage buffer
    // alignment.
    let required_alignment = shared_context.caps.storage_alignment;
    for binding in [&ssbo, &vertex, &index, &indirect] {
        assert!(is_offset_aligned(binding.offset, required_alignment));
    }

    // Transfers the ownership of used buffers to a Recording.
    let mut recording = recorder.snap().expect("snap succeeds");

    // Ensure that the buffers are still unmapped.
    for binding in [&ssbo, &vertex, &index, &indirect] {
        assert!(!binding.buffer.as_ref().unwrap().is_mapped());
    }

    // Since these buffers never need their contents to be host-visible, no buffer transfer/copy
    // tasks should have been created for them.
    assert!(!recording.priv_().has_tasks());

    // Request a mapped ssbo followed by an unmapped one. The two buffers should be distinct.
    let priv_ = recorder.priv_();
    let mgr = priv_.draw_buffer_manager();
    let mapped_ssbo = mgr
        .get_mapped_storage_buffer(/*count=*/ 10, /*stride=*/ 1)
        .unwrap();
    ssbo = mgr.get_storage(10, ClearBuffer::No);
    assert!(!ssbo.buffer.as_ref().unwrap().is_mapped());
    assert!(!same_buffer(&ssbo, &mapped_ssbo.binding));
}

// Port of: tests/graphite/BufferManagerTest.cpp#L71-L113 (chrome/m156)
#[test]
fn buffer_manager_stale_allocator_test() {
    let (mut recorder, _shared_context) = make_recorder(MockCaps::default());

    // Keep a reference to the buffer to prevent reuse false positives.
    let raw_buffer: ResourceRef<Buffer>;
    {
        let priv_ = recorder.priv_();
        let dbm = priv_.draw_buffer_manager();
        let info = dbm.get_mapped_index_buffer(/*count=*/ 16).unwrap();
        assert!(info.allocator.is_valid());
        raw_buffer = info.binding.ref_buffer();

        // Force the buffer allocator to fail to simulate an allocation failure
        dbm.testing_only_on_failed_buffer();
        assert!(dbm.has_mapping_failed());

        // BufferSubAllocator::reset() is called on allocator destruction
    }

    // Recorder::snap() failure branch clears fMappingFailed and drops the failed Recording.
    let failed_recording = recorder.snap();
    assert!(failed_recording.is_none());

    // Purge everything in the cache by setting the max budget to 0 and freeing all resources
    let original_budget = recorder.max_budgeted_bytes();
    recorder.set_max_budgeted_bytes(0);
    recorder.free_gpu_resources();
    recorder.set_max_budgeted_bytes(original_budget); // probably unnecessary but safe

    // Get another allocator
    {
        let priv_ = recorder.priv_();
        let dbm = priv_.draw_buffer_manager();
        let stale = dbm.get_mapped_index_buffer(/*count=*/ 16).unwrap();
        assert!(stale.allocator.is_valid());

        // The buffer should not be the same as rawBuffer
        assert!(!Arc::ptr_eq(
            stale.binding.buffer.as_ref().unwrap(),
            raw_buffer.as_arc()
        ));
    }

    let success_recording = recorder.snap();
    assert!(success_recording.is_some());
}

#[test]
fn draw_buffer_manager_mapped_vertex_data_is_committed_on_snap() {
    let (mut recorder, _shared_context) = make_recorder(MockCaps::default());
    let binding;
    {
        let priv_ = recorder.priv_();
        let mgr = priv_.draw_buffer_manager();
        let mut info = mgr.get_mapped_vertex_buffer(4, 4, 0, 1).unwrap();
        assert!(info.binding.buffer.as_ref().unwrap().is_mapped());
        assert_eq!(info.binding.size, 16);
        info.writer()
            .write_bytes(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);

        // A second mapped subrange comes from the same buffer, after the first.
        let (mut writer, second) = info.allocator.get_mapped_subrange(2, 4, 1).unwrap();
        writer.write_bytes(&[0xAA; 8]);
        assert!(same_buffer(&second, &info.binding));
        assert_eq!(second.offset, 16);
        assert_eq!(second.size, 8);
        binding = info.binding.clone();
    }
    let buffer = binding.buffer.clone().unwrap();
    let mut recording = recorder.snap().unwrap();
    // The buffer was unmapped and its bytes committed.
    assert!(!buffer.is_mapped());
    let bytes = committed_bytes(&buffer).unwrap();
    assert_eq!(
        &bytes[..16],
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    assert_eq!(&bytes[16..24], &[0xAA; 8]);
    // The recording keeps the buffer alive and has no copy tasks.
    assert!(!recording.priv_().has_tasks());
}

#[test]
fn draw_buffer_manager_transfer_buffers_copy_through_upload_buffers() {
    let caps = MockCaps {
        draw_buffer_can_be_mapped: false,
        ..MockCaps::default()
    };
    let (mut recorder, _shared_context) = make_recorder(caps);
    let binding;
    {
        let priv_ = recorder.priv_();
        let mgr = priv_.draw_buffer_manager();
        let mut info = mgr.get_mapped_vertex_buffer(2, 8, 0, 1).unwrap();
        info.writer().write_bytes(&[7u8; 16]);
        binding = info.binding.clone();
    }
    let mut recording = recorder.snap().unwrap();
    // One copy task from the transfer buffer to the draw buffer.
    assert!(recording.priv_().has_tasks());
    assert_eq!(recording.priv_().task_list().size(), 1);

    // Run the recording's tasks on a command buffer: the copy covers the whole draw buffer.
    let (mut context, _) = MockContext::new(MockCaps::default());
    let mut command_buffer = MockCommandBuffer::default();
    assert!(recording.priv_().add_commands(
        &mut context,
        &mut command_buffer,
        None,
        IPoint::default(),
        IRect::default(),
    ));
    let buffer = binding.buffer.unwrap();
    assert!(command_buffer.calls.contains(&Call::CopyBufferToBuffer {
        src_offset: 0,
        dst_offset: 0,
        size: buffer.size(),
    }));
}

#[test]
fn draw_buffer_manager_buffers_grow_and_are_reused() {
    let options = RecorderOptions {
        recorder_options_priv: Some(RecorderOptionsPriv {
            dbm_options: Some(DrawBufferManagerOptions {
                vertex_buffer_min_size: 64,
                vertex_buffer_max_size: 256,
                ..DrawBufferManagerOptions::default()
            }),
        }),
        ..RecorderOptions::default()
    };
    let (recorder, shared_context) = make_recorder_with(MockCaps::default(), &options);
    let priv_ = recorder.priv_();
    let mgr = priv_.draw_buffer_manager();

    // The first buffer is the minimum block size.
    let first = mgr.get_mapped_vertex_buffer(1, 16, 0, 1).unwrap();
    assert_eq!(first.binding.buffer.as_ref().unwrap().size(), 64);
    let first_buffer = first.binding.buffer.clone().unwrap();
    drop(first);

    // The rest of the buffer is available to the next allocation, at a later offset.
    let second = mgr.get_mapped_vertex_buffer(1, 16, 0, 1).unwrap();
    assert!(Arc::ptr_eq(
        second.binding.buffer.as_ref().unwrap(),
        &first_buffer
    ));
    assert_eq!(second.binding.offset, 16);
    drop(second);

    // A request that does not fit makes a larger buffer (doubling, up to the maximum).
    let third = mgr.get_mapped_vertex_buffer(1, 128, 0, 1).unwrap();
    assert!(!Arc::ptr_eq(
        third.binding.buffer.as_ref().unwrap(),
        &first_buffer
    ));
    assert_eq!(third.binding.buffer.as_ref().unwrap().size(), 128);
    drop(third);
    let fourth = mgr.get_mapped_vertex_buffer(1, 300, 0, 1).unwrap();
    assert_eq!(fourth.binding.buffer.as_ref().unwrap().size(), 320);
    assert_eq!(
        shared_context
            .counts
            .buffers
            .load(std::sync::atomic::Ordering::Relaxed),
        3
    );

    // Requests that overflow 32 bits fail without breaking the manager.
    assert!(mgr.get_mapped_vertex_buffer(usize::MAX, 8, 0, 1).is_none());
    assert!(!mgr.has_mapping_failed());
}

#[test]
fn draw_buffer_manager_scratch_storage_is_reused_after_reset() {
    // A request of the maximum block size always makes a buffer of exactly that size; smaller
    // ones grow with every new buffer, so they would not find each other.
    const SIZE: usize = 1 << 20;
    let (recorder, shared_context) = make_recorder(MockCaps::default());
    let priv_ = recorder.priv_();
    let mgr = priv_.draw_buffer_manager();
    let mut first = mgr.get_scratch_storage(SIZE);
    assert!(first.is_valid());
    let first_binding = first.get_subrange(SIZE, 1, 1);
    // A second scratch request while the first is alive gets another buffer.
    let mut second = mgr.get_scratch_storage(SIZE);
    let second_binding = second.get_subrange(SIZE, 1, 1);
    assert!(!same_buffer(&first_binding, &second_binding));

    // Returning the first makes its buffer available to a new request.
    first.reset();
    assert!(!first.is_valid());
    let mut third = mgr.get_scratch_storage(SIZE);
    let third_binding = third.get_subrange(SIZE, 1, 1);
    assert!(same_buffer(&first_binding, &third_binding));
    assert_eq!(
        shared_context
            .counts
            .buffers
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
}

#[test]
fn draw_buffer_manager_clear_list_becomes_a_task() {
    let (mut recorder, _shared_context) = make_recorder(MockCaps::default());
    let cleared;
    {
        let priv_ = recorder.priv_();
        let mgr = priv_.draw_buffer_manager();
        cleared = mgr.get_storage(64, ClearBuffer::Yes);
        assert!(cleared.is_valid());
    }
    let mut recording = recorder.snap().unwrap();
    assert!(recording.priv_().has_tasks());
    let (mut context, _) = MockContext::new(MockCaps::default());
    let mut command_buffer = MockCommandBuffer::default();
    assert!(recording.priv_().add_commands(
        &mut context,
        &mut command_buffer,
        None,
        IPoint::default(),
        IRect::default(),
    ));
    assert!(command_buffer.calls.contains(&Call::ClearBuffer {
        offset: 0,
        size: cleared.buffer.unwrap().size(),
    }));
}

// Port of: tests/graphite/UploadBufferManagerTest.cpp#L19-L129 (chrome/m156)
#[test]
#[allow(clippy::too_many_lines)] // one test, as in Skia
fn upload_buffer_manager_test() {
    let (mut recorder, _shared_context) = make_recorder(MockCaps::default());
    let upload_manager = recorder.priv_().upload_buffer_manager().clone();

    // The test source data.
    let src: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

    // Test multiple small writes to a reused buffer.
    let sm_buffer_info0;
    {
        let mut manager = upload_manager.borrow_mut();
        let (mut writer, info) = manager.get_texture_upload_writer(10, 1).unwrap();
        writer.write(
            /*offset=*/ 0, &src, 4, /*dst_row_bytes=*/ 3, /*trim_row_bytes=*/ 3, 2,
        );
        writer.write(/*offset=*/ 6, &src, 4, 2, 2, 2);
        sm_buffer_info0 = info;
    }
    let sm_buffer_info1;
    {
        let mut manager = upload_manager.borrow_mut();
        let (mut writer, info) = manager.get_texture_upload_writer(4, 1).unwrap();
        writer.write(0, &src, 4, 2, 2, 2);
        sm_buffer_info1 = info;
    }
    assert!(same_buffer(&sm_buffer_info0, &sm_buffer_info1));
    assert_eq!(sm_buffer_info0.offset, 0);
    assert!(sm_buffer_info1.offset >= 10);

    // Test a large write, which should get its own dedicated buffer.
    let lg_buffer_info;
    {
        let mut manager = upload_manager.borrow_mut();
        let (mut writer, info) = manager
            .get_texture_upload_writer((64 << 10) + 1, 1)
            .unwrap();
        writer.write(0, &src, 4, 2, 2, 2);
        lg_buffer_info = info;
    }
    assert!(!same_buffer(&lg_buffer_info, &sm_buffer_info0));
    assert_eq!(lg_buffer_info.offset, 0);
    assert!(lg_buffer_info.buffer.as_ref().unwrap().is_mapped());

    // Test another small write after the large write.
    let sm_buffer_info2;
    {
        let mut manager = upload_manager.borrow_mut();
        let (mut writer, info) = manager.get_texture_upload_writer(2, 1).unwrap();
        writer.write(0, &src, 4, 2, 2, 1);
        sm_buffer_info2 = info;
    }
    assert!(same_buffer(&sm_buffer_info2, &sm_buffer_info0));
    assert!(sm_buffer_info2.offset >= 4 + sm_buffer_info1.offset);
    assert!(sm_buffer_info0.buffer.as_ref().unwrap().is_mapped());

    // Snap a Recording from the Recorder. This will transfer resources from the
    // UploadBufferManager to the Recording, committing the bytes written so far.
    let _recording = recorder.snap();

    // Skia reads the buffers through map() before the snap. Here the bytes are the ones
    // committed to the buffers by the snap.
    let lg_buffer = lg_buffer_info.buffer.as_ref().unwrap();
    assert!(!lg_buffer.is_mapped());
    let lg_buffer_map = committed_bytes(lg_buffer).unwrap();
    let expected_lg_buffer_map: [u8; 4] = [1, 2, 5, 6];
    assert_eq!(&lg_buffer_map[..4], &expected_lg_buffer_map);

    // Each section of written data could be offset and aligned by GPU-required rules, so we
    // can't easily validate the contents of the buffer in one go, and instead test at each of
    // the three reported offsets.
    let sm_buffer = sm_buffer_info0.buffer.as_ref().unwrap();
    assert!(!sm_buffer.is_mapped());
    let sm_buffer_map = committed_bytes(sm_buffer).unwrap();
    let expected_sm_buffer0: [u8; 10] = [1, 2, 3, 5, 6, 7, 1, 2, 5, 6];
    let expected_sm_buffer1: [u8; 4] = [1, 2, 5, 6];
    let expected_sm_buffer2: [u8; 2] = [1, 2];
    let at = |offset: u32, len: usize| &sm_buffer_map[offset as usize..offset as usize + len];
    assert_eq!(at(sm_buffer_info0.offset, 10), &expected_sm_buffer0);
    assert_eq!(at(sm_buffer_info1.offset, 4), &expected_sm_buffer1);
    assert_eq!(at(sm_buffer_info2.offset, 2), &expected_sm_buffer2);

    // Test writes with a required alignment.
    let al_buffer_info0;
    {
        let mut manager = upload_manager.borrow_mut();
        let (mut writer, info) = manager.get_texture_upload_writer(6, 4).unwrap();
        writer.write(0, &src, 4, 3, 3, 2);
        al_buffer_info0 = info;
    }
    let al_buffer_info1;
    {
        let mut manager = upload_manager.borrow_mut();
        let (mut writer, info) = manager.get_texture_upload_writer(2, 4).unwrap();
        writer.write(0, &src, 4, 2, 2, 1);
        al_buffer_info1 = info;
    }

    // Should not share a buffer with earlier small writes, since we've transferred previously-
    // allocated resources to the command buffer.
    assert!(!same_buffer(&al_buffer_info0, &sm_buffer_info0));
    assert!(same_buffer(&al_buffer_info0, &al_buffer_info1));
    assert_eq!(al_buffer_info0.offset, 0);
    assert_eq!(al_buffer_info1.offset, 8);

    // The bytes of both writes land in the buffer when it is transferred.
    assert!(al_buffer_info0.buffer.as_ref().unwrap().is_mapped());
    let _recording = recorder.snap();
    let al_buffer_map = committed_bytes(al_buffer_info0.buffer.as_ref().unwrap()).unwrap();
    let expected_al_buffer_map0: [u8; 6] = [1, 2, 3, 5, 6, 7];
    let expected_al_buffer_map1: [u8; 2] = [1, 2];
    assert_eq!(&al_buffer_map[..6], &expected_al_buffer_map0);
    assert_eq!(&al_buffer_map[8..10], &expected_al_buffer_map1);
}

#[test]
fn upload_buffer_manager_transfer_to_command_buffer_tracks_and_unmaps() {
    let (provider, _) = shared_provider();
    let caps = MockCaps::default();
    let mut manager = UploadBufferManager::new(provider, &caps);
    let (mut writer, info) = manager.get_texture_upload_writer(8, 1).unwrap();
    writer.write(0, &[9; 8], 8, 8, 8, 1);
    let buffer = info.buffer.unwrap();
    assert!(buffer.is_mapped());
    let mut command_buffer = MockCommandBuffer::default();
    manager.transfer_to_command_buffer(&mut command_buffer);
    assert!(!buffer.is_mapped());
    assert_eq!(command_buffer.tracked.len(), 1);
    assert_eq!(&committed_bytes(&buffer).unwrap()[..8], &[9; 8]);
}

struct MockHost {
    tasks: Vec<TaskRef>,
    static_buffers: Vec<ResourceRef<Buffer>>,
    refs_added: usize,
    fail_tasks: bool,
}

impl StaticBufferHost for MockHost {
    fn add_upload_buffer_manager_refs(&mut self, _upload_manager: &mut UploadBufferManager) {
        self.refs_added += 1;
    }

    fn add_task(&mut self, task: &TaskRef, is_protected: Protected) -> bool {
        assert_eq!(is_protected, Protected::No);
        self.tasks.push(task.clone());
        !self.fail_tasks
    }

    fn add_static_resource(&mut self, buffer: ResourceRef<Buffer>) {
        self.static_buffers.push(buffer);
    }
}

#[test]
fn static_buffer_manager_packs_vertex_and_index_data() {
    let (provider, _) = shared_provider();
    let caps = MockCaps::default();
    let mut manager = StaticBufferManager::new(provider, &caps);

    let vertex_binding = StaticBufferBinding::new();
    {
        let mut writer = manager.get_vertex_writer(3, 8, &vertex_binding).unwrap();
        writer.put(&[1u8; 24]);
    }
    let index_binding = StaticBufferBinding::new();
    {
        let mut writer = manager.get_index_writer(6, &index_binding).unwrap();
        writer.put(&[2u8; 6]);
    }
    // Nothing is bound until finalize().
    assert!(!vertex_binding.get().is_valid());

    let mut host = MockHost {
        tasks: Vec::new(),
        static_buffers: Vec::new(),
        refs_added: 0,
        fail_tasks: false,
    };
    assert_eq!(manager.finalize(&mut host), StaticFinishResult::Success);
    assert_eq!(host.refs_added, 1);
    assert_eq!(host.tasks.len(), 2);
    assert_eq!(host.static_buffers.len(), 2);

    let vertex = vertex_binding.get();
    let index = index_binding.get();
    assert!(vertex.is_valid() && index.is_valid());
    assert!(!same_buffer(&vertex, &index));
    assert_eq!(vertex.offset, 0);
    // The vertex data was padded to a count of 4.
    assert!(vertex.size >= 24);
    assert_eq!(index.offset, 0);

    // The copy tasks copy from the transfer buffers to the static buffers.
    let mut list = TaskList::new();
    for task in &host.tasks {
        list.add(task.clone());
    }
    let (mut context, _) = MockContext::new(MockCaps::default());
    let mut command_buffer = MockCommandBuffer::default();
    let status = list.add_commands(
        &mut context,
        &mut command_buffer,
        &ReplayTargetData::default(),
    );
    assert_eq!(status, Status::Success);
    assert_eq!(command_buffer.calls.len(), 2);

    // A second finalize has nothing to do.
    assert_eq!(manager.finalize(&mut host), StaticFinishResult::NoWork);
}

#[test]
fn static_buffer_manager_reports_failures() {
    let (provider, _) = shared_provider();
    let caps = MockCaps::default();
    let mut manager = StaticBufferManager::new(provider, &caps);
    let binding = StaticBufferBinding::new();
    let _writer = manager.get_index_writer(4, &binding).unwrap();
    let mut host = MockHost {
        tasks: Vec::new(),
        static_buffers: Vec::new(),
        refs_added: 0,
        fail_tasks: true,
    };
    assert_eq!(manager.finalize(&mut host), StaticFinishResult::Failure);

    // Requests too large for 32 bits fail and zero the binding.
    let mut manager = StaticBufferManager::new(shared_provider().0, &caps);
    assert!(manager.get_index_writer(usize::MAX / 2, &binding).is_none());
    assert!(!binding.get().is_valid());
}

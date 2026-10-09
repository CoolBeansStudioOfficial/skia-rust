// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/BufferManagerTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_gpu::graphite::buffer::{BindBufferInfo, Buffer};
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::resource::Resource;
use skia_rust_gpu::graphite::resource_types::ClearBuffer;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// `is_offset_aligned` of the C++ test.
fn is_offset_aligned(offset: usize, alignment: usize) -> bool {
    0 == (offset & (alignment - 1))
}

// `binding.fBuffer == buffer` for a possibly-null `fBuffer`.
fn buffer_is(binding: &BindBufferInfo, buffer: &Arc<Resource<Buffer>>) -> bool {
    binding
        .buffer
        .as_ref()
        .is_some_and(|b| Arc::ptr_eq(b, buffer))
}

// Port of: tests/graphite/BufferManagerTest.cpp#L29-L71 (chrome/m156)
def_graphite_test_for_all_contexts!(BufferManagerGpuOnlyBufferTest, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let priv_ = recorder.priv_();
    let mgr = priv_.draw_buffer_manager();

    // Allocate a series of GPU-only buffers. These buffers should not be mapped before and after
    // they get transferred to the recording.
    let ssbo = mgr.get_storage(10, ClearBuffer::No);
    let vertex = mgr.get_vertex_storage(10);
    let index = mgr.get_index_storage(10);
    let indirect = mgr.get_indirect_storage(10, ClearBuffer::No);

    let is_mapped = |info: &BindBufferInfo| info.buffer.as_ref().is_some_and(|b| b.is_mapped());
    reporter_assert!(reporter, !is_mapped(&ssbo));
    reporter_assert!(reporter, !is_mapped(&vertex));
    reporter_assert!(reporter, !is_mapped(&index));
    reporter_assert!(reporter, !is_mapped(&indirect));

    // Ensure that the buffers' starting alignment matches the required storage buffer alignment.
    let required_alignment = ContextPriv::caps(context).required_storage_buffer_alignment();
    reporter_assert!(
        reporter,
        is_offset_aligned(ssbo.offset as usize, required_alignment)
    );
    reporter_assert!(
        reporter,
        is_offset_aligned(vertex.offset as usize, required_alignment)
    );
    reporter_assert!(
        reporter,
        is_offset_aligned(index.offset as usize, required_alignment)
    );
    reporter_assert!(
        reporter,
        is_offset_aligned(indirect.offset as usize, required_alignment)
    );

    // Transfers the ownership of used buffers to a Recording.
    let recording = recorder.snap();

    // Ensure that the buffers are still unmapped.
    reporter_assert!(reporter, !is_mapped(&ssbo));
    reporter_assert!(reporter, !is_mapped(&vertex));
    reporter_assert!(reporter, !is_mapped(&index));
    reporter_assert!(reporter, !is_mapped(&indirect));

    // Since these buffers never need their contents to be host-visible, no buffer transfer/copy
    // tasks should have been created for them.
    let mut recording = recording.expect("snap succeeds");
    reporter_assert!(reporter, !recording.priv_().has_tasks());
    drop(recording);

    // Request a mapped ssbo followed by an unmapped one. The two buffers should be distinct.
    let priv_ = recorder.priv_();
    let mgr = priv_.draw_buffer_manager();
    let mapped_ssbo = mgr
        .get_mapped_storage_buffer(/*count=*/ 10, /*stride=*/ 1)
        .expect("a mapped storage buffer");
    let ssbo = mgr.get_storage(10, ClearBuffer::No);
    reporter_assert!(reporter, !is_mapped(&ssbo));
    let mapped_buffer = mapped_ssbo.binding.buffer.as_ref().expect("a buffer");
    reporter_assert!(reporter, !buffer_is(&ssbo, mapped_buffer));
});

// Port of: tests/graphite/BufferManagerTest.cpp#L73-L112 (chrome/m156)
def_graphite_test_for_all_contexts!(BufferManagerStaleAllocatorTest, |reporter, context| {
    let mut recorder = context.make_recorder(None);

    // Keep a reference to the buffer to prevent reuse false positives.
    let raw_buffer = {
        let priv_ = recorder.priv_();
        let dbm = priv_.draw_buffer_manager();
        let mapped = dbm
            .get_mapped_index_buffer(/*count=*/ 16)
            .expect("a mapped index buffer");
        reporter_assert!(reporter, mapped.allocator.is_valid());
        let raw_buffer = mapped.binding.buffer.clone();
        reporter_assert!(reporter, raw_buffer.is_some());

        // Force the buffer allocator to fail to simulate an allocation failure
        dbm.testing_only_on_failed_buffer();
        reporter_assert!(reporter, dbm.has_mapping_failed());

        // BufferSubAllocator::reset() is called on allocator destruction
        raw_buffer
    };

    // Recorder::snap() failure branch clears fMappingFailed and drops the failed Recording.
    let failed_recording = recorder.snap();
    reporter_assert!(reporter, failed_recording.is_none());

    // Purge everything in the cache by setting the max budget to 0 and freeing all resources
    let original_budget = recorder.max_budgeted_bytes();
    recorder.set_max_budgeted_bytes(0);
    recorder.free_gpu_resources();
    recorder.set_max_budgeted_bytes(original_budget); // probably unnecessary but safe

    // Get another allocator
    let priv_ = recorder.priv_();
    let dbm = priv_.draw_buffer_manager();
    let stale = dbm
        .get_mapped_index_buffer(/*count=*/ 16)
        .expect("a mapped index buffer");
    reporter_assert!(reporter, stale.allocator.is_valid());

    // The buffer should not be the same as rawBuffer
    let stale_buffer = stale.binding.buffer.as_ref().expect("a buffer");
    let raw_buffer = raw_buffer.expect("a buffer");
    reporter_assert!(reporter, !Arc::ptr_eq(stale_buffer, &raw_buffer));

    let success_recording = recorder.snap();
    reporter_assert!(reporter, success_recording.is_some());
});

// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/UploadBufferManagerTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// Port of: tests/graphite/UploadBufferManagerTest.cpp#L19-L126 (chrome/m156)
//
// The C++ test reads the written bytes through `Buffer::map()`, which returns the mapped pointer
// of a buffer that is already mapped. The port's `Buffer::map()` hands the staging block out once,
// to the `UploadBufferManager`, so the bytes are read with `UploadBufferManager::mapped_data()`.
// The writers borrow the manager, so each is finished before the next one is made, as in the C++.
def_graphite_test_for_all_contexts!(UploadBufferManagerTest, |reporter, context| {
    let mut recorder = context.make_recorder(None);
    let buffer_manager = recorder.priv_().upload_buffer_manager().clone();

    // The test source data.
    let src: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

    // Test multiple small writes to a reused buffer.
    let sm_buffer_info0 = {
        let mut manager = buffer_manager.borrow_mut();
        let (mut sm_writer0, sm_buffer_info0) = manager.get_texture_upload_writer(10, 1).unwrap();
        sm_writer0.write(
            /* offset= */ 0, &src, /* src_row_bytes= */ 4, /* dst_row_bytes= */ 3,
            /* trim_row_bytes= */ 3, /* row_count= */ 2,
        );
        sm_writer0.write(
            /* offset= */ 6, &src, /* src_row_bytes= */ 4, /* dst_row_bytes= */ 2,
            /* trim_row_bytes= */ 2, /* row_count= */ 2,
        );
        sm_buffer_info0
    };

    let sm_buffer_info1 = {
        let mut manager = buffer_manager.borrow_mut();
        let (mut sm_writer1, sm_buffer_info1) = manager.get_texture_upload_writer(4, 1).unwrap();
        sm_writer1.write(
            /* offset= */ 0, &src, /* src_row_bytes= */ 4, /* dst_row_bytes= */ 2,
            /* trim_row_bytes= */ 2, /* row_count= */ 2,
        );
        sm_buffer_info1
    };

    let same_buffer = |a: &skia_rust_gpu::graphite::buffer::BindBufferInfo,
                       b: &skia_rust_gpu::graphite::buffer::BindBufferInfo| {
        match (&a.buffer, &b.buffer) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    };
    reporter_assert!(reporter, same_buffer(&sm_buffer_info0, &sm_buffer_info1));
    reporter_assert!(reporter, sm_buffer_info0.offset == 0);
    reporter_assert!(reporter, sm_buffer_info1.offset >= 10);

    // Test a large write, which should get its own dedicated buffer.
    let lg_buffer_info = {
        let mut manager = buffer_manager.borrow_mut();
        let (mut lg_writer, lg_buffer_info) = manager
            .get_texture_upload_writer((64 << 10) + 1, 1)
            .unwrap();
        lg_writer.write(
            /* offset= */ 0, &src, /* src_row_bytes= */ 4, /* dst_row_bytes= */ 2,
            /* trim_row_bytes= */ 2, /* row_count= */ 2,
        );
        lg_buffer_info
    };

    reporter_assert!(reporter, !same_buffer(&lg_buffer_info, &sm_buffer_info0));
    reporter_assert!(reporter, lg_buffer_info.offset == 0);
    let lg_buffer = lg_buffer_info.buffer.clone().unwrap();
    reporter_assert!(reporter, lg_buffer.is_mapped());
    {
        let manager = buffer_manager.borrow();
        let lg_buffer_map = manager.mapped_data(&lg_buffer).unwrap();
        let expected_lg_buffer_map: [u8; 4] = [1, 2, 5, 6];
        reporter_assert!(
            reporter,
            lg_buffer_map[..expected_lg_buffer_map.len()] == expected_lg_buffer_map
        );
    }

    // Test another small write after the large write.
    let sm_buffer_info2 = {
        let mut manager = buffer_manager.borrow_mut();
        let (mut sm_writer2, sm_buffer_info2) = manager.get_texture_upload_writer(2, 1).unwrap();
        sm_writer2.write(
            /* offset= */ 0, &src, /* src_row_bytes= */ 4, /* dst_row_bytes= */ 2,
            /* trim_row_bytes= */ 2, /* row_count= */ 1,
        );
        sm_buffer_info2
    };

    reporter_assert!(reporter, same_buffer(&sm_buffer_info2, &sm_buffer_info0));
    reporter_assert!(
        reporter,
        sm_buffer_info2.offset >= 4 + sm_buffer_info1.offset
    );

    let sm_buffer = sm_buffer_info0.buffer.clone().unwrap();
    reporter_assert!(reporter, sm_buffer.is_mapped());
    {
        let manager = buffer_manager.borrow();
        let sm_buffer_map = manager.mapped_data(&sm_buffer).unwrap();
        // Each section of written data could be offset and aligned by GPU-required rules, so we
        // can't easily validate the contents of the buffer in one go, and instead test at each of
        // the three reported offsets.
        let expected_sm_buffer0: [u8; 10] = [1, 2, 3, 5, 6, 7, 1, 2, 5, 6];
        let expected_sm_buffer1: [u8; 4] = [1, 2, 5, 6];
        let expected_sm_buffer2: [u8; 2] = [1, 2];
        let at = |offset: u32, len: usize| {
            let offset = offset as usize;
            &sm_buffer_map[offset..offset + len]
        };
        reporter_assert!(
            reporter,
            at(sm_buffer_info0.offset, expected_sm_buffer0.len()) == expected_sm_buffer0
        );
        reporter_assert!(
            reporter,
            at(sm_buffer_info1.offset, expected_sm_buffer1.len()) == expected_sm_buffer1
        );
        // (The C++ compares `sizeof(expectedSmBuffer2)` bytes against `expectedSmBuffer1`.)
        reporter_assert!(
            reporter,
            at(sm_buffer_info2.offset, expected_sm_buffer2.len())
                == &expected_sm_buffer1[..expected_sm_buffer2.len()]
        );
    }

    // Snap a Recording from the Recorder. This will transfer resources from the
    // UploadBufferManager to the Recording.
    let recording = recorder.snap();
    reporter_assert!(reporter, recording.is_some());

    // Test writes with a required alignment.
    let al_buffer_info0 = {
        let mut manager = buffer_manager.borrow_mut();
        let (mut al_writer0, al_buffer_info0) = manager.get_texture_upload_writer(6, 4).unwrap();
        al_writer0.write(
            /* offset= */ 0, &src, /* src_row_bytes= */ 4, /* dst_row_bytes= */ 3,
            /* trim_row_bytes= */ 3, /* row_count= */ 2,
        );
        al_buffer_info0
    };

    let al_buffer_info1 = {
        let mut manager = buffer_manager.borrow_mut();
        let (mut al_writer1, al_buffer_info1) = manager.get_texture_upload_writer(2, 4).unwrap();
        al_writer1.write(
            /* offset= */ 0, &src, /* src_row_bytes= */ 4, /* dst_row_bytes= */ 2,
            /* trim_row_bytes= */ 2, /* row_count= */ 1,
        );
        al_buffer_info1
    };

    // Should not share a buffer with earlier small writes, since we've transferred previously-
    // allocated resources to the command buffer.
    reporter_assert!(reporter, !same_buffer(&al_buffer_info0, &sm_buffer_info0));
    reporter_assert!(reporter, same_buffer(&al_buffer_info0, &al_buffer_info1));
    reporter_assert!(reporter, al_buffer_info0.offset == 0);
    reporter_assert!(reporter, al_buffer_info1.offset == 8);

    // From alWriter0.
    let expected_al_buffer_map0: [u8; 6] = [1, 2, 3, 5, 6, 7];
    // From alWriter1.
    let expected_al_buffer_map1: [u8; 2] = [1, 2];

    let al_buffer = al_buffer_info0.buffer.clone().unwrap();
    reporter_assert!(reporter, al_buffer.is_mapped());
    {
        let manager = buffer_manager.borrow();
        let al_buffer_map = manager.mapped_data(&al_buffer).unwrap();
        reporter_assert!(
            reporter,
            al_buffer_map[..expected_al_buffer_map0.len()] == expected_al_buffer_map0
        );

        let al_buffer_map = &al_buffer_map[8..];
        reporter_assert!(
            reporter,
            al_buffer_map[..expected_al_buffer_map1.len()] == expected_al_buffer_map1
        );
    }
});

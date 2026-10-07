// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MallocPixelRefTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::malloc_pixel_ref;
use skia_rust_core::pixel_ref::PixelRef;
use skia_rust_core::pixel_ref_priv::make_pixel_ref_with_proc;

use crate::{def_test, reporter_assert};

// Port of: tests/MallocPixelRefTest.cpp#L18-L20 (chrome/m156)
// `delete[] static_cast<uint8_t*>(ptr)`: the pixel ref hands the bytes back; dropping them is
// the delete.
fn delete_uint8_proc(pixels: Vec<u8>) {
    drop(pixels);
}

// Port of: tests/MallocPixelRefTest.cpp#L22-L24 (chrome/m156)
// `set_to_one_proc(void*, void* context)`: the context is the captured value.
fn set_to_one_proc(x: Arc<AtomicI32>) -> Box<dyn FnOnce(Vec<u8>) + Send> {
    Box::new(move |_pixels| x.store(1, Ordering::SeqCst))
}

// Port of: tests/MallocPixelRefTest.cpp#L28-L108 (chrome/m156)
def_test!(MallocPixelRef, |reporter| {
    reporter_assert!(reporter, true);
    let info = ImageInfo::new_n32_premul((10, 13), None);
    {
        let pr = malloc_pixel_ref::make_allocate(&info, info.min_row_bytes() - 1);
        // rowbytes too small.
        reporter_assert!(reporter, pr.is_none());
    }
    {
        let row_bytes = info.min_row_bytes() - 1;
        let size = info.compute_byte_size(row_bytes);
        let data = Data::new_uninitialized(size);
        let pr = malloc_pixel_ref::make_with_data(&info, row_bytes, data);
        // rowbytes too small.
        reporter_assert!(reporter, pr.is_none());
    }
    {
        let row_bytes = info.min_row_bytes() + info.bytes_per_pixel();
        let size = info.compute_byte_size(row_bytes) - 1;
        let data = Data::new_uninitialized(size);
        let pr = malloc_pixel_ref::make_with_data(&info, row_bytes, data);
        // data too small.
        reporter_assert!(reporter, pr.is_none());
    }
    let row_bytes = info.min_row_bytes() + info.bytes_per_pixel();
    let size = info.compute_byte_size(row_bytes) + 9;
    {
        // SkAutoMalloc memory(size);
        let memory = vec![0u8; size];
        let memory_ptr = memory.as_ptr();
        let pr = PixelRef::new(info.width(), info.height(), memory, row_bytes);
        // skia-rust: not expressible in Rust: `pr.get() != nullptr` (a PixelRef is never null).
        reporter_assert!(reporter, memory_ptr == pr.pixels().as_ptr());
    }
    {
        let pr = malloc_pixel_ref::make_allocate(&info, row_bytes);
        reporter_assert!(reporter, pr.is_some());
        // `REPORTER_ASSERT(reporter, pr->pixels())`: the pixels exist.
        reporter_assert!(reporter, pr.is_some_and(|pr| pr.pixels().len() == size - 9));
    }
    {
        let addr = vec![0u8; size];
        let addr_ptr = addr.as_ptr();
        let pr = make_pixel_ref_with_proc(
            info.width(),
            info.height(),
            row_bytes,
            addr,
            Some(Box::new(delete_uint8_proc)),
        );
        // skia-rust: not expressible in Rust: `pr.get() != nullptr` (a PixelRef is never null).
        reporter_assert!(reporter, addr_ptr == pr.pixels().as_ptr());
    }
    {
        let x = Arc::new(AtomicI32::new(0));
        // SkAutoMalloc memory(size);
        let memory = vec![0u8; size];
        let memory_ptr = memory.as_ptr();
        let pr = make_pixel_ref_with_proc(
            info.width(),
            info.height(),
            row_bytes,
            memory,
            Some(set_to_one_proc(x.clone())),
        );
        // skia-rust: not expressible in Rust: `pr.get() != nullptr` (a PixelRef is never null).
        reporter_assert!(reporter, memory_ptr == pr.pixels().as_ptr());
        reporter_assert!(reporter, 0 == x.load(Ordering::SeqCst));
        drop(pr); // pr.reset(nullptr)
        // make sure that set_to_one_proc was called.
        reporter_assert!(reporter, 1 == x.load(Ordering::SeqCst));
    }
    {
        let addr = vec![0u8; size];
        let addr_ptr = addr.as_ptr();
        // `REPORTER_ASSERT(reporter, addr != nullptr)`: a Vec's buffer is never null.
        reporter_assert!(reporter, !addr_ptr.is_null());
        let pr = make_pixel_ref_with_proc(
            info.width(),
            info.height(),
            row_bytes,
            addr,
            Some(Box::new(delete_uint8_proc)),
        );
        reporter_assert!(reporter, addr_ptr == pr.pixels().as_ptr());
    }
    {
        let data = Data::new_uninitialized(size);
        // `SkData* dataPtr = data.get()`: the pointer of the data, to compare with the pixels.
        let data_ptr = data.as_bytes().as_ptr();
        reporter_assert!(reporter, data.unique());
        let pr = malloc_pixel_ref::make_with_data(&info, row_bytes, data.clone()).unwrap();
        reporter_assert!(reporter, !data.unique());
        drop(data); // data.reset(nullptr)
        // skia-rust: not expressible in Rust: `REPORTER_ASSERT(reporter, dataPtr->unique())`
        // after the last other reference is dropped: C++ keeps using the raw `SkData*`, Rust has
        // no handle to the pixel ref's data to ask.
        reporter_assert!(reporter, data_ptr == pr.pixels().as_ptr());
    }
});

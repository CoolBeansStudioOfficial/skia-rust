// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PixelRefTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::id_change_listener::IdChangeListener;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::malloc_pixel_ref;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/PixelRefTest.cpp#L18-L21 (chrome/m156)
// `decrement_counter_proc(void* pixels, void* ctx)`: the context is the captured counter.
fn decrement_counter_proc(counter: Arc<AtomicI32>) -> Box<dyn FnOnce(Vec<u8>) + Send> {
    Box::new(move |_pixels| {
        counter.fetch_sub(1, Ordering::SeqCst);
    })
}

// Port of: tests/PixelRefTest.cpp#L23-L49 (chrome/m156)
fn test_dont_leak_install(reporter: &mut Reporter) {
    let mut bm = Bitmap::new();

    let mut info = ImageInfo::new_n32_premul((0, 0), None);
    let release_counter = Arc::new(AtomicI32::new(1));
    let mut success = bm.install_pixels_with_proc(
        &info,
        None,
        0,
        Some(decrement_counter_proc(release_counter.clone())),
    );
    reporter_assert!(reporter, success);
    bm.reset();
    reporter_assert!(reporter, 0 == release_counter.load(Ordering::SeqCst));

    info = ImageInfo::new_n32_premul((10, 10), None);
    release_counter.store(1, Ordering::SeqCst);
    success = bm.install_pixels_with_proc(
        &info,
        None,
        0,
        Some(decrement_counter_proc(release_counter.clone())),
    );
    reporter_assert!(reporter, success);
    bm.reset();
    reporter_assert!(reporter, 0 == release_counter.load(Ordering::SeqCst));

    info = ImageInfo::new_n32_premul((-10, -10), None);
    release_counter.store(1, Ordering::SeqCst);
    success = bm.install_pixels_with_proc(
        &info,
        None,
        0,
        Some(decrement_counter_proc(release_counter.clone())),
    );
    reporter_assert!(reporter, !success);
    bm.reset();
    reporter_assert!(reporter, 0 == release_counter.load(Ordering::SeqCst));
}

// Port of: tests/PixelRefTest.cpp#L51-L63 (chrome/m156)
fn test_install(reporter: &mut Reporter) {
    let mut info = ImageInfo::new_n32_premul((0, 0), None);
    let mut bm = Bitmap::new();
    // make sure we don't assert on an empty install
    let mut success = bm.install_pixels(&info, None, 0);
    reporter_assert!(reporter, success);

    // no pixels should be the same as setInfo()
    info = ImageInfo::new_n32_premul((10, 10), None);
    success = bm.install_pixels(&info, None, 0);
    reporter_assert!(reporter, success);
}

// Port of: tests/PixelRefTest.cpp#L65-L72 (chrome/m156)
// `TestListener`: a listener that counts how often it was called.
fn test_listener(count: &Arc<AtomicI32>) -> Arc<IdChangeListener> {
    let count = count.clone();
    IdChangeListener::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
    })
}

// Port of: tests/PixelRefTest.cpp#L72-L126 (chrome/m156)
def_test!(PixelRef_GenIDChange, |r| {
    let info = ImageInfo::new_n32_premul((10, 10), None);

    let pixel_ref = malloc_pixel_ref::make_allocate(&info, 0).unwrap();

    // Register a listener.
    let count = Arc::new(AtomicI32::new(0));
    pixel_ref.add_gen_id_change_listener(Some(test_listener(&count)));
    reporter_assert!(r, 0 == count.load(Ordering::SeqCst));

    // No one has looked at our pixelRef's generation ID, so invalidating it doesn't make sense.
    // (An SkPixelRef tree falls in the forest but there's nobody around to hear it.  Do we care?)
    pixel_ref.notify_pixels_changed();
    reporter_assert!(r, 0 == count.load(Ordering::SeqCst));

    // Force the generation ID to be calculated.
    reporter_assert!(r, 0 != pixel_ref.generation_id());

    // Our listener was dropped in the first call to notifyPixelsChanged().  This is a no-op.
    pixel_ref.notify_pixels_changed();
    reporter_assert!(r, 0 == count.load(Ordering::SeqCst));

    // Force the generation ID to be recalculated, then add a listener.
    reporter_assert!(r, 0 != pixel_ref.generation_id());
    pixel_ref.add_gen_id_change_listener(Some(test_listener(&count)));
    pixel_ref.notify_pixels_changed();
    reporter_assert!(r, 1 == count.load(Ordering::SeqCst));

    // Check that asking for deregistration causes the listener to not be called.
    reporter_assert!(r, 0 != pixel_ref.generation_id());
    let mut listener = test_listener(&count);
    pixel_ref.add_gen_id_change_listener(Some(listener.clone()));
    reporter_assert!(r, 1 == count.load(Ordering::SeqCst));
    listener.mark_should_deregister();
    pixel_ref.notify_pixels_changed();
    reporter_assert!(r, 1 == count.load(Ordering::SeqCst));

    // Check that we use deregistration to prevent unbounded growth.
    reporter_assert!(r, 0 != pixel_ref.generation_id());
    listener = test_listener(&count);
    pixel_ref.add_gen_id_change_listener(Some(listener.clone()));
    reporter_assert!(r, 1 == count.load(Ordering::SeqCst));
    listener.mark_should_deregister();
    // Add second listener. Should deregister first listener.
    pixel_ref.add_gen_id_change_listener(Some(test_listener(&count)));
    // `listener->unique()`
    reporter_assert!(r, Arc::strong_count(&listener) == 1);

    // Quick check that nullptr is safe.
    reporter_assert!(r, 0 != pixel_ref.generation_id());
    pixel_ref.add_gen_id_change_listener(None);
    pixel_ref.notify_pixels_changed();

    test_install(r);
    test_dont_leak_install(r);
});

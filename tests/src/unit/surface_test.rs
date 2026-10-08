// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SurfaceTest.cpp (chrome/m156)
//
// Not ported (the raster-only tests that need types skia-rust does not have yet):
// * `SurfaceCopyOnWrite`: `drawString` (text, Phase 3).
// * `SurfaceGetTexture`: GPU-only assertions.
// * `surface_image_unity`: builds an `SkPixmap` over a one-byte address with a huge row-bytes,
//   which a safe `Pixmap` cannot express.
// * `Surface_null`: `SkSurfaces::Null` and `makeImageSnapshot() == nullptr`.
// * `OverdrawSurface_Raster`: `SkOverdrawCanvas`/`SkOverdrawColorFilter` and `SkImage`.
// The Ganesh/Graphite tests are excluded.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::ContentChangeMode;
use skia_rust_core::color::{Color, pre_multiply_color};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, reporter_assert};

// Port of: tests/SurfaceTest.cpp#L85-L93 (chrome/m156)
fn create_surface(at: AlphaType, requested_info: Option<&mut ImageInfo>) -> Surface<'static> {
    let info = ImageInfo::new_n32((10, 10), at, None);
    if let Some(requested_info) = requested_info {
        *requested_info = info.clone();
    }
    surfaces::raster(&info, None, None).expect("surface")
}

// Port of: tests/SurfaceTest.cpp#L94-L104 (chrome/m156)
// `release_direct_surface_storage` is `storage` going out of scope in the caller; the surface
// works on a copy of the bytes (docs/design/pixels.md).
fn create_direct_surface<'a>(
    storage: &'a mut Vec<u8>,
    at: AlphaType,
    requested_info: Option<&mut ImageInfo>,
) -> Surface<'a> {
    let info = ImageInfo::new_n32((10, 10), at, None);
    if let Some(requested_info) = requested_info {
        *requested_info = info.clone();
    }
    let row_bytes = info.min_row_bytes();
    *storage = vec![0_u8; info.compute_byte_size(row_bytes)];
    surfaces::wrap_pixels(&info, storage, row_bytes, None).expect("surface")
}

// Port of: tests/SurfaceTest.cpp#L109-L113 (chrome/m156)
def_tier_test!(SurfaceEmpty, |reporter| {
    let info = ImageInfo::new((0, 0), ColorType::N32, AlphaType::Premul, None);
    reporter_assert!(reporter, surfaces::raster(&info, None, None).is_none());
    reporter_assert!(
        reporter,
        surfaces::wrap_pixels(&info, &mut [], 0_usize, None).is_none()
    );
});

// Port of: tests/SurfaceTest.cpp#L280-L306 (chrome/m156)
fn test_canvas_peek(
    reporter: &mut Reporter,
    surface: &mut Surface<'_>,
    request_info: &ImageInfo,
    expect_peek_success: bool,
) {
    let color = Color::RED;
    let pmcolor = pre_multiply_color(color);
    surface.canvas().clear(color);

    // `pmap`: the address, info, row bytes and first pixel of the canvas's peek.
    let pmap = surface.canvas().peek_pixels().map(|peeked| {
        let pm = peeked.pixmap();
        (
            pm.addr().map(<[u8]>::as_ptr),
            pm.info().clone(),
            pm.row_bytes(),
            pm.addr32(0, 0),
        )
    });
    let success = pmap.is_some();
    reporter_assert!(reporter, expect_peek_success == success);

    let pmap2 = surface.peek_pixels().map(|peeked| {
        let pm = peeked.pixmap();
        (
            pm.addr().map(<[u8]>::as_ptr),
            pm.info().clone(),
            pm.row_bytes(),
        )
    });
    let addr2 = pmap2.as_ref().and_then(|p| p.0);

    if let Some((addr, info, row_bytes, pixel)) = pmap {
        let pmap2 = pmap2.as_ref();
        reporter_assert!(reporter, *request_info == info);
        reporter_assert!(reporter, request_info.min_row_bytes() <= row_bytes);
        reporter_assert!(reporter, pmcolor == pixel);

        reporter_assert!(reporter, addr == pmap2.and_then(|p| p.0));
        reporter_assert!(reporter, Some(&info) == pmap2.map(|p| &p.1));
        reporter_assert!(reporter, Some(row_bytes) == pmap2.map(|p| p.2));
    } else {
        reporter_assert!(reporter, addr2.is_none());
    }
}

// Port of: tests/SurfaceTest.cpp#L308-L314 (chrome/m156)
def_tier_test!(SurfaceCanvasPeek, |reporter| {
    for direct in [false, true] {
        let mut request_info = ImageInfo::new_unknown(None);
        let mut storage = Vec::new();
        let mut surface = if direct {
            create_direct_surface(&mut storage, AlphaType::Premul, Some(&mut request_info))
        } else {
            create_surface(AlphaType::Premul, Some(&mut request_info))
        };
        test_canvas_peek(reporter, &mut surface, &request_info, true);
    }
});

// Port of: tests/SurfaceTest.cpp#L796-L806 (chrome/m156)
def_tier_test!(surface_raster_zeroinitialized, |reporter| {
    let mut s = surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None)
        .expect("surface");
    let peeked = s.peek_pixels();
    reporter_assert!(reporter, peeked.is_some());
    let Some(peeked) = peeked else {
        return;
    };
    let pixmap = peeked.pixmap();

    for i in 0..pixmap.info().width() {
        for j in 0..pixmap.info().height() {
            reporter_assert!(reporter, pixmap.addr32(i, j) == 0);
        }
    }
});

// Port of: tests/SurfaceTest.cpp#L328-L338 (chrome/m156)
fn test_snapshot_alphatype(
    reporter: &mut Reporter,
    surface: &mut Surface<'_>,
    expected_alpha_type: AlphaType,
) {
    let image = surface.image_snapshot();
    reporter_assert!(reporter, image.is_some());
    if let Some(image) = image {
        reporter_assert!(reporter, image.alpha_type() == expected_alpha_type);
    }
}

// Port of: tests/SurfaceTest.cpp#L339-L346 (chrome/m156)
def_tier_test!(SurfaceSnapshotAlphaType, |reporter| {
    for direct in [false, true] {
        for at in [AlphaType::Opaque, AlphaType::Premul, AlphaType::Unpremul] {
            let mut storage = Vec::new();
            let mut surface = if direct {
                create_direct_surface(&mut storage, at, None)
            } else {
                create_surface(at, None)
            };
            test_snapshot_alphatype(reporter, &mut surface, at);
        }
    }
});

// Port of: tests/SurfaceTest.cpp#L569-L578 (chrome/m156)
fn test_writable_after_snapshot_release(surface: &mut Surface<'_>) {
    // This test succeeds by not triggering an assertion.
    // The test verifies that the surface remains writable (usable) after
    // acquiring and releasing a snapshot without triggering a copy on write.
    surface.canvas().clear(Color::new(1));
    let _ = surface.image_snapshot(); // Create and destroy SkImage
    surface.canvas().clear(Color::new(2)); // Must not assert internally
}

// Port of: tests/SurfaceTest.cpp#L579-L581 (chrome/m156)
def_tier_test!(SurfaceWriteableAfterSnapshotRelease, |_reporter| {
    test_writable_after_snapshot_release(&mut create_surface(AlphaType::Premul, None));
});

// Port of: tests/SurfaceTest.cpp#L714-L719 (chrome/m156)
fn test_no_canvas1(_reporter: &mut Reporter, surface: &mut Surface<'_>, mode: ContentChangeMode) {
    // Test passes by not asserting
    surface.notify_content_will_change(mode);
}

// Port of: tests/SurfaceTest.cpp#L720-L731 (chrome/m156)
fn test_no_canvas2(reporter: &mut Reporter, surface: &mut Surface<'_>, mode: ContentChangeMode) {
    // Verifies the robustness of SkSurface for handling use cases where calls
    // are made before a canvas is created.
    let image1 = surface.image_snapshot().expect("a snapshot");
    surface.notify_content_will_change(mode);
    let image2 = surface.image_snapshot().expect("a snapshot");
    reporter_assert!(reporter, !image1.ptr_eq(&image2));
}

// Port of: tests/SurfaceTest.cpp#L732-L741 (chrome/m156)
def_tier_test!(SurfaceNoCanvas, |reporter| {
    let modes = [ContentChangeMode::Discard, ContentChangeMode::Retain];
    let test_funcs: [fn(&mut Reporter, &mut Surface<'_>, ContentChangeMode); 2] =
        [test_no_canvas1, test_no_canvas2];
    for test_func in test_funcs {
        for mode in modes {
            test_func(reporter, &mut create_surface(AlphaType::Premul, None), mode);
        }
    }
});

// Port of: tests/SurfaceTest.cpp#L758-L777 (chrome/m156)
fn check_rowbytes_remain_consistent(surface: &mut Surface<'_>, reporter: &mut Reporter) {
    let surface_rb = surface.peek_pixels().map(|p| p.pixmap().row_bytes());
    reporter_assert!(reporter, surface_rb.is_some());

    let image = surface.image_snapshot().expect("a snapshot");
    let pm_rb = image.peek_pixels().map(|pm| pm.row_bytes());
    reporter_assert!(reporter, pm_rb.is_some());

    reporter_assert!(reporter, surface_rb == pm_rb);

    // trigger a copy-on-write
    surface.canvas().draw_paint(&Paint::default());
    let image2 = surface.image_snapshot().expect("a snapshot");
    reporter_assert!(reporter, image.unique_id() != image2.unique_id());

    let pm2_rb = image2.peek_pixels().map(|pm| pm.row_bytes());
    reporter_assert!(reporter, pm2_rb.is_some());
    reporter_assert!(reporter, pm2_rb == pm_rb);
}

// Port of: tests/SurfaceTest.cpp#L779-L792 (chrome/m156)
def_tier_test!(surface_rowbytes, |reporter| {
    let info = ImageInfo::new_n32_premul((100, 100), None);

    let mut surf0 = surfaces::raster(&info, None, None).expect("surface");
    check_rowbytes_remain_consistent(&mut surf0, reporter);

    // specify a larger rowbytes
    let mut surf1 = surfaces::raster(&info, 500_usize, None).expect("surface");
    check_rowbytes_remain_consistent(&mut surf1, reporter);

    // Try some illegal rowByte values
    let s = surfaces::raster(&info, 396_usize, None); // needs to be at least 400
    reporter_assert!(reporter, s.is_none());
    let s = surfaces::raster(&info, usize::MAX, None);
    reporter_assert!(reporter, s.is_none());
});

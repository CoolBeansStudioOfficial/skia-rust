// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SurfaceTest.cpp (chrome/m156)
//
// Not ported (the raster-only tests that need types skia-rust does not have yet):
// * `SurfaceSnapshotAlphaType`, `SurfaceCopyOnWrite`, `SurfaceWriteableAfterSnapshotRelease`,
//   `SurfaceGetTexture`, `SurfaceNoCanvas` (`test_no_canvas2`), `surface_rowbytes`,
//   `surface_image_unity`: `SkSurface::makeImageSnapshot` and `SkImage` (Phase 3).
// * `Surface_null`: `SkSurfaces::Null` and `makeImageSnapshot() == nullptr`.
// * `OverdrawSurface_Raster`: `SkOverdrawCanvas`/`SkOverdrawColorFilter` and `SkImage`.
// The Ganesh/Graphite tests are excluded.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Color, pre_multiply_color};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
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

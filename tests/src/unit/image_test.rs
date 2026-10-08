// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageTest.cpp (chrome/m156)
//
// Not ported (what the Rust port does not have, or cannot express):
// * `ImageEncode`, `image_roundtrip_encode`, `image_subset_encode_skbug_7752`,
//   `ImageScalePixels`, `ImageReadPixels`, `ImageLegacyBitmap`, `ImagePeek`: every one of them
//   makes `create_codec_image()` or encodes a PNG (`SkPngEncoder`, `DeferredFromEncodedData`),
//   which is not ported.
// * `image_from_encoded_alphatype_override`, `Image_ColorSpace`, `Image_makeColorSpace`,
//   `Image_nonfinite_dst`: decode image resources (png, jpg, webp) or make a lazy picture image
//   (`DeferredFromPicture`).
// * `ImageEmpty`: ends with `SkImages::DeferredFromGenerator` (`SkImageGenerator`, lazy images).
// The Ganesh tests are excluded.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_raster::CopyPixelsMode;
use skia_rust_core::images;
use skia_rust_core::m44::M44;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::serial_procs::SerialProcs;
use skia_rust_core::shaders::image_shader::ImageShader;
use skia_rust_raster::surfaces;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Reporter, def_test, def_tier_test, reporter_assert};

// Port of: tests/ImageTest.cpp#L235-L266 (chrome/m156)
def_tier_test!(Image_MakeFromRasterBitmap, |reporter| {
    struct Rec {
        cpm: CopyPixelsMode,
        expect_same_as_mutable: bool,
        expect_same_as_immutable: bool,
    }
    let recs = [
        Rec {
            cpm: CopyPixelsMode::IfMutable,
            expect_same_as_mutable: false,
            expect_same_as_immutable: true,
        },
        Rec {
            cpm: CopyPixelsMode::Always,
            expect_same_as_mutable: false,
            expect_same_as_immutable: false,
        },
        Rec {
            cpm: CopyPixelsMode::Never,
            expect_same_as_mutable: true,
            expect_same_as_immutable: true,
        },
    ];
    for rec in recs {
        let mut bm = Bitmap::new();
        bm.alloc_n32_pixels((100, 100), None);

        // (`pm.addr32(0, 0) == bm.getAddr32(0, 0)` compares addresses.)
        let bm_addr = |bm: &Bitmap| {
            bm.peek_pixels()
                .expect("pixels")
                .addr()
                .expect("addr")
                .as_ptr()
        };

        let img = images::make_image_from_raster_bitmap(&bm, rec.cpm).expect("an image");
        let pm = img.peek_pixels();
        reporter_assert!(reporter, pm.is_some());
        let same_mutable = pm
            .as_ref()
            .is_some_and(|pm| pm.addr().expect("addr").as_ptr() == bm_addr(&bm));
        drop(pm);
        reporter_assert!(reporter, rec.expect_same_as_mutable == same_mutable);
        reporter_assert!(
            reporter,
            (bm.generation_id() == img.unique_id()) == same_mutable
        );

        bm.notify_pixels_changed(); // force a new generation ID

        bm.set_immutable();
        let img = images::make_image_from_raster_bitmap(&bm, rec.cpm).expect("an image");
        let pm = img.peek_pixels();
        reporter_assert!(reporter, pm.is_some());
        let same_immutable = pm
            .as_ref()
            .is_some_and(|pm| pm.addr().expect("addr").as_ptr() == bm_addr(&bm));
        drop(pm);
        reporter_assert!(reporter, rec.expect_same_as_immutable == same_immutable);
        reporter_assert!(
            reporter,
            (bm.generation_id() == img.unique_id()) == same_immutable
        );
    }
});

// Test that a draw that only partially covers the drawing surface isn't
// interpreted as covering the entire drawing surface (i.e., exercise one of the
// conditions of SkCanvas::wouldOverwriteEntireSurface()).
// Port of: tests/ImageTest.cpp#L302-L334 (chrome/m156)
def_tier_test!(Image_RetainSnapshot, |reporter| {
    let red = pack_argb32(0xFF, 0xFF, 0, 0);
    let green = pack_argb32(0xFF, 0, 0xFF, 0);
    let info = ImageInfo::new_n32_premul((2, 2), None);
    let mut surface = surfaces::raster(&info, None, None).expect("surface");
    surface.canvas().clear(Color::new(0xFF00_FF00));

    let mut pixels = [0xFFFF_FFFF_u32; 4]; // init with values we don't expect
    let dst_info = ImageInfo::new_n32_premul((2, 2), None);
    let dst_row_bytes = 2 * 4;

    let read_pixels = |image: &skia_rust_core::image::Image, pixels: &mut [u32; 4]| {
        let mut bytes = [0_u8; 16];
        for (i, p) in pixels.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&p.to_ne_bytes());
        }
        let ok = image.read_pixels(&dst_info, &mut bytes, dst_row_bytes, (0, 0));
        for (i, p) in pixels.iter_mut().enumerate() {
            *p = u32::from_ne_bytes([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ]);
        }
        ok
    };

    let image1 = surface.image_snapshot().expect("a snapshot");
    reporter_assert!(reporter, read_pixels(&image1, &mut pixels));
    for p in pixels {
        reporter_assert!(reporter, p == green);
    }

    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    paint.set_color(Color::RED);

    surface
        .canvas()
        .draw_rect(Rect::from_xywh(1.0, 1.0, 1.0, 1.0), &paint);

    let image2 = surface.image_snapshot().expect("a snapshot");
    reporter_assert!(reporter, read_pixels(&image2, &mut pixels));
    reporter_assert!(reporter, pixels[0] == green);
    reporter_assert!(reporter, pixels[1] == green);
    reporter_assert!(reporter, pixels[2] == green);
    reporter_assert!(reporter, pixels[3] == red);
});

/////////////////////////////////////////////////////////////////////////////////////////////////

// Port of: tests/ImageTest.cpp#L336-L338 (chrome/m156)
fn make_bitmap_mutable(bm: &mut Bitmap) {
    bm.alloc_n32_pixels((10, 10), None);
}

// Port of: tests/ImageTest.cpp#L340-L343 (chrome/m156)
fn make_bitmap_immutable(bm: &mut Bitmap) {
    bm.alloc_n32_pixels((10, 10), None);
    bm.set_immutable();
}

// Port of: tests/ImageTest.cpp#L345-L373 (chrome/m156)
def_tier_test!(image_newfrombitmap, |reporter| {
    struct Rec {
        make_proc: fn(&mut Bitmap),
        expect_peek_success: bool,
        expect_shared_id: bool,
        expect_lazy: bool,
    }
    let rec = [
        Rec {
            make_proc: make_bitmap_mutable,
            expect_peek_success: true,
            expect_shared_id: false,
            expect_lazy: false,
        },
        Rec {
            make_proc: make_bitmap_immutable,
            expect_peek_success: true,
            expect_shared_id: true,
            expect_lazy: false,
        },
    ];

    for r in &rec {
        let mut bm = Bitmap::new();
        (r.make_proc)(&mut bm);

        let image = bm.as_image().expect("an image");

        let shared_id = image.unique_id() == bm.generation_id();
        reporter_assert!(reporter, shared_id == r.expect_shared_id);

        let peek_success = image.peek_pixels().is_some();
        reporter_assert!(reporter, peek_success == r.expect_peek_success);

        let lazy = image.is_lazy_generated();
        reporter_assert!(reporter, lazy == r.expect_lazy);
    }
});

// Port of: tests/ImageTest.cpp#L717-L727 (chrome/m156)
def_test!(ImageDataRef, |reporter| {
    let info = ImageInfo::new_n32_premul((1, 1), None);
    let row_bytes = info.min_row_bytes();
    let size = info.compute_byte_size(row_bytes);
    let data = Data::new_uninitialized(size);
    reporter_assert!(reporter, data.unique());
    let image = images::raster_from_data(&info, data.clone(), row_bytes);
    reporter_assert!(reporter, !data.unique());
    drop(image);
    reporter_assert!(reporter, data.unique());
});

///////////////////////////////////////////////////////////////////////////////////////////////////

// Port of: tests/ImageTest.cpp#L1401-L1410 (chrome/m156)
fn make_all_premul(bm: &mut Bitmap) {
    bm.alloc_pixels_info(
        &ImageInfo::new_n32((256, 256), AlphaType::Premul, None),
        None,
    );
    for a in 0..256_i32 {
        for r in 0..256_i32 {
            // make all valid premul combinations
            let c = a.min(r);
            #[allow(clippy::cast_sign_loss)] // 0..=255
            bm.set_addr32(a, r, pack_argb32(a as u32, c as u32, c as u32, c as u32));
        }
    }
}

// Port of: tests/ImageTest.cpp#L1412-L1425 (chrome/m156)
fn equal(a: &Bitmap, b: &Bitmap) -> bool {
    debug_assert_eq!(a.width(), b.width());
    debug_assert_eq!(a.height(), b.height());
    for y in 0..a.height() {
        for x in 0..a.width() {
            let pa = a.get_addr32(x, y);
            let pb = b.get_addr32(x, y);
            if pa != pb {
                return false;
            }
        }
    }
    true
}

// Port of: tests/ImageTest.cpp#L1442-L1455 (chrome/m156)
def_tier_test!(image_roundtrip_premul, |reporter| {
    let mut bm0 = Bitmap::new();
    make_all_premul(&mut bm0);

    let mut bm1 = Bitmap::new();
    bm1.alloc_pixels_info(
        &ImageInfo::new_n32((256, 256), AlphaType::Unpremul, None),
        None,
    );
    let _ = bm0.read_pixels_to_pixmap(&mut bm1.peek_pixels_mut().expect("pixels"), (0, 0));

    let mut bm2 = Bitmap::new();
    bm2.alloc_pixels_info(
        &ImageInfo::new_n32((256, 256), AlphaType::Premul, None),
        None,
    );
    let _ = bm1.read_pixels_to_pixmap(&mut bm2.peek_pixels_mut().expect("pixels"), (0, 0));

    reporter_assert!(reporter, equal(&bm0, &bm2));
});

// Port of: tests/ImageTest.cpp#L1631-L1659 (chrome/m156)
// (with `gCentripetalCatmulRom` and `gMitchellNetravali`)
def_test!(image_cubicresampler, |reporter| {
    let g_centripetal_catmul_rom = M44::new(
        0.0 / 2.0,
        -1.0 / 2.0,
        2.0 / 2.0,
        -1.0 / 2.0,
        2.0 / 2.0,
        0.0 / 2.0,
        -5.0 / 2.0,
        3.0 / 2.0,
        0.0 / 2.0,
        1.0 / 2.0,
        4.0 / 2.0,
        -3.0 / 2.0,
        0.0 / 2.0,
        0.0 / 2.0,
        -1.0 / 2.0,
        1.0 / 2.0,
    );

    let g_mitchell_netravali = M44::new(
        1.0 / 18.0,
        -9.0 / 18.0,
        15.0 / 18.0,
        -7.0 / 18.0,
        16.0 / 18.0,
        0.0 / 18.0,
        -36.0 / 18.0,
        21.0 / 18.0,
        1.0 / 18.0,
        9.0 / 18.0,
        27.0 / 18.0,
        -21.0 / 18.0,
        0.0 / 18.0,
        0.0 / 18.0,
        -6.0 / 18.0,
        7.0 / 18.0,
    );

    let diff = |reporter: &mut Reporter, a: &M44, b: &M44| {
        let tolerance = 0.000_001_f32;
        for r in 0..4 {
            for c in 0..4 {
                let d = (a.rc(r, c) - b.rc(r, c)).abs();
                #[allow(clippy::neg_cmp_op_on_partial_ord)] // REPORTER_ASSERT(d <= tolerance)
                {
                    reporter_assert!(reporter, d <= tolerance);
                }
            }
        }
    };

    diff(
        reporter,
        &ImageShader::cubic_resampler_matrix(1.0 / 3.0, 1.0 / 3.0),
        &g_mitchell_netravali,
    );

    diff(
        reporter,
        &ImageShader::cubic_resampler_matrix(0.0, 1.0 / 2.0),
        &g_centripetal_catmul_rom,
    );
});

// Port of: tests/ImageTest.cpp#L268-L297 (chrome/m156)
def_test!(Image_Serialize_Encoding_Failure, |reporter| {
    let mut surface = surfaces::raster_n32_premul((100, 100)).expect("a surface");
    surface.canvas().clear(Color::GREEN);
    let image = surface.image_snapshot();
    reporter_assert!(reporter, image.is_some());
    let image = image.expect("an image");

    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::from_wh(100.0, 100.0), false);
    canvas.draw_image(&image, (0.0, 0.0), None);
    let picture = recorder.finish_recording_as_picture(None);
    reporter_assert!(reporter, picture.is_some());
    let picture = picture.expect("a picture");
    reporter_assert!(reporter, picture.approximate_op_count() > 0);

    // The image procedure is called, and it returns empty data (`SkData::MakeEmpty()`).
    let was_called = Arc::new(AtomicBool::new(false));
    let procs = SerialProcs {
        image: Some({
            let was_called = Arc::clone(&was_called);
            Arc::new(move |_image: &Image| {
                was_called.store(true, Ordering::Relaxed);
                Some(Data::new_empty())
            })
        }),
        ..SerialProcs::default()
    };

    reporter_assert!(reporter, !was_called.load(Ordering::Relaxed));
    let data = picture.serialize(Some(&procs));
    reporter_assert!(reporter, was_called.load(Ordering::Relaxed));
    reporter_assert!(reporter, data.as_ref().is_some_and(|data| data.size() > 0));
    let Some(data) = data else {
        return;
    };

    let deserialized = Picture::from_data(data.as_bytes(), None);
    reporter_assert!(reporter, deserialized.is_some());
    let deserialized = deserialized.expect("a picture");
    reporter_assert!(reporter, deserialized.approximate_op_count() > 0);
});

// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageTest.cpp (chrome/m156)
//
// Not ported (what the Rust port does not have, or cannot express):
// * `ImageScalePixels`, `ImageReadPixels`, `ImageLegacyBitmap`, `ImagePeek`: each of them
//   makes `create_codec_image()` (SkPngEncoder + DeferredFromEncodedData) in its last case, and
//   the rest of the test has not been ported yet.
// * `Image_ColorSpace`, `Image_nonfinite_dst`: decode image resources
//   (png, jpg, webp) or make a lazy picture image (`DeferredFromPicture`), and the last two need
//   `ToolUtils::PixelIter`/`any_image_will_do` helpers that are not ported yet.
// The Ganesh tests are excluded.

#![cfg(test)]

use skia_rust_codec::encode::png_encoder;
use skia_rust_codec::image_generator_from_encoded::make_from_encoded;
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::data::Data;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_base::NEED_NEW_IMAGE_UNIQUE_ID;
use skia_rust_core::image_generator::{ImageGenerator, generator_unique_id};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_raster::CopyPixelsMode;
use skia_rust_core::images;
use skia_rust_core::m44::M44;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::serial_procs::SerialProcs;
use skia_rust_core::shaders::image_shader::ImageShader;
use skia_rust_raster::surfaces;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::resources::{get_resource_as_data, get_resource_as_image};
use crate::tools::tool_utils::equal_pixels_image;
use crate::{Reporter, def_test, def_tier_test, errorf, reporter_assert, skip_missing_resource};

// Port of: tests/ImageTest.cpp#L702-L705 (chrome/m156)
struct EmptyGenerator {
    info: ImageInfo,
    unique_id: u32,
}

impl EmptyGenerator {
    fn new() -> Self {
        EmptyGenerator {
            info: ImageInfo::new_n32_premul((0, 0), None),
            unique_id: generator_unique_id(NEED_NEW_IMAGE_UNIQUE_ID),
        }
    }
}

impl ImageGenerator for EmptyGenerator {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }
}

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

// Port of: tests/ImageTest.cpp#L1360-L1397 (chrome/m156)
def_test!(Image_makeColorSpace, |reporter| {
    use skia_rust_core::color_data::swizzle_rgba_to_pm_color;
    use skia_rust_core::color_priv::{get_packed_b32, get_packed_g32, get_packed_r32};
    use skia_rust_core::color_space::{named_gamut, named_transfer_fn};
    use skia_rust_core::image::RequiredProperties;
    use skia_rust_skcms::TransferFunction;

    let p3 = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3)
        .expect("MakeRGB(kSRGB, kDisplayP3)");
    let fn_ = TransferFunction {
        g: 1.8,
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
    };
    let adobe_gamut =
        ColorSpace::new_rgb(&fn_, &named_gamut::ADOBE_RGB).expect("MakeRGB(fn, AdobeRGB)");

    let mut srgb_bitmap = Bitmap::new();
    let pixel = swizzle_rgba_to_pm_color(0xFF60_4020);
    let installed = srgb_bitmap.install_pixels(
        &ImageInfo::new_s32((1, 1), AlphaType::Opaque),
        pixel.to_ne_bytes().to_vec(),
        4,
    );
    reporter_assert!(reporter, installed);
    srgb_bitmap.set_immutable();
    let srgb_image = srgb_bitmap.as_image().expect("asImage");

    let p3_image = srgb_image
        .make_color_space(p3.clone(), RequiredProperties::default())
        .expect("makeColorSpace(p3)");
    let p3_bitmap = p3_image.as_legacy_bitmap();
    reporter_assert!(reporter, p3_bitmap.is_some());
    let Some(p3_bitmap) = p3_bitmap else {
        return;
    };

    let almost_equal = |a: u32, b: u32| a.abs_diff(b) <= 2;

    let px = p3_bitmap.get_addr32(0, 0);
    reporter_assert!(reporter, almost_equal(0x28, get_packed_r32(px)));
    reporter_assert!(reporter, almost_equal(0x40, get_packed_g32(px)));
    reporter_assert!(reporter, almost_equal(0x5E, get_packed_b32(px)));

    let adobe_image = srgb_image
        .make_color_space(adobe_gamut, RequiredProperties::default())
        .expect("makeColorSpace(adobe)");
    let adobe_bitmap = adobe_image.as_legacy_bitmap();
    reporter_assert!(reporter, adobe_bitmap.is_some());
    let Some(adobe_bitmap) = adobe_bitmap else {
        return;
    };
    let px = adobe_bitmap.get_addr32(0, 0);
    reporter_assert!(reporter, almost_equal(0x21, get_packed_r32(px)));
    reporter_assert!(reporter, almost_equal(0x31, get_packed_g32(px)));
    reporter_assert!(reporter, almost_equal(0x4C, get_packed_b32(px)));

    let Some(srgb_image) = get_resource_as_image("images/1x1.png") else {
        reporter_assert!(reporter, false);
        return;
    };
    let p3_image = srgb_image
        .make_color_space(p3, RequiredProperties::default())
        .expect("makeColorSpace(p3)");
    let p3_bitmap = p3_image.as_legacy_bitmap();
    reporter_assert!(reporter, p3_bitmap.is_some());
    let Some(p3_bitmap) = p3_bitmap else {
        return;
    };
    let px = p3_bitmap.get_addr32(0, 0);
    reporter_assert!(reporter, almost_equal(0x8B, get_packed_r32(px)));
    reporter_assert!(reporter, almost_equal(0x82, get_packed_g32(px)));
    reporter_assert!(reporter, almost_equal(0x77, get_packed_b32(px)));
});

// Port of: tests/ImageTest.cpp#L707-L715 (chrome/m156)
def_test!(ImageEmpty, |reporter| {
    let info = ImageInfo::new_n32_premul((0, 0), None);
    // skia-rust: the nullptr pixels of the C++ are an empty slice (and an empty Data).
    let mut no_pixels: [u8; 0] = [];
    let pmap = Pixmap::new(&info, &mut no_pixels, 0).expect("pixmap");
    reporter_assert!(reporter, images::raster_from_pixmap_copy(&pmap).is_none());
    reporter_assert!(
        reporter,
        images::raster_from_data(&info, Data::new_empty(), 0).is_none()
    );
    reporter_assert!(reporter, images::raster_from_pixmap(&pmap, || {}).is_none());
    reporter_assert!(
        reporter,
        images::deferred_from_generator(Some(Box::new(EmptyGenerator::new()))).is_none()
    );
});

// Port of: tests/ImageTest.cpp#L1457-L1470 (chrome/m156)
def_test!(image_from_encoded_alphatype_override, |reporter| {
    let path = "images/mandrill_32.png";
    let data = Data::new_from_vec(skip_missing_resource!(get_resource_as_data(path), path));

    // Ensure that we can decode the image when we specifically request premul or unpremul, but
    // not when we request kOpaque
    reporter_assert!(
        reporter,
        deferred_from_encoded_data(Some(data.clone()), Some(AlphaType::Premul)).is_some()
    );
    reporter_assert!(
        reporter,
        deferred_from_encoded_data(Some(data.clone()), Some(AlphaType::Unpremul)).is_some()
    );
    reporter_assert!(
        reporter,
        deferred_from_encoded_data(Some(data.clone()), Some(AlphaType::Opaque)).is_none()
    );

    // Same tests as above, but using SkImageGenerators::MakeFromEncoded
    reporter_assert!(
        reporter,
        make_from_encoded(Some(data.clone()), Some(AlphaType::Premul)).is_some()
    );
    reporter_assert!(
        reporter,
        make_from_encoded(Some(data.clone()), Some(AlphaType::Unpremul)).is_some()
    );
    reporter_assert!(
        reporter,
        make_from_encoded(Some(data), Some(AlphaType::Opaque)).is_none()
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

// Port of: tests/ImageTest.cpp#L1427-L1440 (chrome/m156)
def_test!(image_roundtrip_encode, |reporter| {
    let mut bm0 = Bitmap::new();
    make_all_premul(&mut bm0);

    let Some(img0) = bm0.as_image() else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(data) = skia_rust_codec::encode::png_encoder::encode_image(
        &img0,
        &skia_rust_codec::encode::png_encoder::Options::default(),
    ) else {
        reporter_assert!(reporter, false);
        return;
    };
    let Some(img1) = deferred_from_encoded_data(Some(data), None) else {
        reporter_assert!(reporter, false);
        return;
    };

    let mut bm1 = Bitmap::new();
    bm1.alloc_pixels_info(
        &ImageInfo::new_n32((256, 256), AlphaType::Premul, None),
        None,
    );
    let Some(mut pm) = bm1.peek_pixels_mut() else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(reporter, img1.read_pixels_to_pixmap(&mut pm, (0, 0)));
    reporter_assert!(reporter, equal(&bm0, &bm1));
});

// Port of: tests/ImageTest.cpp#L132-L136 (chrome/m156), draw_image_test_pattern: a white canvas
// with a black square at (5, 5) of 10 by 10.
fn draw_image_test_pattern(canvas: &Canvas) {
    canvas.clear(Color::WHITE);
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    canvas.draw_rect(Rect::from_xywh(5.0, 5.0, 10.0, 10.0), &paint);
}

// Port of: tests/ImageTest.cpp#L138-L143 (chrome/m156), create_image
fn create_image() -> Option<Image> {
    let info = ImageInfo::new_n32((20, 20), AlphaType::Opaque, None);
    let mut surface = surfaces::raster(&info, None, None)?;
    draw_image_test_pattern(surface.canvas());
    surface.image_snapshot()
}

// Port of: tests/ImageTest.cpp#L87-L103 (chrome/m156), read_pixels_info: the N32 info of the
// image's size and alpha type.
fn read_pixels_info(image: &Image) -> ImageInfo {
    ImageInfo::new_n32((image.width(), image.height()), image.alpha_type(), None)
}

// Port of: tests/ImageTest.cpp#L106-L130 (chrome/m156), assert_equal (dContextA is always null
// here: there is no GPU backend). `b` is raster, so it can be read back.
fn assert_equal(reporter: &mut Reporter, a: &Image, subset_a: Option<IRect>, b: &Image) {
    let width_a = subset_a.map_or(a.width(), |s| s.width());
    let height_a = subset_a.map_or(a.height(), |s| s.height());

    reporter_assert!(reporter, width_a == b.width());
    reporter_assert!(reporter, height_a == b.height());

    // see skbug.com/40035123
    //REPORTER_ASSERT(reporter, a->isOpaque() == b->isOpaque());

    let info_a = read_pixels_info(a);
    let info_b = read_pixels_info(b);
    let row_bytes_a = info_a.min_row_bytes();
    let row_bytes_b = info_b.min_row_bytes();
    let mut pmap_a = vec![0u8; info_a.compute_byte_size(row_bytes_a)];
    let mut pmap_b = vec![0u8; info_b.compute_byte_size(row_bytes_b)];

    let (src_x, src_y) = subset_a.map_or((0, 0), |s| (s.x(), s.y()));

    reporter_assert!(
        reporter,
        a.read_pixels(&info_a, &mut pmap_a, row_bytes_a, (src_x, src_y))
    );
    reporter_assert!(
        reporter,
        b.read_pixels(&info_b, &mut pmap_b, row_bytes_b, (0, 0))
    );

    let width_bytes = usize::try_from(width_a).unwrap_or(0) * 4;
    for y in 0..usize::try_from(height_a).unwrap_or(0) {
        // pmapA.addr32(0, y) and pmapB.addr32(0, y), compared over widthA pixels.
        reporter_assert!(
            reporter,
            pmap_a[y * row_bytes_a..][..width_bytes] == pmap_b[y * row_bytes_b..][..width_bytes]
        );
    }
}

// Port of: tests/ImageTest.cpp#L193-L209 (chrome/m156), test_encode with a null dContext
fn test_encode(reporter: &mut Reporter, image: &Image) {
    let ir = IRect::from_xywh(5, 5, 10, 10);
    let Some(orig_encoded) = png_encoder::encode_image(image, &png_encoder::Options::default())
    else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(reporter, orig_encoded.size() > 0);

    let Some(decoded) = deferred_from_encoded_data(Some(orig_encoded.clone()), None) else {
        errorf!(reporter, "failed to decode image!");
        return;
    };
    assert_equal(reporter, image, None, &decoded);

    // Now see if we can instantiate an image from a subset of the surface/origEncoded
    let Some(decoded) = deferred_from_encoded_data(Some(orig_encoded), None)
        .and_then(|encoded| encoded.make_subset(ir, RequiredProperties::default()))
    else {
        reporter_assert!(reporter, false);
        return;
    };
    assert_equal(reporter, image, Some(ir), &decoded);
}

// Port of: tests/ImageTest.cpp#L211-L213 (chrome/m156), ImageEncode
def_test!(ImageEncode, |reporter| {
    let Some(image) = create_image() else {
        reporter_assert!(reporter, false);
        return;
    };
    test_encode(reporter, &image);
});

// Port of: tests/ImageTest.cpp#L1687-L1700 (chrome/m156), image_subset_encode_skbug_7752
def_test!(image_subset_encode_skbug_7752, |reporter| {
    let path = "images/mandrill_128.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Some(image) = deferred_from_encoded_data(Some(Data::new_from_vec(data)), None) else {
        errorf!(reporter, "failed to decode {}", path);
        return;
    };
    let w = image.width();
    let h = image.height();

    // Port of the check_roundtrip lambda.
    let check_roundtrip = |reporter: &mut Reporter, img: &Image| {
        let encoded = png_encoder::encode_image(img, &png_encoder::Options::default());
        let Some(img2) = deferred_from_encoded_data(encoded, None) else {
            reporter_assert!(reporter, false);
            return;
        };
        reporter_assert!(reporter, equal_pixels_image(img, &img2));
    };
    check_roundtrip(reporter, &image); // should trivially pass
    let Some(subset) = image.make_subset(
        IRect::from_ltrb(0, 0, w / 2, h / 2),
        RequiredProperties::default(),
    ) else {
        reporter_assert!(reporter, false);
        return;
    };
    check_roundtrip(reporter, &subset);
    let Some(subset) = image.make_subset(
        IRect::from_ltrb(w / 2, h / 2, w, h),
        RequiredProperties::default(),
    ) else {
        reporter_assert!(reporter, false);
        return;
    };
    check_roundtrip(reporter, &subset);
    let Some(linear) =
        image.make_color_space(ColorSpace::new_srgb_linear(), RequiredProperties::default())
    else {
        reporter_assert!(reporter, false);
        return;
    };
    check_roundtrip(reporter, &linear);
});

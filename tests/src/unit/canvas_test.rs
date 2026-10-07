// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CanvasTest.cpp (chrome/m156)
//
// Not ported yet (each needs a type or build option that is not available):
// - `canvas_unsorted_clip`: `SkPictureRecorder` / `SkRecord` (picture recording).
// - `canvas_clipbounds`: its last block calls `SkPictureRecorder().beginRecording` (picture
//   recording), and `SkCanvas c(-10, -20)` (`Canvas::new_no_pixels` returns `None` for a
//   negative size as in skia-safe, where the C++ clamps to zero).
// - `canvas_clip_restriction`, `canvas_empty_clip`: compiled only with `SK_SUPPORT_PDF`, and drive
//   a recording canvas and a PDF canvas as well as the raster one (picture recording, PDF).
// - `Canvas_bitmap`: steps use `SkPictureRecorder` + `drawPicture`, `SkVertices` and an image
//   shader (`SkBitmap::makeShader`, Phase 3).
// - `Canvas_pdf`: PDF.
// - `Canvas_LegacyColorBehavior`: compiled only with `SK_BUILD_FOR_ANDROID_FRAMEWORK`
//   (`SkCanvas::ColorBehavior`).
// - `Canvas_SaveLayerWithNullBoundsAndZeroBoundsImageFilter`, `Canvas_ClippedOutImageFilter`,
//   `Canvas_degenerate_dimension`: image filters (`SkImageFilters::Empty/Blur/Shader`, Phase 3).
// - `NWayCanvas`, `CanvasStack`: `SkNWayCanvas`, `SkCanvasStack` and `SkCanvas` subclasses.
// - `PaintFilterCanvas_ConsistentState`: `SkPaintFilterCanvas`.
// - `TestManyDrawsGanesh`, `TestManyDrawsGraphite`: GPU.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::{Color, pre_multiply_color};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;

use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, errorf, reporter_assert};

// Port of: tests/CanvasTest.cpp#L200-L243 (chrome/m156)
def_tier_test!(CanvasNewRasterTest, |reporter| {
    let mut info = ImageInfo::new_n32_premul((10, 10), None);
    let min_row_bytes = info.min_row_bytes();
    let size = info.compute_byte_size(min_row_bytes);
    let mut storage = vec![0_u8; size]; // AutoTMalloc + sk_bzero

    {
        let canvas = Canvas::from_raster_direct(&info, &mut storage, min_row_bytes, None);
        reporter_assert!(reporter, canvas.is_some());
        let canvas = canvas.expect("checked above");

        let peek = canvas.peek_pixels();
        reporter_assert!(reporter, peek.is_some());
        let peek = peek.expect("checked above");
        let pmap = peek.pixmap();
        reporter_assert!(reporter, info == *pmap.info());
        reporter_assert!(reporter, min_row_bytes == pmap.row_bytes());
        for y in 0..info.height() {
            for x in 0..info.width() {
                reporter_assert!(reporter, 0 == pmap.addr32(x, y));
            }
        }
    }

    // unaligned rowBytes
    reporter_assert!(
        reporter,
        Canvas::from_raster_direct(&info, &mut storage, min_row_bytes + 1, None).is_none()
    );

    // now try a deliberately bad info
    info = info.with_wh(-1, info.height());
    reporter_assert!(
        reporter,
        Canvas::from_raster_direct(&info, &mut storage, min_row_bytes, None).is_none()
    );

    // too big
    info = info.with_wh(1 << 30, 1 << 30);
    reporter_assert!(
        reporter,
        Canvas::from_raster_direct(&info, &mut storage, min_row_bytes, None).is_none()
    );

    // not a valid pixel type
    info = ImageInfo::new((10, 10), ColorType::Unknown, info.alpha_type(), None);
    reporter_assert!(
        reporter,
        Canvas::from_raster_direct(&info, &mut storage, min_row_bytes, None).is_none()
    );

    // We should not succeed with a zero-sized valid info
    info = ImageInfo::new_n32_premul((0, 0), None);
    let canvas = Canvas::from_raster_direct(&info, &mut storage, min_row_bytes, None);
    reporter_assert!(reporter, canvas.is_none());
});

// Port of: tests/CanvasTest.cpp#L430-L446 (chrome/m156)
def_tier_test!(Canvas_SaveState, |reporter| {
    let canvas = Canvas::new_no_pixels((10, 10), None).expect("canvas");
    reporter_assert!(reporter, 1 == canvas.save_count());

    let mut n = canvas.save();
    reporter_assert!(reporter, 1 == n);
    reporter_assert!(reporter, 2 == canvas.save_count());

    n = canvas.save_layer(&SaveLayerRec::default());
    reporter_assert!(reporter, 2 == n);
    reporter_assert!(reporter, 3 == canvas.save_count());

    canvas.restore();
    reporter_assert!(reporter, 2 == canvas.save_count());
    canvas.restore();
    reporter_assert!(reporter, 1 == canvas.save_count());
});

// Port of: tests/CanvasTest.cpp#L448-L465 (chrome/m156)
def_tier_test!(Canvas_ClipEmptyPath, |_reporter| {
    let canvas = Canvas::new_no_pixels((10, 10), None).expect("canvas");
    canvas.save();
    let path = Path::default();
    canvas.clip_path(&path, None, None);
    canvas.restore();

    canvas.save();
    let mut builder = PathBuilder::new();
    builder.move_to((5.0, 5.0));
    canvas.clip_path(&builder.snapshot(), None, None);
    canvas.restore();

    canvas.save();
    builder.move_to((7.0, 7.0));
    canvas.clip_path(&builder.detach(), None, None); // should not assert here
    canvas.restore();
});

// Port of: tests/CanvasTest.cpp#L568-L592 (chrome/m156)
fn test_cliptype(canvas: &Canvas, r: &mut Reporter) {
    reporter_assert!(r, !canvas.is_clip_empty());
    reporter_assert!(r, canvas.is_clip_rect());

    canvas.save();
    canvas.clip_rect(Rect::new(0.0, 0.0, 0.0, 0.0), None, None);
    reporter_assert!(r, canvas.is_clip_empty());
    reporter_assert!(r, !canvas.is_clip_rect());
    canvas.restore();

    canvas.save();
    canvas.clip_rect(Rect::new(2.0, 2.0, 6.0, 6.0), None, None);
    reporter_assert!(r, !canvas.is_clip_empty());
    reporter_assert!(r, canvas.is_clip_rect());
    canvas.restore();

    canvas.save();
    // punch a hole in the clip
    canvas.clip_rect(Rect::new(2.0, 2.0, 6.0, 6.0), ClipOp::Difference, None);
    reporter_assert!(r, !canvas.is_clip_empty());
    reporter_assert!(r, !canvas.is_clip_rect());
    canvas.restore();

    reporter_assert!(r, !canvas.is_clip_empty());
    reporter_assert!(r, canvas.is_clip_rect());
}

// Port of: tests/CanvasTest.cpp#L594-L605 (chrome/m156)
// The `SK_SUPPORT_PDF` clipstack backend block is not here (PDF is not ported).
def_tier_test!(CanvasClipType, |r| {
    // test rasterclip backend
    let mut surface = surfaces::raster_n32_premul((10, 10)).expect("surface");
    test_cliptype(surface.canvas(), r);
});

// Port of: tests/CanvasTest.cpp#L680-L736 (chrome/m156)
def_tier_test!(canvas_savelayer_destructor, |reporter| {
    // What should happen in our destructor if we have unbalanced saveLayers?

    let info = ImageInfo::new_n32_premul((4, 4), None);
    // skia-rust: a surface owns the pixels it draws into and copies them back to `pixels` when it
    // drops (docs/design/pixels.md), so `pm` is a view of `pixels` between surfaces, and of the
    // live surface while one exists.
    let mut pixels = [0_u8; 16 * 4];

    // check all of the pixel values in pm
    let check_pixels = |reporter: &mut Reporter, pm: &Pixmap<'_>, expected: Color| {
        let pmc = pre_multiply_color(expected);
        for y in 0..pm.info().height() {
            for x in 0..pm.info().width() {
                if pm.addr32(x, y) != pmc {
                    errorf!(reporter, "check_pixels_failed");
                    return;
                }
            }
        }
    };
    let check_buffer = |reporter: &mut Reporter, pixels: &[u8], expected: Color| {
        let pm = Pixmap::new_readonly(&info, pixels, 4 * 4).expect("pixmap");
        check_pixels(reporter, &pm, expected);
    };

    let do_test =
        |reporter: &mut Reporter, pixels: &mut [u8], save_count: i32, restore_count: i32| {
            assert!(restore_count <= save_count); // SkASSERT

            let mut surf = surfaces::wrap_pixels(&info, pixels, 4 * 4, None).expect("surface");
            surf.canvas().clear(Color::RED);
            check_pixels(
                reporter,
                &surf.peek_pixels().expect("pixels").pixmap(),
                Color::RED,
            );

            {
                let canvas = surf.canvas();
                for _ in 0..save_count {
                    canvas.save_layer(&SaveLayerRec::default());
                }

                canvas.clear(Color::BLUE);
            }
            // so far, we still expect to see the red, since the blue was drawn in a layer
            check_pixels(
                reporter,
                &surf.peek_pixels().expect("pixels").pixmap(),
                Color::RED,
            );

            {
                let canvas = surf.canvas();
                for _ in 0..restore_count {
                    canvas.restore();
                }
            }
            // by returning, we are implicitly deleting the surface, and its associated canvas
        };

    do_test(reporter, &mut pixels, 1, 1);
    // since we called restore, we expect to see now see blue
    check_buffer(reporter, &pixels, Color::BLUE);

    // Now repeat that, but delete the canvas before we restore it
    do_test(reporter, &mut pixels, 1, 0);
    // We don't blit the unbalanced saveLayers, so we expect to see red (not the layer's blue)
    check_buffer(reporter, &pixels, Color::RED);

    // Finally, test with multiple unbalanced saveLayers. This led to a crash in an earlier
    // implementation (crbug.com/1238731)
    do_test(reporter, &mut pixels, 2, 0);
    check_buffer(reporter, &pixels, Color::RED);
});

// Port of: tests/CanvasTest.cpp#L738-L754 (chrome/m156)
def_tier_test!(Canvas_saveLayer_colorSpace, |reporter| {
    let info = ImageInfo::new_n32((1, 1), AlphaType::Opaque, None);
    let mut pixels = [0_u8; 4];
    pixels.copy_from_slice(&u32::from(Color::BLACK).to_ne_bytes());

    {
        let mut surf = surfaces::wrap_pixels(&info, &mut pixels, 4, None).expect("surface");
        let canvas = surf.canvas();

        let cs = ColorSpace::new_srgb().with_color_spin();
        canvas.save_layer(&SaveLayerRec::default().color_space(&cs));
        let mut paint = Paint::default();
        paint.set_color(Color::RED);
        canvas.draw_paint(&paint);
        canvas.restore();
    }

    let pm = Pixmap::new_readonly(&info, &pixels, 4).expect("pixmap");
    reporter_assert!(reporter, pm.get_color((0, 0)) == Color::BLUE);
});

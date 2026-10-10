// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageFilterTest.cpp (chrome/m156)
//
// Only the tests whose filters are ported are here: the Offset, Merge, Blend, Image and
// DropShadow filters and the raster backend. The morphology, lighting, displacement, picture,
// shader, runtime and matrix-convolution filters, the Graphite and Ganesh variants (except
// `ImageFilterMakeWithFilter_Graphite`), the
// blur-dependent bounds tests and the image-filter-cache tests are not ported yet.

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filters;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_filter::MapDirection;
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters;
use skia_rust_raster::image_filter_backend::make_raster_backend;
use skia_rust_raster::surfaces;

use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::image_factories as graphite_image_factories;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;

use crate::{
    Reporter, def_graphite_test_for_all_contexts, def_test, def_tier_test, reporter_assert,
};

/// `make_context(out, src)`: a raster context whose desired output is `out` and whose source is
/// `src` (`make_backend_compatible_with_image` is the raster backend here).
// Port of: tests/ImageFilterTest.cpp#L324-L333 (chrome/m156)
fn make_context<'a>(out: IRect, src: Option<Arc<SpecialImage>>) -> Context<'a> {
    let color_space = src.as_ref().and_then(|s| s.color_info().color_space());
    let backend = make_raster_backend(SurfaceProps::default(), ColorType::N32);
    Context::new(
        backend,
        Mapping::new(),
        out,
        FilterResult::new(src, skia_rust_core::point::IPoint::new(0, 0)),
        color_space,
        None,
    )
}

/// `make_context(outWidth, outHeight, src)`.
// Port of: tests/ImageFilterTest.cpp#L335-L337 (chrome/m156)
fn make_context_wh<'a>(
    out_width: i32,
    out_height: i32,
    src: Option<Arc<SpecialImage>>,
) -> Context<'a> {
    make_context(IRect::from_wh(out_width, out_height), src)
}

/// `create_empty_special_image(rContext, widthHeight, color)`: a raster special image of `color`.
// Port of: tests/ImageFilterTest.cpp#L417-L429 (chrome/m156)
fn create_empty_special_image(width_height: i32, color: Color) -> Option<Arc<SpecialImage>> {
    use skia_rust_core::device::Device;
    use skia_rust_core::paint::Paint;
    let info = ImageInfo::new(
        (width_height, width_height),
        ColorType::N32,
        AlphaType::Premul,
        None,
    );
    let mut device =
        skia_rust_raster::bitmap_device::BitmapDevice::create(&info, SurfaceProps::default())?;
    let mut p = Paint::default();
    p.set_color(color);
    p.set_blend_mode(BlendMode::Src);
    device.draw_paint(&p);
    device
        .snap_special(&IRect::from_wh(width_height, width_height), false)
        .map(Arc::new)
}

// Port of: tests/ImageFilterTest.cpp#L2271-L2294 (chrome/m156)
def_tier_test!(OffsetImageFilterBounds, |reporter| {
    let src = IRect::from_xywh(0, 0, 100, 100);
    let src_offset = (-50.5_f32, -50.5_f32);
    let offset = image_filters::offset(src_offset, None, None).expect("an offset filter");

    // Because the offset has a fractional component, the final output and required input bounds
    // will be rounded out to include an extra pixel.
    let expected_forward = Rect::from_irect(src).with_offset(src_offset).round_out();
    let bounds_forward =
        offset.filter_bounds(&src, &Matrix::new_identity(), MapDirection::Forward, None);
    reporter_assert!(reporter, bounds_forward == expected_forward);

    let expected_reverse_rect = Rect::from_irect(src).with_offset((-src_offset.0, -src_offset.1));
    let mut expected_reverse = expected_reverse_rect.round_out();

    // Intersect 'expectedReverse' with the source because we are passing &src in as the known
    // input bounds, which is the bounds of non-transparent pixels that can be moved by the offset.
    // While the ::Offset filter could show all pixels inside 'expectedReverse' given that 'src'
    // is also the target device output of the filter, the required input can be made tighter.
    let intersected = IRect::intersect(&expected_reverse, &src);
    reporter_assert!(reporter, intersected.is_some());
    expected_reverse = intersected.expect("intersects");

    let bounds_reverse = offset.filter_bounds(
        &src,
        &Matrix::new_identity(),
        MapDirection::Reverse,
        Some(&src),
    );
    reporter_assert!(reporter, bounds_reverse == expected_reverse);
});

// Port of: tests/ImageFilterTest.cpp#L2296-L2324 (chrome/m156)
def_tier_test!(OffsetImageFilterBoundsNoOverflow, |reporter| {
    let src = IRect::from_xywh(-10, -10, 20, 20);
    // `SkIntToScalar(std::numeric_limits<int>::max()) * 2.f / 3.f`
    #[allow(clippy::cast_precision_loss)]
    // `SkIntToScalar` of the C++ test, which rounds the same way.
    let big_offset = (i32::MAX as f32) * 2.0 / 3.0;

    let filter = image_filters::blend(
        BlendMode::SrcOver,
        image_filters::offset((-big_offset, -big_offset), None, None),
        image_filters::offset((big_offset, big_offset), None, None),
        None,
    )
    .expect("a blend filter");
    let bounds_forward =
        filter.filter_bounds(&src, &Matrix::new_identity(), MapDirection::Forward, None);
    // NOTE: isEmpty() will return true even if the l/r or t/b didn't overflow but the dimensions
    // would overflow an int32. However, when isEmpty64() is false, it means the actual edge coords
    // are valid, which is good enough for our purposes.
    reporter_assert!(reporter, !bounds_forward.is_empty_64());

    // When querying with unbounded input content, it should not overflow and should not be empty.
    let bounds_reverse =
        filter.filter_bounds(&src, &Matrix::new_identity(), MapDirection::Reverse, None);
    reporter_assert!(reporter, !bounds_reverse.is_empty_64());

    // However in this case, when 'src' is also passed as the content bounds, the ::Offset() filters
    // detect that they would be transparent black. This propagates up to the src-over blend and
    // the entire graph is identified as empty.
    let bounds_reverse = filter.filter_bounds(
        &src,
        &Matrix::new_identity(),
        MapDirection::Reverse,
        Some(&src),
    );
    reporter_assert!(reporter, bounds_reverse.is_empty_64());
});

// Port of: tests/ImageFilterTest.cpp#L1090-L1109 (chrome/m156)
def_tier_test!(ImageFilterUnionBounds, |reporter| {
    let offset = image_filters::offset((50.0, 0.0), None, None);
    // Regardless of which order they appear in, the image filter bounds should
    // be combined correctly.
    {
        let composite = image_filters::blend(BlendMode::SrcOver, offset.clone(), None, None)
            .expect("a blend filter");
        // Intentionally aliasing here, as that's what the real callers do.
        let bounds = composite.compute_fast_bounds(Rect::from_iwh(100, 100));
        reporter_assert!(reporter, bounds == Rect::from_iwh(150, 100));
    }
    {
        let composite = image_filters::blend(BlendMode::SrcOver, None, offset.clone(), None)
            .expect("a blend filter");
        let bounds = composite.compute_fast_bounds(Rect::from_iwh(100, 100));
        reporter_assert!(reporter, bounds == Rect::from_iwh(150, 100));
    }
});

// Port of: tests/ImageFilterTest.cpp#L1111-L1129 (chrome/m156)
def_tier_test!(ImageFilterMergeResultSize, |reporter| {
    let mut green_bm = Bitmap::new();
    green_bm.alloc_n32_pixels((20, 20), None);
    green_bm.erase_color(Color::GREEN);
    let green_image = green_bm.as_image().expect("an image");
    let source = image_filters::image_sampled(Some(green_image), SamplingOptions::default());
    let merge = image_filters::merge(&[source.clone(), source], None).expect("a merge filter");
    let src_img = create_empty_special_image(1, Color::TRANSPARENT);
    let ctx = make_context_wh(100, 100, src_img);
    let (result_img, _offset) = merge.as_base().filter_image(&ctx).image_and_offset(&ctx);
    reporter_assert!(reporter, result_img.is_some());
    let result_img = result_img.expect("a result image");
    reporter_assert!(
        reporter,
        result_img.width() == 20 && result_img.height() == 20
    );
});

// Port of: tests/ImageFilterTest.cpp#L2539-L2549 (chrome/m156)
def_tier_test!(DropShadowImageFilter_Huge, |reporter| {
    // Successful if it doesn't crash or trigger ASAN. (crbug.com/1264705)
    let mut surf = surfaces::raster(&ImageInfo::new_n32_premul((300, 150), None), None, None)
        .expect("a raster surface");

    let mut paint = Paint::default();
    paint.set_image_filter(image_filters::drop_shadow_only(
        (0.0, 0.437_009),
        (14129.6, 14129.6),
        Color4f::from_color(Color::GRAY),
        None,
        None,
        None,
    ));

    let canvas = surf.canvas();
    canvas.save_layer(&SaveLayerRec::default().paint(&paint));
    canvas.restore();
    let _ = reporter;
});

// Port of: tests/ImageFilterTest.cpp#L2489-L2537 (chrome/m156)
def_tier_test!(PictureImageSourceBounds, |reporter| {
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(Rect::from_iwh(64, 64), false);

    let mut green_paint = Paint::default();
    green_paint.set_color(Color::GREEN);
    recording_canvas.draw_rect(
        Rect::from_irect(IRect::from_xywh(10, 10, 30, 20)),
        &green_paint,
    );
    let picture = recorder
        .finish_recording_as_picture(None)
        .expect("a picture");

    // Default target rect.
    let source1 = image_filters::picture(Some(picture.clone()), None);
    let picture_bounds = IRect::from_wh(64, 64);
    let input = IRect::from_xywh(10, 20, 30, 40);
    reporter_assert!(
        reporter,
        picture_bounds
            == source1.as_ref().expect("a picture filter").filter_bounds(
                &input,
                &Matrix::new_identity(),
                MapDirection::Forward,
                None
            )
    );
    reporter_assert!(
        reporter,
        source1
            .as_ref()
            .expect("a picture filter")
            .filter_bounds(
                &input,
                &Matrix::new_identity(),
                MapDirection::Reverse,
                Some(&input)
            )
            .is_empty()
    );
    let scale = Matrix::scale((2.0, 2.0));
    let scaled_picture_bounds = IRect::from_wh(128, 128);
    reporter_assert!(
        reporter,
        scaled_picture_bounds
            == source1.as_ref().expect("a picture filter").filter_bounds(
                &input,
                &scale,
                MapDirection::Forward,
                None
            )
    );
    reporter_assert!(
        reporter,
        source1
            .as_ref()
            .expect("a picture filter")
            .filter_bounds(&input, &scale, MapDirection::Reverse, Some(&input))
            .is_empty()
    );

    // Specified target rect.
    let mut target_rect = Rect::from_xywh(9.5, 9.5, 31.0, 21.0);
    let source2 = image_filters::picture(Some(picture.clone()), Some(&target_rect));
    reporter_assert!(
        reporter,
        <Rect as RoundOut<IRect>>::round_out(&target_rect)
            == source2.as_ref().expect("a picture filter").filter_bounds(
                &input,
                &Matrix::new_identity(),
                MapDirection::Forward,
                None
            )
    );
    reporter_assert!(
        reporter,
        source2
            .as_ref()
            .expect("a picture filter")
            .filter_bounds(
                &input,
                &Matrix::new_identity(),
                MapDirection::Reverse,
                Some(&input)
            )
            .is_empty()
    );
    target_rect = scale.map_rect(target_rect).0;
    reporter_assert!(
        reporter,
        <Rect as RoundOut<IRect>>::round_out(&target_rect)
            == source2.as_ref().expect("a picture filter").filter_bounds(
                &input,
                &scale,
                MapDirection::Forward,
                None
            )
    );
    reporter_assert!(
        reporter,
        source2
            .as_ref()
            .expect("a picture filter")
            .filter_bounds(&input, &scale, MapDirection::Reverse, Some(&input))
            .is_empty()
    );
});

// Port of: tests/ImageFilterTest.cpp#L1345-L1375 (chrome/m156)
def_tier_test!(ImageFilterClippedPictureImageFilter, |reporter| {
    let picture = {
        let mut recorder = PictureRecorder::new();
        let recording_canvas = recorder.begin_recording(Rect::from_iwh(1, 1), true);

        // Create an SkPicture which simply draws a green 1x1 rectangle.
        let mut green_paint = Paint::default();
        green_paint.set_color(Color::GREEN);
        recording_canvas.draw_rect(Rect::from_irect(IRect::from_wh(1, 1)), &green_paint);
        recorder.finish_recording_as_picture(None)
    };

    let src_img = create_empty_special_image(2, Color::TRANSPARENT);

    let image_filter = image_filters::picture(picture, None).expect("a picture filter");

    let ctx = make_context(IRect::from_xywh(1, 1, 1, 1), src_img);
    let (result_image, _offset) = image_filter
        .as_base()
        .filter_image(&ctx)
        .image_and_offset(&ctx);
    reporter_assert!(reporter, result_image.is_none());
});

// Port of: tests/ImageFilterTest.cpp#L2571-L2592 (chrome/m156)
def_tier_test!(
    ImageFilter_DrawExtremeMatrixTransform_DoesNotAssert,
    |reporter| {
        // Found by fuzzing
        let mut p = Paint::default();
        p.set_dither(true);
        p.set_color(Color::from_argb(255, 1, 255, 255));
        p.set_style(Style::Fill);

        let rr = RRect::new_rect_xy(Rect::new(5.0, 10.0, 15.0, 20.0), 2.0, 2.0);

        let blur = image_filters::blur(
            f32::from_bits(0x0e0e_0e0e),
            f32::from_bits(0x1010_8000),
            TileMode::Decal,
            None,
            None,
        );
        let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);
        let matrix = Matrix::new_all(
            f32::from_bits(0xfdfe_0200),
            f32::from_bits(0xfdfd_fdfd),
            f32::from_bits(0x2a02_02fe),
            f32::from_bits(0x0202_0202),
            f32::from_bits(0x0202_0202),
            f32::from_bits(0x2020_0202),
            f32::from_bits(0x2fab_0024),
            f32::from_bits(0x0000_0008),
            f32::from_bits(0x0000_0000),
        );
        let matrix_filter = image_filters::matrix_transform(&matrix, sampling, None);
        let shader_filter = image_filters::shader(None, image_filters::Dither::No, None);
        let merged = image_filters::merge(&[None, blur, matrix_filter, shader_filter], None);
        p.set_image_filter(merged);

        let mut surf = surfaces::raster(&ImageInfo::new_n32_premul((128, 160), None), None, None)
            .expect("a raster surface");
        let canvas = surf.canvas();
        canvas.clear(Color::WHITE);
        canvas.draw_rrect(rr, &p);
        let _ = reporter;
    }
);

// Port of: tests/ImageFilterTest.cpp#L379-L389 (chrome/m156)
fn make_grayscale(input: Option<ImageFilter>, crop_rect: Option<Rect>) -> Option<ImageFilter> {
    let mut matrix = [0.0f32; 20];
    matrix[0] = 0.2126;
    matrix[5] = 0.2126;
    matrix[10] = 0.2126;
    matrix[1] = 0.7152;
    matrix[6] = 0.7152;
    matrix[11] = 0.7152;
    matrix[2] = 0.0722;
    matrix[7] = 0.0722;
    matrix[12] = 0.0722;
    matrix[18] = 1.0;
    let filter = color_filters::matrix_row_major(&matrix, color_filters::Clamp::Yes);
    image_filters::color_filter(filter, input, crop_rect)
}

/// `makeWithFilter(src, filter, subset, clipBounds, &outSubset, &offset)`: the result, its
/// subset and offset (the out-parameters are the returned tuple).
type MakeWithFilterFn<'a> =
    dyn Fn(&Image, &ImageFilter, &IRect, &IRect) -> Option<(Image, IRect, IPoint)> + 'a;

/// `createSurface(width, height)` followed by the drawing of `draw` into its canvas and
/// `makeImageSnapshot()`.
type CreateSnapshotFn<'a> = dyn Fn(i32, i32, &dyn Fn(&Canvas)) -> Option<Image> + 'a;

// Port of: tests/ImageFilterTest.cpp#L1839-L1911 (chrome/m156)
fn test_make_with_filter(
    reporter: &mut Reporter,
    create_snapshot: &CreateSnapshotFn<'_>,
    make_with_filter: &MakeWithFilterFn<'_>,
) {
    let mut subset = IRect::from_xywh(25, 20, 50, 50);
    let source_image = create_snapshot(192, 128, &|canvas| {
        canvas.clear(Color::RED);
        let mut blue_paint = Paint::default();
        blue_paint.set_color(Color::BLUE);
        canvas.draw_rect(Rect::from_irect(subset), &blue_paint);
    });
    reporter_assert!(reporter, source_image.is_some());
    let Some(source_image) = source_image else {
        return;
    };

    let mut filter = make_grayscale(None, None).expect("a grayscale filter");
    let mut clip_bounds = IRect::from_xywh(30, 35, 100, 100);

    // (The "filter is required", "outSubset is required" and "offset is required" cases cannot
    // be expressed: the filter is a reference and the out-parameters are the returned tuple.)

    let big_subset = IRect::from_xywh(-10000, -10000, 20000, 20000);
    let result = make_with_filter(&source_image, &filter, &big_subset, &clip_bounds);
    reporter_assert!(reporter, result.is_none()); // subset needs to be w/in source's bounds

    let k_empty = IRect::new_empty();
    let result = make_with_filter(&source_image, &filter, &k_empty, &clip_bounds);
    reporter_assert!(reporter, result.is_none()); // subset can't be empty

    let result = make_with_filter(&source_image, &filter, &subset, &k_empty);
    reporter_assert!(reporter, result.is_none()); // clipBounds can't be empty

    let k_left_field = IRect::from_xywh(-1000, 0, 100, 100);
    let result = make_with_filter(&source_image, &filter, &subset, &k_left_field);
    reporter_assert!(reporter, result.is_none());

    let result = make_with_filter(&source_image, &filter, &subset, &clip_bounds);

    reporter_assert!(reporter, result.is_some());
    if let Some((result, out_subset, offset)) = &result {
        reporter_assert!(reporter, result.bounds().contains(out_subset));
        let dest_rect =
            IRect::from_xywh(offset.x, offset.y, out_subset.width(), out_subset.height());
        reporter_assert!(reporter, clip_bounds.contains(&dest_rect));
    }

    // In GPU-mode, this case creates a special image with a backing size that differs from
    // the content size
    {
        clip_bounds = IRect::from_xywh(0, 0, 170, 100);
        subset = IRect::from_xywh(0, 0, 160, 90);

        filter = image_filters::blend(BlendMode::SrcOver, None, None, None).expect("a blend");
        let result = make_with_filter(&source_image, &filter, &subset, &clip_bounds);
        reporter_assert!(reporter, result.is_some());

        // (The Ganesh origin check does not apply.)
    }
}

// Port of: tests/ImageFilterTest.cpp#L1915-L1936 (chrome/m156)
def_test!(ImageFilterMakeWithFilter, |reporter| {
    let create_raster_snapshot = |width: i32, height: i32, draw: &dyn Fn(&Canvas)| {
        let info = ImageInfo::new_n32((width, height), AlphaType::Opaque, None);
        let mut surface = surfaces::raster(&info, None, None)?;
        draw(surface.canvas());
        surface.image_snapshot()
    };

    let raster = |src: &Image, filter: &ImageFilter, subset: &IRect, clip_bounds: &IRect| {
        skia_rust_raster::images::make_with_filter(src, filter, subset, clip_bounds)
    };

    test_make_with_filter(reporter, &create_raster_snapshot, &raster);
});

// Port of: tests/ImageFilterTest.cpp#L1972-L2000 (chrome/m156)
def_graphite_test_for_all_contexts!(ImageFilterMakeWithFilter_Graphite, |reporter, context| {
    let recorder = context.make_recorder(None);

    let create_graphite_snapshot = |width: i32, height: i32, draw: &dyn Fn(&Canvas)| {
        let info = ImageInfo::new_n32((width, height), AlphaType::Premul, None);
        let surface = GraphiteSurface::render_target(&recorder, &info, Mipmapped::No, None, "")?;
        draw(surface.canvas());
        // `makeImageSnapshot()` of a Graphite surface (`onNewImageSnapshot`).
        surface.make_image_copy(None, Mipmapped::No)
    };

    let graphite = |src: &Image, filter: &ImageFilter, subset: &IRect, clip_bounds: &IRect| {
        graphite_image_factories::make_with_filter(&recorder, src, filter, subset, clip_bounds)
    };

    test_make_with_filter(reporter, &create_graphite_snapshot, &graphite);
});

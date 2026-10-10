// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageFilterTest.cpp (chrome/m156)
//
// Only the tests whose filters are ported are here: the Offset, Merge, Blend, Image, DropShadow,
// Morphology (Dilate/Erode), MatrixConvolution and DisplacementMap filters and the raster backend.
// The lighting, magnifier, arithmetic and runtime-image filters, the Ganesh variants
// and the image-filter-cache tests are not ported yet.

// Only the tests whose filters are ported are here: the Offset, Merge, Blend (with the arithmetic
// blender), Image, DropShadow, Lighting and Magnifier filters and the raster backend. The
// morphology, displacement and matrix-convolution filters, the Ganesh variants, the
// blur-dependent bounds tests and the image-filter-cache tests are not ported yet.

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::canvas::{SaveLayerRec, SrcRectConstraint};
use skia_rust_core::color::{Color, Color4f, ColorChannel};
use skia_rust_core::color_filters;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::{ImageFilter, MapDirection};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping, irect_intersect_in_place};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::Contains;
use skia_rust_core::rect::{IRect, Rect, RoundOut, rect_priv};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::shaders;
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{self, Dither};
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::graphite_types::InsertRecordingInfo;
use skia_rust_gpu::graphite::image_factories as graphite_image_factories;
use skia_rust_gpu::graphite::resource_types::Shareable;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;
use skia_rust_raster::image_filter_backend::make_raster_backend;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

use crate::{
    Reporter, def_graphite_adapter_test, def_graphite_test_for_all_contexts, def_test,
    def_tier_test, reporter_assert,
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

/// `make_special_from_image` on the raster path: a special image of `subset` of `bitmap`.
// Port of: tests/ImageFilterTest.cpp#L583-L590 (chrome/m156)
fn make_special_from_bitmap(bitmap: &Bitmap, subset: IRect) -> Option<Arc<SpecialImage>> {
    SpecialImage::make_from_raster(&subset, bitmap, &SurfaceProps::default()).map(Arc::new)
}

/// `special_image_to_bitmap`: the pixels of `src` as an N32 bitmap.
// Port of: tests/ImageFilterTest.cpp#L568-L581 (chrome/m156)
fn special_image_to_bitmap(src: &SpecialImage) -> Option<Bitmap> {
    src.as_bitmap()
}

/// `make_drop_shadow(input)`: a blue shadow with a 100px offset and 10px sigma.
// Port of: tests/ImageFilterTest.cpp#L952-L954 (chrome/m156)
fn make_drop_shadow(input: Option<ImageFilter>) -> Option<ImageFilter> {
    image_filters::drop_shadow(
        (100.0, 100.0),
        (10.0, 10.0),
        Color4f::from(Color::BLUE),
        None,
        input,
        None,
    )
}

// Port of: tests/ImageFilterTest.cpp#L653-L723 (chrome/m156)
def_tier_test!(
    #[allow(clippy::similar_names)] // mirrors the C++ mirrorX / mirrorY names
    MorphologyFilterRadiusWithMirrorCTM,
    |reporter| {
        // Check that SkMorphologyImageFilter maps the radius correctly when the
        // CTM contains a mirroring transform.
        const WIDTH: i32 = 32;
        const HEIGHT: i32 = 32;
        const RADIUS: f32 = 8.0;

        let filter = image_filters::dilate((RADIUS, RADIUS), None, None).expect("a dilate filter");

        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels((WIDTH, HEIGHT), None);
        {
            let canvas = CoreCanvas::from_bitmap(&mut bitmap, None).expect("a canvas");
            canvas.clear(Color::TRANSPARENT);
            let mut paint = Paint::default();
            paint.set_color(Color::WHITE);
            canvas.draw_rect(Rect::from_xywh(8.0, 8.0, 16.0, 16.0), &paint);
        }
        let img_src = make_special_from_bitmap(&bitmap, IRect::from_wh(WIDTH, HEIGHT));

        let ctx = make_context_wh(32, 32, img_src);

        let (normal_result, _offset) = filter.as_base().filter_image(&ctx).image_and_offset(&ctx);
        reporter_assert!(reporter, normal_result.is_some());

        let mut mirror_x = M44::translate(0.0, 32.0, 0.0);
        mirror_x.pre_scale(1.0, -1.0);
        let mirror_x_ctx = ctx.with_new_mapping(Mapping::from_layer_matrix(&mirror_x));

        let (mirror_x_result, _offset) = filter
            .as_base()
            .filter_image(&mirror_x_ctx)
            .image_and_offset(&ctx);
        reporter_assert!(reporter, mirror_x_result.is_some());

        let mut mirror_y = M44::translate(32.0, 0.0, 0.0);
        mirror_y.pre_scale(-1.0, 1.0);
        let mirror_y_ctx = ctx.with_new_mapping(Mapping::from_layer_matrix(&mirror_y));

        let (mirror_y_result, _offset) = filter
            .as_base()
            .filter_image(&mirror_y_ctx)
            .image_and_offset(&ctx);
        reporter_assert!(reporter, mirror_y_result.is_some());

        let normal_bm = normal_result.as_deref().and_then(special_image_to_bitmap);
        let mirror_x_bm = mirror_x_result.as_deref().and_then(special_image_to_bitmap);
        let mirror_y_bm = mirror_y_result.as_deref().and_then(special_image_to_bitmap);
        reporter_assert!(reporter, normal_bm.is_some());
        reporter_assert!(reporter, mirror_x_bm.is_some());
        reporter_assert!(reporter, mirror_y_bm.is_some());
        if let (Some(normal_bm), Some(mirror_x_bm), Some(mirror_y_bm)) =
            (normal_bm, mirror_x_bm, mirror_y_bm)
        {
            // memcmp of each row: the rows must be identical, and the loop stops at the first
            // difference.
            let row = |bm: &Bitmap, y: i32| -> Vec<u32> {
                (0..bm.width()).map(|x| bm.get_addr32(x, y)).collect()
            };
            for y in 0..normal_bm.height() {
                let diffs = row(&normal_bm, y) != row(&mirror_x_bm, y);
                reporter_assert!(reporter, !diffs);
                if diffs {
                    break;
                }
                let diffs = row(&normal_bm, y) != row(&mirror_y_bm, y);
                reporter_assert!(reporter, !diffs);
                if diffs {
                    break;
                }
            }
        }
    }
);

// Port of: tests/ImageFilterTest.cpp#L1002-L1024 (chrome/m156)
def_tier_test!(ImageFilterDilateThenBlurBounds, |reporter| {
    let filter1 = image_filters::dilate((2.0, 2.0), None, None);
    let filter2 = make_drop_shadow(filter1).expect("a drop shadow filter");

    let content_bounds = IRect::from_xywh(0, 0, 100, 100);
    // For output, the [0,0,100,100] source is outset by dilate radius (2px) to [-2,-2,102,102].
    // This is then translated by 100px and outset by 30px for the drop shadow = [68,68,232,232].
    // Finally this is joined with the original dilate result to get [-2,-2,232,232].
    let expected_output_bounds = IRect::from_ltrb(-2, -2, 232, 232);
    let output_bounds = filter2.filter_bounds(
        &content_bounds,
        &Matrix::new_identity(),
        MapDirection::Forward,
        None,
    );
    reporter_assert!(reporter, output_bounds == expected_output_bounds);

    // For input, it should be able to restrict itself to the source content.
    let input_bounds = filter2.filter_bounds(
        &expected_output_bounds,
        &Matrix::new_identity(),
        MapDirection::Reverse,
        Some(&content_bounds),
    );
    reporter_assert!(reporter, input_bounds == content_bounds);
});

// Port of: tests/ImageFilterTest.cpp#L1208-L1230 (chrome/m156)
def_tier_test!(ImageFilterMatrixConvolution, |reporter| {
    let _ = reporter;
    // Check that a 1x3 filter does not cause a spurious assert.
    let kernel = [1.0_f32, 1.0, 1.0];
    let filter = image_filters::matrix_convolution(
        (1, 3),
        &kernel,
        1.0,
        0.0,
        (0, 0),
        TileMode::Repeat,
        false,
        None,
        None,
    );

    let (width, height) = (16, 16);
    let mut surf = surfaces::raster(
        &ImageInfo::new_n32_premul((width, height), None),
        None,
        None,
    )
    .expect("a raster surface");
    let canvas = surf.canvas();
    canvas.clear(Color::TRANSPARENT);

    let mut paint = Paint::default();
    paint.set_image_filter(filter);
    let rect = Rect::from_irect(IRect::from_wh(width, height));
    canvas.draw_rect(rect, &paint);
});

// Port of: tests/ImageFilterTest.cpp#L1232-L1265 (chrome/m156)
def_tier_test!(ImageFilterMatrixConvolutionBorder, |reporter| {
    let _ = reporter;
    // Check that a filter with borders outside the target bounds
    // does not crash.
    let kernel = [0.0_f32, 0.0, 0.0];
    let filter = image_filters::matrix_convolution(
        (3, 1),
        &kernel,
        1.0,
        0.0,
        (2, 0),
        TileMode::Clamp,
        true,
        None,
        None,
    );

    let (width, height) = (10, 10);
    let mut surf = surfaces::raster(
        &ImageInfo::new_n32_premul((width, height), None),
        None,
        None,
    )
    .expect("a raster surface");
    let canvas = surf.canvas();
    canvas.clear(Color::TRANSPARENT);

    let mut filter_paint = Paint::default();
    filter_paint.set_image_filter(filter);
    let bounds = Rect::from_iwh(1, 10);
    let rect = Rect::from_irect(IRect::from_wh(width, height));
    let rect_paint = Paint::default();
    canvas.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&filter_paint));
    canvas.draw_rect(rect, &rect_paint);
    canvas.restore();
});

// Port of: tests/ImageFilterTest.cpp#L1293-L1296 and #L1257-L1291 (chrome/m156)
def_tier_test!(ImageFilterMatrixConvolutionBigKernel, |reporter| {
    // Check that a kernel that is too big for the GPU still works
    #[rustfmt::skip]
    let identity_kernel = [
        0.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    ];
    let filter = image_filters::matrix_convolution(
        (7, 7),
        &identity_kernel,
        1.0,
        0.0,
        (0, 0),
        TileMode::Clamp,
        true,
        None,
        None,
    )
    .expect("a matrix convolution filter");

    let src_img = create_empty_special_image(100, Color::TRANSPARENT);
    let ctx = make_context_wh(100, 100, src_img);
    let (result_img, offset) = filter.as_base().filter_image(&ctx).image_and_offset(&ctx);
    reporter_assert!(reporter, result_img.is_some());
    // `SkToBool(rContext) == resultImg->isGaneshBacked()`: there is no Ganesh context here.
    if let Some(result_img) = result_img {
        reporter_assert!(reporter, !result_img.is_ganesh_backed());
        reporter_assert!(
            reporter,
            result_img.width() == 100 && result_img.height() == 100
        );
    }
    reporter_assert!(reporter, offset.x == 0 && offset.y == 0);
});

// Port of: tests/ImageFilterTest.cpp#L2400-L2425 (chrome/m156)
def_tier_test!(DisplacementMapBounds, |reporter| {
    let flood_bounds = IRect::from_xywh(20, 30, 10, 10);
    let flood = image_filters::shader(
        Some(shaders::color(Color::GREEN)),
        image_filters::Dither::No,
        Some(Rect::from_irect(flood_bounds)),
    );
    let tiling_bounds = IRect::from_xywh(0, 0, 200, 100);
    let tiling = image_filters::tile(
        &Rect::from_irect(flood_bounds),
        &Rect::from_irect(tiling_bounds),
        flood,
    );
    let displace = image_filters::displacement_map(
        (ColorChannel::R, ColorChannel::B),
        20.0,
        None,
        tiling,
        None,
    )
    .expect("a displacement map filter");

    // The filter graph rooted at 'displace' uses the dynamic source image for the displacement
    // component of ::DisplacementMap, modifying the color component produced by the ::Tile. The
    // output of the tiling filter will be 'tilingBounds', regardless of its input, so 'floodBounds'
    // has no effect on the output. Since 'tiling' doesn't reference any dynamic source image, it
    // also will not affect the required input bounds.
    let input = IRect::from_xywh(20, 30, 40, 50);

    // 'input' is the desired output, which directly constrains the displacement component in this
    // specific filter graph.
    let actual_input =
        displace.filter_bounds(&input, &Matrix::new_identity(), MapDirection::Reverse, None);
    reporter_assert!(reporter, input == actual_input);

    // 'input' is the content bounds, which don't affect output bounds because it's only referenced
    // by the displacement component and not the color component.
    let actual_output =
        displace.filter_bounds(&input, &Matrix::new_identity(), MapDirection::Forward, None);
    let mut expected_output = tiling_bounds;
    expected_output.outset((10, 10));
    reporter_assert!(reporter, expected_output == actual_output);
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

/// `test_arithmetic_bounds(reporter, k1, k2, k3, k4, background, foreground, crop, expected)`.
// Port of: tests/ImageFilterTest.cpp#L2330-L2342 (chrome/m156)
fn test_arithmetic_bounds(
    reporter: &mut crate::Reporter,
    k: [f32; 4],
    background: Option<ImageFilter>,
    foreground: Option<ImageFilter>,
    crop: Option<&IRect>,
    expected: IRect,
) {
    let arithmetic = image_filters::arithmetic(
        k[0],
        k[1],
        k[2],
        k[3],
        false,
        background,
        foreground,
        crop.map(|rect| Rect::from_irect(*rect)),
    )
    .expect("an arithmetic filter");
    // Use a very large input bounds so that the crop rects stored in 'background' and 'foreground'
    // aren't restricted.
    let src = rect_priv::make_i_large();
    let bounds =
        arithmetic.filter_bounds(&src, &Matrix::new_identity(), MapDirection::Forward, None);
    reporter_assert!(reporter, expected == bounds);
}

/// `test_arithmetic_combinations(reporter, v)`.
// Port of: tests/ImageFilterTest.cpp#L2344-L2391 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the long C++ case table, one call per case
fn test_arithmetic_combinations(reporter: &mut crate::Reporter, v: f32) {
    let bg_rect = IRect::from_xywh(0, 0, 100, 100);
    let fg_rect = IRect::from_xywh(50, 50, 100, 100);
    let background = || image_filters::crop(&Rect::from_irect(bg_rect), TileMode::Decal, None);
    let foreground = || image_filters::crop(&Rect::from_irect(fg_rect), TileMode::Decal, None);

    let union_rect = IRect::join(&bg_rect, &fg_rect);
    let mut intersection = bg_rect;
    irect_intersect_in_place(&mut intersection, &fg_rect);

    // Test with crop. When k4 is non-zero, the result is expected to be cropRect regardless of
    // inputs because the filter affects the whole crop area. When there is no crop rect, it should
    // report an effectively infinite output.
    let inf = rect_priv::make_i_large();
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, 0.0, 0.0],
        background(),
        foreground(),
        None,
        IRect::new_empty(),
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, 0.0, v],
        background(),
        foreground(),
        None,
        inf,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, v, 0.0],
        background(),
        foreground(),
        None,
        bg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, v, v],
        background(),
        foreground(),
        None,
        inf,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, 0.0, 0.0],
        background(),
        foreground(),
        None,
        fg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, 0.0, v],
        background(),
        foreground(),
        None,
        inf,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, v, 0.0],
        background(),
        foreground(),
        None,
        union_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, v, v],
        background(),
        foreground(),
        None,
        inf,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, 0.0, 0.0],
        background(),
        foreground(),
        None,
        intersection,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, 0.0, v],
        background(),
        foreground(),
        None,
        inf,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, v, 0.0],
        background(),
        foreground(),
        None,
        bg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, v, v],
        background(),
        foreground(),
        None,
        inf,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, 0.0, 0.0],
        background(),
        foreground(),
        None,
        fg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, 0.0, v],
        background(),
        foreground(),
        None,
        inf,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, v, 0.0],
        background(),
        foreground(),
        None,
        union_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, v, v],
        background(),
        foreground(),
        None,
        inf,
    );

    let crop_rect = IRect::from_xywh(-111, -222, 333, 444);
    let crop = Some(&crop_rect);
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, 0.0, 0.0],
        background(),
        foreground(),
        crop,
        IRect::new_empty(),
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, 0.0, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, v, 0.0],
        background(),
        foreground(),
        crop,
        bg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, 0.0, v, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, 0.0, 0.0],
        background(),
        foreground(),
        crop,
        fg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, 0.0, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, v, 0.0],
        background(),
        foreground(),
        crop,
        union_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [0.0, v, v, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, 0.0, 0.0],
        background(),
        foreground(),
        crop,
        intersection,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, 0.0, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, v, 0.0],
        background(),
        foreground(),
        crop,
        bg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, 0.0, v, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, 0.0, 0.0],
        background(),
        foreground(),
        crop,
        fg_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, 0.0, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, v, 0.0],
        background(),
        foreground(),
        crop,
        union_rect,
    );
    test_arithmetic_bounds(
        reporter,
        [v, v, v, v],
        background(),
        foreground(),
        crop,
        crop_rect,
    );
}

// Port of: tests/ImageFilterTest.cpp#L2394-L2398 (chrome/m156)
def_tier_test!(ArithmeticImageFilterBounds, |reporter| {
    // Test SkArithmeticImageFilter::filterBounds with different blending modes.
    test_arithmetic_combinations(reporter, 1.0);
    test_arithmetic_combinations(reporter, 0.5);
});

// Port of: tests/ImageFilterTest.cpp#L2603-L2644 (chrome/m156)
def_tier_test!(ImageFilter_UnboundedInputMagnifier_EdgeLeak, |reporter| {
    // Create an image filter graph that starts with unbounded/infinite input
    // (in this case a solid-color magenta shader) and ends with a magnifier
    // with a lens bounds that will be larger than the layer we draw to.
    let infinite_input =
        image_filters::shader(Some(shaders::color(Color::MAGENTA)), Dither::No, None);
    let magnifier = image_filters::magnifier(
        &Rect::from_wh(100.0, 100.0),
        2.5,
        2.0,
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        infinite_input,
        None,
    );

    let info = ImageInfo::new_n32_premul((120, 120), None);
    let mut surf = surfaces::raster(&info, None, None).expect("a raster surface");
    {
        let canvas = surf.canvas();

        canvas.clear(Color::BLACK);

        canvas.clip_rect(Rect::from_wh(40.0, 100.0), None, None); // small enough that zoom center is not visible

        let mut paint = Paint::default();
        paint.set_image_filter(magnifier);
        canvas.save_layer(&SaveLayerRec::default().paint(&paint));

        // Contents are irrelevant given they will be overwritten by infiniteInput
        canvas.clear(Color::GREEN);
        canvas.restore();
        canvas.restore();
    }

    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&info, None);
    {
        let mut dst = bm.peek_pixels_mut().expect("allocated");
        reporter_assert!(
            reporter,
            surf.read_pixels_to_pixmap(&mut dst, (0, 0)),
            "Unable to read pixels"
        );
    }

    // When the edge leaking bug manifested, the transparent black padding pixels around the
    // reduced visible input would get sampled and fill in the right edge of the clipped zoom.
    // This would appear as black in the final rendering instead of solid magenta.
    let mut leaked = false;
    for y in 10..90 {
        for x in 35..40 {
            let c = bm.get_color((x, y));
            // Any pixel that is not solid magenta came from the padding.
            if c != Color::MAGENTA {
                leaked = true;
                break;
            }
        }
    }

    reporter_assert!(reporter, !leaked, "Edge padding leaked by magnifier");
});

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
type CreateSnapshotFn<'a> = dyn Fn(i32, i32, &dyn Fn(&CoreCanvas)) -> Option<Image> + 'a;

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
    let create_raster_snapshot = |width: i32, height: i32, draw: &dyn Fn(&CoreCanvas)| {
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

    let create_graphite_snapshot = |width: i32, height: i32, draw: &dyn Fn(&CoreCanvas)| {
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

// See b/549741762 - Normal image filtering in Graphite creates TextureProxies that are instantiated
// with scratch resources. The SkImage returned from MakeWithFilter() cannot be scratch, since it
// is going to the client for likely reuse. If it is assigned a scratch texture, then later
// recordings may incorrectly overwrite the contents of the scratch texture.
// Port of: tests/ImageFilterTest.cpp#L2006-L2135 (chrome/m156)
def_graphite_adapter_test!(
    ImageFilterMakeWithFilter_ScratchReuse_Graphite,
    |reporter, context| {
        // (The C++ uses `ToolUtils::CreateTestingRecorderOptions()`, which only adds a testing image
        // provider. This test never looks up an image by id, so the default options are used.)
        let mut recorder = context.make_recorder(None);

        let info = ImageInfo::new((16, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let surface = GraphiteSurface::render_target(&recorder, &info, Mipmapped::No, None, "");
        reporter_assert!(reporter, surface.is_some());
        let Some(surface) = surface else {
            return;
        };

        let filter_cyan =
            image_filters::shader(Some(shaders::color(Color::CYAN)), Dither::No, None)
                .expect("a shader filter");
        let filter_magenta =
            image_filters::shader(Some(shaders::color(Color::MAGENTA)), Dither::No, None)
                .expect("a shader filter");

        // First, make the cyan image filter result, where we also draw the resulting SkImage in the
        // same Recorder that produced it. Before fixing b/549741762, this tricked the
        // ScratchResourceManager into instantiating it as a scratch texture.
        let cyan_filter_image;
        let cyan_subset;
        let cyan_offset;
        {
            // NOTE The source image is ignored for these solid-color image filters.
            let made = graphite_image_factories::make_with_filter(
                &recorder,
                &surface.as_image(),
                &filter_cyan,
                &surface.image_info().bounds(),
                &IRect::from_wh(8, 8),
            );
            reporter_assert!(reporter, made.is_some());
            let Some((image, subset, offset)) = made else {
                return;
            };
            cyan_filter_image = image;
            cyan_subset = subset;
            cyan_offset = offset;

            // Like other image factories returning Graphite-backed SkImages to a client, the
            // underlying TextureProxy should be instantiated upon return, be non-shareable, and
            // be non-budgeted.
            let graphite_image =
                skia_rust_gpu::graphite::image_graphite::Image::from_core(&cyan_filter_image);
            reporter_assert!(reporter, graphite_image.is_some());
            if let Some(graphite_image) = graphite_image {
                let proxy = graphite_image.texture_proxy_view().proxy();
                reporter_assert!(reporter, proxy.is_some_and(|p| p.is_instantiated()));
                if let Some(texture) = proxy.and_then(|p| p.ref_texture()) {
                    reporter_assert!(reporter, texture.base().budgeted() == Budgeted::No);
                    reporter_assert!(reporter, texture.base().shareable() == Shareable::No);
                }
            }

            // Now draw back to surface at (0,0), which should fill left half with cyan.
            surface.canvas().draw_image_rect_with_sampling_options(
                &cyan_filter_image,
                Some((&Rect::from_irect(cyan_subset), SrcRectConstraint::Strict)),
                Rect::from_irect(IRect::from_pt_size(cyan_offset, cyan_subset.size())),
                SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                &Paint::default(),
            );
            // Snap the Recording and insert it to prepare the resources used by MakeWithFilter()
            let recording = recorder.snap();
            reporter_assert!(reporter, recording.is_some());
            if let Some(mut recording) = recording {
                let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
            }
        }

        // Second, use the magenta image filter as part of a save layer to render into the right half
        // of the surface with a texture that definitely should be classified as scratch.
        {
            let canvas = surface.canvas();
            canvas.save();
            canvas.clip_irect(IRect::from_ltrb(8, 0, 16, 8), None);
            let mut layer_paint = Paint::default();
            layer_paint.set_image_filter(filter_magenta);
            canvas.save_layer(&SaveLayerRec::default().paint(&layer_paint));
            canvas.restore();
            canvas.restore();

            // Snap the Recording and insert it to instantiate the layer's scratch texture. The layer's
            // scratch texture and the cyanFilterImage result were crafted to be the same size. If the
            // latter was incorrectly instantiated as a scratch texture, its contents will be replaced
            // with magenta. Future use of cyanFilterImage will then draw incorrectly.
            let recording = recorder.snap();
            reporter_assert!(reporter, recording.is_some());
            if let Some(mut recording) = recording {
                let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
            }
        }

        // Redraw the cyan image on both the left and right-hand side. If everything works correctly,
        // this should be a no-op for the left side and replace the RHS with cyan to leave `surface`
        // completely cyan. If the image was corrupted, the LHS will turn magenta instead.
        // (and if it's neither solid cyan (correct) or solid magenta (expected corruption), then
        //  something extra bad is happening!)
        {
            let canvas = surface.canvas();
            canvas.save();
            let dst = Rect::from_irect(IRect::from_pt_size(cyan_offset, cyan_subset.size()));
            canvas.draw_image_rect_with_sampling_options(
                &cyan_filter_image,
                Some((&Rect::from_irect(cyan_subset), SrcRectConstraint::Strict)),
                dst,
                SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                &Paint::default(),
            );
            canvas.translate((8, 0));
            canvas.draw_image_rect_with_sampling_options(
                &cyan_filter_image,
                Some((&Rect::from_irect(cyan_subset), SrcRectConstraint::Strict)),
                dst,
                SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                &Paint::default(),
            );
            canvas.restore();

            // Insert the recording before doing the test readback
            let recording = recorder.snap();
            reporter_assert!(reporter, recording.is_some());
            if let Some(mut recording) = recording {
                let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
            }
        }

        // Readback and confirm the image is cyan
        let mut dst = Bitmap::new();
        dst.alloc_pixels_info(&surface.image_info().clone(), None);
        let Some(mut pm) = dst.peek_pixels_mut() else {
            return;
        };
        if !context.read_surface_pixels(&surface, &mut pm, 0, 0) {
            // Probably a protected context: the C++ logs and returns.
            return;
        }

        let mut all_cyan = true;
        let mut all_magenta = true;
        for y in 0..pm.height() {
            for x in 0..pm.width() {
                let c = pm.get_color((x, y));
                if c != Color::CYAN {
                    all_cyan = false;
                }
                if c != Color::MAGENTA {
                    all_magenta = false;
                }
            }
        }
        reporter_assert!(reporter, !(all_cyan && all_magenta));
        reporter_assert!(
            reporter,
            all_cyan,
            "MakeWithFilter image was corrupted, all magenta = {}",
            all_magenta
        );
    }
);

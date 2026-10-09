// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagemakewithfilter.cpp (chrome/m156)
//
// Only the `Strategy::kSaveLayer` (reference) GMs are ported here. The `kMakeWithFilter` GMs
// need `SkImages::MakeWithFilter`, which skia-rust does not have.

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::{draw_checkerboard, get_resource_as_image};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{SaveLayerRec, SrcRectConstraint};
use skia_rust_core::color::{Color4f, ColorChannel};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::degrees_to_radians;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{
    arithmetic, blend, blur, color_filter, dilate, displacement_map, drop_shadow, erode,
    image_sampled, matrix_convolution, matrix_transform, merge, offset, point_lit_diffuse,
    point_lit_specular, tile,
};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Factories for creating image filters, with or without a crop rect.
type FilterFactory = fn(Option<Image>, Option<Rect>) -> Option<ImageFilter>;

// Port of: gm/imagemakewithfilter.cpp#L65-L69 (chrome/m156), color_filter_factory
fn color_filter_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    // The color filter uses kSrcIn so that it respects the transparency introduced by clamping.
    let cf = color_filters::blend(Color4f::from(Color::GREEN), None, BlendMode::SrcIn);
    color_filter(cf, None, crop)
}

// Port of: gm/imagemakewithfilter.cpp#L71-L73 (chrome/m156), blur_filter_factory
fn blur_filter_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    blur(2.0, 2.0, TileMode::Decal, None, crop)
}

// Port of: gm/imagemakewithfilter.cpp#L75-L77 (chrome/m156), drop_shadow_factory
fn drop_shadow_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    drop_shadow(
        (10.0, 5.0),
        (3.0, 3.0),
        Color4f::from(Color::BLUE),
        None,
        None,
        crop,
    )
}

// Port of: gm/imagemakewithfilter.cpp#L79-L81 (chrome/m156), offset_factory
fn offset_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    offset((10.0, 5.0), None, crop)
}

// Port of: gm/imagemakewithfilter.cpp#L83-L85 (chrome/m156), dilate_factory
fn dilate_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    dilate((10.0, 5.0), None, crop)
}

// Port of: gm/imagemakewithfilter.cpp#L87-L89 (chrome/m156), erode_factory
fn erode_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    erode((10.0, 5.0), None, crop)
}

// Port of: gm/imagemakewithfilter.cpp#L91-L95 (chrome/m156), displacement_factory
fn displacement_factory(aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    let displacement = image_sampled(aux, SamplingOptions::from(FilterMode::Linear));
    displacement_map(
        (ColorChannel::R, ColorChannel::G),
        40.0,
        displacement,
        None,
        crop,
    )
}

// Port of: gm/imagemakewithfilter.cpp#L97-L102 (chrome/m156), arithmetic_factory
fn arithmetic_factory(aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    let background = image_sampled(aux, SamplingOptions::from(FilterMode::Linear));
    arithmetic(0.0, 0.6, 1.0, 0.0, false, background, None, crop)
}

// Port of: gm/imagemakewithfilter.cpp#L104-L109 (chrome/m156), blend_factory
fn blend_factory(aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    let background = image_sampled(aux, SamplingOptions::from(FilterMode::Linear));
    blend(BlendMode::Modulate, background, None, crop)
}

// Port of: gm/imagemakewithfilter.cpp#L111-L120 (chrome/m156), convolution_factory
fn convolution_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    // A Laplacian edge detector, ee https://en.wikipedia.org/wiki/Kernel_(image_processing)
    #[rustfmt::skip]
    let kernel: [f32; 9] = [
        -1.0, -1.0, -1.0,
        -1.0,  8.0, -1.0,
        -1.0, -1.0, -1.0,
    ];
    matrix_convolution(
        (3, 3),
        &kernel,
        1.0,
        0.0,
        (1, 1),
        TileMode::Clamp,
        false,
        None,
        crop,
    )
}

// Port of: gm/imagemakewithfilter.cpp#L122-L127 (chrome/m156), matrix_factory
fn matrix_factory(_aux: Option<Image>, _crop: Option<Rect>) -> Option<ImageFilter> {
    let mut matrix = Matrix::new_identity();
    matrix.set_rotate(45.0, Point::new(50.0, 50.0));
    // This doesn't support a cropRect
    matrix_transform(&matrix, SamplingOptions::from(FilterMode::Linear), None)
}

// Port of: gm/imagemakewithfilter.cpp#L129-L154 (chrome/m156), lighting_factory
fn lighting_factory(_aux: Option<Image>, crop: Option<Rect>) -> Option<ImageFilter> {
    // Must convert the RGB values of the source to alpha, since that is what the lighting filters
    // use to estimate their normals. This color matrix changes the color to white and the alpha
    // to be equal to the approx. luminance of the original color.
    #[rustfmt::skip]
    let k_matrix: [f32; 20] = [
        0.0, 0.0, 0.0, 0.0, 1.0,
        0.0, 0.0, 0.0, 0.0, 1.0,
        0.0, 0.0, 0.0, 0.0, 1.0,
        0.2126, 0.7152, 0.0722, 0.0, 0.0,
    ];
    let cf = color_filters::matrix_row_major(&k_matrix, Clamp::Yes);
    let src_to_alpha = color_filter(cf, None, None);

    // Combine both specular and diffuse into a single DAG since they use separate internal filter
    // implementations.
    let sin_azimuth = degrees_to_radians(225.0).sin();
    let cos_azimuth = degrees_to_radians(225.0).cos();
    let spot_target = Point3::new(40.0, 40.0, 0.0);
    let diff_location = Point3::new(
        spot_target.x + 50.0 * cos_azimuth,
        spot_target.y + 50.0 * sin_azimuth,
        10.0,
    );
    let spec_location = Point3::new(
        spot_target.x - 50.0 * sin_azimuth,
        spot_target.y + 50.0 * cos_azimuth,
        10.0,
    );
    let diffuse = point_lit_diffuse(
        diff_location,
        Color::WHITE,
        /* scale */ 1.0,
        /* kd */ 2.0,
        src_to_alpha.clone(),
        crop,
    );
    let specular = point_lit_specular(
        spec_location,
        Color::RED,
        /* scale */ 1.0,
        /* ks */ 1.0,
        /* shine */ 8.0,
        src_to_alpha,
        crop,
    );
    merge(&[diffuse, specular], crop)
}

// Port of: gm/imagemakewithfilter.cpp#L156-L160 (chrome/m156), tile_factory
fn tile_factory(_aux: Option<Image>, _crop: Option<Rect>) -> Option<ImageFilter> {
    // Tile the subset over a large region
    tile(
        &Rect::from_ltrb(25.0, 25.0, 75.0, 75.0),
        &Rect::from_wh(100.0, 100.0),
        None,
    )
}

// Port of: gm/imagemakewithfilter.cpp#L33-L49 (chrome/m156), show_bounds
fn show_bounds(
    canvas: &Canvas,
    clip: Option<IRect>,
    in_subset: Option<IRect>,
    out_subset: Option<IRect>,
) {
    let rects = [clip, in_subset, out_subset];
    let colors = [Color::BLUE, Color::YELLOW, Color::RED];
    let mut paint = Paint::default();
    paint.set_style(skia_rust_core::paint::Style::Stroke);
    for (i, rect) in rects.iter().enumerate() {
        // Skip null bounds rects, since not all methods have subsets
        if let Some(rect) = rect {
            paint.set_color(colors[i]);
            canvas.draw_rect(Rect::from(*rect), &paint);
        }
    }
}

// Port of: gm/imagemakewithfilter.cpp#L183-L199 (chrome/m156), ImageMakeWithFilterGM (kSaveLayer)
struct ImageMakeWithFilterRefGm {
    filter_with_crop_rect: bool,
    main_image: Option<Image>,
    aux_image: Option<Image>,
}

impl ImageMakeWithFilterRefGm {
    // Port of: gm/imagemakewithfilter.cpp#L316-L345 (chrome/m156), drawImageWithFilter (kSaveLayer)
    fn draw_image_with_filter(
        &self,
        canvas: &Canvas,
        main_image: &Image,
        aux_image: &Image,
        filter_factory: FilterFactory,
        clip: IRect,
        subset: IRect,
    ) {
        // When creating the filter with a crop rect equal to the clip, we should expect to see no
        // difference from a filter without a crop rect.
        let crop = if self.filter_with_crop_rect {
            Some(Rect::from(clip))
        } else {
            None
        };
        let filter = filter_factory(Some(aux_image.clone()), crop);

        // SkAutoCanvasRestore acr(canvas, true)
        canvas.save();
        // Clip before the saveLayer with the filter
        canvas.clip_rect(Rect::from(clip), None, None);
        // Put the image filter on the layer
        let mut paint = Paint::default();
        paint.set_image_filter(filter);
        canvas.save_layer(&SaveLayerRec::default().paint(&paint));
        // Draw the original subset of the image
        let r = Rect::from(subset);
        canvas.draw_image_rect_with_sampling_options(
            main_image,
            Some((&r, SrcRectConstraint::Strict)),
            r,
            SamplingOptions::default(),
            &Paint::default(),
        );
        canvas.restore();
        canvas.restore();
    }
}

impl GM for ImageMakeWithFilterRefGm {
    fn name(&self) -> String {
        let mut name = String::from("imagemakewithfilter");
        if self.filter_with_crop_rect {
            name.push_str("_crop");
        }
        name.push_str("_ref");
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(1840, 860)
    }

    // Port of: gm/imagemakewithfilter.cpp#L203-L218 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let info = ImageInfo::new((100, 100), ColorType::N32, AlphaType::Unpremul, None);
        let Some(mut surface) = skia_rust_raster::surfaces::raster(&info, None, None) else {
            return;
        };
        if let Some(color_image) = get_resource_as_image("images/mandrill_128.png") {
            // Resize to 100x100
            let src = Rect::from_wh(color_image.width() as f32, color_image.height() as f32);
            let dst = Rect::from_wh(100.0, 100.0);
            surface.canvas().draw_image_rect_with_sampling_options(
                &color_image,
                Some((&src, SrcRectConstraint::Strict)),
                dst,
                SamplingOptions::default(),
                &Paint::default(),
            );
        }
        self.main_image = surface.image_snapshot();
        draw_checkerboard(
            surface.canvas(),
            Color::from(0xFF99_9999),
            Color::from(0xFF66_6666),
            8,
        );
        self.aux_image = surface.image_snapshot();
    }

    // Port of: gm/imagemakewithfilter.cpp#L221-L300 (chrome/m156), onDraw (kSaveLayer path)
    fn on_draw(&mut self, canvas: &Canvas) {
        let (Some(main_image), Some(aux_image)) = (self.main_image.clone(), self.aux_image.clone())
        else {
            return;
        };
        let filters: [FilterFactory; 13] = [
            color_filter_factory,
            blur_filter_factory,
            drop_shadow_factory,
            offset_factory,
            dilate_factory,
            erode_factory,
            displacement_factory,
            arithmetic_factory,
            blend_factory,
            convolution_factory,
            matrix_factory,
            lighting_factory,
            tile_factory,
        ];
        let filter_names: [&str; 13] = [
            "Color",
            "Blur",
            "Drop Shadow",
            "Offset",
            "Dilate",
            "Erode",
            "Displacement",
            "Arithmetic",
            "Blend",
            "Convolution",
            "Matrix Xform",
            "Lighting",
            "Tile",
        ];
        let clip_bounds: [IRect; 6] = [
            IRect::from_ltrb(-20, -20, 100, 100),
            IRect::from_ltrb(0, 0, 75, 75),
            IRect::from_ltrb(20, 20, 100, 100),
            IRect::from_ltrb(-20, -20, 50, 50),
            IRect::from_ltrb(20, 20, 50, 50),
            IRect::from_ltrb(30, 30, 75, 75),
        ];

        let margin: f32 = 40.0;
        let dx = main_image.width() as f32 + margin;
        let dy = aux_image.height() as f32 + margin;

        // Header hinting at what the filters do
        let mut text_paint = Paint::default();
        text_paint.set_anti_alias(true);
        let mut font = default_portable_font();
        font.set_size(12.0);
        for (i, name) in filter_names.iter().enumerate() {
            canvas.draw_str(name, (dx * i as f32 + margin, 15.0), &font, &text_paint);
        }

        canvas.translate((margin, margin));
        for clip_bound in clip_bounds {
            canvas.save();
            for filter in filters {
                let subset = IRect::from_xywh(25, 25, 50, 50);
                // Draw the original image faintly so that it aids in checking alignment of the
                // filtered result.
                let mut alpha = Paint::default();
                alpha.set_alpha_f(0.3);
                canvas.draw_image_with_sampling_options(
                    &main_image,
                    (0.0, 0.0),
                    SamplingOptions::default(),
                    Some(&alpha),
                );
                self.draw_image_with_filter(
                    canvas,
                    &main_image,
                    &aux_image,
                    filter,
                    clip_bound,
                    subset,
                );
                // No output subset is displayed for kSaveLayer since that information isn't
                // available.
                show_bounds(canvas, Some(clip_bound), Some(subset), None);
                canvas.translate((dx, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, dy));
        }
    }
}

// Port of: gm/imagemakewithfilter.cpp#L421 (chrome/m156), DEF_GM( return new ImageMakeWithFilterGM(Strategy::kSaveLayer); )
crate::def_gm!(
    ImageMakeWithFilterGM_ = "ImageMakeWithFilterGM(Strategy::kSaveLayer)",
    ImageMakeWithFilterRefGm {
        filter_with_crop_rect: false,
        main_image: None,
        aux_image: None
    }
);

// Port of: gm/imagemakewithfilter.cpp#L429 (chrome/m156), DEF_GM( return new ImageMakeWithFilterGM(Strategy::kSaveLayer, true); )
crate::def_gm!(
    ImageMakeWithFilterGM_crop = "ImageMakeWithFilterGM(Strategy::kSaveLayer, true)",
    ImageMakeWithFilterRefGm {
        filter_with_crop_rect: true,
        main_image: None,
        aux_image: None
    }
);

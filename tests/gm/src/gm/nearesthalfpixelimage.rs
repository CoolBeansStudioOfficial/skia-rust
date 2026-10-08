// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/nearesthalfpixelimage.cpp (chrome/m156)

#![allow(clippy::similar_names)] // cpmx/cpmy, apmx/apmy, images: the C++ names

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/nearesthalfpixelimage.cpp#L22-L118 (chrome/m156)
crate::def_simple_gm_can_fail!(nearest_half_pixel_image, canvas, error_msg, 264, 235, {
    // We don't run this test on the GPU because we're at the driver/hw's mercy for how this
    // is handled.
    // (`canvas->recordingContext()` and the surface's recorder are null on a raster canvas.)

    // We scale up in the direction not being tested, the one with image dimension of 1, to
    // make the result more easily visible.
    const OFF_AXIS_SCALE: f32 = 4.0;

    // We make 2x1 and 1x2 images for each color type.
    struct Images {
        image_x: Image,
        image_y: Image,
    }

    let colors: [u32; 2] = [0xFFFF_0000, 0xFF00_00FF];
    let color_bytes: Vec<u8> = colors.iter().flat_map(|c| c.to_ne_bytes()).collect();
    let cpmx = Pixmap::new_readonly(
        &ImageInfo::new((2, 1), ColorType::RGBA8888, AlphaType::Premul, None),
        &color_bytes,
        8,
    )
    .expect("a pixmap");
    let cpmy = Pixmap::new_readonly(
        &ImageInfo::new((1, 2), ColorType::RGBA8888, AlphaType::Premul, None),
        &color_bytes,
        4,
    )
    .expect("a pixmap");
    let image0 = Images {
        image_x: images::raster_from_pixmap_copy(&cpmx).expect("an image"),
        image_y: images::raster_from_pixmap_copy(&cpmy).expect("an image"),
    };

    let alphas: [u8; 2] = [0xFF, 0xAA];
    let apmx = Pixmap::new_readonly(
        &ImageInfo::new((2, 1), ColorType::Alpha8, AlphaType::Premul, None),
        &alphas,
        2,
    )
    .expect("a pixmap");
    let apmy = Pixmap::new_readonly(
        &ImageInfo::new((1, 2), ColorType::Alpha8, AlphaType::Premul, None),
        &alphas,
        1,
    )
    .expect("a pixmap");
    let image1 = Images {
        image_x: images::raster_from_pixmap_copy(&apmx).expect("an image"),
        image_y: images::raster_from_pixmap_copy(&apmy).expect("an image"),
    };
    let images = [image0, image1];

    // We draw offscreen and then zoom that up to make the result clear.
    let Some(mut surf) = canvas.new_surface(&canvas.image_info().with_dimensions((80, 80)), None)
    else {
        *error_msg = "Test only works with SkSurface backed canvases".to_string();
        return DrawResult::Skip;
    };
    {
        let c = surf.canvas();
        c.clear(Color::WHITE);

        let draw = |image: &Image, shader: bool, do_x: bool, mirror: bool, alpha: u8| {
            c.save();
            let mut paint = Paint::default();
            paint.set_alpha(alpha);
            if shader {
                paint.set_shader(image.to_shader(None, FilterMode::Nearest, None));
            }
            if do_x {
                c.scale((if mirror { -1.0 } else { 1.0 }, OFF_AXIS_SCALE));
                c.translate((if mirror { -2.5 } else { 0.5 }, 0.0));
            } else {
                c.scale((OFF_AXIS_SCALE, if mirror { -1.0 } else { 1.0 }));
                c.translate((0.0, if mirror { -2.5 } else { 0.5 }));
            }

            if shader {
                c.draw_rect(Rect::from_isize(image.dimensions()), &paint);
            } else {
                c.draw_image_with_sampling_options(
                    image,
                    (0.0, 0.0),
                    FilterMode::Nearest,
                    Some(&paint),
                );
            }
            c.restore();
        };

        for shader in [false, true] {
            for alpha in [0xFF_u8, 0x70] {
                c.save();
                for i in &images {
                    for mirror in [false, true] {
                        draw(&i.image_x, shader, /*do_x=*/ true, mirror, alpha);
                        c.save();
                        c.translate((4.0, 0.0));
                        draw(&i.image_y, shader, /*do_x=*/ false, mirror, alpha);
                        c.restore();
                        c.translate((0.0, OFF_AXIS_SCALE * 2.0));
                    }
                }
                c.restore();
                c.translate((OFF_AXIS_SCALE * 2.0, 0.0));
            }
        }
    }
    canvas.scale((8.0, 8.0));
    canvas.draw_image(surf.image_snapshot().expect("a snapshot"), (0.0, 0.0), None);

    DrawResult::Ok
});

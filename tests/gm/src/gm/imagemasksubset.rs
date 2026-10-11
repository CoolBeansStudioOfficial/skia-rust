// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagemasksubset.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{draw_checkerboard, int_to_scalar, make_texture_image};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_generator::ImageGenerator;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_raster::surfaces;

// Port of: gm/imagemasksubset.cpp#L17-L20 (chrome/m156), kSize, kSubset and kDest
const K_SIZE: ISize = ISize {
    width: 100,
    height: 100,
};
const K_SUBSET: IRect = IRect {
    left: 25,
    top: 25,
    right: 75,
    bottom: 75,
};
const K_DEST: Rect = Rect {
    left: 10.0,
    top: 10.0,
    right: 110.0,
    bottom: 110.0,
};

// Port of: gm/imagemasksubset.cpp#L22-L26 (chrome/m156), make_mask
fn make_mask(surface: &mut skia_rust_raster::surface::Surface<'_>) -> Option<Image> {
    draw_checkerboard(surface.canvas(), Color::new(0x8080_8080), Color::new(0), 5);
    surface.image_snapshot()
}

// Port of: gm/imagemasksubset.cpp#L28-L43 (chrome/m156), MaskGenerator
struct MaskGenerator {
    info: ImageInfo,
}

impl ImageGenerator for MaskGenerator {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        // SkNextID::ImageID(): any id that is unique to this generator.
        0
    }

    // Port of: gm/imagemasksubset.cpp#L33-L41 (chrome/m156), onGetPixels
    fn on_get_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        // `if (kAlpha_8_SkColorType == info.colorType()) surfaceInfo.makeColorSpace(nullptr)`:
        // an alpha-only image has no color space, so the info is used as it is.
        let Some(mut surface) = surfaces::wrap_pixels(info, pixels, row_bytes, None) else {
            return false;
        };
        make_mask(&mut surface);
        true
    }
}

// Port of: gm/imagemasksubset.cpp#L53-L57 (chrome/m156), the SkImage_Raster maker
fn make_raster_mask(info: &ImageInfo) -> Option<Image> {
    let mut surface = surfaces::raster(info, None, None)?;
    make_mask(&mut surface)
}

// Port of: gm/imagemasksubset.cpp#L58-L69 (chrome/m156), the SkImage_Ganesh maker; with no
// recording context or recorder on a raster canvas it falls back to a raster surface.
fn make_ganesh_mask(info: &ImageInfo) -> Option<Image> {
    make_raster_mask(info)
}

// Port of: gm/imagemasksubset.cpp#L70-L73 (chrome/m156), the SkImage_Lazy maker
fn make_lazy_mask(info: &ImageInfo) -> Option<Image> {
    images::deferred_from_generator(Some(Box::new(MaskGenerator { info: info.clone() })))
}

// Checks whether subset SkImages preserve the original color type (A8 in this case).
// Port of: gm/imagemasksubset.cpp#L78-L101 (chrome/m156), imagemasksubset
crate::def_simple_gm!(imagemasksubset, canvas, 480, 480, {
    let mut paint = Paint::default();
    paint.set_color(Color::new(0xff00_ff00));
    let info = ImageInfo::new(K_SIZE, ColorType::Alpha8, AlphaType::Premul, None);
    let makers: [fn(&ImageInfo) -> Option<Image>; 3] =
        [make_raster_mask, make_ganesh_mask, make_lazy_mask];
    for maker in makers {
        let image = make_texture_image(canvas, maker(&info));
        if let Some(image) = image {
            canvas.draw_image_rect_with_sampling_options(
                &image,
                Some((&Rect::from_irect(K_SUBSET), SrcRectConstraint::Strict)),
                K_DEST,
                SamplingOptions::default(),
                &paint,
            );
            let subset = image.make_subset(K_SUBSET, RequiredProperties::default());
            if let Some(subset) = subset {
                canvas.draw_image_rect_with_sampling_options(
                    &subset,
                    None,
                    K_DEST.with_offset((int_to_scalar(K_SIZE.width) * 1.5, 0.0)),
                    SamplingOptions::default(),
                    &paint,
                );
            }
        }
        canvas.translate((0.0, int_to_scalar(K_SIZE.height) * 1.5));
    }
});

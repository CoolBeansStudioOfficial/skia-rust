// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ImageFactories.cpp (the bitmap, raster, subset and texture
// factories; `TextureFromImage` and `SubsetTextureFrom`)

//! Image factories (`include/gpu/graphite/Image.h`): `TextureFromImage`, `SubsetTextureFrom`, and the
//! bitmap path they share (`make_from_bitmap`).
//!
//! Not ported here: the lazy-generator path of `TextureFromImage` (`make_texture_image_from_lazy`,
//! the generator and picture cases of `SkImage_Lazy`, which need the generator's texture
//! callbacks), the YUVA factories (G15), and `WrapTexture` and `PromiseTextureFrom` (G11a and
//! G15). `MakeWithFilter` is [`make_with_filter`].

use std::sync::Arc;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::image::{Image as CoreImage, RequiredProperties};
use skia_rust_core::image_base::{ImageBase, ImageType};
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::image_raster::ImageRaster;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::surface_props::SurfaceProps;

use crate::gpu::gpu_types::{Budgeted, Mipmapped};
use crate::graphite::image_filter_backend::make_graphite_backend;
use crate::graphite::image_graphite::{Image, make_non_budgeted, make_subset};
use crate::graphite::recorder::Recorder;
use crate::graphite::texture_utils::make_bitmap_proxy_view;

/// `make_from_bitmap(recorder, colorInfo, bitmap, mipmaps, budgeted, requiredProps, label)`: an
/// image over a texture the bitmap is uploaded to.
// Port of: src/gpu/graphite/ImageFactories.cpp#L445-L466 (chrome/m156)
#[must_use]
pub fn make_from_bitmap(
    recorder: &Recorder,
    color_info: &ColorInfo,
    bitmap: &Bitmap,
    mipmaps: Option<Arc<Mipmap>>,
    budgeted: Budgeted,
    required_props: RequiredProperties,
    label: &str,
) -> Option<CoreImage> {
    let mm = if required_props.mipmapped {
        Mipmapped::Yes
    } else {
        Mipmapped::No
    };
    let view = make_bitmap_proxy_view(recorder, bitmap, mipmaps, mm, budgeted, label)?;
    debug_assert!(!required_props.mipmapped || view.mipmapped() == Mipmapped::Yes);
    Some(Image::new(view, color_info).into_core())
}

/// `TextureFromImage(recorder, image, requiredProps)`: a Graphite-backed version of `image`. A
/// raster image is uploaded, a Graphite-backed image is returned as is (or copied to the required
/// subset), and the lazy path is not ported (see the module docs).
// Port of: src/gpu/graphite/ImageFactories.cpp#L508-L537 (chrome/m156)
#[doc(alias = "TextureFromImage")]
#[must_use]
pub fn texture_from_image(
    recorder: &Recorder,
    image: &CoreImage,
    mut required_props: RequiredProperties,
) -> Option<CoreImage> {
    if i64::from(image.width()) * i64::from(image.height()) <= 1 {
        required_props.mipmapped = false;
    }
    let ib = image.as_base();
    if ib.is_raster_backed() {
        let raster = ib.as_any().downcast_ref::<ImageRaster>()?;
        return make_from_bitmap(
            recorder,
            raster.info().color_info(),
            raster.bitmap(),
            ib.on_peek_mips().cloned(),
            Budgeted::No,
            required_props,
            "RasterBitmapTexture",
        );
    }
    if ib.is_lazy_generated() {
        return make_texture_image_from_lazy(recorder, image, required_props);
    }
    debug_assert_eq!(ib.image_type(), ImageType::Graphite);
    let bounds = IRect::from_size(image.dimensions());
    make_subset(recorder, image, bounds, required_props)
}

/// `make_texture_image_from_lazy(recorder, img, requiredProps)`: a lazy image's texture, from the
/// pixels its generator gives. A picture-backed image needs `SkImage_Picture`, which is not ported
/// (its replay into a surface is the `generate_picture_texture` path, not built here).
// Port of: src/gpu/graphite/ImageFactories.cpp#L468-L506 (chrome/m156), the bitmap branch
fn make_texture_image_from_lazy(
    recorder: &Recorder,
    image: &CoreImage,
    required_props: RequiredProperties,
) -> Option<CoreImage> {
    if image.as_base().image_type() == ImageType::LazyPicture {
        // `generate_picture_texture` replays the picture into a Surface (not ported with
        // `SkImage_Picture`).
        return None;
    }
    let bitmap = image.as_base().get_ro_pixels()?;
    make_from_bitmap(
        recorder,
        image.image_info().color_info(),
        &bitmap,
        None,
        Budgeted::No,
        required_props,
        "LazySkImageBitmapTexture",
    )
}

/// `SubsetTextureFrom(recorder, img, subset, requiredProps)`: a Graphite-backed copy of `subset` of
/// `img`.
// Port of: src/gpu/graphite/ImageFactories.cpp#L379-L388 (chrome/m156)
#[doc(alias = "SubsetTextureFrom")]
#[must_use]
pub fn subset_texture_from(
    recorder: &Recorder,
    img: &CoreImage,
    subset: IRect,
    required_props: RequiredProperties,
) -> Option<CoreImage> {
    let subset_img = if img.as_base().is_graphite_backed() {
        make_subset(recorder, img, subset, required_props)?
    } else {
        img.make_subset(subset, required_props)?
    };
    texture_from_image(recorder, &subset_img, required_props)
}

/// `MakeWithFilter(recorder, src, filter, subset, clipBounds, outSubset, offset)`: filters
/// `subset` of `src` on the GPU with the Graphite image filter backend. Returns the result image,
/// its subset that holds the result (`outSubset`) and the offset of that subset relative to `src`
/// (`offset`).
// Port of: src/gpu/graphite/ImageFactories.cpp#L390-L414 (chrome/m156)
#[doc(alias = "MakeWithFilter")]
#[must_use]
pub fn make_with_filter(
    recorder: &Recorder,
    src: &CoreImage,
    filter: &ImageFilter,
    subset: &IRect,
    clip_bounds: &IRect,
) -> Option<(CoreImage, IRect, IPoint)> {
    let backend = make_graphite_backend(recorder, &SurfaceProps::default(), src.color_type());
    let (image, out_subset, offset) =
        filter.make_image_with_filter(backend, src, subset, clip_bounds)?;
    // The skif backend creates budgeted, scratch textures. This is what we want most of the time,
    // but for the final result image returned from MakeWithFilter(), it needs to be a
    // non-budgeted non-shareable texture (i.e. matching what we return from the other factory
    // methods).
    debug_assert!(image.as_base().is_graphite_backed());
    let image = make_non_budgeted(recorder, &image)?;
    Some((image, out_subset, offset))
}

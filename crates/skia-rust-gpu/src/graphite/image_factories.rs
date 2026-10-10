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

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::ColorChannelFlag;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image as CoreImage, RequiredProperties};
use skia_rust_core::image_base::{ImageBase, ImageType};
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::image_info_priv::{color_info_is_valid, image_info_is_valid};
use skia_rust_core::image_raster::ImageRaster;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;

use crate::gpu::gpu_types::{Budgeted, Mipmapped, Origin};
use crate::gpu::ref_cnted_callback::{CallbackProc, RefCntedCallback};
use crate::gpu::sk_log::skia_log_w;
use crate::gpu::swizzle::Swizzle;
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::image_filter_backend::make_graphite_backend;
use crate::graphite::image_graphite::{Image, make_non_budgeted, make_subset};
use crate::graphite::caps::Caps;
use crate::graphite::recorder::Recorder;
use crate::graphite::texture::ReleaseCallback;
use crate::graphite::texture_format::{
    are_color_type_and_format_compatible, read_swizzle_for_color_type,
    texture_format_channel_mask, texture_format_color_type_info,
};
use crate::graphite::graphite_types::Volatile;
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::texture_utils::{
    PromiseTextureFulfillProc, make_bitmap_proxy_view, make_promise_image_lazy_proxy,
};

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

/// `validate_backend_texture(caps, texture, info)`: whether `texture` can back an image with `info`.
// Port of: src/gpu/graphite/ImageFactories.cpp#L50-L68 (chrome/m156)
fn validate_backend_texture(caps: &dyn Caps, texture: &BackendTexture, info: &ColorInfo) -> bool {
    let dimensions = texture.dimensions();
    if !texture.is_valid() || dimensions.width <= 0 || dimensions.height <= 0 {
        return false;
    }

    if !color_info_is_valid(info) {
        return false;
    }

    if !caps.is_texturable(&texture.info(), false) {
        return false;
    }

    are_color_type_and_format_compatible(
        info.color_type(),
        texture_info_priv::view_format(&texture.info()),
    )
}

/// `WrapTexture` with an explicit color type (the deprecated overload): the image reads `texture`
/// as `color_type`, with the given origin. The label is `"WrappedImage"` and the texture has no
/// release callback.
// Port of: src/gpu/graphite/ImageFactories.cpp#L72-L127 (chrome/m156), without the mipmap and
// release arguments (`genMipmaps` = kNo)
#[doc(alias = "WrapTexture")]
#[must_use]
pub fn wrap_texture(
    recorder: &Recorder,
    backend_texture: &BackendTexture,
    color_type: ColorType,
    alpha_type: AlphaType,
    color_space: Option<ColorSpace>,
    origin: Origin,
) -> Option<CoreImage> {
    let info = ColorInfo::new(color_type, alpha_type, color_space);
    let priv_ = recorder.priv_();
    let caps = Arc::clone(priv_.caps());
    if !validate_backend_texture(&*caps, backend_texture, &info) {
        return None;
    }

    let texture = {
        let mut provider = priv_
            .resource_provider()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        provider.create_wrapped_texture(backend_texture, "WrappedImage")?
    };
    let proxy = TextureProxy::wrap(texture);
    let swizzle = read_swizzle_for_color_type(color_type, proxy.format());
    let view = TextureProxyView::new_with_origin(Some(proxy), swizzle, origin);
    Some(Image::new(view, &info).into_core())
}

/// `WrapTexture(recorder, backendTex, alphaType, colorSpace, origin, releaseP, releaseC)`: an image
/// over a client's backend texture, with the color type inferred from the texture's format.
/// `release` is the texture's release callback. The label is `"WrappedImage"`.
// Port of: src/gpu/graphite/ImageFactories.cpp#L129-L213 (chrome/m156), with `genMipmaps` = kNo
#[doc(alias = "WrapTexture")]
#[must_use]
pub fn wrap_texture_for_alpha_type(
    recorder: &Recorder,
    backend_texture: &BackendTexture,
    alpha_type: AlphaType,
    color_space: Option<ColorSpace>,
    origin: Origin,
    release: Option<CallbackProc>,
) -> Option<CoreImage> {
    let release_helper = release.map(RefCntedCallback::make);

    let priv_ = recorder.priv_();
    let caps = Arc::clone(priv_.caps());

    let format = texture_info_priv::view_format(&backend_texture.info());
    let (mut ct, _) = texture_format_color_type_info(format);
    // The ambiguity for single-channel textures (R) is resolved by the alpha type: premul and
    // unpremul mean an alpha-only texture; opaque means red data.
    if alpha_type == AlphaType::Premul || alpha_type == AlphaType::Unpremul {
        ct = match ct {
            ColorType::R8UNorm => ColorType::Alpha8,
            ColorType::R16UNorm => ColorType::A16UNorm,
            ColorType::R16Float => ColorType::A16Float,
            other => other,
        };
    }

    let mut swizzle = read_swizzle_for_color_type(ct, format);
    let mut alpha_type = alpha_type;
    // An unknown alpha type needs to be forced to opaque by a swizzle if the texture format won't
    // do it for us automatically. Once we force it to opaque, we can report kOpaque for the
    // higher-level image's alpha type.
    if alpha_type == AlphaType::Unknown {
        alpha_type = AlphaType::Opaque;
        if texture_format_channel_mask(format) & ColorChannelFlag::ALPHA.bits() != 0
            && swizzle.as_string().as_bytes()[3] != b'1'
        {
            swizzle = Swizzle::concat(&swizzle, &Swizzle::rgb1());
            // Patch `ct` if possible:
            ct = match ct {
                ColorType::RGBA8888 => ColorType::RGB888x,
                ColorType::RGBA1010102 => ColorType::RGB101010x,
                ColorType::BGRA1010102 => ColorType::BGR101010x,
                ColorType::RGBAF16 => ColorType::RGBF16F16F16x,
                other => other,
            };
        }
    }

    let info = ColorInfo::new(ct, alpha_type, color_space);
    if !validate_backend_texture(&*caps, backend_texture, &info) {
        return None;
    }

    let texture = {
        let mut provider = priv_
            .resource_provider()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        provider.create_wrapped_texture(backend_texture, "WrappedImage")?
    };
    texture.set_release_callback(release_helper.map(|helper| helper as ReleaseCallback));

    let view = TextureProxyView::new_with_origin(Some(TextureProxy::wrap(texture)), swizzle, origin);
    Some(Image::new(view, &info).into_core())
}

/// `PromiseTextureFrom(recorder, dimensions, textureInfo, colorInfo, origin, isVolatile,
/// fulfillProc, imageReleaseProc, textureReleaseProc, imageContext)`: an image whose texture is
/// provided by `fulfill_proc` when the recording that uses it is inserted. `image_release` runs
/// once the image and every recording that uses it are gone.
// Port of: src/gpu/graphite/ImageFactories.cpp#L255-L311 (chrome/m156)
#[doc(alias = "PromiseTextureFrom")]
#[must_use]
pub fn promise_texture_from(
    recorder: &Recorder,
    dimensions: ISize,
    texture_info: &TextureInfo,
    color_info: &ColorInfo,
    origin: Origin,
    is_volatile: Volatile,
    fulfill_proc: PromiseTextureFulfillProc,
    image_release: Option<Box<dyn FnOnce() + Send>>,
) -> Option<CoreImage> {
    // Our contract is that we will always call the _image_ release proc even on failure. We use
    // the helper to convey the imageContext, so we need to ensure Make doesn't fail.
    let release_helper = RefCntedCallback::make(CallbackProc::Plain(
        image_release.unwrap_or_else(|| Box::new(|| {})),
    ));

    let priv_ = recorder.priv_();
    let caps = Arc::clone(priv_.caps());

    let info = ImageInfo::from_color_info(dimensions, color_info.clone());
    if !image_info_is_valid(&info) {
        skia_log_w!("Invalid SkImageInfo");
        return None;
    }

    let format = texture_info_priv::view_format(texture_info);
    if !are_color_type_and_format_compatible(color_info.color_type(), format) {
        skia_log_w!("Incompatible SkColorType and TextureInfo");
        return None;
    }

    // Non-YUVA promise images use the 'imageContext' for both the release proc and fulfill proc.
    let proxy = make_promise_image_lazy_proxy(
        &*caps,
        dimensions,
        texture_info.clone(),
        is_volatile,
        release_helper,
        fulfill_proc,
        "",
    )?;

    let swizzle = read_swizzle_for_color_type(color_info.color_type(), format);
    let view = TextureProxyView::new_with_origin(Some(proxy), swizzle, origin);
    Some(Image::new(view, color_info).into_core())
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
        filter.make_image_with_filter(backend, src, *subset, *clip_bounds)?;
    // The skif backend creates budgeted, scratch textures. This is what we want most of the time,
    // but for the final result image returned from MakeWithFilter(), it needs to be a
    // non-budgeted non-shareable texture (i.e. matching what we return from the other factory
    // methods).
    debug_assert!(image.as_base().is_graphite_backed());
    let image = make_non_budgeted(recorder, &image)?;
    Some((image, out_subset, offset))
}

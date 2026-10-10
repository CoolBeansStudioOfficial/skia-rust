// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureUtils.cpp (ComputeSize, MakeBitmapProxyView, CopyAsDraw,
// GenerateMipmaps, GetGraphiteBacked, AsView)

//! Texture helpers (`TextureUtils.h`): `ComputeSize`, `MakeBitmapProxyView` (mipmapped or not),
//! `CopyAsDraw`, `GenerateMipmaps`, `GetGraphiteBacked` and `AsView`.
//!
//! Not ported here: `RescaleImage` (it reads pixels back, G11c), `MakePromiseImageLazyProxy` (G15)
//! and `skif::MakeGraphiteBackend` (G10c).

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::compressed_data_utils::compressed_format_data_size;
use skia_rust_core::image::{Image as CoreImage, RequiredProperties};
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::image_info_priv::image_info_is_valid;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::sampling_priv::aniso_fallback;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;

use crate::gpu::backing_fit::BackingFit;
use crate::gpu::gpu_types::{Budgeted, Mipmapped, Renderable};
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::caps::Caps;
use crate::graphite::draw_context::DrawContext;
use crate::graphite::image_graphite::Image;
use crate::graphite::image_provider::{
    DefaultImageProvider, ImageProvider, valid_client_provided_image,
};
use crate::graphite::recorder::Recorder;
use crate::graphite::surface_graphite::Surface;
use crate::graphite::task::copy_task::CopyTextureToTextureTask;
use crate::graphite::task::upload_task::{ImageUploadContext, MipLevel, UploadSource};
use crate::graphite::texture_format::{
    are_color_type_and_format_compatible, read_swizzle_for_color_type,
    texture_format_bytes_per_block, texture_format_color_type_info,
    texture_format_compression_type,
};
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;

/// `ComputeSize`: the approximate GPU memory a texture of `dimensions` described by `info` uses.
// Port of: src/gpu/graphite/TextureUtils.cpp#L370-L393 (chrome/m156)
#[doc(alias = "ComputeSize")]
#[allow(clippy::cast_sign_loss)] // mirrors the (size_t) casts on non-negative dimensions
#[must_use]
pub fn compute_size(dimensions: ISize, info: &TextureInfo) -> usize {
    let format = texture_info_priv::view_format(info);
    let compression = texture_format_compression_type(format);

    let color_size = if compression == TextureCompressionType::None {
        // TODO(b/401016699): Add logic to handle multiplanar formats
        let bytes_per_pixel = texture_format_bytes_per_block(format);

        (dimensions.width as usize)
            .wrapping_mul(dimensions.height as usize)
            .wrapping_mul(bytes_per_pixel as usize)
    } else {
        compressed_format_data_size(compression, dimensions, info.mipmapped() == Mipmapped::Yes)
    };

    // size_t arithmetic wraps in C++.
    let mut final_size = color_size.wrapping_mul(info.sample_count() as usize);

    if info.mipmapped() == Mipmapped::Yes {
        final_size = final_size.wrapping_add(color_size / 3);
    }
    final_size
}

/// `MakeBitmapProxyView(recorder, bitmap, mipmaps, mipmapped, budgeted, label)`: a texture the
/// bitmap's pixels (and, when `mipmapped` is `Yes`, its mip levels) are uploaded to.
///
/// `mipmaps` are the bitmap's mipmaps if the caller has them; otherwise they are built from the
/// bitmap. The upload goes on the host when possible, and otherwise through a task on the root
/// upload list. The resource provider is locked only to create the proxy: the upload needs it
/// again (its transfer buffers come from the provider).
// Port of: src/gpu/graphite/TextureUtils.cpp#L254-L343 (chrome/m156)
#[doc(alias = "MakeBitmapProxyView")]
#[must_use]
pub fn make_bitmap_proxy_view(
    recorder: &Recorder,
    bitmap: &Bitmap,
    mipmaps: Option<Arc<Mipmap>>,
    mut mipmapped: Mipmapped,
    budgeted: Budgeted,
    label: &str,
) -> Option<TextureProxyView> {
    let priv_ = recorder.priv_();
    let caps = Arc::clone(priv_.caps());
    let ct = bitmap.color_type();
    if i64::from(bitmap.dimensions().width) * i64::from(bitmap.dimensions().height) <= 1 {
        mipmapped = Mipmapped::No;
    }

    let texture_info =
        caps.get_default_sampled_texture_info(ct, mipmapped, priv_.is_protected(), Renderable::No);
    if !texture_info.is_valid() {
        return None;
    }
    if !image_info_is_valid(bitmap.info()) {
        return None;
    }

    let mip_level_count = if mipmapped == Mipmapped::Yes {
        Mipmap::compute_level_count_size(bitmap.dimensions()) + 1
    } else {
        1
    };
    let mips = if mip_level_count > 1 {
        if let Some(mips) = mipmaps {
            Some(mips)
        } else {
            let Some(mips) = Mipmap::build(&bitmap.pixmap(), true) else {
                skia_log_e!("Generating mipmaps failed");
                return None;
            };
            Some(Arc::new(mips))
        }
    } else {
        None
    };

    // Create proxy.
    let proxy = {
        let mut provider = priv_
            .resource_provider()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        TextureProxy::make(
            &*caps,
            &mut provider,
            bitmap.dimensions(),
            &texture_info,
            budgeted,
            label,
        )?
    };

    let format = proxy.format();
    debug_assert!(are_color_type_and_format_compatible(ct, format));
    debug_assert!(mipmapped == Mipmapped::No || proxy.mipmapped() == Mipmapped::Yes);

    let swizzle = read_swizzle_for_color_type(ct, format);
    let view = TextureProxyView::new(Some(proxy), swizzle);

    // The base level is always included. The pixel bytes are borrowed from the bitmap and the
    // mipmaps, which live until the upload is recorded.
    let pixmap = bitmap.pixmap();
    let mut levels = vec![MipLevel {
        pixels: pixmap.addr(),
        row_bytes: bitmap.row_bytes(),
    }];
    if let Some(mips) = &mips {
        for i in 1..mip_level_count {
            levels.push(MipLevel {
                pixels: mips.level_bytes(usize::try_from(i - 1).ok()?),
                row_bytes: mips.get_level(i - 1)?.pixmap.row_bytes(),
            });
        }
    }

    // Src and dst colorInfo are the same.
    let color_info = bitmap.info().color_info().clone();
    // Add upload to the root upload list. These bitmaps are uploaded to unique textures so there
    // is no need to coordinate resource sharing. It is better to then group them into a single
    // task at the start of the Recording.
    let dimensions = IRect::from_size(bitmap.dimensions());
    // The upload source takes the view's proxy as the unique holder while it is uploaded; the
    // C++ moves `view` into it.
    let upload_source =
        UploadSource::make(&*caps, &view, &color_info, &color_info, &levels, dimensions);
    drop(view);
    if !upload_source.is_valid() {
        skia_log_e!("MakeBitmapProxyView: Could not create UploadSource");
        return None;
    }

    if upload_source.attempt_upload_on_host() {
        return Some(upload_source.view().clone());
    }

    // Otherwise it failed or was unavailable, so use a task on the root upload list.
    let upload_buffer_manager = priv_.upload_buffer_manager().clone();
    let mut upload_buffer_manager = upload_buffer_manager.borrow_mut();
    let uploaded = priv_.root_upload_list().borrow_mut().record_upload(
        &*caps,
        &mut upload_buffer_manager,
        &upload_source,
        Some(Box::new(ImageUploadContext)),
    );
    if !uploaded {
        skia_log_e!("MakeBitmapProxyView: Could not create UploadInstance");
        return None;
    }

    Some(upload_source.view().clone())
}

/// `make_renderable(srcInfo, dstInfo)`: the color info a copy of `src_info` into `dst_info` renders
/// into.
// Port of: src/gpu/graphite/TextureUtils.cpp#L26-L70 (chrome/m156), with `renderable_colortype`
// and `renderable_alphatype`
fn make_renderable(src_info: &ColorInfo, dst_info: &ColorInfo) -> ColorInfo {
    let color_type = match dst_info.color_type() {
        ColorType::RGB101010x => ColorType::RGBA1010102,
        ColorType::BGR101010x => ColorType::BGRA1010102,
        ColorType::RGBF16F16F16x | ColorType::RGBAF16Norm => ColorType::RGBAF16,
        ColorType::RGB888x => ColorType::RGBA8888,
        ColorType::Gray8 => {
            if src_info.color_type() == ColorType::Gray8 {
                ColorType::R8UNorm
            } else {
                ColorType::Unknown
            }
        }
        other => other,
    };
    let dst_at = dst_info.alpha_type();
    let alpha_type = match src_info.alpha_type() {
        AlphaType::Premul if dst_at == AlphaType::Unpremul => AlphaType::Unpremul,
        AlphaType::Premul => AlphaType::Premul,
        AlphaType::Unpremul if dst_at == AlphaType::Premul => AlphaType::Premul,
        // Unknown and opaque sources, and an unpremul source not blended into a premul
        // destination, are opaque.
        _ => AlphaType::Opaque,
    };
    dst_info
        .with_color_type(color_type)
        .with_alpha_type(alpha_type)
}

/// `final_alphatype(srcAT, renderedAT)`: an unpremul source that rendered opaque stays unpremul.
// Port of: src/gpu/graphite/TextureUtils.cpp#L72-L75 (chrome/m156)
fn final_alpha_type(src: AlphaType, rendered: AlphaType) -> AlphaType {
    if src == AlphaType::Unpremul && rendered == AlphaType::Opaque {
        AlphaType::Unpremul
    } else {
        rendered
    }
}

/// `CopyAsDraw(recorder, drawContext, image, subset, dstColorInfo, budgeted, mipmapped,
/// backingFit, label)`: copies `subset` of `image` into a scratch surface by drawing it (a `kSrc`
/// draw with nearest sampling), for sources that cannot be blitted.
///
/// The copy's tasks go into `draw_context` when there is one, and into the root task list
/// otherwise.
// Port of: src/gpu/graphite/TextureUtils.cpp#L100-L130 (chrome/m156)
#[doc(alias = "CopyAsDraw")]
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[must_use]
pub fn copy_as_draw(
    recorder: &Recorder,
    draw_context: Option<&mut DrawContext>,
    image: &CoreImage,
    subset: IRect,
    dst_color_info: &ColorInfo,
    budgeted: Budgeted,
    mipmapped: Mipmapped,
    backing_fit: BackingFit,
    label: &str,
) -> Option<CoreImage> {
    let src_color_info = image.image_info().color_info().clone();
    let dst_info = ImageInfo::from_color_info(
        subset.size(),
        make_renderable(&src_color_info, dst_color_info),
    );
    let surface = Surface::make_scratch(
        recorder,
        &dst_info,
        label,
        budgeted,
        mipmapped,
        backing_fit,
        /*allow_unpremul=*/ true,
    )?;

    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    surface.canvas().draw_image_with_sampling_options(
        image,
        (-subset.left(), -subset.top()),
        SamplingOptions::from(FilterMode::Nearest),
        Some(&paint),
    );
    surface.flush_to_draw_context(draw_context);
    surface.as_image_with_color(
        dst_color_info.color_type(),
        final_alpha_type(image.image_info().alpha_type(), dst_info.alpha_type()),
    )
}

/// `GenerateMipmaps(recorder, drawContext, texture)`: fills the mip levels of `texture` by
/// repeatedly drawing each level into a half-size scratch surface and copying it back.
// Port of: src/gpu/graphite/TextureUtils.cpp#L185-L240 (chrome/m156)
#[doc(alias = "GenerateMipmaps")]
#[must_use]
pub fn generate_mipmaps(
    recorder: &Recorder,
    mut draw_context: Option<&mut DrawContext>,
    texture: &Arc<TextureProxy>,
) -> bool {
    debug_assert_eq!(texture.mipmapped(), Mipmapped::Yes);
    let (color_type, _) = texture_format_color_type_info(texture.format());
    let color_info = ColorInfo::new(color_type, AlphaType::Opaque, None);
    debug_assert_eq!(make_renderable(&color_info, &color_info), color_info);
    let img_swizzle = read_swizzle_for_color_type(color_info.color_type(), texture.format());
    let mut scratch_img = Image::new(
        TextureProxyView::new(Some(Arc::clone(texture)), img_swizzle),
        &color_info,
    )
    .into_core();

    let mut src_size = texture.dimensions();
    let mut scratch_surfaces = Vec::with_capacity(2);
    for i in 0..2 {
        let size = ISize::new(
            (src_size.width >> (i + 1)).max(1),
            (src_size.height >> (i + 1)).max(1),
        );
        let Some(surface) = Surface::make_scratch(
            recorder,
            &ImageInfo::from_color_info(size, color_info.clone()),
            "GenerateMipmapsScratchTexture",
            Budgeted::Yes,
            Mipmapped::No,
            BackingFit::Approx,
            false,
        ) else {
            return false;
        };
        scratch_surfaces.push(surface);
    }

    let mut mip_level = 1;
    while src_size.width > 1 || src_size.height > 1 {
        let dst_size = ISize::new((src_size.width >> 1).max(1), (src_size.height >> 1).max(1));
        let scratch_surface = &scratch_surfaces[usize::from(mip_level % 2 == 0)];
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);
        let src_rect = Rect::from_size(src_size);
        let dst_rect = Rect::from_size(dst_size);
        scratch_surface
            .canvas()
            .draw_image_rect_with_sampling_options(
                &scratch_img,
                Some((&src_rect, SrcRectConstraint::Strict)),
                dst_rect,
                SamplingOptions::from(FilterMode::Linear),
                &paint,
            );
        scratch_surface.flush_to_draw_context(draw_context.as_deref_mut());

        let Some(copy_task) = CopyTextureToTextureTask::make(
            scratch_surface.target().ref_proxy(),
            IRect::from_size(dst_size),
            Some(Arc::clone(texture)),
            IPoint::new(0, 0),
            mip_level,
        ) else {
            return false;
        };
        if let Some(draw_context) = draw_context.as_deref_mut() {
            draw_context.record_dependency(copy_task);
        } else {
            recorder.priv_().add(copy_task);
        }
        scratch_img = scratch_surface.as_image();
        src_size = dst_size;
        mip_level += 1;
    }
    true
}

/// `GetGraphiteBacked(recorder, image, sampling)`: a Graphite-backed version of `image` to draw
/// with, and the sampling to draw it with. The default image provider converts an image that is
/// not Graphite-backed.
// Port of: src/gpu/graphite/TextureUtils.cpp#L650-L698 (chrome/m156)
#[doc(alias = "GetGraphiteBacked")]
#[must_use]
pub fn get_graphite_backed(
    recorder: &Recorder,
    image: &CoreImage,
    mut sampling: SamplingOptions,
) -> (Option<CoreImage>, SamplingOptions) {
    let mut mipmapped = if sampling.mipmap == MipmapMode::None {
        Mipmapped::No
    } else {
        Mipmapped::Yes
    };
    if i64::from(image.width()) * i64::from(image.height()) <= 1 && mipmapped == Mipmapped::Yes {
        mipmapped = Mipmapped::No;
        sampling = SamplingOptions::from(FilterMode::Linear);
    }

    let result = if image.as_base().is_graphite_backed() {
        let caps: Arc<dyn Caps> = Arc::clone(recorder.priv_().caps());
        let texturable = Image::from_core(image).is_none_or(|graphite| {
            graphite
                .texture_proxy_view()
                .proxy()
                .is_none_or(|proxy| caps.is_texturable(proxy.texture_info(), false))
        });
        if !texturable {
            sampling = SamplingOptions::from(FilterMode::Nearest);
        } else if mipmapped == Mipmapped::Yes && !image.has_mipmaps() {
            sampling = SamplingOptions::from(FilterMode::Linear);
        }
        Some(image.clone())
    } else {
        let provider = DefaultImageProvider;
        let required = RequiredProperties {
            mipmapped: mipmapped == Mipmapped::Yes,
        };
        provider
            .find_or_create(recorder, image, required)
            .filter(|found| valid_client_provided_image(Some(found), image))
    };

    if sampling.is_aniso()
        && let Some(result) = &result
    {
        sampling = aniso_fallback(result.has_mipmaps());
    }
    (result, sampling)
}

/// `AsView(image)`: the texture view of a Graphite-backed image that is not YUVA, or an empty view
/// for anything else.
// Port of: src/gpu/graphite/TextureUtils.cpp#L700-L712 (chrome/m156)
#[doc(alias = "AsView")]
#[must_use]
pub fn as_view(image: Option<&CoreImage>) -> TextureProxyView {
    let Some(image) = image else {
        return TextureProxyView::default();
    };
    if !image.as_base().is_graphite_backed() || image.as_base().is_yuva() {
        return TextureProxyView::default();
    }
    Image::from_core(image)
        .map(|graphite| graphite.texture_proxy_view().clone())
        .unwrap_or_default()
}

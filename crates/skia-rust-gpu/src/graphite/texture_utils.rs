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
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::compressed_data_utils::compressed_format_data_size;
use skia_rust_core::image::{Image as CoreImage, RequiredProperties, RescaleGamma, RescaleMode};
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::image_info_priv::image_info_is_valid;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::sampling_priv::aniso_fallback;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;
use skia_rust_core::yuva_info::YUVAChannels;

use crate::gpu::backing_fit::BackingFit;
use crate::gpu::gpu_types::{Budgeted, Mipmapped, Renderable};
use crate::gpu::ref_cnted_callback::{CallbackProc, RefCntedCallback};
use crate::gpu::sk_log::{skia_log_e, skia_log_w};
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::caps::Caps;
use crate::graphite::draw_context::DrawContext;
use crate::graphite::graphite_types::Volatile;
use crate::graphite::image_graphite::Image;
use crate::graphite::image_provider::{
    DefaultImageProvider, ImageProvider, valid_client_provided_image,
};
use crate::graphite::image_yuva_graphite::ImageYuva;
use crate::graphite::recorder::Recorder;
use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::surface_graphite::Surface;
use crate::graphite::task::copy_task::CopyTextureToTextureTask;
use crate::graphite::task::upload_task::{ImageUploadContext, MipLevel, UploadSource};
use crate::graphite::texture::{ReleaseCallback, Texture};
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

/// `GraphitePromiseTextureFulfillProc`: called when a promise image's texture is instantiated. It
/// returns the backend texture the promise is fulfilled with and the callback run when that
/// texture is released (`GraphitePromiseTextureReleaseProc` with its context). The fulfill call
/// may happen more than once for volatile promise images.
pub type PromiseTextureFulfillProc =
    Arc<dyn Fn() -> (BackendTexture, Option<Box<dyn FnOnce() + Send>>) + Send + Sync>;

/// `PromiseLazyInstantiateCallback`: the lazy instantiation of a promise image's proxy. It calls
/// the fulfill proc, and wraps the returned backend texture. It holds the image release helper,
/// so the image release runs when the last proxy referencing it goes away.
// Port of: src/gpu/graphite/TextureUtils.cpp#L198-L250 (chrome/m156)
struct PromiseLazyInstantiateCallback {
    _release_helper: Arc<RefCntedCallback>,
    fulfill_proc: PromiseTextureFulfillProc,
    label: String,
}

impl PromiseLazyInstantiateCallback {
    // Port of: src/gpu/graphite/TextureUtils.cpp#L223-L242 (chrome/m156)
    fn instantiate(
        &self,
        resource_provider: &mut ResourceProvider,
    ) -> Option<ResourceRef<Texture>> {
        // Invoke the fulfill proc to get the promised backend texture.
        let (backend_texture, texture_release) = (self.fulfill_proc)();
        if !backend_texture.is_valid() {
            skia_log_w!("FulfillProc returned an invalid backend texture");
            return None;
        }

        let texture_release_cb =
            texture_release.map(|proc| RefCntedCallback::make(CallbackProc::Plain(proc)));

        let texture = resource_provider.create_wrapped_texture(&backend_texture, &self.label);
        let Some(texture) = texture else {
            skia_log_w!("Failed to wrap BackendTexture returned by fulfill proc");
            return None;
        };
        texture.set_release_callback(texture_release_cb.map(|cb| cb as ReleaseCallback));
        Some(texture)
    }
}

/// `MakePromiseImageLazyProxy(caps, dimensions, textureInfo, isVolatile, releaseHelper, fulfillProc,
/// fulfillContext, textureReleaseProc, label)`: a lazy proxy that is instantiated by calling
/// `fulfill_proc`. Promise proxies are never budgeted, as they belong to a client's image.
// Port of: src/gpu/graphite/TextureUtils.cpp#L345-L368 (chrome/m156)
#[doc(alias = "MakePromiseImageLazyProxy")]
#[must_use]
pub fn make_promise_image_lazy_proxy(
    caps: &dyn Caps,
    dimensions: ISize,
    texture_info: &TextureInfo,
    is_volatile: Volatile,
    release_helper: Arc<RefCntedCallback>,
    fulfill_proc: PromiseTextureFulfillProc,
    label: &str,
) -> Option<Arc<TextureProxy>> {
    debug_assert!(!dimensions.is_empty());

    let callback = PromiseLazyInstantiateCallback {
        _release_helper: release_helper,
        fulfill_proc,
        label: label.to_owned(),
    };
    // Proxies for promise images are assumed to always be destined for a client's SkImage so
    // are never considered budgeted.
    TextureProxy::make_lazy(
        caps,
        dimensions,
        texture_info,
        Budgeted::No,
        is_volatile,
        Box::new(move |resource_provider| callback.instantiate(resource_provider)),
    )
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

/// `SkRect::Make(SkIRect)`.
fn rect_from_irect(r: IRect) -> Rect {
    // mirrors SkRect::Make(SkIRect): the coordinates are integers, which an f32 holds exactly
    #[allow(clippy::cast_precision_loss)]
    Rect::new(
        r.left() as f32,
        r.top() as f32,
        r.right() as f32,
        r.bottom() as f32,
    )
}

/// `SkRect::Make(SkISize)`.
fn rect_from_size(s: ISize) -> Rect {
    // mirrors SkRect::Make(SkISize): the dimensions are integers, which an f32 holds exactly
    #[allow(clippy::cast_precision_loss)]
    Rect::from_xywh(0.0, 0.0, s.width as f32, s.height as f32)
}

/// `RescaleImage(recorder, srcImage, srcIRect, dstInfo, rescaleGamma, rescaleMode)`: rescales the
/// `src_irect` part of `src_image` to the dimensions of `dst_info`, by drawing it into scratch
/// surfaces (halving each step for the repeated modes), and returns the result with the color
/// type and alpha type of `dst_info`.
// Port of: src/gpu/graphite/TextureUtils.cpp#L428-L520 (chrome/m156)
#[doc(alias = "RescaleImage")]
#[must_use]
pub fn rescale_image(
    recorder: &Recorder,
    src_image: &CoreImage,
    src_irect: IRect,
    dst_info: &ImageInfo,
    mut rescale_gamma: RescaleGamma,
    mut rescale_mode: RescaleMode,
) -> Option<CoreImage> {
    let src_color_info = src_image.image_info().color_info().clone();

    // Other than the final step, rescaling will be performed in the source color type and color
    // space, possibly with a linear gamma adjustment (although changing the SkColorSpace can be
    // applied to the result of make_renderable).
    let mut step_info = make_renderable(&src_color_info, &src_color_info);

    // Make a Surface *exactly* (barring renderable adjustments) matching dstInfo for the final
    // scaling step.
    let render_dst_info = make_renderable(&src_color_info, dst_info.color_info());
    let dst = Surface::make_scratch(
        recorder,
        &ImageInfo::from_color_info(dst_info.dimensions(), render_dst_info.clone()),
        "RescaleDstTexture",
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        /*allow_unpremul=*/ true,
    )?;

    let mut step_src = rect_from_irect(src_irect);

    let final_size = dst_info.dimensions();
    if final_size == src_irect.size() {
        rescale_gamma = RescaleGamma::Src;
        rescale_mode = RescaleMode::Nearest;
    }

    // Within a rescaling pass tempInput is read from and tempOutput is written to.
    // At the end of the pass tempOutput's texture is wrapped and assigned to tempInput.
    let mut temp_input: CoreImage = src_image.clone();

    // Assume we should ignore the rescale linear request if the surface has no color space since
    // it's unclear how we'd linearize from an unknown color space.
    if rescale_gamma == RescaleGamma::Linear
        && step_info
            .color_space_ref()
            .is_some_and(|cs| !cs.gamma_is_linear())
    {
        // Draw the src image into a new surface with linear gamma, and make that the new tempInput.
        let linear_gamma = step_info
            .color_space_ref()
            .map(ColorSpace::with_linear_gamma)?;
        let gamma_dst_info =
            ImageInfo::from_color_info(src_irect.size(), step_info.with_color_space(linear_gamma));
        let gamma_dst = Surface::make_scratch(
            recorder,
            &gamma_dst_info,
            "RescaleLinearGammaTexture",
            Budgeted::Yes,
            Mipmapped::No,
            BackingFit::Approx,
            /*allow_unpremul=*/ false,
        )?;
        let gamma_dst_rect = rect_from_size(src_irect.size());

        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);
        gamma_dst.canvas().draw_image_rect_with_sampling_options(
            &temp_input,
            Some((&step_src, SrcRectConstraint::Strict)),
            gamma_dst_rect,
            SamplingOptions::from(FilterMode::Nearest),
            &paint,
        );
        // `gamma_dst` is released at the end of this block, where Skia's `tempOutput` is reassigned
        // by the first step of the loop.
        temp_input = gamma_dst.as_image();
        step_src = gamma_dst_rect;
        // The remaining steps output linear gamma too.
        step_info = gamma_dst_info.color_info().clone();
    }

    loop {
        // The next step's dimensions. For the repeated modes each step at most halves (or
        // doubles) the size, so the filtering stays close to the target scale.
        let next_dims = next_rescale_dims(step_src, final_size, rescale_mode);

        // The step's target: a scratch surface, or `dst` for the final step (Skia's `tempOutput`).
        // The scratch surface is released at the end of the step, once `tempInput` holds its image.
        let step_surface = if next_dims == final_size {
            None
        } else {
            Some(Surface::make_scratch(
                recorder,
                &ImageInfo::from_color_info(next_dims, step_info.clone()),
                "RescaleImageTempTexture",
                Budgeted::Yes,
                Mipmapped::No,
                BackingFit::Approx,
                /*allow_unpremul=*/ false,
            )?)
        };
        let step_dst_rect = rect_from_size(next_dims);

        let sampling = match rescale_mode {
            RescaleMode::RepeatedCubic => SamplingOptions::from(CubicResampler::catmull_rom()),
            RescaleMode::Nearest => SamplingOptions::from(FilterMode::Nearest),
            RescaleMode::Linear | RescaleMode::RepeatedLinear => {
                SamplingOptions::from(FilterMode::Linear)
            }
        };
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);
        let target = step_surface.as_ref().unwrap_or(&dst);
        target.canvas().draw_image_rect_with_sampling_options(
            &temp_input,
            Some((&step_src, SrcRectConstraint::Strict)),
            step_dst_rect,
            sampling,
            &paint,
        );

        temp_input = target.as_image();
        step_src = rect_from_size(next_dims);

        if rect_has_size(step_src, final_size) {
            break;
        }
    }

    // Cast back to the requested color type and final alpha type now that rendering is finished.
    dst.as_image_with_color(
        dst_info.color_type(),
        final_alpha_type(src_image.alpha_type(), render_dst_info.alpha_type()),
    )
}

/// Whether `rect` has the size `size`. Both are integer-valued, so the exact comparison is the
/// C++ `!=` of the loop of `RescaleImage`.
#[allow(clippy::float_cmp, clippy::cast_precision_loss)] // mirrors the C++ exact comparison
fn rect_has_size(rect: Rect, size: ISize) -> bool {
    rect.width() == size.width as f32 && rect.height() == size.height as f32
}

/// The next step's dimensions of `RescaleImage`'s loop: for the repeated modes, each step halves
/// (or doubles) a side towards `final_size`; the other modes go straight to `final_size`.
// Port of: src/gpu/graphite/TextureUtils.cpp#L460-L475 (chrome/m156)
// mirrors the C++ float arithmetic on integer-valued rects, and its float to int conversions
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::manual_midpoint
)]
fn next_rescale_dims(src_rect: Rect, final_size: ISize, mode: RescaleMode) -> ISize {
    let mut next = final_size;
    if mode != RescaleMode::Nearest && mode != RescaleMode::Linear {
        let (fin_w, fin_h) = (final_size.width as f32, final_size.height as f32);
        if src_rect.width() > fin_w {
            next.width = ((src_rect.width() + 1.0) / 2.0).max(fin_w) as i32;
        } else if src_rect.width() < fin_w {
            next.width = (src_rect.width() * 2.0).min(fin_w) as i32;
        }
        if src_rect.height() > fin_h {
            next.height = ((src_rect.height() + 1.0) / 2.0).max(fin_h) as i32;
        } else if src_rect.height() < fin_h {
            next.height = (src_rect.height() * 2.0).min(fin_h) as i32;
        }
    }
    next
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
        // Every plane of a YUVA image must be texturable; a single-texture image has one view.
        let view_texturable = |view: &TextureProxyView| {
            view.proxy()
                .is_none_or(|proxy| caps.is_texturable(proxy.texture_info(), false))
        };
        let texturable = if let Some(yuva) = ImageYuva::from_core(image) {
            (0..YUVAChannels::COUNT).all(|i| view_texturable(yuva.proxy_view(i)))
        } else {
            Image::from_core(image)
                .is_none_or(|graphite| view_texturable(graphite.texture_proxy_view()))
        };
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

// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Image_Graphite.{h,cpp}, and the parts of
// src/gpu/graphite/Image_Base_Graphite.{h,cpp} that a Graphite image without linked Devices needs
// (`copyImage`, `onMakeSubset`, `makeColorTypeAndColorSpace`, `textureSize`).
//
// skia-rust deviations:
// - `Image_Base::notifyInUse()` and the linked `Device`s (`linkDevice`, `isDynamic`, `unlinkDevices`,
//   `makeNonBudgeted`) are not ported. `ImageBase` is `Send + Sync` and a `Device` is `Rc`-based
//   (`docs/design/gpu.md` §5.1), so an image cannot own a `Device`. `Surface::as_image` flushes the
//   device instead (see `surface_graphite`), so an image snapshot holds the draws made before it.
// - Operations that take an `SkRecorder*` (`onMakeSubset`, `makeColorTypeAndColorSpace`) need the
//   recorder, which `ImageBase` cannot pass; they are `*_with_recorder` methods here, and the
//   `ImageBase` methods return `None`.

use std::any::Any;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image as CoreImage, RequiredProperties};
use skia_rust_core::image_base::{ImageBase, ImageType};
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::pixel_ref::next_image_id;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;

use crate::gpu::backing_fit::{BackingFit, get_approx_size};
use crate::gpu::gpu_types::{Budgeted, Mipmapped, Protected};
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::draw_context::DrawContext;
use crate::graphite::recorder::Recorder;
use crate::graphite::task::copy_task::CopyTextureToTextureTask;
use crate::graphite::texture_format::{
    are_color_type_and_format_compatible, read_swizzle_for_color_type,
};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::texture_utils::{copy_as_draw, generate_mipmaps};

/// `Image`: a Graphite-backed image, which is a view of a texture (`skgpu::graphite::Image`).
// Port of: src/gpu/graphite/Image_Graphite.h#L24-L69 (chrome/m156)
#[doc(alias = "skgpu::graphite::Image")]
#[derive(Debug)]
pub struct Image {
    info: ImageInfo,
    unique_id: u32,
    texture_proxy_view: TextureProxyView,
}

impl Image {
    /// `Image(TextureProxyView, const SkColorInfo&)`. The image's dimensions are the proxy's.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L24-L30 (chrome/m156)
    #[doc(alias = "Image")]
    #[must_use]
    pub fn new(view: TextureProxyView, color_info: &ColorInfo) -> Image {
        // Graphite does not cache based on the image's unique ID so always request a new one
        // (`kNeedNewImageUniqueID`).
        let info = ImageInfo::from_color_info(view.dimensions(), color_info.clone());
        Image {
            info,
            unique_id: next_image_id(),
            texture_proxy_view: view,
        }
    }

    /// Wraps this image as a core `SkImage` handle (`sk_sp<Image>`).
    #[must_use]
    pub fn into_core(self) -> CoreImage {
        CoreImage::from_base(self)
    }

    /// The Graphite image behind a core image, if it is one (`static_cast<const Image*>` after
    /// `isGraphiteBacked()`).
    // Port of: src/gpu/graphite/TextureUtils.cpp#L700-L712 (chrome/m156), the cast in `AsView`
    #[must_use]
    pub fn from_core(image: &CoreImage) -> Option<&Image> {
        image.as_base().as_any().downcast_ref::<Image>()
    }

    /// `textureProxyView()`.
    #[doc(alias = "textureProxyView")]
    #[must_use]
    pub fn texture_proxy_view(&self) -> &TextureProxyView {
        &self.texture_proxy_view
    }

    /// `Image::textureSize()`.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L120-L128 (chrome/m156)
    #[doc(alias = "textureSize")]
    #[must_use]
    pub fn texture_size(&self) -> usize {
        let Some(proxy) = self.texture_proxy_view.proxy() else {
            return 0;
        };
        proxy.with_texture(|texture| match texture {
            None => proxy.uninstantiated_gpu_memory_size(),
            Some(texture) => texture.base().gpu_memory_size(),
        })
    }

    /// `Image::Copy(recorder, drawContext, srcView, srcColorInfo, subset, budgeted, mipmapped,
    /// backingFit, label)`: copies `src_view`'s `subset` into a new texture, as a blit when the
    /// source is copyable and as a draw otherwise.
    ///
    /// `draw_context` is `None` for copies made outside a draw; otherwise the copy's tasks are
    /// recorded as dependencies of that draw context.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L44-L112 (chrome/m156)
    #[doc(alias = "Copy")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[must_use]
    pub fn copy(
        recorder: &Recorder,
        mut draw_context: Option<&mut DrawContext>,
        src_view: &TextureProxyView,
        src_color_info: &ColorInfo,
        subset: IRect,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        label: &str,
    ) -> Option<CoreImage> {
        debug_assert!(draw_context.is_none() || budgeted == Budgeted::Yes);
        debug_assert!(!(mipmapped == Mipmapped::Yes && backing_fit == BackingFit::Approx));
        let src_proxy = src_view.proxy()?;

        let priv_ = recorder.priv_();
        let caps = Arc::clone(priv_.caps());
        if !caps.is_copyable_src(src_proxy.texture_info()) {
            if !caps.is_texturable(src_proxy.texture_info(), false) {
                // The texture is not blittable nor texturable so copying cannot be done.
                return None;
            }
            // Copy-as-draw
            let src_image = Image::new(src_view.clone(), src_color_info).into_core();
            return copy_as_draw(
                recorder,
                draw_context,
                &src_image,
                subset,
                src_color_info,
                budgeted,
                mipmapped,
                backing_fit,
                label,
            );
        }

        let texture_info =
            caps.get_texture_info_for_sampled_copy(src_proxy.texture_info(), mipmapped);
        let dst_size = if backing_fit == BackingFit::Approx {
            get_approx_size(subset.size())
        } else {
            subset.size()
        };
        let dst = {
            let mut provider = priv_
                .resource_provider()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            TextureProxy::make(
                &*caps,
                &mut provider,
                dst_size,
                &texture_info,
                budgeted,
                label,
            )?
        };

        let copy_task = CopyTextureToTextureTask::make(
            src_view.ref_proxy(),
            subset,
            Some(Arc::clone(&dst)),
            IPoint::new(0, 0),
            0,
        )?;
        if let Some(draw_context) = draw_context.as_deref_mut() {
            draw_context.record_dependency(copy_task);
        } else {
            priv_.add(copy_task);
        }

        if mipmapped == Mipmapped::Yes && !generate_mipmaps(recorder, draw_context, &dst) {
            skia_log_w!("Image::Copy failed to generate mipmaps");
            return None;
        }

        Some(
            Image::new(
                TextureProxyView::new(Some(dst), src_view.swizzle()),
                src_color_info,
            )
            .into_core(),
        )
    }

    /// `Image::copyImage(recorder, subset, budgeted, mipmapped, backingFit, label)`.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L130-L138 (chrome/m156)
    #[doc(alias = "copyImage")]
    #[must_use]
    pub fn copy_image(
        &self,
        recorder: &Recorder,
        subset: IRect,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        label: &str,
    ) -> Option<CoreImage> {
        Image::copy(
            recorder,
            None,
            &self.texture_proxy_view,
            self.info.color_info(),
            subset,
            budgeted,
            mipmapped,
            backing_fit,
            label,
        )
    }

    /// `Image::onReinterpretColorSpace(newCS)`: the same texture under another color space.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L140-L146 (chrome/m156)
    #[doc(alias = "onReinterpretColorSpace")]
    #[must_use]
    pub fn reinterpret_color_space(&self, new_cs: Option<ColorSpace>) -> CoreImage {
        Image::new(
            self.texture_proxy_view.clone(),
            &self.info.color_info().with_color_space(new_cs),
        )
        .into_core()
    }

    fn bounds_irect(&self) -> IRect {
        IRect::from_size(self.info.dimensions())
    }

    /// The bounds of the image (`bounds()`).
    #[must_use]
    pub fn bounds(&self) -> IRect {
        self.bounds_irect()
    }

    /// `hasMipmaps()`: whether the texture has its mip levels.
    #[must_use]
    pub fn has_mipmaps(&self) -> bool {
        self.texture_proxy_view
            .proxy()
            .is_some_and(|p| p.mipmapped() == Mipmapped::Yes)
    }
}

/// The label of a copy of an image's texture (`get_chained_label`): the texture's label with
/// `concat` added, or `empty` when the texture is unlabeled.
// Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L63-L75 (chrome/m156)
fn get_chained_label(image: &Image, empty: &str, concat: &str) -> String {
    let label = image
        .texture_proxy_view()
        .proxy()
        .map_or("", |proxy| proxy.label());
    if label.is_empty() {
        empty.to_owned()
    } else {
        format!("{label}{concat}")
    }
}

/// `Image_Base::onMakeSubset(recorder, subset, requiredProps)`: `this` itself when it already is
/// `subset` with the mipmaps that are required, and a copy of `subset` otherwise.
///
/// Skia's `!isDynamic()` check is not made (see the module docs).
// Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L185-L201 (chrome/m156)
#[doc(alias = "onMakeSubset")]
#[must_use]
pub fn make_subset(
    recorder: &Recorder,
    this: &CoreImage,
    subset: IRect,
    required_props: RequiredProperties,
) -> Option<CoreImage> {
    let image = Image::from_core(this)?;
    if image.bounds() == subset && (!required_props.mipmapped || image.has_mipmaps()) {
        return Some(this.clone());
    }
    image.copy_image(
        recorder,
        subset,
        Budgeted::No,
        if required_props.mipmapped {
            Mipmapped::Yes
        } else {
            Mipmapped::No
        },
        BackingFit::Exact,
        &get_chained_label(image, "ImageSubsetTexture", "_Subset"),
    )
}

/// `Image_Base::makeColorTypeAndColorSpace(recorder, targetCT, targetCS, requiredProps)`: `this`
/// itself when the color info already matches, and a copy drawn into the new color info otherwise.
// Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L226-L247 (chrome/m156), without the
// `!isDynamic()` check (see the module docs)
#[doc(alias = "makeColorTypeAndColorSpace")]
#[must_use]
pub fn make_color_type_and_color_space(
    recorder: &Recorder,
    this: &CoreImage,
    target_color_type: ColorType,
    target_color_space: Option<ColorSpace>,
    required_props: RequiredProperties,
) -> Option<CoreImage> {
    let image = Image::from_core(this)?;
    let dst_color_info = ColorInfo::new(target_color_type, this.alpha_type(), target_color_space);
    if *this.image_info().color_info() == dst_color_info {
        return Some(this.clone());
    }
    copy_as_draw(
        recorder,
        None,
        this,
        image.bounds(),
        &dst_color_info,
        Budgeted::No,
        if required_props.mipmapped {
            Mipmapped::Yes
        } else {
            Mipmapped::No
        },
        BackingFit::Exact,
        &get_chained_label(image, "ImageMakeCTandCSTexture", "_CTandCSConversion"),
    )
}

/// `Image::WrapDevice`'s view-level part: the image of a device's target, read with `override_info`
/// when one is given. `None` if the target cannot be sampled or the override does not fit the
/// texture's format or the device's alpha type.
// Port of: src/gpu/graphite/Image_Graphite.cpp#L49-L85 (chrome/m156), without `linkDevice`
#[doc(alias = "WrapDevice")]
#[must_use]
pub fn wrap_device(
    target: &TextureProxyView,
    texturable: bool,
    device_info: &ImageInfo,
    override_info: Option<ColorInfo>,
) -> Option<Image> {
    if !target.is_valid() || !texturable {
        return None;
    }
    let format = target.proxy()?.format();
    let (view, info) = match override_info {
        Some(override_info) => {
            // If an overrideInfo is provided, it needs to be compatible with the format still and
            // the changes to alpha type need to make sense.
            if !are_color_type_and_format_compatible(override_info.color_type(), format) {
                return None;
            }
            // For alpha type, it should match the device's alpha type or be changing from opaque
            // back to premul or unpremul.
            let override_at = override_info.alpha_type();
            let dev_at = device_info.alpha_type();
            if override_at != dev_at
                && (dev_at != AlphaType::Opaque
                    || (override_at != AlphaType::Premul && override_at != AlphaType::Unpremul))
            {
                return None;
            }
            // Update the swizzle on the texture view to match the change in color type.
            let read_swizzle = read_swizzle_for_color_type(override_info.color_type(), format);
            (target.replace_swizzle(read_swizzle), override_info)
        }
        // Leave the texture view alone since its swizzle should match the device already.
        None => (target.clone(), device_info.color_info().clone()),
    };
    // The image's dimensions are the target's, which can be larger than the device's when the
    // device was created with an approximate backing fit.
    Some(Image::new(view, &info))
}

impl ImageBase for Image {
    // Port of: src/gpu/graphite/Image_Graphite.h#L38-L44 (chrome/m156)
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    fn image_type(&self) -> ImageType {
        ImageType::Graphite
    }

    // `Image_Base::onReadPixels` is a no-op for Graphite: reading needs a Context.
    fn on_read_pixels(
        &self,
        _dst_info: &ImageInfo,
        _dst_pixels: &mut [u8],
        _dst_row_bytes: usize,
        _src_x: i32,
        _src_y: i32,
    ) -> bool {
        false
    }

    // Port of: src/gpu/graphite/Image_Graphite.h#L48-L50 (chrome/m156)
    fn on_has_mipmaps(&self) -> bool {
        self.has_mipmaps()
    }

    // Port of: src/gpu/graphite/Image_Graphite.h#L51-L53 (chrome/m156)
    fn on_is_protected(&self) -> bool {
        self.texture_proxy_view.is_protected() == Protected::Yes
    }

    fn get_ro_pixels(&self) -> Option<Bitmap> {
        None
    }

    // The subset of a Graphite image is copied with a recorder: see `make_subset_with_recorder`.
    fn on_make_subset(
        &self,
        _subset: &IRect,
        _required_properties: RequiredProperties,
    ) -> Option<CoreImage> {
        None
    }

    // Port of: src/gpu/graphite/Image_Graphite.cpp#L148-L156 (chrome/m156)
    fn on_reinterpret_color_space(&self, new_cs: ColorSpace) -> Option<CoreImage> {
        Some(self.reinterpret_color_space(Some(new_cs)))
    }

    fn make_color_type_and_color_space(
        &self,
        _target_color_type: ColorType,
        _target_color_space: Option<ColorSpace>,
        _required_properties: RequiredProperties,
    ) -> Option<CoreImage> {
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

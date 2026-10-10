// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Image_YUVA_Graphite.{h,cpp} (chrome/m156)

//! `Image_YUVA`: a YUV(A) image whose planes are Graphite textures. Each of the Y, U, V and A
//! channels is a [`TextureProxyView`] over one of the planes, with a swizzle that moves the channel
//! into the R slot.

use std::any::Any;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{ColorChannel, ColorChannelFlag};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image as CoreImage, RequiredProperties};
use skia_rust_core::image_base::{ImageBase, ImageType};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::image_info_is_valid;
use skia_rust_core::pixel_ref::next_image_id;
use skia_rust_core::rect::IRect;
use skia_rust_core::swizzle::Swizzle;
use skia_rust_core::yuva_info::{YUVAChannels, YUVAInfo, YUVALocation, subsampling_factors};

use crate::gpu::gpu_types::{Mipmapped, Protected};
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::caps::Caps;
use crate::graphite::image_graphite::{Image as GraphiteImage, ImageLinks};
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::texture_info::texture_info_priv;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::texture_utils::as_view;

/// `kAssumedColorType`: the colour type a YUVA image reports (the planes are converted in the
/// shader).
// Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L22 (chrome/m156)
const ASSUMED_COLOR_TYPE: ColorType = ColorType::RGBA8888;

/// `yuva_alpha_type(yuvaInfo)`: an image with an alpha plane is always premultiplied, since the
/// client expects premul even though the planar data is unpremultiplied.
// Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L32-L38 (chrome/m156)
fn yuva_alpha_type(yuva_info: &YUVAInfo) -> AlphaType {
    if yuva_info.has_alpha() {
        AlphaType::Premul
    } else {
        AlphaType::Opaque
    }
}

/// `Image_YUVA`: a YUV(A) image over Graphite textures.
// Port of: src/gpu/graphite/Image_YUVA_Graphite.h#L21-L76 (chrome/m156)
#[doc(alias = "skgpu::graphite::Image_YUVA")]
#[derive(Debug)]
pub struct ImageYuva {
    info: ImageInfo,
    unique_id: u32,
    // The proxy views are ordered Y, U, V, A. If channels share a plane, the views share the
    // TextureProxy but have the swizzle that reads that channel into the R slot. The alpha view
    // may be empty.
    proxies: [TextureProxyView; YUVAChannels::COUNT],
    yuva_info: YUVAInfo,
    uv_subsample_factors: (i32, i32),
    // Aggregate mipmap/protected status from the proxies.
    mipmapped: Mipmapped,
    protected: Protected,
    // `Image_Base::fLinkedDevices`.
    pub(crate) links: ImageLinks,
}

impl ImageYuva {
    /// `Image_YUVA(proxies, yuvaInfo, imageColorSpace)`: the channel views are already ordered
    /// Y, U, V, A.
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L53-L84 (chrome/m156)
    fn from_channel_proxies(
        proxies: [TextureProxyView; YUVAChannels::COUNT],
        yuva_info: &YUVAInfo,
        image_color_space: Option<ColorSpace>,
    ) -> Self {
        let info = ImageInfo::new(
            yuva_info.dimensions(),
            ASSUMED_COLOR_TYPE,
            yuva_alpha_type(yuva_info),
            image_color_space,
        );
        let mut mipmapped = Mipmapped::Yes;
        let mut protected = Protected::No;
        for (i, proxy) in proxies.iter().enumerate() {
            if !proxy.is_valid() {
                debug_assert_eq!(i, YUVAChannels::A as usize);
                continue;
            }
            if proxy.mipmapped() == Mipmapped::No {
                mipmapped = Mipmapped::No;
            }
            if proxy.is_protected() == Protected::Yes {
                protected = Protected::Yes;
            }
        }
        Self {
            info,
            // Graphite does not cache based on the image's unique ID, so always request a new one
            // (`kNeedNewImageUniqueID`).
            unique_id: next_image_id(),
            proxies,
            yuva_info: *yuva_info,
            uv_subsample_factors: subsampling_factors(yuva_info.subsampling()),
            mipmapped,
            protected,
            links: ImageLinks::default(),
        }
    }

    /// `Image_YUVA::Make(caps, yuvaInfo, planes, imageColorSpace)`: an image over `planes`, one
    /// view per plane of `yuva_info`. `None` if the info or the planes do not make a valid
    /// multiplane image.
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L88-L175 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        caps: &dyn Caps,
        yuva_info: &YUVAInfo,
        planes: &[TextureProxyView],
        image_color_space: Option<ColorSpace>,
    ) -> Option<ImageYuva> {
        if !yuva_info.is_valid() {
            return None;
        }
        let info = ImageInfo::new(
            yuva_info.dimensions(),
            ASSUMED_COLOR_TYPE,
            yuva_alpha_type(yuva_info),
            image_color_space.clone(),
        );
        if !image_info_is_valid(&info) {
            return None;
        }

        // Invoke the PlaneProxyFactoryFn for each plane and validate it against the plane config.
        let num_planes = yuva_info.num_planes();
        let plane_dimensions = yuva_info.plane_dimensions();
        if num_planes != plane_dimensions.len() {
            return None;
        }
        let mut pixmap_channel_masks = [ColorChannelFlag::from_bits(0); YUVAInfo::MAX_PLANES];
        for i in 0..num_planes {
            let plane = planes.get(i)?;
            let proxy = plane.proxy()?;
            if !caps.is_texturable(proxy.texture_info(), false) {
                return None;
            }
            if plane.dimensions() != plane_dimensions[i] {
                return None;
            }
            pixmap_channel_masks[i] =
                ColorChannelFlag::from_bits(texture_info_priv::channel_mask(proxy.texture_info()));
        }

        // Re-arrange the proxies from planes to channels.
        let locations = yuva_info.to_yuva_locations(&pixmap_channel_masks)?;
        let (valid_locations, expected_planes) = YUVALocation::are_valid_locations(&locations);
        if !valid_locations || expected_planes != num_planes {
            return None;
        }

        // The Y channel should match the YUVAInfo dimensions.
        let plane_of = |channel: YUVAChannels| -> Option<&TextureProxyView> {
            usize::try_from(locations[channel as usize].plane)
                .ok()
                .and_then(|plane| planes.get(plane))
        };
        let y_plane = plane_of(YUVAChannels::Y)?;
        if y_plane.dimensions() != yuva_info.dimensions() {
            return None;
        }

        // The UV channels should have planes with the same dimensions and subsampling factor.
        let u_plane = plane_of(YUVAChannels::U)?;
        let v_plane = plane_of(YUVAChannels::V)?;
        if u_plane.dimensions() != v_plane.dimensions() {
            return None;
        }

        // If the A channel is present, it should match the Y channel.
        if locations[YUVAChannels::A as usize].plane >= 0 {
            let a_plane = plane_of(YUVAChannels::A)?;
            if a_plane.dimensions() != yuva_info.dimensions() {
                return None;
            }
        }

        if yuva_info.plane_subsampling_factors(locations[YUVAChannels::U as usize].plane)
            != yuva_info.plane_subsampling_factors(locations[YUVAChannels::V as usize].plane)
        {
            return None;
        }

        // Re-arrange into YUVA channel order and apply the location to the swizzle.
        let mut channel_proxies: [TextureProxyView; YUVAChannels::COUNT] =
            std::array::from_fn(|_| TextureProxyView::default());
        for (i, channel_proxy) in channel_proxies.iter_mut().enumerate() {
            let YUVALocation { plane, mut channel } = locations[i];
            if let Ok(plane) = usize::try_from(plane) {
                let view = &planes[plane];
                let format = view.proxy().map(|proxy| proxy.format());
                // Compose the YUVA location with the data's read swizzle. This maps the data into
                // the R channel for the rest of the YUV shader logic. We add an extra check to
                // detect alpha-only colortype swizzles (e.g. 000r), which can show up when
                // wrapping single-channel planar data (the public APIs accept A8 or R8).
                if view.swizzle() == Swizzle::new("000r") || format == Some(TextureFormat::A8) {
                    // Pull the alpha channel into R: this is equivalent to having concatenated
                    // Swizzle("aaaa") with the plane's read swizzle.
                    channel = ColorChannel::A;
                }
                let channel_swizzle = view.swizzle().select_channel_in_r(channel as usize);
                // Use replace_swizzle() since select_channel_in_r effectively includes a concat.
                *channel_proxy = view.replace_swizzle(channel_swizzle);
            } else if i == YUVAChannels::A as usize {
                // The alpha channel is allowed to be absent: its view stays empty.
            } else {
                skia_log_w!("YUVA channel {i} does not have a valid location");
                return None;
            }
        }

        Some(Self::from_channel_proxies(
            channel_proxies,
            yuva_info,
            image_color_space,
        ))
    }

    /// `Image_YUVA::WrapImages(caps, yuvaInfo, images, imageColorSpace)`: a YUVA image over the
    /// planes of the Graphite-backed `images`. The result shares the textures of the planes, so it
    /// inherits their linked devices.
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L177-L204 (chrome/m156)
    #[doc(alias = "WrapImages")]
    #[must_use]
    pub fn wrap_images(
        caps: &dyn Caps,
        yuva_info: &YUVAInfo,
        images: &[CoreImage],
        image_color_space: Option<ColorSpace>,
    ) -> Option<ImageYuva> {
        let num_planes = yuva_info.num_planes();
        if images.len() < num_planes {
            return None;
        }
        let mut planes: [TextureProxyView; YUVAInfo::MAX_PLANES] =
            std::array::from_fn(|_| TextureProxyView::default());
        for (plane, image) in planes.iter_mut().zip(images).take(num_planes) {
            // A null image, or not graphite-backed, or not backed by a single texture.
            *plane = as_view(Some(image));
            if !plane.is_valid() {
                return None;
            }
        }
        let image = Self::make(caps, yuva_info, &planes[..num_planes], image_color_space)?;
        // Unlike the other factories, this YUVA image shares the texture proxies with each plane
        // image, so if those are linked to Devices, it must inherit those same links.
        for plane_image in images.iter().take(num_planes) {
            debug_assert!(plane_image.as_base().is_graphite_backed());
            if let Some(graphite) = GraphiteImage::from_core(plane_image) {
                image.links.link_devices(graphite.links());
            }
        }
        Some(image)
    }

    /// `Image_YUVA::textureSize()`: the GPU memory of the distinct textures of the planes.
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L206-L235 (chrome/m156)
    #[doc(alias = "textureSize")]
    #[must_use]
    pub fn texture_size(&self) -> usize {
        // We could look at the plane config and plane count to determine how many different
        // textures to expect, but a plane can be aliased, so sum the total GPU memory of the
        // non-duplicate textures.
        let mut size = 0;
        for i in 0..YUVAChannels::COUNT {
            let Some(proxy) = self.proxies[i].proxy() else {
                continue; // Null channels (A) have no size.
            };
            let repeat = (0..i).rev().any(|j| {
                self.proxies[j]
                    .proxy()
                    .is_some_and(|other| Arc::ptr_eq(proxy, other))
            });
            if !repeat {
                size += proxy.with_texture(|texture| match texture {
                    None => proxy.uninstantiated_gpu_memory_size(),
                    Some(texture) => texture.base().gpu_memory_size(),
                });
            }
        }
        size
    }

    /// `onReinterpretColorSpace(newCS)`: an image over the same planes with another colour space.
    /// It shares the linked devices of this image.
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L237-L244 (chrome/m156)
    #[doc(alias = "onReinterpretColorSpace")]
    #[must_use]
    pub fn reinterpret_color_space(&self, new_cs: Option<ColorSpace>) -> CoreImage {
        let view = Self::from_channel_proxies(self.proxies.clone(), &self.yuva_info, new_cs);
        // The new image object shares the same texture planes, so it should also share linked
        // devices.
        view.links.link_devices(&self.links);
        view.into_core()
    }

    /// The proxy view that provides the value of the YUVA channel `channel_index` (`proxyView`).
    /// The view applies a swizzle that maps the data channel into all slots of the sample value.
    /// The alpha view may be empty.
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.h#L51-L56 (chrome/m156)
    #[doc(alias = "proxyView")]
    #[must_use]
    pub fn proxy_view(&self, channel_index: usize) -> &TextureProxyView {
        &self.proxies[channel_index]
    }

    /// The subsampling factors of the U and V planes (`uvSubsampleFactors`).
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.h#L58 (chrome/m156)
    #[doc(alias = "uvSubsampleFactors")]
    #[must_use]
    pub fn uv_subsample_factors(&self) -> (i32, i32) {
        self.uv_subsample_factors
    }

    /// The YUVA layout of the image (`yuvaInfo`).
    // Port of: src/gpu/graphite/Image_YUVA_Graphite.h#L59 (chrome/m156)
    #[doc(alias = "yuvaInfo")]
    #[must_use]
    pub fn yuva_info(&self) -> &YUVAInfo {
        &self.yuva_info
    }

    /// Wraps this image as a core `SkImage` handle (`sk_sp<Image_YUVA>`).
    #[must_use]
    pub fn into_core(self) -> CoreImage {
        CoreImage::from_base(self)
    }

    /// The YUVA image behind a core image, if it is one (`static_cast<const Image_YUVA*>` after
    /// `isYUVA()`).
    #[must_use]
    pub fn from_core(image: &CoreImage) -> Option<&ImageYuva> {
        image.as_base().as_any().downcast_ref::<ImageYuva>()
    }

    /// `linkDevices` / `notifyInUse` / `unlinkDevices` share the links of the image.
    #[must_use]
    pub fn links(&self) -> &ImageLinks {
        &self.links
    }
}

/// The image links of any Graphite image: a [`GraphiteImage`] or a YUVA image.
#[must_use]
pub fn image_links(image: &CoreImage) -> Option<&ImageLinks> {
    if let Some(graphite) = GraphiteImage::from_core(image) {
        return Some(graphite.links());
    }
    ImageYuva::from_core(image).map(ImageYuva::links)
}

impl ImageBase for ImageYuva {
    // Port of: src/gpu/graphite/Image_Base_Graphite.cpp (chrome/m156), the image info
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    fn image_type(&self) -> ImageType {
        ImageType::GraphiteYuva
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

    // Port of: src/gpu/graphite/Image_YUVA_Graphite.h#L38 (chrome/m156)
    fn on_has_mipmaps(&self) -> bool {
        self.mipmapped == Mipmapped::Yes
    }

    // Port of: src/gpu/graphite/Image_YUVA_Graphite.h#L39 (chrome/m156)
    fn on_is_protected(&self) -> bool {
        self.protected == Protected::Yes
    }

    fn get_ro_pixels(&self) -> Option<Bitmap> {
        None
    }

    fn on_make_subset(
        &self,
        _subset: &IRect,
        _required_properties: RequiredProperties,
    ) -> Option<CoreImage> {
        None
    }

    // Port of: src/gpu/graphite/Image_YUVA_Graphite.cpp#L237-L244 (chrome/m156)
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

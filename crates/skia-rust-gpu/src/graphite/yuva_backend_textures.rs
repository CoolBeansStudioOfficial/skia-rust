// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/YUVABackendTextures.h and
// src/gpu/graphite/YUVABackendTextures.cpp (chrome/m156)

//! The backend textures of a YUVA image: [`YUVABackendTextureInfo`] describes the textures a
//! planar image needs, and [`YUVABackendTextures`] holds the textures themselves.

use std::array;

use skia_rust_core::color::ColorChannelFlag;
use skia_rust_core::image_info::YUVColorSpace;
use skia_rust_core::yuva_info::{YUVAInfo, YUVALocations};

use crate::gpu::gpu_types::Mipmapped;
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};

/// `YUVABackendTextureInfo::kMaxPlanes` (`SkYUVAInfo::kMaxPlanes`).
const MAX_PLANES: usize = YUVAInfo::MAX_PLANES;

/// `num_channels(ChannelMasks)`: the number of channels a set of channel flags describes, 0 for
/// an unknown set.
// Port of: src/gpu/graphite/YUVABackendTextures.cpp#L19-L32 (chrome/m156)
fn num_channels(channel_masks: u32) -> usize {
    let red = ColorChannelFlag::RED.bits();
    let alpha = ColorChannelFlag::ALPHA.bits();
    let gray = ColorChannelFlag::GRAY.bits();
    let rg = ColorChannelFlag::RG.bits();
    let rgb = ColorChannelFlag::RGB.bits();
    let rgba = ColorChannelFlag::RGBA.bits();
    if channel_masks == red || channel_masks == alpha || channel_masks == gray {
        1
    } else if channel_masks == (gray | alpha) || channel_masks == rg {
        2
    } else if channel_masks == rgb {
        3
    } else if channel_masks == rgba {
        4
    } else {
        0
    }
}

/// `YUVABackendTextureInfo`: the texture infos of the planes of a YUVA image, with the channel
/// flags of each plane. The default is invalid.
// Port of: include/gpu/graphite/YUVABackendTextures.h#L24-L95 (chrome/m156)
#[derive(Clone, Debug)]
pub struct YUVABackendTextureInfo {
    yuva_info: YUVAInfo,
    plane_texture_infos: [TextureInfo; MAX_PLANES],
    plane_channel_masks: [u32; MAX_PLANES],
    mipmapped: Mipmapped,
}

impl Default for YUVABackendTextureInfo {
    /// `YUVABackendTextureInfo() = default`: invalid.
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L29-L30 (chrome/m156)
    fn default() -> Self {
        Self {
            yuva_info: YUVAInfo::default(),
            plane_texture_infos: array::from_fn(|_| TextureInfo::default()),
            plane_channel_masks: [0; MAX_PLANES],
            mipmapped: Mipmapped::No,
        }
    }
}

impl PartialEq for YUVABackendTextureInfo {
    // Port of: src/gpu/graphite/YUVABackendTextures.cpp#L63-L68 (chrome/m156)
    #[doc(alias = "operator==")]
    fn eq(&self, that: &Self) -> bool {
        if self.yuva_info != that.yuva_info || self.mipmapped != that.mipmapped {
            return false;
        }
        self.plane_texture_infos == that.plane_texture_infos
    }
}

impl YUVABackendTextureInfo {
    /// Initializes a `YUVABackendTextureInfo` to describe a set of textures that can store the
    /// planes indicated by `yuva_info`. The texture dimensions are taken from the info's plane
    /// dimensions. All the described textures share a common origin. The planar image this
    /// describes will be mip mapped if all the textures are individually mip mapped as indicated
    /// by `mipmapped`. This is invalid (not [`is_valid`](Self::is_valid)) if the textures' formats'
    /// channels don't agree with `yuva_info`.
    // Port of: src/gpu/graphite/YUVABackendTextures.cpp#L34-L61 (chrome/m156)
    #[doc(alias = "YUVABackendTextureInfo")]
    #[must_use]
    pub fn new(yuva_info: YUVAInfo, texture_infos: &[TextureInfo], mipmapped: Mipmapped) -> Self {
        let num_planes = yuva_info.num_planes();
        if !yuva_info.is_valid() || num_planes == 0 || num_planes > texture_infos.len() {
            return Self::default();
        }
        let mut this = Self {
            yuva_info,
            mipmapped,
            ..Self::default()
        };
        for (i, texture_info) in texture_infos.iter().enumerate().take(num_planes) {
            let num_required_channels = yuva_info.num_channels_in_plane(i).unwrap_or(0);
            this.plane_channel_masks[i] = texture_info_priv::channel_mask(texture_info);
            if !texture_info.is_valid()
                || texture_info.backend() != texture_infos[0].backend()
                || num_channels(this.plane_channel_masks[i]) < num_required_channels
            {
                return Self::default();
            }
            this.plane_texture_infos[i] = texture_info.clone();
        }
        this
    }

    /// The `TextureInfo` for the `i`th plane, or an invalid one if `i >= num_planes()`.
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L51-L55 (chrome/m156)
    #[doc(alias = "planeTextureInfo")]
    #[must_use]
    pub fn plane_texture_info(&self, i: usize) -> &TextureInfo {
        &self.plane_texture_infos[i]
    }

    /// The YUVA layout of the planes.
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L57 (chrome/m156)
    #[doc(alias = "yuvaInfo")]
    #[must_use]
    pub fn yuva_info(&self) -> &YUVAInfo {
        &self.yuva_info
    }

    /// The colour space of the YUV values (`yuvColorSpace`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L58 (chrome/m156)
    #[doc(alias = "yuvColorSpace")]
    #[must_use]
    pub fn yuv_color_space(&self) -> YUVColorSpace {
        self.yuva_info.yuv_color_space()
    }

    /// Whether the planes are mip mapped (`mipmapped`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L60 (chrome/m156)
    #[must_use]
    pub fn mipmapped(&self) -> Mipmapped {
        self.mipmapped
    }

    /// The number of planes, 0 if this info is invalid (`numPlanes`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L63 (chrome/m156)
    #[doc(alias = "numPlanes")]
    #[must_use]
    pub fn num_planes(&self) -> usize {
        self.yuva_info.num_planes()
    }

    /// Whether this info has a valid `YUVAInfo` with compatible texture formats (`isValid`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L68 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.yuva_info.is_valid()
    }

    /// The locations of Y, U, V and A in the planes (`toYUVALocations`).
    // Port of: src/gpu/graphite/YUVABackendTextures.cpp#L70-L78 (chrome/m156)
    #[doc(alias = "toYUVALocations")]
    #[must_use]
    pub fn to_yuva_locations(&self) -> Option<YUVALocations> {
        self.yuva_info
            .to_yuva_locations(&plane_flags(&self.plane_channel_masks))
    }
}

/// `YUVABackendTextures`: the backend textures of a YUVA image's planes. The default is invalid.
// Port of: include/gpu/graphite/YUVABackendTextures.h#L97-L148 (chrome/m156)
#[derive(Clone, Debug)]
pub struct YUVABackendTextures {
    yuva_info: YUVAInfo,
    plane_textures: [BackendTexture; MAX_PLANES],
    plane_channel_masks: [u32; MAX_PLANES],
}

impl Default for YUVABackendTextures {
    /// `YUVABackendTextures() = default`: invalid.
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L103 (chrome/m156)
    fn default() -> Self {
        Self {
            yuva_info: YUVAInfo::default(),
            plane_textures: array::from_fn(|_| BackendTexture::new()),
            plane_channel_masks: [0; MAX_PLANES],
        }
    }
}

impl YUVABackendTextures {
    /// Initializes a `YUVABackendTextures` from a set of textures that store the planes indicated
    /// by `yuva_info`. This is invalid if the textures' formats' channels don't agree with
    /// `yuva_info`.
    // Port of: src/gpu/graphite/YUVABackendTextures.cpp#L80-L108 (chrome/m156)
    #[doc(alias = "YUVABackendTextures")]
    #[must_use]
    pub fn new(yuva_info: YUVAInfo, textures: &[BackendTexture]) -> Self {
        if !yuva_info.is_valid() {
            return Self::default();
        }
        let plane_dimensions = yuva_info.plane_dimensions();
        let num_planes = plane_dimensions.len();
        if num_planes == 0 || num_planes > textures.len() {
            return Self::default();
        }
        let mut this = Self {
            yuva_info,
            ..Self::default()
        };
        for (i, plane_dimension) in plane_dimensions.iter().enumerate() {
            let num_required_channels = yuva_info.num_channels_in_plane(i).unwrap_or(0);
            this.plane_channel_masks[i] = texture_info_priv::channel_mask(&textures[i].info());
            if !textures[i].is_valid()
                || textures[i].dimensions() != *plane_dimension
                || textures[i].backend() != textures[0].backend()
                || num_channels(this.plane_channel_masks[i]) < num_required_channels
            {
                // Skia asserts that the result is invalid here; the release build keeps the
                // info, which this port does not reproduce.
                return Self::default();
            }
            this.plane_textures[i] = textures[i].clone();
        }
        this
    }

    /// The textures of all the planes (`planeTextures`): the array of [`MAX_PLANES`] entries, the
    /// ones past [`num_planes`](Self::num_planes) invalid.
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L111-L113 (chrome/m156)
    #[doc(alias = "planeTextures")]
    #[must_use]
    pub fn plane_textures(&self) -> &[BackendTexture] {
        &self.plane_textures
    }

    /// The texture of the `i`th plane, or an invalid one if `i >= num_planes()` (`planeTexture`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L117-L119 (chrome/m156)
    #[doc(alias = "planeTexture")]
    #[must_use]
    pub fn plane_texture(&self, i: usize) -> BackendTexture {
        self.plane_textures[i].clone()
    }

    /// The YUVA layout of the planes.
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L121 (chrome/m156)
    #[doc(alias = "yuvaInfo")]
    #[must_use]
    pub fn yuva_info(&self) -> &YUVAInfo {
        &self.yuva_info
    }

    /// The colour space of the YUV values (`yuvColorSpace`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L122 (chrome/m156)
    #[doc(alias = "yuvColorSpace")]
    #[must_use]
    pub fn yuv_color_space(&self) -> YUVColorSpace {
        self.yuva_info.yuv_color_space()
    }

    /// The number of planes, 0 if this object is invalid (`numPlanes`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L125 (chrome/m156)
    #[doc(alias = "numPlanes")]
    #[must_use]
    pub fn num_planes(&self) -> usize {
        self.yuva_info.num_planes()
    }

    /// Whether this object has a valid `YUVAInfo` with compatible texture formats (`isValid`).
    // Port of: include/gpu/graphite/YUVABackendTextures.h#L131 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.yuva_info.is_valid()
    }

    /// The locations of Y, U, V and A in the planes (`toYUVALocations`).
    // Port of: src/gpu/graphite/YUVABackendTextures.cpp#L110-L117 (chrome/m156)
    #[doc(alias = "toYUVALocations")]
    #[must_use]
    pub fn to_yuva_locations(&self) -> Option<YUVALocations> {
        self.yuva_info
            .to_yuva_locations(&plane_flags(&self.plane_channel_masks))
    }
}

/// The per-plane channel masks as the flags `YUVAInfo::to_yuva_locations` takes.
fn plane_flags(masks: &[u32; MAX_PLANES]) -> [ColorChannelFlag; MAX_PLANES] {
    masks.map(ColorChannelFlag::from_bits)
}

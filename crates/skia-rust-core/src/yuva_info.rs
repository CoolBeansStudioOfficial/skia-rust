// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkYUVAInfo.h, src/core/SkYUVAInfo.cpp, src/core/SkYUVAInfoLocation.h

//! [`YUVAInfo`]: how YUV (and optionally alpha) values are divided among planes, and their
//! dimensions, subsampling, colour space and origin.

use crate::color::{ColorChannel, ColorChannelFlag};
use crate::encoded_origin::EncodedOrigin;
use crate::image_info::YUVColorSpace;
use crate::matrix::Matrix;
use crate::safe_math::SafeMath;
use crate::size::ISize;

/// The channels of a YUVA value (`SkYUVAInfo::YUVAChannels`).
// Port of: include/core/SkYUVAInfo.h#L22-L24 (chrome/m156)
#[doc(alias = "SkYUVAInfo::YUVAChannels")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(usize)]
pub enum YUVAChannels {
    /// Luma.
    #[doc(alias = "kY")]
    Y = 0,
    /// Blue-difference chroma.
    #[doc(alias = "kU")]
    U = 1,
    /// Red-difference chroma.
    #[doc(alias = "kV")]
    V = 2,
    /// Alpha.
    #[doc(alias = "kA")]
    A = 3,
}

impl YUVAChannels {
    /// `kYUVAChannelCount`: the number of YUVA channels.
    pub const COUNT: usize = 4;
}

/// Port of `SkYUVAInfo::PlaneConfig`: how the Y, U, V and A values are divided among planes.
/// The names are Skia's (the underscores separate the planes).
// Port of: include/core/SkYUVAInfo.h#L26-L48 (chrome/m156)
#[doc(alias = "SkYUVAInfo::PlaneConfig")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[allow(non_camel_case_types)] // Skia's names
pub enum PlaneConfig {
    /// Not a valid configuration.
    #[default]
    #[doc(alias = "kUnknown")]
    Unknown,
    /// Plane 0: Y, Plane 1: U, Plane 2: V.
    #[doc(alias = "kY_U_V")]
    Y_U_V,
    /// Plane 0: Y, Plane 1: V, Plane 2: U.
    #[doc(alias = "kY_V_U")]
    Y_V_U,
    /// Plane 0: Y, Plane 1: UV.
    #[doc(alias = "kY_UV")]
    Y_UV,
    /// Plane 0: Y, Plane 1: VU.
    #[doc(alias = "kY_VU")]
    Y_VU,
    /// Plane 0: YUV.
    #[doc(alias = "kYUV")]
    YUV,
    /// Plane 0: UYV.
    #[doc(alias = "kUYV")]
    UYV,
    /// Plane 0: Y, Plane 1: U, Plane 2: V, Plane 3: A.
    #[doc(alias = "kY_U_V_A")]
    Y_U_V_A,
    /// Plane 0: Y, Plane 1: V, Plane 2: U, Plane 3: A.
    #[doc(alias = "kY_V_U_A")]
    Y_V_U_A,
    /// Plane 0: Y, Plane 1: UV, Plane 2: A.
    #[doc(alias = "kY_UV_A")]
    Y_UV_A,
    /// Plane 0: Y, Plane 1: VU, Plane 2: A.
    #[doc(alias = "kY_VU_A")]
    Y_VU_A,
    /// Plane 0: YUVA.
    #[doc(alias = "kYUVA")]
    YUVA,
    /// Plane 0: UYVA.
    #[doc(alias = "kUYVA")]
    UYVA,
}

impl PlaneConfig {
    /// `kLast`.
    #[doc(alias = "kLast")]
    pub const LAST: Self = Self::UYVA;
}

/// Port of `SkYUVAInfo::Subsampling`: the UV subsampling, named by Skia's J:a:b notation.
// Port of: include/core/SkYUVAInfo.h#L50-L62 (chrome/m156)
#[doc(alias = "SkYUVAInfo::Subsampling")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum Subsampling {
    /// Not a valid subsampling.
    #[default]
    #[doc(alias = "kUnknown")]
    Unknown,
    /// No subsampling. UV values for each Y.
    #[doc(alias = "k444")]
    S444,
    /// 1 set of UV values for each 2x1 block of Y values.
    #[doc(alias = "k422")]
    S422,
    /// 1 set of UV values for each 2x2 block of Y values.
    #[doc(alias = "k420")]
    S420,
    /// 1 set of UV values for each 1x2 block of Y values.
    #[doc(alias = "k440")]
    S440,
    /// 1 set of UV values for each 4x1 block of Y values.
    #[doc(alias = "k411")]
    S411,
    /// 1 set of UV values for each 4x2 block of Y values.
    #[doc(alias = "k410")]
    S410,
}

/// Port of `SkYUVAInfo::Siting`: how subsampled chroma is sited relative to luma.
// Port of: include/core/SkYUVAInfo.h#L64-L66 (chrome/m156)
#[doc(alias = "SkYUVAInfo::Siting")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum Siting {
    /// Centered siting, the only one Skia supports.
    #[default]
    #[doc(alias = "kCentered")]
    Centered,
}

/// The location of a Y, U, V or A value within the planes of a [`YUVAInfo`]
/// (`SkYUVAInfo::YUVALocation`).
// Port of: src/core/SkYUVAInfoLocation.h#L18-L26 (chrome/m156)
#[doc(alias = "SkYUVAInfo::YUVALocation")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct YUVALocation {
    /// The index of the plane where the Y, U, V, or A value is to be found, or -1 when the
    /// value is absent (only A can be).
    pub plane: i32,
    /// The channel in the plane that contains the Y, U, V, or A value.
    pub channel: ColorChannel,
}

impl Default for YUVALocation {
    // Port of: src/core/SkYUVAInfoLocation.h#L18-L20 (chrome/m156), the member initializers
    fn default() -> Self {
        Self {
            plane: -1,
            channel: ColorChannel::A,
        }
    }
}

impl YUVALocation {
    /// Port of `YUVALocation::AreValidLocations`: whether the locations form a packed set of
    /// planes (every plane up to the largest one used is used), and the number of planes.
    // Port of: src/core/SkYUVAInfoLocation.h#L28-L56 (chrome/m156)
    #[doc(alias = "AreValidLocations")]
    #[must_use]
    pub fn are_valid_locations(locations: &YUVALocations) -> (bool, usize) {
        let mut max_slot_used: i32 = -1;
        let mut used = [false; YUVAInfo::MAX_PLANES];
        let mut valid = true;
        for (i, loc) in locations.iter().enumerate() {
            if loc.plane < 0 {
                if i != YUVAChannels::A as usize {
                    valid = false; // only the 'A' plane can be omitted
                }
            } else if loc.plane >= YUVAInfo::MAX_PLANES as i32 {
                valid = false; // A maximum of four input textures is allowed
            } else {
                max_slot_used = max_slot_used.max(loc.plane);
                used[i] = true;
            }
        }
        // All the used slots should be packed starting at 0 with no gaps
        for i in 0..=max_slot_used {
            if !used[i as usize] {
                valid = false;
            }
        }
        let num_planes = if valid { (max_slot_used + 1) as usize } else { 0 };
        (valid, num_planes)
    }
}

/// The locations of the Y, U, V and A values (`SkYUVAInfo::YUVALocations`).
pub type YUVALocations = [YUVALocation; YUVAChannels::COUNT];

/// Port of `SkYUVAInfo`: the dimensions, plane configuration, subsampling, colour space, origin
/// and siting of a YUV(A) image. An invalid info has [`PlaneConfig::Unknown`].
// Port of: include/core/SkYUVAInfo.h#L19-L233 (chrome/m156)
#[doc(alias = "SkYUVAInfo")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct YUVAInfo {
    dimensions: ISize,
    plane_config: PlaneConfig,
    subsampling: Subsampling,
    yuv_color_space: YUVColorSpace,
    origin: EncodedOrigin,
    siting_x: Siting,
    siting_y: Siting,
}

impl Default for YUVAInfo {
    /// The invalid info (`SkYUVAInfo() = default`).
    // Port of: include/core/SkYUVAInfo.h#L113-L118 (chrome/m156), the member initializers
    fn default() -> Self {
        Self {
            dimensions: ISize::new(0, 0),
            plane_config: PlaneConfig::Unknown,
            subsampling: Subsampling::Unknown,
            yuv_color_space: YUVColorSpace::Identity,
            origin: EncodedOrigin::TopLeft,
            siting_x: Siting::Centered,
            siting_y: Siting::Centered,
        }
    }
}

// Port of: src/core/SkYUVAInfo.cpp#L14-L25 (is_plane_config_compatible_with_subsampling)
fn is_plane_config_compatible_with_subsampling(config: PlaneConfig, subsampling: Subsampling) -> bool {
    if config == PlaneConfig::Unknown || subsampling == Subsampling::Unknown {
        return false;
    }
    subsampling == Subsampling::S444
        || (config != PlaneConfig::YUV
            && config != PlaneConfig::YUVA
            && config != PlaneConfig::UYV
            && config != PlaneConfig::UYVA)
}

/// Port of `SkYUVAInfo::SubsamplingFactors`: the Y to UV ratio in x and y.
// Port of: src/core/SkYUVAInfo.cpp#L27-L38 (chrome/m156)
#[doc(alias = "SkYUVAInfo::SubsamplingFactors")]
#[must_use]
pub fn subsampling_factors(subsampling: Subsampling) -> (i32, i32) {
    match subsampling {
        Subsampling::Unknown => (0, 0),
        Subsampling::S444 => (1, 1),
        Subsampling::S422 => (2, 1),
        Subsampling::S420 => (2, 2),
        Subsampling::S440 => (1, 2),
        Subsampling::S411 => (4, 1),
        Subsampling::S410 => (4, 2),
    }
}

/// Port of `SkYUVAInfo::PlaneSubsamplingFactors`: the subsampling factors of plane `plane_idx`.
// Port of: src/core/SkYUVAInfo.cpp#L40-L72 (chrome/m156)
#[doc(alias = "SkYUVAInfo::PlaneSubsamplingFactors")]
#[must_use]
pub fn plane_subsampling_factors(
    plane_config: PlaneConfig,
    subsampling: Subsampling,
    plane_idx: i32,
) -> (i32, i32) {
    if !is_plane_config_compatible_with_subsampling(plane_config, subsampling)
        || plane_idx < 0
        || plane_idx > num_planes(plane_config) as i32
    {
        return (0, 0);
    }
    let is_subsampled_plane = match plane_config {
        PlaneConfig::Unknown => unreachable!("checked by is_plane_config_compatible"),
        PlaneConfig::Y_U_V | PlaneConfig::Y_V_U | PlaneConfig::Y_U_V_A | PlaneConfig::Y_V_U_A => {
            plane_idx == 1 || plane_idx == 2
        }
        PlaneConfig::Y_UV | PlaneConfig::Y_VU | PlaneConfig::Y_UV_A | PlaneConfig::Y_VU_A => {
            plane_idx == 1
        }
        PlaneConfig::YUV | PlaneConfig::UYV | PlaneConfig::YUVA | PlaneConfig::UYVA => false,
    };
    if is_subsampled_plane {
        subsampling_factors(subsampling)
    } else {
        (1, 1)
    }
}

/// The per-plane dimensions and the number of planes (`SkYUVAInfo::PlaneDimensions`). Entries past
/// the returned count are `ISize::new(0, 0)`.
// Port of: src/core/SkYUVAInfo.cpp#L74-L131 (chrome/m156)
#[doc(alias = "SkYUVAInfo::PlaneDimensions")]
#[must_use]
pub fn plane_dimensions_array(
    image_dimensions: ISize,
    plane_config: PlaneConfig,
    subsampling: Subsampling,
    origin: EncodedOrigin,
) -> (usize, [ISize; YUVAInfo::MAX_PLANES]) {
    let mut plane_dims = [ISize::new(0, 0); YUVAInfo::MAX_PLANES];
    if !is_plane_config_compatible_with_subsampling(plane_config, subsampling) {
        return (0, plane_dims);
    }
    let mut w = image_dimensions.width;
    let mut h = image_dimensions.height;
    if origin as i32 >= EncodedOrigin::LeftTop as i32 {
        std::mem::swap(&mut w, &mut h);
    }
    let down2 = |x: i32| (x + 1) / 2;
    let down4 = |x: i32| (x + 3) / 4;
    let uv_size = match subsampling {
        Subsampling::Unknown => unreachable!("checked by is_plane_config_compatible"),
        Subsampling::S444 => ISize::new(w, h),
        Subsampling::S422 => ISize::new(down2(w), h),
        Subsampling::S420 => ISize::new(down2(w), down2(h)),
        Subsampling::S440 => ISize::new(w, down2(h)),
        Subsampling::S411 => ISize::new(down4(w), h),
        Subsampling::S410 => ISize::new(down4(w), down2(h)),
    };
    let n = match plane_config {
        PlaneConfig::Unknown => unreachable!("checked by is_plane_config_compatible"),
        PlaneConfig::Y_U_V | PlaneConfig::Y_V_U => {
            plane_dims[0] = ISize::new(w, h);
            plane_dims[1] = uv_size;
            plane_dims[2] = uv_size;
            3
        }
        PlaneConfig::Y_UV | PlaneConfig::Y_VU => {
            plane_dims[0] = ISize::new(w, h);
            plane_dims[1] = uv_size;
            2
        }
        PlaneConfig::Y_U_V_A | PlaneConfig::Y_V_U_A => {
            plane_dims[0] = ISize::new(w, h);
            plane_dims[3] = ISize::new(w, h);
            plane_dims[1] = uv_size;
            plane_dims[2] = uv_size;
            4
        }
        PlaneConfig::Y_UV_A | PlaneConfig::Y_VU_A => {
            plane_dims[0] = ISize::new(w, h);
            plane_dims[2] = ISize::new(w, h);
            plane_dims[1] = uv_size;
            3
        }
        PlaneConfig::YUV | PlaneConfig::UYV | PlaneConfig::YUVA | PlaneConfig::UYVA => {
            plane_dims[0] = ISize::new(w, h);
            debug_assert!(plane_dims[0] == uv_size);
            1
        }
    };
    (n, plane_dims)
}

/// Port of `SkYUVAInfo::NumPlanes`.
// Port of: include/core/SkYUVAInfo.h#L125-L142 (chrome/m156)
#[doc(alias = "SkYUVAInfo::NumPlanes")]
#[must_use]
pub const fn num_planes(plane_config: PlaneConfig) -> usize {
    match plane_config {
        PlaneConfig::Unknown => 0,
        PlaneConfig::Y_U_V => 3,
        PlaneConfig::Y_V_U => 3,
        PlaneConfig::Y_UV => 2,
        PlaneConfig::Y_VU => 2,
        PlaneConfig::YUV => 1,
        PlaneConfig::UYV => 1,
        PlaneConfig::Y_U_V_A => 4,
        PlaneConfig::Y_V_U_A => 4,
        PlaneConfig::Y_UV_A => 3,
        PlaneConfig::Y_VU_A => 3,
        PlaneConfig::YUVA => 1,
        PlaneConfig::UYVA => 1,
    }
}

/// Port of `SkYUVAInfo::NumChannelsInPlane`, `None` where Skia returns 0 (no such plane).
// Port of: include/core/SkYUVAInfo.h#L144-L183 (chrome/m156)
#[doc(alias = "SkYUVAInfo::NumChannelsInPlane")]
#[must_use]
pub fn num_channels_in_plane(plane_config: PlaneConfig, i: usize) -> Option<usize> {
    let n = match plane_config {
        PlaneConfig::Unknown => 0,
        PlaneConfig::Y_U_V | PlaneConfig::Y_V_U => usize::from(i < 3),
        PlaneConfig::Y_UV | PlaneConfig::Y_VU => match i {
            0 => 1,
            1 => 2,
            _ => 0,
        },
        PlaneConfig::YUV | PlaneConfig::UYV => {
            if i == 0 {
                3
            } else {
                0
            }
        }
        PlaneConfig::Y_U_V_A | PlaneConfig::Y_V_U_A => usize::from(i < 4),
        PlaneConfig::Y_UV_A | PlaneConfig::Y_VU_A => match i {
            0 => 1,
            1 => 2,
            2 => 1,
            _ => 0,
        },
        PlaneConfig::YUVA | PlaneConfig::UYVA => {
            if i == 0 {
                4
            } else {
                0
            }
        }
    };
    (n > 0).then_some(n)
}

/// Port of `SkYUVAInfo::GetYUVALocations`: where each of Y, U, V and A is, given the channel
/// flags of each plane. `None` where Skia returns an empty (invalid) result.
// Port of: src/core/SkYUVAInfo.cpp#L134-L240 (chrome/m156)
#[doc(alias = "SkYUVAInfo::GetYUVALocations")]
#[must_use]
pub fn get_yuva_locations(
    plane_config: PlaneConfig,
    plane_channel_flags: &[ColorChannelFlag; YUVAInfo::MAX_PLANES],
) -> Option<YUVALocations> {
    // (plane, chanIdx) for each of Y, U, V and A, where chanIdx indexes the channels of the plane
    // rather than naming them: A is the 0th channel of an alpha-only texture.
    // `plane == -1` marks an absent value.
    let planes_and_indices: [(i32, i32); YUVAChannels::COUNT] = match plane_config {
        PlaneConfig::Unknown => return Some(YUVALocations::default()),
        PlaneConfig::Y_U_V => [(0, 0), (1, 0), (2, 0), (-1, -1)],
        PlaneConfig::Y_V_U => [(0, 0), (2, 0), (1, 0), (-1, -1)],
        PlaneConfig::Y_UV => [(0, 0), (1, 0), (1, 1), (-1, -1)],
        PlaneConfig::Y_VU => [(0, 0), (1, 1), (1, 0), (-1, -1)],
        PlaneConfig::YUV => [(0, 0), (0, 1), (0, 2), (-1, -1)],
        PlaneConfig::UYV => [(0, 1), (0, 0), (0, 2), (-1, -1)],
        PlaneConfig::Y_U_V_A => [(0, 0), (1, 0), (2, 0), (3, 0)],
        PlaneConfig::Y_V_U_A => [(0, 0), (2, 0), (1, 0), (3, 0)],
        PlaneConfig::Y_UV_A => [(0, 0), (1, 0), (1, 1), (2, 0)],
        PlaneConfig::Y_VU_A => [(0, 0), (1, 1), (1, 0), (2, 0)],
        PlaneConfig::YUVA => [(0, 0), (0, 1), (0, 2), (0, 3)],
        PlaneConfig::UYVA => [(0, 1), (0, 0), (0, 2), (0, 3)],
    };
    let mut yuva_locations = YUVALocations::default();
    for (i, &(plane, chan_idx)) in planes_and_indices.iter().enumerate() {
        if plane >= 0 {
            let channel = channel_index_to_channel(plane_channel_flags[plane as usize], chan_idx)?;
            yuva_locations[i] = YUVALocation {
                plane,
                channel,
            };
        } else {
            debug_assert!(i == YUVAChannels::A as usize);
            yuva_locations[i] = YUVALocation {
                plane: -1,
                channel: ColorChannel::R,
            };
        }
    }
    Some(yuva_locations)
}

/// Port of `channel_index_to_channel` (src/core/SkYUVAInfo.cpp): the channel at `channel_idx` of a
/// plane with `channel_flags`, or `None` when the plane has no such channel.
// Port of: src/core/SkYUVAInfo.cpp#L134-L179 (chrome/m156)
fn channel_index_to_channel(channel_flags: ColorChannelFlag, channel_idx: i32) -> Option<ColorChannel> {
    use ColorChannel::{A, B, G, R};
    match channel_flags {
        // For gray returning any of R, G, or B for index 0 is ok.
        f if f == ColorChannelFlag::GRAY || f == ColorChannelFlag::RED => {
            (channel_idx == 0).then_some(R)
        }
        f if f == ColorChannelFlag::GRAY_ALPHA => match channel_idx {
            0 => Some(R),
            1 => Some(A),
            _ => None,
        },
        f if f == ColorChannelFlag::ALPHA => (channel_idx == 0).then_some(A),
        f if f == ColorChannelFlag::RG => match channel_idx {
            0 => Some(R),
            1 => Some(G),
            _ => None,
        },
        f if f == ColorChannelFlag::RGB => match channel_idx {
            0 => Some(R),
            1 => Some(G),
            2 => Some(B),
            _ => None,
        },
        f if f == ColorChannelFlag::RGBA => match channel_idx {
            0 => Some(R),
            1 => Some(G),
            2 => Some(B),
            3 => Some(A),
            _ => None,
        },
        _ => None,
    }
}

/// Port of `SkYUVAInfo::HasAlpha`.
// Port of: src/core/SkYUVAInfo.cpp#L181-L199 (chrome/m156)
#[doc(alias = "SkYUVAInfo::HasAlpha")]
#[must_use]
pub const fn has_alpha(plane_config: PlaneConfig) -> bool {
    match plane_config {
        PlaneConfig::Unknown
        | PlaneConfig::Y_U_V
        | PlaneConfig::Y_V_U
        | PlaneConfig::Y_UV
        | PlaneConfig::Y_VU
        | PlaneConfig::YUV
        | PlaneConfig::UYV => false,
        PlaneConfig::Y_U_V_A
        | PlaneConfig::Y_V_U_A
        | PlaneConfig::Y_UV_A
        | PlaneConfig::Y_VU_A
        | PlaneConfig::YUVA
        | PlaneConfig::UYVA => true,
    }
}

impl YUVAInfo {
    /// `kMaxPlanes`.
    #[doc(alias = "kMaxPlanes")]
    pub const MAX_PLANES: usize = 4;

    /// Port of `SkYUVAInfo::SkYUVAInfo(SkISize, ...)`. Returns `None` (the invalid info of Skia) for
    /// empty dimensions or a plane configuration that does not fit the subsampling.
    // Port of: src/core/SkYUVAInfo.cpp#L201-L216 (chrome/m156)
    #[doc(alias = "SkYUVAInfo")]
    #[must_use]
    pub fn new(
        dimensions: impl Into<ISize>,
        plane_config: PlaneConfig,
        subsampling: Subsampling,
        yuv_color_space: YUVColorSpace,
        origin: impl Into<Option<EncodedOrigin>>,
        siting_xy: impl Into<Option<(Siting, Siting)>>,
    ) -> Option<Self> {
        let origin = origin.into().unwrap_or(EncodedOrigin::TopLeft);
        let (siting_x, siting_y) = siting_xy.into().unwrap_or((Siting::Centered, Siting::Centered));
        let info = Self::new_unchecked(
            dimensions.into(),
            plane_config,
            subsampling,
            yuv_color_space,
            origin,
            siting_x,
            siting_y,
        );
        info.is_valid().then_some(info)
    }

    // The C++ constructor: an empty or incompatible info is replaced by the invalid default.
    // Port of: src/core/SkYUVAInfo.cpp#L201-L216 (chrome/m156), the constructor body
    fn new_unchecked(
        dimensions: ISize,
        plane_config: PlaneConfig,
        subsampling: Subsampling,
        yuv_color_space: YUVColorSpace,
        origin: EncodedOrigin,
        siting_x: Siting,
        siting_y: Siting,
    ) -> Self {
        if dimensions.is_empty() || !is_plane_config_compatible_with_subsampling(plane_config, subsampling) {
            return Self::default();
        }
        Self {
            dimensions,
            plane_config,
            subsampling,
            yuv_color_space,
            origin,
            siting_x,
            siting_y,
        }
    }

    /// The plane configuration.
    #[must_use]
    pub fn plane_config(&self) -> PlaneConfig {
        self.plane_config
    }

    /// The subsampling of the chroma planes.
    #[must_use]
    pub fn subsampling(&self) -> Subsampling {
        self.subsampling
    }

    /// The subsampling factors of plane `plane_idx`.
    #[must_use]
    pub fn plane_subsampling_factors(&self, plane_idx: i32) -> (i32, i32) {
        plane_subsampling_factors(self.plane_config, self.subsampling, plane_idx)
    }

    /// The full-resolution dimensions of the image.
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.dimensions
    }

    /// The width of the full-resolution image.
    #[must_use]
    pub fn width(&self) -> i32 {
        self.dimensions.width
    }

    /// The height of the full-resolution image.
    #[must_use]
    pub fn height(&self) -> i32 {
        self.dimensions.height
    }

    /// The colour space of the YUV values.
    #[must_use]
    pub fn yuv_color_space(&self) -> YUVColorSpace {
        self.yuv_color_space
    }

    /// The siting of the chroma values, horizontally and vertically.
    #[must_use]
    pub fn siting_xy(&self) -> (Siting, Siting) {
        (self.siting_x, self.siting_y)
    }

    /// The origin of the image: how the planes are oriented for display.
    #[must_use]
    pub fn origin(&self) -> EncodedOrigin {
        self.origin
    }

    /// The matrix that maps the stored planes to the displayed orientation (`originMatrix`).
    #[must_use]
    pub fn origin_matrix(&self) -> Matrix {
        self.origin.to_matrix(self.width(), self.height())
    }

    /// The inverse of [`origin_matrix`](Self::origin_matrix) (`inverseOriginMatrix`).
    #[must_use]
    pub fn inverse_origin_matrix(&self) -> Matrix {
        self.origin.to_matrix_inverse(self.width(), self.height())
    }

    /// Whether the image has an alpha plane (`hasAlpha`).
    #[must_use]
    pub fn has_alpha(&self) -> bool {
        has_alpha(self.plane_config)
    }

    /// The dimensions of each plane, as stored (before the origin is applied).
    #[must_use]
    pub fn plane_dimensions(&self) -> Vec<ISize> {
        let (n, dims) = plane_dimensions_array(self.dimensions, self.plane_config, self.subsampling, self.origin);
        dims[..n].to_vec()
    }

    /// The number of planes (`numPlanes`).
    #[must_use]
    pub fn num_planes(&self) -> usize {
        num_planes(self.plane_config)
    }

    /// The number of channels in plane `i`, `None` for no such plane (`numChannelsInPlane`).
    #[must_use]
    pub fn num_channels_in_plane(&self, i: usize) -> Option<usize> {
        num_channels_in_plane(self.plane_config, i)
    }

    /// Port of `SkYUVAInfo::computeTotalBytes`: the bytes needed for all planes with the given row
    /// bytes, and optionally each plane's size. Returns `usize::MAX` on overflow.
    // Port of: src/core/SkYUVAInfo.cpp#L281-L305 (chrome/m156)
    #[doc(alias = "computeTotalBytes")]
    #[must_use]
    pub fn compute_total_bytes(
        &self,
        row_bytes: &[usize; Self::MAX_PLANES],
        plane_sizes: Option<&mut [usize; Self::MAX_PLANES]>,
    ) -> usize {
        if !self.is_valid() {
            return 0;
        }
        let mut safe = SafeMath::new();
        let mut total_bytes: usize = 0;
        let (n, plane_dimensions) =
            plane_dimensions_array(self.dimensions, self.plane_config, self.subsampling, self.origin);
        let mut sizes = [0usize; Self::MAX_PLANES];
        for i in 0..n {
            debug_assert!(!plane_dimensions[i].is_empty());
            debug_assert!(row_bytes[i] != 0);
            let size = safe.mul(row_bytes[i], plane_dimensions[i].height as usize);
            sizes[i] = size;
            total_bytes = safe.add(total_bytes, size);
        }
        if let Some(out) = plane_sizes {
            if safe.ok() {
                for slot in out.iter_mut().skip(n) {
                    *slot = 0;
                }
                out[..n].copy_from_slice(&sizes[..n]);
            } else {
                // Skia's loop here is `for (int i = 0; n < kMaxPlanes; ++i)`, whose condition
                // never changes, so it writes out of bounds. The intent is SIZE_MAX everywhere.
                // skia-rust: the intended behaviour, not the undefined loop.
                for slot in out.iter_mut() {
                    *slot = usize::MAX;
                }
            }
        }
        if safe.ok() { total_bytes } else { usize::MAX }
    }

    /// The locations of Y, U, V and A for the given per-plane channel flags
    /// (`toYUVALocations`), `None` where the flags do not fit the plane configuration.
    #[must_use]
    pub fn to_yuva_locations(
        &self,
        channel_flags: &[ColorChannelFlag; Self::MAX_PLANES],
    ) -> Option<YUVALocations> {
        get_yuva_locations(self.plane_config, channel_flags)
    }

    /// The same info with another subsampling, `None` where the plane configuration does not allow
    /// it (`makeSubsampling`).
    #[must_use]
    pub fn with_subsampling(&self, subsampling: Subsampling) -> Option<Self> {
        let info = Self::new_unchecked(
            self.dimensions,
            self.plane_config,
            subsampling,
            self.yuv_color_space,
            self.origin,
            self.siting_x,
            self.siting_y,
        );
        info.is_valid().then_some(info)
    }

    /// The same info with other dimensions, `None` for empty dimensions (`makeDimensions`).
    #[must_use]
    pub fn with_dimensions(&self, dimensions: impl Into<ISize>) -> Option<Self> {
        let info = Self::new_unchecked(
            dimensions.into(),
            self.plane_config,
            self.subsampling,
            self.yuv_color_space,
            self.origin,
            self.siting_x,
            self.siting_y,
        );
        info.is_valid().then_some(info)
    }

    /// Whether the info describes a usable layout (`isValid`).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.plane_config != PlaneConfig::Unknown
    }
}

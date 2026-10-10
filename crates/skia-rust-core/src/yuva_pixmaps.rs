// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkYUVAPixmaps.h, src/core/SkYUVAPixmaps.cpp

//! [`YUVAPixmapInfo`] (the layout of the planes of a YUVA image) and [`YUVAPixmaps`] (the planes
//! with their pixels).
//!
//! skia-rust deviation: [`YUVAPixmaps`] owns its pixels in one [`Data`] and hands out the planes as
//! [`Pixmap`] views ([`YUVAPixmaps::plane`] returns the view by value). Skia's `SkPixmap` planes
//! are stored beside the data they point into, which a safe Rust struct cannot do. The
//! `FromExternalMemory` and `FromExternalPixmaps` constructors are not ported for the same reason:
//! they borrow memory the caller keeps, so they would need a lifetime on the whole type. Use
//! [`YUVAPixmaps::from_data`] to hand over the memory.

use crate::alpha_type::AlphaType;
use crate::color_type::ColorType;
use crate::data::Data;
use crate::image_info::{ImageInfo, YUVColorSpace};
use crate::pixmap::Pixmap;
use crate::size::ISize;
use crate::yuva_info::{
    self, PlaneConfig, YUVAInfo, YUVALocations, num_channels_in_plane, num_planes,
};

/// Port of `SkYUVAPixmapInfo::DataType`: the per-channel data type of the planes.
// Port of: include/core/SkYUVAPixmaps.h#L40-L50 (chrome/m156)
#[doc(alias = "SkYUVAPixmapInfo::DataType")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[allow(non_camel_case_types)] // Skia's name for the 10-bit and 2-bit channels
pub enum DataType {
    /// 8 bit unsigned normalized.
    #[default]
    #[doc(alias = "kUnorm8")]
    Unorm8,
    /// 16 bit unsigned normalized.
    #[doc(alias = "kUnorm16")]
    Unorm16,
    /// 16 bit (half) floating point.
    #[doc(alias = "kFloat16")]
    Float16,
    /// 10 bit unorm for Y, U, and V. 2 bit unorm for alpha (if present).
    #[doc(alias = "kUnorm10_Unorm2")]
    Unorm10_Unorm2,
}

impl DataType {
    /// `kLast`.
    #[doc(alias = "kLast")]
    pub const LAST: Self = Self::Unorm10_Unorm2;
}

/// `kDataTypeCnt`: the number of [`DataType`] values.
pub const DATA_TYPE_CNT: usize = 4;

/// Port of `SkYUVAPixmapInfo::SupportedDataTypes`: which data types each plane configuration and
/// channel count supports. A bit per (data type, channel count), as in Skia's bitset.
// Port of: include/core/SkYUVAPixmaps.h#L60-L104 (chrome/m156)
#[doc(alias = "SkYUVAPixmapInfo::SupportedDataTypes")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct SupportedDataTypes {
    // bit `dt + DATA_TYPE_CNT * (channels - 1)`
    data_type_support: u32,
}

impl SupportedDataTypes {
    /// All legal combinations of [`PlaneConfig`] and [`DataType`] are supported.
    // Port of: include/core/SkYUVAPixmaps.h#L101-L134 (chrome/m156), `SupportedDataTypes::All`
    #[doc(alias = "SupportedDataTypes::All")]
    #[must_use]
    pub fn all() -> Self {
        let mut bits: u32 = 0;
        for c in 1..=4usize {
            // Skia loops `dt <= kDataTypeCnt`; the extra value is not a DataType and its default
            // colour type is kUnknown, so it sets no bit.
            for dt in 0..DATA_TYPE_CNT {
                let dt = data_type_from_index(dt);
                if default_color_type_for_data_type(dt, c) != ColorType::Unknown {
                    bits |= 1 << (dt as usize + DATA_TYPE_CNT * (c - 1));
                }
            }
        }
        Self {
            data_type_support: bits,
        }
    }

    /// Whether every plane of `config` supports `data_type`.
    // Port of: include/core/SkYUVAPixmaps.h#L136-L150 (chrome/m156), `SupportedDataTypes::supported`
    #[doc(alias = "SupportedDataTypes::supported")]
    #[must_use]
    pub fn supported(&self, config: PlaneConfig, data_type: DataType) -> bool {
        let n = num_planes(config);
        for i in 0..n {
            let c = num_channels_in_plane(config, i).unwrap_or(0);
            debug_assert!((1..=4).contains(&c));
            if self.data_type_support & (1 << (data_type as usize + (c - 1) * DATA_TYPE_CNT)) == 0 {
                return false;
            }
        }
        true
    }

    /// Enables `data_type` for planes with `num_channels` channels (`enableDataType`). Channel
    /// counts outside 1..=4 are ignored.
    // Port of: src/core/SkYUVAPixmaps.cpp#L22-L27 (chrome/m156)
    #[doc(alias = "enableDataType")]
    pub fn enable_data_type(&mut self, data_type: DataType, num_channels: usize) {
        if !(1..=4).contains(&num_channels) {
            return;
        }
        self.data_type_support |= 1 << (data_type as usize + (num_channels - 1) * DATA_TYPE_CNT);
    }
}

// The C++ `static_cast<DataType>(dt)` for a `dt` in 0..kDataTypeCnt.
fn data_type_from_index(dt: usize) -> DataType {
    match dt {
        0 => DataType::Unorm8,
        1 => DataType::Unorm16,
        2 => DataType::Float16,
        _ => DataType::Unorm10_Unorm2,
    }
}

/// Port of `SkYUVAPixmapInfo::DefaultColorTypeForDataType`.
// Port of: include/core/SkYUVAPixmaps.h#L152-L192 (chrome/m156)
#[doc(alias = "SkYUVAPixmapInfo::DefaultColorTypeForDataType")]
#[must_use]
pub fn default_color_type_for_data_type(data_type: DataType, num_channels: usize) -> ColorType {
    match num_channels {
        1 => match data_type {
            DataType::Unorm8 => ColorType::Gray8,
            DataType::Unorm16 => ColorType::A16UNorm,
            DataType::Float16 => ColorType::A16Float,
            DataType::Unorm10_Unorm2 => ColorType::Unknown,
        },
        2 => match data_type {
            DataType::Unorm8 => ColorType::R8G8UNorm,
            DataType::Unorm16 => ColorType::R16G16UNorm,
            DataType::Float16 => ColorType::R16G16Float,
            DataType::Unorm10_Unorm2 => ColorType::Unknown,
        },
        3 => match data_type {
            DataType::Unorm8 => ColorType::RGBA8888,
            DataType::Unorm16 => ColorType::R16G16B16A16UNorm,
            DataType::Float16 => ColorType::RGBAF16,
            DataType::Unorm10_Unorm2 => ColorType::RGBA1010102,
        },
        4 => match data_type {
            DataType::Unorm8 => ColorType::RGBA8888,
            DataType::Unorm16 => ColorType::R16G16B16A16UNorm,
            DataType::Float16 => ColorType::RGBAF16,
            DataType::Unorm10_Unorm2 => ColorType::RGBA1010102,
        },
        _ => ColorType::Unknown,
    }
}

/// Port of `SkYUVAPixmapInfo::NumChannelsAndDataType`: the channel count and data type of a colour
/// type, `(0, Unorm8)` for a colour type that is not a YUVA plane type.
// Port of: src/core/SkYUVAPixmaps.cpp#L29-L58 (chrome/m156)
#[doc(alias = "SkYUVAPixmapInfo::NumChannelsAndDataType")]
#[must_use]
pub fn num_channels_and_data_type(color_type: ColorType) -> (usize, DataType) {
    // We could allow BGR[A] color types, but then we'd have to decide whether B should be the 0th
    // or 2nd channel. Our docs currently say channel order is always R=0, G=1, B=2[, A=3].
    match color_type {
        ColorType::R8UNorm | ColorType::Alpha8 | ColorType::Gray8 => (1, DataType::Unorm8),
        ColorType::R16UNorm | ColorType::A16UNorm => (1, DataType::Unorm16),
        ColorType::R16Float | ColorType::A16Float => (1, DataType::Float16),
        ColorType::R8G8UNorm => (2, DataType::Unorm8),
        ColorType::R16G16UNorm => (2, DataType::Unorm16),
        ColorType::R16G16Float => (2, DataType::Float16),
        ColorType::RGB888x => (3, DataType::Unorm8),
        ColorType::RGB101010x => (3, DataType::Unorm10_Unorm2),
        ColorType::RGBF16F16F16x => (3, DataType::Float16),
        ColorType::RGBA8888 => (4, DataType::Unorm8),
        ColorType::R16G16B16A16UNorm => (4, DataType::Unorm16),
        ColorType::RGBAF16 => (4, DataType::Float16),
        ColorType::RGBAF16Norm => (4, DataType::Float16),
        ColorType::RGBA1010102 => (4, DataType::Unorm10_Unorm2),
        _ => (0, DataType::Unorm8),
    }
}

/// Port of `SkYUVAPixmapInfo`: the image info of each plane, the row bytes of each plane and the
/// data type, for a [`YUVAInfo`]. Invalid when a plane does not fit its colour type.
// Port of: include/core/SkYUVAPixmaps.h#L38-L60 (chrome/m156)
#[doc(alias = "SkYUVAPixmapInfo")]
#[derive(Clone, Debug, PartialEq)]
pub struct YUVAPixmapInfo {
    yuva_info: YUVAInfo,
    plane_infos: [ImageInfo; YUVAInfo::MAX_PLANES],
    row_bytes: [usize; YUVAInfo::MAX_PLANES],
    data_type: DataType,
}

impl Default for YUVAPixmapInfo {
    /// The invalid info (`SkYUVAPixmapInfo() = default`).
    fn default() -> Self {
        Self {
            yuva_info: YUVAInfo::default(),
            plane_infos: std::array::from_fn(|_| {
                ImageInfo::new(ISize::new(0, 0), ColorType::Unknown, AlphaType::Unknown, None)
            }),
            row_bytes: [0; YUVAInfo::MAX_PLANES],
            data_type: DataType::Unorm8,
        }
    }
}

impl YUVAPixmapInfo {
    /// `kMaxPlanes`.
    pub const MAX_PLANES: usize = YUVAInfo::MAX_PLANES;

    /// Port of `SkYUVAPixmapInfo(const SkYUVAInfo&, const SkColorType[], const size_t rowBytes[])`.
    /// `row_bytes` of `None` means tightly packed rows. Returns `None` where Skia's info is
    /// invalid.
    // Port of: src/core/SkYUVAPixmaps.cpp#L60-L107 (chrome/m156)
    #[doc(alias = "SkYUVAPixmapInfo")]
    #[must_use]
    pub fn new(
        yuva_info: &YUVAInfo,
        color_types: &[ColorType],
        row_bytes: Option<&[usize]>,
    ) -> Option<Self> {
        if color_types.len() != yuva_info.num_planes() {
            return None;
        }
        if let Some(rb) = row_bytes {
            if rb.len() != color_types.len() {
                return None;
            }
        }
        let mut color_types_array = [ColorType::Unknown; YUVAInfo::MAX_PLANES];
        color_types_array[..color_types.len()].copy_from_slice(color_types);
        let mut row_bytes_array = [0usize; YUVAInfo::MAX_PLANES];
        if let Some(rb) = row_bytes {
            row_bytes_array[..rb.len()].copy_from_slice(rb);
        }
        Self::new_from_arrays(yuva_info, &color_types_array, row_bytes.map(|_| &row_bytes_array))
    }

    /// Port of `SkYUVAPixmapInfo(const SkYUVAInfo&, DataType, const size_t rowBytes[])`.
    // Port of: src/core/SkYUVAPixmaps.cpp#L109-L122 (chrome/m156)
    #[doc(alias = "SkYUVAPixmapInfo")]
    #[must_use]
    pub fn from_data_type(
        yuva_info: &YUVAInfo,
        data_type: DataType,
        row_bytes: Option<&[usize]>,
    ) -> Option<Self> {
        let mut color_types = [ColorType::Unknown; YUVAInfo::MAX_PLANES];
        let n = yuva_info.num_planes();
        for (i, slot) in color_types.iter_mut().enumerate().take(n) {
            let num_channels = num_channels_in_plane(yuva_info.plane_config(), i).unwrap_or(0);
            *slot = default_color_type_for_data_type(data_type, num_channels);
        }
        Self::new(yuva_info, &color_types[..n], row_bytes)
    }

    // The array form of the constructor, with the row bytes already padded to kMaxPlanes.
    // Port of: src/core/SkYUVAPixmaps.cpp#L60-L107 (chrome/m156), the body after the arguments
    fn new_from_arrays(
        yuva_info: &YUVAInfo,
        color_types: &[ColorType; YUVAInfo::MAX_PLANES],
        row_bytes: Option<&[usize; YUVAInfo::MAX_PLANES]>,
    ) -> Option<Self> {
        if !yuva_info.is_valid() {
            return None;
        }
        let (n, plane_dimensions) = yuva_info::plane_dimensions_array(
            yuva_info.dimensions(),
            yuva_info.plane_config(),
            yuva_info.subsampling(),
            yuva_info.origin(),
        );
        let mut temp_row_bytes = [0usize; YUVAInfo::MAX_PLANES];
        let row_bytes = match row_bytes {
            Some(rb) => *rb,
            None => {
                for i in 0..n {
                    temp_row_bytes[i] =
                        color_types[i].bytes_per_pixel() * plane_dimensions[i].width as usize;
                }
                temp_row_bytes
            }
        };
        let mut ok = true;
        let mut data_type = DataType::Unorm8;
        let mut plane_infos: [ImageInfo; YUVAInfo::MAX_PLANES] = std::array::from_fn(|_| {
            ImageInfo::new(ISize::new(0, 0), ColorType::Unknown, AlphaType::Unknown, None)
        });
        let mut out_row_bytes = [0usize; YUVAInfo::MAX_PLANES];
        for i in 0..n {
            out_row_bytes[i] = row_bytes[i];
            // Use kUnpremul so that we never multiply alpha when copying data in.
            plane_infos[i] = ImageInfo::new(
                plane_dimensions[i],
                color_types[i],
                AlphaType::Unpremul,
                None,
            );
            let num_required_channels = num_channels_in_plane(yuva_info.plane_config(), i).unwrap_or(0);
            debug_assert!(num_required_channels > 0);
            let (num_color_type_channels, color_type_data_type) =
                num_channels_and_data_type(color_types[i]);
            ok &= i == 0 || color_type_data_type == data_type;
            ok &= num_color_type_channels >= num_required_channels;
            ok &= plane_infos[i].valid_row_bytes(out_row_bytes[i]);
            data_type = color_type_data_type;
        }
        if !ok {
            return None;
        }
        Some(Self {
            yuva_info: *yuva_info,
            plane_infos,
            row_bytes: out_row_bytes,
            data_type,
        })
    }

    /// The YUVA info.
    #[must_use]
    pub fn yuva_info(&self) -> &YUVAInfo {
        &self.yuva_info
    }

    /// The colour space of the YUV values.
    #[must_use]
    pub fn yuv_color_space(&self) -> YUVColorSpace {
        self.yuva_info.yuv_color_space()
    }

    /// The number of planes, 0 when invalid (`numPlanes`).
    #[must_use]
    pub fn num_planes(&self) -> usize {
        self.yuva_info.num_planes()
    }

    /// The per-channel data type of all planes (`dataType`).
    #[must_use]
    pub fn data_type(&self) -> DataType {
        self.data_type
    }

    /// The row bytes of plane `i`, `None` for a plane past the last one (`rowBytes`).
    #[must_use]
    pub fn row_bytes(&self, i: usize) -> Option<usize> {
        (i < YUVAInfo::MAX_PLANES).then(|| self.row_bytes[i])
    }

    /// The image info of plane `i`, `None` for a plane past the last one (`planeInfo`).
    #[must_use]
    pub fn plane_info(&self, i: usize) -> Option<&ImageInfo> {
        (i < YUVAInfo::MAX_PLANES).then(|| &self.plane_infos[i])
    }

    /// The bytes needed for all planes, and optionally each plane's size (`computeTotalBytes`).
    // Port of: src/core/SkYUVAPixmaps.cpp#L124-L133 (chrome/m156)
    #[doc(alias = "computeTotalBytes")]
    #[must_use]
    pub fn compute_total_bytes(
        &self,
        plane_sizes: Option<&mut [usize; YUVAInfo::MAX_PLANES]>,
    ) -> usize {
        if !self.is_valid() {
            if let Some(sizes) = plane_sizes {
                sizes.fill(0);
            }
            return 0;
        }
        self.yuva_info.compute_total_bytes(&self.row_bytes, plane_sizes)
    }

    /// Whether this info is valid and uses colour types the `supported` data types allow
    /// (`isSupported`).
    #[must_use]
    pub fn is_supported(&self, supported: &SupportedDataTypes) -> bool {
        if !self.is_valid() {
            return false;
        }
        supported.supported(self.yuva_info.plane_config(), self.data_type)
    }

    /// Whether the info describes a usable layout (`isValid`).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.yuva_info.is_valid()
    }
}

/// Port of `SkYUVAPixmaps`: the YUV(A) planes of an image, with their pixels in one allocation.
// Port of: include/core/SkYUVAPixmaps.h#L199-L335 (chrome/m156)
#[doc(alias = "SkYUVAPixmaps")]
#[derive(Clone, Debug)]
pub struct YUVAPixmaps {
    yuva_info: YUVAInfo,
    data_type: DataType,
    // Each plane's image info, row bytes and offset into `data`.
    plane_infos: [ImageInfo; YUVAInfo::MAX_PLANES],
    plane_row_bytes: [usize; YUVAInfo::MAX_PLANES],
    plane_offsets: [usize; YUVAInfo::MAX_PLANES],
    data: Data,
}

impl Default for YUVAPixmaps {
    /// The invalid pixmaps (`SkYUVAPixmaps() = default`).
    fn default() -> Self {
        Self {
            yuva_info: YUVAInfo::default(),
            data_type: DataType::Unorm8,
            plane_infos: YUVAPixmapInfo::default().plane_infos,
            plane_row_bytes: [0; YUVAInfo::MAX_PLANES],
            plane_offsets: [0; YUVAInfo::MAX_PLANES],
            data: Data::new_empty(),
        }
    }
}

impl YUVAPixmaps {
    /// `kMaxPlanes`.
    pub const MAX_PLANES: usize = YUVAInfo::MAX_PLANES;

    /// Port of `SkYUVAPixmaps::RecommendedRGBAColorType`.
    // Port of: src/core/SkYUVAPixmaps.cpp#L135-L146 (chrome/m156)
    #[doc(alias = "SkYUVAPixmaps::RecommendedRGBAColorType")]
    #[must_use]
    pub fn recommended_rgba_color_type(data_type: DataType) -> ColorType {
        match data_type {
            DataType::Unorm8 => ColorType::RGBA8888,
            // F16 has better GPU support than 16 bit unorm. Often "16" bit unorm values are actually
            // lower precision.
            DataType::Unorm16 => ColorType::RGBAF16,
            DataType::Float16 => ColorType::RGBAF16,
            DataType::Unorm10_Unorm2 => ColorType::RGBA1010102,
        }
    }

    /// Port of `SkYUVAPixmaps::Allocate`: pixmaps with newly allocated, uninitialized pixels.
    /// `None` for an invalid info.
    // Port of: src/core/SkYUVAPixmaps.cpp#L148-L154 (chrome/m156)
    #[doc(alias = "SkYUVAPixmaps::Allocate")]
    #[must_use]
    pub fn allocate(info: &YUVAPixmapInfo) -> Option<Self> {
        if !info.is_valid() {
            return None;
        }
        let total = info.compute_total_bytes(None);
        Some(Self::from_parts(info, Data::new_zero_initialized(total)))
    }

    /// Port of `SkYUVAPixmaps::FromData`: pixmaps over `data`, which must hold all the planes.
    /// `None` for an invalid info or too little data.
    // Port of: src/core/SkYUVAPixmaps.cpp#L156-L164 (chrome/m156)
    #[doc(alias = "SkYUVAPixmaps::FromData")]
    #[must_use]
    pub fn from_data(info: &YUVAPixmapInfo, data: impl Into<Data>) -> Option<Self> {
        if !info.is_valid() {
            return None;
        }
        let data = data.into();
        if info.compute_total_bytes(None) > data.size() {
            return None;
        }
        Some(Self::from_parts(info, data))
    }

    /// Port of `SkYUVAPixmaps::MakeCopy`: a copy of `src` with its own pixels.
    // Port of: src/core/SkYUVAPixmaps.cpp#L166-L184 (chrome/m156)
    #[doc(alias = "SkYUVAPixmaps::MakeCopy")]
    #[must_use]
    pub fn make_copy(src: &YUVAPixmaps) -> Option<Self> {
        if !src.is_valid() {
            return None;
        }
        let mut result = Self::allocate(&src.pixmaps_info())?;
        let n = result.num_planes();
        for i in 0..n {
            // We copy rows rather than reading pixels to ensure that we don't do any alpha type
            // conversion (SkRectMemcpy).
            let s = src.plane(i);
            let s_row_bytes = s.row_bytes();
            let min_row_bytes = s.info().min_row_bytes();
            let height = s.info().height() as usize;
            let s_bytes = s.addr().unwrap_or(&[]);
            let d_row_bytes = result.plane_row_bytes[i];
            let d_offset = result.plane_offsets[i];
            let d_bytes = &mut result.data.writable_data()?[d_offset..];
            for row in 0..height {
                let src_row = &s_bytes[row * s_row_bytes..row * s_row_bytes + min_row_bytes];
                d_bytes[row * d_row_bytes..row * d_row_bytes + min_row_bytes]
                    .copy_from_slice(src_row);
            }
        }
        Some(result)
    }

    // The planes of a fresh allocation: consecutive, each `rowBytes * height` bytes long, as
    // `initPixmapsFromSingleAllocation` lays them out.
    // Port of: src/core/SkYUVAPixmaps.cpp#L186-L208 (chrome/m156), the private constructor
    fn from_parts(info: &YUVAPixmapInfo, data: Data) -> Self {
        let n = info.num_planes();
        let mut plane_row_bytes = [0usize; YUVAInfo::MAX_PLANES];
        let mut plane_offsets = [0usize; YUVAInfo::MAX_PLANES];
        let mut addr = 0usize;
        for i in 0..n {
            debug_assert!(info.plane_infos[i].valid_row_bytes(info.row_bytes[i]));
            plane_row_bytes[i] = info.row_bytes[i];
            plane_offsets[i] = addr;
            let plane_size = info.row_bytes[i] * info.plane_infos[i].height() as usize;
            debug_assert!(plane_size != 0);
            addr += plane_size;
        }
        Self {
            yuva_info: info.yuva_info,
            data_type: info.data_type,
            plane_infos: info.plane_infos.clone(),
            plane_row_bytes,
            plane_offsets,
            data,
        }
    }

    /// Whether these pixmaps have planes that match their info (`isValid`).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.yuva_info.dimensions().is_empty()
    }

    /// The YUVA info.
    #[must_use]
    pub fn yuva_info(&self) -> &YUVAInfo {
        &self.yuva_info
    }

    /// The per-channel data type of all planes.
    #[must_use]
    pub fn data_type(&self) -> DataType {
        self.data_type
    }

    /// The info of these pixmaps (`pixmapsInfo`).
    #[must_use]
    pub fn pixmaps_info(&self) -> YUVAPixmapInfo {
        if !self.is_valid() {
            return YUVAPixmapInfo::default();
        }
        let n = self.num_planes();
        let mut color_types = [ColorType::Unknown; YUVAInfo::MAX_PLANES];
        let mut row_bytes = [0usize; YUVAInfo::MAX_PLANES];
        for i in 0..n {
            color_types[i] = self.plane_infos[i].color_type();
            row_bytes[i] = self.plane_row_bytes[i];
        }
        YUVAPixmapInfo::new_from_arrays(&self.yuva_info, &color_types, Some(&row_bytes))
            .unwrap_or_default()
    }

    /// The number of planes, 0 when invalid (`numPlanes`).
    #[must_use]
    pub fn num_planes(&self) -> usize {
        if self.is_valid() {
            self.yuva_info.num_planes()
        } else {
            0
        }
    }

    /// Plane `i` as a [`Pixmap`] view of the pixels (`plane`).
    #[must_use]
    pub fn plane(&self, i: usize) -> Pixmap<'_> {
        let info = &self.plane_infos[i];
        let size = self.plane_row_bytes[i] * info.height() as usize;
        let start = self.plane_offsets[i];
        let bytes = self.data.as_bytes().get(start..start + size).unwrap_or(&[]);
        Pixmap::new_readonly(info, bytes, self.plane_row_bytes[i]).unwrap_or_default()
    }

    /// Plane `i` as a writable [`Pixmap`], `None` when the pixels are shared with another [`Data`]
    /// (Skia writes through its pointers; a shared buffer cannot be written safely).
    #[must_use]
    pub fn plane_mut(&mut self, i: usize) -> Option<Pixmap<'_>> {
        let size = self.plane_row_bytes[i] * self.plane_infos[i].height() as usize;
        let start = self.plane_offsets[i];
        let row_bytes = self.plane_row_bytes[i];
        let info = &self.plane_infos[i];
        let bytes = self.data.writable_data()?.get_mut(start..start + size)?;
        Pixmap::new(info, bytes, row_bytes)
    }

    /// Row `row` of plane `i` as its `row_bytes` writable bytes, padding included. `None` for a
    /// plane or row past the plane, or when the pixels are shared with another [`Data`]. Decoders
    /// write rows this way, because libjpeg's rows are wider than the plane when the plane is
    /// padded to whole blocks.
    // skia-rust: no Skia counterpart; SkYUVAPixmaps hands out raw row pointers (`writable_addr`).
    #[must_use]
    pub fn plane_row_mut(&mut self, i: usize, row: usize) -> Option<&mut [u8]> {
        if i >= self.num_planes() || row >= self.plane_infos[i].height() as usize {
            return None;
        }
        let row_bytes = self.plane_row_bytes[i];
        let start = self.plane_offsets[i] + row * row_bytes;
        self.data.writable_data()?.get_mut(start..start + row_bytes)
    }

    /// The locations of Y, U, V and A (`toYUVALocations`).
    #[must_use]
    pub fn to_yuva_locations(&self) -> Option<YUVALocations> {
        let channel_flags = std::array::from_fn(|i| {
            crate::image_info_priv::color_type_channel_flags(self.plane_infos[i].color_type())
        });
        self.yuva_info.to_yuva_locations(&channel_flags)
    }

    /// Whether these pixmaps own the memory of the planes (`ownsStorage`). Always true here, as
    /// the pixels are in a [`Data`] that the pixmaps own or share.
    #[must_use]
    pub fn owns_storage(&self) -> bool {
        self.data.size() > 0
    }
}

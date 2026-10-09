// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/dawn/DawnGraphiteTypes.h (DawnTextureInfo),
//                   src/gpu/graphite/dawn/DawnTextureInfo.cpp

//! `DawnTextureInfo` on wgpu: the wgpu half of a [`TextureInfo`].
//!
//! Differences from Dawn, all forced by wgpu's API:
//! - `wgpu::TextureFormat` has no `Undefined`; the C++ "undefined" format is `None`.
//! - There is no `YCbCrVkDescriptor`: wgpu has no YCbCr conversion samplers (`fYcbcrVkDescriptor`
//!   and everything keyed on it is dropped).
//! - `toBackendString()` prints the format by name; Dawn prints its numeric enum value.
//! - The base `Data` fields (sample count and mipmapping) live in [`TextureInfo`] itself
//!   (`texture_info.rs`), so the stored data are [`WgpuTextureInfoData`] and the public
//!   [`WgpuTextureInfo`] (which has them, as `DawnTextureInfo` does) is converted by
//!   [`texture_infos`].

use std::any::Any;

use crate::gpu::gpu_types::{BackendApi, Mipmapped, Protected};
use crate::graphite::graphite_types::{SampleCount, to_sample_count};
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::texture_info::{TextureInfo, TextureInfoData};
use crate::graphite::wgpu::graphite_utils::wgpu_format_to_texture_format;

/// The backend data of a wgpu [`TextureInfo`] (`DawnTextureInfo` without the base `Data` fields).
// Port of: include/gpu/graphite/dawn/DawnGraphiteTypes.h#L24-L36, #L78-L90 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WgpuTextureInfoData {
    /// `fFormat`.
    pub format: Option<wgpu::TextureFormat>,
    /// `fViewFormat`: for multiplanar formats the plane view's format.
    pub view_format: Option<wgpu::TextureFormat>,
    /// `fUsage`.
    pub usage: wgpu::TextureUsages,
    /// `fAspect`.
    pub aspect: wgpu::TextureAspect,
    /// `fSlice`.
    pub slice: u32,
}

impl WgpuTextureInfoData {
    /// `getViewFormat()`: `fViewFormat` unless it is undefined, then `fFormat`.
    // Port of: include/gpu/graphite/dawn/DawnGraphiteTypes.h#L38-L40 (chrome/m156)
    #[doc(alias = "getViewFormat")]
    #[must_use]
    pub fn get_view_format(&self) -> Option<wgpu::TextureFormat> {
        self.view_format.or(self.format)
    }
}

impl TextureInfoData for WgpuTextureInfoData {
    // Port of: include/gpu/graphite/dawn/DawnGraphiteTypes.h#L83 (chrome/m156)
    fn backend(&self) -> BackendApi {
        BackendApi::Dawn
    }

    fn is_protected(&self) -> Protected {
        Protected::No
    }

    // Port of: src/gpu/graphite/dawn/DawnTextureInfo.cpp#L25-L31 (chrome/m156)
    fn view_format(&self) -> TextureFormat {
        self.get_view_format()
            .map_or(TextureFormat::Unsupported, wgpu_format_to_texture_format)
    }

    // Port of: src/gpu/graphite/dawn/DawnTextureInfo.cpp#L33-L38 (chrome/m156)
    fn to_backend_string(&self) -> String {
        format!(
            "wgpuFormat={},usage=0x{:08X},aspect=0x{:08X},slice={}",
            self.format
                .map_or_else(|| "Undefined".to_owned(), |f| format!("{f:?}")),
            self.usage.bits(),
            aspect_value(self.aspect),
            self.slice
        )
    }

    // Port of: src/gpu/graphite/dawn/DawnTextureInfo.cpp#L40-L52 (chrome/m156)
    fn is_compatible(&self, that: &TextureInfo, require_exact: bool) -> bool {
        let Some(dt) = that.get::<Self>() else {
            return false;
        };

        // The usages may match or the usage passed in may be a superset of the usage stored
        // within. The aspect should either match the plane aspect or should be All.
        self.get_view_format() == dt.get_view_format()
            && (self.usage & dt.usage) == self.usage
            && (self.aspect == dt.aspect
                || (!require_exact && self.aspect == wgpu::TextureAspect::All))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Dawn's `wgpu::TextureAspect` enumerator values.
fn aspect_value(aspect: wgpu::TextureAspect) -> u32 {
    match aspect {
        wgpu::TextureAspect::All => 0x0000_0001,
        wgpu::TextureAspect::StencilOnly => 0x0000_0002,
        wgpu::TextureAspect::DepthOnly => 0x0000_0003,
        wgpu::TextureAspect::Plane0 => 0x0005_0000,
        wgpu::TextureAspect::Plane1 => 0x0005_0001,
        wgpu::TextureAspect::Plane2 => 0x0005_0002,
    }
}

/// The properties of a wgpu texture, sans dimensions.
// Port of: include/gpu/graphite/dawn/DawnGraphiteTypes.h#L22-L117 (chrome/m156)
#[doc(alias = "DawnTextureInfo")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WgpuTextureInfo {
    /// `fSampleCount`.
    pub sample_count: SampleCount,
    /// `fMipmapped`.
    pub mipmapped: Mipmapped,
    /// `fFormat`.
    pub format: Option<wgpu::TextureFormat>,
    /// `fViewFormat`.
    pub view_format: Option<wgpu::TextureFormat>,
    /// `fUsage`.
    pub usage: wgpu::TextureUsages,
    /// `fAspect`.
    pub aspect: wgpu::TextureAspect,
    /// `fSlice`.
    pub slice: u32,
}

impl Default for WgpuTextureInfo {
    fn default() -> Self {
        Self {
            sample_count: SampleCount::One,
            mipmapped: Mipmapped::No,
            format: None,
            view_format: None,
            usage: wgpu::TextureUsages::empty(),
            aspect: wgpu::TextureAspect::All,
            slice: 0,
        }
    }
}

impl WgpuTextureInfo {
    /// `DawnTextureInfo(sampleCount, mipmapped, format, usage, aspect)`.
    // Port of: include/gpu/graphite/dawn/DawnGraphiteTypes.h#L48-L61 (chrome/m156)
    #[must_use]
    pub fn new(
        sample_count: SampleCount,
        mipmapped: Mipmapped,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        aspect: wgpu::TextureAspect,
    ) -> Self {
        Self::with_view_format(
            sample_count,
            mipmapped,
            Some(format),
            Some(format),
            usage,
            aspect,
            /* slice= */ 0,
        )
    }

    /// `DawnTextureInfo(sampleCount, mipmapped, format, viewFormat, usage, aspect, slice)`.
    // Port of: include/gpu/graphite/dawn/DawnGraphiteTypes.h#L63-L77 (chrome/m156)
    #[must_use]
    pub fn with_view_format(
        sample_count: SampleCount,
        mipmapped: Mipmapped,
        format: Option<wgpu::TextureFormat>,
        view_format: Option<wgpu::TextureFormat>,
        usage: wgpu::TextureUsages,
        aspect: wgpu::TextureAspect,
        slice: u32,
    ) -> Self {
        Self {
            sample_count,
            mipmapped,
            format,
            view_format,
            usage,
            aspect,
            slice,
        }
    }

    /// `DawnTextureInfo(WGPUTexture)`: the info of an existing texture.
    // Port of: src/gpu/graphite/dawn/DawnTextureInfo.cpp#L14-L23 (chrome/m156)
    #[must_use]
    pub fn from_texture(texture: &wgpu::Texture) -> Self {
        Self::new(
            to_sample_count(texture.sample_count()),
            if texture.mip_level_count() > 1 {
                Mipmapped::Yes
            } else {
                Mipmapped::No
            },
            texture.format(),
            texture.usage(),
            wgpu::TextureAspect::All,
        )
    }

    /// `getViewFormat()`.
    #[doc(alias = "getViewFormat")]
    #[must_use]
    pub fn get_view_format(&self) -> Option<wgpu::TextureFormat> {
        self.view_format.or(self.format)
    }

    fn data(&self) -> WgpuTextureInfoData {
        WgpuTextureInfoData {
            format: self.format,
            view_format: self.view_format,
            usage: self.usage,
            aspect: self.aspect,
            slice: self.slice,
        }
    }
}

/// `skgpu::graphite::TextureInfos` for the wgpu back end.
pub mod texture_infos {
    use super::{TextureInfo, WgpuTextureInfo, WgpuTextureInfoData};

    /// `TextureInfos::MakeDawn`.
    // Port of: src/gpu/graphite/dawn/DawnTextureInfo.cpp#L56-L58 (chrome/m156)
    #[doc(alias = "MakeDawn")]
    #[must_use]
    pub fn make_wgpu(info: &WgpuTextureInfo) -> TextureInfo {
        TextureInfo::make(info.data(), info.sample_count, info.mipmapped)
    }

    /// `TextureInfos::GetDawnTextureInfo`: the wgpu info of `info`, if it is a wgpu one.
    // Port of: src/gpu/graphite/dawn/DawnTextureInfo.cpp#L60-L62 (chrome/m156)
    #[doc(alias = "GetDawnTextureInfo")]
    #[must_use]
    pub fn get_wgpu_texture_info(info: &TextureInfo) -> Option<WgpuTextureInfo> {
        let data = info.get::<WgpuTextureInfoData>()?;
        Some(WgpuTextureInfo {
            sample_count: info.sample_count(),
            mipmapped: info.mipmapped(),
            format: data.format,
            view_format: data.view_format,
            usage: data.usage,
            aspect: data.aspect,
            slice: data.slice,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(usage: wgpu::TextureUsages) -> TextureInfo {
        texture_infos::make_wgpu(&WgpuTextureInfo::new(
            SampleCount::One,
            Mipmapped::No,
            wgpu::TextureFormat::Rgba8Unorm,
            usage,
            wgpu::TextureAspect::All,
        ))
    }

    #[test]
    fn round_trips_and_prints() {
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let ti = info(usage);
        assert_eq!(ti.backend(), BackendApi::Dawn);
        assert!(ti.is_valid());
        let back = texture_infos::get_wgpu_texture_info(&ti).unwrap();
        assert_eq!(back.format, Some(wgpu::TextureFormat::Rgba8Unorm));
        assert_eq!(back.usage, usage);
        assert_eq!(
            ti.to_string(),
            "Dawn(viewFormat=RGBA8,wgpuFormat=Rgba8Unorm,usage=0x00000006,aspect=0x00000001,\
             slice=0,bpp=4,sampleCount=1,mipmapped=0,protected=0)"
        );
        assert!(texture_infos::get_wgpu_texture_info(&TextureInfo::new()).is_none());
    }

    #[test]
    fn compatibility_allows_usage_supersets() {
        let small = info(wgpu::TextureUsages::TEXTURE_BINDING);
        let big = info(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC);
        // The usage passed in may be a superset of the usage stored within.
        assert!(small.can_be_fulfilled_by(&big));
        assert!(!big.can_be_fulfilled_by(&small));
        // `operator==` is `isCompatible(that, requireExact = true)`, which only requires the
        // exact aspect: it is not symmetric for usages.
        assert_eq!(small, big);
        assert_ne!(big, small);
    }
}

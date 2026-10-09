// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/TextureInfo.h, src/gpu/graphite/TextureInfo.cpp,
//                   src/gpu/graphite/TextureInfoPriv.h

//! `TextureInfo`: the backend-agnostic description of a texture, sans dimensions.
//!
//! Skia stores the backend's `TextureInfo::Data` subclass inline (`SkAnySubclass`). The port keeps
//! the backend half behind the [`TextureInfoData`] trait, shared immutably in an `Arc`; the base
//! `Data` fields (sample count and mipmapping) live in [`TextureInfo`] itself so that
//! `ReplaceSampleCount` does not need to clone the backend data.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::gpu::gpu_types::{BackendApi, Mipmapped, Protected, backend_api_to_str};
use crate::graphite::graphite_types::SampleCount;
use crate::graphite::texture_format::{
    TextureFormat, texture_format_bytes_per_block, texture_format_channel_mask, texture_format_name,
};

/// The backend half of a [`TextureInfo`] (`TextureInfo::Data`'s virtual interface plus the
/// static members every backend subclass exposes).
// Port of: include/gpu/graphite/TextureInfo.h#L38-L72 (chrome/m156)
pub trait TextureInfoData: Send + Sync + fmt::Debug + 'static {
    /// `kBackend`.
    fn backend(&self) -> BackendApi;
    /// `isProtected()`.
    fn is_protected(&self) -> Protected;
    /// `viewFormat()`.
    fn view_format(&self) -> TextureFormat;
    /// `toBackendString()`.
    fn to_backend_string(&self) -> String;
    /// `isCompatible(that, requireExact)`: `that` has data of the same backend, and the base
    /// properties have already been checked.
    fn is_compatible(&self, that: &TextureInfo, require_exact: bool) -> bool;
    /// For [`TextureInfo::get`].
    fn as_any(&self) -> &dyn Any;
}

/// A backend-agnostic description of a texture's properties, without its dimensions.
#[doc(alias = "skgpu::graphite::TextureInfo")]
#[derive(Clone, Default)]
pub struct TextureInfo {
    backend: BackendApi,
    data: Option<Arc<dyn TextureInfoData>>,
    // NOTE: these are `Data::fSampleCount` and `Data::fMipmapped` in Skia.
    sample_count: SampleCount,
    mipmapped: Mipmapped,
    // Derived properties from the backend data, cached to avoid a virtual function call.
    view_format: TextureFormat,
    protected: Protected,
}

impl fmt::Debug for TextureInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl PartialEq for TextureInfo {
    // Port of: include/gpu/graphite/TextureInfo.h#L81-L84 (chrome/m156)
    fn eq(&self, that: &Self) -> bool {
        self.is_compatible(that, /* require_exact= */ true)
    }
}

impl TextureInfo {
    /// `TextureInfo()`: an invalid info.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `TextureInfoPriv::Make(data)`: wraps backend data whose base `Data` fields are
    /// `sample_count` and `mipmapped`.
    // Port of: include/gpu/graphite/TextureInfo.h#L112-L119 (chrome/m156),
    //          src/gpu/graphite/TextureInfoPriv.h#L30-L33
    #[must_use]
    pub fn make(
        data: impl TextureInfoData,
        sample_count: SampleCount,
        mipmapped: Mipmapped,
    ) -> Self {
        Self {
            backend: data.backend(),
            view_format: data.view_format(),
            protected: data.is_protected(),
            data: Some(Arc::new(data)),
            sample_count,
            mipmapped,
        }
    }

    /// `isValid()`.
    // Port of: include/gpu/graphite/TextureInfo.h#L86 (chrome/m156)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.data.is_some()
    }

    /// `backend()`.
    // Port of: include/gpu/graphite/TextureInfo.h#L87-L90 (chrome/m156)
    #[must_use]
    pub fn backend(&self) -> BackendApi {
        debug_assert!(self.data.is_some() || self.backend == BackendApi::Unsupported);
        self.backend
    }

    /// `isProtected()`.
    #[must_use]
    pub fn is_protected(&self) -> Protected {
        self.protected
    }

    /// `sampleCount()`.
    // Port of: include/gpu/graphite/TextureInfo.h#L93-L95 (chrome/m156)
    #[must_use]
    pub fn sample_count(&self) -> SampleCount {
        if self.data.is_some() {
            self.sample_count
        } else {
            SampleCount::One
        }
    }

    /// `mipmapped()`.
    // Port of: include/gpu/graphite/TextureInfo.h#L96-L98 (chrome/m156)
    #[must_use]
    pub fn mipmapped(&self) -> Mipmapped {
        if self.data.is_some() {
            self.mipmapped
        } else {
            Mipmapped::No
        }
    }

    /// `canBeFulfilledBy(that)`: true if `that` describes a texture compatible with this info,
    /// which can validly fulfill a promise image created with it.
    // Port of: include/gpu/graphite/TextureInfo.h#L102-L104 (chrome/m156)
    #[doc(alias = "canBeFulfilledBy")]
    #[must_use]
    pub fn can_be_fulfilled_by(&self, that: &TextureInfo) -> bool {
        self.is_compatible(that, /* require_exact= */ false)
    }

    // Port of: src/gpu/graphite/TextureInfo.cpp#L32-L44 (chrome/m156)
    fn is_compatible(&self, that: &TextureInfo, require_exact: bool) -> bool {
        if self.backend != that.backend {
            false
        } else if self.backend == BackendApi::Unsupported {
            debug_assert!(self.data.is_none() && that.data.is_none());
            true
        } else {
            match &self.data {
                Some(data) => {
                    debug_assert!(that.data.is_some());
                    self.sample_count == that.sample_count
                        && self.mipmapped == that.mipmapped
                        && data.is_compatible(that, require_exact)
                }
                None => false,
            }
        }
    }

    /// The backend data, if valid.
    #[must_use]
    pub fn data(&self) -> Option<&dyn TextureInfoData> {
        self.data.as_deref()
    }

    /// `TextureInfoPriv::Get<T>(info)`: the backend data as `T`, if the info is valid and holds
    /// that backend's data.
    // Port of: src/gpu/graphite/TextureInfoPriv.h#L35-L39 (chrome/m156)
    #[must_use]
    pub fn get<T: TextureInfoData>(&self) -> Option<&T> {
        self.data.as_deref()?.as_any().downcast_ref::<T>()
    }
}

impl fmt::Display for TextureInfo {
    /// `toString()`.
    // Port of: src/gpu/graphite/TextureInfo.cpp#L46-L63 (chrome/m156)
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(data) = &self.data else {
            return f.write_str("{}");
        };

        // Strip the leading "k" from the enum name when creating the TextureInfo string.
        let backend_name = &backend_api_to_str(self.backend)[1..];

        write!(
            f,
            "{}(viewFormat={},{},bpp={},sampleCount={},mipmapped={},protected={})",
            backend_name,
            texture_format_name(self.view_format),
            data.to_backend_string(),
            texture_format_bytes_per_block(self.view_format),
            self.sample_count as u32,
            self.mipmapped as i32,
            self.protected as i32
        )
    }
}

/// `TextureInfoPriv`: Graphite-internal accessors of [`TextureInfo`].
// Port of: src/gpu/graphite/TextureInfoPriv.h#L18-L60 (chrome/m156)
pub mod texture_info_priv {
    use super::{SampleCount, TextureFormat, TextureInfo, texture_format_channel_mask};

    /// `TextureInfoPriv::ViewFormat`.
    #[doc(alias = "ViewFormat")]
    #[must_use]
    pub fn view_format(info: &TextureInfo) -> TextureFormat {
        info.view_format
    }

    /// `TextureInfoPriv::ChannelMask`.
    #[doc(alias = "ChannelMask")]
    #[must_use]
    pub fn channel_mask(info: &TextureInfo) -> u32 {
        texture_format_channel_mask(view_format(info))
    }

    /// `TextureInfoPriv::ReplaceSampleCount`.
    // Port of: src/gpu/graphite/TextureInfoPriv.h#L52-L58 (chrome/m156)
    #[doc(alias = "ReplaceSampleCount")]
    #[must_use]
    pub fn replace_sample_count(info: &TextureInfo, sample_count: SampleCount) -> TextureInfo {
        let mut copy = info.clone();
        if copy.is_valid() {
            copy.sample_count = sample_count;
        }
        copy
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A stand-in backend for tests of the backend-neutral code.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct MockTextureInfo {
        pub(crate) format: TextureFormat,
        pub(crate) usage: u32,
    }

    impl TextureInfoData for MockTextureInfo {
        fn backend(&self) -> BackendApi {
            BackendApi::Mock
        }

        fn is_protected(&self) -> Protected {
            Protected::No
        }

        fn view_format(&self) -> TextureFormat {
            self.format
        }

        fn to_backend_string(&self) -> String {
            format!("usage={}", self.usage)
        }

        fn is_compatible(&self, that: &TextureInfo, require_exact: bool) -> bool {
            let Some(that) = that.get::<Self>() else {
                return false;
            };
            self.format == that.format && (!require_exact || self.usage == that.usage)
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    pub(crate) fn mock_info(format: TextureFormat) -> TextureInfo {
        TextureInfo::make(
            MockTextureInfo { format, usage: 1 },
            SampleCount::One,
            Mipmapped::No,
        )
    }

    #[test]
    fn invalid_and_valid_infos() {
        let invalid = TextureInfo::new();
        assert!(!invalid.is_valid());
        assert_eq!(invalid.to_string(), "{}");
        assert_eq!(invalid, TextureInfo::new());

        let info = mock_info(TextureFormat::RGBA8);
        assert!(info.is_valid());
        assert_ne!(info, invalid);
        assert_eq!(
            info.to_string(),
            "Mock(viewFormat=RGBA8,usage=1,bpp=4,sampleCount=1,mipmapped=0,protected=0)"
        );
        let msaa = texture_info_priv::replace_sample_count(&info, SampleCount::Four);
        assert_eq!(msaa.sample_count(), SampleCount::Four);
        assert_ne!(info, msaa);

        let other_usage = TextureInfo::make(
            MockTextureInfo {
                format: TextureFormat::RGBA8,
                usage: 2,
            },
            SampleCount::One,
            Mipmapped::No,
        );
        assert_ne!(info, other_usage);
        assert!(info.can_be_fulfilled_by(&other_usage));
    }
}

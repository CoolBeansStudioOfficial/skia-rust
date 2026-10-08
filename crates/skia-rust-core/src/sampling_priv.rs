// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkSamplingPriv.h

//! `SkSamplingPriv`: private helpers of [`SamplingOptions`].

use crate::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};

/// Given a src rect in texels to be filtered, this number of surrounding texels are needed by
/// the kernel in x and y (`kBicubicFilterTexelPad`).
// Port of: src/core/SkSamplingPriv.h#L21 (chrome/m156)
#[doc(alias = "kBicubicFilterTexelPad")]
pub const BICUBIC_FILTER_TEXEL_PAD: i32 = 2;

/// Private copy of `SkFilterQuality`, just for legacy deserialization (`SkLegacyFQ`).
// Port of: src/core/SkSamplingPriv.h#L23-L31 (chrome/m156)
#[doc(alias = "SkLegacyFQ")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LegacyFQ {
    /// Nearest-neighbor; fastest but lowest quality (`kNone_SkLegacyFQ`).
    None = 0,
    /// Bilerp (`kLow_SkLegacyFQ`).
    Low = 1,
    /// Bilerp + mipmaps; good for down-scaling (`kMedium_SkLegacyFQ`).
    Medium = 2,
    /// Bicubic resampling; slowest but good quality (`kHigh_SkLegacyFQ`).
    High = 3,
}

/// Matches the values in `SkSamplingOptions::MediumBehavior` (`SkMediumAs`).
// Port of: src/core/SkSamplingPriv.h#L34-L37 (chrome/m156)
#[doc(alias = "SkMediumAs")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum MediumAs {
    /// `kNearest_SkMediumAs`.
    #[default]
    Nearest,
    /// `kLinear_SkMediumAs`.
    Linear,
}

/// The size of the serialized options (`SkSamplingPriv::FlatSize`).
// Port of: src/core/SkSamplingPriv.h#L40-L46 (chrome/m156)
#[doc(alias = "FlatSize")]
#[must_use]
pub fn flat_size(options: &SamplingOptions) -> usize {
    let mut size = core::mem::size_of::<u32>(); // maxAniso
    if !options.is_aniso() {
        size += 3 * core::mem::size_of::<u32>(); // bool32 + [2 floats | 2 ints]
    }
    size
}

/// Returns true if the sampling can be ignored when the CTM is identity
/// (`SkSamplingPriv::NoChangeWithIdentityMatrix`).
// Port of: src/core/SkSamplingPriv.h#L48-L53 (chrome/m156)
#[doc(alias = "NoChangeWithIdentityMatrix")]
#[must_use]
#[allow(clippy::float_cmp)] // mirrors `cubic.B == 0`
pub fn no_change_with_identity_matrix(sampling: &SamplingOptions) -> bool {
    // If B == 0, the cubic resampler should have no effect for identity matrices
    // https://entropymine.com/imageworsener/bicubic/
    // We assume aniso has no effect with an identity transform.
    !sampling.use_cubic || sampling.cubic.b == 0.0
}

/// Makes a fallback [`SamplingOptions`] for cases where anisotropic filtering is not allowed.
/// Anisotropic filtering can access mip levels if present, but we don't add mipmaps to
/// non-mipmapped images when the user requests anisotropic. So we shouldn't fall back to a
/// sampling that would trigger mip map creation (`SkSamplingPriv::AnisoFallback`).
// Port of: src/core/SkSamplingPriv.h#L55-L60 (chrome/m156)
#[doc(alias = "AnisoFallback")]
#[must_use]
pub fn aniso_fallback(image_is_mipped: bool) -> SamplingOptions {
    let mm = if image_is_mipped {
        MipmapMode::Linear
    } else {
        MipmapMode::None
    };
    SamplingOptions::new(FilterMode::Linear, mm)
}

/// The options for a legacy filter quality (`SkSamplingPriv::FromFQ`).
// Port of: src/core/SkSamplingPriv.h#L62-L77 (chrome/m156)
#[doc(alias = "FromFQ")]
#[must_use]
pub fn from_fq(fq: LegacyFQ, behavior: MediumAs) -> SamplingOptions {
    match fq {
        LegacyFQ::High => SamplingOptions::from(CubicResampler {
            b: 1.0 / 3.0,
            c: 1.0 / 3.0,
        }),
        LegacyFQ::Medium => SamplingOptions::new(
            FilterMode::Linear,
            if behavior == MediumAs::Nearest {
                MipmapMode::Nearest
            } else {
                MipmapMode::Linear
            },
        ),
        LegacyFQ::Low => SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        LegacyFQ::None => SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
    }
}

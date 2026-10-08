// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkSamplingOptions.h

//! `SkSamplingOptions`: how an image is sampled (filter mode, mipmap mode, cubic resampler).

/// How to interpolate between the texels of an image (`SkFilterMode`).
// Port of: include/core/SkSamplingOptions.h#L16-L21 (chrome/m156)
#[doc(alias = "SkFilterMode")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Default)]
#[repr(i32)]
pub enum FilterMode {
    /// Single sample point (nearest neighbor) (`kNearest`).
    #[default]
    Nearest,
    /// Interpolate between 2x2 sample points (bilinear interpolation) (`kLinear`).
    Linear,
}

/// Which mipmap levels to sample from (`SkMipmapMode`).
// Port of: include/core/SkSamplingOptions.h#L24-L30 (chrome/m156)
#[doc(alias = "SkMipmapMode")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Default)]
#[repr(i32)]
pub enum MipmapMode {
    /// Ignore mipmap levels, sample from the "base" (`kNone`).
    #[default]
    None,
    /// Sample from the nearest level (`kNearest`).
    Nearest,
    /// Interpolate between the two nearest levels (`kLinear`).
    Linear,
}

/// Specify `b` and `c` (each between 0...1) to create a shader that applies the corresponding
/// cubic reconstruction filter to the image (`SkCubicResampler`).
///
/// Example values:
///     b = 1/3, c = 1/3        "Mitchell" filter
///     b = 0,   c = 1/2        "Catmull-Rom" filter
///
/// See "Reconstruction Filters in Computer Graphics", Don P. Mitchell and Arun N. Netravali,
/// 1988.
// Port of: include/core/SkSamplingOptions.h#L42-L51 (chrome/m156)
#[doc(alias = "SkCubicResampler")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct CubicResampler {
    /// `B`.
    pub b: f32,
    /// `C`.
    pub c: f32,
}

impl CubicResampler {
    /// Historic default for `kHigh_SkFilterQuality` (`Mitchell`).
    #[doc(alias = "Mitchell")]
    #[must_use]
    pub fn mitchell() -> CubicResampler {
        CubicResampler {
            b: 1.0 / 3.0,
            c: 1.0 / 3.0,
        }
    }

    /// The Catmull-Rom filter (`CatmullRom`).
    #[doc(alias = "CatmullRom")]
    #[must_use]
    pub fn catmull_rom() -> CubicResampler {
        CubicResampler {
            b: 0.0,
            c: 1.0 / 2.0,
        }
    }
}

/// Sampling options of an image: nearest or linear filtering, optionally across mipmap levels, or
/// a cubic resampler, or anisotropic filtering (`SkSamplingOptions`).
///
/// Equality is Skia's `operator==`: field-wise with float `==`.
// Port of: include/core/SkSamplingOptions.h#L53-L100 (chrome/m156)
#[doc(alias = "SkSamplingOptions")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct SamplingOptions {
    /// `maxAniso`.
    pub max_aniso: i32,
    /// `useCubic`.
    pub use_cubic: bool,
    /// `cubic` (ignored unless `use_cubic`).
    pub cubic: CubicResampler,
    /// `filter`.
    pub filter: FilterMode,
    /// `mipmap`.
    pub mipmap: MipmapMode,
}

impl Default for SamplingOptions {
    // Port of: include/core/SkSamplingOptions.h#L54-L58 (chrome/m156)
    fn default() -> SamplingOptions {
        SamplingOptions {
            max_aniso: 0,
            use_cubic: false,
            // ignored
            cubic: CubicResampler { b: 0.0, c: 0.0 },
            filter: FilterMode::Nearest,
            mipmap: MipmapMode::None,
        }
    }
}

impl SamplingOptions {
    /// Filter `filter_mode` across the mipmap levels chosen by `mm` (`SkSamplingOptions(fm, mm)`).
    #[must_use]
    pub fn new(filter_mode: FilterMode, mm: MipmapMode) -> SamplingOptions {
        SamplingOptions {
            filter: filter_mode,
            mipmap: mm,
            ..SamplingOptions::default()
        }
    }

    /// Anisotropic filtering with at most `max_aniso` samples, at least 1 (`Aniso`).
    #[doc(alias = "Aniso")]
    #[must_use]
    pub fn from_aniso(max_aniso: i32) -> SamplingOptions {
        SamplingOptions {
            max_aniso: max_aniso.max(1),
            ..SamplingOptions::default()
        }
    }

    /// True if anisotropic filtering is requested (`isAniso`).
    #[doc(alias = "isAniso")]
    #[must_use]
    pub fn is_aniso(&self) -> bool {
        self.max_aniso != 0
    }
}

impl From<FilterMode> for SamplingOptions {
    // Port of: include/core/SkSamplingOptions.h#L66-L68 (chrome/m156)
    fn from(fm: FilterMode) -> SamplingOptions {
        SamplingOptions {
            filter: fm,
            mipmap: MipmapMode::None,
            ..SamplingOptions::default()
        }
    }
}

impl From<CubicResampler> for SamplingOptions {
    // Port of: include/core/SkSamplingOptions.h#L70-L72 (chrome/m156)
    fn from(cubic: CubicResampler) -> SamplingOptions {
        SamplingOptions {
            use_cubic: true,
            cubic,
            ..SamplingOptions::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_conversions() {
        let d = SamplingOptions::default();
        assert_eq!(d.filter, FilterMode::Nearest);
        assert_eq!(d.mipmap, MipmapMode::None);
        assert!(!d.use_cubic && !d.is_aniso());
        assert_eq!(SamplingOptions::from(FilterMode::Nearest), d);
        assert_eq!(
            SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest).filter,
            FilterMode::Linear
        );
        let c = SamplingOptions::from(CubicResampler::mitchell());
        assert!(c.use_cubic);
        assert_eq!(
            c.cubic,
            CubicResampler {
                b: 1.0 / 3.0,
                c: 1.0 / 3.0
            }
        );
        assert_ne!(c, d);
        assert_eq!(SamplingOptions::from_aniso(-3).max_aniso, 1);
        assert!(SamplingOptions::from_aniso(4).is_aniso());
    }
}

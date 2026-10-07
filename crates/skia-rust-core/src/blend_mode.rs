// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkBlendMode.h, src/core/SkBlendMode.cpp

//! `SkBlendMode`: the blend modes, their Porter-Duff coefficients and their names.
//!
//! The private helpers of `SkBlendModePriv.h` (raster pipeline stages, coverage, fast paths)
//! are in [`crate::blend_mode_priv`].

/// Blends are operators that take in two colors (source, destination) and return a new color.
/// Many of these operate the same on all 4 components: red, green, blue, alpha. For these, we
/// just document what happens to one component, rather than naming each one separately.
///
/// The documentation is expressed as if the component values are always 0..1 (floats).
/// `s`: source, `d`: destination, `sa`: source alpha, `da`: destination alpha; `r`: the result
/// if all 4 components are computed in the same manner, `ra`: the result alpha, `rc`: the
/// result "color" (red, green, blue).
// Port of: include/core/SkBlendMode.h#L38-L74 (chrome/m156)
#[doc(alias = "SkBlendMode")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum BlendMode {
    /// `r = 0`
    #[doc(alias = "kClear")]
    Clear = 0,
    /// `r = s`
    #[doc(alias = "kSrc")]
    Src,
    /// `r = d`
    #[doc(alias = "kDst")]
    Dst,
    /// `r = s + (1-sa)*d`
    #[doc(alias = "kSrcOver")]
    #[default]
    SrcOver,
    /// `r = d + (1-da)*s`
    #[doc(alias = "kDstOver")]
    DstOver,
    /// `r = s * da`
    #[doc(alias = "kSrcIn")]
    SrcIn,
    /// `r = d * sa`
    #[doc(alias = "kDstIn")]
    DstIn,
    /// `r = s * (1-da)`
    #[doc(alias = "kSrcOut")]
    SrcOut,
    /// `r = d * (1-sa)`
    #[doc(alias = "kDstOut")]
    DstOut,
    /// `r = s*da + d*(1-sa)`
    #[doc(alias = "kSrcATop")]
    SrcATop,
    /// `r = d*sa + s*(1-da)`
    #[doc(alias = "kDstATop")]
    DstATop,
    /// `r = s*(1-da) + d*(1-sa)`
    #[doc(alias = "kXor")]
    Xor,
    /// `r = min(s + d, 1)`
    #[doc(alias = "kPlus")]
    Plus,
    /// `r = s*d`
    #[doc(alias = "kModulate")]
    Modulate,
    /// `r = s + d - s*d`
    #[doc(alias = "kScreen")]
    Screen,
    /// Multiply or screen, depending on destination.
    #[doc(alias = "kOverlay")]
    Overlay,
    /// `rc = s + d - max(s*da, d*sa), ra = kSrcOver`
    #[doc(alias = "kDarken")]
    Darken,
    /// `rc = s + d - min(s*da, d*sa), ra = kSrcOver`
    #[doc(alias = "kLighten")]
    Lighten,
    /// Brighten destination to reflect source.
    #[doc(alias = "kColorDodge")]
    ColorDodge,
    /// Darken destination to reflect source.
    #[doc(alias = "kColorBurn")]
    ColorBurn,
    /// Multiply or screen, depending on source.
    #[doc(alias = "kHardLight")]
    HardLight,
    /// Lighten or darken, depending on source.
    #[doc(alias = "kSoftLight")]
    SoftLight,
    /// `rc = s + d - 2*(min(s*da, d*sa)), ra = kSrcOver`
    #[doc(alias = "kDifference")]
    Difference,
    /// `rc = s + d - two(s*d), ra = kSrcOver`
    #[doc(alias = "kExclusion")]
    Exclusion,
    /// `r = s*(1-da) + d*(1-sa) + s*d`
    #[doc(alias = "kMultiply")]
    Multiply,
    /// Hue of source with saturation and luminosity of destination.
    #[doc(alias = "kHue")]
    Hue,
    /// Saturation of source with hue and luminosity of destination.
    #[doc(alias = "kSaturation")]
    Saturation,
    /// Hue and saturation of source with luminosity of destination.
    #[doc(alias = "kColor")]
    Color,
    /// Luminosity of source with hue and saturation of destination.
    #[doc(alias = "kLuminosity")]
    Luminosity,
}

impl BlendMode {
    /// The last Porter-Duff blend mode (`kLastCoeffMode`).
    #[doc(alias = "kLastCoeffMode")]
    pub const LAST_COEFF_MODE: BlendMode = BlendMode::Screen;
    /// The last blend mode operating separately on components (`kLastSeparableMode`).
    #[doc(alias = "kLastSeparableMode")]
    pub const LAST_SEPARABLE_MODE: BlendMode = BlendMode::Multiply;
    /// The last valid value (`kLastMode`).
    #[doc(alias = "kLastMode")]
    pub const LAST_MODE: BlendMode = BlendMode::Luminosity;
    /// The number of blend modes (`kSkBlendModeCount`).
    // Port of: include/core/SkBlendMode.h#L76 (chrome/m156)
    #[doc(alias = "kSkBlendModeCount")]
    pub const COUNT: usize = BlendMode::LAST_MODE as usize + 1;

    /// Every blend mode, in order.
    pub const VALUES: [BlendMode; BlendMode::COUNT] = [
        BlendMode::Clear,
        BlendMode::Src,
        BlendMode::Dst,
        BlendMode::SrcOver,
        BlendMode::DstOver,
        BlendMode::SrcIn,
        BlendMode::DstIn,
        BlendMode::SrcOut,
        BlendMode::DstOut,
        BlendMode::SrcATop,
        BlendMode::DstATop,
        BlendMode::Xor,
        BlendMode::Plus,
        BlendMode::Modulate,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Multiply,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];

    /// The blend mode with discriminant `value`, if it is one (`value <= kLastMode`).
    #[must_use]
    pub fn from_i32(value: i32) -> Option<BlendMode> {
        usize::try_from(value)
            .ok()
            .and_then(|i| BlendMode::VALUES.get(i).copied())
    }

    /// For a Porter-Duff blend mode (`<= LAST_COEFF_MODE`), its source and destination
    /// coefficients; `None` for the other modes (`SkBlendMode_AsCoeff`).
    // Port of: src/core/SkBlendMode.cpp#L58-L96 (chrome/m156)
    #[doc(alias = "SkBlendMode_AsCoeff")]
    #[must_use]
    pub fn as_coeff(self) -> Option<(BlendModeCoeff, BlendModeCoeff)> {
        use BlendModeCoeff as C;
        // For Porter-Duff blend functions, color = src * src coeff + dst * dst coeff
        const COEFFS: [(BlendModeCoeff, BlendModeCoeff); 15] = [
            // src coeff, dst coeff        blend func
            (C::Zero, C::Zero), // clear
            (C::One, C::Zero),  // src
            (C::Zero, C::One),  // dst
            (C::One, C::ISA),   // src-over
            (C::IDA, C::One),   // dst-over
            (C::DA, C::Zero),   // src-in
            (C::Zero, C::SA),   // dst-in
            (C::IDA, C::Zero),  // src-out
            (C::Zero, C::ISA),  // dst-out
            (C::DA, C::ISA),    // src-atop
            (C::IDA, C::SA),    // dst-atop
            (C::IDA, C::ISA),   // xor
            (C::One, C::One),   // plus
            (C::Zero, C::SC),   // modulate
            (C::One, C::ISC),   // screen
        ];

        if self > BlendMode::Screen {
            return None;
        }
        Some(COEFFS[self as usize])
    }

    /// The name of the blend mode (`SkBlendMode_Name`), e.g. `"SrcOver"`.
    // Port of: src/core/SkBlendMode.cpp#L167-L202 (chrome/m156)
    #[doc(alias = "SkBlendMode_Name")]
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            BlendMode::Clear => "Clear",
            BlendMode::Src => "Src",
            BlendMode::Dst => "Dst",
            BlendMode::SrcOver => "SrcOver",
            BlendMode::DstOver => "DstOver",
            BlendMode::SrcIn => "SrcIn",
            BlendMode::DstIn => "DstIn",
            BlendMode::SrcOut => "SrcOut",
            BlendMode::DstOut => "DstOut",
            BlendMode::SrcATop => "SrcATop",
            BlendMode::DstATop => "DstATop",
            BlendMode::Xor => "Xor",
            BlendMode::Plus => "Plus",
            BlendMode::Modulate => "Modulate",
            BlendMode::Screen => "Screen",

            BlendMode::Overlay => "Overlay",
            BlendMode::Darken => "Darken",
            BlendMode::Lighten => "Lighten",
            BlendMode::ColorDodge => "ColorDodge",
            BlendMode::ColorBurn => "ColorBurn",
            BlendMode::HardLight => "HardLight",
            BlendMode::SoftLight => "SoftLight",
            BlendMode::Difference => "Difference",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Multiply => "Multiply",

            BlendMode::Hue => "Hue",
            BlendMode::Saturation => "Saturation",
            BlendMode::Color => "Color",
            BlendMode::Luminosity => "Luminosity",
        }
    }
}

/// For Porter-Duff blend modes (those `<= BlendMode::LAST_COEFF_MODE`), the coefficients of the
/// blend equation `dst_coeff * dst + src_coeff * src`.
// Port of: include/core/SkBlendMode.h#L84-L97 (chrome/m156)
#[doc(alias = "SkBlendModeCoeff")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum BlendModeCoeff {
    /// `0`
    #[doc(alias = "kZero")]
    Zero = 0,
    /// `1`
    #[doc(alias = "kOne")]
    One,
    /// Source color.
    #[doc(alias = "kSC")]
    SC,
    /// Inverse source color (`1 - sc`).
    #[doc(alias = "kISC")]
    ISC,
    /// Destination color.
    #[doc(alias = "kDC")]
    DC,
    /// Inverse destination color (`1 - dc`).
    #[doc(alias = "kIDC")]
    IDC,
    /// Source alpha.
    #[doc(alias = "kSA")]
    SA,
    /// Inverse source alpha (`1 - sa`).
    #[doc(alias = "kISA")]
    ISA,
    /// Destination alpha.
    #[doc(alias = "kDA")]
    DA,
    /// Inverse destination alpha (`1 - da`).
    #[doc(alias = "kIDA")]
    IDA,
}

impl BlendModeCoeff {
    /// The number of coefficients (`kCoeffCount`).
    #[doc(alias = "kCoeffCount")]
    pub const COUNT: usize = 10;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_in_order() {
        for (i, m) in BlendMode::VALUES.iter().enumerate() {
            assert_eq!(*m as usize, i);
            assert_eq!(BlendMode::from_i32(i32::try_from(i).unwrap()), Some(*m));
        }
        assert_eq!(BlendMode::COUNT, 29);
        assert_eq!(BlendMode::from_i32(29), None);
        assert_eq!(BlendMode::from_i32(-1), None);
        assert_eq!(BlendMode::default(), BlendMode::SrcOver);
    }

    #[test]
    fn coeffs() {
        use BlendModeCoeff as C;
        assert_eq!(BlendMode::SrcOver.as_coeff(), Some((C::One, C::ISA)));
        assert_eq!(BlendMode::Screen.as_coeff(), Some((C::One, C::ISC)));
        assert_eq!(BlendMode::Overlay.as_coeff(), None);
        let n = BlendMode::VALUES
            .iter()
            .filter(|m| m.as_coeff().is_some())
            .count();
        assert_eq!(n, BlendMode::LAST_COEFF_MODE as usize + 1);
    }

    #[test]
    fn names() {
        assert_eq!(BlendMode::Clear.name(), "Clear");
        assert_eq!(BlendMode::SrcATop.name(), "SrcATop");
        assert_eq!(BlendMode::Luminosity.name(), "Luminosity");
        for m in BlendMode::VALUES {
            // The names are the enum's variant names.
            assert_eq!(m.name(), format!("{m:?}"));
        }
    }
}
